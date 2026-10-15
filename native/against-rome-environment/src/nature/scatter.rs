//! Pure calculation of nature object scatter within brush circles.

use crate::random::DotNetRandom;
use serde::{Deserialize, Serialize};

/// 自然物件筆刷的散佈密度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NatureDensity {
    Sparse,
    Normal,
    Dense,
}

impl NatureDensity {
    /// 密度對應每格種植機率。
    pub fn probability(self) -> f64 {
        match self {
            Self::Sparse => 0.15,
            Self::Dense => 0.70,
            Self::Normal => 0.35,
        }
    }

    /// 最小間距（tile），密度越高越近。
    pub fn min_spacing_tiles(self) -> f32 {
        match self {
            Self::Sparse => 0.90,
            Self::Dense => 0.45,
            Self::Normal => 0.65,
        }
    }
}

/// 自然物件散佈的純計算。
pub struct NatureScatter;

impl NatureScatter {
    pub fn probability(density: NatureDensity) -> f64 {
        density.probability()
    }

    pub fn min_spacing_tiles(density: NatureDensity) -> f32 {
        density.min_spacing_tiles()
    }

    /// 回傳新的種植位置（tile 座標，可含小數）。
    pub fn plan(
        center_tile_x: i32,
        center_tile_y: i32,
        brush_size: i32,
        density: NatureDensity,
        dimension: i32,
        existing: &[(f32, f32)],
        random: &mut DotNetRandom,
    ) -> Vec<(f32, f32)> {
        let spacing = density.min_spacing_tiles();
        let mut occupied = existing.to_vec();
        let mut planted = Vec::new();

        if brush_size <= 1 {
            let point = (
                center_tile_x as f32 + 0.2 + random.next_double() as f32 * 0.6,
                center_tile_y as f32 + 0.2 + random.next_double() as f32 * 0.6,
            );
            if center_tile_x >= 0
                && center_tile_y >= 0
                && center_tile_x < dimension
                && center_tile_y < dimension
            {
                planted.push(point);
            }
            return planted;
        }

        let radius = brush_size as f32 / 2.0 + 0.26;
        let prob = density.probability();
        let reach = radius.ceil() as i32;

        let dim_f = dimension as f32;
        let radius_sq = radius * radius;
        let spacing_sq = spacing * spacing;

        for y in (center_tile_y - reach)..=(center_tile_y + reach) {
            for x in (center_tile_x - reach)..=(center_tile_x + reach) {
                if x < 0 || y < 0 || x >= dimension || y >= dimension {
                    continue;
                }
                let dx = x - center_tile_x;
                let dy = y - center_tile_y;
                if (dx * dx + dy * dy) as f32 > radius_sq {
                    continue;
                }
                if random.next_double() >= prob {
                    continue;
                }
                let point = (
                    (x as f32 + random.next_double() as f32).min(dim_f - 0.01),
                    (y as f32 + random.next_double() as f32).min(dim_f - 0.01),
                );

                if is_free(point, &occupied, spacing_sq) {
                    planted.push(point);
                    occupied.push(point);
                }
            }
        }

        planted
    }
}

#[inline]
fn is_free(point: (f32, f32), occupied: &[(f32, f32)], spacing_sq: f32) -> bool {
    occupied.iter().all(|&(ox, oy)| {
        let dx = ox - point.0;
        let dy = oy - point.1;
        dx * dx + dy * dy >= spacing_sq
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_tile_brush_plants_one_point() {
        let mut rng = DotNetRandom::new(42);
        let pts = NatureScatter::plan(10, 10, 1, NatureDensity::Normal, 64, &[], &mut rng);
        assert_eq!(pts.len(), 1);
        assert!(pts[0].0 >= 10.2 && pts[0].0 <= 10.8);
        assert!(pts[0].1 >= 10.2 && pts[0].1 <= 10.8);
    }

    #[test]
    fn spacing_respected_in_large_brush() {
        let mut rng = DotNetRandom::new(100);
        let pts = NatureScatter::plan(20, 20, 5, NatureDensity::Normal, 64, &[], &mut rng);
        assert!(!pts.is_empty());
        let spacing = NatureDensity::Normal.min_spacing_tiles();
        let spacing_sq = spacing * spacing - 1e-4;
        for i in 0..pts.len() {
            for j in (i + 1)..pts.len() {
                let dx = pts[i].0 - pts[j].0;
                let dy = pts[i].1 - pts[j].1;
                assert!(dx * dx + dy * dy >= spacing_sq);
            }
        }
    }
}
