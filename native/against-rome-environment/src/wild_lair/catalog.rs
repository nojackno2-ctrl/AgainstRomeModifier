//! Catalog of observed wild animals and scenario lair definitions.

use super::models::{
    LairCategory, LairDifficultyTier, LairLootReward, NeutralLairDefinition,
};
use std::collections::HashMap;

/// 中立巢穴與野生動物目錄。
#[derive(Debug, Clone)]
pub struct NeutralLairCatalog {
    definitions: HashMap<String, NeutralLairDefinition>,
    order: Vec<String>,
}

impl Default for NeutralLairCatalog {
    fn default() -> Self {
        Self::new(Self::create_canonical_definitions()).expect("canonical definitions are valid")
    }
}

impl NeutralLairCatalog {
    pub fn new(
        definitions: impl IntoIterator<Item = NeutralLairDefinition>,
    ) -> Result<Self, String> {
        let mut map = HashMap::new();
        let mut order = Vec::new();
        for def in definitions {
            Self::validate_definition(&def)?;
            let key = def.id.to_ascii_uppercase();
            if !map.contains_key(&key) {
                order.push(key.clone());
            }
            map.insert(key, def);
        }
        Ok(Self {
            definitions: map,
            order,
        })
    }

    pub fn all_definitions(&self) -> Vec<&NeutralLairDefinition> {
        self.order
            .iter()
            .filter_map(|k| self.definitions.get(k))
            .collect()
    }

    pub fn get_by_id(&self, id: &str) -> Option<&NeutralLairDefinition> {
        if id.trim().is_empty() {
            return None;
        }
        self.definitions.get(&id.to_ascii_uppercase())
    }

    pub fn get_by_category(&self, category: LairCategory) -> Vec<&NeutralLairDefinition> {
        self.all_definitions()
            .into_iter()
            .filter(|d| d.category == category)
            .collect()
    }

    pub fn get_by_tier(&self, tier: LairDifficultyTier) -> Vec<&NeutralLairDefinition> {
        self.all_definitions()
            .into_iter()
            .filter(|d| d.tier == tier)
            .collect()
    }

    pub fn filter(
        &self,
        category: Option<LairCategory>,
        tier: Option<LairDifficultyTier>,
        biome: Option<&str>,
    ) -> Vec<&NeutralLairDefinition> {
        self.all_definitions()
            .into_iter()
            .filter(|d| {
                if let Some(cat) = category {
                    if d.category != cat {
                        return false;
                    }
                }
                if let Some(t) = tier {
                    if d.tier != t {
                        return false;
                    }
                }
                if let Some(b) = biome {
                    if !b.trim().is_empty() && !d.biome_affinity.eq_ignore_ascii_case(b) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    pub fn calculate_threat_rating(definition: &NeutralLairDefinition) -> Result<f32, String> {
        if definition.default_guards.is_empty() && definition.wave_rules.is_empty() {
            return Ok(0.0);
        }
        Err("No evidence-based native threat formula is available.".to_string())
    }

    pub fn validate_definition(def: &NeutralLairDefinition) -> Result<(), String> {
        if def.id.trim().is_empty() {
            return Err("巢穴識別碼不得為空。".to_string());
        }
        if def.display_name_zh.trim().is_empty() {
            return Err("巢穴繁體中文名稱不得為空。".to_string());
        }
        if def.native_building_or_landscape_type.trim().is_empty() {
            return Err("原生建築或地景代碼不得為空。".to_string());
        }
        if !def.footprint_radius_tiles.is_finite() || def.footprint_radius_tiles <= 0.0 {
            return Err("佔地半徑必須大於 0。".to_string());
        }

        for wave in &def.wave_rules {
            if wave.interval_seconds < 10 {
                return Err(format!(
                    "刷怪週期過短（{} 秒），最低間隔須 >= 10 秒。",
                    wave.interval_seconds
                ));
            }
            if !(1..=20).contains(&wave.spawn_count) {
                return Err("刷怪人數必須介於 1 至 20 人之間。".to_string());
            }
        }

        Ok(())
    }

    pub fn create_canonical_definitions() -> Vec<NeutralLairDefinition> {
        vec![
            animal("ALL_WOL00", "FigTieWol00_Wilder_Wolf", "Wild wolf"),
            animal("ALL_BAE00", "FigTieBae00_Baer", "Bear"),
            animal("ALL_EBE00", "FigTieEbe00_Wildschwein", "Wild boar"),
            animal("ALL_RAU00", "FigTieRau00_Raubkatze", "Wild cat"),
        ]
    }

    /// Require the loaded game's alias to resolve to exactly the observed definition.
    pub fn filtered_by(&self, aliases: &[(String, String)]) -> Self {
        let matching = self.all_definitions().into_iter().filter(|d| {
            if let Some(script_alias) = &d.script_alias {
                aliases.iter().any(|(alias, name_def)| {
                    alias.eq_ignore_ascii_case(script_alias)
                        && name_def.eq_ignore_ascii_case(&d.native_building_or_landscape_type)
                })
            } else {
                false
            }
        }).cloned().collect::<Vec<_>>();

        Self::new(matching).unwrap_or_else(|_| Self {
            definitions: HashMap::new(),
            order: Vec::new(),
        })
    }
}

fn animal(alias: &str, name: &str, display: &str) -> NeutralLairDefinition {
    NeutralLairDefinition {
        id: alias.to_string(),
        display_name_zh: display.to_string(),
        display_name_en: display.to_string(),
        category: LairCategory::WildAnimal,
        tier: LairDifficultyTier::Unrated,
        native_building_or_landscape_type: name.to_string(),
        footprint_radius_tiles: 1.0,
        default_guards: Vec::new(),
        wave_rules: Vec::new(),
        loot: LairLootReward::default(),
        biome_affinity: String::new(),
        description: "Observed in objdef and cl_scint.ini; no native lair, patrol, reward or respawn rule established.".to_string(),
        script_alias: Some(alias.to_string()),
    }
}
