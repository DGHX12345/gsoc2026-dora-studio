//! Minimal OTLP/HTTP trace receiver (M11.5 D3).
//!
//! Hand-written prost types for `opentelemetry-proto` `trace_service.proto`
//! (M00 type-copy strategy, no protoc). dora nodes push spans to
//! `POST /v1/traces` (protobuf); the receiver normalizes them into
//! [`OtelSpan`]s and appends to the shared [`OtelCollector`] ring buffer.

use crate::otel::{OtelCollector, OtelSpan};

// ---------------------------------------------------------------------------
// Wire types (opentelemetry-proto trace_service.proto subset)
// ---------------------------------------------------------------------------

/// `opentelemetry.proto.trace.v1.ExportTraceServiceRequest`
#[derive(Clone, PartialEq, prost::Message)]
pub struct ExportTraceServiceRequest {
    #[prost(message, repeated, tag = "1")]
    pub resource_spans: Vec<ResourceSpans>,
}

/// `opentelemetry.proto.collector.trace.v1.ExportTraceServiceResponse`
/// (empty message — encodes to zero bytes).
#[derive(Clone, PartialEq, prost::Message)]
pub struct ExportTraceServiceResponse {}

/// `opentelemetry.proto.collector.metrics.v1.ExportMetricsServiceRequest`
/// (empty stub: any protobuf payload decodes into it — unknown fields are
/// skipped — so the metrics service can accept and discard).
#[derive(Clone, PartialEq, prost::Message)]
pub struct ExportMetricsServiceRequest {}

/// `opentelemetry.proto.collector.metrics.v1.ExportMetricsServiceResponse`
#[derive(Clone, PartialEq, prost::Message)]
pub struct ExportMetricsServiceResponse {}

/// `opentelemetry.proto.trace.v1.ResourceSpans`
#[derive(Clone, PartialEq, prost::Message)]
pub struct ResourceSpans {
    #[prost(message, optional, tag = "1")]
    pub resource: Option<Resource>,
    #[prost(message, repeated, tag = "2")]
    pub scope_spans: Vec<ScopeSpans>,
}

/// `opentelemetry.proto.resource.v1.Resource`
#[derive(Clone, PartialEq, prost::Message)]
pub struct Resource {
    #[prost(message, repeated, tag = "1")]
    pub attributes: Vec<KeyValue>,
}

/// `opentelemetry.proto.trace.v1.ScopeSpans`
#[derive(Clone, PartialEq, prost::Message)]
pub struct ScopeSpans {
    #[prost(message, repeated, tag = "2")]
    pub spans: Vec<Span>,
}

/// `opentelemetry.proto.trace.v1.Span` (subset)
#[derive(Clone, PartialEq, prost::Message)]
pub struct Span {
    #[prost(bytes = "vec", tag = "1")]
    pub trace_id: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub span_id: Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub parent_span_id: Vec<u8>,
    #[prost(string, tag = "5")]
    pub name: String,
    #[prost(fixed64, tag = "7")]
    pub start_time_unix_nano: u64,
    #[prost(fixed64, tag = "8")]
    pub end_time_unix_nano: u64,
    #[prost(message, repeated, tag = "9")]
    pub attributes: Vec<KeyValue>,
}

/// `opentelemetry.proto.common.v1.KeyValue`
#[derive(Clone, PartialEq, prost::Message)]
pub struct KeyValue {
    #[prost(string, tag = "1")]
    pub key: String,
    #[prost(message, optional, tag = "2")]
    pub value: Option<AnyValue>,
}

/// `opentelemetry.proto.common.v1.AnyValue` (scalar members only)
#[derive(Clone, PartialEq, prost::Message)]
pub struct AnyValue {
    #[prost(oneof = "any_value::Value", tags = "1, 2, 3, 4, 7")]
    pub value: Option<any_value::Value>,
}

pub mod any_value {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Value {
        #[prost(string, tag = "1")]
        StringValue(String),
        #[prost(bool, tag = "2")]
        BoolValue(bool),
        #[prost(int64, tag = "3")]
        IntValue(i64),
        #[prost(double, tag = "4")]
        DoubleValue(f64),
        #[prost(bytes, tag = "7")]
        BytesValue(Vec<u8>),
    }
}

// ---------------------------------------------------------------------------
// Decoding + normalization
// ---------------------------------------------------------------------------

