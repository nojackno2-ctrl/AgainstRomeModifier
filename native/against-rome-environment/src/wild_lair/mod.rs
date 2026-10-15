//! Wild lair, neutral creatures, resource regeneration, and timer scenario binding.

pub mod catalog;
pub mod event_binder;
pub mod models;
pub mod planner;

pub use catalog::NeutralLairCatalog;
pub use event_binder::{
    ScenarioAction, ScenarioActionKind, ScenarioCondition, ScenarioConditionKind, ScenarioEvent,
    WildLairScenarioEventBinder, LAIR_EVENT_PREFIX,
};
pub use models::{
    LairCategory, LairDifficultyTier, LairGuardUnit, LairLootReward, LairWaveSpawnRule,
    NativeResourceDefinition, NativeResourceKind, NeutralLairDefinition, PlacedNeutralLair,
    TimedResourceReplacement,
};
pub use planner::ResourceRegenerationPlanner;
