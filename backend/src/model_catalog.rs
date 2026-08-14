//! Robot model catalog (M13 D6) — discovers locally available robot
//! models under `models/` for the frontend model selector. A model is a
//! subdirectory containing at least one `.urdf` file (the frontend's
//! tool-owned loader only handles URDF today).

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelEntry {
    pub id: String,
    pub urdf_path: String,
    pub mesh_base_path: String,
}

/// Scan `models_dir` for subdirectories containing `.urdf` files. The
/// first URDF found per directory wins; directories without URDFs
/// (e.g. MuJoCo-only) are skipped. Sorted by id for stable output.
pub fn list_available_models(models_dir: &Path) -> Vec<ModelEntry> {
    let mut entries = Vec::new();
    let entries_read = match fs::read_dir(models_dir) {
        Ok(entries) => entries,
        Err(_) => return entries, // missing models dir: honest empty list
    };
    for dir_entry in entries_read.flatten() {
        let path = dir_entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let urdf = find_urdf(&path);
        if let Some(urdf) = urdf {
            let urdf_file = urdf
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("model.urdf");
            entries.push(ModelEntry {
                id: name.to_string(),
                urdf_path: format!("/models/{name}/{urdf_file}"),
                mesh_base_path: format!("/models/{name}/"),
            });
        }
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    entries
}

fn find_urdf(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("urdf") {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_models_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dora-studio-model-catalog-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).ok();
        dir
    }

    #[test]
    fn lists_directories_with_urdf_files_only() {
        let dir = temp_models_dir();
        fs::create_dir_all(dir.join("b601")).unwrap();
        fs::write(dir.join("b601/reBot.urdf"), b"<robot name='r'/>").unwrap();
        fs::create_dir_all(dir.join("nano_models")).unwrap();
        fs::write(dir.join("nano_models/nano_full.xml"), b"<mujoco/>").unwrap();
        fs::create_dir_all(dir.join("empty_dir")).unwrap();
        fs::write(dir.join("README.md"), b"not a model").unwrap();

        let models = list_available_models(&dir);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "b601");
        assert_eq!(models[0].urdf_path, "/models/b601/reBot.urdf");
        assert_eq!(models[0].mesh_base_path, "/models/b601/");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn returns_empty_for_missing_or_empty_dir() {
        assert!(list_available_models(Path::new("/nonexistent/path")).is_empty());
        let dir = temp_models_dir();
        assert!(list_available_models(&dir).is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sorts_entries_by_id() {
        let dir = temp_models_dir();
        for name in ["zeta", "alpha", "mid"] {
            fs::create_dir_all(dir.join(name)).unwrap();
            fs::write(dir.join(name).join("m.urdf"), b"<robot/>").unwrap();
        }
        let models = list_available_models(&dir);
        let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "mid", "zeta"]);
        fs::remove_dir_all(&dir).ok();
    }
}
