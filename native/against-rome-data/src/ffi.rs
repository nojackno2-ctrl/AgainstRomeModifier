//! C ABI for host glue (C# P/Invoke). Ownership rules:
//!
//! * `ar_light_pool_open` allocates an opaque handle; the host must release it exactly once with
//!   `ar_light_pool_free`. Freeing null is a no-op; double free / use-after-free is the host's bug.
//! * Input bytes are copied during open; the host keeps ownership of its own buffer.
//! * Output structs / byte buffers are always caller-allocated; Rust never hands out interior pointers.
//! * Every function returns an `i32` code (see `error::code`), never unwinds across the boundary.

use crate::error::{code, Error};
use crate::pool::{LightPool, LightSnapshot, Source, SNAPSHOT_LEN};
use std::ffi::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const ABI_VERSION: u32 = 1;

/// Caller must set `struct_size` to `ar_light_snapshot_struct_size()` before the call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ArLightSnapshot {
    pub struct_size: u32,
    pub slot: u32,
    pub active_raw: u32,
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub effective_radius: f32,
    pub baseline_radius: f32,
    pub type_parameter: f32,
    pub age_raw: u32,
    pub special_fx_raw: u32,
    pub type_raw: u16,
    pub owner: i16,
    pub parent: i16,
    pub definition: i16,
    /// Authoritative saved bytes (the typed fields above are views of these).
    pub raw_fields: [u8; SNAPSHOT_LEN],
}

impl ArLightSnapshot {
    fn fill(&mut self, s: &LightSnapshot) {
        self.slot = s.slot();
        self.active_raw = s.active_raw();
        self.position = s.position();
        self.color = s.color();
        self.effective_radius = s.effective_radius();
        self.baseline_radius = s.baseline_radius();
        self.type_parameter = s.type_parameter();
        self.age_raw = s.age_raw();
        self.special_fx_raw = s.special_fx_raw();
        self.type_raw = s.type_raw();
        self.owner = s.owner();
        self.parent = s.parent();
        self.definition = s.definition();
        self.raw_fields = *s.raw_fields();
    }
}

fn guard(f: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(code::PANIC)
}

#[no_mangle]
pub extern "C" fn ar_light_abi_version() -> u32 {
    ABI_VERSION
}

#[no_mangle]
pub extern "C" fn ar_light_snapshot_struct_size() -> u32 {
    std::mem::size_of::<ArLightSnapshot>() as u32
}

/// Static NUL-terminated ASCII description; never free it.
#[no_mangle]
pub extern "C" fn ar_light_error_message(error: i32) -> *const c_char {
    let s: &'static [u8] = match error {
        code::OK => b"ok\0",
        code::NULL_ARGUMENT => b"null argument\0",
        code::BUFFER_TOO_SMALL => b"buffer too small\0",
        code::BAD_STRUCT_SIZE => b"struct_size mismatch\0",
        code::INVALID_SLOT => b"slot out of range\0",
        code::INACTIVE_SLOT => b"slot inactive\0",
        code::PANIC => b"internal panic\0",
        code::NONFINITE_POSITION => b"saved coordinate is not finite\0",
        code::RESULT_OVERFLOW => b"projected coordinate overflows float32\0",
        code::INVALID_RECORD_INDEX => b"emitter record index out of range\0",
        code::INACTIVE_EMITTER => b"emitter record inactive\0",
        code::INVALID_CHILD => b"child ordinal out of range\0",
        code::LIGHT_LINK_MISMATCH => b"child light link does not match expected slot\0",
        code::FIXED_OVERFLOW => b"fixed position plus delta overflows int32\0",
        code::BAD_RECORDS_LENGTH => b"emitter records length invalid\0",
        code::PFIL_HEADER_TRUNCATED => b"PFIL header truncated\0",
        code::PFIL_BAD_MAGIC => b"missing PFIL magic\0",
        code::DECLARED_SIZE => b"PFIL declared size mismatch\0",
        code::LZSS_TRUNCATED => b"LZSS input truncated\0",
        code::LZSS_OVERSHOOT => b"LZSS back-reference overshoots decoded size\0",
        code::LZSS_TRAILING_INPUT => b"LZSS trailing input\0",
        code::PAYLOAD_LENGTH => b"decoded payload length is not 57352\0",
        code::BAD_VERSION => b"light pool version is not 1\0",
        code::BAD_CAPACITY => b"light pool capacity is not 1024\0",
        _ => b"unknown error\0",
    };
    s.as_ptr().cast()
}

