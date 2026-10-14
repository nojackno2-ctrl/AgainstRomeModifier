//! Owned base/effective catalogs and lifetime frame queries; no native residency inference.
use crate::{error::code, particle_definitions::*};
use std::{
    collections::BTreeMap,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr, slice,
};

/// Live catalog, writable disjoint output; synchronize with free. Error is atomic.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_definition_primary_mode(
    handle: *const DefinitionCatalog,
    owner_type: i32,
    output: *mut i32,
) -> i32 {
    if handle.is_null() || output.is_null() {
        return code::NULL_ARGUMENT;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let catalog = &*handle;
        let chain = match definition_chain(
            owner_type,
            &catalog.objects,
            &catalog.parameters,
            &catalog.geometries,
            &catalog.grafiks,
        ) {
            Ok(chain) => chain,
            Err(DefinitionChainError::NoPrimaryGrafikObject) => return 225,
            Err(_) => return 224,
        };
        let Some(mode) = chain.grafik_object.mode_native() else {
            return 249;
        };
        *output = mode as i32;
        code::OK
    }))
    .unwrap_or(code::PANIC)
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParticleFrameCandidate {
    pub ordinal: u32,
    pub position_native: [i32; 3],
    pub velocity_high: [i32; 3],
    pub lifetime_raw: i32,
    pub frame: u32,
    pub status: i32,
}
#[no_mangle]
pub extern "C" fn ar_particle_frame_candidate_struct_size() -> u32 {
    std::mem::size_of::<ParticleFrameCandidate>() as u32
}

/// Bounded active child candidates; not a visibility, ordering or drawing contract.
/// # Safety
/// Live catalog; readable record/state buffers, writable disjoint output/count buffers.
/// Synchronize with free. All outputs remain unchanged on a whole-query error.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_definition_saved_frames(
    handle: *const DefinitionCatalog,
    owner_type: i32,
    slot: u32,
    record: *const u8,
    record_len: usize,
    state: *const u8,
    state_len: usize,
    alr_id: i32,
    columns: i32,
    directions: i32,
    frame_count: u32,
    output: *mut ParticleFrameCandidate,
    capacity: usize,
    count: *mut u32,
) -> i32 {
    ar_particle_definition_saved_frames_translated(
        handle,
        owner_type,
        slot,
        record,
        record_len,
        state,
        state_len,
        alr_id,
        columns,
        directions,
        frame_count,
        0,
        0,
        0,
        output,
        capacity,
        count,
    )
}

/// Editor translation occurs on a detached snapshot before count-bounded frame selection.
/// # Safety
/// Same buffer/handle preconditions as ar_particle_definition_saved_frames.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_definition_saved_frames_translated(
    handle: *const DefinitionCatalog,
    owner_type: i32,
    slot: u32,
    record: *const u8,
    record_len: usize,
    state: *const u8,
    state_len: usize,
    alr_id: i32,
    columns: i32,
    directions: i32,
    frame_count: u32,
    dx: i32,
    dy: i32,
    dz: i32,
    output: *mut ParticleFrameCandidate,
    capacity: usize,
    count: *mut u32,
) -> i32 {
    if handle.is_null() || record.is_null() || state.is_null() || count.is_null() {
        return code::NULL_ARGUMENT;
    }
    if record_len != crate::particle::RECORD_LEN || state_len != crate::particle::STATE_LEN {
        return code::BAD_RECORDS_LENGTH;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let catalog = &*handle;
        let chain = match definition_chain(
            owner_type,
            &catalog.objects,
            &catalog.parameters,
            &catalog.geometries,
            &catalog.grafiks,
        ) {
            Ok(chain) => chain,
            Err(DefinitionChainError::NoPrimaryGrafikObject) => return 225,
            Err(_) => return 224,
        };
        if chain.grafik_object.alr_id() < 0 {
            return 247;
        }
        if alr_id != chain.grafik_object.alr_id() as i32 {
            return 248;
        }
        let context = match chain.frame_context(columns, directions, frame_count) {
            Ok(context) => context,
            Err(_) => return 224,
        };
        let snapshot = match crate::particle::ParticleSnapshot::from_saved_buffers(
            slot,
            slice::from_raw_parts(record, record_len),
            slice::from_raw_parts(state, state_len),
        ) {
            Ok(snapshot) => snapshot,
            Err(error) => return error.code(),
        };
        let snapshot = match snapshot.translated_fixed([dx, dy, dz]) {
            Ok(snapshot) => snapshot,
            Err(error) => return error.code(),
        };
        let candidates =
            match crate::particle_frames::saved_frame_candidates(&snapshot, Some(&context)) {
                Ok(candidates) => candidates,
                Err(error) => return error.code(),
            };
        if candidates.len() > capacity {
            return code::BUFFER_TOO_SMALL;
        }
        if !candidates.is_empty() && output.is_null() {
            return code::NULL_ARGUMENT;
        }
        let result: Vec<_> = candidates
            .into_iter()
            .map(|child| {
                let (frame, status) = match child.frame {
                    Ok(frame) => (frame, 0),
                    Err(reason) => (u32::MAX, frame_unavailable_code(reason)),
                };
                ParticleFrameCandidate {
                    ordinal: child.ordinal,
                    position_native: child.position_native,
                    velocity_high: child.velocity_high,
                    lifetime_raw: child.lifetime_raw,
                    frame,
                    status,
                }
            })
            .collect();
        if !result.is_empty() {
            std::ptr::copy_nonoverlapping(result.as_ptr(), output, result.len());
        }
        *count = result.len() as u32;
        code::OK
    }))
    .unwrap_or(code::PANIC)
}

