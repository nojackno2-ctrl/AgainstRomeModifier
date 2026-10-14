use against_rome_data::particle_pixels::{
    half_average, indexed_pixel_row, palette_kernel_40e5c0, IndexZeroPolicy::*,
    PaletteKernel40e5c0, PixelError, PixelWriteMode::*,
};

const UNITY: PaletteKernel40e5c0 = PaletteKernel40e5c0 {
    weight: 0,
    gain_low: 256,
    gain_middle: 256,
    gain_high: 256,
    tint: 0,
    alpha_flag: 0,
};

fn entry(a: u32, b: u32, args: PaletteKernel40e5c0) -> u32 {
    palette_kernel_40e5c0(&[0xdedcabcd, a], &[0, b], 2, args).unwrap()[1]
}

#[test]
fn first_entry_is_raw_a_with_all_modifiers_and_count_one() {
    let args = PaletteKernel40e5c0 {
        weight: i32::MIN,
        gain_low: u32::MAX,
        gain_middle: u32::MAX,
        gain_high: u32::MAX,
        tint: u32::MAX,
        alpha_flag: u32::MAX,
    };
    assert_eq!(
        palette_kernel_40e5c0(&[0xabcdef01], &[0x12345678], 1, args),
        Ok(vec![0xabcdef01])
    );
    assert_eq!(
        palette_kernel_40e5c0(&[0xabcdef01, 0], &[0x12345678, 0], 2, args).unwrap()[0],
        0xabcdef01
    );
}

#[test]
fn hand_vectors_weight_endpoints_fraction_and_source_alpha() {
    assert_eq!(entry(0xaa112233, 0xbb446688, UNITY), 0x00112233);
    assert_eq!(
        entry(
            0xaa112233,
            0xbb446688,
            PaletteKernel40e5c0 {
                weight: 256,
                ..UNITY
            }
        ),
        0x00446688
    );
    // (17+68)/2=42, (34+102)/2=68, (51+136)/2=93, fractions discarded.
    assert_eq!(
        entry(
            0xaa112233,
            0xbb446688,
            PaletteKernel40e5c0 {
                weight: 128,
                ..UNITY
            }
        ),
        0x002a445d
    );
    assert_eq!(
        entry(
            0,
            0x010101,
            PaletteKernel40e5c0 {
                weight: 128,
                ..UNITY
            }
        ),
        0
    );
    // No caller weight clamp: extrapolation to twice B.
    assert_eq!(
        entry(
            0,
            0x010203,
            PaletteKernel40e5c0 {
                weight: 512,
                ..UNITY
            }
        ),
        0x020406
    );
}

#[test]
fn hand_vectors_signed_weight_and_wrapping_complement() {
    // B low=1: -1 gives ffffffff, MIN gives 80000000, MAX gives 7fffffff.
    // Logical SHR8 / IMUL256 / logical SHR8 preserve the following cross-byte bits.
    for (weight, expected) in [
        (-1, 0x00ffffff),
        (i32::MIN, 0x00800000),
        (i32::MAX, 0x007fffff),
    ] {
        assert_eq!(
            entry(0, 1, PaletteKernel40e5c0 { weight, ..UNITY }),
            expected
        );
    }
    // A low=1: wrap(256-MIN)=80000100; wrap(256-MAX)=80000101.
    for weight in [i32::MIN, i32::MAX] {
        assert_eq!(
            entry(1, 0, PaletteKernel40e5c0 { weight, ..UNITY }),
            0x00800001
        );
    }
    assert_eq!(
        entry(
            1,
            0,
            PaletteKernel40e5c0 {
                weight: -1,
                ..UNITY
            }
        ),
        1
    );
}

#[test]
fn hand_vectors_raw_gain_masks_and_multiply_overflow() {
    // 255*ffffffff wraps to ffffff01; SHR8 has no low-byte clamp.
    assert_eq!(
        entry(
            255,
            0,
            PaletteKernel40e5c0 {
                gain_low: u32::MAX,
                ..UNITY
            }
        ),
        0x00ffffff
    );
    assert_eq!(
        entry(
            1,
            0,
            PaletteKernel40e5c0 {
                gain_low: 0x80000000,
                ..UNITY
            }
        ),
        0x00800000
    );
    assert_eq!(
        entry(
            2,
            0,
            PaletteKernel40e5c0 {
                gain_low: 0x80000000,
                ..UNITY
            }
        ),
        0
    );
    // 255*01010101=ffffffff; raw gain is not truncated to a byte or saturated.
    assert_eq!(
        entry(
            255,
            0,
            PaletteKernel40e5c0 {
                gain_low: 0x01010101,
                ..UNITY
            }
        ),
        0x00ffffff
    );
    assert_eq!(
        entry(
            0x010101,
            0,
            PaletteKernel40e5c0 {
                gain_low: 512,
                gain_middle: 128,
                gain_high: 768,
                ..UNITY
            }
        ),
        0x030002
    );
    // Middle: 100*ffffffff=ffffff00, SHR8 & ff00 = ff00.
    // High: 100*ffffffff & ff0000 = ff0000.
    assert_eq!(
        entry(
            0x010100,
            0,
            PaletteKernel40e5c0 {
                gain_middle: u32::MAX,
                gain_high: u32::MAX,
                ..UNITY
            }
        ),
        0x00ffff00
    );
}

