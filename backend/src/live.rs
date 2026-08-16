//! Live data feed (M15 B3) — ring buffer of frames ingested by the
//! studio_bridge dora node, served to the frontend LiveFeed poller.

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, RwLock};

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

pub const MAX_PENDING_COMMANDS: usize = 64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveCommand {
    pub seq: u64,
    pub kind: String,
    pub planner: Option<String>,
    pub target: Option<Vec<f64>>,
    pub action: Option<String>,
    pub object: Option<Value>,
}

pub struct CommandQueue {
    next_seq: u64,
    pending: VecDeque<LiveCommand>,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self {
            // seq starts at 1: consumers poll with since_seq=0 and must
            // see the very first command (take_since filters seq > since).
            next_seq: 1,
            pending: VecDeque::new(),
        }
    }

    pub fn push(
        &mut self,
        kind: &str,
        planner: Option<String>,
        target: Option<Vec<f64>>,
        action: Option<String>,
        object: Option<Value>,
    ) -> Result<LiveCommand, IngestError> {
        match kind {
            "plan" => {
                let len = target.as_ref().map(|t| t.len()).unwrap_or(0);
                if len < 2 {
                    return Err(IngestError(
                        "plan command requires a target of at least [x, y]".to_string(),
                    ));
                }
            }
            "execute" | "stop" | "auto" => {}
            "scene" => {
                if action.is_none() {
                    return Err(IngestError("scene command requires an action".to_string()));
                }
                if object.is_none() {
                    return Err(IngestError("scene command requires an object".to_string()));
                }
            }
            other => {
                return Err(IngestError(format!("unknown command kind: {other}")));
            }
        }
        let command = LiveCommand {
            seq: self.next_seq,
            kind: kind.to_string(),
            planner,
            target,
            action,
            object,
        };
        self.next_seq += 1;
        if self.pending.len() >= MAX_PENDING_COMMANDS {
            self.pending.pop_front();
        }
        self.pending.push_back(command.clone());
        Ok(command)
    }

    pub fn take_since(&self, since_seq: u64) -> Vec<LiveCommand> {
        self.pending
            .iter()
            .filter(|c| c.seq > since_seq)
            .cloned()
            .collect()
    }

    /// The seq the next pushed command will get. Consumers use it to
    /// detect a backend restart (their watermark is beyond it).
    pub fn next_seq_value(&self) -> u64 {
        self.next_seq
    }
}

pub struct LiveFeed {
    streams: RwLock<HashMap<String, VecDeque<LiveFrame>>>,
    stream_order: RwLock<VecDeque<String>>,
    commands: Mutex<CommandQueue>,
}

impl LiveFeed {
    pub fn new() -> Self {
        Self {
            streams: RwLock::new(HashMap::new()),
            stream_order: RwLock::new(VecDeque::new()),
            commands: Mutex::new(CommandQueue::new()),
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
        // Newest first: limit keeps the freshest frames, not the oldest.
        frames.sort_by_key(|f| std::cmp::Reverse(f.timestamp));
        frames.truncate(limit);
        frames
    }

    pub fn stream_count(&self) -> usize {
        self.streams.read().unwrap().len()
    }

    pub fn push_command(
        &self,
        kind: &str,
        planner: Option<String>,
        target: Option<Vec<f64>>,
        action: Option<String>,
        object: Option<Value>,
    ) -> Result<LiveCommand, IngestError> {
        self.commands
            .lock()
            .unwrap()
            .push(kind, planner, target, action, object)
    }

    pub fn take_commands(&self, since_seq: u64) -> Vec<LiveCommand> {
        self.commands.lock().unwrap().take_since(since_seq)
    }

    pub fn next_command_seq(&self) -> u64 {
        self.commands.lock().unwrap().next_seq_value()
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
        assert_eq!(
            frames[0].timestamp,
            (PER_STREAM_CAPACITY + 10) as u64
        );
        assert_eq!(frames.last().unwrap().timestamp, 11);
    }

    #[test]
    fn recent_merges_streams_newest_first() {
        let feed = LiveFeed::new();
        feed.ingest(frame("b", "y", 300, json!(1))).unwrap();
        feed.ingest(frame("a", "x", 100, json!(2))).unwrap();
        feed.ingest(frame("a", "x", 200, json!(3))).unwrap();
        let frames = feed.recent(None, None, 500);
        let ts: Vec<u64> = frames.iter().map(|f| f.timestamp).collect();
        assert_eq!(ts, vec![300, 200, 100]);
    }

    #[test]
    fn recent_limit_keeps_the_newest_frames() {
        let feed = LiveFeed::new();
        feed.ingest(frame("a", "x", 100, json!(1))).unwrap();
        feed.ingest(frame("a", "x", 200, json!(2))).unwrap();
        feed.ingest(frame("a", "x", 300, json!(3))).unwrap();
        let frames = feed.recent(Some("a/x"), None, 2);
        let ts: Vec<u64> = frames.iter().map(|f| f.timestamp).collect();
        assert_eq!(ts, vec![300, 200]);
    }

    #[test]
    fn recent_respects_limit() {
        let feed = LiveFeed::new();
        for i in 1..=10u64 {
            feed.ingest(frame("a", "x", i, json!(i))).unwrap();
        }
        let frames = feed.recent(None, None, 3);
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].timestamp, 10);
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

