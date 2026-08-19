//! Multi-project dataflow discovery (M18): scans user project directories
//! plus the built-in Studio examples, aggregates a cross-project node
//! palette, and merges dataflow listings for the explorer UI.

use crate::dataflows::{self, DataflowError, DataflowFile};
use crate::dora_env;
use crate::models::DataflowSummary;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub name: String,
    pub path: String,
    pub builtin: bool,
    pub dataflow_count: u32,
    pub dataflows: Vec<DataflowSummary>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortDef {
    pub name: String,
    pub urn: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaletteEntry {
    pub id: String,
    pub operator: String,
    pub path: Option<String>,
    pub runtime: String,
    pub project: String,
    pub manual: bool,
    pub inputs: Vec<PortDef>,
    pub outputs: Vec<PortDef>,
}

fn builtin_examples_root() -> Option<PathBuf> {
    dataflows::workspace_root()
        .ok()
        .map(|root| root.join("examples"))
}

fn runtime_for_path(path: Option<&str>) -> String {
    match path
        .and_then(|value| Path::new(value).extension())
        .and_then(|value| value.to_str())
    {
        Some("py") => "python".to_string(),
        Some("rs") => "rust".to_string(),
        Some("cpp") | Some("cc") | Some("cxx") => "c++".to_string(),
        Some("c") => "c".to_string(),
        _ => "python".to_string(),
    }
}

pub(crate) fn scan_project_dir(
    dir: &Path,
    project: &str,
) -> Result<Vec<DataflowFile>, DataflowError> {
    let _ = project;
    dataflows::scan_dataflows_in(dir, true)
}

pub(crate) fn find_dataflow_file(id: &str) -> Result<DataflowFile, DataflowError> {
    for dir in dora_env::project_dirs() {
        let root = PathBuf::from(&dir);
        if let Ok(files) = dataflows::scan_dataflows_in(&root, true) {
            if let Some(file) = files.into_iter().find(|file| file.id == id) {
                return Ok(file);
            }
        }
    }
    Err(DataflowError::NotFound(format!(
        "Dataflow '{id}' was not found."
    )))
}

/// All dataflows across builtin examples and project dirs, deduplicated by
/// canonical path (builtin wins on overlap).
pub fn list_all_dataflows() -> Result<Vec<DataflowSummary>, DataflowError> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    if let Some(root) = builtin_examples_root() {
        for file in dataflows::scan_dataflows_in(&root, false)? {
            let canonical = std::fs::canonicalize(&file.path).unwrap_or_else(|_| file.path.clone());
            if seen.insert(canonical) {
                out.push(summary_for(&file, "Studio Examples")?);
            }
        }
    }
    for dir in dora_env::project_dirs() {
        let root = PathBuf::from(&dir);
        if !root.is_dir() {
            continue;
        }
        let name = root
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| dir.clone());
        for file in dataflows::scan_dataflows_in(&root, true)? {
            let canonical = std::fs::canonicalize(&file.path).unwrap_or_else(|_| file.path.clone());
            if seen.insert(canonical) {
                out.push(summary_for(&file, &name)?);
            }
        }
    }
    Ok(out)
}

fn summary_for(file: &DataflowFile, project: &str) -> Result<DataflowSummary, DataflowError> {
    let parsed = dataflows::read_parsed_dataflow(&file.path);
    let (status, node_count, edge_count) = match parsed {
        Ok(dataflow) => (
            "stopped".to_string(),
            dataflow.nodes.len() as u32,
            dataflows::edge_count(&dataflow),
        ),
        Err(_) => ("invalid".to_string(), 0, 0),
    };
    Ok(DataflowSummary {
        id: file.id.clone(),
        name: file.name.clone(),
        project: project.to_string(),
        status,
        node_count,
        edge_count,
    })
}

