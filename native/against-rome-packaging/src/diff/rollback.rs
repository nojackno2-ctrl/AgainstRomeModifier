use super::types::*;
use std::collections::HashSet;
use std::f32::consts::PI;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct RollbackRegionScope {
    pub min_world_x: f32,
    pub min_world_z: f32,
    pub max_world_x: f32,
    pub max_world_z: f32,
    pub layers: RollbackLayerFlags,
    pub enable_edge_blend: bool,
    pub edge_blend_radius: usize,
    pub selected_object_ids: Option<HashSet<Uuid>>,
}

impl RollbackRegionScope {
    pub fn new(
        min_world_x: f32,
        min_world_z: f32,
        max_world_x: f32,
        max_world_z: f32,
        layers: RollbackLayerFlags,
        enable_edge_blend: bool,
        edge_blend_radius: usize,
    ) -> Self {
        Self {
            min_world_x,
            min_world_z,
            max_world_x,
            max_world_z,
            layers,
            enable_edge_blend,
            edge_blend_radius,
            selected_object_ids: None,
        }
    }

    pub fn entire_map(layers: RollbackLayerFlags, selected_object_ids: Option<HashSet<Uuid>>) -> Self {
        Self {
            min_world_x: 0.0,
            min_world_z: 0.0,
            max_world_x: 16384.0,
            max_world_z: 16384.0,
            layers,
            enable_edge_blend: false,
            edge_blend_radius: 0,
            selected_object_ids,
        }
    }

    pub fn contains_world(&self, x: f32, z: f32) -> bool {
        x >= self.min_world_x && x <= self.max_world_x && z >= self.min_world_z && z <= self.max_world_z
    }
}

#[derive(Clone, Debug)]
pub struct RollbackTransaction {
    pub description: String,
    pub scope: RollbackRegionScope,
    pub height_changes: Vec<DiffTerrainSampleChange>,
    pub texture_changes: Vec<TileTextureChange>,
    pub collision_changes: Vec<CollisionTileChange>,
    pub objects_to_restore: Vec<DiffObjectItem>,
    pub objects_to_remove: Vec<Uuid>,
    pub objects_to_revert: Vec<ObjectModification>,
}

impl RollbackTransaction {
    pub fn total_changes_count(&self) -> usize {
        self.height_changes.len()
            + self.texture_changes.len()
            + self.collision_changes.len()
            + self.objects_to_restore.len()
            + self.objects_to_remove.len()
            + self.objects_to_revert.len()
    }
}

pub struct SelectiveRollbackController;

impl SelectiveRollbackController {
    pub fn compute_rollback(
        baseline: &MapVersionSnapshot,
        current: &MapVersionSnapshot,
        diff: &MapDiffReport,
        scope: &RollbackRegionScope,
    ) -> RollbackTransaction {
        let mut height_changes = Vec::new();
        let mut texture_changes = Vec::new();
        let mut collision_changes = Vec::new();
        let mut objects_to_restore = Vec::new();
        let mut objects_to_remove = Vec::new();
        let mut objects_to_revert = Vec::new();

        // 1. 地形高度局部還原
        if scope.layers.contains(RollbackLayerFlags::HEIGHT)
            && baseline.heights.is_some()
            && current.heights.is_some()
            && baseline.height_dimension > 0
            && current.height_dimension > 0
        {
            Self::compute_height_rollback(baseline, current, scope, &mut height_changes);
        }

        // 2. 地表材質局部還原
        if scope.layers.contains(RollbackLayerFlags::TEXTURES)
            && baseline.textures.is_some()
            && current.textures.is_some()
            && !diff.textures.changed_tiles.is_empty()
        {
            Self::compute_texture_rollback(baseline, current, scope, &diff.textures.changed_tiles, &mut texture_changes);
        }

        // 3. 通行碰撞遮罩局部還原
        if scope.layers.contains(RollbackLayerFlags::COLLISION)
            && baseline.collision.is_some()
            && current.collision.is_some()
            && !diff.collision.changes.is_empty()
        {
            Self::compute_collision_rollback(scope, &diff.collision.changes, &mut collision_changes);
        }

        // 4. 場景物件局部還原
        if scope.layers.contains(RollbackLayerFlags::OBJECTS) {
            Self::compute_object_rollback(scope, &diff.objects, &mut objects_to_restore, &mut objects_to_remove, &mut objects_to_revert);
        }

        let desc = format!(
            "局部還原 [{:?}] 範圍 ({:.0},{:.0}) ~ ({:.0},{:.0})",
            scope.layers, scope.min_world_x, scope.min_world_z, scope.max_world_x, scope.max_world_z
        );

        RollbackTransaction {
            description: desc,
            scope: scope.clone(),
            height_changes,
            texture_changes,
            collision_changes,
            objects_to_restore,
            objects_to_remove,
            objects_to_revert,
        }
    }

