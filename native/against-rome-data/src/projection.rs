//! Saved-light projection for editor family copy/move.
//!
//! Input is the *saved* pool state (no owner dynamic transform, no baseline substitution, no tick).
//! Each request names a slot, a host target GUID (opaque identity, never deduplicated) and a signed
//! 16.16 fixed-point XYZ delta. Per axis:
//!
//! * delta == 0: the saved coordinate bits are kept verbatim (including `-0.0` and NaN payloads);
//! * otherwise: `(saved as f64 + delta as f64 / 65536.0) as f32`, which must be finite.
//!
//! Every other byte of the 56-byte snapshot (RGB, both radii, active, type, age, owner/parent/definition…)
//! is preserved. The batch is all-or-nothing: the pool and its source are never modified.
//!
//! This is a direct float projection of the saved light position. It is NOT equivalent to a particle's
//! linked light, whose position must be derived from the active child's fixed position + delta and then
//! re-quantised (SAR16); do not route particle lights through this API.

use crate::error::Error;
use crate::pool::{LightPool, LightSnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightProjectionRequest {
    pub slot: u32,
    /// Host target identity; carried through untouched. (target GUID, slot) is not deduplicated.
    pub target_guid: [u8; 16],
    /// Signed 16.16 fixed-point delta for X, Y, Z.
    pub delta_fixed: [i32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectedLight {
    pub request_index: usize,
    pub target_guid: [u8; 16],
    pub snapshot: LightSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchError {
    pub index: usize,
    pub error: Error,
}

/// Projects one saved coordinate (delta 0 keeps the saved bits).
fn project_axis(saved_bits: u32, delta: i32, slot: u32, axis: u8) -> Result<u32, Error> {
    if delta == 0 {
        return Ok(saved_bits);
    }
    let saved = f32::from_bits(saved_bits);
    if !saved.is_finite() {
        return Err(Error::NonFinitePosition { slot, axis });
    }
    let result = (saved as f64 + delta as f64 / 65536.0) as f32;
    if !result.is_finite() {
        return Err(Error::ResultOverflow { slot, axis });
    }
    Ok(result.to_bits())
}

/// Validates the whole batch (range, active, finiteness, overflow) before returning any output.
pub fn project_batch(pool: &LightPool, requests: &[LightProjectionRequest]) -> Result<Vec<ProjectedLight>, BatchError> {
    let mut out = Vec::with_capacity(requests.len());
    for (index, req) in requests.iter().enumerate() {
        let wrap = |error| BatchError { index, error };
        let src = pool.snapshot_any(req.slot).map_err(wrap)?;
        if src.active_raw() == 0 {
            return Err(wrap(Error::InactiveSlot(req.slot)));
        }
        let saved = src.position_bits();
        let mut bits = [0u32; 3];
        for axis in 0..3 {
            bits[axis] = project_axis(saved[axis], req.delta_fixed[axis], req.slot, axis as u8).map_err(wrap)?;
        }
        out.push(ProjectedLight {
            request_index: index,
            target_guid: req.target_guid,
            snapshot: src.with_position_bits(bits),
        });
    }
    Ok(out)
}

// ---- Particle-linked light projection ----
//
// Mirrors `NativeParticlePool.TranslateBatchFixed` for the light side only: the caller supplies saved
// 2252-byte emitter records (never modified here) and, per request, one child ordinal plus the light
// slot that child must link to. Active child (u8 at child+0 != 0): fixed XYZ (i32 at +1/+5/+9) plus the
// signed 16.16 delta with `checked_add`, then arithmetic `>> 16`, then converted to f32 as the light
// position. Inactive child: the saved light XYZ bits are kept verbatim and the fixed values are
// reported unchanged. Every other light byte is preserved. Whole batch validated before any output.
// This establishes only the saved light position derivation, not particle sprite/frame/blend behaviour.

pub const EMITTER_RECORD_LEN: usize = 2252;
pub const CHILD_COUNT: u32 = 64;
const CHILD_BASE: usize = 12;
const CHILD_STRIDE: usize = 35;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticleLightRequest {
    /// Index into the contiguous emitter record buffer.
    pub record_index: u32,
    /// Child ordinal 0..63.
    pub child_ordinal: u32,
    /// Light slot the child's saved link must equal (the Rust pool light that is projected).
    pub light_slot: u32,
    /// Host target identity; carried through untouched, never deduplicated.
    pub target_guid: [u8; 16],
    pub delta_fixed: [i32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticleProjectedLight {
    pub request_index: usize,
    pub target_guid: [u8; 16],
    pub child_ordinal: u32,
    /// Raw child active byte.
    pub child_active_raw: u8,
    /// Active: fixed + delta. Inactive: the saved fixed values, unchanged.
    pub fixed_position: [i32; 3],
    pub snapshot: LightSnapshot,
}

pub fn project_particle_batch(
    pool: &LightPool,
    records: &[u8],
    requests: &[ParticleLightRequest],
) -> Result<Vec<ParticleProjectedLight>, BatchError> {
    if records.len() % EMITTER_RECORD_LEN != 0 {
        return Err(BatchError { index: usize::MAX, error: Error::RecordsLength(records.len()) });
    }
    let record_count = records.len() / EMITTER_RECORD_LEN;
    let mut out = Vec::with_capacity(requests.len());
    for (index, req) in requests.iter().enumerate() {
        let wrap = |error| BatchError { index, error };
        if req.record_index as usize >= record_count {
            return Err(wrap(Error::InvalidRecordIndex(req.record_index)));
        }
        let rec = &records[req.record_index as usize * EMITTER_RECORD_LEN..][..EMITTER_RECORD_LEN];
        if u16::from_le_bytes([rec[0], rec[1]]) == 0 {
            return Err(wrap(Error::InactiveEmitter(req.record_index)));
        }
        if req.child_ordinal >= CHILD_COUNT {
            return Err(wrap(Error::InvalidChild(req.child_ordinal)));
        }
        let child = &rec[CHILD_BASE + req.child_ordinal as usize * CHILD_STRIDE..][..CHILD_STRIDE];
        let link = i16::from_le_bytes([child[33], child[34]]);
        if req.light_slot as usize >= crate::pool::CAPACITY {
            return Err(wrap(Error::InvalidSlot(req.light_slot)));
        }
        if link as i32 != req.light_slot as i32 {
            return Err(wrap(Error::LightLinkMismatch {
                ordinal: req.child_ordinal,
                expected: req.light_slot,
                actual: link,
            }));
        }
        let src = pool.snapshot_any(req.light_slot).map_err(wrap)?;
        if src.active_raw() == 0 {
            return Err(wrap(Error::InactiveSlot(req.light_slot)));
        }
        let saved_fixed: [i32; 3] =
            std::array::from_fn(|a| i32::from_le_bytes(child[1 + a * 4..5 + a * 4].try_into().unwrap()));
        let (fixed, snapshot) = if child[0] == 0 {
            (saved_fixed, src)
        } else {
            let mut fixed = [0i32; 3];
            for a in 0..3 {
                fixed[a] = saved_fixed[a].checked_add(req.delta_fixed[a]).ok_or_else(|| {
                    wrap(Error::FixedOverflow { ordinal: req.child_ordinal, axis: a as u8 })
                })?;
            }
            // Arithmetic shift (floor), then exact i32 -> f32 (|v| <= 32768).
            let bits = fixed.map(|v| ((v >> 16) as f32).to_bits());
            (fixed, src.with_position_bits(bits))
        };
        out.push(ParticleProjectedLight {
            request_index: index,
            target_guid: req.target_guid,
            child_ordinal: req.child_ordinal,
            child_active_raw: child[0],
            fixed_position: fixed,
            snapshot,
        });
    }
    Ok(out)
}
