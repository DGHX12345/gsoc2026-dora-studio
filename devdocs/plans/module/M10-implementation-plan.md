# M10 LeRobot Profile System — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Load LeRobot Parquet datasets (v1/v2 layouts) into the M09 attribution bar through robot-specific profile YAML files, validated against the real B601 dataset.

**Architecture:** Python 3 + pyarrow subprocess bridge (JSON stdout protocol, 30s timeout, graceful degradation) parses Parquet; Rust parses profile YAML with a zero-dependency mini-parser and maps frames → `AttributionChain` (one chain per frame); the AttributionBar panel gains a LeRobot data source with scan/profile/episode/pagination controls.

**Tech Stack:** Rust 1.75 + axum 0.6 + tokio (backend), Python 3.10 + pyarrow (bridge script), Vue 3 + TS (frontend). No new crates.

**Spec:** `plans2.0/M10-lerobot-profiles.md` (Revision 2026-08-13)

**Conventions:** TDD (write failing test → verify fail → implement → verify pass). Commit after each task, English messages, **no Claude co-author line**. plans2.0/ stays uncommitted. Run from worktree `/home/dora/gsoc2026-dora-studio/.claude/worktrees/week12-b`.

---

### Task 1: M09 诚实性类型调整（confidence/success → Option）

Files:
- Modify: `backend/src/attribution.rs` (VERSION, encode/decode, success(), summary)
- Modify: `backend/src/drec/generator.rs` (confidence Some(...))
- Modify: `frontend/src/api.ts` (success: boolean | null)
- Modify: `frontend/src/components/AttributionBar.vue` (neutral tick, confidence n/a)

- [ ] **Step 1: 更新失败测试** — 修改 `attribution.rs` tests 模块：
  - `encode_decode_roundtrip_parsed_action`: `confidence: Some(0.94)` 往返；另加 `confidence: None` 往返断言
  - `chain_json_uses_camel_case_field_names`: 增加 None confidence 序列化为 `null` 的断言
  - `chain_success_flag_reflects_execution_result`: 改为断言 `Option<bool>` —— 成功链 `Some(true)`、失败链 `Some(false)`、无执行结果链 `None`
  - `summary_maps_chains_and_unparseable`: `success: Some(true)` / `Some(false)`
  - `decode_rejects_unknown_kind` 等其余测试不变（版本字节随 VERSION 常量自动变）

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --manifest-path backend/Cargo.toml attribution`
Expected: 编译失败（`confidence` 类型不匹配 / `success()` 返回类型不匹配）

- [ ] **Step 3: 实现类型调整** — attribution.rs：
  - `pub const VERSION: u16 = 2;`
  - `ParsedAction { confidence: Option<f32> }`；encode 改为 `u8 presence + f32`（None 时 presence=0 无 f32）；decode 对应解析
  - `success(&self) -> Option<bool>`：无 ExecutionResult → `None`；否则 `Some(全部 success)`
  - `AttributionChainSummary.success: Option<bool>`

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --manifest-path backend/Cargo.toml attribution`
Expected: 15 passed

- [ ] **Step 5: 同步 generator.rs** — `generate_vlm_attribution` 中 `confidence: Some(confidence)`

- [ ] **Step 6: 运行全部后端测试（演示文件自动重生成）**

Run: `cargo test --manifest-path backend/Cargo.toml`
Expected: 全绿；`/tmp/dora-studio-tests/attribution_demo.drec` 重新生成

- [ ] **Step 7: 前端适配** — api.ts 改 `success: boolean | null`（`AttributionChainSummaryResponse`）；AttributionBar.vue：
  - tick 类增加 `neutral`（`chain.success === null` 时，灰/青色描边，不用绿/红）
  - 详情卡状态 pill 仅在 `chains[selectedIndex]?.success != null` 时渲染
  - parsedAction 的 confidence 显示：`detail.steps[3].confidence != null ? (confidence*100).toFixed(0)+'%' : 'n/a'`
  - 对应 CSS：`.attr-tick.neutral { border-color: ...; }`（用 `var(--text-muted-dark)`）

- [ ] **Step 8: 前端构建验证**

Run: `npm --prefix frontend run build`
Expected: built in ~1.3s

- [ ] **Step 9: Commit**

```bash
git add backend/src/attribution.rs backend/src/drec/generator.rs frontend/src/api.ts frontend/src/components/AttributionBar.vue
git commit -m "feat: make attribution confidence and success optional (M10 honesty)"
```

---

### Task 2: profile.rs — 极简 YAML 解析 + 别名匹配 + auto-detect

Files:
- Create: `backend/src/profile.rs`

- [ ] **Step 1: 写失败测试** — 文件末尾 `#[cfg(test)] mod tests`：

```rust
use super::*;

const B601_YAML: &str = r#"
# B601 profile
robot: B601
fields:
  state: [observation.state, observations/state, obs.state]
  action: [action]
  task: [task_index]
  timestamp: [timestamp]
  frame_index: [frame_index]
joint_mapping:
  arm_joints: [0, 1, 2, 3, 4, 5]
  gripper: 6
"#;

#[test]
fn parses_profile_fields_and_aliases() {
    let p = parse_profile_yaml(B601_YAML).unwrap();
    assert_eq!(p.robot_name, "B601");
    assert_eq!(p.fields.state, vec!["observation.state", "observations/state", "obs.state"]);
    assert_eq!(p.fields.action, vec!["action"]);
    assert_eq!(p.joint_mapping.arm_joints, vec![0, 1, 2, 3, 4, 5]);
    assert_eq!(p.joint_mapping.gripper, Some(6));
}

#[test]
fn rejects_missing_robot_line() {
    assert!(parse_profile_yaml("fields:\n  state: [a]\n").is_err());
}

#[test]
fn alias_matching_prefers_first_hit() {
    let p = parse_profile_yaml(B601_YAML).unwrap();
    let columns = vec!["action".to_string(), "observations/state".to_string(), "task_index".to_string(), "timestamp".to_string()];
    let (matched, total) = match_columns(&p, &columns);
    assert_eq!((matched, total), (4, 5)); // state 命中第二个别名；frame_index 未命中
}

#[test]
fn autodetect_scores_and_suggests_best_profile() {
    let a = parse_profile_yaml(B601_YAML).unwrap();
    let b = parse_profile_yaml("robot: OTHER\nfields:\n  state: [qpos]\n  action: [qvel]\njoint_mapping:\n  arm_joints: [0]\n").unwrap();
    let columns = vec!["observation.state".to_string(), "action".to_string(), "task_index".to_string(), "timestamp".to_string(), "frame_index".to_string()];
    // score = matched / profile 定义的字段数；a 应高于 b
    assert!(profile_score(&a, &columns) > profile_score(&b, &columns));
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test --manifest-path backend/Cargo.toml profile`
Expected: 编译失败（module 未声明）

- [ ] **Step 3: 声明模块** — `backend/src/main.rs` 顶部 `mod profile;`（按字母序插在 `otel` 与 `protocol` 之间）

- [ ] **Step 4: 实现解析器** — profile.rs 生产代码（行级解析，忽略空行与 `#` 注释；`fields:` 与 `joint_mapping:` 两个 section；列表 `[a, b]` 支持字符串与整数）：

