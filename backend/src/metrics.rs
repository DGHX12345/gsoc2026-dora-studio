//! Metrics collector — polls dora coordinator for per-node resource usage.
//!
//! Data source: `dora node list --format json` (NDJSON per-node cpu/memory/status).
//! Topic-level metrics (latency, queue depth) also available via `dora topic hz/info`
//! when a dataflow has `_unstable_debug.publish_all_messages_to_zenoh: true`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single sample of node-level metrics at a point in time.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeMetricSample {
    pub timestamp_secs: u64,
    pub cpu_percent: f32,
    pub memory_mb: f64,
    pub status: String,
    pub restart_count: u32,
    pub pid: Option<u32>,
}

/// Aggregated metrics for one node, including ring buffer history.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeMetricSummary {
    pub node_id: String,
    pub dataflow_name: Option<String>,
    pub current: NodeMetricSample,
    pub history: Vec<NodeMetricSample>,
}

// ---------------------------------------------------------------------------
// Collector (stub — implementation after tests)
// ---------------------------------------------------------------------------

/// Shared metrics collector, updated by a background poll task.
#[derive(Clone)]
pub struct MetricsCollector {
    inner: Arc<RwLock<MetricsInner>>,
}

struct MetricsInner {
    nodes: HashMap<String, NodeMetricSummary>,
    poll_interval: Duration,
    ring_buffer_capacity: usize,
}

impl MetricsCollector {
    pub fn new(poll_interval: Duration) -> Self {
        Self {
            inner: Arc::new(RwLock::new(MetricsInner {
                nodes: HashMap::new(),
                poll_interval,
                ring_buffer_capacity: 300,
            })),
        }
    }

    /// Spawns a background task that polls `dora node list --format json`.
    pub fn start(&self) {
        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(inner.read().await.poll_interval);
            loop {
                interval.tick().await;
                match poll_dora_node_list().await {
                    Ok(samples) => {
                        let mut guard = inner.write().await;
                        let now = unix_timestamp();
                        let cap = guard.ring_buffer_capacity;
                        for (node_id, entry) in &samples {
                            let sample = entry_to_sample(entry, now);
                            guard.update_node(node_id, entry.dataflow.clone(), sample, cap);
                        }
                        // Remove nodes that disappeared
                        let known: Vec<String> = samples.keys().cloned().collect();
                        guard.nodes.retain(|id, _| known.contains(id));
                    }
                    Err(e) => {
                        eprintln!("metrics poll failed: {e}");
                    }
                }
            }
        });
    }

    /// Returns current metrics for all known nodes.
    pub async fn nodes_summary(&self) -> Vec<NodeMetricSummary> {
        let inner = self.inner.read().await;
        inner.nodes.values().cloned().collect()
    }

    /// Returns the history for a single node, optionally windowed.
    pub async fn node_history(
        &self,
        node_id: &str,
        window_secs: Option<u64>,
    ) -> Option<Vec<NodeMetricSample>> {
        let inner = self.inner.read().await;
        let summary = inner.nodes.get(node_id)?;
        if let Some(window) = window_secs {
            let cutoff = summary.current.timestamp_secs.saturating_sub(window);
            Some(
                summary
                    .history
                    .iter()
                    .filter(|s| s.timestamp_secs >= cutoff)
                    .cloned()
                    .collect(),
            )
        } else {
            Some(summary.history.clone())
        }
    }
}

impl MetricsInner {
    fn update_node(
        &mut self,
        node_id: &str,
        dataflow_name: Option<String>,
        sample: NodeMetricSample,
        capacity: usize,
    ) {
        let summary = self
            .nodes
            .entry(node_id.to_string())
            .or_insert_with(|| NodeMetricSummary {
                node_id: node_id.to_string(),
                dataflow_name: None,
                current: sample.clone(),
                history: Vec::new(),
            });
        summary.dataflow_name = dataflow_name;
        summary.current = sample.clone();
        if summary.history.len() >= capacity {
            summary.history.remove(0);
        }
        summary.history.push(sample);
    }
}

