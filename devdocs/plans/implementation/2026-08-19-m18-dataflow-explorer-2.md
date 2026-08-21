# M18 Dataflow Explorer 2.0 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 Dataflow Explorer 升级为面向真实用户项目的编辑器：扫描真实 dataflow/节点/类型 URN，dora 1.0 语义兼容判定（四色连线），行级 diff patch 写回，validate 终审，保存即真实可跑。

**Architecture:** 后端新增 project_scan.rs（多项目扫描+调色板）、urn_catalog.rs（vendor 类型目录）、compat_engine.rs（复刻 dora 兼容语义）、validate.rs（dora validate 终审）；扩展 dataflow_builder.rs（dora 1.0 生成格式 + diff patch）、dataflows.rs（类型声明解析）、dora_env.rs（settings 扩展）。前端新增纯函数模块（convert/palette/edge-status）+ 画布双向编辑。

**Tech Stack:** Rust 1.75 + Axum（零新依赖）、Vue 3 + TS + Vite、dora 1.0.0-rc.4 CLI。

**规格:** `docs/superpowers/specs/2026-08-19-m18-dataflow-explorer-2-design.md`（本计划是其任务化展开）

---

## 执行环境（每步必读）

- 工作目录: `/home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b`（分支 week12-2.0）
- **cwd 陷阱**: Bash cwd 会被重置到主仓库 `/home/dora/gsoc2026-dora-studio`。所有命令开头 `cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b` 或用绝对路径。每次 Bash 前先 `pwd` 确认。
- 基线命令（每次全量跑）:
  ```bash
  cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b
  cargo test --manifest-path backend/Cargo.toml   # 须无 coordinator/daemon 运行; 偶发 flaky 重跑
  npm --prefix frontend run test:tools
  npm --prefix frontend run build
  cargo fmt --manifest-path backend/Cargo.toml --check
  git diff --check
  ```
- dora 1.0 CLI: `/home/dora/.venvs/dora-studio-1.0/bin/dora`（settings.json 的 doraBin 已指向它）；PATH 默认是 0.5。
- spawn dora 必须 stdin null；`dora list --format json` 是 JSON Lines。
- 改 dora env 或 settings 的测试必须持 `dora_env::TEST_ENV_LOCK`；settings 测试用 `DORA_STUDIO_SETTINGS` 临时文件 + `reset_settings_state_for_tests()`。
- **提交约定（覆盖技能默认）**: 每任务的 commit 步骤执行前先询问学生确认；commit message 英文、不带 Claude 署名；只 `git add` 自己的文件；绝不 add `plans2.0/`、`docs/superpowers/`、`HANDOFF*`、`out/`、`.dora-studio-tmp/`。
- **不碰的文件**: `backend/src/coordinator_ws.rs`、`backend/src/drec/generator.rs`、`backend/src/drec/reader.rs`、`backend/src/lerobot.rs`、`backend/src/live.rs`、`backend/src/model_catalog.rs`、`backend/src/profile.rs`、`backend/src/protocol/types.rs`（格式化残留，不属于本模块）；`api.ts` 既有端点语义、`ReplayTimeline.vue`。
- **暂停点**: Task 3 完成 → 暂停人工验收；Task 4 完成 → 暂停人工验收；Task 5 完成 → 总验收。

## 参考事实（已验证，执行时无需再查）

- dora 1.0 dataflow YAML：节点级 `input_types: {port: urn}` / `output_types: {port: urn}`；数据流级 `type_rules: [{from, to}]`。参考 `/home/dora/dora/examples/typed-dataflow/dataflow.yml`。
- dora 1.0 **拒绝**无 `nodes:` 头的 YAML（已实测 `dora validate` exit 1）和 `operator: <string>`（需 `path:` 或 operator struct）。现有 builder `to_yaml()` 缺 `nodes:` 头——本计划 Task 3.4 修复。
- dora 兼容语义（`/home/dora/dora/libraries/core/src/types.rs`）：同 base URN（参数共享键一致）→ 兼容；内置拓宽 4 条 `UInt8→UInt32`、`UInt32→UInt64`、`Int32→Int64`、`Float32→Float64`；`* → std/core/v1/Bytes`；用户 rules 入图 BFS 深度≤3；struct `schema_compatible`：actual 字段 ⊇ expected 字段（按名查，序无关）。
- dora validate 语义（`libraries/core/src/descriptor/validate.rs`）：wiring 错误是 error（exit≠0）；type mismatch 是 warning（不阻止）。warning 文本形如 `type mismatch on input "x": upstream a/b declares "urn1", but expected "urn2"`；error 文本含 `node `id`` 与 `input "x"`。
- std 类型目录源: `/home/dora/dora/libraries/core/types/std/{core,math,control,media,vision}/v1.yml`，格式 `types: {Name: {arrow, description, fields: [{name, type}], params: [{name, default}]}}`。
- 现有测试基线: cargo 240 单测 + 1 集成（backend/tests/model_assets.rs）；前端 tools 测试为 tsx 文件（package.json `test:tools` 逐文件列出，新增测试文件必须加入该脚本）。
- dataflows.rs 现有 4 个测试断言 slug 式 id（如 `robot-perception-test`）——内置 examples 保留 slug id，仅 projectDirs 文件用 sha1 哈希 id。

---

## Task 1: D1 项目扫描（project_scan.rs + settings 扩展）

### Task 1.1: DoraSettings 扩展 projectDirs/manualNodes

**Files:**
- Modify: `backend/src/dora_env.rs:10-17`（DoraSettings）、`backend/src/dora_env.rs:48-68`（load_or_seed_settings）
- Test: `backend/src/dora_env.rs`（tests 模块内新增）

- [ ] **Step 1: 写失败测试**

在 `backend/src/dora_env.rs` tests 模块内（`use super::TEST_ENV_LOCK as ENV_LOCK;` 之后）新增：

```rust
    #[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub(crate) struct ManualPort {
        #[serde(default)]
        pub name: String,
        #[serde(default)]
        pub urn: String,
    }

    #[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub(crate) struct ManualNode {
        #[serde(default)]
        pub id: String,
        #[serde(default)]
        pub path: String,
        #[serde(default)]
        pub description: String,
        #[serde(default)]
        pub inputs: Vec<ManualPort>,
        #[serde(default)]
        pub outputs: Vec<ManualPort>,
    }
```

（注：上述 struct 定义加到文件主体，非 tests 模块内。测试代码：）

```rust
    #[test]
    fn settings_seed_has_empty_project_dirs_and_manual_nodes() {
        let _guard = ENV_LOCK.lock();
        let guard = SettingsEnvGuard::new();
        let dir = guard.dir();
        std::fs::write(
            dir.join("settings.json"),
            r#"{"doraBin":"/opt/from-settings","candidates":[]}"#,
        )
        .unwrap();
        reset_settings_state_for_tests();
        let settings = super::load_or_seed_settings();
        assert!(settings.project_dirs.is_empty());
        assert!(settings.manual_nodes.is_empty());
    }

    #[test]
    fn add_project_dir_dedupes_and_persists() {
        let _guard = ENV_LOCK.lock();
        let guard = SettingsEnvGuard::new();
        let dir = guard.dir();
        let target = dir.join("proj-a");
        std::fs::create_dir_all(&target).unwrap();
        reset_settings_state_for_tests();
        let list = super::add_project_dir(target.to_string_lossy().as_ref()).unwrap();
        assert_eq!(list.len(), 1);
        // duplicate add is a no-op
        let list2 = super::add_project_dir(target.to_string_lossy().as_ref()).unwrap();
        assert_eq!(list2.len(), 1);
        // persisted: fresh load sees it
        reset_settings_state_for_tests();
        assert_eq!(super::project_dirs().len(), 1);
    }

    #[test]
    fn add_manual_node_persists_and_dedupes() {
        let _guard = ENV_LOCK.lock();
        let guard = SettingsEnvGuard::new();
        reset_settings_state_for_tests();
        let node = super::ManualNode {
            id: "my-converter".into(),
            path: "/tmp/convert.py".into(),
            description: "RGB to BGR".into(),
            inputs: vec![super::ManualPort { name: "image".into(), urn: "std/media/v1/Image".into() }],
            outputs: vec![super::ManualPort { name: "image".into(), urn: "std/media/v1/Image".into() }],
        };
        super::add_manual_node(node.clone()).unwrap();
        super::add_manual_node(node).unwrap(); // duplicate id replaced
        let nodes = super::manual_nodes();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, "my-converter");
    }
```

- [ ] **Step 2: 运行确认失败**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml settings_seed_has_empty
```
Expected: 编译错误（DoraSettings 无 project_dirs/manual_nodes 字段）。

- [ ] **Step 3: 最小实现**

1. `DoraSettings` 增加字段（`backend/src/dora_env.rs:10-17`）：

```rust
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DoraSettings {
    #[serde(default)]
    pub dora_bin: Option<String>,
    #[serde(default)]
    pub candidates: Vec<String>,
    #[serde(default)]
    pub project_dirs: Vec<String>,
    #[serde(default)]
    pub manual_nodes: Vec<ManualNode>,
}
```

2. 在 `DoraSettings` 之后加 `ManualNode`/`ManualPort`（Step 1 的 struct 定义）。
3. `load_or_seed_settings` 的播种分支改为：

```rust
            let seeded = DoraSettings {
                dora_bin: None,
                candidates: seed_candidates(),
                ..Default::default()
            };
```

4. 在 `save_settings` 之后新增 helpers：

```rust
fn mutate_settings(mutator: impl FnOnce(&mut DoraSettings)) -> Result<DoraSettings, String> {
    let mut settings = load_or_seed_settings();
    mutator(&mut settings);
    save_settings(&settings)?;
    *SETTINGS_STATE.write().expect("settings lock") = Some(settings.clone());
    Ok(settings)
}

pub(crate) fn project_dirs() -> Vec<String> {
    load_or_seed_settings().project_dirs
}

pub(crate) fn add_project_dir(path: &str) -> Result<Vec<String>, String> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|error| format!("invalid project directory {path}: {error}"))?;
    if !canonical.is_dir() {
        return Err(format!("not a directory: {path}"));
    }
    let canonical = canonical.to_string_lossy().to_string();
    let settings = mutate_settings(|settings| {
        if !settings.project_dirs.iter().any(|existing| existing == &canonical) {
            settings.project_dirs.push(canonical);
        }
    })?;
    Ok(settings.project_dirs)
}

pub(crate) fn remove_project_dir(path: &str) -> Result<Vec<String>, String> {
    let settings = mutate_settings(|settings| {
        settings.project_dirs.retain(|existing| existing != path);
    })?;
    Ok(settings.project_dirs)
}

pub(crate) fn manual_nodes() -> Vec<ManualNode> {
    load_or_seed_settings().manual_nodes
}

pub(crate) fn add_manual_node(node: ManualNode) -> Result<(), String> {
    if node.id.trim().is_empty() || node.path.trim().is_empty() {
        return Err("manual node requires id and path".to_string());
    }
    mutate_settings(|settings| {
        settings.manual_nodes.retain(|existing| existing.id != node.id);
        settings.manual_nodes.push(node);
    })?;
    Ok(())
}
```

（若 `SettingsEnvGuard::new()` 不存在或签名不同，按现有 tests 模块里的实际 guard 用法调整测试的构造行。）

- [ ] **Step 4: 运行确认通过**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml settings_seed_has_empty add_project_dir add_manual_node
```
Expected: 3 个新测试 PASS；全量 `cargo test` 保持绿。

- [ ] **Step 5: Commit（先问学生）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && git add backend/src/dora_env.rs && git commit -m "feat(settings): add projectDirs and manualNodes to settings store"
```

### Task 1.2: dataflows.rs 解析扩展（input_types/output_types/type_rules + 多根扫描支持）

**Files:**
- Modify: `backend/src/dataflows.rs`（ParsedNode/ParsedDataflow/parse_dataflow/NodeSection、新增 scan 与 hash id helper、find_file 回退）
- Test: `backend/src/dataflows.rs` tests 模块

- [ ] **Step 1: 写失败测试**

```rust
    #[test]
    fn parses_input_output_types_and_type_rules() {
        let parsed = parse_dataflow(
            r#"
type_rules:
  - from: std/core/v1/UInt8
    to: std/core/v1/String
nodes:
  - id: sensor
    path: sensor.py
    outputs:
      - reading
    output_types:
      reading: std/core/v1/Float64
  - id: processor
    path: processor.py
    inputs:
      reading: sensor/reading
    input_types:
      reading: std/core/v1/Float64
"#,
            "typed.yml",
        )
        .expect("typed dataflow parses");
        assert_eq!(
            parsed.type_rules,
            vec![("std/core/v1/UInt8".to_string(), "std/core/v1/String".to_string())]
        );
        let sensor = &parsed.nodes[0];
        assert_eq!(
            sensor.output_types.get("reading").map(String::as_str),
            Some("std/core/v1/Float64")
        );
        let processor = &parsed.nodes[1];
        assert_eq!(
            processor.input_types.get("reading").map(String::as_str),
            Some("std/core/v1/Float64")
        );
    }

    #[test]
    fn hashed_id_is_stable_for_absolute_path() {
        use sha1::{Digest, Sha1};
        let abs = "/home/user/projects/demo/dataflow.yml";
        let expected = format!("{:x}", Sha1::digest(abs.as_bytes()));
        assert_eq!(hashed_dataflow_id(abs), expected[..12].to_string());
    }
```

- [ ] **Step 2: 运行确认失败**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml parses_input_output_types hashed_id_is_stable
```
Expected: 编译错误（字段/函数不存在）。

- [ ] **Step 3: 最小实现**

1. `ParsedDataflow`/`ParsedNode` 扩展（`backend/src/dataflows.rs:18-28`）：

```rust
pub(crate) struct ParsedDataflow {
    pub(crate) nodes: Vec<ParsedNode>,
    pub(crate) type_rules: Vec<(String, String)>,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

pub(crate) struct ParsedNode {
    pub(crate) id: String,
    pub(crate) path: Option<String>,
    pub(crate) inputs: BTreeMap<String, String>,
    pub(crate) outputs: Vec<String>,
    pub(crate) input_types: BTreeMap<String, String>,
    pub(crate) output_types: BTreeMap<String, String>,
}
```

2. `NodeSection` 扩展为：

```rust
enum NodeSection {
    Inputs,
    Outputs,
    InputTypes,
    OutputTypes,
}
```

3. `parse_dataflow` 修改：
   - 函数开头（`let mut nodes = Vec::new();` 之前）加：

```rust
    let type_rules = parse_type_rules(source, label, &mut diagnostics);
```

   - 节点新建处 `current_node = Some(ParsedNode { ... })` 补 `input_types: BTreeMap::new(), output_types: BTreeMap::new()`。
   - section 匹配处（`} else if trimmed == "outputs:" {` 之后）加：

```rust
        } else if trimmed == "input_types:" {
            current_section = Some(NodeSection::InputTypes);
        } else if trimmed == "output_types:" {
            current_section = Some(NodeSection::OutputTypes);
```

   - section 处理处加：

```rust
                NodeSection::InputTypes => parse_typed_port(trimmed, &mut node.input_types),
                NodeSection::OutputTypes => parse_typed_port(trimmed, &mut node.output_types),
```

4. 新函数（`parse_output` 之后）：

```rust
fn parse_typed_port(line: &str, types: &mut BTreeMap<String, String>) {
    if let Some((name, urn)) = line.split_once(':') {
        let urn = clean_scalar(urn);
        if !urn.is_empty() {
            types.insert(clean_scalar(name), urn);
        }
    }
}

fn parse_type_rules(
    source: &str,
    label: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<(String, String)> {
    let mut rules = Vec::new();
    let mut in_rules = false;
    let mut current_from: Option<String> = None;
    for raw_line in source.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = raw_line.chars().take_while(|ch| ch.is_whitespace()).count();
        if !in_rules {
            if trimmed == "type_rules:" && indent == 0 {
                in_rules = true;
            }
            continue;
        }
        if indent == 0 && trimmed != "type_rules:" {
            break;
        }
        if let Some(from) = trimmed.strip_prefix("- from:") {
            current_from = Some(clean_scalar(from));
        } else if let Some(to) = trimmed.strip_prefix("to:") {
            if let Some(from) = current_from.take() {
                rules.push((from, clean_scalar(to)));
            }
        } else if indent > 0 && !trimmed.starts_with('-') {
            // tolerate unknown keys inside rules
        }
    }
    if in_rules && rules.is_empty() {
        diagnostics.push(Diagnostic {
            severity: "warn".to_string(),
            message: format!("No type_rules were parsed from {label}."),
        });
    }
    rules
}
```

5. 解析器可见性与多根扫描（`dataflow_file` 之后新增）：

```rust
pub(crate) fn hashed_dataflow_id(abs_path: &str) -> String {
    use sha1::{Digest, Sha1};
    format!("{:x}", Sha1::digest(abs_path.as_bytes()))[..12].to_string()
}

/// Scan a directory tree for dataflow YAML files, computing DataflowFile
/// entries with ids relative to `root`. With `hash_ids` the id is a sha1
/// of the canonical absolute path (used for user project directories).
pub(crate) fn scan_dataflows_in(
    root: &Path,
    hash_ids: bool,
) -> Result<Vec<DataflowFile>, DataflowError> {
    let mut paths = Vec::new();
    collect_yaml_files(root, &mut paths)?;
    paths.sort();
    paths
        .into_iter()
        .map(|path| dataflow_file_for(root, path, hash_ids))
        .collect()
}

fn dataflow_file_for(
    root: &Path,
    path: PathBuf,
    hash_ids: bool,
) -> Result<DataflowFile, DataflowError> {
    let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    let mut file = dataflow_file(root, path)?;
    if hash_ids {
        file.id = hashed_dataflow_id(&canonical.to_string_lossy());
    }
    Ok(file)
}
```

`parse_dataflow`、`read_parsed_dataflow`、`collect_yaml_files`、`dataflow_file`、`edge_count`、`workspace_root` 的可见性改为 `pub(crate)`（`edge_count` 的参数类型 `ParsedDataflow` 已是 pub(crate)，无需改签名）。

6. `find_file`（`backend/src/dataflows.rs:423`）改为先内置后项目目录回退：

```rust
fn find_file(id: &str) -> Result<DataflowFile, DataflowError> {
    if let Some(file) = discover_files()?.into_iter().find(|file| file.id == id) {
        return Ok(file);
    }
    crate::project_scan::find_dataflow_file(id)
}
```

（project_scan 模块在 Task 1.3 创建；本步骤先让编译通过：在 `main.rs` 的 mod 声明区加 `mod project_scan;`，并创建最小 `backend/src/project_scan.rs`：

```rust
//! Multi-project dataflow discovery (M18). Built in Task 1.3.
use crate::dataflows::{DataflowError, DataflowFile};

pub(crate) fn find_dataflow_file(id: &str) -> Result<DataflowFile, DataflowError> {
    Err(DataflowError::NotFound(format!(
        "Dataflow '{id}' was not found."
    )))
}
```

）

