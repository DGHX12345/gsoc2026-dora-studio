# dora-studio 长期实现方案

## 1. 项目定位

`dora-studio` 的目标不是替代 `dora` CLI，而是为 dora 应用开发者提供一个本地 Web Studio，用于理解、运行、监测和调试 dataflow，并为后续机器人项目中的数据采集、回放、标注、训练等工作流预留扩展接口。

长期定位建议表述为：

> dora-studio 是面向 Dora dataflow 的可视化观测与调试工作台，提供 dataflow 结构理解、运行控制、日志与状态可视化、调试辅助，并作为后续机器人数据工作流的入口。

这比单纯的“Web GUI for Dataflow Management”更准确，也更容易体现项目价值：它不是把命令行搬到网页上，而是补足 dora 在可视化理解、运行期观测和机器人应用调试上的工具链能力。

## 2. 核心设计原则

### 2.1 稳定优先

未来 dora-studio 预计会放到 dora 项目组内作为一个子模块，因此设计上应优先选择稳定、可维护、容易被上游接受的集成方式。

推荐集成优先级：

1. **复用 dora 已有的稳定数据结构和协议**：例如 `dora-message` 中的 coordinator control message、node metrics、logs、trace 等类型。
2. **复用相对独立的核心库**：例如 `dora-core` 的 dataflow descriptor 解析、校验、展开和图生成能力。
3. **通过 coordinator WebSocket 协议交互**：避免直接修改 daemon/coordinator 内部状态。
4. **谨慎复用 CLI 内部 command handler**：CLI 可作为行为参考，但不应把 Studio 后端深度绑定到 CLI 子命令实现细节。
5. **避免依赖明显 unstable 的接口作为核心功能**：例如 `_unstable_debug.enable_debug_inspection` 可用于调试实验，但不应作为 MVP 的唯一关键路径。

### 2.2 先观测，后编辑

Visual editor 很有吸引力，但完整编辑器的复杂度较高，包括 schema、路径、模块、动态 topology、保存回写、错误恢复等问题。

因此建议 GSoC 主线优先级为：

1. 结构可视化
2. 运行状态观测
3. 日志与 debug 辅助
4. 基础配置编辑
5. 更完整的 visual editor

这样可以保证中期就有可演示价值，并降低“编辑器做了一半但不可用”的风险。

### 2.3 UI 先建立产品感

本周目标以简单界面设计为主，优先做一个好看的静态原型。这个原型应帮助 mentor 和社区快速理解 dora-studio 的最终形态，而不是马上追求完整 API 接入。

本周 UI 原型应能表达：

- dataflow graph 是核心视图
- 运行状态和日志是核心调试信息
- Studio 是一个完整工作台，而不是零散页面
- 未来可以自然扩展到机器人数据采集、回放和训练入口

## 3. 总体架构

推荐架构如下：

```text
Vue 3 + TypeScript 前端
        |
        | REST + WebSocket
        v
Rust dora-studio 后端
        |
        | Dora coordinator control WebSocket
        | dora-core / dora-message
        v
Dora coordinator / daemon / runtime
```

### 3.1 前端职责

前端负责：

- 页面布局与交互
- dataflow 图展示
- 节点详情面板
- 运行状态 dashboard
- 日志和事件展示
- debug topic、trace、params 等后续功能的入口

建议技术栈：

- Vue 3
- TypeScript
- Vite
- Vue Flow 或类似成熟 graph 组件
- Pinia 或轻量 composable 状态管理
- Tailwind CSS 或其他可快速产出高质量界面的样式方案

### 3.2 后端职责

后端负责把 dora 的底层能力整理成面向 Studio 的稳定 API：

- dataflow YAML 解析、校验、展开
- graph-friendly JSON 生成
- coordinator 连接和状态检查
- dataflow build/start/stop/restart
- node metrics 查询
- logs 拉取和实时转发
- traces 查询
- debug topic 订阅转发
- params 读取和修改

建议技术栈：

- Rust
- tokio
- axum
- serde
- tokio-tungstenite 或 dora CLI 中可复用的 WebSocket session
- dora-core
- dora-message

### 3.3 为什么不直接让前端连 dora coordinator

不建议浏览器直接连接 dora coordinator，原因是：

