//! Editor brush executor and brush stroke modeling.
//! Implements the 'AI drives editor brush tools' architecture.

use std::collections::HashSet;
use against_rome_editor::terrain::{
    CollisionOperation, HeightOperation, SampleChange, TerrainSession,
};
use against_rome_textures::session::{TerrainBlendEditSession, TerrainTextureChange};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorBrushTool {
    Height,
    Collision,
    Material,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorHeightBrush {
    Raise,
    Lower,
    Smooth,
    Flatten,
    Roughen,
    Water,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditorBrushStroke {
    pub tool: EditorBrushTool,
    pub path: Vec<(i32, i32)>,
    pub size: i32,
    pub strength_level: i32,
    pub height_operation: EditorHeightBrush,
    pub block: bool,
    pub material: Option<String>,
    pub auto_bridge: bool,
}

impl EditorBrushStroke {
    pub const BRUSH_SIZES: &'static [i32] = &[1, 3, 5, 9, 15];

    pub fn new(tool: EditorBrushTool, path: Vec<(i32, i32)>) -> Self {
        Self {
            tool,
            path,
            size: 1,
            strength_level: 1,
            height_operation: EditorHeightBrush::Raise,
            block: true,
            material: None,
            auto_bridge: false,
        }
    }

    pub fn strength_value(&self) -> i32 {
        match self.strength_level {
            0 => 2,
            2 => 14,
            _ => 6,
        }
    }

    pub fn radius_tiles(&self) -> f32 {
        self.size as f32 / 2.0 + 0.26
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EditorBrushResult {
    pub height_samples_changed: usize,
    pub collision_pixels_changed: usize,
    pub material_tiles_changed: usize,
    pub rejected_material_dabs: usize,
    pub notice: Option<String>,
}

pub struct TerrainStrokePath;

impl TerrainStrokePath {
    pub fn between(from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> Vec<(i32, i32)> {
        let dx = (to_x - from_x).abs();
        let dy = -(to_y - from_y).abs();
        let sx = if from_x < to_x { 1 } else { -1 };
        let sy = if from_y < to_y { 1 } else { -1 };
        let mut error = dx + dy;
        let mut x = from_x;
        let mut y = from_y;
        let mut result = Vec::new();

        while x != to_x || y != to_y {
            let twice = 2 * error;
            if twice >= dy {
                error += dy;
                x += sx;
            }
            if twice <= dx {
                error += dx;
                y += sy;
            }
            result.push((x, y));
        }
        result
    }
}

pub struct EditorBrushExecutor;

impl EditorBrushExecutor {
    pub const WATER_BED_DEPTH: i32 = 16;
    pub const MINIMUM_WATER_DEPTH: i32 = 4;

    pub fn water_bed_height(surface_height: f32) -> i32 {
        let surface = surface_height.floor() as i32;
        if surface < Self::MINIMUM_WATER_DEPTH {
            -1
        } else {
            surface - surface.min(Self::WATER_BED_DEPTH)
        }
    }

    pub fn allows_vertex(x: f32, y: f32, dimension: i32, allows_tile: &dyn Fn(i32, i32) -> bool) -> bool {
        let left = (x.ceil() as i32) - 1;
        let right = x.floor() as i32;
        let top = (y.ceil() as i32) - 1;
        let bottom = y.floor() as i32;
        for ty in top..=bottom {
            for tx in left..=right {
                if tx >= 0 && ty >= 0 && tx < dimension && ty < dimension && !allows_tile(tx, ty) {
                    return false;
                }
            }
        }
        true
    }

    pub fn execute(
        stroke: &EditorBrushStroke,
        heights: Option<&mut TerrainSession>,
        blend: Option<&mut TerrainBlendEditSession>,
        dimension: usize,
        surface_height: f32,
        roughness_seed: i32,
        allows_tile: Option<&dyn Fn(i32, i32) -> bool>,
        paint_material_fallback: Option<&dyn Fn(&str, f32, f32, f32) -> bool>,
        on_texture: Option<&mut dyn FnMut(TerrainTextureChange)>,
        on_rejected_dab: Option<&mut dyn FnMut(i32, i32)>,
    ) -> EditorBrushResult {
        let mut flatten_target = -1;
        Self::execute_internal(
            stroke,
            heights,
            blend,
            dimension,
            surface_height,
            &mut flatten_target,
            roughness_seed,
            allows_tile,
            paint_material_fallback,
            on_texture,
            on_rejected_dab,
        )
    }

    pub fn execute_internal(
        stroke: &EditorBrushStroke,
        mut heights: Option<&mut TerrainSession>,
        mut blend: Option<&mut TerrainBlendEditSession>,
        dimension: usize,
        surface_height: f32,
        flatten_target: &mut i32,
        roughness_seed: i32,
        allows_tile: Option<&dyn Fn(i32, i32) -> bool>,
        paint_material_fallback: Option<&dyn Fn(&str, f32, f32, f32) -> bool>,
        mut on_texture: Option<&mut dyn FnMut(TerrainTextureChange)>,
        mut on_rejected_dab: Option<&mut dyn FnMut(i32, i32)>,
    ) -> EditorBrushResult {
        if dimension == 0 || stroke.path.is_empty() {
            return EditorBrushResult::default();
        }

        let mut visited = HashSet::new();
        let mut height_count = 0usize;
        let mut collision_count = 0usize;
        let mut texture_count = 0usize;
        let mut rejected = 0usize;
        let mut notice: Option<String> = None;
        let mut last: Option<(i32, i32)> = None;

        for &(px, py) in &stroke.path {
            let tiles = match last {
                Some(l) => TerrainStrokePath::between(l.0, l.1, px, py),
                None => vec![(px, py)],
            };
            last = Some((px, py));

            for (x0, y0) in tiles {
                let x = x0.clamp(0, dimension as i32 - 1);
                let y = y0.clamp(0, dimension as i32 - 1);
                if !visited.insert((x, y)) {
                    continue;
                }
                if let Some(allows) = allows_tile {
                    if !allows(x, y) {
                        continue;
                    }
                }

                match stroke.tool {
                    EditorBrushTool::Height => {
                        let h_session = match heights.as_deref_mut() {
                            Some(s) => s,
                            None => {
                                return EditorBrushResult {
                                    height_samples_changed: height_count,
                                    collision_pixels_changed: collision_count,
                                    material_tiles_changed: texture_count,
                                    rejected_material_dabs: rejected,
                                    notice: Some("此地圖沒有可編輯的高度圖。".to_string()),
                                };
                            }
                        };

                        let (h, c, n) = Self::paint_height_tile(
                            stroke,
                            h_session,
                            dimension,
                            x,
                            y,
                            surface_height,
                            flatten_target,
                            roughness_seed,
                            allows_tile,
                        );
                        height_count += h;
                        collision_count += c;
                        if notice.is_none() {
                            notice = n.clone();
                        }
                        if n.is_some()
                            && c == 0
                            && h == 0
                            && stroke.height_operation == EditorHeightBrush::Water
                            && *flatten_target < 0
                        {
                            return EditorBrushResult {
                                height_samples_changed: height_count,
                                collision_pixels_changed: collision_count,
                                material_tiles_changed: texture_count,
                                rejected_material_dabs: rejected,
                                notice: n,
                            };
                        }
                    }
                    EditorBrushTool::Collision => {
                        let h_session = match heights.as_deref_mut() {
                            Some(s) if s.collision().is_some() => s,
                            _ => {
                                return EditorBrushResult {
                                    height_samples_changed: height_count,
                                    collision_pixels_changed: collision_count,
                                    material_tiles_changed: texture_count,
                                    rejected_material_dabs: rejected,
                                    notice: Some("此地圖沒有可編輯的 collision.bmp。".to_string()),
                                };
                            }
                        };

                        let cstep = h_session.collision_size() as f32 / dimension as f32;
                        let center_x = (x as f32 + 0.5) * cstep;
                        let center_y = (y as f32 + 0.5) * cstep;
                        let radius = stroke.radius_tiles() * cstep;
                        let op = if stroke.block {
                            CollisionOperation::Block
                        } else {
                            CollisionOperation::Clear
                        };

                        let changes = h_session
                            .paint_collision_where(
                                (center_x, center_y),
                                radius,
                                op,
                                |cx, cy| match allows_tile {
                                    Some(pred) => {
                                        let tx = (cx as f32 / cstep) as i32;
                                        let ty = (cy as f32 / cstep) as i32;
                                        pred(
                                            tx.min(dimension as i32 - 1),
                                            ty.min(dimension as i32 - 1),
                                        )
                                    }
                                    None => true,
                                },
                            )
                            .unwrap_or_default();
                        collision_count += changes.len();
                    }
                    EditorBrushTool::Material => {
                        let material_id = match stroke.material.as_deref() {
                            Some(m) => m,
                            None => continue,
                        };

                        if let Some(b_session) = blend.as_deref_mut() {
                            let allows_closure = allows_tile.map(|pred| {
                                Box::new(move |tx: usize, ty: usize| pred(tx as i32, ty as i32))
                                    as Box<dyn Fn(usize, usize) -> bool>
                            });
                            let allows_ref = allows_closure.as_ref().map(|b| b.as_ref());

                            let paint_res = b_session.paint_circle(
                                x as f32 + 0.5,
                                y as f32 + 0.5,
                                stroke.radius_tiles(),
                                material_id,
                                true,
                                stroke.auto_bridge,
                                allows_ref,
                            );

                            match paint_res {
                                Ok(res) => {
                                    if !res.succeeded {
                                        rejected += 1;
                                        if let Some(ref mut cb) = on_rejected_dab {
                                            cb(x, y);
                                        }
                                    }
                                    for change in res.texture_changes {
                                        texture_count += 1;
                                        if let Some(ref mut cb) = on_texture {
                                            cb(change);
                                        }
                                    }
                                }
                                Err(_) => {
                                    rejected += 1;
                                    if let Some(ref mut cb) = on_rejected_dab {
                                        cb(x, y);
                                    }
                                }
                            }
                        } else if let Some(fallback) = paint_material_fallback {
                            let ok = fallback(
                                material_id,
                                x as f32 + 0.5,
                                y as f32 + 0.5,
                                stroke.radius_tiles(),
                            );
                            if !ok {
                                rejected += 1;
                                if let Some(cb) = on_rejected_dab.as_deref_mut() {
                                    cb(x, y);
                                }
                            } else {
                                texture_count += 1;
                            }
                        } else {
                            return EditorBrushResult {
                                height_samples_changed: height_count,
                                collision_pixels_changed: collision_count,
                                material_tiles_changed: texture_count,
                                rejected_material_dabs: rejected,
                                notice: Some("此地圖沒有可編輯的材質。".to_string()),
                            };
                        }
                    }
                }
            }
        }

        EditorBrushResult {
            height_samples_changed: height_count,
            collision_pixels_changed: collision_count,
            material_tiles_changed: texture_count,
            rejected_material_dabs: rejected,
            notice,
        }
    }

    fn paint_height_tile(
        stroke: &EditorBrushStroke,
        heights: &mut TerrainSession,
        dimension: usize,
        tile_x: i32,
        tile_y: i32,
        surface_height: f32,
        flatten_target: &mut i32,
        roughness_seed: i32,
        allows_tile: Option<&dyn Fn(i32, i32) -> bool>,
    ) -> (usize, usize, Option<String>) {
        let step = (heights.vertex_size() - 1) as f32 / dimension as f32;
        let center_x = (tile_x as f32 + 0.5) * step;
        let center_y = (tile_y as f32 + 0.5) * step;
        let radius = stroke.radius_tiles() * step + 1.0;
        let strength = stroke.strength_value();

        if stroke.height_operation == EditorHeightBrush::Water {
            let bed = Self::water_bed_height(surface_height);
            if bed < 0 {
                return (0, 0, Some("水面太低，無法使用水域筆刷；請先設定水面高度。".to_string()));
            }
            *flatten_target = bed;

            let water_res = match heights.paint_water(
                (center_x, center_y),
                radius,
                strength,
                bed as u8,
                surface_height,
            ) {
                Ok(w) => w,
                Err(e) => return (0, 0, Some(format!("{:?}", e))),
            };

            let protected_heights = Self::protect_heights(
                heights,
                &water_res.heights,
                dimension,
                step,
                allows_tile,
            );

            let mut protected_collision = 0usize;
            if let Some(pred) = allows_tile {
                if !water_res.collision.is_empty() {
                    let cstep = heights.collision_size() as f32 / dimension as f32;
                    let csize = heights.collision_size();
                    let rejected_set: HashSet<usize> = water_res
                        .collision
                        .iter()
                        .filter(|change| {
                            let tx = ((change.index % csize) as f32 / cstep) as i32;
                            let ty = ((change.index / csize) as f32 / cstep) as i32;
                            !pred(tx.min(dimension as i32 - 1), ty.min(dimension as i32 - 1))
                        })
                        .map(|c| c.index)
                        .collect();

                    let scale = heights.collision_size() as f32 / (heights.vertex_size() - 1) as f32;
                    let revert_col = heights
                        .paint_collision_where(
                            (center_x * scale, center_y * scale),
                            radius * scale,
                            CollisionOperation::Clear,
                            |x, y| rejected_set.contains(&(y * csize + x)),
                        )
                        .unwrap_or_default();
                    protected_collision = revert_col.len();
                }
            }

            let effective_h = water_res.heights.len().saturating_sub(protected_heights);
            let effective_c = water_res.collision.len().saturating_sub(protected_collision);
            let notice = if heights.collision().is_some() {
                None
            } else {
                Some("此地圖沒有 collision.bmp；水域通行區域無法自動更新。".to_string())
            };
            return (effective_h, effective_c, notice);
        }

        let op = match stroke.height_operation {
            EditorHeightBrush::Lower => HeightOperation::Lower,
            EditorHeightBrush::Smooth => HeightOperation::Smooth,
            EditorHeightBrush::Flatten => HeightOperation::Flatten,
            EditorHeightBrush::Roughen => HeightOperation::Roughen,
            _ => HeightOperation::Raise,
        };

        if op == HeightOperation::Flatten && *flatten_target < 0 {
            let vx = (center_x.round() as usize).clamp(0, heights.vertex_size() - 1);
            let vy = (center_y.round() as usize).clamp(0, heights.vertex_size() - 1);
            *flatten_target = heights.heights()[vy * heights.vertex_size() + vx] as i32;
        }

        let changes = heights
            .paint_height(
                (center_x, center_y),
                radius,
                op,
                strength,
                if op == HeightOperation::Flatten {
                    Some(*flatten_target as u8)
                } else {
                    None
                },
                roughness_seed,
            )
            .unwrap_or_default();

        let protected_count = Self::protect_heights(
            heights,
            &changes,
            dimension,
            step,
            allows_tile,
        );

        (changes.len().saturating_sub(protected_count), 0, None)
    }

    fn protect_heights(
        heights: &mut TerrainSession,
        changes: &[SampleChange],
        dimension: usize,
        step: f32,
        allows_tile: Option<&dyn Fn(i32, i32) -> bool>,
    ) -> usize {
        let pred = match allows_tile {
            Some(p) => p,
            None => return 0,
        };

        let vsize = heights.vertex_size();
        let revert: Vec<SampleChange> = changes
            .iter()
            .filter(|c| {
                let vx = (c.index % vsize) as f32 / step;
                let vy = (c.index / vsize) as f32 / step;
                !Self::allows_vertex(vx, vy, dimension as i32, pred)
            })
            .map(|c| SampleChange {
                index: c.index,
                before: c.after,
                after: c.before,
            })
            .collect();

        let count = revert.len();
        if !revert.is_empty() {
            heights.apply_height_changes(&revert);
        }
        count
    }
}