- [ ] **Step 4: 运行确认通过**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml parses_input_output_types hashed_id_is_stable
```
Expected: 2 个新测试 PASS；全量 cargo test 保持绿（尤其 `discovers_example_dataflow`、`loads_example_definition` 等旧测试）。

- [ ] **Step 5: Commit（先问学生）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && git add backend/src/dataflows.rs backend/src/main.rs backend/src/project_scan.rs && git commit -m "feat(dataflows): parse typed port declarations and type_rules, add multi-root scan support"
```

### Task 1.3: project_scan.rs 主体（多项目聚合 + 调色板）

**Files:**
- Modify: `backend/src/project_scan.rs`（替换占位实现）
- Test: `backend/src/project_scan.rs` tests 模块

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_project(name: &str, yaml: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("dora-studio-proj-{}-{}", name, uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("dataflow.yml"), yaml).unwrap();
        dir
    }

    #[test]
    fn palette_dedupes_nodes_across_projects() {
        let a = tmp_project("a", "nodes:\n  - id: cam\n    path: cam.py\n    outputs:\n      - image\n    output_types:\n      image: std/media/v1/Image\n");
        let b = tmp_project("b", "nodes:\n  - id: cam\n    path: cam.py\n    outputs:\n      - image\n");
        let entries = palette_for_dirs(&[a.clone(), b.clone()]);
        assert_eq!(entries.len(), 1, "same path deduped");
        let cam = &entries[0];
        assert_eq!(cam.outputs[0].name, "image");
        assert_eq!(cam.outputs[0].urn.as_deref(), Some("std/media/v1/Image"));
        fs::remove_dir_all(&a).ok();
        fs::remove_dir_all(&b).ok();
    }

    #[test]
    fn scan_projects_reports_dataflow_ids_with_hash() {
        let a = tmp_project("a", "nodes:\n  - id: n\n    path: n.py\n    outputs:\n      - out\n");
        let files = scan_project_dir(&a, "proj-a").unwrap();
        assert_eq!(files.len(), 1);
        let canonical = fs::canonicalize(a.join("dataflow.yml")).unwrap();
        assert_eq!(
            files[0].id,
            crate::dataflows::hashed_dataflow_id(&canonical.to_string_lossy())
        );
        fs::remove_dir_all(&a).ok();
    }

    #[test]
    fn manual_nodes_flagged_in_palette() {
        let node = crate::dora_env::ManualNode {
            id: "conv".into(),
            path: "/tmp/conv.py".into(),
            description: "convert".into(),
            inputs: vec![crate::dora_env::ManualPort { name: "in".into(), urn: "std/media/v1/Image".into() }],
            outputs: vec![],
        };
        let entry = palette_entry_from_manual(&node);
        assert!(entry.manual);
        assert_eq!(entry.operator, "conv");
    }
}
```

- [ ] **Step 2: 运行确认失败**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml palette_dedupes scan_projects_reports manual_nodes_flagged
```
Expected: 编译错误。

- [ ] **Step 3: 最小实现**（完整替换 `backend/src/project_scan.rs`）

```rust
//! Multi-project dataflow discovery (M18): scans user project directories
//! plus the built-in Studio examples, aggregates a cross-project node
//! palette, and merges dataflow listings for the explorer UI.

use crate::dataflows::{
    self, DataflowError, DataflowFile, DataflowSummary,
};
use crate::dora_env;
use crate::models::DataflowSummary as Summary;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub name: String,
    pub path: String,
    pub builtin: bool,
    pub dataflow_count: u32,
    pub dataflows: Vec<DataflowSummary>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortDef {
    pub name: String,
    pub urn: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaletteEntry {
    pub id: String,
    pub operator: String,
    pub path: Option<String>,
    pub runtime: String,
    pub project: String,
    pub manual: bool,
    pub inputs: Vec<PortDef>,
    pub outputs: Vec<PortDef>,
}

fn builtin_examples_root() -> Option<PathBuf> {
    dataflows::workspace_root().ok().map(|root| root.join("examples"))
}

fn runtime_for_path(path: Option<&str>) -> String {
    match path
        .and_then(|value| Path::new(value).extension())
        .and_then(|value| value.to_str())
    {
        Some("py") => "python".to_string(),
        Some("rs") => "rust".to_string(),
        Some("cpp") | Some("cc") | Some("cxx") => "c++".to_string(),
        Some("c") => "c".to_string(),
        _ => "python".to_string(),
    }
}

pub(crate) fn scan_project_dir(
    dir: &Path,
    project: &str,
) -> Result<Vec<DataflowFile>, DataflowError> {
    let _ = project;
    dataflows::scan_dataflows_in(dir, true)
}

pub(crate) fn find_dataflow_file(id: &str) -> Result<DataflowFile, DataflowError> {
    for dir in dora_env::project_dirs() {
        let root = PathBuf::from(&dir);
        if let Ok(files) = dataflows::scan_dataflows_in(&root, true) {
            if let Some(file) = files.into_iter().find(|file| file.id == id) {
                return Ok(file);
            }
        }
    }
    Err(DataflowError::NotFound(format!(
        "Dataflow '{id}' was not found."
    )))
}

/// All dataflows across builtin examples and project dirs, deduplicated by
/// canonical path (builtin wins on overlap).
pub fn list_all_dataflows() -> Result<Vec<DataflowSummary>, DataflowError> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    if let Some(root) = builtin_examples_root() {
        for file in dataflows::scan_dataflows_in(&root, false)? {
            let canonical = std::fs::canonicalize(&file.path)
                .unwrap_or_else(|_| file.path.clone());
            if seen.insert(canonical) {
                out.push(summary_for(&file, "Studio Examples")?);
            }
        }
    }
    for dir in dora_env::project_dirs() {
        let root = PathBuf::from(&dir);
        if !root.is_dir() {
            continue;
        }
        let name = root
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| dir.clone());
        for file in dataflows::scan_dataflows_in(&root, true)? {
            let canonical = std::fs::canonicalize(&file.path)
                .unwrap_or_else(|_| file.path.clone());
            if seen.insert(canonical) {
                out.push(summary_for(&file, &name)?);
            }
        }
    }
    Ok(out)
}

fn summary_for(file: &DataflowFile, project: &str) -> Result<DataflowSummary, DataflowError> {
    let parsed = dataflows::read_parsed_dataflow(&file.path);
    let (status, node_count, edge_count) = match parsed {
        Ok(dataflow) => (
            "stopped".to_string(),
            dataflow.nodes.len() as u32,
            dataflows::edge_count(&dataflow),
        ),
        Err(_) => ("invalid".to_string(), 0, 0),
    };
    Ok(Summary {
        id: file.id.clone(),
        name: file.name.clone(),
        project: project.to_string(),
        status,
        node_count,
        edge_count,
    })
}

pub fn list_projects() -> Result<Vec<ProjectSummary>, DataflowError> {
    let mut projects = Vec::new();
    if let Some(root) = builtin_examples_root() {
        let files = dataflows::scan_dataflows_in(&root, false)?;
        let dataflows = files.iter().map(|file| summary_for(file, "Studio Examples")).collect::<Result<Vec<_>, _>>()?;
        projects.push(ProjectSummary {
            name: "Studio Examples".to_string(),
            path: root.to_string_lossy().to_string(),
            builtin: true,
            dataflow_count: dataflows.len() as u32,
            dataflows,
        });
    }
    for dir in dora_env::project_dirs() {
        let root = PathBuf::from(&dir);
        let name = root
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| dir.clone());
        let files = if root.is_dir() {
            dataflows::scan_dataflows_in(&root, true).unwrap_or_default()
        } else {
            Vec::new()
        };
        let dataflows = files.iter().map(|file| summary_for(file, &name)).collect::<Result<Vec<_>, _>>()?;
        projects.push(ProjectSummary {
            name,
            path: dir,
            builtin: false,
            dataflow_count: dataflows.len() as u32,
            dataflows,
        });
    }
    Ok(projects)
}

pub(crate) fn palette_for_dirs(dirs: &[PathBuf]) -> Vec<PaletteEntry> {
    let mut by_path: BTreeMap<String, PaletteEntry> = BTreeMap::new();
    for dir in dirs {
        let Ok(files) = dataflows::scan_dataflows_in(dir, true) else {
            continue;
        };
        for file in files {
            let Ok(source) = std::fs::read_to_string(&file.path) else {
                continue;
            };
            let Ok(parsed) = dataflows::parse_dataflow(&source, &file.relative_path) else {
                continue;
            };
            for node in parsed.nodes {
                let key = node
                    .path
                    .clone()
                    .unwrap_or_else(|| node.id.clone());
                by_path
                    .entry(key.clone())
                    .and_modify(|entry| {
                        // merge: keep the entry with richer type info
                        if entry.inputs.iter().all(|port| port.urn.is_none())
                            && !node.input_types.is_empty()
                        {
                            for port in &mut entry.inputs {
                                if let Some(urn) = node.input_types.get(&port.name) {
                                    port.urn = Some(urn.clone());
                                }
                            }
                        }
                        if entry.outputs.iter().all(|port| port.urn.is_none())
                            && !node.output_types.is_empty()
                        {
                            for port in &mut entry.outputs {
                                if let Some(urn) = node.output_types.get(&port.name) {
                                    port.urn = Some(urn.clone());
                                }
                            }
                        }
                    })
                    .or_insert_with(|| PaletteEntry {
                        id: node.id.clone(),
                        operator: key,
                        path: node.path.clone(),
                        runtime: runtime_for_path(node.path.as_deref()),
                        project: dir
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        manual: false,
                        inputs: node
                            .inputs
                            .keys()
                            .map(|name| PortDef {
                                name: name.clone(),
                                urn: node.input_types.get(name).cloned(),
                            })
                            .collect(),
                        outputs: node
                            .outputs
                            .iter()
                            .map(|name| PortDef {
                                name: name.clone(),
                                urn: node.output_types.get(name).cloned(),
                            })
                            .collect(),
                    });
            }
        }
    }
    by_path.into_values().collect()
}

pub fn palette() -> Vec<PaletteEntry> {
    let mut dirs = Vec::new();
    if let Some(root) = builtin_examples_root() {
        dirs.push(root);
    }
    dirs.extend(dora_env::project_dirs().into_iter().map(PathBuf::from));
    let mut entries = palette_for_dirs(&dirs);
    for node in dora_env::manual_nodes() {
        entries.push(palette_entry_from_manual(&node));
    }
    entries.sort_by(|a, b| a.operator.cmp(&b.operator));
    entries
}

pub(crate) fn palette_entry_from_manual(node: &dora_env::ManualNode) -> PaletteEntry {
    PaletteEntry {
        id: node.id.clone(),
        operator: node.id.clone(),
        path: Some(node.path.clone()),
        runtime: runtime_for_path(Some(&node.path)),
        project: "manual".to_string(),
        manual: true,
        inputs: node
            .inputs
            .iter()
            .map(|port| PortDef { name: port.name.clone(), urn: Some(port.urn.clone()) })
            .collect(),
        outputs: node
            .outputs
            .iter()
            .map(|port| PortDef { name: port.name.clone(), urn: Some(port.urn.clone()) })
            .collect(),
    }
}
```

（`models::DataflowSummary` 的 `project` 字段在 Task 1.4 Step 3 加，本任务编译通过依赖它——若编译器报缺字段，先把该字段加上再继续；`read_parsed_dataflow`/`edge_count`/`workspace_root` 已在 Task 1.2 改为 pub(crate)。注意与现有测试 `discovers_example_dataflow` 兼容：`builtin_examples_root` 的扫描不改变内置 id 方案。）

- [ ] **Step 4: 运行确认通过**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml palette_dedupes scan_projects_reports manual_nodes_flagged
```
Expected: 3 个新测试 PASS；全量保持绿。

- [ ] **Step 5: Commit（先问学生）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && git add backend/src/project_scan.rs backend/src/dataflows.rs && git commit -m "feat(projects): multi-project scan, cross-project palette, manual node entries"
```

### Task 1.4: 端点（/api/projects/*、/api/projects/nodes、/api/dataflows 多项目）

**Files:**
- Modify: `backend/src/models.rs`（DataflowSummary 加 project；新增 ProjectListResponse/ManualNodeRequest）
- Modify: `backend/src/main.rs`（routes + 4 个 handler；`dataflows` handler 换聚合实现）
- Test: `backend/src/main.rs` tests 模块（若有）或 `backend/src/project_scan.rs` tests

- [ ] **Step 1: 写失败测试**

