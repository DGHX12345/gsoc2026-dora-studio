# dora-studio Final Report 演示手册

> 本地文档（`plans2.0/`，不提交 upstream）。供学生 Final Report 演示使用：每一步做什么、怎么演示、用了什么技术、体现 dora 的什么价值。
>
> 技术与能力描述以 `HANDOFF.md`（权威模块记录）为准；本手册与代码不一致处以代码为准。

## 0. 开场准备：环境与启动顺序

**顺序铁律：先起后端 → 再起前端 → 再按需起 dora。**

### 0.1 三个终端

工作目录：

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b
```

**终端 1 — 后端**（监听 `127.0.0.1:3001`）：

```bash
source /home/dora/.venvs/dora-studio/bin/activate   # 让 dora CLI 进入后端子进程的 PATH
cargo run --manifest-path backend/Cargo.toml
```

> 后端通过子进程调用 dora CLI（`dora daemon` / `dora run` / `dora list`）。先 activate venv 保证子进程使用正确的 python 环境（HANDOFF-M15 踩坑记录：不带 `VIRTUAL_ENV` 时 dora daemon 会用 uv 托管的裸 python 跑节点）。

**终端 2 — 前端**（浏览器打开 `http://localhost:5173`）：

```bash
npm --prefix frontend run dev
```

**终端 3 — dora**（需要演示 daemon / 实时数据流时）：

```bash
source /home/dora/.venvs/dora-studio/bin/activate
dora up     # 前台会挂住不退出，属正常，留在后台跑
```

### 0.2 演示数据清单（均已验证存在）

| 文件 | 内容 | 用于 |
|---|---|---|
| `/tmp/dora-studio-tests/joint_animation.drec` | 132 帧、6 关节正弦动画 | M05/M06 回放基础 |
| `/tmp/dora-studio-tests/attribution_demo.drec` | 40 条 VLM 归因链 | M09 |
| `/tmp/dora-studio-tests/tool_demo.drec` | 120 帧：图8 路径 + TF + 关节 | M11/M12 |
| `/tmp/dora-studio-tests/moveit_demo.drec` | 120 帧 moveit 全流 | M13 |
| `/tmp/dora-studio-tests/lerobot_demo_v1` / `_v2` | 合成 LeRobot 数据集 | M10 |
| `/home/dora/.cache/huggingface/lerobot/my_org/b601_pilot_v1` | B601 真实数据集 | M10 |
| `examples/live-demo/` | 真实 A* 规划数据流 | M15 Phase B |
| `examples/planner-demo/dataflow.yml` | 结构示例 | M12 推荐演示 |

> 注意：旧文档提到的 `/tmp/dora-studio-tests/sample.drec` 当前已不存在，回放基础演示统一用 `joint_animation.drec`。这些 `.drec` 演示文件由后端测试生成——若缺失，跑一次 `cargo test --manifest-path backend/Cargo.toml` 会重新写出（generator 的 `writes_*_demo_file` 测试）。

### 0.3 页面一览

左侧边栏 8 页：**Dashboard**（总览）/ **Dataflow Explorer** / **Run & Monitor** / **Logs & Events** / **Performance** / **Replay Timeline**（以上 Dora 区）/ **Visualization** / **Motion Planner**（机器人区）。

演示前建议先完整预演一遍（warm-up），确认 .drec 文件、venv、端口都正常。

---

## 1. Dashboard 总览

**演示目标**：一个页面同时看到 dora daemon/coordinator 状态与 dviz/moveit 生态组件状态，并一键拉起 daemon。

**准备**：后端 + 前端已起（0.1）。不需要数据流。

**演示步骤**：
1. 打开 `http://localhost:5173`，默认落在 Dashboard。
2. Quick Start 面板 → 点「Start dora daemon」按钮：状态 `stopped → starting → running`（按钮随后禁用）。说明：后端 `daemon.rs` 用 `dora daemon` 子进程按需拉起守护进程。
3. 指状态卡：Coordinator 卡从 `Unavailable → Connected`（显示版本号与 coordinator 上的 dataflow 数量）；「3D Viz (dviz)」「Motion (moveit)」卡显示 `Installed/Running`（dviz/moveit 二进制与包的存在性检查）。
4. 演示结束后在终端 3 用 `dora destroy` 清理（可选）。

