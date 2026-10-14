//! Saved-child frame candidates from the 4dcfc0 consumer. Definitions/assets are caller-resolved.
//! Not a simulator, native visibility oracle, definition loader, or complete draw contract.
use crate::particle::{ParticleSnapshot, CHILDREN};

/// Editor painter ordering shared by heterogeneous draw lists. Equal numeric
/// depth (including signed zero) retains input order; rejects nonfinite depth.
pub fn scene_draw_order(depths: &[f32]) -> Option<Vec<u32>> {
    if depths.len() > u32::MAX as usize || depths.iter().any(|depth| !depth.is_finite()) { return None; }
    let mut order: Vec<u32> = (0..depths.len() as u32).collect();
    order.sort_by(|&a, &b| depths[b as usize].partial_cmp(&depths[a as usize]).unwrap());
    Some(order)
}

/// Atomic caller-buffer ordering;207 invalid depth/count. Success writes count
/// indices only. Empty input permits null pointers; output may alias input.
/// # Safety
/// Aligned readable f32 depths and writable u32 output for declared lengths;
/// no concurrent mutation or retained pointers.
#[no_mangle]
pub unsafe extern "C" fn ar_scene_draw_order(depths: *const f32, count: usize,
    output: *mut u32, capacity: usize) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if count > u32::MAX as usize { return 207; }
        if count > isize::MAX as usize / 4 || capacity > isize::MAX as usize / 4
            || capacity < count { return crate::error::code::BUFFER_TOO_SMALL; }
        if count == 0 { return 0; }
        if depths.is_null() || output.is_null() { return crate::error::code::NULL_ARGUMENT; }
        let Some(order) = scene_draw_order(std::slice::from_raw_parts(depths, count)) else { return 207; };
        std::ptr::copy_nonoverlapping(order.as_ptr(), output, count); 0
    })).unwrap_or(crate::error::code::PANIC)
}

/// Editor billboard rectangle relative to the ALR canvas centre, in pixels.
/// This follows the editor's existing ALR anchor convention, not native
/// visibility, perspective, scaling or blend dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticleSpriteRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Four signed edges (16 bytes). Failed requests leave caller output untouched.
/// # Safety
/// `output` must point to four writable aligned i32 values.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_sprite_rect(
    anchor_width: u32, anchor_height: u32, offset_x: i32, offset_y: i32,
    width: i32, height: i32, pixel_count: usize, output: *mut i32,
) -> i32 {
    if output.is_null() { return 1; }
    let Some(rect) = particle_sprite_rect(anchor_width, anchor_height, offset_x,
        offset_y, width, height, pixel_count) else { return 253; };
    std::ptr::copy_nonoverlapping([rect.left, rect.top, rect.right, rect.bottom].as_ptr(), output, 4);
    0
}