/// Parses a light.dat file image (PFIL-wrapped or raw 57352-byte payload) into `*out`.
/// On failure `*out` is set to null.
///
/// # Safety
/// `data` must be valid for `len` bytes; `out` must be a valid writable pointer.
#[no_mangle]
pub unsafe extern "C" fn ar_light_pool_open(data: *const u8, len: usize, out: *mut *mut LightPool) -> i32 {
    if out.is_null() {
        return code::NULL_ARGUMENT;
    }
    *out = std::ptr::null_mut();
    if data.is_null() {
        return code::NULL_ARGUMENT;
    }
    guard(|| {
        let bytes = std::slice::from_raw_parts(data, len);
        match LightPool::from_file_bytes(bytes) {
            Ok(pool) => {
                *out = Box::into_raw(Box::new(pool));
                code::OK
            }
            Err(e) => e.code(),
        }
    })
}

/// # Safety
/// `pool` must be null or a handle from `ar_light_pool_open` not yet freed.
#[no_mangle]
pub unsafe extern "C" fn ar_light_pool_free(pool: *mut LightPool) {
    if !pool.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| drop(Box::from_raw(pool))));
    }
}

/// # Safety
/// `pool` must be a live handle; `out` writable.
#[no_mangle]
pub unsafe extern "C" fn ar_light_pool_active_count(pool: *const LightPool, out: *mut u32) -> i32 {
    if pool.is_null() || out.is_null() {
        return code::NULL_ARGUMENT;
    }
    guard(|| {
        *out = (*pool).active_slots().count() as u32;
        code::OK
    })
}

unsafe fn snapshot_impl(pool: *const LightPool, slot: u32, out: *mut ArLightSnapshot, any: bool) -> i32 {
    if pool.is_null() || out.is_null() {
        return code::NULL_ARGUMENT;
    }
    guard(|| {
        if (*out).struct_size != std::mem::size_of::<ArLightSnapshot>() as u32 {
            return code::BAD_STRUCT_SIZE;
        }
        let pool = &*pool;
        let snap = if any {
            pool.snapshot_any(slot)
        } else {
            pool.snapshot_any(slot).and_then(|s| {
                if s.active_raw() != 0 {
                    Ok(s)
                } else {
                    Err(Error::InactiveSlot(slot))
                }
            })
        };
        match snap {
            Ok(s) => {
                (*out).fill(&s);
                code::OK
            }
            Err(e) => e.code(),
        }
    })
}

/// Active slot only (INACTIVE_SLOT otherwise). `out` is untouched on error.
///
/// # Safety
/// `pool` must be a live handle; `out` must point to a writable `ArLightSnapshot` with `struct_size` set.
#[no_mangle]
pub unsafe extern "C" fn ar_light_pool_snapshot(pool: *const LightPool, slot: u32, out: *mut ArLightSnapshot) -> i32 {
    snapshot_impl(pool, slot, out, false)
}

/// Any in-range slot, including inactive ones (bytes preserved as stored).
///
/// # Safety
/// Same as `ar_light_pool_snapshot`.
#[no_mangle]
pub unsafe extern "C" fn ar_light_pool_snapshot_any(
    pool: *const LightPool,
    slot: u32,
    out: *mut ArLightSnapshot,
) -> i32 {
    snapshot_impl(pool, slot, out, true)
}

unsafe fn copy_impl(src: &[u8], buf: *mut u8, cap: usize, out_len: *mut usize) -> i32 {
    if out_len.is_null() {
        return code::NULL_ARGUMENT;
    }
    *out_len = src.len();
    if src.is_empty() {
        return code::OK;
    }
    if buf.is_null() || cap < src.len() {
        return code::BUFFER_TOO_SMALL;
    }
    std::ptr::copy_nonoverlapping(src.as_ptr(), buf, src.len());
    code::OK
}