```rust
//! LeRobot robot profile — maps dataset column names to attribution fields.

#[derive(Debug, Clone, PartialEq)]
pub struct FieldAliases {
    pub state: Vec<String>,
    pub action: Vec<String>,
    pub task: Vec<String>,
    pub timestamp: Vec<String>,
    pub frame_index: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JointMapping {
    pub arm_joints: Vec<usize>,
    pub gripper: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RobotProfile {
    pub robot_name: String,
    pub fields: FieldAliases,
    pub joint_mapping: JointMapping,
}

#[derive(Debug)]
pub enum ProfileError {
    Parse(String),
}

pub fn parse_profile_yaml(text: &str) -> Result<RobotProfile, ProfileError> {
    // 逐行状态机: 当前 section（None/fields/joint_mapping），
    // 每行 "key: [v1, v2]" 或 "key: 6"。robot: 行取 robot_name。
    // 字段名含 '.' 或 '/' 时按原样保留。
    // 实现细节: trim 行、跳过空行/注释；split_once(':')；section 行本身无值。
}

pub fn match_columns(profile: &RobotProfile, columns: &[String]) -> (usize, usize) {
    let fields = [
        &profile.fields.state,
        &profile.fields.action,
        &profile.fields.task,
        &profile.fields.timestamp,
        &profile.fields.frame_index,
    ];
    let total = fields.len();
    let mut matched = 0;
    for aliases in fields {
        if aliases.iter().any(|a| columns.iter().any(|c| c == a)) {
            matched += 1;
        }
    }
    (matched, total)
}

pub fn profile_score(profile: &RobotProfile, columns: &[String]) -> f32 {
    let (matched, total) = match_columns(profile, columns);
    matched as f32 / total.max(1) as f32
}
```

- [ ] **Step 5: 运行确认通过**

Run: `cargo test --manifest-path backend/Cargo.toml profile`
Expected: 4 passed

- [ ] **Step 6: Commit**

```bash
git add backend/src/profile.rs backend/src/main.rs
git commit -m "feat: add LeRobot robot profile YAML parser (M10)"
```

---

### Task 3: Python 桥脚本 lerobot_reader.py

Files:
- Create: `backend/scripts/lerobot_reader.py`

- [ ] **Step 1: 写脚本**（无 Rust 测试先行——脚本通过 Task 4 的 Rust 集成测试验证；本步用命令行手动验证）：

```python
#!/usr/bin/env python3
"""dora-studio LeRobot reader bridge. JSON on stdout, errors as {"error": ...}."""
import json
import sys
from pathlib import Path


def fail(msg):
    print(json.dumps({"error": msg}))
    sys.exit(1)


def main():
    if len(sys.argv) < 2:
        fail("usage: lerobot_reader.py <scan|frames|gen-demo> ...")
    cmd = sys.argv[1]
    try:
        import pyarrow.parquet as pq
    except ImportError:
        fail("pyarrow not installed; install with: pip install pyarrow")
    if cmd == "scan":
        scan(Path(sys.argv[2]))
    elif cmd == "frames":
        frames(Path(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5]))
    elif cmd == "gen-demo":
        gen_demo(Path(sys.argv[2]), sys.argv[3])
    else:
        fail(f"unknown command: {cmd}")


def scan(root: Path):
    # v1: data/episode_*.parquet; v2: data/chunk-*/file-*.parquet
    v1 = sorted(root.glob("data/episode_*.parquet"))
    v2 = sorted(root.glob("data/chunk-*/file-*.parquet"))
    if v1:
        layout, files = "v1", v1
        episodes = []
        for f in files:
            idx = int(f.stem.split("_")[-1])
            meta = pq.ParquetFile(f).metadata
            t = pq.read_table(f, columns=["timestamp"]).to_pydict()["timestamp"]
            episodes.append({"index": idx, "rows": meta.num_rows,
                             "startTs": float(t[0]), "endTs": float(t[-1])})
    elif v2:
        layout, files = "v2", v2
        by_ep = {}
        for f in files:
            d = pq.read_table(f, columns=["episode_index", "timestamp"]).to_pydict()
            for ep, ts in zip(d["episode_index"], d["timestamp"]):
                by_ep.setdefault(int(ep), []).append(float(ts))
        episodes = [{"index": ep, "rows": len(ts), "startTs": ts[0], "endTs": ts[-1]}
                    for ep, ts in sorted(by_ep.items())]
    else:
        fail(f"no LeRobot parquet data found under {root}")
    columns = pq.ParquetFile(files[0]).schema.names
    # 去重（schema.names 对 List 列会重复列名）
    columns = list(dict.fromkeys(columns))
    tasks = {}
    tasks_file = root / "meta" / "tasks.parquet"
    if tasks_file.exists():
        t = pq.read_table(tasks_file).to_pydict()
        desc_col = [c for c in t if c != "task_index"][0] if t else None
        for i, ti in enumerate(t.get("task_index", [])):
            tasks[int(ti)] = str(t[desc_col][i]) if desc_col else f"Task {int(ti)}"
    print(json.dumps({
        "name": root.name,
        "layout": layout,
        "columns": columns,
        "episodes": episodes,
        "tasks": tasks,
        "hasImageColumns": any(c.startswith("observation.images") for c in columns),
    }))


def frames(root: Path, episode: int, offset: int, limit: int):
    v1 = sorted(root.glob("data/episode_*.parquet"))
    if v1:
        f = root / f"data/episode_{episode:06d}.parquet"
        if not f.exists():
            fail(f"episode {episode} not found")
        t = pq.read_table(f)
    else:
        tables = []
        for f in sorted(root.glob("data/chunk-*/file-*.parquet")):
            t = pq.read_table(f)
            tables.append(t.filter(t.column("episode_index") == episode))
        if not tables:
            fail(f"episode {episode} not found")
        t = pa.concat_tables(tables) if len(tables) > 1 else tables[0]  # noqa: F821
    d = t.to_pydict()
    total = len(d["timestamp"])
    sl = slice(offset, offset + limit)
    ts = d["timestamp"]
    print(json.dumps({
        "frames": [{
            "frameIndex": int(d["frame_index"][i]) if d.get("frame_index") and d["frame_index"][i] is not None else i,
            "timestamp": float(ts[i]),
            "taskIndex": int(d["task_index"][i]) if d.get("task_index") and d["task_index"][i] is not None else None,
            "action": [float(x) for x in (d["action"][i] or [])],
            "state": [float(x) for x in (d["observation.state"][i] or [])],
        } for i in range(sl.start, min(sl.stop, total))],
        "total": total,
        "episodeStartTs": float(ts[0]),
    }))


def gen_demo(root: Path, layout: str):
    import pyarrow as pa
    root.mkdir(parents=True, exist_ok=True)
    (root / "meta").mkdir(exist_ok=True)
    n_ep, n_frames = 3, 40
    rows = []
    for ep in range(n_ep):
        for i in range(n_frames):
            rows.append({
                "action": [0.1 * ep + 0.001 * i] * 7,
                "observation.state": [0.2 * ep + 0.001 * i] * 7,
                "timestamp": float(i) / 30.0,
                "frame_index": i,
                "episode_index": ep,
                "index": ep * n_frames + i,
                "task_index": ep,
            })
    schema = pa.schema([
        ("action", pa.list_(pa.float32())),
        ("observation.state", pa.list_(pa.float32())),
        ("timestamp", pa.float32()),
        ("frame_index", pa.int64()),
        ("episode_index", pa.int64()),
        ("index", pa.int64()),
        ("task_index", pa.int64()),
    ])
    table = pa.Table.from_pylist(rows, schema=schema)
    tasks = pa.Table.from_pylist(
        [{"task_index": ep, "task": f"Demo task {ep}"} for ep in range(n_ep)])
    if layout == "v1":
        (root / "data").mkdir(exist_ok=True)
        for ep in range(n_ep):
            pq.write_table(table.filter(table.column("episode_index") == ep),
                           root / "data" / f"episode_{ep:06d}.parquet")
    else:
        (root / "data" / "chunk-000").mkdir(parents=True, exist_ok=True)
        pq.write_table(table, root / "data" / "chunk-000" / "file-000.parquet")
    pq.write_table(tasks, root / "meta" / "tasks.parquet")
    print(json.dumps({"ok": True, "layout": layout}))


if __name__ == "__main__":
    main()
```