/// Decodes an OTLP `ExportTraceServiceRequest` protobuf payload.
pub fn decode_export_request(bytes: &[u8]) -> Result<ExportTraceServiceRequest, String> {
    prost::Message::decode(bytes).map_err(|e| format!("invalid OTLP protobuf: {e}"))
}

/// Converts an OTLP request into normalized [`OtelSpan`]s.
///
/// Node id comes from the resource attribute `service.name` (same convention
/// as the Jaeger path); ids are hex-encoded; timestamps are nanos→micros.
pub fn spans_from_request(request: &ExportTraceServiceRequest) -> Vec<OtelSpan> {
    let mut spans = Vec::new();
    for resource_spans in &request.resource_spans {
        let node_id = resource_spans
            .resource
            .as_ref()
            .and_then(|r| attribute_string(&r.attributes, "service.name"))
            .unwrap_or_else(|| "unknown".to_string());

        for scope_spans in &resource_spans.scope_spans {
            for span in &scope_spans.spans {
                let mut attributes = std::collections::HashMap::new();
                for kv in &span.attributes {
                    attributes.insert(kv.key.clone(), any_value_to_string(&kv.value));
                }
                spans.push(OtelSpan {
                    span_id: hex_encode(&span.span_id),
                    parent_span_id: if span.parent_span_id.is_empty() {
                        None
                    } else {
                        Some(hex_encode(&span.parent_span_id))
                    },
                    trace_id: hex_encode(&span.trace_id),
                    node_id: node_id.clone(),
                    operation_name: span.name.clone(),
                    start_micros: span.start_time_unix_nano / 1000,
                    duration_micros: span
                        .end_time_unix_nano
                        .saturating_sub(span.start_time_unix_nano)
                        / 1000,
                    attributes,
                });
            }
        }
    }
    spans
}

fn attribute_string(attributes: &[KeyValue], key: &str) -> Option<String> {
    attributes
        .iter()
        .find(|kv| kv.key == key)
        .map(|kv| any_value_to_string(&kv.value))
}

