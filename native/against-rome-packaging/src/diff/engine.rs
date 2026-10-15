use super::types::*;
use against_rome_scenario::model::Event;
use chrono::Utc;
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct MapDiffOptions {
    pub height_tolerance: f32,
    pub position_tolerance: f32,
    pub rotation_tolerance: f32,
    pub spatial_match_radius: f32,
}

impl Default for MapDiffOptions {
    fn default() -> Self {
        Self {
            height_tolerance: 0.0,
            position_tolerance: 0.5,
            rotation_tolerance: 1.0,
            spatial_match_radius: 150.0,
        }
    }
}

pub struct MapDiffEngine;

impl MapDiffEngine {
    pub fn compare(
        baseline: &MapVersionSnapshot,
        current: &MapVersionSnapshot,
        options: Option<&MapDiffOptions>,
    ) -> MapDiffReport {
        let default_opt = MapDiffOptions::default();
        let options = options.unwrap_or(&default_opt);

        let height_diff = Self::compare_heightmaps(baseline, current, options);
        let texture_diff = Self::compare_textures(baseline, current);
        let collision_diff = Self::compare_collision(baseline, current);
        let object_diff = Self::compare_objects(baseline.objects.as_deref(), current.objects.as_deref(), options);
        let event_diff = Self::compare_events(baseline.events.as_deref(), current.events.as_deref());

        MapDiffReport {
            compared_at: Utc::now(),
            baseline_label: baseline.version_label.clone(),
            current_label: current.version_label.clone(),
            height: height_diff,
            textures: texture_diff,
            collision: collision_diff,
            objects: object_diff,
            events: event_diff,
        }
    }

    fn compare_heightmaps(
        baseline: &MapVersionSnapshot,
        current: &MapVersionSnapshot,
        options: &MapDiffOptions,
    ) -> HeightDiffRecord {
        let dim = baseline.height_dimension.max(current.height_dimension);
        if dim == 0 || baseline.heights.is_none() || current.heights.is_none() {
            return HeightDiffRecord {
                dimension: 0,
                delta_grid: Vec::new(),
                changed_vertex_count: 0,
                max_raise: 0.0,
                max_lower: 0.0,
                volume_delta: 0.0,
                sample_changes: Vec::new(),
            };
        }

        let b_dim = baseline.height_dimension;
        let c_dim = current.height_dimension;
        let b_step = baseline.height_step;
        let c_step = current.height_step;
        let b_heights = baseline.heights.as_ref().unwrap();
        let c_heights = current.heights.as_ref().unwrap();

        let mut deltas = vec![0.0f32; dim * dim];
        let mut changed_count = 0;
        let mut max_raise = 0.0f32;
        let mut max_lower = 0.0f32;
        let mut total_volume = 0.0f32;
        let mut samples = Vec::new();

        for y in 0..dim {
            for x in 0..dim {
                let b_val = Self::sample_height(b_heights, b_dim, dim, x, y);
                let c_val = Self::sample_height(c_heights, c_dim, dim, x, y);

                let b_world = b_val as f32 * b_step;
                let c_world = c_val as f32 * c_step;
                let delta = c_world - b_world;

                deltas[y * dim + x] = delta;

                if delta.abs() > options.height_tolerance {
                    changed_count += 1;
                    if delta > 0.0 {
                        if delta > max_raise {
                            max_raise = delta;
                        }
                    } else {
                        let lower = -delta;
                        if lower > max_lower {
                            max_lower = lower;
                        }
                    }
                    total_volume += delta;

                    if samples.len() < 500 {
                        samples.push(HeightVertexChange {
                            vertex_x: x,
                            vertex_y: y,
                            old_height: b_val,
                            new_height: c_val,
                            delta,
                        });
                    }
                }
            }
        }

        HeightDiffRecord {
            dimension: dim,
            delta_grid: deltas,
            changed_vertex_count: changed_count,
            max_raise,
            max_lower,
            volume_delta: total_volume,
            sample_changes: samples,
        }
    }

