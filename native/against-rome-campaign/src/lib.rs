//! Against Rome Campaign, AI, Objectives and Cinematics modules ported from C#.
//!
//! Provides pure Rust implementations of:
//! - AI Archetypes, Tactical Profiles, Waypoints, Campaign Mission Compiler and Faction Sessions.
//! - Objective Dependency Graph, Rule Catalog, Sandbox Simulator and BCI Compiler.
//! - Map Start Camera, Spline Planner, Cutscene Sequences, Previewer, Injector and Cinematic BCI Compiler.

pub mod ai;
pub mod cinematics;
pub mod objectives;

// AI exports
pub use ai::{
    AiArchetypeCatalog, AiArchetypeProfile, CampaignCompilationResult, CampaignMissionCompiler,
    CampaignMissionPlan, CampaignObjective, CampaignObjectiveType, EngagementRule,
    FactionAiProfile, FactionAiSession, ReinforcementDefinition, TacticalPosture, TargetPriority,
    UnitPreferenceWeight, WaveAttackDefinition, WaveSquadDefinition, WaypointMovementMode,
    WaypointNode, WaypointPath, WaypointPathPlanner, WaypointStance,
};

// Cinematics exports
pub use cinematics::{
    CameraEasingType, CameraFlightPreviewer, CameraPose, CameraPreviewState,
    CameraTrackSplinePlanner, CameraWaypoint, CinematicBciCompiler, CutsceneFxEventKind,
    CutsceneFxKeyframe, CutsceneSequence, CutsceneSequenceCatalog, CutsceneUnitOrderKind,
    MapStartCamera, MapStartCameraInjector, StartCameraPrimitive, SubtitleKeyframe,
    UnitOrderKeyframe,
};

// Objectives exports
pub use objectives::{
    BciObjectiveCompilationResult, BciObjectiveCompiler, CampaignResult, DependencyRelation,
    ObjectiveAreaBounds, ObjectiveCategory, ObjectiveDefinition, ObjectiveDependency,
    ObjectiveDependencyGraph, ObjectiveDiagnostic, ObjectiveDiagnosticSeverity, ObjectiveKind,
    ObjectiveReward, ObjectiveRuleCatalog, ObjectiveRuleDescriptor, ObjectiveRuleParameters,
    ObjectiveSandboxSession, ObjectiveState, ObjectiveStateChangedEventArgs,
};
