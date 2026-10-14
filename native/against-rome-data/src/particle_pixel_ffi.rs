//! Atomic caller-buffer ABI for selected indexed spans. This does not select native frames.
use crate::{
    error::code,
    particle_pixels::{palette_kernel_40e5c0, PaletteKernel40e5c0, blit_indexed_samples, indexed_pixel_row, indexed_sample_pixels, PixelError, IndexZeroPolicy, PixelWriteMode},
};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    ptr, slice,
};

pub const INVALID_MODE: i32 = 200;
pub const INVALID_INDEX: i32 = 201;
pub const INVALID_SAMPLE: i32 = 202;

/// Four signed output words; failed selection leaves output unchanged.
/// # Safety
/// output must point to four writable aligned i32 values.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_palette_selection(variants: i32, first: i32,
    second: i32, weight: i32, output: *mut i32) -> i32 {
    if output.is_null() { return code::NULL_ARGUMENT; }
    let Some(plan) = crate::particle_pixels::palette_selection_4ca340(variants, first, second, weight)
        else { return 206; };
    ptr::copy_nonoverlapping(plan.as_ptr(), output, 4); code::OK
}

/// Explicit disk-default endpoint profile; mod8=8 denotes unverified layout.
/// # Safety
/// Aligned readable/writable u32 regions for count/capacity; aliasing allowed.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_primary_endpoint_default(source: *const u32, count: usize,
    key: u32, low: u32, middle: u32, high: u32, tint: u32, alpha_flag: u32,
    buffer_mod8: u32, output: *mut u32, capacity: usize) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if !(1..=256).contains(&count) { return 204; }
        if capacity > isize::MAX as usize / 4 || capacity < count { return code::BUFFER_TOO_SMALL; }
        if source.is_null() || output.is_null() { return code::NULL_ARGUMENT; }
        let result = match crate::particle_pixels::primary_endpoint_default(slice::from_raw_parts(source, count),
            key, low, middle, high, tint, alpha_flag, if buffer_mod8 == 8 { None } else { Some(buffer_mod8) }) {
            Ok(result) => result, Err(crate::particle_pixels::PixelError::UnverifiedAlphaLayout) => return 205,
            Err(_) => return 204,
        };
        ptr::copy_nonoverlapping(result.as_ptr(), output, count); code::OK
    })).unwrap_or(code::PANIC)
}

/// Explicit independent DWORD alpha lanes, not native40186a buffer semantics.
/// # Safety
/// Count readable/writable aligned u32 words; output may alias source.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_palette_alpha_lanes(source: *const u32, count: usize,
    output: *mut u32, capacity: usize) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if !(1..=256).contains(&count) { return 204; }
        if capacity > isize::MAX as usize / 4 || capacity < count { return code::BUFFER_TOO_SMALL; }
        if source.is_null() || output.is_null() { return code::NULL_ARGUMENT; }
        let result: Vec<u32> = slice::from_raw_parts(source, count).iter().map(|&word|
            crate::particle_pixels::palette_alpha_lane_40186a(word)).collect();
        ptr::copy_nonoverlapping(result.as_ptr(), output, count);
        code::OK
    })).unwrap_or(code::PANIC)
}

/// Explicit disk-default40fd50 helper only; not automatic endpoint routing.
/// # Safety
/// Word regions must be aligned/readable/writable. Output may alias source.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_primary_palette_default(source: *const u32, count: usize,
    packed_gain: u32, output: *mut u32, capacity: usize) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if !(1..=256).contains(&count) { return 204; }
        if capacity > isize::MAX as usize / 4 || capacity < count { return code::BUFFER_TOO_SMALL; }
        if source.is_null() || output.is_null() { return code::NULL_ARGUMENT; }
        let result: Vec<u32> = slice::from_raw_parts(source, count).iter().map(|&word|
            crate::particle_pixels::primary_palette_default_40fd50(word, packed_gain)).collect();
        ptr::copy_nonoverlapping(result.as_ptr(), output, count);
        code::OK
    })).unwrap_or(code::PANIC)
}

