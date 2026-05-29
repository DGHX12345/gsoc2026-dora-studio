# Week 1 工作流记录

## 本周目标

本周目标是先建立 dora-studio 的产品形态和前端原型，而不是过早深入 dora runtime 集成。

核心目标：

1. 搭建前端工程骨架。
2. 完成一个可演示的静态 Studio UI 原型。
3. 用 mock 数据表达 dataflow graph、节点状态、日志和运行监控。
4. 预留后续 Rust backend/API 接入边界。
5. 形成本周工作记录，便于和 mentor 沟通。

## 工作原则

- 先 UI 原型，后真实 API。
- 先表达产品闭环，再补工程细节。
- 避免本周陷入完整 visual editor、runtime control、topic debug 等高复杂度功能。
- 页面结构应能支撑后续扩展到 data collection、replay、training workflow。

## 计划任务

### 1. 项目结构

计划建立如下结构：

```text
frontend/        Vue 3 + TypeScript + Vite 前端原型
backend/         后续 Rust backend 预留目录或说明
plans/           长期计划与每周工作记录
```

本周优先实现 `frontend/`。

### 2. 前端页面

计划实现 4 个核心页面或视图区：

1. **Dashboard**
   - coordinator 状态
   - daemon 状态
   - running dataflows
   - recent errors
   - resource summary

2. **Dataflow Explorer**
   - dataflow graph
   - node inspector
   - inputs/outputs
   - validation diagnostics

3. **Run & Monitor**
   - build/start/stop/restart 操作区
   - node metrics table
   - status overview

4. **Logs & Events**
   - log stream
   - node filter
   - level filter
   - error highlight

### 3. Mock 数据

计划准备 mock 数据表达：

- sample dataflow nodes
- edges
- node status
- metrics
- logs
- validation diagnostics

### 4. 交付标准

本周完成后应达到：

- 可以本地启动前端页面。
- mentor 能从 UI 看出 dora-studio 的产品方向。
- Dataflow Explorer 能展示清晰的 graph + inspector。
- Dashboard、Run & Monitor、Logs 页面有完整静态布局。
- 代码结构便于后续替换 mock 数据为真实 API。

## 实际工作记录

### 已完成工作

1. 建立 `frontend/` 前端工程骨架。
   - 使用 Vue 3 + TypeScript + Vite。
   - 固定 Vite 版本到兼容当前 Node 18 环境的版本，避免 latest 版本要求 Node 20 导致构建失败。

2. 完成 Studio 主布局。
   - 左侧导航栏。
   - 顶部页面标题区。
   - 主内容区。
   - mock runtime 状态提示。

3. 完成 4 个核心静态视图。
   - Dashboard：展示 coordinator、daemon、running flows、recent errors 等概览。
   - Dataflow Explorer：展示 dataflow graph、节点 inspector、diagnostics。
   - Run & Monitor：展示运行控制按钮和 node metrics table。
   - Logs & Events：展示日志流、过滤器占位和 future debug hooks。

4. 准备 mock 数据。
   - mock dataflow nodes。
   - mock edges。
   - mock node status。
   - mock CPU、memory、restart count、pending messages。
   - mock logs。
   - mock diagnostics。

5. 增加 `.gitignore`。
   - 忽略 `frontend/node_modules/`。
   - 忽略 `frontend/dist/`。
   - 忽略常见本地生成文件。

6. 根据第一次 UI 观察反馈调整原型。
   - 测试阶段改为中文优先，后续再加入英文版。
   - 放大 Dashboard、指标卡片、运行监控表格和日志区域，提升演示可读性。
   - 将原先的大块空白区改为明确的数据可视化预留区，包括吞吐预览、节点资源占用、Debug/数据工作流入口。
   - 保留双语扩展方向，但当前先避免英文文案影响 mentor 评审。

7. 调整日志展示方式。
   - 将常规日志、警告日志、错误日志分成三个独立区域。
   - 为三类日志增加不同颜色、图标、边框和计数展示。
   - 保留原始合并流作为可折叠预览思路，方便后续排查完整时序。

8. 建立前端双语文案结构。
   - 新增 `frontend/src/i18n.ts`。
   - 当前默认中文。
   - 先为应用壳、导航和顶栏建立 `zh/en` 文案结构。
   - 业务页面仍以中文为主，后续再逐步迁移到统一文案表。

9. 建立 Rust backend mock API skeleton。
   - 新增 `backend/`。
   - 使用 Rust + axum + tokio。
   - 当前提供 mock API，先固定 Studio 前后端边界，后续再接入 Dora coordinator 和 dora-core。

10. 接入前端 API client。
    - 新增 `frontend/src/api.ts`。
    - 封装 `getSystemStatus()`、`getDataflows()`、`getNodes(id)`、`getLogs(id)`。
    - Dashboard 接入 `/api/system/status` 和 `/api/dataflows/:id/logs`。
    - Run & Monitor 接入 `/api/dataflows/:id/nodes`。
    - Logs & Events 接入 `/api/dataflows/:id/logs`。
    - 后端未启动时自动 fallback 到本地 mock 数据，保证 UI 仍可演示。
    - 页面显示 `API connected` 或 `Using mock fallback`，便于测试前后端连接状态。

11. 建立 descriptor graph mock API。
    - 新增 `/api/dataflows/:id/graph`。
    - 后端返回 graph nodes、edges 和 diagnostics。
    - Dataflow Explorer 已接入该接口，并保留本地 mock fallback。

