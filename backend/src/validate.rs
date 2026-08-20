//! dora validate terminal check for dataflow saves (M18).
//!
//! Only runs on dora 1.x (0.5 lacks typed validation semantics). Errors
//! block the save; warnings are mapped back to canvas edges.

use crate::models::SaveIssue;
use std::path::Path;
use std::process::Stdio;

pub struct ValidateOutcome {
    pub errors: Vec<SaveIssue>,
    pub warnings: Vec<SaveIssue>,
}

/// Run `dora validate <path>` with a 10s timeout. Returns Ok(outcome) on
/// dora 1.x, Err(reason) when validation is unavailable (0.x or spawn
/// failure) — callers surface that as a non-blocking notice.
pub async fn validate_yaml(path: &Path) -> Result<ValidateOutcome, String> {
    let version = crate::dora_env::dora_version().await;
    if !crate::dora_env::lifecycle_supported(&version) {
        return Err(format!(
            "{version} does not support typed validation; final check skipped."
        ));
    }
    let bin = crate::dora_env::resolve_dora_bin();
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::process::Command::new(&bin)
            .arg("validate")
            .arg(path)
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "dora validate timed out after 10s".to_string())?
    .map_err(|error| format!("failed to spawn dora validate: {error}"))?;

    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let success = output.status.success();

    // A non-zero exit with no mapped keyword lines must still block the
    // save: synthesize a generic issue from the first non-empty output
    // line so unmappable failures (e.g. YAML syntax errors) never pass
    // through the gate as ok:true.
    let mut errors = if success {
        Vec::new()
    } else {
        map_validate_output(&text, true)
    };
    if !success && errors.is_empty() {
        let message = text
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(|line| line.to_string())
            .unwrap_or_else(|| {
                format!(
                    "dora validate failed with exit code {}",
                    output.status.code().unwrap_or(1)
                )
            });
        errors.push(SaveIssue {
            node_id: None,
            port_id: None,
            message: message.to_string(),
        });
    }

    Ok(ValidateOutcome {
        errors,
        warnings: map_validate_output(&text, false),
    })
}

/// Extract node/port-addressable issues from `dora validate` output.
/// `strict` selects error patterns; otherwise type-mismatch warnings.
pub fn map_validate_output(text: &str, strict: bool) -> Vec<SaveIssue> {
    let mut issues = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let is_warning = trimmed.contains("type mismatch on input");
        if strict == is_warning {
            continue;
        }
        // Real dora 1.0 wiring errors end in "does not exist"; the
        // "references"/"no such" forms appear in the runtime-node guidance
        // messages. Warnings never reach this gate.
        if !is_warning
            && !(trimmed.contains("references")
                || trimmed.contains("no such")
                || trimmed.contains("does not exist"))
        {
            continue;
        }
        // Real dora 1.0 wiring errors address the target input as
        // `input `node/port`` (e.g. "mapped to input `detector/frame`");
        // split it first so the issue points at the canvas edge's target
        // node and port. When the input marker is a bare port name, fall
        // back to the `node `Z`` marker. Warnings quote both the node and
        // the input in double quotes.
        let input_marker = extract_backtick_field(trimmed, "input `");
        let (node_id, port_id) = if let Some(marker) = input_marker {
            match marker.split_once('/') {
                Some((node, port)) => (Some(node.to_string()), Some(port.to_string())),
                None => (
                    extract_backtick_field(trimmed, "node `"),
                    Some(marker.to_string()),
                ),
            }
        } else if is_warning {
            (
                extract_double_quoted_field(trimmed, "node \""),
                extract_quoted_field(trimmed, "input \""),
            )
        } else {
            (
                extract_backtick_field(trimmed, "node `"),
                extract_quoted_field(trimmed, "input \""),
            )
        };
        issues.push(SaveIssue {
            node_id,
            port_id,
            message: trimmed.to_string(),
        });
    }
    issues
}

fn extract_backtick_field(line: &str, prefix: &str) -> Option<String> {
    line.find(prefix).and_then(|start| {
        let rest = &line[start + prefix.len()..];
        rest.find('`').map(|end| rest[..end].to_string())
    })
}

fn extract_quoted_field(line: &str, prefix: &str) -> Option<String> {
    line.find(prefix).and_then(|start| {
        let rest = &line[start + prefix.len()..];
        rest.find('"').map(|end| rest[..end].to_string())
    })
}

fn extract_double_quoted_field(line: &str, prefix: &str) -> Option<String> {
    line.find(prefix).and_then(|start| {
        let rest = &line[start + prefix.len()..];
        rest.find('"').map(|end| rest[..end].to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_type_mismatch_warning_to_issue() {
        let text = r#"type mismatch on input "image": upstream cam/frame declares "std/media/v1/Image", but expected "std/media/v1/CompressedImage""#;
        let issues = map_validate_output(text, false);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].port_id.as_deref(), Some("image"));
        assert!(issues[0].message.contains("std/media/v1/Image"));
    }

    #[test]
    fn maps_wiring_error_to_issue() {
        let text = r#"node `detector`: input `frame` references `cam/frame` but node `cam` has no such output"#;
        let issues = map_validate_output(text, true);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].node_id.as_deref(), Some("detector"));
        assert_eq!(issues[0].port_id.as_deref(), Some("frame"));
    }

    #[test]
    fn ignores_unmappable_lines() {
        let issues = map_validate_output("some unrelated stderr noise\nanother line\n", true);
        assert!(issues.is_empty());
    }

    #[test]
    fn maps_real_dora_does_not_exist_error() {
        // Actual dora 1.0 wording for a missing output source.
        let text =
            r#"    output `cam/nonexistent` mapped to input `detector/frame` does not exist"#;
        let issues = map_validate_output(text, true);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].node_id.as_deref(), Some("detector"));
        assert_eq!(issues[0].port_id.as_deref(), Some("frame"));
        assert!(issues[0].message.contains("does not exist"));
    }

    #[test]
    fn maps_missing_source_node_error() {
        // real dora 1.0 format for a typo'd source node
        let text = "source node `cam` mapped to input `detector/frame` does not exist";
        let issues = map_validate_output(text, true);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].node_id.as_deref(), Some("detector"));
        assert_eq!(issues[0].port_id.as_deref(), Some("frame"));
    }

    #[test]
    fn maps_warning_node_prefix_with_double_quotes() {
        let text = r#"- node "sink": type mismatch on input "reading": upstream sensor/reading declares "std/core/v1/Float64", but expected "std/core/v1/UInt32""#;
        let issues = map_validate_output(text, false);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].node_id.as_deref(), Some("sink"));
        assert_eq!(issues[0].port_id.as_deref(), Some("reading"));
    }
}