`models.rs` 加（若尚无）:

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectListResponse {
    pub projects: Vec<crate::project_scan::ProjectSummary>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddProjectRequest {
    pub path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualNodeRequest {
    pub id: String,
    pub path: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub inputs: Vec<ManualPortRequest>,
    #[serde(default)]
    pub outputs: Vec<ManualPortRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualPortRequest {
    pub name: String,
    #[serde(default)]
    pub urn: String,
}
```

`DataflowSummary` 加字段：

```rust
    #[serde(default)]
    pub project: String,
```

- [ ] **Step 2: 运行确认失败**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml
```
Expected: 编译错误（main.rs 未改、DataflowSummary 构造缺字段——把 `dataflows.rs` 的 `list_dataflows` 里构造处补 `project: "Studio Examples".to_string()`）。

- [ ] **Step 3: 最小实现**

1. routes（`backend/src/main.rs:100` 附近）加：

```rust
        .route("/api/projects/list", get(projects_list))
        .route("/api/projects/add", post(projects_add))
        .route("/api/projects/delete", post(projects_delete))
        .route("/api/projects/nodes", post(projects_nodes))
```

2. `dataflows` handler 改：

```rust
async fn dataflows() -> Result<Json<Vec<models::DataflowSummary>>, ApiError> {
    project_scan::list_all_dataflows().map(Json).map_err(|error| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("Failed to list dataflows: {error}"),
    })
}
```

（原 `dataflows::list_dataflows()` 保留不动——其旧测试继续绿。）

3. 新 handlers（`schema_operator` 之后）：

```rust
async fn projects_list() -> Result<Json<models::ProjectListResponse>, ApiError> {
    project_scan::list_projects().map(|projects| Json(models::ProjectListResponse { projects })).map_err(|error| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("Failed to list projects: {error}"),
    })
}

async fn projects_add(
    Json(req): Json<models::AddProjectRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    dora_env::add_project_dir(&req.path).map_err(|error| ApiError {
        status: StatusCode::UNPROCESSABLE_ENTITY,
        message: error,
    })?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn projects_delete(
    Json(req): Json<models::AddProjectRequest>,
) -> Json<serde_json::Value> {
    let _ = dora_env::remove_project_dir(&req.path);
    Json(serde_json::json!({ "ok": true }))
}

async fn projects_nodes(
    Json(req): Json<models::ManualNodeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let node = dora_env::ManualNode {
        id: req.id,
        path: req.path,
        description: req.description,
        inputs: req.inputs.into_iter().map(|port| dora_env::ManualPort { name: port.name, urn: port.urn }).collect(),
        outputs: req.outputs.into_iter().map(|port| dora_env::ManualPort { name: port.name, urn: port.urn }).collect(),
    };
    dora_env::add_manual_node(node).map_err(|error| ApiError {
        status: StatusCode::UNPROCESSABLE_ENTITY,
        message: error,
    })?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
```

- [ ] **Step 4: 运行确认通过**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml
```
Expected: 全量绿。手动冒烟（后端跑起后）：

```bash
curl -s http://127.0.0.1:3001/api/projects/list | head -c 400
curl -s -X POST http://127.0.0.1:3001/api/projects/add -H 'Content-Type: application/json' -d '{"path":"/home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b/examples"}'
```
Expected: 第一个返回 projects 数组（内置 Studio Examples）；第二个 ok:true 且 `/api/dataflows` 数量不变（canonical 去重，examples 已在内置中）。

- [ ] **Step 5: Commit（先问学生）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && git add backend/src/main.rs backend/src/models.rs && git commit -m "feat(api): add projects and manual-node endpoints, multi-project dataflow listing"
```

### Task 1.5: D1 全量验证

- [ ] **Step 1: 基线**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml && cargo fmt --manifest-path backend/Cargo.toml --check && git diff --check
```
Expected: 全绿（240+1 旧测试 + 新测试）。

---

## Task 2: D2 URN 类型目录（urn_catalog.rs）

### Task 2.1: vendor std 类型 YAML

**Files:**
- Create: `backend/assets/types/core/v1.yml`、`math/v1.yml`、`control/v1.yml`、`media/v1.yml`、`vision/v1.yml`

- [ ] **Step 1: 复制并锁定版本**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b
mkdir -p backend/assets/types
for cat in core math control media vision; do
  cp /home/dora/dora/libraries/core/types/std/$cat/v1.yml backend/assets/types/$cat/v1.yml
done
ls -la backend/assets/types/*/v1.yml
```
Expected: 5 个文件存在。记录来源版本：dora 1.0.0-rc.4（写入 `backend/assets/types/README.md` 一行说明 "Vendored from dora 1.0.0-rc.4 libraries/core/types/std/"）。

### Task 2.2: 行级解析器

**Files:**
- Create: `backend/src/urn_catalog.rs`
- Test: `backend/src/urn_catalog.rs` tests 模块

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_media_catalog() {
        let catalog = Catalog::new();
        let image = catalog.resolve("std/media/v1/Image").expect("Image resolves");
        assert_eq!(image.arrow, "Struct");
        assert_eq!(image.fields.len(), 4);
        assert!(image.fields.iter().any(|field| field.name == "width" && field.field_type == "UInt32"));
        let compressed = catalog.resolve("std/media/v1/CompressedImage").expect("resolves");
        assert_eq!(compressed.arrow, "LargeBinary");
    }

    #[test]
    fn resolves_short_names_and_params() {
        let catalog = Catalog::new();
        assert!(catalog.resolve_short_name("Image").is_some());
        // parameterized URN resolves via base
        assert!(catalog.resolve("std/media/v1/AudioFrame[sample_type=f32]").is_some());
    }

    #[test]
    fn lists_all_urns_grouped() {
        let catalog = Catalog::new();
        let all = catalog.entries();
        assert!(all.iter().any(|entry| entry.urn == "std/core/v1/Bytes"));
        assert!(all.len() >= 20);
    }
}
```

- [ ] **Step 2: 运行确认失败**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml parses_media_catalog resolves_short_names lists_all_urns
```
Expected: 编译错误（模块不存在）。

- [ ] **Step 3: 最小实现**（完整文件）

```rust
//! Vendored dora 1.0 std type catalog (M18).
//!
//! The YAML files under assets/types/ are copied verbatim from dora
//! 1.0.0-rc.4 `libraries/core/types/std/` and parsed with the same
//! zero-dependency line-based approach used by dataflows.rs.

use std::collections::BTreeMap;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeField {
    pub name: String,
    pub field_type: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeParam {
    pub name: String,
    pub default: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeDef {
    pub urn: String,
    pub name: String,
    pub category: String,
    pub arrow: String,
    pub description: Option<String>,
    pub fields: Vec<TypeField>,
    pub params: Vec<TypeParam>,
}

const PACKAGES: &[(&str, &str)] = &[
    ("core", include_str!("../assets/types/core/v1.yml")),
    ("math", include_str!("../assets/types/math/v1.yml")),
    ("control", include_str!("../assets/types/control/v1.yml")),
    ("media", include_str!("../assets/types/media/v1.yml")),
    ("vision", include_str!("../assets/types/vision/v1.yml")),
];

#[derive(Debug, Clone)]
pub struct Catalog {
    types: BTreeMap<String, TypeDef>,
}

impl Catalog {
    pub fn new() -> Self {
        let mut types = BTreeMap::new();
        for (category, yaml) in PACKAGES {
            for def in parse_package(category, yaml) {
                types.insert(def.urn.clone(), def);
            }
        }
        Self { types }
    }

    pub fn entries(&self) -> Vec<TypeDef> {
        self.types.values().cloned().collect()
    }

    /// Resolve a URN, stripping parameter suffixes for lookup.
    pub fn resolve(&self, urn: &str) -> Option<TypeDef> {
        let base = urn.split('[').next().unwrap_or(urn);
        self.types.get(base).cloned()
    }

    /// Resolve by short type name (last path segment).
    pub fn resolve_short_name(&self, name: &str) -> Option<TypeDef> {
        self.types
            .values()
            .find(|def| def.name == name)
            .cloned()
    }
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}

fn clean_scalar(value: &str) -> String {
    value
        .trim()
        .trim_matches(|ch| matches!(ch, '\'' | '"'))
        .to_string()
}

fn parse_package(category: &str, yaml: &str) -> Vec<TypeDef> {
    let mut defs = Vec::new();
    let mut current: Option<TypeDef> = None;
    let mut in_fields = false;
    let mut in_params = false;

    for raw_line in yaml.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = raw_line.chars().take_while(|ch| ch.is_whitespace()).count();

        if indent == 0 {
            // "types:" root key
            continue;
        }
        if indent == 2 && trimmed.ends_with(':') {
            if let Some(def) = current.take() {
                defs.push(def);
            }
            let name = trimmed.trim_end_matches(':');
            current = Some(TypeDef {
                urn: format!("std/{category}/v1/{name}"),
                name: name.to_string(),
                category: category.to_string(),
                arrow: String::new(),
                description: None,
                fields: Vec::new(),
                params: Vec::new(),
            });
            in_fields = false;
            in_params = false;
            continue;
        }

        let Some(def) = current.as_mut() else { continue };

        if trimmed == "fields:" {
            in_fields = true;
            in_params = false;
            continue;
        }
        if trimmed == "params:" {
            in_params = true;
            in_fields = false;
            continue;
        }
        if indent == 4 && !trimmed.starts_with('-') {
            if let Some(value) = trimmed.strip_prefix("arrow:") {
                def.arrow = clean_scalar(value);
            } else if let Some(value) = trimmed.strip_prefix("description:") {
                def.description = Some(clean_scalar(value));
            } else {
                in_fields = false;
                in_params = false;
            }
            continue;
        }
        if in_fields {
            if let Some(value) = trimmed.strip_prefix("- name:") {
                def.fields.push(TypeField {
                    name: clean_scalar(value),
                    field_type: String::new(),
                });
            } else if let Some(value) = trimmed.strip_prefix("type:") {
                if let Some(field) = def.fields.last_mut() {
                    field.field_type = clean_scalar(value);
                }
            }
        } else if in_params {
            if let Some(value) = trimmed.strip_prefix("- name:") {
                def.params.push(TypeParam {
                    name: clean_scalar(value),
                    default: None,
                });
            } else if let Some(value) = trimmed.strip_prefix("default:") {
                if let Some(param) = def.params.last_mut() {
                    param.default = Some(clean_scalar(value));
                }
            }
        }
    }
    if let Some(def) = current {
        defs.push(def);
    }
    defs
}

#[cfg(test)]
mod tests {
    use super::*;
    // (Step 1 的三个测试贴在这里)
}
```

（注意：若实际 YAML 中 `fields:` 条目的 name/type 缩进层级与上面假设不符（如 name/type 在 `- name:` 同缩进 6），执行时先用 `head -60 backend/assets/types/media/v1.yml` 核对缩进再微调解析器；媒体 YAML 已知格式：`fields:` 在缩进 4，`- name:` 在缩进 6，`type:` 在缩进 8。）

- [ ] **Step 4: 运行确认通过**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml parses_media_catalog resolves_short_names lists_all_urns
```
Expected: 3 个测试 PASS（若缩进假设不符，按 Step 3 注释调整后重跑）。

- [ ] **Step 5: Commit（先问学生）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && git add backend/assets/types backend/src/urn_catalog.rs backend/src/main.rs && git commit -m "feat(types): vendor dora 1.0 std type catalog with zero-dependency parser"
```

### Task 2.3: /api/types/* 端点

**Files:**
- Modify: `backend/src/main.rs`（routes + 2 handlers + AppState 加 catalog 字段）
- Test: 冒烟（curl）为主

- [ ] **Step 1: routes 与 handlers**

`main.rs` 顶部 `mod urn_catalog;`；`AppState` 加 `catalog: urn_catalog::Catalog`；state 构造处 `catalog: urn_catalog::Catalog::new()`。

routes:

```rust
        .route("/api/types/catalog", get(types_catalog))
        .route("/api/types/:urn", get(types_get))
```

handlers:

```rust
async fn types_catalog(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "types": state.catalog.entries() }))
}

async fn types_get(
    State(state): State<Arc<AppState>>,
    Path(urn): Path<String>,
) -> Result<Json<urn_catalog::TypeDef>, ApiError> {
    state
        .catalog
        .resolve(&urn)
        .map(Json)
        .ok_or(ApiError {
            status: StatusCode::NOT_FOUND,
            message: format!("Unknown type URN: {urn}"),
        })
}
```

（注意 axum 0.6 的 path 参数带 `:` 的 URN 如 `std/core/v1/Image` 不含特殊字符，直接用即可；若 URN 带 `[param]` 需 URL 编码，前端用 `encodeURIComponent`。）

- [ ] **Step 2: 运行确认通过**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml
```
Expected: 全量绿。冒烟：

```bash
curl -s http://127.0.0.1:3001/api/types/catalog | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['types']))"
curl -s http://127.0.0.1:3001/api/types/std/media/v1/Image | python3 -m json.tool | head -20
```
Expected: types 数量 ≥20；Image 定义含 4 个字段。

- [ ] **Step 3: Commit（先问学生）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && git add backend/src/main.rs backend/src/urn_catalog.rs && git commit -m "feat(api): expose type catalog endpoints"
```

---

## Task 3: D3 兼容判定引擎 + 保存后端

### Task 3.1: compat_engine.rs — URN 解析 + 核心判定

**Files:**
- Create: `backend/src/compat_engine.rs`
- Test: `backend/src/compat_engine.rs` tests 模块

- [ ] **Step 1: 写失败测试**（fixture 取自 dora 源码测试用例，逐条对齐）

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn rules(rules: &[(&str, &str)]) -> Vec<(String, String)> {
        rules.iter().map(|(f, t)| (f.to_string(), t.to_string())).collect()
    }

    // 对齐 dora types.rs: 同 base URN 兼容（参数共享键一致）
    #[test]
    fn same_base_urn_is_compatible() {
        let result = check(
            Some("std/media/v1/Image"),
            Some("std/media/v1/Image"),
            &[],
        );
        assert!(result.compatible);
        assert_eq!(result.level, "compatible");
    }

    // 对齐 dora types.rs: 参数化同 base，共享键一致 → 兼容；不一致 → 不兼容
    #[test]
    fn parameterized_urn_shared_keys_must_agree() {
        let ok = check(
            Some("std/media/v1/AudioFrame[sample_type=f32,channels=2]"),
            Some("std/media/v1/AudioFrame[sample_type=f32]"),
            &[],
        );
        assert!(ok.compatible);
        let bad = check(
            Some("std/media/v1/AudioFrame[sample_type=f32]"),
            Some("std/media/v1/AudioFrame[sample_type=f64]"),
            &[],
        );
        assert!(!bad.compatible);
    }

    // 对齐 dora types.rs 内置拓宽: UInt8→UInt32→UInt64 链（BFS 两步）
    #[test]
    fn widening_chain_uint8_to_uint64() {
        let result = check(
            Some("std/core/v1/UInt8"),
            Some("std/core/v1/UInt64"),
            &[],
        );
        assert!(result.compatible);
        assert_eq!(result.level, "compatible");
    }

    // 对齐 dora validate.rs: anything -> Bytes 兼容
    #[test]
    fn anything_to_bytes_is_compatible() {
        let result = check(
            Some("std/media/v1/Image"),
            Some("std/core/v1/Bytes"),
            &[],
        );
        assert!(result.compatible);
    }

    // 对齐 dora validate.rs compat_user_defined_rule_in_yaml: 用户规则生效
    #[test]
    fn user_rule_makes_compatible_with_rule_level() {
        let result = check(
            Some("std/core/v1/UInt8"),
            Some("std/core/v1/String"),
            &rules(&[("std/core/v1/UInt8", "std/core/v1/String")]),
        );
        assert!(result.compatible);
        assert_eq!(result.level, "rule");
        assert_eq!(
            result.rule,
            Some(("std/core/v1/UInt8".to_string(), "std/core/v1/String".to_string()))
        );
    }

    // 对齐 dora types.rs BFS 深度限制: 4 跳用户链超过深度 3 → 不兼容
    #[test]
    fn user_rule_chain_beyond_depth_3_is_incompatible() {
        let chain = rules(&[
            ("a/b/v1/T0", "a/b/v1/T1"),
            ("a/b/v1/T1", "a/b/v1/T2"),
            ("a/b/v1/T2", "a/b/v1/T3"),
            ("a/b/v1/T3", "a/b/v1/T4"),
        ]);
        let result = check(Some("a/b/v1/T0"), Some("a/b/v1/T4"), &chain);
        assert!(!result.compatible);
    }

    // 不相关类型不兼容
    #[test]
    fn unrelated_types_incompatible_with_reason() {
        let result = check(
            Some("std/media/v1/Image"),
            Some("std/core/v1/Float64"),
            &[],
        );
        assert!(!result.compatible);
        assert_eq!(result.level, "incompatible");
        assert!(result.reason.contains("std/media/v1/Image"));
    }

    // 任一端未声明 → unknown
    #[test]
    fn undeclared_port_is_unknown() {
        let result = check(None, Some("std/core/v1/Float64"), &[]);
        assert!(!result.compatible);
        assert_eq!(result.level, "unknown");
    }

    // parse_urn 边界
    #[test]
    fn parse_urn_handles_malformed_input() {
        assert!(parse_urn("std/media/v1/AudioFrame[").is_none());
        assert!(parse_urn("std/media/v1/AudioFrame[]").is_none());
        assert!(parse_urn("std/media/v1/AudioFrame").is_some());
    }
}
```

- [ ] **Step 2: 运行确认失败**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml same_base_urn parameterized_urn widening_chain anything_to_bytes user_rule user_rule_chain unrelated_types undeclared_port parse_urn
```
Expected: 编译错误。

- [ ] **Step 3: 最小实现**

```rust
//! dora 1.0 compatibility semantics replicated for Studio (M18).
//!
//! Mirrors dora-core `types.rs` CompatibilityGraph + schema_compatible:
//! same-base URN (param agreement), 4 builtin widening edges, universal
//! `* -> Bytes` sink, user type_rules with BFS depth <= 3, and structural
//! struct compatibility (actual fields superset of expected).

use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedUrn {
    pub base: String,
    pub params: BTreeMap<String, String>,
}

pub fn parse_urn(urn: &str) -> Option<ParsedUrn> {
    let (base, params) = match urn.split_once('[') {
        None => (urn.trim(), ""),
        Some((base, rest)) => {
            let params = rest.strip_suffix(']')?.trim();
            if params.is_empty() {
                return None;
            }
            (base.trim(), params)
        }
    };
    if base.is_empty() {
        return None;
    }
    let mut map = BTreeMap::new();
    if !params.is_empty() {
        for pair in params.split(',') {
            let (key, value) = pair.split_once('=')?;
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() || value.is_empty() {
                return None;
            }
            map.insert(key.to_string(), value.to_string());
        }
    }
    Some(ParsedUrn { base: base.to_string(), params: map })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub compatible: bool,
    pub level: String, // "compatible" | "rule" | "incompatible" | "unknown"
    pub reason: String,
    pub suggestion: Option<String>,
    pub rule: Option<(String, String)>,
}

const BUILTIN_WIDENING: &[(&str, &str)] = &[
    ("std/core/v1/UInt8", "std/core/v1/UInt32"),
    ("std/core/v1/UInt32", "std/core/v1/UInt64"),
    ("std/core/v1/Int32", "std/core/v1/Int64"),
    ("std/core/v1/Float32", "std/core/v1/Float64"),
];

/// Check compatibility between a source output URN and sink input URN.
/// `user_rules` are the dataflow-level type_rules declarations.
pub fn check(
    source_urn: Option<&str>,
    sink_urn: Option<&str>,
    user_rules: &[(String, String)],
) -> CheckResult {
    let (Some(from), Some(to)) = (source_urn, sink_urn) else {
        return CheckResult {
            compatible: false,
            level: "unknown".into(),
            reason: "One or both ports have no declared type URN.".into(),
            suggestion: Some("Declare a type URN on both ports to enable compatibility checking.".into()),
            rule: None,
        };
    };

    // Universal sink
    if to == "std/core/v1/Bytes" {
        return CheckResult {
            compatible: true,
            level: "compatible".into(),
            reason: "Anything can connect to std/core/v1/Bytes.".into(),
            suggestion: None,
            rule: None,
        };
    }

    let from_parsed = parse_urn(from);
    let to_parsed = parse_urn(to);
    let from_base = from_parsed.as_ref().map(|p| p.base.as_str()).unwrap_or(from);
    let to_base = to_parsed.as_ref().map(|p| p.base.as_str()).unwrap_or(to);

    if from_base == to_base {
        let agree = match (&from_parsed, &to_parsed) {
            (Some(fp), Some(tp)) if !fp.params.is_empty() && !tp.params.is_empty() => {
                fp.params.iter().all(|(key, value)| match tp.params.get(key) {
                    Some(other) => value == other,
                    None => true,
                })
            }
            _ => true,
        };
        if agree {
            return CheckResult {
                compatible: true,
                level: "compatible".into(),
                reason: format!("Same type: {from_base}."),
                suggestion: None,
                rule: None,
            };
        }
        return CheckResult {
            compatible: false,
            level: "incompatible".into(),
            reason: format!("Parameterized type mismatch between {from} and {to}."),
            suggestion: None,
            rule: None,
        };
    }

    // BFS with depth limit 3; user edges mark the path as "rule"
    let mut edges: BTreeMap<&str, Vec<(&str, bool)>> = BTreeMap::new();
    for (f, t) in BUILTIN_WIDENING {
        edges.entry(f).or_default().push((t, false));
    }
    for (f, t) in user_rules {
        edges.entry(f.as_str()).or_default().push((t.as_str(), true));
    }

    let mut queue = VecDeque::new();
    let mut visited = std::collections::BTreeSet::new();
    let mut used_rule: Option<(String, String)> = None;
    queue.push_back((from_base.to_string(), 0u32, false, None));
    visited.insert(from_base.to_string());

    while let Some((current, depth, rule_used, first_rule)) = queue.pop_front() {
        if current == to_base {
            used_rule = first_rule.or(used_rule);
            // keep searching only for shorter/builtin paths is unnecessary;
            // first found path decides (BFS finds shortest)
            let (compatible, level, reason) = if rule_used {
                (
                    true,
                    "rule",
                    format!(
                        "Compatible via declared type_rules: {from} -> {to} (path uses {current})."
                    ),
                )
            } else {
                (
                    true,
                    "compatible",
                    format!("Compatible via built-in widening: {from} -> {to}."),
                )
            };
            return CheckResult {
                compatible,
                level: level.into(),
                reason,
                suggestion: None,
                rule: used_rule,
            };
        }
        if depth >= 3 {
            continue;
        }
        if let Some(neighbors) = edges.get(current.as_str()) {
            for (next, is_user) in neighbors {
                if !visited.contains(*next) {
                    visited.insert((*next).to_string());
                    let rule = if *is_user {
                        first_rule.or(Some((current.clone(), (*next).to_string())))
                    } else {
                        first_rule
                    };
                    queue.push_back(((*next).to_string(), depth + 1, rule_used || *is_user, rule));
                }
            }
        }
    }

    CheckResult {
        compatible: false,
        level: "incompatible".into(),
        reason: format!(
            "No compatibility path from {from} to {to} (built-in widening and declared type_rules exhausted)."
        ),
        suggestion: Some(format!(
            "Insert a conversion node that transforms {from} into {to}, or declare a type_rule for this pair."
        )),
        rule: None,
    }
}

// schema_compatible 在 Task 3.2 加入
```

- [ ] **Step 4: 运行确认通过**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml same_base_urn parameterized_urn widening_chain anything_to_bytes user_rule user_rule_chain unrelated_types undeclared_port parse_urn
```
Expected: 9 个测试 PASS。

- [ ] **Step 5: Commit（先问学生）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && git add backend/src/compat_engine.rs backend/src/main.rs && git commit -m "feat(compat): replicate dora 1.0 type compatibility semantics"
```

### Task 3.2: schema_compatible 结构容忍 + 建议

**Files:**
- Modify: `backend/src/compat_engine.rs`
- Test: 同上 tests 模块

- [ ] **Step 1: 写失败测试**

```rust
    // 对齐 dora types.rs schema_compatible: actual 字段 ⊇ expected 字段
    #[test]
    fn struct_actual_superset_of_expected_ok() {
        let result = schema_compatible(
            &[field("width", "UInt32"), field("data", "LargeBinary")],
            &[field("width", "UInt32"), field("height", "UInt32"), field("data", "LargeBinary")],
        );
        assert!(result.is_ok());
    }

    // 对齐 dora types.rs SchemaError::MissingField
    #[test]
    fn struct_missing_expected_field_fails() {
        let result = schema_compatible(
            &[field("width", "UInt32"), field("data", "LargeBinary")],
            &[field("width", "UInt32")],
        );
        match result {
            Err(SchemaError::MissingField { field }) => assert_eq!(field, "data"),
            other => panic!("expected MissingField, got {other:?}"),
        }
    }

    // 对齐 dora types.rs SchemaError::TypeMismatch
    #[test]
    fn struct_field_type_mismatch_fails() {
        let result = schema_compatible(
            &[field("data", "LargeBinary")],
            &[field("data", "Utf8")],
        );
        match result {
            Err(SchemaError::TypeMismatch { field, .. }) => assert_eq!(field, "data"),
            other => panic!("expected TypeMismatch, got {other:?}"),
        }
    }

    fn field(name: &str, field_type: &str) -> TypeField {
        TypeField { name: name.to_string(), field_type: field_type.to_string() }
    }
```

- [ ] **Step 2: 运行确认失败**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml struct_actual_superset struct_missing struct_field_type
```
Expected: 编译错误。

- [ ] **Step 3: 最小实现**

compat_engine.rs 追加：

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeField {
    pub name: String,
    pub field_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaError {
    MissingField { field: String },
    TypeMismatch { field: String, expected: String, actual: String },
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaError::MissingField { field } => write!(f, "missing field \"{field}\""),
            SchemaError::TypeMismatch { field, expected, actual } => {
                write!(f, "field \"{field}\" type mismatch: expected {expected}, got {actual}")
            }
        }
    }
}

/// Mirrors dora types.rs `schema_compatible`: every expected field must be
/// present in actual with an equal (normalized) type. Order is irrelevant.
pub fn schema_compatible(
    expected: &[TypeField],
    actual: &[TypeField],
) -> Result<(), SchemaError> {
    for expected_field in expected {
        match actual.iter().find(|field| field.name == expected_field.name) {
            Some(actual_field) => {
                if normalize_field_type(&actual_field.field_type)
                    != normalize_field_type(&expected_field.field_type)
                {
                    return Err(SchemaError::TypeMismatch {
                        field: expected_field.name.clone(),
                        expected: expected_field.field_type.clone(),
                        actual: actual_field.field_type.clone(),
                    });
                }
            }
            None => {
                return Err(SchemaError::MissingField {
                    field: expected_field.name.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Canonical form for comparing field type strings: trims, uppercases
/// primitive Arrow names, and normalizes List<...> recursively. Struct
/// references compare by exact (trimmed) string.
pub fn normalize_field_type(field_type: &str) -> String {
    let trimmed = field_type.trim();
    if let Some(inner) = trimmed
        .strip_prefix("List<")
        .and_then(|s| s.strip_suffix('>'))
    {
        return format!("List<{}>", normalize_field_type(inner));
    }
    trimmed.to_ascii_uppercase()
}
```

- [ ] **Step 4: 运行确认通过**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml struct_
```
Expected: 3 个新测试 PASS。

- [ ] **Step 5: Commit（先问学生）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && git add backend/src/compat_engine.rs && git commit -m "feat(compat): structural schema compatibility with field normalization"
```

### Task 3.3: /api/schema/check 双形状重写

**Files:**
- Modify: `backend/src/main.rs:696-701`（schema_check handler）、`backend/src/models.rs`（SchemaCheckResponse）
- Test: 冒烟（curl）

- [ ] **Step 1: models.rs 新增**

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeRuleDef {
    pub from: String,
    pub to: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaCheckResponse {
    pub compatible: bool,
    pub level: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<TypeRuleDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}
```

- [ ] **Step 2: handler 重写**（替换 `backend/src/main.rs:696-701`）

```rust
async fn schema_check(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Json<models::SchemaCheckResponse> {
    // New shape: {source_urn, sink_urn, type_rules?}
    let source_urn = body
        .get("source_urn")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let sink_urn = body
        .get("sink_urn")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    if source_urn.is_some() || sink_urn.is_some() {
        let user_rules: Vec<(String, String)> = body
            .get("type_rules")
            .and_then(|v| v.as_array())
            .map(|rules| {
                rules
                    .iter()
                    .filter_map(|rule| {
                        Some((
                            rule.get("from")?.as_str()?.to_string(),
                            rule.get("to")?.as_str()?.to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let result = compat_engine::check(source_urn, sink_urn, &user_rules);
        // enrich struct-mismatch reason via catalog (informational only, appended on mismatches)
        let detail = match (&source_urn, &sink_urn) {
            (Some(from), Some(to)) if !result.compatible => {
                enrich_struct_detail(from, to, &state.catalog)
                    .map(|detail| format!("{} ({detail})", result.reason))
                    .unwrap_or_else(|| result.reason.clone())
            }
            _ => result.reason.clone(),
        };
        return Json(models::SchemaCheckResponse {
            compatible: result.compatible,
            level: result.level,
            detail,
            urn: sink_urn.map(str::to_string),
            rule: result.rule.map(|(from, to)| models::TypeRuleDef { from, to }),
            suggestion: result.suggestion,
        });
    }

    // Old shape: {source_operator, source_port, sink_operator, sink_port}
    let request = schema_registry::CheckRequest {
        source_operator: body
            .get("source_operator")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        source_port: body
            .get("source_port")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        sink_operator: body
            .get("sink_operator")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        sink_port: body
            .get("sink_port")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
    };
    let response = state.schemas.check(&request);
    Json(models::SchemaCheckResponse {
        compatible: response.compatible,
        level: response.level,
        detail: response.detail,
        urn: None,
        rule: None,
        suggestion: None,
    })
}

fn enrich_struct_detail(
    from: &str,
    to: &str,
    catalog: &urn_catalog::Catalog,
) -> Option<String> {
    let from_def = catalog.resolve(from)?;
    let to_def = catalog.resolve(to)?;
    if from_def.fields.is_empty() || to_def.fields.is_empty() {
        return None;
    }
    let expected: Vec<compat_engine::TypeField> = to_def
        .fields
        .iter()
        .map(|field| compat_engine::TypeField {
            name: field.name.clone(),
            field_type: field.field_type.clone(),
        })
        .collect();
    let actual: Vec<compat_engine::TypeField> = from_def
        .fields
        .iter()
        .map(|field| compat_engine::TypeField {
            name: field.name.clone(),
            field_type: field.field_type.clone(),
        })
        .collect();
    match compat_engine::schema_compatible(&expected, &actual) {
        Ok(()) => None,
        Err(error) => Some(error.to_string()),
    }
}
```

- [ ] **Step 3: 运行确认通过 + 冒烟**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml
```
Expected: 全量绿（旧 schema 测试在 schema_registry.rs 内部不动）。冒烟：

```bash
curl -s -X POST http://127.0.0.1:3001/api/schema/check -H 'Content-Type: application/json' -d '{"source_urn":"std/core/v1/UInt8","sink_urn":"std/core/v1/UInt32"}' | python3 -m json.tool
curl -s -X POST http://127.0.0.1:3001/api/schema/check -H 'Content-Type: application/json' -d '{"source_operator":"camera_driver","source_port":"image","sink_operator":"object_detection","sink_port":"image"}' | python3 -m json.tool
```
Expected: 第一个 compatible=true level=compatible；第二个走旧路径 level=compatible（registry 判定）。

- [ ] **Step 4: Commit（先问学生）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && git add backend/src/main.rs backend/src/models.rs && git commit -m "feat(api): dual-shape schema check with URN compatibility engine"
```

### Task 3.4: dataflow_builder 升级（dora 1.0 生成格式 + type_rules + path）

**Files:**
- Modify: `backend/src/dataflow_builder.rs`
- Test: `backend/src/dataflow_builder.rs` tests 模块

- [ ] **Step 1: 写失败测试**

```rust
    #[test]
    fn yaml_includes_nodes_header_and_types() {
        let b = make_test_graph();
        let yaml = b.to_yaml();
        assert!(yaml.starts_with("nodes:\n"), "dora 1.0 requires nodes: header");
        // old `type:` under ports is migrated to node-level maps
        assert!(yaml.contains("output_types:\n      image: image"));
        assert!(!yaml.contains("        type: image"));
    }

    #[test]
    fn type_rules_roundtrip_in_yaml() {
        let mut b = make_test_graph();
        b.type_rules
            .push(("std/core/v1/UInt8".into(), "std/core/v1/String".into()));
        let yaml = b.to_yaml();
        assert!(yaml.contains("type_rules:"));
        assert!(yaml.contains("  - from: std/core/v1/UInt8"));
        assert!(yaml.contains("    to: std/core/v1/String"));
        let parsed = DataflowBuilder::from_yaml(&yaml).expect("roundtrip");
        assert_eq!(
            parsed.type_rules,
            vec![("std/core/v1/UInt8".to_string(), "std/core/v1/String".to_string())]
        );
    }

    #[test]
    fn path_field_roundtrips() {
        let mut b = DataflowBuilder::new();
        b.add_node(NodeSpec {
            id: "a".into(),
            operator_id: "a".into(),
            runtime: Runtime::Python,
            path: Some("cam.py".into()),
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            input_types: BTreeMap::new(),
            output_types: BTreeMap::new(),
            position: None,
        })
        .unwrap();
        let yaml = b.to_yaml();
        assert!(yaml.contains("    path: cam.py"));
        let parsed = DataflowBuilder::from_yaml(&yaml).expect("roundtrip");
        assert_eq!(parsed.graph().nodes[0].path.as_deref(), Some("cam.py"));
    }
```

- [ ] **Step 2: 运行确认失败**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml yaml_includes_nodes_header type_rules_roundtrip path_field_roundtrips
```
Expected: 编译错误（字段不存在）。

- [ ] **Step 3: 最小实现**

1. `NodeSpec`（`backend/src/dataflow_builder.rs:42-52`）扩展：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSpec {
    pub id: String,
    pub operator_id: String,
    pub runtime: Runtime,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub inputs: BTreeMap<String, PortSpec>,
    #[serde(default)]
    pub outputs: BTreeMap<String, PortSpec>,
    #[serde(default)]
    pub input_types: BTreeMap<String, String>,
    #[serde(default)]
    pub output_types: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
}
```

2. `DataflowGraph` 加：

```rust
    #[serde(default)]
    pub type_rules: Vec<(String, String)>,
```

（注意：`(String, String)` 的 serde 表示为数组 `[from, to]`——前端发 `[["a","b"]]` 或对象数组均可？不行，serde tuple 固定是数组。为兼容前端对象数组，改为 `Vec<TypeRuleDef>`：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeRuleDef {
    pub from: String,
    pub to: String,
}
// DataflowGraph:
    #[serde(default)]
    pub type_rules: Vec<TypeRuleDef>,
```

Builder 内部 `pub type_rules: Vec<TypeRuleDef>`；测试里 `b.type_rules.push(TypeRuleDef { from: ..., to: ... })`。）

3. `DataflowBuilder` 加字段 `pub type_rules: Vec<TypeRuleDef>`，`new()` 初始化空。

4. `to_yaml` 重写（`backend/src/dataflow_builder.rs:170-203`）：

```rust
    /// Serialize to dora 1.0 dataflow YAML (nodes: header, node-level
    /// input_types/output_types, dataflow-level type_rules).
    pub fn to_yaml(&self) -> String {
        let mut out = String::from("nodes:\n");
        for node in self.nodes.values() {
            out.push_str(&render_node_block(node, &self.edges));
        }
        if !self.type_rules.is_empty() {
            out.push_str("type_rules:\n");
            for rule in &self.type_rules {
                out.push_str(&format!("  - from: {}\n    to: {}\n", rule.from, rule.to));
            }
        }
        out
    }
```

5. 提取 `render_node_block`（`to_yaml` 之后）：

```rust
fn render_node_block(node: &NodeSpec, edges: &BTreeMap<String, EdgeSpec>) -> String {
    let mut out = String::new();
    out.push_str(&format!("  - id: {}\n", node.id));
    if let Some(ref path) = node.path {
        out.push_str(&format!("    path: {}\n", path));
    }
    // REVISION 2026-08-20: dora 1.0 rejects node-level `runtime:` (serde
    // deny_unknown_fields — verified against the real CLI). Do NOT emit it;
    // dora infers the runtime from the path language. Runtime stays
    // canvas-side state only.
    if !node.inputs.is_empty() {
        out.push_str("    inputs:\n");
        for (name, _) in &node.inputs {
            out.push_str(&format!("      {}:\n", name));
            out.push_str(&format!(
                "        source: {}\n",
                find_edge_source(edges, &node.id, name)
            ));
        }
    }
    let input_types: BTreeMap<&String, &String> = node
        .input_types
        .iter()
        .chain(
            node.inputs
                .iter()
                .filter_map(|(name, port)| port.port_type.as_ref().map(|t| (name, t))),
        )
        .collect();
    if !input_types.is_empty() {
        out.push_str("    input_types:\n");
        for (name, urn) in &input_types {
            out.push_str(&format!("      {name}: {urn}\n"));
        }
    }
    if !node.outputs.is_empty() {
        out.push_str("    outputs:\n");
        for name in node.outputs.keys() {
            out.push_str(&format!("      - {}\n", name));
        }
    }
    let output_types: BTreeMap<&String, &String> = node
        .output_types
        .iter()
        .chain(
            node.outputs
                .iter()
                .filter_map(|(name, port)| port.port_type.as_ref().map(|t| (name, t))),
        )
        .collect();
    if !output_types.is_empty() {
        out.push_str("    output_types:\n");
        for (name, urn) in &output_types {
            out.push_str(&format!("      {name}: {urn}\n"));
        }
    }
    out.push('\n');
    out
}
```

（注意：input_types 合并时若同一端口在 `input_types` 与 `PortSpec.port_type` 都有值，`BTreeMap` 链式 collect 后者覆盖前者——PortSpec.port_type 是画布权威值，符合预期。）

6. `from_yaml` 扩展（`backend/src/dataflow_builder.rs:207` 起）：
   - 节点字段解析处（`indent == 4` 分支）加：

```rust
                        } else if let Some(path) = inner_trimmed.strip_prefix("path:") {
                            node_path = Some(path.trim().to_string());
                        } else if inner_trimmed == "input_types:" {
                            i += 1;
                            while i < lines.len() {
                                let t_line = lines[i];
                                let t_trimmed = t_line.trim_start();
                                let t_indent = t_line.len() - t_trimmed.len();
                                if t_indent <= 4 {
                                    break;
                                }
                                if t_indent == 6 {
                                    if let Some((name, urn)) = t_trimmed.split_once(':') {
                                        input_types.insert(
                                            name.trim().to_string(),
                                            urn.trim().to_string(),
                                        );
                                    }
                                }
                                i += 1;
                            }
                            continue;
                        } else if inner_trimmed == "output_types:" {
                            i += 1;
                            while i < lines.len() {
                                let t_line = lines[i];
                                let t_trimmed = t_line.trim_start();
                                let t_indent = t_line.len() - t_trimmed.len();
                                if t_indent <= 4 {
                                    break;
                                }
                                if t_indent == 6 {
                                    if let Some((name, urn)) = t_trimmed.split_once(':') {
                                        output_types.insert(
                                            name.trim().to_string(),
                                            urn.trim().to_string(),
                                        );
                                    }
                                }
                                i += 1;
                            }
                            continue;
```

   - 变量声明处补 `let mut node_path: Option<String> = None; let mut input_types = BTreeMap::new(); let mut output_types = BTreeMap::new();`；`builder.add_node(NodeSpec { ... })` 调用补 `path: node_path, input_types, output_types,`。
   - 类型回填：add_node 之后，把 `input_types`/`output_types` 合并进节点 PortSpec.port_type 以保证画布往返一致——实现为在 `from_yaml` 末尾对 builder.nodes 做一遍 merge（遍历 builder.nodes，对每个 port 若 types map 有值且 PortSpec.port_type 为 None 则填入）。
   - 顶层 type_rules 解析：在 `from_yaml` 开头（找 "nodes:" 之前）加：

```rust
        let type_rules = parse_type_rules_from_yaml(yaml);
        // ... 在 Ok(builder) 前: builder.type_rules = type_rules;
```

   ```rust
fn parse_type_rules_from_yaml(yaml: &str) -> Vec<TypeRuleDef> {
    let mut rules = Vec::new();
    let mut in_rules = false;
    let mut current_from: Option<String> = None;
    for raw_line in yaml.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = raw_line.chars().take_while(|ch| ch.is_whitespace()).count();
        if !in_rules {
            if trimmed == "type_rules:" && indent == 0 {
                in_rules = true;
            }
            continue;
        }
        if indent == 0 && trimmed != "type_rules:" {
            break;
        }
        if let Some(from) = trimmed.strip_prefix("- from:") {
            current_from = Some(from.trim().to_string());
        } else if let Some(to) = trimmed.strip_prefix("to:") {
            if let Some(from) = current_from.take() {
                rules.push(TypeRuleDef { from, to: to.trim().to_string() });
            }
        }
    }
    rules
}
```

- [ ] **Step 4: 运行确认通过**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml yaml_includes_nodes_header type_rules_roundtrip path_field_roundtrips && cargo test --manifest-path backend/Cargo.toml
```
Expected: 新测试 PASS；全量绿（旧测试 `builds_valid_yaml` 仍断言 contains 子串，兼容）。

- [ ] **Step 5: 端到端冒烟（dora validate 真实校验生成格式）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml -- --nocapture builds_valid_yaml
```
再手动：用 `/api/dataflow/build` 生成（见 Task 3.4 之后的冒烟）→ 存文件 → `/home/dora/.venvs/dora-studio-1.0/bin/dora validate <file>` 应 exit 0（若节点只有 operator 无 path，validate 会报 operator 格式错——这是预期已知限制，Build 标签新建节点的 path 由 Task 4 的调色板提供；本条冒烟使用带 path 的图）。

- [ ] **Step 6: Commit（先问学生）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && git add backend/src/dataflow_builder.rs && git commit -m "feat(builder): emit dora 1.0 dataflow format with types and type_rules"
```

### Task 3.5: 行级 diff patch（patch_yaml）

**Files:**
- Modify: `backend/src/dataflow_builder.rs`
- Test: `backend/src/dataflow_builder.rs` tests 模块

- [ ] **Step 1: 写失败测试**

```rust
    #[test]
    fn patch_preserves_comments_and_unknown_fields() {
        let original = r#"# my project dataflow
nodes:
  - id: cam
    path: cam.py
    outputs:
      - image
    output_types:
      image: std/media/v1/Image
  - id: sink
    path: sink.py
    inputs:
      image: cam/image
env:
  RUST_LOG: info
"#;
        let mut b = DataflowBuilder::from_yaml(original).expect("parses");
        // edit: change sink input type declaration
        b.nodes.get_mut("sink").unwrap().input_types.insert(
            "image".to_string(),
            "std/core/v1/Bytes".to_string(),
        );
        // add a node
        b.add_node(NodeSpec {
            id: "proc".into(),
            operator_id: "proc".into(),
            runtime: Runtime::Python,
            path: Some("proc.py".into()),
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            input_types: BTreeMap::new(),
            output_types: BTreeMap::new(),
            position: None,
        })
        .unwrap();
        // remove cam
        b.remove_node("cam").unwrap();
        let patched = patch_yaml(original, &b.graph()).expect("patches");
        assert!(patched.contains("# my project dataflow"), "comments preserved");
        assert!(patched.contains("env:\n  RUST_LOG: info"), "unknown sections preserved");
        assert!(patched.contains("input_types:\n      image: std/core/v1/Bytes"));
        assert!(patched.contains("  - id: proc"), "new node inserted");
        assert!(!patched.contains("  - id: cam"), "removed node gone");
        assert!(!patched.contains("cam/image"), "stale source references gone");
    }

    #[test]
    fn patch_adds_type_rules_section() {
        let original = "nodes:\n  - id: a\n    path: a.py\n    outputs:\n      - out\n";
        let mut b = DataflowBuilder::from_yaml(original).expect("parses");
        b.type_rules.push(TypeRuleDef { from: "x/v1/A".into(), to: "x/v1/B".into() });
        let patched = patch_yaml(original, &b.graph()).expect("patches");
        assert!(patched.contains("type_rules:\n  - from: x/v1/A\n    to: x/v1/B\n"));
    }

    #[test]
    fn patch_migrates_legacy_inline_type_lines() {
        let original = r#"nodes:
  - id: cam
    path: cam.py
    outputs:
      - image
        type: image
"#;
        let mut b = DataflowBuilder::from_yaml(original).expect("parses");
        b.nodes.get_mut("cam").unwrap().output_types.insert(
            "image".to_string(),
            "std/media/v1/Image".to_string(),
        );
        let patched = patch_yaml(original, &b.graph()).expect("patches");
        assert!(!patched.contains("type: image"));
        assert!(patched.contains("output_types:\n      image: std/media/v1/Image"));
    }
```

- [ ] **Step 2: 运行确认失败**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml patch_preserves patch_adds_type_rules patch_migrates
```
Expected: 编译错误（patch_yaml 不存在）。

- [ ] **Step 3: 最小实现**（dataflow_builder.rs 追加；注意 from_yaml 解析 `      - image` 后的 `        type: image` 行——现有解析器输出分支只读 `- name`，type 行会被忽略。patch 的重渲染基于画布权威数据，天然完成迁移。）

```rust
// ---------------------------------------------------------------------------
// Line-based diff patch (M18): rewrite only the regions the canvas owns —
// node blocks and the type_rules section — preserving everything else.
// ---------------------------------------------------------------------------

/// Apply an edited graph onto an existing dataflow YAML, preserving all
/// lines outside node blocks and the type_rules section verbatim.
pub fn patch_yaml(original: &str, graph: &DataflowGraph) -> Result<String, BuildError> {
    let lines: Vec<String> = original.lines().map(str::to_string).collect();

    // 1. Locate the nodes: section and each node block.
    let blocks = find_node_blocks(&lines);
    if blocks.is_empty() && !graph.nodes.is_empty() && !lines.iter().any(|l| l.trim() == "nodes:") {
        // No nodes section at all: fall back to full generation of the
        // canvas-owned regions appended after any existing content.
        let mut out = lines.join("\n");
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("nodes:\n");
        for node in &graph.nodes {
            out.push_str(&render_node_block(node, &edge_map(graph)));
        }
        append_type_rules(&mut out, &graph.type_rules);
        return Ok(out);
    }

    // 2. Replace existing blocks and collect insertion points.
    let mut result: Vec<String> = Vec::new();
    let mut cursor = 0usize;
    let mut inserted_ids: std::collections::BTreeSet<String> = graph
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect();

    // node blocks in source order
    let mut ordered_blocks: Vec<(usize, usize, String)> = Vec::new();
    for (id, (start, end)) in &blocks {
        ordered_blocks.push((*start, *end, id.clone()));
    }
    ordered_blocks.sort();

    // index graph nodes by id
    let mut graph_nodes: BTreeMap<&str, &NodeSpec> =
        graph.nodes.iter().map(|node| (node.id.as_str(), node)).collect();

    let mut insert_after: Option<usize> = None; // line index of last node block end

    for (start, end, id) in &ordered_blocks {
        result.extend(lines[cursor..*start].iter().cloned());
        if let Some(node) = graph_nodes.remove(id.as_str()) {
            result.extend(render_node_block(node, &edge_map(graph)).lines().map(str::to_string));
            inserted_ids.remove(id);
        }
        // removed nodes: emit nothing
        cursor = *end + 1;
        insert_after = Some(result.len());
    }
    result.extend(lines[cursor..].iter().cloned());

    // 3. Insert new nodes after the last existing node block (before any
    // trailing top-level sections).
    let new_nodes: Vec<&NodeSpec> = graph
        .nodes
        .iter()
        .filter(|node| inserted_ids.contains(&node.id))
        .collect();
    if !new_nodes.is_empty() {
        let mut rendered = String::new();
        for node in new_nodes {
            rendered.push_str(&render_node_block(node, &edge_map(graph)));
        }
        let insert_at = insert_after
            .map(|index| {
                // find the end of that node's rendered lines in `result`
                // (block content may differ in length, so re-locate by
                // scanning forward to the next top-level line)
                let mut index = index;
                while index < result.len() {
                    let line = &result[index];
                    if !line.trim().is_empty() && !line.starts_with(' ') {
                        break;
                    }
                    index += 1;
                }
                index
            })
            .unwrap_or_else(|| {
                // no existing blocks: insert right after the nodes: line
                result
                    .iter()
                    .position(|line| line.trim() == "nodes:")
                    .map(|index| index + 1)
                    .unwrap_or(0)
            });
        let rendered_lines: Vec<String> = rendered.lines().map(str::to_string).collect();
        for (offset, line) in rendered_lines.into_iter().enumerate() {
            result.insert(insert_at + offset, line);
        }
    }

    // 4. Patch the type_rules top-level section.
    patch_type_rules_section(&mut result, &graph.type_rules);

    Ok(result.join("\n") + "\n")
}

fn edge_map(graph: &DataflowGraph) -> BTreeMap<String, EdgeSpec> {
    graph.edges.iter().map(|edge| (edge.id.clone(), edge.clone())).collect()
}

fn find_node_blocks(lines: &[String]) -> BTreeMap<String, (usize, usize)> {
    let mut blocks = BTreeMap::new();
    let mut in_nodes = false;
    let mut current: Option<(String, usize)> = None;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed == "nodes:" {
            in_nodes = true;
            continue;
        }
        if in_nodes {
            let indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
            if indent == 0 && !trimmed.is_empty() {
                in_nodes = false;
                if let Some((id, start)) = current.take() {
                    blocks.insert(id, (start, index - 1));
                }
                continue;
            }
            if let Some(id) = trimmed.strip_prefix("- id:") {
                if let Some((prev_id, start)) = current.take() {
                    blocks.insert(prev_id, (start, index - 1));
                }
                current = Some((id.trim().to_string(), index));
            }
        }
    }
    if let Some((id, start)) = current.take() {
        blocks.insert(id, (start, lines.len() - 1));
    }
    blocks
}

fn patch_type_rules_section(result: &mut Vec<String>, rules: &[TypeRuleDef]) {
    // remove existing section
    let mut start: Option<usize> = None;
    let mut end: Option<usize> = None;
    for (index, line) in result.iter().enumerate() {
        let trimmed = line.trim();
        let indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
        if trimmed == "type_rules:" && indent == 0 {
            start = Some(index);
            continue;
        }
        if start.is_some() && indent == 0 && !trimmed.is_empty() {
            end = Some(index);
            break;
        }
    }
    if let Some(start) = start {
        let end = end.unwrap_or(result.len());
        result.drain(start..end);
    }
    if !rules.is_empty() {
        let mut section = vec!["type_rules:".to_string()];
        for rule in rules {
            section.push(format!("  - from: {}", rule.from));
            section.push(format!("    to: {}", rule.to));
        }
        let insert_at = result
            .iter()
            .rposition(|line| !line.trim().is_empty() && !line.starts_with(' '))
            .map(|index| index + 1)
            .unwrap_or(result.len());
        for (offset, line) in section.into_iter().enumerate() {
            result.insert(insert_at + offset, line);
        }
    }
}

fn append_type_rules(out: &mut String, rules: &[TypeRuleDef]) {
    if !rules.is_empty() {
        out.push_str("type_rules:\n");
        for rule in rules {
            out.push_str(&format!("  - from: {}\n    to: {}\n", rule.from, rule.to));
        }
    }
}
```

- [ ] **Step 4: 运行确认通过**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml patch_
```
Expected: 3 个新测试 PASS。若 `patch_preserves_comments_and_unknown_fields` 的 env 保留断言失败，检查 `find_node_blocks` 对 `env:`（indent 0）的块结束处理（`index - 1` 边界）并修正。

- [ ] **Step 5: Commit（先问学生）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && git add backend/src/dataflow_builder.rs && git commit -m "feat(builder): line-based diff patch preserving unknown YAML content"
```

### Task 3.6: validate.rs + 保存端点

**Files:**
- Create: `backend/src/validate.rs`
- Modify: `backend/src/main.rs`（routes + save/save-as handlers）、`backend/src/models.rs`（SaveRequest/SaveResponse）
- Test: `backend/src/validate.rs` tests 模块（纯函数部分）

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_type_mismatch_warning_to_issue() {
        let text = r#"type mismatch on input "image": upstream cam/frame declares "std/media/v1/Image", but expected "std/media/v1/CompressedImage""#;
        let issues = map_validate_output(text, false);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].port_id.as_deref(), Some("image"));
        assert!(issues[0].message.contains("std/media/v1/Image"));
    }

    #[test]
    fn maps_wiring_error_to_issue() {
        let text = r#"node `detector`: input `frame` references `cam/frame` but node `cam` has no such output"#;
        let issues = map_validate_output(text, true);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].node_id.as_deref(), Some("detector"));
        assert_eq!(issues[0].port_id.as_deref(), Some("frame"));
    }

    #[test]
    fn ignores_unmappable_lines() {
        let issues = map_validate_output("some unrelated stderr noise\nanother line\n", true);
        assert!(issues.is_empty());
    }
}
```

- [ ] **Step 2: 运行确认失败**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml maps_type_mismatch maps_wiring ignores_unmappable
```
Expected: 编译错误。

- [ ] **Step 3: 最小实现**（完整文件 `backend/src/validate.rs`）

```rust
//! dora validate terminal check for dataflow saves (M18).
//!
//! Only runs on dora 1.x (0.5 lacks typed validation semantics). Errors
//! block the save; warnings are mapped back to canvas edges.

use crate::models::SaveIssue;
use std::path::Path;
use std::process::Stdio;

pub struct ValidateOutcome {
    pub errors: Vec<SaveIssue>,
    pub warnings: Vec<SaveIssue>,
}

/// Run `dora validate <path>` with a 10s timeout. Returns Ok(outcome) on
/// dora 1.x, Err(reason) when validation is unavailable (0.x or spawn
/// failure) — callers surface that as a non-blocking notice.
pub async fn validate_yaml(path: &Path) -> Result<ValidateOutcome, String> {
    if !crate::dora_env::lifecycle_supported() {
        return Err("dora 0.x does not support typed validation; final check skipped.".into());
    }
    let bin = crate::dora_env::resolve_dora_bin();
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::process::Command::new(&bin)
            .arg("validate")
            .arg(path)
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .map_err(|_| "dora validate timed out after 10s".to_string())?
    .map_err(|error| format!("failed to spawn dora validate: {error}"))?;

    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let success = output.status.success();

    Ok(ValidateOutcome {
        errors: if success { Vec::new() } else { map_validate_output(&text, true) },
        warnings: map_validate_output(&text, false),
    })
}

/// Extract node/port-addressable issues from `dora validate` output.
/// `strict` selects error patterns; otherwise type-mismatch warnings.
pub fn map_validate_output(text: &str, strict: bool) -> Vec<SaveIssue> {
    let mut issues = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let is_warning = trimmed.contains("type mismatch on input");
        if strict == is_warning {
            continue;
        }
        if !is_warning && !(trimmed.contains("references") || trimmed.contains("no such") || trimmed.contains("does not exist")) {
            continue;
        }
        // REVISION 2026-08-20: 真实 dora 1.0 wiring 错误格式为
        // `output `cam/nonexistent` mapped to input `detector/frame` does not exist`
        // （反引号 + "does not exist"，无 node `...` 标记）——strict 关键字集
        // 必须含 "does not exist"，非 warning 行用反引号提取 node/port。
        let node_id = extract_backtick_field(trimmed, "node `");
        let port_id = extract_quoted_field(trimmed, "input \"");
        issues.push(SaveIssue {
            node_id,
            port_id,
            message: trimmed.to_string(),
        });
    }
    issues
}

fn extract_backtick_field(line: &str, prefix: &str) -> Option<String> {
    line.find(prefix)
        .and_then(|start| {
            let rest = &line[start + prefix.len()..];
            rest.find('`').map(|end| rest[..end].to_string())
        })
}

fn extract_quoted_field(line: &str, prefix: &str) -> Option<String> {
    line.find(prefix)
        .and_then(|start| {
            let rest = &line[start + prefix.len()..];
            rest.find('"').map(|end| rest[..end].to_string())
        })
}
```

（`lifecycle_supported` 的实际签名以 dora_env.rs 为准——若返回 `bool` 直接用；若不存在，用 `dora_version()` 解析主版本 ≥1 的等价判断，并在 dora_env.rs 加 pub(crate) 辅助。）

models.rs 加：

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveIssue {
    pub node_id: Option<String>,
    pub port_id: Option<String>,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResponse {
    pub ok: bool,
    pub path: String,
    #[serde(default)]
    pub warnings: Vec<SaveIssue>,
    #[serde(default)]
    pub errors: Vec<SaveIssue>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRequest {
    pub graph: dataflow_builder::DataflowGraph,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAsRequest {
    pub graph: dataflow_builder::DataflowGraph,
    pub target_path: String,
}
```

- [ ] **Step 4: 运行确认通过**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml maps_type_mismatch maps_wiring ignores_unmappable
```
Expected: 3 个测试 PASS。

- [ ] **Step 5: 保存 handlers**（main.rs，`dataflow_parse` 之后）

```rust
fn backup_file(path: &Path) -> Result<PathBuf, String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let backups = Path::new(&home).join(".config/dora-studio/backups");
    std::fs::create_dir_all(&backups).map_err(|error| format!("backup dir: {error}"))?;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "dataflow.yml".to_string());
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let backup = backups.join(format!("{name}.{ts}.bak"));
    std::fs::copy(path, &backup).map_err(|error| format!("backup: {error}"))?;
    Ok(backup)
}

