mod mock;
mod models;
mod runtime;

use axum::{
    extract::{Path, State},
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
        .route("/api/dataflows/:id/nodes", get(dataflow_nodes))
        .route("/api/dataflows/:id/logs", get(dataflow_logs))
        .route("/api/dataflows/:id/graph", get(dataflow_graph))
        .route("/api/runtime/status", get(runtime_status))
        .route("/api/runtime/logs", get(runtime_logs))
        .route("/api/runtime/start", post(runtime_start))
        .route("/api/runtime/stop", post(runtime_stop))
        .with_state(runtime)
        .layer(CorsLayer::permissive());

    let bind_addr = std::env::var("DORA_STUDIO_BACKEND_ADDR").unwrap_or_else(|_| "127.0.0.1:3001".to_string());
    let addr = bind_addr.parse().expect("valid bind address");
    println!("dora-studio backend listening on http://{addr}");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server failed");
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

async fn system_status() -> Json<models::SystemStatus> {
    Json(mock::system_status())
}

async fn dataflows() -> Json<Vec<models::DataflowSummary>> {
    Json(mock::dataflows())
}

async fn dataflow_nodes(Path(_id): Path<String>) -> Json<Vec<models::NodeMetrics>> {
    Json(mock::nodes())
}

async fn dataflow_logs(Path(_id): Path<String>) -> Json<Vec<models::LogEntry>> {
    Json(mock::logs())
}

async fn dataflow_graph(Path(_id): Path<String>) -> Json<models::DataflowGraph> {
    Json(mock::graph())
}

async fn runtime_status(State(runtime): State<runtime::RuntimeHandle>) -> Json<models::RuntimeState> {
    Json(runtime.status().await)
}

async fn runtime_logs(State(runtime): State<runtime::RuntimeHandle>) -> Json<Vec<models::LogEntry>> {
    Json(runtime.logs().await)
}

async fn runtime_start(State(runtime): State<runtime::RuntimeHandle>) -> Json<models::RuntimeState> {
    Json(runtime.start().await)
}

async fn runtime_stop(State(runtime): State<runtime::RuntimeHandle>) -> Json<models::RuntimeState> {
    Json(runtime.stop().await)
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
