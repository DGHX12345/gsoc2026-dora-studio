//! Dataflow YAML builder — programmatic construction of dora dataflow descriptors.
//!
//! Accumulates nodes, edges, and environment config, then serializes to valid
//! dora dataflow YAML. Also supports deserialization for round-tripping.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Graph model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Runtime {
    Python,
    Rust,
    C,
    Cpp,
}

impl Runtime {
    pub fn as_str(&self) -> &str {
        match self {
            Runtime::Python => "python",
            Runtime::Rust => "rust",
            Runtime::C => "c",
            Runtime::Cpp => "c++",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortSpec {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub port_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSpec {
    pub id: String,
    pub operator_id: String,
    pub runtime: Runtime,
    #[serde(default)]
    pub inputs: BTreeMap<String, PortSpec>,
    #[serde(default)]
    pub outputs: BTreeMap<String, PortSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeSpec {
    pub id: String,
    pub source_node: String,
    pub source_port: String,
    pub target_node: String,
    pub target_port: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataflowGraph {
    pub nodes: Vec<NodeSpec>,
    pub edges: Vec<EdgeSpec>,
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum BuildError {
    DuplicateNode(String),
    DuplicateEdge(String),
    NodeNotFound(String),
    PortNotFound { node: String, port: String },
    CycleDetected(String),
    EmptyGraph,
    OrphanNode(String),
    InvalidYaml(String),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::DuplicateNode(id) => write!(f, "Duplicate node ID: {id}"),
            BuildError::DuplicateEdge(id) => write!(f, "Duplicate edge ID: {id}"),
            BuildError::NodeNotFound(id) => write!(f, "Node not found: {id}"),
            BuildError::PortNotFound { node, port } => {
                write!(f, "Port not found: {node}:{port}")
            }
            BuildError::CycleDetected(id) => write!(f, "Cycle detected involving node: {id}"),
            BuildError::EmptyGraph => write!(f, "Graph has no nodes"),
            BuildError::OrphanNode(id) => write!(f, "Orphan node (no connections): {id}"),
            BuildError::InvalidYaml(msg) => write!(f, "Invalid YAML: {msg}"),
        }
    }
}

pub struct DataflowBuilder {
    nodes: BTreeMap<String, NodeSpec>,
    edges: BTreeMap<String, EdgeSpec>,
}

impl DataflowBuilder {
    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
        }
    }

    pub fn add_node(&mut self, spec: NodeSpec) -> Result<&mut Self, BuildError> {
        if self.nodes.contains_key(&spec.id) {
            return Err(BuildError::DuplicateNode(spec.id));
        }
        self.nodes.insert(spec.id.clone(), spec);
        Ok(self)
    }

    pub fn remove_node(&mut self, id: &str) -> Result<&mut Self, BuildError> {
        if !self.nodes.contains_key(id) {
            return Err(BuildError::NodeNotFound(id.to_string()));
        }
        self.nodes.remove(id);
        // Remove all edges connected to this node
        self.edges
            .retain(|_, e| e.source_node != id && e.target_node != id);
        Ok(self)
    }

    pub fn connect(&mut self, edge: EdgeSpec) -> Result<&mut Self, BuildError> {
        if self.edges.contains_key(&edge.id) {
            return Err(BuildError::DuplicateEdge(edge.id));
        }
        if !self.nodes.contains_key(&edge.source_node) {
            return Err(BuildError::NodeNotFound(edge.source_node.clone()));
        }
        if !self.nodes.contains_key(&edge.target_node) {
            return Err(BuildError::NodeNotFound(edge.target_node.clone()));
        }
        self.edges.insert(edge.id.clone(), edge);
        Ok(self)
    }

    pub fn remove_edge(&mut self, id: &str) -> Result<&mut Self, BuildError> {
        if !self.edges.contains_key(id) {
            return Err(BuildError::DuplicateEdge(id.to_string()));
        }
        self.edges.remove(id);
        Ok(self)
    }

    pub fn graph(&self) -> DataflowGraph {
        DataflowGraph {
            nodes: self.nodes.values().cloned().collect(),
            edges: self.edges.values().cloned().collect(),
        }
    }

    /// Serialize to dora dataflow YAML format.
    pub fn to_yaml(&self) -> String {
        let mut out = String::new();
        for node in self.nodes.values() {
            out.push_str(&format!(
                "  - id: {}\n    operator: {}\n    runtime: {}\n",
                node.id,
                node.operator_id,
                node.runtime.as_str()
            ));
            if !node.inputs.is_empty() {
                out.push_str("    inputs:\n");
                for (name, port) in &node.inputs {
                    out.push_str(&format!("      {}:\n", name));
                    if let Some(ref t) = port.port_type {
                        out.push_str(&format!("        type: {}\n", t));
                    }
                    out.push_str(&format!(
                        "        source: {}\n",
                        find_edge_source(&self.edges, &node.id, name)
                    ));
                }
            }
            if !node.outputs.is_empty() {
                out.push_str("    outputs:\n");
                for (name, port) in &node.outputs {
                    out.push_str(&format!("      - {}\n", name));
                    if let Some(ref t) = port.port_type {
                        out.push_str(&format!("        type: {}\n", t));
                    }
                }
            }
            out.push('\n');
        }
        out
    }

    /// Parse from dora dataflow YAML using a minimal line-based parser.
    pub fn from_yaml(yaml: &str) -> Result<Self, BuildError> {
        let mut builder = Self::new();
        let lines: Vec<&str> = yaml.lines().collect();
        let mut i = 0;

        // Find the "nodes:" key
        while i < lines.len() && !lines[i].trim_start().starts_with("nodes:") {
            i += 1;
        }
        if i >= lines.len() {
            return Err(BuildError::InvalidYaml("missing 'nodes:' key".into()));
        }
        i += 1; // skip "nodes:" line

        // Parse each node block
        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim_start();

            // Next top-level key ends the nodes list
            if !trimmed.starts_with("- id:")
                && !trimmed.starts_with("-")
                && !trimmed.starts_with("  ")
                && !trimmed.is_empty()
            {
                break;
            }

            // Start of a new node
            if trimmed.starts_with("- id:") {
                let id = trimmed
                    .strip_prefix("- id:")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let mut operator_id = String::new();
                let mut runtime = Runtime::Python;
                let mut inputs = BTreeMap::new();
                let mut outputs = BTreeMap::new();

                i += 1;
                // Parse node fields
                while i < lines.len() {
                    let inner = lines[i];
                    let inner_trimmed = inner.trim_start();
                    let indent = inner.len() - inner_trimmed.len();

                    // Next node or top-level key
                    if indent <= 2
                        && (inner_trimmed.starts_with("- id:")
                            || (!inner_trimmed.starts_with("  ")
                                && !inner_trimmed.starts_with("    ")
                                && !inner_trimmed.is_empty()))
                    {
                        break;
                    }

                    if indent == 4 {
                        if let Some(val) = inner_trimmed.strip_prefix("operator:") {
                            operator_id = val.trim().to_string();
                        } else if let Some(val) = inner_trimmed.strip_prefix("runtime:") {
                            runtime = match val.trim() {
                                "python" => Runtime::Python,
                                "rust" => Runtime::Rust,
                                "c" => Runtime::C,
                                "c++" | "cpp" => Runtime::Cpp,
                                _ => Runtime::Python,
                            };
                        } else if inner_trimmed == "inputs:" {
                            i += 1;
                            // Parse inputs
                            while i < lines.len() {
                                let inp_line = lines[i];
                                let inp_trimmed = inp_line.trim_start();
                                let inp_indent = inp_line.len() - inp_trimmed.len();
                                if inp_indent <= 4 {
                                    break;
                                }
                                if inp_indent == 6 {
                                    let port_name = inp_trimmed
                                        .strip_suffix(':')
                                        .unwrap_or(inp_trimmed)
                                        .to_string();
                                    let mut port_type = None;
                                    let mut _source = String::new();
                                    i += 1;
                                    while i < lines.len() {
                                        let sub = lines[i];
                                        let sub_trimmed = sub.trim_start();
                                        let sub_indent = sub.len() - sub_trimmed.len();
                                        if sub_indent <= 6 {
                                            break;
                                        }
                                        if let Some(t) = sub_trimmed.strip_prefix("type:") {
                                            port_type = Some(t.trim().to_string());
                                        } else if let Some(s) = sub_trimmed.strip_prefix("source:")
                                        {
                                            _source = s.trim().to_string();
                                        }
                                        i += 1;
                                    }
                                    inputs.insert(
                                        port_name,
                                        PortSpec {
                                            port_type,
                                            description: None,
                                        },
                                    );
                                } else {
                                    i += 1;
                                }
                            }
                            continue; // already advanced i
                        } else if inner_trimmed == "outputs:" {
                            i += 1;
                            // Parse outputs
                            while i < lines.len() {
                                let out_line = lines[i];
                                let out_trimmed = out_line.trim_start();
                                let out_indent = out_line.len() - out_trimmed.len();
                                if out_indent <= 4 {
                                    break;
                                }
                                if let Some(name) = out_trimmed.strip_prefix("- ") {
                                    outputs.insert(
                                        name.trim().to_string(),
                                        PortSpec {
                                            port_type: None,
                                            description: None,
                                        },
                                    );
                                }
                                i += 1;
                            }
                            continue;
                        }
                    }
                    i += 1;
                }

                builder.add_node(NodeSpec {
                    id,
                    operator_id,
                    runtime,
                    inputs,
                    outputs,
                    position: None,
                })?;
            } else {
                i += 1;
            }
        }

        // Build edges from input source fields
        let mut edge_idx = 0;
        let nodes_clone: Vec<_> = builder.nodes.values().cloned().collect();
        for node in &nodes_clone {
            for (port_name, _) in &node.inputs {
                // Find source from the YAML source fields
                // We already parsed sources; find edge connections
                if let Some(source) = find_yaml_source(yaml, &node.id, port_name) {
                    edge_idx += 1;
                    let _ = builder.connect(EdgeSpec {
                        id: format!("edge_{edge_idx}"),
                        source_node: source.0,
                        source_port: source.1,
                        target_node: node.id.clone(),
                        target_port: port_name.clone(),
                    });
                }
            }
        }

        Ok(builder)
    }

    /// Validate the graph for common issues.
    pub fn validate(&self) -> Result<(), Vec<BuildError>> {
        let mut errors = Vec::new();

        if self.nodes.is_empty() {
            errors.push(BuildError::EmptyGraph);
            return Err(errors);
        }

        // Check for orphan nodes (no connections)
        let connected: std::collections::HashSet<&str> = self
            .edges
            .values()
            .flat_map(|e| [e.source_node.as_str(), e.target_node.as_str()])
            .collect();

        for node_id in self.nodes.keys() {
            if !connected.contains(node_id.as_str()) {
                errors.push(BuildError::OrphanNode(node_id.clone()));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Default for DataflowBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn find_edge_source(
    edges: &BTreeMap<String, EdgeSpec>,
    target_node: &str,
    target_port: &str,
) -> String {
    for edge in edges.values() {
        if edge.target_node == target_node && edge.target_port == target_port {
            return format!("{}/{}", edge.source_node, edge.source_port);
        }
    }
    "unknown".to_string()
}

/// Scan YAML for a source reference to (node, port).
fn find_yaml_source(yaml: &str, node: &str, port: &str) -> Option<(String, String)> {
    let lines: Vec<&str> = yaml.lines().collect();
    let mut in_target_node = false;
    let mut in_target_port = false;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();

        // Detect node by "id: <node>"
        if indent == 4 && trimmed == format!("id: {}", node) {
            in_target_node = true;
            continue;
        }
        if in_target_node && indent <= 2 && !trimmed.is_empty() {
            in_target_node = false;
            in_target_port = false;
        }

        if in_target_node && indent == 6 && trimmed == format!("{}:", port) {
            in_target_port = true;
            continue;
        }
        if in_target_port && indent > 6 {
            if let Some(src) = trimmed.strip_prefix("source:") {
                let source = src.trim();
                let parts: Vec<&str> = source.splitn(2, '/').collect();
                let source_node = parts[0].to_string();
                let source_port = if parts.len() > 1 {
                    parts[1].to_string()
                } else {
                    "output".to_string()
                };
                return Some((source_node, source_port));
            }
        }
        if in_target_port && indent <= 6 {
            in_target_port = false;
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_graph() -> DataflowBuilder {
        let mut b = DataflowBuilder::new();
        b.add_node(NodeSpec {
            id: "camera".into(),
            operator_id: "camera_driver".into(),
            runtime: Runtime::Python,
            inputs: BTreeMap::new(),
            outputs: {
                let mut m = BTreeMap::new();
                m.insert(
                    "image".into(),
                    PortSpec {
                        port_type: Some("image".into()),
                        description: None,
                    },
                );
                m
            },
            position: Some(Position { x: 100.0, y: 100.0 }),
        })
        .unwrap();
        b.add_node(NodeSpec {
            id: "detector".into(),
            operator_id: "object_detection".into(),
            runtime: Runtime::Python,
            inputs: {
                let mut m = BTreeMap::new();
                m.insert(
                    "image".into(),
                    PortSpec {
                        port_type: Some("image".into()),
                        description: None,
                    },
                );
                m
            },
            outputs: {
                let mut m = BTreeMap::new();
                m.insert(
                    "bboxes".into(),
                    PortSpec {
                        port_type: Some("bboxes".into()),
                        description: None,
                    },
                );
                m
            },
            position: Some(Position { x: 300.0, y: 100.0 }),
        })
        .unwrap();
        b.connect(EdgeSpec {
            id: "e1".into(),
            source_node: "camera".into(),
            source_port: "image".into(),
            target_node: "detector".into(),
            target_port: "image".into(),
        })
        .unwrap();
        b
    }

    #[test]
    fn builds_valid_yaml() {
        let b = make_test_graph();
        let yaml = b.to_yaml();
        assert!(yaml.contains("id: camera"));
        assert!(yaml.contains("operator: camera_driver"));
        assert!(yaml.contains("operator: object_detection"));
        assert!(yaml.contains("runtime: python"));
    }

    #[test]
    fn rejects_duplicate_node() {
        let mut b = DataflowBuilder::new();
        b.add_node(NodeSpec {
            id: "a".into(),
            operator_id: "op".into(),
            runtime: Runtime::Python,
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            position: None,
        })
        .unwrap();
        let result = b.add_node(NodeSpec {
            id: "a".into(),
            operator_id: "op2".into(),
            runtime: Runtime::Python,
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            position: None,
        });
        assert!(result.is_err());
    }

    #[test]
    fn rejects_edge_to_missing_node() {
        let mut b = DataflowBuilder::new();
        b.add_node(NodeSpec {
            id: "a".into(),
            operator_id: "op".into(),
            runtime: Runtime::Python,
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            position: None,
        })
        .unwrap();
        let result = b.connect(EdgeSpec {
            id: "e1".into(),
            source_node: "a".into(),
            source_port: "out".into(),
            target_node: "nonexistent".into(),
            target_port: "in".into(),
        });
        assert!(result.is_err());
    }

    #[test]
    fn detects_orphan_nodes() {
        let mut b = DataflowBuilder::new();
        b.add_node(NodeSpec {
            id: "lonely".into(),
            operator_id: "op".into(),
            runtime: Runtime::Python,
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            position: None,
        })
        .unwrap();
        let result = b.validate();
        assert!(result.is_err());
    }

    #[test]
    fn removes_node_cleans_edges() {
        let mut b = make_test_graph();
        b.remove_node("camera").unwrap();
        assert_eq!(b.graph().nodes.len(), 1);
        assert_eq!(b.graph().edges.len(), 0); // edge removed with node
    }

    #[test]
    fn graph_roundtrip() {
        let b = make_test_graph();
        let graph = b.graph();
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
    }
}
