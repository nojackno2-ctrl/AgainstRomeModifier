//! Resource cluster planner: forest patches, stone quarries, wildlife herds.

use super::models::{NatureClusterItem, PlacedResourceItem, SettlementTribe};
use super::presets::SettlementTribalPresets;
use crate::random::DotNetRandom;

pub struct ResourceClusterPlanner<'a> {
    dimension: i32,
    tile_world_size: f32,
    sample_height: &'a (dyn Fn(f32, f32) -> f32 + 'a),
    is_blocked: &'a (dyn Fn(i32, i32) -> bool + 'a),
    water_level: f32,
}

impl<'a> ResourceClusterPlanner<'a> {
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

    pub fn plan_clusters(
        &self,
        center_tile_x: f32,
        center_tile_z: f32,
        tribe: SettlementTribe,
        seed: i32,
        base_angle_offset_deg: f32,
    ) -> (
        Vec<NatureClusterItem>,
        Vec<NatureClusterItem>,
        Vec<PlacedResourceItem>,
    ) {
        let mut random = DotNetRandom::new(seed);

        // 1. 規劃木材資源（森林）
        let forest_angle_deg = (base_angle_offset_deg + 30.0 + random.next_double() as f32 * 30.0) % 360.0;
        let forest = self.plan_forest_cluster(center_tile_x, center_tile_z, forest_angle_deg, tribe, &mut random, 45);

        // 2. 規劃採石場與石礦
        let stone_preferred_angle_deg = (forest_angle_deg + 120.0 + random.next_double() as f32 * 40.0) % 360.0;
        let stone = self.plan_stone_quarry_cluster(center_tile_x, center_tile_z, stone_preferred_angle_deg, tribe, &mut random, 7);

        // 3. 規劃糧食/野生動物狩獵區
        let food_preferred_angle_deg = (forest_angle_deg + 220.0 + random.next_double() as f32 * 50.0) % 360.0;
        let wildlife = self.plan_wildlife_herd(center_tile_x, center_tile_z, food_preferred_angle_deg, &mut random, 4);

        (forest, stone, wildlife)
    }

    pub fn plan_forest_cluster(
        &self,
        center_tile_x: f32,
        center_tile_z: f32,
        direction_angle_deg: f32,
        tribe: SettlementTribe,
        random: &mut DotNetRandom,
        target_tree_count: usize,
    ) -> Vec<NatureClusterItem> {
        let palette = SettlementTribalPresets::get_forest_tree_palette(tribe);
        let mut trees = Vec::new();

        let rad = direction_angle_deg * std::f32::consts::PI / 180.0;
        let forest_center_tile_x = center_tile_x + rad.cos() * 21.0;
        let forest_center_tile_z = center_tile_z + rad.sin() * 21.0;

        const MIN_SPACING: f32 = 0.85;
        let mut occupied_points: Vec<(f32, f32)> = Vec::new();

        let noise_scale = 0.12f32;
        let noise_offset_x = (random.next_double() * 1000.0) as f32;
        let noise_offset_z = (random.next_double() * 1000.0) as f32;

        let scan_r = 12;
        let min_x = (forest_center_tile_x - scan_r as f32).floor() as i32;
        let max_x = (forest_center_tile_x + scan_r as f32).ceil() as i32;
        let min_z = (forest_center_tile_z - scan_r as f32).floor() as i32;
        let max_z = (forest_center_tile_z + scan_r as f32).ceil() as i32;

        let min_x = min_x.max(2);
        let max_x = max_x.min(self.dimension - 3);
        let min_z = min_z.max(2);
        let max_z = max_z.min(self.dimension - 3);

        let mut candidates = Vec::new();

        for z in min_z..=max_z {
            for x in min_x..=max_x {
                let dx = x as f32 - forest_center_tile_x;
                let dz = z as f32 - forest_center_tile_z;
                let dist_to_cluster_center = (dx * dx + dz * dz).sqrt();
                if dist_to_cluster_center > scan_r as f32 {
                    continue;
                }

                let dist_to_settlement = ((x as f32 - center_tile_x).powi(2) + (z as f32 - center_tile_z).powi(2)).sqrt();
                if dist_to_settlement < 13.5 || dist_to_settlement > 32.0 {
                    continue;
                }

                if (self.is_blocked)(x, z) {
                    continue;
                }

                let h = (self.sample_height)(x as f32 * self.tile_world_size, z as f32 * self.tile_world_size);
                if h <= self.water_level + 0.8 {
                    continue;
                }

                let n = sample_perlin_noise(x as f32 * noise_scale + noise_offset_x, z as f32 * noise_scale + noise_offset_z);
                let radial_falloff = 1.0 - (dist_to_cluster_center / scan_r as f32);
                let density = n * 0.6 + radial_falloff * 0.4;

                if density > 0.35 {
                    candidates.push((x as f32, z as f32, density));
                }
            }
        }

        // 穩定排序 density 降序
        candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        for cand in candidates {
            if trees.len() >= target_tree_count {
                break;
            }

            let jitter_x = cand.0 + 0.15 + random.next_double() as f32 * 0.7;
            let jitter_z = cand.1 + 0.15 + random.next_double() as f32 * 0.7;

            let mut too_close = false;
            for pt in &occupied_points {
                let dist_sq = (jitter_x - pt.0).powi(2) + (jitter_z - pt.1).powi(2);
                if dist_sq < MIN_SPACING * MIN_SPACING {
                    too_close = true;
                    break;
                }
            }
            if too_close {
                continue;
            }

            let type_name = if cand.2 < 0.45 && palette.len() > 4 {
                palette[palette.len() - 1]
            } else {
                let max_idx = 3.min(palette.len());
                let main_idx = random.next_i32(max_idx as i32) as usize;
                palette[main_idx]
            };

            let rot_deg = random.next_double() as f32 * 360.0;
            occupied_points.push((jitter_x, jitter_z));
            trees.push(NatureClusterItem {
                type_name: type_name.to_string(),
                world_x: jitter_x * self.tile_world_size,
                world_z: jitter_z * self.tile_world_size,
                height_offset: 0.0,
                angle_deg: rot_deg,
            });
        }

        trees
    }

