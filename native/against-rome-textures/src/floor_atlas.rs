//! Terrain floor texture atlas packing, gutter dilation, and UV layout.
//!
//! Direct Rust counterpart to `src.MapEditor/FloorTextureAtlas.cs`.
//!
//! # Overview
//! Maps Against Rome floor texture tiles into a unified power-of-two texture atlas.
//! Each tile is 128x128 pixels surrounded by an 8-pixel gutter (stride 144 pixels).
//! The gutter is filled with stretched edge pixels so mipmapping and bilinear sampling
//! do not bleed adjacent tiles when viewing the terrain at distance.

use crate::error::TextureError;
use crate::floortex::FloortexLibrary;
use crate::terrain_layer::TerrainLayer;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A 2D normalized texture coordinate point (u, v).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AtlasPoint {
    pub u: f32,
    pub v: f32,
}

impl AtlasPoint {
    pub const fn new(u: f32, v: f32) -> Self {
        Self { u, v }
    }
}

/// A pixel rectangle within a texture atlas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AtlasRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl AtlasRect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Converts this pixel rectangle to 4 normalized UV corner points in counter-clockwise / quad order:
    /// [Top-Left, Top-Right, Bottom-Right, Bottom-Left].
    /// Includes a half-texel inset (0.5 / atlasSize) to prevent filtering bleed across tile borders.
    pub fn to_uv(&self, atlas_size: usize) -> [AtlasPoint; 4] {
        if atlas_size == 0 {
            return [AtlasPoint::new(0.0, 0.0); 4];
        }
        let size = atlas_size as f32;
        let inset = 0.5 / size;
        let left = self.x as f32 / size + inset;
        let top = self.y as f32 / size + inset;
        let right = (self.x + self.width) as f32 / size - inset;
        let bottom = (self.y + self.height) as f32 / size - inset;
        [
            AtlasPoint::new(left, top),
            AtlasPoint::new(right, top),
            AtlasPoint::new(right, bottom),
            AtlasPoint::new(left, bottom),
        ]
    }

    /// Returns the (left, top, right, bottom) UV bounding coordinates with half-texel inset.
    pub fn to_uv_bounds(&self, atlas_size: usize) -> (f32, f32, f32, f32) {
        if atlas_size == 0 {
            return (0.0, 0.0, 0.0, 0.0);
        }
        let size = atlas_size as f32;
        let inset = 0.5 / size;
        let left = self.x as f32 / size + inset;
        let top = self.y as f32 / size + inset;
        let right = (self.x + self.width) as f32 / size - inset;
        let bottom = (self.y + self.height) as f32 / size - inset;
        (left, top, right, bottom)
    }
}

/// Stitched texture atlas for Against Rome terrain floor tiles.
///
/// Direct counterpart to C# `FloorTextureAtlas`.
#[derive(Clone, Debug)]
pub struct FloorTextureAtlas {
    size: usize,
    argb_pixels: Vec<u32>,
    rects: HashMap<String, AtlasRect>,
    ordered_names: Vec<String>,
}

impl FloorTextureAtlas {
    /// Tile inner dimension in pixels.
    pub const CELL: usize = 128;
    /// Gutter padding around each tile in pixels to prevent mipmap bleed.
    pub const PAD: usize = 8;
    /// Cell stride including gutters on all four sides.
    pub const STRIDE: usize = Self::CELL + Self::PAD * 2; // 144
    /// Default background clear color matching C# `Color.FromArgb(90, 90, 90)`.
    pub const DEFAULT_BG_COLOR: u32 = 0xFF5A_5A5A;

    /// Atlas width and height in pixels (always square power-of-two, 2048 or 4096).
    pub fn size(&self) -> usize {
        self.size
    }

    /// Linear ARGB32 pixel buffer of dimensions `size * size`.
    pub fn argb_pixels(&self) -> &[u32] {
        &self.argb_pixels
    }

    /// Number of distinct floor textures mapped in this atlas.
    pub fn count(&self) -> usize {
        self.rects.len()
    }

    /// Canonical ordered list of distinct texture names placed in the atlas.
    pub fn names(&self) -> &[String] {
        &self.ordered_names
    }

    /// Checks if a texture name exists in this atlas (case-insensitive).
    pub fn contains(&self, texture: &str) -> bool {
        self.rects.contains_key(&normalize_key(texture))
    }

