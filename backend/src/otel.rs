//! OTel span collector — queries a Jaeger-compatible trace backend.
//!
//! dora exports OTel spans via OTLP gRPC to an external backend (Jaeger,
//! Tempo, or an OpenTelemetry Collector). Studio queries the backend's
//! REST API to fetch spans and build per-node flame graphs.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use tokio::sync::{watch, RwLock};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single span normalized from the Jaeger API JSON format.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OtelSpan {
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub trace_id: String,
    pub node_id: String,
    pub operation_name: String,
    pub start_micros: u64,
    pub duration_micros: u64,
    pub attributes: HashMap<String, String>,
}

/// A span tree node for flame graph rendering.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpanNode {
    pub span: OtelSpan,
    pub children: Vec<SpanNode>,
}

// ---------------------------------------------------------------------------
// Jaeger JSON parsing (v1 API)
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct JaegerResponse {
    data: Vec<JaegerTrace>,
}

#[derive(Debug, serde::Deserialize)]
struct JaegerTrace {
    #[serde(rename = "traceID")]
    trace_id: String,
    spans: Vec<JaegerSpan>,
    processes: HashMap<String, JaegerProcess>,
}

#[derive(Debug, serde::Deserialize)]
struct JaegerSpan {
    #[serde(rename = "spanID")]
    span_id: String,
    #[serde(rename = "operationName")]
    operation_name: String,
    references: Vec<JaegerReference>,
    #[serde(rename = "startTime")]
    start_time: u64,
    duration: u64,
    tags: Vec<JaegerTag>,
    #[serde(rename = "processID")]
    process_id: String,
}

#[derive(Debug, serde::Deserialize)]
struct JaegerReference {
    #[serde(rename = "refType")]
    ref_type: String,
    #[serde(rename = "spanID")]
    span_id: String,
    #[serde(rename = "traceID")]
    trace_id: String,
}

#[derive(Debug, serde::Deserialize)]
struct JaegerProcess {
    #[serde(rename = "serviceName")]
    service_name: String,
    tags: Vec<JaegerTag>,
}

#[derive(Debug, serde::Deserialize)]
struct JaegerTag {
    key: String,
    #[serde(rename = "type")]
    type_: String,
    value: serde_json::Value,
}

/// Parses a Jaeger v1 API `/api/traces` response into normalized spans.
fn parse_jaeger_response(input: &str) -> Result<Vec<OtelSpan>, String> {
    let response: JaegerResponse =
        serde_json::from_str(input).map_err(|e| format!("invalid Jaeger response: {e}"))?;

    let mut spans = Vec::new();
    for trace in &response.data {
        // Build process_id -> node_id map (serviceName is the node name in dora)
        let process_names: HashMap<&str, &str> = trace
            .processes
            .iter()
            .map(|(pid, p)| (pid.as_str(), p.service_name.as_str()))
            .collect();

        for span in &trace.spans {
            let parent_span_id = span
                .references
                .iter()
                .find(|r| r.ref_type == "CHILD_OF")
                .map(|r| r.span_id.clone());

            let node_id = process_names
                .get(span.process_id.as_str())
                .copied()
                .unwrap_or("unknown")
                .to_string();

            let mut attributes = HashMap::new();
            for tag in &span.tags {
                let value = match &tag.value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    _ => tag.value.to_string(),
                };
                attributes.insert(tag.key.clone(), value);
            }

            spans.push(OtelSpan {
                span_id: span.span_id.clone(),
                parent_span_id,
                trace_id: trace.trace_id.clone(),
                node_id,
                operation_name: span.operation_name.clone(),
                start_micros: span.start_time,
                duration_micros: span.duration,
                attributes,
            });
        }
    }
    Ok(spans)
}

