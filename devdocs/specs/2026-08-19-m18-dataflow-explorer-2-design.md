# M18 Dataflow Explorer 2.0 — 设计规格（2026-08-19 与学生确认）

> 本文件是 plans2.0/M18-dataflow-explorer-2.md 的实施细化。计划中的 6 条
> Revision 决策（2026-08-17）仍有效；本文补充 2026-08-19 会话中新确认的
> 实现级决策。按项目约定，本文件只留本地，不提交。

## 1. Purpose

Dataflow Explorer 从「写死的演示目录 + 写死的节点调色板」升级为面向真实
用户项目的编辑器：扫描真实 dataflow、真实节点与真实类型 URN，连线兼容性
用 dora 1.0 真实语义判定，type_rules 声明可视化为连线属性，保存即真实可跑。

## 2. 已确认决策（2026-08-19）

1. **写回策略 = 行级 diff patch**：已有文件只 patch 变化区域（端口类型声明、
   type_rules、新增/删除节点块），注释、env 等未知字段逐字保留；新建文件
   （另存为）用生成器全量输出。零新依赖。
2. **验收暂停 3 次**：D3 后（后端+对照）、D4 后（画布主体）、D5 总验收。
3. **validate 终审**：error（连线指向不存在端口、语法错误等）阻止保存并
   在画布定位；type mismatch 是 dora 的 warning——映射连线黄色，不阻止保存。
4. **projectDirs 与 examples**：Studio examples/ 内置固定分组（不可删、旧
   行为永久兼容）；projectDirs 首播为空，用户添加的项目目录额外分组。
5. **手动补交节点**：存 settings.json 的 `manualNodes` 数组（与 projectDirs
   同文件，复用 M17 settings store）。
6. **多项目 id**：dataflow id = sha1(绝对路径) 前 12 位 hex，同名文件互不
   冲突，id 稳定可定位。
7. **四色连线**：绿 = 原生兼容（相同/拓宽链/结构容忍/→Bytes）；黄 = 靠
   type_rules 声明才兼容；红 = 不兼容（含结构字段缺失）；灰 = 任一端未声明。
8. **Source 标签**：默认画布（双向编辑），保留「原始文本」子视图切换；
   Build 标签保留用于新建。
9. **旧 /api/schema/check 兼容**：双形状——带 URN 走新 compat 引擎；只有
   operator/port 名（旧形状）走 schema_registry.rs 硬编码注册表回退。
   schema_registry.rs 保留不动。
10. **复杂 YAML 边界**：能解析出 nodes 就允许画布编辑（patch 只触可识别
    区域）；完全解析不出 nodes 的降级为只读文本+提示。

## 3. 诚实边界（不自动转换数据格式）

- **声明层不一致**：dora 类型系统本身容纳（同 URN/拓宽/`*→Bytes`/
  type_rules）。Studio 只判定+展示，不改声明。
- **数据格式层不一致**（如 Image vs CompressedImage、RGB 编码差异）：
  **绝不自动转换，绝不伪造声明**。dora 运行时零拷贝转发不做转换
  （`is_compatible` 仅用于 validate 与上报，daemon 无 cast 逻辑，已核实）。
  红色连线给出「插入转换节点」建议；转换本身是业务逻辑，由用户显式
  插入 operator 实现。
- 注意：拓宽/type_rules 只是让 `dora validate` 不报警，数据仍原样传递，
  由接收节点按自己期望反序列化。

## 4. Architecture

```
settings.json 扩展: { doraBin, candidates, projectDirs: [], manualNodes: [] }

backend/src/project_scan.rs (新)
  扫描内置 examples/ + projectDirs **/*.yml → 解析 dataflow
  (节点 id/path/inputs/outputs/input_types/output_types)
  汇总: 数据流列表(按项目分组, id=sha1(abs_path)[..12]) + 节点调色板(跨项目去重)
  手动节点来自 settings.manualNodes, 诚实标注 manual:true

backend/src/urn_catalog.rs (新)
  vendor dora 1.0 std 类型目录:
  backend/assets/types/{core,math,control,media,vision}/v1.yml
  include_str! 嵌入 + 行级解析(M00 零依赖策略), 锁定 1.0.0-rc.4

backend/src/compat_engine.rs (新)
  复刻 dora 语义(见 §5), 输出 { level, reason, suggestion }

backend/src/schema_registry.rs (保留不动)  旧形状回退路径

backend/src/dataflow_builder.rs (扩展)
  生成器: input_types/output_types/type_rules 输出
  写回: 行级 diff patch(只触可识别区域)
```

### 端点

| 端点 | 说明 |
|---|---|
| GET /api/projects/list | 项目列表（内置 examples + 各 projectDirs，含数据流摘要） |
| POST /api/projects/add / delete | 增删 projectDirs，校验目录存在 |
| POST /api/projects/nodes | 手动补交节点，存 settings.manualNodes |
| GET /api/types/catalog | 五包分组 + 类型列表（名称/URN/description） |
| GET /api/types/:urn | 单个类型定义（字段/参数） |
| POST /api/schema/check | 双形状（§2-9），新形状 {source_urn, sink_urn, type_rules?}，响应加 urn/rule/suggestion，旧字段 compatible/level/detail 不变 |
| GET /api/dataflows | 扩展：多项目聚合，响应加 project 字段 |
| POST /api/dataflows/:id/save | 写回（diff patch + 备份 + validate 终审） |
| POST /api/dataflows/save-as | 新建/另存（全量生成 + validate 终审） |