    /// Retrieves the pixel rect for a texture, if present.
    pub fn get_rect(&self, texture: &str) -> Option<AtlasRect> {
        self.rects.get(&normalize_key(texture)).copied()
    }

    /// Retrieves the 4 UV coordinates [TL, TR, BR, BL] for a texture.
    ///
    /// If the texture is not found, returns fallback UV covering the entire atlas
    /// with half-texel inset (matching C# `FloorTextureAtlas.GetUv`).
    pub fn get_uv(&self, texture: &str) -> [AtlasPoint; 4] {
        if let Some(rect) = self.get_rect(texture) {
            rect.to_uv(self.size)
        } else {
            AtlasRect::new(0, 0, self.size as i32, self.size as i32).to_uv(self.size)
        }
    }

    /// Retrieves the (u0, v0, u1, v1) UV bounding box for a texture.
    pub fn get_uv_bounds(&self, texture: &str) -> (f32, f32, f32, f32) {
        if let Some(rect) = self.get_rect(texture) {
            rect.to_uv_bounds(self.size)
        } else {
            AtlasRect::new(0, 0, self.size as i32, self.size as i32).to_uv_bounds(self.size)
        }
    }

    /// Maximum number of 144x144 stride cells that fit in an atlas of the given size.
    pub fn capacity(atlas_size: usize) -> usize {
        let cells_per_axis = atlas_size / Self::STRIDE;
        cells_per_axis * cells_per_axis
    }

    /// Computes grid columns and rows for a given texture count and atlas size.
    pub fn get_grid(count: usize, atlas_size: usize) -> Result<(usize, usize), TextureError> {
        let cells_per_axis = atlas_size / Self::STRIDE;
        let normalized = count.max(1);
        if cells_per_axis == 0 || normalized > cells_per_axis * cells_per_axis {
            return Err(TextureError::InvalidAtlas(format!(
                "地圖使用 {} 種不同地表貼圖，單張 {}×{} atlas 最多只能容納 {} 種。",
                count,
                atlas_size,
                atlas_size,
                cells_per_axis * cells_per_axis
            )));
        }
        let columns = ((normalized as f64).sqrt().ceil() as usize).min(cells_per_axis);
        let rows = (normalized + columns - 1) / columns;
        Ok((columns, rows))
    }

    /// Computes atlas cell placement rectangles for unique texture names without decoding pixel data.
    pub fn layout<S: AsRef<str>>(
        names: impl IntoIterator<Item = S>,
        atlas_size: usize,
    ) -> Result<HashMap<String, AtlasRect>, TextureError> {
        let unique = deduplicate_names(names);
        let (columns, _) = Self::get_grid(unique.len(), atlas_size)?;
        let mut map = HashMap::with_capacity(unique.len());
        for (index, name) in unique.iter().enumerate() {
            let cell_x = (index % columns) * Self::STRIDE;
            let cell_y = (index / columns) * Self::STRIDE;
            let inner_x = cell_x + Self::PAD;
            let inner_y = cell_y + Self::PAD;
            map.insert(
                normalize_key(name),
                AtlasRect::new(
                    inner_x as i32,
                    inner_y as i32,
                    Self::CELL as i32,
                    Self::CELL as i32,
                ),
            );
        }
        Ok(map)
    }

    /// Deterministic fallback color for missing textures, matching C# `FallbackColor`.
    ///
    /// Produces a distinctive grayish-earth tone with R, G, B in range [90..=169] and alpha 0xFF.
    pub fn fallback_color(texture: &str) -> u32 {
        let mut hash: u32 = 0x811c_9dc5;
        for &b in texture.as_bytes() {
            let upper = b.to_ascii_uppercase();
            hash = (hash ^ (upper as u32)).wrapping_mul(0x0100_0193);
        }
        Self::fallback_color_from_hash(hash as i32)
    }

    /// Computes fallback ARGB color from a 32-bit signed hash code matching C# bitwise formula.
    pub fn fallback_color_from_hash(hash: i32) -> u32 {
        let u = hash as u32;
        let r = 90 + (u & 0x4f);
        let g = 90 + ((u >> 8) & 0x4f);
        let b = 90 + ((u >> 16) & 0x4f);
        0xFF00_0000 | (r << 16) | (g << 8) | b
    }