/// Convert an uncropped decoded ALR frame into an anchor-relative rectangle.
/// Pixel count must describe the entire frame; offsets may be negative and the
/// frame need not lie inside its anchor canvas. No cropping or clamping occurs.
pub fn particle_sprite_rect(
    anchor_width: u32,
    anchor_height: u32,
    offset_x: i32,
    offset_y: i32,
    width: i32,
    height: i32,
    pixel_count: usize,
) -> Option<ParticleSpriteRect> {
    if anchor_width == 0 || anchor_height == 0 || width <= 0 || height <= 0 {
        return None;
    }
    let centre_x = i32::try_from(anchor_width).ok()? / 2;
    let centre_y = i32::try_from(anchor_height).ok()? / 2;
    if (width as usize).checked_mul(height as usize)? != pixel_count {
        return None;
    }
    let left = offset_x.checked_sub(centre_x)?;
    let top = offset_y.checked_sub(centre_y)?;
    Some(ParticleSpriteRect {
        left,
        top,
        right: left.checked_add(width)?,
        bottom: top.checked_add(height)?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavedObjectGfxMode {
    pub object_uid: u32,
    pub gfx_slot: u16,
    pub mode: i16,
}

/// Active detached objects record79: UID+3, gfxtype link+73. Follows the link,
/// never object-slot indexing. Caller still owns object-slot/UID provenance.
pub fn saved_object_gfxtype_mode(object: &[u8], gfx_payload: &[u8]) -> Option<SavedObjectGfxMode> {
    if object.len() != 79 || object[0] == 0 {
        return None;
    }
    let gfx_slot = u16::from_le_bytes([object[73], object[74]]);
    let mode = saved_gfxtype_mode_at(gfx_payload, gfx_slot as u32)?;
    let object_uid = u32::from_le_bytes(object[3..7].try_into().ok()?);
    Some(SavedObjectGfxMode {
        object_uid,
        gfx_slot,
        mode,
    })
}

/// gfxtype serialized record is u8+7*u16; mode is the separate parallel u16
/// word mapped by48c460 to runtime+10, NOT record+13 (runtime+e).
/// Requires caller-verified object->gfxtype link and exact detached buffers.
pub fn saved_gfxtype_mode(record: &[u8], mode_word: &[u8]) -> Option<i16> {
    if record.len() != 15 || mode_word.len() != 2 || record[0] == 0 {
        return None;
    }
    Some(i16::from_le_bytes([mode_word[0], mode_word[1]]))
}

/// Checked decoded gfxtype payload lookup. Product admission: version1, N<=14000,
/// exact8+17*N bytes. Caller supplies the verified object link, not object slot.
/// Raw payload only; PFIL must be decoded before this function.
pub fn saved_gfxtype_mode_at(payload: &[u8], slot: u32) -> Option<i16> {
    if payload.len() < 8 || u32::from_le_bytes(payload[..4].try_into().ok()?) != 1 {
        return None;
    }
    let count = u32::from_le_bytes(payload[4..8].try_into().ok()?) as usize;
    if count > 14000 || payload.len() != 8 + count * 17 || slot as usize >= count {
        return None;
    }
    let slot = slot as usize;
    let record = 8 + slot * 15;
    let mode = 8 + count * 15 + slot * 2;
    saved_gfxtype_mode(&payload[record..record + 15], &payload[mode..mode + 2])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedParticleMode {
    pub mode: i32,
    pub leaf_address: Option<u32>,
}

/// 4dcfc0 caller rule: signed gfx mode99 substitutes the PRIMARY GRAFIK OBJECT
/// type's signed mode once. Missing verified object context stays unresolved.
/// No fallback0, recursive substitution, owner-emitter substitution or draw claim.
pub fn resolve_particle_mode(
    gfx_mode: i16,
    primary_object_mode: Option<i16>,
) -> Option<ResolvedParticleMode> {
    let mode = if gfx_mode == 99 {
        primary_object_mode?
    } else {
        gfx_mode
    } as i32;
    Some(ResolvedParticleMode {
        mode,
        leaf_address: native_sprite_leaf_address(mode as u32),
    })
}

/// Static unsigned mode dispatch in 4b53c0, after its resource/owner gates.
/// Returns the leaf address only; it does not establish palette preparation,
/// visibility or a draw. Native modes are NOT pixel-row ABI modes.
pub fn native_sprite_leaf_address(mode: u32) -> Option<u32> {
    match mode {
        0 => Some(0x4ca010),
        1 | 5 | 6 => Some(0x4ca340),
        2 | 7 | 8 => Some(0x4cab80),
        3 => Some(0x4caf80),
        4 => Some(0x4cb380),
        9 => Some(0x4cbcc0),
        49 => Some(0x4ca740),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameContext {
    /// 80EFD4 + parameter_id*58; caller must establish the exact definition identity.
    pub definition_lifetime: i32,
    /// C648C0 + gfx_object_type*2A4, not the parameter lifetime.
    pub type_divisor: i32,
    /// 80EFFC + parameter_id*58: every nonzero selects the velocity branch.
    pub velocity_branch_raw: i32,
    /// 80EFEC + parameter_id*58: positive skips ordinal zero.
    pub skip_first_raw: i32,
    /// Resident asset 4AF200 / 4AF230 values, not unloaded defaults.
    pub columns: i32,
    pub directions: i32,
    pub frame_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameUnavailable {
    MissingContext,
    InvalidLayout,
    VelocityRoundingUnverified,
    DirectionRoundingUnverified,
    ZeroDivisor,
    DivisionOverflow,
    NegativeIndex,
    FrameOutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildFrameCandidate {
    pub ordinal: u32,
    /// Each saved coordinate uses signed SAR16 before renderer conversion.
    pub position_native: [i32; 3],
    pub velocity_high: [i32; 3],
    pub lifetime_raw: i32,
    /// Successful index computation only, not a statement that the native renderer draws it.
    pub frame: Result<u32, FrameUnavailable>,
}

/// Exact low32 SUB/IMUL and two signed CDQ/IDIV operations for the lifetime branch.
/// This does not supply animation fallback when the result is -1 or a default frame when unknown.
pub fn lifetime_frame(lifetime: i32, context: &FrameContext) -> Result<u32, FrameUnavailable> {
    if context.columns <= 0 || context.directions <= 0 || context.frame_count == 0 {
        return Err(FrameUnavailable::InvalidLayout);
    }
    if context.velocity_branch_raw != 0 {
        return Err(FrameUnavailable::VelocityRoundingUnverified);
    }
    if context.directions > 1 {
        return Err(FrameUnavailable::DirectionRoundingUnverified);
    }
    if context.type_divisor == 0 {
        return Err(FrameUnavailable::ZeroDivisor);
    }
    let numerator = context
        .definition_lifetime
        .wrapping_sub(lifetime)
        .wrapping_mul(context.columns);
    let quotient = numerator
        .checked_div(context.type_divisor)
        .ok_or(FrameUnavailable::DivisionOverflow)?;
    // columns is positive, so MIN/-1 cannot occur in the second IDIV.
    let index = quotient % context.columns;
    if index < 0 {
        return Err(FrameUnavailable::NegativeIndex);
    }
    if index as u32 >= context.frame_count {
        return Err(FrameUnavailable::FrameOutOfRange);
    }
    Ok(index as u32)
}

/// Identify count-bounded active ordinals. All saved snapshot bytes/links remain intact.
/// Missing context returns active candidates with an explicit unresolved frame (never guessed).
/// Count outside 0..64 rejects the whole emitter; inactive emitters yield no candidates.
pub fn saved_frame_candidates(
    snapshot: &ParticleSnapshot,
    context: Option<&FrameContext>,
) -> Result<Vec<ChildFrameCandidate>, crate::particle::ParticleError> {
    if snapshot.active_raw() == 0 {
        return Ok(Vec::new());
    }
    let count = snapshot.validate_renderable_count()?;
    let first = u32::from(context.is_some_and(|c| c.skip_first_raw > 0));
    let mut result = Vec::new();
    for ordinal in first..count.min(CHILDREN as u32) {
        let child = snapshot.child(ordinal)?;
        if child.active_raw() == 0 {
            continue;
        }
        result.push(ChildFrameCandidate {
            ordinal,
            position_native: child.position_fixed().map(|x| x >> 16),
            velocity_high: child.velocity_raw().map(|x| x >> 16),
            lifetime_raw: child.lifetime_raw(),
            frame: context.map_or(Err(FrameUnavailable::MissingContext), |c| {
                lifetime_frame(child.lifetime_raw(), c)
            }),
        });
    }
    Ok(result)
}