/// Builds a span tree from flat spans, keyed by trace_id.
pub fn build_span_trees(spans: &[OtelSpan]) -> HashMap<String, Vec<SpanNode>> {
    let mut by_trace: HashMap<String, Vec<OtelSpan>> = HashMap::new();
    for span in spans {
        by_trace
            .entry(span.trace_id.clone())
            .or_default()
            .push(span.clone());
    }

    let mut trees = HashMap::new();
    for (trace_id, trace_spans) in by_trace {
        // Map span_id -> span
        let mut by_id: HashMap<String, OtelSpan> = HashMap::new();
        for span in &trace_spans {
            by_id.insert(span.span_id.clone(), span.clone());
        }

        // Find roots (no parent or parent not in this trace)
        let roots: Vec<SpanNode> = trace_spans
            .iter()
            .filter(|s| {
                s.parent_span_id
                    .as_ref()
                    .map(|p| !by_id.contains_key(p))
                    .unwrap_or(true)
            })
            .map(|root| build_node(&by_id, root))
            .collect();

        trees.insert(trace_id, roots);
    }
    trees
}

fn build_node(by_id: &HashMap<String, OtelSpan>, span: &OtelSpan) -> SpanNode {
    let mut children: Vec<SpanNode> = by_id
        .values()
        .filter(|s| s.parent_span_id.as_deref() == Some(span.span_id.as_str()))
        .map(|child| build_node(by_id, child))
        .collect();
    children.sort_by(|a, b| a.span.start_micros.cmp(&b.span.start_micros));
    SpanNode {
        span: span.clone(),
        children,
    }
}

// ---------------------------------------------------------------------------
// Collector
// ---------------------------------------------------------------------------

/// Polls a Jaeger-compatible HTTP API for spans, keeps a ring buffer.
///
/// Starts stopped (M11.5 D1): monitoring is opt-in, so nothing polls until
/// `start()` is called.
#[derive(Clone)]
pub struct OtelCollector {
    inner: Arc<RwLock<OtelInner>>,
    running: watch::Sender<bool>,
}

struct OtelInner {
    endpoint: String,
    poll_interval: std::time::Duration,
    spans: Vec<OtelSpan>,
    capacity: usize,
    connected: bool,
    last_error: Option<String>,
    sample_count: u64,
    last_poll_at: Option<u64>,
    received_count: u64,
    last_received_at: Option<u64>,
}

impl OtelCollector {
    pub fn new(endpoint: String) -> Self {
        Self {
            inner: Arc::new(RwLock::new(OtelInner {
                endpoint,
                poll_interval: std::time::Duration::from_secs(5),
                spans: Vec::new(),
                capacity: 1000,
                connected: false,
                last_error: None,
                sample_count: 0,
                last_poll_at: None,
                received_count: 0,
                last_received_at: None,
            })),
            running: watch::channel(false).0,
        }
    }