    fn sample_height(heights: &[u8], src_dim: usize, target_dim: usize, target_x: usize, target_y: usize) -> u8 {
        if src_dim == target_dim {
            let idx = target_y * src_dim + target_x;
            return if idx < heights.len() { heights[idx] } else { 0 };
        }

        let fx = (target_x as f32 / (target_dim - 1) as f32) * (src_dim - 1) as f32;
        let fy = (target_y as f32 / (target_dim - 1) as f32) * (src_dim - 1) as f32;
        let x0 = (fx.floor() as usize).min(src_dim - 1);
        let y0 = (fy.floor() as usize).min(src_dim - 1);
        let x1 = (x0 + 1).min(src_dim - 1);
        let y1 = (y0 + 1).min(src_dim - 1);

        let dx = fx - x0 as f32;
        let dy = fy - y0 as f32;

        let h00 = heights[y0 * src_dim + x0] as f32;
        let h10 = heights[y0 * src_dim + x1] as f32;
        let h01 = heights[y1 * src_dim + x0] as f32;
        let h11 = heights[y1 * src_dim + x1] as f32;

        let interp = (1.0 - dx) * (1.0 - dy) * h00 + dx * (1.0 - dy) * h10 + (1.0 - dx) * dy * h01 + dx * dy * h11;
        interp.round().clamp(0.0, 255.0) as u8
    }

    fn compare_textures(baseline: &MapVersionSnapshot, current: &MapVersionSnapshot) -> TextureDiffRecord {
        let dim = baseline.tile_dimension.max(current.tile_dimension);
        if dim == 0 || baseline.textures.is_none() || current.textures.is_none() {
            return TextureDiffRecord {
                dimension: 0,
                changed_tile_count: 0,
                changed_tiles: Vec::new(),
                transitions: BTreeMap::new(),
            };
        }

        let mut changed = Vec::new();
        let mut transitions = BTreeMap::new();

        let b_dim = baseline.tile_dimension;
        let c_dim = current.tile_dimension;
        let b_tex = baseline.textures.as_ref().unwrap();
        let c_tex = current.textures.as_ref().unwrap();

        for y in 0..dim {
            for x in 0..dim {
                let b_str = if x < b_dim && y < b_dim && y * b_dim + x < b_tex.len() {
                    &b_tex[y * b_dim + x]
                } else {
                    ""
                };
                let c_str = if x < c_dim && y < c_dim && y * c_dim + x < c_tex.len() {
                    &c_tex[y * c_dim + x]
                } else {
                    ""
                };

                if !b_str.eq_ignore_ascii_case(c_str) {
                    changed.push(TileTextureChange {
                        tile_x: x,
                        tile_y: y,
                        old_texture: b_str.to_string(),
                        new_texture: c_str.to_string(),
                    });
                    let key = (b_str.to_string(), c_str.to_string());
                    *transitions.entry(key).or_insert(0) += 1;
                }
            }
        }

        TextureDiffRecord {
            dimension: dim,
            changed_tile_count: changed.len(),
            changed_tiles: changed,
            transitions,
        }
    }

    fn compare_collision(baseline: &MapVersionSnapshot, current: &MapVersionSnapshot) -> CollisionDiffRecord {
        let dim = baseline.collision_dimension.max(current.collision_dimension);
        if dim == 0 || baseline.collision.is_none() || current.collision.is_none() {
            return CollisionDiffRecord {
                dimension: 0,
                newly_blocked_count: 0,
                newly_cleared_count: 0,
                changes: Vec::new(),
            };
        }

        let mut changes = Vec::new();
        let mut newly_blocked = 0;
        let mut newly_cleared = 0;

        let b_dim = baseline.collision_dimension;
        let c_dim = current.collision_dimension;
        let b_col = baseline.collision.as_ref().unwrap();
        let c_col = current.collision.as_ref().unwrap();

        for y in 0..dim {
            for x in 0..dim {
                let b_val = if x < b_dim && y < b_dim && y * b_dim + x < b_col.len() {
                    b_col[y * b_dim + x]
                } else {
                    0
                };
                let c_val = if x < c_dim && y < c_dim && y * c_dim + x < c_col.len() {
                    c_col[y * c_dim + x]
                } else {
                    0
                };

                if b_val != c_val {
                    changes.push(CollisionTileChange {
                        tile_x: x,
                        tile_y: y,
                        old_value: b_val,
                        new_value: c_val,
                    });
                    if b_val == 0 && c_val != 0 {
                        newly_blocked += 1;
                    } else if b_val != 0 && c_val == 0 {
                        newly_cleared += 1;
                    }
                }
            }
        }

        CollisionDiffRecord {
            dimension: dim,
            newly_blocked_count: newly_blocked,
            newly_cleared_count: newly_cleared,
            changes,
        }
    }

