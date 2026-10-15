//! Biome Ecology & Flora Auto-Scatterer.
//! Based on multi-octave Perlin noise, elevation hydrology, and slope weighting.

use super::biome::{BiomeEcologyProfile, VegetationLayer};
use crate::noise::{octave_noise_2d, perlin_2d};
use crate::random::DotNetRandom;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// 自然生態植被散播演算法參數。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FloraScatterParameters {
    pub seed: i32,
    pub density_multiplier: f32,
    pub forest_coverage: f32,
    pub forest_patch_scale: f32,
    pub min_tile_x: i32,
    pub min_tile_y: i32,
    pub max_tile_x: i32,
    pub max_tile_y: i32,
    pub jitter_amount: f32,
    pub avoid_roads_and_buildings: bool,
    pub enable_riparian_flora: bool,
    pub enable_cliff_rocks: bool,
}

impl Default for FloraScatterParameters {
    fn default() -> Self {
        Self {
            seed: 1337,
            density_multiplier: 1.0,
            forest_coverage: 1.0,
            forest_patch_scale: 0.035,
            min_tile_x: 0,
            min_tile_y: 0,
            max_tile_x: 255,
            max_tile_y: 255,
            jitter_amount: 0.65,
            avoid_roads_and_buildings: true,
            enable_riparian_flora: true,
            enable_cliff_rocks: true,
        }
    }
}

/// 一個已規劃的自然物件放置項目。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NatureAddition {
    pub type_id: i32,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rotation: f32,
}

impl NatureAddition {
    pub fn new(type_id: i32, name: impl Into<String>, x: f32, y: f32, z: f32, rotation: f32) -> Self {
        Self {
            type_id,
            name: name.into(),
            x,
            y,
            z,
            rotation,
        }
    }
}

/// 生態散播引擎的輸入環境上下文。
#[derive(Debug, Clone)]
pub struct FloraScatterContext {
    pub dimension: i32,
    pub tile_world_size: f32,
    pub heights: Option<Vec<u8>>,
    pub vertex_size: i32,
    pub height_map_step: f32,
    pub water_level: u8,
    pub collision: Option<Vec<u8>>,
    pub obstacle_circles: Option<Vec<(f32, f32, f32)>>, // (x, z, radius_tiles)
    pub road_tiles: Option<HashSet<i32>>,
    pub existing_additions: Option<Vec<(f32, f32)>>,
    pub profile: BiomeEcologyProfile,
    pub available_templates: Vec<i32>,
    pub objdef_names: HashMap<i32, String>,
}

impl Default for FloraScatterContext {
    fn default() -> Self {
        Self {
            dimension: 256,
            tile_world_size: 64.0,
            heights: None,
            vertex_size: 257,
            height_map_step: 4.0,
            water_level: 120,
            collision: None,
            obstacle_circles: None,
            road_tiles: None,
            existing_additions: None,
            profile: BiomeEcologyProfile::germanic_forest(),
            available_templates: Vec::new(),
            objdef_names: HashMap::new(),
        }
    }
}

/// 散播執行統計報告。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloraScatterReport {
    pub total_planted: usize,
    pub canopy_count: usize,
    pub understory_count: usize,
    pub ground_flora_count: usize,
    pub riparian_count: usize,
    pub rock_count: usize,
    pub avoided_obstacles: usize,
    pub biome_name: String,
}

/// 散播引擎生成結果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FloraScatterResult {
    pub additions: Vec<NatureAddition>,
    pub report: FloraScatterReport,
}

/// 自然生態圈植被與生物群落自動散播引擎。
pub struct FloraScatterEngine;