fn frame_unavailable_code(reason: crate::particle_frames::FrameUnavailable) -> i32 {
    use crate::particle_frames::FrameUnavailable::*;
    match reason {
        MissingContext => 224,
        InvalidLayout => 240,
        VelocityRoundingUnverified => 241,
        DirectionRoundingUnverified => 242,
        ZeroDivisor => 243,
        DivisionOverflow => 244,
        NegativeIndex => 245,
        FrameOutOfRange => 246,
    }
}

pub struct DefinitionCatalog {
    source_count: usize,
    parameters: BTreeMap<i32, ParameterDefinition>,
    geometries: BTreeMap<i32, GeometryDefinition>,
    grafiks: BTreeMap<i32, GrafikDefinition>,
    objects: BTreeMap<i32, ObjectParticleDefinition>,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArParticleDefinition {
    pub struct_size: u32,
    pub owner_type: i32,
    pub parameter_id: i32,
    pub geometry_id: i32,
    pub grafik_id: i32,
    pub grafik_object: i32,
    pub alr_id: i32,
    pub lifetime: i32,
    pub type_divisor: i32,
    pub velocity_branch_raw: i32,
    pub skip_first_raw: i32,
}
#[no_mangle]
pub extern "C" fn ar_particle_definition_abi_version() -> u32 {
    1
}
#[no_mangle]
pub extern "C" fn ar_particle_definition_struct_size() -> u32 {
    std::mem::size_of::<ArParticleDefinition>() as u32
}

unsafe fn input<'a>(p: *const u8, n: usize) -> Result<&'a [u8], i32> {
    if n > isize::MAX as usize {
        return Err(code::BUFFER_TOO_SMALL);
    }
    if n == 0 {
        return Ok(&[]);
    }
    if p.is_null() {
        return Err(code::NULL_ARGUMENT);
    }
    Ok(slice::from_raw_parts(p, n))
}
/// Parse four caller-owned file images, raw/PFIL. Failure clears *out; success owns detached catalogs.
/// # Safety
/// Nonempty inputs must be readable for lengths, out writable. Inputs must not overlap out storage.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_definition_open(
    par: *const u8,
    par_len: usize,
    geo: *const u8,
    geo_len: usize,
    gfx: *const u8,
    gfx_len: usize,
    obj: *const u8,
    obj_len: usize,
    out: *mut *mut DefinitionCatalog,
) -> i32 {
    if out.is_null() {
        return code::NULL_ARGUMENT;
    }
    catch_unwind(AssertUnwindSafe(|| {
        *out = ptr::null_mut();
        let result = (|| -> Result<DefinitionCatalog, i32> {
            Ok(DefinitionCatalog {
                source_count: 0,
                parameters: parameter_definitions(input(par, par_len)?).map_err(|_| 220)?,
                geometries: geometry_definitions(input(geo, geo_len)?).map_err(|_| 221)?,
                grafiks: grafik_definitions(input(gfx, gfx_len)?).map_err(|_| 222)?,
                objects: object_particle_definitions(input(obj, obj_len)?).map_err(|_| 223)?,
            })
        })();
        match result {
            Ok(catalog) => {
                *out = Box::into_raw(Box::new(catalog));
                code::OK
            }
            Err(code) => code,
        }
    }))
    .unwrap_or(code::PANIC)
}

