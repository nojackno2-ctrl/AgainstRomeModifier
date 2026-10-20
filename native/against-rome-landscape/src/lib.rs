//! Pure planning ports of the C# landscape brushes. Plans do not write map files.
//! Coordinates are tile (X,Y), or (X,Z) for fortifications; masks are N/E/S/W.
pub mod cliff;
pub mod erosion;
pub mod fortification;
pub mod ramp;
pub mod river;
pub mod road;
pub mod sculpt;

pub use erosion::{
    calculate_falloff, simulate_hydraulic_erosion, simulate_thermal_erosion, BrushRegion,
    DotNetRandom, HydraulicErosionParams, TerrainFalloffType, TerrainHeightWorkBuffer,
    TerrainSampleChange, ThermalErosionParams,
};
pub use ramp::{calculate_ramp, TerrainRampParameters};
pub use sculpt::{carve_valley, sharpen_ridge, terrace, TerraceParams};

pub type Point = (i32, i32);
pub const NEIGHBORS: [(i32, i32, u8, u8); 4] =
    [(0, -1, 1, 4), (1, 0, 2, 8), (0, 1, 4, 1), (-1, 0, 8, 2)];

pub(crate) fn valid_dimensions(dimension: i32, count: usize) -> bool {
    dimension > 0 && (dimension as usize).checked_mul(dimension as usize) == Some(count)
}
pub(crate) fn in_bounds(p: Point, dimension: i32) -> bool {
    p.0 >= 0 && p.1 >= 0 && p.0 < dimension && p.1 < dimension
}

/// C# TerrainStrokePath.Between followed by the road/river X-first diagonal bridge.
/// Includes the end, excludes the start. Uses widened intermediates to avoid overflow.
pub fn orthogonal_between(start: Point, end: Point) -> Vec<Point> {
    let (mut x, mut y) = (i64::from(start.0), i64::from(start.1));
    let (ex, ey) = (i64::from(end.0), i64::from(end.1));
    let dx = (ex - x).abs();
    let dy = -(ey - y).abs();
    let sx = if x < ex { 1 } else { -1 };
    let sy = if y < ey { 1 } else { -1 };
    let mut error = dx + dy;
    let mut result = Vec::new();
    while x != ex || y != ey {
        let previous = (x, y);
        let twice = 2 * error;
        if twice >= dy {
            error += dy;
            x += sx;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
        if previous.0 != x && previous.1 != y {
            result.push((x as i32, previous.1 as i32));
        }
        result.push((x as i32, y as i32));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stroke_matches_csharp_diagonal_bridge() {
        assert_eq!(
            orthogonal_between((0, 0), (3, 2)),
            vec![(1, 0), (1, 1), (2, 1), (3, 1), (3, 2)]
        );
        assert!(orthogonal_between((2, 2), (2, 2)).is_empty());
    }
    #[test]
    fn all_small_strokes_are_orthogonal_and_end_at_target() {
        for x in -6..=6 {
            for y in -6..=6 {
                let mut prev = (0, 0);
                for p in orthogonal_between(prev, (x, y)) {
                    assert_eq!((p.0 - prev.0).abs() + (p.1 - prev.1).abs(), 1);
                    prev = p;
                }
                assert_eq!(prev, (x, y));
            }
        }
    }
}
