//! Native Rust terrain texture storage, format parsing, and blend editing library for Against Rome.

pub mod authoring_map;
pub mod catalog;
pub mod document;
pub mod error;
pub mod floor_atlas;
pub mod floortex;
pub mod format;
pub mod lzss;
pub mod palette;
pub mod region;
pub mod resolver;
pub mod road_path;
pub mod history;
pub mod session;
pub mod sprite;
pub mod terrain_layer;

pub use authoring_map::{
    NativeTerrainBakeIssue, NativeTerrainBakeResult, NativeTerrainCornerConflict,
    NativeTerrainImportResult, TerrainBlendAuthoringMap,
};
pub use catalog::{
    NativeAnimationVisualState, NativeSpriteCatalog, NativeSpriteDefinition,
};
pub use document::{BodenTexturesDocument, TextureEntry};
pub use error::TextureError;
pub use floor_atlas::{AtlasPoint, AtlasRect, FloorTextureAtlas};
pub use floortex::FloortexLibrary;
pub use format::TextureResourceKind;
pub use region::TerrainRegionPlanner;
pub use resolver::{
    corner_key, pick_stable, NativeTerrainMaterialResolver, SimpleRuleResolver,
    StandardFloorMaterialCatalog,
};
pub use road_path::{
    build_available_road_tiles, orthogonal_between, plan_road_stroke, stroke_path_between,
    RoadConnections, RoadStrokePlan, RoadTile, RoadTilePlacement, TerrainRoadPaintResult,
};
pub use session::{
    TerrainBlendEditSession, TerrainBlendPaintResult, TerrainBlendStroke, TerrainCornerChange,
    TerrainTextureChange, MAX_BRIDGE_RINGS,
};
pub use sprite::{
    apt_finished_frame, decode_animation_frames, direction_for_angle, BoundedSpriteCatalog,
    NativeSprite, NativeSpriteAnimation, NativeSpriteAtlas, NativeSpriteUv, DEFAULT_DIRECTION,
};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