/// Load a checked loose-file snapshot from an absolute UTF-8 game root.
/// # Safety
/// root is readable for len, out writable and disjoint; failure clears *out.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_definition_load_utf8(
    root: *const u8,
    len: usize,
    out: *mut *mut DefinitionCatalog,
) -> i32 {
    if out.is_null() {
        return code::NULL_ARGUMENT;
    }
    catch_unwind(AssertUnwindSafe(|| {
        *out = ptr::null_mut();
        let bytes = match input(root, len) {
            Ok(bytes) => bytes,
            Err(code) => return code,
        };
        let text = match std::str::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => return 226,
        };
        let source = match crate::particle_definition_sources::load(std::path::Path::new(text)) {
            Ok(source) => source,
            Err(_) => return 226,
        };
        let catalog = DefinitionCatalog {
            source_count: source.sources.len(),
            parameters: source.parameters,
            geometries: source.geometries,
            grafiks: source.grafiks,
            objects: source.objects,
        };
        *out = Box::into_raw(Box::new(catalog));
        code::OK
    }))
    .unwrap_or(code::PANIC)
}
/// # Safety
/// Live handle, writable output disjoint from handle storage; synchronize with free.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_definition_source_count(
    handle: *const DefinitionCatalog,
    out: *mut u32,
) -> i32 {
    if handle.is_null() || out.is_null() {
        return code::NULL_ARGUMENT;
    }
    *out = (*handle).source_count as u32;
    code::OK
}

/// Lifetime-only frame resolution using this immutable catalog and caller-matched ALR metadata.
/// # Safety
/// Live handle, writable output disjoint from catalog. Synchronize queries with free.
/// Error leaves output unchanged; the caller must not treat an unchanged value as a frame.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_definition_lifetime_frame(
    handle: *const DefinitionCatalog,
    owner_type: i32,
    child_lifetime: i32,
    alr_id: i32,
    columns: i32,
    directions: i32,
    frame_count: u32,
    out: *mut u32,
) -> i32 {
    if handle.is_null() || out.is_null() {
        return code::NULL_ARGUMENT;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let catalog = &*handle;
        let chain = match definition_chain(
            owner_type,
            &catalog.objects,
            &catalog.parameters,
            &catalog.geometries,
            &catalog.grafiks,
        ) {
            Ok(chain) => chain,
            Err(DefinitionChainError::NoPrimaryGrafikObject) => return 225,
            Err(_) => return 224,
        };
        if chain.grafik_object.alr_id() < 0 {
            return 247;
        }
        if alr_id != chain.grafik_object.alr_id() as i32 {
            return 248;
        }
        let context = match chain.frame_context(columns, directions, frame_count) {
            Ok(context) => context,
            Err(_) => return 224,
        };
        let frame = match crate::particle_frames::lifetime_frame(child_lifetime, &context) {
            Ok(frame) => frame,
            Err(reason) => return frame_unavailable_code(reason),
        };
        *out = frame;
        code::OK
    }))
    .unwrap_or(code::PANIC)
}
/// # Safety
/// Null or live handle from open; free exactly once, never use afterward.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_definition_free(handle: *mut DefinitionCatalog) {
    if !handle.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| drop(Box::from_raw(handle))));
    }
}
/// Query one primary definition chain; no caller output is modified on error.
/// # Safety
/// Live handle and aligned readable/writable output, disjoint from handle storage.
/// Initialize output.struct_size to the exported size before calling; synchronize free with queries.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_definition_query(
    handle: *const DefinitionCatalog,
    owner_type: i32,
    out: *mut ArParticleDefinition,
) -> i32 {
    if handle.is_null() || out.is_null() {
        return code::NULL_ARGUMENT;
    }
    catch_unwind(AssertUnwindSafe(|| {
        if (*out).struct_size != ar_particle_definition_struct_size() {
            return code::BAD_STRUCT_SIZE;
        }
        let catalog = &*handle;
        let chain = match definition_chain(
            owner_type,
            &catalog.objects,
            &catalog.parameters,
            &catalog.geometries,
            &catalog.grafiks,
        ) {
            Ok(chain) => chain,
            Err(DefinitionChainError::NoPrimaryGrafikObject) => return 225,
            Err(_) => return 224,
        };
        let result = ArParticleDefinition {
            struct_size: ar_particle_definition_struct_size(),
            owner_type,
            parameter_id: chain.parameter.id,
            geometry_id: chain.geometry.id,
            grafik_id: chain.grafik.id,
            grafik_object: chain.grafik_object.id,
            alr_id: chain.grafik_object.alr_id() as i32,
            lifetime: chain.parameter.lifetime(),
            type_divisor: chain.grafik_object.animation_length(),
            velocity_branch_raw: chain.parameter.velocity_branch_raw(),
            skip_first_raw: chain.parameter.skip_first_raw(),
        };
        *out = result;
        code::OK
    }))
    .unwrap_or(code::PANIC)
}
