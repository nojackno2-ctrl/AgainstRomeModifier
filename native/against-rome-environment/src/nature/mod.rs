//! Nature ecology, species profiles, flora scatter, and edit session.

pub mod biome;
pub mod engine;
pub mod interaction;
pub mod palette;
pub mod scatter;
pub mod session;
pub mod stroke;

pub use biome::{
    BiomeEcologyProfile, BiomeSpeciesEntry, BiomeType, ResolvedEcologyRoster, ResolvedSpecies,
    VegetationLayer,
};
pub use engine::{
    FloraScatterContext, FloraScatterEngine, FloraScatterParameters, FloraScatterReport,
    FloraScatterResult, NatureAddition, SpatialClearanceGrid,
};
pub use interaction::{
    addition_index_from_object_index, apply_blank_terrain_nature_clearance,
    build_nature_display_objects, delete_picked_nature, is_addition_index,
    is_existing_slot_index, is_level_data_object, nature_deleted_status_message,
    nature_selected_status_message, object_index_for_addition, object_index_for_slot,
    resolve_picked_nature, slot_from_object_index, NatureBlankClearanceReport,
    NatureDisplayObject, NaturePickResult, NaturePickTarget, ADDITION_OBJECT_INDEX_BASE,
    LEVEL_OBJECT_INDEX_BASE, NATIVE_DATA_OBJECTS_SOURCE,
};
pub use palette::{
    map_nature_regions, nature_category, nature_region, NatureBrushPainter, NatureBrushRemover,
    NatureCatalogItem, NatureCategory, NatureOperation, NaturePaletteFilter, NatureSpeciesMixer,
    PlacedNatureObject,
};
pub use scatter::{NatureDensity, NatureScatter};
pub use session::{NatureEditSession, NatureSnapshot};
pub use stroke::{
    stroke_tiles_between, NatureHeightField, NatureStrokeDriver, NatureStrokeReport,
    NatureStrokeSettings,
};