async fn finish_save(
    target: &Path,
    content: &str,
    display_path: &str,
) -> Result<Json<models::SaveResponse>, ApiError> {
    let tmp = std::env::temp_dir().join(format!("dora-studio-save-{}.yml", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, content).map_err(|error| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to write temp file: {error}"),
    })?;
    let outcome = match validate::validate_yaml(&tmp).await {
        Ok(outcome) => outcome,
        Err(skipped) => validate::ValidateOutcome {
            errors: Vec::new(),
            warnings: vec![models::SaveIssue {
                node_id: None,
                port_id: None,
                message: skipped,
            }],
        },
    };
    let _ = std::fs::remove_file(&tmp);
    if !outcome.errors.is_empty() {
        return Err(ApiError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: serde_json::to_string(&models::SaveResponse {
                ok: false,
                path: display_path.to_string(),
                warnings: outcome.warnings,
                errors: outcome.errors,
            })
            .unwrap_or_else(|_| "validate failed".to_string()),
        });
    }
    std::fs::write(target, content).map_err(|error| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to write {display_path}: {error}"),
    })?;
    Ok(Json(models::SaveResponse {
        ok: true,
        path: display_path.to_string(),
        warnings: outcome.warnings,
        errors: Vec::new(),
    }))
}

async fn dataflow_save(
    Path(id): Path<String>,
    Json(req): Json<models::SaveRequest>,
) -> Result<Json<models::SaveResponse>, ApiError> {
    let file = dataflows::resolve_dataflow(&id).map_err(|error| ApiError {
        status: StatusCode::NOT_FOUND,
        message: format!("Failed to resolve dataflow '{id}': {error}"),
    })?;
    let original = std::fs::read_to_string(&file.path).map_err(|error| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("Failed to read {}: {error}", file.relative_path),
    })?;
    let patched = dataflow_builder::patch_yaml(&original, &req.graph).map_err(|error| ApiError {
        status: StatusCode::UNPROCESSABLE_ENTITY,
        message: format!("Failed to build YAML: {error}"),
    })?;
    backup_file(&file.path).map_err(|error| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: error,
    })?;
    finish_save(&file.path, &patched, &file.relative_path).await
}

