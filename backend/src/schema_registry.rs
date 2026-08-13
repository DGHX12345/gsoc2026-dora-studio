//! Arrow schema registry for port type compatibility checking.
//!
//! Maps (operator_type, port_name) → expected Arrow data type.
//! Used by the dataflow editor to show green/red/yellow connection lines.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArrowType {
    Int8,
    Int16,
    Int32,
    Int64,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    Float32,
    Float64,
    Utf8,
    Binary,
    Bool,
    Image,           // encoded image bytes
    PointCloud,      // xyz + optional intensity
    Bboxes,          // detection bounding boxes
    JointState,      // joint angle array
    Trajectory,      // waypoints × joints matrix
    Pose,            // x,y,z,qx,qy,qz,qw
    Json,            // JSON-encoded data
    Unknown(String), // unregistered type name
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortSchema {
    pub port_name: String,
    pub port_type: ArrowType,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRequest {
    pub source_operator: String,
    pub source_port: String,
    pub sink_operator: String,
    pub sink_port: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResponse {
    pub compatible: bool,
    pub level: String, // "compatible" | "warning" | "incompatible" | "unknown"
    pub detail: String,
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

pub struct SchemaRegistry {
    /// input_schemas[operator_id][port_name] → ArrowType
    input_schemas: BTreeMap<String, BTreeMap<String, ArrowType>>,
    /// output_schemas[operator_id][port_name] → ArrowType
    output_schemas: BTreeMap<String, BTreeMap<String, ArrowType>>,
}

impl SchemaRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            input_schemas: BTreeMap::new(),
            output_schemas: BTreeMap::new(),
        };
        reg.register_builtin();
        reg
    }

    /// Built-in schema catalog for common dora operator types.
    fn register_builtin(&mut self) {
        // Perception
        self.register_operator("camera_driver", &[], &[("image", ArrowType::Image)]);
        self.register_operator(
            "lidar_driver",
            &[],
            &[("pointcloud", ArrowType::PointCloud)],
        );
        self.register_operator(
            "object_detection",
            &[("image", ArrowType::Image)],
            &[("bboxes", ArrowType::Bboxes)],
        );

        // Planning
        self.register_operator(
            "planner",
            &[("scene_update", ArrowType::Json)],
            &[
                ("trajectory", ArrowType::Trajectory),
                ("plan_status", ArrowType::Json),
            ],
        );
        self.register_operator(
            "path_follower",
            &[("waypoints", ArrowType::Float32), ("pose", ArrowType::Pose)],
            &[("cmd_vel", ArrowType::Float32)],
        );

        // Control
        self.register_operator(
            "controller",
            &[("joint_commands", ArrowType::JointState)],
            &[("joint_positions", ArrowType::JointState)],
        );
        self.register_operator(
            "trajectory_executor",
            &[
                ("trajectory", ArrowType::Trajectory),
                ("joint_positions", ArrowType::JointState),
            ],
            &[
                ("joint_commands", ArrowType::JointState),
                ("execution_status", ArrowType::Json),
            ],
        );

        // LLM
        self.register_operator(
            "vlm_node",
            &[
                ("image", ArrowType::Image),
                ("prompt_template", ArrowType::Utf8),
            ],
            &[
                ("response", ArrowType::Utf8),
                ("action_vector", ArrowType::Float32),
            ],
        );
        self.register_operator(
            "llm_node",
            &[("prompt", ArrowType::Utf8)],
            &[("response", ArrowType::Utf8)],
        );

        // Hardware
        self.register_operator(
            "motor_driver",
            &[("cmd_vel", ArrowType::Float32)],
            &[("odom", ArrowType::Pose)],
        );
        self.register_operator(
            "gripper_driver",
            &[("gripper_cmd", ArrowType::Float32)],
            &[("gripper_state", ArrowType::Float32)],
        );
    }

    pub fn register_operator(
        &mut self,
        id: &str,
        inputs: &[(&str, ArrowType)],
        outputs: &[(&str, ArrowType)],
    ) {
        let in_map: BTreeMap<String, ArrowType> = inputs
            .iter()
            .map(|(name, t)| (name.to_string(), t.clone()))
            .collect();
        let out_map: BTreeMap<String, ArrowType> = outputs
            .iter()
            .map(|(name, t)| (name.to_string(), t.clone()))
            .collect();
        self.input_schemas.insert(id.to_string(), in_map);
        self.output_schemas.insert(id.to_string(), out_map);
    }

    pub fn register_operator_input(&mut self, operator: &str, port: &str, t: ArrowType) {
        self.input_schemas
            .entry(operator.to_string())
            .or_default()
            .insert(port.to_string(), t);
    }

    pub fn register_operator_output(&mut self, operator: &str, port: &str, t: ArrowType) {
        self.output_schemas
            .entry(operator.to_string())
            .or_default()
            .insert(port.to_string(), t);
    }

    pub fn get_input_type(&self, operator: &str, port: &str) -> Option<&ArrowType> {
        self.input_schemas.get(operator)?.get(port)
    }

    pub fn get_output_type(&self, operator: &str, port: &str) -> Option<&ArrowType> {
        self.output_schemas.get(operator)?.get(port)
    }

    /// Check compatibility between a source output and sink input.
    pub fn check(&self, req: &CheckRequest) -> CheckResponse {
        let src_type = self.get_output_type(&req.source_operator, &req.source_port);
        let sink_type = self.get_input_type(&req.sink_operator, &req.sink_port);

        match (src_type, sink_type) {
            (None, _) | (_, None) => CheckResponse {
                compatible: true,
                level: "unknown".into(),
                detail: format!(
                    "Schema not registered for {}:{} or {}:{}. Connection allowed.",
                    req.source_operator, req.source_port, req.sink_operator, req.sink_port
                ),
            },
            (Some(src), Some(sink)) => {
                let compat = type_compatible(src, sink);
                CheckResponse {
                    compatible: compat.0,
                    level: compat.1.into(),
                    detail: compat.2,
                }
            }
        }
    }

    pub fn list_operators(&self) -> Vec<String> {
        let mut ids: std::collections::BTreeSet<String> =
            self.input_schemas.keys().cloned().collect();
        ids.extend(self.output_schemas.keys().cloned());
        ids.into_iter().collect()
    }

    pub fn operator_schemas(&self, operator: &str) -> Option<OperatorSchemas> {
        let inputs = self.input_schemas.get(operator)?;
        let outputs = self.output_schemas.get(operator)?;
        Some(OperatorSchemas {
            operator: operator.to_string(),
            inputs: inputs
                .iter()
                .map(|(n, t)| PortSchema {
                    port_name: n.clone(),
                    port_type: t.clone(),
                    description: None,
                })
                .collect(),
            outputs: outputs
                .iter()
                .map(|(n, t)| PortSchema {
                    port_name: n.clone(),
                    port_type: t.clone(),
                    description: None,
                })
                .collect(),
        })
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorSchemas {
    pub operator: String,
    pub inputs: Vec<PortSchema>,
    pub outputs: Vec<PortSchema>,
}

// ---------------------------------------------------------------------------
// Compatibility logic
// ---------------------------------------------------------------------------

/// Returns (compatible, level, detail).
/// Level: "compatible" | "warning" | "incompatible"
fn type_compatible(src: &ArrowType, sink: &ArrowType) -> (bool, &'static str, String) {
    if src == sink {
        return (
            true,
            "compatible",
            format!("Both ports are {src:?} — exact match."),
        );
    }

    // Widening numeric conversions (no data loss)
    if is_widening(src, sink) {
        return (
            true,
            "compatible",
            format!("Widening conversion: {src:?} → {sink:?} (no data loss)."),
        );
    }

    // Narrowing numeric conversions (potential data loss)
    if is_narrowing(src, sink) {
        return (
            true,
            "warning",
            format!("Narrowing conversion: {src:?} → {sink:?} (potential data loss)."),
        );
    }

    // Float32 → Float64 is widening (sink wider than src)
    if src == &ArrowType::Float32 && sink == &ArrowType::Float64 {
        return (
            true,
            "compatible",
            "Float32 → Float64 (widening, no data loss).".into(),
        );
    }

    // Definitely incompatible
    (
        false,
        "incompatible",
        format!("Type mismatch: {src:?} cannot be connected to {sink:?}."),
    )
}

fn is_widening(src: &ArrowType, sink: &ArrowType) -> bool {
    use ArrowType::*;
    matches!(
        (src, sink),
        (Int8, Int16 | Int32 | Int64)
            | (Int16, Int32 | Int64)
            | (Int32, Int64)
            | (UInt8, UInt16 | UInt32 | UInt64)
            | (UInt16, UInt32 | UInt64)
            | (UInt32, UInt64)
    )
}

fn is_narrowing(src: &ArrowType, sink: &ArrowType) -> bool {
    use ArrowType::*;
    matches!(
        (src, sink),
        (Int64, Int32 | Int16 | Int8)
            | (Int32, Int16 | Int8)
            | (Int16, Int8)
            | (UInt64, UInt32 | UInt16 | UInt8)
            | (UInt32, UInt16 | UInt8)
            | (UInt16, UInt8)
            | (Float64, Float32)
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match_is_compatible() {
        let reg = SchemaRegistry::new();
        let resp = reg.check(&CheckRequest {
            source_operator: "camera_driver".into(),
            source_port: "image".into(),
            sink_operator: "object_detection".into(),
            sink_port: "image".into(),
        });
        assert!(resp.compatible);
        assert_eq!(resp.level, "compatible");
    }

    #[test]
    fn unknown_operator_is_unknown() {
        let reg = SchemaRegistry::new();
        let resp = reg.check(&CheckRequest {
            source_operator: "custom_op".into(),
            source_port: "data".into(),
            sink_operator: "other_op".into(),
            sink_port: "data".into(),
        });
        assert!(resp.compatible);
        assert_eq!(resp.level, "unknown");
    }

    #[test]
    fn incompatible_types_blocked() {
        let reg = SchemaRegistry::new();
        let resp = reg.check(&CheckRequest {
            source_operator: "camera_driver".into(),
            source_port: "image".into(),
            sink_operator: "llm_node".into(),
            sink_port: "prompt".into(),
        });
        assert!(!resp.compatible);
        assert_eq!(resp.level, "incompatible");
    }

    #[test]
    fn float32_to_float64_is_ok() {
        let mut reg = SchemaRegistry::new();
        reg.register_operator("producer", &[], &[("value", ArrowType::Float32)]);
        reg.register_operator("consumer", &[("value", ArrowType::Float64)], &[]);
        let resp = reg.check(&CheckRequest {
            source_operator: "producer".into(),
            source_port: "value".into(),
            sink_operator: "consumer".into(),
            sink_port: "value".into(),
        });
        assert!(resp.compatible);
    }

    #[test]
    fn list_operators_returns_all() {
        let reg = SchemaRegistry::new();
        let ops = reg.list_operators();
        assert!(ops.contains(&"camera_driver".to_string()));
        assert!(ops.contains(&"planner".to_string()));
        assert!(ops.contains(&"vlm_node".to_string()));
    }

    #[test]
    fn operator_schemas_returns_inputs_and_outputs() {
        let reg = SchemaRegistry::new();
        let schemas = reg.operator_schemas("camera_driver").unwrap();
        assert!(schemas.inputs.is_empty());
        assert_eq!(schemas.outputs.len(), 1);
        assert_eq!(schemas.outputs[0].port_name, "image");
    }
}
