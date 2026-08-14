//! Live data feed (M15 B3) — ring buffer of frames ingested by the
//! studio_bridge dora node, served to the frontend LiveFeed poller.

use std::collections::{HashMap, VecDeque};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_FRAME_LIMIT: usize = 500;
pub const MAX_FRAME_LIMIT: usize = 5000;
const PER_STREAM_CAPACITY: usize = 4096;
const MAX_STREAMS: usize = 64;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiveFrame {
    pub node_id: String,
    pub output_id: String,
    pub timestamp: u64,
    pub payload: Value,
}

#[derive(Debug, Deserialize)]
pub struct IngestRequest {
    pub node_id: String,
    pub output_id: String,
    pub timestamp: u64,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecentResponse {
    pub frames: Vec<LiveFrame>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IngestError(pub String);

pub struct LiveFeed {
    streams: RwLock<HashMap<String, VecDeque<LiveFrame>>>,
    stream_order: RwLock<VecDeque<String>>,
}

impl LiveFeed {
    pub fn new() -> Self {
        Self {
            streams: RwLock::new(HashMap::new()),
            stream_order: RwLock::new(VecDeque::new()),
        }
    }

    pub fn ingest(&self, frame: LiveFrame) -> Result<(), IngestError> {
        if frame.node_id.is_empty() {
            return Err(IngestError("node_id must not be empty".to_string()));
        }
        if frame.output_id.is_empty() {
            return Err(IngestError("output_id must not be empty".to_string()));
        }
        if frame.timestamp == 0 {
            return Err(IngestError("timestamp must be positive".to_string()));
        }
        let key = format!("{}/{}", frame.node_id, frame.output_id);
        let mut streams = self.streams.write().unwrap();
        if !streams.contains_key(&key) {
            if streams.len() >= MAX_STREAMS {
                let mut order = self.stream_order.write().unwrap();
                if let Some(evicted) = order.pop_front() {
                    streams.remove(&evicted);
                }
            }
            self.stream_order.write().unwrap().push_back(key.clone());
        }
        let buffer = streams.entry(key).or_default();
        if buffer.len() >= PER_STREAM_CAPACITY {
            buffer.pop_front();
        }
        buffer.push_back(frame);
        Ok(())
    }

    pub fn recent(
        &self,
        stream: Option<&str>,
        since_ts: Option<u64>,
        limit: usize,
    ) -> Vec<LiveFrame> {
        let streams = self.streams.read().unwrap();
        let mut frames: Vec<LiveFrame> = Vec::new();
        for (key, buffer) in streams.iter() {
            if let Some(stream) = stream {
                if key != stream {
                    continue;
                }
            }
            for frame in buffer.iter() {
                if let Some(since) = since_ts {
                    if frame.timestamp <= since {
                        continue;
                    }
                }
                frames.push(frame.clone());
            }
        }
        frames.sort_by_key(|f| f.timestamp);
        frames.truncate(limit);
        frames
    }

    pub fn stream_count(&self) -> usize {
        self.streams.read().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn frame(node: &str, output: &str, ts: u64, payload: Value) -> LiveFrame {
        LiveFrame {
            node_id: node.to_string(),
            output_id: output.to_string(),
            timestamp: ts,
            payload,
        }
    }

    #[test]
    fn ingest_then_recent_returns_frame() {
        let feed = LiveFeed::new();
        feed.ingest(frame("planner", "trajectory", 100, json!({"values": [1.0, 2.0]})))
            .unwrap();
        let frames = feed.recent(None, None, 500);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].node_id, "planner");
        assert_eq!(frames[0].output_id, "trajectory");
        assert_eq!(frames[0].timestamp, 100);
        assert_eq!(frames[0].payload, json!({"values": [1.0, 2.0]}));
    }

    #[test]
    fn recent_filters_by_since_ts_strictly_newer() {
        let feed = LiveFeed::new();
        feed.ingest(frame("a", "x", 100, json!(1))).unwrap();
        feed.ingest(frame("a", "x", 200, json!(2))).unwrap();
        feed.ingest(frame("a", "x", 300, json!(3))).unwrap();
        let frames = feed.recent(None, Some(200), 500);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].timestamp, 300);
    }

    #[test]
    fn recent_filters_by_stream_key() {
        let feed = LiveFeed::new();
        feed.ingest(frame("planner", "trajectory", 100, json!(1))).unwrap();
        feed.ingest(frame("planner", "plan_status", 100, json!(2))).unwrap();
        let frames = feed.recent(Some("planner/trajectory"), None, 500);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].output_id, "trajectory");
    }

    #[test]
    fn ring_buffer_caps_per_stream_dropping_oldest() {
        let feed = LiveFeed::new();
        for i in 1..=(PER_STREAM_CAPACITY + 10) as u64 {
            feed.ingest(frame("a", "x", i, json!(i))).unwrap();
        }
        let frames = feed.recent(Some("a/x"), None, MAX_FRAME_LIMIT);
        assert_eq!(frames.len(), PER_STREAM_CAPACITY);
        assert_eq!(frames[0].timestamp, 11);
        assert_eq!(
            frames.last().unwrap().timestamp,
            (PER_STREAM_CAPACITY + 10) as u64
        );
    }

    #[test]
    fn recent_merges_streams_sorted_by_timestamp() {
        let feed = LiveFeed::new();
        feed.ingest(frame("b", "y", 300, json!(1))).unwrap();
        feed.ingest(frame("a", "x", 100, json!(2))).unwrap();
        feed.ingest(frame("a", "x", 200, json!(3))).unwrap();
        let frames = feed.recent(None, None, 500);
        let ts: Vec<u64> = frames.iter().map(|f| f.timestamp).collect();
        assert_eq!(ts, vec![100, 200, 300]);
    }

    #[test]
    fn recent_respects_limit() {
        let feed = LiveFeed::new();
        for i in 1..=10u64 {
            feed.ingest(frame("a", "x", i, json!(i))).unwrap();
        }
        let frames = feed.recent(None, None, 3);
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].timestamp, 1);
    }

    #[test]
    fn ingest_rejects_invalid_frames() {
        let feed = LiveFeed::new();
        assert!(feed.ingest(frame("", "x", 100, json!(1))).is_err());
        assert!(feed.ingest(frame("a", "", 100, json!(1))).is_err());
        assert!(feed.ingest(frame("a", "x", 0, json!(1))).is_err());
        assert_eq!(feed.stream_count(), 0);
    }

    #[test]
    fn stream_count_capped_at_max_streams() {
        let feed = LiveFeed::new();
        for i in 0..(MAX_STREAMS + 5) {
            feed.ingest(frame(&format!("node{i}"), "x", 100, json!(1)))
                .unwrap();
        }
        assert_eq!(feed.stream_count(), MAX_STREAMS);
    }

    #[test]
    fn recent_returns_empty_for_unknown_stream() {
        let feed = LiveFeed::new();
        feed.ingest(frame("a", "x", 100, json!(1))).unwrap();
        assert!(feed.recent(Some("b/y"), None, 500).is_empty());
    }
}