**技术要点**：Rust + Axum 后端；`daemon.rs` 管理 dora daemon 子进程；`coordinator.rs` 包装 `dora list --format json`；`external.rs` 做 dviz/moveit 状态探测；前端 5s 轮询。

**dora 价值**：daemon/coordinator 是 dora 数据流运行的底座；Studio 把它变成一眼可见的状态面板并支持按需拉起——降低新人上手 dora 的门槛。

**时间建议**：1–1.5 分钟。

---

## 2. Dataflow Explorer（M01 画布 / M02 Schema / M03 Runtime）

**演示目标**：dora 声明式 YAML dataflow 的三个闭环——浏览（Source）、可视化编辑（Build）、类型检查 + 真实运行（Runtime 绑定）。

**准备**：后端 + 前端。Source 标签页不需要 dora；Build 的 Run 需要 dora CLI（终端 1 已 source venv）。

**演示步骤**：
1. **Source 标签页**：展示 `examples/` 扫描出的 dataflow 列表；点选一个 → 左侧 YAML 图（节点/边/输入输出）+ YAML 源文件查看器。
2. **Build 标签页**：从左侧 NodePalette 拖入算子（10 个内置算子，如 `camera_driver`、`object_detection`、`planner`），连接端口；画布支持 pan/zoom、节点拖拽、Delete 删除。
3. **Schema 类型检查（M02）**：连线颜色语义——绿=兼容、红=不兼容、黄=警告、灰=未知（后端 `schema_registry.rs` 内置算子类型表做精确/扩展/收缩/不兼容四分类检查；具体以实际连线结果为准）。
4. **Runtime 绑定（M03）**：工具栏 Run → 生成 YAML 写入临时文件 → `dora run` 子进程启动 → 画布节点状态点变彩色（2s 轮询，WS → CLI → 运行时逐级回退）；右键节点 → Hot Reload（Python 节点，需 coordinator 运行中）；Stop 停止。

**技术要点**：
- 前端：SVG 画布 `DataflowCanvas.vue`、`NodePalette.vue`（分类搜索、HTML5 拖拽）。
- 后端：`dataflow_builder.rs`（graph→YAML 生成、最小 YAML 解析器）；`coordinator_ws.rs`（原始 TCP + 手写 WebSocket 帧、Bearer auth + Hello 握手 + JSON-RPC——因 tokio-tungstenite 依赖链需要 edition 2024，在 Rust 1.75 下手写帧）；token 发现顺序 `DORA_AUTH_TOKEN` env → CWD `.dora-token` → `~/.config/dora/.dora-token`。
- API：`POST /api/dataflow/build`、`/validate`、`/parse`；`GET /api/runtime/nodes/:id`、`POST /api/runtime/nodes/:id/reload`、`POST /api/dataflow/run`。

**dora 价值**：dora dataflow 是声明式 YAML——Studio 让 YAML 可以被可视化编辑、被类型检查、被一键运行，把「写 YAML 试错」变成「拖拽连线试错」。

**时间建议**：2–2.5 分钟（Source 30s / 画布+Schema 1min / Run+状态 1min）。

---

## 3. Run & Monitor

**演示目标**：`dora run` 子进程的 Studio 内管理：启动/停止 dataflow、节点表与状态交叉引用。

**准备**：后端 + 前端；真实运行需 dora 环境（终端 3 可起 `dora up`）。后端进程需有 dora CLI 在 PATH（0.1 已覆盖）。

**演示步骤**：
1. Run & Monitor 页：Target 下拉选择 dataflow（列表来自 `examples/` 扫描，状态与 runtime/coordinator 交叉引用）。
2. 点 **Start** → 后端 `runtime.rs` 拉起 `dora run` 子进程 → Runtime Status 卡变 running（显示 PID）→ 节点表显示各节点状态（含 Restarts 列）。
3. 若终端 3 已起 `dora up` 并用 CLI 跑了 live-planning（见第 12 节），页面状态同样反映 running——说明 Studio 状态是真实交叉引用，不是自报。
4. **Stop** → 子进程停止，状态回 Stopped。中间可演示 **Restart**。
5. 诚实点：节点表不注入假 CPU/内存值（Week 12 移除 mock），拿不到的数据显式标 unavailable。

**技术要点**：`runtime.rs`（`dora run` 子进程管理器 + stdout/stderr 环形缓冲）；`coordinator.rs`（`dora list` 状态）；Week 12 的「无 mock」改造（system_status 返回 unavailable 而非假数据）。

