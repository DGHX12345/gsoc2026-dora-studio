//! Synthetic `.drec` file generator for tests and development.
//!
//! Used when no real dora recordings are available. Generates valid binary
//! `.drec` files conforming to the format in `types.rs`.

use std::io::Write;

use crate::drec::types::{
    RecordEntry, RecordingFooter, RecordingHeader, FOOTER_MAGIC, MAGIC, MAX_RECORD_BYTES,
};

/// Generate a synthetic `.drec` file.
pub struct DrecGenerator;

impl DrecGenerator {
    /// Write a complete `.drec` file to `writer`: header, entries, footer.
    pub fn write_to<W: Write>(
        writer: &mut W,
        header: &RecordingHeader,
        entries: &[RecordEntry],
    ) -> Result<RecordingFooter, String> {
        if header.descriptor_yaml.len() > MAX_RECORD_BYTES as usize {
            return Err(format!(
                "descriptor YAML too large: {} bytes (max {MAX_RECORD_BYTES})",
                header.descriptor_yaml.len()
            ));
        }

        // Header
        writer.write_all(MAGIC).map_err(|e| e.to_string())?;
        writer
            .write_all(&header.version.to_le_bytes())
            .map_err(|e| e.to_string())?;
        writer
            .write_all(&header.start_nanos.to_le_bytes())
            .map_err(|e| e.to_string())?;
        writer
            .write_all(header.dataflow_id.as_bytes())
            .map_err(|e| e.to_string())?;
        let yaml_len = header.descriptor_yaml.len() as u32;
        writer
            .write_all(&yaml_len.to_le_bytes())
            .map_err(|e| e.to_string())?;
        writer
            .write_all(&header.descriptor_yaml)
            .map_err(|e| e.to_string())?;

        let mut total_messages: u64 = 0;
        let mut total_bytes: u64 = 0;

        // Entries
        for entry in entries {
            let record_bytes = write_entry(writer, entry)?;
            total_messages += 1;
            total_bytes += record_bytes as u64;
        }

        // Footer
        let footer = RecordingFooter {
            total_messages,
            total_bytes,
        };
        writer.write_all(FOOTER_MAGIC).map_err(|e| e.to_string())?;
        writer
            .write_all(&footer.total_messages.to_le_bytes())
            .map_err(|e| e.to_string())?;
        writer
            .write_all(&footer.total_bytes.to_le_bytes())
            .map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;

        Ok(footer)
    }

    /// Generate a recording with N nodes, each producing evenly-spaced entries.
    pub fn generate_multi_stream(
        nodes: &[&str],
        entries_per_node: usize,
        interval_nanos: u64,
    ) -> (RecordingHeader, Vec<RecordEntry>) {
        let header = RecordingHeader {
            version: 1,
            start_nanos: 1_000_000_000,
            dataflow_id: uuid::Uuid::new_v4(),
            descriptor_yaml: format!("nodes: [{}]", nodes.join(", ")).into_bytes(),
        };

        let mut entries = Vec::with_capacity(nodes.len() * entries_per_node);
        for (ni, node) in nodes.iter().enumerate() {
            for ei in 0..entries_per_node {
                entries.push(RecordEntry {
                    node_id: node.to_string(),
                    output_id: format!("output_{ni}"),
                    timestamp_offset_nanos: (ei as u64) * interval_nanos,
                    event_bytes: format!("seq={ei},node={node}").into_bytes(),
                });
            }
        }
        // Sort by timestamp for realistic ordering
        entries.sort_by_key(|e| e.timestamp_offset_nanos);

        (header, entries)
    }

