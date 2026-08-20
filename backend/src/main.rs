mod attribution;
mod compat_engine;
mod coordinator;
mod coordinator_ws;
mod dataflow_builder;
mod dataflows;
mod dora_env;
mod drec;
mod external;
mod lerobot;
mod live;
mod metrics;
mod model_catalog;
mod models;
mod monitoring;
mod otel;
mod otlp;
mod otlp_grpc;
mod profile;
mod project_scan;
mod protocol;
mod recording;
mod runtime;
mod schema_registry;
mod session;
mod urn_catalog;
mod validate;

use std::{path::PathBuf, sync::Arc};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use tower_http::{cors::CorsLayer, services::ServeDir};

struct AppState {
    runtime: runtime::RuntimeHandle,
    session: session::SessionHandle,
    schemas: schema_registry::SchemaRegistry,
    ws_client: coordinator_ws::CoordinatorWsClient,
    recordings: drec::service::RecordingManager,
    recording: recording::RecordingHandle,
    monitoring: monitoring::MonitoringController,
    profiles: profile::ProfileManager,
    live: live::LiveFeed,
    catalog: urn_catalog::Catalog,
}

#[tokio::main]
async fn main() {
    let ws_client = coordinator_ws::CoordinatorWsClient::new();

    // Best-effort connect to coordinator WebSocket (non-blocking)
    let ws_connect_handle = {
        let ws = ws_client.clone();
        tokio::spawn(async move {
            if let Err(e) = ws.connect().await {
                eprintln!("coordinator WebSocket unavailable (CLI fallback active): {e}");
            } else {
                eprintln!("coordinator WebSocket connected");
            }
        })
    };

    // Monitoring collectors start STOPPED (M11.5 D1): diagnostics are opt-in,
    // enabled through /api/monitoring/toggle. No polling at boot.
    // D2: node metrics poll the coordinator WebSocket first, falling back to
    // `dora node list` per attempt when the WS is unavailable.
    let metrics_collector = metrics::MetricsCollector::new_with_ws(
        std::time::Duration::from_secs(2),
        ws_client.clone(),
    );

    // OTel trace backend (Jaeger-compatible API). Default: local Jaeger.
    let otel_endpoint = std::env::var("DORA_OTEL_QUERY_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:16686".to_string());
    let otel_collector = otel::OtelCollector::new(otel_endpoint);

    let state = Arc::new(AppState {
        runtime: runtime::RuntimeManager::new(),
        session: session::DoraSessionManager::new(),
        schemas: schema_registry::SchemaRegistry::new(),
        ws_client,
        recordings: drec::service::RecordingManager::new(),
        recording: recording::RecordingController::new(),
        monitoring: monitoring::MonitoringController::new(metrics_collector, otel_collector),
        profiles: profile::ProfileManager::new(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../profiles"),
        ),
        live: live::LiveFeed::new(),
        catalog: urn_catalog::Catalog::new(),
    });

    // Wait for WS connect attempt to settle (up to 3s) before starting server
    let _ = tokio::time::timeout(std::time::Duration::from_secs(3), ws_connect_handle).await;
    let models_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../models");
    let app = Router::new()
        .nest_service("/models", ServeDir::new(models_dir.clone()))
        .route("/api/models", get(available_models))
        .route("/api/types/catalog", get(types_catalog))
        // Wildcard capture so multi-segment URNs (e.g. std/media/v1/Image)
        // resolve as a single parameter; axum 0.6 `:param` only matches one
        // segment and would reject URNs containing `/`.
        .route("/api/types/*urn", get(types_get))
        .route("/api/health", get(health))
        .route("/api/system/status", get(system_status))
        .route("/api/dataflows", get(dataflows))
        // Static route registered before the `:id` parameter routes so
        // "save-as" never resolves as a dataflow id.
        .route("/api/dataflows/save-as", post(dataflow_save_as))
        .route("/api/dataflows/:id/save", post(dataflow_save))
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
        .route("/api/runtime/start-path", post(runtime_start_path))
        .route("/api/runtime/stop", post(runtime_stop))
        .route("/api/runtime/nodes/:dataflow_id", get(runtime_nodes))
        .route(
            "/api/runtime/nodes/:dataflow_id/reload",
            post(runtime_reload),
        )
        .route("/api/coordinator/status", get(coordinator_status))
        .route("/api/dora/versions", get(dora_versions))
        .route("/api/dora/switch", post(dora_switch))
        .route("/api/dora/candidates/add", post(dora_candidates_add))
        .route("/api/dora/candidates/delete", post(dora_candidates_delete))
        .route("/api/session/status", get(session_status))
        .route("/api/session/start", post(session_start))
        .route("/api/session/stop", post(session_stop))
        .route("/api/daemon/status", get(daemon_status))
        .route("/api/daemon/start", post(daemon_start))
        .route("/api/daemon/stop", post(daemon_stop))
        .route("/api/dviz/status", get(dviz_status))
        .route("/api/dviz/topics", get(dviz_topics))
        .route("/api/dviz/displays", get(dviz_displays))
        .route("/api/dviz/snapshot", get(dviz_snapshot))
        .route("/api/robot/profile", get(robot_profile))
        .route("/api/moveit/status", get(moveit_status))
        .route("/api/moveit/snapshot", get(moveit_snapshot))
        .route("/api/dataflow/build", post(dataflow_build))
        .route("/api/dataflow/validate", post(dataflow_validate))
        .route("/api/dataflow/parse", post(dataflow_parse))
        .route("/api/dataflow/run", post(dataflow_run))
        .route("/api/schema/check", post(schema_check))
        .route("/api/schema/operator/:name", get(schema_operator))
        .route("/api/projects/list", get(projects_list))
        .route("/api/projects/add", post(projects_add))
        .route("/api/projects/delete", post(projects_delete))
        .route("/api/projects/nodes", post(projects_nodes))
        .route("/api/palette", get(palette))
        .route("/api/metrics/nodes", get(metrics_nodes))
        .route("/api/metrics/nodes/:id/history", get(metrics_node_history))
        .route("/api/otel/status", get(otel_status))
        .route("/api/otel/spans", get(otel_spans))
        .route("/api/otel/trace/:trace_id", get(otel_trace))
        .route("/api/monitoring/status", get(monitoring_status))
        .route("/api/monitoring/toggle", post(monitoring_toggle))
        .route("/api/recording/capture", post(recording_capture))
        .route("/api/recording/stop", post(recording_stop))
        .route("/api/recording/list", get(recording_list))
        .route("/api/recording/open", post(recording_open))
        .route("/api/recording/:id/streams", get(recording_streams))
        .route("/api/recording/:id/seek", get(recording_seek))
        .route("/api/recording/:id/entries", get(recording_entries))
        .route("/api/recording/:id/close", post(recording_close))
        .route("/api/recording/:id/attribution", get(recording_attribution))
        .route(
            "/api/recording/:id/attribution/chain",
            get(recording_attribution_chain),
        )
        .route("/api/lerobot/status", get(lerobot_status))
        .route("/api/lerobot/scan", post(lerobot_scan))
        .route("/api/lerobot/frames", post(lerobot_frames))
        .route("/api/lerobot/profiles", get(lerobot_profiles))
        .route("/api/lerobot/autodetect", post(lerobot_autodetect))
        .route("/api/lerobot/attribution", post(lerobot_attribution))
        .route("/api/live/ingest", post(live_ingest))
        .route("/api/live/recent", get(live_recent))
        .route("/api/live/command", post(live_command))
        .route("/api/live/command", get(live_command_queue))
        .with_state(state.clone())
        .layer(CorsLayer::permissive());

    let bind_addr =
        std::env::var("DORA_STUDIO_BACKEND_ADDR").unwrap_or_else(|_| "127.0.0.1:3001".to_string());
    let addr = bind_addr.parse().expect("valid bind address");
    println!("dora-studio backend listening on http://{addr}");

    // OTLP receiver (M11.5 D3): passive listener on 4318. dora nodes push
    // spans here via OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318.
    // Not fatal if the port is taken — the main app keeps serving.
    let otlp_bind =
        std::env::var("DORA_STUDIO_OTLP_ADDR").unwrap_or_else(|_| "127.0.0.1:4318".to_string());
    let otlp_collector = state.monitoring.otel.clone();
    let otlp_task = tokio::spawn(async move {
        let otlp_addr = match otlp_bind.parse::<std::net::SocketAddr>() {
            Ok(addr) => addr,
            Err(e) => {
                eprintln!("OTLP receiver disabled (invalid DORA_STUDIO_OTLP_ADDR): {e}");
                return;
            }
        };
        let app = otlp::receiver_router(otlp_collector);
        println!("OTLP receiver listening on http://{otlp_addr}");
        if let Err(e) = axum::Server::bind(&otlp_addr)
            .serve(app.into_make_service())
            .await
        {
            eprintln!("OTLP receiver stopped: {e}");
        }
    });

    // OTLP gRPC receiver (M15.6): dora pushes spans/metrics via
    // DORA_OTLP_ENDPOINT (gRPC). Not fatal if the port is taken.
    let grpc_bind = std::env::var("DORA_STUDIO_OTLP_GRPC_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:4317".to_string());
    let grpc_collector = state.monitoring.otel.clone();
    let grpc_task = tokio::spawn(async move {
        let grpc_addr = match grpc_bind.parse::<std::net::SocketAddr>() {
            Ok(addr) => addr,
            Err(e) => {
                eprintln!("OTLP gRPC receiver disabled (invalid DORA_STUDIO_OTLP_GRPC_ADDR): {e}");
                return;
            }
        };
        let svc = otlp_grpc::service(grpc_collector);
        println!("OTLP gRPC receiver listening on http://{grpc_addr}");
        if let Err(e) = tonic::transport::Server::builder()
            .add_service(svc)
            .serve(grpc_addr)
            .await
        {
            eprintln!("OTLP gRPC receiver stopped: {e:?}");
        }
    });

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server failed");
    otlp_task.abort();
    grpc_task.abort();
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

async fn types_catalog(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "types": state.catalog.entries() }))
}