**dora 价值**：dora 的运行控制原本只有 CLI；Studio 把它放进 GUI 并与 coordinator 列表交叉验证——多一个管理 dataflow 生命周期的入口。

**时间建议**：1–1.5 分钟。

---

## 4. Logs & Events

**演示目标**：三级日志查看（info/warn/error 分面板）+ 原始流的运行时日志聚合。

**准备**：最好有一个正在运行的 dataflow（与第 3 节或第 12 节配合）；没有运行时展示空状态 + 诚实说明。

**演示步骤**：
1. Logs & Events 页：顶部显示 API 连接状态 pill（connected/fallback）。
2. 三个面板：**Info / Warning / Error** 分开展示，各自独立的视觉样式（用户反馈的「日志分级展示」需求）。
3. 展开底部「Raw Stream」折叠区：终端风格的原始流，实时追加（前端 1.2s 轮询）。
4. 说明来源：`dora run` 子进程 stdout/stderr 被 `runtime.rs` 捕获进环形缓冲。

**技术要点**：`runtime.rs` 日志捕获；`LogsEventsView.vue` 三级面板 + LogLine 组件；1.2s 轮询。

**dora 价值**：dora 应用的日志散在各节点进程的 stdout/stderr；Studio 聚合到一处并按级别组织——调试分布式数据流时不用翻多个终端。

**时间建议**：1 分钟。

---

## 5. Replay Timeline（M04 .drec 解析器 + M05 时间轴）

**演示目标**：dora 官方 `.drec` 录制格式的解析、索引与随机访问回放。

**准备**：后端 + 前端；`/tmp/dora-studio-tests/joint_animation.drec`。

**演示步骤**：
1. Replay Timeline 页 → 顶部路径输入框填 `/tmp/dora-studio-tests/joint_animation.drec` → 加载。
2. 展示流面板：按 (node, output) 分组的流列表（132 帧、6 关节流）。
3. 播放控制：Play/Pause（空格键）、Speed 0.5/1/2/5（数字键 1–4）、拖动时间轴 seek、添加书签跳转。
4. 说明随机访问原理（M04）：打开文件时单次顺序扫描构建偏移索引，之后 seek 用二分查找定位时间戳。

**技术要点**：
- 后端 `drec/`：`reader.rs`（二进制读取、MAGIC/版本校验、`read_entry_at` 随机访问、截断记录优雅处理）、`index.rs`（偏移索引 + `seek_to_timestamp` 二分查找 + 按 (node,port) 流分页）、`service.rs`（RecordingManager、`spawn_blocking` 异步加载）。
- `.drec` 格式：Header(MAGIC+version+start_nanos+UUID+yaml_len+yaml) / Record(node+output+timestamp_offset+event) / Footer(FOOTER_MAGIC+total_messages+total_bytes)。
- 前端 `playback.ts` PlaybackEngine 状态机 + `ReplayTimeline.vue`；`onTick` 多监听器（修复了时间显示冻结的 M06 遗留 bug）。
- API：`POST /api/recording/open`、`GET /:id/streams`、`/seek`、`/entries`、`POST /:id/close`。

**dora 价值**：`.drec` 是 dora 的录制回放格式——Studio 是它的图形化播放器：录一次，反复回放、跳转、分页检查任意流。

**时间建议**：1.5–2 分钟。

---

## 6. Visualization 3D 视口（M06）

**演示目标**：dviz 风格 3D 视口：Live/Replay 双模式、`.drec` 驱动关节动画、按需渲染。

**准备**：后端 + 前端；`joint_animation.drec`。

**演示步骤**：
1. Visualization 页 → 顶栏 **Live/Replay** pill 切到 **Replay**（默认 Live）。
2. 路径输入框（默认已填 `joint_animation.drec`）→ 加载 → 视口底部出现浮动毛玻璃回放条。
3. 播放 → Nano 机器人 6 关节按正弦动画运动（`.drec` 条目驱动）。
4. 左栏（可折叠）Data Sources 面板：Robot Profile（各模块状态点）、dviz displays 列表（checkbox 开/关）、topics 过滤搜索。
5. 停止播放后说明按需渲染：无数据时 GPU 渲染降至 0（`requestRender` 机制）。

**技术要点**：`NanoRobotViewer.vue`（Three.js 共享查看器）、`replay-scene.ts`（PlaybackEngine ↔ 3D 视口桥）；M06-UI 改造（删除右栏 Inspector 死组件、全幅视口、浮动回放条、按需渲染）。