/// Copies the decoded 57352-byte payload. `*out_len` always receives the required size; pass a null
/// `buf` to query it (returns BUFFER_TOO_SMALL).
///
/// # Safety
/// `pool` live; `buf` valid for `cap` bytes when non-null; `out_len` writable.
#[no_mangle]
pub unsafe extern "C" fn ar_light_pool_copy_payload(
    pool: *const LightPool,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> i32 {
    if pool.is_null() {
        return code::NULL_ARGUMENT;
    }
    guard(|| copy_impl((*pool).payload(), buf, cap, out_len))
}

/// Copies the original 64-byte PFIL header (`*out_len` = 0 for raw-payload input).
///
/// # Safety
/// Same as `ar_light_pool_copy_payload`.
#[no_mangle]
pub unsafe extern "C" fn ar_light_pool_copy_pfil_header(
    pool: *const LightPool,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> i32 {
    if pool.is_null() {
        return code::NULL_ARGUMENT;
    }
    guard(|| match (*pool).source() {
        Source::Pfil { header } => copy_impl(header, buf, cap, out_len),
        Source::RawPayload => copy_impl(&[], buf, cap, out_len),
    })
}

// ---- Projection (appended; ABI version 1 layouts above are unchanged) ----

/// Caller sets `struct_size` to `ar_light_projection_request_struct_size()`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ArLightProjectionRequest {
    pub struct_size: u32,
    pub slot: u32,
    pub target_guid: [u8; 16],
    /// Signed 16.16 fixed-point delta X, Y, Z.
    pub delta_fixed: [i32; 3],
}

/// Caller sets `struct_size` to `ar_projected_light_struct_size()` on every output element.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ArProjectedLight {
    pub struct_size: u32,
    pub request_index: u32,
    pub target_guid: [u8; 16],
    pub slot: u32,
    pub active_raw: u32,
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub effective_radius: f32,
    pub baseline_radius: f32,
    pub type_parameter: f32,
    pub age_raw: u32,
    pub special_fx_raw: u32,
    pub type_raw: u16,
    pub owner: i16,
    pub parent: i16,
    pub definition: i16,
    /// Authoritative projected bytes: saved 56 bytes with only XYZ possibly replaced.
    pub raw_fields: [u8; SNAPSHOT_LEN],
}

#[no_mangle]
pub extern "C" fn ar_light_projection_request_struct_size() -> u32 {
    std::mem::size_of::<ArLightProjectionRequest>() as u32
}

#[no_mangle]
pub extern "C" fn ar_projected_light_struct_size() -> u32 {
    std::mem::size_of::<ArProjectedLight>() as u32
}

