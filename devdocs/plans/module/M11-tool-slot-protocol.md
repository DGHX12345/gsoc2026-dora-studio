# M11: Tool Slot Protocol

**Layer**: 5 — Tool Slot System
**Depends on**: M06 (3D viewport sync), M03 (runtime)
**Effort**: 1 week

> **Revision 2026-08-13**（AI 接手审阅 + 与学生对齐确认）：
>
> 1. **R1 — ArrowBatch 改为 flat payload**。计划中的 `ArrowBatch`（schema +
>    `columns: Map<string, TypedArray>`）与真实数据不符：M12 审计确认 dviz/moveit
>    数据全部是 flat Float32Array 或 JSON bytes，无结构化 Arrow；代码库前后端均无
>    Arrow 类型。协议改为 `ToolPayload = { f32?: Float32Array; json?: unknown;
>    bytes?: Uint8Array }` 联合载荷，工具自行解析。
> 2. **R2 — D3 Rust 桥取消，改为前端匹配引擎**。`ViewportToolBridge` 引用的
>    `RecordBatch`/`TfTree` 类型在代码库中不存在（不能 link dora crates），后端无
>    Arrow 数据可路由、无消费者。工具全部在 TS 侧挂载 Three.js 场景。推荐匹配逻辑
>    在 TS 侧（原生 RegExp），沿用现有 tsx 测试模式（见 `test:motion` 先例）。
>    验收标准 "cargo test includes tool bridge routing test" 修订为
>    `npm run test:tools`（tsx 单测）。后端零改动。
> 3. **R3 — TfTree 接口保留 + 最小实现**（2026-08-13 已与学生确认，dviz 源码已核实：
>    `dviz-core/src/types/transform.rs` 的 `StampedTransform { parent_frame, child_frame,
>    transform: { translation: Vec3, rotation: Quat(x,y,z,w) } }`；`dviz-rosbag/src/tf.rs`
>    解析 `tf2_msgs/TFMessage | tf/tfMessage`（帧名前导 `/` 去除 + 环检测）。dora 侧无
>    官方 TF 消息，.drec 回放中 TF 以 JSON 载荷流出现）。协议定义 `TfStamped`（parent/
>    child/translation/rotation xyzw/timestampNs?）+ `SimpleTfTree` 实现（Matrix4 组合、
>    父链查找、环检测）；`onBatch` 的 `tf` 参数可选。真实 TF 数据源属未来模块。
> 4. **R4 — renderPanel 改为 `panelComponent?: Component`**（Vue 3 惯用法，
>    `<component :is>` 渲染）；计划中的 `VueComponent` 类型不存在。
> 5. **R5 — ToolPanel 挂载位置**：Visualization 顶栏加 "Tools" 按钮，点开浮动
>    覆盖面板（沿用 AttributionBar 覆盖层先例，最小侵入）。
> 6. **R6 — 参考工具 WaypointEchoTool**：订阅 `waypoints|trajectory` 端口渲染
>    点/线标记，验证注册/挂载/广播/推荐全链路；drec 生成器加含 planner 节点的
>    演示录制。M12 用完整 dviz 工具替换。
> 7. **R7 — 接口加 `category` 字段**（Visualization / Diagnostics / Planning，
>    D5 分组需要）。
> 8. **R8 — 端口示例名修正**：`planned_path` 不存在（审计确认实际是
>    `waypoints|path|trajectory`）。
> 9. **R9 — onAttach 改为 ToolContext**（2026-08-13，实现时发现）：NanoRobotViewer
>    按需渲染（GPU 空闲降 0），工具改场景后必须能触发重绘。`onAttach(context)` 接收
>    `{ scene, camera, requestRender }`；`attachToScene(id, context)`。M12/M13 计划的
>    `onAttach(scene)` 签名按此适配。
> 10. **R10 — 推荐数据源**：D4 的匹配用 .drec streams（`GET /api/recording/:id/streams`）
>     在 Replay 加载时触发；dataflow YAML 侧推荐留待后续模块（M12 集成 DataflowExplorer
>     时做）。
> 11. **R11 — 面板组件绑定**：`panelComponent` 在 `tools/index.ts` 的
>     `registerBuiltinTools()` 中绑定（.vue 无法被 tsx 测试加载，工具类保持纯 TS）；
>     注册幂等（VisualizationView 每次页面切换会重挂载）。

## Purpose

