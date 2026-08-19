use crate::{
    dora_env,
    models::{CoordinatorDataflow, CoordinatorStatus},
};
use serde::Deserialize;

#[cfg(test)]
mod tests {
    use super::{parse_list_output, running_dataflow_count, DoraListEntry};

    /// Real `dora 1.0` `dora list --format json` line (captured 2026-08-17).
    const DORA10_RUNNING_LINE: &str = r#"{"uuid":"01a00e7c-0349-79e6-9253-d0bff3dfb24b","name":"m155-smoke","status":"Running","nodes":5,"cpu":2.9987505674362183,"memory":33.906664}"#;

    /// dora 1.0 serializes status as PascalCase ("Running"/"Failed");
    /// 0.5 used lowercase. The count must be case-insensitive.
    #[test]
    fn counts_dora10_pascal_case_status_as_running() {
        let entries: Vec<DoraListEntry> = serde_json::from_str(&format!(
            "[{DORA10_RUNNING_LINE}, {{\"uuid\":\"x\",\"name\":\"f\",\"status\":\"Failed\",\"nodes\":0,\"cpu\":0.0,\"memory\":0.0}}]"
        ))
        .unwrap();
        assert_eq!(running_dataflow_count(&entries), 1);
    }

    #[test]
    fn parses_real_dora10_list_line() {
        let entries: Vec<DoraListEntry> =
            serde_json::from_str(&format!("[{DORA10_RUNNING_LINE}]")).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].uuid, "01a00e7c-0349-79e6-9253-d0bff3dfb24b");
        assert_eq!(entries[0].nodes, 5);
        assert!(entries[0].cpu.unwrap() > 2.9);
        assert!(entries[0].memory.unwrap() > 33.0);
    }

    #[test]
    fn normalizes_plain_version_line() {
        assert_eq!(
            crate::dora_env::normalize_dora_version("dora 1.0.0-rc.4\n"),
            "dora 1.0.0-rc.4"
        );
    }

    #[test]
    fn prefixes_version_without_dora_prefix() {
        assert_eq!(
            crate::dora_env::normalize_dora_version("1.0.0-rc.4\n"),
            "dora 1.0.0-rc.4"
        );
    }

    #[test]
    fn normalizes_dora_cli_output() {
        // 1.0's `dora --version` prints "dora-cli 1.0.0-rc.4".
        assert_eq!(
            crate::dora_env::normalize_dora_version("dora-cli 1.0.0-rc.4\n"),
            "dora 1.0.0-rc.4"
        );
    }

    #[test]
    fn empty_output_falls_back_to_unknown() {
        assert_eq!(crate::dora_env::normalize_dora_version(""), "unknown");
        assert_eq!(crate::dora_env::normalize_dora_version("\n"), "unknown");
    }

    /// dora 1.0 emits JSON Lines: N dataflows = N separate JSON
    /// object lines, never a single array.
    #[test]
    fn parses_json_lines_list_output() {
        let lines = concat!(
            "{\"uuid\":\"a\",\"name\":\"one\",\"status\":\"Running\",\"nodes\":1}\n",
            "{\"uuid\":\"b\",\"name\":\"two\",\"status\":\"Finished\",\"nodes\":0}\n",
        );
        let entries = parse_list_output(lines).expect("JSON Lines parse");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "one");
        assert_eq!(entries[1].name, "two");

        let array = r#"[{"status":"Running"},{"status":"Finished"}]"#;
        assert_eq!(parse_list_output(array).unwrap().len(), 2);

        let single = r#"{"uuid":"df-1","name":"flow","status":"Running","nodes":2}"#;
        assert_eq!(parse_list_output(single).unwrap().len(), 1);

        assert!(parse_list_output("not-json").is_none());
        assert_eq!(parse_list_output("").unwrap().len(), 0);
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

/// Counts running dataflows; dora 0.5 emits lowercase status while
/// dora 1.0 emits PascalCase, so the match is case-insensitive.
fn running_dataflow_count(entries: &[DoraListEntry]) -> u32 {
    entries
        .iter()
        .filter(|e| e.status.eq_ignore_ascii_case("running"))
        .count() as u32
}

/// dora 1.0 emits `dora list --format json` as JSON Lines: one JSON
/// object per line. Accept JSON Lines, a JSON array, and a single
/// object.
fn parse_list_output(stdout: &str) -> Option<Vec<DoraListEntry>> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Some(Vec::new());
    }
    let lines: Result<Vec<DoraListEntry>, _> = trimmed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(serde_json::from_str)
        .collect();
    if let Ok(entries) = lines {
        return Some(entries);
    }
    if let Ok(entries) = serde_json::from_str::<Vec<DoraListEntry>>(trimmed) {
        return Some(entries);
    }
    serde_json::from_str::<DoraListEntry>(trimmed)
        .ok()
        .map(|entry| vec![entry])
}

/// Returns the installed dora CLI version, fetched once and cached.
pub async fn dora_version() -> String {
    dora_env::dora_version().await
}

pub async fn query_coordinator() -> CoordinatorStatus {
    let version = dora_version().await;
    let output = tokio::process::Command::new(dora_env::resolve_dora_bin())
        .args(["list", "--format", "json"])
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            match parse_list_output(&stdout) {
                Some(entries) => {
                    let running = running_dataflow_count(&entries);
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
                None => CoordinatorStatus {
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
