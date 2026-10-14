//! Additive particle ABI. Handle ownership matches LightPool: free exactly once (null is a no-op).
//! All data pointers must designate valid caller-owned storage; no interior pointers escape.
#![allow(clippy::missing_safety_doc)]
use crate::{error::code, particle::*, pool::Source};
use std::{
    ffi::c_char,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavedObjectMode {
    pub struct_size: u32,
    pub object_uid: u32,
    pub gfx_slot: u32,
    pub gfx_mode: i32,
    pub resolved_mode: i32,
    pub leaf_address: u32,
}
#[no_mangle]
pub extern "C" fn ar_particle_saved_object_mode_struct_size() -> u32 {
    24
}

/// Detached decoded payloads; caller verifies primary grafik type context.
/// All output fields unchanged on rejection. Unknown leaf is0, never a fallback draw.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_saved_object_mode(
    object: *const u8,
    object_len: usize,
    gfx: *const u8,
    gfx_len: usize,
    expected_uid: u32,
    primary_mode: i32,
    has_primary: u32,
    output: *mut SavedObjectMode,
) -> i32 {
    if object.is_null() || gfx.is_null() || output.is_null() {
        return code::NULL_ARGUMENT;
    }
    if (*output).struct_size != 24 {
        return code::BAD_STRUCT_SIZE;
    }
    if has_primary > 1 || (has_primary == 1 && i16::try_from(primary_mode).is_err()) {
        return 250;
    }
    if object_len != 79 || !(8..=238008).contains(&gfx_len) {
        return 252;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let Some(saved) = crate::particle_frames::saved_object_gfxtype_mode(
            std::slice::from_raw_parts(object, object_len),
            std::slice::from_raw_parts(gfx, gfx_len),
        ) else {
            return 252;
        };
        if saved.object_uid != expected_uid {
            return 251;
        }
        let Some(resolved) = crate::particle_frames::resolve_particle_mode(
            saved.mode,
            if has_primary == 1 {
                Some(primary_mode as i16)
            } else {
                None
            },
        ) else {
            return 249;
        };
        *output = SavedObjectMode {
            struct_size: 24,
            object_uid: saved.object_uid,
            gfx_slot: saved.gfx_slot as u32,
            gfx_mode: saved.mode as i32,
            resolved_mode: resolved.mode,
            leaf_address: resolved.leaf_address.unwrap_or(0),
        };
        code::OK
    }))
    .unwrap_or(code::PANIC)
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArParticlePoolSnapshot {
    pub struct_size: u32,
    pub slot: u32,
    pub active_raw: u32,
    pub count_raw: i32,
    pub owner: i16,
    pub reserved: u16,
    pub age_raw: i32,
}
fn guard(f: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(code::PANIC)
}
#[no_mangle]
pub extern "C" fn ar_particle_pool_abi_version() -> u32 {
    1
}
#[no_mangle]
pub extern "C" fn ar_particle_pool_snapshot_struct_size() -> u32 {
    std::mem::size_of::<ArParticlePoolSnapshot>() as u32
}
#[no_mangle]
pub extern "C" fn ar_particle_pool_record_size() -> usize {
    RECORD_LEN
}
#[no_mangle]
pub extern "C" fn ar_particle_pool_state_size() -> usize {
    STATE_LEN
}
#[no_mangle]
pub extern "C" fn ar_particle_pool_child_size() -> usize {
    CHILD_LEN
}
#[no_mangle]
pub extern "C" fn ar_particle_pool_payload_size() -> usize {
    PAYLOAD_LEN
}
#[no_mangle]
pub extern "C" fn ar_particle_pool_error_message(error: i32) -> *const c_char {
    let s: &[u8] = match error {
        BAD_CHILDREN => b"particle children header is not 64\0",
        BAD_RENDERABLE_COUNT => b"particle renderable count outside 0..64\0",
        IO_ERROR => b"particle source read failed\0",
        code::PAYLOAD_LENGTH => b"particle payload length is not 2322444\0",
        code::BAD_VERSION => b"particle version is not 1\0",
        code::BAD_CAPACITY => b"particle capacity is not 1024\0",
        _ => return crate::ffi::ar_light_error_message(error),
    };
    s.as_ptr().cast()
}
/// Copies input; failure sets *out to null. data must be readable for len, out writable.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_pool_open(
    data: *const u8,
    len: usize,
    out: *mut *mut ParticlePool,
) -> i32 {
    if out.is_null() {
        return code::NULL_ARGUMENT;
    }
    guard(|| {
        *out = ptr::null_mut();
        if data.is_null() {
            return code::NULL_ARGUMENT;
        }
        if len > isize::MAX as usize {
            return code::BUFFER_TOO_SMALL;
        }
        match ParticlePool::from_file_bytes(std::slice::from_raw_parts(data, len)) {
            Ok(p) => {
                *out = Box::into_raw(Box::new(p));
                code::OK
            }
            Err(e) => e.code(),
        }
    })
}
/// pool must be null or a live handle allocated by open. Never double-free or use after free.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_pool_free(pool: *mut ParticlePool) {
    if !pool.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| drop(Box::from_raw(pool))));
    }
}
#[no_mangle]
pub unsafe extern "C" fn ar_particle_pool_active_count(
    pool: *const ParticlePool,
    out: *mut u32,
) -> i32 {
    if pool.is_null() || out.is_null() {
        return code::NULL_ARGUMENT;
    }
    guard(|| {
        *out = (*pool).active_slots().count() as u32;
        code::OK
    })
}
/// Atomic three-output snapshot: metadata, record (2252 bytes), state (16 bytes, four LE i32).
/// Output regions must not overlap each other or pool storage. On any error all outputs untouched.
unsafe fn snapshot(
    pool: *const ParticlePool,
    slot: u32,
    out: *mut ArParticlePoolSnapshot,
    record: *mut u8,
    record_cap: usize,
    state: *mut u8,
    state_cap: usize,
    any: bool,
) -> i32 {
    if pool.is_null() || out.is_null() || record.is_null() || state.is_null() {
        return code::NULL_ARGUMENT;
    }
    guard(|| {
        if (*out).struct_size != ar_particle_pool_snapshot_struct_size() {
            return code::BAD_STRUCT_SIZE;
        }
        if record_cap < RECORD_LEN || state_cap < STATE_LEN {
            return code::BUFFER_TOO_SMALL;
        }
        let s = match (*pool).snapshot_any(slot) {
            Ok(s) => s,
            Err(e) => return e.code(),
        };
        if !any && s.active_raw() == 0 {
            return code::INACTIVE_SLOT;
        }
        let metadata = ArParticlePoolSnapshot {
            struct_size: ar_particle_pool_snapshot_struct_size(),
            slot,
            active_raw: s.active_raw() as u32,
            count_raw: s.count_raw(),
            owner: s.owner(),
            reserved: 0,
            age_raw: s.age_raw(),
        };
        ptr::copy_nonoverlapping(s.raw_record().as_ptr(), record, RECORD_LEN);
        ptr::copy_nonoverlapping(s.raw_state().as_ptr(), state, STATE_LEN);
        *out = metadata;
        code::OK
    })
}
#[no_mangle]
pub unsafe extern "C" fn ar_particle_pool_snapshot_any(
    pool: *const ParticlePool,
    slot: u32,
    out: *mut ArParticlePoolSnapshot,
    record: *mut u8,
    record_cap: usize,
    state: *mut u8,
    state_cap: usize,
) -> i32 {
    snapshot(pool, slot, out, record, record_cap, state, state_cap, true)
}
#[no_mangle]
pub unsafe extern "C" fn ar_particle_pool_snapshot(
    pool: *const ParticlePool,
    slot: u32,
    out: *mut ArParticlePoolSnapshot,
    record: *mut u8,
    record_cap: usize,
    state: *mut u8,
    state_cap: usize,
) -> i32 {
    snapshot(pool, slot, out, record, record_cap, state, state_cap, false)
}
/// Bounded child access, independent of emitter/child active flags and count. Error leaves buf intact.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_pool_child_any(
    pool: *const ParticlePool,
    slot: u32,
    ordinal: u32,
    buf: *mut u8,
    cap: usize,
) -> i32 {
    if pool.is_null() || buf.is_null() {
        return code::NULL_ARGUMENT;
    }
    guard(|| {
        if cap < CHILD_LEN {
            return code::BUFFER_TOO_SMALL;
        }
        let c = match (*pool).snapshot_any(slot).and_then(|s| s.child(ordinal)) {
            Ok(c) => c,
            Err(e) => return e.code(),
        };
        ptr::copy_nonoverlapping(c.raw_bytes().as_ptr(), buf, CHILD_LEN);
        code::OK
    })
}
/// Optional count diagnostic; does not impose active/asset/definition render gates.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_pool_validate_renderable_count(
    pool: *const ParticlePool,
    slot: u32,
    out: *mut u32,
) -> i32 {
    if pool.is_null() || out.is_null() {
        return code::NULL_ARGUMENT;
    }
    guard(|| {
        match (*pool)
            .snapshot_any(slot)
            .and_then(|s| s.validate_renderable_count())
        {
            Ok(n) => {
                *out = n;
                code::OK
            }
            Err(e) => e.code(),
        }
    })
}
unsafe fn copy(src: &[u8], buf: *mut u8, cap: usize, out_len: *mut usize) -> i32 {
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
    ptr::copy_nonoverlapping(src.as_ptr(), buf, src.len());
    code::OK
}
#[no_mangle]
pub unsafe extern "C" fn ar_particle_pool_copy_payload(
    pool: *const ParticlePool,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> i32 {
    if pool.is_null() {
        return code::NULL_ARGUMENT;
    }
    guard(|| copy((*pool).payload(), buf, cap, out_len))
}
#[no_mangle]
pub unsafe extern "C" fn ar_particle_pool_copy_pfil_header(
    pool: *const ParticlePool,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> i32 {
    if pool.is_null() {
        return code::NULL_ARGUMENT;
    }
    guard(|| match (*pool).source() {
        Source::RawPayload => copy(&[], buf, cap, out_len),
        Source::Pfil { header } => copy(header, buf, cap, out_len),
    })
}
#[no_mangle]
pub unsafe extern "C" fn ar_particle_pool_copy_input_sha256(
    pool: *const ParticlePool,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> i32 {
    if pool.is_null() {
        return code::NULL_ARGUMENT;
    }
    guard(|| copy((*pool).input_sha256(), buf, cap, out_len))
}
