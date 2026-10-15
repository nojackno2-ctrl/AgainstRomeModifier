//! Multiplayer fairness balancer: 2-8 player central, rotational, and topological equidistant layouts.

use super::cluster_planner::ResourceClusterPlanner;
use super::evaluator::SettlementSiteEvaluator;
use super::models::{
    FairnessScoreReport, MultiplayerDistributionResult, PlayerSettlementLayout,
    SettlementSiteEvaluationResult, SettlementTribe, SymmetryMode,
};
use crate::random::DotNetRandom;

pub struct MultiplayerFairnessBalancer<'a> {
    evaluator: &'a SettlementSiteEvaluator<'a>,
    planner: &'a ResourceClusterPlanner<'a>,
    dimension: i32,
    tile_world_size: f32,
}

impl<'a> MultiplayerFairnessBalancer<'a> {
    pub fn new(
        evaluator: &'a SettlementSiteEvaluator<'a>,
        planner: &'a ResourceClusterPlanner<'a>,
        dimension: i32,
        tile_world_size: f32,
    ) -> Self {
        Self {
            evaluator,
            planner,
            dimension: dimension.max(1),
            tile_world_size: if tile_world_size > 0.0 { tile_world_size } else { 64.0 },
        }
    }

    pub fn dimension(&self) -> i32 {
        self.dimension
    }

    pub fn tile_world_size(&self) -> f32 {
        self.tile_world_size
    }

    pub fn generate(
        &self,
        player_count: usize,
        mut mode: SymmetryMode,
        tribes: Option<&[SettlementTribe]>,
        base_seed: i32,
        spawn_radius_ratio: f32,
    ) -> Result<MultiplayerDistributionResult, String> {
        if !(2..=8).contains(&player_count) {
            return Err("Player count must be between 2 and 8.".to_string());
        }

        let default_tribes = vec![SettlementTribe::Germanic; player_count];
        let tribe_list = tribes.unwrap_or(&default_tribes);
        if tribe_list.len() < player_count {
            return Err("Tribes list count must match or exceed player count.".to_string());
        }

        let center_tile = self.dimension as f32 * 0.5;
        let spawn_radius_tiles = center_tile * spawn_radius_ratio.clamp(0.4, 0.75);

        let mut player_layouts = Vec::new();

        match mode {
            SymmetryMode::CentralSymmetry if player_count == 2 => {
                player_layouts.extend(self.generate_central_symmetry_2p(
                    center_tile,
                    spawn_radius_tiles,
                    tribe_list,
                    base_seed,
                ));
            }
            _ => {
                if mode == SymmetryMode::CentralSymmetry && player_count > 2 {
                    mode = SymmetryMode::RotationalSymmetry;
                }
                if mode == SymmetryMode::RotationalSymmetry {
                    player_layouts.extend(self.generate_rotational_symmetry(
                        player_count,
                        center_tile,
                        spawn_radius_tiles,
                        tribe_list,
                        base_seed,
                    ));
                } else {
                    player_layouts.extend(self.generate_topological_equidistant(
                        player_count,
                        center_tile,
                        spawn_radius_tiles,
                        tribe_list,
                        base_seed,
                    ));
                }
            }
        }

        let report = Self::compute_fairness_report(&player_layouts);

        Ok(MultiplayerDistributionResult {
            mode,
            player_count,
            players: player_layouts,
            fairness_report: report,
        })
    }

    fn generate_central_symmetry_2p(
        &self,
        center_tile: f32,
        spawn_radius_tiles: f32,
        tribes: &[SettlementTribe],
        seed: i32,
    ) -> Vec<PlayerSettlementLayout> {
        let mut random = DotNetRandom::new(seed);
        let angle_rad = (random.next_double() * std::f64::consts::PI) as f32;
        let dx = angle_rad.cos() * spawn_radius_tiles;
        let dz = angle_rad.sin() * spawn_radius_tiles;

        // Player 1
        let p1_x = center_tile + dx;
        let p1_z = center_tile + dz;
        let mut eval1 = self.evaluator.evaluate(
            p1_x,
            p1_z,
            tribes[0],
            0,
            angle_rad * 180.0 / std::f32::consts::PI,
        );

        // Player 2
        let p2_x = center_tile - dx;
        let p2_z = center_tile - dz;
        let mut eval2 = self.evaluator.evaluate(
            p2_x,
            p2_z,
            tribes[1],
            1,
            (angle_rad * 180.0 / std::f32::consts::PI + 180.0) % 360.0,
        );

        if !eval1.is_valid || !eval2.is_valid {
            let (e1, e2) = self.search_symmetric_pair(center_tile, spawn_radius_tiles, tribes, angle_rad);
            eval1 = e1;
            eval2 = e2;
        }

        let (f1, s1, w1) = self.planner.plan_clusters(
            eval1.anchor_tile_x,
            eval1.anchor_tile_z,
            tribes[0],
            seed + 1,
            angle_rad * 180.0 / std::f32::consts::PI,
        );
        let (f2, s2, w2) = self.planner.plan_clusters(
            eval2.anchor_tile_x,
            eval2.anchor_tile_z,
            tribes[1],
            seed + 2,
            (angle_rad * 180.0 / std::f32::consts::PI + 180.0) % 360.0,
        );

        vec![
            PlayerSettlementLayout {
                team_index: 0,
                tribe: tribes[0],
                buildings: eval1.planned_buildings.clone(),
                site_evaluation: eval1,
                forest_trees: f1,
                stone_quarries: s1,
                wildlife_and_food: w1,
            },
            PlayerSettlementLayout {
                team_index: 1,
                tribe: tribes[1],
                buildings: eval2.planned_buildings.clone(),
                site_evaluation: eval2,
                forest_trees: f2,
                stone_quarries: s2,
                wildlife_and_food: w2,
            },
        ]
    }

