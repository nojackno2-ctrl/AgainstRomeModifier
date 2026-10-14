//! Immutable integer primitives from 40e5c0 and 40e750/7b0/900/980.
//!
//! These operate on caller-supplied raw words and selected index spans, not ALR frames or
//! native primary/endpoint palette dispatch. See docs/reverse-engineering/particle-pixel-contract.md.

use std::fmt;

/// Bounded4ca340 palette choice. Variant upper bounds and weight>256 follow
/// native signed comparisons; unsupported negative variant values are rejected.
/// Output words: route0 endpoint/1 interpolation, first, second(-1 unused), weight.
pub fn palette_selection_4ca340(variants: i32, first: i32, second: i32, weight: i32)
    -> Option<[i32; 4]>
{
    if variants <= 0 || first < 0 || second < -1 { return None; }
    let first = first.min(variants - 1);
    let second = second.min(variants - 1);
    let weight = weight.min(256);
    if second == -1 || weight == 0 || weight == 256 || first == second {
        let chosen = if second != -1 && weight == 256 { second } else { first };
        Some([0, chosen, -1, weight])
    } else { Some([1, first, second, weight]) }
}

/// One DWORD lane of40186a with source-bound615fa0..615fc0 masks.
/// Does not model the native buffer's source-alignment-dependent pointer moves.
pub fn palette_alpha_lane_40186a(source: u32) -> u32 {
    let low = source.wrapping_shl(22) & 0x3fc00000;
    let middle = source.wrapping_shl(15) & 0x7f800000;
    let high = source.wrapping_shl(6) & 0x3fc00000;
    (source & 0xffffff) + (low.wrapping_add(middle).wrapping_add(high) & 0xff000000)
}

/// Source-bound on-disk default helper40fd50, not proof of live618b54 dispatch.
/// Three gains are unsigned bytes extracted from the supplied packed word.
/// Native masks truncate each product; source and output high bytes are ignored.
pub fn primary_palette_default_40fd50(source: u32, packed_gain: u32) -> u32 {
    let low = ((source & 255) * (packed_gain & 255)) >> 8;
    let middle = (((source >> 8) & 255) * ((packed_gain >> 8) & 255)) & 0xff00;
    let high = (((source & 0xff0000) >> 8) * ((packed_gain >> 16) & 255)) & 0xff0000;
    low + middle + high
}