pub fn list_projects() -> Result<Vec<ProjectSummary>, DataflowError> {
    let mut projects = Vec::new();
    if let Some(root) = builtin_examples_root() {
        let files = dataflows::scan_dataflows_in(&root, false)?;
        let dataflows = files
            .iter()
            .map(|file| summary_for(file, "Studio Examples"))
            .collect::<Result<Vec<_>, _>>()?;
        projects.push(ProjectSummary {
            name: "Studio Examples".to_string(),
            path: root.to_string_lossy().to_string(),
            builtin: true,
            dataflow_count: dataflows.len() as u32,
            dataflows,
        });
    }
    for dir in dora_env::project_dirs() {
        let root = PathBuf::from(&dir);
        let name = root
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| dir.clone());
        let files = if root.is_dir() {
            dataflows::scan_dataflows_in(&root, true).unwrap_or_default()
        } else {
            Vec::new()
        };
        let dataflows = files
            .iter()
            .map(|file| summary_for(file, &name))
            .collect::<Result<Vec<_>, _>>()?;
        projects.push(ProjectSummary {
            name,
            path: dir,
            builtin: false,
            dataflow_count: dataflows.len() as u32,
            dataflows,
        });
    }
    Ok(projects)
}

pub(crate) fn palette_for_dirs(dirs: &[PathBuf]) -> Vec<PaletteEntry> {
    let mut by_path: BTreeMap<String, PaletteEntry> = BTreeMap::new();
    for dir in dirs {
        let Ok(files) = dataflows::scan_dataflows_in(dir, true) else {
            continue;
        };
        for file in files {
            let Ok(source) = std::fs::read_to_string(&file.path) else {
                continue;
            };
            let Ok(parsed) = dataflows::parse_dataflow(&source, &file.relative_path) else {
                continue;
            };
            for node in parsed.nodes {
                let key = node.path.clone().unwrap_or_else(|| node.id.clone());
                by_path
                    .entry(key.clone())
                    .and_modify(|entry| {
                        // merge: fill in missing URNs from the incoming node; existing
                        // per-port URNs are never overwritten (first scan wins on conflict).
                        if !node.input_types.is_empty() {
                            for port in &mut entry.inputs {
                                if port.urn.is_none() {
                                    if let Some(urn) = node.input_types.get(&port.name) {
                                        port.urn = Some(urn.clone());
                                    }
                                }
                            }
                        }
                        if !node.output_types.is_empty() {
                            for port in &mut entry.outputs {
                                if port.urn.is_none() {
                                    if let Some(urn) = node.output_types.get(&port.name) {
                                        port.urn = Some(urn.clone());
                                    }
                                }
                            }
                        }
                    })
                    .or_insert_with(|| PaletteEntry {
                        id: node.id.clone(),
                        operator: key,
                        path: node.path.clone(),
                        runtime: runtime_for_path(node.path.as_deref()),
                        project: dir
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        manual: false,
                        inputs: node
                            .inputs
                            .keys()
                            .map(|name| PortDef {
                                name: name.clone(),
                                urn: node.input_types.get(name).cloned(),
                            })
                            .collect(),
                        outputs: node
                            .outputs
                            .iter()
                            .map(|name| PortDef {
                                name: name.clone(),
                                urn: node.output_types.get(name).cloned(),
                            })
                            .collect(),
                    });
            }
        }
    }
    by_path.into_values().collect()
}

pub fn palette() -> Vec<PaletteEntry> {
    let mut dirs = Vec::new();
    if let Some(root) = builtin_examples_root() {
        dirs.push(root);
    }
    dirs.extend(dora_env::project_dirs().into_iter().map(PathBuf::from));
    let mut entries = palette_for_dirs(&dirs);
    for node in dora_env::manual_nodes() {
        entries.push(palette_entry_from_manual(&node));
    }
    entries.sort_by(|a, b| a.operator.cmp(&b.operator));
    entries
}

