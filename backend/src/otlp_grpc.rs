//! Hand-written OTLP gRPC receiver (M15.6).
//!
//! dora exports OTel spans/metrics via OTLP gRPC (`with_tonic()`,
//! `DORA_OTLP_ENDPOINT`). This module implements the trace + metrics
//! services without protoc: the wire types come from [`crate::otlp`]
//! (prost, hand-written) and the tonic server boilerplate is written by
//! hand (M00 strategy). Traces feed the shared [`OtelCollector`] ring
//! buffer; metrics are accepted and discarded (stub — Studio uses
//! WS/CLI for node metrics, not OTLP).

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body_util::BodyExt;
use tonic::body::BoxBody;

use crate::otel::OtelCollector;
use crate::otlp::{decode_export_request, spans_from_request};

pub const TRACE_EXPORT_PATH: &str = "/opentelemetry.proto.collector.trace.v1.TraceService/Export";
pub const METRICS_EXPORT_PATH: &str =
    "/opentelemetry.proto.collector.metrics.v1.MetricsService/Export";

/// Frames a protobuf payload as a single gRPC message:
/// 1 byte flags + 4 byte big-endian length + payload.
pub fn grpc_frame(payload: &[u8]) -> Vec<u8> {
    let mut framed = Vec::with_capacity(5 + payload.len());
    framed.push(0);
    framed.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    framed.extend_from_slice(payload);
    framed
}

/// Shared handler state; cloned into every request.
#[derive(Clone)]
pub struct OtelGrpcHandlers {
    collector: OtelCollector,
}

impl OtelGrpcHandlers {
    pub fn new(collector: OtelCollector) -> Self {
        Self { collector }
    }
}

/// Decodes a trace export payload and ingests the spans.
pub async fn handle_trace(
    handlers: &OtelGrpcHandlers,
    payload: &[u8],
) -> Result<(), tonic::Status> {
    let request = decode_export_request(payload)
        .map_err(|e| tonic::Status::invalid_argument(format!("invalid OTLP protobuf: {e}")))?;
    handlers
        .collector
        .ingest(spans_from_request(&request))
        .await;
    Ok(())
}

/// Metrics stub: accepts any payload and discards it.
pub async fn handle_metrics(
    _handlers: &OtelGrpcHandlers,
    _payload: &[u8],
) -> Result<(), tonic::Status> {
    Ok(())
}

/// Single gRPC service routing trace + metrics export paths.
#[derive(Clone)]
pub struct OtelGrpcService {
    inner: Arc<OtelGrpcHandlers>,
}

/// Builds the gRPC service around a collector.
pub fn service(collector: OtelCollector) -> OtelGrpcService {
    OtelGrpcService {
        inner: Arc::new(OtelGrpcHandlers::new(collector)),
    }
}

impl tonic::server::NamedService for OtelGrpcService {
    const NAME: &'static str = "opentelemetry.proto.collector.trace.v1.TraceService";
}

type GrpcResult = Result<http::Response<BoxBody>, Infallible>;

fn grpc_ok_response(framed: Vec<u8>) -> http::Response<BoxBody> {
    let body = http_body_util::Full::new(Bytes::from(framed))
        .map_err(|never| match never {})
        .boxed_unsync();
    let mut response = http::Response::new(body);
    response.headers_mut().insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/grpc"),
    );
    response
}

fn grpc_err_response(status: tonic::Status) -> http::Response<BoxBody> {
    let body = http_body_util::StreamBody::new(futures_util::stream::once(async move {
        Err::<http_body::Frame<Bytes>, tonic::Status>(status)
    }))
    .boxed_unsync();
    http::Response::new(body)
}

