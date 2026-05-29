use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeState {
    pub status: String,
    pub pid: Option<u32>,
    pub last_message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatus {
    pub coordinator: &'static str,
    pub daemon: &'static str,
    pub version: &'static str,
    pub running_dataflows: u32,
    pub active_nodes: u32,
    pub error_count: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataflowSummary {
    pub id: &'static str,
    pub name: &'static str,
    pub status: &'static str,
    pub node_count: u32,
    pub edge_count: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeMetrics {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: &'static str,
    pub status: &'static str,
    pub cpu: u32,
    pub memory: u32,
    pub restarts: u32,
    pub pending: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub time: String,
    pub node: String,
    pub level: String,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: &'static str,
    pub status: &'static str,
    pub x: u32,
    pub y: u32,
    pub inputs: Vec<&'static str>,
    pub outputs: Vec<&'static str>,
    pub cpu: u32,
    pub memory: u32,
    pub restarts: u32,
    pub pending: u32,
    pub note: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub id: &'static str,
    pub from: &'static str,
    pub to: &'static str,
    pub label: &'static str,
}

#[derive(Serialize)]
pub struct Diagnostic {
    pub severity: &'static str,
    pub message: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataflowGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub diagnostics: Vec<Diagnostic>,
}