/// Runs `dora node list --format json` and returns parsed entries keyed by node_id.
///
/// Returns `Ok(empty)` when the coordinator is not reachable (normal idle state).
/// Returns `Err` only for unexpected failures (dora binary missing, etc.).
async fn poll_dora_node_list() -> Result<HashMap<String, NodeListEntry>, String> {
    let output = tokio::process::Command::new("dora")
        .args(["node", "list", "--format", "json"])
        .output()
        .await
        .map_err(|e| format!("failed to spawn dora: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        // `dora node` subcommand not available in this dora version
        if stderr.contains("unrecognized subcommand") {
            return Ok(HashMap::new());
        }

        // Coordinator not running — normal idle state, not an error
        if stderr.contains("Connection refused") || stderr.contains("failed to connect") {
            return Ok(HashMap::new());
        }

        return Err(format!("dora node list failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let entries = parse_node_list_json(&stdout);
    let map: HashMap<String, NodeListEntry> =
        entries.into_iter().map(|e| (e.node.clone(), e)).collect();
    Ok(map)
}

fn unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ---------------------------------------------------------------------------
// Parsing — `dora node list --format json` (NDJSON)
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct NodeListEntry {
    node: String,
    status: String,
    pid: String,
    cpu: String,
    memory: String,
    restarts: String,
    dataflow: Option<String>,
}

fn parse_node_list_json(input: &str) -> Vec<NodeListEntry> {
    input
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            serde_json::from_str(line).ok()
        })
        .collect()
}

fn parse_cpu_percent(raw: &str) -> Option<f32> {
    raw.trim().trim_end_matches('%').parse::<f32>().ok()
}

fn parse_memory_mb(raw: &str) -> Option<f64> {
    let raw = raw.trim().trim_end_matches(" MB");
    raw.parse::<f64>().ok()
}

fn parse_pid(raw: &str) -> Option<u32> {
    raw.trim().parse::<u32>().ok()
}

fn parse_restarts(raw: &str) -> Option<u32> {
    raw.trim().parse::<u32>().ok()
}

fn entry_to_sample(entry: &NodeListEntry, timestamp_secs: u64) -> NodeMetricSample {
    NodeMetricSample {
        timestamp_secs,
        cpu_percent: parse_cpu_percent(&entry.cpu).unwrap_or(0.0),
        memory_mb: parse_memory_mb(&entry.memory).unwrap_or(0.0),
        status: entry.status.clone(),
        restart_count: parse_restarts(&entry.restarts).unwrap_or(0),
        pid: parse_pid(&entry.pid),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const NODE_LIST_JSON_3_NODES: &str = r#"{"cpu":"12.5%","memory":"256 MB","node":"019abc12-3456-7890-abcd-ef0123456789","pid":"12345","restarts":"0","status":"Running","dataflow":"camera-pipeline"}
{"cpu":"3.2%","memory":"128 MB","node":"019def34-5678-90ab-cdef-012345678901","pid":"12346","restarts":"2","status":"Running","dataflow":"camera-pipeline"}
{"cpu":"45.1%","memory":"1024 MB","node":"01956789-0abc-def0-1234-567890abcdef","pid":"12347","restarts":"0","status":"Running","dataflow":"lidar-processing"}
"#;

    const NODE_LIST_JSON_NO_METRICS: &str = r#"{"node":"019abc12-3456-7890-abcd-ef0123456789","status":"Unknown","pid":"-","cpu":"-","memory":"-","restarts":"-","dataflow":"test-flow"}
"#;

    // -- NDJSON parsing --

    #[test]
    fn parse_three_nodes() {
        let entries = parse_node_list_json(NODE_LIST_JSON_3_NODES);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].node, "019abc12-3456-7890-abcd-ef0123456789");
        assert_eq!(entries[0].cpu, "12.5%");
        assert_eq!(entries[0].memory, "256 MB");
        assert_eq!(entries[0].status, "Running");
        assert_eq!(entries[0].pid, "12345");
        assert_eq!(entries[1].node, "019def34-5678-90ab-cdef-012345678901");
        assert_eq!(entries[1].restarts, "2");
    }

    #[test]
    fn parse_no_metrics_node() {
        let entries = parse_node_list_json(NODE_LIST_JSON_NO_METRICS);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].status, "Unknown");
        assert_eq!(entries[0].cpu, "-");
        assert_eq!(entries[0].memory, "-");
    }

    #[test]
    fn parse_empty_output() {
        let entries = parse_node_list_json("");
        assert_eq!(entries.len(), 0);
    }

    // -- Field parsing --

    #[test]
    fn parse_valid_cpu() {
        assert_eq!(parse_cpu_percent("12.5%"), Some(12.5));
        assert_eq!(parse_cpu_percent("0.0%"), Some(0.0));
        assert_eq!(parse_cpu_percent("100%"), Some(100.0));
    }

    #[test]
    fn parse_invalid_cpu_is_none() {
        assert_eq!(parse_cpu_percent("-"), None);
        assert_eq!(parse_cpu_percent("N/A"), None);
    }

    #[test]
    fn parse_valid_memory() {
        assert_eq!(parse_memory_mb("256 MB"), Some(256.0));
        assert_eq!(parse_memory_mb("1024 MB"), Some(1024.0));
    }

    #[test]
    fn parse_invalid_memory_is_none() {
        assert_eq!(parse_memory_mb("-"), None);
    }

    #[test]
    fn parse_valid_pid() {
        assert_eq!(parse_pid("12345"), Some(12345));
    }

    #[test]
    fn parse_invalid_pid_is_none() {
        assert_eq!(parse_pid("-"), None);
    }

    // -- Sample conversion --

    #[test]
    fn entry_to_sample_converts_fields() {
        let entry = NodeListEntry {
            node: "n1".into(),
            status: "Running".into(),
            pid: "123".into(),
            cpu: "25.5%".into(),
            memory: "512 MB".into(),
            restarts: "3".into(),
            dataflow: Some("flow-a".into()),
        };
        let sample = entry_to_sample(&entry, 1000);
        assert_eq!(sample.timestamp_secs, 1000);
        assert_eq!(sample.cpu_percent, 25.5);
        assert_eq!(sample.memory_mb, 512.0);
        assert_eq!(sample.status, "Running");
        assert_eq!(sample.restart_count, 3);
        assert_eq!(sample.pid, Some(123));
    }

    #[test]
    fn entry_to_sample_missing_metrics_uses_defaults() {
        let entry = NodeListEntry {
            node: "n2".into(),
            status: "Unknown".into(),
            pid: "-".into(),
            cpu: "-".into(),
            memory: "-".into(),
            restarts: "-".into(),
            dataflow: None,
        };
        let sample = entry_to_sample(&entry, 2000);
        assert_eq!(sample.cpu_percent, 0.0);
        assert_eq!(sample.memory_mb, 0.0);
        assert_eq!(sample.restart_count, 0);
        assert_eq!(sample.pid, None);
        assert_eq!(sample.status, "Unknown");
    }

    // -- Ring buffer --

    #[test]
    fn history_capacity_trims_oldest() {
        let mut summary = NodeMetricSummary {
            node_id: "n1".into(),
            dataflow_name: None,
            current: NodeMetricSample {
                timestamp_secs: 0,
                cpu_percent: 0.0,
                memory_mb: 0.0,
                status: "Unknown".into(),
                restart_count: 0,
                pid: None,
            },
            history: Vec::new(),
        };

        // Simulate push logic with capacity 3
        let cap = 3;
        for i in 0..5 {
            let sample = NodeMetricSample {
                timestamp_secs: i,
                cpu_percent: i as f32,
                memory_mb: 0.0,
                status: "Running".into(),
                restart_count: 0,
                pid: None,
            };
            summary.current = sample.clone();
            if summary.history.len() >= cap {
                summary.history.remove(0);
            }
            summary.history.push(sample);
        }

        assert_eq!(summary.history.len(), 3);
        assert_eq!(summary.history[0].timestamp_secs, 2);
        assert_eq!(summary.history[2].timestamp_secs, 4);
    }

    #[test]
    fn update_node_creates_and_appends() {
        let mut inner = MetricsInner {
            nodes: HashMap::new(),
            poll_interval: Duration::from_secs(2),
            ring_buffer_capacity: 3,
        };

        let sample1 = NodeMetricSample {
            timestamp_secs: 100,
            cpu_percent: 10.0,
            memory_mb: 100.0,
            status: "Running".into(),
            restart_count: 0,
            pid: Some(1),
        };
        inner.update_node("n1", Some("f1".into()), sample1, 3);

        let sample2 = NodeMetricSample {
            timestamp_secs: 101,
            cpu_percent: 20.0,
            memory_mb: 200.0,
            status: "Running".into(),
            restart_count: 0,
            pid: Some(1),
        };
        inner.update_node("n1", Some("f1".into()), sample2, 3);

        let summary = inner.nodes.get("n1").unwrap();
        assert_eq!(summary.history.len(), 2);
        assert_eq!(summary.current.cpu_percent, 20.0);
        assert_eq!(summary.history[0].timestamp_secs, 100);
        assert_eq!(summary.history[1].timestamp_secs, 101);
    }

    #[test]
    fn history_window_filters_by_cutoff() {
        let mut summary = NodeMetricSummary {
            node_id: "n1".into(),
            dataflow_name: None,
            current: NodeMetricSample {
                timestamp_secs: 0,
                cpu_percent: 0.0,
                memory_mb: 0.0,
                status: "Unknown".into(),
                restart_count: 0,
                pid: None,
            },
            history: Vec::new(),
        };

        for i in 0..10u64 {
            let sample = NodeMetricSample {
                timestamp_secs: i,
                cpu_percent: i as f32,
                memory_mb: 0.0,
                status: "Running".into(),
                restart_count: 0,
                pid: None,
            };
            summary.current = sample.clone();
            summary.history.push(sample);
        }

        let cutoff = summary.current.timestamp_secs.saturating_sub(5);
        let window: Vec<_> = summary
            .history
            .iter()
            .filter(|s| s.timestamp_secs >= cutoff)
            .cloned()
            .collect();

        assert!(window.len() >= 5);
        for s in &window {
            assert!(s.timestamp_secs >= cutoff);
        }
    }
}