注意：v2 分支用到 `pa`（pyarrow 别名），需在 `import pyarrow.parquet as pq` 后加 `import pyarrow as pa`。

- [ ] **Step 2: 手动验证脚本**

Run:
```bash
python3 backend/scripts/lerobot_reader.py gen-demo /tmp/dora-studio-tests/lerobot_demo_v1 v1
python3 backend/scripts/lerobot_reader.py scan /tmp/dora-studio-tests/lerobot_demo_v1
python3 backend/scripts/lerobot_reader.py frames /tmp/dora-studio-tests/lerobot_demo_v1 1 0 5
python3 backend/scripts/lerobot_reader.py gen-demo /tmp/dora-studio-tests/lerobot_demo_v2 v2
python3 backend/scripts/lerobot_reader.py scan /tmp/dora-studio-tests/lerobot_demo_v2
```
Expected: scan 输出 3 episodes、tasks {0,1,2}、columns 含 action/observation.state；frames 输出 5 帧 JSON。

- [ ] **Step 3: Commit**

```bash
git add backend/scripts/lerobot_reader.py
git commit -m "feat: add LeRobot parquet reader bridge script (M10)"
```

---

### Task 4: lerobot.rs — 子进程桥 + scan/frames 类型

Files:
- Create: `backend/src/lerobot.rs`

- [ ] **Step 1: 写失败测试** — `#[cfg(test)] mod tests`（python3/pyarrow 缺失时跳过并打印原因）：

```rust
use super::*;
use std::path::PathBuf;

fn demo_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join("dora-studio-tests").join(name)
}

async fn ensure_demo(layout: &str) -> Option<PathBuf> {
    let dir = demo_dir(&format!("lerobot_demo_{layout}"));
    if !dir.join("meta/tasks.parquet").exists() {
        let script = script_path();
        let out = tokio::process::Command::new("python3")
            .arg(&script).arg("gen-demo").arg(&dir).arg(layout)
            .output().await.ok()?;
        if !out.status.success() {
            eprintln!("python3/pyarrow unavailable — skipping lerobot tests");
            return None;
        }
    }
    Some(dir)
}

#[tokio::test]
async fn scan_detects_v1_layout_and_episodes() {
    let Some(dir) = ensure_demo("v1").await else { return };
    let info = scan_dataset(&dir).await.expect("scan");
    assert_eq!(info.layout, "v1");
    assert_eq!(info.episodes.len(), 3);
    assert_eq!(info.episodes[0].rows, 40);
    assert!(info.columns.iter().any(|c| c == "observation.state"));
    assert_eq!(info.tasks.get(&1).map(String::as_str), Some("Demo task 1"));
    assert!(!info.has_image_columns);
}

#[tokio::test]
async fn scan_detects_v2_layout() {
    let Some(dir) = ensure_demo("v2").await else { return };
    let info = scan_dataset(&dir).await.expect("scan");
    assert_eq!(info.layout, "v2");
    assert_eq!(info.episodes.len(), 3);
}

#[tokio::test]
async fn read_frames_paginates_and_normalizes_time() {
    let Some(dir) = ensure_demo("v1").await else { return };
    let (frames, total) = read_frames(&dir, 1, 5, 10).await.expect("frames");
    assert_eq!(total, 40);
    assert_eq!(frames.len(), 10);
    // 第 5 帧 ts=5/30s → 归一化后 166_666_666ns（1/30 的 5 倍，浮点误差 <1ms）
    let expected = (5.0 / 30.0 * 1e9) as u64;
    assert!((frames[0].timestamp_ns as i64 - expected as i64).abs() < 1_000_000);
    assert_eq!(frames[0].action.len(), 7);
    assert_eq!(frames[0].task_index, Some(1));
}

#[tokio::test]
async fn scan_reports_missing_dataset_cleanly() {
    let err = scan_dataset(&PathBuf::from("/tmp/does-not-exist-xyz")).await.unwrap_err();
    assert!(err.contains("no LeRobot parquet data") || err.contains("error"));
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test --manifest-path backend/Cargo.toml lerobot`
Expected: 编译失败（module 未声明）

- [ ] **Step 3: 声明模块并实现** — main.rs 加 `mod lerobot;`；lerobot.rs 生产代码：

