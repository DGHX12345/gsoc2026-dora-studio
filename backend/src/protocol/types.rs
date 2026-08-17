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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    #[serde(default)]
    pub restart_count: u32,
    /// Inputs that have timed out (causing Degraded status).
    #[serde(default)]
    pub broken_inputs: Vec<String>,
    #[serde(default)]
    pub status: NodeStatus,
    /// Number of pending messages in the input queue.
    #[serde(default)]
    pub pending_messages: u64,
}

/// Per-dataflow network counters (shared across all nodes in the dataflow).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    #[serde(default)]
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
    #[serde(default)]
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

/// Wire form of a log level: either `"stdout"` or a [`LogLevel`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevelOrStdout {
    #[serde(rename = "stdout")]
    Stdout,
    #[serde(untagged)]
    LogLevel(LogLevel),
}

/// A structured log message from a dora node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMessage {
    pub build_id: Option<Uuid>,
    pub dataflow_id: Option<Uuid>,
    pub node_id: Option<String>,
    pub daemon_id: Option<String>,
    pub level: LogLevelOrStdout,
    pub target: Option<String>,
    pub module_path: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub fields: Option<BTreeMap<String, String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixture shaped like a real dora 1.0 `GetNodeInfo` reply: the
    /// coordinator omits `network` and several metrics fields when
    /// unavailable (all `#[serde(default)]` upstream).
    #[test]
    fn node_info_list_parses_10_json_with_missing_optionals() {
        let json = r#"[
            {
                "dataflow_id": "11111111-1111-1111-1111-111111111111",
                "dataflow_name": "demo",
                "node_id": "planner",
                "daemon_id": "aaaa-bbbb",
                "metrics": {
                    "pid": 123,
                    "cpu_usage": 12.5,
                    "memory_mb": 48.0,
                    "disk_read_mb_s": null,
                    "disk_write_mb_s": null
                }
            }
        ]"#;
        let list: NodeInfoList = serde_json::from_str(json).unwrap();
        assert_eq!(list.0.len(), 1);
        let node = &list.0[0];
        assert_eq!(node.node_id, "planner");
        assert!(node.network.is_none());
        let metrics = node.metrics.as_ref().unwrap();
        assert_eq!(metrics.restart_count, 0);
        assert!(metrics.broken_inputs.is_empty());
        assert_eq!(metrics.status, NodeStatus::Running);
        assert_eq!(metrics.pending_messages, 0);
    }

    #[test]
    fn network_metrics_parses_without_publish_failures() {
        let json = r#"{
            "dataflow_id": "11111111-1111-1111-1111-111111111111",
            "dataflow_name": null,
            "node_id": "planner",
            "daemon_id": "aaaa-bbbb",
            "metrics": null,
            "network": {
                "bytes_sent": 10,
                "bytes_received": 20,
                "messages_sent": 1,
                "messages_received": 2
            }
        }"#;
        let node: NodeInfo = serde_json::from_str(json).unwrap();
        let network = node.network.unwrap();
        assert_eq!(network.publish_failures, 0);
    }

    /// dora 1.0 `DataflowStatus` serializes without a rename attribute:
    /// the wire form is PascalCase ("Running"), not kebab-case.
    #[test]
    fn dataflow_status_parses_pascal_case() {
        let json = r#"{"id":{"uuid":"11111111-1111-1111-1111-111111111111","name":"demo"},"status":"Running"}"#;
        let entry: DataflowListEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.status, DataflowStatus::Running);
    }

    /// dora 1.0 log levels are `LogLevelOrStdout`: a node writing to
    /// stdout emits `"level": "stdout"`.
    #[test]
    fn log_message_parses_stdout_level() {
        let json = r#"{
            "build_id": null,
            "dataflow_id": null,
            "node_id": "planner",
            "daemon_id": null,
            "level": "stdout",
            "target": null,
            "module_path": null,
            "file": null,
            "line": null,
            "message": "hello",
            "timestamp": "2026-08-17T10:00:00Z",
            "fields": null
        }"#;
        let msg: LogMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg.level, LogLevelOrStdout::Stdout));
        assert_eq!(msg.message, "hello");
    }

    #[test]
    fn log_message_parses_log_level() {
        let json = r#"{
            "build_id": null,
            "dataflow_id": null,
            "node_id": "planner",
            "daemon_id": null,
            "level": "WARN",
            "target": null,
            "module_path": null,
            "file": null,
            "line": null,
            "message": "careful",
            "timestamp": "2026-08-17T10:00:00Z",
            "fields": null
        }"#;
        let msg: LogMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg.level, LogLevelOrStdout::LogLevel(LogLevel::Warn)));
    }
}
