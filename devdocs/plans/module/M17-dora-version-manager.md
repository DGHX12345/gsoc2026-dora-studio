# M17: dora Version Manager（版本管理）

**Layer**: 0 — Foundation（环境）
**Depends on**: M16.5（其 D1 的 dora_env.rs 基座）
**Effort**: 0.5 周
**Timing**: M16.5 之后、M16 之前（实施顺序;本计划只设计,实施待定）

> **Revision 2026-08-17**（与学生逐节确认的设计决策）：
>
> 1. **级别 = 检测 + 切换,不做安装器**：下载/安装是 uv 的职责;Studio 负责
>    「看清」与「切换」。双 venv（0.5/1.0）场景是直接动因。
> 2. **发现 = 候选列表自动播种**：settings 文件维护候选路径,首次运行播种
>    （env/PATH 指向 + `~/.venvs/*/bin/dora` + `~/.local/bin/dora`）;
>    不做全盘扫描;UI 可手动加/删。
> 3. **热切换**：切换写 settings + 更新后端内存态 + 使版本缓存失效;新起
>    子进程立即用新版;运行中会话不受影响（UI 提示）。
> 4. **解析层级**：显式 `DORA_STUDIO_DORA_BIN` > settings.doraBin > PATH。
>    env 存在时 UI 显示「被环境变量覆盖」并禁用切换。
> 5. **实现路径 A**：扩展 M16.5 的 dora_env.rs（三级解析 + 检测/切换端点）,
>    不另立解析入口;`lifecycleUnavailable`（M16.5）= 当前版本非 1.x,单真相源。
> 6. settings 文件位置：`~/.config/dora-studio/settings.json`（用户级,不随仓库走）。

## Purpose

双版本/多版本环境下,用户一眼看清已装的 dora 版本、当前生效版本与兼容性,
一键切换;与 M16.5 的生命周期版本门联动,把「需要 dora 1.x」的提示变成
可操作的面板。

## Architecture

```
settings: ~/.config/dora-studio/settings.json
  { "doraBin": "/path", "candidates": ["/p1", "/p2"] }
  首次运行自动播种(env/PATH + ~/.venvs/*/bin/dora + ~/.local/bin/dora)

backend/src/dora_env.rs (扩展 M16.5 D1)
  resolve_dora_bin(): DORA_STUDIO_DORA_BIN > settings.doraBin > PATH "dora"
  detect_versions(): 对 candidates 逐个 `<bin> --version`(3s 超时) → 去重
    → [{ path, version, compatible(1.x), active }]
  switch(path): 校验存在且 --version 成功 → 写 settings + 更新内存态
    + dora_version() 缓存失效
  lifecycleUnavailable()(M16.5 复用)= active 非 1.x

端点
  GET  /api/dora/versions         → { active, overriddenByEnv, items[] }
  POST /api/dora/switch           { path }
  POST /api/dora/candidates/add   { path }
  POST /api/dora/candidates/delete { path }

frontend (DashboardView 环境卡,Quick Start 旁)
  当前版本 pill + 兼容徽章(完整 1.x / 降级 0.5+不可用项列表 / 未知)
  已装列表: 版本 + 路径 + 徽章,当前高亮,点选即切(热切换)
  env 覆盖: 禁用切换 + 「被环境变量覆盖」提示
  添加路径 / 删除候选;i18n 中英
```

## Deliverables

### D1: dora_env.rs 三级解析 + 检测（TDD）

- [ ] Step 1: 失败测试——`resolve_dora_bin` 三级优先级（env 胜 settings 胜
      PATH）;`detect_versions` 对 fixture 输出解析/去重/兼容判定
      （1.0.0-rc.4 → compatible,0.5.0 → 否）;settings 缺失时播种逻辑。
- [ ] Step 2: 实现（settings 读写用 serde_json + 手写路径展开,
      无新依赖）。
- [ ] Step 3: 全后端测试 + commit。

### D2: 端点 + 热切换（TDD）

- [ ] Step 1: 失败测试——switch 校验（不存在路径拒绝）;switch 后
      `resolve_dora_bin()` 返回新路径且 `dora_version()` 重新查询
      （缓存失效可测:先调 version 再 switch 再调,断言第二次查询发生）。
- [ ] Step 2: 实现 GET/POST 端点 + 内存态更新。
- [ ] Step 3: 全后端测试 + commit。

### D3: 前端环境卡（i18n + 状态机测试）

- [ ] DashboardView 环境卡:版本 pill、兼容徽章（降级列出不可用项:
      会话控制/WS 节点指标/录制）、列表点选切换、env 覆盖禁用+提示、
      添加/删除候选、切换后提示。
- [ ] i18n 中英;tools 测试覆盖按钮状态机。
- [ ] commit。

### D4: 基线 + 人工验收

- [ ] 基线:后端 ≥186+1、前端 204、build、python 97。
- [ ] 验收清单:
  1. 双 venv 自动播种:面板列出 1.0 与 0.5 两条
  2. 切 1.0 → 新子进程用 1.0、版本串更新
  3. 切 0.5 → 降级徽章 + M16.5 生命周期按钮禁用（联动）
  4. env 覆盖提示与切换禁用
  5. 手动添加/删除候选;重启后端后 settings 持久
  6. i18n 中英切换正常

## Acceptance Criteria

- [ ] 全部 6 条人工验收通过
- [ ] 基线全绿（同上）
- [ ] settings 文件用户级持久化,不随仓库提交

## Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| 播种路径不可达/权限噪音 | Low | Low | 每候选 3s 超时,失败静默跳过 |
| 热切换时运行中会话引用旧二进制 | Low | Low | UI 明示「运行中会话不受影响」;建议先停会话 |
| `dora --version` 输出格式再变 | Low | Low | 复用 M15.5 D2 归一化 + fixture 测试 |
| settings 写入失败(只读 HOME) | Low | Low | 失败时回退内存态 + 明确报错 |
