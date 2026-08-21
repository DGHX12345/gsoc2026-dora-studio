# AI Handoff — dora-studio v2.0

## Quick Context

你是接手的 AI。项目是 **dora-studio**——一个为 dora-rs 构建的本地 Web GUI，用于可视化、运行和调试 DORA 应用（GSoC 2026 项目）。

**当前状态**：v2.0 架构，已完成 M00-M13（最新：M13 MoveIt Bridge）。134 后端单测 + 1 集成 + 204 前端工具测试通过，前端构建通过。

## 环境

```
工作目录: /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b
当前分支: week12-2.0
Rust:     1.75.0 (edition 2021)
Node:     (check with node --version)
```

## 如何运行

```bash
# 后端 (终端 1)
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b
cargo run --manifest-path backend/Cargo.toml
# 监听 127.0.0.1:3001

# 前端 (终端 2)
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b
npm --prefix frontend run dev
# 浏览器 → http://localhost:5173
```

## 如何验证

```bash
cargo test --manifest-path /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b/backend/Cargo.toml
npm --prefix /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b/frontend run build
```

## 架构

```
backend/src/        Rust + Axum 0.6 + Tokio
frontend/src/       Vue 3 + Vite + TypeScript
  纯 CSS，无 UI 框架
  Three.js 3D 视口 (NanoRobotViewer.vue)
  SVG 画布 (DataflowCanvas.vue) — 用于 dataflow 编辑

关键架构决策:
- CLI 子进程驱动 (coordinator.rs, runtime.rs, daemon.rs)
- 无 WebSocket 接入（dora coordinator 有 WS 协议但 Studio 未接）
- 前端轮询，非流式 (Dashboard 5s, Logs 1.2s)
- Fallback API 模式: withFallback<T>(path, fallback)
- 无 vue-router，通过 v-if 切换页面
```

## 已完成模块

### M00 — 类型定义（0.5 周）
**不能 link dora crates**。dora-rs 需要 edition 2024 + rustc 1.88，我们只有 1.75。
解决：复制关键类型定义。

新增文件:
- `backend/src/drec/types.rs` — .drec 格式 structs (RecordingHeader, RecordEntry, RecordingFooter)
- `backend/src/protocol/types.rs` — 协调器协议类型 (WsMessage, NodeInfo, LogMessage 等)
- `backend/Cargo.toml` — 新增 uuid 1.6.0, chrono 0.4.45（pin 到 rustc 1.75 兼容版本）
- `RUST-COMPAT.md` — 版本兼容策略文档

### M01 — Dataflow 编辑器画布（2 周）
可视化拖放 dataflow 编辑器，自动生成 dora YAML。

新增/修改文件:
- `backend/src/dataflow_builder.rs` — DataflowBuilder（add/remove/connect 节点, YAML 生成, 最小 YAML 解析器）
- `frontend/src/components/DataflowCanvas.vue` — SVG 画布（pan/zoom, 节点拖拽, 端口连线, Delete 删除）
- `frontend/src/components/NodePalette.vue` — 左侧节点面板（按分类搜索, HTML5 拖拽）
- `frontend/src/components/DataflowExplorer.vue` — 大幅改造（Source/Build 两个标签页, 工具栏, 状态栏）
- `frontend/src/api.ts` — 新增 buildDataflow, validateDataflow, parseDataflow
- `frontend/src/styles.css` — 新增 "machined-dark" 设计 token (Linear/n8n 启发)

API 端点:
- `POST /api/dataflow/build` — JSON graph → YAML
- `POST /api/dataflow/validate` — 验证 graph
- `POST /api/dataflow/parse` — YAML → JSON graph

### M02 — Arrow Schema 类型检查（1 周）
连线时检查端口类型兼容性。

新增/修改文件:
- `backend/src/schema_registry.rs` — 10 个内置算子类型, 兼容性检查（精确/扩展/收缩/不兼容）
- `frontend/src/api.ts` — 新增 checkSchema, getOperatorSchema
- `frontend/src/components/DataflowCanvas.vue` — 连线颜色（绿=兼容, 红=不兼容, 黄=警告, 灰=未知）

API 端点:
- `POST /api/schema/check` — 检查两个端口是否兼容
- `GET /api/schema/operator/:name` — 查询算子 schema

## 当前测试状态

```
backend: cargo test → 134/134 passed + 1 integration
frontend: npm run build → passes (vue-tsc + vite)
```

## 已完成模块: M04 — .drec 解析器 + 索引（1.5 周）

**目标**: 解析 dora 的二进制 `.drec` 录制格式，构建偏移索引以支持随机访问时间轴。

### 新增文件

- `backend/src/drec/reader.rs` — 二进制读取器（验证 MAGIC/版本/大小限制，`read_entry_at` 随机访问，`scan_entries` 顺序扫描，截断记录优雅处理）
- `backend/src/drec/index.rs` — 偏移索引构建器 + 查询 API（单次扫描构建，二分查找 `seek_to_timestamp`，按 (node,port) 分组流分页，时间范围查询）
- `backend/src/drec/service.rs` — RecordingManager（open/close 生命周期，`spawn_blocking` 异步加载，多录制并发）
- `backend/src/drec/generator.rs` — 合成 `.drec` 生成器（测试用，`generate_multi_stream` 可生成任意规模数据）
- `backend/src/drec/mod.rs` — 模块注册

### 修改文件

- `backend/src/main.rs` — +5 个 `/api/recording/*` 端点，AppState 集成 RecordingManager
- `backend/src/models.rs` — +`OpenRecordingRequest`, `RecordingOpened`, `SeekQuery`, `EntriesQuery`

### 新增 API 端点