- coordinator 协议是 dora 内部控制协议，不是浏览器友好的产品 API
- Studio 需要整理、聚合、过滤和转换底层数据
- 后端可以处理版本兼容、错误信息、权限边界和订阅生命周期
- 未来如果 dora 内部协议变化，只需改 Studio 后端，不必大规模改前端

## 4. 功能分层

### 4.1 第一层：Dataflow 理解与可视化

目标：让用户不用先读完整 YAML，就能理解 dataflow 的结构。

功能：

- 打开 dataflow YAML
- 解析 nodes、inputs、outputs、edges
- 识别 timer source，例如 `dora/timer/millis/100`
- 识别 log source，例如 `dora/logs/error`
- 识别 ROS2 bridge 节点
- 展开 module dataflow
- 展示 graph
- 点击节点查看详情
- 显示 parse/validate 错误

建议 API：

```http
POST /api/descriptors/parse
POST /api/descriptors/validate
POST /api/descriptors/expand
POST /api/descriptors/graph
```

返回给前端的 graph 数据建议使用 Studio 自己定义的稳定格式：

```ts
type StudioGraph = {
  nodes: StudioGraphNode[]
  edges: StudioGraphEdge[]
  diagnostics: Diagnostic[]
}
```

### 4.2 第二层：运行控制

目标：让用户可以从 Studio 完成基本运行闭环。

功能：

- 检查 dora coordinator 是否可达
- 检查 daemon 是否连接
- build dataflow
- start dataflow
- stop dataflow
- restart dataflow
- list dataflows
- inspect dataflow

建议 API：

```http
GET  /api/system/status
GET  /api/system/daemons
GET  /api/dataflows
GET  /api/dataflows/{id}
POST /api/dataflows/build
POST /api/dataflows/start
POST /api/dataflows/{id}/stop
POST /api/dataflows/{id}/restart
```

实现上优先通过 coordinator control WebSocket 使用 dora 已有消息，而不是解析 CLI 输出。

### 4.3 第三层：运行期观测

目标：让用户知道 dataflow 运行时发生了什么。

功能：

- dataflow 生命周期状态
- node 状态
- CPU 使用率
- 内存使用量
- 磁盘读写
- restart count
- broken inputs
- pending messages
- network metrics
- daemon heartbeat
- fault-tolerance stats

建议 API：

```http
GET /api/dataflows/{id}/nodes
GET /api/system/status
```

UI 上建议做成：

- 左侧 graph 中用颜色表达节点状态
- 右侧 node detail panel 展示指标
- 底部或独立页面展示 node table

### 4.4 第四层：日志与事件

目标：把分散在终端和文件中的调试信息集中到 Studio。

功能：

- 按 dataflow 查看日志
- 按 node 查看日志
- 按 level 过滤日志
- 实时日志流
- 错误高亮
- 基础搜索

建议 API：

```http
GET /api/dataflows/{id}/logs
WS  /api/dataflows/{id}/logs/stream
```

MVP 中日志功能比完整 visual editor 更重要，因为它直接服务 debug。

### 4.5 第五层：调试增强

目标：提供比 CLI 更直观的调试体验。

候选功能：

- trace list
- trace detail / span timeline
- params 查看和修改
- node restart / stop
- debug topic subscribe
- payload preview

建议 API：

```http
GET    /api/dataflows/{id}/traces
GET    /api/dataflows/{id}/traces/{trace_id}
GET    /api/dataflows/{id}/params
GET    /api/dataflows/{id}/params/{key}
PUT    /api/dataflows/{id}/params/{key}
DELETE /api/dataflows/{id}/params/{key}
POST   /api/dataflows/{id}/topics/subscribe
WS     /api/dataflows/{id}/topics/stream
```

其中 debug topic 相关功能应标记为 experimental，因为 dora 当前使用 `_unstable_debug.enable_debug_inspection`。

### 4.6 第六层：机器人数据工作流扩展

这是长期方向，不建议作为第一阶段主线，但架构上要预留入口。

未来可以扩展：

- recording session 管理
- dataflow 运行时数据采集
- topic/sample preview
- image / point cloud / bounding box 等类型展示
- dataset export
- replay
- training job hook
- evaluation result viewer