/// Raw arguments to the 40e5c0 kernel. Gains are low32 IMUL operands, not bounded channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaletteKernel40e5c0 {
    pub weight: i32,
    pub gain_low: u32,
    pub gain_middle: u32,
    pub gain_high: u32,
    pub tint: u32,
    /// Native TEST semantics: every nonzero raw value enables alpha synthesis.
    pub alpha_flag: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexZeroPolicy {
    AlwaysWrite,
    SkipZero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelWriteMode {
    Replacement,
    HalfAverage,
    PaletteAlpha256,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PixelError {
    UnverifiedAlphaLayout,
    RasterDimensions,
    InvalidSample { position: usize, sample: u16 },
    InvalidPaletteCount {
        supplied_count: usize,
    },
    InsufficientPaletteBuffer {
        buffer: &'static str,
        required: usize,
        actual: usize,
    },
    SpanLengthMismatch {
        indices: usize,
        destination: usize,
    },
    PaletteIndexOutOfRange {
        position: usize,
        index: u8,
        palette_len: usize,
    },
}

/// Explicit40e4d0 profile with the verified disk-default40fd50 helper.
/// Caller must establish working-buffer start mod8 for alpha; only the aligned
///40186a route is admitted. This does not prove live pointer dispatch.
pub fn primary_endpoint_default(source: &[u32], key: u32, low: u32, middle: u32,
    high: u32, tint: u32, alpha_flag: u32, buffer_mod8: Option<u32>) -> Result<Vec<u32>, PixelError> {
    if source.is_empty() || source.len() > 256 {
        return Err(PixelError::InvalidPaletteCount { supplied_count: source.len() });
    }
    let skip = usize::from(source[0] & 0xffffff == key & 0xffffff);
    if alpha_flag != 0 {
        let start = buffer_mod8.ok_or(PixelError::UnverifiedAlphaLayout)?;
        if start > 7 || (start + skip as u32 * 4) % 8 != 0 {
            return Err(PixelError::UnverifiedAlphaLayout);
        }
    }
    let packed = low.wrapping_add(middle.wrapping_shl(8)).wrapping_add(high.wrapping_shl(16));
    let mut result = source.to_vec();
    for word in &mut result[skip..] {
        *word = primary_palette_default_40fd50(*word, packed);
        if tint != 0 { *word = half_average(*word, tint); }
        if alpha_flag != 0 { *word = palette_alpha_lane_40186a(*word); }
    }
    Ok(result)
}

/// Integer unscaled blit into a caller-sized raw framebuffer. Validate the full
/// source, including clipped pixels, before returning a detached destination.
/// Screen projection and native palette/mode dispatch remain caller-owned.
pub fn blit_indexed_samples(samples: &[u16], source_width: usize, source_height: usize,
    palette: &[u32], destination: &[u32], destination_width: usize, destination_height: usize,
    x: i32, y: i32, mode: PixelWriteMode) -> Result<Vec<u32>, PixelError>
{
    if source_width > 2047 || source_height > 2047
        || source_width.checked_mul(source_height) != Some(samples.len())
        || destination_width > i32::MAX as usize || destination_height > i32::MAX as usize
        || destination_width.checked_mul(destination_height) != Some(destination.len()) {
        return Err(PixelError::RasterDimensions);
    }
    let raw = indexed_sample_pixels(samples, palette, &vec![0; samples.len()], PixelWriteMode::Replacement)?;
    let mut result = destination.to_vec();
    for sy in 0..source_height {
        let dy = i64::from(y) + sy as i64;
        if dy < 0 || dy >= destination_height as i64 { continue; }
        for sx in 0..source_width {
            let dx = i64::from(x) + sx as i64;
            if dx < 0 || dx >= destination_width as i64 { continue; }
            let source = sy * source_width + sx;
            let sample = samples[source];
            if sample == 0 || (sample & 255 == 0 && sample & 0x200 == 0) { continue; }
            let target = dy as usize * destination_width + dx as usize;
            result[target] = match mode {
                PixelWriteMode::Replacement => raw[source],
                PixelWriteMode::HalfAverage => half_average(raw[source], result[target]),
                PixelWriteMode::PaletteAlpha256 => palette_alpha_256(raw[source], result[target]),
            };
        }
    }
    Ok(result)
}

/// Compose packed ALR samples from ar_alr_decode_indexed over caller raw words.
/// Absent spans and segment-skipped zero preserve destination exactly. This
/// accepts an explicitly selected kernel, not native mode/palette dispatch.
pub fn indexed_sample_pixels(samples: &[u16], palette: &[u32], destination: &[u32],
    mode: PixelWriteMode) -> Result<Vec<u32>, PixelError>
{
    if samples.len() != destination.len() {
        return Err(PixelError::SpanLengthMismatch { indices: samples.len(), destination: destination.len() });
    }
    let mut result = Vec::with_capacity(samples.len());
    for (position, (&sample, &dst)) in samples.iter().zip(destination).enumerate() {
        if sample == 0 { result.push(dst); continue; }
        if sample & !0x3ff != 0 || sample & 0x100 == 0 {
            return Err(PixelError::InvalidSample { position, sample });
        }
        let index = (sample & 255) as u8;
        let src = *palette.get(index as usize).ok_or(PixelError::PaletteIndexOutOfRange {
            position, index, palette_len: palette.len(),
        })?;
        result.push(if index == 0 && sample & 0x200 == 0 { dst } else {
            match mode {
                PixelWriteMode::Replacement => src,
                PixelWriteMode::HalfAverage => half_average(src, dst),
                PixelWriteMode::PaletteAlpha256 => palette_alpha_256(src, dst),
            }
        });
    }
    Ok(result)
}

impl fmt::Display for PixelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "particle pixel input rejected: {self:?}")
    }
}

impl std::error::Error for PixelError {}

