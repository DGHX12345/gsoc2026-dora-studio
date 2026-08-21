# M00: Type Definitions — Copy dora Structs

**Layer**: 0 — Foundation
**Depends on**: nothing
**Effort**: 0.5 weeks

## Purpose

dora-rs requires Rust edition 2024 + rustc 1.88. Our environment is Rust
1.75 + edition 2021. We cannot link dora crates as path dependencies.
Instead, copy the key type definitions and binary format constants into
our codebase, pinned to the current dora revision.

## Strategy

Copy struct definitions + binary format parsing logic for:

1. `.drec` recording format — header, entry, footer structs + magic bytes
2. Coordinator WebSocket protocol — `ControlRequest`, `ControlRequestReply`,
   `WsMessage`, `NodeInfo`, `NodeMetricsInfo`, `DataflowList`, `LogMessage`
3. Daemon metrics types — `NodeStatus`, `NetworkMetrics`

These types are stable since dora v1.0.0-rc.4. We pin to the exact revision
at `/home/dora/dora` (HEAD). If dora upstream changes these types, we update
manually — same maintenance burden as a path dependency, but without the
compiler version constraint.

## Deliverables

### D1: .drec format types

New file: `backend/src/drec/types.rs`

Copy from `/home/dora/dora/libraries/recording/src/lib.rs`:

```rust
pub const MAGIC: &[u8; 8] = b"DORAREC\x00";
pub const FOOTER_MAGIC: &[u8; 8] = b"DORAEND\x00";
pub const FORMAT_VERSION: u16 = 1;
pub const MAX_RECORD_BYTES: u32 = 64 * 1024 * 1024; // 64 MiB

pub struct RecordingHeader {
    pub version: u16,
    pub start_nanos: u64,
    pub dataflow_id: uuid::Uuid,
    pub descriptor_yaml: Vec<u8>,
}

pub struct RecordEntry {
    pub node_id: String,
    pub output_id: String,
    pub timestamp_offset_nanos: u64,
    pub event_bytes: Vec<u8>,
}

pub struct RecordingFooter {
    pub total_messages: u64,
    pub total_bytes: u64,
}
```

### D2: Coordinator protocol types

New file: `backend/src/protocol/types.rs`

Copy from:
- `/home/dora/dora/libraries/message/src/cli_to_coordinator.rs` — `ControlRequest` enum
- `/home/dora/dora/libraries/message/src/coordinator_to_cli.rs` — `ControlRequestReply`, `NodeInfo`, `NodeMetricsInfo`, `DataflowList`, `DataflowStatus`
- `/home/dora/dora/libraries/message/src/ws_protocol.rs` — `WsMessage` (Request/Response/Event)
- `/home/dora/dora/libraries/message/src/common.rs` — `LogMessage`, `LogLevelOrStdout`

Key types we need (non-exhaustive, add as needed by later modules):

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct WsRequest {
    pub id: Uuid,
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WsResponse {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WsEvent {
    pub event: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WsMessage {
    Request(WsRequest),
    Response(WsResponse),
    Event(WsEvent),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    pub dataflow_id: Uuid,
    pub dataflow_name: Option<String>,
    pub node_id: String,
    pub daemon_id: String,
    pub metrics: Option<NodeMetricsInfo>,
    pub network: Option<NetworkMetrics>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeMetricsInfo {
    pub pid: u32,
    pub cpu_usage: f32,
    pub memory_mb: f64,
    pub disk_read_mb_s: Option<f64>,
    pub disk_write_mb_s: Option<f64>,
    pub restart_count: u32,
    pub broken_inputs: Vec<String>,
    pub status: NodeStatus,
    pub pending_messages: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeStatus {
    Running,
    Restarting,
    Degraded,
    Failed,
    Stopped,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataflowList(pub Vec<DataflowListEntry>);

#[derive(Debug, Serialize, Deserialize)]
pub struct DataflowListEntry {
    pub id: DataflowIdAndName,
    pub status: DataflowStatus,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DataflowStatus {
    Running,
    Finished,
    Failed,
}

#[derive(Debug, Serialize, Deserialize)]
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
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub fields: Option<BTreeMap<String, String>>,
}
```

### D3: Divergence documentation

New file: `RUST-COMPAT.md` in project root:

- Record the pinned dora revision (`git rev-parse HEAD` in /home/dora/dora)
- List all copied files and their dora source paths
- Procedure for updating when dora upstream changes
- Note: edition 2024 / rustc 1.88 requirement blocks path dependency

## Acceptance Criteria

- [ ] `cargo build` succeeds with new `backend/src/drec/types.rs` and `backend/src/protocol/types.rs`
- [ ] All existing tests still pass (no regressions)
- [ ] Copied types match dora source (manual diff review)
- [ ] `RUST-COMPAT.md` documents divergence and update procedure
- [ ] `npm run build` passes

## Exposed Interfaces

```rust
// backend/src/drec/types.rs
pub use self::{RecordingHeader, RecordEntry, RecordingFooter, MAGIC, FOOTER_MAGIC, FORMAT_VERSION};

// backend/src/protocol/types.rs
pub use self::{WsMessage, WsRequest, WsResponse, WsEvent,
    NodeInfo, NodeMetricsInfo, NodeStatus, NetworkMetrics,
    DataflowList, DataflowListEntry, DataflowStatus,
    LogMessage, LogLevelOrStdout};
```
