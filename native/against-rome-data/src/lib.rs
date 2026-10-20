//! Read-only parser for Against Rome saved `light.dat` pools.
//!
//! Scope: decode (strict PFIL/LZSS), validate version/capacity/length, and expose immutable typed
//! snapshots of the *saved* fields. It does not simulate runtime ticks, derive +0x3C/+0x40, rebuild
//! from definitions, or write files. See README.md for verification limits.

pub mod alr;
pub mod alr_metadata;
pub mod error;
pub mod alr_scanlines;
pub mod alr_scanline_ffi;
pub mod apt;
pub mod apt_geometry;
pub mod ffi;
pub mod json;
pub mod lzss;
pub mod particle;
pub mod particle_definitions;
pub mod particle_definition_sources;
pub mod particle_definition_ffi;
pub mod particle_frames;
pub mod particle_ffi;
pub mod particle_pixel_ffi;
pub mod particle_pixels;
pub mod pool;
pub mod projection;
pub mod sha256;

pub use alr::{AlrDocument, AlrError, AlrFrame, AlrFrameInfo};
pub use apt::{
    palette_to_argb, AptAttachmentGroup, AptAttachments, AptDocument, AptError, AptFrameRange,
    AptIndexedImage, AptPoint, AptTileInfo,
};
pub use apt_geometry::{
    extract_light_points, get_building_world_light_positions, place, project, round_half_to_even,
    world_grid, world_grid_origin, AptLightPoint, AptVector3,
};
pub use error::Error;
pub use pool::{LightPool, LightSnapshot, Source, CAPACITY, PAYLOAD_LEN, SNAPSHOT_LEN};
pub use projection::{
    project_batch, project_particle_batch, BatchError, LightProjectionRequest,
    ParticleLightRequest, ParticleProjectedLight, ProjectedLight,
};