async fn dataflow_save_as(
    Json(req): Json<models::SaveAsRequest>,
) -> Result<Json<models::SaveResponse>, ApiError> {
    let target = PathBuf::from(req.target_path.trim());
    if target.as_os_str().is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "missing 'targetPath' field".to_string(),
        });
    }
    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|error| ApiError {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                message: format!("cannot create target directory: {error}"),
            })?;
        }
    }
    let mut builder = dataflow_builder::DataflowBuilder::new();
    builder.type_rules = req.graph.type_rules.clone();
    for node in req.graph.nodes {
        builder.add_node(node).map_err(|error| ApiError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: error.to_string(),
        })?;
    }
    for edge in req.graph.edges {
        builder.connect(edge).map_err(|error| ApiError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: error.to_string(),
        })?;
    }
    let yaml = builder.to_yaml();
    let display = target.to_string_lossy().to_string();
    finish_save(&target, &yaml, &display).await
}
```

routes:

```rust
        .route("/api/dataflows/:id/save", post(dataflow_save))
        .route("/api/dataflows/save-as", post(dataflow_save_as))
```

- [ ] **Step 6: 运行确认通过 + 冒烟**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml
```
Expected: 全量绿。冒烟（后端运行中，且 venv PATH 前置使 validate 走 1.0）：

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b
curl -s -X POST http://127.0.0.1:3001/api/dataflows/save-as -H 'Content-Type: application/json' -d '{"targetPath":"/tmp/m18-smoke/dataflow.yml","graph":{"nodes":[{"id":"sensor","operatorId":"sensor","runtime":"python","path":"sensor.py","inputs":{},"outputs":{"reading":{"type":"std/core/v1/Float64"}},"inputTypes":{},"outputTypes":{},"position":{"x":80,"y":80}}],"edges":[],"typeRules":[]}}'
```
Expected: ok:true；`/home/dora/.venvs/dora-studio-1.0/bin/dora validate /tmp/m18-smoke/dataflow.yml` exit 0（路径不存在只出 warning 或 error？dora validate 的 wiring 检查不查路径存在性（check_wiring 只查端口），静态检查不查 path 存在——若报 path 不存在错误，把 graph 的 path 换成真实存在的脚本路径再验证）。保存含不兼容连线的图再故意破坏一条 wiring（输入引用不存在的输出）→ 422 且 errors 非空。

- [ ] **Step 7: Commit（先问学生）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && git add backend/src/validate.rs backend/src/main.rs backend/src/models.rs && git commit -m "feat(save): write-back and save-as endpoints with dora validate terminal check"
```

### Task 3.7: fixture 对照 + 暂停验收 1

- [ ] **Step 1: 对照脚本**（本地文件 `out/` 下，不提交）

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b
mkdir -p out/m18-parity
cat > out/m18-parity/case1-widening.yml <<'EOF'
nodes:
  - id: sender
    outputs:
      - data
    output_types:
      data: std/core/v1/UInt8
  - id: recv
    inputs:
      data: sender/data
    input_types:
      data: std/core/v1/UInt32
EOF
cat > out/m18-parity/case2-mismatch.yml <<'EOF'
nodes:
  - id: sender
    outputs:
      - data
    output_types:
      data: std/media/v1/Image
  - id: recv
    inputs:
      data: sender/data
    input_types:
      data: std/core/v1/Float64
EOF
cat > out/m18-parity/case3-rule.yml <<'EOF'
type_rules:
  - from: std/core/v1/UInt8
    to: std/core/v1/String
nodes:
  - id: sender
    outputs:
      - data
    output_types:
      data: std/core/v1/UInt8
  - id: recv
    inputs:
      data: sender/data
    input_types:
      data: std/core/v1/String
