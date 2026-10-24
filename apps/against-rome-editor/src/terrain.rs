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
    history: against_rome_textures::history::HistoryTokens,
    recovery_history: Option<against_rome_textures::history::HistoryTokens>,
    receipts: Vec<crate::undo_journal::SubsystemStep>,
    recovery_swap: Option<CoreSession>,
    recovery_applied: bool,
    original_heights: Vec<u8>,
    original_collision: Vec<u8>,
}

impl TerrainSession {
    pub fn history_token(&self) -> u64 { self.history.token() }
    pub fn take_history_receipts(&mut self) -> Vec<crate::undo_journal::SubsystemStep> {
        std::mem::take(&mut self.receipts)
    }
    fn emit_receipts(&mut self) {
        self.receipts.extend(self.history.take_receipts().into_iter().map(|(before_token, after_token)| crate::undo_journal::SubsystemStep {
            subsystem: crate::undo_journal::SubsystemId::Terrain, before_token, after_token,
        }));
    }
    pub fn clear_redo(&mut self) {
        self.core.clear_redo();
        self.history.clear_redo();
        if let Some(core) = self.recovery_swap.as_mut() { core.clear_redo(); }
        if let Some(history) = self.recovery_history.as_mut() { history.clear_redo(); }
        if !self.recovery_applied { self.recovery_swap = None; self.recovery_history = None; }
    }
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
            history: Default::default(),
            recovery_history: None,
            receipts: Vec::new(),
            recovery_swap: None,
            recovery_applied: false,
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
    /// Direct access to the core session for domain operators (AI planner). Callers must commit strokes themselves.
    pub fn core_mut(&mut self) -> &mut CoreSession {
        &mut self.core
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
        self.core.is_dirty() || self.recovery_applied
    }
    pub fn can_undo(&self) -> bool {
        self.core.can_undo() || (self.recovery_swap.is_some() && self.recovery_applied)
    }
    pub fn can_redo(&self) -> bool {
        self.core.can_redo() || (self.recovery_swap.is_some() && !self.recovery_applied)
    }
    pub fn commit_stroke(&mut self) -> bool {
        let changed = self.core.commit_stroke();
        if changed { self.history.commit(); self.emit_receipts(); }
        if changed && !self.recovery_applied { self.recovery_swap = None; self.recovery_history = None; }
        changed
    }
    pub fn cancel_stroke(&mut self) -> StrokeTransaction {
        self.core.cancel_stroke()
    }
    pub fn undo(&mut self) -> bool {
        self.commit_stroke();
        if self.core.can_undo() { let done = self.core.undo().is_some(); if done { self.history.undo(); } return done; }
        self.swap_recovery(true)
    }
    pub fn redo(&mut self) -> bool {
        if !self.recovery_applied && self.recovery_swap.is_some() { return self.swap_recovery(false); }
        let done = self.core.redo().is_some();
        if done { self.history.redo(); }
        done
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
    /// 保留原始 BMP 與基準，以整個地形子系統的一步復原套用快照。
    pub(crate) fn restore_recovery(&mut self, heights: Vec<u8>, collision: Vec<u8>) -> Result<(), String> {
        let replacement = CoreSession::new(self.vertex_size, heights, self.collision_size, Some(collision)).map_err(|e| e.to_string())?;
        self.recovery_history = Some(self.history.clone());
        self.history.commit();
        self.emit_receipts();
        self.recovery_swap = Some(std::mem::replace(&mut self.core, replacement));
        self.recovery_applied = true;
        Ok(())
    }
    fn swap_recovery(&mut self, undo: bool) -> bool {
        if self.recovery_applied != undo { return false; }
        if let Some(other) = self.recovery_swap.as_mut() {
            std::mem::swap(&mut self.core, other);
            if let Some(history) = self.recovery_history.as_mut() { std::mem::swap(&mut self.history, history); }
            self.recovery_applied = !undo;
            true
        } else { false }
    }
    /// 明確捨棄時回到開圖基準，而不是復原後建立的暫時核心基準。
    pub fn reset_to_baseline(&mut self) {
        if self.recovery_applied {
            if let Some(original) = self.recovery_swap.take() { self.core = original; }
        }
        self.recovery_swap = None;
        self.recovery_applied = false;
        self.core.reset_to_baseline();
        self.history = Default::default();
        self.recovery_history = None;
    }
    pub fn commit_baseline(&mut self) {
        self.recovery_swap = None;
        self.recovery_applied = false;
        self.core.commit_baseline();
        self.history = Default::default();
        self.recovery_history = None;
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

#[cfg(test)]
mod history_tests {
    use super::*;
    fn edit(s: &mut TerrainSession, after: u8) {
        s.apply_height_changes(&[SampleChange { index: 0, before: s.heights()[0], after }]);
    }
    #[test]
    fn tokens_round_trip_branch_and_clear_only_redo() {
        let mut s = TerrainSession::create_blank(3, 10);
        let original = s.history_token();
        edit(&mut s, 20);
        assert!(s.commit_stroke());
        let first = s.history_token();
        let receipt = s.take_history_receipts();
        assert_eq!((receipt[0].before_token, receipt[0].after_token), (original, first));
        assert!(s.undo()); assert_eq!(s.history_token(), original);
        assert!(s.redo()); assert_eq!(s.history_token(), first);
        assert!(s.undo());
        edit(&mut s, 30); assert!(s.commit_stroke());
        assert_ne!(s.history_token(), first); assert!(!s.can_redo());
        assert!(s.undo()); s.clear_redo();
        assert!(!s.can_redo()); assert_eq!(s.history_token(), original);
    }
    #[test]
    fn no_op_and_cancel_emit_nothing() {
        let mut s = TerrainSession::create_blank(3, 10);
        let original = s.history_token();
        assert!(!s.commit_stroke());
        edit(&mut s, 10); assert!(!s.commit_stroke());
        edit(&mut s, 20); s.cancel_stroke(); assert!(!s.commit_stroke());
        assert_eq!(s.history_token(), original); assert!(s.take_history_receipts().is_empty());
    }
    #[test]
    fn recovery_swap_preserves_original_lineage_and_receipts() {
        let mut s = TerrainSession::create_blank(3, 10);
        edit(&mut s, 15); s.commit_stroke(); s.take_history_receipts();
        let original = s.history_token();
        s.restore_recovery(vec![20; 9], vec![1; 4]).unwrap();
        let restored = s.history_token(); assert_ne!(original, restored);
        assert_eq!(s.take_history_receipts().len(), 1);
        edit(&mut s, 25); s.commit_stroke(); s.take_history_receipts();
        assert!(s.undo()); assert_eq!(s.history_token(), restored);
        assert!(s.undo()); assert_eq!(s.history_token(), original);
        assert!(s.take_history_receipts().is_empty());
        assert!(s.redo()); assert_eq!(s.history_token(), restored);
        assert!(s.redo()); assert_eq!(s.heights()[0], 25);
        assert!(s.undo()); assert!(s.undo()); s.clear_redo();
        assert!(!s.can_redo()); assert!(s.can_undo());
    }
}
