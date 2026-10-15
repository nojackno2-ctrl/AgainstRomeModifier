//! Error types for against-rome-textures.

use std::fmt;

#[derive(Debug)]
pub enum TextureError {
    Io(std::io::Error),
    Encoding(String),
    InvalidFormat(String),
    DimensionMismatch { expected: usize, actual: usize },
    OutOfRange { x: usize, y: usize, dimension: usize },
    EmptyMaterial,
    UnsupportedJunction(String),
    InvalidRadius,
}

impl fmt::Display for TextureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::Encoding(s) => write!(f, "Encoding error: {}", s),
            Self::InvalidFormat(s) => write!(f, "Invalid format: {}", s),
            Self::DimensionMismatch { expected, actual } => {
                write!(f, "Dimension mismatch: expected {}, got {}", expected, actual)
            }
            Self::OutOfRange { x, y, dimension } => {
                write!(f, "Coordinates ({}, {}) out of range for dimension {}", x, y, dimension)
            }
            Self::EmptyMaterial => write!(f, "Material cannot be empty"),
            Self::UnsupportedJunction(s) => write!(f, "Unsupported terrain junction: {}", s),
            Self::InvalidRadius => write!(f, "Radius must be finite and non-negative"),
        }
    }
}

impl std::error::Error for TextureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for TextureError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
