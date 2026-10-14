//! Version-1, 1024-slot, 16-parallel-column `light.dat` pool and immutable per-slot snapshots.
//!
//! Layout mirrors `NativeLightPool` (C#) and docs/reverse-engineering/saved-light-render-contract.md.
//! Nothing here rebuilds fields from definitions, clamps, or interprets unknown state; the decoded
//! payload is retained byte-for-byte.

use crate::error::Error;
use crate::lzss;

pub const VERSION: i32 = 1;
pub const CAPACITY: usize = 1024;
pub const SNAPSHOT_LEN: usize = 56;
pub const PAYLOAD_LEN: usize = 8 + CAPACITY * SNAPSHOT_LEN; // 57352

/// Column widths in bytes. Snapshot field offsets are their running sums.
pub const WIDTHS: [usize; 16] = [4, 4, 4, 4, 4, 4, 4, 4, 4, 2, 4, 4, 4, 2, 2, 2];

const fn snapshot_offsets() -> [usize; 16] {
    let mut o = [0usize; 16];
    let mut i = 1;
    while i < 16 {
        o[i] = o[i - 1] + WIDTHS[i - 1];
        i += 1;
    }
    o
}
/// 0,4,8,12,16,20,24,28,32,36,38,42,46,50,52,54
pub const SNAPSHOT_OFFSETS: [usize; 16] = snapshot_offsets();

const fn column_offsets() -> [usize; 16] {
    let mut o = [0usize; 16];
    let mut i = 0;
    let mut acc = 8usize;
    while i < 16 {
        o[i] = acc;
        acc += CAPACITY * WIDTHS[i];
        i += 1;
    }
    o
}
const COLUMN_OFFSETS: [usize; 16] = column_offsets();

/// Where the pool bytes came from; kept so a host can tell wrapped from raw input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    RawPayload,
    Pfil { header: [u8; 64] },
}

/// Immutable decoded pool. The payload is private and only exposed as `&[u8]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightPool {
    payload: Box<[u8]>,
    source: Source,
}

impl LightPool {
    /// Validates and copies a decoded 57352-byte payload.
    pub fn from_payload(payload: &[u8]) -> Result<Self, Error> {
        Self::build(payload.to_vec(), Source::RawPayload)
    }

    /// Accepts a file image: PFIL-wrapped (strict LZSS, decoded length must be 57352) or a raw payload.
    pub fn from_file_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if lzss::has_pfil_magic(bytes) {
            let decoded = lzss::decompress_pfil(bytes, Some(PAYLOAD_LEN))?;
            let mut header = [0u8; 64];
            header.copy_from_slice(&bytes[..64]); // decompress_pfil guarantees >= 64 bytes
            Self::build(decoded, Source::Pfil { header })
        } else {
            Self::from_payload(bytes)
        }
    }

    fn build(payload: Vec<u8>, source: Source) -> Result<Self, Error> {
        if payload.len() != PAYLOAD_LEN {
            return Err(Error::PayloadLength { actual: payload.len(), expected: PAYLOAD_LEN });
        }
        let version = i32::from_le_bytes(payload[0..4].try_into().unwrap());
        let capacity = i32::from_le_bytes(payload[4..8].try_into().unwrap());
        if version != VERSION {
            return Err(Error::BadVersion(version));
        }
        if capacity != CAPACITY as i32 {
            return Err(Error::BadCapacity(capacity));
        }
        Ok(Self { payload: payload.into_boxed_slice(), source })
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
    pub fn source(&self) -> &Source {
        &self.source
    }

    fn field(&self, column: usize, slot: usize) -> &[u8] {
        let start = COLUMN_OFFSETS[column] + slot * WIDTHS[column];
        &self.payload[start..start + WIDTHS[column]]
    }

    /// Raw `active` u32 of a slot; `None` when out of range.
    pub fn active_raw(&self, slot: u32) -> Option<u32> {
        ((slot as usize) < CAPACITY)
            .then(|| u32::from_le_bytes(self.field(0, slot as usize).try_into().unwrap()))
    }

    /// Snapshot of any in-range slot, active or not (preserves whatever bytes the slot holds).
    pub fn snapshot_any(&self, slot: u32) -> Result<LightSnapshot, Error> {
        if slot as usize >= CAPACITY {
            return Err(Error::InvalidSlot(slot));
        }
        let mut fields = [0u8; SNAPSHOT_LEN];
        for c in 0..16 {
            let o = SNAPSHOT_OFFSETS[c];
            fields[o..o + WIDTHS[c]].copy_from_slice(self.field(c, slot as usize));
        }
        Ok(LightSnapshot { slot, fields })
    }

    /// Matches C# `Capture`: `None` for inactive (active == 0) or out-of-range slots.
    pub fn capture(&self, slot: u32) -> Option<LightSnapshot> {
        match self.active_raw(slot) {
            Some(a) if a != 0 => self.snapshot_any(slot).ok(),
            _ => None,
        }
    }

    pub fn active_slots(&self) -> impl Iterator<Item = u32> + '_ {
        (0..CAPACITY as u32).filter(|&s| self.active_raw(s).is_some_and(|a| a != 0))
    }

    pub fn active_snapshots(&self) -> impl Iterator<Item = LightSnapshot> + '_ {
        self.active_slots().filter_map(|s| self.capture(s))
    }
}