async fn types_get(
    State(state): State<Arc<AppState>>,
    Path(urn): Path<String>,
) -> Result<Json<urn_catalog::TypeDef>, ApiError> {
    state.catalog.resolve(&urn).map(Json).ok_or(ApiError {
        status: StatusCode::NOT_FOUND,
        message: format!("Unknown type URN: {urn}"),
    })
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
        Json(models::SystemStatus {
            coordinator: "unavailable".to_string(),
            daemon: "unavailable".to_string(),
            version: String::new(),
            running_dataflows: 0,
            active_nodes: 0,
            error_count: 0,
        })
    }
}

async fn coordinator_status() -> Json<models::CoordinatorStatus> {
    Json(coordinator::query_coordinator().await)
}

// --- dora version manager (M17) ---

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DoraVersionsResponse {
    active: String,
    overridden_by_env: bool,
    items: Vec<dora_env::DoraVersionItem>,
}

async fn dora_versions() -> Json<DoraVersionsResponse> {
    Json(DoraVersionsResponse {
        active: dora_env::resolve_dora_bin(),
        overridden_by_env: dora_env::env_bin_overrides(),
        items: dora_env::detect_versions().await,
    })
}

#[derive(serde::Deserialize)]
struct DoraPathRequest {
    path: String,
}

fn dora_path_error(message: String) -> ApiError {
    ApiError {
        status: StatusCode::BAD_REQUEST,
        message,
    }
}

