//! LayoutTypeResolver: resolves short type names to catalog aliases or template keys.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdlObjectType {
    pub name_def: String,
    pub alias: Option<String>,
}

impl SdlObjectType {
    pub fn new(name_def: impl Into<String>, alias: Option<impl Into<String>>) -> Self {
        Self {
            name_def: name_def.into(),
            alias: alias.map(|a| a.into()),
        }
    }

    pub fn alias(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.name_def)
    }
}

pub struct LayoutTypeResolver;

impl LayoutTypeResolver {
    /// Resolves short type names (e.g. BauGerHau00) to existing catalog aliases (e.g. GER_HAU00).
    pub fn resolve_alias(catalog: &[SdlObjectType], name: &str) -> Option<String> {
        let name_lower = name.to_ascii_lowercase();

        // 1. Exact alias match (case-insensitive)
        if let Some(hit) = catalog.iter().find(|t| t.alias().eq_ignore_ascii_case(name)) {
            return Some(hit.alias().to_string());
        }

        // 2. Exact name_def match (case-insensitive)
        if let Some(hit) = catalog.iter().find(|t| t.name_def.eq_ignore_ascii_case(name)) {
            return Some(hit.alias().to_string());
        }

        // 3. Prefix match name + "_", ordered alphabetically by name_def
        let prefix = format!("{}_", name_lower);
        let mut candidates: Vec<&SdlObjectType> = catalog
            .iter()
            .filter(|t| t.name_def.to_ascii_lowercase().starts_with(&prefix))
            .collect();

        candidates.sort_by(|a, b| {
            a.name_def
                .to_ascii_lowercase()
                .cmp(&b.name_def.to_ascii_lowercase())
        });

        candidates.first().map(|hit| hit.alias().to_string())
    }

    /// Resolves short type names to template keys in dictionary (e.g. LanGerNad00 -> LanGerNad00_Tanne_gross).
    pub fn resolve_key<V>(templates: &HashMap<String, V>, name: &str) -> Option<String> {
        // 1. Direct match (case-insensitive)
        if let Some((k, _)) = templates.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)) {
            return Some(k.clone());
        }

        // 2. Prefix match name + "_", sorted case-insensitively
        let prefix = format!("{}_", name.to_ascii_lowercase());
        let mut candidates: Vec<&String> = templates
            .keys()
            .filter(|k| k.to_ascii_lowercase().starts_with(&prefix))
            .collect();

        candidates.sort_by(|a, b| {
            a.to_ascii_lowercase()
                .cmp(&b.to_ascii_lowercase())
        });

        candidates.first().map(|k| (*k).clone())
    }
}
