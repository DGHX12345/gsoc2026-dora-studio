use crate::models::{
    DataflowDefinition, DataflowDefinitionNode, DataflowGraph, DataflowSummary, Diagnostic,
    GraphEdge, GraphNode, NodeMetrics, TypeRuleDef,
};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum DataflowError {
    Io(String),
    Invalid(String),
    NotFound(String),
}

pub(crate) struct ParsedDataflow {
    pub(crate) nodes: Vec<ParsedNode>,
    pub(crate) type_rules: Vec<(String, String)>,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

pub(crate) struct ParsedNode {
    pub(crate) id: String,
    pub(crate) path: Option<String>,
    pub(crate) inputs: BTreeMap<String, String>,
    pub(crate) outputs: Vec<String>,
    pub(crate) input_types: BTreeMap<String, String>,
    pub(crate) output_types: BTreeMap<String, String>,
}

pub struct DataflowFile {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub relative_path: String,
}

enum NodeSection {
    Inputs,
    Outputs,
    InputTypes,
    OutputTypes,
}

pub fn list_dataflows() -> Result<Vec<DataflowSummary>, DataflowError> {
    discover_files()?
        .into_iter()
        .map(|file| {
            let parsed = read_parsed_dataflow(&file.path);
            let (status, node_count, edge_count) = match parsed {
                Ok(dataflow) => (
                    "stopped".to_string(),
                    dataflow.nodes.len() as u32,
                    edge_count(&dataflow),
                ),
                Err(_) => ("invalid".to_string(), 0, 0),
            };

            Ok(DataflowSummary {
                id: file.id,
                name: file.name,
                project: "Studio Examples".to_string(),
                status,
                node_count,
                edge_count,
            })
        })
        .collect()
}

pub fn load_definition(id: &str) -> Result<DataflowDefinition, DataflowError> {
    let file = find_file(id)?;
    let source = fs::read_to_string(&file.path).map_err(|error| {
        DataflowError::Io(format!("Failed to read {}: {error}", file.relative_path))
    })?;
    let parsed = parse_dataflow(&source, &file.relative_path)?;
    let node_count = parsed.nodes.len() as u32;
    let edge_count = edge_count(&parsed);
    let nodes = parsed.nodes.into_iter().map(definition_node).collect();
    let type_rules = parsed
        .type_rules
        .into_iter()
        .map(|(from, to)| TypeRuleDef { from, to })
        .collect();

    Ok(DataflowDefinition {
        id: file.id,
        name: file.name,
        relative_path: file.relative_path.clone(),
        source,
        node_count,
        edge_count,
        project: project_name(&file.relative_path),
        type_rules,
        nodes,
    })
}

/// Minimal project attribution for a definition: builtin examples live under
/// "examples/" in the workspace, everything else derives from the first path
/// segment of the relative path (project-dir scans are rooted at the project).
fn project_name(relative_path: &str) -> String {
    if relative_path.starts_with("examples/") {
        return "Studio Examples".to_string();
    }
    relative_path
        .split('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "Studio Examples".to_string())
}

pub fn graph(id: &str) -> Result<DataflowGraph, DataflowError> {
    let definition = load_definition(id)?;
    let layout = graph_layout(&definition.nodes);
    let nodes = definition
        .nodes
        .iter()
        .map(|node| graph_node(node, &layout))
        .collect::<Vec<_>>();
    let edges = definition
        .nodes
        .iter()
        .flat_map(|node| graph_edges_for_node(node, &definition.nodes))
        .enumerate()
        .map(|(index, mut edge)| {
            edge.id = format!("e{}", index + 1);
            edge
        })
        .collect::<Vec<_>>();
    let parsed = read_parsed_dataflow(&find_file(id)?.path)?;
    let mut diagnostics = parsed.diagnostics;
    diagnostics.insert(
        0,
        Diagnostic {
            severity: "info".to_string(),
            message: format!(
                "Loaded {} from {}.",
                definition.name, definition.relative_path
            ),
        },
    );

    Ok(DataflowGraph {
        nodes,
        edges,
        diagnostics,
    })
}

pub fn nodes(id: &str) -> Result<Vec<NodeMetrics>, DataflowError> {
    Ok(load_definition(id)?
        .nodes
        .into_iter()
        .map(|node| NodeMetrics {
            id: node.id.clone(),
            label: node.id,
            kind: kind_for_path(node.path.as_deref()),
            status: "stopped".to_string(),
            cpu: 0,
            memory: 0,
            restarts: 0,
            pending: 0,
        })
        .collect())
}

pub(crate) fn read_parsed_dataflow(path: &Path) -> Result<ParsedDataflow, DataflowError> {
    let source = fs::read_to_string(path).map_err(|error| {
        DataflowError::Io(format!("Failed to read {}: {error}", path.display()))
    })?;
    parse_dataflow(&source, &path.display().to_string())
}

pub(crate) fn parse_dataflow(source: &str, label: &str) -> Result<ParsedDataflow, DataflowError> {
    let mut nodes = Vec::new();
    let mut current_node: Option<ParsedNode> = None;
    let mut current_section: Option<NodeSection> = None;
    let mut pending_input: Option<String> = None;
    let mut in_nodes = false;

    let mut diagnostics = Vec::new();
    let type_rules = parse_type_rules(source, label, &mut diagnostics);

    for raw_line in source.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let indent = raw_line.chars().take_while(|ch| ch.is_whitespace()).count();
        if !in_nodes {
            in_nodes = trimmed == "nodes:";
            continue;
        }

        if indent == 0 && trimmed != "nodes:" {
            if let Some(section) = trimmed.strip_suffix(':') {
                diagnostics.push(Diagnostic {
                    severity: "warn".to_string(),
                    message: format!(
                        "Unsupported top-level section '{section}' in {label} was ignored."
                    ),
                });
            }
            break;
        }

        if let Some(id) = trimmed.strip_prefix("- id:") {
            push_node(&mut nodes, current_node.take(), label)?;
            current_node = Some(ParsedNode {
                id: clean_scalar(id),
                path: None,
                inputs: BTreeMap::new(),
                outputs: Vec::new(),
                input_types: BTreeMap::new(),
                output_types: BTreeMap::new(),
            });
            current_section = None;
            pending_input = None;
            continue;
        }

        let Some(node) = current_node.as_mut() else {
            continue;
        };

        if let Some(id) = trimmed.strip_prefix("id:") {
            node.id = clean_scalar(id);
            current_section = None;
            pending_input = None;
        } else if let Some(path) = trimmed.strip_prefix("path:") {
            node.path = Some(clean_scalar(path));
            current_section = None;
            pending_input = None;
        } else if trimmed == "inputs:" {
            current_section = Some(NodeSection::Inputs);
            pending_input = None;
        } else if trimmed == "outputs:" {
            current_section = Some(NodeSection::Outputs);
            pending_input = None;
        } else if trimmed == "input_types:" {
            current_section = Some(NodeSection::InputTypes);
            pending_input = None;
        } else if trimmed == "output_types:" {
            current_section = Some(NodeSection::OutputTypes);
            pending_input = None;
        } else if let Some(section) = current_section.as_ref() {
            match section {
                NodeSection::Inputs => parse_input(trimmed, &mut node.inputs, &mut pending_input),
                NodeSection::Outputs => parse_output(trimmed, &mut node.outputs),
                NodeSection::InputTypes => parse_typed_port(trimmed, &mut node.input_types),
                NodeSection::OutputTypes => parse_typed_port(trimmed, &mut node.output_types),
            }
        }
    }

    push_node(&mut nodes, current_node, label)?;

    if nodes.is_empty() {
        return Err(DataflowError::Invalid(format!(
            "No nodes were found in {label}."
        )));
    }

    Ok(ParsedDataflow {
        nodes,
        type_rules,
        diagnostics,
    })
}

fn push_node(
    nodes: &mut Vec<ParsedNode>,
    node: Option<ParsedNode>,
    label: &str,
) -> Result<(), DataflowError> {
    let Some(node) = node else {
        return Ok(());
    };

    if node.id.is_empty() {
        return Err(DataflowError::Invalid(format!(
            "A node in {label} is missing an id."
        )));
    }

    nodes.push(node);
    Ok(())
}

/// Parse one line inside a node's `inputs:` section. dora 1.0 accepts both
/// the compact form (`name: source/port`) and the nested form written back by
/// Studio (`name:` followed by an indented `source: source/port` line). For
/// the nested form a pending port name is remembered on the empty `name:`
/// line and resolved by the following `source:` sub-key.
fn parse_input(line: &str, inputs: &mut BTreeMap<String, String>, pending: &mut Option<String>) {
    if let Some(port) = pending.as_deref() {
        if let Some(value) = line.strip_prefix("source:") {
            let value = clean_scalar(value);
            if !value.is_empty() {
                inputs.insert(port.to_string(), value);
            }
            *pending = None;
            return;
        }
    }
    if let Some((name, source)) = line.split_once(':') {
        let name = clean_scalar(name);
        let source = clean_scalar(source);
        if !source.is_empty() {
            inputs.insert(name, source);
            *pending = None;
        } else if !name.is_empty() {
            *pending = Some(name);
        }
    }
}

fn parse_output(line: &str, outputs: &mut Vec<String>) {
    if let Some(output) = line.strip_prefix("- ") {
        let output = clean_scalar(output);
        if !output.is_empty() {
            outputs.push(output);
        }
    }
}

fn parse_typed_port(line: &str, types: &mut BTreeMap<String, String>) {
    if let Some((name, urn)) = line.split_once(':') {
        let urn = clean_scalar(urn);
        if !urn.is_empty() {
            types.insert(clean_scalar(name), urn);
        }
    }
}

fn parse_type_rules(
    source: &str,
    label: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<(String, String)> {
    let mut rules = Vec::new();
    let mut in_rules = false;
    let mut current_from: Option<String> = None;
    for raw_line in source.lines() {
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
            current_from = Some(clean_scalar(from));
        } else if let Some(to) = trimmed.strip_prefix("to:") {
            if let Some(from) = current_from.take() {
                rules.push((from, clean_scalar(to)));
            }
        } else if indent > 0 && !trimmed.starts_with('-') {
            // tolerate unknown keys inside rules
        }
    }
    if in_rules && rules.is_empty() {
        diagnostics.push(Diagnostic {
            severity: "warn".to_string(),
            message: format!("No type_rules were parsed from {label}."),
        });
    }
    rules
}

fn clean_scalar(value: &str) -> String {
    value
        .trim()
        .trim_matches(|ch| matches!(ch, '\'' | '"'))
        .to_string()
}

fn definition_node(node: ParsedNode) -> DataflowDefinitionNode {
    DataflowDefinitionNode {
        id: node.id,
        path: node.path,
        inputs: node
            .inputs
            .into_iter()
            .map(|(name, source)| format!("{name}: {source}"))
            .collect(),
        outputs: node.outputs,
        input_types: node.input_types,
        output_types: node.output_types,
    }
}

fn graph_node(node: &DataflowDefinitionNode, layout: &HashMap<String, (u32, u32)>) -> GraphNode {
    let (x, y) = layout.get(&node.id).copied().unwrap_or((70, 90));

    GraphNode {
        id: node.id.clone(),
        label: node.id.clone(),
        kind: kind_for_path(node.path.as_deref()),
        status: "stopped".to_string(),
        x,
        y,
        inputs: node.inputs.clone(),
        outputs: node.outputs.clone(),
        cpu: 0,
        memory: 0,
        restarts: 0,
        pending: 0,
        note: format!(
            "Loaded from {}.",
            node.path.as_deref().unwrap_or("inline operator")
        ),
    }
}

fn graph_layout(nodes: &[DataflowDefinitionNode]) -> HashMap<String, (u32, u32)> {
    let mut depths = HashMap::new();
    for node in nodes {
        node_depth(&node.id, nodes, &mut depths, &mut Vec::new());
    }

    let mut rows_by_depth = BTreeMap::<u32, u32>::new();
    let mut layout = HashMap::new();
    for node in nodes {
        let depth = *depths.get(&node.id).unwrap_or(&0);
        let row = rows_by_depth.entry(depth).or_insert(0);
        layout.insert(node.id.clone(), (80 + depth * 400, 80 + *row * 200));
        *row += 1;
    }

    layout
}

fn node_depth(
    id: &str,
    nodes: &[DataflowDefinitionNode],
    depths: &mut HashMap<String, u32>,
    visiting: &mut Vec<String>,
) -> u32 {
    if let Some(depth) = depths.get(id) {
        return *depth;
    }
    if visiting.iter().any(|node| node == id) {
        return 0;
    }

    let Some(node) = nodes.iter().find(|node| node.id == id) else {
        return 0;
    };

    visiting.push(id.to_string());
    let depth = node
        .inputs
        .iter()
        .filter_map(|input| input_source_node(input, nodes))
        .map(|source| node_depth(&source, nodes, depths, visiting) + 1)
        .max()
        .unwrap_or(0);
    visiting.pop();

    depths.insert(id.to_string(), depth);
    depth
}

fn input_source_node(input: &str, nodes: &[DataflowDefinitionNode]) -> Option<String> {
    let (_, source) = input.split_once(": ")?;
    let (from, _) = source.split_once('/')?;
    nodes
        .iter()
        .any(|node| node.id == from)
        .then(|| from.to_string())
}

fn graph_edges_for_node(
    node: &DataflowDefinitionNode,
    nodes: &[DataflowDefinitionNode],
) -> Vec<GraphEdge> {
    node.inputs
        .iter()
        .filter_map(|input| {
            let (label, source) = input.split_once(": ")?;
            let (from, output) = source.split_once('/')?;
            if nodes.iter().any(|candidate| candidate.id == from) {
                Some(GraphEdge {
                    id: String::new(),
                    from: from.to_string(),
                    to: node.id.clone(),
                    label: format!("{label}: {output}"),
                })
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn edge_count(dataflow: &ParsedDataflow) -> u32 {
    dataflow
        .nodes
        .iter()
        .map(|node| {
            node.inputs
                .values()
                .filter(|input| {
                    input
                        .split_once('/')
                        .map(|(from, _)| dataflow.nodes.iter().any(|node| node.id == from))
                        .unwrap_or(false)
                })
                .count() as u32
        })
        .sum()
}

fn kind_for_path(path: Option<&str>) -> String {
    match path
        .and_then(|value| Path::new(value).extension())
        .and_then(|value| value.to_str())
    {
        Some("py") => "Python node".to_string(),
        Some("rs") => "Rust node".to_string(),
        Some("cpp") | Some("cc") | Some("cxx") => "C++ node".to_string(),
        Some(other) => format!("{other} node"),
        None => "Dora node".to_string(),
    }
}

pub fn resolve_dataflow(id: &str) -> Result<DataflowFile, DataflowError> {
    find_file(id)
}

fn find_file(id: &str) -> Result<DataflowFile, DataflowError> {
    if let Some(file) = discover_files()?.into_iter().find(|file| file.id == id) {
        return Ok(file);
    }
    crate::project_scan::find_dataflow_file(id)
}

fn discover_files() -> Result<Vec<DataflowFile>, DataflowError> {
    let root = workspace_root()?;
    let examples = root.join("examples");
    if !examples.is_dir() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    collect_yaml_files(&examples, &mut paths)?;
    paths.sort();

    paths
        .into_iter()
        .map(|path| dataflow_file(&root, path))
        .collect()
}

pub(crate) fn collect_yaml_files(
    directory: &Path,
    paths: &mut Vec<PathBuf>,
) -> Result<(), DataflowError> {
    for entry in fs::read_dir(directory).map_err(|error| {
        DataflowError::Io(format!("Failed to read {}: {error}", directory.display()))
    })? {
        let entry = entry.map_err(|error| {
            DataflowError::Io(format!("Failed to read directory entry: {error}"))
        })?;
        let path = entry.path();

        if path.is_dir() {
            // dora session artifacts (out/dataflow-dora-session.yml) are
            // runtime byproducts, not source dataflows — skip them.
            if path.file_name().and_then(|name| name.to_str()) == Some("out") {
                continue;
            }
            collect_yaml_files(&path, paths)?;
        } else if is_yaml_file(&path) {
            paths.push(path);
        }
    }

    Ok(())
}

fn is_yaml_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("yml") | Some("yaml")
    )
}

pub(crate) fn dataflow_file(root: &Path, path: PathBuf) -> Result<DataflowFile, DataflowError> {
    let relative_path = path
        .strip_prefix(root)
        .map_err(|error| {
            DataflowError::Io(format!("Failed to normalize {}: {error}", path.display()))
        })?
        .to_string_lossy()
        .replace('\\', "/");
    let id = dataflow_id(&relative_path);
    let name = if path.file_name().and_then(|value| value.to_str()) == Some("dataflow.yml") {
        path.parent()
            .and_then(|parent| parent.file_name())
            .and_then(|value| value.to_str())
            .map(|parent| format!("{parent}/dataflow.yml"))
            .unwrap_or_else(|| relative_path.clone())
    } else {
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(&relative_path)
            .to_string()
    };

    Ok(DataflowFile {
        id,
        name,
        path,
        relative_path,
    })
}

pub(crate) fn hashed_dataflow_id(abs_path: &str) -> String {
    use sha1::{Digest, Sha1};
    format!("{:x}", Sha1::digest(abs_path.as_bytes()))[..12].to_string()
}

/// Scan a directory tree for dataflow YAML files, computing DataflowFile
/// entries with ids relative to `root`. With `hash_ids` the id is a sha1
/// of the canonical absolute path (used for user project directories).
pub(crate) fn scan_dataflows_in(
    root: &Path,
    hash_ids: bool,
) -> Result<Vec<DataflowFile>, DataflowError> {
    let mut paths = Vec::new();
    collect_yaml_files(root, &mut paths)?;
    paths.sort();
    paths
        .into_iter()
        .map(|path| dataflow_file_for(root, path, hash_ids))
        .collect()
}

fn dataflow_file_for(
    root: &Path,
    path: PathBuf,
    hash_ids: bool,
) -> Result<DataflowFile, DataflowError> {
    let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    let mut file = dataflow_file(root, path)?;
    if hash_ids {
        file.id = hashed_dataflow_id(&canonical.to_string_lossy());
    }
    Ok(file)
}

fn dataflow_id(relative_path: &str) -> String {
    let path = Path::new(relative_path);
    if path.file_name().and_then(|value| value.to_str()) == Some("dataflow.yml") {
        if let Some(parent) = path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|value| value.to_str())
        {
            return slug(parent);
        }
    }

    slug(
        relative_path
            .trim_end_matches(".yaml")
            .trim_end_matches(".yml"),
    )
}

fn slug(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub(crate) fn workspace_root() -> Result<PathBuf, DataflowError> {
    // The compiled backend owns the repository layout, so prefer the
    // manifest root over the launch-time cwd (which may point at a
    // different checkout and hide dataflows).
    let manifest_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .filter(|root| root.join("examples").is_dir() && root.join("backend").is_dir());
    if let Some(root) = manifest_root {
        return Ok(root);
    }

    let mut current = std::env::current_dir()
        .map_err(|error| DataflowError::Io(format!("Failed to read current directory: {error}")))?;

    loop {
        if current.join("examples").is_dir() && current.join("backend").is_dir() {
            return Ok(current);
        }

        if !current.pop() {
            return std::env::current_dir().map_err(|error| {
                DataflowError::Io(format!("Failed to read current directory: {error}"))
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
nodes:
  - id: camera
    path: camera.py
    inputs:
      tick: dora/timer/millis/500
    outputs:
      - frame
  - id: detector
    path: detector.py
    inputs:
      frame: camera/frame
    outputs:
      - boxes
"#;

    #[test]
    fn parses_node_and_edge_counts() {
        let parsed = parse_dataflow(SAMPLE, "sample").expect("sample parses");

        assert_eq!(parsed.nodes.len(), 2);
        assert_eq!(edge_count(&parsed), 1);
    }

    #[test]
    fn parses_nested_input_form() {
        let parsed = parse_dataflow(
            r#"
nodes:
  - id: camera
    path: camera.py
    inputs:
      tick:
        source: dora/timer/millis/500
    outputs:
      - frame
  - id: sink
    path: sink.py
    inputs:
      frame:
        source: camera/frame
"#,
            "nested.yml",
        )
        .expect("nested inputs parse");
        assert_eq!(parsed.nodes.len(), 2);
        assert_eq!(
            parsed.nodes[0].inputs.get("tick").map(String::as_str),
            Some("dora/timer/millis/500")
        );
        assert_eq!(
            parsed.nodes[1].inputs.get("frame").map(String::as_str),
            Some("camera/frame")
        );
        assert_eq!(edge_count(&parsed), 1);
    }

    #[test]
    fn creates_stable_id_for_example_dataflow() {
        assert_eq!(
            dataflow_id("examples/robot-perception-test/dataflow.yml"),
            "robot-perception-test"
        );
    }

    #[test]
    fn discovers_example_dataflow() {
        let dataflows = list_dataflows().expect("dataflows load");

        assert!(dataflows
            .iter()
            .any(|dataflow| dataflow.id == "robot-perception-test"));
    }

    #[test]
    fn skips_dora_session_artifacts_under_out_directories() {
        // `dora start` writes out/dataflow-dora-session.yml next to the
        // source dataflow; those session artifacts must not appear in the
        // dataflow list (their expanded format fails graph parsing).
        let dir = std::env::temp_dir().join(format!("dora-studio-df-{}", uuid::Uuid::new_v4()));
        let nested = dir.join("nested").join("out");
        fs::create_dir_all(&nested).unwrap();
        fs::write(dir.join("real.yml"), b"nodes: []").unwrap();
        fs::write(nested.join("dataflow-dora-session.yml"), b"nodes: []").unwrap();

        let mut paths = Vec::new();
        collect_yaml_files(&dir, &mut paths).unwrap();

        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("real.yml"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn loads_example_definition() {
        let definition = load_definition("robot-perception-test").expect("definition loads");

        assert_eq!(definition.id, "robot-perception-test");
        assert_eq!(
            definition.relative_path,
            "examples/robot-perception-test/dataflow.yml"
        );
        assert_eq!(definition.node_count, 5);
        assert_eq!(definition.edge_count, 6);
        assert!(definition.source.contains("robot_bridge"));
        assert!(definition.nodes.iter().any(|node| node.id == "camera"));
    }

    #[test]
    fn resolves_example_dataflow_path() {
        let file = resolve_dataflow("robot-perception-test").expect("dataflow resolves");

        assert_eq!(file.id, "robot-perception-test");
        assert_eq!(
            file.relative_path,
            "examples/robot-perception-test/dataflow.yml"
        );
        assert!(file
            .path
            .ends_with("examples/robot-perception-test/dataflow.yml"));
    }

    #[test]
    fn graph_reports_unsupported_top_level_sections() {
        let parsed = parse_dataflow(
            r#"
nodes:
  - id: camera
    path: camera.py
    outputs:
      - frame
_unstable_debug:
  enable_debug_inspection: true
"#,
            "sample.yml",
        )
        .expect("sample parses with warning");

        assert!(parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == "warn"
                && diagnostic.message.contains("_unstable_debug")));
    }

    /// dora 1.0 adds optional top-level fields (health_check_interval,
    /// strict_types, type_rules). They must not break node extraction —
    /// at most a warning diagnostic.
    #[test]
    fn parses_dora10_optional_top_level_fields() {
        let parsed = parse_dataflow(
            r#"
health_check_interval: 2.5
strict_types: true
type_rules:
  - from: a/b
    to: c/d
nodes:
  - id: camera
    path: camera.py
    outputs:
      - frame
"#,
            "dora10.yml",
        )
        .expect("dora 1.0 optional fields parse");

        assert_eq!(parsed.nodes.len(), 1);
        assert_eq!(parsed.nodes[0].id, "camera");
    }

    #[test]
    fn parses_input_output_types_and_type_rules() {
        let parsed = parse_dataflow(
            r#"
type_rules:
  - from: std/core/v1/UInt8
    to: std/core/v1/String
nodes:
  - id: sensor
    path: sensor.py
    outputs:
      - reading
    output_types:
      reading: std/core/v1/Float64
  - id: processor
    path: processor.py
    inputs:
      reading: sensor/reading
    input_types:
      reading: std/core/v1/Float64
"#,
            "typed.yml",
        )
        .expect("typed dataflow parses");
        assert_eq!(
            parsed.type_rules,
            vec![(
                "std/core/v1/UInt8".to_string(),
                "std/core/v1/String".to_string()
            )]
        );
        let sensor = &parsed.nodes[0];
        assert_eq!(
            sensor.output_types.get("reading").map(String::as_str),
            Some("std/core/v1/Float64")
        );
        let processor = &parsed.nodes[1];
        assert_eq!(
            processor.input_types.get("reading").map(String::as_str),
            Some("std/core/v1/Float64")
        );
    }

    #[test]
    fn hashed_id_is_stable_for_absolute_path() {
        use sha1::{Digest, Sha1};
        let abs = "/home/user/projects/demo/dataflow.yml";
        let expected = format!("{:x}", Sha1::digest(abs.as_bytes()));
        assert_eq!(hashed_dataflow_id(abs), expected[..12].to_string());
    }

    #[test]
    fn reports_missing_dataflow() {
        let error = match load_definition("missing-dataflow") {
            Ok(_) => panic!("missing id should fail"),
            Err(error) => error,
        };

        assert!(matches!(error, DataflowError::NotFound(_)));
    }

    #[test]
    fn scan_dataflows_in_hashes_ids_of_canonical_paths() {
        let dir = std::env::temp_dir().join(format!("dora-studio-scan-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("dataflow.yml"),
            "nodes:\n  - id: n\n    outputs:\n      - out\n",
        )
        .unwrap();
        let files = scan_dataflows_in(&dir, true).unwrap();
        assert_eq!(files.len(), 1);
        let canonical = fs::canonicalize(dir.join("dataflow.yml")).unwrap();
        assert_eq!(
            files[0].id,
            hashed_dataflow_id(&canonical.to_string_lossy())
        );
        // non-hash mode keeps slug id derived from relative path
        let files2 = scan_dataflows_in(&dir, false).unwrap();
        assert_eq!(files2[0].id, "dataflow");
        fs::remove_dir_all(&dir).ok();
    }
}