async fn dora_switch(
    Json(req): Json<DoraPathRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    dora_env::switch_dora_bin(req.path)
        .await
        .map_err(dora_path_error)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn dora_candidates_add(
    Json(req): Json<DoraPathRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    dora_env::add_candidate(req.path).map_err(dora_path_error)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn dora_candidates_delete(
    Json(req): Json<DoraPathRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    dora_env::delete_candidate(req.path).map_err(dora_path_error)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn dviz_status() -> Json<models::DvizStatus> {
    Json(external::query_dviz())
}

/// M13 D6: locally available robot models (URDF directories under
/// models/) for the tool panel's model selector.
async fn available_models() -> Json<models::AvailableModelsResponse> {
    let models_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../models");
    Json(models::AvailableModelsResponse {
        models: model_catalog::list_available_models(&models_dir)
            .into_iter()
            .map(|m| models::AvailableModel {
                id: m.id,
                urdf_path: m.urdf_path,
                mesh_base_path: m.mesh_base_path,
            })
            .collect(),
    })
}

async fn dviz_topics() -> Json<models::DvizTopicsResponse> {
    Json(external::query_dviz_topics())
}

async fn dviz_displays() -> Json<models::DvizDisplaysResponse> {
    Json(external::query_dviz_displays())
}

async fn dviz_snapshot() -> Json<models::DvizSnapshotResponse> {
    Json(external::query_dviz_snapshot())
}

async fn robot_profile() -> Json<models::RobotProfileResponse> {
    Json(external::query_robot_profile())
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

async fn moveit_snapshot() -> Json<models::MoveitSnapshotResponse> {
    Json(external::query_moveit_snapshot())
}

async fn dataflows(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<models::DataflowSummary>>, ApiError> {
    let mut dataflows = project_scan::list_all_dataflows().map_err(ApiError::from)?;
    let rt = state.runtime.status().await;
    let coord = coordinator::query_coordinator().await;

    for df in &mut dataflows {
        if rt.status == "running" && rt.dataflow_id.as_deref() == Some(&df.id) {
            df.status = "running".to_string();
        } else if coord.connected {
            for cdf in &coord.dataflows {
                if cdf.status == "running"
                    && (df.name.contains(&cdf.name) || cdf.name.contains(&df.name))
                {
                    df.status = "running".to_string();
                }
            }
        }
    }
    Ok(Json(dataflows))
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
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<models::NodeMetrics>>, ApiError> {
    let mut metrics = dataflows::nodes(&id).map_err(ApiError::from)?;
    let state = state.runtime.status().await;
    if state.status == "running" && state.dataflow_id.as_deref() == Some(&id) {
        for node in &mut metrics {
            node.status = "running".to_string();
        }
    }
    Ok(Json(metrics))
}

async fn dataflow_logs(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Json<Vec<models::LogEntry>> {
    let rt = state.runtime.status().await;
    if rt.status == "running" && rt.dataflow_id.as_deref() == Some(&id) {
        Json(state.runtime.logs().await)
    } else {
        Json(Vec::new())
    }
}

async fn dataflow_graph(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<models::DataflowGraph>, ApiError> {
    let mut graph = dataflows::graph(&id).map_err(ApiError::from)?;
    let state = state.runtime.status().await;
    if state.status == "running" && state.dataflow_id.as_deref() == Some(&id) {
        for node in &mut graph.nodes {
            node.status = "running".to_string();
        }
    }
    Ok(Json(graph))
}

async fn runtime_status(State(state): State<Arc<AppState>>) -> Json<models::RuntimeState> {
    Json(state.runtime.status().await)
}

async fn runtime_logs(State(state): State<Arc<AppState>>) -> Json<Vec<models::LogEntry>> {
    Json(state.runtime.logs().await)
}

async fn session_status(State(state): State<Arc<AppState>>) -> Json<session::SessionStatus> {
    Json(state.session.status().await)
}

async fn session_start(State(state): State<Arc<AppState>>) -> Json<session::SessionStatus> {
    Json(state.session.start().await)
}

async fn session_stop(State(state): State<Arc<AppState>>) -> Json<session::SessionStatus> {
    Json(state.session.stop().await)
}

async fn daemon_status(State(state): State<Arc<AppState>>) -> Json<session::LegacyDaemonStatus> {
    Json(session::legacy_daemon_status(&state.session.status().await))
}

async fn daemon_start(State(state): State<Arc<AppState>>) -> Json<session::LegacyDaemonStatus> {
    Json(session::legacy_daemon_status(&state.session.start().await))
}

async fn daemon_stop(State(state): State<Arc<AppState>>) -> Json<session::LegacyDaemonStatus> {
    Json(session::legacy_daemon_status(&state.session.stop().await))
}

async fn runtime_start(State(state): State<Arc<AppState>>) -> Json<models::RuntimeState> {
    Json(state.runtime.start().await)
}

#[derive(serde::Deserialize)]
struct RuntimeStartPathRequest {
    path: String,
}

/// Start a dataflow by arbitrary YAML path (not necessarily one of the
/// discovered example dataflows).
async fn runtime_start_path(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RuntimeStartPathRequest>,
) -> Result<Json<models::RuntimeState>, ApiError> {
    let path = req.path.trim();
    if path.is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "missing 'path' field".to_string(),
        });
    }
    let source = std::path::Path::new(path);
    let stem = source
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| "custom-dataflow".to_string());
    let parent = source
        .parent()
        .and_then(|parent| parent.file_name())
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    // Parent dir + file stem keeps names unique across folders full of
    // dataflow.yml files.
    let id = if parent.is_empty() {
        stem
    } else {
        format!("{parent}-{stem}")
    };
    Ok(Json(
        state
            .runtime
            .start_dataflow(id, PathBuf::from(path), path.to_string())
            .await,
    ))
}

async fn runtime_stop(State(state): State<Arc<AppState>>) -> Json<models::RuntimeState> {
    Json(state.runtime.stop().await)
}

async fn dataflow_start(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<models::RuntimeState>, ApiError> {
    let dataflow = dataflows::resolve_dataflow(&id).map_err(ApiError::from)?;
    Ok(Json(
        state
            .runtime
            .start_dataflow(dataflow.id, dataflow.path, dataflow.relative_path)
            .await,
    ))
}

async fn dataflow_stop(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<models::RuntimeState>, ApiError> {
    dataflows::resolve_dataflow(&id).map_err(ApiError::from)?;
    Ok(Json(state.runtime.stop().await))
}

async fn dataflow_restart(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<models::RuntimeState>, ApiError> {
    let dataflow = dataflows::resolve_dataflow(&id).map_err(ApiError::from)?;
    state.runtime.stop().await;
    Ok(Json(
        state
            .runtime
            .start_dataflow(dataflow.id, dataflow.path, dataflow.relative_path)
            .await,
    ))
}

async fn dataflow_build(
    Json(graph): Json<dataflow_builder::DataflowGraph>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut builder = dataflow_builder::DataflowBuilder::new();
    for node in graph.nodes {
        builder.add_node(node).map_err(|e| ApiError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: e.to_string(),
        })?;
    }
    for edge in graph.edges {
        builder.connect(edge).map_err(|e| ApiError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: e.to_string(),
        })?;
    }
    let yaml = builder.to_yaml();
    Ok(Json(serde_json::json!({
        "yaml": yaml,
        "node_count": builder.graph().nodes.len(),
        "edge_count": builder.graph().edges.len(),
    })))
}

async fn dataflow_validate(
    Json(graph): Json<dataflow_builder::DataflowGraph>,
) -> Json<serde_json::Value> {
    let mut builder = dataflow_builder::DataflowBuilder::new();
    let mut errors = Vec::new();
    for node in graph.nodes {
        if let Err(e) = builder.add_node(node) {
            errors.push(e.to_string());
        }
    }
    for edge in graph.edges {
        if let Err(e) = builder.connect(edge) {
            errors.push(e.to_string());
        }
    }
    match builder.validate() {
        Ok(()) if errors.is_empty() => Json(serde_json::json!({ "valid": true, "errors": [] })),
        Ok(()) => Json(serde_json::json!({ "valid": false, "errors": errors })),
        Err(mut validation_errors) => {
            for e in validation_errors {
                errors.push(e.to_string());
            }
            Json(serde_json::json!({ "valid": false, "errors": errors }))
        }
    }
}

async fn dataflow_parse(
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let yaml = body.get("yaml").and_then(|v| v.as_str()).ok_or(ApiError {
        status: StatusCode::BAD_REQUEST,
        message: "missing 'yaml' field".to_string(),
    })?;
    let builder = dataflow_builder::DataflowBuilder::from_yaml(yaml).map_err(|e| ApiError {
        status: StatusCode::UNPROCESSABLE_ENTITY,
        message: e.to_string(),
    })?;
    Ok(Json(serde_json::json!({
        "graph": builder.graph(),
    })))
}

// --- Save API (M18 Task 3.6) ---

/// Copy the current dataflow file into ~/.config/dora-studio/backups
/// before a write-back, so a broken save can be restored manually.
fn backup_file(path: &std::path::Path) -> Result<PathBuf, String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let backups = std::path::Path::new(&home).join(".config/dora-studio/backups");
    std::fs::create_dir_all(&backups).map_err(|error| format!("backup dir: {error}"))?;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "dataflow.yml".to_string());
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let backup = backups.join(format!("{name}.{ts}.bak"));
    std::fs::copy(path, &backup).map_err(|error| format!("backup: {error}"))?;
    Ok(backup)
}

/// Validate content against `dora validate`, then write it to `target`.
/// Validation errors return 422 with the full SaveResponse JSON body;
/// warnings (and the "validation skipped" notice on dora 0.x) are
/// non-blocking and returned with the successful save.
async fn finish_save(
    target: &std::path::Path,
    content: &str,
    display_path: &str,
) -> Result<Json<models::SaveResponse>, ApiError> {
    // Stage the temp file in the target's directory so the final rename is
    // a same-filesystem atomic replace (spec §4 step 6: a crash mid-write
    // must not corrupt the dataflow file). Fall back to the system temp dir
    // when the target has no parent directory.
    let tmp_dir = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(|parent| parent.to_path_buf())
        .unwrap_or_else(std::env::temp_dir);
    let tmp = tmp_dir.join(format!("dora-studio-save-{}.yml", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, content).map_err(|error| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to write temp file: {error}"),
    })?;
    let outcome = match validate::validate_yaml(&tmp).await {
        Ok(outcome) => outcome,
        Err(skipped) => validate::ValidateOutcome {
            errors: Vec::new(),
            warnings: vec![models::SaveIssue {
                node_id: None,
                port_id: None,
                message: skipped,
            }],
        },
    };
    if !outcome.errors.is_empty() {
        let _ = std::fs::remove_file(&tmp);
        return Err(ApiError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: serde_json::to_string(&models::SaveResponse {
                ok: false,
                path: display_path.to_string(),
                warnings: outcome.warnings,
                errors: outcome.errors,
            })
            .unwrap_or_else(|_| "validate failed".to_string()),
        });
    }
    // The temp file already holds `content`; rename it into place instead
    // of re-writing the target (atomic on the same filesystem).
    std::fs::rename(&tmp, target).map_err(|error| {
        let _ = std::fs::remove_file(&tmp);
        ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("failed to write {display_path}: {error}"),
        }
    })?;
    Ok(Json(models::SaveResponse {
        ok: true,
        path: display_path.to_string(),
        warnings: outcome.warnings,
        errors: Vec::new(),
    }))
}

/// Write-back: patch an existing discovered dataflow in place.
async fn dataflow_save(
    Path(id): Path<String>,
    Json(req): Json<models::SaveRequest>,
) -> Result<Json<models::SaveResponse>, ApiError> {
    let file = dataflows::resolve_dataflow(&id).map_err(ApiError::from)?;
    let original = std::fs::read_to_string(&file.path).map_err(|error| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("Failed to read {}: {error}", file.relative_path),
    })?;
    let patched =
        dataflow_builder::patch_yaml(&original, &req.graph).map_err(|error| ApiError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: format!("Failed to build YAML: {error}"),
        })?;
    backup_file(&file.path).map_err(|error| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: error,
    })?;
    finish_save(&file.path, &patched, &file.relative_path).await
}