这部分可以作为 dora-studio 区别于普通 dataflow GUI 的长期亮点。

## 5. UI 信息架构

建议第一版 UI 分为 4 个主页面。

### 5.1 Dashboard

用途：展示系统整体状态。

内容：

- coordinator 状态
- daemon 状态
- 当前运行中的 dataflows
- 最近错误
- resource summary
- 快速入口：打开 dataflow、启动运行、查看日志

### 5.2 Dataflow Explorer

用途：理解和编辑 dataflow 结构。

布局建议：

```text
┌──────────────────────────────────────────────┐
│ Top Bar: project / dataflow / actions         │
├──────────────┬───────────────────┬───────────┤
│ File / Flows │ Graph Canvas       │ Inspector │
│              │                   │           │
│              │ nodes + edges      │ node info │
│              │                   │ inputs    │
│              │                   │ outputs   │
├──────────────┴───────────────────┴───────────┤
│ Diagnostics / validation errors / warnings    │
└──────────────────────────────────────────────┘
```

本周静态原型应优先完成这个页面。

### 5.3 Run & Monitor

用途：运行和观测 dataflow。

内容：

- build/start/stop/restart 按钮
- dataflow status
- graph 状态颜色
- node metrics table
- daemon status

### 5.4 Logs & Events

用途：集中调试。

内容：

- log stream
- node filter
- level filter
- search
- error highlight
- event timeline

### 5.5 Debug Lab

用途：后续扩展。

内容：

- traces
- params
- debug topic
- payload preview
- recording/replay 入口

第一版可以只放入口或占位，不需要实现完整功能。

## 6. GSoC 阶段计划

### Community Bonding

目标：明确边界、完成设计、降低技术风险。

任务：

- 系统阅读 dora descriptor、CLI、coordinator、daemon、message crate
- 和 mentor 确认 dora-studio 是否作为 dora 组织下子模块推进
- 确认稳定集成方式：优先 coordinator protocol + dora-core/dora-message
- 完成 UI 信息架构设计
- 完成 API contract 草案
- 准备最小技术原型

产出：

- guidance.md
- API 草案
- UI 原型图或静态页面
- descriptor parse/graph demo

### Coding Phase 1 前半段

目标：完成最小可用工程骨架和 dataflow 可视化。

任务：

- 搭建 Rust backend
- 搭建 Vue frontend
- 实现 descriptor parse/validate/expand/graph API
- 实现 Dataflow Explorer 页面
- 用 mock data 完成 graph、inspector、diagnostics
- 初步接入真实 dataflow YAML

验收标准：

- 用户可以打开或提交一个 dataflow YAML
- Studio 可以显示 graph
- Studio 可以显示节点输入输出和错误信息
- UI 有较完整产品形态

### Coding Phase 1 后半段

目标：完成基本运行控制。

任务：

- 实现 coordinator 连接
- 实现 system status
- 实现 list dataflows
- 实现 build/start/stop/restart 基础 API
- 实现 Run & Monitor 页面基础版

验收标准：

- 用户可以从 Studio 查看 dora 系统状态
- 用户可以启动和停止至少一个简单 dataflow
- 用户可以看到运行中的 dataflow 列表

### Midterm Evaluation 目标

中期评估时应展示：

- 一个可运行的 dora-studio 原型
- dataflow graph 可视化
- 基础运行控制
- 初步运行状态展示
- 清晰的后续计划

### Coding Phase 2 前半段

目标：补足观测和日志能力。

任务：

- node metrics 查询
- graph 节点状态着色
- node detail panel 指标展示
- logs API
- logs stream
- Logs & Events 页面

验收标准：

- 用户可以看到 node-level 状态和资源指标
- 用户可以在 Studio 中查看和过滤日志
- Studio 可以明显改善 debug 体验

### Coding Phase 2 后半段

目标：增强 debug 能力和 polish。

任务：

- traces viewer
- params viewer/editor
- node restart/stop
- debug topic experimental viewer
- UI 细节优化
- 错误处理和文档

验收标准：

- Studio 不仅能运行 dataflow，还能辅助定位问题
- debug topic/trace/params 至少有一个形成可演示增强点
- 文档可指导新用户本地运行

### Final Submission

目标：形成可交付项目。

任务：

