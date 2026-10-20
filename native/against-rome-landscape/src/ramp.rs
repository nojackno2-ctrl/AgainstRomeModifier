//! Port of C# `TerrainRampTool` (two-point ramp with width, falloff and caps).
//! Pure planning: the height slice is never mutated.
use crate::erosion::{calculate_falloff, TerrainFalloffType, TerrainSampleChange};

#[derive(Clone, Debug, PartialEq)]
pub struct TerrainRampParameters {
    /// Full width of the flat ramp body (vertex units).
    pub width: f32,
    /// Width of the soft edge transition (vertex units).
    pub falloff: f32,
    /// Start height override (clamped 0..=255); `None` samples the terrain.
    pub start_height: Option<f32>,
    pub end_height: Option<f32>,
    pub falloff_type: TerrainFalloffType,
    pub round_caps: bool,
}

impl Default for TerrainRampParameters {
    fn default() -> Self {
        Self {
            width: 4.0,
            falloff: 2.0,
            start_height: None,
            end_height: None,
            falloff_type: TerrainFalloffType::Smoothstep,
            round_caps: true,
        }
    }
}

/// Bilinear vertex sample; coordinates are clamped to the grid like C#.
pub fn sample_bilinear(heights: &[u8], size: usize, x: f32, y: f32) -> f32 {
    if size < 2 || heights.len() != size * size {
        return 0.0;
    }
    let max = (size - 1) as f32;
    let x = x.clamp(0.0, max);
    let y = y.clamp(0.0, max);
    let left = (x as usize).min(size - 2);
    let top = (y as usize).min(size - 2);
    let (tx, ty) = (x - left as f32, y - top as f32);
    let at = |r: usize, c: usize| f32::from(heights[r * size + c]);
    let (a, b, c, d) = (
        at(top, left),
        at(top, left + 1),
        at(top + 1, left),
        at(top + 1, left + 1),
    );
    (a + (b - a) * tx) * (1.0 - ty) + (c + (d - c) * tx) * ty
}

/// Tile-centre to height-vertex coordinate (C# `TileToVertex`).
pub fn tile_to_vertex(tile_x: i32, tile_y: i32, dimension: i32, size: usize) -> (f32, f32) {
    if dimension <= 0 {
        return (0.0, 0.0);
    }
    let step = (size as f32 - 1.0) / dimension as f32;
    ((tile_x as f32 + 0.5) * step, (tile_y as f32 + 0.5) * step)
}

/// Computes the height changes of a ramp. Errors mirror the C# argument exceptions.
pub fn calculate_ramp(
    heights: &[u8],
    size: usize,
    start: (f32, f32),
    end: (f32, f32),
    parameters: &TerrainRampParameters,
) -> Result<Vec<TerrainSampleChange>, String> {
    if size < 2 || size.checked_mul(size) != Some(heights.len()) {
        return Err(format!(
            "invalid height grid (length {} != {size}x{size})",
            heights.len()
        ));
    }
    if ![start.0, start.1, end.0, end.1]
        .iter()
        .all(|v| v.is_finite())
    {
        return Err("ramp coordinates must be finite".into());
    }
    let p = parameters;
    if !(p.width >= 0.0) || !(p.falloff >= 0.0) {
        return Err("width and falloff must be non-negative".into());
    }
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-6 {
        return Ok(Vec::new());
    }
    let len = len_sq.sqrt();
    let h0 = p.start_height.map_or_else(
        || sample_bilinear(heights, size, start.0, start.1),
        |h| h.clamp(0.0, 255.0),
    );
    let h1 = p.end_height.map_or_else(
        || sample_bilinear(heights, size, end.0, end.1),
        |h| h.clamp(0.0, 255.0),
    );

    let half = p.width / 2.0;
    let radius = half + p.falloff;
    if !(radius > 0.0) {
        return Ok(Vec::new());
    }
    let last = (size - 1) as i64;
    let min_x = ((start.0.min(end.0) - radius).floor() as i64).max(0);
    let max_x = ((start.0.max(end.0) + radius).ceil() as i64).min(last);
    let min_y = ((start.1.min(end.1) - radius).floor() as i64).max(0);
    let max_y = ((start.1.max(end.1) + radius).ceil() as i64).min(last);
    if min_x > max_x || min_y > max_y {
        return Ok(Vec::new());
    }

    let radius_sq = radius * radius;
    let mut changes = Vec::new();
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let (fx, fy) = (x as f32, y as f32);
            let (vx, vy) = (fx - start.0, fy - start.1);
            let t = (vx * dx + vy * dy) / len_sq;
            let (dist, target) = if p.round_caps {
                let tc = t.clamp(0.0, 1.0);
                let (px, py) = (start.0 + tc * dx, start.1 + tc * dy);
                let d_sq = (fx - px) * (fx - px) + (fy - py) * (fy - py);
                if d_sq > radius_sq {
                    continue;
                }
                (d_sq.sqrt(), h0 + tc * (h1 - h0))
            } else {
                if !(0.0..=1.0).contains(&t) {
                    continue;
                }
                let d = (vx * (-dy) + vy * dx).abs() / len;
                if d > radius {
                    continue;
                }
                (d, h0 + t * (h1 - h0))
            };
            let weight = if dist <= half {
                1.0
            } else if p.falloff > 1e-4 {
                calculate_falloff((dist - half) / p.falloff, p.falloff_type)
            } else {
                0.0
            };
            if weight <= 0.0 {
                continue;
            }
            let index = y as usize * size + x as usize;
            let before = heights[index];
            let blended = weight * target + (1.0 - weight) * f32::from(before);
            let after = blended.round_ties_even().clamp(0.0, 255.0) as u8;
            if after != before {
                changes.push(TerrainSampleChange::new(index, before, after));
            }
        }
    }
    Ok(changes)
}

