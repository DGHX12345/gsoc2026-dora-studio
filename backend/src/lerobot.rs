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
    cmd.arg(script_path())
        .args(args)
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
    match run_script(&[
        "scan".to_string(),
        "/tmp/nonexistent-dora-studio-check".to_string(),
    ])
    .await
    {
        Err(e) if e.contains("pyarrow") => LerobotStatus {
            python_available: true,
            pyarrow_available: false,
            message: e,
        },
        Err(e) if e.contains("python3") => LerobotStatus {
            python_available: false,
            pyarrow_available: false,
            message: e,
        },
        // 路径不存在 → 脚本在 pyarrow 检查后才报数据缺失 = pyarrow OK
        _ => LerobotStatus {
            python_available: true,
            pyarrow_available: true,
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
        episodes: raw
            .episodes
            .into_iter()
            .map(|e| EpisodeInfo {
                index: e.index,
                rows: e.rows,
                start_ns: (e.start_ts * 1e9) as u64,
                end_ns: (e.end_ts * 1e9) as u64,
            })
            .collect(),
        tasks: raw.tasks,
        has_image_columns: raw.has_image_columns,
    })
}

pub async fn read_frames(
    path: &Path,
    episode: u32,
    offset: usize,
    limit: usize,
) -> Result<(Vec<FrameData>, usize), String> {
    let out = run_script(&[
        "frames".to_string(),
        path.to_string_lossy().to_string(),
        episode.to_string(),
        offset.to_string(),
        limit.to_string(),
    ])
    .await?;
    let raw: RawFramesResponse =
        serde_json::from_str(&out).map_err(|e| format!("bad frames response: {e}"))?;
    let frames = raw
        .frames
        .into_iter()
        .map(|f| FrameData {
            frame_index: f.frame_index,
            timestamp_ns: (((f.timestamp - raw.episode_start_ts).max(0.0)) * 1e9) as u64,
            task_index: f.task_index,
            action: f.action,
            state: f.state,
        })
        .collect();
    Ok((frames, raw.total))
}

/// 每帧一条归因链（缺失步骤不生成，UI 显示占位）。
pub fn chains_from_frames(
    frames: &[FrameData],
    tasks: &BTreeMap<u32, String>,
) -> Vec<AttributionChain> {
    frames
        .iter()
        .map(|f| {
            let task_text = f
                .task_index
                .and_then(|t| tasks.get(&t))
                .cloned()
                .unwrap_or_else(|| {
                    format!(
                        "Task {}",
                        f.task_index
                            .map(|t| t.to_string())
                            .unwrap_or_else(|| "?".to_string())
                    )
                });
            let steps = vec![
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
            AttributionChain {
                timestamp_nanos: f.timestamp_ns,
                steps,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
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
                .arg(&script)
                .arg("gen-demo")
                .arg(&dir)
                .arg(layout)
                .output()
                .await
                .ok()?;
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
        let err = scan_dataset(&PathBuf::from("/tmp/does-not-exist-xyz"))
            .await
            .unwrap_err();
        assert!(
            err.contains("no LeRobot parquet data") || err.contains("error"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn chains_from_frames_maps_steps_and_omits_unavailable() {
        use crate::attribution::AttributionStep;
        let frames = vec![FrameData {
            frame_index: 0,
            timestamp_ns: 33_333_333,
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
        use crate::attribution::AttributionStep;
        let frames = vec![FrameData {
            frame_index: 0,
            timestamp_ns: 0,
            task_index: Some(7),
            action: vec![],
            state: vec![],
        }];
        let chains = chains_from_frames(&frames, &BTreeMap::new());
        assert!(matches!(&chains[0].steps[1], AttributionStep::Prompt { text, .. } if text == "Task 7"));
    }

    #[tokio::test]
    async fn real_b601_dataset_end_to_end() {
        let path = std::path::PathBuf::from(
            "/home/dora/.cache/huggingface/lerobot/my_org/b601_pilot_v1",
        );
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
}