- 完成 README 和开发文档
- 完成架构说明
- 完成 demo dataflow
- 完成 mentor review 修改
- 整理未来路线图

最终交付：

- dora-studio backend
- dora-studio frontend
- dataflow graph viewer
- runtime status dashboard
- logs viewer
- basic lifecycle control
- 文档和 demo

## 7. 本周执行计划

本周目标以 UI 设计为主，API 只做核心 dataflow 接口的设计或轻量 mock。

### 7.1 本周必须做

1. 完成前端静态原型
   - Dashboard
   - Dataflow Explorer
   - Run & Monitor
   - Logs & Events 基础页面

2. 完成 mock 数据模型
   - mock dataflow graph
   - mock node metrics
   - mock logs
   - mock diagnostics

3. 完成 graph 页面
   - 节点和边展示
   - 节点点击后右侧 inspector
   - 状态颜色
   - validation panel

4. 完成核心 API contract 草案
   - descriptor parse
   - descriptor graph
   - system status
   - dataflow list
   - node metrics
   - logs stream

### 7.2 本周可以做但不强求

- Rust backend skeleton
- `/api/descriptors/graph` mock endpoint
- 读取本地 sample YAML
- 真实调用 dora-core 做 parse demo

### 7.3 本周不建议做

- 完整 visual editor
- 完整 start/stop 接入
- topic debug
- recording/training
- 多机部署
- 权限系统

## 8. Proposal 修改建议

建议把 proposal 的重点从“Web GUI for Dataflow Management”调整为：

> dora-studio: Visual Observability and Debugging Studio for Dora Dataflows

需要强调：

- Studio 是 dora CLI 的补充，不是替代
- MVP 核心闭环是 inspect → run → observe → debug
- visual editor 是一部分，但不是唯一核心
- logs、metrics、status、trace 对机器人 debug 更关键
- 后续预留 data collection / replay / training workflow

建议弱化：

- “完整编辑器”承诺
- “支持所有属性编辑”
- “替代高级 CLI 工作流”

建议强化：

- descriptor visualization
- runtime observability
- node-level metrics
- log streaming
- debug workflow
- stable integration with dora coordinator protocol
- future robotics data workflow foundation

## 9. 风险与应对

### 风险 1：dora 内部 API 不稳定

应对：Studio 后端封装一层自己的 REST/WebSocket API。前端只依赖 Studio API，不直接依赖 dora 内部协议。

### 风险 2：Visual editor 范围膨胀

应对：先做 graph viewer 和基础结构编辑；复杂属性编辑和动态 topology 后移。

### 风险 3：运行期信息不够完整

应对：先使用已有 NodeInfo、logs、trace、daemon heartbeat 和 fault-tolerance stats；缺少的信息作为后续 upstream API 改进点。

### 风险 4：UI 花费过多时间

应对：本周集中做静态原型，确定设计语言；后续逐步替换 mock 数据为真实 API。

### 风险 5：debug topic 依赖 unstable 功能

应对：debug topic 功能标记 experimental，不放入 MVP 核心验收路径。

## 10. 推荐的近期里程碑

### 里程碑 1：Studio UI 原型

- 静态页面完整
- graph viewer 好看可演示
- mock status/logs/metrics
- mentor 能直观看到产品方向

### 里程碑 2：Descriptor API

- parse/validate/expand/graph
- 接真实 YAML
- 错误诊断可显示

### 里程碑 3：Runtime Control

- system status
- list dataflows
- start/stop/restart

### 里程碑 4：Observability

- node metrics
- logs viewer
- live updates

### 里程碑 5：Debug Extensions

- traces
- params
- topic debug experimental

## 11. 当前结论

dora-studio 最稳妥的路径是：

1. 先用高质量 UI 原型明确产品方向。
2. 后端使用 Rust，稳定封装 dora 能力。
3. descriptor 能力优先用 `dora-core`。
4. runtime 控制优先用 coordinator control WebSocket 和 `dora-message`。
5. UI 优先实现 graph、status、logs，而不是一开始追求完整 visual editor。
6. 长期为机器人数据采集、回放、训练接口预留空间，但不把它们放入第一阶段主线。

这条路线既符合 GSoC 的可交付性，也更容易被 dora 社区接受和持续维护。