/// Applies (or with `undo`, reverts) changes; rejects stale samples atomically.
pub fn apply_changes(
    heights: &mut [u8],
    changes: &[TerrainSampleChange],
    undo: bool,
) -> Result<(), String> {
    for c in changes {
        let expect = if undo { c.after } else { c.before };
        if heights.get(c.index) != Some(&expect) {
            return Err(format!("stale or out-of-range sample at index {}", c.index));
        }
    }
    for c in changes {
        heights[c.index] = if undo { c.before } else { c.after };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(size: usize, v: u8) -> Vec<u8> {
        vec![v; size * size]
    }

    #[test]
    fn coincident_points_and_zero_radius_yield_nothing() {
        let h = flat(9, 10);
        let p = TerrainRampParameters::default();
        assert!(calculate_ramp(&h, 9, (3.0, 3.0), (3.0, 3.0), &p)
            .unwrap()
            .is_empty());
        let z = TerrainRampParameters {
            width: 0.0,
            falloff: 0.0,
            ..p
        };
        assert!(calculate_ramp(&h, 9, (1.0, 1.0), (6.0, 1.0), &z)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn rejects_bad_input() {
        let h = flat(9, 10);
        let p = TerrainRampParameters::default();
        assert!(calculate_ramp(&h, 9, (f32::NAN, 0.0), (1.0, 1.0), &p).is_err());
        assert!(calculate_ramp(&h[..80], 9, (0.0, 0.0), (1.0, 1.0), &p).is_err());
        let neg = TerrainRampParameters {
            width: -1.0,
            ..p.clone()
        };
        assert!(calculate_ramp(&h, 9, (0.0, 0.0), (1.0, 1.0), &neg).is_err());
        let nan = TerrainRampParameters {
            falloff: f32::NAN,
            ..p
        };
        assert!(calculate_ramp(&h, 9, (0.0, 0.0), (1.0, 1.0), &nan).is_err());
    }

    #[test]
    fn flat_body_interpolates_linearly_and_apply_undo_roundtrip() {
        let mut h = flat(9, 0);
        let p = TerrainRampParameters {
            width: 2.0,
            falloff: 0.0,
            start_height: Some(0.0),
            end_height: Some(80.0),
            round_caps: false,
            ..Default::default()
        };
        let ch = calculate_ramp(&h, 9, (0.0, 4.0), (8.0, 4.0), &p).unwrap();
        assert!(!ch.is_empty());
        for c in &ch {
            assert_eq!(c.after as usize, (c.index % 9) * 10);
        }
        let original = h.clone();
        apply_changes(&mut h, &ch, false).unwrap();
        assert_eq!(h[4 * 9 + 8], 80);
        assert!(apply_changes(&mut h, &ch, false).is_err());
        apply_changes(&mut h, &ch, true).unwrap();
        assert_eq!(h, original);
    }

    #[test]
    fn overrides_are_clamped_and_caps_differ() {
        let h = flat(9, 100);
        let base = TerrainRampParameters {
            width: 2.0,
            falloff: 0.0,
            start_height: Some(-50.0),
            end_height: Some(900.0),
            ..Default::default()
        };
        let round = calculate_ramp(&h, 9, (2.0, 4.0), (6.0, 4.0), &base).unwrap();
        let flat_caps = TerrainRampParameters {
            round_caps: false,
            ..base
        };
        let cut = calculate_ramp(&h, 9, (2.0, 4.0), (6.0, 4.0), &flat_caps).unwrap();
        assert!(round.len() > cut.len());
        assert!(cut.iter().any(|c| c.index == 4 * 9 + 2 && c.after == 0));
        assert!(cut.iter().any(|c| c.index == 4 * 9 + 6 && c.after == 255));
    }

    #[test]
    fn sample_and_tile_helpers() {
        let mut h = flat(3, 0);
        h[1] = 100;
        assert_eq!(sample_bilinear(&h, 3, 0.5, 0.0), 50.0);
        assert_eq!(sample_bilinear(&h, 3, -9.0, -9.0), 0.0);
        assert_eq!(sample_bilinear(&h, 3, 99.0, 99.0), 0.0);
        assert_eq!(tile_to_vertex(0, 0, 0, 193), (0.0, 0.0));
        assert_eq!(tile_to_vertex(1, 2, 2, 5), (3.0, 5.0));
    }
}