    fn generate_rotational_symmetry(
        &self,
        count: usize,
        center_tile: f32,
        spawn_radius_tiles: f32,
        tribes: &[SettlementTribe],
        seed: i32,
    ) -> Vec<PlayerSettlementLayout> {
        let mut random = DotNetRandom::new(seed);
        let base_angle = (random.next_double() * (std::f64::consts::PI * 2.0 / count as f64)) as f32;
        let mut layouts = Vec::new();

        for i in 0..count {
            let angle = base_angle + i as f32 * (std::f32::consts::PI * 2.0 / count as f32);
            let tile_x = center_tile + angle.cos() * spawn_radius_tiles;
            let tile_z = center_tile + angle.sin() * spawn_radius_tiles;
            let angle_deg = angle * 180.0 / std::f32::consts::PI;

            let mut eval = self.evaluator.evaluate(tile_x, tile_z, tribes[i], i as i32, angle_deg);
            if !eval.is_valid {
                eval = self.search_near_valid_site(tile_x, tile_z, tribes[i], i as i32, angle_deg);
            }

            let (f, s, w) = self.planner.plan_clusters(
                eval.anchor_tile_x,
                eval.anchor_tile_z,
                tribes[i],
                seed + i as i32 * 7 + 1,
                angle_deg,
            );

            layouts.push(PlayerSettlementLayout {
                team_index: i as i32,
                tribe: tribes[i],
                buildings: eval.planned_buildings.clone(),
                site_evaluation: eval,
                forest_trees: f,
                stone_quarries: s,
                wildlife_and_food: w,
            });
        }

        layouts
    }

    fn generate_topological_equidistant(
        &self,
        count: usize,
        center_tile: f32,
        spawn_radius_tiles: f32,
        tribes: &[SettlementTribe],
        seed: i32,
    ) -> Vec<PlayerSettlementLayout> {
        let mut positions = Vec::new();

        for i in 0..count {
            let a = i as f32 * (std::f32::consts::PI * 2.0 / count as f32);
            positions.push((
                center_tile + a.cos() * spawn_radius_tiles,
                center_tile + a.sin() * spawn_radius_tiles,
            ));
        }

        let dim_f = self.dimension as f32;

        // 5 次鬆弛排斥迭代
        for _ in 0..5 {
            for i in 0..count {
                let mut fx = 0.0f32;
                let mut fz = 0.0f32;

                for j in 0..count {
                    if i == j {
                        continue;
                    }
                    let dx = positions[i].0 - positions[j].0;
                    let dz = positions[i].1 - positions[j].1;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > 0.01 {
                        let force = 100.0 / (dist * dist);
                        fx += (dx / dist) * force;
                        fz += (dz / dist) * force;
                    }
                }

                // 牽引力向 spawn_radius_tiles 軌道
                let cdx = positions[i].0 - center_tile;
                let cdz = positions[i].1 - center_tile;
                let current_r = (cdx * cdx + cdz * cdz).sqrt();
                if current_r > 0.01 {
                    let r_error = spawn_radius_tiles - current_r;
                    fx += (cdx / current_r) * (r_error * 0.1);
                    fz += (cdz / current_r) * (r_error * 0.1);
                }

                positions[i] = (
                    (positions[i].0 + fx).clamp(20.0, dim_f - 21.0),
                    (positions[i].1 + fz).clamp(20.0, dim_f - 21.0),
                );
            }
        }

        let mut layouts = Vec::new();
        for i in 0..count {
            let tx = positions[i].0;
            let tz = positions[i].1;
            let angle_deg = (tz - center_tile).atan2(tx - center_tile) * 180.0 / std::f32::consts::PI;

            let mut eval = self.evaluator.evaluate(tx, tz, tribes[i], i as i32, angle_deg);
            if !eval.is_valid {
                eval = self.search_near_valid_site(tx, tz, tribes[i], i as i32, angle_deg);
            }

            let (f, s, w) = self.planner.plan_clusters(
                eval.anchor_tile_x,
                eval.anchor_tile_z,
                tribes[i],
                seed + i as i32 * 11 + 3,
                angle_deg,
            );

            layouts.push(PlayerSettlementLayout {
                team_index: i as i32,
                tribe: tribes[i],
                buildings: eval.planned_buildings.clone(),
                site_evaluation: eval,
                forest_trees: f,
                stone_quarries: s,
                wildlife_and_food: w,
            });
        }

        layouts
    }