/// Explicit supplied-buffer 40e5c0 kernel, not native endpoint dispatch.
/// Count1..256; raw arguments retain native bit semantics. Atomic output/tail.
/// # Safety
/// Declared u32 regions must be aligned/readable/writable. Output may alias
/// inputs; no concurrent mutation or retained pointers.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_palette_kernel(a: *const u32, a_len: usize,
    b: *const u32, b_len: usize, count: usize, weight: i32,
    low: u32, middle: u32, high: u32, tint: u32, alpha_flag: u32,
    output: *mut u32, capacity: usize) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if !(1..=256).contains(&count) || a_len < count || b_len < count { return 204; }
        if a_len > isize::MAX as usize / 4 || b_len > isize::MAX as usize / 4
            || capacity > isize::MAX as usize / 4 || capacity < count { return code::BUFFER_TOO_SMALL; }
        if a.is_null() || b.is_null() || output.is_null() { return code::NULL_ARGUMENT; }
        let result = match palette_kernel_40e5c0(slice::from_raw_parts(a, a_len),
            slice::from_raw_parts(b, b_len), count, PaletteKernel40e5c0 {
                weight, gain_low: low, gain_middle: middle, gain_high: high, tint, alpha_flag,
            }) { Ok(result) => result, Err(_) => return 204 };
        ptr::copy_nonoverlapping(result.as_ptr(), output, count);
        code::OK
    })).unwrap_or(code::PANIC)
}

/// Atomic unscaled/clipped raw framebuffer blit. Source/destination sizes must
/// exactly match their declared element counts. Kernel IDs0/1/2 are explicit.
/// # Safety
/// Declared regions must be readable/writable and word-aligned; output may
/// overlap inputs. No concurrent mutation or retained pointers.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_sample_blit(samples: *const u16, sample_count: usize,
    sw: u32, sh: u32, palette: *const u32, palette_len: usize,
    destination: *const u32, destination_count: usize, dw: u32, dh: u32,
    x: i32, y: i32, mode: u32, output: *mut u32, capacity: usize) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let mode = match mode { 0 => PixelWriteMode::Replacement, 1 => PixelWriteMode::HalfAverage,
            2 => PixelWriteMode::PaletteAlpha256, _ => return INVALID_MODE };
        if sw > 2047 || sh > 2047 || (sw as usize).checked_mul(sh as usize) != Some(sample_count)
            || dw > i32::MAX as u32 || dh > i32::MAX as u32
            || (dw as usize).checked_mul(dh as usize) != Some(destination_count) { return 203; }
        if palette_len > isize::MAX as usize / 4 || destination_count > isize::MAX as usize / 4
            || capacity > isize::MAX as usize / 4 || capacity < destination_count { return code::BUFFER_TOO_SMALL; }
        if (sample_count != 0 && samples.is_null()) || (palette_len != 0 && palette.is_null())
            || (destination_count != 0 && (destination.is_null() || output.is_null())) { return code::NULL_ARGUMENT; }
        let s = if sample_count == 0 { &[] } else { slice::from_raw_parts(samples, sample_count) };
        let p = if palette_len == 0 { &[] } else { slice::from_raw_parts(palette, palette_len) };
        let d = if destination_count == 0 { &[] } else { slice::from_raw_parts(destination, destination_count) };
        let result = match blit_indexed_samples(s, sw as usize, sh as usize, p, d, dw as usize, dh as usize, x, y, mode) {
            Ok(result) => result, Err(PixelError::InvalidSample { .. }) => return INVALID_SAMPLE,
            Err(PixelError::RasterDimensions) => return 203, Err(_) => return INVALID_INDEX,
        };
        if destination_count != 0 { ptr::copy_nonoverlapping(result.as_ptr(), output, destination_count); }
        code::OK
    })).unwrap_or(code::PANIC)
}

