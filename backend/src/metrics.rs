//! Metrics collector — polls dora coordinator for per-node resource usage.
//!
//! Data source: `dora node list --format json` (NDJSON per-node cpu/memory/status).
//! Topic-level metrics (latency, queue depth) also available via `dora topic hz/info`
//! when a dataflow has `_unstable_debug.publish_all_messages_to_zenoh: true`.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{watch, RwLock};

use crate::coordinator_ws::{status_to_string, CoordinatorWsClient};
use crate::protocol::types::NodeInfo;

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

/// Unified per-node snapshot produced by any poll source (R6): the CLI NDJSON
/// parser and the WS GetNodeInfo mapper both produce this shape, so the poll
/// loop is independent of the data source.
#[derive(Debug, Clone)]
pub struct PolledNode {
    pub node_id: String,
    pub dataflow: Option<String>,
    pub cpu_percent: f32,
    pub memory_mb: f64,
    pub status: String,
    pub restart_count: u32,
    pub pid: Option<u32>,
}

// ---------------------------------------------------------------------------
// Collector
// ---------------------------------------------------------------------------

/// Shared metrics collector, updated by a background poll task.
///
/// Starts stopped (M11.5 D1): monitoring is opt-in, so nothing polls until
/// `start()` is called.
#[derive(Clone)]
pub struct MetricsCollector {
    inner: Arc<RwLock<MetricsInner>>,
    running: watch::Sender<bool>,
    join_handle: Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    source: MetricsSource,
}

struct MetricsInner {
    nodes: HashMap<String, NodeMetricSummary>,
    poll_interval: Duration,
    ring_buffer_capacity: usize,
    sample_count: u64,
    last_poll_at: Option<u64>,
    source: Option<String>,
}

/// Which data source the collector polls (M11.5 D2).
#[derive(Clone)]
pub enum MetricsSource {
    Cli,
    Ws { client: CoordinatorWsClient },
}

impl MetricsCollector {
    pub fn new(poll_interval: Duration) -> Self {
        Self::new_with_source(poll_interval, MetricsSource::Cli)
    }

    pub fn new_with_ws(poll_interval: Duration, client: CoordinatorWsClient) -> Self {
        Self::new_with_source(poll_interval, MetricsSource::Ws { client })
    }

    fn new_with_source(poll_interval: Duration, source: MetricsSource) -> Self {
        Self {
            inner: Arc::new(RwLock::new(MetricsInner {
                nodes: HashMap::new(),
                poll_interval,
                ring_buffer_capacity: 300,
                sample_count: 0,
                last_poll_at: None,
                source: None,
            })),
            running: watch::channel(false).0,
            join_handle: Arc::new(std::sync::Mutex::new(None)),
            source,
        }
    }

    /// Spawns the background poll task unless it is already running.
    pub fn start(&self) {
        if *self.running.borrow() {
            return;
        }
        let rx = self.running.subscribe();
        self.running.send_replace(true);

        let inner = Arc::clone(&self.inner);
        let handle = match &self.source {
            MetricsSource::Cli => tokio::spawn(async move {
                poll_loop(inner, rx, || async {
                    poll_cli_source()
                        .await
                        .map(|nodes| (nodes, "cli".to_string()))
                })
                .await;
            }),
            MetricsSource::Ws { client } => {
                let client = client.clone();
                tokio::spawn(async move {
                    poll_loop(inner, rx, move || {
                        let client = client.clone();
                        async move { poll_ws_with_cli_fallback(&client).await }
                    })
                    .await;
                })
            }
        };
        *self.join_handle.lock().unwrap() = Some(handle);
    }

    /// Signals the poll task to exit; the task stops at the next tick.
    pub fn stop(&self) {
        self.running.send_replace(false);
    }

    /// Waits for the poll task to fully exit after `stop()`.
    pub async fn join(&self) {
        if let Some(handle) = self.join_handle.lock().unwrap().take() {
            let _ = handle.await;
        }
    }

    pub fn is_running(&self) -> bool {
        *self.running.borrow()
    }

