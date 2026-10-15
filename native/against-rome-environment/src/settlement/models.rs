//! Data models for settlements, player layouts, blueprints, and fairness reports.

use serde::{Deserialize, Serialize};

/// 部族風格代碼。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SettlementTribe {
    Germanic,
    Roman,
    Celtic,
    Hun,
}

/// 多勢力分佈對稱模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymmetryMode {
    CentralSymmetry,
    RotationalSymmetry,
    TopologicalEquidistant,
}

/// 建築類型規格與佔地大小。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildingBlueprint {
    pub type_name: String,
    pub category: String,
    pub footprint_width: f32,
    pub footprint_height: f32,
    pub preferred_distance_to_main: f32,
    pub preferred_angle_deg: f32,
}

impl BuildingBlueprint {
    pub fn new(
        type_name: impl Into<String>,
        category: impl Into<String>,
        footprint_width: f32,
        footprint_height: f32,
        preferred_distance_to_main: f32,
        preferred_angle_deg: f32,
    ) -> Self {
        Self {
            type_name: type_name.into(),
            category: category.into(),
            footprint_width,
            footprint_height,
            preferred_distance_to_main,
            preferred_angle_deg,
        }
    }
}

/// 單棟規劃完成之建築物擺放資訊。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlacedBuildingPlan {
    pub type_name: String,
    pub world_x: f32,
    pub world_z: f32,
    pub height_offset: f32,
    pub angle: f32,
    pub team: i32,
    pub footprint_radius: f32,
}

/// 單個自然資源物件規劃（樹木、岩石露頭）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NatureClusterItem {
    pub type_name: String,
    pub world_x: f32,
    pub world_z: f32,
    pub height_offset: f32,
    pub angle_deg: f32,
}

/// 單個放置型資源/單位規劃（野生動物等）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlacedResourceItem {
    pub type_name: String,
    pub world_x: f32,
    pub world_z: f32,
    pub height_offset: f32,
    pub angle_deg: f32,
    pub team: i32,
    pub count: i32,
}

/// 聚落腹地評估指標結果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettlementSiteEvaluationResult {
    pub is_valid: bool,
    pub total_score: f32,
    pub flatness_score: f32,
    pub water_safety_score: f32,
    pub space_clearance_score: f32,
    pub expansion_potential_score: f32,
    pub anchor_tile_x: f32,
    pub anchor_tile_z: f32,
    pub world_x: f32,
    pub world_z: f32,
    pub planned_buildings: Vec<PlacedBuildingPlan>,
    pub rejection_reason: Option<String>,
}

/// 單一勢力聚落與週邊資源規劃總集。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerSettlementLayout {
    pub team_index: i32,
    pub tribe: SettlementTribe,
    pub site_evaluation: SettlementSiteEvaluationResult,
    pub forest_trees: Vec<NatureClusterItem>,
    pub stone_quarries: Vec<NatureClusterItem>,
    pub wildlife_and_food: Vec<PlacedResourceItem>,
    pub buildings: Vec<PlacedBuildingPlan>,
}

/// 公平性指標評估報告。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FairnessScoreReport {
    pub average_inter_player_distance: f32,
    pub min_inter_player_distance: f32,
    pub max_inter_player_distance: f32,
    pub wood_resource_variance: f32,
    pub stone_resource_variance: f32,
    pub food_resource_variance: f32,
    pub is_balanced: bool,
}

/// 多勢力生成全域方案與公平性度量。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiplayerDistributionResult {
    pub mode: SymmetryMode,
    pub player_count: usize,
    pub players: Vec<PlayerSettlementLayout>,
    pub fairness_report: FairnessScoreReport,
}
