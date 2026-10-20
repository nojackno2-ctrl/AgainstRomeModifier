//! Object shadow projection data and terrain shadow-map occlusion queries.
//!
//! Port of the C# `NativeShadowCatalog` / `NativeShadowDocument` / `MapLightingShadowMap`
//! (src.MapEditor.Modules/NativeAssets). Pure data and math: no GPU draw calls.

mod catalog;
mod document;
mod error;
mod map;

pub use catalog::{
    parse_objdef_shadows, parse_shadow_names, ObjectShadow, ShadowCatalog, ShadowDefinition,
    ShadowType, ShadowWorldQuad, TextureReader,
};
pub use document::{darken_argb, ShadowDocument, ShadowFrame};
pub use error::ShadowError;
pub use map::{LightingShadowMap, DECODED_LENGTH, DIMENSION, SLICE_COUNT};
