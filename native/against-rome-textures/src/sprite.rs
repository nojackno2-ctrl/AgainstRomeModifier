//! Native sprite representation, animations, bounded catalog, and shelf-packed texture atlas.
//!
//! Directly ported from `src.MapEditor.Modules/NativeAssets/NativeSpriteAnimation.cs`,
//! `NativeSpriteAtlas.cs`, and `NativeSpriteCatalog.cs`.

use crate::error::TextureError;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

/// Default direction for units (corresponds to row 14).
pub const DEFAULT_DIRECTION: i32 = 14;

/// A cropped, straight-alpha ARGB still image.
/// AnchorX/AnchorY is the ground contact point in sprite pixels (may lie outside opaque bounds).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeSprite {
    pub width: i32,
    pub height: i32,
    pub argb_pixels: Vec<u32>,
    alpha_mask: Arc<[u8]>,
    pub anchor_x: i32,
    pub anchor_y: i32,
    pub asset_name: String,
}

impl NativeSprite {
    /// Create a new sprite, validating dimensions and pixel buffer capacity.
    pub fn new(
        width: i32,
        height: i32,
        argb_pixels: Vec<u32>,
        anchor_x: i32,
        anchor_y: i32,
        asset_name: impl Into<String>,
    ) -> Result<Self, TextureError> {
        if width <= 0 || height <= 0 {
            return Err(TextureError::InvalidSprite(
                "Sprite dimensions must be strictly positive".into(),
            ));
        }
        let expected_len = (width as usize).saturating_mul(height as usize);
        if argb_pixels.len() != expected_len {
            return Err(TextureError::InvalidSprite(format!(
                "Pixel count mismatch: expected {} for {}x{}, got {}",
                expected_len,
                width,
                height,
                argb_pixels.len()
            )));
        }
        let alpha_mask: Arc<[u8]> = argb_pixels
            .iter()
            .map(|pixel| u8::from((pixel >> 24) != 0))
            .collect::<Vec<_>>()
            .into();
        Ok(Self {
            width,
            height,
            argb_pixels,
            alpha_mask,
            anchor_x,
            anchor_y,
            asset_name: asset_name.into(),
        })
    }

    /// Shared per-pixel opacity mask for hit testing without copying full ARGB pixels.
    pub fn alpha_mask(&self) -> &Arc<[u8]> {
        &self.alpha_mask
    }

    /// Crop transparent boundaries from an uncropped ARGB pixel buffer.
    /// Anchor point is adjusted relative to the cropped minimum bounds.
    /// Returns None if the image contains no opaque pixels or has invalid dimensions.
    pub fn crop(
        name: impl Into<String>,
        width: i32,
        height: i32,
        pixels: &[u32],
        anchor_x: i32,
        anchor_y: i32,
    ) -> Option<Self> {
        if width <= 0 || height <= 0 || pixels.len() != (width as usize) * (height as usize) {
            return None;
        }

        let mut min_x = width;
        let mut min_y = height;
        let mut max_x = -1;
        let mut max_y = -1;

        let w_usize = width as usize;
        for y in 0..height {
            let row_offset = (y as usize) * w_usize;
            for x in 0..width {
                let color = pixels[row_offset + (x as usize)];
                if (color >> 24) != 0 {
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                }
            }
        }

        if max_x < 0 {
            return None;
        }

        let cropped_w = max_x - min_x + 1;
        let cropped_h = max_y - min_y + 1;
        let mut cropped = vec![0u32; (cropped_w as usize) * (cropped_h as usize)];

        for y in 0..cropped_h {
            let src_row = ((y + min_y) as usize) * w_usize;
            let dst_row = (y as usize) * (cropped_w as usize);
            for x in 0..cropped_w {
                cropped[dst_row + (x as usize)] = pixels[src_row + ((x + min_x) as usize)];
            }
        }

        Self::new(
            cropped_w,
            cropped_h,
            cropped,
            anchor_x - min_x,
            anchor_y - min_y,
            name,
        )
        .ok()
    }
}

/// A read-only cyclic animation sequence; all frame crop anchors share the same ground contact point.
#[derive(Clone, Debug)]
pub struct NativeSpriteAnimation {
    frames: Vec<Arc<NativeSprite>>,
    cycle_duration_ms: u32,
}

