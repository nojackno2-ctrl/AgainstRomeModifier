//! Error type shared by the Rust API and the C ABI (see `code`).

use std::fmt;

/// Numeric codes returned by the C ABI. 0 is success; 1..=99 are ABI usage errors; 100.. are data errors.
pub mod code {
    pub const OK: i32 = 0;
    pub const NULL_ARGUMENT: i32 = 1;
    pub const BUFFER_TOO_SMALL: i32 = 2;
    pub const BAD_STRUCT_SIZE: i32 = 3;
    pub const INVALID_SLOT: i32 = 4;
    pub const INACTIVE_SLOT: i32 = 5;
    pub const PANIC: i32 = 6;
    pub const NONFINITE_POSITION: i32 = 7;
    pub const RESULT_OVERFLOW: i32 = 8;
    pub const INVALID_RECORD_INDEX: i32 = 9;
    pub const INACTIVE_EMITTER: i32 = 10;
    pub const INVALID_CHILD: i32 = 11;
    pub const LIGHT_LINK_MISMATCH: i32 = 12;
    pub const FIXED_OVERFLOW: i32 = 13;
    pub const BAD_RECORDS_LENGTH: i32 = 14;
    pub const PFIL_HEADER_TRUNCATED: i32 = 100;
    pub const PFIL_BAD_MAGIC: i32 = 101;
    pub const DECLARED_SIZE: i32 = 102;
    pub const LZSS_TRUNCATED: i32 = 103;
    pub const LZSS_OVERSHOOT: i32 = 104;
    pub const LZSS_TRAILING_INPUT: i32 = 105;
    pub const PAYLOAD_LENGTH: i32 = 110;
    pub const BAD_VERSION: i32 = 111;
    pub const BAD_CAPACITY: i32 = 112;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    PfilHeaderTruncated { len: usize },
    PfilBadMagic,
    DeclaredSize { declared: u32, expected: usize },
    LzssTruncated { produced: usize, expected: usize },
    LzssOvershoot { produced: usize, copy_len: usize, expected: usize },
    LzssTrailingInput { unused: usize },
    PayloadLength { actual: usize, expected: usize },
    BadVersion(i32),
    BadCapacity(i32),
    InvalidSlot(u32),
    InactiveSlot(u32),
    /// A changed axis (non-zero delta) started from a non-finite saved coordinate.
    NonFinitePosition { slot: u32, axis: u8 },
    /// saved + delta was not finite in float32.
    ResultOverflow { slot: u32, axis: u8 },
    /// Particle request named a record index outside the supplied emitter records.
    InvalidRecordIndex(u32),
    /// Emitter record active word (u16 at offset 0) is zero.
    InactiveEmitter(u32),
    /// Child ordinal is not in 0..64.
    InvalidChild(u32),
    /// The child's saved light link (i16 at child+33) is not the expected light slot.
    LightLinkMismatch { ordinal: u32, expected: u32, actual: i16 },
    /// Active child fixed coordinate + delta overflowed i32.
    FixedOverflow { ordinal: u32, axis: u8 },
    /// Emitter record buffer length is not a multiple of 2252.
    RecordsLength(usize),
}

impl Error {
    pub fn code(&self) -> i32 {
        match self {
            Error::PfilHeaderTruncated { .. } => code::PFIL_HEADER_TRUNCATED,
            Error::PfilBadMagic => code::PFIL_BAD_MAGIC,
            Error::DeclaredSize { .. } => code::DECLARED_SIZE,
            Error::LzssTruncated { .. } => code::LZSS_TRUNCATED,
            Error::LzssOvershoot { .. } => code::LZSS_OVERSHOOT,
            Error::LzssTrailingInput { .. } => code::LZSS_TRAILING_INPUT,
            Error::PayloadLength { .. } => code::PAYLOAD_LENGTH,
            Error::BadVersion(_) => code::BAD_VERSION,
            Error::BadCapacity(_) => code::BAD_CAPACITY,
            Error::InvalidSlot(_) => code::INVALID_SLOT,
            Error::InactiveSlot(_) => code::INACTIVE_SLOT,
            Error::NonFinitePosition { .. } => code::NONFINITE_POSITION,
            Error::ResultOverflow { .. } => code::RESULT_OVERFLOW,
            Error::InvalidRecordIndex(_) => code::INVALID_RECORD_INDEX,
            Error::InactiveEmitter(_) => code::INACTIVE_EMITTER,
            Error::InvalidChild(_) => code::INVALID_CHILD,
            Error::LightLinkMismatch { .. } => code::LIGHT_LINK_MISMATCH,
            Error::FixedOverflow { .. } => code::FIXED_OVERFLOW,
            Error::RecordsLength(_) => code::BAD_RECORDS_LENGTH,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::PfilHeaderTruncated { len } => write!(f, "PFIL header truncated: {len} bytes (< 64)"),
            Error::PfilBadMagic => write!(f, "missing PFIL magic"),
            Error::DeclaredSize { declared, expected } => {
                write!(f, "PFIL declared decoded size {declared} does not match expected/allowed {expected}")
            }
            Error::LzssTruncated { produced, expected } => {
                write!(f, "LZSS input ended after {produced} of {expected} decoded bytes")
            }
            Error::LzssOvershoot { produced, copy_len, expected } => write!(
                f,
                "LZSS back-reference of {copy_len} bytes at output {produced} exceeds decoded size {expected}"
            ),
            Error::LzssTrailingInput { unused } => {
                write!(f, "{unused} unused input bytes after the declared decoded size was reached")
            }
            Error::PayloadLength { actual, expected } => {
                write!(f, "decoded light pool is {actual} bytes, expected exactly {expected}")
            }
            Error::BadVersion(v) => write!(f, "light pool version {v}, expected 1"),
            Error::BadCapacity(c) => write!(f, "light pool capacity {c}, expected 1024"),
            Error::InvalidSlot(s) => write!(f, "slot {s} out of range"),
            Error::InactiveSlot(s) => write!(f, "slot {s} is inactive"),
            Error::NonFinitePosition { slot, axis } => write!(f, "slot {slot} axis {axis} saved coordinate is not finite"),
            Error::ResultOverflow { slot, axis } => write!(f, "slot {slot} axis {axis} projected coordinate overflows float32"),
            Error::InvalidRecordIndex(i) => write!(f, "emitter record index {i} out of range"),
            Error::InactiveEmitter(i) => write!(f, "emitter record {i} is inactive"),
            Error::InvalidChild(c) => write!(f, "child ordinal {c} out of range 0..64"),
            Error::LightLinkMismatch { ordinal, expected, actual } => {
                write!(f, "child {ordinal} light link {actual} != expected slot {expected}")
            }
            Error::FixedOverflow { ordinal, axis } => {
                write!(f, "child {ordinal} axis {axis} fixed position + delta overflows i32")
            }
            Error::RecordsLength(n) => write!(f, "emitter records length {n} is not a multiple of 2252"),
        }
    }
}

impl std::error::Error for Error {}
