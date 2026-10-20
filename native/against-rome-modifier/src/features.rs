//! Extensible feature registry ported faithfully from src.Core/Core/Features.
//! Defines all 49 game features across Stats, Compat, and Language categories.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureCategory {
    Stats,
    Compat,
    Language,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureControlKind {
    Toggle,
    Specialized,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FeatureValue {
    Bool(bool),
    Int(i32),
    String(String),
}

impl FeatureValue {
    pub fn as_bool(&self) -> bool {
        match self {
            FeatureValue::Bool(b) => *b,
            FeatureValue::Int(i) => *i != 0,
            FeatureValue::String(s) => s.eq_ignore_ascii_case("true") || s == "1",
        }
    }

    pub fn as_int(&self) -> i32 {
        match self {
            FeatureValue::Int(i) => *i,
            FeatureValue::Bool(b) => {
                if *b {
                    1
                } else {
                    0
                }
            }
            FeatureValue::String(s) => s.parse::<i32>().unwrap_or(0),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            FeatureValue::String(s) => s.as_str(),
            FeatureValue::Bool(true) => "true",
            FeatureValue::Bool(false) => "false",
            FeatureValue::Int(_) => "",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeatureDefinition {
    pub id: &'static str,
    pub category: FeatureCategory,
    pub control_kind: FeatureControlKind,
    pub disabled_value: FeatureValue,
}

pub struct FeatureRegistry;

impl FeatureRegistry {
    pub const ALL: &'static [FeatureDefinition] = &[
        FeatureDefinition {
            id: "FastCiviProduction",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "InfiniteMorale",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "NoRunHpLoss",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "Balance",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "FreeProduction",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "FreeUpgrade",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "NoSpellCost",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "HousingCapacity20x",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "StorageCapacity10x",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "HqHp10x",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "FastBuildUpgradeRepair",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "FoodHealing10x",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "MaxPopulation",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "CiviProduce20",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "UnitRecruit20",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "IdleSelect999",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "CorpseRetention",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "RomanEndless",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "RomanReinforcementGarrison",
            category: FeatureCategory::Compat,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "VillageGarrisonQuota3x",
            category: FeatureCategory::Compat,
            control_kind: FeatureControlKind::Specialized,
            disabled_value: FeatureValue::Int(1),
        },
        FeatureDefinition {
            id: "SpellDamage5x",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "SpellHealing10x",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "SpellResurrection",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "GeneralSkills",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "LeaderGlory",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "RangedRange3x",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "UnitMovementSpeed2x",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "VillagerMovementSpeed5x",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "SpellEntireMap",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "SpellRange3x",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "ProjectileArcHeight",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "AllUnitsEntireMapVision",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "CustomUnitStats",
            category: FeatureCategory::Stats,
            control_kind: FeatureControlKind::Specialized,
            disabled_value: FeatureValue::String(String::new()),
        },
        FeatureDefinition {
            id: "FocusLoss",
            category: FeatureCategory::Compat,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "VillageBuildRange",
            category: FeatureCategory::Compat,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "NoSpellAltar",
            category: FeatureCategory::Compat,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "GameSpeed",
            category: FeatureCategory::Compat,
            control_kind: FeatureControlKind::Specialized,
            disabled_value: FeatureValue::Int(1),
        },
        FeatureDefinition {
            id: "DgVoodoo",
            category: FeatureCategory::Compat,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "ArgmTrace",
            category: FeatureCategory::Compat,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "NativeWidescreen1920x1080",
            category: FeatureCategory::Compat,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "CameraZoomOut1",
            category: FeatureCategory::Compat,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "EndlessAi.M1",
            category: FeatureCategory::Compat,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "EndlessAi.Core",
            category: FeatureCategory::Compat,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "EndlessAi.M2",
            category: FeatureCategory::Compat,
            control_kind: FeatureControlKind::Specialized,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "EndlessAi.M3",
            category: FeatureCategory::Compat,
            control_kind: FeatureControlKind::Specialized,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "EndlessAi.M4",
            category: FeatureCategory::Compat,
            control_kind: FeatureControlKind::Specialized,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "EndlessAi.M5",
            category: FeatureCategory::Compat,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "EndlessAi.M6",
            category: FeatureCategory::Compat,
            control_kind: FeatureControlKind::Specialized,
            disabled_value: FeatureValue::Bool(false),
        },
        FeatureDefinition {
            id: "ToEnglish",
            category: FeatureCategory::Language,
            control_kind: FeatureControlKind::Toggle,
            disabled_value: FeatureValue::Bool(false),
        },
    ];

    pub fn get(id: &str) -> Option<&'static FeatureDefinition> {
        Self::ALL.iter().find(|f| f.id.eq_ignore_ascii_case(id))
    }

    pub fn by_category(
        category: FeatureCategory,
    ) -> impl Iterator<Item = &'static FeatureDefinition> {
        Self::ALL.iter().filter(move |f| f.category == category)
    }

    pub fn toggle_features() -> impl Iterator<Item = &'static FeatureDefinition> {
        Self::ALL
            .iter()
            .filter(|f| f.control_kind == FeatureControlKind::Toggle)
    }

    pub fn get_disabled_value(id: &str) -> FeatureValue {
        Self::get(id)
            .map(|f| f.disabled_value.clone())
            .unwrap_or(FeatureValue::Bool(false))
    }

    pub fn validate_toggle_ids(ids: &[&str]) -> Result<(), String> {
        let expected: HashSet<&str> = Self::toggle_features().map(|f| f.id).collect();
        let actual: HashSet<&str> = ids.iter().copied().collect();

        if expected == actual {
            return Ok(());
        }

        let missing: Vec<&str> = expected.difference(&actual).copied().collect();
        let unexpected: Vec<&str> = actual.difference(&expected).copied().collect();

        Err(format!(
            "Feature toggle map mismatch. Missing: {:?}. Unexpected: {:?}",
            missing, unexpected
        ))
    }
}