    /// Spawns the background poll task unless it is already running.
    ///
    /// Jaeger's `/api/traces` requires a `service` parameter, so the poll
    /// first lists services (`/api/services`), then fetches recent traces
    /// per service and merges the results.
    pub fn start(&self) {
        if *self.running.borrow() {
            return;
        }
        let rx = self.running.subscribe();
        self.running.send_replace(true);

        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            let endpoint = inner.read().await.endpoint.clone();
            otel_poll_loop(inner, rx, move || {
                let endpoint = endpoint.clone();
                async move { fetch_all_traces(&endpoint).await }
            })
            .await;
        });
    }

    /// Signals the poll task to exit; the task stops at the next tick.
    pub fn stop(&self) {
        self.running.send_replace(false);
    }

    pub fn is_running(&self) -> bool {
        *self.running.borrow()
    }

    /// Appends pushed spans (M11.5 D3 OTLP receiver) to the ring buffer and
    /// updates the receive stats. Returns the number of spans ingested.
    pub async fn ingest(&self, spans: Vec<OtelSpan>) -> usize {
        let count = spans.len();
        if count == 0 {
            return 0;
        }
        let mut inner = self.inner.write().await;
        for span in spans {
            if inner.spans.len() >= inner.capacity {
                inner.spans.remove(0);
            }
            inner.spans.push(span);
        }
        inner.received_count += count as u64;
        inner.last_received_at = Some(crate::metrics::unix_timestamp());
        count
    }

    /// Returns cached spans (newest first), optionally filtered by node and limited.
    pub async fn spans_for_node(&self, node: Option<&str>, limit: usize) -> Vec<OtelSpan> {
        let inner = self.inner.read().await;
        inner
            .spans
            .iter()
            .filter(|s| node.map(|n| s.node_id == n).unwrap_or(true))
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    /// Returns the span tree for a single trace_id.
    pub async fn trace_tree(&self, trace_id: &str) -> Option<Vec<SpanNode>> {
        let inner = self.inner.read().await;
        let spans: Vec<OtelSpan> = inner
            .spans
            .iter()
            .filter(|s| s.trace_id == trace_id)
            .cloned()
            .collect();
        if spans.is_empty() {
            return None;
        }
        build_span_trees(&spans).remove(trace_id)
    }

    /// Status: running flag, poll stats, endpoint, connected flag, last error.
    ///
    /// `connected` is true when either the last Jaeger poll succeeded or spans
    /// have been received through the OTLP push receiver (M11.5 D3).
    pub async fn status(&self) -> serde_json::Value {
        let inner = self.inner.read().await;
        serde_json::json!({
            "enabled": *self.running.borrow(),
            "sampleCount": inner.sample_count,
            "lastPollAt": inner.last_poll_at,
            "endpoint": inner.endpoint,
            "connected": inner.connected || inner.last_received_at.is_some(),
            "spanCount": inner.spans.len(),
            "lastError": inner.last_error,
            "receivedCount": inner.received_count,
            "lastReceivedAt": inner.last_received_at,
        })
    }
}