pub(crate) fn palette_entry_from_manual(node: &dora_env::ManualNode) -> PaletteEntry {
    PaletteEntry {
        id: node.id.clone(),
        operator: node.id.clone(),
        path: Some(node.path.clone()),
        runtime: runtime_for_path(Some(&node.path)),
        project: "manual".to_string(),
        manual: true,
        inputs: node
            .inputs
            .iter()
            .map(|port| PortDef {
                name: port.name.clone(),
                urn: Some(port.urn.clone()),
            })
            .collect(),
        outputs: node
            .outputs
            .iter()
            .map(|port| PortDef {
                name: port.name.clone(),
                urn: Some(port.urn.clone()),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_project(name: &str, yaml: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "dora-studio-proj-{}-{}",
            name,
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("dataflow.yml"), yaml).unwrap();
        dir
    }

    #[test]
    fn palette_dedupes_nodes_across_projects() {
        let a = tmp_project(
            "a",
            "nodes:\n  - id: cam\n    path: cam.py\n    outputs:\n      - image\n    output_types:\n      image: std/media/v1/Image\n",
        );
        let b = tmp_project(
            "b",
            "nodes:\n  - id: cam\n    path: cam.py\n    outputs:\n      - image\n",
        );
        let entries = palette_for_dirs(&[a.clone(), b.clone()]);
        assert_eq!(entries.len(), 1, "same path deduped");
        let cam = &entries[0];
        assert_eq!(cam.outputs[0].name, "image");
        assert_eq!(cam.outputs[0].urn.as_deref(), Some("std/media/v1/Image"));
        fs::remove_dir_all(&a).ok();
        fs::remove_dir_all(&b).ok();
    }

    #[test]
    fn palette_merge_fills_missing_urns_from_later_projects() {
        // untyped project first, typed project second -> URNs filled in
        let a = tmp_project(
            "a-untyped",
            "nodes:\n  - id: cam\n    path: cam.py\n    outputs:\n      - image\n",
        );
        let b = tmp_project("b-typed", "nodes:\n  - id: cam\n    path: cam.py\n    outputs:\n      - image\n    output_types:\n      image: std/media/v1/Image\n");
        let entries = palette_for_dirs(&[a.clone(), b.clone()]);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].outputs[0].urn.as_deref(),
            Some("std/media/v1/Image")
        );
        fs::remove_dir_all(&a).ok();
        fs::remove_dir_all(&b).ok();
    }

    #[test]
    fn palette_merge_fills_partial_urn_coverage() {
        // first project types only port x; later project types x and y -> y filled
        let a = tmp_project(
            "a-partial",
            "nodes:\n  - id: node\n    path: node.py\n    outputs:\n      - x\n      - y\n    output_types:\n      x: std/core/v1/UInt8\n",
        );
        let b = tmp_project(
            "b-full",
            "nodes:\n  - id: node\n    path: node.py\n    outputs:\n      - x\n      - y\n    output_types:\n      x: std/core/v1/UInt8\n      y: std/core/v1/String\n",
        );
        let entries = palette_for_dirs(&[a.clone(), b.clone()]);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].outputs[0].urn.as_deref(),
            Some("std/core/v1/UInt8")
        );
        assert_eq!(
            entries[0].outputs[1].urn.as_deref(),
            Some("std/core/v1/String")
        );
        fs::remove_dir_all(&a).ok();
        fs::remove_dir_all(&b).ok();
    }

    #[test]
    fn palette_merge_first_scan_wins_on_conflicting_urns() {
        // project a types x as UInt8; project b types x as Image.
        // The first scan's URN must survive.
        let a = tmp_project(
            "a-first",
            "nodes:\n  - id: node\n    path: node.py\n    outputs:\n      - x\n    output_types:\n      x: std/core/v1/UInt8\n",
        );
        let b = tmp_project(
            "b-second",
            "nodes:\n  - id: node\n    path: node.py\n    outputs:\n      - x\n    output_types:\n      x: std/media/v1/Image\n",
        );
        let entries = palette_for_dirs(&[a.clone(), b.clone()]);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].outputs[0].urn.as_deref(),
            Some("std/core/v1/UInt8")
        );
        fs::remove_dir_all(&a).ok();
        fs::remove_dir_all(&b).ok();
    }

    #[test]
    fn scan_projects_reports_dataflow_ids_with_hash() {
        let a = tmp_project(
            "a",
            "nodes:\n  - id: n\n    path: n.py\n    outputs:\n      - out\n",
        );
        let files = scan_project_dir(&a, "proj-a").unwrap();
        assert_eq!(files.len(), 1);
        let canonical = fs::canonicalize(a.join("dataflow.yml")).unwrap();
        assert_eq!(
            files[0].id,
            crate::dataflows::hashed_dataflow_id(&canonical.to_string_lossy())
        );
        fs::remove_dir_all(&a).ok();
    }

    #[test]
    fn manual_nodes_flagged_in_palette() {
        let node = crate::dora_env::ManualNode {
            id: "conv".into(),
            path: "/tmp/conv.py".into(),
            description: "convert".into(),
            inputs: vec![crate::dora_env::ManualPort {
                name: "in".into(),
                urn: "std/media/v1/Image".into(),
            }],
            outputs: vec![],
        };
        let entry = palette_entry_from_manual(&node);
        assert!(entry.manual);
        assert_eq!(entry.operator, "conv");
    }
}