    /// Generate a synthetic recording with VLM attribution chains (camera →
    /// prompt → LLM response → parsed action → execution result) plus
    /// joint-state entries that drive the 3D viewport. Every 5th chain fails.
    pub fn generate_vlm_attribution(
        chain_count: usize,
        interval_nanos: u64,
    ) -> (RecordingHeader, Vec<RecordEntry>) {
        use crate::attribution::{AttributionEvent, AttributionStep};

        let header = RecordingHeader {
            version: 1,
            start_nanos: 1_000_000_000,
            dataflow_id: uuid::Uuid::new_v4(),
            descriptor_yaml: b"nodes: [cam, vlm, executor, robot_state]".to_vec(),
        };

        const TASKS: [&str; 4] = [
            "Pick up the red cube and place it in the bin.",
            "Move the gripper above the blue cylinder and close it.",
            "Push the yellow box toward the left edge of the table.",
            "Open the drawer and place the screwdriver inside.",
        ];
        const RESPONSES: [&str; 4] = [
            "Target at (0.42, -0.18, 0.31); approach from above, close gripper, lift 5 cm, place in bin.",
            "Cylinder at (0.30, 0.22, 0.28); align gripper, close to 60%, lift and hold.",
            "Push along -X with gentle force; stop at table edge marker.",
            "Drawer handle at (0.55, 0.10, 0.25); grasp handle, pull 20 cm, place tool inside.",
        ];

        let mut entries = Vec::with_capacity(chain_count * 6);
        for i in 0..chain_count {
            let ts = (i as u64) * interval_nanos;
            let task = TASKS[i % TASKS.len()];
            let response = RESPONSES[i % RESPONSES.len()];
            let token_count = task.split_whitespace().count() as u32;
            let resp_tokens = response.split_whitespace().count() as u32;
            let latency_ms = 850 + ((i as u32) * 37) % 400;
            let failed = i % 5 == 4;

            // 6-joint target from a sine sweep; joint-state entries use the
            // same angles so the 3D viewport motion matches the parsed action.
            let mut vector = Vec::with_capacity(6);
            for j in 0..6 {
                let angle = ((i as f32) * 0.18 + (j as f32) * 0.8).sin() * 0.6;
                vector.push((angle * 1000.0).round() / 1000.0);
            }
            let confidence = 0.82 + ((i as u32) % 17) as f32 / 100.0;

            entries.push(RecordEntry {
                node_id: "cam".to_string(),
                output_id: "frame".to_string(),
                timestamp_offset_nanos: ts,
                event_bytes: AttributionEvent {
                    frame_timestamp_nanos: ts,
                    step: AttributionStep::SensorFrame {
                        topic: "camera/color".to_string(),
                        width: 640,
                        height: 480,
                        encoding: "jpeg".to_string(),
                    },
                }
                .encode(),
            });
            entries.push(RecordEntry {
                node_id: "vlm".to_string(),
                output_id: "attribution".to_string(),
                timestamp_offset_nanos: ts + interval_nanos / 20,
                event_bytes: AttributionEvent {
                    frame_timestamp_nanos: ts,
                    step: AttributionStep::Prompt {
                        text: task.to_string(),
                        token_count,
                    },
                }
                .encode(),
            });
            entries.push(RecordEntry {
                node_id: "vlm".to_string(),
                output_id: "attribution".to_string(),
                timestamp_offset_nanos: ts
                    + interval_nanos / 20 * 2
                    + latency_ms as u64 * 1_000_000,
                event_bytes: AttributionEvent {
                    frame_timestamp_nanos: ts,
                    step: AttributionStep::LlmResponse {
                        text: response.to_string(),
                        token_count: resp_tokens,
                        model: "qwen2.5-vl-7b".to_string(),
                        latency_ms,
                    },
                }
                .encode(),
            });
            entries.push(RecordEntry {
                node_id: "vlm".to_string(),
                output_id: "attribution".to_string(),
                timestamp_offset_nanos: ts
                    + interval_nanos / 20 * 3
                    + latency_ms as u64 * 1_000_000,
                event_bytes: AttributionEvent {
                    frame_timestamp_nanos: ts,
                    step: AttributionStep::ParsedAction {
                        action_type: "joint_target".to_string(),
                        vector: vector.clone(),
                        confidence,
                    },
                }
                .encode(),
            });
            entries.push(RecordEntry {
                node_id: "executor".to_string(),
                output_id: "result".to_string(),
                timestamp_offset_nanos: ts
                    + interval_nanos / 20 * 4
                    + latency_ms as u64 * 1_000_000,
                event_bytes: AttributionEvent {
                    frame_timestamp_nanos: ts,
                    step: AttributionStep::ExecutionResult {
                        success: !failed,
                        error_message: if failed {
                            Some("Gripper collision detected near target pose".to_string())
                        } else {
                            None
                        },
                    },
                }
                .encode(),
            });

            let joints = serde_json::json!({
                "joints": {
                    "joint_1": vector[0], "joint_2": vector[1], "joint_3": vector[2],
                    "joint_4": vector[3], "joint_5": vector[4], "joint_6": vector[5],
                },
                "basePose": { "x": 0.0, "y": 0.0, "yaw": 0.0 },
            });
            entries.push(RecordEntry {
                node_id: "robot_state".to_string(),
                output_id: "joint_state".to_string(),
                timestamp_offset_nanos: ts + interval_nanos / 20,
                event_bytes: serde_json::to_vec(&joints).unwrap(),
            });
        }
        entries.sort_by_key(|e| e.timestamp_offset_nanos);

        (header, entries)
    }
}

