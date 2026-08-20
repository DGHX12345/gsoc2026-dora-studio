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
        return Err("dora 0.x does not support typed validation; final check skipped.".into());
    }
    let bin = crate::dora_env::resolve_dora_bin();
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::process::Command::new(&bin)
            .arg("validate")
            .arg(path)
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .map_err(|_| "dora validate timed out after 10s".to_string())?
    .map_err(|error| format!("failed to spawn dora validate: {error}"))?;

    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let success = output.status.success();

    Ok(ValidateOutcome {
        errors: if success {
            Vec::new()
        } else {
            map_validate_output(&text, true)
        },
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
        let mut node_id = extract_backtick_field(trimmed, "node `");
        // Warnings quote the input id (`input "image"`); wiring errors use
        // backticks (`input `frame``) — mirror both real dora formats.
        let mut port_id = if is_warning {
            extract_quoted_field(trimmed, "input \"")
        } else {
            extract_backtick_field(trimmed, "input `")
        };
        // Wiring errors address the input as `node/port` without a separate
        // `node `...`` marker; split it so the issue points at the canvas
        // edge's target node and port.
        if let (Some(raw), None) = (&port_id, &node_id) {
            if let Some((node, port)) = raw.split_once('/') {
                node_id = Some(node.to_string());
                port_id = Some(port.to_string());
            }
        }
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
}