fn any_value_to_string(value: &Option<AnyValue>) -> String {
    match value.as_ref().and_then(|v| v.value.as_ref()) {
        Some(any_value::Value::StringValue(s)) => s.clone(),
        Some(any_value::Value::BoolValue(b)) => b.to_string(),
        Some(any_value::Value::IntValue(i)) => i.to_string(),
        Some(any_value::Value::DoubleValue(d)) => d.to_string(),
        Some(any_value::Value::BytesValue(b)) => hex_encode(b),
        None => String::new(),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

// ---------------------------------------------------------------------------
// HTTP receiver
// ---------------------------------------------------------------------------

const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Router for the OTLP receiver: `POST /v1/traces` (protobuf in, empty
/// `ExportTraceServiceResponse` out — the empty response encodes to zero
/// bytes, so a bare 200 with protobuf content-type is spec-compliant).
pub fn receiver_router(collector: OtelCollector) -> axum::Router {
    axum::Router::new()
        .route("/v1/traces", axum::routing::post(ingest_http))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(collector)
}

async fn ingest_http(
    axum::extract::State(collector): axum::extract::State<OtelCollector>,
    body: axum::body::Bytes,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let request = match decode_export_request(&body) {
        Ok(request) => request,
        Err(e) => return (axum::http::StatusCode::BAD_REQUEST, e).into_response(),
    };
    collector.ingest(spans_from_request(&request)).await;
    (
        [(axum::http::header::CONTENT_TYPE, "application/x-protobuf")],
        axum::http::StatusCode::OK,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use crate::otel::OtelCollector;
    use crate::otlp::{
        any_value, decode_export_request, ingest_http, receiver_router, spans_from_request,
        AnyValue, ExportTraceServiceRequest, KeyValue, Resource, ResourceSpans, ScopeSpans, Span,
    };

    // Hand-written wire encoder (independent of prost) for golden bytes.
    fn len_field(tag: u8, bytes: &[u8]) -> Vec<u8> {
        assert!(
            bytes.len() < 128,
            "golden helper only handles 1-byte lengths"
        );
        let mut out = vec![(tag << 3) | 2, bytes.len() as u8];
        out.extend_from_slice(bytes);
        out
    }

    fn fixed64_field(tag: u8, value: u64) -> Vec<u8> {
        let mut out = vec![(tag << 3) | 1];
        out.extend_from_slice(&value.to_le_bytes());
        out
    }

    /// Golden bytes per the official proto field numbers:
    /// ExportTraceServiceRequest.1 → ResourceSpans { 1: Resource { 1:
    /// KeyValue{service.name}, }, 2: ScopeSpans { 2: Span } }.
    fn golden_request_bytes() -> Vec<u8> {
        let key_value = [
            len_field(1, b"service.name"),
            len_field(2, &len_field(1, b"camera_node")),
        ]
        .concat();
        let resource = len_field(1, &key_value);
        let span = [
            len_field(1, &[0x0f; 16]),
            len_field(2, &[0x5a; 8]),
            len_field(4, &[0x01; 8]),
            len_field(5, b"process_frame"),
            fixed64_field(7, 1_700_000_000_000_000_000),
            fixed64_field(8, 1_700_000_000_005_000_000),
        ]
        .concat();
        let scope_spans = len_field(2, &span);
        let resource_spans = [len_field(1, &resource), len_field(2, &scope_spans)].concat();
        len_field(1, &resource_spans)
    }

    #[test]
    fn decode_golden_bytes_matches_proto_field_numbers() {
        let request = decode_export_request(&golden_request_bytes()).unwrap();
        assert_eq!(request.resource_spans.len(), 1);

        let resource = request.resource_spans[0].resource.as_ref().unwrap();
        assert_eq!(resource.attributes.len(), 1);
        assert_eq!(resource.attributes[0].key, "service.name");

        let spans = &request.resource_spans[0].scope_spans[0].spans;
        assert_eq!(spans.len(), 1);
        let span = &spans[0];
        assert_eq!(span.trace_id, vec![0x0f; 16]);
        assert_eq!(span.span_id, vec![0x5a; 8]);
        assert_eq!(span.parent_span_id, vec![0x01; 8]);
        assert_eq!(span.name, "process_frame");
        assert_eq!(span.start_time_unix_nano, 1_700_000_000_000_000_000);
        assert_eq!(span.end_time_unix_nano, 1_700_000_000_005_000_000);
    }

    #[test]
    fn decode_rejects_garbage() {
        assert!(decode_export_request(b"not protobuf").is_err());
        // An empty body is a valid protobuf message: a request with zero spans.
        let empty = decode_export_request(b"").unwrap();
        assert!(empty.resource_spans.is_empty());
    }

    #[test]
    fn roundtrip_encode_decode_preserves_fields() {
        let request = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: Some(Resource {
                    attributes: vec![KeyValue {
                        key: "service.name".into(),
                        value: Some(AnyValue {
                            value: Some(any_value::Value::StringValue("planner".into())),
                        }),
                    }],
                }),
                scope_spans: vec![ScopeSpans {
                    spans: vec![Span {
                        trace_id: vec![7u8; 16],
                        span_id: vec![8u8; 8],
                        parent_span_id: vec![],
                        name: "plan".into(),
                        start_time_unix_nano: 1000,
                        end_time_unix_nano: 3000,
                        attributes: vec![KeyValue {
                            key: "retries".into(),
                            value: Some(AnyValue {
                                value: Some(any_value::Value::IntValue(3)),
                            }),
                        }],
                    }],
                }],
            }],
        };
        let bytes = request.encode_to_vec();
        let decoded = decode_export_request(&bytes).unwrap();
        assert_eq!(decoded, request);
    }

    #[test]
    fn spans_from_request_maps_fields() {
        let request = decode_export_request(&golden_request_bytes()).unwrap();
        let spans = spans_from_request(&request);
        assert_eq!(spans.len(), 1);
        let span = &spans[0];
        assert_eq!(span.node_id, "camera_node");
        assert_eq!(span.operation_name, "process_frame");
        assert_eq!(span.trace_id, "0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f");
        assert_eq!(span.span_id, "5a5a5a5a5a5a5a5a");
        assert_eq!(span.parent_span_id.as_deref(), Some("0101010101010101"));
        assert_eq!(span.start_micros, 1_700_000_000_000_000);
        assert_eq!(span.duration_micros, 5_000);
        assert!(span.attributes.is_empty());
    }

    #[test]
    fn spans_from_request_defaults_unknown_node_id() {
        let request = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: None,
                scope_spans: vec![ScopeSpans {
                    spans: vec![Span {
                        trace_id: vec![1u8; 16],
                        span_id: vec![2u8; 8],
                        parent_span_id: vec![],
                        name: "op".into(),
                        start_time_unix_nano: 0,
                        end_time_unix_nano: 1000,
                        attributes: vec![],
                    }],
                }],
            }],
        };
        let spans = spans_from_request(&request);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].node_id, "unknown");
        assert_eq!(spans[0].parent_span_id, None);
    }

    #[test]
    fn spans_from_request_stringifies_attribute_value_kinds() {
        let request = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: None,
                scope_spans: vec![ScopeSpans {
                    spans: vec![Span {
                        trace_id: vec![1u8; 16],
                        span_id: vec![2u8; 8],
                        parent_span_id: vec![],
                        name: "op".into(),
                        start_time_unix_nano: 0,
                        end_time_unix_nano: 1000,
                        attributes: vec![
                            KeyValue {
                                key: "flag".into(),
                                value: Some(AnyValue {
                                    value: Some(any_value::Value::BoolValue(true)),
                                }),
                            },
                            KeyValue {
                                key: "count".into(),
                                value: Some(AnyValue {
                                    value: Some(any_value::Value::IntValue(-2)),
                                }),
                            },
                            KeyValue {
                                key: "ratio".into(),
                                value: Some(AnyValue {
                                    value: Some(any_value::Value::DoubleValue(0.5)),
                                }),
                            },
                            KeyValue {
                                key: "blob".into(),
                                value: Some(AnyValue {
                                    value: Some(any_value::Value::BytesValue(vec![0xab, 0xcd])),
                                }),
                            },
                            KeyValue {
                                key: "empty".into(),
                                value: None,
                            },
                        ],
                    }],
                }],
            }],
        };
        let spans = spans_from_request(&request);
        let attributes = &spans[0].attributes;
        assert_eq!(attributes.get("flag").map(String::as_str), Some("true"));
        assert_eq!(attributes.get("count").map(String::as_str), Some("-2"));
        assert_eq!(attributes.get("ratio").map(String::as_str), Some("0.5"));
        assert_eq!(attributes.get("blob").map(String::as_str), Some("abcd"));
        assert_eq!(attributes.get("empty").map(String::as_str), Some(""));
    }

    #[test]
    fn spans_from_request_zeroes_negative_duration() {
        let request = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: None,
                scope_spans: vec![ScopeSpans {
                    spans: vec![Span {
                        trace_id: vec![1u8; 16],
                        span_id: vec![2u8; 8],
                        parent_span_id: vec![],
                        name: "op".into(),
                        start_time_unix_nano: 2000,
                        end_time_unix_nano: 1000,
                        attributes: vec![],
                    }],
                }],
            }],
        };
        let spans = spans_from_request(&request);
        assert_eq!(spans[0].duration_micros, 0);
    }

    fn sample_request_bytes() -> Vec<u8> {
        let request = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: Some(Resource {
                    attributes: vec![KeyValue {
                        key: "service.name".into(),
                        value: Some(AnyValue {
                            value: Some(any_value::Value::StringValue("camera_node".into())),
                        }),
                    }],
                }),
                scope_spans: vec![ScopeSpans {
                    spans: vec![Span {
                        trace_id: vec![0x0f; 16],
                        span_id: vec![0x5a; 8],
                        parent_span_id: vec![],
                        name: "process_frame".into(),
                        start_time_unix_nano: 1_700_000_000_000_000_000,
                        end_time_unix_nano: 1_700_000_000_005_000_000,
                        attributes: vec![],
                    }],
                }],
            }],
        };
        request.encode_to_vec()
    }

    #[tokio::test]
    async fn receiver_post_ingests_spans() {
        let collector = OtelCollector::new("http://localhost:1".into());
        let _app = receiver_router(collector.clone());

        let response = ingest_http(
            axum::extract::State(collector.clone()),
            axum::body::Bytes::from(sample_request_bytes()),
        )
        .await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "application/x-protobuf");

        let spans = collector.spans_for_node(None, 10).await;
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].operation_name, "process_frame");
        assert_eq!(spans[0].node_id, "camera_node");
    }

    #[tokio::test]
    async fn receiver_post_rejects_invalid_protobuf() {
        let collector = OtelCollector::new("http://localhost:1".into());

        let response = ingest_http(
            axum::extract::State(collector.clone()),
            axum::body::Bytes::from_static(b"not protobuf"),
        )
        .await;
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);

        let spans = collector.spans_for_node(None, 10).await;
        assert!(spans.is_empty());
    }
}