    /// Monitoring status: enabled flag, active source, poll attempt count,
    /// last poll time.
    pub async fn status(&self) -> serde_json::Value {
        let inner = self.inner.read().await;
        serde_json::json!({
            "enabled": *self.running.borrow(),
            "source": inner.source,
            "sampleCount": inner.sample_count,
            "lastPollAt": inner.last_poll_at,
        })
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

/// Polls `poll` on the configured interval until the watch channel flips to
/// `false` (or the sender is dropped). Errors are logged, not fatal.
async fn poll_loop<F, Fut>(
    inner: Arc<RwLock<MetricsInner>>,
    mut rx: watch::Receiver<bool>,
    mut poll: F,
) where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<(Vec<PolledNode>, String), String>>,
{
    let poll_interval = inner.read().await.poll_interval;
    let mut interval = tokio::time::interval(poll_interval);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let result = poll().await;
                let mut guard = inner.write().await;
                guard.sample_count += 1;
                guard.last_poll_at = Some(unix_timestamp());
                match result {
                    Ok((nodes, source)) => {
                        guard.source = Some(source);
                        let cap = guard.ring_buffer_capacity;
                        let now = unix_timestamp();
                        let known: HashSet<String> =
                            nodes.iter().map(|n| n.node_id.clone()).collect();
                        for node in nodes {
                            guard.update_node(node, now, cap);
                        }
                        // Remove nodes that disappeared
                        guard.nodes.retain(|id, _| known.contains(id));
                    }
                    Err(e) => {
                        eprintln!("metrics poll failed: {e}");
                    }
                }
            }
            changed = rx.changed() => {
                if changed.is_err() || !*rx.borrow() {
                    break;
                }
            }
        }
    }
}

impl MetricsInner {
    fn update_node(&mut self, node: PolledNode, timestamp_secs: u64, capacity: usize) {
        let sample = NodeMetricSample {
            timestamp_secs,
            cpu_percent: node.cpu_percent,
            memory_mb: node.memory_mb,
            status: node.status,
            restart_count: node.restart_count,
            pid: node.pid,
        };
        let summary = self
            .nodes
            .entry(node.node_id.clone())
            .or_insert_with(|| NodeMetricSummary {
                node_id: node.node_id,
                dataflow_name: None,
                current: sample.clone(),
                history: Vec::new(),
            });
        summary.dataflow_name = node.dataflow;
        summary.current = sample.clone();
        if summary.history.len() >= capacity {
            summary.history.remove(0);
        }
        summary.history.push(sample);
    }
}