Define and implement the standardized protocol for mounting tools into
the Three.js 3D viewport. Tools register via a common interface, declare
their subscribed dora ports, and receive batched Arrow data at playback
time. This is the extensibility foundation for dviz, MoveIt, and future
tools.

## Deliverables

### D1: TypeScript trait definition

New file: `frontend/src/tools/types.ts`

```typescript
interface ViewportTool {
  readonly id: string;
  readonly displayName: string;
  readonly subscribePorts: ArrowPortSpec[];

  onAttach(scene: THREE.Scene, camera: THREE.Camera): void;
  onBatch(batch: ArrowBatch, tf: TfTree): void;
  onTimelineSeek?(timestampNs: number): void;
  onDetach(): void;

  // Optional: render control panel in sidebar
  renderPanel?: () => VueComponent;
}
```

### D2: Tool registry

New file: `frontend/src/tools/registry.ts`

- `ToolRegistry` singleton:
  - `register(tool: ViewportTool)` — add a tool implementation
  - `unregister(id: string)` — remove a tool
  - `get(id: string): ViewportTool | undefined`
  - `list(): ViewportTool[]`
  - `attachToScene(id: string, scene, camera)` — mount tool
  - `detachFromScene(id: string)` — unmount tool
  - `broadcastBatch(batch: ArrowBatch, tf: TfTree)` — send to all attached tools that subscribe to this port
  - `broadcastSeek(timestampNs: number)` — notify all tools with onTimelineSeek

### D3: Rust tool bridge trait

New file: `backend/src/tool_bridge.rs`

```rust
pub trait ViewportToolBridge: Send + Sync {
    fn tool_id(&self) -> &str;
    fn subscribed_ports(&self) -> Vec<ArrowPortSpec>;
    fn on_batch(&mut self, batch: &RecordBatch, tf: &TfTree);
    fn render_context(&self) -> RenderContext;
}
```

- `ToolBridgeManager` manages Rust-side tool instances
- Routes incoming Arrow batches to matching tools by port subscription
- Runs in a dedicated tokio task to avoid blocking the HTTP server

### D4: Tool recommendation engine

- When a dataflow YAML is loaded or a .drec is opened:
  1. Scan for nodes whose output ports match registered tool subscriptions
  2. Show recommendation: "detected planner node → dviz path tool available"
  3. User clicks → tool auto-attaches to viewport
- This makes tool discovery contextual rather than manual

### D5: Tool sidebar panel

New component: `frontend/src/components/ToolPanel.vue`

- Lists all registered tools with on/off toggles
- Each tool: name, description, status (attached/detached/error)
- Attached tools show expandable control panel (renderPanel())
- Recommendation badge: "New" when a matching port is detected
- Tools grouped by category: Visualization / Diagnostics / Planning

## Acceptance Criteria

- [ ] Register, attach, and detach a tool without affecting other tools or viewport
- [ ] `broadcastBatch` delivers to all tools subscribed to the matching port
- [ ] `broadcastSeek` calls `onTimelineSeek` only on tools that implement it
- [ ] Tool recommendation appears when matching dataflow nodes detected
- [ ] Tool sidebar panel shows correct status for all tools
- [ ] Adding a new tool requires implementing only the `ViewportTool` interface (no other files to touch)
- [ ] `cargo test` includes tool bridge routing test

## Exposed Interfaces

```typescript
// frontend/src/tools/types.ts
export interface ArrowPortSpec {
  nodeIdPattern: string | RegExp;  // e.g., "planner*" or "moveit*"
  outputIdPattern: string | RegExp; // e.g., "trajectory" or "planned_path"
}

export interface ArrowBatch {
  nodeId: string;
  outputId: string;
  timestampNs: number;
  schema: ArrowSchema;
  columns: Map<string, TypedArray>;
  rowCount: number;
}

export interface TfTree {
  getTransform(from: string, to: string): Matrix4 | null;
  frames: Map<string, { parent: string; transform: Matrix4 }>;
}
```

```rust
// backend/src/tool_bridge.rs
pub struct ToolBridgeManager { ... }
impl ToolBridgeManager {
    pub fn new() -> Self;
    pub fn register(&mut self, tool: Box<dyn ViewportToolBridge>);
    pub fn route_batch(&mut self, node_id: &str, output_id: &str, batch: &RecordBatch);
    pub fn route_seek(&mut self, timestamp_ns: u64);
}
```