12. 新增实际 Dora 测试 dataflow。
    - 新增 `examples/robot-perception-test/dataflow.yml`。
    - 新增 `camera.py`、`detector.py`、`planner.py`、`logger.py`、`robot_bridge.py`。
    - 该示例模拟 camera → detector → planner → robot_bridge，同时 logger 记录 frame、boxes、cmd_vel 组合，作为后续 Studio 实测输入。
    - 当前示例兼容本机 Dora CLI 0.5.0，暂未使用 `input_types` / `output_types` 字段。

13. 建立 Studio 端真实运行测试闭环。
    - 后端新增 runtime lifecycle API，可从 Studio 启动和停止 `examples/robot-perception-test/dataflow.yml`。
    - Run & Monitor 页面按钮已连接真实 `dora run` 子进程。
    - Logs & Events 页面会轮询 backend runtime logs，展示真实 Dora 输出。
    - 后端默认端口仍为 `127.0.0.1:3001`，也支持通过 `DORA_STUDIO_BACKEND_ADDR` 临时指定测试端口。

14. 优化真实运行日志展示。
    - 后端 runtime logs 不再限制为 80 条，返回当前保存的完整日志，便于前端显示真实总数。
    - 日志页面每个分区默认只显示最近 5 条，避免实时日志撑爆页面。
    - 常规、警告、错误和原始合并流均支持“查看全部日志”，在页面上层弹窗中滚动查看完整内容。
    - 保留分区计数，让用户能先关注警告和报错，再按需查看全量日志。
    - 后端会从 Dora 输出中解析真实产生时间，例如 `18:24:40`，避免页面只显示 `live`。

15. 增加 warning/error 日志测试入口。
    - `detector.py` 每处理 5 帧输出一次 `logging.warning(...)`。
    - `robot_bridge.py` 每处理 9 条命令输出一次 `logging.error(...)`。
    - 运行示例约 5-6 秒即可在 Studio 的警告和错误分区看到真实日志。

### 当前 mock API 合约

```text
GET /api/health
GET /api/system/status
GET /api/dataflows
GET /api/dataflows/:id/nodes
GET /api/dataflows/:id/logs
GET /api/dataflows/:id/graph
GET /api/runtime/status
GET /api/runtime/logs
POST /api/runtime/start
POST /api/runtime/stop
```

后端默认监听：

```text
http://127.0.0.1:3001
```

### 验证记录

已执行：

```bash
npm --prefix frontend install
npm --prefix frontend run build
npm --prefix frontend audit --omit=dev
cargo check --manifest-path backend/Cargo.toml
cargo run --manifest-path backend/Cargo.toml
python3 -m py_compile examples/robot-perception-test/*.py
dora graph examples/robot-perception-test/dataflow.yml
dora run examples/robot-perception-test/dataflow.yml --stop-after 6s
DORA_STUDIO_BACKEND_ADDR=127.0.0.1:3101 cargo run --manifest-path backend/Cargo.toml
POST /api/runtime/start、GET /api/runtime/logs、POST /api/runtime/stop smoke test
```

结果：

- 前端生产构建通过。
- TypeScript 检查通过。
- Vite build 通过。
- 生产依赖无漏洞。
- npm 对开发依赖报告 2 个 moderate vulnerabilities，当前不影响生产依赖；后续可以在升级 Node/Vite 时统一处理。
- Rust backend `cargo check` 通过。
- Rust backend mock server 可启动，监听 `http://127.0.0.1:3001`。
- 已验证 `/api/health`、`/api/system/status`、`/api/dataflows`、`/api/dataflows/:id/nodes`、`/api/dataflows/:id/logs` 均可返回 JSON。
- 前端已接入 backend mock API，并保留本地 mock fallback。
- 重新执行 `npm --prefix frontend run build` 通过。
- 重新执行 `cargo check --manifest-path backend/Cargo.toml` 通过。
- `python3 -m py_compile examples/robot-perception-test/*.py` 通过。
- `dora graph examples/robot-perception-test/dataflow.yml` 通过，可生成 dataflow graph HTML。
- runtime lifecycle API smoke test 通过：`/api/runtime/start` 可启动示例 dataflow，`/api/runtime/logs` 可返回真实 Dora 输出，`/api/runtime/stop` 可停止进程。
- 日志 UI 调整后重新执行 `npm --prefix frontend run build` 通过。
- runtime logs 分级与完整返回调整后重新执行 `cargo check --manifest-path backend/Cargo.toml` 通过。
- `dora run examples/robot-perception-test/dataflow.yml --stop-after 6s` 已确认会产生 `WARN detector` 和 `ERROR robot_bridge` 测试日志。

开发服务器已验证可启动：

```text
http://127.0.0.1:5173/
```

## 完成情况总结

本周计划中的前端原型主线已完成：当前仓库已经具备可启动、可构建的 Vue UI 原型，能够通过 backend mock API 或本地 mock fallback 展示 dora-studio 的产品方向，包括 dataflow graph、节点详情、运行监控和日志视图。根据第一次 UI 观察反馈，当前版本已调整为中文优先，并把 Dashboard 空白区域明确设计为未来数据可视化区。当前还新增了可被 Dora CLI 解析的 `examples/robot-perception-test/dataflow.yml`，并已打通从 Studio 按钮启动真实 `dora run`、轮询真实日志、停止进程的最小测试闭环。

后续建议：

1. 让 mentor 先看 UI 产品方向。
2. 根据反馈调整页面布局和信息架构。
3. 下一步把 `/api/dataflows/:id/graph` 从 mock 改为读取并解析真实 dataflow YAML。
4. 将当前 mock graph 数据逐步替换为真实 dataflow YAML parse/validate/expand 结果。