**dora 价值**：视口按 dviz 的显示/插件工作流设计（Week 9 决策）；录制的 dora 数据流可以「动起来」看——机器人调试从看日志变成看 3D。

**时间建议**：1.5–2 分钟。

---

## 7. VLM/LLM 归因条（M09）

**演示目标**：摄像头帧 → VLM 提示词 → LLM 回复 → 解析动作 → 执行结果 的因果链：解释机器人「为什么」做这个动作。

**准备**：`attribution_demo.drec`（40 条链，每第 5 条失败）。

**演示步骤**：
1. Visualization → Replay 模式 → 加载 `/tmp/dora-studio-tests/attribution_demo.drec`。
2. 播放 → 浮动回放条上方出现可折叠归因面板：图标链条（绿=成功/红=失败）。
3. 点开一条链 → 详情卡 5 步（帧/提示词/回复/动作/执行），>200 字符折叠、token 流 50 tok/s 动画、动作向量表。
4. 用 ‹ › 在链之间导航；点「在 3D 中查看」→ 视口 seek 到对应时刻并自动折叠面板。
5. 数据源下拉展示三段来源设计：`.drec 录制` / `LeRobot 数据集 (M10)` / `Live dora VLM 节点`（需运行中 dataflow）。

**技术要点**：
- 载荷为 Studio 结构化格式（magic `DORAATT\0` + 长度前缀字段，手写 cursor 编解码）；真实 dora VLM 输出是 Arrow IPC，本环境无法解析时诚实标记 unparseable，不伪造。
- `attribution.rs` AttributionExtractor 单次顺序扫描 `.drec`，按 `frame_timestamp` 分组组装链。
- API：`GET /api/recording/:id/attribution`（摘要）、`/attribution/chain?timestamp=T`（详情），`spawn_blocking` 提取。

**dora 价值**：dora 生态主打 VLM/LLM 驱动的机器人应用；归因条把模型决策链可视化，直接服务这类应用的调试需求。

**时间建议**：1.5 分钟。

---

## 8. LeRobot 数据集（M10）

**演示目标**：把 LeRobot 数据集（Parquet）直接变成可浏览的归因链：真实 B601 机器人数据端到端。

**准备**：后端 + 前端；B601 真实数据 `/home/dora/.cache/huggingface/lerobot/my_org/b601_pilot_v1`（合成备选 `/tmp/dora-studio-tests/lerobot_demo_v1`，展示 v2 布局用 `lerobot_demo_v2`）。

**演示步骤**：
1. Visualization → Replay 模式（归因面板无需先加载 .drec 即可见）。
2. 归因面板数据源下拉 → **LeRobot 数据集 (M10)**。
3. 路径输入 B601 路径 → 点「扫描」：显示数据集信息（5 episodes、任务描述进入提示词）。
4. 选择 Profile（B601 自动检测，≥0.5 分阈值）+ Episode → 加载：每帧一条链（分页 200）。
5. 点一条链 →「在 3D 中查看」：动作向量驱动 3D 姿态（B601 角度是度，前端 deg→rad 转换后预览，预览带提示标签）。
6. 诚实点：LLM 回复/执行结果在数据集中未记录 → UI 显示「数据集中未记录」占位，不伪造；confidence/success 为 Option（缺失时中性灰链）。

**技术要点**：
- Parquet 解析走 Python 子进程桥：`backend/scripts/lerobot_reader.py`（scan/frames/gen-demo 子命令、JSON stdout 协议、30s 超时、pyarrow 缺失优雅降级）——目标用户必有 Python+pyarrow（lerobot 强制依赖），Rust 手写解析不可行（用户优先决策）。
- Profile YAML（`profiles/lerobot_profile_b601.yaml` 等）：字段别名映射（首个命中优先）、`angle_unit: degrees`、autodetect。
- 支持 v1（`episode_*.parquet`）与 v2（`chunk-*/file-*.parquet`）两种布局。

**dora 价值**：LeRobot 是机器人学习的数据标准（dora 生态 dorobot 也写它）；Studio 成为 LeRobot 数据的可视化检查器——训练前先看清数据。

**时间建议**：1.5–2 分钟。

---

## 9. 工具槽协议（M11）+ dviz 路径工具（M12）

**演示目标**：视口工具协议（注册/挂载/订阅广播/推荐引擎 + TF 树），以及按 dviz 真实端口语义渲染的路径/costmap/目标点工具。

