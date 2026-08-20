use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub error: String,
}

// --- Coordinator ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoordinatorStatus {
    pub connected: bool,
    pub version: String,
    pub running_dataflows: u32,
    pub active_nodes: u32,
    pub dataflows: Vec<CoordinatorDataflow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoordinatorDataflow {
    pub id: String,
    pub name: String,
    pub status: String,
    pub nodes: u32,
}

// --- dviz ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DvizStatus {
    pub installed: bool,
    pub running: bool,
    pub binary_path: Option<String>,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DvizTopicsResponse {
    pub source: String,
    pub message: String,
    pub topics: Vec<DvizTopic>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DvizTopic {
    pub name: String,
    pub data_type: String,
    pub source: String,
    pub status: String,
    pub message_rate_hz: f32,
    pub last_seen: String,
    pub summary: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DvizDisplaysResponse {
    pub source: String,
    pub message: String,
    pub displays: Vec<DvizDisplay>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DvizDisplay {
    pub id: String,
    pub name: String,
    pub data_type: String,
    pub enabled: bool,
    pub source_topic: Option<String>,
    pub status: String,
    pub summary: String,
    pub color: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DvizSnapshotResponse {
    pub source: String,
    pub message: String,
    pub status: DvizStatus,
    pub summary: DvizSnapshotSummary,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DvizSnapshotSummary {
    pub topic_count: usize,
    pub ready_topic_count: usize,
    pub idle_topic_count: usize,
    pub display_count: usize,
    pub enabled_display_count: usize,
}

// --- robot profile ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotProfileResponse {
    pub source: String,
    pub message: String,
    pub profile: RobotProfile,
}

// --- available robot models (M13 D6) ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableModel {
    pub id: String,
    pub urdf_path: String,
    pub mesh_base_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableModelsResponse {
    pub models: Vec<AvailableModel>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotProfile {
    pub id: String,
    pub name: String,
    pub family: String,
    pub summary: String,
    pub simulation_owner: String,
    pub viewport_role: String,
    pub modules: Vec<RobotModule>,
    pub workflows: Vec<RobotWorkflow>,
    pub visualization_displays: Vec<String>,
    pub planning_capabilities: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotModule {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub role: String,
    pub transport: String,
    pub frame: String,
    pub status: String,
    pub summary: String,
    pub required: bool,
    pub source_topics: Vec<String>,
    pub linked_displays: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotWorkflow {
    pub id: String,
    pub name: String,
    pub status: String,
    pub owner: String,
    pub summary: String,
}

// --- dora-moveit2 ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveitStatus {
    pub installed: bool,
    pub running: bool,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveitSnapshotResponse {
    pub source: String,
    pub message: String,
    pub robot_profile_id: String,
    pub robot_config_id: String,
    pub simulation_owner: String,
    pub viewport_role: String,
    pub freshness: MoveitSnapshotFreshness,
    pub joints: Vec<MoveitJointState>,
    pub end_effector_pose: MoveitEndEffectorPose,
    pub scene: MoveitPlanningScene,
    pub trajectory: MoveitTrajectorySummary,
    pub visual_model: MoveitVisualModel,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveitSnapshotFreshness {
    pub status: String,
    pub last_updated: String,
    pub source_label: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveitJointState {
    pub name: String,
    pub value: f32,
    pub unit: String,
    pub lower_limit: f32,
    pub upper_limit: f32,
    pub status: String,
    pub source: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveitEndEffectorPose {
    pub frame: String,
    pub position: [f32; 3],
    pub quaternion: [f32; 4],
    pub source: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveitPlanningScene {
    pub status: String,
    pub object_count: usize,
    pub objects: Vec<MoveitSceneObject>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveitSceneObject {
    pub name: String,
    pub shape: String,
    pub dims: String,
    pub dimensions: Vec<f32>,
    pub frame: String,
    pub status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveitTrajectorySummary {
    pub status: String,
    pub waypoint_count: usize,
    pub duration_seconds: f32,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveitVisualModel {
    pub model_id: String,
    pub name: String,
    pub format: String,
    pub source: String,
    pub joint_order: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeState {
    pub status: String,
    pub pid: Option<u32>,
    pub last_message: String,
    pub dataflow_id: Option<String>,
    pub dataflow_path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatus {
    pub coordinator: String,
    pub daemon: String,
    pub version: String,
    pub running_dataflows: u32,
    pub active_nodes: u32,
    pub error_count: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataflowSummary {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub project: String,
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
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub type_rules: Vec<TypeRuleDef>,
    pub nodes: Vec<DataflowDefinitionNode>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataflowDefinitionNode {
    pub id: String,
    pub path: Option<String>,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default)]
    pub input_types: BTreeMap<String, String>,
    #[serde(default)]
    pub output_types: BTreeMap<String, String>,
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

// --- Runtime node status (M03) ---

/// Per-node runtime status for canvas overlay.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeRuntimeStatus {
    pub node_id: String,
    pub status: String,
    pub uptime_secs: Option<u64>,
    pub restart_count: u32,
    pub cpu_usage: Option<f32>,
    pub memory_mb: Option<f64>,
    pub pending_messages: Option<u64>,
}

/// Request body for hot reload.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReloadRequest {
    pub node_id: String,
    pub operator_id: Option<String>,
}

// --- Monitoring control (M11.5) ---

/// Partial toggle: absent fields leave the target unchanged.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitoringToggleRequest {
    #[serde(default)]
    pub node_metrics: Option<bool>,
    #[serde(default)]
    pub otel_spans: Option<bool>,
}

// --- Recording API types (M04) ---

#[derive(Debug, Deserialize)]
pub struct OpenRecordingRequest {
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingOpened {
    pub id: String,
    pub dataflow_id: String,
    pub version: u16,
    pub start_nanos: u64,
    pub message_count: usize,
    pub duration_nanos: u64,
    pub stream_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct SeekQuery {
    pub timestamp: u64,
}

#[derive(Debug, Deserialize)]
pub struct EntriesQuery {
    pub node: Option<String>,
    pub output: Option<String>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub include_data: bool,
}

fn default_limit() -> usize {
    100
}

// --- Project explorer API types (M18 Task 1.4) ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectListResponse {
    pub projects: Vec<crate::project_scan::ProjectSummary>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddProjectRequest {
    pub path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualNodeRequest {
    pub id: String,
    pub path: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub inputs: Vec<ManualPortRequest>,
    #[serde(default)]
    pub outputs: Vec<ManualPortRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualPortRequest {
    pub name: String,
    #[serde(default)]
    pub urn: String,
}

// --- Schema check API types (M18 Task 3.3) ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeRuleDef {
    pub from: String,
    pub to: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaCheckResponse {
    pub compatible: bool,
    pub level: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<TypeRuleDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

// --- Save API types (M18 Task 3.6) ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveIssue {
    pub node_id: Option<String>,
    pub port_id: Option<String>,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResponse {
    pub ok: bool,
    pub path: String,
    #[serde(default)]
    pub warnings: Vec<SaveIssue>,
    #[serde(default)]
    pub errors: Vec<SaveIssue>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRequest {
    pub graph: crate::dataflow_builder::DataflowGraph,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAsRequest {
    pub graph: crate::dataflow_builder::DataflowGraph,
    pub target_path: String,
}
