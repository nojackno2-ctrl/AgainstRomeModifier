//! Settlement generation, site evaluation, tribal presets, and multiplayer fairness.

pub mod balancer;
pub mod cluster_planner;
pub mod engine;
pub mod evaluator;
pub mod layout_preset;
pub mod models;
pub mod presets;

pub use balancer::MultiplayerFairnessBalancer;
pub use cluster_planner::ResourceClusterPlanner;
pub use engine::SettlementGeneratorEngine;
pub use evaluator::SettlementSiteEvaluator;
pub use layout_preset::{MapLayoutEntry, MapLayoutKind, MapLayoutPreset, MapLayoutPresets};
pub use models::{
    BuildingBlueprint, FairnessScoreReport, MultiplayerDistributionResult, NatureClusterItem,
    PlacedBuildingPlan, PlacedResourceItem, PlayerSettlementLayout, SettlementSiteEvaluationResult,
    SettlementTribe, SymmetryMode,
};
pub use presets::SettlementTribalPresets;