impl NativeSpriteAnimation {
    /// Create a new sprite animation.
    /// Requires cycle_duration_ms > 0 and at least two frames.
    pub fn new(
        frames: impl IntoIterator<Item = Arc<NativeSprite>>,
        cycle_duration_ms: u32,
    ) -> Result<Self, TextureError> {
        if cycle_duration_ms == 0 {
            return Err(TextureError::InvalidAnimation(
                "cycleDurationMs must be positive".into(),
            ));
        }
        let snapshot: Vec<Arc<NativeSprite>> = frames.into_iter().collect();
        if snapshot.len() < 2 {
            return Err(TextureError::InvalidAnimation(
                "Animation requires at least two frames".into(),
            ));
        }
        Ok(Self {
            frames: snapshot,
            cycle_duration_ms,
        })
    }

    pub fn frames(&self) -> &[Arc<NativeSprite>] {
        &self.frames
    }

    pub fn cycle_duration_ms(&self) -> u32 {
        self.cycle_duration_ms
    }

    pub fn frame_duration_ms(&self) -> f64 {
        self.cycle_duration_ms as f64 / self.frames.len() as f64
    }

    /// Selects the frame index for an elapsed time in milliseconds.
    /// Exact match for C# GetFrameIndex wrapping without rounding drift.
    pub fn get_frame_index(&self, elapsed_ms: f64) -> usize {
        if !elapsed_ms.is_finite() || elapsed_ms <= 0.0 {
            0
        } else {
            let cycle = self.cycle_duration_ms as f64;
            let count = self.frames.len();
            let rem = elapsed_ms % cycle;
            let index = (rem * (count as f64) / cycle) as usize;
            index.min(count.saturating_sub(1))
        }
    }

    pub fn get_frame(&self, elapsed_ms: f64) -> &Arc<NativeSprite> {
        let index = self.get_frame_index(elapsed_ms);
        &self.frames[index]
    }
}

/// Normalized texture rectangle of a sprite inside an atlas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSpriteUv {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

/// Shelf-packs still sprites into one ARGB texture so a scene draws in a single call.
/// Each sprite keeps a one-pixel transparent gutter against filtering bleed.
/// Sprites that do not fit are omitted instead of failing the atlas.
#[derive(Clone, Debug)]
pub struct NativeSpriteAtlas {
    width: i32,
    height: i32,
    argb_pixels: Vec<u32>,
    uv: HashMap<usize, NativeSpriteUv>,
}

struct ShelfPacker {
    max_size: i32,
    placed: Vec<(Arc<NativeSprite>, i32, i32)>,
    included: HashSet<usize>,
    x: i32,
    y: i32,
    shelf: i32,
    used_width: i32,
    used_height: i32,
}

impl ShelfPacker {
    fn new(max_size: i32) -> Self {
        Self {
            max_size,
            placed: Vec::new(),
            included: HashSet::new(),
            x: 0,
            y: 0,
            shelf: 0,
            used_width: 1,
            used_height: 1,
        }
    }

    fn place(&mut self, sprite: &Arc<NativeSprite>) -> bool {
        let ptr = Arc::as_ptr(sprite) as usize;
        if self.included.contains(&ptr) {
            return true;
        }
        let w = sprite.width + NativeSpriteAtlas::GUTTER * 2;
        let h = sprite.height + NativeSpriteAtlas::GUTTER * 2;
        if w > self.max_size || h > self.max_size {
            return false;
        }
        let mut next_x = self.x;
        let mut next_y = self.y;
        let mut next_shelf = self.shelf;
        if next_x + w > self.max_size {
            next_x = 0;
            next_y += next_shelf;
            next_shelf = 0;
        }
        if next_y + h > self.max_size {
            return false;
        }
        self.placed.push((
            sprite.clone(),
            next_x + NativeSpriteAtlas::GUTTER,
            next_y + NativeSpriteAtlas::GUTTER,
        ));
        self.included.insert(ptr);
        self.x = next_x + w;
        self.y = next_y;
        self.shelf = next_shelf.max(h);
        self.used_width = self.used_width.max(self.x);
        self.used_height = self.used_height.max(self.y + self.shelf);
        true
    }
}

impl NativeSpriteAtlas {
    pub const GUTTER: i32 = 1;

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn argb_pixels(&self) -> &[u32] {
        &self.argb_pixels
    }

    pub fn count(&self) -> usize {
        self.uv.len()
    }

