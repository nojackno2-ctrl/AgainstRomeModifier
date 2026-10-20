//! against-rome-ai-planner: Pure Rust AI map planning, editor operator and placement layouts.

pub mod brush;
pub mod formation;
pub mod layout_resolver;
pub mod multi_planner;
pub mod ollama;
pub mod operator;
pub mod plan;
pub mod preview;
pub mod scope;

pub use brush::{
    BrushCommand, EditorBrushExecutor, EditorBrushResult, EditorBrushStroke, EditorBrushTool, EditorHeightBrush,
    TerrainStrokePath,
};
pub use formation::{FormationLayout, FormationLayoutDefinition, FormationLayoutSegment};
pub use layout_resolver::{LayoutTypeResolver, SdlObjectType};
pub use multi_planner::{
    AiMapDesignRole, AiMapRoleProgress, AiMapRoleResult, CancelToken, AiMapRoleStatus, MultiAiMapPlanResult,
    MultiAiMapPlanner, MultiAiMapRoleRequest,
};
pub use ollama::{
    AiMaterialOption, HttpResponse, HttpTransport, MockHttpTransport, OllamaMapPlanner,
    ConfiguredTcpHttpTransport, TcpHttpTransport, TcpTransportOptions,
};
pub use operator::AiEditorOperator;
pub use plan::{
    AiMapApplyResult, AiMapFeature, AiMapPlan, FEATURE_TYPES, LOCATION_NAMES, MAX_FEATURES,
};
pub use preview::{AiMapPlanPreview, AiMapPlanPreviewBuilder, RgbaImage};
pub use scope::{AiMapEditScope, Rectangle};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