/// Polls `fetch` on the configured interval until the watch channel flips to
/// `false` (or the sender is dropped). Errors are recorded, not fatal.
async fn otel_poll_loop<F, Fut>(
    inner: Arc<RwLock<OtelInner>>,
    mut rx: watch::Receiver<bool>,
    mut fetch: F,
) where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<Vec<OtelSpan>, String>>,
{
    let poll_interval = inner.read().await.poll_interval;
    let mut interval = tokio::time::interval(poll_interval);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let result = fetch().await;
                let mut guard = inner.write().await;
                guard.sample_count += 1;
                guard.last_poll_at = Some(crate::metrics::unix_timestamp());
                match result {
                    Ok(spans) => {
                        guard.connected = true;
                        guard.last_error = None;
                        for span in spans {
                            if guard.spans.len() >= guard.capacity {
                                guard.spans.remove(0);
                            }
                            guard.spans.push(span);
                        }
                    }
                    Err(e) => {
                        guard.connected = false;
                        guard.last_error = Some(e);
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

/// Two-step fetch: list services, then fetch traces per service.
///
/// Jaeger's `/api/traces` requires a `service` parameter; `/api/services`
/// returns all known service names (in dora, one service per node).
async fn fetch_all_traces(endpoint: &str) -> Result<Vec<OtelSpan>, String> {
    let services_url = format!("{endpoint}/api/services");
    let services_body = reqwest_get(&services_url).await?;
    let services = parse_services_response(&services_body)?;

    let mut all_spans = Vec::new();
    for service in services {
        let url = build_traces_url(endpoint, &service, 20);
        let body = reqwest_get(&url).await?;
        let spans = parse_jaeger_response(&body)?;
        all_spans.extend(spans);
    }
    Ok(all_spans)
}

/// Parses `/api/services` response: `{"data":["svc1","svc2"],"total":2,...}`.
fn parse_services_response(input: &str) -> Result<Vec<String>, String> {
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|e| format!("invalid services response: {e}"))?;
    let data = value
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| "missing 'data' array in services response".to_string())?;
    let services = data
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    Ok(services)
}

/// Builds the traces URL for a service.
fn build_traces_url(endpoint: &str, service: &str, limit: usize) -> String {
    format!("{endpoint}/api/traces?service={service}&limit={limit}")
}

/// Decodes an HTTP chunked transfer-encoding body.
///
/// Format: `<hex-size>\r\n<data>\r\n` repeated, terminated by `0\r\n\r\n`.
fn decode_chunked_body(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < body.len() {
        // Find end of chunk-size line
        let line_end = match body[pos..].windows(2).position(|w| w == b"\r\n") {
            Some(i) => pos + i,
            None => break,
        };
        let size_str = String::from_utf8_lossy(&body[pos..line_end]);
        let size = match usize::from_str_radix(size_str.trim(), 16) {
            Ok(s) => s,
            Err(_) => break,
        };
        pos = line_end + 2;
        if size == 0 {
            break;
        }
        let data_end = pos + size;
        if data_end > body.len() {
            break;
        }
        out.extend_from_slice(&body[pos..data_end]);
        pos = data_end;
        // Skip trailing CRLF after chunk data
        if pos + 2 <= body.len() && &body[pos..pos + 2] == b"\r\n" {
            pos += 2;
        }
    }
    out
}

/// Minimal HTTP GET over raw TCP — keeps the dependency-light strategy
/// (same approach as coordinator_ws.rs). No reqwest needed.
async fn reqwest_get(url: &str) -> Result<String, String> {
    // Parse URL: http://host:port/path
    let without_scheme = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("unsupported URL scheme: {url}"))?;
    let (host_port, path) = match without_scheme.find('/') {
        Some(idx) => (&without_scheme[..idx], &without_scheme[idx..]),
        None => (without_scheme, "/"),
    };
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().map_err(|_| "invalid port".to_string())?),
        None => (host_port, 80),
    };

    let mut stream = tokio::net::TcpStream::connect((host, port))
        .await
        .map_err(|e| format!("connect failed: {e}"))?;

    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host_port}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    );
    tokio::io::AsyncWriteExt::write_all(&mut stream, request.as_bytes())
        .await
        .map_err(|e| format!("write failed: {e}"))?;

    let mut buffer = Vec::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut buffer),
    )
    .await
    .map_err(|_| "read timeout".to_string())?
    .map_err(|e| format!("read failed: {e}"))?;

    let response = String::from_utf8_lossy(&buffer);
    // Split headers from body
    let (headers, body_bytes) = match buffer.windows(4).position(|w| w == b"\r\n\r\n") {
        Some(idx) => {
            let headers = &buffer[..idx];
            let body = &buffer[idx + 4..];
            (String::from_utf8_lossy(headers).into_owned(), body)
        }
        None => (response.into_owned(), &buffer[..]),
    };

    // Check status line
    let status_line = headers.lines().next().unwrap_or("");
    if !status_line.contains("200") {
        return Err(format!("HTTP error: {status_line}"));
    }

    // Decode chunked transfer encoding if present
    let body = if headers.lines().any(|l| {
        l.to_ascii_lowercase()
            .starts_with("transfer-encoding: chunked")
    }) {
        decode_chunked_body(body_bytes)
    } else {
        body_bytes.to_vec()
    };

    Ok(String::from_utf8_lossy(&body).to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Sample Jaeger v1 API response with two traces.
    const JAEGER_SAMPLE: &str = r#"{
  "data": [
    {
      "traceID": "0000000000000000trace1",
      "spans": [
        {
          "traceID": "0000000000000000trace1",
          "spanID": "00000000000000a1",
          "operationName": "process_frame",
          "references": [],
          "startTime": 1700000000000000,
          "duration": 5000,
          "tags": [],
          "logs": [],
          "processID": "p1"
        },
        {
          "traceID": "0000000000000000trace1",
          "spanID": "00000000000000a2",
          "operationName": "detect_objects",
          "references": [
            {"refType": "CHILD_OF", "traceID": "0000000000000000trace1", "spanID": "00000000000000a1"}
          ],
          "startTime": 1700000000001000,
          "duration": 3000,
          "tags": [{"key": "model", "type": "string", "value": "yolo-v8"}],
          "logs": [],
          "processID": "p2"
        },
        {
          "traceID": "0000000000000000trace1",
          "spanID": "00000000000000a3",
          "operationName": "publish_result",
          "references": [
            {"refType": "CHILD_OF", "traceID": "0000000000000000trace1", "spanID": "00000000000000a2"}
          ],
          "startTime": 1700000000002500,
          "duration": 1500,
          "tags": [],
          "logs": [],
          "processID": "p2"
        }
      ],
      "processes": {
        "p1": {"serviceName": "camera_node", "tags": []},
        "p2": {"serviceName": "detector_node", "tags": []}
      },
      "warnings": null
    },
    {
      "traceID": "0000000000000000trace2",
      "spans": [
        {
          "traceID": "0000000000000000trace2",
          "spanID": "00000000000000b1",
          "operationName": "control_loop",
          "references": [],
          "startTime": 1700000001000000,
          "duration": 8000,
          "tags": [],
          "logs": [],
          "processID": "p3"
        }
      ],
      "processes": {
        "p3": {"serviceName": "controller_node", "tags": []}
      },
      "warnings": null
    }
  ],
  "total": 4,
  "limit": 100,
  "offset": 0,
  "errors": null
}"#;

    #[test]
    fn parse_jaeger_sample_spans() {
        let spans = parse_jaeger_response(JAEGER_SAMPLE).unwrap();
        assert_eq!(spans.len(), 4);

        // Root span of trace1
        assert_eq!(spans[0].span_id, "00000000000000a1");
        assert_eq!(spans[0].parent_span_id, None);
        assert_eq!(spans[0].node_id, "camera_node");
        assert_eq!(spans[0].operation_name, "process_frame");
        assert_eq!(spans[0].duration_micros, 5000);

        // Child span of trace1
        assert_eq!(spans[1].parent_span_id.as_deref(), Some("00000000000000a1"));
        assert_eq!(spans[1].node_id, "detector_node");
        assert_eq!(
            spans[1].attributes.get("model").map(String::as_str),
            Some("yolo-v8")
        );

        // Trace2 root
        assert_eq!(spans[3].trace_id, "0000000000000000trace2");
        assert_eq!(spans[3].node_id, "controller_node");
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        let result = parse_jaeger_response("not json");
        assert!(result.is_err());
    }

    #[test]
    fn parse_services_response_returns_names() {
        let input = r#"{"data":["camera_node","detector_node","jaeger-all-in-one"],"total":3,"limit":0,"offset":0,"errors":null}"#;
        let services = parse_services_response(input).unwrap();
        assert_eq!(
            services,
            vec!["camera_node", "detector_node", "jaeger-all-in-one"]
        );
    }

    #[test]
    fn parse_services_response_missing_data_is_error() {
        let input = r#"{"data":null,"total":0}"#;
        assert!(parse_services_response(input).is_err());
    }

    #[test]
    fn build_traces_url_includes_service_and_limit() {
        let url = build_traces_url("http://localhost:16686", "camera_node", 20);
        assert_eq!(
            url,
            "http://localhost:16686/api/traces?service=camera_node&limit=20"
        );
    }

    // -- Chunked transfer decoding --

    #[test]
    fn decode_chunked_single_chunk() {
        // {"a":1} = 7 bytes
        let body = b"7\r\n{\"a\":1}\r\n0\r\n\r\n";
        let decoded = decode_chunked_body(body);
        assert_eq!(String::from_utf8_lossy(&decoded), "{\"a\":1}");
    }

    #[test]
    fn decode_chunked_multiple_chunks() {
        // "hello" (5) + "world" (5)
        let body = b"5\r\nhello\r\n5\r\nworld\r\n0\r\n\r\n";
        let decoded = decode_chunked_body(body);
        assert_eq!(String::from_utf8_lossy(&decoded), "helloworld");
    }

    #[test]
    fn decode_chunked_incomplete_is_truncated_gracefully() {
        // Chunk claims 10 bytes but only 3 available
        let body = b"a\r\nabc";
        let decoded = decode_chunked_body(body);
        assert!(decoded.is_empty());
    }

    #[test]
    fn parse_empty_data_returns_empty() {
        let result =
            parse_jaeger_response(r#"{"data":[],"total":0,"limit":0,"offset":0,"errors":null}"#)
                .unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn build_tree_creates_parent_child_hierarchy() {
        let spans = parse_jaeger_response(JAEGER_SAMPLE).unwrap();
        let trees = build_span_trees(&spans);

        // trace1 should have one root with 2 levels of children
        let trace1 = trees.get("0000000000000000trace1").unwrap();
        assert_eq!(trace1.len(), 1);
        assert_eq!(trace1[0].span.span_id, "00000000000000a1");
        assert_eq!(trace1[0].children.len(), 1);
        assert_eq!(trace1[0].children[0].span.span_id, "00000000000000a2");
        assert_eq!(trace1[0].children[0].children.len(), 1);
        assert_eq!(
            trace1[0].children[0].children[0].span.span_id,
            "00000000000000a3"
        );

        // trace2 should have one root with no children
        let trace2 = trees.get("0000000000000000trace2").unwrap();
        assert_eq!(trace2.len(), 1);
        assert_eq!(trace2[0].children.len(), 0);
    }

    #[test]
    fn build_tree_handles_orphan_parents() {
        // Span whose parent is not in the set becomes a root
        let spans = vec![OtelSpan {
            span_id: "child".into(),
            parent_span_id: Some("missing-parent".into()),
            trace_id: "t1".into(),
            node_id: "n1".into(),
            operation_name: "op".into(),
            start_micros: 100,
            duration_micros: 50,
            attributes: HashMap::new(),
        }];
        let trees = build_span_trees(&spans);
        let roots = trees.get("t1").unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].span.span_id, "child");
    }

    #[tokio::test]
    async fn spans_for_node_filters_by_node_id() {
        let collector = OtelCollector::new("http://localhost:1".into());
        {
            let mut inner = collector.inner.write().await;
            inner.spans = vec![
                OtelSpan {
                    span_id: "s1".into(),
                    parent_span_id: None,
                    trace_id: "t1".into(),
                    node_id: "node-a".into(),
                    operation_name: "op1".into(),
                    start_micros: 1,
                    duration_micros: 10,
                    attributes: HashMap::new(),
                },
                OtelSpan {
                    span_id: "s2".into(),
                    parent_span_id: None,
                    trace_id: "t2".into(),
                    node_id: "node-b".into(),
                    operation_name: "op2".into(),
                    start_micros: 2,
                    duration_micros: 20,
                    attributes: HashMap::new(),
                },
            ];
        }

        let node_a = collector.spans_for_node(Some("node-a"), 100).await;
        assert_eq!(node_a.len(), 1);
        assert_eq!(node_a[0].span_id, "s1");

        let all = collector.spans_for_node(None, 100).await;
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn spans_for_node_returns_newest_first() {
        let collector = OtelCollector::new("http://localhost:1".into());
        {
            let mut inner = collector.inner.write().await;
            for i in 0..10u64 {
                inner.spans.push(OtelSpan {
                    span_id: format!("s{i}"),
                    parent_span_id: None,
                    trace_id: "t1".into(),
                    node_id: "node-a".into(),
                    operation_name: "op".into(),
                    start_micros: i * 1000,
                    duration_micros: 10,
                    attributes: HashMap::new(),
                });
            }
        }

        let recent = collector.spans_for_node(None, 3).await;
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].span_id, "s9");
        assert_eq!(recent[1].span_id, "s8");
        assert_eq!(recent[2].span_id, "s7");
    }

    // -- Poll loop lifecycle (M11.5) --

    fn stub_span(id: &str) -> OtelSpan {
        OtelSpan {
            span_id: id.to_string(),
            parent_span_id: None,
            trace_id: "t1".into(),
            node_id: "n1".into(),
            operation_name: "op".into(),
            start_micros: 0,
            duration_micros: 1,
            attributes: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn otel_poll_loop_counts_attempts_and_applies_spans() {
        let collector = OtelCollector::new("http://stub".into());
        collector.inner.write().await.poll_interval = std::time::Duration::from_millis(5);
        let inner = Arc::clone(&collector.inner);
        let (tx, rx) = tokio::sync::watch::channel(true);

        let task = tokio::spawn(otel_poll_loop(inner, rx, || async {
            Ok(vec![stub_span("s1")])
        }));

        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        tx.send(false).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();

        let guard = collector.inner.read().await;
        assert!(guard.sample_count >= 2);
        assert!(guard.last_poll_at.is_some());
        assert_eq!(guard.spans.len(), guard.sample_count as usize);
        assert!(guard.spans.iter().all(|s| s.span_id == "s1"));
        assert!(guard.connected);
    }

    #[tokio::test]
    async fn otel_poll_loop_errors_do_not_stop_the_loop() {
        let collector = OtelCollector::new("http://stub".into());
        collector.inner.write().await.poll_interval = std::time::Duration::from_millis(5);
        let inner = Arc::clone(&collector.inner);
        let (tx, rx) = tokio::sync::watch::channel(true);

        let task = tokio::spawn(otel_poll_loop(inner, rx, || async {
            Err("backend down".to_string())
        }));

        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        tx.send(false).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();

        let guard = collector.inner.read().await;
        assert!(guard.sample_count >= 2);
        assert!(!guard.connected);
        assert_eq!(guard.last_error.as_deref(), Some("backend down"));
    }

    #[tokio::test]
    async fn otel_collector_start_stop_toggles_running() {
        let collector = OtelCollector::new("http://localhost:1".into());
        assert!(!collector.is_running());

        collector.start();
        assert!(collector.is_running());
        collector.start(); // must not spawn a second loop

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        collector.stop();
        assert!(!collector.is_running());

        let status = collector.status().await;
        assert_eq!(status["enabled"], false);
        assert!(status["sampleCount"].as_u64().unwrap() >= 1);
        assert!(status["lastPollAt"].is_u64());
    }

    // -- Push ingestion (M11.5 D3) --

    #[tokio::test]
    async fn ingest_appends_spans_and_trims_capacity() {
        let collector = OtelCollector::new("http://localhost:1".into());
        collector.inner.write().await.capacity = 3;

        let spans: Vec<OtelSpan> = (0..5)
            .map(|i| OtelSpan {
                span_id: format!("s{i}"),
                ..stub_span("s")
            })
            .collect();
        collector.ingest(spans).await;

        let stored = collector.spans_for_node(None, 10).await;
        assert_eq!(stored.len(), 3);
        assert_eq!(stored[0].span_id, "s4");
        assert_eq!(stored[2].span_id, "s2");
    }

    #[tokio::test]
    async fn ingest_updates_received_stats() {
        let collector = OtelCollector::new("http://localhost:1".into());
        collector
            .ingest(vec![stub_span("s1"), stub_span("s2")])
            .await;

        let status = collector.status().await;
        assert_eq!(status["receivedCount"], 2);
        assert!(status["lastReceivedAt"].is_u64());
    }

    #[tokio::test]
    async fn status_reports_connected_after_received_spans_without_polling() {
        let collector = OtelCollector::new("http://localhost:1".into());
        assert_eq!(collector.status().await["connected"], false);

        collector.ingest(vec![stub_span("s1")]).await;
        assert_eq!(collector.status().await["connected"], true);
    }
}