    #[test]
    fn command_push_assigns_increasing_seq() {
        let feed = LiveFeed::new();
        let c1 = feed
            .push_command("plan", None, Some(vec![0.5, 0.2]), None, None)
            .unwrap();
        let c2 = feed
            .push_command("execute", None, None, None, None)
            .unwrap();
        assert_eq!(c1.seq, 1);
        assert_eq!(c2.seq, 2);
    }

    #[test]
    fn next_seq_tracks_commands_for_restart_detection() {
        let feed = LiveFeed::new();
        assert_eq!(feed.next_command_seq(), 1);
        feed.push_command("execute", None, None, None, None).unwrap();
        feed.push_command("stop", None, None, None, None).unwrap();
        assert_eq!(feed.next_command_seq(), 3);
    }

    #[test]
    fn first_command_is_visible_to_a_fresh_poller() {
        // A poller starting at seq 0 must see the very first command —
        // take_since filters strictly newer seqs, so seq 0 itself is
        // invisible (regression: the console node lost the first command).
        let feed = LiveFeed::new();
        feed.push_command("plan", None, Some(vec![0.5, 0.2]), None, None).unwrap();
        let commands = feed.take_commands(0);
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].seq, 1);
        // and a poller that already saw it gets nothing new
        assert!(feed.take_commands(1).is_empty());
    }

    #[test]
    fn take_commands_returns_only_newer_seq() {
        let feed = LiveFeed::new();
        feed.push_command("plan", None, Some(vec![0.1, 0.2]), None, None).unwrap();
        feed.push_command("execute", None, None, None, None).unwrap();
        feed.push_command("stop", None, None, None, None).unwrap();
        let newer = feed.take_commands(2);
        assert_eq!(newer.len(), 1);
        assert_eq!(newer[0].kind, "stop");
        assert_eq!(newer[0].seq, 3);
    }

    #[test]
    fn take_commands_is_empty_without_new_commands() {
        let feed = LiveFeed::new();
        feed.push_command("execute", None, None, None, None).unwrap();
        assert_eq!(feed.take_commands(0).len(), 1);
        assert!(feed.take_commands(1).is_empty());
        assert!(feed.take_commands(5).is_empty());
    }

    #[test]
    fn command_queue_caps_pending_commands() {
        let feed = LiveFeed::new();
        for i in 0..(MAX_PENDING_COMMANDS + 10) {
            feed.push_command("execute", None, None, None, None).unwrap();
        }
        let all = feed.take_commands(0);
        assert_eq!(all.len(), MAX_PENDING_COMMANDS);
        assert_eq!(all[0].seq, 11, "oldest commands evicted");
    }

    #[test]
    fn plan_command_requires_target() {
        let feed = LiveFeed::new();
        assert!(feed.push_command("plan", None, None, None, None).is_err());
        assert!(feed.push_command("plan", None, Some(vec![0.5]), None, None).is_err());
        assert!(feed.push_command("plan", None, Some(vec![0.5, 0.2]), None, None).is_ok());
    }

    #[test]
    fn scene_command_requires_action_and_object() {
        let feed = LiveFeed::new();
        assert!(feed.push_command("scene", None, None, None, None).is_err());
        assert!(feed
            .push_command("scene", None, None, Some("add".into()), None)
            .is_err());
        assert!(feed
            .push_command(
                "scene",
                None,
                None,
                Some("add".into()),
                Some(json!({"name": "box1", "type": "box"})),
            )
            .is_ok());
    }

    #[test]
    fn unknown_command_kind_is_rejected() {
        let feed = LiveFeed::new();
        assert!(feed.push_command("explode", None, None, None, None).is_err());
    }
}