    fn compute_height_rollback(
        baseline: &MapVersionSnapshot,
        current: &MapVersionSnapshot,
        scope: &RollbackRegionScope,
        output: &mut Vec<DiffTerrainSampleChange>,
    ) {
        let dim = current.height_dimension;
        if dim != baseline.height_dimension || current.heights.is_none() || baseline.heights.is_none() {
            return;
        }

        let cur_heights = current.heights.as_ref().unwrap();
        let base_heights = baseline.heights.as_ref().unwrap();

        let world_step = 16384.0 / (dim - 1) as f32;

        let min_vx = ((scope.min_world_x / world_step).floor() as usize).min(dim - 1);
        let max_vx = ((scope.max_world_x / world_step).ceil() as usize).min(dim - 1);
        let min_vz = ((scope.min_world_z / world_step).floor() as usize).min(dim - 1);
        let max_vz = ((scope.max_world_z / world_step).ceil() as usize).min(dim - 1);

        let blend_r = scope.edge_blend_radius.max(1);

        for vz in min_vz..=max_vz {
            for vx in min_vx..=max_vx {
                let index = vz * dim + vx;
                if index >= cur_heights.len() || index >= base_heights.len() {
                    continue;
                }
                let cur_h = cur_heights[index];
                let base_h = base_heights[index];

                if cur_h == base_h {
                    continue;
                }

                let target_h = if scope.enable_edge_blend && blend_r > 0 {
                    let dist_x = (vx - min_vx).min(max_vx - vx);
                    let dist_z = (vz - min_vz).min(max_vz - vz);
                    let dist_border = dist_x.min(dist_z);

                    if dist_border < blend_r {
                        let t = dist_border as f32 / blend_r as f32;
                        let w = 0.5 * (1.0 - (PI * t).cos());
                        let blended = base_h as f32 * w + cur_h as f32 * (1.0 - w);
                        blended.round().clamp(0.0, 255.0) as u8
                    } else {
                        base_h
                    }
                } else {
                    base_h
                };

                if target_h != cur_h {
                    output.push(DiffTerrainSampleChange {
                        index,
                        before: cur_h,
                        after: target_h,
                    });
                }
            }
        }
    }

    fn compute_texture_rollback(
        _baseline: &MapVersionSnapshot,
        current: &MapVersionSnapshot,
        scope: &RollbackRegionScope,
        changed_tiles: &[TileTextureChange],
        output: &mut Vec<TileTextureChange>,
    ) {
        let t_dim = current.tile_dimension;
        if t_dim == 0 {
            return;
        }
        let tile_world_size = 16384.0 / t_dim as f32;

        let min_tile_x = ((scope.min_world_x / tile_world_size).floor() as usize).min(t_dim - 1);
        let max_tile_x = ((scope.max_world_x / tile_world_size).ceil() as usize).min(t_dim - 1);
        let min_tile_z = ((scope.min_world_z / tile_world_size).floor() as usize).min(t_dim - 1);
        let max_tile_z = ((scope.max_world_z / tile_world_size).ceil() as usize).min(t_dim - 1);

        for change in changed_tiles {
            if change.tile_x >= min_tile_x
                && change.tile_x <= max_tile_x
                && change.tile_y >= min_tile_z
                && change.tile_y <= max_tile_z
            {
                output.push(TileTextureChange {
                    tile_x: change.tile_x,
                    tile_y: change.tile_y,
                    old_texture: change.new_texture.clone(),
                    new_texture: change.old_texture.clone(),
                });
            }
        }
    }

    fn compute_collision_rollback(
        scope: &RollbackRegionScope,
        collision_changes: &[CollisionTileChange],
        output: &mut Vec<CollisionTileChange>,
    ) {
        let col_world_size = 16384.0 / 256.0;

        let min_col_x = ((scope.min_world_x / col_world_size).floor() as usize).min(255);
        let max_col_x = ((scope.max_world_x / col_world_size).ceil() as usize).min(255);
        let min_col_z = ((scope.min_world_z / col_world_size).floor() as usize).min(255);
        let max_col_z = ((scope.max_world_z / col_world_size).ceil() as usize).min(255);

        for change in collision_changes {
            if change.tile_x >= min_col_x
                && change.tile_x <= max_col_x
                && change.tile_y >= min_col_z
                && change.tile_y <= max_col_z
            {
                output.push(CollisionTileChange {
                    tile_x: change.tile_x,
                    tile_y: change.tile_y,
                    old_value: change.new_value,
                    new_value: change.old_value,
                });
            }
        }
    }

    fn compute_object_rollback(
        scope: &RollbackRegionScope,
        object_diff: &ObjectDiffRecord,
        to_restore: &mut Vec<DiffObjectItem>,
        to_remove: &mut Vec<Uuid>,
        to_revert: &mut Vec<ObjectModification>,
    ) {
        for obj in &object_diff.deleted {
            let match_filter = scope
                .selected_object_ids
                .as_ref()
                .map_or(true, |ids| ids.contains(&obj.id));
            if match_filter && scope.contains_world(obj.x, obj.z) {
                to_restore.push(obj.clone());
            }
        }

        for obj in &object_diff.added {
            let match_filter = scope
                .selected_object_ids
                .as_ref()
                .map_or(true, |ids| ids.contains(&obj.id));
            if match_filter && scope.contains_world(obj.x, obj.z) {
                to_remove.push(obj.id);
            }
        }

        for m in &object_diff.modified {
            let match_filter = scope
                .selected_object_ids
                .as_ref()
                .map_or(true, |ids| ids.contains(&m.current.id));
            let in_current = scope.contains_world(m.current.x, m.current.z);
            let in_baseline = scope.contains_world(m.baseline.x, m.baseline.z);

            if match_filter && (in_current || in_baseline) {
                to_revert.push(m.clone());
            }
        }
    }
}