    /// Look up UV coordinates for an Arc sprite instance.
    pub fn try_get_uv(&self, sprite: &Arc<NativeSprite>) -> Option<NativeSpriteUv> {
        self.uv.get(&(Arc::as_ptr(sprite) as usize)).copied()
    }

    /// Look up UV coordinates by raw sprite reference.
    pub fn try_get_uv_by_ref(&self, sprite: &NativeSprite) -> Option<NativeSpriteUv> {
        self.uv.get(&(sprite as *const NativeSprite as usize)).copied()
    }

    /// Shelf-pack still sprites with default max_size of 4096.
    pub fn pack(sprites: &[Arc<NativeSprite>], max_size: i32) -> Result<Self, TextureError> {
        Self::pack_animations(sprites, &[], max_size)
    }

    /// Pack still sprites first, then atomically add animations.
    /// Sequences that cannot fit in full do not consume extra space.
    pub fn pack_animations(
        sprites: &[Arc<NativeSprite>],
        animations: &[NativeSpriteAnimation],
        max_size: i32,
    ) -> Result<Self, TextureError> {
        if max_size < 1 {
            return Err(TextureError::InvalidAtlas(
                "maxSize must be at least 1".into(),
            ));
        }

        // Distinct by reference identity, ordered descending by Height then Width
        let mut ordered: Vec<Arc<NativeSprite>> = Vec::new();
        let mut seen = HashSet::new();
        for sprite in sprites {
            let ptr = Arc::as_ptr(sprite) as usize;
            if seen.insert(ptr) {
                ordered.push(sprite.clone());
            }
        }
        ordered.sort_by(|a, b| b.height.cmp(&a.height).then_with(|| b.width.cmp(&a.width)));

        let mut packer = ShelfPacker::new(max_size);

        for sprite in &ordered {
            packer.place(sprite);
        }

        for animation in animations {
            let state = (
                packer.x,
                packer.y,
                packer.shelf,
                packer.used_width,
                packer.used_height,
            );
            let start = packer.placed.len();
            let mut fits = true;
            for frame in animation.frames() {
                if !packer.place(frame) {
                    fits = false;
                    break;
                }
            }
            if fits {
                continue;
            }
            // Roll back placed frames for this animation
            for i in start..packer.placed.len() {
                let ptr = Arc::as_ptr(&packer.placed[i].0) as usize;
                packer.included.remove(&ptr);
            }
            packer.placed.truncate(start);
            packer.x = state.0;
            packer.y = state.1;
            packer.shelf = state.2;
            packer.used_width = state.3;
            packer.used_height = state.4;
        }

        let width = max_size.min(Self::next_power_of_two(packer.used_width));
        let height = max_size.min(Self::next_power_of_two(packer.used_height));
        let mut pixels = vec![0u32; (width as usize) * (height as usize)];
        let mut uv = HashMap::new();

        let w_usize = width as usize;
        for (sprite, left, top) in packer.placed {
            for row in 0..sprite.height {
                let src_start = (row as usize) * (sprite.width as usize);
                let src_end = src_start + (sprite.width as usize);
                let dst_start = ((top + row) as usize) * w_usize + (left as usize);
                pixels[dst_start..dst_start + (sprite.width as usize)]
                    .copy_from_slice(&sprite.argb_pixels[src_start..src_end]);
            }
            let uv_coords = NativeSpriteUv {
                u0: left as f32 / width as f32,
                v0: top as f32 / height as f32,
                u1: (left + sprite.width) as f32 / width as f32,
                v1: (top + sprite.height) as f32 / height as f32,
            };
            uv.insert(Arc::as_ptr(&sprite) as usize, uv_coords);
        }

        Ok(Self {
            width,
            height,
            argb_pixels: pixels,
            uv,
        })
    }

    fn next_power_of_two(value: i32) -> i32 {
        let mut result = 1;
        while result < value {
            result <<= 1;
        }
        result
    }
}

/// Calculate direction row for a scenario angle in degrees.
/// In game, row 14 is default direction for angle 0.
pub fn direction_for_angle(degrees: f32, rows: i32) -> i32 {
    if rows <= 1 || !degrees.is_finite() {
        return 0;
    }
    let step = (degrees / 360.0 * rows as f32).round_ties_even() as i32;
    let row = DEFAULT_DIRECTION * rows / 16 - step;
    ((row % rows) + rows) % rows
}