#[test]
fn no_alpha_clamp_and_alpha_synthesis_uses_result_bytes() {
    let args = PaletteKernel40e5c0 {
        gain_low: u32::MAX,
        ..UNITY
    };
    // ffffff + ff00 + ff0000 = 01fffeff; alpha disabled retains arithmetic carry.
    assert_eq!(entry(0xffffff, 0, args), 0x01fffeff);
    // (255 + 2*254 + 255)/4=254, replacing the carried high byte.
    for alpha_flag in [1, 0x80000000, u32::MAX] {
        assert_eq!(
            entry(0xffffff, 0, PaletteKernel40e5c0 { alpha_flag, ..args }),
            0xfefffeff
        );
    }
    assert_eq!(
        entry(
            0xffffff,
            0,
            PaletteKernel40e5c0 {
                alpha_flag: 1,
                ..UNITY
            }
        ),
        0xffffffff
    );
}

#[test]
fn tint_is_after_gain_before_alpha_and_raw_nonzero_gate() {
    // Blue ff and red ff become 7f007f. Its weighted alpha is 63, not 127.
    assert_eq!(
        entry(
            0x0000ff,
            0,
            PaletteKernel40e5c0 {
                tint: 0xffff0000,
                alpha_flag: 1,
                ..UNITY
            }
        ),
        0x3f7f007f
    );
    // High-only tint is nonzero, so it still halves RGB although masked tint RGB=0.
    assert_eq!(
        entry(
            0xffffff,
            0,
            PaletteKernel40e5c0 {
                tint: 0xff000000,
                alpha_flag: 1,
                ..UNITY
            }
        ),
        0x7f7f7f7f
    );
    assert_eq!(
        entry(
            1,
            0,
            PaletteKernel40e5c0 {
                gain_low: 512,
                tint: 0x000002,
                alpha_flag: 1,
                ..UNITY
            }
        ),
        2
    );
    assert_eq!(
        entry(
            0xffffff,
            0,
            PaletteKernel40e5c0 {
                tint: u32::MAX,
                ..UNITY
            }
        ),
        0x00fefefe
    );
}

#[test]
fn exhaustive_single_channel_half_average_65536_pairs() {
    for src in 0..=255u32 {
        for dst in 0..=255u32 {
            // Independent arithmetic oracle: divide each operand before adding.
            let expected = src / 2 + dst / 2;
            for shift in [0, 8, 16] {
                assert_eq!(
                    half_average(0xa5000000 | (src << shift), 0x5a000000 | (dst << shift)),
                    expected << shift
                );
            }
        }
    }
    assert_eq!(half_average(0xffffffff, 0xffffffff), 0x00fefefe);
    assert_eq!(half_average(0x00010101, 0x00010101), 0);
    assert_eq!(half_average(0, 0), 0);
}

#[test]
fn replacement_and_half_rows_have_distinct_alpha_and_zero_semantics() {
    let indices = [0, 1, 2];
    let palette = [0xaaffffff, 0xbb010305, 0xccffffff];
    let destination = [0xdeadbeef, 0xdd030507, 0xeeffffff];
    let before = (indices, palette, destination);
    assert_eq!(
        indexed_pixel_row(&indices, &palette, &destination, AlwaysWrite, Replacement).unwrap(),
        palette
    );
    assert_eq!(
        indexed_pixel_row(&indices, &palette, &destination, SkipZero, Replacement).unwrap(),
        [0xdeadbeef, 0xbb010305, 0xccffffff]
    );
    assert_eq!(
        indexed_pixel_row(&indices, &palette, &destination, SkipZero, HalfAverage).unwrap(),
        [0xdeadbeef, 0x00010305, 0x00fefefe]
    );
    assert_eq!(
        indexed_pixel_row(&[0], &[0xffffffff], &[0xffffffff], AlwaysWrite, HalfAverage).unwrap(),
        [0x00fefefe]
    );
    assert_eq!((indices, palette, destination), before);
}

#[test]
fn all_skipped_zeros_need_no_palette_and_empty_rows_are_valid() {
    for mode in [Replacement, HalfAverage] {
        assert_eq!(
            indexed_pixel_row(&[0, 0], &[], &[0xabcdef01, 0xdeadbeef], SkipZero, mode).unwrap(),
            [0xabcdef01, 0xdeadbeef]
        );
        assert_eq!(
            indexed_pixel_row(&[], &[], &[], AlwaysWrite, mode).unwrap(),
            Vec::<u32>::new()
        );
        assert_eq!(
            indexed_pixel_row(&[0], &[], &[42], AlwaysWrite, mode),
            Err(PixelError::PaletteIndexOutOfRange {
                position: 0,
                index: 0,
                palette_len: 0
            })
        );
    }
}

