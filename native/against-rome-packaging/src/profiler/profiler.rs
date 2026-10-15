use super::limits::MapBudgetLimits;
use super::report::*;
use against_rome_scenario::model::Spawn;
use chrono::Utc;

#[derive(Clone, Debug, PartialEq)]
pub struct MapSceneObject {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rotation: f32,
    pub source_sdl: String,
    pub team: i32,
}

impl MapSceneObject {
    pub fn new(
        name: impl Into<String>,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
        source_sdl: impl Into<String>,
        team: i32,
    ) -> Self {
        Self {
            name: name.into(),
            x,
            y,
            z,
            rotation,
            source_sdl: source_sdl.into(),
            team,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeLightInstance {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub radius: f32,
    pub is_active: bool,
    pub index: u32,
}

impl NativeLightInstance {
    pub fn new(x: f32, y: f32, z: f32, radius: f32) -> Self {
        Self {
            x,
            y,
            z,
            radius,
            is_active: true,
            index: 1,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct MapBudgetSnapshot {
    pub scene_objects: Option<Vec<MapSceneObject>>,
    pub spawns: Option<Vec<Spawn>>,
    pub lights: Option<Vec<NativeLightInstance>>,
    pub collision_size: usize,
    pub collision: Option<Vec<u8>>,
    pub height_size: usize,
    pub heights: Option<Vec<u8>>,
    pub height_step: f32,
    pub water_level: f32,
    pub decoded_sprite_count: usize,
    pub decoded_shadow_count: usize,
    pub decoded_sprite_types: usize,
}

pub struct MapBudgetProfiler;

impl MapBudgetProfiler {
    pub fn analyze(snapshot: &MapBudgetSnapshot) -> MapBudgetReport {
        let mut bottlenecks = Vec::new();

        let object_budget = Self::analyze_objects(snapshot, &mut bottlenecks);
        let light_budget = Self::analyze_lights(snapshot, &mut bottlenecks);
        let draw_budget = Self::analyze_draw_calls(snapshot, &object_budget, &mut bottlenecks);
        let path_budget = Self::analyze_pathfinding(snapshot, &mut bottlenecks);

        let (grade, overall_score) = Self::compute_overall_health(
            &object_budget,
            &light_budget,
            &draw_budget,
            &path_budget,
            &bottlenecks,
        );

        MapBudgetReport {
            timestamp: Utc::now(),
            overall_grade: grade,
            overall_score,
            object_budget,
            light_budget,
            draw_budget,
            path_budget,
            bottlenecks,
        }
    }

    fn analyze_objects(snapshot: &MapBudgetSnapshot, bottlenecks: &mut Vec<MapHealthBottleneck>) -> ObjectSlotBudget {
        let scene_count = snapshot.scene_objects.as_ref().map_or(0, |v| v.len());
        let spawn_count = snapshot.spawns.as_ref().map_or(0, |v| v.len());
        let total_active = scene_count + spawn_count;

        let mut landscape_count = 0;
        let mut building_count = 0;
        let mut unit_count = 0;
        let mut fx_count = 0;
        let mut other_count = 0;

        let mut classify = |name: &str| {
            let lower = name.to_ascii_lowercase();
            if lower.starts_with("lan") {
                landscape_count += 1;
            } else if lower.starts_with("bau") {
                building_count += 1;
            } else if lower.starts_with("fig")
                || lower.starts_with("ger")
                || lower.starts_with("rom")
                || lower.starts_with("cel")
                || lower.starts_with("hun")
            {
                unit_count += 1;
            } else if lower.starts_with("fx") || lower.starts_with("fil") {
                fx_count += 1;
            } else {
                other_count += 1;
            }
        };

        if let Some(ref scene) = snapshot.scene_objects {
            for obj in scene {
                classify(&obj.name);
            }
        }
        if let Some(ref spawns) = snapshot.spawns {
            for spawn in spawns {
                classify(&spawn.alias);
            }
        }

        let obj_percent = (total_active as f32 / MapBudgetLimits::MAX_OBJECTS_DAT_SLOTS as f32) * 100.0;
        let estimated_positions = total_active * 2;
        let pos_percent = (estimated_positions as f32 / MapBudgetLimits::MAX_POSITION_DAT_RECORDS as f32) * 100.0;

        if total_active > MapBudgetLimits::MAX_OBJECTS_DAT_SLOTS {
            bottlenecks.push(MapHealthBottleneck::new(
                "Objects",
                "objects-dat-overflow",
                format!("物件總數 ({total_active}) 已超出 objects.dat 原生硬限制 ({})！遊戲執行期將崩潰或無法載入。", MapBudgetLimits::MAX_OBJECTS_DAT_SLOTS),
                format!("Total object count ({total_active}) exceeds native objects.dat hard limit ({})! The game will crash or fail to load.", MapBudgetLimits::MAX_OBJECTS_DAT_SLOTS),
                true,
            ));
        } else if total_active > MapBudgetLimits::REDLINE_OBJECTS_DAT_SLOTS {
            bottlenecks.push(MapHealthBottleneck::new(
                "Objects",
                "objects-dat-redline",
                format!("物件總數 ({total_active}) 超過安全警戒線 ({})，可能擠佔遊戲執行期動態生成空間。", MapBudgetLimits::REDLINE_OBJECTS_DAT_SLOTS),
                format!("Total object count ({total_active}) exceeds safe redline ({}), risking runtime dynamic spawn exhaustion.", MapBudgetLimits::REDLINE_OBJECTS_DAT_SLOTS),
                false,
            ));
        }

        if estimated_positions > MapBudgetLimits::MAX_POSITION_DAT_RECORDS {
            bottlenecks.push(MapHealthBottleneck::new(
                "Positions",
                "position-dat-overflow",
                format!("位置記錄需求 ({estimated_positions}) 超出 position.dat 標頭容量 ({})！", MapBudgetLimits::MAX_POSITION_DAT_RECORDS),
                format!("Required position records ({estimated_positions}) exceed position.dat capacity ({})!", MapBudgetLimits::MAX_POSITION_DAT_RECORDS),
                true,
            ));
        }

        if spawn_count > MapBudgetLimits::REDLINE_SCENARIO_SPAWNS {
            bottlenecks.push(MapHealthBottleneck::new(
                "Spawns",
                "spawns-redline",
                format!("劇本預放生成物件數 ({spawn_count}) 超過建議警戒線 ({})，開局可能引發腳本解譯延遲。", MapBudgetLimits::REDLINE_SCENARIO_SPAWNS),
                format!("Scenario spawn count ({spawn_count}) exceeds redline ({}), which may cause initial script lag.", MapBudgetLimits::REDLINE_SCENARIO_SPAWNS),
                false,
            ));
        }

        ObjectSlotBudget {
            total_active_objects: total_active,
            objects_dat_limit: MapBudgetLimits::MAX_OBJECTS_DAT_SLOTS,
            objects_usage_percent: obj_percent,
            estimated_position_records: estimated_positions,
            position_limit: MapBudgetLimits::MAX_POSITION_DAT_RECORDS,
            position_usage_percent: pos_percent,
            scenario_spawns_count: spawn_count,
            landscape_count,
            building_count,
            unit_count,
            fx_count,
            other_count,
        }
    }

    fn analyze_lights(snapshot: &MapBudgetSnapshot, bottlenecks: &mut Vec<MapHealthBottleneck>) -> LightDensityBudget {
        let lights = snapshot.lights.as_deref().unwrap_or(&[]);
        let total_lights = lights.len();

        const GRID_RES: usize = 32;
        let mut density_grid = vec![vec![0.0f32; GRID_RES]; GRID_RES];
        let cell_world_span = MapBudgetLimits::MAP_WORLD_SIZE / GRID_RES as f32; // 512.0

        let mut peak_cluster_lights = 0;
        let mut raw_hotspots = Vec::new();

        if total_lights > 0 {
            let mut count_grid = [[0usize; GRID_RES]; GRID_RES];

            for light in lights {
                if !light.is_active {
                    continue;
                }
                let lx = light.x.clamp(0.0, MapBudgetLimits::MAP_WORLD_SIZE - 1.0);
                let lz = light.z.clamp(0.0, MapBudgetLimits::MAP_WORLD_SIZE - 1.0);
                let radius = light.radius.max(50.0);

                let min_cx = (((lx - radius) / cell_world_span).floor() as usize).min(GRID_RES - 1);
                let max_cx = (((lx + radius) / cell_world_span).floor() as usize).min(GRID_RES - 1);
                let min_cz = (((lz - radius) / cell_world_span).floor() as usize).min(GRID_RES - 1);
                let max_cz = (((lz + radius) / cell_world_span).floor() as usize).min(GRID_RES - 1);

                for cz in min_cz..=max_cz {
                    for cx in min_cx..=max_cx {
                        let cell_center_x = (cx as f32 + 0.5) * cell_world_span;
                        let cell_center_z = (cz as f32 + 0.5) * cell_world_span;
                        let dx = cell_center_x - lx;
                        let dz = cell_center_z - lz;
                        let r_bound = radius + cell_world_span * 0.707;
                        if dx * dx + dz * dz <= r_bound * r_bound {
                            count_grid[cz][cx] += 1;
                        }
                    }
                }
            }

            for cz in 0..GRID_RES {
                for cx in 0..GRID_RES {
                    let c = count_grid[cz][cx];
                    if c > peak_cluster_lights {
                        peak_cluster_lights = c;
                    }
                    if c >= MapBudgetLimits::REDLINE_LIGHTS_PER_CLUSTER {
                        raw_hotspots.push((
                            (cx as f32 + 0.5) * cell_world_span,
                            (cz as f32 + 0.5) * cell_world_span,
                            c,
                            cell_world_span,
                        ));
                    }
                }
            }

            let max_count = (peak_cluster_lights as f32).max(1.0);
            for cz in 0..GRID_RES {
                for cx in 0..GRID_RES {
                    density_grid[cz][cx] = count_grid[cz][cx] as f32 / max_count;
                }
            }
        }

        // Hotspot clustering with non-max suppression within 600 world units
        let mut filtered_hotspots = Vec::new();
        raw_hotspots.sort_by(|a, b| b.2.cmp(&a.2));
        let mut visited = vec![false; raw_hotspots.len()];

        for i in 0..raw_hotspots.len() {
            if visited[i] {
                continue;
            }
            let h = raw_hotspots[i];
            let exceeds = h.2 > MapBudgetLimits::HARD_LIMIT_LIGHTS_PER_CLUSTER;
            filtered_hotspots.push(LightClusterHotspot {
                world_x: h.0,
                world_z: h.1,
                light_count: h.2,
                radius: h.3,
                exceeds_hard_limit: exceeds,
            });
            visited[i] = true;

            for j in i + 1..raw_hotspots.len() {
                if visited[j] {
                    continue;
                }
                let dx = h.0 - raw_hotspots[j].0;
                let dz = h.1 - raw_hotspots[j].1;
                if dx * dx + dz * dz < 600.0 * 600.0 {
                    visited[j] = true;
                }
            }
        }

        if total_lights > MapBudgetLimits::MAX_GLOBAL_RUNTIME_LIGHTS {
            bottlenecks.push(MapHealthBottleneck::new(
                "Lighting",
                "global-lights-overflow",
                format!("全域動態光源數量 ({total_lights}) 超出執行期陣列硬限制 ({})！", MapBudgetLimits::MAX_GLOBAL_RUNTIME_LIGHTS),
                format!("Total dynamic lights ({total_lights}) exceed runtime array limit ({})!", MapBudgetLimits::MAX_GLOBAL_RUNTIME_LIGHTS),
                true,
            ));
        }

        if peak_cluster_lights > MapBudgetLimits::HARD_LIMIT_LIGHTS_PER_CLUSTER {
            let worst = filtered_hotspots.iter().find(|h| h.exceeds_hard_limit);
            let mut b = MapHealthBottleneck::new(
                "Lighting",
                "cluster-lights-overflow",
                format!("局部區域重疊光源峰值高達 {peak_cluster_lights}，已突破原生管線 64 光源硬限制！超出部分將被強制截斷，引發閃爍瑕疵。"),
                format!("Peak local light overlap ({peak_cluster_lights}) exceeds native 64 lights limit! Excess lights will be dropped, causing flickering."),
                true,
            );
            if let Some(w) = worst {
                b = b.with_location(w.world_x, w.world_z);
            }
            bottlenecks.push(b);
        } else if peak_cluster_lights >= MapBudgetLimits::REDLINE_LIGHTS_PER_CLUSTER {
            let worst = filtered_hotspots.first();
            let mut b = MapHealthBottleneck::new(
                "Lighting",
                "cluster-lights-redline",
                format!("局部區域光源密集度接近上限 (當前峰值 {peak_cluster_lights} / 64)，相機平移時可能發生動態光源更替與抖動。"),
                format!("Local light density approaches limit (peak {peak_cluster_lights} / 64). Camera movement may cause light popping."),
                false,
            );
            if let Some(w) = worst {
                b = b.with_location(w.world_x, w.world_z);
            }
            bottlenecks.push(b);
        }

        LightDensityBudget {
            total_scene_lights: total_lights,
            peak_cluster_lights,
            peak_density_ratio: peak_cluster_lights as f32 / MapBudgetLimits::MAX_ACTIVE_LIGHTS_IN_VIEW as f32,
            hotspots: filtered_hotspots,
            cluster_grid_resolution: GRID_RES,
            density_grid,
        }
    }

    fn analyze_draw_calls(
        snapshot: &MapBudgetSnapshot,
        object_budget: &ObjectSlotBudget,
        bottlenecks: &mut Vec<MapHealthBottleneck>,
    ) -> DrawCallBudget {
        let sprite_quads = if snapshot.decoded_sprite_count > 0 {
            snapshot.decoded_sprite_count
        } else {
            (object_budget.total_active_objects as f32 * 0.95) as usize
        };

        let shadow_quads = if snapshot.decoded_shadow_count > 0 {
            snapshot.decoded_shadow_count
        } else {
            (object_budget.total_active_objects as f32 * 0.30) as usize
        };

        let sprite_types = if snapshot.decoded_sprite_types > 0 {
            snapshot.decoded_sprite_types
        } else {
            (object_budget.total_active_objects / 30).clamp(10, 300)
        };

        let terrain_passes = 3;

        let base_sprite_batches = 1.max((sprite_quads as f32 / MapBudgetLimits::SAFE_SPRITE_QUADS_PER_DRAW_CALL as f32).ceil() as usize);
        let estimated_atlas_pages = 1.max((sprite_types as f32 / 220.0).ceil() as usize);

        let atlas_switches = if estimated_atlas_pages > 1 {
            let term = (sprite_quads as f32 / 40.0).min(150.0);
            ((estimated_atlas_pages - 1) as f32 * term).min(200.0) as usize
        } else {
            0
        };

        let total_sprite_batches = base_sprite_batches + atlas_switches;

        let shadow_batches = if shadow_quads > 0 {
            1.max((shadow_quads as f32 / MapBudgetLimits::MAX_SPRITE_QUADS_PER_BATCH as f32).ceil() as usize)
        } else {
            0
        };

        let total_draw_calls = terrain_passes + total_sprite_batches + shadow_batches;

        let frag_score = if sprite_quads > 0 {
            (atlas_switches as f32 / (sprite_quads as f32 / 50.0).max(1.0)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        if estimated_atlas_pages > MapBudgetLimits::REDLINE_ATLAS_PAGES {
            bottlenecks.push(MapHealthBottleneck::new(
                "Rendering",
                "atlas-pages-redline",
                format!("Sprite 種類過多，預估圖集頁數達 {estimated_atlas_pages} 頁 (建議 <= {})，將引起頻繁紋理切換並大幅增加 Draw Calls。", MapBudgetLimits::RECOMMENDED_ATLAS_PAGES),
                format!("Too many sprite types ({sprite_types}). Estimated atlas pages ({estimated_atlas_pages}) exceed recommended {}, causing texture thrashing.", MapBudgetLimits::RECOMMENDED_ATLAS_PAGES),
                false,
            ));
        }

        if sprite_quads > MapBudgetLimits::MAX_SPRITE_QUADS_PER_BATCH {
            bottlenecks.push(MapHealthBottleneck::new(
                "Rendering",
                "sprite-quads-overflow",
                format!("場景 Sprite 數量 ({sprite_quads}) 超出 16-bit 頂點索引單批次容量 (16,384 quads)，必須拆分多個繪製批次。"),
                format!("Scene sprite count ({sprite_quads}) exceeds 16-bit index capacity (16,384 quads), requiring multiple draw batches."),
                false,
            ));
        }

        if total_draw_calls > 100 {
            bottlenecks.push(MapHealthBottleneck::new(
                "Rendering",
                "high-draw-calls",
                format!("預估單幀 Draw Calls ({total_draw_calls}) 偏高，可能增加 CPU 驅動程式調度開銷。"),
                format!("Estimated draw calls per frame ({total_draw_calls}) are elevated, increasing CPU driver submission overhead."),
                false,
            ));
        }

        DrawCallBudget {
            estimated_total_draw_calls: total_draw_calls,
            terrain_draw_calls: terrain_passes,
            sprite_batch_draw_calls: total_sprite_batches,
            shadow_batch_draw_calls: shadow_batches,
            estimated_atlas_pages,
            estimated_atlas_switches: atlas_switches,
            batch_fragmentation_score: frag_score,
            total_sprite_quads: sprite_quads,
            total_shadow_quads: shadow_quads,
        }
    }

    fn analyze_pathfinding(snapshot: &MapBudgetSnapshot, bottlenecks: &mut Vec<MapHealthBottleneck>) -> PathfindingBudget {
        let size = if snapshot.collision_size > 0 {
            snapshot.collision_size
        } else {
            MapBudgetLimits::MAP_GRID_DIMENSION
        };

        let raw_collision = match snapshot.collision.as_deref() {
            Some(c) if c.len() == size * size => c,
            _ => {
                return PathfindingBudget {
                    obstacle_density_percent: 0.0,
                    passable_regions_count: 1,
                    chokepoint_count: 0,
                    chokepoints: Vec::new(),
                    dead_end_count: 0,
                    pathfinding_complexity_score: 0,
                    complexity_rating: "Unknown".to_string(),
                };
            }
        };

        let total_cells = size * size;
        let mut blocked = vec![false; total_cells];
        let mut blocked_count = 0;

        let has_height = snapshot.heights.as_deref().map_or(false, |h| {
            snapshot.height_size > 1
                && h.len() == snapshot.height_size * snapshot.height_size
                && snapshot.height_step.is_finite()
                && snapshot.water_level.is_finite()
        });

        for y in 0..size {
            for x in 0..size {
                let idx = y * size + x;
                let mut is_blocked = raw_collision[idx] != 0;

                if !is_blocked && has_height {
                    let h_size = snapshot.height_size;
                    let hx = (((x as f32 + 0.5) * (h_size - 1) as f32 / size as f32) as usize).min(h_size - 1);
                    let hy = (((y as f32 + 0.5) * (h_size - 1) as f32 / size as f32) as usize).min(h_size - 1);
                    let h_val = snapshot.heights.as_ref().unwrap()[hy * h_size + hx] as f32 * snapshot.height_step;
                    if h_val < snapshot.water_level {
                        is_blocked = true;
                    }
                }

                blocked[idx] = is_blocked;
                if is_blocked {
                    blocked_count += 1;
                }
            }
        }

        let obstacle_percent = (blocked_count as f32 / total_cells as f32) * 100.0;

        // 1. 連通區域分析 (Flood Fill)
        let mut components = vec![-1i32; total_cells];
        let mut component_count = 0;
        let mut queue = vec![0usize; total_cells];

        for origin in 0..total_cells {
            if blocked[origin] || components[origin] != -1 {
                continue;
            }

            let mut head = 0;
            let mut tail = 0;
            queue[tail] = origin;
            tail += 1;
            components[origin] = component_count;

            while head < tail {
                let curr = queue[head];
                head += 1;
                let cx = curr % size;
                let cy = curr / size;

                let try_visit = |next: usize, comp: &mut [i32], q: &mut [usize], t: &mut usize| {
                    if next < total_cells && !blocked[next] && comp[next] == -1 {
                        comp[next] = component_count;
                        q[*t] = next;
                        *t += 1;
                    }
                };

                if cx > 0 {
                    try_visit(curr - 1, &mut components, &mut queue, &mut tail);
                }
                if cx + 1 < size {
                    try_visit(curr + 1, &mut components, &mut queue, &mut tail);
                }
                if cy > 0 {
                    try_visit(curr - size, &mut components, &mut queue, &mut tail);
                }
                if cy + 1 < size {
                    try_visit(curr + size, &mut components, &mut queue, &mut tail);
                }
            }

            component_count += 1;
        }

        // 2. 窄道隘口偵測
        let mut chokepoint_cells = Vec::new();

        for y in 1..size - 1 {
            for x in 1..size - 1 {
                let idx = y * size + x;
                if blocked[idx] {
                    continue;
                }

                let left_wall = blocked[idx - 1];
                let right_wall = blocked[idx + 1] || (x + 2 < size && !blocked[idx + 1] && blocked[idx + 2]);
                let vert_passable = !blocked[idx - size] && !blocked[idx + size];

                if left_wall && right_wall && vert_passable {
                    let width = if blocked[idx + 1] { 1 } else { 2 };
                    chokepoint_cells.push((x, y, width));
                    continue;
                }

                let top_wall = blocked[idx - size];
                let bottom_wall = blocked[idx + size] || (y + 2 < size && !blocked[idx + size] && blocked[idx + 2 * size]);
                let horiz_passable = !blocked[idx - 1] && !blocked[idx + 1];

                if top_wall && bottom_wall && horiz_passable {
                    let width = if blocked[idx + size] { 1 } else { 2 };
                    chokepoint_cells.push((x, y, width));
                }
            }
        }

        // Cluster adjacent chokepoint cells
        let mut chokepoint_locations = Vec::new();
        let mut cp_visited = vec![false; chokepoint_cells.len()];

        for i in 0..chokepoint_cells.len() {
            if cp_visited[i] {
                continue;
            }
            let cp = chokepoint_cells[i];
            cp_visited[i] = true;

            let mut sum_x = cp.0;
            let mut sum_y = cp.1;
            let mut count = 1;
            let mut min_w = cp.2;

            for j in i + 1..chokepoint_cells.len() {
                if cp_visited[j] {
                    continue;
                }
                let other = chokepoint_cells[j];
                if (cp.0 as i32 - other.0 as i32).abs() <= 2 && (cp.1 as i32 - other.1 as i32).abs() <= 2 {
                    cp_visited[j] = true;
                    sum_x += other.0;
                    sum_y += other.1;
                    count += 1;
                    min_w = min_w.min(other.2);
                }
            }

            let avg_x = sum_x / count;
            let avg_y = sum_y / count;
            let wx = (avg_x as f32 + 0.5) * MapBudgetLimits::WORLD_UNITS_PER_TILE;
            let wz = (avg_y as f32 + 0.5) * MapBudgetLimits::WORLD_UNITS_PER_TILE;
            chokepoint_locations.push(ChokepointLocation {
                cell_x: avg_x,
                cell_z: avg_y,
                world_x: wx,
                world_z: wz,
                passage_width_cells: min_w,
            });
        }

        // 3. 死胡同口袋估算
        let mut dead_end_count = 0;
        for y in 1..size - 1 {
            for x in 1..size - 1 {
                let idx = y * size + x;
                if blocked[idx] {
                    continue;
                }

                let mut wall_neighbors = 0;
                if blocked[idx - 1] {
                    wall_neighbors += 1;
                }
                if blocked[idx + 1] {
                    wall_neighbors += 1;
                }
                if blocked[idx - size] {
                    wall_neighbors += 1;
                }
                if blocked[idx + size] {
                    wall_neighbors += 1;
                }

                if wall_neighbors == 3 {
                    dead_end_count += 1;
                }
            }
        }

        // 4. 尋路複雜度評分 (0 ~ 100)
        let score_obs = 25.0f32.min(obstacle_percent * 0.5);
        let score_choke = 35.0f32.min(chokepoint_locations.len() as f32 * 1.5);
        let score_conn = 20.0f32.min(0.0f32.max((component_count - 1) as f32) * 4.0);
        let score_dead = 20.0f32.min(dead_end_count as f32 * 1.0);

        let complexity_score = (score_obs + score_choke + score_conn + score_dead).round().clamp(0.0, 100.0) as i32;
        let rating = match complexity_score {
            0..=30 => "Low",
            31..=60 => "Medium",
            61..=80 => "High",
            _ => "Severe",
        }
        .to_string();

        if chokepoint_locations.len() > MapBudgetLimits::REDLINE_CHOKEPOINTS {
            let worst_cp = chokepoint_locations.first();
            let mut b = MapHealthBottleneck::new(
                "Pathfinding",
                "excessive-chokepoints",
                format!("地圖存在大量窄道隘口 ({} 處)，部隊集體行軍時容易發生擠塞與高頻率 A* 重新尋路風暴。", chokepoint_locations.len()),
                format!("Map contains excessive narrow chokepoints ({}), causing crowd congestion and A* re-pathing storms.", chokepoint_locations.len()),
                false,
            );
            if let Some(cp) = worst_cp {
                b = b.with_location(cp.world_x, cp.world_z);
            }
            bottlenecks.push(b);
        }

        if (component_count as usize) > MapBudgetLimits::REDLINE_ISOLATED_REGIONS {
            bottlenecks.push(MapHealthBottleneck::new(
                "Pathfinding",
                "isolated-regions",
                format!("地圖通行區域碎裂為 {component_count} 個獨立連通區塊，部分區域部隊可能永遠無法抵達。"),
                format!("Passable ground is fragmented into {component_count} disconnected regions; units may be unreachable."),
                false,
            ));
        }

        if complexity_score > 80 {
            bottlenecks.push(MapHealthBottleneck::new(
                "Pathfinding",
                "severe-complexity",
                format!("尋路複雜度達嚴重等級 ({complexity_score} / 100)，極易造成遊戲尋路執行緒卡死與幀率暴跌。"),
                format!("Pathfinding complexity is Severe ({complexity_score} / 100), posing a high risk of pathfinder freezing and FPS drops."),
                false,
            ));
        }

        PathfindingBudget {
            obstacle_density_percent: obstacle_percent,
            passable_regions_count: component_count as usize,
            chokepoint_count: chokepoint_locations.len(),
            chokepoints: chokepoint_locations,
            dead_end_count,
            pathfinding_complexity_score: complexity_score,
            complexity_rating: rating,
        }
    }

    fn compute_overall_health(
        object_budget: &ObjectSlotBudget,
        light_budget: &LightDensityBudget,
        draw_budget: &DrawCallBudget,
        path_budget: &PathfindingBudget,
        bottlenecks: &[MapHealthBottleneck],
    ) -> (MapHealthGrade, i32) {
        let has_hard_limit_violation = bottlenecks.iter().any(|b| b.is_engine_limit_violation);
        let mut penalty = 0.0f32;

        if object_budget.total_active_objects > MapBudgetLimits::MAX_OBJECTS_DAT_SLOTS {
            penalty += 60.0;
        } else if object_budget.total_active_objects > MapBudgetLimits::REDLINE_OBJECTS_DAT_SLOTS {
            penalty += 30.0;
        } else if object_budget.total_active_objects > MapBudgetLimits::RECOMMENDED_OBJECTS_DAT_SLOTS {
            penalty += 15.0;
        }

        if light_budget.peak_cluster_lights > MapBudgetLimits::HARD_LIMIT_LIGHTS_PER_CLUSTER {
            penalty += 40.0;
        } else if light_budget.peak_cluster_lights >= MapBudgetLimits::REDLINE_LIGHTS_PER_CLUSTER {
            penalty += 20.0;
        } else if light_budget.peak_cluster_lights > MapBudgetLimits::RECOMMENDED_LIGHTS_PER_CLUSTER {
            penalty += 10.0;
        }

        if draw_budget.estimated_total_draw_calls > 120 {
            penalty += 25.0;
        } else if draw_budget.estimated_total_draw_calls > 60 {
            penalty += 12.0;
        }

        if path_budget.pathfinding_complexity_score > 80 {
            penalty += 30.0;
        } else if path_budget.pathfinding_complexity_score > 60 {
            penalty += 15.0;
        } else if path_budget.pathfinding_complexity_score > 40 {
            penalty += 5.0;
        }

        let overall_score = (100.0 - penalty).round().clamp(0.0, 100.0) as i32;

        let grade = if has_hard_limit_violation {
            MapHealthGrade::EngineExceeded
        } else if overall_score < 45
            || object_budget.total_active_objects > 12_000
            || light_budget.peak_cluster_lights > 58
            || path_budget.pathfinding_complexity_score > 80
        {
            MapHealthGrade::Critical
        } else if overall_score < 70
            || object_budget.total_active_objects > MapBudgetLimits::REDLINE_OBJECTS_DAT_SLOTS
            || light_budget.peak_cluster_lights >= MapBudgetLimits::REDLINE_LIGHTS_PER_CLUSTER
            || path_budget.pathfinding_complexity_score > 60
        {
            MapHealthGrade::Warning
        } else if overall_score < 85
            || object_budget.total_active_objects > MapBudgetLimits::RECOMMENDED_OBJECTS_DAT_SLOTS
            || light_budget.peak_cluster_lights > MapBudgetLimits::RECOMMENDED_LIGHTS_PER_CLUSTER
            || path_budget.pathfinding_complexity_score > 40
        {
            MapHealthGrade::Caution
        } else {
            MapHealthGrade::Healthy
        };

        (grade, overall_score)
    }
}
