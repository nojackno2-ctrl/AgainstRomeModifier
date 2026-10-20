use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeError {
    InvalidData(String),
    NotSupported(String),
    OutOfRange(String),
    Io(String),
    Zip(String),
    Apt(String),
    Base64(String),
    Json(String),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(msg) => write!(f, "Invalid data: {msg}"),
            Self::NotSupported(msg) => write!(f, "Not supported: {msg}"),
            Self::OutOfRange(msg) => write!(f, "Out of range: {msg}"),
            Self::Io(msg) => write!(f, "I/O error: {msg}"),
            Self::Zip(msg) => write!(f, "ZIP error: {msg}"),
            Self::Apt(msg) => write!(f, "APT error: {msg}"),
            Self::Base64(msg) => write!(f, "Base64 error: {msg}"),
            Self::Json(msg) => write!(f, "JSON error: {msg}"),
        }
    }
}

impl std::error::Error for ProbeError {}

impl From<std::io::Error> for ProbeError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<zip::result::ZipError> for ProbeError {
    fn from(err: zip::result::ZipError) -> Self {
        Self::Zip(err.to_string())
    }
}

impl From<serde_json::Error> for ProbeError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err.to_string())
    }
}

impl From<against_rome_data::apt::AptError> for ProbeError {
    fn from(err: against_rome_data::apt::AptError) -> Self {
        Self::Apt(err.to_string())
    }
}

impl From<against_rome_environment::shadow::ShadowError> for ProbeError {
    fn from(err: against_rome_environment::shadow::ShadowError) -> Self {
        use against_rome_environment::shadow::ShadowError;
        match err {
            ShadowError::InvalidData(msg) => Self::InvalidData(msg),
            ShadowError::NotSupported(msg) => Self::NotSupported(msg),
            ShadowError::OutOfRange(msg) => Self::OutOfRange(msg.to_string()),
            ShadowError::Io(msg) => Self::Io(msg),
        }
    }
}