#[test]
fn count_and_buffer_rejections_are_explicit_and_immutable() {
    let a = [0xabcdef01, 0x11223344];
    let b = [0x55667788, 0xdeadbeef];
    let before = (a, b);
    for supplied_count in [0, 257, usize::MAX] {
        assert_eq!(
            palette_kernel_40e5c0(&a, &b, supplied_count, UNITY),
            Err(PixelError::InvalidPaletteCount { supplied_count })
        );
    }
    for (buffer, aa, bb) in [("A", &a[..1], &b[..]), ("B", &a[..], &b[..1])] {
        assert_eq!(
            palette_kernel_40e5c0(aa, bb, 2, UNITY),
            Err(PixelError::InsufficientPaletteBuffer {
                buffer,
                required: 2,
                actual: 1
            })
        );
    }
    assert_eq!(
        palette_kernel_40e5c0(&a[..1], &[], 1, UNITY),
        Err(PixelError::InsufficientPaletteBuffer {
            buffer: "B",
            required: 1,
            actual: 0
        })
    );
    assert_eq!((a, b), before);
}

#[test]
fn count_256_and_index_255_are_supported_and_output_is_independent() {
    let a: Vec<u32> = (0..256).map(|x| 0xab000000 | x).collect();
    let b = vec![0; 256];
    let before = (a.clone(), b.clone());
    let mut output = palette_kernel_40e5c0(&a, &b, 256, UNITY).unwrap();
    assert_eq!(output.len(), 256);
    assert_eq!(output[0], 0xab000000);
    assert_eq!(output[255], 255);
    assert_eq!(
        indexed_pixel_row(&[255], &a, &[0], AlwaysWrite, Replacement).unwrap(),
        [0xab0000ff]
    );
    output[0] = 0;
    assert_eq!((a, b), before);
}

#[test]
fn late_bad_index_rejects_whole_row_without_changing_any_input() {
    let indices = [1, 0, 2];
    let palette = [0xabcdef01, 0xffffffff];
    let destination = [0x12345678, 0xdeadbeef, 0x87654321];
    let before = (indices, palette, destination);
    for zero_policy in [AlwaysWrite, SkipZero] {
        for mode in [Replacement, HalfAverage] {
            assert_eq!(
                indexed_pixel_row(&indices, &palette, &destination, zero_policy, mode),
                Err(PixelError::PaletteIndexOutOfRange {
                    position: 2,
                    index: 2,
                    palette_len: 2
                })
            );
            assert_eq!(
                indexed_pixel_row(&indices, &palette, &destination[..2], zero_policy, mode),
                Err(PixelError::SpanLengthMismatch {
                    indices: 3,
                    destination: 2
                })
            );
        }
    }
    assert_eq!((indices, palette, destination), before);
}

#[cfg(target_arch = "x86_64")]
#[test]
fn palette_alpha_matches_actual_mmx_instructions_for_all_alphas_and_random_words() {
    use against_rome_data::particle_pixels::palette_alpha_256;
    fn mmx(source: u32, destination: u32) -> u32 {
        let weight = (source >> 24) as u64 * 0x0001000100010001;
        let result: u32;
        unsafe {
            std::arch::asm!(
                "pxor mm7, mm7", "movd mm0, {source:e}", "movd mm1, {destination:e}",
                "movq mm4, qword ptr [{weight}]", "punpcklbw mm0, mm7", "punpcklbw mm1, mm7",
                "pmullw mm0, mm4", "movq mm2, mm1", "pmullw mm1, mm4",
                "psllw mm2, 8", "psubw mm2, mm1", "paddw mm0, mm2", "psrlw mm0, 8",
                "packuswb mm0, mm0", "movd {result:e}, mm0", "emms",
                source = in(reg) source, destination = in(reg) destination,
                weight = in(reg) &weight, result = out(reg) result, options(nostack));
        }
        result
    }
    let mut count = 0;
    for alpha in 0..256u32 {
        for rgb in [0, 0x010101, 0x7f80fe, 0xffffff] {
            for destination in [0, 0x01020304, 0x7f80feff, u32::MAX] {
                let source = alpha << 24 | rgb;
                assert_eq!(
                    palette_alpha_256(source, destination),
                    mmx(source, destination)
                );
                count += 1;
            }
        }
    }
    let mut random = 0x12345678u32;
    for _ in 0..32768 {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        let source = random;
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        assert_eq!(palette_alpha_256(source, random), mmx(source, random));
        count += 1;
    }
    assert_eq!(count, 36864);
    assert_eq!(palette_alpha_256(0xffffffff, 0), 0xfefefefe);
    assert_eq!(palette_alpha_256(0x00123456, 0x89abcdef), 0x89abcdef);
}