    pub fn plan_stone_quarry_cluster(
        &self,
        center_tile_x: f32,
        center_tile_z: f32,
        preferred_angle_deg: f32,
        tribe: SettlementTribe,
        random: &mut DotNetRandom,
        target_rock_count: usize,
    ) -> Vec<NatureClusterItem> {
        let palette = SettlementTribalPresets::get_stone_quarry_palette(tribe);
        let mut rocks = Vec::new();

        let (anchor_tile_x, anchor_tile_z) = if let Some(anchor) = self.try_find_cliff_foot_anchor(center_tile_x, center_tile_z, preferred_angle_deg) {
            anchor
        } else {
            let rad = preferred_angle_deg * std::f32::consts::PI / 180.0;
            (
                (center_tile_x + rad.cos() * 22.0).clamp(10.0, self.dimension as f32 - 11.0),
                (center_tile_z + rad.sin() * 22.0).clamp(10.0, self.dimension as f32 - 11.0),
            )
        };

        let mut occupied: Vec<(f32, f32)> = Vec::new();
        const ROCK_SPACING: f32 = 1.3;

        for _ in 0..40 {
            if rocks.len() >= target_rock_count {
                break;
            }

            let r = 0.5 + random.next_double() as f32 * 4.0;
            let angle = random.next_double() as f32 * std::f32::consts::PI * 2.0;
            let cand_x = anchor_tile_x + angle.cos() * r;
            let cand_z = anchor_tile_z + angle.sin() * r;

            if cand_x < 3.0 || cand_x > self.dimension as f32 - 4.0 || cand_z < 3.0 || cand_z > self.dimension as f32 - 4.0 {
                continue;
            }
            if (self.is_blocked)(cand_x as i32, cand_z as i32) {
                continue;
            }

            let h = (self.sample_height)(cand_x * self.tile_world_size, cand_z * self.tile_world_size);
            if h <= self.water_level + 0.5 {
                continue;
            }

            let too_close = occupied.iter().any(|pt| (cand_x - pt.0).powi(2) + (cand_z - pt.1).powi(2) < ROCK_SPACING * ROCK_SPACING);
            if too_close {
                continue;
            }

            occupied.push((cand_x, cand_z));
            let rock_type = palette[random.next_i32(palette.len() as i32) as usize];
            let rot_deg = random.next_double() as f32 * 360.0;

            rocks.push(NatureClusterItem {
                type_name: rock_type.to_string(),
                world_x: cand_x * self.tile_world_size,
                world_z: cand_z * self.tile_world_size,
                height_offset: 0.0,
                angle_deg: rot_deg,
            });
        }

        rocks
    }

