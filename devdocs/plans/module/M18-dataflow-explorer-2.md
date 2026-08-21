# M18: Dataflow Explorer 2.0（真实节点 + 真实类型）

**Layer**: 1-2 — Dataflow Live Graph（M01/M02 升级）
**Depends on**: M17（settings 文件基座）、M15.5（dora 1.0 类型语义）
**Effort**: 2 周
**Timing**: M17 之后、M16 之前（mentor 重点关注模块;本计划只设计,实施待定）

> **Revision 2026-08-17**（与学生逐节确认的设计决策）：
>
> 1. **节点来源 = 项目扫描为主 + 手动补交**：dora 1.0 无本地算子注册表
>    （hub 是远程、node list 只列运行中）;用户自己的 dataflow YAML 是节点
>    第一手事实源（id/path/inputs/outputs/input_types/output_types）。
>    用户配置项目目录（默认含 Studio examples/），扫描汇总成调色板;
>    手动补交单个节点脚本（端口与类型手工声明,诚实标注）。
> 2. **兼容判断 = 复刻 dora 语义 + validate 终审**：vendor dora 1.0 std 类型
>    目录 YAML（M00 类型复制策略）;复刻 schema_compatible（结构容忍:实际字段
>    ⊇ 期望）+ 4 条数值拓宽 + `*→Bytes` + 用户 type_rules（BFS≤3）;
>    连线级本地判定实时变色,保存/构建前 `dora validate` 真实语义终审,
>    fixture 对齐 dora 真实行为。
> 3. **「不同格式变一样」的诚实边界**：声明型兼容（type_rules/拓宽/结构容忍）
>    完全可行（dora 原生）;真运行时转换（Image→CompressedImage）dora 架构
>    不做——Studio 提示「插入转换节点」,不自动转换。
> 4. **画布双向编辑 + 另存保护**：点开扫描到的真实 YAML 直接编辑;保存可选
>    「写回原文件（确认弹窗）」或「另存为新 dataflow」。
> 5. **项目目录配置**：M17 的 `~/.config/dora-studio/settings.json` 增加
>    `projectDirs`;旧单根扫描（Studio examples/）保留兼容。
> 6. **编号 M18**（M1/M2 升级,用新号避免时序混淆）;画布在现有
>    DataflowCanvas 基础上演进,不重写。

## Purpose

Dataflow Explorer 从「写死的演示目录 + 写死的节点调色板」升级为面向真实
用户项目的编辑器:扫描真实 dataflow、真实节点与真实类型 URN,连线兼容性
用 dora 1.0 真实语义判定,type_rules 声明可视化为连线属性,保存即真实可跑。

## Architecture

```
settings 扩展(M17): ~/.config/dora-studio/settings.json + "projectDirs": [...]

backend/src/project_scan.rs (新)   真实项目发现
  扫描 projectDirs **/*.yml → 解析 dataflow(节点/端口/类型 URN)
  汇总: 数据流列表(按项目分组) + 节点调色板(跨项目去重)
  GET  /api/projects/{list,add,delete}
  POST /api/projects/nodes         手动补交(手工声明端口/类型)
  /api/dataflows 扩展多项目(旧单根扫描兼容)

backend/src/urn_catalog.rs (新)   类型目录(M00 类型复制策略)
  vendor dora 1.0 libraries/core/types/std/**/v1.yml(core/media/math/control/vision)
  零依赖 YAML 解析(复用 dataflows.rs 最小解析器风格)
  GET /api/types/catalog、GET /api/types/:urn

backend/src/compat_engine.rs (新) 兼容判定(复刻 dora 语义)
  schema_compatible(结构容忍) + 内置拓宽 + 用户 type_rules(BFS≤3)
  POST /api/schema/check 重写(旧端点形状兼容,新增 urn/rule/suggestion)
  输出 { level, reason, suggestion }(拓宽链 / 需 type_rule / 需转换节点)

backend/src/dataflow_builder.rs (扩展) 生成 + 终审
  端口类型写 input_types/output_types;连线声明写 type_rules
  保存/构建前 spawn `dora validate <yaml>` → 错误映射回画布(连线/节点)

frontend
  Source 标签: 数据流按项目分组、添加项目目录入口、打开即画布编辑(双向)
  Build 标签: 调色板 = 扫描汇总 + 手动添加;端口 URN 选择器(目录分组+搜索);
    连线实时判定(绿/黄/红/灰) + 连线属性面板(type_rules 声明、判定原因);
    保存 = 写回确认 / 另存为;validate 错误画布定位
  i18n 中英;tools 测试覆盖调色板汇总、判定状态机、保存流程
```