    /// Builds a full stitched texture atlas using a custom tile decoder closure.
    ///
    /// Follows C# dynamic sizing:
    /// - 2048x2048 when unique tile count <= 196 (`Capacity(2048)`).
    /// - 4096x4096 when unique tile count > 196 and <= 784 (`Capacity(4096)`).
    /// - Rejects with `TextureError::InvalidAtlas` when unique tile count > 784.
    pub fn create_with_decoder<S, I, F>(names: I, mut decode: F) -> Result<Self, TextureError>
    where
        S: AsRef<str>,
        I: IntoIterator<Item = S>,
        F: FnMut(&str) -> Option<TerrainLayer>,
    {
        let unique = deduplicate_names(names);
        let size = if unique.len() <= Self::capacity(2048) {
            2048
        } else {
            4096
        };

        let (columns, _) = Self::get_grid(unique.len(), size)?;
        let mut pixels = vec![Self::DEFAULT_BG_COLOR; size * size];
        let mut rects = HashMap::with_capacity(unique.len());

        for (index, name) in unique.iter().enumerate() {
            let cell_x = (index % columns) * Self::STRIDE;
            let cell_y = (index / columns) * Self::STRIDE;
            let inner_x = cell_x + Self::PAD;
            let inner_y = cell_y + Self::PAD;

            let rect = AtlasRect::new(
                inner_x as i32,
                inner_y as i32,
                Self::CELL as i32,
                Self::CELL as i32,
            );
            rects.insert(normalize_key(name), rect);

            if let Some(layer) = decode(name) {
                blit_cell_with_gutter(&mut pixels, size, cell_x, cell_y, &layer);
            } else {
                let color = Self::fallback_color(name);
                fill_cell(&mut pixels, size, cell_x, cell_y, color);
            }
        }

        Ok(Self {
            size,
            argb_pixels: pixels,
            rects,
            ordered_names: unique,
        })
    }

    /// Builds a full stitched texture atlas from `floortex.dat` via `FloortexLibrary`.
    pub fn create<S, I>(names: I, library: &mut FloortexLibrary) -> Result<Self, TextureError>
    where
        S: AsRef<str>,
        I: IntoIterator<Item = S>,
    {
        Self::create_with_decoder(names, |name| library.get_layer(name))
    }
}

/// Normalizes texture key for case-insensitive and extension-tolerant lookup.
fn normalize_key(name: &str) -> String {
    let trimmed = name.trim();
    let without_ext = if let Some(stripped) = trimmed.strip_suffix(".bmp") {
        stripped
    } else if let Some(stripped) = trimmed.strip_suffix(".BMP") {
        stripped
    } else {
        trimmed
    };
    without_ext.to_ascii_lowercase()
}

/// Deduplicates names preserving original first appearance.
fn deduplicate_names<S: AsRef<str>>(names: impl IntoIterator<Item = S>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for name in names {
        let key = normalize_key(name.as_ref());
        if !key.is_empty() && seen.insert(key) {
            result.push(name.as_ref().trim().to_string());
        }
    }
    result
}