impl FloraScatterEngine {
    pub fn generate(
        context: &FloraScatterContext,
        parameters: &FloraScatterParameters,
    ) -> FloraScatterResult {
        let roster = context
            .profile
            .resolve_templates(&context.available_templates, &context.objdef_names);

        if roster.total_species_count() == 0 {
            return FloraScatterResult {
                additions: Vec::new(),
                report: FloraScatterReport {
                    total_planted: 0,
                    canopy_count: 0,
                    understory_count: 0,
                    ground_flora_count: 0,
                    riparian_count: 0,
                    rock_count: 0,
                    avoided_obstacles: 0,
                    biome_name: context.profile.display_name_zh.clone(),
                },
            };
        }

        let mut additions = Vec::new();
        let mut spatial_grid = SpatialClearanceGrid::new(
            context.dimension as f32 * context.tile_world_size,
            2.0 * context.tile_world_size,
        );

        // 預填既有物件至空間網格，避免新植被重疊
        if let Some(existing) = &context.existing_additions {
            let default_spacing = 0.65 * context.tile_world_size;
            for &(x, z) in existing {
                spatial_grid.add(x, z, default_spacing);
            }
        }

        // 預填建築與特殊障礙圓形
        if parameters.avoid_roads_and_buildings {
            if let Some(obstacles) = &context.obstacle_circles {
                for &(ox, oz, radius_tiles) in obstacles {
                    spatial_grid.add(ox, oz, radius_tiles * context.tile_world_size);
                }
            }
        }

        let mut random = DotNetRandom::new(parameters.seed);
        let mut avoided_obstacles = 0;
        let mut canopy_count = 0;
        let mut understory_count = 0;
        let mut ground_flora_count = 0;
        let mut riparian_count = 0;
        let mut rock_count = 0;

        let min_x = parameters.min_tile_x.clamp(0, context.dimension - 1);
        let max_x = parameters.max_tile_x.clamp(0, context.dimension - 1);
        let min_y = parameters.min_tile_y.clamp(0, context.dimension - 1);
        let max_y = parameters.max_tile_y.clamp(0, context.dimension - 1);

        let water_world_y = context.water_level as f32 * context.height_map_step;
        let has_heights = context
            .heights
            .as_ref()
            .map(|h| !h.is_empty())
            .unwrap_or(false);

        for tz in min_y..=max_y {
            for tx in min_x..=max_x {
                let tile_index = tz * context.dimension + tx;

                // 1. 道路避障
                if parameters.avoid_roads_and_buildings {
                    if let Some(roads) = &context.road_tiles {
                        if roads.contains(&tile_index) {
                            avoided_obstacles += 1;
                            continue;
                        }
                    }
                }

                // 2. 碰撞阻擋避障 (collision 255)
                if parameters.avoid_roads_and_buildings {
                    if let Some(col) = &context.collision {
                        if (tile_index as usize) < col.len() && col[tile_index as usize] == 255 {
                            avoided_obstacles += 1;
                            continue;
                        }
                    }
                }

                // 3. 高度與坡度計算
                let elevation_byte = sample_tile_elevation_byte(tx, tz, context);
                let tile_world_y = elevation_byte as f32 * context.height_map_step;
                let slope_degrees = calculate_slope_degrees(tx, tz, context);

                // 4. 水位檢查：過深水域（水深超過 2.5 步長單位）禁止陸生植被生長
                let water_depth = water_world_y - tile_world_y;
                if has_heights && water_depth > 2.5 * context.height_map_step {
                    continue;
                }

                // 5. 生態生境決策
                let chosen_layer: VegetationLayer;

                // 生境 A: 水岸濱水帶 (Riparian)
                if has_heights
                    && parameters.enable_riparian_flora
                    && (tile_world_y - water_world_y).abs()
                        <= context.profile.riparian_elevation_delta * context.height_map_step
                    && roster.has_layer(VegetationLayer::Riparian)
                {
                    let rip_noise = perlin_2d(tx as f32 * 0.12, tz as f32 * 0.12, parameters.seed + 333);
                    let rip_prob = 0.40 * parameters.density_multiplier;
                    if rip_noise > 0.35 && random.next_double() < rip_prob as f64 {
                        chosen_layer = VegetationLayer::Riparian;
                    } else {
                        continue;
                    }
                }
                // 生境 B: 懸崖陡坡岩石 (Cliff / Rock)
                else if parameters.enable_cliff_rocks
                    && slope_degrees >= context.profile.cliff_slope_threshold
                {
                    let rock_noise = perlin_2d(tx as f32 * 0.15, tz as f32 * 0.15, parameters.seed + 555);
                    if rock_noise > 0.45 && roster.has_layer(VegetationLayer::Rock) {
                        chosen_layer = VegetationLayer::Rock;
                    } else if rock_noise > 0.28 && roster.has_layer(VegetationLayer::Understory) {
                        chosen_layer = VegetationLayer::Understory;
                    } else {
                        continue; // 陡峭裸岩，不生長物件
                    }
                }
                // 生境 C: 森林矩陣 (Forest & Ecotone Matrix)
                else {
                    let macro_noise = octave_noise_2d(
                        tx as f32 * parameters.forest_patch_scale,
                        tz as f32 * parameters.forest_patch_scale,
                        3,
                        0.5,
                        2.0,
                        parameters.seed,
                    );
                    let micro_noise = octave_noise_2d(
                        tx as f32 * 0.14,
                        tz as f32 * 0.14,
                        2,
                        0.5,
                        2.0,
                        parameters.seed + 777,
                    );

                    let forest_index =
                        (macro_noise * 0.72 + micro_noise * 0.28) * parameters.forest_coverage;

                    if forest_index >= context.profile.canopy_threshold {
                        // 深林核心區（Core Forest）：高密度、以高大喬木為主
                        let chance = 0.82 * parameters.density_multiplier;
                        if random.next_double() > chance as f64 {
                            continue;
                        }

                        if random.next_double() < 0.85 && roster.has_layer(VegetationLayer::Canopy) {
                            chosen_layer = VegetationLayer::Canopy;
                        } else if roster.has_layer(VegetationLayer::Understory) {
                            chosen_layer = VegetationLayer::Understory;
                        } else {
                            chosen_layer = VegetationLayer::Canopy;
                        }
                    } else if forest_index >= context.profile.understory_threshold {
                        // 林緣過渡帶（Canopy Ecotone）：灌木繁茂，次冠喬木
                        let chance = 0.58 * parameters.density_multiplier;
                        if random.next_double() > chance as f64 {
                            continue;
                        }

                        let roll = random.next_double();
                        if roll < 0.62 && roster.has_layer(VegetationLayer::Understory) {
                            chosen_layer = VegetationLayer::Understory;
                        } else if roll < 0.85 && roster.has_layer(VegetationLayer::Canopy) {
                            chosen_layer = VegetationLayer::Canopy;
                        } else if roster.has_layer(VegetationLayer::GroundFlora) {
                            chosen_layer = VegetationLayer::GroundFlora;
                        } else {
                            chosen_layer = VegetationLayer::Understory;
                        }
                    } else if forest_index >= context.profile.ground_flora_threshold {
                        // 林窗草甸（Meadow Glade）：地表草花為主，偶有點綴矮灌木
                        let chance = 0.38 * parameters.density_multiplier;
                        if random.next_double() > chance as f64 {
                            continue;
                        }

                        let roll = random.next_double();
                        if roll < 0.70 && roster.has_layer(VegetationLayer::GroundFlora) {
                            chosen_layer = VegetationLayer::GroundFlora;
                        } else if roll < 0.88 && roster.has_layer(VegetationLayer::Understory) {
                            chosen_layer = VegetationLayer::Understory;
                        } else if roster.has_layer(VegetationLayer::Rock) {
                            chosen_layer = VegetationLayer::Rock;
                        } else {
                            chosen_layer = VegetationLayer::GroundFlora;
                        }
                    } else {
                        // 開闊平原（Open Plains）：極稀疏野草，留白維持視野通透
                        let chance = 0.08 * parameters.density_multiplier;
                        if random.next_double() > chance as f64 {
                            continue;
                        }

                        if roster.has_layer(VegetationLayer::GroundFlora) {
                            chosen_layer = VegetationLayer::GroundFlora;
                        } else {
                            continue;
                        }
                    }
                }

                // 6. 挑選物種並驗證物種專屬坡度/海拔容許度
                let species = match roster.try_pick(chosen_layer, &mut random) {
                    Some(s) => s,
                    None => continue,
                };

                if slope_degrees > species.max_slope {
                    continue;
                }
                if elevation_byte < species.min_elevation || elevation_byte > species.max_elevation {
                    continue;
                }

                // 7. 計算格內抖動坐標與世界空間位置
                let jitter = parameters.jitter_amount;
                let jitter_x = 0.5 + (random.next_double() - 0.5) as f32 * jitter;
                let jitter_z = 0.5 + (random.next_double() - 0.5) as f32 * jitter;

                let mut world_x = (tx as f32 + jitter_x) * context.tile_world_size;
                let mut world_z = (tz as f32 + jitter_z) * context.tile_world_size;

                world_x = world_x.clamp(0.0, 16383.0);
                world_z = world_z.clamp(0.0, 16383.0);

                let footprint_world = species.footprint_radius_tiles * context.tile_world_size;

                // 8. 空間佔位防重疊檢查
                if !spatial_grid.is_free(world_x, world_z, footprint_world) {
                    avoided_obstacles += 1;
                    continue;
                }

                // 9. 精確表面高度雙線性插值取樣
                let surface_y = interpolate_surface_world_height(world_x, world_z, context);

                // 10. 隨機旋轉角 [0, 2π)
                let rotation = (random.next_double() * std::f64::consts::PI * 2.0) as f32;

                // 11. 註冊至空間網格並記錄新增
                spatial_grid.add(world_x, world_z, footprint_world);
                additions.push(NatureAddition::new(
                    species.type_id,
                    species.name,
                    world_x,
                    surface_y,
                    world_z,
                    rotation,
                ));

                match chosen_layer {
                    VegetationLayer::Canopy => canopy_count += 1,
                    VegetationLayer::Understory => understory_count += 1,
                    VegetationLayer::GroundFlora => ground_flora_count += 1,
                    VegetationLayer::Riparian => riparian_count += 1,
                    VegetationLayer::Rock => rock_count += 1,
                }
            }
        }

        let report = FloraScatterReport {
            total_planted: additions.len(),
            canopy_count,
            understory_count,
            ground_flora_count,
            riparian_count,
            rock_count,
            avoided_obstacles,
            biome_name: context.profile.display_name_zh.clone(),
        };

        FloraScatterResult { additions, report }
    }
}

