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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortSpec {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub port_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Raw input source for non-node producers (e.g. `dora/timer/millis/500`).
    /// Node-to-node sources are carried by edges, not on the port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSpec {
    pub id: String,
    pub operator_id: String,
    pub runtime: Runtime,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub inputs: BTreeMap<String, PortSpec>,
    #[serde(default)]
    pub outputs: BTreeMap<String, PortSpec>,
    #[serde(default)]
    pub input_types: BTreeMap<String, String>,
    #[serde(default)]
    pub output_types: BTreeMap<String, String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeRuleDef {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataflowGraph {
    pub nodes: Vec<NodeSpec>,
    pub edges: Vec<EdgeSpec>,
    #[serde(default)]
    pub type_rules: Vec<TypeRuleDef>,
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
    pub type_rules: Vec<TypeRuleDef>,
}

impl DataflowBuilder {
    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            type_rules: Vec::new(),
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
            type_rules: self.type_rules.clone(),
        }
    }

    /// Serialize to dora 1.0 dataflow YAML (nodes: header, node-level
    /// input_types/output_types, dataflow-level type_rules).
    pub fn to_yaml(&self) -> String {
        let mut out = String::from("nodes:\n");
        for node in self.nodes.values() {
            out.push_str(&render_node_block(node, &self.edges));
        }
        if !self.type_rules.is_empty() {
            out.push_str("type_rules:\n");
            for rule in &self.type_rules {
                out.push_str(&format!("  - from: {}\n    to: {}\n", rule.from, rule.to));
            }
        }
        out
    }

    /// Parse from dora dataflow YAML using a minimal line-based parser.
    pub fn from_yaml(yaml: &str) -> Result<Self, BuildError> {
        let mut builder = Self::new();
        let type_rules = parse_type_rules_from_yaml(yaml);
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
                let mut node_path: Option<String> = None;
                let mut inputs = BTreeMap::new();
                let mut outputs = BTreeMap::new();
                let mut input_types = BTreeMap::new();
                let mut output_types = BTreeMap::new();

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
                        } else if let Some(path) = inner_trimmed.strip_prefix("path:") {
                            node_path = Some(path.trim().to_string());
                        } else if inner_trimmed == "input_types:" {
                            i += 1;
                            parse_typed_ports(&lines, &mut i, &mut input_types);
                            continue;
                        } else if inner_trimmed == "output_types:" {
                            i += 1;
                            parse_typed_ports(&lines, &mut i, &mut output_types);
                            continue;
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
                                    // Port line is either "name:" (nested
                                    // source form) or the compact
                                    // "name: node/port" form used by many
                                    // dora dataflows; split the name off so
                                    // it round-trips instead of mangling the
                                    // whole line into the port id.
                                    let (port_name, inline_source) = match inp_trimmed
                                        .split_once(':')
                                    {
                                        Some((name, rest)) if !rest.trim().is_empty() => {
                                            (name.trim().to_string(), Some(rest.trim().to_string()))
                                        }
                                        Some((name, _)) => (name.trim().to_string(), None),
                                        None => (inp_trimmed.to_string(), None),
                                    };
                                    let mut port_type = None;
                                    // The compact "name: node/port" form and the
                                    // nested "source:" sub-key both feed the same
                                    // field; node-to-node sources are cleared
                                    // after the full node set is known (edges
                                    // carry those), while non-node sources
                                    // (e.g. dora/timer/...) survive on the port.
                                    let mut source = inline_source.clone().unwrap_or_default();
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
                                            source = s.trim().to_string();
                                        }
                                        i += 1;
                                    }
                                    inputs.insert(
                                        port_name,
                                        PortSpec {
                                            port_type,
                                            description: None,
                                            source: if source.is_empty() {
                                                None
                                            } else {
                                                Some(source)
                                            },
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
                                            source: None,
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
                    path: node_path,
                    inputs,
                    outputs,
                    input_types,
                    output_types,
                    position: None,
                })?;
            } else {
                i += 1;
            }
        }

        // Drop port-level sources that resolve to a graph node: those are
        // node-to-node connections carried by edges, and leaving the raw
        // source on the port would make a stale reference survive node
        // removal in patch_yaml. Non-node sources (dora/timer/... and bare
        // external sources) stay on the port so they re-render verbatim.
        let node_ids: Vec<String> = builder.nodes.keys().cloned().collect();
        for node in builder.nodes.values_mut() {
            for port in node.inputs.values_mut() {
                if let Some(source) = &port.source {
                    let from = source.split('/').next().unwrap_or("");
                    if node_ids.iter().any(|id| id == from) {
                        port.source = None;
                    }
                }
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

        // Backfill PortSpec.port_type from the input_types/output_types maps
        // so the canvas round-trips types on the ports themselves.
        for node in builder.nodes.values_mut() {
            for (name, urn) in &node.input_types {
                if let Some(port) = node.inputs.get_mut(name) {
                    if port.port_type.is_none() {
                        port.port_type = Some(urn.clone());
                    }
                }
            }
            for (name, urn) in &node.output_types {
                if let Some(port) = node.outputs.get_mut(name) {
                    if port.port_type.is_none() {
                        port.port_type = Some(urn.clone());
                    }
                }
            }
        }

        builder.type_rules = type_rules;

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

/// Render one node block (without the surrounding "nodes:" header) in dora
/// 1.0 format: id, optional path, inputs with sources, node-level
/// input_types/output_types. Node-level `runtime` is NOT emitted: dora 1.0
/// removed it from the node schema (the runtime is implied by the path's
/// language), so it is canvas-side state only.
fn render_node_block(node: &NodeSpec, edges: &BTreeMap<String, EdgeSpec>) -> String {
    let mut out = String::new();
    out.push_str(&format!("  - id: {}\n", node.id));
    if let Some(ref path) = node.path {
        out.push_str(&format!("    path: {}\n", path));
    }
    if !node.inputs.is_empty() {
        out.push_str("    inputs:\n");
        for (name, port) in &node.inputs {
            out.push_str(&format!("      {}:\n", name));
            let source = port
                .source
                .clone()
                .unwrap_or_else(|| find_edge_source(edges, &node.id, name));
            out.push_str(&format!("        source: {source}\n"));
        }
    }
    // Merge contract: PortSpec.port_type is the canonical canvas state;
    // input_types/output_types maps are derived/backfill. port_type wins
    // on conflict (chain-collect inserts later entries over earlier ones).
    let input_types: BTreeMap<&String, &String> = node
        .input_types
        .iter()
        .chain(
            node.inputs
                .iter()
                .filter_map(|(name, port)| port.port_type.as_ref().map(|t| (name, t))),
        )
        .collect();
    if !input_types.is_empty() {
        out.push_str("    input_types:\n");
        for (name, urn) in &input_types {
            out.push_str(&format!("      {name}: {urn}\n"));
        }
    }
    if !node.outputs.is_empty() {
        out.push_str("    outputs:\n");
        for name in node.outputs.keys() {
            out.push_str(&format!("      - {}\n", name));
        }
    }
    let output_types: BTreeMap<&String, &String> = node
        .output_types
        .iter()
        .chain(
            node.outputs
                .iter()
                .filter_map(|(name, port)| port.port_type.as_ref().map(|t| (name, t))),
        )
        .collect();
    if !output_types.is_empty() {
        out.push_str("    output_types:\n");
        for (name, urn) in &output_types {
            out.push_str(&format!("      {name}: {urn}\n"));
        }
    }
    out.push('\n');
    out
}

/// Parse the dataflow-level "type_rules:" section (indent 0) into
/// (from, to) pairs. Ignores any other sections.
fn parse_type_rules_from_yaml(yaml: &str) -> Vec<TypeRuleDef> {
    let mut rules = Vec::new();
    let mut in_rules = false;
    let mut current_from: Option<String> = None;
    for raw_line in yaml.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = raw_line.chars().take_while(|ch| ch.is_whitespace()).count();
        if !in_rules {
            if trimmed == "type_rules:" && indent == 0 {
                in_rules = true;
            }
            continue;
        }
        if indent == 0 && trimmed != "type_rules:" {
            break;
        }
        if let Some(from) = trimmed.strip_prefix("- from:") {
            current_from = Some(from.trim().to_string());
        } else if let Some(to) = trimmed.strip_prefix("to:") {
            if let Some(from) = current_from.take() {
                rules.push(TypeRuleDef {
                    from,
                    to: to.trim().to_string(),
                });
            }
        }
    }
    rules
}

/// Parse a node-level `input_types:`/`output_types:` section body (entries at
/// indent 6) into a name -> URN map. Stops at the first line indented <= 4.
/// Empty URNs are filtered, matching dataflows.rs `parse_typed_port`.
fn parse_typed_ports(lines: &[&str], i: &mut usize, map: &mut BTreeMap<String, String>) {
    while *i < lines.len() {
        let t_line = lines[*i];
        let t_trimmed = t_line.trim_start();
        let t_indent = t_line.len() - t_trimmed.len();
        if t_indent <= 4 {
            break;
        }
        if t_indent == 6 {
            if let Some((name, urn)) = t_trimmed.split_once(':') {
                let urn = urn.trim();
                if !urn.is_empty() {
                    map.insert(name.trim().to_string(), urn.to_string());
                }
            }
        }
        *i += 1;
    }
}

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
    let mut in_inputs = false;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();

        // Detect node by "id: <node>": both the plain form at indent 4 and
        // the list item "  - id: <node>" at indent 2 emitted by to_yaml.
        if (indent == 4 && trimmed == format!("id: {}", node))
            || (indent <= 2 && trimmed == format!("- id: {}", node))
        {
            in_target_node = true;
            in_inputs = false;
            continue;
        }
        if in_target_node && indent <= 2 && !trimmed.is_empty() {
            in_target_node = false;
            in_target_port = false;
            in_inputs = false;
        }

        // Track the node's `inputs:` section: the compact `name: node/port`
        // source form only applies there. Without this, `input_types:`
        // entries like `image: std/media/v1/Image` are mistaken for a source
        // reference when input_types precedes inputs, and the real edge is
        // silently dropped (order-dependent).
        if in_target_node && indent == 4 {
            if trimmed == "inputs:" {
                in_inputs = true;
            } else if trimmed == "input_types:"
                || trimmed == "outputs:"
                || trimmed == "output_types:"
            {
                in_inputs = false;
            }
        }

        if in_target_node && indent == 6 && trimmed == format!("{}:", port) {
            in_target_port = true;
            continue;
        }
        // Compact inline source form: `image: cam/image` (older dora).
        if in_target_node && in_inputs && indent == 6 {
            if let Some((name, src)) = trimmed.split_once(':') {
                if name.trim() == port && !src.trim().is_empty() {
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
// Line-based diff patch (M18): rewrite only the regions the canvas owns —
// node blocks and the type_rules section — preserving everything else.
// ---------------------------------------------------------------------------

/// Apply an edited graph onto an existing dataflow YAML, preserving all
/// lines outside node blocks and the type_rules section verbatim.
pub fn patch_yaml(original: &str, graph: &DataflowGraph) -> Result<String, BuildError> {
    let lines: Vec<String> = original.lines().map(str::to_string).collect();

    // 1. Locate the nodes: section and each node block.
    let blocks = find_node_blocks(&lines);
    if blocks.is_empty() && !graph.nodes.is_empty() && !lines.iter().any(|l| l.trim() == "nodes:") {
        // No nodes section at all: fall back to full generation of the
        // canvas-owned regions appended after any existing content.
        let mut out = lines.join("\n");
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("nodes:\n");
        for node in &graph.nodes {
            out.push_str(&render_node_block(node, &edge_map(graph)));
        }
        append_type_rules(&mut out, &graph.type_rules);
        return Ok(out);
    }

    // 2. Replace existing blocks and collect insertion points.
    let mut result: Vec<String> = Vec::new();
    let mut cursor = 0usize;
    let mut inserted_ids: std::collections::BTreeSet<String> =
        graph.nodes.iter().map(|node| node.id.clone()).collect();

    // node blocks in source order
    let mut ordered_blocks: Vec<(usize, usize, String)> = Vec::new();
    for (id, (start, end)) in &blocks {
        ordered_blocks.push((*start, *end, id.clone()));
    }
    ordered_blocks.sort();

    // index graph nodes by id
    let mut graph_nodes: BTreeMap<&str, &NodeSpec> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();

    let mut insert_after: Option<usize> = None; // line index of last node block end

    for (start, end, id) in &ordered_blocks {
        result.extend(lines[cursor..*start].iter().cloned());
        if let Some(node) = graph_nodes.remove(id.as_str()) {
            result.extend(
                render_node_block(node, &edge_map(graph))
                    .lines()
                    .map(str::to_string),
            );
            inserted_ids.remove(id);
        }
        // removed nodes: emit nothing
        cursor = *end + 1;
        insert_after = Some(result.len());
    }
    result.extend(lines[cursor..].iter().cloned());

    // 3. Insert new nodes after the last existing node block (before any
    // trailing top-level sections).
    let new_nodes: Vec<&NodeSpec> = graph
        .nodes
        .iter()
        .filter(|node| inserted_ids.contains(&node.id))
        .collect();
    if !new_nodes.is_empty() {
        let mut rendered = String::new();
        for node in new_nodes {
            rendered.push_str(&render_node_block(node, &edge_map(graph)));
        }
        let insert_at = insert_after
            .map(|index| {
                // find the end of that node's rendered lines in `result`
                // (block content may differ in length, so re-locate by
                // scanning forward to the next top-level line)
                let mut index = index;
                while index < result.len() {
                    let line = &result[index];
                    if !line.trim().is_empty() && !line.starts_with(' ') {
                        break;
                    }
                    index += 1;
                }
                index
            })
            .unwrap_or_else(|| {
                // no existing blocks: insert right after the nodes: line
                result
                    .iter()
                    .position(|line| line.trim() == "nodes:")
                    .map(|index| index + 1)
                    .unwrap_or(0)
            });
        let rendered_lines: Vec<String> = rendered.lines().map(str::to_string).collect();
        for (offset, line) in rendered_lines.into_iter().enumerate() {
            result.insert(insert_at + offset, line);
        }
    }

    // 4. Patch the type_rules top-level section.
    patch_type_rules_section(&mut result, &graph.type_rules);

    Ok(result.join("\n") + "\n")
}

fn edge_map(graph: &DataflowGraph) -> BTreeMap<String, EdgeSpec> {
    graph
        .edges
        .iter()
        .map(|edge| (edge.id.clone(), edge.clone()))
        .collect()
}

fn find_node_blocks(lines: &[String]) -> BTreeMap<String, (usize, usize)> {
    let mut blocks = BTreeMap::new();
    let mut in_nodes = false;
    let mut current: Option<(String, usize)> = None;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed == "nodes:" {
            in_nodes = true;
            continue;
        }
        if in_nodes {
            let indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
            if indent == 0 && !trimmed.is_empty() {
                // Top-level comments between node blocks are not content:
                // keep scanning the nodes section, but close any open block
                // so the comment is not swallowed by the re-rendered block.
                if trimmed.starts_with('#') {
                    if let Some((id, start)) = current.take() {
                        blocks.insert(id, (start, index - 1));
                    }
                    continue;
                }
                in_nodes = false;
                if let Some((id, start)) = current.take() {
                    blocks.insert(id, (start, index - 1));
                }
                continue;
            }
            // Node list items sit at indent 2. A `- id:` deeper inside a
            // nested section (e.g. a custom list) must not be treated as a
            // node block, or the phantom block's lines vanish from the
            // patched output.
            if indent == 2 {
                if let Some(id) = trimmed.strip_prefix("- id:") {
                    if let Some((prev_id, start)) = current.take() {
                        blocks.insert(prev_id, (start, index - 1));
                    }
                    current = Some((id.trim().to_string(), index));
                }
            }
        }
    }
    if let Some((id, start)) = current.take() {
        blocks.insert(id, (start, lines.len() - 1));
    }
    blocks
}

/// Given an index into `result` that sits at a top-level header line, return
/// the index just past that header's indented block (children plus any blank
/// lines). Used to place new top-level sections after the whole block instead
/// of inside it.
fn skip_indented_block(result: &[String], mut index: usize) -> usize {
    if index < result.len() && !result[index].trim().is_empty() && !result[index].starts_with(' ') {
        index += 1;
        while index < result.len()
            && (result[index].starts_with(' ') || result[index].trim().is_empty())
        {
            index += 1;
        }
    }
    index
}

fn patch_type_rules_section(result: &mut Vec<String>, rules: &[TypeRuleDef]) {
    // remove existing section
    let mut start: Option<usize> = None;
    let mut end: Option<usize> = None;
    for (index, line) in result.iter().enumerate() {
        let trimmed = line.trim();
        let indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
        if trimmed == "type_rules:" && indent == 0 {
            start = Some(index);
            continue;
        }
        if start.is_some() && indent == 0 && !trimmed.is_empty() {
            end = Some(index);
            break;
        }
    }
    // When a section already exists, re-insert at its old position so it
    // keeps its place relative to surrounding top-level sections (e.g. a
    // trailing `env:` block stays after type_rules).
    let mut insert_at = match start {
        Some(start) => {
            let end = end.unwrap_or(result.len());
            result.drain(start..end);
            start
        }
        // No existing section: insert after the last top-level section.
        // Never insert between a top-level header and its indented block: the
        // naive insertion point (right after the last top-level line) sits
        // inside that block when the file ends with a section that has
        // children (e.g. a trailing `env:`), so skip past the whole block to
        // append after it.
        None if !rules.is_empty() => result
            .iter()
            .rposition(|line| !line.trim().is_empty() && !line.starts_with(' '))
            .map(|index| skip_indented_block(result, index))
            .unwrap_or(result.len()),
        None => result.len(),
    };
    if !rules.is_empty() {
        let mut section = vec!["type_rules:".to_string()];
        for rule in rules {
            section.push(format!("  - from: {}", rule.from));
            section.push(format!("    to: {}", rule.to));
        }
        for (offset, line) in section.into_iter().enumerate() {
            result.insert(insert_at + offset, line);
        }
    }
}

fn append_type_rules(out: &mut String, rules: &[TypeRuleDef]) {
    if !rules.is_empty() {
        out.push_str("type_rules:\n");
        for rule in rules {
            out.push_str(&format!("  - from: {}\n    to: {}\n", rule.from, rule.to));
        }
    }
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
            path: None,
            inputs: BTreeMap::new(),
            input_types: BTreeMap::new(),
            outputs: {
                let mut m = BTreeMap::new();
                m.insert(
                    "image".into(),
                    PortSpec {
                        port_type: Some("image".into()),
                        description: None,
                        source: None,
                    },
                );
                m
            },
            output_types: BTreeMap::new(),
            position: Some(Position { x: 100.0, y: 100.0 }),
        })
        .unwrap();
        b.add_node(NodeSpec {
            id: "detector".into(),
            operator_id: "object_detection".into(),
            runtime: Runtime::Python,
            path: None,
            inputs: {
                let mut m = BTreeMap::new();
                m.insert(
                    "image".into(),
                    PortSpec {
                        port_type: Some("image".into()),
                        description: None,
                        source: None,
                    },
                );
                m
            },
            input_types: BTreeMap::new(),
            outputs: {
                let mut m = BTreeMap::new();
                m.insert(
                    "bboxes".into(),
                    PortSpec {
                        port_type: Some("bboxes".into()),
                        description: None,
                        source: None,
                    },
                );
                m
            },
            output_types: BTreeMap::new(),
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
        assert!(yaml.starts_with("nodes:\n"));
        assert!(yaml.contains("id: camera"));
        assert!(yaml.contains("output_types:"));
        assert!(yaml.contains("source: camera/image"));
        // dora 1.0 node schema has no node-level `runtime` field.
        assert!(!yaml.contains("runtime:"));
    }

    #[test]
    fn rejects_duplicate_node() {
        let mut b = DataflowBuilder::new();
        b.add_node(NodeSpec {
            id: "a".into(),
            operator_id: "op".into(),
            runtime: Runtime::Python,
            path: None,
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            input_types: BTreeMap::new(),
            output_types: BTreeMap::new(),
            position: None,
        })
        .unwrap();
        let result = b.add_node(NodeSpec {
            id: "a".into(),
            operator_id: "op2".into(),
            runtime: Runtime::Python,
            path: None,
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            input_types: BTreeMap::new(),
            output_types: BTreeMap::new(),
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
            path: None,
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            input_types: BTreeMap::new(),
            output_types: BTreeMap::new(),
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
            path: None,
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            input_types: BTreeMap::new(),
            output_types: BTreeMap::new(),
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

    #[test]
    fn edge_survives_yaml_roundtrip() {
        let b = make_test_graph();
        let yaml = b.to_yaml();
        let parsed = DataflowBuilder::from_yaml(&yaml).expect("roundtrip");
        let graph = parsed.graph();
        assert_eq!(graph.edges.len(), 1);
        let edge = &graph.edges[0];
        assert_eq!(edge.source_node, "camera");
        assert_eq!(edge.source_port, "image");
        assert_eq!(edge.target_node, "detector");
        assert_eq!(edge.target_port, "image");
    }

    #[test]
    fn yaml_includes_nodes_header_and_types() {
        let b = make_test_graph();
        let yaml = b.to_yaml();
        assert!(
            yaml.starts_with("nodes:\n"),
            "dora 1.0 requires nodes: header"
        );
        // old `type:` under ports is migrated to node-level maps
        assert!(yaml.contains("output_types:\n      image: image"));
        assert!(!yaml.contains("        type: image"));
    }

    #[test]
    fn type_rules_roundtrip_in_yaml() {
        let mut b = make_test_graph();
        b.type_rules.push(TypeRuleDef {
            from: "std/core/v1/UInt8".into(),
            to: "std/core/v1/String".into(),
        });
        let yaml = b.to_yaml();
        assert!(yaml.contains("type_rules:"));
        assert!(yaml.contains("  - from: std/core/v1/UInt8"));
        assert!(yaml.contains("    to: std/core/v1/String"));
        let parsed = DataflowBuilder::from_yaml(&yaml).expect("roundtrip");
        assert_eq!(
            parsed.type_rules,
            vec![TypeRuleDef {
                from: "std/core/v1/UInt8".into(),
                to: "std/core/v1/String".into()
            }]
        );
    }

    #[test]
    fn path_field_roundtrips() {
        let mut b = DataflowBuilder::new();
        b.add_node(NodeSpec {
            id: "a".into(),
            operator_id: "a".into(),
            runtime: Runtime::Python,
            path: Some("cam.py".into()),
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            input_types: BTreeMap::new(),
            output_types: BTreeMap::new(),
            position: None,
        })
        .unwrap();
        let yaml = b.to_yaml();
        assert!(yaml.contains("    path: cam.py"));
        let parsed = DataflowBuilder::from_yaml(&yaml).expect("roundtrip");
        assert_eq!(parsed.graph().nodes[0].path.as_deref(), Some("cam.py"));
    }

    #[test]
    fn patch_preserves_comments_and_unknown_fields() {
        let original = r#"# my project dataflow
nodes:
  - id: cam
    path: cam.py
    outputs:
      - image
    output_types:
      image: std/media/v1/Image
  - id: sink
    path: sink.py
    inputs:
      image: cam/image
env:
  RUST_LOG: info
"#;
        let mut b = DataflowBuilder::from_yaml(original).expect("parses");
        // edit: change sink input type declaration
        b.nodes
            .get_mut("sink")
            .unwrap()
            .input_types
            .insert("image".to_string(), "std/core/v1/Bytes".to_string());
        // add a node
        b.add_node(NodeSpec {
            id: "proc".into(),
            operator_id: "proc".into(),
            runtime: Runtime::Python,
            path: Some("proc.py".into()),
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            input_types: BTreeMap::new(),
            output_types: BTreeMap::new(),
            position: None,
        })
        .unwrap();
        // remove cam
        b.remove_node("cam").unwrap();
        let patched = patch_yaml(original, &b.graph()).expect("patches");
        assert!(
            patched.contains("# my project dataflow"),
            "comments preserved"
        );
        assert!(
            patched.contains("env:\n  RUST_LOG: info"),
            "unknown sections preserved"
        );
        assert!(patched.contains("input_types:\n      image: std/core/v1/Bytes"));
        assert!(patched.contains("  - id: proc"), "new node inserted");
        assert!(!patched.contains("  - id: cam"), "removed node gone");
        assert!(
            !patched.contains("cam/image"),
            "stale source references gone"
        );
    }

    #[test]
    fn patch_adds_type_rules_section() {
        let original = "nodes:\n  - id: a\n    path: a.py\n    outputs:\n      - out\n";
        let mut b = DataflowBuilder::from_yaml(original).expect("parses");
        b.type_rules.push(TypeRuleDef {
            from: "x/v1/A".into(),
            to: "x/v1/B".into(),
        });
        let patched = patch_yaml(original, &b.graph()).expect("patches");
        assert!(patched.contains("type_rules:\n  - from: x/v1/A\n    to: x/v1/B\n"));
    }

    #[test]
    fn patch_type_rules_after_trailing_env_section() {
        let original = "nodes:\n  - id: a\n    path: a.py\n    outputs:\n      - out\nenv:\n  RUST_LOG: info\n";
        let mut b = DataflowBuilder::from_yaml(original).expect("parses");
        b.type_rules.push(TypeRuleDef {
            from: "x/v1/A".into(),
            to: "x/v1/B".into(),
        });
        let patched = patch_yaml(original, &b.graph()).expect("patches");
        // env section intact, type_rules AFTER the whole env block
        assert!(patched.contains("env:\n  RUST_LOG: info\n"));
        assert!(patched.contains("type_rules:\n  - from: x/v1/A\n    to: x/v1/B\n"));
        let env_pos = patched.find("env:").unwrap();
        let rules_pos = patched.find("type_rules:").unwrap();
        assert!(
            rules_pos > env_pos,
            "type_rules must come after the env block"
        );
    }

    #[test]
    fn patch_replaces_type_rules_before_trailing_env_section() {
        let original = "nodes:\n  - id: a\n    path: a.py\n    outputs:\n      - out\ntype_rules:\n  - from: x/v1/A\n    to: x/v1/B\nenv:\n  RUST_LOG: info\n";
        let mut b = DataflowBuilder::from_yaml(original).expect("parses");
        assert_eq!(b.type_rules.len(), 1, "existing rule parsed");
        b.type_rules.push(TypeRuleDef {
            from: "x/v1/C".into(),
            to: "x/v1/D".into(),
        });
        let patched = patch_yaml(original, &b.graph()).expect("patches");
        // replaced section stays before env:, env block not mangled
        assert!(patched.contains("env:\n  RUST_LOG: info\n"));
        assert!(patched.contains(
            "type_rules:\n  - from: x/v1/A\n    to: x/v1/B\n  - from: x/v1/C\n    to: x/v1/D\n"
        ));
        let rules_pos = patched.find("type_rules:").unwrap();
        let env_pos = patched.find("env:").unwrap();
        assert!(
            rules_pos < env_pos,
            "replaced type_rules must stay before the env block"
        );
    }

    #[test]
    fn patch_migrates_legacy_inline_type_lines() {
        let original = r#"nodes:
  - id: cam
    path: cam.py
    outputs:
      - image
        type: image
"#;
        let mut b = DataflowBuilder::from_yaml(original).expect("parses");
        b.nodes
            .get_mut("cam")
            .unwrap()
            .output_types
            .insert("image".to_string(), "std/media/v1/Image".to_string());
        let patched = patch_yaml(original, &b.graph()).expect("patches");
        assert!(!patched.contains("type: image"));
        assert!(patched.contains("output_types:\n      image: std/media/v1/Image"));
    }

    #[test]
    fn patch_preserves_edges_when_input_types_precede_inputs() {
        let original = r#"nodes:
  - id: sink
    path: sink.py
    input_types:
      image: std/media/v1/Image
    inputs:
      image: cam/image
  - id: cam
    path: cam.py
    outputs:
      - image
"#;
        let mut b = DataflowBuilder::from_yaml(original).expect("parses");
        assert_eq!(
            b.graph().edges.len(),
            1,
            "edge must survive input_types-before-inputs ordering"
        );
        let sink = b.nodes.get_mut("sink").unwrap();
        // PortSpec.port_type is the canonical type channel (render_node_block
        // merges with port_type winning over input_types), and from_yaml has
        // backfilled it from the fixture's input_types — so update it directly
        // for the type edit to take effect.
        sink.input_types
            .insert("image".into(), "std/core/v1/Bytes".into());
        sink.inputs.get_mut("image").unwrap().port_type = Some("std/core/v1/Bytes".into());
        let patched = patch_yaml(original, &b.graph()).expect("patches");
        assert!(patched.contains("cam/image"), "source must be preserved");
        assert!(patched.contains("std/core/v1/Bytes"));
    }

    #[test]
    fn patch_preserves_timer_input_sources() {
        let original = r#"nodes:
  - id: camera
    path: camera.py
    inputs:
      tick: dora/timer/millis/500
    outputs:
      - frame
"#;
        let mut b = DataflowBuilder::from_yaml(original).expect("parses");
        b.nodes
            .get_mut("camera")
            .unwrap()
            .output_types
            .insert("frame".into(), "std/media/v1/Image".into());
        let patched = patch_yaml(original, &b.graph()).expect("patches");
        assert!(
            patched.contains("tick: dora/timer/millis/500")
                || patched.contains("source: dora/timer/millis/500"),
            "timer source must survive: {patched}"
        );
        assert!(!patched.contains("unknown"), "no unknown sources allowed");
    }

    #[test]
    fn patch_ignores_nested_list_items_in_node_sections() {
        let original = r#"nodes:
  - id: a
    path: a.py
    custom_list:
      - id: nested
        x: 1
    outputs:
      - out
"#;
        // A nested `- id:` inside a node section must not be mistaken for a
        // node block boundary: no phantom "nested" block may be registered.
        let lines: Vec<String> = original.lines().map(str::to_string).collect();
        let blocks = find_node_blocks(&lines);
        assert_eq!(
            blocks.len(),
            1,
            "nested - id: must not open a phantom node block"
        );
        assert_eq!(
            blocks.get("a"),
            Some(&(1, 7)),
            "node a covers its whole block"
        );

        let mut b = DataflowBuilder::from_yaml(original).expect("parses");
        b.nodes
            .get_mut("a")
            .unwrap()
            .output_types
            .insert("out".into(), "std/core/v1/String".into());
        let patched = patch_yaml(original, &b.graph()).expect("patches");
        assert!(patched.contains("output_types:\n      out: std/core/v1/String"));
    }

    #[test]
    fn patch_handles_top_level_comments_between_nodes() {
        let original = r#"nodes:
  - id: a
    path: a.py
    outputs:
      - out
# comment between blocks
  - id: b
    path: b.py
    outputs:
      - out
"#;
        // from_yaml stops parsing at any top-level non-node line (the
        // comment), so node "b" would never be parsed from this fixture —
        // build the graph directly instead.
        let mut b = DataflowBuilder::new();
        for (id, path) in [("a", "a.py"), ("b", "b.py")] {
            let mut outputs = BTreeMap::new();
            outputs.insert(
                "out".to_string(),
                PortSpec {
                    port_type: None,
                    description: None,
                    source: None,
                },
            );
            b.add_node(NodeSpec {
                id: id.into(),
                operator_id: id.into(),
                runtime: Runtime::Python,
                path: Some(path.into()),
                inputs: BTreeMap::new(),
                outputs,
                input_types: BTreeMap::new(),
                output_types: BTreeMap::new(),
                position: None,
            })
            .unwrap();
        }
        b.nodes.get_mut("b").unwrap().path = Some("b2.py".into());
        let patched = patch_yaml(original, &b.graph()).expect("patches");
        assert!(
            patched.contains("# comment between blocks"),
            "comment preserved"
        );
        assert!(patched.contains("path: b2.py"), "node b edit applied");
        assert!(patched.contains("path: a.py"));
        assert_eq!(
            patched.matches("- id: b").count(),
            1,
            "node b must appear exactly once (no stale duplicate block)"
        );
        assert!(
            !patched.contains("path: b.py"),
            "stale original b block must be gone"
        );
    }
}