### 保存/终审流程

1. 写回前备份到 `~/.config/dora-studio/backups/<basename>.<ts>.bak`
2. 生成目标内容写临时文件
3. spawn `dora validate <tmp>`（仅 1.x；0.5 跳过+UI 提示；10s 超时）
4. error 阻止保存并映射画布定位（解析 stderr/stdout 中的节点/端口信息）
5. warning 映射对应连线黄色，不阻止
6. 成功后原子替换目标文件

## 5. 复刻的 dora 1.0 兼容语义（源自 /home/dora/dora 源码）

- **同 base URN** → 兼容（带参数时共享键须一致，见 types.rs `is_compatible`）
- **4 条内置拓宽链**：UInt8→UInt32→UInt64、Int32→Int64、Float32→Float64
  （注意 UInt8→UInt64 是两步 BFS，不是直接边）
- **`* → std/core/v1/Bytes`** 万能 sink
- **用户 type_rules**：`type_rules: [{from, to}]` 数据流级声明，BFS 深度 ≤3
- **结构容忍**（schema_compatible）：actual 字段 ⊇ expected 字段，字段序无关；
  缺字段/类型不符 → 不兼容（含字段名与期望/实际类型详情）
- **URN 参数**：`std/media/v1/AudioFrame[sample_type=f32]`，parse_urn 解析
  base + params
- **validate 语义**：wiring 错误等是 error（bail）；type mismatch 是 warning
  （TypeWarning，不阻止运行）；上游未声明类型 → inference（推断）

## 6. 前端

- **Source 标签**：左侧项目分组列表 + 添加项目目录入口；打开即画布编辑
  （双向）；「原始文本」子视图切换；解析不出 nodes 的 YAML 只读文本+提示
- **Build 标签**：调色板 = 扫描汇总 + 手动添加按钮；端口 URN 选择器
  （目录分组+搜索）
- **连线**：实时判定四色（§2-7）+ 连线属性面板（判定原因、依赖的 type_rule）
- **type_rules 管理**：红连线面板提供「创建规则」按钮（写入数据流级
  type_rules）；画布侧栏规则列表可查看/删除（删除前提示影响多条连线）；
  删除连线不自动删规则
- **新建保存**：选择 projectDirs 之一或自定义路径，保存后归入对应分组
- i18n 中英双语，中文优先；样式只用主题 token（--text-*/--bg-*/--card-*/
  --hairline/--accent-*），正文 ≥12px

## 7. 测试策略

- 全 TDD：先失败测试再实现；测试跑器必须 await 异步测试
- D3 fixture 从 dora 1.0 源码测试用例提取（types.rs / validate.rs），
  同一组用例在 `dora validate` 与 Studio 侧对照，记录对齐结果
- 前端 tools 测试（tsx/node 无 DOM）：纯函数放独立 ts 模块
  （session-ui.ts 模式），覆盖调色板汇总、判定状态机、保存流程
- 基线：cargo 240 单测 + 1 集成、test:tools 204、npm build、cargo fmt --check、
  git diff --check
- 已知环境约束：cargo test 须无 coordinator/daemon 运行；dora 相关测试持
  TEST_ENV_LOCK；`dora list --format json` 是 JSON Lines 三格式兼容；
  spawn dora 必须 stdin null

## 8. Acceptance Criteria（与计划一致 + 本次细化）

1. 添加真实项目目录 → 左侧按项目分组；节点入调色板（非写死）
2. 打开真实 YAML → 节点/端口/类型全部来自文件；改端口类型 → 写回
   input_types/output_types；注释与未知字段原样保留
3. UInt8→UInt32 自动绿；结构缺失红 + 原因；声明 type_rules → 变黄且写入 YAML
4. 故意破坏类型 → validate 终审在画布定位错误（error 阻止保存）；
   `dora start` 该 YAML 真实可跑
5. 兼容 fixture 与 dora validate 行为对齐（两边同用例）
6. 手动补交节点上画布（诚实标注手工声明）
7. i18n 中英
8. 旧单根扫描 / 旧 schema/check 形状兼容（无破坏）
9. README 增加项目目录配置说明

## 9. Risks

| Risk | Mitigation |
|---|---|
| 写回误伤用户项目 | diff patch 只触可识别区域 + 备份 + 确认弹窗 + 另存为默认 |
| 复刻语义与 dora 判定不一致 | fixture 双跑对照（D3 Step 3），不一致修正或记录 |
| vendor 目录与上游漂移 | 锁定 1.0.0-rc.4 + fixture + validate 终审兜底 |
| validate 子进程性能 | 仅保存时调用，异步 + 10s 超时 |
| 画布既有行为回归 | tools 测试全量回归 + 3 次分段人工验收 |
| 未声明类型端口全灰 | URN 选择器引导补全；灰 = 诚实「未声明」 |

## 10. 约束（继承交接）

- 工作目录必须是 worktree week12-b；Bash cwd 会重置到主仓库，一切命令
  cd worktree 或绝对路径
- 8 个格式化残留文件（coordinator_ws.rs、drec/generator.rs、drec/reader.rs、
  lerobot.rs、live.rs、model_catalog.rs、profile.rs、protocol/types.rs）不碰
- 不自动提交；每交付物完成后问学生；只显式 git add 自己的文件
- 不动：api.ts 既有端点语义、ReplayTimeline
- 无 protoc/reqwest/serde_yaml 等新重依赖