/// Atomic batch projection of saved lights. All requests are validated before any output is written.
///
/// * `count == 0`: returns OK with `*out_count = 0`; `requests`/`out` may be null.
/// * `out_capacity < count`: `AR_BUFFER_TOO_SMALL`, `*out_count = count` (required size), nothing written.
/// * Success: `*out_count = count`; `out[i]` corresponds to `requests[i]`.
/// * Any error: `out` is untouched, `*out_count = 0` (except BUFFER_TOO_SMALL), and `*failed_index`
///   (if non-null) receives the offending request index, or `u32::MAX` when not request-specific.
/// The pool is never modified. `out` must not overlap `requests`.
///
/// # Safety
/// `pool` live; `requests` valid for `count` elements; `out` valid for `out_capacity` elements;
/// `out_count` writable; `failed_index` null or writable.
#[no_mangle]
pub unsafe extern "C" fn ar_light_pool_project_batch(
    pool: *const LightPool,
    requests: *const ArLightProjectionRequest,
    count: usize,
    out: *mut ArProjectedLight,
    out_capacity: usize,
    out_count: *mut usize,
    failed_index: *mut u32,
) -> i32 {
    if !failed_index.is_null() {
        *failed_index = u32::MAX;
    }
    if out_count.is_null() {
        return code::NULL_ARGUMENT;
    }
    *out_count = 0;
    if pool.is_null() || (count > 0 && (requests.is_null() || out.is_null())) {
        return code::NULL_ARGUMENT;
    }
    guard(|| {
        if count == 0 {
            return code::OK;
        }
        if out_capacity < count {
            *out_count = count;
            return code::BUFFER_TOO_SMALL;
        }
        let reqs = std::slice::from_raw_parts(requests, count);
        let fail = |i: usize, c: i32| {
            if !failed_index.is_null() {
                *failed_index = i.min(u32::MAX as usize - 1) as u32;
            }
            c
        };
        let req_size = std::mem::size_of::<ArLightProjectionRequest>() as u32;
        let out_size = std::mem::size_of::<ArProjectedLight>() as u32;
        let outs = std::slice::from_raw_parts(out, count);
        for (i, (r, o)) in reqs.iter().zip(outs).enumerate() {
            if r.struct_size != req_size || o.struct_size != out_size {
                return fail(i, code::BAD_STRUCT_SIZE);
            }
        }
        let rust_reqs: Vec<crate::projection::LightProjectionRequest> = reqs
            .iter()
            .map(|r| crate::projection::LightProjectionRequest {
                slot: r.slot,
                target_guid: r.target_guid,
                delta_fixed: r.delta_fixed,
            })
            .collect();
        match crate::projection::project_batch(&*pool, &rust_reqs) {
            Err(e) => fail(e.index, e.error.code()),
            Ok(done) => {
                let outs = std::slice::from_raw_parts_mut(out, count);
                for (o, p) in outs.iter_mut().zip(&done) {
                    let s = &p.snapshot;
                    o.request_index = p.request_index as u32;
                    o.target_guid = p.target_guid;
                    o.slot = s.slot();
                    o.active_raw = s.active_raw();
                    o.position = s.position();
                    o.color = s.color();
                    o.effective_radius = s.effective_radius();
                    o.baseline_radius = s.baseline_radius();
                    o.type_parameter = s.type_parameter();
                    o.age_raw = s.age_raw();
                    o.special_fx_raw = s.special_fx_raw();
                    o.type_raw = s.type_raw();
                    o.owner = s.owner();
                    o.parent = s.parent();
                    o.definition = s.definition();
                    o.raw_fields = *s.raw_fields();
                }
                *out_count = count;
                code::OK
            }
        }
    })
}

// ---- Particle-linked light projection (appended; existing layouts unchanged) ----

/// Caller sets `struct_size` to `ar_particle_light_request_struct_size()`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ArParticleLightRequest {
    pub struct_size: u32,
    /// Index into the contiguous 2252-byte emitter record buffer.
    pub record_index: u32,
    pub child_ordinal: u32,
    /// Light slot the child's saved link must equal.
    pub light_slot: u32,
    pub target_guid: [u8; 16],
    /// Signed 16.16 fixed-point delta X, Y, Z.
    pub delta_fixed: [i32; 3],
}

/// Caller sets `struct_size` to `ar_particle_projected_light_struct_size()` on every output element.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ArParticleProjectedLight {
    pub struct_size: u32,
    pub request_index: u32,
    pub target_guid: [u8; 16],
    pub slot: u32,
    pub active_raw: u32,
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub effective_radius: f32,
    pub baseline_radius: f32,
    pub type_parameter: f32,
    pub age_raw: u32,
    pub special_fx_raw: u32,
    pub type_raw: u16,
    pub owner: i16,
    pub parent: i16,
    pub definition: i16,
    /// Authoritative projected light bytes: saved 56 bytes with only XYZ possibly replaced.
    pub raw_fields: [u8; SNAPSHOT_LEN],
    pub child_ordinal: u32,
    /// Raw child active byte (non-zero = active).
    pub child_active_raw: u32,
    /// Active child: fixed + delta. Inactive child: saved fixed, unchanged.
    pub fixed_position: [i32; 3],
}

#[no_mangle]
pub extern "C" fn ar_particle_light_request_struct_size() -> u32 {
    std::mem::size_of::<ArParticleLightRequest>() as u32
}

#[no_mangle]
pub extern "C" fn ar_particle_projected_light_struct_size() -> u32 {
    std::mem::size_of::<ArParticleProjectedLight>() as u32
}