impl tower::Service<http::Request<BoxBody>> for OtelGrpcService {
    type Response = http::Response<BoxBody>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = GrpcResult> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: http::Request<BoxBody>) -> Self::Future {
        let inner = Arc::clone(&self.inner);
        Box::pin(async move {
            let path = request.uri().path().to_string();
            let body = request.into_body();
            let payload = match body.collect().await {
                Ok(collected) => {
                    let bytes = collected.to_bytes();
                    // strip the 5-byte gRPC frame prefix
                    if bytes.len() >= 5 {
                        bytes.slice(5..).to_vec()
                    } else {
                        bytes.to_vec()
                    }
                }
                Err(status) => return Ok(grpc_err_response(status)),
            };

            let result = match path.as_str() {
                TRACE_EXPORT_PATH => handle_trace(&inner, &payload).await,
                METRICS_EXPORT_PATH => handle_metrics(&inner, &payload).await,
                _ => Err(tonic::Status::unimplemented(format!(
                    "unknown gRPC method: {path}"
                ))),
            };

            match result {
                Ok(()) => Ok(grpc_ok_response(grpc_frame(&[]))),
                Err(status) => Ok(grpc_err_response(status)),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use tower::Service as _;

    use super::{
        grpc_frame, handle_metrics, handle_trace, service, OtelGrpcHandlers, METRICS_EXPORT_PATH,
        TRACE_EXPORT_PATH,
    };
    use crate::otel::OtelCollector;
    use crate::otlp::{
        any_value, AnyValue, ExportTraceServiceRequest, KeyValue, Resource, ResourceSpans,
        ScopeSpans, Span,
    };
    use prost::Message;

    fn sample_request() -> ExportTraceServiceRequest {
        ExportTraceServiceRequest {
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
        }
    }

    #[tokio::test]
    async fn handle_trace_ingests_spans_into_collector() {
        let collector = OtelCollector::new("http://localhost:1".into());
        let handlers = OtelGrpcHandlers::new(collector.clone());

        let payload = sample_request().encode_to_vec();
        handle_trace(&handlers, &payload)
            .await
            .expect("trace accepted");

        let spans = collector.spans_for_node(None, 10).await;
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].node_id, "camera_node");
        assert_eq!(spans[0].operation_name, "process_frame");
    }

    #[tokio::test]
    async fn handle_trace_rejects_invalid_protobuf() {
        let collector = OtelCollector::new("http://localhost:1".into());
        let handlers = OtelGrpcHandlers::new(collector.clone());

        let result = handle_trace(&handlers, b"not protobuf").await;
        assert!(result.is_err());

        let spans = collector.spans_for_node(None, 10).await;
        assert!(spans.is_empty());
    }

    #[tokio::test]
    async fn handle_metrics_accepts_and_discards() {
        let collector = OtelCollector::new("http://localhost:1".into());
        let handlers = OtelGrpcHandlers::new(collector.clone());

        handle_metrics(&handlers, b"arbitrary metrics payload")
            .await
            .expect("metrics accepted");

        let spans = collector.spans_for_node(None, 10).await;
        assert!(spans.is_empty());
    }

    /// A framed gRPC request routed to the trace path must ingest the span
    /// and answer with a framed empty response + application/grpc.
    #[tokio::test]
    async fn service_routes_trace_path_and_returns_grpc_response() {
        use http_body_util::BodyExt;

        let collector = OtelCollector::new("http://localhost:1".into());
        let mut svc = service(collector.clone());

        let body = tonic::body::boxed(http_body_util::Full::new(bytes::Bytes::from(grpc_frame(
            &sample_request().encode_to_vec(),
        ))));
        let request = http::Request::builder()
            .method("POST")
            .uri(TRACE_EXPORT_PATH)
            .header("content-type", "application/grpc")
            .body(body)
            .unwrap();

        let response = svc.call(request).await.unwrap();
        assert_eq!(response.headers()["content-type"], "application/grpc");
        let collected = response.into_body().collect().await.expect("ok body");
        let framed = collected.to_bytes();
        assert_eq!(&framed[..5], &[0u8, 0, 0, 0, 0], "empty response frame");

        let spans = collector.spans_for_node(None, 10).await;
        assert_eq!(spans.len(), 1);
    }

    /// Unknown paths get the canonical UNIMPLEMENTED gRPC status via body error.
    #[tokio::test]
    async fn service_returns_unimplemented_for_unknown_path() {
        use http_body_util::BodyExt;

        let collector = OtelCollector::new("http://localhost:1".into());
        let mut svc = service(collector.clone());

        let request = http::Request::builder()
            .method("POST")
            .uri("/some.unknown.Service/Method")
            .body(tonic::body::empty_body())
            .unwrap();

        let response = svc.call(request).await.unwrap();
        let err = response
            .into_body()
            .collect()
            .await
            .expect_err("status body error");
        assert_eq!(err.code(), tonic::Code::Unimplemented);
    }

    /// The metrics path accepts anything and returns an empty ok frame.
    #[tokio::test]
    async fn service_routes_metrics_path_to_stub() {
        use http_body_util::BodyExt;

        let collector = OtelCollector::new("http://localhost:1".into());
        let mut svc = service(collector.clone());

        let body = tonic::body::boxed(http_body_util::Full::new(bytes::Bytes::from(grpc_frame(
            b"whatever",
        ))));
        let request = http::Request::builder()
            .method("POST")
            .uri(METRICS_EXPORT_PATH)
            .body(body)
            .unwrap();

        let response = svc.call(request).await.unwrap();
        let collected = response.into_body().collect().await.expect("ok body");
        assert_eq!(&collected.to_bytes()[..5], &[0u8, 0, 0, 0, 0]);
    }
}