    fn compare_objects(
        baseline_list: Option<&[DiffObjectItem]>,
        current_list: Option<&[DiffObjectItem]>,
        options: &MapDiffOptions,
    ) -> ObjectDiffRecord {
        let baseline = baseline_list.unwrap_or(&[]);
        let current = current_list.unwrap_or(&[]);

        let mut baseline_by_id: HashMap<Uuid, &DiffObjectItem> = HashMap::new();
        let mut baseline_unmatched: Vec<&DiffObjectItem> = Vec::new();
        for b in baseline {
            if b.id != Uuid::nil() {
                baseline_by_id.insert(b.id, b);
            } else {
                baseline_unmatched.push(b);
            }
        }

        let mut current_by_id: HashMap<Uuid, &DiffObjectItem> = HashMap::new();
        let mut current_unmatched: Vec<&DiffObjectItem> = Vec::new();
        for c in current {
            if c.id != Uuid::nil() {
                current_by_id.insert(c.id, c);
            } else {
                current_unmatched.push(c);
            }
        }

        let mut added = Vec::new();
        let mut deleted = Vec::new();
        let mut modified = Vec::new();

        let mut matched_current_ids = HashSet::new();
        let mut matched_baseline_ids = HashSet::new();

        // 階段 1：持久 GUID 配對
        for (&id, &cur) in &current_by_id {
            if let Some(&base_item) = baseline_by_id.get(&id) {
                matched_current_ids.insert(id);
                matched_baseline_ids.insert(id);
                Self::check_object_modification(base_item, cur, options, &mut modified);
            }
        }

        for (&id, &item) in &current_by_id {
            if !matched_current_ids.contains(&id) {
                current_unmatched.push(item);
            }
        }
        for (&id, &item) in &baseline_by_id {
            if !matched_baseline_ids.contains(&id) {
                baseline_unmatched.push(item);
            }
        }

        // 階段 2：Slot / UID 配對
        let mut matched_slot_base = HashSet::new();
        let mut matched_slot_cur = HashSet::new();

        for (i, cur) in current_unmatched.iter().enumerate() {
            if cur.slot < 0 || cur.uid == 0 {
                continue;
            }
            for (j, base_item) in baseline_unmatched.iter().enumerate() {
                if matched_slot_base.contains(&j) {
                    continue;
                }
                if base_item.slot == cur.slot && base_item.uid == cur.uid {
                    matched_slot_cur.insert(i);
                    matched_slot_base.insert(j);
                    Self::check_object_modification(base_item, cur, options, &mut modified);
                    break;
                }
            }
        }

        let cur_remaining: Vec<&DiffObjectItem> = current_unmatched
            .iter()
            .enumerate()
            .filter(|(idx, _)| !matched_slot_cur.contains(idx))
            .map(|(_, item)| *item)
            .collect();
        let base_remaining: Vec<&DiffObjectItem> = baseline_unmatched
            .iter()
            .enumerate()
            .filter(|(idx, _)| !matched_slot_base.contains(idx))
            .map(|(_, item)| *item)
            .collect();

        // 階段 3：啟發式空間鄰近度與類型配對
        let mut used_base_remaining = HashSet::new();
        let mut used_cur_remaining = HashSet::new();

        for (i, cur) in cur_remaining.iter().enumerate() {
            let mut best_base_idx: Option<usize> = None;
            let mut best_dist = options.spatial_match_radius;

            for (j, base_item) in base_remaining.iter().enumerate() {
                if used_base_remaining.contains(&j) {
                    continue;
                }
                let type_match = (cur.type_id > 0 && cur.type_id == base_item.type_id)
                    || (!cur.alias.is_empty() && cur.alias.eq_ignore_ascii_case(&base_item.alias));

                if type_match {
                    let dist = cur.distance_to(base_item);
                    if dist < best_dist {
                        best_dist = dist;
                        best_base_idx = Some(j);
                    }
                }
            }

            if let Some(j) = best_base_idx {
                used_cur_remaining.insert(i);
                used_base_remaining.insert(j);
                Self::check_object_modification(base_remaining[j], cur, options, &mut modified);
            }
        }

        // 階段 4：剩餘項目歸類為 Added 或 Deleted
        for (i, cur) in cur_remaining.iter().enumerate() {
            if !used_cur_remaining.contains(&i) {
                added.push((*cur).clone());
            }
        }

        for (j, base_item) in base_remaining.iter().enumerate() {
            if !used_base_remaining.contains(&j) {
                deleted.push((*base_item).clone());
            }
        }

        ObjectDiffRecord {
            added,
            deleted,
            modified,
        }
    }

