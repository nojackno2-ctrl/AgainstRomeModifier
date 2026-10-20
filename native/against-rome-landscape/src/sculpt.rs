//! Ports of the C# `TerrainSculptFilter` Terrace / SharpenRidge / CarveValley filters and the
//! `TerrainSculptPipeline` execute-and-diff wrapper. Pure in-memory transforms: they never touch
//! map files. Float math deliberately mirrors the C# expression order (f32, .NET `Math.Clamp`
//! NaN pass-through, ties-to-even quantization in `extract_changes`).

use crate::erosion::{
    calculate_falloff, TerrainFalloffType, TerrainHeightWorkBuffer, TerrainSampleChange,
};

/// Terrace filter tuning (`stepInterval` has no C# default).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerraceParams {
    pub step_interval: f32,
    pub flatness: f32,
    pub edge_sharpness: f32,
    pub strength: f32,
    pub base_offset: f32,
}

impl TerraceParams {
    pub fn new(step_interval: f32) -> Self {
        Self {
            step_interval,
            flatness: 0.85,
            edge_sharpness: 1.5,
            strength: 1.0,
            base_offset: 0.0,
        }
    }
}

/// `Math.Max(float, float)`: propagates NaN (unlike `f32::max`).
fn net_max(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else if a > b {
        a
    } else {
        b
    }
}

/// `Math.Clamp(float, min, max)` (min <= max here); NaN passes through.
fn net_clamp(v: f32, min: f32, max: f32) -> f32 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// `GetBrushBounds`: inclusive tile bounds `(min_x, max_x, min_y, max_y)` of the brush square.
fn brush_bounds(size: usize, cx: f32, cy: f32, radius: f32) -> (i32, i32, i32, i32) {
    let last = size as i32 - 1;
    (
        ((cx - radius).floor() as i32).max(0),
        ((cx + radius).ceil() as i32).min(last),
        ((cy - radius).floor() as i32).max(0),
        ((cy + radius).ceil() as i32).min(last),
    )
}

/// Whole-map mode used by Terrace / SharpenRidge when the brush is non-positive or off-map.
fn is_global(cx: f32, cy: f32, radius: f32) -> bool {
    radius <= 0.0 || cx < 0.0 || cy < 0.0
}

fn region(buffer: &TerrainHeightWorkBuffer, cx: f32, cy: f32, radius: f32) -> (i32, i32, i32, i32) {
    if is_global(cx, cy, radius) {
        let last = buffer.size() as i32 - 1;
        (0, last, 0, last)
    } else {
        brush_bounds(buffer.size(), cx, cy, radius)
    }
}

fn distance(x: i32, y: i32, cx: f32, cy: f32) -> f32 {
    let dx = x as f32 - cx;
    let dy = y as f32 - cy;
    (dx * dx + dy * dy).sqrt()
}

/// Terrace (stepped plateau) filter. `cx/cy < 0` or `radius <= 0` applies to the whole map.
pub fn terrace(
    buffer: &mut TerrainHeightWorkBuffer,
    cx: f32,
    cy: f32,
    radius: f32,
    params: &TerraceParams,
) {
    let step_interval = params.step_interval;
    let base_offset = params.base_offset;
    if step_interval <= 0.1 || params.strength <= 0.0 {
        return;
    }
    let global = is_global(cx, cy, radius);
    let (min_x, max_x, min_y, max_y) = region(buffer, cx, cy, radius);

    let flatness = net_clamp(params.flatness, 0.0, 1.0);
    let edge_sharpness = net_clamp(params.edge_sharpness, 0.5, 4.0);
    let strength = net_clamp(params.strength, 0.0, 1.0);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let mut brush_weight = 1.0f32;
            if !global {
                let dist = distance(x, y, cx, cy);
                if dist > radius {
                    continue;
                }
                brush_weight = calculate_falloff(dist / radius, TerrainFalloffType::Smoothstep);
            }

            let current = buffer.get(x, y);
            let normalized = (current - base_offset) / step_interval;
            let step_index = normalized.floor();
            let frac = normalized - step_index;

            let shaped = if edge_sharpness <= 1.0 {
                let centered = 2.0 * frac - 1.0;
                0.5 + 0.5 * centered * centered * centered
            } else {
                let p = edge_sharpness;
                if frac < 0.5 {
                    0.5 - 0.5 * (1.0 - 2.0 * frac).powf(p)
                } else {
                    0.5 + 0.5 * (2.0 * frac - 1.0).powf(p)
                }
            };

            let terrace_frac = lerp(frac, shaped, flatness);
            let target = base_offset + (step_index + terrace_frac) * step_interval;
            let blend = brush_weight * strength;
            buffer.set(x, y, net_clamp(lerp(current, target, blend), 0.0, 255.0));
        }
    }
}