EOF
for f in out/m18-parity/*.yml; do
  echo "=== $f ==="
  /home/dora/.venvs/dora-studio-1.0/bin/dora validate "$f" 2>&1 | grep -E "type mismatch|error|Error" || echo "(no type issues)"
done
```

Expected（与 compat_engine 单测判定一致，记录到 `out/m18-parity/RESULTS.md`）：
- case1: 无 type mismatch（拓宽兼容）— Studio: green
- case2: 有 type mismatch warning — Studio: red
- case3: 无 type mismatch（用户规则生效）— Studio: yellow
任一不一致：以 dora 为准修正 compat_engine 并更新单测。

- [ ] **Step 2: 基线 + 暂停**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml && cargo fmt --manifest-path backend/Cargo.toml --check && git diff --check
```
Expected: 全绿。**然后暂停，请学生人工验收**（验收点：curl 冒烟 projects/types/schema/check/save 五组；对照结果文件）。

---

## Task 4: D4 画布真实化（前端主体）

前端约定：tools 测试（tsx/node 无 DOM）只测纯 TS 模块；新增测试文件必须加入 `package.json` 的 `test:tools` 脚本；样式只用主题 token；正文 ≥12px；i18n 中英（中文优先）。

### Task 4.1: api.ts 新端点与类型

**Files:**
- Modify: `frontend/src/api.ts`
- Test: `npm run build` 类型检查

- [ ] **Step 1: 追加代码**（`frontend/src/api.ts` 末尾，`// --- M18 Dataflow Explorer 2.0 ---`）

```ts
// --- M18 Dataflow Explorer 2.0 ---

export type TypeField = { name: string; fieldType: string }
export type TypeParam = { name: string; default?: string }
export type TypeCatalogEntry = {
  urn: string; name: string; category: string; arrow: string;
  description?: string; fields: TypeField[]; params: TypeParam[]
}

export type ProjectSummaryResponse = {
  name: string; path: string; builtin: boolean; dataflowCount: number
  dataflows: DataflowSummaryResponse[]
}

export type PalettePort = { name: string; urn?: string }
export type PaletteEntry = {
  id: string; operator: string; path?: string; runtime: string
  project: string; manual: boolean; inputs: PalettePort[]; outputs: PalettePort[]
}

export type ManualNodeSpec = {
  id: string; path: string; description?: string
  inputs: PalettePort[]; outputs: PalettePort[]
}

export type TypeRule = { from: string; to: string }

export type SchemaCheckUrnRequest = {
  source_urn?: string; sink_urn?: string; type_rules?: TypeRule[]
}
export type SchemaCheckResponse = {
  compatible: boolean; level: string; detail: string
  urn?: string; rule?: TypeRule | null; suggestion?: string
}

export type SaveIssue = { nodeId?: string; portId?: string; message: string }
export type SaveResponse = { ok: boolean; path: string; warnings: SaveIssue[]; errors: SaveIssue[] }

export function getProjects() {
  return fetchJson<{ projects: ProjectSummaryResponse[] }>('/projects/list')
}
export function addProjectDir(path: string) {
  return fetchJson<{ ok: boolean }>('/projects/add', { method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ path }) })
}
export function deleteProjectDir(path: string) {
  return fetchJson<{ ok: boolean }>('/projects/delete', { method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ path }) })
}
export function submitManualNode(node: ManualNodeSpec) {
  return fetchJson<{ ok: boolean }>('/projects/nodes', { method: 'POST', headers: JSON_HEADER, body: JSON.stringify(node) })
}
export function getTypeCatalog() {
  return fetchJson<{ types: TypeCatalogEntry[] }>('/types/catalog')
}
export function checkSchemaUrn(req: SchemaCheckUrnRequest) {
  return fetchJson<SchemaCheckResponse>('/schema/check', { method: 'POST', headers: JSON_HEADER, body: JSON.stringify(req) })
}
export function saveDataflow(id: string, graph: unknown) {
  return fetchJson<SaveResponse>(`/dataflows/${encodeURIComponent(id)}/save`, { method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ graph }) })
}
export function saveDataflowAs(graph: unknown, targetPath: string) {
  return fetchJson<SaveResponse>('/dataflows/save-as', { method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ graph, targetPath }) })
}
```

（`checkSchema` 旧函数与 `SchemaCheckRequest` 保留不动；`DataflowDefinitionNodeResponse` 增加可选 `inputTypes?: Record<string,string>`、`outputTypes?: Record<string,string>`、`path?: string`，与后端 `DataflowDefinitionNode` 新增字段对应——后端 `definition_node()` 同步输出这些字段（Task 4.2 Step 3 一并改）。）

- [ ] **Step 2: 运行确认通过**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && npm --prefix frontend run build
```
Expected: 构建通过。

### Task 4.2: 纯函数模块（dataflow-convert.ts / dataflow-palette.ts / edge-status.ts）+ tools 测试

**Files:**
- Create: `frontend/src/dataflow-convert.ts`、`frontend/src/dataflow-palette.ts`、`frontend/src/edge-status.ts`
- Create: `frontend/src/dataflow-convert.test.ts`、`frontend/src/dataflow-palette.test.ts`、`frontend/src/edge-status.test.ts`
- Modify: `frontend/package.json`（test:tools 加 3 个文件）
- Modify: `frontend/src/components/DataflowCanvas.vue:6-10`（NodeSpec 接口加 `path?: string`）
- Modify: `backend/src/dataflows.rs`（definition_node 输出 input_types/output_types/path）、`backend/src/models.rs`（DataflowDefinitionNode 加字段）

- [ ] **Step 1: 写失败测试**（`frontend/src/dataflow-convert.test.ts`）

```ts
import { definitionToGraph, graphToPayload } from './dataflow-convert'
import type { DataflowDefinitionResponse } from './api'
import type { DataflowGraph } from './components/DataflowCanvas.vue'

const def: DataflowDefinitionResponse = {
  id: 'abc', name: 'demo', relativePath: 'p/dataflow.yml', source: '',
  nodeCount: 2, edgeCount: 1, project: 'p',
  nodes: [
    {
      id: 'cam', path: 'cam.py',
      inputs: [],
      outputs: ['image'],
      inputTypes: {},
      outputTypes: { image: 'std/media/v1/Image' },
    },
    {
      id: 'sink', path: 'sink.py',
      inputs: ['image: cam/image'],
      outputs: [],
      inputTypes: { image: 'std/media/v1/Image' },
      outputTypes: {},
    },
  ],
}

function assertDefined<T>(value: T | undefined): T {
  if (value === undefined) throw new Error('expected defined')
  return value
}

const graph = definitionToGraph(def)
const cam = assertDefined(graph.nodes.find(n => n.id === 'cam'))
const sink = assertDefined(graph.nodes.find(n => n.id === 'sink'))
if (cam.outputs.image?.type !== 'std/media/v1/Image') throw new Error('output URN lost')
if (sink.inputs.image?.type !== 'std/media/v1/Image') throw new Error('input URN lost')
if (cam.path !== 'cam.py') throw new Error('path lost')
if (graph.edges.length !== 1) throw new Error('edge not derived from source')
if (graph.edges[0].sourceNode !== 'cam' || graph.edges[0].targetPort !== 'image') throw new Error('edge endpoints wrong')

const payload = graphToPayload(graph)
if (payload.nodes.length !== 2 || payload.edges.length !== 1) throw new Error('payload roundtrip failed')
if (payload.nodes.find(n => n.id === 'cam')?.output_types?.image !== 'std/media/v1/Image') throw new Error('output_types not emitted')
if (payload.type_rules.length !== 0) throw new Error('type_rules default empty')

const typed = { ...def, nodes: def.nodes.map((n, i) => ({ ...n, id: i === 0 ? 'cam' : 'sink' })) }
const g2 = definitionToGraph(typed)
if (g2.nodes.length !== 2) throw new Error('reconvert failed')

console.log('dataflow-convert tests passed')
```

`frontend/src/dataflow-palette.test.ts`:

```ts
import { aggregatePalette } from './dataflow-palette'
import type { PaletteEntry } from './api'

const entries: PaletteEntry[] = [
  { id: 'cam', operator: 'cam.py', path: 'cam.py', runtime: 'python', project: 'a', manual: false, inputs: [], outputs: [{ name: 'image', urn: 'std/media/v1/Image' }] },
  { id: 'cam2', operator: 'cam.py', path: 'cam.py', runtime: 'python', project: 'b', manual: false, inputs: [], outputs: [{ name: 'image' }] },
  { id: 'conv', operator: 'conv', path: '/tmp/conv.py', runtime: 'python', project: 'manual', manual: true, inputs: [{ name: 'in', urn: 'std/media/v1/Image' }], outputs: [] },
]

const grouped = aggregatePalette(entries)
if (grouped.length !== 2) throw new Error('expected 2 deduped entries, got ' + grouped.length)
const cam = grouped.find(g => g.operator === 'cam.py')
if (!cam || cam.outputs[0].urn !== 'std/media/v1/Image') throw new Error('richest entry must win')
const manualGroup = grouped.find(g => g.operator === 'conv')
if (!manualGroup?.manual) throw new Error('manual flag lost')

const byProject = aggregatePalette(entries, { groupBy: 'project' })
if (!byProject.find(g => g.operator === 'cam.py' && g.project === 'a')) throw new Error('project grouping failed')

console.log('dataflow-palette tests passed')
```

`frontend/src/edge-status.test.ts`:

```ts
import { edgeLevel, edgeColor, buildRulePatch } from './edge-status'
import type { SchemaCheckResponse, TypeRule } from './api'

function resp(level: string, rule: TypeRule | null = null): SchemaCheckResponse {
  return { compatible: level !== 'incompatible' && level !== 'unknown', level, detail: '', rule }
}

if (edgeLevel(resp('compatible')) !== 'green') throw new Error('compatible should be green')
if (edgeLevel(resp('rule')) !== 'yellow') throw new Error('rule should be yellow')
if (edgeLevel(resp('warning')) !== 'yellow') throw new Error('legacy warning should be yellow')
if (edgeLevel(resp('incompatible')) !== 'red') throw new Error('incompatible should be red')
if (edgeLevel(resp('unknown')) !== 'gray') throw new Error('unknown should be gray')

if (edgeColor('green') !== 'var(--accent-green)') throw new Error('green token wrong')
if (edgeColor('yellow') !== 'var(--accent-yellow)') throw new Error('yellow token wrong')
if (edgeColor('red') !== 'var(--accent-red)') throw new Error('red token wrong')
if (edgeColor('gray') !== 'var(--text-muted-dark)') throw new Error('gray token wrong')

const existing: TypeRule[] = [{ from: 'a/v1/T', to: 'b/v1/U' }]
const patched = buildRulePatch(existing, 'a/v1/T', 'b/v1/U')
if (patched.length !== 1) throw new Error('duplicate rule not deduped')
const added = buildRulePatch(existing, 'c/v1/X', 'd/v1/Y')
if (added.length !== 2) throw new Error('new rule not appended')
if (added[1].from !== 'c/v1/X') throw new Error('appended rule wrong')

console.log('edge-status tests passed')
```

- [ ] **Step 2: 运行确认失败**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && npm --prefix frontend run test:tools
```
Expected: tsx 报模块不存在。

- [ ] **Step 3: 最小实现**

`frontend/src/components/DataflowCanvas.vue` 的 NodeSpec 接口（第 6-10 行）加 `path?: string`：

```ts
export interface NodeSpec {
  id: string; operatorId: string; runtime: string; path?: string;
  inputs: Record<string, PortSpec>; outputs: Record<string, PortSpec>;
  position: { x: number; y: number };
}
```

`frontend/src/dataflow-convert.ts`:

```ts
import type { DataflowDefinitionResponse, TypeRule } from './api'
import type { DataflowGraph, NodeSpec, EdgeSpec } from './components/DataflowCanvas.vue'

function portSpec(urn?: string) {
  return urn ? { type: urn } : {}
}

/** Convert a backend dataflow definition into the canvas graph model. */
export function definitionToGraph(def: DataflowDefinitionResponse): DataflowGraph {
  const nodes: NodeSpec[] = def.nodes.map((node, index) => {
    const inputs: Record<string, { type?: string }> = {}
    for (const entry of node.inputs) {
      const [name] = entry.split(': ')
      inputs[name] = portSpec(node.inputTypes?.[name])
    }
    const outputs: Record<string, { type?: string }> = {}
    for (const name of node.outputs) {
      outputs[name] = portSpec(node.outputTypes?.[name])
    }
    return {
      id: node.id,
      operatorId: node.path ?? node.id,
      runtime: runtimeForPath(node.path),
      path: node.path,
      inputs,
      outputs,
      position: { x: 80 + index * 240, y: 80 + (index % 3) * 180 },
    }
  })
  const edges: EdgeSpec[] = []
  let edgeIndex = 0
  for (const node of def.nodes) {
    for (const entry of node.inputs) {
      const [name, source] = entry.split(': ')
      const [from, output] = source.split('/')
      if (nodes.some(n => n.id === from)) {
        edgeIndex += 1
        edges.push({
          id: `e${edgeIndex}`,
          sourceNode: from,
          sourcePort: output ?? 'output',
          targetNode: node.id,
          targetPort: name,
        })
      }
    }
  }
  return { nodes, edges }
}

function runtimeForPath(path?: string): string {
  const ext = path?.split('.').pop()
  return { py: 'python', rs: 'rust', cpp: 'c++', cc: 'c++', cxx: 'c++', c: 'c' }[ext ?? ''] ?? 'python'
}

export type BuilderGraphPayload = {
  nodes: Array<{
    id: string; operator_id: string; runtime: string; path?: string
    inputs: Record<string, { type?: string }>; outputs: Record<string, { type?: string }>
    input_types: Record<string, string>; output_types: Record<string, string>
    position: { x: number; y: number }
  }>
  edges: Array<{ id: string; source_node: string; source_port: string; target_node: string; target_port: string }>
  type_rules: TypeRule[]
}

/** Convert the canvas graph into the backend save/build payload. */
export function graphToPayload(graph: DataflowGraph, typeRules: TypeRule[] = []): BuilderGraphPayload {
  return {
    nodes: graph.nodes.map(node => ({
      id: node.id,
      operator_id: node.operatorId,
      runtime: node.runtime,
      path: node.path,
      inputs: node.inputs,
      outputs: node.outputs,
      input_types: Object.fromEntries(
        Object.entries(node.inputs).filter(([, port]) => port.type).map(([name, port]) => [name, port.type as string])
      ),
      output_types: Object.fromEntries(
        Object.entries(node.outputs).filter(([, port]) => port.type).map(([name, port]) => [name, port.type as string])
      ),
      position: node.position,
    })),
    edges: graph.edges.map(edge => ({
      id: edge.id,
      source_node: edge.sourceNode,
      source_port: edge.sourcePort,
      target_node: edge.targetNode,
      target_port: edge.targetPort,
    })),
    type_rules: typeRules,
  }
}
```

`frontend/src/dataflow-palette.ts`:

```ts
import type { PaletteEntry } from './api'

/** Dedupe palette entries by path (rich entry wins) and optionally split
 *  per project. Manual nodes are never deduped against scanned ones. */
export function aggregatePalette(
  entries: PaletteEntry[],
  options: { groupBy?: 'project' | 'none' } = {}
): PaletteEntry[] {
  const groupBy = options.groupBy ?? 'none'
  const buckets = new Map<string, PaletteEntry>()
  const order: string[] = []
  for (const entry of entries) {
    const key = entry.manual
      ? `manual:${entry.id}`
      : groupBy === 'project'
        ? `${entry.project}:${entry.path ?? entry.operator}`
        : `scan:${entry.path ?? entry.operator}`
    const existing = buckets.get(key)
    if (!existing) {
      buckets.set(key, entry)
      order.push(key)
    } else {
      buckets.set(key, richer(existing, entry))
    }
  }
  return order.map(key => buckets.get(key) as PaletteEntry)
}

function richer(a: PaletteEntry, b: PaletteEntry): PaletteEntry {
  const aTyped = [...a.inputs, ...a.outputs].filter(p => p.urn).length
  const bTyped = [...b.inputs, ...b.outputs].filter(p => p.urn).length
  if (bTyped > aTyped) return b
  if (bTyped < aTyped) return a
  return a.inputs.length + a.outputs.length >= b.inputs.length + b.outputs.length ? a : b
}
```

`frontend/src/edge-status.ts`:

```ts
import type { SchemaCheckResponse, TypeRule } from './api'

export type EdgeLevel = 'green' | 'yellow' | 'red' | 'gray'

/** Map a schema check response to the four-color edge semantics. */
export function edgeLevel(response: SchemaCheckResponse): EdgeLevel {
  switch (response.level) {
    case 'compatible': return 'green'
    case 'rule':
    case 'warning': return 'yellow'
    case 'incompatible': return 'red'
    default: return 'gray'
  }
}

export function edgeColor(level: EdgeLevel): string {
  return {
    green: 'var(--accent-green)',
    yellow: 'var(--accent-yellow)',
    red: 'var(--accent-red)',
    gray: 'var(--text-muted-dark)',
  }[level]
}

/** Add a from→to rule to the dataflow rules, deduplicating. */
export function buildRulePatch(existing: TypeRule[], from: string, to: string): TypeRule[] {
  if (existing.some(rule => rule.from === from && rule.to === to)) return existing
  return [...existing, { from, to }]
}
```

`backend/src/dataflows.rs` 的 `definition_node` 与 `backend/src/models.rs` 的 `DataflowDefinitionNode` 扩展：

```rust
// models.rs
pub struct DataflowDefinitionNode {
    pub id: String,
    pub path: Option<String>,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default)]
    pub input_types: BTreeMap<String, String>,
    #[serde(default)]
    pub output_types: BTreeMap<String, String>,
}
// dataflows.rs definition_node:
fn definition_node(node: ParsedNode) -> DataflowDefinitionNode {
    DataflowDefinitionNode {
        id: node.id,
        path: node.path,
        inputs: node.inputs.into_iter().map(|(name, source)| format!("{name}: {source}")).collect(),
        outputs: node.outputs,
        input_types: node.input_types,
        output_types: node.output_types,
    }
}
```

（`DataflowDefinition` 同步加 `#[serde(default)] pub project: String`，`load_definition` 里填 `project_name_for(file)`；`list_dataflows` 的 Summary 构造处补 `project: "Studio Examples".to_string()`。）

`frontend/package.json` 的 test:tools 改为：

```json
"test:tools": "tsx src/tools/tests.ts && tsx src/playback.test.ts && tsx src/live-feed.test.ts && tsx src/live-command.test.ts && tsx src/dataflow-convert.test.ts && tsx src/dataflow-palette.test.ts && tsx src/edge-status.test.ts"
```

（`DataflowDefinitionNodeResponse`/`DataflowDefinitionResponse` 在 api.ts 加 `inputTypes?`/`outputTypes?`/`path?`/`project?` 可选字段，见 Task 4.1。）

- [ ] **Step 4: 运行确认通过**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && npm --prefix frontend run test:tools && npm --prefix frontend run build && cargo test --manifest-path backend/Cargo.toml
```
Expected: 3 个新 tools 测试输出 "tests passed"；build 通过；cargo 全量绿。

- [ ] **Step 5: Commit（先问学生）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && git add frontend/src/dataflow-convert.ts frontend/src/dataflow-convert.test.ts frontend/src/dataflow-palette.ts frontend/src/dataflow-palette.test.ts frontend/src/edge-status.ts frontend/src/edge-status.test.ts frontend/package.json backend/src/dataflows.rs backend/src/models.rs && git commit -m "feat(convert): pure canvas conversion, palette aggregation, edge status modules with tools tests"
```