**准备**：`tool_demo.drec`（120 帧：图8 waypoints+trajectory、TF 链、关节动画、无关 camera 流、60–89 帧静默间隙、24×24 高斯障碍 costmap）。

**演示步骤**：
1. Visualization → Replay → 加载 `tool_demo.drec`。
2. 顶栏 **Tools** → 工具面板：dviz-path 工具带「推荐」徽章（推荐引擎：匹配 .drec 流 + dataflow YAML 节点输出——可用 `examples/planner-demo/dataflow.yml` 结构示例说明）。
3. 挂载 dviz-path → 播放：视口出现青色 2px 路径线（图8）、绿/红起终点标记、粉色目标点（沿图8步进）、costmap 平面（蓝→黄→红 LUT 平铺地面 z=0.02）。
4. 播放到 60–89 帧静默间隙 → **stale 徽章**出现（100ms 阈值、2×帧窗口的数据新鲜度语义）。
5. 面板控制：路径列表显隐、聚焦（focusOn）、滑块、中英文切换。
6. 说明 TF（M11）：`SimpleTfTree`（Matrix4 组合、父链查找、环检测、帧名前导 `/` 去除）——dora 无官方 TF 消息，回放中 TF 为 JSON 载荷流。
7. 说明协议：`ToolRegistry` 单例（register/attach/`broadcastBatch`/`broadcastSeek`）、`ToolPayload {f32?/json?/bytes?}` 联合（dviz/moveit 数据是 flat Float32Array 或 JSON，无结构化 Arrow——M12 审计结论）、工具零改动即可同时消费回放与直播（第 12 节验证）。

**技术要点**：
- `frontend/src/tools/`：types.ts / registry.ts / matching.ts（glob+RegExp 匹配、推荐合并）/ feed.ts（.drec 条目→ToolBatch）/ tf.ts / dviz/（parse/format/DvizPathTool/DvizPathPanel）。
- Line2/LineMaterial（three/examples/jsm）实现 2px 线宽（普通 Line 忽略 linewidth）。
- costmap 单 JSON `{width,height,resolution,values}` → RGBA DataTexture，同尺寸复用纹理。
- 四端口订阅：`waypoints|path`、`trajectory`、`target_point|target|goal`、`costmap|esdf`（来自 dviz 源码审计的真实端口名）。
- 测试：`npm --prefix frontend run test:tools`（tsx：tf/matching/registry/feed/dviz/playback）。

**dora 价值**：dviz 是 dora 生态的 3D 可视化组件，端口语义来自对 dviz 源码的审计；工具槽让 Studio 成为 dora 工具生态的挂载点——第三方工具只写一个类就同时获得回放与直播数据。

**时间建议**：2 分钟。

---

## 10. MoveIt Bridge（M13，含 M15 Phase A）

**演示目标**：消费 dora-moveit2 输出：轨迹、幽灵姿态、当前姿态、黄色碰撞线框；工具自带 URDF 加载器（一个机制覆盖 UR5e/GEN72/B601）；独立轨迹播放器 + 模型选择器。

**准备**：`moveit_demo.drec`（120 帧，真实节点名 planner/planning_scene/trajectory_executor/mujoco_sim + dviz 流 simple_planner/costmap_node）；`models/b601/` 已本地化。

**演示步骤**：
1. Visualization → Replay → 加载 `moveit_demo.drec` → Tools → 挂载 moveit 工具。
2. 挂载即隐藏 nano 模型 → 模型选择器选 **B601** → 3D 中 B601 机械臂替换 nano。
3. 播放：EE 末端路径 + 幽灵姿态（ghost 数量滑块）、当前姿态随轨迹运动、规划场景障碍的黄色碰撞线框（开关控制）。
4. 独立轨迹播放器：播放/暂停/速度/同步时间轴、waypoint 进度（n/total）、关节表实时数值。
5. **M15 Phase A 说明**：这就是「回放式规划可视化」——规划场景 + 轨迹 + 幽灵 + 执行状态在回放数据上的完整呈现。
6. 已知问题诚实说明：B601 165 万顶点 ×(1+幽灵数) 播放卡顿（学生接受延期；修复选项：减面/骨架幽灵/播放时隐藏幽灵）。

