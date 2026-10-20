//! Nature brush stroke driver: Bresenham gap filling, per-stroke tile dedupe, Plant/Remove
//! orchestration over `NatureEditSession`, and one undoable commit per stroke.
//! Port of C# `MapEditorForm.Nature.cs:PaintNature` / `PaintNatureTile` and
//! `TerrainStrokePath.Between`.

use super::palette::{
    NatureBrushPainter, NatureBrushRemover, NatureCatalogItem, NatureOperation, PlacedNatureObject,
};
use super::scatter::NatureDensity;
use super::session::NatureEditSession;
use crate::random::DotNetRandom;
use std::collections::HashSet;

/// Tiles after `(from)` up to and including `(to)`; mirrors C# `TerrainStrokePath.Between`.
pub fn stroke_tiles_between(from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> Vec<(i32, i32)> {
    let dx = (to_x - from_x).abs();
    let dy = -(to_y - from_y).abs();
    let sx = if from_x < to_x { 1 } else { -1 };
    let sy = if from_y < to_y { 1 } else { -1 };
    let mut error = dx + dy;
    let (mut x, mut y) = (from_x, from_y);
    let mut tiles = Vec::new();
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
        tiles.push((x, y));
    }
    tiles
}

/// Terrain height field used to seat planted objects (C# `_terrainLayers.Heights`, byte samples).
#[derive(Debug, Clone, Copy)]
pub struct NatureHeightField<'a> {
    pub vertex_size: usize,
    pub heights: &'a [u8],
    /// C# `_heightMapStep`.
    pub height_step: f32,
}

impl NatureHeightField<'_> {
    /// Nearest-vertex sample at tile-unit coordinates. C# `MathF.Round` is banker's rounding
    /// (ToEven) and `step = (VertexSize - 1) / Dimension` is integer division.
    pub fn sample_tile(&self, tile_x: f32, tile_z: f32, dimension: i32) -> f32 {
        if self.vertex_size == 0 || dimension <= 0 {
            return 0.0;
        }
        let step = ((self.vertex_size as i64 - 1) / dimension as i64) as f32;
        let max = (self.vertex_size - 1) as f32;
        let vx = (tile_x * step).round_ties_even().clamp(0.0, max) as usize;
        let vy = (tile_z * step).round_ties_even().clamp(0.0, max) as usize;
        match self.heights.get(vy * self.vertex_size + vx) {
            Some(&h) => h as f32 * self.height_step,
            None => 0.0,
        }
    }
}

/// Static brush settings for one stroke.
#[derive(Debug, Clone)]
pub struct NatureStrokeSettings<'a> {
    pub operation: NatureOperation,
    pub brush_size: i32,
    pub density: NatureDensity,
    pub dimension: i32,
    pub tile_world_size: f32,
    pub mix_species: bool,
    pub selected: Option<NatureCatalogItem>,
    pub heights: Option<NatureHeightField<'a>>,
}

/// Outcome of one `paint` call.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NatureStrokeReport {
    /// Tiles newly visited by this call (after dedupe), in visit order.
    pub visited: Vec<(i32, i32)>,
    pub planted: usize,
    pub removed_slots: usize,
    pub removed_additions: usize,
    /// Plant requested with no selected type (C# shows a status hint and skips).
    pub missing_selection: bool,
}

impl NatureStrokeReport {
    pub fn changed(&self) -> bool {
        self.planted > 0 || self.removed_slots > 0 || self.removed_additions > 0
    }
}

/// Stateful brush stroke: holds last tile and the dedupe set until `end`.
#[derive(Debug, Clone, Default)]
pub struct NatureStrokeDriver {
    last_tile: Option<(i32, i32)>,
    visited: HashSet<(i32, i32)>,
}

impl NatureStrokeDriver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_active(&self) -> bool {
        self.last_tile.is_some() || !self.visited.is_empty()
    }

    /// Paint at `(x, y)`, filling gaps from the previous tile; each tile is applied once per stroke.
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &mut self,
        session: &mut NatureEditSession,
        settings: &NatureStrokeSettings<'_>,
        level_objects: &[PlacedNatureObject],
        palette_items: &[NatureCatalogItem],
        random: &mut DotNetRandom,
        x: i32,
        y: i32,
    ) -> NatureStrokeReport {
        let tiles = match self.last_tile {
            Some((lx, ly)) => stroke_tiles_between(lx, ly, x, y),
            None => vec![(x, y)],
        };
        self.last_tile = Some((x, y));

        let mut report = NatureStrokeReport::default();
        for (tx, ty) in tiles {
            if !self.visited.insert((tx, ty)) {
                continue;
            }
            report.visited.push((tx, ty));
            Self::paint_tile(
                session,
                settings,
                level_objects,
                palette_items,
                random,
                tx,
                ty,
                &mut report,
            );
        }
        report
    }

    /// End the stroke: reset state and commit one undo step. Returns whether a step was recorded.
    pub fn end(&mut self, session: &mut NatureEditSession) -> bool {
        self.last_tile = None;
        self.visited.clear();
        session.commit_stroke()
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_tile(
        session: &mut NatureEditSession,
        settings: &NatureStrokeSettings<'_>,
        level_objects: &[PlacedNatureObject],
        palette_items: &[NatureCatalogItem],
        random: &mut DotNetRandom,
        tile_x: i32,
        tile_y: i32,
        report: &mut NatureStrokeReport,
    ) {
        match settings.operation {
            NatureOperation::Remove => {
                let (slots, additions) = NatureBrushRemover::plan_removal_tile(
                    tile_x,
                    tile_y,
                    settings.brush_size,
                    settings.tile_world_size,
                    level_objects,
                    session.additions(),
                    session.removed_slots(),
                );
                if slots.is_empty() && additions.is_empty() {
                    return;
                }
                if session.remove(&slots, &additions) {
                    report.removed_slots += slots.len();
                    report.removed_additions += additions.len();
                }
            }
            NatureOperation::Plant => {
                let Some(selected) = settings.selected.as_ref() else {
                    report.missing_selection = true;
                    return;
                };
                let tile_world = settings.tile_world_size;
                let planned = NatureBrushPainter::plan_plant(
                    tile_x,
                    tile_y,
                    settings.brush_size,
                    settings.density,
                    settings.dimension,
                    tile_world,
                    session.additions(),
                    level_objects,
                    session.removed_slots(),
                    selected,
                    palette_items,
                    settings.mix_species,
                    |wx, wz| match &settings.heights {
                        Some(field) => {
                            field.sample_tile(wx / tile_world, wz / tile_world, settings.dimension)
                        }
                        None => 0.0,
                    },
                    random,
                );
                for addition in planned {
                    session.plant(addition);
                    report.planted += 1;
                }
            }
        }
    }
}