fn sample_tile_elevation_byte(tx: i32, tz: i32, context: &FloraScatterContext) -> u8 {
    let heights = match &context.heights {
        Some(h) if !h.is_empty() => h,
        _ => return 0,
    };
    if context.dimension <= 0 {
        return 0;
    }
    let step = (context.vertex_size - 1) / context.dimension;
    let vx = (tx * step).clamp(0, context.vertex_size - 1) as usize;
    let vz = (tz * step).clamp(0, context.vertex_size - 1) as usize;
    let index = vz * (context.vertex_size as usize) + vx;
    if index < heights.len() {
        heights[index]
    } else {
        0
    }
}

fn calculate_slope_degrees(tx: i32, tz: i32, context: &FloraScatterContext) -> f32 {
    let heights = match &context.heights {
        Some(h) if !h.is_empty() => h,
        _ => return 0.0,
    };
    if context.dimension <= 0 {
        return 0.0;
    }
    let step = (context.vertex_size - 1) / context.dimension;
    let vx = (tx * step).clamp(0, context.vertex_size - 1);
    let vz = (tz * step).clamp(0, context.vertex_size - 1);

    let vx_prev = (vx - 1).max(0) as usize;
    let vx_next = (vx + 1).min(context.vertex_size - 1) as usize;
    let vz_prev = (vz - 1).max(0) as usize;
    let vz_next = (vz + 1).min(context.vertex_size - 1) as usize;

    let v_size = context.vertex_size as usize;
    let vx_u = vx as usize;
    let vz_u = vz as usize;

    let hx_prev = heights[vz_u * v_size + vx_prev] as f32 * context.height_map_step;
    let hx_next = heights[vz_u * v_size + vx_next] as f32 * context.height_map_step;
    let hz_prev = heights[vz_prev * v_size + vx_u] as f32 * context.height_map_step;
    let hz_next = heights[vz_next * v_size + vx_u] as f32 * context.height_map_step;

    let dx = (hx_next - hx_prev).abs();
    let dz = (hz_next - hz_prev).abs();
    let dist = 2.0 * context.tile_world_size;

    let slope_rad = ((dx * dx + dz * dz).sqrt()).atan2(dist);
    slope_rad * (180.0 / std::f32::consts::PI)
}

