use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub error: String,
}

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
    pub id: String,
    pub name: String,
    pub status: String,
    pub node_count: u32,
    pub edge_count: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataflowDefinition {
    pub id: String,
    pub name: String,
    pub relative_path: String,
    pub source: String,
    pub node_count: u32,
    pub edge_count: u32,
    pub nodes: Vec<DataflowDefinitionNode>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataflowDefinitionNode {
    pub id: String,
    pub path: Option<String>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeMetrics {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub status: String,
    pub cpu: u32,
    pub memory: u32,
    pub restarts: u32,
    pub pending: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub time: String,
    pub timestamp: String,
    pub node: String,
    pub level: String,
    pub message: String,
    pub raw_message: String,
    pub source: String,
    pub source_file: Option<String>,
    pub source_line: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub status: String,
    pub x: u32,
    pub y: u32,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub cpu: u32,
    pub memory: u32,
    pub restarts: u32,
    pub pending: u32,
    pub note: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub label: String,
}

#[derive(Serialize)]
pub struct Diagnostic {
    pub severity: String,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataflowGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub diagnostics: Vec<Diagnostic>,
}
