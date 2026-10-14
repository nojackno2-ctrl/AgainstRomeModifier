use crate::{alr_scanlines::{decode_argb, decode_indexed}, error::code};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    ptr, slice,
};

pub const INVALID_SCANLINE: i32 = 210;

/// Packed u16 samples: absent=0, present bit8, write-zero bit9, index bits0..7.
/// Errors leave output untouched; success writes width*height elements only.
/// # Safety
/// Inputs must be readable for declared lengths, output writable and aligned.
/// Output may overlap input; no concurrent input mutation or retained pointers.
#[no_mangle]
pub unsafe extern "C" fn ar_alr_decode_indexed(
    width: u32, height: u32, rows: *const u32, row_count: usize,
    data: *const u8, data_len: usize, palette_len: usize,
    output: *mut u16, output_capacity: usize,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if width > 2047 || height > 2047 || row_count != height as usize + 1
            || !(1..=511).contains(&palette_len) { return INVALID_SCANLINE; }
        let count = width as usize * height as usize;
        if data_len > isize::MAX as usize || output_capacity > isize::MAX as usize / 2
            || output_capacity < count { return code::BUFFER_TOO_SMALL; }
        if rows.is_null() || (data_len != 0 && data.is_null())
            || (count != 0 && output.is_null()) { return code::NULL_ARGUMENT; }
        let bytes = if data_len == 0 { &[] } else { slice::from_raw_parts(data, data_len) };
        let samples = match decode_indexed(width as usize, height as usize,
            slice::from_raw_parts(rows, row_count), bytes, palette_len) {
            Ok(samples) => samples, Err(_) => return INVALID_SCANLINE,
        };
        let words: Vec<u16> = samples.into_iter().map(|sample| match sample {
            None => 0, Some(sample) => 0x100 | (u16::from(sample.write_zero) << 9) | u16::from(sample.index),
        }).collect();
        if count != 0 { ptr::copy_nonoverlapping(words.as_ptr(), output, count); }
        code::OK
    })).unwrap_or(code::PANIC)
}
#[no_mangle]
pub extern "C" fn ar_alr_scanline_abi_version() -> u32 {
    1
}

/// Decodes an explicitly supplied frame to editor ARGB with atomic caller-buffer output.
/// Output has width*height words; output tail is untouched. No frame selection or native blending.
/// Null is accepted only for empty data/output. All lengths are element counts.
///
/// # Safety
/// Input regions must be readable and output writable for their declared lengths; u32 pointers
/// must be aligned. Output may overlap inputs because the full result is staged first.
/// No concurrent input mutation; no pointers retained. Errors leave output unchanged.
#[no_mangle]
pub unsafe extern "C" fn ar_alr_decode_argb(
    width: u32,
    height: u32,
    rows: *const u32,
    row_count: usize,
    data: *const u8,
    data_len: usize,
    palette: *const u32,
    palette_len: usize,
    output: *mut u32,
    output_capacity: usize,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if width > 2047
            || height > 2047
            || row_count != height as usize + 1
            || !(1..=511).contains(&palette_len)
        {
            return INVALID_SCANLINE;
        }
        let count = width as usize * height as usize;
        if data_len > isize::MAX as usize
            || output_capacity > isize::MAX as usize / 4
            || output_capacity < count
        {
            return code::BUFFER_TOO_SMALL;
        }
        if rows.is_null()
            || palette.is_null()
            || (data_len != 0 && data.is_null())
            || (count != 0 && output.is_null())
        {
            return code::NULL_ARGUMENT;
        }
        let bytes = if data_len == 0 {
            &[]
        } else {
            slice::from_raw_parts(data, data_len)
        };
        let result = match decode_argb(
            width as usize,
            height as usize,
            slice::from_raw_parts(rows, row_count),
            bytes,
            slice::from_raw_parts(palette, palette_len),
        ) {
            Ok(result) => result,
            Err(_) => return INVALID_SCANLINE,
        };
        if count != 0 {
            ptr::copy_nonoverlapping(result.as_ptr(), output, count);
        }
        code::OK
    }))
    .unwrap_or(code::PANIC)
}
