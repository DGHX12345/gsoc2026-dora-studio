# devdocs — 开发知识库索引

本目录是 dora-studio 的**开发记忆归档**（本地开发资料，不属于用户版 PR）。
换电脑继续开发时：`git clone <fork> && git checkout dev` 即可恢复全部上下文。

## 目录结构

| 目录 | 内容 | 用途 |
|---|---|---|
| `handoffs/` | 9 个交接文档（HANDOFF.md 总览 + D3/DEMO/M11-M18 各代交接） | 接手时先读 `HANDOFF-M18.txt`（最新）再读 `HANDOFF.md`（M00-M18 总览） |
| `plans/module/` | M00-M18 全部模块计划 + OVERVIEW.md（模块总表/关键路径/风险）+ RUST-COMPAT.md（依赖钉死表）+ demo-manual.md | 查任何模块的设计/交付/坑位 |
| `plans/weekly/` | 早期周计划（week12-b-implementation-plan.md） | 历史参考 |
| `plans/implementation/` | superpowers 实施计划（M16.5 dora-env、M18 dataflow-explorer-2） | 已执行模块的逐任务实施记录 |
| `specs/` | 设计规格（2026-08-18 dora-env、2026-08-19 M18 设计） | 设计决策的权威记录 |
| `acceptance/` | M18 验收记录 + m18-parity（fixture 与 dora validate 对照结果） | 验收结论与对齐证据 |

## 其他关键文档（在仓库根目录，已跟踪）

- `CLAUDE.md` — 项目说明 + AI Onboarding Guide（架构/约定/运行方式）
- `README.md` — 用户版文档（PR 中的那版）
- `RUST-COMPAT.md` — 依赖钉死表（与 plans/module/ 下副本相同，以根目录为准）

## 环境记忆要点（2026-08 状态）

- 开发分支历史：`week12-2.0`（172+ commit 完整开发史）；用户版为单一 commit 的 `week12` 分支
- venv：`~/.venvs/dora-studio-1.0`（dora 1.0.0-rc.4 + pyarrow + opencv + dorobot editable）
- settings：`~/.config/dora-studio/settings.json`（doraBin 指向 1.0 venv）
- 测试注意：cargo test 须无后端占用 4318 时跑；dora-CLI 测试有已知并行 flaky
- 真实节点验收项目：`/home/dora/m18-acceptance`（本地，未入库）