- `POST /api/recording/open` — 打开 `.drec` 文件并构建索引
- `GET /api/recording/:id/streams` — 列出所有流
- `GET /api/recording/:id/seek?timestamp=T` — 按时间戳查找
- `GET /api/recording/:id/entries?node=&output=&offset=&limit=` — 分页获取条目
- `POST /api/recording/:id/close` — 关闭录制

### 二进制格式

```
Header:  MAGIC(8) + version(2) + start_nanos(8) + UUID(16) + yaml_len(4) + yaml
Record:  record_len(4) + node_len(2) + node_id + output_len(2) + output_id
         + timestamp_offset(8) + event_len(4) + event_bytes
Footer:  FOOTER_MAGIC(8) + total_messages(8) + total_bytes(8)
```

### 测试结果（自动化）

```
drec::reader::tests   — 7 passed (header/single/multi/footer/random/corrupt/magic)
drec::generator::tests — 2 passed (empty/deterministic)
drec::index::tests    — 5 passed (empty/seek/streams/pagination/range)
drec::service::tests  — 2 passed (open_query/open_close)
```

### 关键决策

- **不使用 memmap2** — 避免依赖膨胀，`File::seek + read_exact` 足够
- **不使用 tempfile crate** — `getrandom v0.4` 需要 edition 2024 (Rust 1.88)，用 `std::env::temp_dir` + UUID 替代
- **索引在 `spawn_blocking` 中构建** — 不阻塞 tokio runtime
- **无前端改动** — M04 纯后端模块，API 就绪供 M05 Timeline UI 消费

## 已完成模块: M03 — Runtime 绑定（1.5 周）

**目标**: 连接 DataflowCanvas 到真实 dora runtime。节点显示实时状态，Python 算子支持 Hot Reload。

### 新增/修改文件

- `backend/src/coordinator_ws.rs` — WebSocket 客户端（原始 TCP + 手动 WebSocket 帧，无外部 WS 依赖；Bearer auth + Hello 握手 + JSON-RPC 请求/响应）
- `backend/src/runtime.rs` — +`run_yaml()` 方法（YAML → 临时文件 → `dora run`）
- `backend/src/main.rs` — +`/api/runtime/nodes/:id`、+`/api/runtime/nodes/:id/reload`、+`/api/dataflow/run`，AppState 集成 WS 客户端
- `backend/src/models.rs` — +`NodeRuntimeStatus`、`ReloadRequest`
- `backend/Cargo.toml` — +`getrandom` 0.2、+`sha1` 0.10、uuid +`v4` feature
- `frontend/src/api.ts` — +`getRuntimeNodeStatuses`、`reloadNode`、`runDataflow`
- `frontend/src/components/DataflowCanvas.vue` — 节点状态轮询(2s)、彩色状态指示点+脉冲动画、右键上下文菜单(Hot Reload)、节点名截断、独立 operator 行、`dataflowId` prop
- `frontend/src/components/DataflowExplorer.vue` — Build 工具栏 Run/Stop 按钮

### 新增 API 端点

- `GET /api/runtime/nodes/:dataflow_id` — 节点状态(WS → CLI → 运行时状态 逐级回退)
- `POST /api/runtime/nodes/:dataflow_id/reload` — Hot Reload (需要 WS)
- `POST /api/dataflow/run` — 构建 YAML 并立即运行

### 关键决策

- **不使用 tokio-tungstenite**：依赖链需要 edition 2024 (Rust 1.88)，我们使用原始 TCP + 手动 WebSocket 帧
- **Coordinator 端口**：默认 6013，可通过 `DORA_COORDINATOR_PORT` 覆盖
- **Auth token 发现**：`DORA_AUTH_TOKEN` env → CWD `.dora-token` → `~/.config/dora/.dora-token`
- **`dataflowId` prop 可选**：Builder 模式下不传，不触发轮询
- **Run 流程**：Generate YAML → POST `/api/dataflow/run` → 写入临时文件 → `dora run`

### M03 测试结果（人工）

- D1 WebSocket 客户端：✅ 连接/握手/Token 发现逻辑通过单元测试
- D2 节点状态 API：✅ CLI 回退返回正确状态
- D3 画布状态覆盖：✅ 节点卡片独立 operator 行，无文本重叠
- D4 Hot Reload：✅ 右键菜单正确弹出，Python 节点显示 Hot Reload（未 Run 时灰色禁用）
- D5 Run/Stop：✅ 工具栏 Run/Stop 按钮完整流程
- 回归：✅ Dashboard / Run & Monitor / Logs / Motion Planner / Visualization 均正常

## 已完成模块: M07 — 指标面板 (Metrics Panels)

**目标**: Dashboard 展示节点级实时 CPU/内存指标。

**重要发现**: 计划中的 `dora top --once --format json` 命令**不存在**于 dora 0.5+。实际改用 `dora node list --format json`（NDJSON 格式，含 cpu%/memory/status/restarts/pid）。

### 新增/修改文件

- `backend/src/metrics.rs` — MetricsCollector（2s 轮询 `dora node list --format json`，环形缓冲 300 样本/节点；静默处理 coordinator 未连接和 `dora node` 子命令不存在的情况）
- `backend/src/main.rs` — +`/api/metrics/nodes`、+`/api/metrics/nodes/:id/history?window=S`，AppState 集成 MetricsCollector
- `frontend/src/components/MetricsDashboard.vue` — Performance 页面（Summary bar、Canvas CPU/内存时间线、节点指标卡片网格、Topic Metrics unavailable 说明）
- `frontend/src/api.ts`、`frontend/src/i18n.ts`、`frontend/src/types.ts`、`frontend/src/App.vue` — Performance 页面集成（Dora 分区，icon 08）

### 测试

```
metrics::tests — 14 passed (NDJSON 解析、字段转换、环形缓冲、更新逻辑)
```

## 已完成模块: M08 — OTel Flame Graph

**目标**: 拉取 OTel span 数据，渲染火焰图定位 CPU 热点。

