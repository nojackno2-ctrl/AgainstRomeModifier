use std::fmt;

/// Failure classes mirroring the C# exceptions of the shadow assets.
#[derive(Debug)]
pub enum ShadowError {
    /// `InvalidDataException`: malformed or truncated bytes.
    InvalidData(String),
    /// `NotSupportedException`: well-formed but unsupported variant.
    NotSupported(String),
    /// `ArgumentOutOfRangeException`: a query argument outside its documented range.
    OutOfRange(&'static str),
    /// Filesystem / archive failure while opening a game asset directory.
    Io(String),
}

impl ShadowError {
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidData(message.into())
    }

    pub(crate) fn unsupported(message: impl Into<String>) -> Self {
        Self::NotSupported(message.into())
    }
}

impl fmt::Display for ShadowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(m) => write!(f, "invalid shadow data: {m}"),
            Self::NotSupported(m) => write!(f, "unsupported shadow data: {m}"),
            Self::OutOfRange(name) => write!(f, "argument out of range: {name}"),
            Self::Io(m) => write!(f, "shadow asset I/O error: {m}"),
        }
    }
}

impl std::error::Error for ShadowError {}