/// Save-as: generate a fresh dora 1.0 dataflow YAML at an arbitrary path.
async fn dataflow_save_as(
    Json(req): Json<models::SaveAsRequest>,
) -> Result<Json<models::SaveResponse>, ApiError> {
    let target = PathBuf::from(req.target_path.trim());
    if target.as_os_str().is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "missing 'targetPath' field".to_string(),
        });
    }
    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|error| ApiError {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                message: format!("cannot create target directory: {error}"),
            })?;
        }
    }
    let mut builder = dataflow_builder::DataflowBuilder::new();
    builder.type_rules = req.graph.type_rules.clone();
    for node in req.graph.nodes {
        builder.add_node(node).map_err(|error| ApiError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: error.to_string(),
        })?;
    }
    for edge in req.graph.edges {
        builder.connect(edge).map_err(|error| ApiError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: error.to_string(),
        })?;
    }
    let yaml = builder.to_yaml();
    // Back up an existing target before overwriting it, matching the
    // write-back precedent.
    if target.exists() {
        backup_file(&target).map_err(|error| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error,
        })?;
    }
    let display = target.to_string_lossy().to_string();
    finish_save(&target, &yaml, &display).await
}

async fn dataflow_run(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<models::RuntimeState>, ApiError> {
    let yaml = body.get("yaml").and_then(|v| v.as_str()).ok_or(ApiError {
        status: StatusCode::BAD_REQUEST,
        message: "missing 'yaml' field".to_string(),
    })?;
    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("studio-dataflow");

    // Stop any existing run first
    if state.runtime.status().await.status == "running" {
        state.runtime.stop().await;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    let rt_state = state.runtime.run_yaml(yaml, name).await;
    Ok(Json(rt_state))
}

async fn schema_check(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Json<models::SchemaCheckResponse> {
    // New shape: {source_urn, sink_urn, type_rules?}
    let source_urn = body
        .get("source_urn")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let sink_urn = body
        .get("sink_urn")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    if source_urn.is_some() || sink_urn.is_some() {
        let user_rules: Vec<(String, String)> = body
            .get("type_rules")
            .and_then(|v| v.as_array())
            .map(|rules| {
                rules
                    .iter()
                    .filter_map(|rule| {
                        Some((
                            rule.get("from")?.as_str()?.to_string(),
                            rule.get("to")?.as_str()?.to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let result = compat_engine::check(source_urn, sink_urn, &user_rules);
        // enrich struct-mismatch reason via catalog (informational only, appended on mismatches)
        let detail = match (&source_urn, &sink_urn) {
            (Some(from), Some(to)) if !result.compatible => {
                enrich_struct_detail(from, to, &state.catalog)
                    .map(|detail| format!("{} ({detail})", result.reason))
                    .unwrap_or_else(|| result.reason.clone())
            }
            _ => result.reason.clone(),
        };
        return Json(models::SchemaCheckResponse {
            compatible: result.compatible,
            level: result.level,
            detail,
            urn: sink_urn.map(str::to_string),
            rule: result
                .rule
                .map(|(from, to)| models::TypeRuleDef { from, to }),
            suggestion: result.suggestion,
        });
    }

    // Old shape: {source_operator, source_port, sink_operator, sink_port}
    let request = schema_registry::CheckRequest {
        source_operator: body
            .get("source_operator")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        source_port: body
            .get("source_port")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        sink_operator: body
            .get("sink_operator")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        sink_port: body
            .get("sink_port")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
    };
    let response = state.schemas.check(&request);
    Json(models::SchemaCheckResponse {
        compatible: response.compatible,
        level: response.level,
        detail: response.detail,
        urn: None,
        rule: None,
        suggestion: None,
    })
}

fn enrich_struct_detail(from: &str, to: &str, catalog: &urn_catalog::Catalog) -> Option<String> {
    let from_def = catalog.resolve(from)?;
    let to_def = catalog.resolve(to)?;
    if from_def.fields.is_empty() || to_def.fields.is_empty() {
        return None;
    }
    let expected: Vec<compat_engine::TypeField> = to_def
        .fields
        .iter()
        .map(|field| compat_engine::TypeField {
            name: field.name.clone(),
            field_type: field.field_type.clone(),
        })
        .collect();
    let actual: Vec<compat_engine::TypeField> = from_def
        .fields
        .iter()
        .map(|field| compat_engine::TypeField {
            name: field.name.clone(),
            field_type: field.field_type.clone(),
        })
        .collect();
    match compat_engine::schema_compatible(&expected, &actual) {
        Ok(()) => None,
        Err(error) => Some(error.to_string()),
    }
}

async fn schema_operator(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<schema_registry::OperatorSchemas>, ApiError> {
    state
        .schemas
        .operator_schemas(&name)
        .map(Json)
        .ok_or(ApiError {
            status: StatusCode::NOT_FOUND,
            message: format!("Operator '{}' not found in schema registry", name),
        })
}

/// GET /api/projects/list — builtin + configured project dirs.
async fn projects_list() -> Result<Json<models::ProjectListResponse>, ApiError> {
    project_scan::list_projects()
        .map(|projects| Json(models::ProjectListResponse { projects }))
        .map_err(ApiError::from)
}

/// POST /api/projects/add — persist a user project dir (canonical, deduped).
async fn projects_add(
    Json(req): Json<models::AddProjectRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    dora_env::add_project_dir(&req.path).map_err(|error| ApiError {
        status: StatusCode::UNPROCESSABLE_ENTITY,
        message: error,
    })?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /api/projects/delete — remove a user project dir from settings.
async fn projects_delete(Json(req): Json<models::AddProjectRequest>) -> Json<serde_json::Value> {
    let _ = dora_env::remove_project_dir(&req.path);
    Json(serde_json::json!({ "ok": true }))
}

/// POST /api/projects/nodes — store a manually defined node for the palette.
async fn projects_nodes(
    Json(req): Json<models::ManualNodeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let node = dora_env::ManualNode {
        id: req.id,
        path: req.path,
        description: req.description,
        inputs: req
            .inputs
            .into_iter()
            .map(|port| dora_env::ManualPort {
                name: port.name,
                urn: port.urn,
            })
            .collect(),
        outputs: req
            .outputs
            .into_iter()
            .map(|port| dora_env::ManualPort {
                name: port.name,
                urn: port.urn,
            })
            .collect(),
    };
    dora_env::add_manual_node(node).map_err(|error| ApiError {
        status: StatusCode::UNPROCESSABLE_ENTITY,
        message: error,
    })?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /api/palette — aggregated cross-project node palette (scan of
/// builtin examples + configured project dirs, merged with manual nodes).
async fn palette() -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        serde_json::json!({ "entries": project_scan::palette() }),
    ))
}

/// GET /api/runtime/nodes/:dataflow_id — per-node runtime status.
async fn runtime_nodes(
    Path(dataflow_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Json<Vec<models::NodeRuntimeStatus>> {
    // Try WebSocket first
    if state.ws_client.is_connected().await {
        match state.ws_client.node_statuses(&dataflow_id).await {
            Ok(statuses) => return Json(statuses),
            Err(e) => eprintln!("WS node_statuses failed: {e}"),
        }
    }

    // CLI fallback: query dora list for dataflow-level status
    let coord = coordinator::query_coordinator().await;
    let rt = state.runtime.status().await;

    let mut statuses = Vec::new();

    // If we have graph data, use it to produce per-node status
    if coord.connected {
        for df in &coord.dataflows {
            if df.id == dataflow_id || df.name.contains(&dataflow_id) {
                // We can't get per-node info from CLI, so produce a summary entry
                let status = if df.status == "running" {
                    "running".to_string()
                } else {
                    "exited".to_string()
                };
                // For CLI fallback, create one entry per dataflow (not per-node)
                statuses.push(models::NodeRuntimeStatus {
                    node_id: df.id.clone(),
                    status,
                    uptime_secs: None,
                    restart_count: 0,
                    cpu_usage: None,
                    memory_mb: None,
                    pending_messages: None,
                });
            }
        }
    }

    // If runtime has this dataflow running, include that info
    if rt.status == "running" && rt.dataflow_id.as_deref() == Some(&dataflow_id) {
        if statuses.is_empty() {
            statuses.push(models::NodeRuntimeStatus {
                node_id: dataflow_id.clone(),
                status: "running".to_string(),
                uptime_secs: None,
                restart_count: 0,
                cpu_usage: None,
                memory_mb: None,
                pending_messages: None,
            });
        }
    }

    if statuses.is_empty() {
        statuses.push(models::NodeRuntimeStatus {
            node_id: dataflow_id,
            status: "unknown".to_string(),
            uptime_secs: None,
            restart_count: 0,
            cpu_usage: None,
            memory_mb: None,
            pending_messages: None,
        });
    }

    Json(statuses)
}

/// POST /api/runtime/nodes/:dataflow_id/reload — hot reload a node.
async fn runtime_reload(
    Path(dataflow_id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<models::ReloadRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !state.ws_client.is_connected().await {
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "Coordinator WebSocket not connected. Hot reload requires the WebSocket API."
                .to_string(),
        });
    }

    state
        .ws_client
        .reload_node(&dataflow_id, &body.node_id)
        .await
        .map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e,
        })?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "nodeId": body.node_id,
        "message": "Reload signal sent. Node will restart with updated code."
    })))
}

// --- Recording API (M04) ---

use axum::extract::Query;

#[derive(serde::Deserialize)]
struct RecordingCaptureRequest {
    #[serde(rename = "dataflowPath")]
    dataflow_path: String,
}

async fn recording_capture(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RecordingCaptureRequest>,
) -> Json<recording::RecordingStatus> {
    Json(state.recording.capture(req.dataflow_path).await)
}

async fn recording_stop(State(state): State<Arc<AppState>>) -> Json<recording::RecordingStatus> {
    Json(state.recording.stop().await)
}

async fn recording_list(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<recording::RecordingEntry>> {
    Json(state.recording.list().await)
}

async fn recording_open(
    State(state): State<Arc<AppState>>,
    Json(body): Json<models::OpenRecordingRequest>,
) -> Result<Json<models::RecordingOpened>, ApiError> {
    let path = std::path::PathBuf::from(&body.path);
    let handle = state.recordings.open(&path).await.map_err(|e| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: e,
    })?;

    Ok(Json(models::RecordingOpened {
        id: handle.id.to_string(),
        dataflow_id: handle.header.dataflow_id.to_string(),
        version: handle.header.version,
        start_nanos: handle.header.start_nanos,
        message_count: handle.message_count(),
        duration_nanos: handle.duration_nanos(),
        stream_count: handle.streams().len(),
    }))
}

async fn recording_streams(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let uid: uuid::Uuid = id.parse().map_err(|_| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: format!("invalid recording id: {id}"),
    })?;
    let handle = state.recordings.get(&uid).await.ok_or(ApiError {
        status: StatusCode::NOT_FOUND,
        message: "recording not found".to_string(),
    })?;

    let streams: Vec<serde_json::Value> = handle
        .streams()
        .iter()
        .map(|s| {
            serde_json::json!({
                "nodeId": s.node_id,
                "outputId": s.output_id,
                "entryCount": s.entry_count,
                "timeRange": [s.time_range.0, s.time_range.1],
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "streams": streams })))
}

async fn recording_seek(
    Path(id): Path<String>,
    Query(q): Query<models::SeekQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let uid: uuid::Uuid = id.parse().map_err(|_| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: format!("invalid recording id: {id}"),
    })?;
    let handle = state.recordings.get(&uid).await.ok_or(ApiError {
        status: StatusCode::NOT_FOUND,
        message: "recording not found".to_string(),
    })?;

    match handle.seek(q.timestamp) {
        Some(entry) => Ok(Json(serde_json::json!({
            "byteOffset": entry.byte_offset,
            "timestampNanos": entry.timestamp_absolute_nanos,
            "nodeId": entry.node_id,
            "outputId": entry.output_id,
        }))),
        None => Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: "no entry at or before timestamp".to_string(),
        }),
    }
}

async fn recording_entries(
    Path(id): Path<String>,
    Query(q): Query<models::EntriesQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let uid: uuid::Uuid = id.parse().map_err(|_| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: format!("invalid recording id: {id}"),
    })?;
    let handle = state.recordings.get(&uid).await.ok_or(ApiError {
        status: StatusCode::NOT_FOUND,
        message: "recording not found".to_string(),
    })?;

    let entries = if let (Some(node), Some(output)) = (&q.node, &q.output) {
        handle.stream_entries(node, output, q.offset, q.limit)
    } else {
        let all = handle.index.all_entries();
        let start = q.offset.min(all.len());
        let end = (start + q.limit).min(all.len());
        all[start..end].iter().collect()
    };

    let items: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            let mut obj = serde_json::json!({
                "byteOffset": e.byte_offset,
                "timestampNanos": e.timestamp_absolute_nanos,
                "nodeId": e.node_id,
                "outputId": e.output_id,
            });
            if q.include_data {
                match handle.read_event_bytes(e.byte_offset) {
                    Ok(bytes) => {
                        obj["eventBytes"] = serde_json::json!(bytes);
                    }
                    Err(err) => {
                        eprintln!("read_event_bytes failed at offset {}: {err}", e.byte_offset);
                    }
                }
            }
            obj
        })
        .collect();

    Ok(Json(serde_json::json!({
        "entries": items,
        "offset": q.offset,
        "limit": q.limit,
        "total": handle.message_count(),
    })))
}

