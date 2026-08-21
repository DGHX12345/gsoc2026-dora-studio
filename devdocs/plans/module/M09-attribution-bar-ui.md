# M09: VLM/LLM Attribution Bar

**Layer**: 4 — Attribution Bar
**Depends on**: M05 (timeline), M03 (runtime parsing)
**Effort**: 1.5 weeks

## Revision 2026-08-13 — confirmed decisions (with mentor/student)

1. **UI 位置**: AttributionBar 集成进 VisualizationView 的 Replay 模式，
   作为浮动回放条上方的一层可折叠覆盖层（纯加法，不重构现有面板）。
   "Show in 3D" 就地调用现有 `PlaybackEngine.seek(timestamp)`。
2. **D4 范围**: 仅 seek。planned vs actual 箭头/幽灵渲染不在本模块做，
   留给 M12/M13 工具层。
3. **数据格式**: 真实 dora VLM 算子输出 Arrow IPC，本环境（Rust 1.75、
   不加重依赖）无法解析。采用 Studio 结构化载荷（magic `DORAATTR\0` +
   长度前缀字段），extractor 严格解析；遇 Arrow IPC（magic 0xFFFFFFFF）
   明确标 unavailable 并注明原因，不伪造。演示/测试数据由
   `DrecGenerator::generate_vlm_attribution` 合成。

## Purpose

Render the causal chain from camera frame → VLM prompt → LLM output →
parsed action → downstream execution result. This shows WHY the robot
made a decision, not just WHAT happened.

## Deliverables

### D1: Attribution data model

New file: `backend/src/attribution.rs`

- `AttributionChain`: ordered list of causal steps at one timestamp
- Steps:
  1. `SensorFrame` — camera image / lidar scan metadata (timestamp, topic, dimensions)
  2. `Prompt` — text sent to VLM/LLM, with token count
  3. `LLMResponse` — raw text output, token count, model name, latency
  4. `ParsedAction` — structured action extracted from response (joint angles, velocity, waypoint)
  5. `ExecutionResult` — did the action succeed? What was the outcome?

Data source: parse attribution events from dora VLM/LLM operator node
outputs in `.drec` recordings. Payload is the Studio attribution format
(magic `DORAATTR\0`, version u16, kind u8, frame_timestamp u64, then
kind-specific length-prefixed fields). Real Arrow IPC payloads
(magic 0xFFFFFFFF) are reported as unparseable, not fabricated.

Each `AttributionEvent` carries a `frame_timestamp` key; the extractor
groups events by key and orders steps SensorFrame → Prompt →
LLMResponse → ParsedAction → ExecutionResult.

### D2: Attribution bar component

New file: `frontend/src/components/AttributionBar.vue`
集成位置（Revision 2026-08-13）: VisualizationView Replay 模式下、
浮动回放条上方的覆盖层面板（可折叠，纯加法）。

- Horizontal timeline strip below main timeline
- Each time tick: compact causal chain icon stack
  - Camera icon → text bubble → gear icon → checkmark/cross
- Click a tick → expand to full detail card:
  - Frame thumbnail (if image available)
  - Complete prompt text (collapsible if >200 chars)
  - Full LLM response (collapsible)
  - Parsed action vector as table
  - Execution status with latency
- Color coding: green chain = successful, red chain = failed action
- Navigation: left/right arrows to move between attribution frames

### D3: Token stream viewer

- Expand LLM response → show token-by-token streaming view
- Animate token appearance (simulated stream, 50 tokens/sec)
- Token count + model info badge

### D4: Action → 3D viewport highlight

- Click "Show in 3D" on a parsed action → viewport jumps to that timestamp
- Integration: AttributionBar emits timestamp → PlaybackEngine.seek(timestamp)
- ~~Action vector rendered as arrow/ghost in viewport (planned vs actual)~~ —
  **移出本模块**（Revision 2026-08-13），留给 M12/M13 工具层

### D5: Data source configuration

- Dropdown to select attribution data source:
  - ".drec recording" — extract from loaded recording（当前可用）
  - "Live dora VLM node" — 显示 unavailable 说明（需运行中 dataflow）
  - "LeRobot dataset" — (M10) placeholder
- Auto-detect: if .drec loaded and contains VLM node output, use it

## Acceptance Criteria

- [ ] Correctly parses VLM node attribution payload (Studio format): prompt + response + action vector；Arrow IPC 载荷明确标 unavailable，不伪造
- [ ] Attribution bar renders causal chain for each timestamp in recording
- [ ] Click chain → expanded detail card with all 5 steps visible
- [ ] "Show in 3D" correctly seeks viewport to action timestamp
- [ ] Token stream viewer animates at ~50 tokens/sec
- [ ] Toggle between .drec source and LeRobot source (placeholder for M10)
- [ ] Empty state: "No VLM data detected" when source has no attribution data

## Exposed Interfaces

```rust
// backend/src/attribution.rs
pub struct AttributionChain {
    pub timestamp_nanos: u64,
    pub steps: Vec<AttributionStep>,
}

pub enum AttributionStep {
    SensorFrame { topic: String, metadata: SensorMetadata },
    Prompt { text: String, token_count: u32 },
    LLMResponse { text: String, token_count: u32, model: String, latency_ms: u32 },
    ParsedAction { action_type: String, vector: Vec<f32>, confidence: f32 },
    ExecutionResult { success: bool, error_message: Option<String> },
}

pub struct AttributionExtractor { ... }
impl AttributionExtractor {
    pub fn from_recording(recording: &RecordingHandle) -> Self;  // drec::service::RecordingHandle
    pub fn chains(&self) -> Vec<AttributionChain>;
    pub fn chain_at(&self, timestamp_ns: u64) -> Option<AttributionChain>;
    pub fn unparseable_streams(&self) -> Vec<UnparseableStream>; // Arrow IPC 等，诚实 unavailable
}
```

```typescript
// frontend/src/components/AttributionBar.vue
// Props:
//   chains: AttributionChain[]
//   currentTimestamp: number
//   expandedChainId: string | null
// Events:
//   @seek-timestamp(timestampNs: number)
//   @expand-chain(chainId: string)
```