/// Laplacian ridge sharpening. Each iteration reads a snapshot, so cells never see same-pass edits.
pub fn sharpen_ridge(
    buffer: &mut TerrainHeightWorkBuffer,
    cx: f32,
    cy: f32,
    radius: f32,
    gain: f32,
    ridge_only: bool,
    iterations: i32,
) {
    if gain <= 0.0 || iterations <= 0 {
        return;
    }
    let global = is_global(cx, cy, radius);
    let (min_x, max_x, min_y, max_y) = region(buffer, cx, cy, radius);

    for _ in 0..iterations {
        let temp = buffer.clone();
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let mut brush_weight = 1.0f32;
                if !global {
                    let dist = distance(x, y, cx, cy);
                    if dist > radius {
                        continue;
                    }
                    brush_weight = calculate_falloff(dist / radius, TerrainFalloffType::Smoothstep);
                }

                let laplacian = temp.calculate_laplacian(x, y);
                let mut delta = 0.0f32;
                if laplacian < 0.0 || !ridge_only {
                    delta = -gain * laplacian;
                }

                if delta.abs() > 0.001 {
                    let new_h = net_clamp(temp.get(x, y) + delta * brush_weight, 0.0, 255.0);
                    buffer.set(x, y, new_h);
                }
            }
        }
    }
}

/// Valley carving: deepens concave (Laplacian > 0) cells inside the brush. Always brush-bound.
pub fn carve_valley(
    buffer: &mut TerrainHeightWorkBuffer,
    cx: f32,
    cy: f32,
    radius: f32,
    depth: f32,
    strength: f32,
) {
    if depth <= 0.0 || strength <= 0.0 || radius <= 0.0 {
        return;
    }
    let (min_x, max_x, min_y, max_y) = brush_bounds(buffer.size(), cx, cy, radius);
    let temp = buffer.clone();

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dist = distance(x, y, cx, cy);
            if dist > radius {
                continue;
            }
            let weight = calculate_falloff(dist / radius, TerrainFalloffType::Smoothstep);
            let laplacian = temp.calculate_laplacian(x, y);
            if laplacian > 0.0 {
                let carve = laplacian * depth * weight * strength;
                let current = buffer.get(x, y);
                buffer.set(x, y, net_max(0.0, current - carve));
            }
        }
    }
}

/// `TerrainSculptPipeline.Execute` core: builds a work buffer from `heights` (left untouched),
/// runs `process`, and returns the quantized sample changes. Errors match the buffer constructor.
pub fn execute(
    size: usize,
    heights: &[u8],
    process: impl FnOnce(&mut TerrainHeightWorkBuffer),
) -> Result<Vec<TerrainSampleChange>, String> {
    let mut buffer = TerrainHeightWorkBuffer::from_bytes(size, heights)?;
    process(&mut buffer);
    Ok(buffer.extract_changes(heights))
}

/// `TerrainSculptPipeline.Terrace`.
pub fn terrace_changes(
    size: usize,
    heights: &[u8],
    cx: f32,
    cy: f32,
    radius: f32,
    params: &TerraceParams,
) -> Result<Vec<TerrainSampleChange>, String> {
    execute(size, heights, |b| terrace(b, cx, cy, radius, params))
}

/// `TerrainSculptPipeline.SharpenRidge`.
#[allow(clippy::too_many_arguments)]
pub fn sharpen_ridge_changes(
    size: usize,
    heights: &[u8],
    cx: f32,
    cy: f32,
    radius: f32,
    gain: f32,
    ridge_only: bool,
    iterations: i32,
) -> Result<Vec<TerrainSampleChange>, String> {
    execute(size, heights, |b| {
        sharpen_ridge(b, cx, cy, radius, gain, ridge_only, iterations)
    })
}