async fn recording_close(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let uid: uuid::Uuid = match id.parse() {
        Ok(u) => u,
        Err(_) => return Json(serde_json::json!({ "ok": false, "error": "invalid id" })),
    };
    state.recordings.close(&uid).await;
    Json(serde_json::json!({ "ok": true }))
}

// --- Attribution API (M09) ---

async fn resolve_recording(
    state: &AppState,
    id: &str,
) -> Result<Arc<drec::service::RecordingHandle>, ApiError> {
    let uid: uuid::Uuid = id.parse().map_err(|_| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: format!("invalid recording id: {id}"),
    })?;
    state.recordings.get(&uid).await.ok_or(ApiError {
        status: StatusCode::NOT_FOUND,
        message: "recording not found".to_string(),
    })
}

async fn extract_attribution(
    handle: Arc<drec::service::RecordingHandle>,
) -> Result<attribution::AttributionExtractor, ApiError> {
    tokio::task::spawn_blocking(move || attribution::AttributionExtractor::from_recording(&handle))
        .await
        .map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("attribution extraction panicked: {e}"),
        })?
        .map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e,
        })
}

async fn recording_attribution(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<attribution::AttributionSummary>, ApiError> {
    let handle = resolve_recording(&state, &id).await?;
    let extractor = extract_attribution(handle).await?;
    Ok(Json(attribution::AttributionSummary::from(&extractor)))
}

#[derive(serde::Deserialize)]
struct AttributionChainQuery {
    timestamp: u64,
}

async fn recording_attribution_chain(
    Path(id): Path<String>,
    Query(q): Query<AttributionChainQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<attribution::AttributionChain>, ApiError> {
    let handle = resolve_recording(&state, &id).await?;
    let extractor = extract_attribution(handle).await?;
    extractor
        .chain_at(q.timestamp)
        .cloned()
        .map(Json)
        .ok_or(ApiError {
            status: StatusCode::NOT_FOUND,
            message: format!("no attribution chain at timestamp {}", q.timestamp),
        })
}

// --- LeRobot API (M10) ---

#[derive(serde::Deserialize)]
struct LerobotScanRequest {
    path: String,
}

#[derive(serde::Deserialize)]
struct LerobotFramesRequest {
    path: String,
    episode: u32,
    #[serde(default = "default_frame_offset")]
    offset: usize,
    #[serde(default = "default_frame_limit")]
    limit: usize,
}

fn default_frame_offset() -> usize {
    0
}

fn default_frame_limit() -> usize {
    200
}

#[derive(serde::Deserialize)]
struct LerobotAttributionRequest {
    path: String,
    profile: Option<String>,
    episode: u32,
    #[serde(default = "default_frame_offset")]
    offset: usize,
    #[serde(default = "default_frame_limit")]
    limit: usize,
}

fn lerobot_api_error(e: String) -> ApiError {
    ApiError {
        status: StatusCode::UNPROCESSABLE_ENTITY,
        message: e,
    }
}

async fn lerobot_status() -> Json<lerobot::LerobotStatus> {
    Json(lerobot::check_status().await)
}

async fn lerobot_scan(
    Json(req): Json<LerobotScanRequest>,
) -> Result<Json<lerobot::DatasetInfo>, ApiError> {
    lerobot::scan_dataset(std::path::Path::new(&req.path))
        .await
        .map(Json)
        .map_err(lerobot_api_error)
}

async fn lerobot_frames(
    Json(req): Json<LerobotFramesRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (frames, total) = lerobot::read_frames(
        std::path::Path::new(&req.path),
        req.episode,
        req.offset,
        req.limit,
    )
    .await
    .map_err(lerobot_api_error)?;
    Ok(Json(
        serde_json::json!({ "frames": frames, "total": total }),
    ))
}

async fn lerobot_profiles(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let names = state.profiles.list().unwrap_or_default();
    let profiles: Vec<serde_json::Value> = names
        .iter()
        .filter_map(|n| {
            state
                .profiles
                .load(n)
                .ok()
                .map(|p| serde_json::json!({ "name": n, "robot": p.robot_name }))
        })
        .collect();
    Json(serde_json::json!({ "profiles": profiles }))
}

async fn lerobot_autodetect(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LerobotScanRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let info = lerobot::scan_dataset(std::path::Path::new(&req.path))
        .await
        .map_err(lerobot_api_error)?;
    let suggestion = state
        .profiles
        .autodetect(&info.columns)
        .map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.to_string(),
        })?;
    Ok(Json(serde_json::json!({
        "columns": info.columns,
        "suggestedProfile": suggestion.as_ref().map(|(n, _)| n),
        "score": suggestion.as_ref().map(|(_, s)| s),
    })))
}

