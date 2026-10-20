//! `against-rome-environment`: Pure Rust environment, ecology noise, flora scatter,
//! wild lair, and settlement fairness engine for Against Rome.

pub mod nature;
pub mod noise;
pub mod random;
pub mod settlement;
pub mod shadow;
pub mod wild_lair;

pub use nature::{
    addition_index_from_object_index, apply_blank_terrain_nature_clearance,
    build_nature_display_objects, delete_picked_nature, is_addition_index,
    is_existing_slot_index, is_level_data_object, map_nature_regions,
    nature_category, nature_deleted_status_message, nature_region,
    nature_selected_status_message, object_index_for_addition, object_index_for_slot,
    resolve_picked_nature, slot_from_object_index, BiomeEcologyProfile,
    BiomeSpeciesEntry, BiomeType, FloraScatterContext, FloraScatterEngine,
    FloraScatterParameters, FloraScatterReport, FloraScatterResult, NatureAddition,
    NatureBlankClearanceReport, NatureBrushPainter, NatureBrushRemover,
    NatureCatalogItem, NatureCategory, NatureDensity, NatureDisplayObject,
    NatureEditSession, NatureOperation, NaturePaletteFilter, NaturePickResult,
    NaturePickTarget, NatureScatter, NatureSnapshot, NatureSpeciesMixer,
    PlacedNatureObject, ResolvedEcologyRoster, ResolvedSpecies, SpatialClearanceGrid,
    VegetationLayer, ADDITION_OBJECT_INDEX_BASE, LEVEL_OBJECT_INDEX_BASE,
    NATIVE_DATA_OBJECTS_SOURCE,
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
pub use shadow::{
    darken_argb, parse_objdef_shadows, parse_shadow_names, LightingShadowMap, ObjectShadow,
    ShadowCatalog, ShadowDefinition, ShadowDocument, ShadowError, ShadowFrame, ShadowType,
    ShadowWorldQuad, TextureReader,
};
pub use wild_lair::{
    LairCategory, LairDifficultyTier, LairGuardUnit, LairLootReward, LairWaveSpawnRule,
    NativeResourceDefinition, NativeResourceKind, NeutralLairCatalog, NeutralLairDefinition,
    PlacedNeutralLair, ResourceRegenerationPlanner, ScenarioAction, ScenarioActionKind,
    ScenarioCondition, ScenarioConditionKind, ScenarioEvent, TimedResourceReplacement,
    WildLairScenarioEventBinder, LAIR_EVENT_PREFIX,
};
