# M03: Runtime Binding — Hot Reload + Node Status

**Layer**: 1 — Dataflow Live Graph
**Depends on**: M01 (canvas), M00 (protocol types)
**Effort**: 1.5 weeks

## Purpose

Connect the visual dataflow editor to the live dora runtime. Canvas
nodes show real-time status (running/crashed/reloading/exited). Python
operator nodes support Hot Reload with visual feedback.

## Deliverables

### D1: Coordinator WebSocket client

New file: `backend/src/coordinator_ws.rs`

- Connect to `dora coordinator` at `ws://127.0.0.1:<port>/api/control`
- **Auth**: Read token from coordinator's working directory, send
  `Authorization: Bearer <hex>` header (constant-time compare on server).
  Loopback connections are rate-limit exempt.
- **Hello handshake** (mandatory first message):
  Send `{"id": "<uuid>", "method": "Hello", "params": {"dora_version": "1.0.0-rc.4"}}`
  Wait for `HelloOk` reply before any other requests.
  Coordinator rejects with semver-incompatible versions.
- **JSON-RPC request/reply** (via typed WsMessage from M00):
  - `{"id": "...", "method": "...", "params": {...}}` → request
  - `{"id": "...", "result": {...}}` or `{"id": "...", "error": "..."}` → reply
  - `{"event": "...", "payload": {...}}` → server push (logs, etc.)
- Key requests (using copied types from M00):
  - `List` → `DataflowList(Vec<DataflowListEntry>)`
  - `Start { dataflow, name, ... }` → `DataflowSpawned`
  - `Stop { dataflow_uuid, ... }` → `DataflowStopped`
  - `LogSubscribe { dataflow_id, level }` → `{"subscribed": true}` then `log` events
  - `GetNodeInfo` → `NodeInfoList(Vec<NodeInfo>)` — per-node CPU/memory/status
  - `Reload { dataflow_id, node_id, operator_id }` → `DataflowReloaded`
- Maintain persistent connection with auto-reconnect:
  - Exponential backoff: 1s → 2s → 4s → 8s → max 10s
  - Full re-handshake (Hello) on reconnect
  - Re-subscribe to log streams after reconnect

If coordinator WebSocket is unavailable, fall back to CLI subprocess
(`dora list --format json`, `dora run`, etc. — existing behavior in
coordinator.rs / runtime.rs).

### D2: Node status polling loop

Update `backend/src/runtime.rs`:
- New type: `NodeRuntimeStatus { node_id, status: Running|Crashed|Reloading|Exited|Unknown, uptime_secs, restart_count }`
- Poll coordinator every 2s for node status changes
- Cache last-known status; emit change events on transition
- Status source priority: WebSocket > CLI `dora list` > unknown

API endpoint:
- `GET /api/runtime/nodes/:dataflow_id` — returns `Vec<NodeRuntimeStatus>`

### D3: Canvas status overlay

Update M01 DataflowCanvas:
- Each node card shows status badge:
  - Green pulse: running
  - Red: crashed
  - Yellow spinning: reloading
  - Gray: exited/stopped
- Status updates via frontend polling (2s interval, same as backend)
- Transition animation: crashed nodes flash red briefly; reloading nodes show progress spinner

### D4: Hot Reload button

- Python operator nodes show a "Reload" button in their context menu
- Click → `POST /api/runtime/reload/:node_id` → coordinator sends reload signal
- Backend sends `ReloadRequest` via coordinator WebSocket
- Node status transitions: running → reloading (yellow) → running (green)
- On success: flash green border for 2s, tooltip "Reloaded at HH:MM:SS"
- On failure: status becomes crashed (red), error message in tooltip

### D5: Runtime lifecycle integration

- "Run" button on canvas toolbar:
  1. Save current YAML to temp file
  2. Call `POST /api/runtime/start` with YAML path
  3. Coordinator spawns the dataflow
  4. Canvas polls for node status updates
- "Stop" button: calls `POST /api/runtime/stop`
- Running indicator: canvas border glows green when dataflow is active

## Acceptance Criteria

- [ ] Canvas nodes show correct status (running/crashed/reloading) within 2s of state change
- [ ] Hot Reload on a Python node: status → reloading → running in <2s
- [ ] Hot Reload failure: status → crashed, error tooltip visible
- [ ] Run/Stop from canvas toolbar works end-to-end
- [ ] Coordinator WebSocket disconnect → auto-reconnect → status recovers
- [ ] CLI fallback works when WebSocket unavailable
- [ ] `cargo test` includes status lifecycle test cases

## Exposed Interfaces

```rust
// backend/src/runtime.rs (extended)
pub struct RuntimeManager {
    // ... existing fields ...
    ws_client: Option<CoordinatorWsClient>,
}
impl RuntimeManager {
    pub async fn node_statuses(&self, dataflow_id: &str) -> Vec<NodeRuntimeStatus>;
    pub async fn reload_node(&self, node_id: &str) -> Result<(), ReloadError>;
    pub async fn start_dataflow(&self, yaml_path: &Path) -> Result<String, StartError>;
    pub async fn stop_dataflow(&self, dataflow_id: &str) -> Result<(), StopError>;
}
```

```typescript
// frontend - node status type
type NodeStatus = 'running' | 'crashed' | 'reloading' | 'exited' | 'unknown';
interface NodeRuntimeInfo {
  nodeId: string;
  status: NodeStatus;
  uptimeSecs: number;
  restartCount: number;
}
```