/// Composes packed samples (present bit8/write-zero bit9/index low8) atomically.
/// Explicit kernel modes0/1/2; absent and skipped-zero keep raw destination.
/// Empty count accepts null. Success writes count words; output tail untouched.
/// # Safety
/// Inputs must be readable, output writable and all word pointers aligned.
/// Output may alias inputs; no concurrent mutation or retained pointers.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_sample_pixels(samples: *const u16, count: usize,
    palette: *const u32, palette_len: usize, destination: *const u32, mode: u32,
    output: *mut u32, output_capacity: usize) -> i32
{
    catch_unwind(AssertUnwindSafe(|| {
        let mode = match mode { 0 => PixelWriteMode::Replacement, 1 => PixelWriteMode::HalfAverage,
            2 => PixelWriteMode::PaletteAlpha256, _ => return INVALID_MODE };
        if count > isize::MAX as usize / 4 || palette_len > isize::MAX as usize / 4
            || output_capacity > isize::MAX as usize / 4 || output_capacity < count {
            return code::BUFFER_TOO_SMALL;
        }
        if count == 0 { return code::OK; }
        if samples.is_null() || destination.is_null() || output.is_null()
            || (palette_len != 0 && palette.is_null()) { return code::NULL_ARGUMENT; }
        let colors = if palette_len == 0 { &[] } else { slice::from_raw_parts(palette, palette_len) };
        let result = match indexed_sample_pixels(slice::from_raw_parts(samples, count), colors,
            slice::from_raw_parts(destination, count), mode) {
            Ok(result) => result, Err(PixelError::InvalidSample { .. }) => return INVALID_SAMPLE,
            Err(_) => return INVALID_INDEX,
        };
        ptr::copy_nonoverlapping(result.as_ptr(), output, count);
        code::OK
    })).unwrap_or(code::PANIC)
}

#[no_mangle]
pub extern "C" fn ar_particle_pixel_abi_version() -> u32 {
    1
}

/// Writes exactly `count` packed words after validating the whole span.
/// `mode`: 0 replacement, 1 half-average, 2 raw palette-alpha/256; `skip_zero`: 0 always write, 1 skip zero.
/// Lengths and capacity are element counts. Empty buffers may be null.
/// Output may alias destination or palette: the complete result is staged before writing.
/// Errors leave output unchanged, including late invalid palette indices.
///
/// # Safety
/// Nonempty inputs must be valid readable regions for their lengths. Output must be
/// writable for `output_capacity` u32 words. u32 pointers must be aligned. No concurrent
/// mutations are allowed. The caller owns all storage; no pointers are retained.
#[no_mangle]
pub unsafe extern "C" fn ar_particle_pixel_row(
    indices: *const u8,
    count: usize,
    palette: *const u32,
    palette_len: usize,
    destination: *const u32,
    mode: u32,
    skip_zero: u32,
    output: *mut u32,
    output_capacity: usize,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let mode = match mode {
            0 => PixelWriteMode::Replacement,
            1 => PixelWriteMode::HalfAverage,
            2 => PixelWriteMode::PaletteAlpha256,
            _ => return INVALID_MODE,
        };
        let zero = match skip_zero {
            0 => IndexZeroPolicy::AlwaysWrite,
            1 => IndexZeroPolicy::SkipZero,
            _ => return INVALID_MODE,
        };
        if count > isize::MAX as usize / 4
            || palette_len > isize::MAX as usize / 4
            || output_capacity > isize::MAX as usize / 4
            || output_capacity < count
        {
            return code::BUFFER_TOO_SMALL;
        }
        if count == 0 {
            return code::OK;
        }
        if indices.is_null()
            || destination.is_null()
            || output.is_null()
            || (palette_len != 0 && palette.is_null())
        {
            return code::NULL_ARGUMENT;
        }
        let colors = if palette_len == 0 {
            &[]
        } else {
            slice::from_raw_parts(palette, palette_len)
        };
        // All immutable borrows end before copying, so in-place/overlapping output is allowed.
        let result = match indexed_pixel_row(
            slice::from_raw_parts(indices, count),
            colors,
            slice::from_raw_parts(destination, count),
            zero,
            mode,
        ) {
            Ok(result) => result,
            Err(_) => return INVALID_INDEX,
        };
        ptr::copy_nonoverlapping(result.as_ptr(), output, count);
        code::OK
    }))
    .unwrap_or(code::PANIC)
}