## Deliverables

### D1: 项目扫描（TDD）

- [ ] Step 1: 失败测试——projectDirs 读取/合并、多目录扫描、YAML 解析提取
      input_types/output_types、调色板跨项目去重、手动补交记录结构。
- [ ] Step 2: 实现 project_scan.rs + settings 扩展 + 端点。
- [ ] Step 3: /api/dataflows 多项目聚合（旧行为回归测试保持绿）。
- [ ] Step 4: 全后端测试 + commit。

### D2: URN 类型目录（TDD）

- [ ] Step 1: 失败测试——vendor 的 std YAML 解析:URN 列表、结构字段/类型
      提取（Image/CompressedImage/PointCloud/Detection 等）。
- [ ] Step 2: 实现 urn_catalog.rs + /api/types/* 端点。
- [ ] Step 3: 全后端测试 + commit。

### D3: 兼容判定引擎（TDD,fixture 对齐 dora 真实行为）

- [ ] Step 1: 失败测试——从 dora 1.0 源码（libraries/core/src/types.rs 的
      测试用例 + descriptor/validate.rs 的 edge mismatch 用例）提取 fixture:
      结构容忍、拓宽链、type_rules BFS、Bytes 兜底、不兼容原因。
- [ ] Step 2: 实现 compat_engine + /api/schema/check 重写。
- [ ] Step 3: 同一组用例在 dora 1.0 侧（dora validate）与 Studio 侧对照,
      记录对齐结果;全后端测试 + commit。**（PR 1 = D1+D2+D3,学生人工验收）**

### D4: 画布真实化（前端主体）

- [ ] 调色板接扫描结果;端口 URN 选择器;连线实时判定（四色）+ 连线属性
      面板（type_rules 声明、判定原因、转换节点建议）。
- [ ] 双向编辑:Source 打开 → 画布;保存写回确认 / 另存为。
- [ ] validate 终审:保存前调用,错误映射画布定位。
- [ ] i18n + tools 测试。

### D5: 前端收尾 + 验收

- [ ] 项目分组列表、添加项目目录、手动添加节点 UI、空状态。
- [ ] 基线:后端 ≥186+1、前端 204、build、python 97。
- [ ] 人工验收清单:
  1. 添加真实项目目录 → 左侧按项目分组;节点入调色板（非写死）
  2. 打开真实 YAML → 节点/端口/类型全部来自文件;改端口类型 → 写回 input_types/output_types
  3. UInt8→UInt32 自动绿;结构缺失红 + 原因;声明 type_rules → 变绿且写入 YAML
  4. 故意破坏类型 → validate 终审在画布定位错误;`dora start` 该 YAML 真实可跑
  5. 兼容 fixture 与 dora validate 行为对齐（两边同用例）
  6. 手动补交节点上画布（诚实标注手工声明）
  7. i18n 中英

## Acceptance Criteria

- [ ] 全部 7 条人工验收通过
- [ ] 基线全绿
- [ ] 旧单根扫描 / 旧 schema/check 形状兼容（无破坏）
- [ ] README 增加项目目录配置说明

## Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| vendor 目录与 dora 上游漂移 | Medium | Medium | 锁定版本 + fixture 对齐 + validate 终审兜底 |
| 复刻语义与 dora 判定不一致 | Medium | Medium | 同用例对照（D3 Step 3）;不一致处修正或记录 |
| validate 子进程性能（保存卡顿） | Low | Low | 仅保存/构建时调用,异步 + 超时 |
| 真实 YAML 写回误伤用户项目 | Medium | High | 确认弹窗 + 另存为默认;写回前备份原文件 |
| 画布既有拖拽/连线行为回归 | Medium | Medium | tools 测试全量回归 + 双 PR 分段验收 |
| 未声明类型的存量 YAML 端口全灰 | High | Low | 端口 URN 选择器引导补全;灰 = 诚实「未声明」 |
