use against_rome_data::{error::code, particle_pixel_ffi::*};
use std::ptr;

#[test]
fn actual_abi_supports_in_place_composition_and_preserves_skipped_alpha() {
    assert_eq!(ar_particle_pixel_abi_version(), 1);
    let indices = [0, 1, 2];
    let palette = [0, 0xffffffff, 0x010305];
    let mut dst = [0xaabbccdd, 0xffffffff, 0xff030507];
    assert_eq!(
        unsafe {
            ar_particle_pixel_row(
                indices.as_ptr(),
                3,
                palette.as_ptr(),
                3,
                dst.as_ptr(),
                1,
                1,
                dst.as_mut_ptr(),
                3,
            )
        },
        code::OK
    );
    assert_eq!(dst, [0xaabbccdd, 0x00fefefe, 0x00010305]);
}

#[test]
fn late_invalid_index_and_bad_options_leave_entire_output_unchanged() {
    let indices = [1, 1, 2];
    let palette = [0, 0x12345678];
    let destination = [0; 3];
    let mut output = [0xdeadbeef; 4];
    for (mode, zero, capacity, expected) in [
        (0, 0, 4, INVALID_INDEX),
        (3, 0, 4, INVALID_MODE),
        (0, 2, 4, INVALID_MODE),
        (0, 0, 2, code::BUFFER_TOO_SMALL),
    ] {
        assert_eq!(
            unsafe {
                ar_particle_pixel_row(
                    indices.as_ptr(),
                    3,
                    palette.as_ptr(),
                    2,
                    destination.as_ptr(),
                    mode,
                    zero,
                    output.as_mut_ptr(),
                    capacity,
                )
            },
            expected
        );
        assert_eq!(output, [0xdeadbeef; 4]);
    }
}

#[test]
fn null_and_empty_buffers_and_length_overflow_are_checked_before_dereference() {
    let zeros = [0; 2];
    let mut output = [0xdeadbeef; 3];
    assert_eq!(
        unsafe {
            ar_particle_pixel_row(
                zeros.as_ptr(),
                2,
                ptr::null(),
                0,
                output.as_ptr(),
                0,
                1,
                output.as_mut_ptr(),
                3,
            )
        },
        code::OK
    );
    assert_eq!(output, [0xdeadbeef; 3]);
    assert_eq!(
        unsafe {
            ar_particle_pixel_row(
                ptr::null(),
                0,
                ptr::null(),
                0,
                ptr::null(),
                0,
                0,
                ptr::null_mut(),
                0,
            )
        },
        code::OK
    );
    assert_eq!(
        unsafe {
            ar_particle_pixel_row(
                ptr::null(),
                1,
                ptr::null(),
                0,
                ptr::null(),
                0,
                0,
                output.as_mut_ptr(),
                3,
            )
        },
        code::NULL_ARGUMENT
    );
    assert_eq!(
        unsafe {
            ar_particle_pixel_row(
                ptr::null(),
                usize::MAX,
                ptr::null(),
                0,
                ptr::null(),
                0,
                0,
                ptr::null_mut(),
                usize::MAX,
            )
        },
        code::BUFFER_TOO_SMALL
    );
    assert_eq!(output, [0xdeadbeef; 3]);
}

#[test]
fn output_can_overlap_palette_and_success_preserves_capacity_tail() {
    let indices = [1, 0];
    let destination = [0; 2];
    let mut palette = [0x11223344, 0xaabbccdd, 0xdeadbeef];
    assert_eq!(
        unsafe {
            ar_particle_pixel_row(
                indices.as_ptr(),
                2,
                palette.as_ptr(),
                3,
                destination.as_ptr(),
                0,
                0,
                palette.as_mut_ptr(),
                3,
            )
        },
        code::OK
    );
    assert_eq!(palette, [0xaabbccdd, 0x11223344, 0xdeadbeef]);
}

#[test]
fn explicit_alpha256_ffi_preserves_raw_four_lane_values_and_zero_policy() {
    let indices = [0u8, 1];
    let palette = [u32::MAX, 0x80ff0000];
    let destination = [0u32, u32::MAX];
    for (skip, first) in [(0, 0xfefefefe), (1, 0)] {
        let mut output = [0xdeadbeef; 3];
        assert_eq!(
            unsafe {
                ar_particle_pixel_row(
                    indices.as_ptr(),
                    2,
                    palette.as_ptr(),
                    2,
                    destination.as_ptr(),
                    2,
                    skip,
                    output.as_mut_ptr(),
                    3,
                )
            },
            0
        );
        assert_eq!(output, [first, 0xbfff7f7f, 0xdeadbeef]);
    }
}
