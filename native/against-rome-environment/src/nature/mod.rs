//! Nature ecology, species profiles, flora scatter, and edit session.

pub mod biome;
pub mod engine;
pub mod scatter;
pub mod session;

pub use biome::{
    BiomeEcologyProfile, BiomeSpeciesEntry, BiomeType, ResolvedEcologyRoster, ResolvedSpecies,
    VegetationLayer,
};
pub use engine::{
    FloraScatterContext, FloraScatterEngine, FloraScatterParameters, FloraScatterReport,
    FloraScatterResult, NatureAddition, SpatialClearanceGrid,
};
pub use scatter::{NatureDensity, NatureScatter};
pub use session::{NatureEditSession, NatureSnapshot};
