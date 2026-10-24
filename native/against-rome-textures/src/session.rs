//! Transactional authoring session for native Against Rome terrain blending.
//!
//! A stroke changes corner materials and bakes corresponding game tiles as a single atomic unit.
//! Any unsupported native junction rolls back the entire pending stroke, preventing partial or corrupt states.

use crate::authoring_map::{
    NativeTerrainBakeIssue, NativeTerrainCornerConflict, NativeTerrainImportResult,
    TerrainBlendAuthoringMap,
};
use crate::error::TextureError;
use crate::resolver::NativeTerrainMaterialResolver;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

pub const MAX_BRIDGE_RINGS: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainTextureChange {
    pub x: usize,
    pub y: usize,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainCornerChange {
    pub index: usize,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainBlendStroke {
    pub corners: Vec<TerrainCornerChange>,
    pub textures: Vec<TerrainTextureChange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainBlendPaintResult {
    pub succeeded: bool,
    pub texture_changes: Vec<TerrainTextureChange>,
    pub issues: Vec<NativeTerrainBakeIssue>,
}

impl TerrainBlendPaintResult {
    pub fn success(changes: Vec<TerrainTextureChange>) -> Self {
        Self {
            succeeded: true,
            texture_changes: changes,
            issues: Vec::new(),
        }
    }

    pub fn rejected(rollback: Vec<TerrainTextureChange>, issues: Vec<NativeTerrainBakeIssue>) -> Self {
        Self {
            succeeded: false,
            texture_changes: rollback,
            issues,
        }
    }
}

/// Transactional terrain blend edit session.
pub struct TerrainBlendEditSession {
    catalog: Arc<dyn NativeTerrainMaterialResolver>,
    map: TerrainBlendAuthoringMap,
    current_textures: Vec<String>,
    baseline_textures: Vec<String>,
    baseline_corners: Vec<String>,
    pending_corners: BTreeMap<usize, TerrainCornerChange>,
    pending_textures: BTreeMap<usize, TerrainTextureChange>,
    undo_stack: Vec<TerrainBlendStroke>,
    redo_stack: Vec<TerrainBlendStroke>,
    history: crate::history::HistoryTokens,
    unresolved_tile_indices: Vec<usize>,
    initial_corner_conflicts: Vec<NativeTerrainCornerConflict>,
}

impl TerrainBlendEditSession {
    pub fn history_token(&self) -> u64 { self.history.token() }
    pub fn take_history_receipts(&mut self) -> Vec<(u64, u64)> { self.history.take_receipts() }
    /// 僅清除重做，保留復原與目前筆畫。
    pub fn clear_redo(&mut self) { self.redo_stack.clear(); self.history.clear_redo(); }
    /// Creates a new edit session from an import result, source textures, and a material catalog.
    pub fn new(
        import: NativeTerrainImportResult,
        source_textures: &[String],
        catalog: Arc<dyn NativeTerrainMaterialResolver>,
    ) -> Result<Self, TextureError> {
        let expected = import.map.tile_dimension() * import.map.tile_dimension();
        if source_textures.len() != expected {
            return Err(TextureError::DimensionMismatch {
                expected,
                actual: source_textures.len(),
            });
        }

        let baseline_corners = import.map.corner_materials().to_vec();
        let current_textures = source_textures.to_vec();
        let baseline_textures = source_textures.to_vec();

        Ok(Self {
            catalog,
            map: import.map,
            current_textures,
            baseline_textures,
            baseline_corners,
            pending_corners: BTreeMap::new(),
            pending_textures: BTreeMap::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            history: Default::default(),
            unresolved_tile_indices: import.unresolved_tile_indices,
            initial_corner_conflicts: import.corner_conflicts,
        })
    }

    #[inline]
    pub fn tile_dimension(&self) -> usize {
        self.map.tile_dimension()
    }

    #[inline]
    pub fn corner_dimension(&self) -> usize {
        self.map.corner_dimension()
    }

    #[inline]
    pub fn current_textures(&self) -> &[String] {
        &self.current_textures
    }

    #[inline]
    pub fn authoring_map(&self) -> &TerrainBlendAuthoringMap {
        &self.map
    }

    #[inline]
    pub fn unresolved_tile_indices(&self) -> &[usize] {
        &self.unresolved_tile_indices
    }

    #[inline]
    pub fn initial_corner_conflicts(&self) -> &[NativeTerrainCornerConflict] {
        &self.initial_corner_conflicts
    }

    #[inline]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty() || !self.pending_corners.is_empty() || !self.pending_textures.is_empty()
    }

    #[inline]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn is_dirty(&self) -> bool {
        self.current_textures
            .iter()
            .zip(self.baseline_textures.iter())
            .any(|(a, b)| !a.eq_ignore_ascii_case(b))
            || self
                .map
                .corner_materials()
                .iter()
                .zip(self.baseline_corners.iter())
                .any(|(a, b)| !a.eq_ignore_ascii_case(b))
    }

    /// Rebinds the material resolver while retaining current corner materials, textures, and history.
    pub fn rebind_material_resolver(&mut self, catalog: Arc<dyn NativeTerrainMaterialResolver>) {
        self.catalog = catalog;
    }

    /// Creates an independent clone of the edit session for dry runs or plan previews.
    pub fn fork(&self) -> Self {
        let mut map_clone = TerrainBlendAuthoringMap::new(
            self.map.tile_dimension(),
            &self.map.corner_materials()[0],
        )
        .expect("Valid dimension");

        for y in 0..self.map.corner_dimension() {
            for x in 0..self.map.corner_dimension() {
                let c = self.map.get_corner(x, y).unwrap();
                map_clone.set_corner(x, y, c).unwrap();
            }
        }

        Self {
            catalog: Arc::clone(&self.catalog),
            map: map_clone,
            current_textures: self.current_textures.clone(),
            baseline_textures: self.baseline_textures.clone(),
            baseline_corners: self.baseline_corners.clone(),
            pending_corners: self.pending_corners.clone(),
            pending_textures: self.pending_textures.clone(),
            undo_stack: self.undo_stack.clone(),
            redo_stack: self.redo_stack.clone(),
            history: self.history.clone(),
            unresolved_tile_indices: self.unresolved_tile_indices.clone(),
            initial_corner_conflicts: self.initial_corner_conflicts.clone(),
        }
    }

    pub fn get_texture(&self, x: usize, y: usize) -> Result<&str, TextureError> {
        if x >= self.map.tile_dimension() || y >= self.map.tile_dimension() {
            return Err(TextureError::OutOfRange {
                x,
                y,
                dimension: self.map.tile_dimension(),
            });
        }
        let index = y * self.map.tile_dimension() + x;
        Ok(&self.current_textures[index])
    }

    /// Paints a circular area of corners with the specified material.
    pub fn paint_circle(
        &mut self,
        center_x: f32,
        center_y: f32,
        radius: f32,
        material_id: &str,
        rollback_stroke_on_failure: bool,
        auto_bridge: bool,
        allows_tile: Option<&dyn Fn(usize, usize) -> bool>,
    ) -> Result<TerrainBlendPaintResult, TextureError> {
        if allows_tile.is_some() && auto_bridge {
            return Err(TextureError::InvalidFormat(
                "Protected painting cannot auto-bridge beyond its boundary.".to_string(),
            ));
        }

        let corner_before = self.map.corner_materials().to_vec();
        let changed_corners = self.map.paint_circle(center_x, center_y, radius, material_id)?;

        let final_changed = if let Some(predicate) = allows_tile {
            let mut allowed = Vec::new();
            for &corner in &changed_corners {
                let corner_tiles = self.affected_tiles(&[corner]);
                let all_allowed = corner_tiles
                    .iter()
                    .all(|&tile| predicate(tile % self.map.tile_dimension(), tile / self.map.tile_dimension()));

                if all_allowed {
                    allowed.push(corner);
                } else {
                    self.map
                        .set_corner_by_index(corner, &corner_before[corner])?;
                }
            }
            allowed
        } else {
            changed_corners
        };

        Ok(self.bake_changes(
            &corner_before,
            &final_changed,
            material_id,
            rollback_stroke_on_failure,
            auto_bridge,
        ))
    }

    /// Paints all four corners of each specified tile and bakes them as a single atomic stroke.
    pub fn paint_tiles(
        &mut self,
        tiles: &[(usize, usize)],
        material_id: &str,
        auto_bridge: bool,
    ) -> Result<TerrainBlendPaintResult, TextureError> {
        let trimmed = material_id.trim();
        if trimmed.is_empty() {
            return Err(TextureError::EmptyMaterial);
        }

        for &(x, y) in tiles {
            if x >= self.map.tile_dimension() || y >= self.map.tile_dimension() {
                return Err(TextureError::OutOfRange {
                    x,
                    y,
                    dimension: self.map.tile_dimension(),
                });
            }
        }

        let corner_before = self.map.corner_materials().to_vec();
        let mut changed_set = HashSet::new();

        for &(tx, ty) in tiles {
            let tile_corners = [
                (tx, ty),
                (tx + 1, ty),
                (tx + 1, ty + 1),
                (tx, ty + 1),
            ];

            for (cx, cy) in tile_corners {
                let idx = cy * self.map.corner_dimension() + cx;
                if !self.map.get_corner(cx, cy)?.eq_ignore_ascii_case(trimmed) {
                    self.map.set_corner(cx, cy, trimmed)?;
                    changed_set.insert(idx);
                }
            }
        }

        let changed: Vec<usize> = changed_set.into_iter().collect();
        Ok(self.bake_changes(&corner_before, &changed, trimmed, false, auto_bridge))
    }

    /// Tile stamp: sets a single tile directly to a specified texture without altering corner materials.
    /// Recorded into pending stroke, supports undo/redo.
    pub fn stamp_texture(&mut self, x: usize, y: usize, texture: &str) -> Option<TerrainTextureChange> {
        let dim = self.map.tile_dimension();
        if x >= dim || y >= dim || texture.trim().is_empty() {
            return None;
        }

        let index = y * dim + x;
        let before = self.current_textures[index].clone();
        if before.eq_ignore_ascii_case(texture.trim()) {
            return None;
        }

        let trimmed = texture.trim().to_string();
        self.current_textures[index] = trimmed.clone();

        let change = TerrainTextureChange {
            x,
            y,
            before: before.clone(),
            after: trimmed.clone(),
        };

        if let Some(pending) = self.pending_textures.get_mut(&index) {
            pending.after = trimmed;
        } else {
            self.pending_textures.insert(index, change.clone());
        }

        self.redo_stack.clear();
        Some(change)
    }

    /// Commits all pending corner and texture modifications into an undoable stroke.
    /// Returns true if any changes were committed.
    pub fn commit_stroke(&mut self) -> bool {
        let mut corners: Vec<TerrainCornerChange> = self
            .pending_corners
            .values()
            .filter(|c| !c.before.eq_ignore_ascii_case(&c.after))
            .cloned()
            .collect();
        corners.sort_by_key(|c| c.index);

        let mut textures: Vec<TerrainTextureChange> = self
            .pending_textures
            .values()
            .filter(|t| !t.before.eq_ignore_ascii_case(&t.after))
            .cloned()
            .collect();
        textures.sort_by(|a, b| a.y.cmp(&b.y).then_with(|| a.x.cmp(&b.x)));

        self.pending_corners.clear();
        self.pending_textures.clear();

        if corners.is_empty() && textures.is_empty() {
            return false;
        }

        self.history.commit();
        self.undo_stack.push(TerrainBlendStroke { corners, textures });
        self.redo_stack.clear();
        true
    }

    /// Undoes the last committed stroke. Returns the texture updates to be applied to the canvas/UI.
    pub fn undo(&mut self) -> Option<Vec<TerrainTextureChange>> {
        self.commit_stroke();
        let stroke = self.undo_stack.pop()?;

        for change in stroke.corners.iter().rev() {
            let _ = self.map.set_corner_by_index(change.index, &change.before);
        }

        let dim = self.map.tile_dimension();
        for change in stroke.textures.iter().rev() {
            let index = change.y * dim + change.x;
            self.current_textures[index] = change.before.clone();
        }

        self.history.undo();
        self.redo_stack.push(stroke.clone());

        let reverted: Vec<TerrainTextureChange> = stroke
            .textures
            .iter()
            .rev()
            .map(|t| TerrainTextureChange {
                x: t.x,
                y: t.y,
                before: t.after.clone(),
                after: t.before.clone(),
            })
            .collect();

        Some(reverted)
    }

    /// Redoes the next undone stroke. Returns the texture updates to be applied to the canvas/UI.
    pub fn redo(&mut self) -> Option<Vec<TerrainTextureChange>> {
        if self.redo_stack.is_empty() {
            return None;
        }
        let stroke = self.redo_stack.pop()?;

        for change in &stroke.corners {
            let _ = self.map.set_corner_by_index(change.index, &change.after);
        }

        let dim = self.map.tile_dimension();
        for change in &stroke.textures {
            let index = change.y * dim + change.x;
            self.current_textures[index] = change.after.clone();
        }

        self.history.redo();
        self.undo_stack.push(stroke.clone());
        Some(stroke.textures)
    }

    /// Reverts all pending, uncommitted changes back to immediate before state.
    pub fn cancel_stroke(&mut self) -> Vec<TerrainTextureChange> {
        for change in self.pending_corners.values() {
            let _ = self.map.set_corner_by_index(change.index, &change.before);
        }

        let mut rollback = Vec::new();
        let dim = self.map.tile_dimension();

        for change in self.pending_textures.values() {
            let index = change.y * dim + change.x;
            let current = &self.current_textures[index];
            if !current.eq_ignore_ascii_case(&change.before) {
                rollback.push(TerrainTextureChange {
                    x: change.x,
                    y: change.y,
                    before: current.clone(),
                    after: change.before.clone(),
                });
            }
            self.current_textures[index] = change.before.clone();
        }

        self.pending_corners.clear();
        self.pending_textures.clear();
        rollback
    }

    /// Commits the current authoring state as the saved baseline.
    pub fn commit_baseline(&mut self) {
        self.commit_stroke();
        self.baseline_textures = self.current_textures.clone();
        self.baseline_corners = self.map.corner_materials().to_vec();
    }

    /// Reverts all tiles and corners to the saved baseline state, clearing undo/redo history.
    pub fn reset_to_baseline(&mut self) -> Vec<TerrainTextureChange> {
        let dim = self.map.tile_dimension();
        let mut changes = Vec::new();

        for index in 0..self.current_textures.len() {
            if !self.current_textures[index].eq_ignore_ascii_case(&self.baseline_textures[index]) {
                changes.push(TerrainTextureChange {
                    x: index % dim,
                    y: index / dim,
                    before: self.current_textures[index].clone(),
                    after: self.baseline_textures[index].clone(),
                });
                self.current_textures[index] = self.baseline_textures[index].clone();
            }
        }

        for (index, baseline_mat) in self.baseline_corners.iter().enumerate() {
            let _ = self.map.set_corner_by_index(index, baseline_mat);
        }

        self.history = Default::default();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.pending_corners.clear();
        self.pending_textures.clear();

        changes
    }

    fn bake_changes(
        &mut self,
        corner_before: &[String],
        changed_corners: &[usize],
        material_id: &str,
        rollback_stroke_on_failure: bool,
        auto_bridge: bool,
    ) -> TerrainBlendPaintResult {
        if changed_corners.is_empty() {
            return TerrainBlendPaintResult::success(Vec::new());
        }

        let mut touched: HashSet<usize> = changed_corners.iter().copied().collect();
        let (mut issues, mut baked) = self.resolve(&touched);

        if auto_bridge && !issues.is_empty() {
            let mut layer = touched.clone();
            for _ring in 0..MAX_BRIDGE_RINGS {
                if issues.is_empty() {
                    break;
                }
                let mut next_ring = HashSet::new();
                if !self.try_bridge(&mut touched, &layer, &issues, material_id, &mut next_ring) {
                    break;
                }
                layer = next_ring;
                let (new_issues, new_baked) = self.resolve(&touched);
                issues = new_issues;
                baked = new_baked;
            }
        }

        let final_corners: Vec<usize> = touched.into_iter().collect();

        if !issues.is_empty() {
            // Revert all changed corners to before state
            for &idx in &final_corners {
                let _ = self.map.set_corner_by_index(idx, &corner_before[idx]);
            }
            let rollback = if rollback_stroke_on_failure {
                self.cancel_stroke()
            } else {
                Vec::new()
            };
            return TerrainBlendPaintResult::rejected(rollback, issues);
        }

        // Record pending corners
        for &idx in &final_corners {
            let before = corner_before[idx].clone();
            let after = self.map.corner_materials()[idx].clone();

            if let Some(pending) = self.pending_corners.get_mut(&idx) {
                pending.after = after;
            } else {
                self.pending_corners.insert(
                    idx,
                    TerrainCornerChange {
                        index: idx,
                        before,
                        after,
                    },
                );
            }
        }

        // Apply baked textures and record pending
        let dim = self.map.tile_dimension();
        let mut changes = Vec::new();

        for (tile_idx, texture) in baked {
            let immediate_before = self.current_textures[tile_idx].clone();
            if immediate_before.eq_ignore_ascii_case(&texture) {
                continue;
            }

            let change = TerrainTextureChange {
                x: tile_idx % dim,
                y: tile_idx / dim,
                before: immediate_before,
                after: texture.clone(),
            };

            self.current_textures[tile_idx] = texture.clone();
            changes.push(change.clone());

            if let Some(pending) = self.pending_textures.get_mut(&tile_idx) {
                pending.after = texture;
            } else {
                self.pending_textures.insert(tile_idx, change);
            }
        }

        TerrainBlendPaintResult::success(changes)
    }

    fn resolve(&self, corners: &HashSet<usize>) -> (Vec<NativeTerrainBakeIssue>, Vec<(usize, String)>) {
        let mut baked = Vec::new();
        let mut issues = Vec::new();
        let dim = self.map.tile_dimension();

        for tile_idx in self.affected_tiles(corners) {
            let tx = tile_idx % dim;
            let ty = tile_idx / dim;
            let tile_corners = self.tile_corners(tx, ty);
            let corner_refs = [
                tile_corners[0].as_str(),
                tile_corners[1].as_str(),
                tile_corners[2].as_str(),
                tile_corners[3].as_str(),
            ];

            match self.catalog.resolve_native_tile(&corner_refs, tx, ty) {
                Some(tex) => baked.push((tile_idx, tex)),
                None => issues.push(NativeTerrainBakeIssue {
                    tile_x: tx,
                    tile_y: ty,
                    corner_material_ids: tile_corners,
                }),
            }
        }

        (issues, baked)
    }

    fn try_bridge(
        &mut self,
        touched: &mut HashSet<usize>,
        layer: &HashSet<usize>,
        issues: &[NativeTerrainBakeIssue],
        painted_material: &str,
        out_ring: &mut HashSet<usize>,
    ) -> bool {
        out_ring.clear();
        for issue in issues {
            for corner in self.tile_corner_indices(issue.tile_x, issue.tile_y) {
                if !touched.contains(&corner) {
                    out_ring.insert(corner);
                }
            }
        }

        if out_ring.is_empty() {
            return false;
        }

        let original: HashMap<usize, String> = out_ring
            .iter()
            .map(|&idx| (idx, self.map.corner_materials()[idx].clone()))
            .collect();

        let ring_affected = self.affected_tiles(&*out_ring);
        let mut inner_tiles = Vec::new();
        let mut outer_tiles = Vec::new();
        let dim = self.map.tile_dimension();

        for &tile in &ring_affected {
            let tx = tile % dim;
            let ty = tile / dim;
            let corners = self.tile_corner_indices(tx, ty);
            if corners.iter().any(|c| layer.contains(c)) {
                inner_tiles.push(tile);
            } else {
                outer_tiles.push(tile);
            }
        }

        let mut best_material: Option<String> = None;
        let mut best_outer = usize::MAX;
        let mut best_changed = usize::MAX;

        for candidate in self.catalog.material_ids() {
            if candidate.eq_ignore_ascii_case(painted_material) {
                continue;
            }

            for &idx in out_ring.iter() {
                let _ = self.map.set_corner_by_index(idx, &candidate);
            }

            let inner_ok = inner_tiles.iter().all(|&tile| {
                let tx = tile % dim;
                let ty = tile / dim;
                let c = self.tile_corners(tx, ty);
                let cr = [c[0].as_str(), c[1].as_str(), c[2].as_str(), c[3].as_str()];
                self.catalog.resolve_native_tile(&cr, 0, 0).is_some()
            });

            if inner_ok {
                let outer_issues = outer_tiles
                    .iter()
                    .filter(|&&tile| {
                        let tx = tile % dim;
                        let ty = tile / dim;
                        let c = self.tile_corners(tx, ty);
                        let cr = [c[0].as_str(), c[1].as_str(), c[2].as_str(), c[3].as_str()];
                        self.catalog.resolve_native_tile(&cr, 0, 0).is_none()
                    })
                    .count();

                let changed = out_ring
                    .iter()
                    .filter(|&&idx| !original[&idx].eq_ignore_ascii_case(&candidate))
                    .count();

                if outer_issues < best_outer || (outer_issues == best_outer && changed < best_changed) {
                    best_material = Some(candidate);
                    best_outer = outer_issues;
                    best_changed = changed;
                }
            }

            for (&idx, orig_mat) in &original {
                let _ = self.map.set_corner_by_index(idx, orig_mat);
            }
        }

        if let Some(mat) = best_material {
            for &idx in out_ring.iter() {
                let _ = self.map.set_corner_by_index(idx, &mat);
            }
            touched.extend(out_ring.iter().copied());
            true
        } else {
            false
        }
    }

    fn tile_corners(&self, x: usize, y: usize) -> [String; 4] {
        let cdim = self.map.corner_dimension();
        [
            self.map.corner_materials()[y * cdim + x].clone(),
            self.map.corner_materials()[y * cdim + (x + 1)].clone(),
            self.map.corner_materials()[(y + 1) * cdim + (x + 1)].clone(),
            self.map.corner_materials()[(y + 1) * cdim + x].clone(),
        ]
    }

    fn tile_corner_indices(&self, x: usize, y: usize) -> [usize; 4] {
        let cdim = self.map.corner_dimension();
        [
            y * cdim + x,
            y * cdim + x + 1,
            (y + 1) * cdim + x + 1,
            (y + 1) * cdim + x,
        ]
    }

    fn affected_tiles<'a, I>(&self, corner_indices: I) -> HashSet<usize>
    where
        I: IntoIterator<Item = &'a usize>,
    {
        let mut affected = HashSet::new();
        let cdim = self.map.corner_dimension();
        let dim = self.map.tile_dimension();

        for &idx in corner_indices {
            let cx = idx % cdim;
            let cy = idx / cdim;

            let min_y = if cy > 0 { cy - 1 } else { 0 };
            let max_y = cy.min(dim.saturating_sub(1));
            let min_x = if cx > 0 { cx - 1 } else { 0 };
            let max_x = cx.min(dim.saturating_sub(1));

            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    affected.insert(y * dim + x);
                }
            }
        }

        affected
    }
}
