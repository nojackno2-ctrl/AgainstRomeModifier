//! Platform independent editor state. Height values are the game's green-channel
//! samples; collision is an independent, designer-authored navigation layer.
pub mod emboss;
pub mod terrain;
pub mod texture_history;

pub use emboss::*;