/// Model only 40e5c0 on supplied A/B buffers; return exactly `supplied_count` new words.
///
/// Count 1..=256 and both buffers at least that long are product restrictions, not native
/// validation claims. Weight is unrestricted i32 and all other operands retain their raw bits.
/// Entry zero copies A[0] unchanged, even when tint/alpha/gains would change later entries.
/// This must not substitute for the unresolved 40e4d0 endpoint/primary palette route.
pub fn palette_kernel_40e5c0(
    a: &[u32],
    b: &[u32],
    supplied_count: usize,
    args: PaletteKernel40e5c0,
) -> Result<Vec<u32>, PixelError> {
    if !(1..=256).contains(&supplied_count) {
        return Err(PixelError::InvalidPaletteCount { supplied_count });
    }
    for (buffer, actual) in [("A", a.len()), ("B", b.len())] {
        if actual < supplied_count {
            return Err(PixelError::InsufficientPaletteBuffer {
                buffer,
                required: supplied_count,
                actual,
            });
        }
    }
    let w = args.weight as u32;
    let u = 256u32.wrapping_sub(w);
    let mut output = Vec::with_capacity(supplied_count);
    output.push(a[0]);
    for j in 1..supplied_count {
        let sum = |shift: u32| {
            ((a[j] >> shift) & 0xff)
                .wrapping_mul(u)
                .wrapping_add(((b[j] >> shift) & 0xff).wrapping_mul(w))
        };
        let low = (sum(0) >> 8).wrapping_mul(args.gain_low) >> 8;
        let middle = ((sum(8) & 0xff00).wrapping_mul(args.gain_middle) >> 8) & 0xff00;
        let high = (sum(16) & 0xff00).wrapping_mul(args.gain_high) & 0xff0000;
        let mut color = low.wrapping_add(middle).wrapping_add(high);
        if args.tint != 0 {
            color = half_average(color, args.tint);
        }
        if args.alpha_flag != 0 {
            let alpha = ((color & 0xff) + 2 * ((color >> 8) & 0xff) + ((color >> 16) & 0xff)) >> 2;
            color = (color & 0xffffff).wrapping_add(alpha << 24);
        }
        output.push(color);
    }
    Ok(output)
}

/// Exact 40e900/980 packed operation: floor each RGB operand half, then add.
/// High alpha is discarded; white + white is 0x00fefefe, not 0x00ffffff.
pub fn half_average(source: u32, destination: u32) -> u32 {
    ((source & 0x00fefefe) >> 1).wrapping_add((destination & 0x00fefefe) >> 1)
}

/// Apply selected indices to an equally sized immutable destination span, returning new words.
///
/// Replacement + AlwaysWrite/SkipZero model 40e750/7b0; HalfAverage + the same policies model
/// 40e900/980. All required indices are preflighted before allocating output. Skipped zero
/// retains every destination bit and requires no palette[0], even with an empty palette.
/// This API does not select/clip/stride/decode rows or infer native dispatch from a frame.
pub fn indexed_pixel_row(
    indices: &[u8],
    palette: &[u32],
    destination: &[u32],
    zero_policy: IndexZeroPolicy,
    mode: PixelWriteMode,
) -> Result<Vec<u32>, PixelError> {
    if indices.len() != destination.len() {
        return Err(PixelError::SpanLengthMismatch {
            indices: indices.len(),
            destination: destination.len(),
        });
    }
    for (position, &index) in indices.iter().enumerate() {
        if index == 0 && zero_policy == IndexZeroPolicy::SkipZero {
            continue;
        }
        if usize::from(index) >= palette.len() {
            return Err(PixelError::PaletteIndexOutOfRange {
                position,
                index,
                palette_len: palette.len(),
            });
        }
    }
    Ok(indices
        .iter()
        .zip(destination)
        .map(|(&index, &dst)| {
            if index == 0 && zero_policy == IndexZeroPolicy::SkipZero {
                dst
            } else {
                let src = palette[usize::from(index)];
                match mode {
                    PixelWriteMode::Replacement => src,
                    PixelWriteMode::HalfAverage => half_average(src, dst),
                    PixelWriteMode::PaletteAlpha256 => palette_alpha_256(src, dst),
                }
            }
        })
        .collect())
}

/// Raw four-byte MMX blend observed at 401644/40173f with static table618320.
/// Alpha is the source high byte, used for all four lanes, with denominator256.
/// This is not an editor ARGB conversion or native dispatch/visibility claim.
pub fn palette_alpha_256(source: u32, destination: u32) -> u32 {
    let alpha = source >> 24;
    let mut result = 0;
    for shift in [0, 8, 16, 24] {
        let s = (source >> shift) & 255;
        let d = (destination >> shift) & 255;
        result |= ((s * alpha + d * (256 - alpha)) >> 8) << shift;
    }
    result
}
