//! Texture-layer edit history and stroke path interpolation.
//!
//! Ports C# `TerrainEditHistory` and `TerrainStrokePath`: stroke grouping, dirty tracking,
//! undo/redo and save baselines, with no UI dependency. Texture names compare ordinal
//! case-insensitively like the C# `StringComparer.OrdinalIgnoreCase`.
use crate::terrain::Error;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureChange {
    pub x: usize,
    pub y: usize,
    pub before: String,
    pub after: String,
}

fn same(a: &str, b: &str) -> bool {
    a.chars()
        .flat_map(char::to_uppercase)
        .eq(b.chars().flat_map(char::to_uppercase))
}

#[derive(Debug, Clone)]
pub struct TextureHistory {
    dimension: usize,
    current: Vec<String>,
    baseline: Vec<String>,
    undo: Vec<Vec<TextureChange>>,
    redo: Vec<Vec<TextureChange>>,
    pending: BTreeMap<usize, TextureChange>,
    pending_order: Vec<usize>,
    pending_seen: BTreeSet<usize>,
}

impl TextureHistory {
    pub fn new(dimension: usize, textures: Vec<String>) -> Result<Self, Error> {
        if dimension == 0 || dimension.checked_mul(dimension) != Some(textures.len()) {
            return Err(Error::InvalidDimensions);
        }
        Ok(Self {
            dimension,
            baseline: textures.clone(),
            current: textures,
            undo: vec![],
            redo: vec![],
            pending: BTreeMap::new(),
            pending_order: vec![],
            pending_seen: BTreeSet::new(),
        })
    }
    pub fn current(&self) -> &[String] {
        &self.current
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || !self.pending.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn is_dirty(&self) -> bool {
        self.current
            .iter()
            .zip(&self.baseline)
            .any(|(a, b)| !same(a, b))
    }
    /// Paints one tile; returns the immediate change, or `None` when the name is unchanged
    /// (case-insensitively). Out-of-range tiles and blank names are rejected.
    pub fn paint(
        &mut self,
        x: usize,
        y: usize,
        texture: &str,
    ) -> Result<Option<TextureChange>, Error> {
        if x >= self.dimension || y >= self.dimension || texture.trim().is_empty() {
            return Err(Error::InvalidParameters);
        }
        let index = y * self.dimension + x;
        let immediate_before = self.current[index].clone();
        if same(&immediate_before, texture) {
            return Ok(None);
        }
        self.current[index] = texture.to_string();
        match self.pending.get_mut(&index) {
            Some(existing) => {
                if same(&existing.before, texture) {
                    self.pending.remove(&index);
                } else {
                    existing.after = texture.to_string();
                }
            }
            None => {
                self.pending.insert(
                    index,
                    TextureChange {
                        x,
                        y,
                        before: immediate_before.clone(),
                        after: texture.to_string(),
                    },
                );
                if self.pending_seen.insert(index) {
                    self.pending_order.push(index);
                }
            }
        }
        Ok(Some(TextureChange {
            x,
            y,
            before: immediate_before,
            after: texture.to_string(),
        }))
    }
    fn clear_pending(&mut self) {
        self.pending.clear();
        self.pending_order.clear();
        self.pending_seen.clear();
    }
    pub fn commit_stroke(&mut self) -> bool {
        let stroke: Vec<TextureChange> = self
            .pending_order
            .iter()
            .filter_map(|i| self.pending.get(i).cloned())
            .collect();
        self.clear_pending();
        if stroke.is_empty() {
            return false;
        }
        self.undo.push(stroke);
        self.redo.clear();
        true
    }
    pub fn undo(&mut self) -> Option<Vec<TextureChange>> {
        self.commit_stroke();
        let stroke = self.undo.pop()?;
        for c in stroke.iter().rev() {
            self.current[c.y * self.dimension + c.x] = c.before.clone();
        }
        self.redo.push(stroke.clone());
        Some(stroke)
    }
    pub fn redo(&mut self) -> Option<Vec<TextureChange>> {
        let stroke = self.redo.pop()?;
        for c in &stroke {
            self.current[c.y * self.dimension + c.x] = c.after.clone();
        }
        self.undo.push(stroke.clone());
        Some(stroke)
    }
    /// Restores the baseline, clears all history and returns the applied tile changes.
    pub fn reset_to_baseline(&mut self) -> Vec<TextureChange> {
        let mut changes = vec![];
        for index in 0..self.current.len() {
            if same(&self.current[index], &self.baseline[index]) {
                continue;
            }
            changes.push(TextureChange {
                x: index % self.dimension,
                y: index / self.dimension,
                before: self.current[index].clone(),
                after: self.baseline[index].clone(),
            });
            self.current[index] = self.baseline[index].clone();
        }
        self.undo.clear();
        self.redo.clear();
        self.clear_pending();
        changes
    }
    pub fn commit_baseline(&mut self) {
        self.commit_stroke();
        self.baseline = self.current.clone();
    }
}

/// Bresenham tiles between two points, excluding the start and including the end.
pub fn stroke_path_between(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let dx = (to.0 - from.0).abs();
    let dy = -(to.1 - from.1).abs();
    let sx = if from.0 < to.0 { 1 } else { -1 };
    let sy = if from.1 < to.1 { 1 } else { -1 };
    let mut error = dx + dy;
    let (mut x, mut y) = from;
    let mut out = vec![];
    while x != to.0 || y != to.1 {
        let twice = 2 * error;
        if twice >= dy {
            error += dy;
            x += sx;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
        out.push((x, y));
    }
    out
}