    fn check_object_modification(
        base_item: &DiffObjectItem,
        cur: &DiffObjectItem,
        options: &MapDiffOptions,
        modified_list: &mut Vec<ObjectModification>,
    ) {
        let mut changes = Vec::new();
        let dist = cur.distance_to(base_item);
        let rot_delta = (cur.rotation - base_item.rotation).abs();

        if dist > options.position_tolerance {
            changes.push(format!(
                "坐標移動 ({:.0},{:.0}) -> ({:.0},{:.0}) 距離 {:.1}",
                base_item.x, base_item.z, cur.x, cur.z, dist
            ));
        }

        if rot_delta > options.rotation_tolerance {
            changes.push(format!("旋轉角度 {:.0}° -> {:.0}°", base_item.rotation, cur.rotation));
        }

        if base_item.team != cur.team {
            changes.push(format!("隊伍 {} -> {}", base_item.team, cur.team));
        }

        if base_item.unit_count != cur.unit_count {
            changes.push(format!("部隊人數 {} -> {}", base_item.unit_count, cur.unit_count));
        }

        if base_item.prebuilt != cur.prebuilt {
            changes.push(format!("預建狀態 {} -> {}", base_item.prebuilt, cur.prebuilt));
        }

        if !base_item.alias.eq_ignore_ascii_case(&cur.alias) && !base_item.alias.is_empty() && !cur.alias.is_empty() {
            changes.push(format!("別名變更 {} -> {}", base_item.alias, cur.alias));
        }

        if !changes.is_empty() {
            modified_list.push(ObjectModification {
                baseline: base_item.clone(),
                current: cur.clone(),
                distance_moved: dist,
                rotation_delta: rot_delta,
                changed_attributes: changes,
            });
        }
    }

    fn compare_events(
        baseline_events: Option<&[Event]>,
        current_events: Option<&[Event]>,
    ) -> EventDiffRecord {
        let baseline = baseline_events.unwrap_or(&[]);
        let current = current_events.unwrap_or(&[]);

        let mut baseline_dict: HashMap<String, &Event> = HashMap::new();
        for b in baseline {
            baseline_dict.insert(b.name.to_ascii_lowercase(), b);
        }

        let mut current_dict: HashMap<String, &Event> = HashMap::new();
        for c in current {
            current_dict.insert(c.name.to_ascii_lowercase(), c);
        }

        let mut added = Vec::new();
        let mut deleted = Vec::new();
        let mut modified = Vec::new();

        for cur in current {
            let key = cur.name.to_ascii_lowercase();
            if let Some(&base_evt) = baseline_dict.get(&key) {
                let mut detail_changes = Vec::new();

                if cur.delay_seconds != base_evt.delay_seconds {
                    detail_changes.push(format!("延遲時間 {}s -> {}s", base_evt.delay_seconds, cur.delay_seconds));
                }
                if cur.repeat != base_evt.repeat {
                    detail_changes.push(format!("循環重複 {} -> {}", base_evt.repeat, cur.repeat));
                }
                if cur.enabled != base_evt.enabled {
                    detail_changes.push(format!("啟用狀態 {} -> {}", base_evt.enabled, cur.enabled));
                }

                if cur.conditions.len() != base_evt.conditions.len() {
                    detail_changes.push(format!(
                        "觸發條件異動 ({} 項 -> {} 項)",
                        base_evt.conditions.len(),
                        cur.conditions.len()
                    ));
                }

                if cur.actions.len() != base_evt.actions.len() {
                    detail_changes.push(format!(
                        "執行動作異動 ({} 項 -> {} 項)",
                        base_evt.actions.len(),
                        cur.actions.len()
                    ));
                }

                if !detail_changes.is_empty() {
                    modified.push(EventModification {
                        event_name: cur.name.clone(),
                        baseline: base_evt.clone(),
                        current: cur.clone(),
                        detail_changes,
                    });
                }
            } else {
                added.push(cur.clone());
            }
        }

        for b in baseline {
            let key = b.name.to_ascii_lowercase();
            if !current_dict.contains_key(&key) {
                deleted.push(b.clone());
            }
        }

        EventDiffRecord {
            added,
            deleted,
            modified,
        }
    }
}
