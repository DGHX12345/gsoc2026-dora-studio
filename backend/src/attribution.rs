//! VLM/LLM attribution — causal chain extraction from .drec recordings.
//!
//! Studio attribution payload format (not Arrow IPC — see plans2.0/M09):
//! magic "DORAATT\0" (8) + version u16 le + kind u8 + frame_timestamp u64 le
//! + kind-specific length-prefixed fields. `frame_timestamp_nanos` is relative
//! to the recording start (matches the PlaybackEngine seek space).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::drec::reader::DrecReader;
use crate::drec::service::RecordingHandle;

pub const MAGIC: &[u8; 8] = b"DORAATT\0";
pub const VERSION: u16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    NotAttribution,
    ArrowIpc,
    UnsupportedVersion(u16),
    UnknownKind(u8),
    Truncated,
    BadUtf8,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAttribution => write!(f, "not an attribution payload"),
            Self::ArrowIpc => write!(f, "Arrow IPC payload"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported attribution version {v}"),
            Self::UnknownKind(k) => write!(f, "unknown attribution kind {k}"),
            Self::Truncated => write!(f, "truncated payload"),
            Self::BadUtf8 => write!(f, "invalid UTF-8 in payload"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AttributionStep {
    SensorFrame {
        topic: String,
        width: u32,
        height: u32,
        encoding: String,
    },
    Prompt {
        text: String,
        token_count: u32,
    },
    LlmResponse {
        text: String,
        token_count: u32,
        model: String,
        latency_ms: u32,
    },
    ParsedAction {
        action_type: String,
        vector: Vec<f32>,
        confidence: Option<f32>,
    },
    ExecutionResult {
        success: bool,
        error_message: Option<String>,
    },
}

impl AttributionStep {
    fn kind(&self) -> u8 {
        match self {
            Self::SensorFrame { .. } => 1,
            Self::Prompt { .. } => 2,
            Self::LlmResponse { .. } => 3,
            Self::ParsedAction { .. } => 4,
            Self::ExecutionResult { .. } => 5,
        }
    }

    fn order(&self) -> u8 {
        self.kind()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttributionEvent {
    pub frame_timestamp_nanos: u64,
    pub step: AttributionStep,
}

impl AttributionEvent {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.push(self.step.kind());
        out.extend_from_slice(&self.frame_timestamp_nanos.to_le_bytes());
        match &self.step {
            AttributionStep::SensorFrame {
                topic,
                width,
                height,
                encoding,
            } => {
                put_str16(&mut out, topic);
                out.extend_from_slice(&width.to_le_bytes());
                out.extend_from_slice(&height.to_le_bytes());
                put_str16(&mut out, encoding);
            }
            AttributionStep::Prompt { text, token_count } => {
                put_str32(&mut out, text);
                out.extend_from_slice(&token_count.to_le_bytes());
            }
            AttributionStep::LlmResponse {
                text,
                token_count,
                model,
                latency_ms,
            } => {
                put_str32(&mut out, text);
                out.extend_from_slice(&token_count.to_le_bytes());
                put_str16(&mut out, model);
                out.extend_from_slice(&latency_ms.to_le_bytes());
            }
            AttributionStep::ParsedAction {
                action_type,
                vector,
                confidence,
            } => {
                put_str16(&mut out, action_type);
                out.extend_from_slice(&(vector.len() as u16).to_le_bytes());
                for v in vector {
                    out.extend_from_slice(&v.to_le_bytes());
                }
                match confidence {
                    Some(c) => {
                        out.push(1);
                        out.extend_from_slice(&c.to_le_bytes());
                    }
                    None => out.push(0),
                }
            }
            AttributionStep::ExecutionResult {
                success,
                error_message,
            } => {
                out.push(u8::from(*success));
                match error_message {
                    Some(msg) => put_str16(&mut out, msg),
                    None => out.extend_from_slice(&0u16.to_le_bytes()),
                }
            }
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ParseError> {
        let mut cur = Cursor { bytes, pos: 0 };

        let magic = cur.read(8)?;
        if magic == [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF] {
            return Err(ParseError::ArrowIpc);
        }
        if magic == [0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0] {
            return Err(ParseError::ArrowIpc);
        }
        if magic != *MAGIC {
            return Err(ParseError::NotAttribution);
        }

        let version = cur.read_u16()?;
        if version != VERSION {
            return Err(ParseError::UnsupportedVersion(version));
        }

        let kind = cur.read_u8()?;
        let frame_timestamp_nanos = cur.read_u64()?;

        let step = match kind {
            1 => AttributionStep::SensorFrame {
                topic: cur.read_str16()?,
                width: cur.read_u32()?,
                height: cur.read_u32()?,
                encoding: cur.read_str16()?,
            },
            2 => AttributionStep::Prompt {
                text: cur.read_str32()?,
                token_count: cur.read_u32()?,
            },
            3 => AttributionStep::LlmResponse {
                text: cur.read_str32()?,
                token_count: cur.read_u32()?,
                model: cur.read_str16()?,
                latency_ms: cur.read_u32()?,
            },
            4 => AttributionStep::ParsedAction {
                action_type: cur.read_str16()?,
                vector: {
                    let len = cur.read_u16()? as usize;
                    let mut vector = Vec::with_capacity(len);
                    for _ in 0..len {
                        vector.push(cur.read_f32()?);
                    }
                    vector
                },
                confidence: {
                    let present = cur.read_u8()? != 0;
                    if present {
                        Some(cur.read_f32()?)
                    } else {
                        None
                    }
                },
            },
            5 => AttributionStep::ExecutionResult {
                success: cur.read_u8()? != 0,
                error_message: {
                    let err = cur.read_str16()?;
                    if err.is_empty() {
                        None
                    } else {
                        Some(err)
                    }
                },
            },
            other => return Err(ParseError::UnknownKind(other)),
        };

        Ok(Self {
            frame_timestamp_nanos,
            step,
        })
    }
}

fn put_str16(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u16).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

fn put_str32(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u32).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn read(&mut self, n: usize) -> Result<&'a [u8], ParseError> {
        let end = self.pos.checked_add(n).ok_or(ParseError::Truncated)?;
        if end > self.bytes.len() {
            return Err(ParseError::Truncated);
        }
        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn read_u8(&mut self) -> Result<u8, ParseError> {
        Ok(self.read(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, ParseError> {
        Ok(u16::from_le_bytes(self.read(2)?.try_into().unwrap()))
    }

    fn read_u32(&mut self) -> Result<u32, ParseError> {
        Ok(u32::from_le_bytes(self.read(4)?.try_into().unwrap()))
    }

    fn read_u64(&mut self) -> Result<u64, ParseError> {
        Ok(u64::from_le_bytes(self.read(8)?.try_into().unwrap()))
    }

    fn read_f32(&mut self) -> Result<f32, ParseError> {
        Ok(f32::from_le_bytes(self.read(4)?.try_into().unwrap()))
    }

    fn read_str16(&mut self) -> Result<String, ParseError> {
        let len = self.read_u16()? as usize;
        String::from_utf8(self.read(len)?.to_vec()).map_err(|_| ParseError::BadUtf8)
    }

    fn read_str32(&mut self) -> Result<String, ParseError> {
        let len = self.read_u32()? as usize;
        String::from_utf8(self.read(len)?.to_vec()).map_err(|_| ParseError::BadUtf8)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributionChain {
    pub timestamp_nanos: u64,
    pub steps: Vec<AttributionStep>,
}

impl AttributionChain {
    /// None when no ExecutionResult step is recorded (e.g. LeRobot frames);
    /// otherwise Some(true) unless a result reports failure.
    pub fn success(&self) -> Option<bool> {
        let results: Vec<bool> = self
            .steps
            .iter()
            .filter_map(|s| match s {
                AttributionStep::ExecutionResult { success, .. } => Some(*success),
                _ => None,
            })
            .collect();
        if results.is_empty() {
            None
        } else {
            Some(results.into_iter().all(|s| s))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnparseableStream {
    pub node_id: String,
    pub output_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributionChainSummary {
    pub timestamp_nanos: u64,
    pub success: Option<bool>,
    pub step_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributionSummary {
    pub chains: Vec<AttributionChainSummary>,
    pub unparseable_streams: Vec<UnparseableStream>,
}

impl From<&AttributionExtractor> for AttributionSummary {
    fn from(extractor: &AttributionExtractor) -> Self {
        Self {
            chains: extractor
                .chains
                .iter()
                .map(|c| AttributionChainSummary {
                    timestamp_nanos: c.timestamp_nanos,
                    success: c.success(),
                    step_count: c.steps.len(),
                })
                .collect(),
            unparseable_streams: extractor.unparseable.clone(),
        }
    }
}

pub struct AttributionExtractor {
    chains: Vec<AttributionChain>,
    unparseable: Vec<UnparseableStream>,
}

impl AttributionExtractor {
    /// Single sequential scan: decode every entry's payload, group attribution
    /// events by frame timestamp, and order steps into causal chains.
    pub fn from_recording(handle: &RecordingHandle) -> Result<Self, String> {
        let mut reader =
            DrecReader::open(&handle.path).map_err(|e| format!("failed to open recording: {e}"))?;

        let mut groups: BTreeMap<u64, Vec<AttributionStep>> = BTreeMap::new();
        let mut unparseable: BTreeMap<(String, String), String> = BTreeMap::new();

        reader
            .scan_entries(|_offset, entry| {
                match AttributionEvent::decode(&entry.event_bytes) {
                    Ok(event) => {
                        groups
                            .entry(event.frame_timestamp_nanos)
                            .or_default()
                            .push(event.step);
                    }
                    Err(ParseError::ArrowIpc) => {
                        unparseable
                            .entry((entry.node_id.clone(), entry.output_id.clone()))
                            .or_insert_with(|| {
                                "Arrow IPC payload (real dora VLM output); parsing requires Arrow support not available in this build".to_string()
                            });
                    }
                    Err(ParseError::NotAttribution) => {}
                    Err(e) => {
                        unparseable
                            .entry((entry.node_id.clone(), entry.output_id.clone()))
                            .or_insert_with(|| format!("unparseable attribution payload: {e}"));
                    }
                }
            })
            .map_err(|e| format!("failed to scan recording: {e}"))?;

        let chains = groups
            .into_iter()
            .map(|(timestamp_nanos, mut steps)| {
                steps.sort_by_key(AttributionStep::order);
                AttributionChain {
                    timestamp_nanos,
                    steps,
                }
            })
            .collect();

        let unparseable = unparseable
            .into_iter()
            .map(|((node_id, output_id), reason)| UnparseableStream {
                node_id,
                output_id,
                reason,
            })
            .collect();

        Ok(Self {
            chains,
            unparseable,
        })
    }

    pub fn chains(&self) -> &[AttributionChain] {
        &self.chains
    }

    pub fn chain_at(&self, timestamp_ns: u64) -> Option<&AttributionChain> {
        self.chains
            .iter()
            .find(|c| c.timestamp_nanos == timestamp_ns)
    }

    pub fn unparseable_streams(&self) -> &[UnparseableStream] {
        &self.unparseable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drec::generator::DrecGenerator;
    use crate::drec::service::RecordingManager;
    use crate::drec::types::{RecordEntry, RecordingHeader};
    use std::path::PathBuf;

    fn temp_file_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("dora-studio-tests");
        std::fs::create_dir_all(&dir).ok();
        dir.join(name)
    }

    fn write_file(path: &std::path::Path, header: &RecordingHeader, entries: &[RecordEntry]) {
        let mut file = std::fs::File::create(path).unwrap();
        DrecGenerator::write_to(&mut file, header, entries).unwrap();
    }

    // --- payload codec ---

    #[test]
    fn encode_decode_roundtrip_sensor_frame() {
        let event = AttributionEvent {
            frame_timestamp_nanos: 1_000,
            step: AttributionStep::SensorFrame {
                topic: "camera/color".to_string(),
                width: 640,
                height: 480,
                encoding: "jpeg".to_string(),
            },
        };
        let decoded = AttributionEvent::decode(&event.encode()).expect("decode");
        assert_eq!(decoded, event);
    }

    #[test]
    fn encode_decode_roundtrip_prompt() {
        let event = AttributionEvent {
            frame_timestamp_nanos: 2_000,
            step: AttributionStep::Prompt {
                text: "Pick up the red cube.".to_string(),
                token_count: 5,
            },
        };
        let decoded = AttributionEvent::decode(&event.encode()).expect("decode");
        assert_eq!(decoded, event);
    }

    #[test]
    fn encode_decode_roundtrip_llm_response() {
        let event = AttributionEvent {
            frame_timestamp_nanos: 2_100,
            step: AttributionStep::LlmResponse {
                text: "Move arm to target and close gripper.".to_string(),
                token_count: 8,
                model: "qwen2.5-vl-7b".to_string(),
                latency_ms: 920,
            },
        };
        let decoded = AttributionEvent::decode(&event.encode()).expect("decode");
        assert_eq!(decoded, event);
    }

    #[test]
    fn encode_decode_roundtrip_parsed_action() {
        let event = AttributionEvent {
            frame_timestamp_nanos: 2_200,
            step: AttributionStep::ParsedAction {
                action_type: "joint_target".to_string(),
                vector: vec![0.42, -0.18, 0.31, 0.05, 0.0, 1.2],
                confidence: Some(0.94),
            },
        };
        let decoded = AttributionEvent::decode(&event.encode()).expect("decode");
        assert_eq!(decoded, event);

        let no_confidence = AttributionEvent {
            frame_timestamp_nanos: 2_200,
            step: AttributionStep::ParsedAction {
                action_type: "joint_target".to_string(),
                vector: vec![0.1, 0.2],
                confidence: None,
            },
        };
        let decoded = AttributionEvent::decode(&no_confidence.encode()).expect("decode");
        assert_eq!(decoded, no_confidence);
    }

    #[test]
    fn encode_decode_roundtrip_execution_result() {
        let ok = AttributionEvent {
            frame_timestamp_nanos: 2_300,
            step: AttributionStep::ExecutionResult {
                success: false,
                error_message: Some("Gripper collision detected".to_string()),
            },
        };
        let decoded = AttributionEvent::decode(&ok.encode()).expect("decode");
        assert_eq!(decoded, ok);

        let no_error = AttributionEvent {
            frame_timestamp_nanos: 2_300,
            step: AttributionStep::ExecutionResult {
                success: true,
                error_message: None,
            },
        };
        let decoded = AttributionEvent::decode(&no_error.encode()).expect("decode");
        assert_eq!(decoded, no_error);
    }

    #[test]
    fn decode_rejects_arrow_ipc_magic() {
        let mut bytes = vec![0xFF, 0xFF, 0xFF, 0xFF];
        bytes.extend_from_slice(&[0u8; 16]);
        match AttributionEvent::decode(&bytes) {
            Err(ParseError::ArrowIpc) => {}
            other => panic!("expected ArrowIpc, got {other:?}"),
        }
    }

    #[test]
    fn decode_rejects_unknown_magic() {
        match AttributionEvent::decode(b"NOTATTR!") {
            Err(ParseError::NotAttribution) => {}
            other => panic!("expected NotAttribution, got {other:?}"),
        }
    }

    #[test]
    fn decode_rejects_truncated_payload() {
        let event = AttributionEvent {
            frame_timestamp_nanos: 7,
            step: AttributionStep::Prompt {
                text: "hello world".to_string(),
                token_count: 2,
            },
        };
        let bytes = event.encode();
        match AttributionEvent::decode(&bytes[..bytes.len() - 3]) {
            Err(ParseError::Truncated) => {}
            other => panic!("expected Truncated, got {other:?}"),
        }
    }

    #[test]
    fn decode_rejects_unknown_kind() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.push(9);
        bytes.extend_from_slice(&0u64.to_le_bytes());
        match AttributionEvent::decode(&bytes) {
            Err(ParseError::UnknownKind(9)) => {}
            other => panic!("expected UnknownKind(9), got {other:?}"),
        }
    }

    // --- chain assembly ---

    #[tokio::test]
    async fn extractor_builds_ordered_chains_from_recording() {
        let (header, entries) = DrecGenerator::generate_vlm_attribution(3, 100_000_000);
        let path = temp_file_path("attribution_extract.drec");
        write_file(&path, &header, &entries);

        let mgr = RecordingManager::new();
        let handle = mgr.open(&path).await.unwrap();
        let extractor = AttributionExtractor::from_recording(&handle).expect("extract");

        let chains = extractor.chains();
        assert_eq!(chains.len(), 3);
        for chain in chains {
            assert_eq!(chain.steps.len(), 5);
            assert!(matches!(
                chain.steps[0],
                AttributionStep::SensorFrame { .. }
            ));
            assert!(matches!(chain.steps[1], AttributionStep::Prompt { .. }));
            assert!(matches!(
                chain.steps[2],
                AttributionStep::LlmResponse { .. }
            ));
            assert!(matches!(
                chain.steps[3],
                AttributionStep::ParsedAction { .. }
            ));
            assert!(matches!(
                chain.steps[4],
                AttributionStep::ExecutionResult { .. }
            ));
        }
        assert!(chains
            .windows(2)
            .all(|w| w[0].timestamp_nanos < w[1].timestamp_nanos));
        assert_eq!(chains[0].timestamp_nanos, 0);
        assert!(extractor.chain_at(chains[1].timestamp_nanos).is_some());
        assert!(extractor.chain_at(999_999_999).is_none());
        std::fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn extractor_reports_arrow_ipc_streams_as_unparseable() {
        let header = RecordingHeader {
            version: 1,
            start_nanos: 0,
            dataflow_id: uuid::Uuid::new_v4(),
            descriptor_yaml: b"nodes: [vlm]".to_vec(),
        };
        let event = AttributionEvent {
            frame_timestamp_nanos: 0,
            step: AttributionStep::Prompt {
                text: "hi".to_string(),
                token_count: 1,
            },
        };
        let mut arrow_bytes = vec![0xFF, 0xFF, 0xFF, 0xFF];
        arrow_bytes.extend_from_slice(&[0u8; 32]);
        let entries = vec![
            RecordEntry {
                node_id: "vlm".to_string(),
                output_id: "attribution".to_string(),
                timestamp_offset_nanos: 0,
                event_bytes: event.encode(),
            },
            RecordEntry {
                node_id: "vlm".to_string(),
                output_id: "raw".to_string(),
                timestamp_offset_nanos: 0,
                event_bytes: arrow_bytes,
            },
        ];
        let path = temp_file_path("attribution_arrow.drec");
        write_file(&path, &header, &entries);

        let mgr = RecordingManager::new();
        let handle = mgr.open(&path).await.unwrap();
        let extractor = AttributionExtractor::from_recording(&handle).expect("extract");

        assert_eq!(extractor.chains().len(), 1);
        assert_eq!(extractor.unparseable_streams().len(), 1);
        let unparseable = &extractor.unparseable_streams()[0];
        assert_eq!(unparseable.node_id, "vlm");
        assert_eq!(unparseable.output_id, "raw");
        assert!(unparseable.reason.contains("Arrow IPC"));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn chain_success_flag_reflects_execution_result() {
        let ok = AttributionChain {
            timestamp_nanos: 0,
            steps: vec![AttributionStep::ExecutionResult {
                success: true,
                error_message: None,
            }],
        };
        assert_eq!(ok.success(), Some(true));

        let failed = AttributionChain {
            timestamp_nanos: 0,
            steps: vec![AttributionStep::ExecutionResult {
                success: false,
                error_message: Some("collision".to_string()),
            }],
        };
        assert_eq!(failed.success(), Some(false));

        let no_result = AttributionChain {
            timestamp_nanos: 0,
            steps: vec![AttributionStep::Prompt {
                text: "incomplete".to_string(),
                token_count: 1,
            }],
        };
        assert_eq!(no_result.success(), None);
    }

    #[test]
    fn chain_json_uses_camel_case_field_names() {
        let chain = AttributionChain {
            timestamp_nanos: 5,
            steps: vec![
                AttributionStep::LlmResponse {
                    text: "ok".to_string(),
                    token_count: 1,
                    model: "m".to_string(),
                    latency_ms: 10,
                },
                AttributionStep::ParsedAction {
                    action_type: "joint_target".to_string(),
                    vector: vec![0.5],
                    confidence: None,
                },
            ],
        };
        let json = serde_json::to_value(&chain).unwrap();
        let step = &json["steps"][0];
        assert_eq!(step["kind"], "llmResponse");
        assert_eq!(step["tokenCount"], 1);
        assert_eq!(step["latencyMs"], 10);
        assert!(step.get("token_count").is_none());
        assert!(json["steps"][1]["confidence"].is_null());
    }

    #[test]
    fn summary_maps_chains_and_unparseable() {
        let extractor = AttributionExtractor {
            chains: vec![
                AttributionChain {
                    timestamp_nanos: 10,
                    steps: vec![
                        AttributionStep::Prompt {
                            text: "a".to_string(),
                            token_count: 1,
                        },
                        AttributionStep::ExecutionResult {
                            success: true,
                            error_message: None,
                        },
                    ],
                },
                AttributionChain {
                    timestamp_nanos: 20,
                    steps: vec![AttributionStep::ExecutionResult {
                        success: false,
                        error_message: Some("boom".to_string()),
                    }],
                },
            ],
            unparseable: vec![UnparseableStream {
                node_id: "vlm".to_string(),
                output_id: "raw".to_string(),
                reason: "Arrow IPC payload".to_string(),
            }],
        };
        let summary = AttributionSummary::from(&extractor);
        assert_eq!(summary.chains.len(), 2);
        assert_eq!(summary.chains[0].success, Some(true));
        assert_eq!(summary.chains[0].step_count, 2);
        assert_eq!(summary.chains[0].timestamp_nanos, 10);
        assert_eq!(summary.chains[1].success, Some(false));
        assert_eq!(summary.unparseable_streams.len(), 1);
    }

    #[tokio::test]
    async fn generator_writes_attribution_demo_file() {
        let (header, entries) = DrecGenerator::generate_vlm_attribution(40, 100_000_000);
        // Keep the file for manual UI testing (like sample.drec / joint_animation.drec)
        let path = temp_file_path("attribution_demo.drec");
        write_file(&path, &header, &entries);

        let mgr = RecordingManager::new();
        let handle = mgr.open(&path).await.unwrap();
        let extractor = AttributionExtractor::from_recording(&handle).unwrap();
        assert_eq!(extractor.chains().len(), 40);
        assert!(extractor.unparseable_streams().is_empty());
    }
}