    pub fn plan_wildlife_herd(
        &self,
        center_tile_x: f32,
        center_tile_z: f32,
        preferred_angle_deg: f32,
        random: &mut DotNetRandom,
        herd_size: usize,
    ) -> Vec<PlacedResourceItem> {
        let wildlife_palette = SettlementTribalPresets::get_wildlife_palette();
        let mut herd = Vec::new();

        let rad = preferred_angle_deg * std::f32::consts::PI / 180.0;
        let mut pasture_center_x = center_tile_x + rad.cos() * 18.0;
        let mut pasture_center_z = center_tile_z + rad.sin() * 18.0;

        pasture_center_x = pasture_center_x.clamp(10.0, self.dimension as f32 - 11.0);
        pasture_center_z = pasture_center_z.clamp(10.0, self.dimension as f32 - 11.0);

        let animal_type = wildlife_palette[random.next_i32(wildlife_palette.len() as i32) as usize];
        let mut occupied: Vec<(f32, f32)> = Vec::new();
        const ANIMAL_SPACING: f32 = 2.0;

        for _ in 0..30 {
            if herd.len() >= herd_size {
                break;
            }

            let r = 0.8 + random.next_double() as f32 * 5.0;
            let angle = random.next_double() as f32 * std::f32::consts::PI * 2.0;
            let cand_x = pasture_center_x + angle.cos() * r;
            let cand_z = pasture_center_z + angle.sin() * r;

            if cand_x < 3.0 || cand_x > self.dimension as f32 - 4.0 || cand_z < 3.0 || cand_z > self.dimension as f32 - 4.0 {
                continue;
            }
            if (self.is_blocked)(cand_x as i32, cand_z as i32) {
                continue;
            }

            let h = (self.sample_height)(cand_x * self.tile_world_size, cand_z * self.tile_world_size);
            if h <= self.water_level + 1.0 {
                continue;
            }

            let too_close = occupied.iter().any(|pt| (cand_x - pt.0).powi(2) + (cand_z - pt.1).powi(2) < ANIMAL_SPACING * ANIMAL_SPACING);
            if too_close {
                continue;
            }

            occupied.push((cand_x, cand_z));
            let rot_deg = random.next_double() as f32 * 360.0;

            herd.push(PlacedResourceItem {
                type_name: animal_type.to_string(),
                world_x: cand_x * self.tile_world_size,
                world_z: cand_z * self.tile_world_size,
                height_offset: 0.0,
                angle_deg: rot_deg,
                team: -1,
                count: 0,
            });
        }

        herd
    }

    fn try_find_cliff_foot_anchor(
        &self,
        center_tile_x: f32,
        center_tile_z: f32,
        preferred_angle_deg: f32,
    ) -> Option<(f32, f32)> {
        let mut best_anchor = None;
        let mut best_cliff_score = -1.0f32;

        let mut dist = 16.0f32;
        while dist <= 30.0 {
            let mut d_angle = -45.0f32;
            while d_angle <= 45.0 {
                let angle = (preferred_angle_deg + d_angle + 360.0) % 360.0;
                let rad = angle * std::f32::consts::PI / 180.0;

                let x = center_tile_x + rad.cos() * dist;
                let z = center_tile_z + rad.sin() * dist;

                if x >= 5.0 && x <= self.dimension as f32 - 6.0 && z >= 5.0 && z <= self.dimension as f32 - 6.0 {
                    if !(self.is_blocked)(x as i32, z as i32) {
                        let center_h = (self.sample_height)(x * self.tile_world_size, z * self.tile_world_size);
                        if center_h > self.water_level + 1.0 {
                            let mut max_neighbor_h = center_h;
                            let mut dz = -2;
                            while dz <= 2 {
                                let mut dx = -2;
                                while dx <= 2 {
                                    let nh = (self.sample_height)((x + dx as f32) * self.tile_world_size, (z + dz as f32) * self.tile_world_size);
                                    max_neighbor_h = max_neighbor_h.max(nh);
                                    dx += 2;
                                }
                                dz += 2;
                            }

                            let cliff_height = max_neighbor_h - center_h;
                            if cliff_height > 3.0 && cliff_height > best_cliff_score {
                                best_cliff_score = cliff_height;
                                best_anchor = Some((x, z));
                            }
                        }
                    }
                }
                d_angle += 15.0;
            }
            dist += 2.5;
        }

        best_anchor
    }
}

fn sample_perlin_noise(x: f32, y: f32) -> f32 {
    let x0 = x.floor() as i32;
    let x1 = x0 + 1;
    let y0 = y.floor() as i32;
    let y1 = y0 + 1;

    let sx = x - x0 as f32;
    let sy = y - y0 as f32;

    let u = sx * sx * (3.0 - 2.0 * sx);
    let v = sy * sy * (3.0 - 2.0 * sy);

    let n0 = dot_grid_gradient(x0, y0, x, y);
    let n1 = dot_grid_gradient(x1, y0, x, y);
    let ix0 = n0 + u * (n1 - n0);

    let n0 = dot_grid_gradient(x0, y1, x, y);
    let n1 = dot_grid_gradient(x1, y1, x, y);
    let ix1 = n0 + u * (n1 - n0);

    let val = ix0 + v * (ix1 - ix0);
    ((val + 1.0) * 0.5).clamp(0.0, 1.0)
}

fn dot_grid_gradient(ix: i32, iy: i32, x: f32, y: f32) -> f32 {
    let mut hash = ((ix as u32).wrapping_mul(374761393) ^ (iy as u32).wrapping_mul(668265263)) ^ 0x5bf03635;
    hash = (hash ^ (hash >> 13)).wrapping_mul(1274126177);
    let angle = (hash & 0xFFFF) as f32 / 65536.0 * std::f32::consts::PI * 2.0;

    let dx = x - ix as f32;
    let dy = y - iy as f32;
    dx * angle.cos() + dy * angle.sin()
}