**技术要点**：
- 工具自带 Three.js URDF 加载器（最小 XML 解析器 → 关节树 → FK；STL + `package://` 路径重映射）——不侵入 NanoRobotViewer。
- 审计修正：plan_status 7 键、execution_status 5 键、joint_positions 是 float64、`.drec` 无 metadata（reshape 靠关节数）、真实 .drec 是 Arrow IPC 字节（诚实 unsupported，M15-B 直播桥发 JSON 后自然消失）。
- 模型目录扫描：`GET /api/models`（`models/*/*.urdf` 动态发现，加模型零代码改动）。
- nano 替换用 `modelVisible` prop（只藏模型不藏画布——v-show 藏画布曾致工具渲染全黑）。

**dora 价值**：dora-moveit2 是 dora 生态的运动规划节点集；Studio 扮演 rviz 的观察端——同一视口内 moveit + dviz 工具协同（moveit_demo 同时含 dviz 流）。

**时间建议**：2 分钟。

---

## 11. Metrics / Performance（M07 + M08 + M11.5）

**演示目标**：按需开启的诊断模式：节点 CPU/内存指标 + OTel 火焰图，默认关闭、后端启动零轮询。

**准备**：后端 + 前端。最好有运行中的 dataflow（live-planning，见第 12 节）。火焰图需要本地 Jaeger（见步骤 4）。

**演示步骤**：
1. Performance 页 → 顶部监测控制条：**主开关 + 分开关**（Node Metrics / OTel Spans）+ 样本计数；初始状态三个面板显示「监测已关闭」空状态（默认关、启动零轮询）。
2. 打开开关 → 样本计数开始增长；若本机无 coordinator → 状态显示 `source: "cli"`（WS 失败自动回退 CLI——诚实展示回退机制）。
3. Node Metrics 面板：CPU/内存时间线（canvas）+ 节点指标卡片网格；Topic Metrics 显式标 unavailable。
4. OTel 面板：未连接 Jaeger 时显示配置说明；启动容器：

   ```bash
   docker run -d --rm --name jaeger -p 16686:16686 jaegertracing/all-in-one
   ```

   → 数秒内 `connected: true`、spanCount 累积 → 火焰图渲染真实 spans（横向堆叠条、点击放大+面包屑、搜索高亮、Node 过滤下拉、30s/2min/5min/1h 时间范围按钮）。
5. 关闭开关 → 采样冻结（watch channel 取消）→ 刷新页面后开关状态保持（localStorage 持久化，刷新后自动重开）。

**技术要点**：
- `metrics.rs`：2s 轮询 `dora node list --format json`（NDJSON；计划中的 `dora top --once` 不存在——审计发现）、环形缓冲 300 样本/节点、`PolledNode` 统一中间类型、`MetricsSource::{Cli,Ws}`。
- `otel.rs`：5s 轮询 Jaeger 兼容 HTTP API（`/api/services` → `/api/traces?service=X`；`DORA_OTEL_QUERY_ENDPOINT` 可覆盖）、环形缓冲 1000 spans、span 树构建、手写最小 HTTP 客户端（含 chunked 解码）。
- `monitoring.rs`：MonitoringController + watch channel 启停（无新依赖）；主题 token 修复浅色主题对比度（用户两次反馈）。
- 未做（路线图）：M11.5 D3 内置 OTLP HTTP 接收器（4318 推送模式，免 Jaeger 依赖）。

**dora 价值**：dora daemon 通过 GetNodeInfo 暴露每节点指标、dora 通过 OTLP gRPC 导出 spans——Studio 把可观测性做成按需诊断模式：需要时开、默认零开销。

**时间建议**：2 分钟（含 Jaeger 容器起停）。

---

## 12. M15 Live Planning & Simulation（Phase B 实时链路）

**演示目标**：真实 dora dataflow 端到端——真实 A* 规划（避障 + 视线平滑）→ 经 studio_bridge 实时流入 Studio 视口 → Motion Planner 控制台 Plan/Execute/Stop/Auto/场景箱子全程操控。

**准备**：后端 + 前端已起（终端 1 已 source venv）；终端 3 起 `dora up`（0.1）。

**演示步骤**：
1. 起数据流（终端 3）：

   ```bash
   source /home/dora/.venvs/dora-studio/bin/activate
   dora up     # 若还没起；前台挂住属正常
   cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b/examples/live-demo
   dora start --name live-planning dataflow.yml
   ```

   说明节点分工：`costmap_source`（合成传感器，YAML 头注释标注 DEMO）→ `simple_planner`（真实 A*：避障 + 视线平滑）→ `trajectory_executor`（五次样条演示执行器，标注无物理——mujoco 物理镜像在 Phase C）→ `studio_bridge`（POST 到后端 `/api/live/ingest`）；`studio_console` 轮询后端命令队列。
