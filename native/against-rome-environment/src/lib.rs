//! `against-rome-environment`: Pure Rust environment, ecology noise, flora scatter,
//! wild lair, and settlement fairness engine for Against Rome.

pub mod nature;
pub mod noise;
pub mod random;
pub mod settlement;
pub mod wild_lair;

pub use nature::{
    BiomeEcologyProfile, BiomeSpeciesEntry, BiomeType, FloraScatterContext, FloraScatterEngine,
    FloraScatterParameters, FloraScatterReport, FloraScatterResult, NatureAddition,
    NatureDensity, NatureEditSession, NatureScatter, NatureSnapshot, ResolvedEcologyRoster,
    ResolvedSpecies, SpatialClearanceGrid, VegetationLayer,
};
pub use noise::{octave_noise_2d, perlin_2d};
pub use random::DotNetRandom;
pub use settlement::{
    BuildingBlueprint, FairnessScoreReport, MapLayoutEntry, MapLayoutKind, MapLayoutPreset,
    MapLayoutPresets, MultiplayerDistributionResult, MultiplayerFairnessBalancer,
    NatureClusterItem, PlacedBuildingPlan, PlacedResourceItem, PlayerSettlementLayout,
    ResourceClusterPlanner, SettlementGeneratorEngine, SettlementSiteEvaluationResult,
    SettlementSiteEvaluator, SettlementTribalPresets, SettlementTribe, SymmetryMode,
};
pub use wild_lair::{
    LairCategory, LairDifficultyTier, LairGuardUnit, LairLootReward, LairWaveSpawnRule,
    NativeResourceDefinition, NativeResourceKind, NeutralLairCatalog, NeutralLairDefinition,
    PlacedNeutralLair, ResourceRegenerationPlanner, ScenarioAction, ScenarioActionKind,
    ScenarioCondition, ScenarioConditionKind, ScenarioEvent, TimedResourceReplacement,
    WildLairScenarioEventBinder, LAIR_EVENT_PREFIX,
};