**架构决策**: dora 通过 OTLP **gRPC** (4317) 导出 spans 到外部后端（Jaeger/Tempo/Collector），Studio 无法直接查询 dora。正确路径：Studio 查询 **Jaeger 兼容 HTTP API**（默认 `http://localhost:16686`，可用 `DORA_OTEL_QUERY_ENDPOINT` 覆盖）。

### 新增/修改文件

- `backend/src/otel.rs` — OtelCollector（5s 轮询；两步 fetch：`/api/services` → 每 service `/api/traces?service=X`；环形缓冲 1000 spans；**最新优先**返回）；Jaeger v1 JSON 解析 → OtelSpan；span 树构建（父子关系）；**原始 TCP 最小 HTTP 客户端**（含 chunked transfer-encoding 解码，延续无 reqwest 策略）
- `backend/src/main.rs` — +`/api/otel/status`、+`/api/otel/spans?node=N&limit=L`、+`/api/otel/trace/:trace_id`
- `frontend/src/components/FlameGraph.vue` — 火焰图（横向堆叠条、点击放大+面包屑、点击背景缩小、搜索高亮、悬停 tooltip、max-height 420px 内滚动）
- `frontend/src/components/MetricsDashboard.vue` — Trace Flame Graph 面板（中置控件行、Node 过滤下拉、时间范围按钮 30s/2min/5min/1h、未连接时显示 Jaeger 配置说明）

### 测试

```
otel::tests — 10 passed (Jaeger 解析、树构建、孤儿父节点、services 解析、
              URL 构建、chunked 解码、最新优先、节点过滤)
```

### 完整链路验证（已人工验证）

Jaeger Docker 容器 (`jaegertracing/all-in-one`) + Studio 后端：`connected: true`、spanCount 持续累积、火焰图渲染真实 spans（Jaeger 自追踪数据）。

## 已完成模块: M09 — VLM/LLM Attribution Bar（归因条）

**目标**: 渲染 摄像头帧 → VLM 提示词 → LLM 回复 → 解析动作 → 执行结果 的因果链，解释机器人"为什么"做决策。

### 关键决策（已与学生确认，写入 M09 计划 Revision 2026-08-13）

- **UI 位置**: AttributionBar 集成进 VisualizationView Replay 模式，浮动回放条上方的可折叠覆盖层（纯加法，未重构现有面板）；"Show in 3D" 复用现有 `PlaybackEngine.seek`，点击后自动折叠面板
- **D4 范围**: 仅 seek；planned vs actual 幽灵渲染移给 M12/M13
- **数据格式**: 真实 dora VLM 输出是 Arrow IPC，本环境无法解析。采用 Studio 结构化载荷（magic `DORAATT\0` + 长度前缀字段）；Arrow IPC 流诚实标记 unparseable，不伪造

### 新增/修改文件

- `backend/src/attribution.rs` — AttributionStep/Chain/Event 类型、二进制编解码（手写 cursor，无新依赖）、AttributionExtractor（单次顺序扫描 .drec，按 frame_timestamp 分组组装链）、AttributionSummary
- `backend/src/drec/generator.rs` — +`generate_vlm_attribution`（40 链演示，每第 5 条失败，关节条目与动作向量同角度驱动 3D）+`generate_joint_animation`（132 帧无归因，空状态测试）
- `backend/src/main.rs` — +`GET /api/recording/:id/attribution`（摘要）、`GET /api/recording/:id/attribution/chain?timestamp=T`（详情），spawn_blocking 提取
- `frontend/src/components/AttributionBar.vue` — 图标链条（绿=成功/红=失败）、详情卡（5 步、>200 字符折叠、token 流 50 tok/s 动画、动作向量表）、‹›导航、数据源下拉（.drec 可用 / LeRobot M10 占位 / Live 需运行中 dataflow）、放大版空状态
- `frontend/src/components/VisualizationView.vue` — 回放模式叠加 AttributionBar；**修复黑边**（全局 `.viz-robot-viewer-card` 残留 `grid-template-columns: 1fr 300px` 空列，作用域内覆盖为 block）；路径输入框加大；Load 失败显示红色错误
- `frontend/src/api.ts`、`frontend/src/i18n.ts` — attribution 类型/函数/中英双语

### 测试

```
attribution::tests — 15 passed（编解码往返×5、Arrow IPC/未知 magic/截断/未知 kind 拒绝、
                     链组装顺序、unparseable 上报、success 标志、camelCase JSON、summary 映射、演示文件）
generator::tests   — +2 (joint_animation 条目结构 + 演示文件写出)
```

### 人工验证（已通过）

归因条渲染、详情卡 5 步、token 流动画、Show in 3D 视口跳转+自动折叠、空状态（joint_animation.drec）、黑边修复、Load 错误提示。

### 演示文件

- `/tmp/dora-studio-tests/attribution_demo.drec` — 40 链归因演示
- `/tmp/dora-studio-tests/joint_animation.drec` — 132 帧空状态测试

## 已完成模块: M10 — LeRobot Profile System

**目标**: 加载 LeRobot 数据集（Parquet）到归因条，Profile YAML 把各机器人字段名映射到统一归因 schema。已用真实 B601 数据端到端验证。

### 关键决策（已与学生确认，写入 M10 计划 Revision 2026-08-13）

