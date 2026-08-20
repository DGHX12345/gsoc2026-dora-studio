//! dora 1.0 compatibility semantics replicated for Studio (M18).
//!
//! Mirrors dora-core `types.rs` CompatibilityGraph + schema_compatible:
//! same-base URN (param agreement), 4 builtin widening edges, universal
//! `* -> Bytes` sink, user type_rules with BFS depth <= 3, and structural
//! struct compatibility (schema_compatible).

use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedUrn {
    pub base: String,
    pub params: BTreeMap<String, String>,
}

pub fn parse_urn(urn: &str) -> Option<ParsedUrn> {
    if urn.is_empty() {
        return None;
    }
    let Some((base, rest)) = urn.split_once('[') else {
        // Mirror dora types.rs: bracketless base kept verbatim (no trim).
        return Some(ParsedUrn {
            base: urn.to_string(),
            params: BTreeMap::new(),
        });
    };
    // Has '[' but no closing ']' -> malformed (mirrors dora's ends_with check).
    let params = rest.strip_suffix(']')?;
    if params.is_empty() {
        return None; // empty brackets
    }
    // NOTE: dora accepts "[a=b]" as base ""; we intentionally reject an
    // empty base (stricter, fail-safe direction). Identical malformed
    // strings on both ports still compare equal via the raw-string
    // fallback in check().
    if base.is_empty() {
        return None;
    }
    let mut map = BTreeMap::new();
    for pair in params.split(',') {
        // Mirror dora types.rs: params trimmed per part/key/value (base is not).
        let pair = pair.trim();
        let (key, value) = pair.split_once('=')?;
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            return None;
        }
        map.insert(key.to_string(), value.to_string());
    }
    Some(ParsedUrn {
        base: base.to_string(),
        params: map,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub compatible: bool,
    pub level: String, // "compatible" | "rule" | "incompatible" | "unknown"
    pub reason: String,
    pub suggestion: Option<String>,
    pub rule: Option<(String, String)>,
}

const BUILTIN_WIDENING: &[(&str, &str)] = &[
    ("std/core/v1/UInt8", "std/core/v1/UInt32"),
    ("std/core/v1/UInt32", "std/core/v1/UInt64"),
    ("std/core/v1/Int32", "std/core/v1/Int64"),
    ("std/core/v1/Float32", "std/core/v1/Float64"),
];

/// Check compatibility between a source output URN and sink input URN.
/// `user_rules` are the dataflow-level type_rules declarations.
pub fn check(
    source_urn: Option<&str>,
    sink_urn: Option<&str>,
    user_rules: &[(String, String)],
) -> CheckResult {
    let (Some(from), Some(to)) = (source_urn, sink_urn) else {
        return CheckResult {
            compatible: false,
            level: "unknown".into(),
            reason: "One or both ports have no declared type URN.".into(),
            suggestion: Some(
                "Declare a type URN on both ports to enable compatibility checking.".into(),
            ),
            rule: None,
        };
    };

    // Universal sink
    if to == "std/core/v1/Bytes" {
        return CheckResult {
            compatible: true,
            level: "compatible".into(),
            reason: "Anything can connect to std/core/v1/Bytes.".into(),
            suggestion: None,
            rule: None,
        };
    }

    let from_parsed = parse_urn(from);
    let to_parsed = parse_urn(to);
    let from_base = from_parsed
        .as_ref()
        .map(|p| p.base.as_str())
        .unwrap_or(from);
    let to_base = to_parsed.as_ref().map(|p| p.base.as_str()).unwrap_or(to);

    if from_base == to_base {
        let agree = match (&from_parsed, &to_parsed) {
            (Some(fp), Some(tp)) if !fp.params.is_empty() && !tp.params.is_empty() => fp
                .params
                .iter()
                .all(|(key, value)| match tp.params.get(key) {
                    Some(other) => value == other,
                    None => true,
                }),
            _ => true,
        };
        if agree {
            return CheckResult {
                compatible: true,
                level: "compatible".into(),
                reason: format!("Same type: {from_base}."),
                suggestion: None,
                rule: None,
            };
        }
        return CheckResult {
            compatible: false,
            level: "incompatible".into(),
            reason: format!("Parameterized type mismatch between {from} and {to}."),
            suggestion: Some(
                "Align the type parameters on both ports, or use a conversion node.".to_string(),
            ),
            rule: None,
        };
    }

    // BFS with depth limit 3; user edges mark the path as "rule"
    let mut edges: BTreeMap<&str, Vec<(&str, bool)>> = BTreeMap::new();
    for (f, t) in BUILTIN_WIDENING {
        edges.entry(f).or_default().push((t, false));
    }
    for (f, t) in user_rules {
        edges
            .entry(f.as_str())
            .or_default()
            .push((t.as_str(), true));
    }

    let mut queue = VecDeque::new();
    let mut visited = std::collections::BTreeSet::new();
    queue.push_back((from_base.to_string(), 0u32, false, None));
    visited.insert(from_base.to_string());

    while let Some((current, depth, rule_used, first_rule)) = queue.pop_front() {
        if current == to_base {
            let (level, reason) = if rule_used {
                (
                    "rule",
                    format!("Compatible via declared type_rules: {from} -> {to}."),
                )
            } else {
                (
                    "compatible",
                    format!("Compatible via built-in widening: {from} -> {to}."),
                )
            };
            return CheckResult {
                compatible: true,
                level: level.into(),
                reason,
                suggestion: None,
                rule: first_rule,
            };
        }
        if depth >= 3 {
            continue;
        }
        if let Some(neighbors) = edges.get(current.as_str()) {
            for (next, is_user) in neighbors {
                if !visited.contains(*next) {
                    visited.insert((*next).to_string());
                    let rule = if *is_user {
                        first_rule
                            .clone()
                            .or(Some((current.clone(), (*next).to_string())))
                    } else {
                        first_rule.clone()
                    };
                    queue.push_back(((*next).to_string(), depth + 1, rule_used || *is_user, rule));
                }
            }
        }
    }

    CheckResult {
        compatible: false,
        level: "incompatible".into(),
        reason: format!(
            "No compatibility path from {from} to {to} (built-in widening and declared type_rules exhausted)."
        ),
        suggestion: Some(format!(
            "Insert a conversion node that transforms {from} into {to}, or declare a type_rule for this pair."
        )),
        rule: None,
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeField {
    pub name: String,
    pub field_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaError {
    MissingField {
        field: String,
    },
    TypeMismatch {
        field: String,
        expected: String,
        actual: String,
    },
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaError::MissingField { field } => write!(f, "missing field \"{field}\""),
            SchemaError::TypeMismatch {
                field,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "field \"{field}\" type mismatch: expected {expected}, got {actual}"
                )
            }
        }
    }
}

/// Mirrors dora types.rs `schema_compatible`: every expected field must be
/// present in actual with an equal (normalized) type. Order is irrelevant.
pub fn schema_compatible(expected: &[TypeField], actual: &[TypeField]) -> Result<(), SchemaError> {
    for expected_field in expected {
        match actual
            .iter()
            .find(|field| field.name == expected_field.name)
        {
            Some(actual_field) => {
                if normalize_field_type(&actual_field.field_type)
                    != normalize_field_type(&expected_field.field_type)
                {
                    return Err(SchemaError::TypeMismatch {
                        field: expected_field.name.clone(),
                        expected: expected_field.field_type.clone(),
                        actual: actual_field.field_type.clone(),
                    });
                }
            }
            None => {
                return Err(SchemaError::MissingField {
                    field: expected_field.name.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Canonical form for comparing field type strings: trims, uppercases
/// primitive Arrow names, and normalizes List<...> recursively. Struct
/// references compare by exact (trimmed) string.
///
/// Conservative deviation from dora (which deep-compares resolved struct
/// schemas): two different struct URNs with identical nested schemas
/// compare unequal here.
pub fn normalize_field_type(field_type: &str) -> String {
    let trimmed = field_type.trim();
    if let Some(inner) = trimmed
        .strip_prefix("List<")
        .and_then(|s| s.strip_suffix('>'))
    {
        return format!("List<{}>", normalize_field_type(inner));
    }
    trimmed.to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(rules: &[(&str, &str)]) -> Vec<(String, String)> {
        rules
            .iter()
            .map(|(f, t)| (f.to_string(), t.to_string()))
            .collect()
    }

    // 对齐 dora types.rs: 同 base URN 兼容（参数共享键一致）
    #[test]
    fn same_base_urn_is_compatible() {
        let result = check(Some("std/media/v1/Image"), Some("std/media/v1/Image"), &[]);
        assert!(result.compatible);
        assert_eq!(result.level, "compatible");
    }

    // 对齐 dora types.rs: 参数化同 base，共享键一致 → 兼容；不一致 → 不兼容
    #[test]
    fn parameterized_urn_shared_keys_must_agree() {
        let ok = check(
            Some("std/media/v1/AudioFrame[sample_type=f32,channels=2]"),
            Some("std/media/v1/AudioFrame[sample_type=f32]"),
            &[],
        );
        assert!(ok.compatible);
        let bad = check(
            Some("std/media/v1/AudioFrame[sample_type=f32]"),
            Some("std/media/v1/AudioFrame[sample_type=f64]"),
            &[],
        );
        assert!(!bad.compatible);
    }

    // 对齐 dora types.rs 内置拓宽: UInt8→UInt32→UInt64 链（BFS 两步）
    #[test]
    fn widening_chain_uint8_to_uint64() {
        let result = check(Some("std/core/v1/UInt8"), Some("std/core/v1/UInt64"), &[]);
        assert!(result.compatible);
        assert_eq!(result.level, "compatible");
    }

    // 对齐 dora validate.rs: anything -> Bytes 兼容
    #[test]
    fn anything_to_bytes_is_compatible() {
        let result = check(Some("std/media/v1/Image"), Some("std/core/v1/Bytes"), &[]);
        assert!(result.compatible);
    }

    // 对齐 dora validate.rs compat_user_defined_rule_in_yaml: 用户规则生效
    #[test]
    fn user_rule_makes_compatible_with_rule_level() {
        let result = check(
            Some("std/core/v1/UInt8"),
            Some("std/core/v1/String"),
            &rules(&[("std/core/v1/UInt8", "std/core/v1/String")]),
        );
        assert!(result.compatible);
        assert_eq!(result.level, "rule");
        assert_eq!(
            result.rule,
            Some((
                "std/core/v1/UInt8".to_string(),
                "std/core/v1/String".to_string()
            ))
        );
    }

    // 对齐 dora types.rs BFS 深度限制: 4 跳用户链超过深度 3 → 不兼容
    #[test]
    fn user_rule_chain_beyond_depth_3_is_incompatible() {
        let chain = rules(&[
            ("a/b/v1/T0", "a/b/v1/T1"),
            ("a/b/v1/T1", "a/b/v1/T2"),
            ("a/b/v1/T2", "a/b/v1/T3"),
            ("a/b/v1/T3", "a/b/v1/T4"),
        ]);
        let result = check(Some("a/b/v1/T0"), Some("a/b/v1/T4"), &chain);
        assert!(!result.compatible);
    }

    // 对齐 dora types.rs compat_depth_limit: 3 跳用户链在深度限制内 → 兼容
    #[test]
    fn user_rule_chain_within_depth_3_is_compatible() {
        let chain = rules(&[
            ("a/b/v1/T0", "a/b/v1/T1"),
            ("a/b/v1/T1", "a/b/v1/T2"),
            ("a/b/v1/T2", "a/b/v1/T3"),
        ]);
        let result = check(Some("a/b/v1/T0"), Some("a/b/v1/T3"), &chain);
        assert!(result.compatible);
        assert_eq!(result.level, "rule");
    }

    // 不相关类型不兼容
    #[test]
    fn unrelated_types_incompatible_with_reason() {
        let result = check(Some("std/media/v1/Image"), Some("std/core/v1/Float64"), &[]);
        assert!(!result.compatible);
        assert_eq!(result.level, "incompatible");
        assert!(result.reason.contains("std/media/v1/Image"));
    }

    // 任一端未声明 → unknown
    #[test]
    fn undeclared_port_is_unknown() {
        let result = check(None, Some("std/core/v1/Float64"), &[]);
        assert!(!result.compatible);
        assert_eq!(result.level, "unknown");
    }

    // parse_urn 边界
    #[test]
    fn parse_urn_handles_malformed_input() {
        assert!(parse_urn("std/media/v1/AudioFrame[").is_none());
        assert!(parse_urn("std/media/v1/AudioFrame[]").is_none());
        assert!(parse_urn("std/media/v1/AudioFrame").is_some());
    }

    // 对齐 dora: base 不做 trim（尾部空格与 dora 行为一致 → 不兼容）
    #[test]
    fn padded_urn_matches_dora_behavior() {
        let result = check(Some("std/core/v1/UInt32"), Some("std/core/v1/UInt32 "), &[]);
        assert!(!result.compatible);
    }

    // 对齐 dora types.rs schema_compatible: actual 字段 ⊇ expected 字段
    #[test]
    fn struct_actual_superset_of_expected_ok() {
        let result = schema_compatible(
            &[field("width", "UInt32"), field("data", "LargeBinary")],
            &[
                field("width", "UInt32"),
                field("height", "UInt32"),
                field("data", "LargeBinary"),
            ],
        );
        assert!(result.is_ok());
    }

    // 对齐 dora types.rs SchemaError::MissingField
    #[test]
    fn struct_missing_expected_field_fails() {
        let result = schema_compatible(
            &[field("width", "UInt32"), field("data", "LargeBinary")],
            &[field("width", "UInt32")],
        );
        match result {
            Err(SchemaError::MissingField { field }) => assert_eq!(field, "data"),
            other => panic!("expected MissingField, got {other:?}"),
        }
    }

    // 对齐 dora types.rs SchemaError::TypeMismatch
    #[test]
    fn struct_field_type_mismatch_fails() {
        let result = schema_compatible(&[field("data", "LargeBinary")], &[field("data", "Utf8")]);
        match result {
            Err(SchemaError::TypeMismatch { field, .. }) => assert_eq!(field, "data"),
            other => panic!("expected TypeMismatch, got {other:?}"),
        }
    }

    fn field(name: &str, field_type: &str) -> TypeField {
        TypeField {
            name: name.to_string(),
            field_type: field_type.to_string(),
        }
    }
}
