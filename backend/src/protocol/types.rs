//! Coordinator WebSocket protocol types.
//!
//! Copied from dora-rs message libraries:
//! - `libraries/message/src/ws_protocol.rs` — WsMessage envelope
//! - `libraries/message/src/coordinator_to_cli.rs` — reply types, NodeInfo, etc.
//! - `libraries/message/src/common.rs` — LogMessage
//!
//! Pinned to the dora revision documented in plans2.0/RUST-COMPAT.md.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// WebSocket message envelope
// ---------------------------------------------------------------------------

/// Top-level WebSocket message (untagged JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WsMessage {
    Request(WsRequest),
    Response(WsResponse),
    Event(WsEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsRequest {
    pub id: Uuid,
    #[serde(default)]
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsResponse {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsEvent {
    pub event: String,
    pub payload: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Coordinator reply payloads
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataflowIdAndName {
    pub uuid: Uuid,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DataflowStatus {
    Running,
    Finished,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataflowListEntry {
    pub id: DataflowIdAndName,
    pub status: DataflowStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataflowList(pub Vec<DataflowListEntry>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeStatus {
    Running,
    Restarting,
    Degraded,
    Failed,
    Stopped,
}

impl Default for NodeStatus {
    fn default() -> Self {
        Self::Running
    }
}

/// Per-node resource metrics.
///
/// Note: daemon sends `memory_bytes` but coordinator converts to `memory_mb`
/// for the wire. All memory fields are in megabytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetricsInfo {
    pub pid: u32,
    /// CPU usage as percentage of one core.
    pub cpu_usage: f32,
    /// Memory usage in megabytes.
    pub memory_mb: f64,
    pub disk_read_mb_s: Option<f64>,
    pub disk_write_mb_s: Option<f64>,
    pub restart_count: u32,
    /// Inputs that have timed out (causing Degraded status).
    pub broken_inputs: Vec<String>,
    pub status: NodeStatus,
    /// Number of pending messages in the input queue.
    pub pending_messages: u64,
}

/// Per-dataflow network counters (shared across all nodes in the dataflow).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub publish_failures: u64,
}

/// A single node's full info returned by `GetNodeInfo`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub dataflow_id: Uuid,
    pub dataflow_name: Option<String>,
    pub node_id: String,
    pub daemon_id: String,
    pub metrics: Option<NodeMetricsInfo>,
    pub network: Option<NetworkMetrics>,
}

/// Wrapper for `GetNodeInfo` / `List` replies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfoList(pub Vec<NodeInfo>);

/// Summary returned by `GetTraces`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSummary {
    pub trace_id: String,
    pub root_span_name: String,
    pub span_count: u32,
    pub start_time: String,
    pub total_duration_us: u64,
}

/// Individual span returned by `GetTraceSpans`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub target: Option<String>,
    pub level: Option<String>,
    pub start_time: String,
    pub duration_us: u64,
    pub fields: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------------

/// Severity level for log messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// A structured log message from a dora node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMessage {
    pub build_id: Option<Uuid>,
    pub dataflow_id: Option<Uuid>,
    pub node_id: Option<String>,
    pub daemon_id: Option<String>,
    pub level: LogLevel,
    pub target: Option<String>,
    pub module_path: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub fields: Option<BTreeMap<String, String>>,
}