/// Atomic batch projection of particle-linked saved lights. Same capacity/error/`failed_index`
/// conventions as `ar_light_pool_project_batch`. `records` holds `record_count` contiguous 2252-byte
/// saved emitter records and is never modified; the host applies the returned `fixed_position` values to
/// its own copy of the emitter record. `record_count == 0` allows a null `records`.
///
/// # Safety
/// `pool` live; `records` valid for `record_count * 2252` bytes; `requests` valid for `count` elements;
/// `out` valid for `out_capacity` elements; `out_count` writable; `failed_index` null or writable.
#[no_mangle]
pub unsafe extern "C" fn ar_light_pool_project_particle_batch(
    pool: *const LightPool,
    records: *const u8,
    record_count: usize,
    requests: *const ArParticleLightRequest,
    count: usize,
    out: *mut ArParticleProjectedLight,
    out_capacity: usize,
    out_count: *mut usize,
    failed_index: *mut u32,
) -> i32 {
    use crate::projection::{project_particle_batch, ParticleLightRequest, EMITTER_RECORD_LEN};
    if !failed_index.is_null() {
        *failed_index = u32::MAX;
    }
    if out_count.is_null() {
        return code::NULL_ARGUMENT;
    }
    *out_count = 0;
    if pool.is_null() || (count > 0 && (requests.is_null() || out.is_null())) || (record_count > 0 && records.is_null())
    {
        return code::NULL_ARGUMENT;
    }
    guard(|| {
        if count == 0 {
            return code::OK;
        }
        if out_capacity < count {
            *out_count = count;
            return code::BUFFER_TOO_SMALL;
        }
        let fail = |i: usize, c: i32| {
            if !failed_index.is_null() {
                *failed_index = if i >= u32::MAX as usize { u32::MAX } else { i as u32 };
            }
            c
        };
        let Some(record_bytes) = record_count.checked_mul(EMITTER_RECORD_LEN).filter(|n| *n <= isize::MAX as usize) else {
            return code::BAD_RECORDS_LENGTH;
        };
        let recs: &[u8] = if record_bytes == 0 { &[] } else { std::slice::from_raw_parts(records, record_bytes) };
        let reqs = std::slice::from_raw_parts(requests, count);
        let outs = std::slice::from_raw_parts(out, count);
        let req_size = std::mem::size_of::<ArParticleLightRequest>() as u32;
        let out_size = std::mem::size_of::<ArParticleProjectedLight>() as u32;
        for (i, (r, o)) in reqs.iter().zip(outs).enumerate() {
            if r.struct_size != req_size || o.struct_size != out_size {
                return fail(i, code::BAD_STRUCT_SIZE);
            }
        }
        let rust_reqs: Vec<ParticleLightRequest> = reqs
            .iter()
            .map(|r| ParticleLightRequest {
                record_index: r.record_index,
                child_ordinal: r.child_ordinal,
                light_slot: r.light_slot,
                target_guid: r.target_guid,
                delta_fixed: r.delta_fixed,
            })
            .collect();
        match project_particle_batch(&*pool, recs, &rust_reqs) {
            Err(e) => fail(e.index, e.error.code()),
            Ok(done) => {
                let outs = std::slice::from_raw_parts_mut(out, count);
                for (o, p) in outs.iter_mut().zip(&done) {
                    let s = &p.snapshot;
                    o.request_index = p.request_index as u32;
                    o.target_guid = p.target_guid;
                    o.slot = s.slot();
                    o.active_raw = s.active_raw();
                    o.position = s.position();
                    o.color = s.color();
                    o.effective_radius = s.effective_radius();
                    o.baseline_radius = s.baseline_radius();
                    o.type_parameter = s.type_parameter();
                    o.age_raw = s.age_raw();
                    o.special_fx_raw = s.special_fx_raw();
                    o.type_raw = s.type_raw();
                    o.owner = s.owner();
                    o.parent = s.parent();
                    o.definition = s.definition();
                    o.raw_fields = *s.raw_fields();
                    o.child_ordinal = p.child_ordinal;
                    o.child_active_raw = p.child_active_raw as u32;
                    o.fixed_position = p.fixed_position;
                }
                *out_count = count;
                code::OK
            }
        }
    })
}