fn interpolate_surface_world_height(world_x: f32, world_z: f32, context: &FloraScatterContext) -> f32 {
    let heights = match &context.heights {
        Some(h) if !h.is_empty() => h,
        _ => return 0.0,
    };

    let tile_x = world_x / context.tile_world_size;
    let tile_z = world_z / context.tile_world_size;

    let step = (context.vertex_size - 1) as f32 / context.dimension as f32;
    let vx = (tile_x * step).clamp(0.0, (context.vertex_size - 1) as f32);
    let vz = (tile_z * step).clamp(0.0, (context.vertex_size - 1) as f32);

    let x0 = vx.floor() as usize;
    let z0 = vz.floor() as usize;
    let x1 = (x0 + 1).min((context.vertex_size - 1) as usize);
    let z1 = (z0 + 1).min((context.vertex_size - 1) as usize);

    let fx = vx - x0 as f32;
    let fz = vz - z0 as f32;

    let v_size = context.vertex_size as usize;
    let h00 = heights[z0 * v_size + x0] as f32;
    let h10 = heights[z0 * v_size + x1] as f32;
    let h01 = heights[z1 * v_size + x0] as f32;
    let h11 = heights[z1 * v_size + x1] as f32;

    let h0 = h00 + fx * (h10 - h00);
    let h1 = h01 + fx * (h11 - h01);
    let h = h0 + fz * (h1 - h0);

    h * context.height_map_step
}

