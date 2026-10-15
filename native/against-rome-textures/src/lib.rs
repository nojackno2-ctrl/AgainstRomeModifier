//! Native Rust terrain texture storage, format parsing, and blend editing library for Against Rome.

pub mod authoring_map;
pub mod document;
pub mod error;
pub mod floortex;
pub mod format;
pub mod lzss;
pub mod region;
pub mod resolver;
pub mod session;

pub use authoring_map::{
    NativeTerrainBakeIssue, NativeTerrainBakeResult, NativeTerrainCornerConflict,
    NativeTerrainImportResult, TerrainBlendAuthoringMap,
};
pub use document::{BodenTexturesDocument, TextureEntry};
pub use error::TextureError;
pub use floortex::FloortexLibrary;
pub use format::TextureResourceKind;
pub use region::TerrainRegionPlanner;
pub use resolver::{
    corner_key, pick_stable, NativeTerrainMaterialResolver, SimpleRuleResolver,
    StandardFloorMaterialCatalog,
};
pub use session::{
    TerrainBlendEditSession, TerrainBlendPaintResult, TerrainBlendStroke, TerrainCornerChange,
    TerrainTextureChange, MAX_BRIDGE_RINGS,
};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
