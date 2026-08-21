# M08: OTel Flame Graph

**Layer**: 3 — Performance Visualization
**Depends on**: M07 (metrics infrastructure)
**Effort**: 1 week

## Purpose

Dora integrates OpenTelemetry for tracing. Studio pulls OTel span data
and renders per-node flame graphs so developers can identify CPU hotspots
in their operator code.

## Deliverables

### D1: OTel span collector

Extend `backend/src/metrics.rs`:

- `OtelCollector` connects to dora's OTel export endpoint
- Parse OTLP JSON (OpenTelemetry Line Protocol) spans
- Build span tree: parent-child relationships by `span_id` / `parent_span_id`
- Group spans by node_id (operator name)
- Ring buffer: last 1000 spans per node
- If OTel endpoint unavailable, show "OTel not configured" with setup instructions

API endpoints:
- `GET /api/metrics/otel/spans?node=N&limit=L` — recent spans for a node
- `GET /api/metrics/otel/trace/:trace_id` — full trace tree for one trace

### D2: Flame graph component

New file: `frontend/src/components/FlameGraph.vue`

- Horizontal stacked bar chart representing call stack depth
- Each bar = one span: width proportional to duration, labeled with function name
- Color by: self-time (darker = more CPU), or by operator node
- Interaction:
  - Hover: show span details (name, duration, self-time, children count)
  - Click: zoom into that span (re-root flame graph at clicked span)
  - Click background: zoom out one level
  - Double-click: reset to root
- Search: text input highlights matching span names

### D3: Node grouping

- Sidebar: list of nodes with span counts, grouped by dataflow
- Click a node → flame graph filters to that node's spans
- "All Nodes" option for global view
- Color legend: node color mapping

### D4: Time range selector

- Slider or drag-select on a mini timeline above flame graph
- Only spans within selected time range are displayed
- "Last 30s" / "Last 5min" / "Last 1h" quick-select buttons
- Time range label: "Showing spans from HH:MM:SS to HH:MM:SS"

## Acceptance Criteria

- [ ] Correctly parses OTLP JSON spans from dora OTel endpoint
- [ ] Flame graph renders 1000+ spans without layout jank
- [ ] Click-to-zoom and click-to-reset navigation works
- [ ] Search highlights matching spans in <100ms
- [ ] Time range selector filters spans correctly
- [ ] Disconnected OTel endpoint shows clear status message, not error
- [ ] `cargo test` includes OTLP parsing test with sample span data

## Exposed Interfaces

```rust
// backend/src/metrics.rs (extended)
pub struct OtelCollector { ... }
impl OtelCollector {
    pub fn new(otel_endpoint: String) -> Self;
    pub async fn fetch_spans(&mut self) -> Result<Vec<OtelSpan>>;
    pub fn spans_for_node(&self, node: &str, limit: usize) -> Vec<OtelSpan>;
    pub fn trace_tree(&self, trace_id: &str) -> Option<SpanNode>;
}

pub struct OtelSpan {
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub trace_id: String,
    pub node_id: String,
    pub operation_name: String,
    pub start_nanos: u64,
    pub duration_nanos: u64,
    pub attributes: HashMap<String, String>,
}
```

```typescript
// frontend/src/components/FlameGraph.vue
// Props:
//   spans: OtelSpan[]
//   searchQuery: string
//   timeRange: [number, number]
// Events:
//   @span-clicked(spanId: string)
//   @zoom-changed(depth: number)
```
