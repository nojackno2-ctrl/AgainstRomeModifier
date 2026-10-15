//! Data models for neutral lairs, wild animals, and resource definitions.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 巢穴分類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LairCategory {
    BeastDen,
    BarbarianCamp,
    BanditStronghold,
    SacredGroveOrRuins,
    WildAnimal,
}

/// 巢穴難度等級。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum LairDifficultyTier {
    Unrated = 0,
    Tier1Scout = 1,
    Tier2Standard = 2,
    Tier3Major = 3,
    Tier4Boss = 4,
}

/// 巢穴常駐守衛。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LairGuardUnit {
    pub unit_alias: String,
    pub count: i32,
    pub patrol_radius_tiles: f32,
    pub aggro_radius_tiles: f32,
    pub respawn_interval_seconds: i32,
    pub angle_offset_deg: f32,
}

impl LairGuardUnit {
    pub fn new(unit_alias: impl Into<String>, count: i32) -> Self {
        Self {
            unit_alias: unit_alias.into(),
            count,
            patrol_radius_tiles: 4.0,
            aggro_radius_tiles: 8.0,
            respawn_interval_seconds: 0,
            angle_offset_deg: 0.0,
        }
    }
}

/// 定期部隊生成規則。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LairWaveSpawnRule {
    pub wave_id: String,
    pub unit_alias: String,
    pub spawn_count: i32,
    pub interval_seconds: i32,
    pub initial_delay_seconds: i32,
    pub max_active_waves: i32,
    pub spawn_radius_tiles: f32,
    pub aggro_behavior: String,
}

impl LairWaveSpawnRule {
    pub fn new(
        wave_id: impl Into<String>,
        unit_alias: impl Into<String>,
        spawn_count: i32,
        interval_seconds: i32,
    ) -> Self {
        Self {
            wave_id: wave_id.into(),
            unit_alias: unit_alias.into(),
            spawn_count,
            interval_seconds,
            initial_delay_seconds: interval_seconds,
            max_active_waves: 0,
            spawn_radius_tiles: 2.5,
            aggro_behavior: "None".to_string(),
        }
    }
}

/// 戰利品獎勵。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LairLootReward {
    pub wood: i32,
    pub food: i32,
    pub gold: i32,
    pub honor_points: i32,
    pub completion_message: String,
}

impl LairLootReward {
    pub fn is_empty(&self) -> bool {
        self.wood == 0
            && self.food == 0
            && self.gold == 0
            && self.honor_points == 0
            && self.completion_message.is_empty()
    }
}

/// 中立巢穴與野生動物定義。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeutralLairDefinition {
    pub id: String,
    pub display_name_zh: String,
    pub display_name_en: String,
    pub category: LairCategory,
    pub tier: LairDifficultyTier,
    pub native_building_or_landscape_type: String,
    pub footprint_radius_tiles: f32,
    pub default_guards: Vec<LairGuardUnit>,
    pub wave_rules: Vec<LairWaveSpawnRule>,
    pub loot: LairLootReward,
    pub biome_affinity: String,
    pub description: String,
    pub script_alias: Option<String>,
}

/// 已在地圖中放置的中立巢穴實例。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlacedNeutralLair {
    pub instance_id: Uuid,
    pub definition_id: String,
    pub world_x: f32,
    pub world_y: f32,
    pub world_z: f32,
    pub rotation_deg: f32,
    pub team: i32,
    pub core_structure_spawn_id: Option<Uuid>,
    pub guard_spawn_ids: Option<Vec<Uuid>>,
    pub bound_event_names: Option<Vec<String>>,
    pub is_active: bool,
}

impl PlacedNeutralLair {
    pub fn new(
        instance_id: Uuid,
        definition_id: impl Into<String>,
        world_x: f32,
        world_y: f32,
        world_z: f32,
    ) -> Self {
        Self {
            instance_id,
            definition_id: definition_id.into(),
            world_x,
            world_y,
            world_z,
            rotation_deg: 0.0,
            team: 8,
            core_structure_spawn_id: None,
            guard_spawn_ids: None,
            bound_event_names: None,
            is_active: true,
        }
    }
}

/// 原生資源類別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NativeResourceKind {
    Tree,
    Stone,
    Field,
    MineBuilding,
    GoldsmithBuilding,
}

/// 原生資源定義。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeResourceDefinition {
    pub name_def: String,
    pub kind: NativeResourceKind,
    pub definition_id: i32,
    pub script_alias: Option<String>,
}

impl NativeResourceDefinition {
    pub fn new(name_def: impl Into<String>, kind: NativeResourceKind, definition_id: i32) -> Self {
        Self {
            name_def: name_def.into(),
            kind,
            definition_id,
            script_alias: None,
        }
    }

    pub fn with_alias(
        name_def: impl Into<String>,
        kind: NativeResourceKind,
        definition_id: i32,
        alias: impl Into<String>,
    ) -> Self {
        Self {
            name_def: name_def.into(),
            kind,
            definition_id,
            script_alias: Some(alias.into()),
        }
    }
}

/// 定時資源替換請求。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimedResourceReplacement {
    pub name_def: String,
    pub world_x: f32,
    pub world_z: f32,
    pub delay_seconds: i32,
    pub team: i32,
}