2. **Visualization 页** → Live 模式 → 顶栏「实时数据」开关打开（默认关，opt-in）→ Tools 挂载 **dviz-path** → 视口实时出现路径/costmap/目标点（100ms 轮询，M12/M13 工具零改动——回放与直播同一广播协议）。
3. **Motion Planner 页**（规划控制台）：
   - 「目标位置 (x, y, z)」输入（默认 0.55/0.20/0.30）→ **Plan**（规划一次）→ 状态 pill 显示成功 + 路径长度（m）；视口中新路径实时出现。
   - **Execute** → 机械臂（演示执行器）沿轨迹运动（joint_positions 实时流）。
   - **Stop** → 回 home；**Auto** → 打开轨道演示（新规划自动执行）。
   - 场景：点「添加箱子」→ 规划场景加入障碍 → 再 **Plan** → 路径明显绕开箱子（真实 A* 避障——这是全场「真实计算」高光）。
   - 模式 pill 指示「手动目标/自动轨道」（控制台优先语义：Plan 关闭轨道，Auto 才开轨道）。
   - 规划器选择器列出可用规划器（如 `simple_planner (A* grid)`）。
4. 命令反馈：每个命令显示 seq/状态（命令队列协议：seq 从 1 起、`since_seq`/`next_seq` 重启检测）。

**技术要点**：
- `backend/scripts/studio_bridge.py`（dora python API 订阅端口 → HTTP POST `{node_id, output_id, timestamp, payload}`）。
- `backend/src/live.rs`：每流环形缓冲（4096/流、64 流上限）、`GET /api/live/recent`（`since_ts` 严格取新）、`POST+GET /api/live/command`。
- 前端 `live-feed.ts`：100ms 轮询 → ToolBatch → `toolRegistry.broadcastBatch`——M12/M13 工具零改动消费直播。
- `examples/live-demo/nodes/`：simple_planner（A* + 视线平滑）、studio_console（B6 协议）、trajectory_executor——41 个 python 测试（以当前代码为准）。
- 无新 Rust 依赖。

**dora 价值**：这是「rviz 三件套」的 dora 版本：rviz=Studio 前端、MoveIt=dora-moveit2、物理=dora-mujoco（Phase C）。dora 的实时数据面（节点间端口消息）经一个 bridge 节点变成 GUI 可消费的直播流——Studio 成为 dora 应用真正的运行时调试器。

**时间建议**：3–4 分钟（全演示高潮）。

---

## 13. 完整演示编排建议（约 25–30 分钟）

叙事线：从「这是什么」→「它能干什么」→「它多真实」。

| # | 模块 | 时长 | 叙事作用 |
|---|---|---|---|
| 1 | Dashboard | 1min | 开场：一个页面看到整个 dora 系统 |
| 2 | Dataflow Explorer | 2.5min | dora 的 dataflow 范式：可视化编辑+类型检查+运行 |
| 3 | Run & Monitor | 1.5min | 运行时管理 |
| 4 | Logs & Events | 1min | 日志聚合 |
| 5 | Replay Timeline | 2min | .drec 录制回放（进入机器人区） |
| 6 | Visualization 视口 | 1.5min | 3D 看数据流 |
| 7 | 归因条（M09） | 1.5min | 机器人「为什么」 |
| 8 | LeRobot（M10） | 2min | 真实机器人数据 |
| 9 | 工具槽 + dviz（M11/M12） | 2min | 生态对接点 |
| 10 | MoveIt（M13） | 2min | rviz 观察端 |
| 11 | Metrics/OTel（M07/M08/M11.5） | 2min | 可观测性 |
| 12 | M15 实时规划 | 3–4min | 高潮：真实 A* 避障 + 控制台操控 |
| 13 | 收尾 | 1min | 诚实性 + 路线图一句话 |

**衔接建议**：M15 依赖 `dora up` + live-planning（终端 3）。推荐在演示开始时就起好 `dora up` 放后台，并在 Run & Monitor 阶段就用 CLI 起 live-planning——这样 Run & Monitor 和 Logs 都有真实数据可看，M15 阶段直接切页面，不用等启动（`dora start` 约需数秒，注意衔接）。若现场环境不稳，M15 放最后也有压轴效果。

