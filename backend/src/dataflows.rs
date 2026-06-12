use crate::models::{
    DataflowDefinition, DataflowDefinitionNode, DataflowGraph, DataflowSummary, Diagnostic,
    GraphEdge, GraphNode, NodeMetrics,
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

struct ParsedDataflow {
    nodes: Vec<ParsedNode>,
}

struct ParsedNode {
    id: String,
    path: Option<String>,
    inputs: BTreeMap<String, String>,
    outputs: Vec<String>,
}

struct DataflowFile {
    id: String,
    name: String,
    path: PathBuf,
    relative_path: String,
}

enum NodeSection {
    Inputs,
    Outputs,
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

    Ok(DataflowDefinition {
        id: file.id,
        name: file.name,
        relative_path: file.relative_path,
        source,
        node_count,
        edge_count,
        nodes,
    })
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
    let diagnostics = vec![Diagnostic {
        severity: "info".to_string(),
        message: format!(
            "Loaded {} from {}.",
            definition.name, definition.relative_path
        ),
    }];

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

fn read_parsed_dataflow(path: &Path) -> Result<ParsedDataflow, DataflowError> {
    let source = fs::read_to_string(path).map_err(|error| {
        DataflowError::Io(format!("Failed to read {}: {error}", path.display()))
    })?;
    parse_dataflow(&source, &path.display().to_string())
}

fn parse_dataflow(source: &str, label: &str) -> Result<ParsedDataflow, DataflowError> {
    let mut nodes = Vec::new();
    let mut current_node: Option<ParsedNode> = None;
    let mut current_section: Option<NodeSection> = None;
    let mut in_nodes = false;

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
            break;
        }

        if let Some(id) = trimmed.strip_prefix("- id:") {
            push_node(&mut nodes, current_node.take(), label)?;
            current_node = Some(ParsedNode {
                id: clean_scalar(id),
                path: None,
                inputs: BTreeMap::new(),
                outputs: Vec::new(),
            });
            current_section = None;
            continue;
        }

        let Some(node) = current_node.as_mut() else {
            continue;
        };

        if let Some(id) = trimmed.strip_prefix("id:") {
            node.id = clean_scalar(id);
            current_section = None;
        } else if let Some(path) = trimmed.strip_prefix("path:") {
            node.path = Some(clean_scalar(path));
            current_section = None;
        } else if trimmed == "inputs:" {
            current_section = Some(NodeSection::Inputs);
        } else if trimmed == "outputs:" {
            current_section = Some(NodeSection::Outputs);
        } else if let Some(section) = current_section.as_ref() {
            match section {
                NodeSection::Inputs => parse_input(trimmed, &mut node.inputs),
                NodeSection::Outputs => parse_output(trimmed, &mut node.outputs),
            }
        }
    }

    push_node(&mut nodes, current_node, label)?;

    if nodes.is_empty() {
        return Err(DataflowError::Invalid(format!(
            "No nodes were found in {label}."
        )));
    }

    Ok(ParsedDataflow { nodes })
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

fn parse_input(line: &str, inputs: &mut BTreeMap<String, String>) {
    if let Some((name, source)) = line.split_once(':') {
        let source = clean_scalar(source);
        if !source.is_empty() {
            inputs.insert(clean_scalar(name), source);
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
        layout.insert(node.id.clone(), (70 + depth * 260, 90 + *row * 140));
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

fn edge_count(dataflow: &ParsedDataflow) -> u32 {
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

fn find_file(id: &str) -> Result<DataflowFile, DataflowError> {
    discover_files()?
        .into_iter()
        .find(|file| file.id == id)
        .ok_or_else(|| DataflowError::NotFound(format!("Dataflow '{id}' was not found.")))
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

fn collect_yaml_files(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<(), DataflowError> {
    for entry in fs::read_dir(directory).map_err(|error| {
        DataflowError::Io(format!("Failed to read {}: {error}", directory.display()))
    })? {
        let entry = entry.map_err(|error| {
            DataflowError::Io(format!("Failed to read directory entry: {error}"))
        })?;
        let path = entry.path();

        if path.is_dir() {
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

fn dataflow_file(root: &Path, path: PathBuf) -> Result<DataflowFile, DataflowError> {
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

fn workspace_root() -> Result<PathBuf, DataflowError> {
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
    fn reports_missing_dataflow() {
        let error = match load_definition("missing-dataflow") {
            Ok(_) => panic!("missing id should fail"),
            Err(error) => error,
        };

        assert!(matches!(error, DataflowError::NotFound(_)));
    }
}
