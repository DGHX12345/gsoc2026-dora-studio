use crate::models::{CoordinatorDataflow, CoordinatorStatus};
use serde::Deserialize;

#[cfg(test)]
mod tests {
    use super::normalize_dora_version;

    #[test]
    fn normalizes_plain_version_line() {
        assert_eq!(
            normalize_dora_version("dora 1.0.0-rc.4\n"),
            "dora 1.0.0-rc.4"
        );
    }

    #[test]
    fn prefixes_version_without_dora_prefix() {
        assert_eq!(normalize_dora_version("1.0.0-rc.4\n"), "dora 1.0.0-rc.4");
    }

    #[test]
    fn empty_output_falls_back_to_unknown() {
        assert_eq!(normalize_dora_version(""), "unknown");
        assert_eq!(normalize_dora_version("\n"), "unknown");
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DoraListEntry {
    #[serde(default)]
    uuid: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    nodes: u32,
    #[serde(default)]
    cpu: Option<f64>,
    #[serde(default)]
    memory: Option<f64>,
}

/// Normalizes `dora --version` output to a display string like
/// `"dora 1.0.0-rc.4"`; `"unknown"` when the output is empty.
fn normalize_dora_version(raw: &str) -> String {
    let line = raw.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        return "unknown".to_string();
    }
    if line.starts_with("dora ") {
        line.to_string()
    } else {
        format!("dora {line}")
    }
}

static DORA_VERSION_CELL: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();

/// Returns the installed dora CLI version, fetched once and cached.
pub async fn dora_version() -> String {
    DORA_VERSION_CELL
        .get_or_init(|| async {
            match tokio::time::timeout(
                std::time::Duration::from_secs(3),
                tokio::process::Command::new("dora")
                    .arg("--version")
                    .output(),
            )
            .await
            {
                Ok(Ok(out)) if out.status.success() => {
                    normalize_dora_version(&String::from_utf8_lossy(&out.stdout))
                }
                _ => "unknown".to_string(),
            }
        })
        .await
        .clone()
}

pub async fn query_coordinator() -> CoordinatorStatus {
    let version = dora_version().await;
    let output = tokio::process::Command::new("dora")
        .args(["list", "--format", "json"])
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            match serde_json::from_str::<Vec<DoraListEntry>>(&stdout) {
                Ok(entries) => {
                    let running = entries.iter().filter(|e| e.status == "running").count() as u32;
                    let active_nodes: u32 = entries.iter().map(|e| e.nodes).sum();
                    let dataflows = entries
                        .into_iter()
                        .map(|e| CoordinatorDataflow {
                            id: e.uuid,
                            name: e.name,
                            status: e.status,
                            nodes: e.nodes,
                        })
                        .collect();

                    CoordinatorStatus {
                        connected: true,
                        version: version.clone(),
                        running_dataflows: running,
                        active_nodes,
                        dataflows,
                    }
                }
                Err(_) => CoordinatorStatus {
                    connected: true,
                    version,
                    running_dataflows: 0,
                    active_nodes: 0,
                    dataflows: Vec::new(),
                },
            }
        }
        Ok(_) => CoordinatorStatus {
            connected: false,
            version: String::new(),
            running_dataflows: 0,
            active_nodes: 0,
            dataflows: Vec::new(),
        },
        Err(_) => CoordinatorStatus {
            connected: false,
            version: String::new(),
            running_dataflows: 0,
            active_nodes: 0,
            dataflows: Vec::new(),
        },
    }
}
