use against_rome_data::{alr_scanline_ffi::*, alr_scanlines::*, error::code};
use std::ptr;

#[test]
fn indexed_ffi_flags_tail_and_late_failure_are_atomic() {
    let rows = [0x100000, 5]; let data = [1, 0x81, 0, 0, 1];
    let mut output = [0xaaaa; 7];
    unsafe {
        assert_eq!(ar_alr_decode_indexed(5, 1, rows.as_ptr(), 2, data.as_ptr(), 5, 2,
            output.as_mut_ptr(), 7), 0);
        assert_eq!(output, [0x100, 0, 0x300, 0x301, 0, 0xaaaa, 0xaaaa]);
        let before = output;
        assert_eq!(ar_alr_decode_indexed(5, 1, rows.as_ptr(), 2, data.as_ptr(), 5, 1,
            output.as_mut_ptr(), 7), INVALID_SCANLINE);
        assert_eq!(output, before);
        assert_eq!(ar_alr_decode_indexed(5, 1, rows.as_ptr(), 2, data.as_ptr(), 5, 2,
            output.as_mut_ptr(), 4), code::BUFFER_TOO_SMALL);
        assert_eq!(output, before);
        assert_eq!(ar_alr_decode_indexed(5, 1, ptr::null(), 2, data.as_ptr(), 5, 2,
            output.as_mut_ptr(), 7), code::NULL_ARGUMENT);
        assert_eq!(output, before);
    }
}

#[test]
fn indexed_samples_distinguish_absent_skipped_zero_and_opaque_zero() {
    let samples = decode_indexed(5, 1, &[0x100000, 5], &[1, 0x81, 0, 0, 1], 2).unwrap();
    assert_eq!(samples, vec![
        Some(IndexedSample { index: 0, write_zero: false }), None,
        Some(IndexedSample { index: 0, write_zero: true }),
        Some(IndexedSample { index: 1, write_zero: true }), None,
    ]);
    assert_eq!(decode_indexed(1, 1, &[0, 1], &[2], 2), Err(ScanlineError::Index));
    assert_eq!(decode_indexed(1, 1, &[0, 0], &[], 0), Err(ScanlineError::Palette));
}

#[test]
fn asymmetric_rows_and_segment_flags_match_fixed_editor_pixels() {
    let rows = [0x80000000 | (1 << 21) | 0x100000, (2 << 21) | 6, 9];
    let data = [2, 2, 0, 1, 0, 2, 2, 0, 1, 0xaa];
    let palette = [0x123456, 0xcc1122, 0x3344ee];
    assert_eq!(
        decode_argb(8, 2, &rows, &data, &palette).unwrap(),
        vec![
            0, 0xff563412, 0xff2211cc, 0, 0, 0, 0xffee4433, 0, 0, 0, 0xffee4433, 0, 0xff2211cc, 0,
            0, 0
        ]
    );
    assert_eq!(
        decode_argb(
            5,
            1,
            &[0x100000, 5],
            &[1, 0x81, 0, 0, 1],
            &[0xaa000000, 0xffffff]
        )
        .unwrap(),
        vec![0, 0, 0xff000000, 0xffffffff, 0]
    );
}

#[test]
fn invalid_rows_dimensions_prefix_palette_and_bounds_are_rejected() {
    for (row, end, bytes) in [
        (0x100000, 1, vec![1]),
        (0x100000, 3, vec![2, 0, 1]),
        (2, 1, vec![1, 1]),
        (0, 2, vec![1]),
        (0, 1, vec![2]),
        (3 << 21, 1, vec![1]),
        (0x100000, 4, vec![1, 2, 1, 1]),
    ] {
        assert!(decode_argb(2, 1, &[row, end], &bytes, &[0, 0x112233]).is_err());
    }
    assert_eq!(
        decode_argb(2048, 1, &[0, 0], &[], &[0]),
        Err(ScanlineError::Dimensions)
    );
    assert_eq!(
        decode_argb(1, 1, &[0], &[], &[0]),
        Err(ScanlineError::Descriptors)
    );
    assert_eq!(
        decode_argb(1, 1, &[0, 0], &[], &[]),
        Err(ScanlineError::Palette)
    );
    assert!(decode_argb(0, 1, &[0, 0], &[], &[0]).unwrap().is_empty());
    assert_eq!(
        decode_argb(2, 2, &[0, 0, 0], &[], &[0]).unwrap(),
        vec![0; 4]
    );
}

#[test]
fn ffi_late_error_is_atomic_and_success_preserves_tail() {
    assert_eq!(ar_alr_scanline_abi_version(), 1);
    let palette = [0x0000ff, 0x00ff00];
    let rows = [0x80000000, 1, 2];
    let mut output = [0xdeadbeef; 3];
    for (data, expected) in [([0, 2], INVALID_SCANLINE), ([0, 1], code::OK)] {
        assert_eq!(
            unsafe {
                ar_alr_decode_argb(
                    1,
                    2,
                    rows.as_ptr(),
                    3,
                    data.as_ptr(),
                    2,
                    palette.as_ptr(),
                    2,
                    output.as_mut_ptr(),
                    3,
                )
            },
            expected
        );
        if expected != 0 {
            assert_eq!(output, [0xdeadbeef; 3]);
        }
    }
    assert_eq!(output, [0xffff0000, 0xff00ff00, 0xdeadbeef]);
}

#[test]
fn ffi_empty_null_and_impossible_lengths_are_safe() {
    let row = [0];
    let palette = [0];
    assert_eq!(
        unsafe {
            ar_alr_decode_argb(
                0,
                0,
                row.as_ptr(),
                1,
                ptr::null(),
                0,
                palette.as_ptr(),
                1,
                ptr::null_mut(),
                0,
            )
        },
        code::OK
    );
    assert_eq!(
        unsafe {
            ar_alr_decode_argb(
                0,
                0,
                row.as_ptr(),
                1,
                ptr::null(),
                usize::MAX,
                palette.as_ptr(),
                1,
                ptr::null_mut(),
                0,
            )
        },
        code::BUFFER_TOO_SMALL
    );
    assert_eq!(
        unsafe {
            ar_alr_decode_argb(
                0,
                0,
                ptr::null(),
                1,
                ptr::null(),
                0,
                palette.as_ptr(),
                1,
                ptr::null_mut(),
                0,
            )
        },
        code::NULL_ARGUMENT
    );
}