/// Detached, immutable 56-byte concatenation of one slot's 16 columns (not a runtime memcpy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightSnapshot {
    slot: u32,
    fields: [u8; SNAPSHOT_LEN],
}

impl LightSnapshot {
    fn u32_at(&self, o: usize) -> u32 {
        u32::from_le_bytes(self.fields[o..o + 4].try_into().unwrap())
    }
    fn i16_at(&self, o: usize) -> i16 {
        i16::from_le_bytes(self.fields[o..o + 2].try_into().unwrap())
    }
    fn f32_at(&self, o: usize) -> f32 {
        f32::from_bits(self.u32_at(o))
    }

    pub fn slot(&self) -> u32 {
        self.slot
    }
    /// All 56 saved bytes, unmodified.
    pub fn raw_fields(&self) -> &[u8; SNAPSHOT_LEN] {
        &self.fields
    }
    /// Column 0 (offset 0): any non-zero value is active; the exact value is preserved.
    pub fn active_raw(&self) -> u32 {
        self.u32_at(0)
    }
    /// Columns 1..3 (offsets 4/8/12): native X, Y, Z as saved.
    pub fn position(&self) -> [f32; 3] {
        [self.f32_at(4), self.f32_at(8), self.f32_at(12)]
    }
    pub fn position_bits(&self) -> [u32; 3] {
        [self.u32_at(4), self.u32_at(8), self.u32_at(12)]
    }
    /// Columns 4..6 (offsets 16/20/24): render R, G, B as saved (not clamped).
    pub fn color(&self) -> [f32; 3] {
        [self.f32_at(16), self.f32_at(20), self.f32_at(24)]
    }
    pub fn color_bits(&self) -> [u32; 3] {
        [self.u32_at(16), self.u32_at(20), self.u32_at(24)]
    }
    /// Column 7 (offset 28): saved effective radius (runtime +0x1C).
    pub fn effective_radius(&self) -> f32 {
        self.f32_at(28)
    }
    pub fn effective_radius_bits(&self) -> u32 {
        self.u32_at(28)
    }
    /// Column 8 (offset 32): baseline radius (runtime +0x20); may differ from effective.
    pub fn baseline_radius(&self) -> f32 {
        self.f32_at(32)
    }
    pub fn baseline_radius_bits(&self) -> u32 {
        self.u32_at(32)
    }
    /// Column 9 (offset 36): update type, unsigned 16-bit.
    pub fn type_raw(&self) -> u16 {
        u16::from_le_bytes([self.fields[36], self.fields[37]])
    }
    /// Column 10 (offset 38): type parameter.
    pub fn type_parameter(&self) -> f32 {
        self.f32_at(38)
    }
    pub fn type_parameter_bits(&self) -> u32 {
        self.u32_at(38)
    }
    /// Column 11 (offset 42): accumulated age as raw u32.
    pub fn age_raw(&self) -> u32 {
        self.u32_at(42)
    }
    /// Same four bytes read as signed (type 2 compares it signed).
    pub fn age_signed(&self) -> i32 {
        self.u32_at(42) as i32
    }
    /// Column 12 (offset 46): special-fx raw u32 (consumer not traced).
    pub fn special_fx_raw(&self) -> u32 {
        self.u32_at(46)
    }
    /// Column 13 (offset 50): owner link, signed.
    pub fn owner(&self) -> i16 {
        self.i16_at(50)
    }
    /// Column 14 (offset 52): parent link, signed.
    pub fn parent(&self) -> i16 {
        self.i16_at(52)
    }
    /// Column 15 (offset 54): definition index, signed.
    pub fn definition(&self) -> i16 {
        self.i16_at(54)
    }
}

impl LightSnapshot {
    /// Copy with only the three XYZ words (offsets 4/8/12) replaced; all other 44+ bytes untouched.
    pub(crate) fn with_position_bits(&self, bits: [u32; 3]) -> LightSnapshot {
        let mut copy = *self;
        for (i, b) in bits.iter().enumerate() {
            copy.fields[4 + i * 4..8 + i * 4].copy_from_slice(&b.to_le_bytes());
        }
        copy
    }
}
