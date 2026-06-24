use crate::models::{CoordinatorDataflow, CoordinatorStatus};
use serde::Deserialize;

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

pub async fn query_coordinator() -> CoordinatorStatus {
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
                        version: "dora 0.5".to_string(),
                        running_dataflows: running,
                        active_nodes,
                        dataflows,
                    }
                }
                Err(_) => CoordinatorStatus {
                    connected: true,
                    version: "dora 0.5".to_string(),
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