async fn lerobot_attribution(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LerobotAttributionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let info = lerobot::scan_dataset(std::path::Path::new(&req.path))
        .await
        .map_err(lerobot_api_error)?;
    let profile_name = match req.profile {
        Some(n) => n,
        None => state
            .profiles
            .autodetect(&info.columns)
            .map_err(|e| ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: e.to_string(),
            })?
            .map(|(n, _)| n)
            .ok_or(ApiError {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                message: "no matching robot profile; add one under profiles/".to_string(),
            })?,
    };
    let profile = state
        .profiles
        .load(&profile_name)
        .map_err(|e| lerobot_api_error(e.to_string()))?;
    let (frames, total) = lerobot::read_frames(
        std::path::Path::new(&req.path),
        req.episode,
        req.offset,
        req.limit,
    )
    .await
    .map_err(lerobot_api_error)?;
    let chains = lerobot::chains_from_frames(&frames, &info.tasks);
    let summaries: Vec<serde_json::Value> = chains
        .iter()
        .map(|c| {
            serde_json::json!({
                "timestampNanos": c.timestamp_nanos,
                "success": c.success(),
                "stepCount": c.steps.len(),
            })
        })
        .collect();
    Ok(Json(serde_json::json!({
        "chains": chains,
        "summaries": summaries,
        "total": total,
        "profile": profile_name,
        "tasks": info.tasks,
        "angleUnit": profile.angle_unit.as_str(),
    })))
}

