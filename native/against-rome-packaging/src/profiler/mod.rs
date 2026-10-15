pub mod limits;
pub mod profiler;
pub mod report;

pub use limits::MapBudgetLimits;
pub use profiler::{MapBudgetProfiler, MapBudgetSnapshot, MapSceneObject, NativeLightInstance};
pub use report::{
    ChokepointLocation, DrawCallBudget, LightClusterHotspot, LightDensityBudget, MapBudgetReport,
    MapHealthBottleneck, MapHealthGrade, ObjectSlotBudget, PathfindingBudget,
};