/// Compute finished APT linear frame index from multi-axis layout.
pub fn apt_finished_frame(layout: &[u32], frame_count: usize) -> usize {
    if frame_count == 0 {
        return 0;
    }
    let mut product: u64 = 1;
    for &size in layout {
        product = product.saturating_mul(size.max(1) as u64);
    }
    let stages = if !layout.is_empty() {
        layout[0].max(1) as u64
    } else {
        1
    };
    let frame = (stages - 1) * (product / stages);
    if (frame as usize) < frame_count {
        frame as usize
    } else {
        frame_count - 1
    }
}

/// Helper to decode animation frames safely; if any frame fails, returns None.
pub fn decode_animation_frames<F>(
    count: usize,
    cycle_duration_ms: u32,
    still: Option<Arc<NativeSprite>>,
    mut decode: F,
) -> Option<NativeSpriteAnimation>
where
    F: FnMut(usize) -> Option<Arc<NativeSprite>>,
{
    if count < 2 || cycle_duration_ms <= 1 {
        return None;
    }
    let still = still?;
    let mut frames = Vec::with_capacity(count);
    frames.push(still);
    for i in 1..count {
        let frame = decode(i)?;
        frames.push(frame);
    }
    NativeSpriteAnimation::new(frames, cycle_duration_ms).ok()
}

/// Bounded in-memory sprite and animation catalog with LRU eviction to prevent runaway allocations.
#[derive(Debug)]
pub struct BoundedSpriteCatalog {
    max_sprites: usize,
    max_animations: usize,
    sprites: HashMap<String, Arc<NativeSprite>>,
    sprite_order: VecDeque<String>,
    animations: HashMap<String, Arc<NativeSpriteAnimation>>,
    animation_order: VecDeque<String>,
}

impl BoundedSpriteCatalog {
    pub fn new(max_sprites: usize, max_animations: usize) -> Self {
        Self {
            max_sprites: max_sprites.max(1),
            max_animations: max_animations.max(1),
            sprites: HashMap::new(),
            sprite_order: VecDeque::new(),
            animations: HashMap::new(),
            animation_order: VecDeque::new(),
        }
    }

    pub fn insert_sprite(&mut self, key: impl Into<String>, sprite: Arc<NativeSprite>) {
        let k = key.into();
        if !self.sprites.contains_key(&k) {
            while self.sprites.len() >= self.max_sprites {
                if let Some(oldest) = self.sprite_order.pop_front() {
                    self.sprites.remove(&oldest);
                } else {
                    break;
                }
            }
            self.sprite_order.push_back(k.clone());
        }
        self.sprites.insert(k, sprite);
    }

    pub fn get_sprite(&self, key: &str) -> Option<Arc<NativeSprite>> {
        self.sprites.get(key).cloned()
    }

    pub fn insert_animation(
        &mut self,
        key: impl Into<String>,
        animation: Arc<NativeSpriteAnimation>,
    ) {
        let k = key.into();
        if !self.animations.contains_key(&k) {
            while self.animations.len() >= self.max_animations {
                if let Some(oldest) = self.animation_order.pop_front() {
                    self.animations.remove(&oldest);
                } else {
                    break;
                }
            }
            self.animation_order.push_back(k.clone());
        }
        self.animations.insert(k, animation);
    }

    pub fn get_animation(&self, key: &str) -> Option<Arc<NativeSpriteAnimation>> {
        self.animations.get(key).cloned()
    }

    pub fn cached_sprite_count(&self) -> usize {
        self.sprites.len()
    }

    pub fn cached_animation_count(&self) -> usize {
        self.animations.len()
    }

    /// Pack all currently cached sprites and animations into an atlas.
    pub fn build_atlas(&self, max_size: i32) -> Result<NativeSpriteAtlas, TextureError> {
        let sprites: Vec<Arc<NativeSprite>> = self.sprites.values().cloned().collect();
        let animations: Vec<NativeSpriteAnimation> =
            self.animations.values().map(|a| (**a).clone()).collect();
        NativeSpriteAtlas::pack_animations(&sprites, &animations, max_size)
    }