- **Parquet 解析用 Python 子进程桥**（用户优先决策）: 目标用户必然有 Python+pyarrow（lerobot 强制依赖）；Rust 手写解析不可行。`python3 backend/scripts/lerobot_reader.py`，JSON stdout 协议、30s 超时、pyarrow 缺失优雅降级
- **每帧一条链**；LLM 回复/执行结果缺失 → UI "数据集中未记录"占位，不伪造
- **UI 在归因面板内扩展**（数据源下拉 LeRobot 项）；归因面板 Replay 模式即可见（无需先加载 .drec）
- **B601 优先**；SO-100/GEN72 为模板配置
- **诚实性**: confidence/success 改 Option（载荷 v2）、`angle_unit: degrees` 配置字段（B601 数据是度，前端 deg→rad 转换再预览）、"在 3D 中查看"在 Nano 模型上预览并加提示标签（真实 B601 URDF 渲染挂 M13 D4.1）
- B601 = Seeed Studio 开源 reBot Arm。**模型已获取并验证**（学生本地克隆）: `/home/dora/reBotArmController_ROS2/src/rebotarm_bringup/description/`，主文件 `urdf/reBot_B601_DM_with_gripper.urdf`（joint1-6 + 夹爪），网格在 `meshes_b601_gripper/`（package:// 路径需重映射）。接入计划已写入 M13 D4.1

### 新增/修改文件

- `backend/scripts/lerobot_reader.py` — scan/frames/gen-demo 子命令；v1（episode_*.parquet）+ v2（chunk-*/file-*.parquet）布局；列名用 schema_arrow.names（List 列否则显示 element）；任务描述取非 task_index 列（B601 是 pandas 遗留名）
- `backend/src/lerobot.rs` — tokio 子进程桥（30s 超时）、DatasetInfo/FrameData、`chains_from_frames`（每帧 SensorFrame+Prompt+ParsedAction）、check_status
- `backend/src/profile.rs` — 零依赖行级 YAML 解析、字段别名（首个命中优先）、AngleUnit、ProfileManager（list/load/autodetect ≥0.5 分）
- `profiles/` — b601/so100/gen72 三配置 + README（格式/别名/angle_unit 文档）
- `backend/src/main.rs` — +6 端点 `/api/lerobot/{status,scan,frames,profiles,autodetect,attribution}`，AppState 加 ProfileManager
- `backend/src/attribution.rs` — confidence→Option<f32>、success()→Option<bool>、载荷 VERSION 2
- `frontend/src/components/AttributionBar.vue` — LeRobot 数据源（路径扫描/数据集信息/配置+Episode 选择/分页 200）、缺失步骤占位、中性灰链（success null）、deg→rad 转换、Nano 预览标签
- `frontend/src/components/VisualizationView.vue` — apply-action 写 Replay 关节（不切模式，面板保持挂载）；**侧栏文字浅色修复**（machined-dark 背景 + 浅色主题黑字不可见）
- `frontend/src/api.ts` — fetchJson 错误带后端 message；lerobot 类型/函数
- `frontend/src/i18n.ts` — lerobot 中英双语

### 测试

```
profile::tests  — 8 passed（解析/别名/自动检测/angle_unit/ProfileManager）
lerobot::tests  — 7 passed（v1/v2 scan、frames 分页+时间归一化、错误上报、
                  chains_from_frames 映射/回退、真实 B601 端到端[缺失跳过]）
attribution::tests — 15 passed（v2 载荷、Option confidence/success）
后端总计 110 passed + 1 integration；前端构建通过
```

### 人工验证（已通过）

B601 真实数据扫描（5 episodes、任务描述入 Prompt）、链条/分页/详情卡占位、
Show in 3D 摆姿态（deg→rad）、面板折叠保持挂载、.drec 源回归。

### 演示数据

- LeRobot 真实: `/home/dora/.cache/huggingface/lerobot/my_org/b601_pilot_v1`
- 合成（测试生成）: `/tmp/dora-studio-tests/lerobot_demo_v{1,2}`

## 下一个模块: M11 — Tool Slot Protocol

**计划文件**: `plans2.0/M11-tool-slot-protocol.md`（M09/M10 归因线完成，Layer 4 收口）

## 已完成模块: M11 — Tool Slot Protocol

**目标**: 标准化视口工具挂载协议（Layer 5 起点）——工具注册、挂载、按端口订阅广播、时间轴 seek 通知、推荐引擎、侧栏面板。

### 关键决策（已与学生确认，写入 M11 计划 Revision 2026-08-13）

- **R1 flat payload**: 计划中的 `ArrowBatch`（schema + columns Map）不存在——M12 审计确认 dviz/moveit 数据是 flat Float32Array 或 JSON bytes，无结构化 Arrow。协议载荷改为 `ToolPayload { f32?/json?/bytes? }` 联合，工具自行解析
- **R2 D3 Rust 桥取消**: `ViewportToolBridge` 引用的 `RecordBatch`/`TfTree` 在代码库中不存在且无消费者，改为前端匹配引擎（原生 RegExp + tsx 测试模式）。验收标准 "cargo test includes tool bridge routing test" 修订为 `npm run test:tools`
- **R3 TfTree 保留 + 最小实现**: 学生提供并核实 dviz 真实 TF 格式（`StampedTransform { parent_frame, child_frame, translation, rotation(xyzw) }`；ROS `tf2_msgs/TFMessage | tf/tfMessage`；帧名前导 `/` 去除 + 环检测）。dora 无官方 TF 消息，.drec 回放中 TF 为 JSON 载荷流。`onBatch` 的 tf 可选
- **R9 ToolContext**: NanoRobotViewer 按需渲染（GPU 空闲降 0），`onAttach(context)` 接收 `{ scene, camera, requestRender }`（原计划裸 scene/camera 对无法触发重绘）
- **R10 推荐数据源**: 匹配用 .drec streams（Replay 加载时触发）；dataflow YAML 侧留待 M12
- **R11 面板绑定**: panelComponent 在 `tools/index.ts` 注册时绑定（.vue 无法被 tsx 加载，工具类纯 TS）；注册幂等

### 新增/修改文件

