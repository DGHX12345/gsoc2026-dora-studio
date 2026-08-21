# M18 验收记录（2026-08-20，本地不提交）

## 基线（Task 5.5）

- cargo: **293 单测 + 1 集成全绿**（需在无后端运行时跑——后端占用 4318 会干扰 otel 测试；已按此验证）
- npm test:tools: 全过（session-ui 11 + live-feed/command/playback + dataflow-convert/palette/edge-status/save-issues）
- npm build: vue-tsc + vite 通过
- cargo fmt --check: 干净（8 个残留文件的 rustfmt 1.8.0 差异已消除——当前环境 fmt 全绿）
- git diff --check: 干净

## 环境说明

- dora 1.0.0-rc.4（settings.json doraBin 指向 venv）
- 后端 3001 / 前端 5173；真实节点验收项目 /home/dora/m18-acceptance 已加入 Studio
- opencv-python 5.0.0 已装入 venv
- 测试隔离修复：dora_env 测试加 reset_settings_state_for_tests（82cb9b3），单线程全绿
- 已知并行 flaky：lerobot 桥 30s 超时（偶发，外部 python 桥）；与 M18 无关

## 人工验收清单（对照 Task 5.5 Step 3，学生实测记录）

1. 真实项目分组 + 真实调色板 — 学生已测（m18-acceptance 分组、真实节点入调色板）
2. 打开真实 YAML 画布编辑 + 写回 — 学生已测（保存 ok:true、注释/env 保留、备份生成）
3. 四色判定 — 学生已测（红/黄/绿/灰 + type_rules 写入）
4. validate 终审 + 真实可跑 — 写回已测；**真实相机跑通待测**（real-camera-pipeline：dora start → frame_logger 心跳）
5. typed-local 真实跑通 — **待测**（sink 输出 doubled: N × 30）
6. fixture 对齐 — out/m18-parity/RESULTS.md 3/3 对齐
7. 手动补交节点 — 学生已测（manual 徽章、上画布）
8. 臂只读 — **学生决定跳过（2026-08-21）**。诊断结论：/dev/ttyACM0 + dialout ✓、校准文件 ✓、dorobot2 可导入（已 editable 装入 venv）；但 (a) dora 1.0.0-rc.4 对 `operator: {python: module:Class}` 格式做路径解析不兼容（dorobot 自己的 CLI 包装才能跑）；(b) FeetechDriver 依赖 lerobot，dora-studio-1.0 venv 与 anaconda3/envs 下均无可用环境。按计划降级路径记录归档；arm-readonly.yml 的 CALIBRATION_DIR 已改绝对路径、节点 path 已改 ../nodes/（dora 相对路径基于 YAML 位置——此修复对全部 3 个验收 YAML 生效）。
9. i18n 中英 — 已实现（29 键）；学生可切语言验证
10. 旧行为兼容 — Studio Examples slug id 保持；旧形状 schema/check 可用
11. README — 项目目录配置说明已加

## 学生 UI 反馈修复记录（D4 验收轮次）

- 列表名不截断 + tooltip；侧栏滚动条（21319dd）
- 画布节点两行文字截断 + hover 全名（SVG JS 截断）
- pill 11px 限宽（用户明确要求）
- 选中数据流自动切到 Source 画布编辑器（Save 工具栏可见）
- timer 等外部输入源全程保留（ba00209）——修复 Save 422 根因
- validate 通用错误信息提取真实错误行