    pub fn clear(&mut self) {
        self.sprites.clear();
        self.sprite_order.clear();
        self.animations.clear();
        self.animation_order.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction_for_angle_ties_to_even() {
        // 90 degrees with 2 rows is exact 0.5 tie (90 / 360 * 2 = 0.5).
        // round_ties_even rounds 0.5 to 0.0, step = 0, row = 14 * 2 / 16 - 0 = 1.
        // Away-from-zero would round to 1.0, step = 1, row = 0 (the bug reported by C# oracle).
        assert_eq!(direction_for_angle(90.0, 2), 1);
        assert_eq!(direction_for_angle(270.0, 2), 1); // 1.5 rounds to 2.0 -> row 1 - 2 = -1 -> 1
        assert_eq!(direction_for_angle(450.0, 2), 1); // 2.5 rounds to 2.0 -> row 1 - 2 = -1 -> 1
        assert_eq!(direction_for_angle(630.0, 2), 1); // 3.5 rounds to 4.0 -> row 1 - 4 = -3 -> 1

        // Negative angle ties:
        assert_eq!(direction_for_angle(-90.0, 2), 1); // -0.5 rounds to 0.0 -> row 1
        assert_eq!(direction_for_angle(-270.0, 2), 1); // -1.5 rounds to -2.0 -> row 1 - (-2) = 3 -> 1
        assert_eq!(direction_for_angle(-450.0, 2), 1); // -2.5 rounds to -2.0 -> row 1 - (-2) = 3 -> 1
        assert_eq!(direction_for_angle(-630.0, 2), 1); // -3.5 rounds to -4.0 -> row 1 - (-4) = 5 -> 1

        // 4 rows ties:
        assert_eq!(direction_for_angle(45.0, 4), 3); // 0.5 -> 0, row = 3 - 0 = 3
        assert_eq!(direction_for_angle(135.0, 4), 1); // 1.5 -> 2, row = 3 - 2 = 1
        assert_eq!(direction_for_angle(225.0, 4), 1); // 2.5 -> 2, row = 3 - 2 = 1
        assert_eq!(direction_for_angle(315.0, 4), 3); // 3.5 -> 4, row = 3 - 4 = -1 -> 3
        assert_eq!(direction_for_angle(-45.0, 4), 3); // -0.5 -> 0, row = 3
        assert_eq!(direction_for_angle(-135.0, 4), 1); // -1.5 -> -2, row = 3 - (-2) = 5 -> 1
        assert_eq!(direction_for_angle(-225.0, 4), 1); // -2.5 -> -2, row = 3 - (-2) = 5 -> 1
        assert_eq!(direction_for_angle(-315.0, 4), 3); // -3.5 -> -4, row = 3 - (-4) = 7 -> 3
    }

    #[test]
    fn test_direction_for_angle_boundaries_and_negatives() {
        // 16 rows standard cases
        assert_eq!(direction_for_angle(0.0, 16), 14);
        assert_eq!(direction_for_angle(45.0, 16), 12);
        assert_eq!(direction_for_angle(90.0, 16), 10);
        assert_eq!(direction_for_angle(135.0, 16), 8);
        assert_eq!(direction_for_angle(180.0, 16), 6);
        assert_eq!(direction_for_angle(315.0, 16), 0);
        assert_eq!(direction_for_angle(360.0, 16), 14);
        assert_eq!(direction_for_angle(22.5, 16), 13);

        // Negative angles
        assert_eq!(direction_for_angle(-45.0, 16), 0);
        assert_eq!(direction_for_angle(-90.0, 16), 2);
        assert_eq!(direction_for_angle(-180.0, 16), 6);
        assert_eq!(direction_for_angle(-270.0, 16), 10);
        assert_eq!(direction_for_angle(-360.0, 16), 14);

        // Large multiples
        assert_eq!(direction_for_angle(720.0, 16), 14);
        assert_eq!(direction_for_angle(-720.0, 16), 14);
        assert_eq!(direction_for_angle(3600.0, 16), 14);
        assert_eq!(direction_for_angle(-3600.0, 16), 14);

        // 32 rows (cavalry)
        assert_eq!(direction_for_angle(0.0, 32), 28);
        assert_eq!(direction_for_angle(90.0, 32), 20);

        // Non-finite
        assert_eq!(direction_for_angle(f32::NAN, 16), 0);
        assert_eq!(direction_for_angle(f32::INFINITY, 16), 0);
        assert_eq!(direction_for_angle(f32::NEG_INFINITY, 16), 0);

        // Degenerate rows
        assert_eq!(direction_for_angle(90.0, 1), 0);
        assert_eq!(direction_for_angle(90.0, 0), 0);
        assert_eq!(direction_for_angle(90.0, -1), 0);
        assert_eq!(direction_for_angle(90.0, -16), 0);
    }
}