    fn search_symmetric_pair(
        &self,
        center_tile: f32,
        spawn_radius_tiles: f32,
        tribes: &[SettlementTribe],
        initial_angle_rad: f32,
    ) -> (
        SettlementSiteEvaluationResult,
        SettlementSiteEvaluationResult,
    ) {
        for step in 1..=12 {
            let d_angle = step as f32 * (std::f32::consts::PI / 24.0);
            for &sign in &[1.0f32, -1.0] {
                let a = initial_angle_rad + sign * d_angle;
                let dx = a.cos() * spawn_radius_tiles;
                let dz = a.sin() * spawn_radius_tiles;

                let e1 = self.evaluator.evaluate(
                    center_tile + dx,
                    center_tile + dz,
                    tribes[0],
                    0,
                    a * 180.0 / std::f32::consts::PI,
                );
                let e2 = self.evaluator.evaluate(
                    center_tile - dx,
                    center_tile - dz,
                    tribes[1],
                    1,
                    (a * 180.0 / std::f32::consts::PI + 180.0) % 360.0,
                );

                if e1.is_valid && e2.is_valid {
                    return (e1, e2);
                }
            }
        }

        let def_dx = initial_angle_rad.cos() * spawn_radius_tiles;
        let def_dz = initial_angle_rad.sin() * spawn_radius_tiles;
        (
            self.evaluator.evaluate(center_tile + def_dx, center_tile + def_dz, tribes[0], 0, 0.0),
            self.evaluator.evaluate(center_tile - def_dx, center_tile - def_dz, tribes[1], 1, 0.0),
        )
    }

    fn search_near_valid_site(
        &self,
        tile_x: f32,
        tile_z: f32,
        tribe: SettlementTribe,
        team: i32,
        angle_deg: f32,
    ) -> SettlementSiteEvaluationResult {
        let mut r = 2.0f32;
        let dim_f = self.dimension as f32;
        while r <= 14.0 {
            let mut a = 0.0f32;
            while a < 360.0 {
                let rad = a * std::f32::consts::PI / 180.0;
                let cx = (tile_x + rad.cos() * r).clamp(20.0, dim_f - 21.0);
                let cz = (tile_z + rad.sin() * r).clamp(20.0, dim_f - 21.0);

                let res = self.evaluator.evaluate(cx, cz, tribe, team, angle_deg);
                if res.is_valid {
                    return res;
                }
                a += 30.0;
            }
            r += 2.0;
        }

        self.evaluator.evaluate(tile_x, tile_z, tribe, team, angle_deg)
    }

    pub fn compute_fairness_report(layouts: &[PlayerSettlementLayout]) -> FairnessScoreReport {
        if layouts.len() < 2 {
            return FairnessScoreReport {
                average_inter_player_distance: 0.0,
                min_inter_player_distance: 0.0,
                max_inter_player_distance: 0.0,
                wood_resource_variance: 0.0,
                stone_resource_variance: 0.0,
                food_resource_variance: 0.0,
                is_balanced: true,
            };
        }

        let mut distances = Vec::new();
        for i in 0..layouts.len() {
            for j in (i + 1)..layouts.len() {
                let dx = layouts[i].site_evaluation.world_x - layouts[j].site_evaluation.world_x;
                let dz = layouts[i].site_evaluation.world_z - layouts[j].site_evaluation.world_z;
                distances.push((dx * dx + dz * dz).sqrt());
            }
        }

        let avg_dist = distances.iter().sum::<f32>() / distances.len() as f32;
        let min_dist = distances.iter().copied().fold(f32::MAX, f32::min);
        let max_dist = distances.iter().copied().fold(f32::MIN, f32::max);

        let wood_var = compute_variance(&layouts.iter().map(|l| l.forest_trees.len() as f32).collect::<Vec<_>>());
        let stone_var = compute_variance(&layouts.iter().map(|l| l.stone_quarries.len() as f32).collect::<Vec<_>>());
        let food_var = compute_variance(&layouts.iter().map(|l| l.wildlife_and_food.len() as f32).collect::<Vec<_>>());

        let is_balanced = (max_dist - min_dist) / avg_dist.max(1.0) < 0.35
            && wood_var <= 9.0
            && stone_var <= 2.0
            && food_var <= 1.0;

        FairnessScoreReport {
            average_inter_player_distance: avg_dist,
            min_inter_player_distance: min_dist,
            max_inter_player_distance: max_dist,
            wood_resource_variance: wood_var,
            stone_resource_variance: stone_var,
            food_resource_variance: food_var,
            is_balanced,
        }
    }
}

fn compute_variance(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let avg = values.iter().sum::<f32>() / values.len() as f32;
    values.iter().map(|&v| (v - avg) * (v - avg)).sum::<f32>() / values.len() as f32
}
