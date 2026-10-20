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

/// Canonical, replayable `/brush` command text. Mirrors C# `BrushCommand.Format/Execute`:
/// `/brush height <raise|lower|smooth|flatten|roughen|water> <x,y> ... [--size 1|3|5|9|15] [--strength gentle|medium|strong]`,
/// `/brush collision <block|passable> <x,y> ... [--size 1|3|5|9|15]`,
/// `/brush material <materialId> <x,y> ... [--size N] [--auto-bridge]`.
pub struct BrushCommand;

impl BrushCommand {
    /// Canonical usage syntax string.
    pub const USAGE: &'static str = "/brush height <raise|lower|smooth|flatten|roughen|water> <x,y> ... [--size 1|3|5|9|15] [--strength gentle|medium|strong] | /brush collision <block|passable> <x,y> ... [--size 1|3|5|9|15] | /brush material <materialId> <x,y> ... [--size N] [--auto-bridge]";

    /// Formats an [`EditorBrushStroke`] into canonical, replayable command text.
    ///
    /// Validates stroke properties (non-empty path, non-negative coordinates, valid sizes,
    /// and required material names) so that every formatted command is guaranteed to parse back.
    pub fn format(stroke: &EditorBrushStroke) -> Result<String, String> {
        if stroke.path.is_empty() {
            return Err("Brush path must not be empty.".to_string());
        }
        if stroke.path.iter().any(|p| p.0 < 0 || p.1 < 0) {
            return Err("Invalid coordinate in brush path.".to_string());
        }
        if stroke.size <= 0
            || (stroke.tool != EditorBrushTool::Material
                && !EditorBrushStroke::BRUSH_SIZES.contains(&stroke.size))
        {
            return Err("Invalid brush size.".to_string());
        }
        let (tool, argument) = match stroke.tool {
            EditorBrushTool::Height => (
                "height",
                match stroke.height_operation {
                    EditorHeightBrush::Raise => "raise",
                    EditorHeightBrush::Lower => "lower",
                    EditorHeightBrush::Smooth => "smooth",
                    EditorHeightBrush::Flatten => "flatten",
                    EditorHeightBrush::Roughen => "roughen",
                    EditorHeightBrush::Water => "water",
                }
                .to_string(),
            ),
            EditorBrushTool::Collision => (
                "collision",
                if stroke.block { "block" } else { "passable" }.to_string(),
            ),
            EditorBrushTool::Material => match stroke.material.as_deref() {
                Some(m) if !m.trim().is_empty() => (
                    "material",
                    format!("\"{}\"", m.replace('\\', "\\\\").replace('"', "\\\"")),
                ),
                _ => return Err("Invalid brush tool or material.".to_string()),
            },
        };
        let path: Vec<String> = stroke.path.iter().map(|p| format!("{},{}", p.0, p.1)).collect();
        let mut text = format!("/brush {} {} {} --size {}", tool, argument, path.join(" "), stroke.size);
        if stroke.tool == EditorBrushTool::Height {
            text.push_str(match stroke.strength_level {
                0 => " --strength gentle",
                1 => " --strength medium",
                2 => " --strength strong",
                _ => return Err("Invalid strength.".to_string()),
            });
        }
        if stroke.tool == EditorBrushTool::Material && stroke.auto_bridge {
            text.push_str(" --auto-bridge");
        }
        Ok(text)
    }