// --- Live API (M15 B3) ---

#[derive(serde::Deserialize)]
struct LiveRecentQuery {
    stream: Option<String>,
    since_ts: Option<u64>,
    limit: Option<usize>,
}

async fn live_ingest(
    State(state): State<Arc<AppState>>,
    Json(req): Json<live::IngestRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .live
        .ingest(live::LiveFrame {
            node_id: req.node_id,
            output_id: req.output_id,
            timestamp: req.timestamp,
            payload: req.payload,
        })
        .map_err(|e| ApiError {
            status: StatusCode::BAD_REQUEST,
            message: e.0,
        })?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn live_recent(
    State(state): State<Arc<AppState>>,
    Query(q): Query<LiveRecentQuery>,
) -> Json<live::RecentResponse> {
    let limit = q
        .limit
        .unwrap_or(live::DEFAULT_FRAME_LIMIT)
        .min(live::MAX_FRAME_LIMIT);
    let frames = state.live.recent(q.stream.as_deref(), q.since_ts, limit);
    Json(live::RecentResponse { frames })
}

#[derive(serde::Deserialize)]
struct LiveCommandRequest {
    kind: String,
    planner: Option<String>,
    target: Option<Vec<f64>>,
    action: Option<String>,
    object: Option<serde_json::Value>,
}

#[derive(serde::Deserialize)]
struct LiveCommandQueueQuery {
    since_seq: Option<u64>,
}

async fn live_command(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LiveCommandRequest>,
) -> Result<Json<live::LiveCommand>, ApiError> {
    state
        .live
        .push_command(&req.kind, req.planner, req.target, req.action, req.object)
        .map(Json)
        .map_err(|e| ApiError {
            status: StatusCode::BAD_REQUEST,
            message: e.0,
        })
}

async fn live_command_queue(
    State(state): State<Arc<AppState>>,
    Query(q): Query<LiveCommandQueueQuery>,
) -> Json<serde_json::Value> {
    let commands = state.live.take_commands(q.since_seq.unwrap_or(0));
    let next_seq = state.live.next_command_seq();
    Json(serde_json::json!({ "commands": commands, "next_seq": next_seq }))
}

// --- Metrics API (M07) ---

async fn metrics_nodes(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<metrics::NodeMetricSummary>> {
    Json(state.monitoring.metrics.nodes_summary().await)
}

#[derive(serde::Deserialize)]
struct NodeHistoryQuery {
    #[serde(default)]
    window: Option<u64>,
}

async fn metrics_node_history(
    State(state): State<Arc<AppState>>,
    Path(node_id): Path<String>,
    Query(q): Query<NodeHistoryQuery>,
) -> Result<Json<Vec<metrics::NodeMetricSample>>, ApiError> {
    match state
        .monitoring
        .metrics
        .node_history(&node_id, q.window)
        .await
    {
        Some(history) => Ok(Json(history)),
        None => Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: format!("Node '{}' not found in metrics", node_id),
        }),
    }
}

