use crate::{
    scene::{ground_point, SceneObjectPosition, TILES_PER_SPRITE_PIXEL},
    terrain::TerrainHeightField,
    Mat4, Vec2,
};
use std::sync::Arc;

/// Sprite description used for pixel-accurate cursor picking.
#[derive(Clone, Debug)]
pub struct PickableSprite<'a> {
    pub width: u32,
    pub height: u32,
    pub anchor_x: f32,
    pub anchor_y: f32,
    pub argb_pixels: Option<&'a [u32]>,
    pub alpha_mask: Option<Arc<[u8]>>,
}

impl<'a> PickableSprite<'a> {
    pub fn new(
        width: u32,
        height: u32,
        anchor_x: f32,
        anchor_y: f32,
        argb_pixels: Option<&'a [u32]>,
    ) -> Self {
        Self {
            width,
            height,
            anchor_x,
            anchor_y,
            argb_pixels,
            alpha_mask: None,
        }
    }

    pub fn with_alpha_mask(
        width: u32,
        height: u32,
        anchor_x: f32,
        anchor_y: f32,
        alpha_mask: Arc<[u8]>,
    ) -> Self {
        Self {
            width,
            height,
            anchor_x,
            anchor_y,
            argb_pixels: None,
            alpha_mask: Some(alpha_mask),
        }
    }

    /// Checks if the pixel at (px, py) is non-transparent (alpha > 0).
    /// If no pixel buffer is provided, defaults to opaque (always hits).
    pub fn is_opaque(&self, px: u32, py: u32) -> bool {
        if px >= self.width || py >= self.height {
            return false;
        }
        if let Some(mask) = &self.alpha_mask {
            let idx = (py as usize)
                .saturating_mul(self.width as usize)
                .saturating_add(px as usize);
            return mask.get(idx).is_some_and(|a| *a > 0);
        }
        if let Some(pixels) = self.argb_pixels {
            let idx = (py as usize)
                .saturating_mul(self.width as usize)
                .saturating_add(px as usize);
            if idx < pixels.len() {
                return (pixels[idx] >> 24) != 0;
            }
            false
        } else {
            true
        }
    }
}

/// Projects a 4D eye-space point to 2D screen viewport coordinates (top-left origin).
pub fn project_eye_to_screen(eye: [f32; 4], projection: Mat4, viewport: Vec2) -> Vec2 {
    let clip = projection.transform4(eye);
    let w = if clip[3].abs() > 1e-12 { clip[3] } else { 1.0 };
    let screen_x = (clip[0] / w * 0.5 + 0.5) * viewport.x;
    let screen_y = (0.5 - clip[1] / w * 0.5) * viewport.y;
    Vec2::new(screen_x, screen_y)
}

/// Replicates C# `SceneObjectRenderer.PickObject`.
/// Identifies the nearest object visible under the given screen cursor point.
/// Sprites hit on opaque pixels within their projected quad, ordered far-to-near.
/// Objects without an active sprite hit if the cursor is within `marker_radius` pixels of their anchor.
pub fn pick_object(
    objects: &[SceneObjectPosition],
    sprites: &[Option<PickableSprite>],
    drawn: impl Fn(usize, &PickableSprite) -> bool,
    heights: &TerrainHeightField,
    view: Mat4,
    projection: Mat4,
    viewport: Vec2,
    point: Vec2,
    marker_radius: f32,
    marker_visible: impl Fn(usize) -> bool,
) -> Option<usize> {
    if viewport.x <= 0.0 || viewport.y <= 0.0 {
        return None;
    }

    let mut best_index: Option<usize> = None;
    let mut best_depth: f32 = f32::MAX;

    for (index, &object) in objects.iter().enumerate() {
        let sprite = sprites.get(index).and_then(|s| s.clone());
        let has_sprite = sprite.as_ref().map(|s| drawn(index, s)).unwrap_or(false);

        if !has_sprite && !marker_visible(index) {
            continue;
        }

        let anchor = ground_point(object, heights, if has_sprite { 0.0 } else { 0.25 });
        let eye = view.transform4([anchor.x, anchor.y, anchor.z, 1.0]);

        // In right-handed view space, the camera looks down -Z. eye[2] >= 0 is behind the camera.
        if eye[2] >= 0.0 {
            continue;
        }

        let hit = if let (true, Some(s)) = (has_sprite, sprite) {
            if s.width == 0 || s.height == 0 {
                false
            } else {
                let bl_eye = [
                    eye[0] - s.anchor_x * TILES_PER_SPRITE_PIXEL,
                    eye[1] + (s.anchor_y - s.height as f32) * TILES_PER_SPRITE_PIXEL,
                    eye[2],
                    eye[3],
                ];
                let tr_eye = [
                    eye[0] + (s.width as f32 - s.anchor_x) * TILES_PER_SPRITE_PIXEL,
                    eye[1] + s.anchor_y * TILES_PER_SPRITE_PIXEL,
                    eye[2],
                    eye[3],
                ];

                let bottom_left = project_eye_to_screen(bl_eye, projection, viewport);
                let top_right = project_eye_to_screen(tr_eye, projection, viewport);

                // Note: on screen, top has smaller Y, bottom has larger Y.
                let x_min = bottom_left.x.min(top_right.x);
                let x_max = bottom_left.x.max(top_right.x);
                let y_min = top_right.y.min(bottom_left.y);
                let y_max = top_right.y.max(bottom_left.y);

                if point.x < x_min || point.x >= x_max || point.y < y_min || point.y >= y_max {
                    false
                } else {
                    let span_x = (x_max - x_min).max(1e-6);
                    let span_y = (y_max - y_min).max(1e-6);
                    let px = (((point.x - x_min) / span_x) * s.width as f32)
                        .floor()
                        .clamp(0.0, (s.width - 1) as f32) as u32;
                    let py = (((point.y - y_min) / span_y) * s.height as f32)
                        .floor()
                        .clamp(0.0, (s.height - 1) as f32) as u32;
                    s.is_opaque(px, py)
                }
            }
        } else {
            let marker_screen = project_eye_to_screen(eye, projection, viewport);
            marker_screen.distance(point) <= marker_radius
        };

        let depth = -eye[2];
        if hit && depth < best_depth {
            best_depth = depth;
            best_index = Some(index);
        }
    }

    best_index
}
