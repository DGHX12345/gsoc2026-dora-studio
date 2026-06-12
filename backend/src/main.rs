mod dataflows;
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
        .route("/api/runtime/status", get(runtime_status))
        .route("/api/runtime/logs", get(runtime_logs))
        .route("/api/runtime/start", post(runtime_start))
        .route("/api/runtime/stop", post(runtime_stop))
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
    Json(mock::system_status())
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
) -> Result<Json<Vec<models::NodeMetrics>>, ApiError> {
    dataflows::nodes(&id).map(Json).map_err(ApiError::from)
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

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