// --- OTel API (M08) ---

async fn otel_status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(state.monitoring.otel.status().await)
}

#[derive(serde::Deserialize)]
struct OtelSpansQuery {
    node: Option<String>,
    #[serde(default = "default_otel_limit")]
    limit: usize,
}

fn default_otel_limit() -> usize {
    200
}

async fn otel_spans(
    State(state): State<Arc<AppState>>,
    Query(q): Query<OtelSpansQuery>,
) -> Json<Vec<otel::OtelSpan>> {
    Json(
        state
            .monitoring
            .otel
            .spans_for_node(q.node.as_deref(), q.limit)
            .await,
    )
}

async fn otel_trace(
    State(state): State<Arc<AppState>>,
    Path(trace_id): Path<String>,
) -> Result<Json<Vec<otel::SpanNode>>, ApiError> {
    match state.monitoring.otel.trace_tree(&trace_id).await {
        Some(tree) => Ok(Json(tree)),
        None => Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: format!("Trace '{}' not found", trace_id),
        }),
    }
}

// --- Monitoring control API (M11.5) ---

async fn monitoring_status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(state.monitoring.status().await)
}

async fn monitoring_toggle(
    State(state): State<Arc<AppState>>,
    Json(req): Json<models::MonitoringToggleRequest>,
) -> Json<serde_json::Value> {
    if let Some(enabled) = req.node_metrics {
        state
            .monitoring
            .set_enabled(monitoring::MonitorTarget::NodeMetrics, enabled);
    }
    if let Some(enabled) = req.otel_spans {
        state
            .monitoring
            .set_enabled(monitoring::MonitorTarget::OtelSpans, enabled);
    }
    Json(state.monitoring.status().await)
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