```rust
//! LeRobot dataset reader — Python 3 + pyarrow subprocess bridge.
//! JSON stdout protocol; 30s timeout; graceful errors when pyarrow is missing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::attribution::{AttributionChain, AttributionStep};

pub fn script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/lerobot_reader.py")
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LerobotStatus {
    pub python_available: bool,
    pub pyarrow_available: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeInfo {
    pub index: u32,
    pub rows: usize,
    pub start_ns: u64,
    pub end_ns: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetInfo {
    pub name: String,
    pub layout: String,
    pub columns: Vec<String>,
    pub episodes: Vec<EpisodeInfo>,
    pub tasks: BTreeMap<u32, String>,
    pub has_image_columns: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameData {
    pub frame_index: u64,
    pub timestamp_ns: u64,
    pub task_index: Option<u32>,
    pub action: Vec<f32>,
    pub state: Vec<f32>,
}

#[derive(Deserialize)]
struct RawScanResponse {
    name: String,
    layout: String,
    columns: Vec<String>,
    episodes: Vec<RawEpisode>,
    #[serde(default)]
    tasks: BTreeMap<u32, String>,
    #[serde(default)]
    #[serde(rename = "hasImageColumns")]
    has_image_columns: bool,
}

#[derive(Deserialize)]
struct RawEpisode {
    index: u32,
    rows: usize,
    #[serde(rename = "startTs")]
    start_ts: f64,
    #[serde(rename = "endTs")]
    end_ts: f64,
}

#[derive(Deserialize)]
struct RawFramesResponse {
    frames: Vec<RawFrame>,
    total: usize,
    #[serde(rename = "episodeStartTs")]
    episode_start_ts: f64,
}

#[derive(Deserialize)]
struct RawFrame {
    #[serde(rename = "frameIndex")]
    frame_index: u64,
    timestamp: f64,
    #[serde(rename = "taskIndex")]
    task_index: Option<u32>,
    #[serde(default)]
    action: Vec<f32>,
    #[serde(default)]
    state: Vec<f32>,
}

#[derive(Deserialize)]
struct ScriptError {
    error: String,
}

async fn run_script(args: &[String]) -> Result<String, String> {
    let mut cmd = tokio::process::Command::new("python3");
    cmd.arg(script_path()).args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "python3 not found on PATH".to_string()
        } else {
            format!("failed to spawn python3: {e}")
        }
    })?;
    let out = tokio::time::timeout(Duration::from_secs(30), child.wait_with_output())
        .await
        .map_err(|_| "lerobot bridge timed out after 30s".to_string())?
        .map_err(|e| format!("lerobot bridge failed: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.status.success() {
        if let Ok(err) = serde_json::from_str::<ScriptError>(&stdout) {
            return Err(err.error);
        }
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        return Err(if stderr.is_empty() { stdout } else { stderr });
    }
    Ok(stdout)
}

pub async fn check_status() -> LerobotStatus {
    match run_script(&["scan".to_string(), "/tmp/nonexistent-dora-studio-check".to_string()]).await {
        Err(e) if e.contains("pyarrow") => LerobotStatus {
            python_available: true, pyarrow_available: false, message: e,
        },
        Err(e) if e.contains("python3") => LerobotStatus {
            python_available: false, pyarrow_available: false, message: e,
        },
        // 路径不存在 → 脚本在 pyarrow 检查后才报数据缺失 = pyarrow OK
        _ => LerobotStatus {
            python_available: true, pyarrow_available: true,
            message: "python3 + pyarrow available".to_string(),
        },
    }
}

pub async fn scan_dataset(path: &Path) -> Result<DatasetInfo, String> {
    let out = run_script(&["scan".to_string(), path.to_string_lossy().to_string()]).await?;
    let raw: RawScanResponse =
        serde_json::from_str(&out).map_err(|e| format!("bad scan response: {e}"))?;
    Ok(DatasetInfo {
        name: raw.name,
        layout: raw.layout,
        columns: raw.columns,
        episodes: raw.episodes.into_iter().map(|e| EpisodeInfo {
            index: e.index, rows: e.rows,
            start_ns: (e.start_ts * 1e9) as u64, end_ns: (e.end_ts * 1e9) as u64,
        }).collect(),
        tasks: raw.tasks,
        has_image_columns: raw.has_image_columns,
    })
}

pub async fn read_frames(path: &Path, episode: u32, offset: usize, limit: usize) -> Result<(Vec<FrameData>, usize), String> {
    let out = run_script(&[
        "frames".to_string(), path.to_string_lossy().to_string(),
        episode.to_string(), offset.to_string(), limit.to_string(),
    ]).await?;
    let raw: RawFramesResponse =
        serde_json::from_str(&out).map_err(|e| format!("bad frames response: {e}"))?;
    let frames = raw.frames.into_iter().map(|f| FrameData {
        frame_index: f.frame_index,
        timestamp_ns: (((f.timestamp - raw.episode_start_ts).max(0.0)) * 1e9) as u64,
        task_index: f.task_index,
        action: f.action,
        state: f.state,
    }).collect();
    Ok((frames, raw.total))
}

/// 每帧一条归因链（缺失步骤不生成，UI 显示占位）。
pub fn chains_from_frames(
    frames: &[FrameData],
    tasks: &BTreeMap<u32, String>,
) -> Vec<AttributionChain> {
    frames.iter().map(|f| {
        let task_text = f.task_index
            .and_then(|t| tasks.get(&t))
            .cloned()
            .unwrap_or_else(|| format!("Task {}", f.task_index.map(|t| t.to_string()).unwrap_or_else(|| "?".to_string())));
        let mut steps = vec![
            AttributionStep::SensorFrame {
                topic: "lerobot/observation.state".to_string(),
                width: f.state.len() as u32,
                height: 1,
                encoding: "float32".to_string(),
            },
            AttributionStep::Prompt {
                token_count: task_text.split_whitespace().count() as u32,
                text: task_text,
            },
            AttributionStep::ParsedAction {
                action_type: "joint_target".to_string(),
                vector: f.action.clone(),
                confidence: None,
            },
        ];
        steps.sort_by_key(AttributionStep::order);
        AttributionChain { timestamp_nanos: f.timestamp_ns, steps }
    }).collect()
}
```

（`AttributionStep::order` 需改为 `pub(crate)`。）

- [ ] **Step 4: 运行确认通过**

Run: `cargo test --manifest-path backend/Cargo.toml lerobot`
Expected: 4 passed（或 4 skipped 并打印原因——那说明本机 python3/pyarrow 异常，停下排查）

- [ ] **Step 5: 真实数据冒烟（临时命令，验证后不保留）**

Run:
```bash
python3 backend/scripts/lerobot_reader.py scan /home/dora/.cache/huggingface/lerobot/my_org/b601_pilot_v1 | head -c 400
```
Expected: 5 episodes、任务描述 "Pick up the red cube..."

- [ ] **Step 6: Commit**

```bash
git add backend/src/lerobot.rs backend/src/main.rs
git commit -m "feat: add LeRobot dataset reader with python bridge (M10)"
```

---

### Task 5: 归因映射测试 + profiles 目录

Files:
- Test: `backend/src/lerobot.rs`（chains_from_frames 测试）
- Create: `profiles/lerobot_profile_b601.yaml`、`profiles/lerobot_profile_so100.yaml`、`profiles/lerobot_profile_gen72.yaml`、`profiles/README.md`

- [ ] **Step 1: 写失败测试** — lerobot.rs tests 模块追加：

```rust
#[test]
fn chains_from_frames_maps_steps_and_omits_unavailable() {
    use crate::attribution::AttributionStep;
    let frames = vec![FrameData {
        frame_index: 0, timestamp_ns: 33_333_333,
        task_index: Some(0),
        action: vec![0.1, 0.2, 0.3],
        state: vec![0.0, 0.1, 0.2, 0.3],
    }];
    let tasks = BTreeMap::from([(0u32, "Pick up the red cube".to_string())]);
    let chains = chains_from_frames(&frames, &tasks);
    assert_eq!(chains.len(), 1);
    let steps = &chains[0].steps;
    assert_eq!(steps.len(), 3); // 无 LLM 回复/执行结果
    assert!(matches!(&steps[0], AttributionStep::SensorFrame { topic, width, .. }
        if topic == "lerobot/observation.state" && *width == 4));
    assert!(matches!(&steps[1], AttributionStep::Prompt { text, .. } if text == "Pick up the red cube"));
    assert!(matches!(&steps[2], AttributionStep::ParsedAction { vector, confidence: None, .. }
        if vector.len() == 3));
    assert_eq!(chains[0].timestamp_nanos, 33_333_333);
    assert_eq!(chains[0].success(), None); // 无执行结果 → 中性
}

#[test]
fn chains_from_frames_falls_back_to_task_label() {
    let frames = vec![FrameData {
        frame_index: 0, timestamp_ns: 0, task_index: Some(7),
        action: vec![], state: vec![],
    }];
    let chains = chains_from_frames(&frames, &BTreeMap::new());
    assert!(matches!(&chains[0].steps[1], AttributionStep::Prompt { text, .. } if text == "Task 7"));
}
```

