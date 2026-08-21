# M07: Performance Metrics Panels

**Layer**: 3 — Performance Visualization
**Depends on**: M03 (runtime status), M00 (protocol types)
**Effort**: 1.5 weeks

## Purpose

Visualize per-topic performance metrics: end-to-end latency, shared-memory
queue depth, and frame drop rate. Data comes from dora coordinator metrics
(initially via `dora top --once` JSON, with option to upgrade to mmap later).

## Deliverables

### D1: Metrics collector

New file: `backend/src/metrics.rs`

- `MetricsCollector` polls coordinator for per-topic metrics
- Data source: parse `dora top --once --format json` output (Scheme A)
- Poll interval: configurable, default 2s
- Ring buffer: keep last 300 samples per topic (10 minutes at 2s)
- Metrics per topic:
  - `latency_us: Vec<u64>` — end-to-end producer→consumer latency samples
  - `queue_depth: u32` — current shared memory ring buffer fill level
  - `queue_capacity: u32` — total ring buffer capacity
  - `frames_produced: u64` — cumulative counter
  - `frames_consumed: u64` — cumulative counter
  - `frame_drop_rate: f32` — (produced - consumed) / produced

API endpoints:
- `GET /api/metrics/topics` — list of topics with current metrics
- `GET /api/metrics/topic/:name/history?window=S` — time series for one topic (last S seconds)

### D2: Latency line chart

New file: `frontend/src/components/LatencyChart.vue`

- HTML5 Canvas line chart (or lightweight chart library, no ECharts dependency)
- X axis: time (last N seconds, scrolling)
- Y axis: latency in microseconds
- Per-topic line, color-coded
- Hover tooltip: exact value at cursor
- Threshold line: configurable alert level (e.g., 1000µs), drawn as dashed red line
- Auto-scale Y axis; option to pin to fixed range

### D3: Queue depth gauge

New file: `frontend/src/components/QueueGauge.vue`

- Donut or bar gauge per topic showing queue_depth / queue_capacity
- Color zones: green (<50%), yellow (50-80%), red (>80%)
- Threshold alert: gauge border flashes yellow when >80% for >500ms
- Compact card layout: show all topics as small gauges in a grid

### D4: Frame drop rate panel

- Per-topic bar chart or sparkline showing drop rate over time
- Drop rate = (frames_produced - frames_consumed) / frames_produced * 100%
- Color: green (<1%), yellow (1-5%), red (>5%)
- Hover: show absolute numbers (produced, consumed, dropped)

### D5: Metrics dashboard page

New page or panel integrating all three visualizations:
- Top: summary bar — total topics, topics with alerts, overall health
- Middle: latency chart (large, spans full width)
- Bottom left: queue depth gauges grid
- Bottom right: frame drop rates

Update sidebar navigation to include "Performance" page.

## Acceptance Criteria

- [ ] Metrics collector parses `dora top --once --format json` without errors
- [ ] Latency chart refreshes at ≥10Hz with ≤100ms render time
- [ ] Queue depth gauge turns yellow within 500ms of crossing 80% threshold
- [ ] 20+ topics simultaneously monitored without UI lag
- [ ] Metrics persist across page navigation (ring buffer in backend state)
- [ ] `cargo test` includes metrics parsing test with sample dora top output

## Exposed Interfaces

```rust
// backend/src/metrics.rs
pub struct MetricsCollector { ... }
impl MetricsCollector {
    pub fn new(poll_interval: Duration) -> Self;
    pub async fn start(&mut self);
    pub fn topic_metrics(&self) -> Vec<TopicMetrics>;
    pub fn topic_history(&self, topic: &str, window: Duration) -> Vec<MetricsSample>;
}

pub struct TopicMetrics {
    pub topic_name: String,
    pub producer_node: String,
    pub consumer_nodes: Vec<String>,
    pub latency_p50_us: u64,
    pub latency_p99_us: u64,
    pub queue_depth: u32,
    pub queue_capacity: u32,
    pub frame_drop_rate: f32,
}

pub struct MetricsSample {
    pub timestamp: u64,
    pub latency_us: u64,
    pub queue_depth: u32,
    pub frames_produced: u64,
    pub frames_consumed: u64,
}
```