/// Runs `dora node list --format json` and returns parsed entries.
///
/// Returns `Ok(empty)` when the coordinator is not reachable (normal idle state).
/// Returns `Err` only for unexpected failures (dora binary missing, etc.).
async fn poll_cli_source() -> Result<Vec<PolledNode>, String> {
    let output = tokio::process::Command::new(crate::dora_env::resolve_dora_bin())
        .args(["node", "list", "--format", "json"])
        .output()
        .await
        .map_err(|e| format!("failed to spawn dora: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        // `dora node` subcommand not available in this dora version
        if stderr.contains("unrecognized subcommand") {
            return Ok(Vec::new());
        }

        // Coordinator not running — normal idle state, not an error
        if stderr.contains("Connection refused") || stderr.contains("failed to connect") {
            return Ok(Vec::new());
        }

        return Err(format!("dora node list failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_node_list_json(&stdout)
        .iter()
        .map(entry_to_polled)
        .collect())
}

// ---------------------------------------------------------------------------
// WS data source (M11.5 D2)
// ---------------------------------------------------------------------------

/// Maps a coordinator `NodeInfo` (GetNodeInfo over WS) to the unified
/// `PolledNode` shape — field-aligned with the CLI NDJSON parser.
fn info_to_polled(info: &NodeInfo) -> PolledNode {
    let metrics = info.metrics.as_ref();
    PolledNode {
        node_id: info.node_id.clone(),
        dataflow: info.dataflow_name.clone(),
        cpu_percent: metrics.map(|m| m.cpu_usage).unwrap_or(0.0),
        memory_mb: metrics.map(|m| m.memory_mb).unwrap_or(0.0),
        status: status_to_string(metrics.map(|m| &m.status)),
        restart_count: metrics.map(|m| m.restart_count).unwrap_or(0),
        pid: metrics.map(|m| m.pid),
    }
}

/// Fetches node metrics over the coordinator WebSocket.
async fn poll_ws_source(client: &CoordinatorWsClient) -> Result<Vec<PolledNode>, String> {
    let infos = client.all_node_infos().await?;
    Ok(infos.iter().map(info_to_polled).collect())
}

/// WS-first poll with CLI fallback (R3): any WS failure — including a dead
/// connection timing out — degrades to `dora node list` for that attempt.
async fn poll_ws_with_cli_fallback(
    client: &CoordinatorWsClient,
) -> Result<(Vec<PolledNode>, String), String> {
    match poll_ws_source(client).await {
        Ok(nodes) => Ok((nodes, "ws".to_string())),
        Err(_) => Ok((poll_cli_source().await?, "cli".to_string())),
    }
}

pub(crate) fn unix_timestamp() -> u64 {
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

fn entry_to_polled(entry: &NodeListEntry) -> PolledNode {
    PolledNode {
        node_id: entry.node.clone(),
        dataflow: entry.dataflow.clone(),
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
    fn entry_to_polled_converts_fields() {
        let entry = NodeListEntry {
            node: "n1".into(),
            status: "Running".into(),
            pid: "123".into(),
            cpu: "25.5%".into(),
            memory: "512 MB".into(),
            restarts: "3".into(),
            dataflow: Some("flow-a".into()),
        };
        let node = entry_to_polled(&entry);
        assert_eq!(node.node_id, "n1");
        assert_eq!(node.cpu_percent, 25.5);
        assert_eq!(node.memory_mb, 512.0);
        assert_eq!(node.status, "Running");
        assert_eq!(node.restart_count, 3);
        assert_eq!(node.pid, Some(123));
        assert_eq!(node.dataflow.as_deref(), Some("flow-a"));
    }

    #[test]
    fn entry_to_polled_missing_metrics_uses_defaults() {
        let entry = NodeListEntry {
            node: "n2".into(),
            status: "Unknown".into(),
            pid: "-".into(),
            cpu: "-".into(),
            memory: "-".into(),
            restarts: "-".into(),
            dataflow: None,
        };
        let node = entry_to_polled(&entry);
        assert_eq!(node.cpu_percent, 0.0);
        assert_eq!(node.memory_mb, 0.0);
        assert_eq!(node.restart_count, 0);
        assert_eq!(node.pid, None);
        assert_eq!(node.status, "Unknown");
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
            sample_count: 0,
            last_poll_at: None,
            source: None,
        };

        let node1 = PolledNode {
            node_id: "n1".into(),
            dataflow: Some("f1".into()),
            cpu_percent: 10.0,
            memory_mb: 100.0,
            status: "Running".into(),
            restart_count: 0,
            pid: Some(1),
        };
        inner.update_node(node1, 100, 3);

        let node2 = PolledNode {
            node_id: "n1".into(),
            dataflow: Some("f1".into()),
            cpu_percent: 20.0,
            memory_mb: 200.0,
            status: "Running".into(),
            restart_count: 0,
            pid: Some(1),
        };
        inner.update_node(node2, 101, 3);

        let summary = inner.nodes.get("n1").unwrap();
        assert_eq!(summary.history.len(), 2);
        assert_eq!(summary.current.cpu_percent, 20.0);
        assert_eq!(summary.history[0].timestamp_secs, 100);
        assert_eq!(summary.history[1].timestamp_secs, 101);
    }

    // -- Poll loop lifecycle (M11.5) --

    fn stub_inner(interval: Duration) -> Arc<RwLock<MetricsInner>> {
        Arc::new(RwLock::new(MetricsInner {
            nodes: HashMap::new(),
            poll_interval: interval,
            ring_buffer_capacity: 300,
            sample_count: 0,
            last_poll_at: None,
            source: None,
        }))
    }

    fn stub_node(id: &str) -> PolledNode {
        PolledNode {
            node_id: id.to_string(),
            dataflow: None,
            cpu_percent: 1.0,
            memory_mb: 2.0,
            status: "Running".into(),
            restart_count: 0,
            pid: None,
        }
    }

    #[tokio::test]
    async fn poll_loop_counts_attempts_and_applies_samples() {
        let inner = stub_inner(Duration::from_millis(5));
        let (tx, rx) = tokio::sync::watch::channel(true);

        let task = tokio::spawn(poll_loop(Arc::clone(&inner), rx, || async {
            Ok((vec![stub_node("n1")], "stub".to_string()))
        }));

        tokio::time::sleep(Duration::from_millis(40)).await;
        tx.send(false).unwrap();
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();

        let guard = inner.read().await;
        assert!(guard.sample_count >= 2);
        assert!(guard.last_poll_at.is_some());
        assert!(guard.nodes.contains_key("n1"));
    }

    #[tokio::test]
    async fn poll_loop_errors_do_not_stop_the_loop() {
        let inner = stub_inner(Duration::from_millis(5));
        let (tx, rx) = tokio::sync::watch::channel(true);

        let task = tokio::spawn(poll_loop(Arc::clone(&inner), rx, || async {
            Err("boom".to_string())
        }));

        tokio::time::sleep(Duration::from_millis(40)).await;
        tx.send(false).unwrap();
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();

        assert!(inner.read().await.sample_count >= 2);
    }

    // -- WS data source (M11.5 D2) --

    fn ws_info(
        metrics: Option<crate::protocol::types::NodeMetricsInfo>,
    ) -> crate::protocol::types::NodeInfo {
        crate::protocol::types::NodeInfo {
            dataflow_id: uuid::Uuid::nil(),
            dataflow_name: Some("flow-ws".into()),
            node_id: "node-ws".into(),
            daemon_id: "daemon-1".into(),
            metrics,
            network: None,
        }
    }

    #[test]
    fn info_to_polled_maps_ws_metrics_fields() {
        use crate::protocol::types::{NodeMetricsInfo, NodeStatus};

        let info = ws_info(Some(NodeMetricsInfo {
            pid: 4242,
            cpu_usage: 33.5,
            memory_mb: 512.0,
            disk_read_mb_s: None,
            disk_write_mb_s: None,
            restart_count: 2,
            broken_inputs: vec![],
            status: NodeStatus::Running,
            pending_messages: 7,
        }));

        let node = info_to_polled(&info);
        assert_eq!(node.node_id, "node-ws");
        assert_eq!(node.dataflow.as_deref(), Some("flow-ws"));
        assert_eq!(node.cpu_percent, 33.5);
        assert_eq!(node.memory_mb, 512.0);
        assert_eq!(node.status, "running");
        assert_eq!(node.restart_count, 2);
        assert_eq!(node.pid, Some(4242));
    }

    #[test]
    fn info_to_polled_handles_missing_metrics() {
        let info = ws_info(None);

        let node = info_to_polled(&info);
        assert_eq!(node.node_id, "node-ws");
        assert_eq!(node.cpu_percent, 0.0);
        assert_eq!(node.memory_mb, 0.0);
        assert_eq!(node.status, "unknown");
        assert_eq!(node.restart_count, 0);
        assert_eq!(node.pid, None);
    }

    #[tokio::test]
    async fn ws_source_with_unconnected_client_falls_back_to_cli() {
        // This test spawns the resolved dora binary, so it must share the
        // env/settings lock with the dora_env tests (parallel runs would
        // otherwise read a transient temp settings path).
        let _lock = crate::dora_env::TEST_ENV_LOCK.lock().unwrap();
        // A never-connected WS client fails fast, so the source must fall
        // back to the CLI poller and report "cli" as the active source.
        let client = crate::coordinator_ws::CoordinatorWsClient::new();

        let (nodes, source) = poll_ws_with_cli_fallback(&client).await.unwrap();

        assert_eq!(source, "cli");
        // `dora node list` with no coordinator returns an empty list
        assert!(nodes.is_empty());
    }

    #[tokio::test]
    async fn collector_start_stop_toggles_running_and_records_stats() {
        let collector = MetricsCollector::new(Duration::from_millis(10));
        assert!(!collector.is_running());

        collector.start();
        assert!(collector.is_running());
        // starting again must not spawn a second loop
        collector.start();

        // `dora node list` takes ~0.5s per invocation, so wait for at least
        // one full poll cycle.
        tokio::time::sleep(Duration::from_millis(1200)).await;
        collector.stop();
        assert!(!collector.is_running());

        let status = collector.status().await;
        assert_eq!(status["enabled"], false);
        assert!(status["sampleCount"].as_u64().unwrap() >= 1);
        assert!(status["lastPollAt"].is_u64());

        // Wait for the poll task to fully exit, then the count must freeze.
        collector.join().await;
        let frozen = collector.status().await["sampleCount"].as_u64().unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        let after = collector.status().await;
        assert_eq!(after["sampleCount"].as_u64().unwrap(), frozen);
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