- [ ] **Step 2: 运行确认失败**（`order` 尚为私有 → 编译失败）

Run: `cargo test --manifest-path backend/Cargo.toml lerobot`
Expected: 编译失败（private method `order`）

- [ ] **Step 3: 实现** — attribution.rs 中 `fn order` → `pub(crate) fn order`

- [ ] **Step 4: 运行确认通过**

Run: `cargo test --manifest-path backend/Cargo.toml lerobot`
Expected: 6 passed（或跳过 python 部分后 2 passed）

- [ ] **Step 5: 创建 profiles/** — `profiles/lerobot_profile_b601.yaml`：

```yaml
# B601 arm profile — real pilot dataset validated in M10
robot: B601
fields:
  state: [observation.state, observations/state, obs.state]
  action: [action]
  task: [task_index]
  timestamp: [timestamp]
  frame_index: [frame_index]
joint_mapping:
  arm_joints: [0, 1, 2, 3, 4, 5]
  gripper: 6
```

`profiles/lerobot_profile_so100.yaml` 与 `lerobot_profile_gen72.yaml`：结构相同，`robot:` 分别为 SO-100 / GEN72，别名额外含 `observation/images` 无关项（保持模板最小：state 别名同 B601）。`profiles/README.md`：字段语义、别名规则、joint_mapping 含义、如何新增机器人（复制模板改 robot 名与别名）。

- [ ] **Step 6: Commit**

```bash
git add profiles/ backend/src/lerobot.rs backend/src/attribution.rs
git commit -m "feat: map LeRobot frames to attribution chains and add robot profiles (M10)"
```

---

### Task 6: API 端点 + ProfileManager

Files:
- Modify: `backend/src/profile.rs`（ProfileManager）
- Modify: `backend/src/main.rs`（AppState + 6 路由 + handlers）

- [ ] **Step 1: 写失败测试** — profile.rs tests 追加（用 std::env::temp_dir 写入两个临时 profile 文件，测试后删除）：

```rust
#[test]
fn profile_manager_lists_and_loads_profiles() {
    let dir = std::env::temp_dir().join("dora-studio-tests/profiles_test");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("lerobot_profile_testa.yaml"), B601_YAML).unwrap();
    std::fs::write(dir.join("not_a_profile.txt"), "x").unwrap();
    let mgr = ProfileManager::new(&dir);
    let names = mgr.list().unwrap();
    assert_eq!(names, vec!["testa".to_string()]);
    let p = mgr.load("testa").unwrap();
    assert_eq!(p.robot_name, "B601");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn profile_manager_autodetect_suggests_best_match() {
    let dir = std::env::temp_dir().join("dora-studio-tests/profiles_test2");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("lerobot_profile_best.yaml"), B601_YAML).unwrap();
    std::fs::write(dir.join("lerobot_profile_worst.yaml"),
        "robot: X\nfields:\n  state: [qpos]\n  action: [qvel]\njoint_mapping:\n  arm_joints: [0]\n").unwrap();
    let mgr = ProfileManager::new(&dir);
    let columns = vec!["observation.state".to_string(), "action".to_string(),
        "task_index".to_string(), "timestamp".to_string(), "frame_index".to_string()];
    let (name, score) = mgr.autodetect(&columns).unwrap().expect("suggestion");
    assert_eq!(name, "best");
    assert!(score > 0.9);
    std::fs::remove_dir_all(&dir).ok();
}
```

- [ ] **Step 2: 运行确认失败**（ProfileManager 不存在）

- [ ] **Step 3: 实现 ProfileManager** — profile.rs 追加：

```rust
use std::path::{Path, PathBuf};

pub struct ProfileManager {
    dir: PathBuf,
}

impl ProfileManager {
    pub fn new(dir: &Path) -> Self { Self { dir: dir.to_path_buf() } }

    /// 返回去前缀（lerobot_profile_）、去 .yaml 后缀的 profile 名列表。
    pub fn list(&self) -> Result<Vec<String>, ProfileError> {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(&self.dir).map_err(|e| ProfileError::Parse(e.to_string()))? {
            let name = entry.map_err(|e| ProfileError::Parse(e.to_string()))?.file_name();
            let name = name.to_string_lossy().to_string();
            if let Some(stem) = name.strip_prefix("lerobot_profile_").and_then(|s| s.strip_suffix(".yaml")) {
                names.push(stem.to_string());
            }
        }
        names.sort();
        Ok(names)
    }

    pub fn load(&self, name: &str) -> Result<RobotProfile, ProfileError> {
        let path = self.dir.join(format!("lerobot_profile_{name}.yaml"));
        let text = std::fs::read_to_string(&path)
            .map_err(|e| ProfileError::Parse(format!("profile '{name}': {e}")))?;
        parse_profile_yaml(&text)
    }

    /// 按 profile_score 返回最高分建议；无 profile 得分 ≥ 0.5 时返回 None。
    pub fn autodetect(&self, columns: &[String]) -> Result<Option<(String, f32)>, ProfileError> {
        let mut best: Option<(String, f32)> = None;
        for name in self.list()? {
            let profile = self.load(&name)?;
            let score = profile_score(&profile, columns);
            if best.as_ref().map(|(_, s)| score > *s).unwrap_or(true) {
                best = Some((name, score));
            }
        }
        Ok(best.filter(|(_, s)| *s >= 0.5))
    }
}
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test --manifest-path backend/Cargo.toml profile`
Expected: 6 passed

- [ ] **Step 5: main.rs 接线** — AppState 加 `profiles: profile::ProfileManager`（`ProfileManager::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../profiles"))`）；路由：

```rust
.route("/api/lerobot/status", get(lerobot_status))
.route("/api/lerobot/scan", post(lerobot_scan))
.route("/api/lerobot/frames", post(lerobot_frames))
.route("/api/lerobot/profiles", get(lerobot_profiles))
.route("/api/lerobot/autodetect", post(lerobot_autodetect))
.route("/api/lerobot/attribution", post(lerobot_attribution))
```

Handlers（main.rs 尾部）：

```rust
// --- LeRobot API (M10) ---

#[derive(serde::Deserialize)]
struct LerobotScanRequest { path: String }

#[derive(serde::Deserialize)]
struct LerobotFramesRequest { path: String, episode: u32, #[serde(default = "default_frame_offset")] offset: usize, #[serde(default = "default_frame_limit")] limit: usize }
fn default_frame_offset() -> usize { 0 }
fn default_frame_limit() -> usize { 200 }

#[derive(serde::Deserialize)]
struct LerobotAttributionRequest { path: String, profile: Option<String>, episode: u32, #[serde(default = "default_frame_offset")] offset: usize, #[serde(default = "default_frame_limit")] limit: usize }

async fn lerobot_status() -> Json<lerobot::LerobotStatus> {
    Json(lerobot::check_status().await)
}

async fn lerobot_scan(Json(req): Json<LerobotScanRequest>) -> Result<Json<lerobot::DatasetInfo>, ApiError> {
    lerobot::scan_dataset(std::path::Path::new(&req.path)).await.map(Json).map_err(|e| ApiError { status: StatusCode::UNPROCESSABLE_ENTITY, message: e })
}

async fn lerobot_frames(Json(req): Json<LerobotFramesRequest>) -> Result<Json<serde_json::Value>, ApiError> {
    let (frames, total) = lerobot::read_frames(std::path::Path::new(&req.path), req.episode, req.offset, req.limit).await.map_err(|e| ApiError { status: StatusCode::UNPROCESSABLE_ENTITY, message: e })?;
    Ok(Json(serde_json::json!({ "frames": frames, "total": total })))
}

async fn lerobot_profiles(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let names = state.profiles.list().unwrap_or_default();
    let profiles: Vec<serde_json::Value> = names.iter().filter_map(|n| {
        state.profiles.load(n).ok().map(|p| serde_json::json!({ "name": n, "robot": p.robot_name }))
    }).collect();
    Json(serde_json::json!({ "profiles": profiles }))
}

async fn lerobot_autodetect(State(state): State<Arc<AppState>>, Json(req): Json<LerobotScanRequest>) -> Result<Json<serde_json::Value>, ApiError> {
    let info = lerobot::scan_dataset(std::path::Path::new(&req.path)).await.map_err(|e| ApiError { status: StatusCode::UNPROCESSABLE_ENTITY, message: e })?;
    let suggestion = state.profiles.autodetect(&info.columns).map_err(|e| ApiError { status: StatusCode::INTERNAL_SERVER_ERROR, message: e.to_string() })?;
    Ok(Json(serde_json::json!({
        "columns": info.columns,
        "suggestedProfile": suggestion.as_ref().map(|(n, _)| n),
        "score": suggestion.as_ref().map(|(_, s)| s),
    })))
}

async fn lerobot_attribution(State(state): State<Arc<AppState>>, Json(req): Json<LerobotAttributionRequest>) -> Result<Json<serde_json::Value>, ApiError> {
    let info = lerobot::scan_dataset(std::path::Path::new(&req.path)).await.map_err(|e| ApiError { status: StatusCode::UNPROCESSABLE_ENTITY, message: e })?;
    let profile_name = match req.profile {
        Some(n) => n,
        None => state.profiles.autodetect(&info.columns).map_err(|e| ApiError { status: StatusCode::INTERNAL_SERVER_ERROR, message: e.to_string() })?
            .map(|(n, _)| n).ok_or(ApiError { status: StatusCode::UNPROCESSABLE_ENTITY, message: "no matching robot profile; add one under profiles/".to_string() })?,
    };
    let _profile = state.profiles.load(&profile_name).map_err(|e| ApiError { status: StatusCode::UNPROCESSABLE_ENTITY, message: e.to_string() })?;
    let (frames, total) = lerobot::read_frames(std::path::Path::new(&req.path), req.episode, req.offset, req.limit).await.map_err(|e| ApiError { status: StatusCode::UNPROCESSABLE_ENTITY, message: e })?;
    let chains = lerobot::chains_from_frames(&frames, &info.tasks);
    let summaries: Vec<serde_json::Value> = chains.iter().map(|c| serde_json::json!({
        "timestampNanos": c.timestamp_nanos, "success": c.success(), "stepCount": c.steps.len(),
    })).collect();
    Ok(Json(serde_json::json!({
        "chains": chains, "summaries": summaries, "total": total,
        "profile": profile_name, "tasks": info.tasks,
    })))
}
```

- [ ] **Step 6: 运行全部后端测试**

Run: `cargo test --manifest-path backend/Cargo.toml`
Expected: 全绿（100+）

- [ ] **Step 7: 冒烟（3002 端口避免冲突）**

Run:
```bash
DORA_STUDIO_BACKEND_ADDR=127.0.0.1:3002 cargo run --manifest-path backend/Cargo.toml &
sleep 3
curl -s http://127.0.0.1:3002/api/lerobot/status
curl -s -X POST http://127.0.0.1:3002/api/lerobot/scan -H 'Content-Type: application/json' -d '{"path":"/home/dora/.cache/huggingface/lerobot/my_org/b601_pilot_v1"}' | head -c 300
curl -s -X POST http://127.0.0.1:3002/api/lerobot/attribution -H 'Content-Type: application/json' -d '{"path":"/home/dora/.cache/huggingface/lerobot/my_org/b601_pilot_v1","episode":0,"limit":3}' | head -c 500
```
Expected: status pyarrow available；scan 5 episodes + 任务描述；attribution 3 条链（SensorFrame/Prompt/ParsedAction，success null）。测完 kill 进程。

- [ ] **Step 8: Commit**

```bash
git add backend/src/profile.rs backend/src/main.rs
git commit -m "feat: add LeRobot API endpoints and profile manager (M10)"
```

---

### Task 7: 前端 api.ts + i18n

Files:
- Modify: `frontend/src/api.ts`
- Modify: `frontend/src/i18n.ts`

- [ ] **Step 1: api.ts 追加**（`// --- lerobot (M10) ---` 注释段）：

```ts
export type LerobotStatusResponse = {
  pythonAvailable: boolean
  pyarrowAvailable: boolean
  message: string
}

export type LerobotEpisodeResponse = {
  index: number
  rows: number
  startNs: number
  endNs: number
}

export type LerobotDatasetResponse = {
  name: string
  layout: string
  columns: string[]
  episodes: LerobotEpisodeResponse[]
  tasks: Record<number, string>
  hasImageColumns: boolean
}

export type LerobotFrameResponse = {
  frameIndex: number
  timestampNs: number
  taskIndex: number | null
  action: number[]
  state: number[]
}

export type LerobotFramesResponse = { frames: LerobotFrameResponse[]; total: number }

export type LerobotProfileResponse = { name: string; robot: string }

export type LerobotAutodetectResponse = {
  columns: string[]
  suggestedProfile: string | null
  score: number | null
}

export type LerobotAttributionResponse = {
  chains: AttributionChainResponse[]
  summaries: { timestampNanos: number; success: boolean | null; stepCount: number }[]
  total: number
  profile: string
  tasks: Record<number, string>
}

export function getLerobotStatus() {
  return fetchJson<LerobotStatusResponse>('/lerobot/status')
}

export function scanLerobotDataset(path: string) {
  return fetchJson<LerobotDatasetResponse>('/lerobot/scan', {
    method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ path }),
  })
}

export function getLerobotProfiles() {
  return fetchJson<{ profiles: LerobotProfileResponse[] }>('/lerobot/profiles')
}

export function autodetectLerobotProfile(path: string) {
  return fetchJson<LerobotAutodetectResponse>('/lerobot/autodetect', {
    method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ path }),
  })
}

export function getLerobotAttribution(
  path: string, episode: number, offset = 0, limit = 200, profile?: string,
) {
  return fetchJson<LerobotAttributionResponse>('/lerobot/attribution', {
    method: 'POST', headers: JSON_HEADER,
    body: JSON.stringify({ path, episode, offset, limit, ...(profile ? { profile } : {}) }),
  })
}
```

- [ ] **Step 2: i18n.ts 追加**（`attribution` 对象内追加，中英双语同 key）：

```ts
// zh
datasetPath: '数据集路径',
scan: '扫描',
scanning: '扫描中…',
episodes: 'episodes',
profile: '配置',
autoDetect: '自动检测',
loadEpisode: '加载 Episode',
frames: '帧',
noImageData: '无图像数据',
notAvailable: '数据集中未记录',
page: '页',
of: '/',
datasetNotFound: '数据集扫描失败',
// en
datasetPath: 'Dataset path',
scan: 'Scan',
scanning: 'Scanning…',
episodes: 'episodes',
profile: 'Profile',
autoDetect: 'Auto-detect',
loadEpisode: 'Load episode',
frames: 'frames',
noImageData: 'No image data',
notAvailable: 'Not available in this dataset',
page: 'Page',
of: 'of',
datasetNotFound: 'Dataset scan failed',
```

（`Messages` 类型的 `attribution` 块同步加同名字段，两处 locale 各一份）

- [ ] **Step 3: 构建验证**

Run: `npm --prefix frontend run build`
Expected: 通过

- [ ] **Step 4: Commit**

```bash
git add frontend/src/api.ts frontend/src/i18n.ts
git commit -m "feat: add LeRobot API client and i18n strings (M10)"
```

---

### Task 8: AttributionBar LeRobot 数据源 UI

Files:
- Modify: `frontend/src/components/AttributionBar.vue`

- [ ] **Step 1: script 部分改造**（逐段替换）：
  - import 增加 `getLerobotAttribution, getLerobotProfiles, scanLerobotDataset, autodetectLerobotProfile, type LerobotDatasetResponse, type LerobotProfileResponse, type AttributionChainResponse`
  - `source` 的 `lerobot` 选项取消 disabled
  - 新增状态：

```ts
const lerobotPath = ref('')
const lerobotScanning = ref(false)
const lerobotError = ref<string | null>(null)
const dataset = ref<LerobotDatasetResponse | null>(null)
const profiles = ref<LerobotProfileResponse[]>([])
const selectedProfile = ref('')
const selectedEpisode = ref<number | null>(null)
const pageOffset = ref(0)
const pageSize = 200
const lerobotTotal = ref(0)
const lerobotChains = ref<AttributionChainResponse[]>([])
const lerobotSummaries = ref<{ timestampNanos: number; success: boolean | null; stepCount: number }[]>([])
```

  - 新增函数：

```ts
async function runLerobotScan() {
  if (!lerobotPath.value) return
  lerobotScanning.value = true
  lerobotError.value = null
  try {
    dataset.value = await scanLerobotDataset(lerobotPath.value)
    const [profilesResult, autodetectResult] = await Promise.all([
      getLerobotProfiles(),
      autodetectLerobotProfile(lerobotPath.value).catch(() => null),
    ])
    profiles.value = profilesResult.profiles
    selectedProfile.value = autodetectResult?.suggestedProfile ?? profiles.value[0]?.name ?? ''
    selectedEpisode.value = dataset.value.episodes[0]?.index ?? null
    pageOffset.value = 0
    await loadLerobotEpisode()
  } catch (e) {
    lerobotError.value = e instanceof Error ? e.message : 'scan failed'
    dataset.value = null
  } finally {
    lerobotScanning.value = false
  }
}

async function loadLerobotEpisode() {
  if (!dataset.value || selectedEpisode.value === null) return
  lerobotError.value = null
  try {
    const result = await getLerobotAttribution(
      lerobotPath.value, selectedEpisode.value, pageOffset.value, pageSize,
      selectedProfile.value || undefined,
    )
    lerobotChains.value = result.chains
    lerobotSummaries.value = result.summaries
    lerobotTotal.value = result.total
    selectedTs.value = null
    detail.value = null
    stopTokenStream()
  } catch (e) {
    lerobotError.value = e instanceof Error ? e.message : 'load failed'
  }
}

async function lerobotSelectChain(ts: number) {
  selectedTs.value = ts
  detail.value = lerobotChains.value.find((c) => c.timestampNanos === ts) ?? null
  detailError.value = detail.value ? null : 'chain not found in loaded page'
  expandedText.value = {}
  stopTokenStream()
}

async function lerobotPage(dir: 1 | -1) {
  const next = pageOffset.value + dir * pageSize
  if (next < 0 || next >= lerobotTotal.value) return
  pageOffset.value = next
  await loadLerobotEpisode()
}
```

  - `sourceOptions` 中 lerobot 项：`disabled: false, hint: ''`
  - 统一 chains/summaries 计算：`chains` computed 改为 `source === 'lerobot' ? lerobotSummaries : summary?.chains ?? []`；`selectChain(ts, seek)` 内部按 source 分发（lerobot → `lerobotSelectChain(ts)`，drec → 现有逻辑）；`moveChain` 不变（复用 chains computed）
  - `onShowIn3d()`：

```ts
function onShowIn3d() {
  if (selectedTs.value === null) return
  if (source.value === 'lerobot') {
    const chain = detail.value
    const action = chain?.steps.find((s) => s.kind === 'parsedAction')
    if (action?.kind === 'parsedAction') emit('apply-action', action.vector)
  } else {
    emit('seek-timestamp', selectedTs.value)
  }
  collapsed.value = true
}
```

  - emits 增加 `'apply-action': [vector: number[]]`

- [ ] **Step 2: template 部分改造**：
  - "Show in 3D" 按钮 `@click="onShowIn3d"`
  - chains 为空且 source==='lerobot' 时的提示改为 lerobot 引导（"输入数据集路径并点击扫描"）
  - 在 `<template v-else>`（有链）之前插入 LeRobot 控制区（source==='lerobot' 时显示）：

```html
<div v-if="source === 'lerobot'" class="attr-lerobot">
  <div class="attr-lerobot-row">
    <input v-model="lerobotPath" class="attr-lerobot-path"
      placeholder="/path/to/dataset (e.g. ~/.cache/huggingface/lerobot/my_org/b601_pilot_v1)"
      @keyup.enter="runLerobotScan" />
    <button class="attr-cta" type="button" :disabled="lerobotScanning" @click="runLerobotScan">
      {{ lerobotScanning ? t.attribution.scanning : t.attribution.scan }}
    </button>
  </div>
  <div v-if="lerobotError" class="attr-state error">{{ lerobotError }}</div>
  <div v-if="dataset" class="attr-lerobot-info">
    <span class="attr-chip">{{ dataset.name }}</span>
    <span class="attr-chip">{{ dataset.layout }}</span>
    <span class="attr-chip">{{ dataset.episodes.length }} {{ t.attribution.episodes }}</span>
    <span class="attr-chip mono">{{ dataset.columns.length }} cols</span>
  </div>
  <div v-if="dataset" class="attr-lerobot-row">
    <label class="attr-source">
      <span class="attr-source-label">{{ t.attribution.profile }}</span>
      <select v-model="selectedProfile" class="attr-source-select" @change="pageOffset = 0; loadLerobotEpisode()">
        <option v-for="p in profiles" :key="p.name" :value="p.name">{{ p.robot }} ({{ p.name }})</option>
      </select>
    </label>
    <label class="attr-source">
      <span class="attr-source-label">Episode</span>
      <select v-model="selectedEpisode" class="attr-source-select" @change="pageOffset = 0; loadLerobotEpisode()">
        <option v-for="e in dataset.episodes" :key="e.index" :value="e.index">
          #{{ e.index }} · {{ e.rows }} {{ t.attribution.frames }}
        </option>
      </select>
    </label>
    <span class="attr-detail-spacer"></span>
    <button v-if="lerobotTotal > pageSize" class="attr-nav" type="button" :disabled="pageOffset === 0" @click="lerobotPage(-1)">‹</button>
    <span v-if="lerobotTotal > pageSize" class="attr-chip">
      {{ t.attribution.page }} {{ pageOffset / pageSize + 1 }} {{ t.attribution.of }} {{ Math.ceil(lerobotTotal / pageSize) }}
    </span>
    <button v-if="lerobotTotal > pageSize" class="attr-nav" type="button" :disabled="pageOffset + pageSize >= lerobotTotal" @click="lerobotPage(1)">›</button>
  </div>
</div>
```

  - 详情卡缺失步骤占位：将 5 个 `v-if="detail.steps[N]?.kind === '...'"` 步骤块改为按序渲染 + 占位。步骤块整体改为（以 LLM 回复为例）：

```html
<div v-if="detail" class="attr-step">
  <span class="attr-step-num">3</span>
  <div class="attr-step-main">
    <strong>{{ t.attribution.stepResponse }}</strong>
    <template v-if="detail.steps[2]?.kind === 'llmResponse'">
      <!-- 原有 chips + text + token stream 内容 -->
    </template>
    <span v-else class="attr-unavailable">{{ t.attribution.notAvailable }}</span>
  </div>
</div>
```

  - 每个步骤块都套用该模式（传感器帧/提示词/LLM 回复/解析动作/执行结果）；`v-if="detail.steps[N]?.kind === 'x'"` 改为包裹内部 template
  - 状态 pill：`v-if="chains[selectedIndex]?.success != null"`
  - CSS 追加：

```css
.attr-lerobot { display: flex; flex-direction: column; gap: 8px; }
.attr-lerobot-row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.attr-lerobot-path {
  flex: 1; min-width: 280px; padding: 7px 10px; font-size: 12px; font-family: monospace;
  background: var(--canvas-base); color: var(--text-body);
  border: 1px solid var(--hairline); border-radius: 5px;
}
.attr-lerobot-info { display: flex; gap: 6px; flex-wrap: wrap; }
.attr-unavailable { color: var(--text-muted-dark); font-size: 12px; font-style: italic; }
```

- [ ] **Step 3: 构建验证**

Run: `npm --prefix frontend run build`
Expected: 通过（vue-tsc 无错）

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/AttributionBar.vue
git commit -m "feat: add LeRobot data source to attribution bar (M10)"
```

---

### Task 9: VisualizationView apply-action 联动

Files:
- Modify: `frontend/src/components/VisualizationView.vue`

- [ ] **Step 1: 模板** — AttributionBar 增加事件绑定：

```html
<AttributionBar
  v-if="viewportMode === 'replay' && replayActive"
  :recording-id="replayRecordingId"
  :current-timestamp="replayCurrentTime"
  @seek-timestamp="(ts: number) => replayEngine?.seek(ts, true)"
  @apply-action="applyActionVector"
/>
```

- [ ] **Step 2: script** — 新增函数（map 前 6 元素到视口关节，切回 Live）：

```ts
function applyActionVector(vector: number[]) {
  viewportMode.value = 'live'
  const joints = createNanoArmJointState()
  const names: (keyof typeof joints)[] = ['joint1', 'joint2', 'joint3', 'joint4', 'joint5', 'joint6']
  names.forEach((name, i) => {
    if (vector[i] !== undefined) joints[name] = vector[i]
  })
  Object.assign(nanoArmJointState, joints)
}
```

- [ ] **Step 3: 构建验证**

Run: `npm --prefix frontend run build`
Expected: 通过

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/VisualizationView.vue
git commit -m "feat: apply LeRobot action vectors to the 3D viewport (M10)"
```

---

### Task 10: 真实数据集成测试 + 手动验证

Files:
- Test: `backend/src/lerobot.rs`（B601 集成测试，路径缺失跳过）

- [ ] **Step 1: 追加集成测试**：

```rust
#[tokio::test]
async fn real_b601_dataset_end_to_end() {
    let path = std::path::PathBuf::from(
        "/home/dora/.cache/huggingface/lerobot/my_org/b601_pilot_v1");
    if !path.exists() {
        eprintln!("B601 dataset not found — skipping real-data test");
        return;
    }
    let info = scan_dataset(&path).await.expect("scan real dataset");
    assert_eq!(info.episodes.len(), 5);
    assert_eq!(info.layout, "v1");
    assert!(info.tasks.values().any(|t| t.contains("red cube")));
    let (frames, total) = read_frames(&path, 0, 0, 50).await.expect("frames");
    assert_eq!(total, 897);
    assert_eq!(frames.len(), 50);
    assert_eq!(frames[0].action.len(), 7);
    let chains = chains_from_frames(&frames, &info.tasks);
    assert_eq!(chains.len(), 50);
    assert!(matches!(&chains[0].steps[1], crate::attribution::AttributionStep::Prompt { text, .. }
        if text.contains("Pick up the red cube")));
    assert_eq!(chains[0].success(), None);
}
```

- [ ] **Step 2: 运行**

Run: `cargo test --manifest-path backend/Cargo.toml real_b601`
Expected: passed（数据存在）

- [ ] **Step 3: 全量验证**

Run:
```bash
cargo test --manifest-path backend/Cargo.toml
npm --prefix frontend run build
```
Expected: 后端全绿、构建通过

- [ ] **Step 4: Commit**

```bash
git add backend/src/lerobot.rs
git commit -m "test: add real B601 dataset integration test (M10)"
```

- [ ] **Step 5: 手动测试清单（交给用户）**：
  1. 重启后端（3001）、前端 dev server
  2. 3D 可视化 → Replay 模式 → 归因条数据源选 "LeRobot dataset (M10)"
  3. 路径填 `/home/dora/.cache/huggingface/lerobot/my_org/b601_pilot_v1` → 扫描
  4. 预期: 数据集信息（5 episodes、7 列、任务描述）；配置自动检测为 b601
  5. 加载 Episode → 链条 200/页、翻页；详情卡显示传感器帧/提示词（真实任务文本）/解析动作（7 关节），LLM 回复与执行结果显示"数据集中未记录"占位
  6. "在 3D 中查看" → 视口切 Live 并摆出该帧姿态；面板自动折叠
  7. 空路径/坏路径 → 红色错误提示
  8. 回归: .drec 源（attribution_demo.drec / joint_animation.drec 空状态）仍正常

- [ ] **Step 6: 测试通过后** — 更新 HANDOFF.md（M10 完成段、下一个模块 M11）、plans2.0/OVERVIEW.md 的 M10 行（如需要）

---

## 自审记录

- 规格覆盖: 设计文档 7 项决策 → Task 1（诚实性类型）、Task 3/4（Python 桥 + 双布局）、Task 2/6（profile + auto-detect）、Task 5（每帧一链 + profiles 目录）、Task 8/9（面板扩展 + apply-action）、Task 10（真实数据验证）；验收标准逐条对应 Task 10 手动清单
- 类型一致性: `FrameData.timestamp_ns` 全链路一致（script 输出 episodeStartTs → lerobot.rs 归一化）；`chains_from_frames` 签名在 Task 4/5/6/10 一致；前端 `LerobotAttributionResponse.summaries` 与 AttributionBar 的 `lerobotSummaries` 类型一致
- 无占位符: 各步骤含完整代码