- `frontend/src/tools/types.ts` — ViewportTool/PortPattern/ToolPayload/ToolBatch/ToolContext/ToolStatus（协议核心）
- `frontend/src/tools/tf.ts` — SimpleTfTree（Matrix4 组合、父链查找、环检测、parseTfPayload）
- `frontend/src/tools/registry.ts` — ToolRegistry 单例（register/unregister/attach/detach/broadcastBatch/broadcastSeek、工具隔离、subscribe 通知）
- `frontend/src/tools/matching.ts` — patternMatches（string glob 锚定不区分大小写 / RegExp 原样）+ findRecommendations（D4）
- `frontend/src/tools/feed.ts` — entryToToolBatch（.drec 条目 → ToolBatch；数字数组→Float32Array）
- `frontend/src/tools/demo/WaypointEchoTool.ts` + `WaypointEchoPanel.vue` — M11 参考工具（订阅 waypoints|path|trajectory，3D 路径线 + 起终点标记）
- `frontend/src/tools/index.ts` — registerBuiltinTools（幂等）
- `frontend/src/components/ToolPanel.vue` — 工具面板（分类分组、状态 pill、挂载/卸载开关、推荐徽章、可展开控制面板）
- `frontend/src/components/VisualizationView.vue` — 顶栏 Tools 按钮 + ToolPanel 覆盖层；startReplay 后拉 streams 计算推荐；unmount 时全量 detach
- `frontend/src/components/NanoRobotViewer.vue` — defineExpose({ getScene, getCamera, requestRender })
- `frontend/src/replay-scene.ts` — 广播 hook：每帧条目 → entryToToolBatch → broadcastBatch（TF 载荷先入树）；broadcastSeek
- `backend/src/drec/generator.rs` — +`generate_tool_demo`（planner/waypoints+trajectory、tf_broadcaster/tf、robot_state、camera/image）
- `frontend/src/i18n.ts` — tools 词条中英双语；`frontend/package.json` — +`test:tools`

### 测试

```
tools tsx — 54 passed（tf 13 / matching 10 / registry 17 / feed 5 / waypoint-echo 9）
backend   — 111 passed + 1 integration（+2 tool_demo generator）
前端构建通过（vue-tsc + vite）
```

### 人工验证

- 演示文件: `/tmp/dora-studio-tests/tool_demo.drec`（120 帧：图8 waypoints + trajectory + TF 链 + 关节动画 + 无关 camera 流）
- 验证路径: Visualization → Replay 模式加载 tool_demo.drec → 顶栏 Tools → Waypoint Echo 显示"推荐"徽章 → 挂载 → 播放时 3D 视口出现青色路径+绿/红标记；camera 流不触发工具；面板控制项展开正常；页面切换后状态重置

## 已完成模块: M12 — dviz Path Visualization Tool（D1-D5）

**目标**: 按 dviz 语义渲染规划路径: flat Float32Array 解析、2px 路径线、costmap 平面、控制面板、数据流推荐。

### 关键决策（写入 M12 计划 Revision 2026-08-13/14）

- **R1 接口修正**: 计划旧签名(onAttach(scene,camera)/ArrowBatch/renderPanel)全部按 M11 协议改为 ToolContext/ToolPayload/panelComponent 注册绑定
- **R2 Line2**: 核心 linewidth 被忽略,用 three/examples/jsm 的 Line2/LineMaterial 实现 2px(主路径实线节点色,备选路径虚线白 30%)
- **R3 costmap 载荷**: 单 JSON 对象 {width,height,resolution,values}; 渲染为 RGBA DataTexture 平面(z=0.02),蓝→黄→红 LUT,同尺寸复用纹理
- **R4 D5 状态收缩**: 视口仅回放数据源,M12 只做数据新鲜度 stale 徽章(100ms 阈值,2×帧窗口);运行时节点状态留 M13
- **R5 snap camera**: NanoRobotViewer 加 defineExpose focusOn(同步 controls.target 防回弹);ToolContext 加可选 focusOn 字段
- **R6 echo 移除**: DvizPathTool 人工验收通过后删除 WaypointEchoTool(已执行,三文件+注册+测试)
- **R7 面板响应性**: batch 广播不触发面板重渲染,工具暴露 subscribe(listener),面板 onMounted 订阅
- **R8 数据流推荐**: getDataflows+getDataflowGraph 的节点输出 → findRecommendations+mergeRecommendations;与 .drec 流推荐之间有请求令牌竞态守卫
- **R9**: 四端口订阅(waypoints|path、trajectory、target_point|target|goal、costmap|esdf);每节点配色 青/品红/橙
- **R10 demo**: generate_tool_demo 加 target_point(沿图8步进)+costmap(24×24 高斯障碍)+**planner 静默间隙 60-89 帧**(stale 徽章可演示);新增 examples/planner-demo/dataflow.yml 结构示例(推荐演示)
- **R11**: dviz world 话题已是 world-frame,路径不做 TF 变换(tf 留给 M13)

### Bug 修复（M06 遗留,测试中暴露）

- **时间显示冻结**: PlaybackEngine.onTick 是单槽回调,VisualizationView 的显示回调被 ReplayScene.attach 覆盖 → 浮动条时间永远 0:00.000。修复:onTick 改多监听器集合(TDD,playback.test.ts,纳入 test:tools)

### 新增/修改文件

- `frontend/src/tools/dviz/` — parse.ts/format.ts/DvizPathTool.ts/DvizPathPanel.vue + 3 个测试文件
- `frontend/src/tools/matching.ts` — +mergeRecommendations;`tools/index.ts` — 仅注册 dviz-path;`tools/tests.ts`
- `frontend/src/components/VisualizationView.vue` — focusOn 接线、数据流推荐、浮动条对比度修复(用户反馈)
- `frontend/src/components/NanoRobotViewer.vue` — +focusOn expose
- `frontend/src/playback.ts` — onTick 多监听器;`frontend/package.json` — test:tools 加 playback 测试
- `frontend/src/i18n.ts` — tools.dviz 中英双语
- `backend/src/drec/generator.rs` — target_point + costmap 流 + 静默间隙 + 3 测试
- `examples/planner-demo/dataflow.yml` — 结构示例(无脚本,注释说明)