    fn tokenize(text: &str) -> Result<Vec<String>, String> {
        let mut tokens = Vec::new();
        let mut cur = String::new();
        let mut in_token = false;
        let mut quoted = false;
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if quoted {
                match c {
                    '\\' => match chars.next() {
                        Some(n @ ('\\' | '"')) => cur.push(n),
                        Some(n) => {
                            cur.push('\\');
                            cur.push(n);
                        }
                        None => return Err("未結束的跳脫字元。".to_string()),
                    },
                    '"' => quoted = false,
                    _ => cur.push(c),
                }
            } else if c == '"' {
                quoted = true;
                in_token = true;
            } else if c.is_whitespace() {
                if in_token {
                    tokens.push(std::mem::take(&mut cur));
                    in_token = false;
                }
            } else {
                cur.push(c);
                in_token = true;
            }
        }
        if quoted {
            return Err("未結束的引號。".to_string());
        }
        if in_token {
            tokens.push(cur);
        }
        Ok(tokens)
    }

    /// Strictly parses a `/brush` line; nothing is executed when it fails.
    pub fn parse(text: &str, dimension: usize) -> Result<EditorBrushStroke, String> {
        let tokens = Self::tokenize(text.trim())?;
        let mut it = tokens.into_iter();
        match it.next() {
            Some(c) if c.eq_ignore_ascii_case("/brush") => {}
            _ => return Err(Self::USAGE.to_string()),
        }
        let mut positional: Vec<String> = Vec::new();
        let mut size_flag: Option<String> = None;
        let mut strength_flag: Option<String> = None;
        let mut auto_bridge = false;
        let rest: Vec<String> = it.collect();
        let mut i = 0;
        while i < rest.len() {
            let t = &rest[i];
            if let Some(name) = t.strip_prefix("--") {
                let has_value = rest.get(i + 1).is_some_and(|n| !n.starts_with("--"));
                match name.to_ascii_lowercase().as_str() {
                    "size" | "strength" => {
                        if !has_value {
                            return Err(format!("旗標 --{name} 缺少值。"));
                        }
                        let slot = if name.eq_ignore_ascii_case("size") { &mut size_flag } else { &mut strength_flag };
                        if slot.replace(rest[i + 1].clone()).is_some() {
                            return Err(format!("旗標 --{name} 重複。"));
                        }
                        i += 2;
                    }
                    "auto-bridge" => {
                        if has_value {
                            return Err("--auto-bridge 不接受參數，請放在座標之後。".to_string());
                        }
                        if auto_bridge {
                            return Err("旗標 --auto-bridge 重複。".to_string());
                        }
                        auto_bridge = true;
                        i += 1;
                    }
                    _ => return Err(format!("未知的筆刷旗標：--{name}")),
                }
            } else {
                positional.push(t.clone());
                i += 1;
            }
        }

        let tool = match positional.first().map(|s| s.to_ascii_lowercase()).as_deref() {
            Some("height") => EditorBrushTool::Height,
            Some("collision") => EditorBrushTool::Collision,
            Some("material") => EditorBrushTool::Material,
            _ => return Err(Self::USAGE.to_string()),
        };
        let argument = positional
            .get(1)
            .filter(|a| !a.trim().is_empty())
            .ok_or_else(|| Self::USAGE.to_string())?;
        let mut stroke = EditorBrushStroke::new(tool, Vec::new());
        match tool {
            EditorBrushTool::Height => {
                stroke.height_operation = match argument.to_ascii_lowercase().as_str() {
                    "raise" => EditorHeightBrush::Raise,
                    "lower" => EditorHeightBrush::Lower,
                    "smooth" => EditorHeightBrush::Smooth,
                    "flatten" => EditorHeightBrush::Flatten,
                    "roughen" => EditorHeightBrush::Roughen,
                    "water" => EditorHeightBrush::Water,
                    _ => return Err("未知的高度筆刷操作。".to_string()),
                }
            }
            EditorBrushTool::Collision => {
                stroke.block = match argument.to_ascii_lowercase().as_str() {
                    "block" => true,
                    "passable" => false,
                    _ => return Err("通行筆刷須指定 block 或 passable。".to_string()),
                }
            }
            EditorBrushTool::Material => stroke.material = Some(argument.clone()),
        }
        if (strength_flag.is_some() && tool != EditorBrushTool::Height)
            || (auto_bridge && tool != EditorBrushTool::Material)
        {
            return Err("此旗標不適用於選取的筆刷。".to_string());
        }
        if let Some(s) = size_flag {
            let size: i32 = s.parse().map_err(|_| "筆刷大小無效。".to_string())?;
            if size <= 0 || (tool != EditorBrushTool::Material && !EditorBrushStroke::BRUSH_SIZES.contains(&size)) {
                return Err("筆刷大小無效。".to_string());
            }
            stroke.size = size;
        }
        if let Some(s) = strength_flag {
            stroke.strength_level = match s.to_ascii_lowercase().as_str() {
                "gentle" => 0,
                "medium" => 1,
                "strong" => 2,
                _ => return Err("筆刷強度須為 gentle、medium 或 strong。".to_string()),
            };
        }
        stroke.auto_bridge = auto_bridge;
        for raw in &positional[2..] {
            let bad = || format!("tile 座標無效：{raw}");
            let (xs, ys) = raw.split_once(',').ok_or_else(bad)?;
            // C# int.TryParse(NumberStyles.Integer) accepts surrounding whitespace and one leading sign.
            let int = |s: &str| {
                let s = s.trim();
                let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
                if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                s.parse::<i32>().ok()
            };
            let x = int(xs).ok_or_else(bad)?;
            let y = int(ys).ok_or_else(bad)?;
            if x < 0 || y < 0 || x as usize >= dimension || y as usize >= dimension {
                return Err(bad());
            }
            stroke.path.push((x, y));
        }
        if stroke.path.is_empty() || dimension == 0 {
            return Err(Self::USAGE.to_string());
        }
        Ok(stroke)
    }

    /// Canonicalizes any valid `/brush` text into the standard canonical replay string.
    pub fn canonicalize(text: &str, dimension: usize) -> Result<String, String> {
        let stroke = Self::parse(text, dimension)?;
        Self::format(&stroke)
    }

    /// Parse, execute and commit one undo step like the C# macro command (no scope/material fallback).
    ///
    /// If execution fails (e.g., rejected material dabs or invalid map state), pending changes
    /// are rolled back immediately, leaving the terrain untouched.
    pub fn run(
        text: &str,
        heights: Option<&mut TerrainSession>,
        blend: Option<&mut TerrainBlendEditSession>,
        dimension: usize,
        surface_height: f32,
    ) -> Result<EditorBrushResult, String> {
        let stroke = Self::parse(text, dimension)?;
        let (mut heights, mut blend) = (heights, blend);
        let missing = if stroke.tool == EditorBrushTool::Material {
            blend.is_none()
        } else {
            heights.is_none()
        };
        if missing {
            return Err("此地圖沒有可編輯的筆刷圖層。".to_string());
        }
        if stroke.tool == EditorBrushTool::Collision
            && heights.as_ref().and_then(|h| h.collision()).is_none()
        {
            return Err("此地圖沒有可編輯的 collision.bmp。".to_string());
        }
        if stroke.tool == EditorBrushTool::Height
            && stroke.height_operation == EditorHeightBrush::Water
            && (!surface_height.is_finite() || surface_height < 0.0)
        {
            return Err("水面高度無效。".to_string());
        }

        let result = EditorBrushExecutor::execute(
            &stroke,
            heights.as_deref_mut(),
            blend.as_deref_mut(),
            dimension,
            surface_height,
            0,
            None,
            None,
            None,
            None,
        );

        let total_changed = result.height_samples_changed
            + result.collision_pixels_changed
            + result.material_tiles_changed;

        if result.rejected_material_dabs > 0 {
            if let Some(b) = blend.as_deref_mut() {
                b.cancel_stroke();
            }
            return Err("材質筆觸無法由原版 transition tile 表達。".to_string());
        }

        if let Some(ref notice) = result.notice {
            if total_changed == 0 {
                if let Some(h) = heights.as_deref_mut() {
                    h.cancel_stroke();
                }
                if let Some(b) = blend.as_deref_mut() {
                    b.cancel_stroke();
                }
                return Err(notice.clone());
            }
        }

        if stroke.tool == EditorBrushTool::Material {
            if let Some(b) = blend.as_deref_mut() {
                b.commit_stroke();
            }
        } else if let Some(h) = heights.as_deref_mut() {
            h.commit_stroke();
        }
        Ok(result)
    }
}

#[cfg(test)]
mod parse_compat_tests {
    use super::*;

    #[test]
    fn coordinates_follow_csharp_integer_style() {
        let s = BrushCommand::parse("/brush collision block +3,4 \" 5, 6 \" -0,+0", 10).unwrap();
        assert_eq!(s.path, vec![(3, 4), (5, 6), (0, 0)]);
        for bad in ["+,1", "1,", "--1,2", "+-1,2", "1,2,3", "-1,2", "+10,0"] {
            let cmd = format!("/brush collision block {bad}");
            assert!(BrushCommand::parse(&cmd, 10).is_err(), "{cmd}");
        }
        // canonical output drops the sign/whitespace and round-trips
        let canon = BrushCommand::canonicalize("/brush collision block +3,4", 10).unwrap();
        assert_eq!(canon, "/brush collision block 3,4 --size 1");
    }
}