/// 高效二維空間雜湊網格，提供 O(1) 佔位半徑避碰查詢。
#[derive(Debug, Clone)]
pub struct SpatialClearanceGrid {
    cell_size: f32,
    cells_per_axis: i32,
    grid: HashMap<i32, Vec<(f32, f32, f32)>>, // (x, z, radius)
}

impl SpatialClearanceGrid {
    pub fn new(world_extent: f32, cell_size: f32) -> Self {
        let size = if cell_size > 0.0 { cell_size } else { 128.0 };
        let cells_per_axis = (world_extent / size).ceil() as i32 + 1;
        Self {
            cell_size: size,
            cells_per_axis,
            grid: HashMap::new(),
        }
    }

    pub fn add(&mut self, x: f32, z: f32, radius: f32) {
        let cell_x = (x / self.cell_size).max(0.0) as i32;
        let cell_z = (z / self.cell_size).max(0.0) as i32;
        let key = cell_z * self.cells_per_axis + cell_x;
        self.grid.entry(key).or_default().push((x, z, radius));
    }

    pub fn is_free(&self, x: f32, z: f32, radius: f32) -> bool {
        let center_cell_x = (x / self.cell_size).max(0.0) as i32;
        let center_cell_z = (z / self.cell_size).max(0.0) as i32;

        let reach = (radius / self.cell_size).ceil() as i32 + 1;

        for cz in (center_cell_z - reach)..=(center_cell_z + reach) {
            if cz < 0 {
                continue;
            }
            for cx in (center_cell_x - reach)..=(center_cell_x + reach) {
                if cx < 0 {
                    continue;
                }
                let key = cz * self.cells_per_axis + cx;
                if let Some(list) = self.grid.get(&key) {
                    for &(ix, iz, iradius) in list {
                        let dx = ix - x;
                        let dz = iz - z;
                        let min_dist = iradius + radius;
                        if dx * dx + dz * dz < min_dist * min_dist {
                            return false;
                        }
                    }
                }
            }
        }

        true
    }
}