/// Blits a tile into `cell_x, cell_y` (stride 144x144) with stretched edge padding.
fn blit_cell_with_gutter(
    pixels: &mut [u32],
    atlas_size: usize,
    cell_x: usize,
    cell_y: usize,
    layer: &TerrainLayer,
) {
    let src_w = layer.width;
    let src_h = layer.height;
    if src_w == 0 || src_h == 0 || layer.argb.len() != src_w * src_h {
        return;
    }

    // 1. Bilinear stretch of texture into the full 144x144 cell (including 8px gutter)
    for gy in 0..FloorTextureAtlas::STRIDE {
        let sy = (gy as f32 + 0.5) * (src_h as f32 / FloorTextureAtlas::STRIDE as f32) - 0.5;
        let y0 = (sy.floor() as i32).clamp(0, (src_h.saturating_sub(1)) as i32) as usize;
        let y1 = (y0 + 1).min(src_h.saturating_sub(1));
        let fy = (sy - sy.floor()).clamp(0.0, 1.0);

        let dst_row = (cell_y + gy) * atlas_size + cell_x;
        for gx in 0..FloorTextureAtlas::STRIDE {
            let sx = (gx as f32 + 0.5) * (src_w as f32 / FloorTextureAtlas::STRIDE as f32) - 0.5;
            let x0 = (sx.floor() as i32).clamp(0, (src_w.saturating_sub(1)) as i32) as usize;
            let x1 = (x0 + 1).min(src_w.saturating_sub(1));
            let fx = (sx - sx.floor()).clamp(0.0, 1.0);

            let c00 = layer.argb[y0 * src_w + x0];
            let c10 = layer.argb[y0 * src_w + x1];
            let c01 = layer.argb[y1 * src_w + x0];
            let c11 = layer.argb[y1 * src_w + x1];

            pixels[dst_row + gx] = bilerp_color(c00, c10, c01, c11, fx, fy);
        }
    }

    // 2. Clear, exact central 128x128 placement
    let inner_x = cell_x + FloorTextureAtlas::PAD;
    let inner_y = cell_y + FloorTextureAtlas::PAD;
    if src_w == FloorTextureAtlas::CELL && src_h == FloorTextureAtlas::CELL {
        for dy in 0..FloorTextureAtlas::CELL {
            let src_start = dy * FloorTextureAtlas::CELL;
            let dst_start = (inner_y + dy) * atlas_size + inner_x;
            pixels[dst_start..dst_start + FloorTextureAtlas::CELL]
                .copy_from_slice(&layer.argb[src_start..src_start + FloorTextureAtlas::CELL]);
        }
    } else {
        // Fallback resample for non-128x128 textures
        for dy in 0..FloorTextureAtlas::CELL {
            let sy = (dy as f32 + 0.5) * (src_h as f32 / FloorTextureAtlas::CELL as f32) - 0.5;
            let y0 = (sy.floor() as i32).clamp(0, (src_h.saturating_sub(1)) as i32) as usize;
            let y1 = (y0 + 1).min(src_h.saturating_sub(1));
            let fy = (sy - sy.floor()).clamp(0.0, 1.0);

            let dst_row = (inner_y + dy) * atlas_size + inner_x;
            for dx in 0..FloorTextureAtlas::CELL {
                let sx = (dx as f32 + 0.5) * (src_w as f32 / FloorTextureAtlas::CELL as f32) - 0.5;
                let x0 = (sx.floor() as i32).clamp(0, (src_w.saturating_sub(1)) as i32) as usize;
                let x1 = (x0 + 1).min(src_w.saturating_sub(1));
                let fx = (sx - sx.floor()).clamp(0.0, 1.0);

                let c00 = layer.argb[y0 * src_w + x0];
                let c10 = layer.argb[y0 * src_w + x1];
                let c01 = layer.argb[y1 * src_w + x0];
                let c11 = layer.argb[y1 * src_w + x1];

                pixels[dst_row + dx] = bilerp_color(c00, c10, c01, c11, fx, fy);
            }
        }
    }
}

/// Bilinear interpolation of two 32-bit ARGB values.
fn bilerp_color(c00: u32, c10: u32, c01: u32, c11: u32, fx: f32, fy: f32) -> u32 {
    let lerp_ch = |shift: u32| -> u32 {
        let v00 = ((c00 >> shift) & 0xFF) as f32;
        let v10 = ((c10 >> shift) & 0xFF) as f32;
        let v01 = ((c01 >> shift) & 0xFF) as f32;
        let v11 = ((c11 >> shift) & 0xFF) as f32;
        let top = v00 + (v10 - v00) * fx;
        let bot = v01 + (v11 - v01) * fx;
        (top + (bot - top) * fy).round().clamp(0.0, 255.0) as u32
    };
    (lerp_ch(24) << 24) | (lerp_ch(16) << 16) | (lerp_ch(8) << 8) | lerp_ch(0)
}

/// Fills a full 144x144 cell with a solid color.
fn fill_cell(pixels: &mut [u32], atlas_size: usize, cell_x: usize, cell_y: usize, color: u32) {
    for gy in 0..FloorTextureAtlas::STRIDE {
        let dst_start = (cell_y + gy) * atlas_size + cell_x;
        pixels[dst_start..dst_start + FloorTextureAtlas::STRIDE].fill(color);
    }
}