/// Patch profile holding arbitrary configuration values for all features.
#[derive(Debug, Clone, Default)]
pub struct PatchProfile {
    values: HashMap<String, FeatureValue>,
    pub custom_unit_stats: Option<std::collections::BTreeMap<String, Vec<f64>>>,
    pub endless_ai_modules: HashMap<String, bool>,
}

impl PatchProfile {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: &str) -> FeatureValue {
        if let Some((_, v)) = self.values.iter().find(|(k, _)| k.eq_ignore_ascii_case(id)) {
            v.clone()
        } else {
            FeatureRegistry::get_disabled_value(id)
        }
    }

    pub fn get_bool(&self, id: &str) -> bool {
        self.get(id).as_bool()
    }

    pub fn get_int(&self, id: &str) -> i32 {
        self.get(id).as_int()
    }

    pub fn has_explicit_value(&self, id: &str) -> bool {
        self.values.keys().any(|k| k.eq_ignore_ascii_case(id))
    }

    pub fn set(&mut self, id: impl Into<String>, value: FeatureValue) {
        let key = id.into();
        if let Some(existing) = self
            .values
            .keys()
            .find(|k| k.eq_ignore_ascii_case(&key))
            .cloned()
        {
            self.values.remove(&existing);
        }
        self.values.insert(key, value);
    }

    pub fn set_bool(&mut self, id: impl Into<String>, value: bool) {
        self.set(id, FeatureValue::Bool(value));
    }

    pub fn set_int(&mut self, id: impl Into<String>, value: i32) {
        self.set(id, FeatureValue::Int(value));
    }

    pub fn set_string(&mut self, id: impl Into<String>, value: impl Into<String>) {
        self.set(id, FeatureValue::String(value.into()));
    }

    /// C# PatchProfile.GetEndlessAiModule, including legacy profile selections.
    pub fn get_endless_ai_module(&self, module_id: &str) -> bool {
        let legacy = self
            .endless_ai_modules
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(module_id))
            .map(|(_, v)| *v);
        if module_id.eq_ignore_ascii_case("M6") {
            return self.get_bool("RomanReinforcementGarrison")
                || legacy.unwrap_or_else(|| self.get_bool("EndlessAi.M6"));
        }
        if module_id.eq_ignore_ascii_case("VillageGarrisonQuota3x") {
            return self.get_int("VillageGarrisonQuotaMultiplier") > 1 || legacy.unwrap_or(false);
        }
        if let Some(v) = legacy {
            return v;
        }
        match module_id.to_ascii_uppercase().as_str() {
            "M1" => self.get_bool("EndlessAi.M1"),
            "CORE" => self.get_bool("EndlessAi.Core"),
            "M2" | "M3" | "M4" => self.get_bool("EndlessAi.Core"),
            "M5" => self.get_bool("EndlessAi.M5"),
            _ => false,
        }
    }

    /// C# PatchProfile.NormalizeCompositeValues: M6 mirrors RomanReinforcementGarrison; M2/M3/M4 mirror Core.
    pub fn normalize_composite_values(&mut self) {
        let modules: Vec<(String, bool)> = self
            .endless_ai_modules
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        let mut roman = self.get_bool("RomanReinforcementGarrison");
        let mut legacy_core = false;
        for (id, enabled) in &modules {
            if id.eq_ignore_ascii_case("M6") {
                roman |= *enabled;
            } else if id.eq_ignore_ascii_case("VillageGarrisonQuota3x") {
                if *enabled && self.get_int("VillageGarrisonQuotaMultiplier") <= 1 {
                    self.set_int("VillageGarrisonQuotaMultiplier", 3);
                }
            } else {
                self.set_bool(format!("EndlessAi.{id}"), *enabled);
                if ["M2", "M3", "M4"]
                    .iter()
                    .any(|m| id.eq_ignore_ascii_case(m))
                {
                    legacy_core |= *enabled;
                }
            }
        }
        let roman = roman | self.get_bool("EndlessAi.M6");
        self.set_bool("RomanReinforcementGarrison", roman);
        self.set_bool("EndlessAi.M6", roman);
        let core = self.get_bool("EndlessAi.Core") | legacy_core;
        if modules.iter().any(|(id, _)| {
            ["Core", "M2", "M3", "M4"]
                .iter()
                .any(|m| id.eq_ignore_ascii_case(m))
        }) {
            for id in ["Core", "M2", "M3", "M4"] {
                self.endless_ai_modules
                    .retain(|key, _| !key.eq_ignore_ascii_case(id));
                self.endless_ai_modules.insert(id.into(), core);
            }
        }
        self.set_bool("EndlessAi.Core", core);
        for id in ["EndlessAi.M2", "EndlessAi.M3", "EndlessAi.M4"] {
            self.set_bool(id, core);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_registry_contains_expected_count() {
        assert_eq!(FeatureRegistry::ALL.len(), 49);
        let toggles: Vec<_> = FeatureRegistry::toggle_features().collect();
        assert_eq!(toggles.len(), 42);
        let specialized: Vec<_> = FeatureRegistry::ALL
            .iter()
            .filter(|f| f.control_kind == FeatureControlKind::Specialized)
            .collect();
        assert_eq!(specialized.len(), 7);
    }

    #[test]
    fn validate_toggle_ids_matches_canonical() {
        let all_toggle_ids: Vec<&str> = FeatureRegistry::toggle_features().map(|f| f.id).collect();
        assert!(FeatureRegistry::validate_toggle_ids(&all_toggle_ids).is_ok());

        let incomplete = &all_toggle_ids[..5];
        assert!(FeatureRegistry::validate_toggle_ids(incomplete).is_err());
    }

    #[test]
    fn patch_profile_case_insensitive_lookup_and_defaults() {
        let mut profile = PatchProfile::new();
        assert!(!profile.get_bool("FastCiviProduction"));
        assert!(!profile.get_bool("fastciviproduction"));
        assert_eq!(profile.get_int("GameSpeed"), 1);

        profile.set_bool("FastCiviProduction", true);
        assert!(profile.get_bool("fastciviproduction"));

        profile.set_int("GameSpeed", 5);
        assert_eq!(profile.get_int("gamespeed"), 5);
    }
}
