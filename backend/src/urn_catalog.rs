//! Vendored dora 1.0 std type catalog (M18).
//!
//! The YAML files under assets/types/ are copied verbatim from dora
//! 1.0.0-rc.4 `libraries/core/types/std/` and parsed with the same
//! zero-dependency line-based approach used by dataflows.rs.

use std::collections::BTreeMap;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeField {
    pub name: String,
    pub field_type: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeParam {
    pub name: String,
    pub default: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeDef {
    pub urn: String,
    pub name: String,
    pub category: String,
    pub arrow: String,
    pub description: Option<String>,
    pub fields: Vec<TypeField>,
    pub params: Vec<TypeParam>,
}

const PACKAGES: &[(&str, &str)] = &[
    ("core", include_str!("../assets/types/core/v1.yml")),
    ("math", include_str!("../assets/types/math/v1.yml")),
    ("control", include_str!("../assets/types/control/v1.yml")),
    ("media", include_str!("../assets/types/media/v1.yml")),
    ("vision", include_str!("../assets/types/vision/v1.yml")),
];

#[derive(Debug, Clone)]
pub struct Catalog {
    types: BTreeMap<String, TypeDef>,
}

impl Catalog {
    pub fn new() -> Self {
        let mut types = BTreeMap::new();
        for (category, yaml) in PACKAGES {
            for def in parse_package(category, yaml) {
                types.insert(def.urn.clone(), def);
            }
        }
        Self { types }
    }

    pub fn entries(&self) -> Vec<TypeDef> {
        self.types.values().cloned().collect()
    }

    /// Resolve a URN, stripping parameter suffixes for lookup.
    pub fn resolve(&self, urn: &str) -> Option<TypeDef> {
        let base = urn.split('[').next().unwrap_or(urn);
        self.types.get(base).cloned()
    }

    /// Resolve by short type name (last path segment).
    pub fn resolve_short_name(&self, name: &str) -> Option<TypeDef> {
        self.types.values().find(|def| def.name == name).cloned()
    }
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}

fn clean_scalar(value: &str) -> String {
    value
        .trim()
        .trim_matches(|ch| matches!(ch, '\'' | '"'))
        .to_string()
}

fn parse_package(category: &str, yaml: &str) -> Vec<TypeDef> {
    let mut defs = Vec::new();
    let mut current: Option<TypeDef> = None;
    let mut in_fields = false;
    let mut in_params = false;

    for raw_line in yaml.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = raw_line.chars().take_while(|ch| ch.is_whitespace()).count();

        if indent == 0 {
            // "types:" root key
            continue;
        }
        if indent == 2 && trimmed.ends_with(':') {
            if let Some(def) = current.take() {
                defs.push(def);
            }
            let name = trimmed.trim_end_matches(':');
            current = Some(TypeDef {
                urn: format!("std/{category}/v1/{name}"),
                name: name.to_string(),
                category: category.to_string(),
                arrow: String::new(),
                description: None,
                fields: Vec::new(),
                params: Vec::new(),
            });
            in_fields = false;
            in_params = false;
            continue;
        }

        let Some(def) = current.as_mut() else {
            continue;
        };

        if trimmed == "fields:" {
            in_fields = true;
            in_params = false;
            continue;
        }
        if trimmed == "params:" {
            in_params = true;
            in_fields = false;
            continue;
        }
        if indent == 4 && !trimmed.starts_with('-') {
            if let Some(value) = trimmed.strip_prefix("arrow:") {
                def.arrow = clean_scalar(value);
            } else if let Some(value) = trimmed.strip_prefix("description:") {
                def.description = Some(clean_scalar(value));
            } else {
                eprintln!(
                    "urn_catalog: ignoring unknown type-level key '{}' in {category}",
                    trimmed
                );
                in_fields = false;
                in_params = false;
            }
            continue;
        }
        if in_fields {
            if let Some(value) = trimmed.strip_prefix("- name:") {
                def.fields.push(TypeField {
                    name: clean_scalar(value),
                    field_type: String::new(),
                });
            } else if let Some(value) = trimmed.strip_prefix("type:") {
                if let Some(field) = def.fields.last_mut() {
                    field.field_type = clean_scalar(value);
                }
            }
        } else if in_params {
            if let Some(value) = trimmed.strip_prefix("- name:") {
                def.params.push(TypeParam {
                    name: clean_scalar(value),
                    default: None,
                });
            } else if let Some(value) = trimmed.strip_prefix("default:") {
                if let Some(param) = def.params.last_mut() {
                    param.default = Some(clean_scalar(value));
                }
            }
        }
    }
    if let Some(def) = current {
        defs.push(def);
    }
    defs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_media_catalog() {
        let catalog = Catalog::new();
        let image = catalog
            .resolve("std/media/v1/Image")
            .expect("Image resolves");
        assert_eq!(image.arrow, "Struct");
        assert_eq!(image.fields.len(), 4);
        assert!(image
            .description
            .as_deref()
            .unwrap_or_default()
            .contains("image"));
        assert!(image
            .fields
            .iter()
            .any(|field| field.name == "width" && field.field_type == "UInt32"));
        let compressed = catalog
            .resolve("std/media/v1/CompressedImage")
            .expect("resolves");
        assert_eq!(compressed.arrow, "LargeBinary");
    }

    #[test]
    fn resolves_short_names_and_params() {
        let catalog = Catalog::new();
        assert!(catalog.resolve_short_name("Image").is_some());
        // parameterized URN resolves via base
        let audio = catalog
            .resolve("std/media/v1/AudioFrame[sample_type=f32]")
            .expect("resolves");
        assert!(!audio.params.is_empty());
        assert_eq!(audio.params[0].name, "sample_type");
        assert_eq!(audio.params[0].default.as_deref(), Some("f32"));
    }

    #[test]
    fn lists_all_urns_grouped() {
        let catalog = Catalog::new();
        let all = catalog.entries();
        assert!(all.iter().any(|entry| entry.urn == "std/core/v1/Bytes"));
        assert!(all.len() >= 20);
    }
}
