mod coordinator;
mod dataflows;
mod external;
mod mock;
mod models;
mod runtime;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    let runtime = runtime::RuntimeManager::new();
    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/system/status", get(system_status))
        .route("/api/dataflows", get(dataflows))
        .route("/api/dataflows/:id/definition", get(dataflow_definition))
        .route("/api/dataflows/:id/nodes", get(dataflow_nodes))
        .route("/api/dataflows/:id/logs", get(dataflow_logs))
        .route("/api/dataflows/:id/graph", get(dataflow_graph))
        .route("/api/dataflows/:id/start", post(dataflow_start))
        .route("/api/dataflows/:id/stop", post(dataflow_stop))
        .route("/api/dataflows/:id/restart", post(dataflow_restart))
        .route("/api/runtime/status", get(runtime_status))
        .route("/api/runtime/logs", get(runtime_logs))
        .route("/api/runtime/start", post(runtime_start))
        .route("/api/runtime/stop", post(runtime_stop))
        .route("/api/coordinator/status", get(coordinator_status))
        .route("/api/dviz/status", get(dviz_status))
        .route("/api/moveit/status", get(moveit_status))
        .with_state(runtime)
        .layer(CorsLayer::permissive());

    let bind_addr =
        std::env::var("DORA_STUDIO_BACKEND_ADDR").unwrap_or_else(|_| "127.0.0.1:3001".to_string());
    let addr = bind_addr.parse().expect("valid bind address");
    println!("dora-studio backend listening on http://{addr}");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server failed");
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl From<dataflows::DataflowError> for ApiError {
    fn from(error: dataflows::DataflowError) -> Self {
        match error {
            dataflows::DataflowError::NotFound(message) => Self {
                status: StatusCode::NOT_FOUND,
                message,
            },
            dataflows::DataflowError::Invalid(message) => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                message,
            },
            dataflows::DataflowError::Io(message) => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message,
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(models::ApiError {
                error: self.message,
            }),
        )
            .into_response()
    }
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

async fn system_status() -> Json<models::SystemStatus> {
    let coordinator = coordinator::query_coordinator().await;
    if coordinator.connected {
        Json(models::SystemStatus {
            coordinator: "connected".to_string(),
            daemon: "healthy".to_string(),
            version: coordinator.version,
            running_dataflows: coordinator.running_dataflows,
            active_nodes: coordinator.active_nodes,
            error_count: 0,
        })
    } else {
        Json(mock::system_status())
    }
}

async fn coordinator_status() -> Json<models::CoordinatorStatus> {
    Json(coordinator::query_coordinator().await)
}

async fn dviz_status() -> Json<models::DvizStatus> {
    Json(external::query_dviz())
}

async fn moveit_status() -> Json<models::MoveitStatus> {
    let mut status = external::query_moveit();

    // Cross-reference with running dataflows: check if moveit nodes are active
    let coordinator = coordinator::query_coordinator().await;
    for df in &coordinator.dataflows {
        if df.name.contains("moveit") || df.name.contains("motion") {
            status.running = df.status == "running";
            if status.running {
                status.message = format!(
                    "dora-moveit2 dataflow '{}' is running with {} nodes.",
                    df.name, df.nodes
                );
            }
        }
    }

    Json(status)
}

async fn dataflows() -> Result<Json<Vec<models::DataflowSummary>>, ApiError> {
    dataflows::list_dataflows()
        .map(Json)
        .map_err(ApiError::from)
}

async fn dataflow_definition(
    Path(id): Path<String>,
) -> Result<Json<models::DataflowDefinition>, ApiError> {
    dataflows::load_definition(&id)
        .map(Json)
        .map_err(ApiError::from)
}

async fn dataflow_nodes(
    Path(id): Path<String>,
    State(runtime): State<runtime::RuntimeHandle>,
) -> Result<Json<Vec<models::NodeMetrics>>, ApiError> {
    let mut metrics = dataflows::nodes(&id).map_err(ApiError::from)?;
    let state = runtime.status().await;
    if state.status == "running" && state.dataflow_id.as_deref() == Some(&id) {
        for node in &mut metrics {
            node.status = "running".to_string();
            match node.id.as_str() {
                "camera" => {
                    node.cpu = 18;
                    node.memory = 164;
                    node.pending = 3;
                }
                "detector" => {
                    node.status = "degraded".to_string();
                    node.cpu = 61;
                    node.memory = 512;
                    node.restarts = 1;
                    node.pending = 17;
                }
                "planner" => {
                    node.cpu = 22;
                    node.memory = 210;
                    node.pending = 5;
                }
                "logger" => {
                    node.cpu = 12;
                    node.memory = 340;
                    node.pending = 7;
                }
                "robot_bridge" => {
                    node.cpu = 7;
                    node.memory = 86;
                    node.pending = 0;
                }
                _ => {}
            }
        }
    }
    Ok(Json(metrics))
}

async fn dataflow_logs(Path(_id): Path<String>) -> Json<Vec<models::LogEntry>> {
    Json(mock::logs())
}

async fn dataflow_graph(Path(id): Path<String>) -> Result<Json<models::DataflowGraph>, ApiError> {
    dataflows::graph(&id).map(Json).map_err(ApiError::from)
}

async fn runtime_status(
    State(runtime): State<runtime::RuntimeHandle>,
) -> Json<models::RuntimeState> {
    Json(runtime.status().await)
}

async fn runtime_logs(
    State(runtime): State<runtime::RuntimeHandle>,
) -> Json<Vec<models::LogEntry>> {
    Json(runtime.logs().await)
}

async fn runtime_start(
    State(runtime): State<runtime::RuntimeHandle>,
) -> Json<models::RuntimeState> {
    Json(runtime.start().await)
}

async fn runtime_stop(State(runtime): State<runtime::RuntimeHandle>) -> Json<models::RuntimeState> {
    Json(runtime.stop().await)
}

async fn dataflow_start(
    Path(id): Path<String>,
    State(runtime): State<runtime::RuntimeHandle>,
) -> Result<Json<models::RuntimeState>, ApiError> {
    let dataflow = dataflows::resolve_dataflow(&id).map_err(ApiError::from)?;
    Ok(Json(
        runtime
            .start_dataflow(dataflow.id, dataflow.path, dataflow.relative_path)
            .await,
    ))
}

async fn dataflow_stop(
    Path(id): Path<String>,
    State(runtime): State<runtime::RuntimeHandle>,
) -> Result<Json<models::RuntimeState>, ApiError> {
    dataflows::resolve_dataflow(&id).map_err(ApiError::from)?;
    Ok(Json(runtime.stop().await))
}

async fn dataflow_restart(
    Path(id): Path<String>,
    State(runtime): State<runtime::RuntimeHandle>,
) -> Result<Json<models::RuntimeState>, ApiError> {
    let dataflow = dataflows::resolve_dataflow(&id).map_err(ApiError::from)?;
    runtime.stop().await;
    Ok(Json(
        runtime
            .start_dataflow(dataflow.id, dataflow.path, dataflow.relative_path)
            .await,
    ))
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
