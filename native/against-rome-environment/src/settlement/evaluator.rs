//! Settlement site evaluation: flatness, water safety, clear packing of buildings.

use super::models::{PlacedBuildingPlan, SettlementSiteEvaluationResult, SettlementTribe};
use super::presets::SettlementTribalPresets;

pub struct SettlementSiteEvaluator<'a> {
    dimension: i32,
    tile_world_size: f32,
    sample_height: &'a (dyn Fn(f32, f32) -> f32 + 'a),
    is_blocked: &'a (dyn Fn(i32, i32) -> bool + 'a),
    water_level: f32,
}

impl<'a> SettlementSiteEvaluator<'a> {
    pub fn new(
        dimension: i32,
        sample_height: &'a (dyn Fn(f32, f32) -> f32 + 'a),
        is_blocked: &'a (dyn Fn(i32, i32) -> bool + 'a),
        water_level: f32,
        tile_world_size: f32,
    ) -> Self {
        Self {
            dimension: dimension.max(1),
            tile_world_size: if tile_world_size > 0.0 { tile_world_size } else { 64.0 },
            sample_height,
            is_blocked,
            water_level,
        }
    }

    pub fn evaluate(
        &self,
        center_tile_x: f32,
        center_tile_z: f32,
        tribe: SettlementTribe,
        team: i32,
        orientation_angle_deg: f32,
    ) -> SettlementSiteEvaluationResult {
        let world_center_x = center_tile_x * self.tile_world_size;
        let world_center_z = center_tile_z * self.tile_world_size;

        // 1. 邊界檢查（聚落周邊需至少預留 18 格腹地）
        const MARGIN_TILES: f32 = 18.0;
        let dim_f = self.dimension as f32;
        if center_tile_x < MARGIN_TILES
            || center_tile_x > dim_f - MARGIN_TILES
            || center_tile_z < MARGIN_TILES
            || center_tile_z > dim_f - MARGIN_TILES
        {
            return SettlementSiteEvaluationResult {
                is_valid: false,
                total_score: 0.0,
                flatness_score: 0.0,
                water_safety_score: 0.0,
                space_clearance_score: 0.0,
                expansion_potential_score: 0.0,
                anchor_tile_x: center_tile_x,
                anchor_tile_z: center_tile_z,
                world_x: world_center_x,
                world_z: world_center_z,
                planned_buildings: Vec::new(),
                rejection_reason: Some("Too close to map edge".to_string()),
            };
        }

        // 2. 水體安全評估
        let (water_safety_score, min_water_dist) =
            self.evaluate_water_safety(center_tile_x, center_tile_z);
        if min_water_dist < 6.0 {
            return SettlementSiteEvaluationResult {
                is_valid: false,
                total_score: 0.0,
                flatness_score: 0.0,
                water_safety_score,
                space_clearance_score: 0.0,
                expansion_potential_score: 0.0,
                anchor_tile_x: center_tile_x,
                anchor_tile_z: center_tile_z,
                world_x: world_center_x,
                world_z: world_center_z,
                planned_buildings: Vec::new(),
                rejection_reason: Some(format!(
                    "Too close to water (distance {:.1} tiles < 6.0)",
                    min_water_dist
                )),
            };
        }

        // 3. 取得部族建築藍圖
        let (main_blueprint, core_blueprints) = SettlementTribalPresets::get_blueprints(tribe);

        // 4. 主屋足跡檢驗
        let main_radius = main_blueprint.footprint_width.max(main_blueprint.footprint_height) * 0.5;
        let (main_buildable, main_height_delta, _avg_main_height) =
            self.is_footprint_buildable(center_tile_x, center_tile_z, main_radius);

        if !main_buildable {
            return SettlementSiteEvaluationResult {
                is_valid: false,
                total_score: 0.0,
                flatness_score: 0.0,
                water_safety_score,
                space_clearance_score: 0.0,
                expansion_potential_score: 0.0,
                anchor_tile_x: center_tile_x,
                anchor_tile_z: center_tile_z,
                world_x: world_center_x,
                world_z: world_center_z,
                planned_buildings: Vec::new(),
                rejection_reason: Some(format!(
                    "Main house footprint obstructed or terrain too steep (delta {:.2})",
                    main_height_delta
                )),
            };
        }

        let mut planned_buildings = vec![PlacedBuildingPlan {
            type_name: main_blueprint.type_name,
            world_x: world_center_x,
            world_z: world_center_z,
            height_offset: 0.0,
            angle: orientation_angle_deg,
            team,
            footprint_radius: main_radius,
        }];

        // 5. 核心 6 棟建築佈局規劃
        let mut total_building_height_delta = main_height_delta;
        let mut all_buildings_placed = true;

        for bp in &core_blueprints {
            let bp_radius = bp.footprint_width.max(bp.footprint_height) * 0.5;
            let target_angle = (bp.preferred_angle_deg + orientation_angle_deg) % 360.0;
            let target_dist = bp.preferred_distance_to_main;

            if let Some((spot_x, spot_z, spot_angle, spot_delta)) = self.try_find_clear_building_spot(
                center_tile_x,
                center_tile_z,
                target_dist,
                target_angle,
                bp_radius,
                &planned_buildings,
            ) {
                planned_buildings.push(PlacedBuildingPlan {
                    type_name: bp.type_name.clone(),
                    world_x: spot_x * self.tile_world_size,
                    world_z: spot_z * self.tile_world_size,
                    height_offset: 0.0,
                    angle: spot_angle,
                    team,
                    footprint_radius: bp_radius,
                });
                total_building_height_delta += spot_delta;
            } else {
                all_buildings_placed = false;
                break;
            }
        }

        if !all_buildings_placed || planned_buildings.len() < 7 {
            return SettlementSiteEvaluationResult {
                is_valid: false,
                total_score: 0.0,
                flatness_score: 0.0,
                water_safety_score,
                space_clearance_score: 0.0,
                expansion_potential_score: 0.0,
                anchor_tile_x: center_tile_x,
                anchor_tile_z: center_tile_z,
                world_x: world_center_x,
                world_z: world_center_z,
                planned_buildings,
                rejection_reason: Some(
                    "Insufficient clear space to pack 5 core settlement buildings".to_string(),
                ),
            };
        }

        // 6. 計算平坦度得分
        let avg_height_delta = total_building_height_delta / planned_buildings.len() as f32;
        let flatness_score = (100.0 - avg_height_delta * 15.0).clamp(0.0, 100.0);

        // 7. 計算腹地擴充潛力
        let expansion_score = self.evaluate_expansion_potential(center_tile_x, center_tile_z, 16.0);

        // 8. 空間排布緊湊與通路充裕度得分
        let space_clearance_score = 95.0 - (planned_buildings.len() as f32 - 6.0) * 5.0;

        // 9. 總分加權合成
        let total_score = flatness_score * 0.35
            + water_safety_score * 0.25
            + space_clearance_score * 0.20
            + expansion_score * 0.20;

        SettlementSiteEvaluationResult {
            is_valid: true,
            total_score,
            flatness_score,
            water_safety_score,
            space_clearance_score,
            expansion_potential_score: expansion_score,
            anchor_tile_x: center_tile_x,
            anchor_tile_z: center_tile_z,
            world_x: world_center_x,
            world_z: world_center_z,
            planned_buildings,
            rejection_reason: None,
        }
    }

    fn try_find_clear_building_spot(
        &self,
        center_tile_x: f32,
        center_tile_z: f32,
        preferred_dist: f32,
        preferred_angle_deg: f32,
        footprint_radius: f32,
        existing_buildings: &[PlacedBuildingPlan],
    ) -> Option<(f32, f32, f32, f32)> {
        let angle_offsets = [
            0.0f32, 15.0, -15.0, 30.0, -30.0, 45.0, -45.0, 60.0, -60.0, 75.0, -75.0, 90.0, -90.0,
        ];
        let dist_offsets = [0.0f32, 0.8, -0.8, 1.5, -1.2];
        const MIN_CORRIDOR_CLEARANCE: f32 = 1.2;

        for &d_offset in &dist_offsets {
            let dist = preferred_dist + d_offset;
            if dist < 3.0 {
                continue;
            }

            for &a_offset in &angle_offsets {
                let angle_deg = (preferred_angle_deg + a_offset + 360.0) % 360.0;
                let rad = angle_deg * std::f32::consts::PI / 180.0;

                let cand_x = center_tile_x + rad.cos() * dist;
                let cand_z = center_tile_z + rad.sin() * dist;

                if cand_x < 4.0
                    || cand_x > self.dimension as f32 - 5.0
                    || cand_z < 4.0
                    || cand_z > self.dimension as f32 - 5.0
                {
                    continue;
                }

                let (buildable, height_delta, _) =
                    self.is_footprint_buildable(cand_x, cand_z, footprint_radius);
                if !buildable {
                    continue;
                }

                let mut collides = false;
                for placed in existing_buildings {
                    let placed_tile_x = placed.world_x / self.tile_world_size;
                    let placed_tile_z = placed.world_z / self.tile_world_size;
                    let min_dist = placed.footprint_radius + footprint_radius + MIN_CORRIDOR_CLEARANCE;
                    let dx = cand_x - placed_tile_x;
                    let dz = cand_z - placed_tile_z;
                    if dx * dx + dz * dz < min_dist * min_dist {
                        collides = true;
                        break;
                    }
                }

                if !collides {
                    return Some((cand_x, cand_z, angle_deg, height_delta));
                }
            }
        }

        None
    }

    fn is_footprint_buildable(&self, tile_x: f32, tile_z: f32, radius: f32) -> (bool, f32, f32) {
        let min_x = (tile_x - radius).floor() as i32;
        let max_x = (tile_x + radius).ceil() as i32;
        let min_z = (tile_z - radius).floor() as i32;
        let max_z = (tile_z + radius).ceil() as i32;

        let mut min_h = f32::MAX;
        let mut max_h = f32::MIN;
        let mut sum_h = 0.0f32;
        let mut count = 0;

        let radius_sq = radius * radius;

        for z in min_z..=max_z {
            for x in min_x..=max_x {
                if x < 0 || x >= self.dimension || z < 0 || z >= self.dimension {
                    return (false, f32::MAX, 0.0);
                }

                let dx = x as f32 - tile_x;
                let dz = z as f32 - tile_z;
                if dx * dx + dz * dz > radius_sq {
                    continue;
                }

                if (self.is_blocked)(x, z) {
                    return (false, f32::MAX, 0.0);
                }

                let h = (self.sample_height)(
                    x as f32 * self.tile_world_size,
                    z as f32 * self.tile_world_size,
                );
                if h < self.water_level + 0.8 {
                    return (false, f32::MAX, 0.0);
                }

                min_h = min_h.min(h);
                max_h = max_h.max(h);
                sum_h += h;
                count += 1;
            }
        }

        if count == 0 {
            return (false, f32::MAX, 0.0);
        }

        let height_delta = max_h - min_h;
        let avg_h = sum_h / count as f32;
        let buildable = height_delta <= 5.0;
        (buildable, height_delta, avg_h)
    }

    fn evaluate_water_safety(&self, center_tile_x: f32, center_tile_z: f32) -> (f32, f32) {
        let mut min_water_dist = f32::MAX;
        const SCAN_RADIUS: i32 = 18;

        let mut dz = -SCAN_RADIUS;
        while dz <= SCAN_RADIUS {
            let mut dx = -SCAN_RADIUS;
            while dx <= SCAN_RADIUS {
                let x = (center_tile_x + dx as f32).round() as i32;
                let z = (center_tile_z + dz as f32).round() as i32;

                if x >= 0 && x < self.dimension && z >= 0 && z < self.dimension {
                    let h = (self.sample_height)(
                        x as f32 * self.tile_world_size,
                        z as f32 * self.tile_world_size,
                    );
                    if h <= self.water_level + 0.5 {
                        let dist = ((dx * dx + dz * dz) as f32).sqrt();
                        min_water_dist = min_water_dist.min(dist);
                    }
                }
                dx += 2;
            }
            dz += 2;
        }

        if min_water_dist < 6.0 {
            (0.0, min_water_dist)
        } else if min_water_dist >= 18.0 {
            (90.0, min_water_dist)
        } else {
            let score = 50.0 + (min_water_dist - 6.0) / 12.0 * 45.0;
            (score, min_water_dist)
        }
    }

    fn evaluate_expansion_potential(&self, center_tile_x: f32, center_tile_z: f32, radius: f32) -> f32 {
        let mut valid_count = 0;
        let mut total_sampled = 0;
        let r = radius.ceil() as i32;
        let radius_sq = radius * radius;

        let mut dz = -r;
        while dz <= r {
            let mut dx = -r;
            while dx <= r {
                let x = (center_tile_x + dx as f32).round() as i32;
                let z = (center_tile_z + dz as f32).round() as i32;

                if x >= 0 && x < self.dimension && z >= 0 && z < self.dimension {
                    if (dx * dx + dz * dz) as f32 <= radius_sq {
                        total_sampled += 1;
                        if !(self.is_blocked)(x, z) {
                            let h = (self.sample_height)(
                                x as f32 * self.tile_world_size,
                                z as f32 * self.tile_world_size,
                            );
                            if h > self.water_level + 1.0 {
                                valid_count += 1;
                            }
                        }
                    }
                }
                dx += 2;
            }
            dz += 2;
        }

        if total_sampled > 0 {
            (valid_count as f32 / total_sampled as f32) * 100.0
        } else {
            0.0
        }
    }
}
