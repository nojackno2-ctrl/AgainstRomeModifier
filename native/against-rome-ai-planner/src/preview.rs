//! AiMapPlanPreview: generates visual map plan preview without saving to disk.

use against_rome_editor::terrain::TerrainSession;
use against_rome_textures::session::TerrainBlendEditSession;
use crate::operator::AiEditorOperator;
use crate::plan::{AiMapApplyResult, AiMapPlan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl RgbaImage {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![255; width * height * 4],
        }
    }

    pub fn get_pixel(&self, x: usize, y: usize) -> (u8, u8, u8, u8) {
        let idx = (y * self.width + x) * 4;
        (
            self.pixels[idx],
            self.pixels[idx + 1],
            self.pixels[idx + 2],
            self.pixels[idx + 3],
        )
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, r: u8, g: u8, b: u8, a: u8) {
        let idx = (y * self.width + x) * 4;
        self.pixels[idx] = r;
        self.pixels[idx + 1] = g;
        self.pixels[idx + 2] = b;
        self.pixels[idx + 3] = a;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiMapPlanPreview {
    pub image: RgbaImage,
    pub changes: AiMapApplyResult,
    pub has_collision: bool,
    pub issues: Vec<String>,
}

pub struct AiMapPlanPreviewBuilder;

impl AiMapPlanPreviewBuilder {
    pub fn build(
        plan: &AiMapPlan,
        source: &TerrainSession,
        materials: Option<&TerrainBlendEditSession>,
        dimension: usize,
        water: f32,
        diagnose: Option<&dyn Fn(&TerrainSession) -> Vec<String>>,
    ) -> AiMapPlanPreview {
        let mut heights = TerrainSession::new(
            source.vertex_size(),
            source.heights().to_vec(),
            source.collision_size(),
            source.collision().map(|c| c.to_vec()),
        )
        .expect("Valid session clone");

        let mut blend = materials.map(|m| m.fork());
        let mut rejected: Vec<(f32, f32, f32)> = Vec::new();

        let mut on_rejected = |x: f32, y: f32, radius: f32| {
            rejected.push((x, y, radius));
        };

        let changes = AiEditorOperator::apply(
            plan,
            Some(&mut heights),
            blend.as_mut(),
            dimension,
            water,
            true,
            None,
            None,
            Some(&mut on_rejected),
        );

        let img_width = heights.vertex_size();
        let img_height = heights.vertex_size();
        let mut image = RgbaImage::new(img_width, img_height);

        for y in 0..img_height {
            for x in 0..img_width {
                let height = heights.heights()[y * img_width + x];
                let mut color = if (height as f32) < water {
                    let depth = ((water.floor() as i32 - height as i32).clamp(0, 30)) as u8;
                    (
                        19u8.saturating_add(depth),
                        127u8.saturating_sub(depth.saturating_mul(2)),
                        202u8.saturating_sub(depth.saturating_mul(2)),
                    )
                } else {
                    (
                        65 + height / 2,
                        155 - height / 4,
                        55 + height / 4,
                    )
                };

                if let (Some(b), Some(m)) = (blend.as_ref(), materials) {
                    let tx = (x * dimension / img_width).min(dimension - 1);
                    let ty = (y * dimension / img_height).min(dimension - 1);
                    let tile = ty * dimension + tx;
                    if x % 4 == 0
                        && y % 4 == 0
                        && !b.current_textures()[tile].eq_ignore_ascii_case(&m.current_textures()[tile])
                    {
                        color = (215, 145, 45);
                    }
                }

                if x % 4 == 0
                    && y % 4 == 0
                    && rejected.iter().any(|area| {
                        let px = x as f32 * dimension as f32 / img_width as f32;
                        let py = y as f32 * dimension as f32 / img_height as f32;
                        (px - area.0).powi(2) + (py - area.1).powi(2) <= area.2 * area.2
                    })
                {
                    color = (180, 60, 200);
                }

                if heights.collision().is_some() {
                    let csize = heights.collision_size();
                    let cx = (x * csize / img_width).min(csize - 1);
                    let cy = (y * csize / img_height).min(csize - 1);
                    if heights.collision().unwrap()[cy * csize + cx] > 0 {
                        color = (195, color.1 / 2, color.2 / 2);
                    }
                }

                if let Some(scope) = &plan.edit_scope {
                    let tx = (x * dimension / img_width).min(dimension - 1) as i32;
                    let ty = (y * dimension / img_height).min(dimension - 1) as i32;
                    if !scope.allows_tile(tx, ty) {
                        color = (
                            ((color.0 as u16 + 80) / 2) as u8,
                            ((color.1 as u16 + 80) / 2) as u8,
                            ((color.2 as u16 + 80) / 2) as u8,
                        );
                    }
                }

                image.set_pixel(x, y, color.0, color.1, color.2, 255);
            }
        }

        let issues = match diagnose {
            Some(d) => d(&heights),
            None => Vec::new(),
        };

        AiMapPlanPreview {
            image,
            changes,
            has_collision: heights.collision().is_some(),
            issues,
        }
    }
}