### Task 4.3: NodePalette 接扫描 + URN 选择器 + 手动添加

**Files:**
- Create: `frontend/src/components/PortTypePanel.vue`（节点端口类型编辑面板）
- Modify: `frontend/src/components/NodePalette.vue`（props 驱动 + 手动添加按钮）

- [ ] **Step 1: NodePalette.vue 改造**（完整替换 script + 模板关键块）

```vue
<script setup lang="ts">
import { ref, computed } from 'vue'
import type { PaletteEntry } from '../api'

defineProps<{ entries: PaletteEntry[] }>()
const emit = defineEmits<{
  'drag-start': [entry: PaletteEntry]
  'add-manual': []
}>()

const search = ref('')

const filtered = computed(() => {
  const q = search.value.toLowerCase()
  if (!q) return entriesByProject()
  return entriesByProject().filter(entry =>
    entry.operator.toLowerCase().includes(q) ||
    entry.inputs.some(port => port.name.includes(q)) ||
    entry.outputs.some(port => port.name.includes(q)))
})

function entriesByProject(): PaletteEntry[] {
  // keep props order; manual entries flagged in UI
  return []
}
</script>
```

（实现要点：`defineProps` 收到 `entries: PaletteEntry[]`（来自 `/api/projects` 汇总或 `aggregatePalette`）；模板对每个 entry 渲染拖拽项，dragstart payload 为完整 PaletteEntry（含 path 与端口 URN）；manual 项显示「manual」徽章；底部「+ Add node manually」按钮 emit `add-manual`；样式沿用现有 palette 类，仅新增 `.palette-item-manual` 徽章样式（用 `--accent-yellow` token）。原硬编码 categories 删除。）

- [ ] **Step 2: PortTypePanel.vue**（完整文件）

```vue
<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { getTypeCatalog, type TypeCatalogEntry } from '../api'
import type { NodeSpec } from './DataflowCanvas.vue'

const props = defineProps<{ node: NodeSpec | null }>()
const emit = defineEmits<{ 'update-port': [portName: string, isInput: boolean, urn: string] }>()

const catalog = ref<TypeCatalogEntry[]>([])
const search = ref('')
onMounted(async () => {
  try {
    const result = await getTypeCatalog()
    catalog.value = result.types
  } catch {
    catalog.value = []
  }
})

const grouped = computed(() => {
  const q = search.value.toLowerCase()
  const groups: Record<string, TypeCatalogEntry[]> = {}
  for (const entry of catalog.value) {
    if (q && !(entry.urn.toLowerCase().includes(q) || entry.name.toLowerCase().includes(q))) continue
    ;(groups[entry.category] ??= []).push(entry)
  }
  return groups
})

function pick(urn: string, portName: string, isInput: boolean) {
  emit('update-port', portName, isInput, urn)
}
</script>

<template>
  <div v-if="props.node" class="port-type-panel">
    <div class="ptp-header">Port types — {{ props.node.id }}</div>
    <div class="ptp-section" v-if="Object.keys(props.node.inputs).length">
      <div class="ptp-label">Inputs</div>
      <div v-for="(port, name) in props.node.inputs" :key="name" class="ptp-port">
        <span class="ptp-name">{{ name }}</span>
        <input class="ptp-search" v-model="search" :placeholder="port.type ?? 'Select type URN...'" />
        <select
          v-if="grouped"
          class="ptp-select"
          :value="port.type ?? ''"
          @change="pick(($event.target as HTMLSelectElement).value, name, true)"
        >
          <option value="">{{ port.type ? 'Clear type' : 'Select type...' }}</option>
          <optgroup v-for="(entries, category) in grouped" :key="category" :label="category">
            <option v-for="entry in entries" :key="entry.urn" :value="entry.urn">{{ entry.name }} — {{ entry.urn }}</option>
          </optgroup>
        </select>
      </div>
    </div>
    <div class="ptp-section" v-if="Object.keys(props.node.outputs).length">
      <div class="ptp-label">Outputs</div>
      <div v-for="(port, name) in props.node.outputs" :key="name" class="ptp-port">
        <span class="ptp-name">{{ name }}</span>
        <select class="ptp-select" :value="port.type ?? ''" @change="pick(($event.target as HTMLSelectElement).value, name, false)">
          <option value="">{{ port.type ? 'Clear type' : 'Select type...' }}</option>
          <optgroup v-for="(entries, category) in grouped" :key="category" :label="category">
            <option v-for="entry in entries" :key="entry.urn" :value="entry.urn">{{ entry.name }} — {{ entry.urn }}</option>
          </optgroup>
        </select>
      </div>
    </div>
  </div>
</template>

<style scoped>
.port-type-panel { padding: 12px; border-left: 1px solid var(--hairline); background: var(--panel-surface); overflow-y: auto; }
.ptp-header { font-size: 12px; font-weight: 600; color: var(--text-heading); margin-bottom: 8px; }
.ptp-label { font-size: 12px; color: var(--text-muted-dark); margin: 8px 0 4px; }
.ptp-port { display: flex; flex-direction: column; gap: 4px; margin-bottom: 8px; }
.ptp-name { font-size: 12px; color: var(--text-body); }
.ptp-search { padding: 6px 8px; font-size: 12px; background: var(--card-surface); border: 1px solid var(--hairline); border-radius: 6px; color: var(--text-body); }
.ptp-select { padding: 6px 8px; font-size: 12px; background: var(--card-surface); border: 1px solid var(--hairline); border-radius: 6px; color: var(--text-body); }
</style>
```

- [ ] **Step 3: 运行确认通过**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && npm --prefix frontend run build
```
Expected: vue-tsc 通过。

### Task 4.4: DataflowExplorer Source 双向编辑 + 保存流程 + 连线判定

**Files:**
- Modify: `frontend/src/components/DataflowExplorer.vue`（主体改造）

- [ ] **Step 1: script 改造要点（完整代码逐函数落地）**

1. 状态新增：

```ts
const projects = ref<ProjectSummaryResponse[]>([])
const paletteEntries = ref<PaletteEntry[]>([])
const typeRules = ref<TypeRule[]>([])
const editingDefinition = ref<DataflowDefinitionResponse | null>(null)  // 当前 Source 编辑目标
const sourceSubView = ref<'canvas' | 'text'>('canvas')
const saveStatus = ref('')
const saveWarnings = ref<SaveIssue[]>([])
const typeCatalog = ref<TypeCatalogEntry[]>([])
const portPanelNode = computed(() => buildGraph.value.nodes.find(n => n.id === selectedBuildNode.value) ?? null)
```

2. 加载（替换 `loadDataflow`）：

```ts
async function loadDataflow(id: string) {
  selectedDataflowId.value = id
  const result = await getDataflowDefinition(id, emptyDefinition)
  definition.value = result.source === 'connected' ? result.data : null
  if (definition.value) {
    editingDefinition.value = definition.value
    buildGraph.value = definitionToGraph(definition.value)
    typeRules.value = definition.value.typeRules ?? []
    sourceSubView.value = 'canvas'
    await checkAllEdges()
  }
}
```

（`DataflowDefinitionResponse` 增加 `typeRules?: TypeRule[]`；后端 `DataflowDefinition` 增加 `#[serde(default)] pub type_rules: Vec<TypeRuleDef>`（models.rs），`load_definition` 从 parsed 填入。）

3. `checkAllEdges` 重写（URN 优先，旧形状回退）：

```ts
async function checkAllEdges() {
  schemaChecking.value = true
  const styles: Record<string, { color: string; tooltip: string }> = {}
  for (const edge of buildGraph.value.edges) {
    const srcNode = buildGraph.value.nodes.find(n => n.id === edge.sourceNode)
    const tgtNode = buildGraph.value.nodes.find(n => n.id === edge.targetNode)
    if (!srcNode || !tgtNode) continue
    const srcUrn = srcNode.outputs[edge.sourcePort]?.type
    const tgtUrn = tgtNode.inputs[edge.targetPort]?.type
    try {
      const resp = srcUrn || tgtUrn
        ? await checkSchemaUrn({ source_urn: srcUrn, sink_urn: tgtUrn, type_rules: typeRules.value })
        : await checkSchema({
            source_operator: srcNode.operatorId, source_port: edge.sourcePort,
            sink_operator: tgtNode.operatorId, sink_port: edge.targetPort,
          })
      const level = edgeLevel(resp)
      styles[edge.id] = { color: edgeColor(level), tooltip: resp.detail + (resp.suggestion ? ` — ${resp.suggestion}` : '') }
    } catch {
      styles[edge.id] = { color: 'var(--text-muted-dark)', tooltip: 'Schema check unavailable' }
    }
  }
  edgeStyles.value = styles
  schemaChecking.value = false
}
```

4. 保存（新增函数）：

```ts
async function saveCurrent() {
  if (!editingDefinition.value) return
  saveStatus.value = 'Saving...'
  try {
    const result = await saveDataflow(editingDefinition.value.id, graphToPayload(buildGraph.value, typeRules.value))
    if (!result.ok) {
      saveStatus.value = `Save blocked: ${result.errors.length} error(s)`
      applySaveIssues(result.errors, true)
    } else {
      saveStatus.value = `Saved to ${result.path}${result.warnings.length ? ` (${result.warnings.length} warning(s))` : ''}`
      applySaveIssues(result.warnings, false)
      await loadDataflow(editingDefinition.value.id)
    }
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e)
    saveStatus.value = `Save failed: ${message}`
    try {
      const parsed = JSON.parse(message.replace(/^.*?\{/, '{')) as SaveResponse
      if (!parsed.ok) applySaveIssues(parsed.errors, true)
    } catch { /* plain error message */ }
  }
}

function applySaveIssues(issues: SaveIssue[], blocking: boolean) {
  const styles: Record<string, { color: string; tooltip: string }> = {}
  for (const issue of issues) {
    for (const edge of buildGraph.value.edges) {
      const matchesPort = (issue.portId && (edge.sourcePort === issue.portId || edge.targetPort === issue.portId)) ||
        (!issue.portId && issue.nodeId && (edge.sourceNode === issue.nodeId || edge.targetNode === issue.nodeId))
      if (matchesPort) {
        styles[edge.id] = { color: blocking ? 'var(--accent-red)' : 'var(--accent-yellow)', tooltip: issue.message }
      }
    }
  }
  edgeStyles.value = { ...edgeStyles.value, ...styles }
}
```

5. 另存为：

```ts
async function saveAsCurrent() {
  const target = window.prompt('Save dataflow as (absolute path, e.g. /home/me/proj/dataflow.yml):')
  if (!target) return
  saveStatus.value = 'Saving...'
  try {
    const result = await saveDataflowAs(graphToPayload(buildGraph.value, typeRules.value), target)
    saveStatus.value = result.ok ? `Saved to ${result.path}` : `Save blocked: ${result.errors.length} error(s)`
  } catch (e) {
    saveStatus.value = e instanceof Error ? `Save failed: ${e.message}` : 'Save failed'
  }
}
```

6. URN 更新（PortTypePanel 事件）：

```ts
function onPortUpdate(portName: string, isInput: boolean, urn: string) {
  if (!selectedBuildNode.value) return
  const node = buildGraph.value.nodes.find(n => n.id === selectedBuildNode.value)
  if (!node) return
  const ports = isInput ? node.inputs : node.outputs
  if (ports[portName]) {
    if (urn) ports[portName] = { type: urn }
    else delete ports[portName].type
  }
  buildChecked.value = false
  checkAllEdges()
}
```

7. 模板改动：
   - 左侧 sidebar：按 `projects` 分组渲染（内置在前），每组 header + flow 按钮（沿用现有 `.flow-file` 样式）；顶部「+ Add project directory」按钮（prompt 输入路径 → `addProjectDir` → 刷新 projects）。
   - 主区 Source 标签：顶部子切换 `canvas | text`；canvas 时渲染 Build 布局复用段（DataflowCanvas + NodePalette + PortTypePanel + TypeRulesPanel），工具条加 Save / Save As 按钮与 saveStatus 显示；text 时保留原 `<pre>` 查看器。
   - 定义加载失败（definition null）显示只读提示「This YAML could not be parsed — canvas editing disabled」（诚实边界）。

- [ ] **Step 2: TypeRulesPanel.vue**（完整文件，保存 type_rules 管理）

```vue
<script setup lang="ts">
import type { TypeRule } from '../api'

const props = defineProps<{ rules: TypeRule[] }>()
const emit = defineEmits<{ 'update:rules': [rules: TypeRule[]]; 'remove-rule': [index: number] }>()

function remove(index: number) {
  const next = props.rules.filter((_, i) => i !== index)
  emit('update:rules', next)
}
</script>

<template>
  <div class="type-rules-panel">
    <div class="trp-header">Type Rules ({{ props.rules.length }})</div>
    <div v-if="!props.rules.length" class="trp-empty">No declared type rules.</div>
    <div v-for="(rule, index) in props.rules" :key="`${rule.from}-${rule.to}`" class="trp-rule">
      <span class="trp-rule-text">{{ rule.from }} → {{ rule.to }}</span>
      <button class="trp-remove" title="Remove rule" @click="remove(index)">✕</button>
    </div>
  </div>
</template>

<style scoped>
.type-rules-panel { padding: 12px; border-left: 1px solid var(--hairline); background: var(--panel-surface); overflow-y: auto; }
.trp-header { font-size: 12px; font-weight: 600; color: var(--text-heading); margin-bottom: 8px; }
.trp-empty { font-size: 12px; color: var(--text-muted-dark); }
.trp-rule { display: flex; justify-content: space-between; align-items: center; gap: 8px; padding: 6px 8px; margin-bottom: 4px; background: var(--card-surface); border: 1px solid var(--hairline); border-radius: 6px; }
.trp-rule-text { font-size: 12px; color: var(--text-body); }
.trp-remove { background: none; border: none; color: var(--text-muted-dark); cursor: pointer; font-size: 12px; }
.trp-remove:hover { color: var(--accent-red); }
</style>
```

（删除前提示：`remove` 内先 `window.confirm('This rule may affect multiple connections. Remove it?')`，取消则不 emit。）

- [ ] **Step 3: 连线属性面板**（选中边时显示判定原因 + 创建规则按钮）

在 DataflowExplorer 模板选中边区域（`selectedBuildEdge` 非空时）渲染：

```vue
      <div v-if="selectedBuildEdge && edgeStyles[selectedBuildEdge]" class="edge-props">
        <div class="edge-props-title">Connection</div>
        <div class="edge-props-reason">{{ edgeStyles[selectedBuildEdge]?.tooltip ?? 'No check result yet.' }}</div>
        <button
          v-if="edgeStyles[selectedBuildEdge]?.color === 'var(--accent-red)'"
          class="edge-props-rule"
          @click="createRuleForSelectedEdge"
        >Declare type rule for this connection</button>
      </div>
```

```ts
function createRuleForSelectedEdge() {
  const edge = buildGraph.value.edges.find(e => e.id === selectedBuildEdge.value)
  if (!edge) return
  const srcNode = buildGraph.value.nodes.find(n => n.id === edge.sourceNode)
  const tgtNode = buildGraph.value.nodes.find(n => n.id === edge.targetNode)
  const from = srcNode?.outputs[edge.sourcePort]?.type
  const to = tgtNode?.inputs[edge.targetPort]?.type
  if (!from || !to) return
  typeRules.value = buildRulePatch(typeRules.value, from, to)
  checkAllEdges()
}
```

- [ ] **Step 4: 运行确认通过**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && npm --prefix frontend run test:tools && npm --prefix frontend run build
```
Expected: tools 测试全绿（含 3 个新模块）；vue-tsc 通过。

- [ ] **Step 5: 浏览器手测**（后端 + 前端 dev server 起后）

1. 打开 Dataflow Explorer → Source：左侧按项目分组；点击 `robot-perception-test` → 画布显示节点/端口/URN。
2. 改某端口类型（PortTypePanel）→ 对应连线变色（绿/黄/红/灰）。
3. 红连线 → 点「Declare type rule」→ 变黄且 TypeRulesPanel 出现该规则。
4. Save → 后端写回（文件有 .bak 备份）；打开文本子视图确认注释保留。
5. 故意把某输入 source 改成不存在的端口 → Save 被 422 拦截，画布对应连线标红。

- [ ] **Step 6: Commit（先问学生）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && git add frontend/src/components/DataflowExplorer.vue frontend/src/components/NodePalette.vue frontend/src/components/PortTypePanel.vue frontend/src/components/TypeRulesPanel.vue frontend/src/api.ts && git commit -m "feat(explorer): two-way canvas editing, live URN edge checking, save with validate mapping"
```

### Task 4.5: D4 收尾验证 + 暂停验收 2

- [ ] **Step 1: 基线**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && cargo test --manifest-path backend/Cargo.toml && npm --prefix frontend run test:tools && npm --prefix frontend run build && cargo fmt --manifest-path backend/Cargo.toml --check && git diff --check
```
Expected: 全绿。

- [ ] **Step 2: 暂停，请学生人工验收**（计划验收清单 1-6 条中前端相关项）。

---

## Task 5: D5 收尾 + 总验收

### Task 5.1: 项目分组 UI 细节 + 空状态 + 手动添加节点表单

**Files:**
- Modify: `frontend/src/components/DataflowExplorer.vue`

- [ ] **Step 1: 空状态与分组**：projects 为空时（仅内置组）sidebar 显示内置组；`projectDirs` 目录不存在时该组显示「(directory missing)」灰态；「+ Add project directory」用 prompt 输入并调用 `addProjectDir`，失败弹 `alert` 显示后端错误；Source 文本子视图空态「No source available.」已有。
- [ ] **Step 2: 手动添加节点表单**：NodePalette 的 `add-manual` 事件在 DataflowExplorer 弹出 prompt 序列（id → path → 端口列表，格式 `name=urn,name=urn`）→ 组装 `ManualNodeSpec` → `submitManualNode` → 刷新调色板；校验失败 alert。
- [ ] **Step 3: 验证**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && npm --prefix frontend run build && npm --prefix frontend run test:tools
```

