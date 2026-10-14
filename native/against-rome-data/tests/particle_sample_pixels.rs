use against_rome_data::{alr_scanlines::decode_indexed, particle_pixels::*, particle_pixel_ffi::*};

#[test]
fn clipped_blit_places_rows_and_preserves_outside_destination() {
    let samples = [0x301, 0x300, 0x100, 0x301]; let palette = [0x01020304, 0x80ffffff];
    let destination = [0x11223344; 9];
    assert_eq!(blit_indexed_samples(&samples, 2, 2, &palette, &destination, 3, 3,
        -1, 1, PixelWriteMode::Replacement).unwrap(),
        vec![0x11223344, 0x11223344, 0x11223344, palette[0], 0x11223344, 0x11223344,
            palette[1], 0x11223344, 0x11223344]);
    for (x,y) in [(i32::MIN,0), (i32::MAX,0), (0,i32::MIN), (0,i32::MAX)] {
        assert_eq!(blit_indexed_samples(&samples, 2, 2, &palette, &destination, 3, 3,
            x, y, PixelWriteMode::HalfAverage).unwrap(), destination);
    }
    let bad = [0x301, 0x300, 0x100, 0x400];
    assert!(blit_indexed_samples(&bad, 2, 2, &palette, &destination, 3, 3,
        i32::MIN, 0, PixelWriteMode::Replacement).is_err());
    assert_eq!(blit_indexed_samples(&samples, 3, 2, &palette, &destination, 3, 3,
        0, 0, PixelWriteMode::Replacement), Err(PixelError::RasterDimensions));
}

#[test]
fn clipped_blit_blends_against_the_correct_destination_cell() {
    let destination = [0, 0x11223344, 0x55667788, 0xaabbccdd]; let palette = [0x80abcdef];
    for mode in [PixelWriteMode::HalfAverage, PixelWriteMode::PaletteAlpha256] {
        let result = blit_indexed_samples(&[0x300], 1, 1, &palette, &destination, 2, 2, 1, 1, mode).unwrap();
        assert_eq!(&result[..3], &destination[..3]);
        assert_eq!(result[3], match mode {
            PixelWriteMode::HalfAverage => half_average(palette[0], destination[3]),
            _ => palette_alpha_256(palette[0], destination[3]),
        });
    }
}

#[test]
fn decoded_segments_compose_without_erasing_absent_or_skipped_pixels() {
    let decoded = decode_indexed(5, 1, &[0x100000, 5], &[1, 0x81, 0, 0, 1], 2).unwrap();
    let samples: Vec<u16> = decoded.into_iter().map(|s| s.map_or(0, |s|
        0x100 | (u16::from(s.write_zero) << 9) | u16::from(s.index))).collect();
    let palette = [0xabcdef01, 0x80ffffff]; let destination = [0x11223344; 5];
    for mode in [PixelWriteMode::Replacement, PixelWriteMode::HalfAverage, PixelWriteMode::PaletteAlpha256] {
        let result = indexed_sample_pixels(&samples, &palette, &destination, mode).unwrap();
        assert_eq!([result[0], result[1], result[4]], [0x11223344; 3]);
        for (position, src) in [(2, palette[0]), (3, palette[1])] {
            assert_eq!(result[position], match mode {
                PixelWriteMode::Replacement => src,
                PixelWriteMode::HalfAverage => half_average(src, destination[position]),
                PixelWriteMode::PaletteAlpha256 => palette_alpha_256(src, destination[position]),
            });
        }
    }
}

#[test]
fn ffi_in_place_tail_and_late_errors_preserve_storage() {
    let samples = [0, 0x100, 0x300, 0x301]; let palette = [0x12345678, 0x80ffffff];
    let mut output = [0x11223344; 6];
    unsafe {
        assert_eq!(ar_particle_sample_pixels(samples.as_ptr(), 4, palette.as_ptr(), 2,
            output.as_ptr(), 0, output.as_mut_ptr(), 6), 0);
        assert_eq!(output, [0x11223344, 0x11223344, palette[0], palette[1], 0x11223344, 0x11223344]);
        let before = output;
        for (bad, code) in [([0, 0x100, 0x300, 0x302], INVALID_INDEX),
            ([0, 0x100, 0x300, 0x400], INVALID_SAMPLE), ([0, 0x100, 0x300, 1], INVALID_SAMPLE)] {
            assert_eq!(ar_particle_sample_pixels(bad.as_ptr(), 4, palette.as_ptr(), 2,
                output.as_ptr(), 1, output.as_mut_ptr(), 6), code);
            assert_eq!(output, before);
        }
    }
}