### 测试

```
tools tsx — 116 passed（114 tools + 2 playback;移除 echo 9 测试后）
backend   — 125 unit + 1 integration
前端构建通过（vue-tsc + vite）
```

### 人工验证（已通过）

D1+D2 路径渲染(青 2px 图8+绿/红标记+箭头+粉目标)、D3 costmap(蓝黄红斑块平铺地面)、D4 面板(路径列表/显隐/滑块/聚焦/中英)、D5 数据流推荐徽章 + 静默间隙 stale 徽章演示。用户反馈已修:面板字体对比度、浮动条对比度(两次"太浅"反馈,已入记忆)。

### 已知问题

- 后端偶发 flaky(~1/70,~145 轮未能复现定位):疑似 metrics/otel start-stop 时序测试;M12 diff 不含时序改动,判定既有问题。若复现,先查 metrics.rs:746 / otel.rs:798 的 1s timeout。

## 已完成模块: M13 — MoveIt Bridge（D1-D7）

**目标**: 消费 dora-moveit2 输出并在视口渲染——轨迹/幽灵姿态/当前姿态、规划场景碰撞线框、独立轨迹播放器与模型选择器。

### 关键决策（写入 M13 计划 Revision R1 2026-08-14，学生确认）

- **R1 审计修正**: plan_status 实为 7 键、execution_status 5 键、joint_positions 是 float64（json 通道保精度）、.drec 无 metadata（reshape 靠 D3 关节数）、真实 .drec 是 Arrow IPC 字节（诚实 unsupported，M15-B 直播桥发 JSON 后自然消失）、executor 空闲持续发 HOME
- **D4 Option B**: 工具自带 Three.js URDF 加载器（最小 XML 解析器→关节树→FK；STL+package:// 重映射）；一个机制覆盖 UR5e/GEN72/B601；不侵入 NanoRobotViewer
- **B601**: 复制到 models/b601/（27MB，gitignore，/models 静态服务）；夹爪限位实为 0-0.0715m（交接曾写 0.0285 是旧变体）；lerobot 预览切 B601（previewPose，夹爪 56.8°→m 线性映射）
- **端口碰撞**: moveit demo 用 envelope 形式 + 不含 dviz trajectory 流；扁平 reshape 留 M15-B 直播（带 metadata）
- **模型选择器**: 后端 GET /api/models 动态发现（models/*/*.urdf），加模型零代码改动
- **nano 替换**: 工具挂载即隐藏 nano 模型（modelVisible prop，只藏模型不藏画布——v-show 藏画布曾致工具渲染全黑）；可见性只由注册表事件驱动（onDetach 清空监听器曾致卸载重挂后 nano 不再隐藏）

### 新增/修改文件

- `frontend/src/tools/moveit/` — parse/types/joint-config/collision/MoveItTool/MoveItPanel + urdf/（xml/urdf/robot/meshes）+ 6 个测试文件
- `frontend/src/components/NanoRobotViewer.vue` — +modelVisible prop
- `frontend/src/components/VisualizationView.vue` — nano 可见性接线、previewPose 预览切换
- `frontend/src/components/AttributionBar.vue` — apply-action 带 profile robot、预览标签按模型隐藏
- `backend/src/model_catalog.rs`（新）— models/ 目录扫描；`main.rs` +GET /api/models
- `backend/src/drec/generator.rs` — +generate_moveit_demo（真实节点名 planner/planning_scene/trajectory_executor/mujoco_sim + dviz 流）

### 测试

```
后端   — 134 passed + 1 integration（+3 model_catalog）
前端   — 204 tools tests（+79 moveit：解析 31/关节配置 6/工具 19/URDF 30/碰撞 5/协同 2 等）
构建   — vue-tsc + vite 通过
```

### 人工验证（已通过）

B601 加载替换 nano、EE 路径+幽灵姿态、lerobot 预览切 B601、碰撞线框、面板全功能（播放器/关节表/模型选择器/幽灵滑块/碰撞开关）、dviz+moveit 同视口协同、卸载重挂无共生、对比度修复（黑字）。

### 已知问题

- **B601 播放卡顿**（学生接受延期）: 165 万顶点 × (1+幽灵数) ≈ 千万级/帧。修复选项: 离线减面 / 骨架幽灵 / 播放时隐藏幽灵（见 M13 计划）
- ur5e（6 关节）选中时 dviz xyz 流长度若整除 6 可能误 reshape（演示数据不受影响；M15-B 带 metadata 无此问题）
- 后端偶发 flaky 测试仍在（见 M12 记录）

## 下一个模块: M15 — Live Planning & Simulation

**计划文件**: `plans2.0/M15-live-planning-simulation.md`。M13 已交付 Phase A（回放式规划可视化）；Phase B（环境 spike、studio_bridge 节点、/api/live/ingest、LiveFeed、端到端演示、Motion Planner 控制台接线）1.5 周；Phase C（mujoco 物理镜像+避障）可选 1 周。**M11.5 D3**(内置 OTLP HTTP 接收器,prost 预检已过)时机: M15 前后、Final 演示前。

**2026-08-17: M15 Phase C 已交付**（见 HANDOFF-M15.txt 的完成记录）——C1 mujoco 物理镜像（ur5e, ≥20Hz via 50ms LiveFeed）、C2 真实碰撞避障（dora-moveit2 stub 解除 + DH FK）、C3 验收、箱子控制台、历史命令跳过。测试基线: 后端 154+1、Python 46+37+14、前端 225。遗留: 每连杆单球碰撞的穿透限制（延期，计划已记录）+ daemon 路由退化 issue（已报社区）。

## 已完成模块: M15.5 — dora 1.0 Upgrade（2026-08-17 交付）

WS 客户端对齐 1.0（Hello 变体包装、GetNodeInfo 回复解包、写循环 pending 登记 bug 修复、类型逐字段对齐、版本串动态化、running 大小写、.drec 中段截断容错）；新 venv `/home/dora/.venvs/dora-studio-1.0`（PyPI cli wheel rc1/rc2 是坏的 → GitHub release 二进制；dora-rs==1.0.0rc4 与 daemon 配对）；**WS 路径首次真实激活**（监测 source: "ws"）；B5 数据流 5 节点 Running；真实 1.0 .drec fixture 入库（backend/tests/fixtures/dora10.drec）。测试基线: 后端 186+1、前端 204、Python 46+37+14。9 个 commit，详见 plans2.0/M15.5-dora-10-upgrade.md 交付记录。

## 已完成模块: M15.6 — OTLP gRPC Receiver（2026-08-17 交付）

dora telemetry 是 gRPC-only（`DORA_OTLP_ENDPOINT`），新增 4317 gRPC 接收器（tonic 0.12.3 + indexmap=2.7.0 钉死；手写 tonic 服务样板无 protoc；metrics stub 接收丢弃）；火焰图达成免 Jaeger 推送模式（e2e 实测 10k+ spans，connected:True 纯推送）。**已知限制**: dora 从不设置 service.name → spans 归因 unknown_service:python3.11，**已提交上游 PR**（DGHX12345/dora:fix/otel-service-name）。5 个 commit，详见 plans2.0/M15.6-otlp-grpc-receiver.md 交付记录。

**UI 修复（2026-08-17, e4ad684）**: Performance 页 health 详情可展开（原 slice(0,2) 截断）、Memory Timeline 图例悬停全名、Node Metrics 节点名主题 token 对比度 + 固定截断。

**下一步（计划已批准,只设计未实施）**: M16.5 去终端化（plans2.0/M16.5-terminal-free-ops.md，两 PR: 生命周期/录制）→ M17 版本管理 → M18 Dataflow Explorer 2.0。阻塞项: week12-2.0 与 main 合并策略等 mentor 同步。

## 新需求(2026-08-14,已立项)

- **M15 Live Planning & Simulation**(`plans2.0/M15-live-planning-simulation.md`)——**必要编码,学生已确认纳入项目**。rviz 式规划+模拟 = Studio 前端 + dora-moveit2 规划 + dora-mujoco 物理(照搬 ROS 三件套分工)。Phase A 随 M13 交付;Phase B(实时链路,studio_bridge 节点,无新 Rust 依赖)1.5 周;Phase C(物理镜像+避障)1 周。**已核查**: python3.11/3.12 + uv 已装(环境升级可行);dora-mujoco 完整可用;moveit2 碰撞检测是 stub 需补。**放置位置**: 渲染/演示在 Visualization 视口(M15 工具挂载);Motion Planner 页面演化为规划控制台(见下)。
- **M16 LeRobot Interop**(`plans2.0/M16-lerobot-interop.md`): 与 dorobot-studio **不合并代码**,做数据互通(dorobot 写的标准 LeRobot v2 数据集 → Studio M10 直接打开;reader 补 tasks.jsonl/episodes.jsonl 支持)。0.5 周。**已核查**: dorobot 的 lerobot_writer.rs 写标准 v2.x 布局。**非必要编码**(NON-ESSENTIAL):做与不做不影响项目完整性,时间紧可跳过。

## 已完成模块: M11.5 — Monitoring Control & Efficiency（D1+D2）

**目标**: 监测改为按需开启的"诊断模式"（默认关闭、零启动轮询），节点指标数据源从 CLI 子进程升级为 Coordinator WebSocket。

### 关键决策（写入 M11.5 计划 Revision 2026-08-13）

- **R1 watch channel**: 取消机制用 tokio 自带 `watch::Sender::send_replace`（无接收者也能更新状态；无新依赖）。start() 先 subscribe 再 send_replace(true)；stop() send_replace(false)；`join()` 等循环退出（测试确定性）
- **R2 D3 延后但非时间驱动**: 学生已获 Final 延期（2026-11-02）；D3（内置 OTLP HTTP 接收器，prost+protobuf）开工前先做 `cargo add prost` 在 Rust 1.75 的编译预检
- **R3 WS 断连不加重连**: 接受 "WS 断 → 5s 超时 → 该次尝试回退 CLI" 降级
- **R6 PolledNode 统一中间类型**: CLI NDJSON 解析与 WS NodeInfo 映射产出同一 `PolledNode`，轮询循环与数据源解耦；`MetricsSource::{Cli, Ws}` 枚举选择数据源
- **R7 运行统计口径**: sample_count = 轮询尝试次数（含空结果）；status 含 enabled/source/sampleCount/lastPollAt
- **前端主题教训**: MetricsDashboard 旧 `--col-*` token 已在 week12 主题重构中移除、静默回退硬编码深色值，浅色主题白底上近白文字不可见。改用 `--text-primary`/`--text-secondary`/`--bg-surface`/`--border-card` 主题感知 token（用户两次反馈对比度，已入记忆）

### 新增/修改文件

- `backend/src/metrics.rs` — MetricsCollector 可启停（watch + JoinHandle）；poll_loop 提取为可注入测试的泛型函数；PolledNode；`info_to_polled`（WS NodeInfo → PolledNode）；`poll_ws_with_cli_fallback`
- `backend/src/otel.rs` — OtelCollector 同样可启停；poll_interval 入 inner（测试可调短）
- `backend/src/monitoring.rs`（新）— MonitoringController（MonitorTarget::{NodeMetrics,OtelSpans}、set_enabled/status）
- `backend/src/coordinator_ws.rs` — +`all_node_infos()`（GetNodeInfo 无参数、返回全部节点）；node_statuses 复用；status_to_string 改 pub(crate)
- `backend/src/main.rs` — AppState 持 MonitoringController；**启动零轮询**；`GET /api/monitoring/status` + `POST /api/monitoring/toggle`；metrics 用 `new_with_ws`
- `backend/src/models.rs` — +MonitoringToggleRequest（Option<bool> 部分开关）
- `frontend/src/api.ts` — +getMonitoringStatus/setMonitoringToggle + 类型
- `frontend/src/components/MetricsDashboard.vue` — 顶部监测控制条（主开关 + 分开关 + 样本计数）；关闭时三面板"监测已关闭"空状态 + 一键开启；localStorage 持久化（刷新后重开后端）；主题 token 修正
- `frontend/src/i18n.ts` — +monitoring 词条中英双语

### 测试

```
backend — 123 passed + 1 integration（+6 启停/回退测试，连跑两次无 flaky）
tools tsx — 54 passed；前端构建通过
```

### 人工验证（已通过）

- 后端 curl：启动全 disabled → toggle 开 → 3s 内 sampleCount>0 → 关后冻结；D2 开启后 `source: "cli"`（本机无协调器 → WS 失败自动回退）
- 前端：Performance 页开关组、空状态、主/分开关、刷新保持、中英切换；浅色主题对比度修复后确认

### D3 已交付（2026-08-17，测试全绿 + 学生手动测试通过）

内置 OTLP HTTP 接收器：`backend/src/otlp.rs`（prost 0.13 手写最小 OTLP 类型 +
解码 + `POST /v1/traces` 路由，16MB 限制，空 protobuf 响应=规范 200）；
`OtelCollector::ingest()` + receivedCount/lastReceivedAt；`connected` 语义 =
「轮询成功或收到推送」；main.rs 第二监听器 4318（`DORA_STUDIO_OTLP_ADDR`
覆盖，端口占用警告降级不退出）；前端空状态文案加 OTLP 推送说明（中英）。
测试 +12（otlp 9 + otel 3，含独立手写 wire encoder 的 golden 字节测试）。
基线：后端 166+1、B5 46、M15 37、bridge 14、前端 226、build ✓。

**已知限制（学生已确认，方向 A 立项）**：dora 0.5/1.0-rc.4 的 telemetry
导出器硬编码 `with_tonic()`（gRPC-only；开关是 `DORA_OTLP_ENDPOINT` 而非
标准 `OTEL_EXPORTER_OTLP_ENDPOINT`），dora 节点无法推送到 HTTP 接收器。
"免 Jaeger 推送模式"由 **M15.6 OTLP gRPC 接收器**（`plans2.0/` 计划随
M15.5 后实施）达成；HTTP 接收器保留为标准 OTel 生态兼容层。

## 已完成模块: M05 — Timeline UI + M06 — 3D Viewport Sync + M06-UI 改造

**M05** (ReplayTimeline.vue + playback.ts): 时间轴回放控件、PlaybackEngine 状态机、书签、流面板网格。

**M06** (replay-scene.ts + VisualizationView 改造): ReplayScene 桥接 PlaybackEngine ↔ 3D 视口，Live/Replay 模式切换，.drec 录制驱动关节动画。

**M06-UI 改造**: 删除了 VisualizationView 的右栏 Inspector（Interaction Workflow、propertyGroups 滑条等死组件）、Base Control 面板。左栏改为可折叠。3D 视口全幅铺满，回放控件改为视口底部浮动毛玻璃条。NanoRobotViewer 渲染改为按需渲染（GPU 空闲时降至 0）。

### 测试文件
```
/tmp/dora-studio-tests/sample.drec          — 200条, 4节点
/tmp/dora-studio-tests/joint_animation.drec  — 132条, 6关节正弦动画
```

## 重要约定

1. **直接交流用中文**；GitHub 内容（commit/PR/discussion）必须英文
2. **不在 commit/PR 中加入 Claude/AI 联合署名**
3. **plans2.0/ 文件仅本地**，不进 upstream PR
4. **不伪造数据**——拿不到真实数据就明确标为 unavailable
5. **每完成一个重要模块**（UI、功能），让用户手动测试后再继续
6. **一个模块一个模块来**：plan → code → test，不要一次排全部
7. Motion Planner 和 Visualization 页面保持稳定——没有讨论不要大改

## 计划文件

所有 v2.0 模块计划在 `plans2.0/`:
- `OVERVIEW.md` — 模块堆栈图 + 关键路径 + 风险登记
- `M00-type-definitions.md` ~ `M14-tauri-integration.md` — 各模块详细计划
- `RUST-COMPAT.md` — Rust 版本兼容策略

## 已知问题 / 待办

1. Rust 1.75 无法 link dora crates — 类型复制策略，见 RUST-COMPAT.md
2. DataflowCanvas 画布初始缩放可能需手动调（右下角 +/− 按钮或点 ⊡）
3. Schema 注册表现在只有 10 个内置类型，后续需扩展
4. Hot Reload 需要 dora coordinator 运行中才生效
5. 旧的 Graph 视图和 Inspector 面板已删除——不要恢复它们
6. **轮询监控可靠性** — 已立项 M11.5（`plans2.0/M11.5-monitoring-control.md`，M11 后 M12 前实施）：监测开关默认关闭（纯手动）+ M07 数据源升级 WS + 可选 OTLP 接收器。实施前 M07/M08 的常开轮询暂时保留。
7. **环境限制** — 本机 Python 3.10 + Rust 1.75 无法运行 dora 0.5+ dataflow（dora-rs 包 message v0.6 vs CLI v0.8 不匹配；Rust 示例需 edition 2024）。真实数据验证需升级环境。

## 源码参考

- dora 核心: `/home/dora/dora`
- dviz 源码: `/home/dora/dviz`
- dora-moveit2 源码: `/home/dora/dora-moveit2`
