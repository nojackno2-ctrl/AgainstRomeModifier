use crate::{Vec2, Vec3};

/// Byte height grid, independent of editor core ownership. Dimensions are in tiles.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainHeightField {
    width: usize,
    height: usize,
    samples: Vec<u8>,
    pub height_scale: f32,
    pub tile_width: f32,
    pub tile_height: f32,
    pub native_step: Option<f32>,
}

impl TerrainHeightField {
    pub fn new(
        width: usize,
        height: usize,
        samples: Vec<u8>,
        height_scale: f32,
        tile_width: f32,
        tile_height: f32,
        native_step: Option<f32>,
    ) -> Result<Self, &'static str> {
        if width < 2 || height < 2 || width.checked_mul(height) != Some(samples.len()) {
            return Err("height grid must be at least 2x2 and match samples length");
        }
        if !tile_width.is_finite()
            || !tile_height.is_finite()
            || tile_width <= 0.0
            || tile_height <= 0.0
        {
            return Err("tile dimensions must be finite and positive");
        }
        Ok(Self {
            width,
            height,
            samples,
            height_scale,
            tile_width,
            tile_height,
            native_step: native_step.filter(|v| v.is_finite() && *v > 0.0),
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn samples(&self) -> &[u8] {
        &self.samples
    }

    pub fn sample_raw(&self, x: f32, z: f32) -> f32 {
        let x = x.clamp(0.0, self.tile_width) * (self.width - 1) as f32 / self.tile_width;
        let z = z.clamp(0.0, self.tile_height) * (self.height - 1) as f32 / self.tile_height;
        let l = (x.floor() as usize).min(self.width - 2);
        let t = (z.floor() as usize).min(self.height - 2);
        let tx = x - l as f32;
        let tz = z - t as f32;
        let a = self.samples[t * self.width + l] as f32;
        let b = self.samples[t * self.width + l + 1] as f32;
        let c = self.samples[(t + 1) * self.width + l] as f32;
        let d = self.samples[(t + 1) * self.width + l + 1] as f32;
        let top = a + (b - a) * tx;
        let bottom = c + (d - c) * tx;
        top + (bottom - top) * tz
    }

    pub fn sample_height(&self, x: f32, z: f32) -> f32 {
        self.sample_raw(x, z) / 255.0 * self.height_scale
    }

    pub fn has_native_query(&self) -> bool {
        self.native_step
            .is_some_and(|step| (255.0 * step as f64) <= f32::MAX as f64)
    }

    pub fn native_height(&self, y: f32) -> Option<f32> {
        let step = self.native_step?;
        if !y.is_finite() || !self.height_scale.is_finite() {
            return None;
        }
        let v = (y as f64 / step as f64 / 255.0) * self.height_scale as f64;
        if v.is_finite() && v.abs() <= f32::MAX as f64 {
            Some(v as f32)
        } else {
            None
        }
    }

    pub fn native_query_height(&self, x: f32, z: f32) -> Option<f32> {
        if !self.has_native_query() {
            return None;
        }
        let step = self.native_step?;
        Some((self.sample_raw(x, z) as f64 * step as f64) as f32)
    }

    pub fn minimum_native_height(&self) -> Option<f32> {
        if !self.has_native_query() {
            return None;
        }
        let step = self.native_step?;
        let min_val = self.samples.iter().copied().min().unwrap_or(0);
        Some((min_val as f64 * step as f64) as f32)
    }

    pub fn build_native_height_texels(&self) -> Option<Vec<f32>> {
        if !self.has_native_query() {
            return None;
        }
        let step = self.native_step?;
        Some(
            self.samples
                .iter()
                .map(|&s| (s as f64 * step as f64) as f32)
                .collect(),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub tex_coord: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerrainMeshData {
    pub vertices: Vec<TerrainVertex>,
    pub indices: Vec<u32>,
}

/// Each tile owns four vertices, preserving independent atlas seams.
pub fn build_terrain_mesh(
    field: &TerrainHeightField,
    dimension: usize,
    mut uv: impl FnMut(usize, usize) -> [Vec2; 4],
) -> Result<TerrainMeshData, &'static str> {
    let count = dimension
        .checked_mul(dimension)
        .filter(|&n| n > 0 && n <= u32::MAX as usize / 4)
        .ok_or("invalid terrain dimension")?;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    vertices
        .try_reserve_exact(count.checked_mul(4).ok_or("mesh overflow")?)
        .map_err(|_| "mesh allocation failed")?;
    indices
        .try_reserve_exact(count.checked_mul(6).ok_or("mesh overflow")?)
        .map_err(|_| "mesh allocation failed")?;

    for y in 0..dimension {
        for x in 0..dimension {
            let start = vertices.len() as u32;
            let tile_uv = uv(x, y);
            let coords = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)];
            for (&(vx, vz_coord), &tex_coord) in coords.iter().zip(tile_uv.iter()) {
                let (vx_f, vz_f) = (vx as f32, vz_coord as f32);
                let dx = field.sample_height(vx_f + 0.5, vz_f) - field.sample_height(vx_f - 0.5, vz_f);
                let dz = field.sample_height(vx_f, vz_f + 0.5) - field.sample_height(vx_f, vz_f - 0.5);
                vertices.push(TerrainVertex {
                    position: Vec3::new(vx_f, field.sample_height(vx_f, vz_f), vz_f),
                    normal: Vec3::new(-dx, 1.0, -dz).normalized(),
                    tex_coord,
                });
            }
            indices.extend([start, start + 2, start + 1, start, start + 3, start + 2]);
        }
    }
    Ok(TerrainMeshData { vertices, indices })
}