- [ ] **Step 4: Commit（先问学生）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && git add frontend/src/components/DataflowExplorer.vue frontend/src/components/NodePalette.vue && git commit -m "feat(explorer): project groups, empty states, manual node submission"
```

### Task 5.2: i18n 中英

**Files:**
- Modify: `frontend/src/i18n.ts`、`frontend/src/components/DataflowExplorer.vue`（新 UI 字符串接入 t()）

- [ ] **Step 1: i18n.ts 新增键**（沿用现有 t() 结构与 zh/en 两栏；示例）：

```ts
// zh
'explorer.projects': '项目',
'explorer.addProjectDir': '添加项目目录',
'explorer.projectMissing': '（目录不存在）',
'explorer.canvas': '画布',
'explorer.text': '原始文本',
'explorer.save': '保存',
'explorer.saveAs': '另存为',
'explorer.saved': '已保存到 {path}',
'explorer.saveBlocked': '保存被阻止：{count} 个错误',
'explorer.declareRule': '为此连线声明 type rule',
'explorer.typeRules': '类型规则',
'explorer.noRules': '未声明类型规则',
'explorer.removeRuleConfirm': '该规则可能影响多条连线，确认删除？',
'explorer.portTypes': '端口类型',
'explorer.selectType': '选择类型...',
'explorer.clearType': '清除类型',
'explorer.unparseable': '该 YAML 无法解析，画布编辑已禁用',
'explorer.addManual': '手动添加节点',
// en
'explorer.projects': 'Projects',
'explorer.addProjectDir': 'Add project directory',
'explorer.projectMissing': '(directory missing)',
'explorer.canvas': 'Canvas',
'explorer.text': 'Raw text',
'explorer.save': 'Save',
'explorer.saveAs': 'Save As',
'explorer.saved': 'Saved to {path}',
'explorer.saveBlocked': 'Save blocked: {count} error(s)',
'explorer.declareRule': 'Declare type rule for this connection',
'explorer.typeRules': 'Type Rules',
'explorer.noRules': 'No declared type rules.',
'explorer.removeRuleConfirm': 'This rule may affect multiple connections. Remove it?',
'explorer.portTypes': 'Port types',
'explorer.selectType': 'Select type...',
'explorer.clearType': 'Clear type',
'explorer.unparseable': 'This YAML could not be parsed — canvas editing disabled',
'explorer.addManual': 'Add node manually',
```

- [ ] **Step 2: 替换硬编码字符串**：DataflowExplorer/NodePalette/PortTypePanel/TypeRulesPanel 中新 UI 文本全部走 t()；中文优先默认。
- [ ] **Step 3: 验证**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && npm --prefix frontend run build && npm --prefix frontend run test:tools
```

- [ ] **Step 4: Commit（先问学生）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && git add frontend/src/i18n.ts frontend/src/components/DataflowExplorer.vue frontend/src/components/NodePalette.vue frontend/src/components/PortTypePanel.vue frontend/src/components/TypeRulesPanel.vue && git commit -m "feat(i18n): bilingual explorer 2.0 strings"
```

### Task 5.3: README 项目目录配置说明

**Files:**
- Modify: `README.md`

- [ ] **Step 1: 追加章节**（英文）：

```markdown
## Project Directories (Dataflow Explorer 2.0)

The Dataflow Explorer scans the built-in `examples/` directory plus any
project directories you configure. Add a directory from the Explorer's
Source tab ("Add project directory"); it is persisted in
`~/.config/dora-studio/settings.json` under `projectDirs`.

Nodes discovered in your dataflow YAML files populate the node palette,
including port type URNs (`input_types` / `output_types`). Edge
compatibility follows dora 1.0 semantics (widening, `type_rules`,
structural struct checks) and saving runs `dora validate` as a final
check. Write-back edits only touch node blocks and `type_rules`; all
other content (comments, `env`, etc.) is preserved, and a backup is
written to `~/.config/dora-studio/backups/` before each write-back.

Manual nodes (nodes without a dataflow YAML) can be submitted from the
palette; they are stored in `manualNodes` in the same settings file and
are marked as manually declared in the UI.
```

- [ ] **Step 2: Commit（先问学生）**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b && git add README.md && git commit -m "docs: document project directories and write-back behavior"
```

### Task 5.4: 真实节点验收项目搭建（/home/dora/m18-acceptance）

**决策（2026-08-19 与学生确认）**：最终验收使用真实节点。摄像头 = 新写 cv2 真实 USB 相机节点（uv 装 opencv-python 到 venv）；机械臂 = SO101 只读模式（leader 读关节，不写运动，硬件不在位时降级跳过 start）；测试项目 = 新建 `/home/dora/m18-acceptance/`（写回/破坏测试全部在副本上进行，绝不碰 dorobot2 原件）。

**Files:**
- Create: `/home/dora/m18-acceptance/dataflows/real-camera-pipeline.yml`、`typed-local.yml`、`arm-readonly.yml`
- Create: `/home/dora/m18-acceptance/nodes/camera_usb.py`、`image_stats.py`、`frame_logger.py`、`sensor_uint8.py`、`processor_uint8.py`、`sink.py`、`joint_logger.py`
- 注：m18-acceptance 是验收环境文件，**不提交**到 git。

- [ ] **Step 1: 安装 opencv-python 到 venv**

```bash
uv pip install --python /home/dora/.venvs/dora-studio-1.0/bin/python opencv-python
/home/dora/.venvs/dora-studio-1.0/bin/python -c "import cv2; print(cv2.__version__)"
```
Expected: 打印 opencv 版本。

- [ ] **Step 2: 创建项目结构与真实节点脚本**

```bash
mkdir -p /home/dora/m18-acceptance/dataflows /home/dora/m18-acceptance/nodes
```

`/home/dora/m18-acceptance/dataflows/real-camera-pipeline.yml`：

```yaml
# Real camera acceptance dataflow (m18-acceptance).
# Comments and unknown fields must survive canvas edits.
env:
  RUST_LOG: info
nodes:
  - id: camera_usb
    path: nodes/camera_usb.py
    inputs:
      tick: dora/timer/millis/500
    outputs:
      - image
    output_types:
      image: std/media/v1/Image

  - id: image_stats
    path: nodes/image_stats.py
    inputs:
      image: camera_usb/image
    input_types:
      image: std/media/v1/Image
    outputs:
      - stats
    output_types:
      stats: std/core/v1/String

  - id: frame_logger
    path: nodes/frame_logger.py
    inputs:
      stats: image_stats/stats
    input_types:
      stats: std/core/v1/String
```

`/home/dora/m18-acceptance/nodes/camera_usb.py`：

```python
"""Real USB camera node: captures frames with OpenCV and publishes
std/media/v1/Image (width, height, encoding, data struct)."""
import logging
import os

import cv2
import pyarrow as pa
from dora import Node


def main():
    node = Node()
    device = int(os.environ.get("CAMERA_DEVICE", "0"))
    cap = cv2.VideoCapture(device)
    if not cap.isOpened():
        raise RuntimeError(f"cannot open camera device {device}")

    for event in node:
        if event["type"] == "INPUT" and event["id"] == "tick":
            ok, frame = cap.read()
            if not ok:
                logging.warning("frame read failed")
                continue
            ok, encoded = cv2.imencode(".jpg", frame)
            data = encoded.tobytes() if ok else frame.tobytes()
            height, width = frame.shape[:2]
            struct = pa.StructArray.from_arrays(
                [
                    pa.array([width], type=pa.uint32()),
                    pa.array([height], type=pa.uint32()),
                    pa.array(["jpeg"], type=pa.string()),
                    pa.array([data], type=pa.large_binary()),
                ],
                names=["width", "height", "encoding", "data"],
            )
            node.send_output("image", struct)
        elif event["type"] == "STOP":
            break

    cap.release()


if __name__ == "__main__":
    main()
```

`/home/dora/m18-acceptance/nodes/image_stats.py`：

```python
"""Receives std/media/v1/Image structs and emits frame stats as String."""
import pyarrow as pa
from dora import Node


def main():
    node = Node()
    count = 0
    for event in node:
        if event["type"] == "INPUT" and event["id"] == "image":
            values = event["value"]
            width = values.field("width")[0].as_py()
            height = values.field("height")[0].as_py()
            size = len(values.field("data")[0].as_py())
            count += 1
            node.send_output(
                "stats",
                pa.array([f"frame {count}: {width}x{height} {size}B"]),
            )
        elif event["type"] == "STOP":
            break


if __name__ == "__main__":
    main()
```

`/home/dora/m18-acceptance/nodes/frame_logger.py`：

```python
"""Logs incoming stats strings; prints a heartbeat every 10 frames."""
from dora import Node


def main():
    node = Node()
    count = 0
    for event in node:
        if event["type"] == "INPUT" and event["id"] == "stats":
            message = event["value"].to_pylist()[0]
            count += 1
            if count % 10 == 0:
                print(f"[frame_logger] {count} frames received, last: {message}", flush=True)
        elif event["type"] == "STOP":
            break


if __name__ == "__main__":
    main()
```

`/home/dora/m18-acceptance/dataflows/typed-local.yml`（电脑侧真实节点，改编自 dora typed-dataflow，验证拓宽绿线）：

```yaml
# Computer-side real nodes (adapted from dora typed-dataflow example).
# sensor sends UInt8; processor expects UInt32 -> widening (green).
nodes:
  - id: sensor
    path: nodes/sensor_uint8.py
    outputs:
      - reading
    output_types:
      reading: std/core/v1/UInt8

  - id: processor
    path: nodes/processor_uint8.py
    inputs:
      reading: sensor/reading
    input_types:
      reading: std/core/v1/UInt32
    outputs:
      - result
    output_types:
      result: std/core/v1/String

  - id: sink
    path: nodes/sink.py
    inputs:
      result: processor/result
    input_types:
      result: std/core/v1/String
```

`/home/dora/m18-acceptance/nodes/sensor_uint8.py`：

```python
"""Sensor node emitting UInt8 readings (adapted from dora typed-dataflow)."""
import random
import time

import pyarrow as pa
from dora import Node


def main():
    node = Node()

    for _ in range(30):
        value = random.randint(0, 255)
        node.send_output("reading", pa.array([value], type=pa.uint8()))
        time.sleep(0.1)


if __name__ == "__main__":
    main()
```

`/home/dora/m18-acceptance/nodes/processor_uint8.py`：

```python
"""Doubles the UInt8 reading and emits the result as a String."""
import pyarrow as pa
from dora import Node


def main():
    node = Node()
    for event in node:
        if event["type"] == "INPUT" and event["id"] == "reading":
            value = event["value"].to_pylist()[0]
            node.send_output("result", pa.array([f"doubled: {value * 2}"]))
        elif event["type"] == "STOP":
            break


if __name__ == "__main__":
    main()
```

`/home/dora/m18-acceptance/nodes/sink.py`：

```python
"""Prints incoming result strings."""
from dora import Node


def main():
    node = Node()
    for event in node:
        if event["type"] == "INPUT" and event["id"] == "result":
            print(f"[sink] {event['value'].to_pylist()[0]}", flush=True)
        elif event["type"] == "STOP":
            break


if __name__ == "__main__":
    main()
```

`/home/dora/m18-acceptance/dataflows/arm-readonly.yml`（真实 SO101 臂只读：leader 读关节，不写运动；`operator: {python: ...}` 高级格式同时验证「能解析 nodes 即可编辑、patch 保留未知结构」边界）：

```yaml
# Real SO101 arm read-only dataflow (leader reads joint state, no motion).
nodes:
  - id: arm_reader
    operator:
      python: dorobot.nodes.arm_node:ArmNode
      inputs:
        tick: dora/timer/millis/33
      outputs:
        - all_joint
    env:
      ARM_NAME: leader
      ARM_ROLE: leader
      ARM_DRIVER: so101
      PORT: /dev/ttyACM0
      CALIBRATION_DIR: .calibration/

  - id: joint_logger
    path: nodes/joint_logger.py
    inputs:
      joints: arm_reader/all_joint
    input_types:
      joints: std/control/v1/JointState
```

`/home/dora/m18-acceptance/nodes/joint_logger.py`：

```python
"""Logs joint state arrays from the real arm (read-only role)."""
from dora import Node


def main():
    node = Node()
    count = 0
    for event in node:
        if event["type"] == "INPUT" and event["id"] == "joints":
            values = event["value"].to_pylist()
            count += 1
            if count % 30 == 0:
                print(f"[joint_logger] {count} readings, joints: {values}", flush=True)
        elif event["type"] == "STOP":
            break


if __name__ == "__main__":
    main()
```

- [ ] **Step 3: 冒烟（不经过 Studio，先确认节点本身真实可用）**

```bash
cd /home/dora/m18-acceptance/dataflows
export VIRTUAL_ENV=/home/dora/.venvs/dora-studio-1.0
export PATH="$VIRTUAL_ENV/bin:$PATH"
/home/dora/.venvs/dora-studio-1.0/bin/dora validate real-camera-pipeline.yml && echo CAMERA-YAML-OK
/home/dora/.venvs/dora-studio-1.0/bin/dora validate typed-local.yml && echo TYPED-YAML-OK
/home/dora/.venvs/dora-studio-1.0/bin/dora validate arm-readonly.yml && echo ARM-YAML-OK
```
Expected: 三个 OK（arm-readonly 若因 operator 解析报错，检查 dorobot 是否在 PYTHONPATH 可导入——validate 不导入 python 模块，应无碍）。

- [ ] **Step 4: 将项目加入 Studio 并确认分组/调色板**

```bash
curl -s -X POST http://127.0.0.1:3001/api/projects/add -H 'Content-Type: application/json' -d '{"path":"/home/dora/m18-acceptance"}'
curl -s http://127.0.0.1:3001/api/projects/list | python3 -c "import json,sys; d=json.load(sys.stdin); print([(p['name'], p['dataflowCount']) for p in d['projects']])"
```
Expected: 列表含 `('m18-acceptance', 3)`；`/api/dataflows` 中出现 3 个哈希 id 的新数据流。

### Task 5.5: 基线 + 真实节点人工验收清单 + 暂停总验收

- [ ] **Step 1: 全基线**

```bash
cd /home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b
cargo test --manifest-path backend/Cargo.toml          # 旧 240+1 全绿 + 新测试
npm --prefix frontend run test:tools                   # 旧测试 + 3 个新模块
npm --prefix frontend run build                        # vue-tsc + vite
cargo fmt --manifest-path backend/Cargo.toml --check
git diff --check
```
Expected: 全绿。

- [ ] **Step 2: 清理测试残留**：`dora down`（若测试起过）+ 按端口 kill 遗留后端进程。
- [ ] **Step 3: 真实节点人工验收清单**（逐条与学生走查，全部基于 /home/dora/m18-acceptance 真实节点）：

**验收前准备**：后端 + 前端跑起；venv PATH 前置（daemon 节点需 venv python 的 pyarrow）；相机可用（`ls /dev/video0`）；臂若在位接 /dev/ttyACM0。

1. **真实项目分组 + 真实调色板**：Studio 内「Add project directory」添加 `/home/dora/m18-acceptance` → 左侧出现 m18-acceptance 分组（3 个数据流）；调色板出现 camera_usb/image_stats/sensor/processor/sink/joint_logger 真实节点（端口与 URN 来自扫描，非写死）。
2. **打开真实 YAML 画布编辑 + 写回**：打开 real-camera-pipeline.yml → 画布节点/端口/类型全部来自文件（camera_usb 输出 std/media/v1/Image）；把 frame_logger 的输入类型改为 std/core/v1/String 之外再改回 → Save → 打开原始文本子视图确认：注释（`# Real camera acceptance...`）与 `env: RUST_LOG` 原样保留，input_types 正确写回；`~/.config/dora-studio/backups/` 出现 .bak。
3. **四色判定（真实类型）**：
   - 绿（相同）: real-camera-pipeline 的 camera_usb→image_stats（Image→Image）
   - 绿（拓宽）: typed-local.yml 的 sensor→processor（UInt8→UInt32）
   - 红 + 原因: 画布把 image_stats 输入改成 std/media/v1/CompressedImage → 连线变红、tooltip 含不兼容原因与「插入转换节点」建议 → 改回
   - 黄: 对红连线点「Declare type rule」→ 变黄、Type Rules 面板出现该规则 → 保存 → YAML 写入 type_rules 段 → 删除规则并保存 → type_rules 段移除
4. **validate 终审 + 真实可跑**：画布把 frame_logger 的输入 source 改成 `image_stats/nonexistent` → Save → 被阻止（422）且画布连线标红定位 → 改回 → Save 成功 → Run Monitor（或 Studio start）启动 real-camera-pipeline.yml → `frame_logger` 终端输出每 10 帧心跳且帧计数持续增长（真实 USB 相机数据），`dora start` 该 YAML 真实可跑。
5. **typed-local 真实跑通**：Studio start typed-local.yml → sink 输出 `doubled: N` 30 条（电脑侧真实节点，UInt8 拓宽边在 dora validate 下无 type mismatch 警告）。
6. **fixture 对齐**：out/m18-parity/RESULTS.md 3 用例与 dora validate 行为一致（Task 3.7 产物）。
7. **手动补交节点**：手动提交一个转换节点（如 `image_to_bgr`，声明 Image→Image）→ 调色板出现且带 manual 徽章 → 拖上画布连线正常判定。
8. **臂只读（硬件在位时）**：打开 arm-readonly.yml → 画布正确显示 arm_reader（operator 高级格式）与 joint_logger 及端口类型（joints: std/control/v1/JointState）→ 改 joint_logger 输入类型再改回 → Save 后原始文本确认 operator 块原样保留 → validate 通过 → 启动（PYTHONPATH=/home/dora/dorobot2 前置，臂接 /dev/ttyACM0）→ joint_logger 每 30 次读数输出真实关节值（只读，无运动）。**臂不在位**：跳过 start，只完成编辑/保存/validate（记录降级说明）。
9. **i18n 中英**：切换语言，新增 UI 字符串全部双语。
10. **旧行为兼容**：Studio examples 内置组仍按 slug id 列出；旧形状 schema/check（curl 冒烟）仍返回兼容判定。
11. **README**：项目目录配置说明存在。

- [ ] **Step 4: 验收产物归档**（本地）：`out/m18-acceptance-notes.md` 记录每条验收的通过/降级/截图路径（不提交）。
- [ ] **Step 5: 暂停总验收**。M18 完成后按学生 2026-08-19 决策：不提前准备 PR，等学生指示合并策略。

---

## Self-Review 备注（已内联修正）

- Spec §2-7 四色语义 → Task 3.1 `check()` level 输出 + Task 4.2 `edge-status.ts` 映射。
- Spec §4 保存流程（备份/临时文件/validate/映射）→ Task 3.6。
- Spec §8 验收 1-9 → Task 5.5 真实节点清单逐条对应（2026-08-19 学生要求真实节点验收：真实相机 + 电脑侧真实节点 + SO101 臂只读；测试项目 /home/dora/m18-acceptance，不碰 dorobot2 原件）。
- 计划 M18 原 D1-D5 与本文 Task 1-5 对应；D3 内新增 3.4-3.7（生成格式修复 + patch + 保存）为 PR1 完整后端所必需；Task 5.4（真实节点验收项目搭建）与 Task 5.5（总验收）为 2026-08-19 新增/重写。
- 已知偏差记录：`normalize_field_type` 对「不同 URN 但结构相同」的 struct 判定为不兼容（dora 会深比较相等）——保守方向，fixture 对照（Task 3.7）如遇此情形以注释记录，不改变行为。
- REVISION 2026-08-20（Task 3.4 实施中发现）：dora 1.0 拒绝节点级 `runtime:` 字段（serde deny_unknown_fields，已用真实 CLI 验证）——生成格式不输出 runtime，dora 按 path 语言推断；`builds_valid_yaml` 旧测试的 operator/runtime 子串断言随之更新；顺带修复 `find_yaml_source` 的 indent 匹配 bug（`- id:` 缩进 2 而非 4，往返丢边）。Task 3.5/3.6 复用 render_node_block，不得重新引入 runtime 行。
- 臂只读验收的运行时依赖（dorobot2 的 python 环境、臂在位）在 Task 5.5 第 8 条显式降级路径覆盖。
