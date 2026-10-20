use crate::{camera::TerrainRay, terrain::TerrainHeightField, Vec3};

/// Terrain intersection ray picker replicating C# `TerrainRayPicker`.
/// Uses bounding-box slab clipping, fixed-step interval marching (0.25 units),
/// and 12-iteration bisection refinement against continuous terrain height samples.
pub struct TerrainRayPicker;

impl TerrainRayPicker {
    /// Attempts to pick a discrete map tile (tile_x, tile_y) intersected by the ray.
    pub fn try_pick(field: &TerrainHeightField, ray: TerrainRay) -> Option<(i32, i32)> {
        let point = Self::try_pick_point(field, ray)?;
        let max_x = (field.tile_width as i32).saturating_sub(1).max(0);
        let max_y = (field.tile_height as i32).saturating_sub(1).max(0);
        let tile_x = (point.x.floor() as i32).clamp(0, max_x);
        let tile_y = (point.z.floor() as i32).clamp(0, max_y);
        Some((tile_x, tile_y))
    }

    /// Attempts to compute the continuous world hit point (in tile coordinates) on the terrain.
    pub fn try_pick_point(field: &TerrainHeightField, ray: TerrainRay) -> Option<Vec3> {
        let direction = ray.direction.normalized();
        if direction.length_squared() < 1e-12 {
            return None;
        }

        let (start, end) = Self::clip_to_map(field, ray.origin, direction)?;

        let delta = |distance: f32| -> f32 {
            let p = ray.origin + direction * distance;
            if p.x < 0.0 || p.z < 0.0 || p.x > field.tile_width || p.z > field.tile_height {
                return f32::INFINITY;
            }
            p.y - field.sample_height(p.x, p.z)
        };

        let mut previous_distance = start;
        let mut previous_delta = delta(previous_distance);

        let mut distance = start + 0.25;
        while distance <= end + 0.25 {
            let d = delta(distance);
            if previous_delta >= 0.0 && d <= 0.0 {
                let mut low = previous_distance;
                let mut high = distance;
                for _ in 0..12 {
                    let middle = (low + high) * 0.5;
                    if delta(middle) >= 0.0 {
                        low = middle;
                    } else {
                        high = middle;
                    }
                }
                let hit_distance = (low + high) * 0.5;
                let hit_point = ray.origin + direction * hit_distance;
                if hit_point.x >= 0.0
                    && hit_point.z >= 0.0
                    && hit_point.x < field.tile_width
                    && hit_point.z < field.tile_height
                {
                    return Some(hit_point);
                }
            }
            previous_distance = distance;
            previous_delta = d;
            distance += 0.25;
        }

        None
    }

    /// Clips a ray to the map bounding volume: [0, tile_width] x [-1000, 1000] x [0, tile_height].
    pub fn clip_to_map(
        field: &TerrainHeightField,
        origin: Vec3,
        direction: Vec3,
    ) -> Option<(f32, f32)> {
        let mut start: f32 = 0.0;
        let mut end: f32 = f32::MAX;

        let slabs = [
            (0.0, field.tile_width, origin.x, direction.x),
            (-1000.0, 1000.0, origin.y, direction.y),
            (0.0, field.tile_height, origin.z, direction.z),
        ];

        for (min, max, o, d) in slabs {
            if d.abs() < 1e-8 {
                if o < min || o > max {
                    return None;
                }
                continue;
            }
            let mut t0 = (min - o) / d;
            let mut t1 = (max - o) / d;
            if t0 > t1 {
                std::mem::swap(&mut t0, &mut t1);
            }
            start = start.max(t0);
            end = end.min(t1);
        }

        if start <= end {
            Some((start, end))
        } else {
            None
        }
    }
}
