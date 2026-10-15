//! Desktop adapter for the differential-tested Rust terrain domain.
//! BMP provenance lives here; all height/collision math and history live in core.
use crate::bmp::BmpImage;
use against_rome_editor::terrain::TerrainSession as CoreSession;
pub use against_rome_editor::terrain::{
    CollisionOperation as CollisionBrushOp, Error, HeightOperation as HeightBrushOp,
    RampParameters, SampleChange, Stroke as StrokeTransaction,
};
use std::collections::HashSet;

pub struct TerrainSession {
    pub vertex_size: usize,
    pub collision_size: usize,
    pub raw_boden_bmp: Option<BmpImage>,
    pub raw_collision_bmp: Option<BmpImage>,
    core: CoreSession,
    original_heights: Vec<u8>,
    original_collision: Vec<u8>,
}

impl TerrainSession {
    pub fn try_new(
        vertex_size: usize,
        heights: Vec<u8>,
        collision_size: usize,
        collision: Vec<u8>,
        raw_boden_bmp: Option<BmpImage>,
        raw_collision_bmp: Option<BmpImage>,
    ) -> Result<Self, Error> {
        let core = CoreSession::new(
            vertex_size,
            heights.clone(),
            collision_size,
            Some(collision.clone()),
        )?;
        for (raw, size) in [
            (&raw_boden_bmp, vertex_size),
            (&raw_collision_bmp, collision_size),
        ] {
            if let Some(raw) = raw {
                if raw.width != size || raw.height != size {
                    return Err(Error::InvalidDimensions);
                }
            }
        }
        Ok(Self {
            vertex_size,
            collision_size,
            raw_boden_bmp,
            raw_collision_bmp,
            core,
            original_heights: heights,
            original_collision: collision,
        })
    }
    /// Convenience constructor for already validated editor data.
    pub fn new(
        vertex_size: usize,
        heights: Vec<u8>,
        collision_size: usize,
        collision: Vec<u8>,
        raw_boden_bmp: Option<BmpImage>,
        raw_collision_bmp: Option<BmpImage>,
    ) -> Self {
        Self::try_new(
            vertex_size,
            heights,
            collision_size,
            collision,
            raw_boden_bmp,
            raw_collision_bmp,
        )
        .expect("valid terrain dimensions")
    }
    pub fn create_blank(vertex_size: usize, base_height: u8) -> Self {
        assert!(
            vertex_size >= 2,
            "height grid requires at least two vertices"
        );
        let count = vertex_size
            .checked_mul(vertex_size)
            .expect("height grid overflow");
        Self::new(
            vertex_size,
            vec![base_height; count],
            vertex_size - 1,
            vec![0; (vertex_size - 1) * (vertex_size - 1)],
            None,
            None,
        )
    }
    pub fn heights(&self) -> &[u8] {
        self.core.heights()
    }
    pub fn collision(&self) -> &[u8] {
        self.core.collision().expect("desktop collision layer")
    }
    pub fn original_heights(&self) -> &[u8] {
        &self.original_heights
    }
    pub fn original_collision(&self) -> &[u8] {
        &self.original_collision
    }
    pub fn is_dirty(&self) -> bool {
        self.core.is_dirty()
    }
    pub fn can_undo(&self) -> bool {
        self.core.can_undo()
    }
    pub fn can_redo(&self) -> bool {
        self.core.can_redo()
    }
    pub fn commit_stroke(&mut self) -> bool {
        self.core.commit_stroke()
    }
    pub fn cancel_stroke(&mut self) -> StrokeTransaction {
        self.core.cancel_stroke()
    }
    pub fn undo(&mut self) -> bool {
        self.core.undo().is_some()
    }
    pub fn redo(&mut self) -> bool {
        self.core.redo().is_some()
    }
    pub fn apply_height_changes(&mut self, changes: &[SampleChange]) -> Vec<SampleChange> {
        self.core.apply_height_changes(changes)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn paint_height(
        &mut self,
        x: f32,
        y: f32,
        radius: f32,
        op: HeightBrushOp,
        strength: u8,
        flatten_target: Option<u8>,
        roughness_seed: i32,
    ) -> Result<Vec<SampleChange>, Error> {
        self.core.paint_height(
            (x, y),
            radius,
            op,
            strength as i32,
            flatten_target,
            roughness_seed,
        )
    }
    pub fn paint_collision(
        &mut self,
        x: f32,
        y: f32,
        radius: f32,
        op: CollisionBrushOp,
    ) -> Result<Vec<SampleChange>, Error> {
        self.core.paint_collision((x, y), radius, op)
    }
    pub fn paint_water(
        &mut self,
        x: f32,
        y: f32,
        radius: f32,
        strength: u8,
        bed: u8,
        surface: f32,
    ) -> Result<StrokeTransaction, Error> {
        self.core
            .paint_water((x, y), radius, strength as i32, bed, surface)
    }
    pub fn apply_ramp(
        &mut self,
        start: (f32, f32),
        end: (f32, f32),
        parameters: &RampParameters,
    ) -> Result<Vec<SampleChange>, Error> {
        self.core.apply_ramp(start, end, parameters)
    }
    pub fn commit_baseline(&mut self) {
        self.core.commit_baseline();
        self.original_heights = self.heights().to_vec();
        self.original_collision = self.collision().to_vec();
    }
}

/// Frame-rate independent dab path. Repeated positions are ignored until mouse-up;
/// skipped integer grid positions are filled with Bresenham samples.
#[derive(Default)]
pub struct BrushPath {
    visited: HashSet<(i32, i32)>,
    last: Option<(i32, i32)>,
    pub flatten_target: Option<u8>,
}
impl BrushPath {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn advance(&mut self, to: (i32, i32)) -> Vec<(i32, i32)> {
        let mut points = vec![];
        let Some((mut x, mut y)) = self.last.replace(to) else {
            if self.visited.insert(to) {
                points.push(to);
            }
            return points;
        };
        let dx = (to.0 - x).abs();
        let dy = -(to.1 - y).abs();
        let sx = if x < to.0 { 1 } else { -1 };
        let sy = if y < to.1 { 1 } else { -1 };
        let mut error = dx + dy;
        while (x, y) != to {
            let twice = 2 * error;
            if twice >= dy {
                error += dy;
                x += sx;
            }
            if twice <= dx {
                error += dx;
                y += sy;
            }
            if self.visited.insert((x, y)) {
                points.push((x, y));
            }
        }
        points
    }
}
