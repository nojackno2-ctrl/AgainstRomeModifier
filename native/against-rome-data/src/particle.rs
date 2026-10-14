//! Immutable saved particle.dat layout. Consumer names are qualified static observations,
//! not a runtime simulator or a sprite/frame/blend contract.
use crate::{lzss, pool::Source, sha256::sha256, Error};
use std::{
    fmt,
    path::{Path, PathBuf},
};

pub const CAPACITY: usize = 1024;
pub const CHILDREN: usize = 64;
pub const CHILD_LEN: usize = 35;
pub const RECORD_LEN: usize = 2252;
pub const STATE_LEN: usize = 16;
pub const PAYLOAD_LEN: usize = 12 + CAPACITY * (RECORD_LEN + STATE_LEN);
pub const BAD_CHILDREN: i32 = 113;
pub const BAD_RENDERABLE_COUNT: i32 = 114;
pub const IO_ERROR: i32 = 115;

#[derive(Debug)]
pub enum ParticleError {
    Data(Error),
    BadChildren(i32),
    RenderableCount { slot: u32, count: i32 },
    Io(std::io::Error),
}
impl ParticleError {
    pub fn code(&self) -> i32 {
        match self {
            Self::Data(e) => e.code(),
            Self::BadChildren(_) => BAD_CHILDREN,
            Self::RenderableCount { .. } => BAD_RENDERABLE_COUNT,
            Self::Io(_) => IO_ERROR,
        }
    }
}
impl From<Error> for ParticleError {
    fn from(e: Error) -> Self {
        Self::Data(e)
    }
}
impl fmt::Display for ParticleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Data(Error::PayloadLength { actual, expected }) => {
                write!(f, "particle payload length {actual}, expected {expected}")
            }
            Self::Data(Error::BadVersion(v)) => write!(f, "particle version {v}, expected 1"),
            Self::Data(Error::BadCapacity(v)) => write!(f, "particle capacity {v}, expected 1024"),
            Self::Data(e) => e.fmt(f),
            Self::BadChildren(v) => write!(f, "particle children {v}, expected 64"),
            Self::RenderableCount { slot, count } => write!(
                f,
                "particle slot {slot} renderable count {count} outside 0..=64"
            ),
            Self::Io(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for ParticleError {}

#[derive(Debug, Clone)]
pub struct ParticlePool {
    payload: Box<[u8]>,
    source: Source,
    input_sha256: [u8; 32],
    source_path: Option<PathBuf>,
}
fn i32_at(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn i16_at(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
impl ParticlePool {
    pub fn from_payload(bytes: &[u8]) -> Result<Self, ParticleError> {
        if bytes.len() != PAYLOAD_LEN {
            return Err(Error::PayloadLength {
                actual: bytes.len(),
                expected: PAYLOAD_LEN,
            }
            .into());
        }
        Self::build(bytes.to_vec(), Source::RawPayload, sha256(bytes))
    }
    pub fn from_file_bytes(bytes: &[u8]) -> Result<Self, ParticleError> {
        if lzss::has_pfil_magic(bytes) {
            let decoded = lzss::decompress_pfil(bytes, Some(PAYLOAD_LEN))?;
            let mut header = [0; 64];
            header.copy_from_slice(&bytes[..64]);
            Self::build(decoded, Source::Pfil { header }, sha256(bytes))
        } else {
            Self::from_payload(bytes)
        }
    }
    /// Read only. Path is the caller-supplied path; digest identifies the exact input file image.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ParticleError> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(ParticleError::Io)?;
        let mut pool = Self::from_file_bytes(&bytes)?;
        pool.source_path = Some(path.to_path_buf());
        Ok(pool)
    }
    fn build(
        payload: Vec<u8>,
        source: Source,
        input_sha256: [u8; 32],
    ) -> Result<Self, ParticleError> {
        if payload.len() != PAYLOAD_LEN {
            return Err(Error::PayloadLength {
                actual: payload.len(),
                expected: PAYLOAD_LEN,
            }
            .into());
        }
        if i32_at(&payload, 0) != 1 {
            return Err(Error::BadVersion(i32_at(&payload, 0)).into());
        }
        if i32_at(&payload, 4) != CAPACITY as i32 {
            return Err(Error::BadCapacity(i32_at(&payload, 4)).into());
        }
        if i32_at(&payload, 8) != CHILDREN as i32 {
            return Err(ParticleError::BadChildren(i32_at(&payload, 8)));
        }
        Ok(Self {
            payload: payload.into_boxed_slice(),
            source,
            input_sha256,
            source_path: None,
        })
    }
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
    pub fn source(&self) -> &Source {
        &self.source
    }
    pub fn input_sha256(&self) -> &[u8; 32] {
        &self.input_sha256
    }
    pub fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }
    pub fn snapshot_any(&self, slot: u32) -> Result<ParticleSnapshot, ParticleError> {
        if slot as usize >= CAPACITY {
            return Err(Error::InvalidSlot(slot).into());
        }
        let start = 12 + slot as usize * RECORD_LEN;
        let mut record = [0; RECORD_LEN];
        record.copy_from_slice(&self.payload[start..start + RECORD_LEN]);
        let mut state = [0; STATE_LEN];
        for c in 0..4 {
            let o = 12 + CAPACITY * RECORD_LEN + c * CAPACITY * 4 + slot as usize * 4;
            state[c * 4..c * 4 + 4].copy_from_slice(&self.payload[o..o + 4]);
        }
        Ok(ParticleSnapshot {
            slot,
            record,
            state,
        })
    }
    pub fn capture(&self, slot: u32) -> Option<ParticleSnapshot> {
        self.snapshot_any(slot).ok().filter(|s| s.active_raw() != 0)
    }
    pub fn active_slots(&self) -> impl Iterator<Item = u32> + '_ {
        (0..CAPACITY as u32).filter(|&s| i16_at(&self.payload, 12 + s as usize * RECORD_LEN) != 0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticleSnapshot {
    slot: u32,
    record: [u8; RECORD_LEN],
    state: [u8; STATE_LEN],
}
impl ParticleSnapshot {
    /// Detached caller buffers, copied without rewriting inactive children or state words.
    pub fn from_saved_buffers(
        slot: u32,
        record: &[u8],
        state: &[u8],
    ) -> Result<Self, ParticleError> {
        if slot as usize >= CAPACITY {
            return Err(Error::InvalidSlot(slot).into());
        }
        let record = record
            .try_into()
            .map_err(|_| Error::RecordsLength(record.len()))?;
        let state = state
            .try_into()
            .map_err(|_| Error::RecordsLength(state.len()))?;
        Ok(Self {
            slot,
            record,
            state,
        })
    }
    pub fn slot(&self) -> u32 {
        self.slot
    }
    /// Storage-compatible editor translation: every active child, including out-of-count
    /// and skip-first children. Original snapshot and opaque words remain untouched.
    pub fn translated_fixed(&self, delta: [i32; 3]) -> Result<Self, ParticleError> {
        let mut result = self.clone();
        if self.active_raw() == 0 {
            return Ok(result);
        }
        for ordinal in 0..CHILDREN as u32 {
            let child = self.child(ordinal)?;
            if child.active_raw() == 0 {
                continue;
            }
            for (axis, value) in child.position_fixed().into_iter().enumerate() {
                let translated = value.checked_add(delta[axis]).ok_or(Error::FixedOverflow {
                    ordinal,
                    axis: axis as u8,
                })?;
                let offset = 12 + ordinal as usize * CHILD_LEN + 1 + axis * 4;
                result.record[offset..offset + 4].copy_from_slice(&translated.to_le_bytes());
            }
        }
        Ok(result)
    }
    pub fn raw_record(&self) -> &[u8; RECORD_LEN] {
        &self.record
    }
    pub fn raw_state(&self) -> &[u8; STATE_LEN] {
        &self.state
    }
    pub fn state_words(&self) -> [i32; 4] {
        std::array::from_fn(|c| i32_at(&self.state, c * 4))
    }
    pub fn active_raw(&self) -> u16 {
        i16_at(&self.record, 0) as u16
    }
    pub fn count_raw(&self) -> i32 {
        i32_at(&self.record, 2)
    }
    pub fn owner(&self) -> i16 {
        i16_at(&self.record, 6)
    }
    pub fn age_raw(&self) -> i32 {
        i32_at(&self.record, 8)
    }
    /// Optional consumer diagnostic; does not affect parsing or bounded child access.
    pub fn validate_renderable_count(&self) -> Result<u32, ParticleError> {
        let count = self.count_raw();
        if !(0..=CHILDREN as i32).contains(&count) {
            return Err(ParticleError::RenderableCount {
                slot: self.slot,
                count,
            });
        }
        Ok(count as u32)
    }
    pub fn child(&self, ordinal: u32) -> Result<ParticleChild, ParticleError> {
        if ordinal as usize >= CHILDREN {
            return Err(Error::InvalidChild(ordinal).into());
        }
        let o = 12 + ordinal as usize * CHILD_LEN;
        let mut raw = [0; CHILD_LEN];
        raw.copy_from_slice(&self.record[o..o + CHILD_LEN]);
        Ok(ParticleChild { raw })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticleChild {
    raw: [u8; CHILD_LEN],
}
impl ParticleChild {
    pub fn raw_bytes(&self) -> &[u8; CHILD_LEN] {
        &self.raw
    }
    pub fn active_raw(&self) -> u8 {
        self.raw[0]
    }
    pub fn position_fixed(&self) -> [i32; 3] {
        std::array::from_fn(|a| i32_at(&self.raw, 1 + a * 4))
    }
    /// Static render/update consumers observe velocity at these saved offsets; no tick semantics.
    pub fn velocity_raw(&self) -> [i32; 3] {
        std::array::from_fn(|a| i32_at(&self.raw, 13 + a * 4))
    }
    /// Static lifetime/index consumer observation only; no frame resolution.
    pub fn lifetime_raw(&self) -> i32 {
        i32_at(&self.raw, 25)
    }
    pub fn opaque_shorts(&self) -> [u16; 2] {
        [i16_at(&self.raw, 29) as u16, i16_at(&self.raw, 31) as u16]
    }
    pub fn light_link(&self) -> i16 {
        i16_at(&self.raw, 33)
    }
}