---

## 14. 常见故障排查

| 症状 | 处理 |
|---|---|
| 后端起不来：3001 被占 | `ss -tlnp` 查进程；确认遗留 cargo run 二进制真正退出（踩坑：遗留二进制占端口） |
| 前端 5173 被占 | 同上；vite 也可能自动换端口——看终端输出 |
| dora up 后节点用错 python | 必须 `source /home/dora/.venvs/dora-studio/bin/activate` 再 `dora up`；不带 VIRTUAL_ENV 时 daemon 用 uv 托管的裸 python（无依赖） |
| dora up 前台挂住 | 正常现象，留在后台，不要 Ctrl+C |
| `dora start` 报 already a running dataflow | `dora stop` 后名字释放有延迟，等 2–3 秒或换 `--name` |
| 演示 .drec 文件缺失 | 跑 `cargo test --manifest-path backend/Cargo.toml`（`writes_*_demo_file` 测试重写 `/tmp/dora-studio-tests/*.drec`） |
| Monitoring 无数据 | 确认开了主/分开关；`source: "cli"` 表示 WS 回退（本机无 coordinator 属正常）；需运行中的 dataflow 才有节点指标 |
| 火焰图无数据 | 确认 Jaeger 容器在 16686；端点可用 `DORA_OTEL_QUERY_ENDPOINT` 覆盖 |
| Hot Reload 灰色禁用 | 需 dora coordinator 运行中；未 Run 的 dataflow 也禁用 |
| 「实时数据」开了没数据 | 确认 live-planning 在运行（`dora logs` 只能查运行中 dataflow）；`curl http://127.0.0.1:3001/api/live/recent` 手动验证后端 |
| B601 播放卡顿 | 已知问题（165 万顶点×幽灵数）：幽灵数量调低或播放时隐藏幽灵 |
| 后端测试偶发 flaky | 已知 ~1/70（metrics/otel 时序），重跑即可，勿追查 |
| 演示结束清理 | 终端 3：`dora stop`（停数据流）→ `dora destroy`（停 coordinator/daemon） |

---

## 15. 诚实性说明（演示前必读）

- **全部 .drec 演示文件**（joint_animation / attribution_demo / tool_demo / moveit_demo）由后端 generator 合成——格式真实、数据合成；`lerobot_demo_v1/v2` 同理。
- **B601 LeRobot 数据集是真实数据**（5 episodes）；但数据集中未记录的步骤（LLM 回复/执行结果）显示占位，不伪造。
- **live-demo 中**：`simple_planner` 是真实 A* 规划（避障 + 视线平滑）——全场唯一「真实计算」环节，值得强调；`costmap_source` 是合成传感器（标注 DEMO）；`trajectory_executor` 是演示执行器（无物理，标注）；真实 mujoco 物理镜像在 Phase C（进行中）。
- **归因演示载荷**是 Studio 结构化格式（`DORAATT\0`）；真实 dora VLM 的 Arrow IPC 输出本环境无法解析，UI 诚实标 unparseable。
- **拿不到的数据一律显式 unavailable**（无 daemon 时的节点 CPU/内存、Topic Metrics、生命周期事件等）——Week 12 起已删除所有 mock 注入。
- **已知问题**：B601 播放卡顿；ur5e（6 关节）选中时 dviz xyz 流长度整除 6 可能误 reshape（演示数据不受影响；M15-B 带 metadata 无此问题）；后端偶发 flaky 测试。

---

## 16. 路线图（未完成项——不要写成已完成）

- **M15 Phase C**（mujoco 物理镜像 + 避障）——**进行中**：`examples/moveit-live-demo/` 骨架已建（moveit_console、gated executor、arm_joints 过滤器 qpos[7:13] 等 + 测试）；碰撞检测 un-stub（dora-moveit2 外部仓库）、LiveFeed 50ms（当前 100ms）、端到端验收待完成。
- **专用机械臂 arm viewer**（Motion Planner 页独立 viewer，复用 tools/moveit/urdf 纯函数模块）——学生后置项。
- **M14 Tauri 桌面打包**——未做。
- **M16 LeRobot 互操作**（dorobot 数据互通）——非必要，未做。
- **M11.5 D3 内置 OTLP HTTP 接收器**（4318 推送模式，免 Jaeger 依赖）——未实施（prost 预检已通过）。