fn write_entry<W: Write>(writer: &mut W, entry: &RecordEntry) -> Result<usize, String> {
    let node_bytes = entry.node_id.as_bytes();
    let output_bytes = entry.output_id.as_bytes();

    let record_len: usize =
        2 + node_bytes.len() + 2 + output_bytes.len() + 8 + 4 + entry.event_bytes.len();

    if record_len > MAX_RECORD_BYTES as usize {
        return Err(format!("record too large: {record_len} bytes"));
    }

    let node_len = u16::try_from(node_bytes.len())
        .map_err(|_| format!("node_id too long: {} bytes", node_bytes.len()))?;
    let output_len = u16::try_from(output_bytes.len())
        .map_err(|_| format!("output_id too long: {} bytes", output_bytes.len()))?;

    writer
        .write_all(&(record_len as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;
    writer
        .write_all(&node_len.to_le_bytes())
        .map_err(|e| e.to_string())?;
    writer.write_all(node_bytes).map_err(|e| e.to_string())?;
    writer
        .write_all(&output_len.to_le_bytes())
        .map_err(|e| e.to_string())?;
    writer.write_all(output_bytes).map_err(|e| e.to_string())?;
    writer
        .write_all(&entry.timestamp_offset_nanos.to_le_bytes())
        .map_err(|e| e.to_string())?;
    writer
        .write_all(&(entry.event_bytes.len() as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;
    writer
        .write_all(&entry.event_bytes)
        .map_err(|e| e.to_string())?;

    Ok(record_len + 4) // +4 for the record_len prefix
}

impl DrecGenerator {
    /// Generate a synthetic recording with only joint-state entries (6-joint
    /// sine animation) and no attribution data — used for empty-state testing.
    pub fn generate_joint_animation(
        frame_count: usize,
        interval_nanos: u64,
    ) -> (RecordingHeader, Vec<RecordEntry>) {
        let header = RecordingHeader {
            version: 1,
            start_nanos: 1_000_000_000,
            dataflow_id: uuid::Uuid::new_v4(),
            descriptor_yaml: b"nodes: [robot_state]".to_vec(),
        };

        let mut entries = Vec::with_capacity(frame_count);
        for i in 0..frame_count {
            let joints = serde_json::json!({
                "joints": {
                    "joint_1": ((i as f32) * 0.18).sin() * 0.6,
                    "joint_2": ((i as f32) * 0.18 + 0.8).sin() * 0.6,
                    "joint_3": ((i as f32) * 0.18 + 1.6).sin() * 0.6,
                    "joint_4": ((i as f32) * 0.18 + 2.4).sin() * 0.6,
                    "joint_5": ((i as f32) * 0.18 + 3.2).sin() * 0.6,
                    "joint_6": ((i as f32) * 0.18 + 4.0).sin() * 0.6,
                },
                "basePose": { "x": 0.0, "y": 0.0, "yaw": 0.0 },
            });
            entries.push(RecordEntry {
                node_id: "robot_state".to_string(),
                output_id: "joint_state".to_string(),
                timestamp_offset_nanos: (i as u64) * interval_nanos,
                event_bytes: serde_json::to_vec(&joints).unwrap(),
            });
        }

        (header, entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_empty_recording() {
        let header = RecordingHeader {
            version: 1,
            start_nanos: 0,
            dataflow_id: uuid::Uuid::nil(),
            descriptor_yaml: b"nodes: []".to_vec(),
        };
        let mut buf = Vec::new();
        let footer = DrecGenerator::write_to(&mut buf, &header, &[]).unwrap();
        assert_eq!(footer.total_messages, 0);
        // Should still have footer
        assert!(buf.len() > 28); // header (38) + footer (24)
    }

    #[test]
    fn generate_deterministic() {
        let (header, entries) = DrecGenerator::generate_multi_stream(&["a", "b"], 5, 100);
        assert_eq!(entries.len(), 10);
        let mut buf = Vec::new();
        let footer = DrecGenerator::write_to(&mut buf, &header, &entries).unwrap();
        assert_eq!(footer.total_messages, 10);
        assert!(footer.total_bytes > 0);
        assert!(footer.total_bytes < buf.len() as u64);
    }

    #[test]
    fn generate_joint_animation_produces_joint_json_entries() {
        let (_header, entries) = DrecGenerator::generate_joint_animation(10, 100_000_000);
        assert_eq!(entries.len(), 10);
        assert_eq!(entries[0].node_id, "robot_state");
        assert_eq!(entries[0].output_id, "joint_state");
        assert_eq!(entries[0].timestamp_offset_nanos, 0);
        assert_eq!(entries[9].timestamp_offset_nanos, 900_000_000);

        let data: serde_json::Value = serde_json::from_slice(&entries[0].event_bytes).unwrap();
        assert!(data["joints"]["joint_1"].is_number());
        assert_eq!(data["joints"].as_object().unwrap().len(), 6);
        assert!(data["basePose"]["x"].is_number());

        // Contains no attribution payloads
        for entry in &entries {
            assert!(!entry.event_bytes.starts_with(crate::attribution::MAGIC));
        }
    }

    #[test]
    fn writes_joint_animation_demo_file() {
        let (header, entries) = DrecGenerator::generate_joint_animation(132, 33_333_333);
        let dir = std::env::temp_dir().join("dora-studio-tests");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("joint_animation.drec");
        let mut file = std::fs::File::create(&path).unwrap();
        DrecGenerator::write_to(&mut file, &header, &entries).unwrap();
        assert!(path.exists());
        // Kept for manual empty-state testing
    }
}
