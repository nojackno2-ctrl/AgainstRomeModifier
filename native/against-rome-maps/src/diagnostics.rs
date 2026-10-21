use crate::preflight::ScenarioEventValidator;
use against_rome_landscape::cliff::CliffTileCatalog;
use crate::route_check::TRAVEL_PATH_CHOKE;
use against_rome_navigation::{
    analyze_connectivity, ChokePoint, IsolatedRegion, RoadGap, RouteDiagnostic, TileChange,
};
use against_rome_scenario::model::{Event, Spawn};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MapIssueSeverity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapIssue {
    pub severity: MapIssueSeverity,
    pub code: String,
    pub chinese: String,
    pub english: String,
    pub object_id: Uuid,
    pub event_index: i32,
    pub world_x: Option<f32>,
    pub world_z: Option<f32>,
}

impl MapIssue {
    pub fn new(
        severity: MapIssueSeverity,
        code: impl Into<String>,
        chinese: impl Into<String>,
        english: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            chinese: chinese.into(),
            english: english.into(),
            object_id: Uuid::nil(),
            event_index: -1,
            world_x: None,
            world_z: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MapCheckObject {
    pub spawn: Spawn,
    pub is_building: bool,
    pub has_completed_template: bool,
}

impl MapCheckObject {
    pub fn new(spawn: Spawn, is_building: bool, has_completed_template: bool) -> Self {
        Self {
            spawn,
            is_building,
            has_completed_template,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MapCheckSnapshot {
    pub objects: Vec<MapCheckObject>,
    pub events: Vec<Event>,
    pub aliases: Vec<String>,
    pub collision_size: usize,
    pub collision: Option<Vec<u8>>,
    pub height_size: usize,
    pub heights: Option<Vec<u8>>,
    pub height_step: f32,
    pub water_level: f32,
}

impl MapCheckSnapshot {
    pub fn new(
        objects: Vec<MapCheckObject>,
        events: Vec<Event>,
        aliases: Vec<String>,
        collision_size: usize,
        collision: Option<Vec<u8>>,
    ) -> Self {
        Self {
            objects,
            events,
            aliases,
            collision_size,
            collision,
            height_size: 0,
            heights: None,
            height_step: 4.0,
            water_level: 0.0,
        }
    }
}

/// 懸崖陡坡與通行碰撞驗證診斷器。
pub struct CliffDiagnostics;

impl CliffDiagnostics {
    /// 檢查地圖快照中的懸崖與高低差通行規則。
    pub fn check(
        map: &MapCheckSnapshot,
        catalog: Option<&CliffTileCatalog>,
        textures: Option<&[String]>,
        tile_dimension: usize,
    ) -> Vec<MapIssue> {
        let mut issues = Vec::new();
        if map.collision.is_none() || map.collision_size == 0 {
            return issues;
        }

        let collision = map.collision.as_ref().unwrap();
        let collision_size = map.collision_size;
        if collision.len() != collision_size * collision_size {
            return issues;
        }

        let has_heights = map.heights.as_ref().map_or(false, |h| {
            map.height_size > 1 && h.len() == map.height_size * map.height_size
        });

        // 1. 檢查陡坡未標記阻擋 (cliff-steep-unblocked)
        if has_heights && tile_dimension > 0 {
            let height_size = map.height_size;
            let heights = map.heights.as_ref().unwrap();

            for ty in 0..tile_dimension {
                for tx in 0..tile_dimension {
                    let vx = (tx * (height_size - 1) / tile_dimension).clamp(0, height_size - 2);
                    let vy = (ty * (height_size - 1) / tile_dimension).clamp(0, height_size - 2);

                    let h00 = heights[vy * height_size + vx] as i32;
                    let h10 = heights[vy * height_size + (vx + 1)] as i32;
                    let h01 = heights[(vy + 1) * height_size + vx] as i32;
                    let h11 = heights[(vy + 1) * height_size + (vx + 1)] as i32;

                    let max_delta = (h10 - h00)
                        .abs()
                        .max((h01 - h00).abs())
                        .max((h11 - h00).abs().max((h11 - h10).abs()));

                    // 坡度超過 45 度（delta >= 15）
                    if max_delta >= 15 {
                        let cx = (tx * collision_size / tile_dimension).clamp(0, collision_size - 1);
                        let cy = (ty * collision_size / tile_dimension).clamp(0, collision_size - 1);
                        let collision_idx = cy * collision_size + cx;

                        if collision[collision_idx] == 0 {
                            let wx = (tx as f32 + 0.5) * (16384.0 / tile_dimension as f32);
                            let wz = (ty as f32 + 0.5) * (16384.0 / tile_dimension as f32);

                            issues.push(MapIssue {
                                severity: MapIssueSeverity::Warning,
                                code: "cliff-steep-unblocked".into(),
                                chinese: "坡度超過 45 度之陡坡未設定通行阻擋，單位可能異常攀爬。".into(),
                                english: "Steep slope exceeding 45 degrees has no collision blocking; units may climb abnormally.".into(),
                                object_id: Uuid::nil(),
                                event_index: -1,
                                world_x: Some(wx),
                                world_z: Some(wz),
                            });
                        }
                    }
                }
            }
        }

        // 2. 檢查已放置懸崖印章圖塊處缺少通行碰撞 (cliff-missing-collision)
        if let (Some(cat), Some(texs)) = (catalog, textures) {
            if tile_dimension > 0 && texs.len() == tile_dimension * tile_dimension {
                for ty in 0..tile_dimension {
                    for tx in 0..tile_dimension {
                        let texture = &texs[ty * tile_dimension + tx];
                        if cat.is_cliff_texture(texture) {
                            let cx = (tx * collision_size / tile_dimension).clamp(0, collision_size - 1);
                            let cy = (ty * collision_size / tile_dimension).clamp(0, collision_size - 1);
                            let collision_idx = cy * collision_size + cx;

                            if collision[collision_idx] == 0 {
                                let wx = (tx as f32 + 0.5) * (16384.0 / tile_dimension as f32);
                                let wz = (ty as f32 + 0.5) * (16384.0 / tile_dimension as f32);

                                issues.push(MapIssue {
                                    severity: MapIssueSeverity::Warning,
                                    code: "cliff-missing-collision".into(),
                                    chinese: "懸崖岩壁圖塊處未阻擋通行，單位可能穿透垂直岩壁。".into(),
                                    english: "Cliff face tile has no collision blocking; units may walk through sheer cliffs.".into(),
                                    object_id: Uuid::nil(),
                                    event_index: -1,
                                    world_x: Some(wx),
                                    world_z: Some(wz),
                                });
                            }
                        }
                    }
                }
            }
        }

        issues
    }
}

#[derive(Debug)]
pub struct NavMeshAnalysisResult {
    pub isolated_regions: Vec<IsolatedRegion>,
    pub road_gaps: Vec<RoadGap>,
    pub formation_choke_points: Vec<ChokePoint>,
    pub repairs: Vec<Vec<TileChange>>,
    /// Per-route formation diagnostics for the default scenario routes.
    pub routes: Vec<RouteDiagnostic>,
    /// Actionable route findings (blocked endpoint, disconnected, formation choke on a route).
    pub route_issues: Vec<MapIssue>,
}

impl NavMeshAnalysisResult {
    pub fn empty() -> Self {
        Self {
            isolated_regions: vec![],
            road_gaps: vec![],
            formation_choke_points: vec![],
            repairs: vec![],
            routes: vec![],
            route_issues: vec![],
        }
    }
}

/// 編輯器唯讀地圖診斷邏輯（連通性估計採用通行層與水面判定，非遊戲本體完整尋路）。
pub struct MapDiagnostics;

impl MapDiagnostics {
    pub fn valid_point(x: f32, z: f32) -> bool {
        x.is_finite() && z.is_finite() && (0.0..=16383.0).contains(&x) && (0.0..=16383.0).contains(&z)
    }

    pub fn check(map: &MapCheckSnapshot) -> Vec<MapIssue> {
        let mut issues = Vec::new();

        // An empty alias list means the ObjDefName catalog could not be read (map outside a
        // game directory). Report that once instead of calling every script alias unknown;
        // the BCI compiler still refuses script objects without a catalog.
        let catalog_missing = map.aliases.is_empty();
        let needs_catalog = map.objects.iter().any(|o| !o.spawn.prebuilt)
            || map.events.iter().any(|e| e.actions.iter().any(|a| a.kind == 2));
        if catalog_missing && needs_catalog {
            issues.push(MapIssue::new(
                MapIssueSeverity::Error,
                "alias-catalog-missing",
                "找不到物件別名目錄（SYSTEM/CLAK/cl_scint.ini），無法核對或編譯腳本生成的物件與事件部隊。請從遊戲目錄內的 MAPS 開啟地圖（--game 遊戲路徑 --map 代號）。",
                "The object alias catalog (SYSTEM/CLAK/cl_scint.ini) was not found, so script-spawned objects and event troops cannot be verified or compiled. Open the map from MAPS inside a game directory (--game <PATH> --map <ID>).",
            ));
        }

        let mut by_id = HashMap::new();
        for obj in &map.objects {
            let spawn = &obj.spawn;
            // C# `TryAdd`: the first object keeps the id; later duplicates are reported only.
            let duplicate = match by_id.entry(spawn.id) {
                std::collections::hash_map::Entry::Occupied(_) => true,
                std::collections::hash_map::Entry::Vacant(slot) => {
                    slot.insert(obj);
                    false
                }
            };
            if spawn.id.is_nil() || duplicate {
                issues.push(MapIssue {
                    severity: MapIssueSeverity::Error,
                    code: "identity".into(),
                    chinese: "放置物件的持久 ID 缺失或重複。".into(),
                    english: "A placed object has a missing or duplicate persistent ID.".into(),
                    object_id: spawn.id,
                    event_index: -1,
                    world_x: if spawn.x.is_finite() { Some(spawn.x) } else { None },
                    world_z: if spawn.z.is_finite() { Some(spawn.z) } else { None },
                });
            }

            if !Self::valid_point(spawn.x, spawn.z) || !spawn.y.is_finite() {
                issues.push(MapIssue {
                    severity: MapIssueSeverity::Error,
                    code: "coordinates".into(),
                    chinese: "物件座標超出地圖範圍或不是有限數值。".into(),
                    english: "Object coordinates are outside the map or non-finite.".into(),
                    object_id: spawn.id,
                    event_index: -1,
                    world_x: if spawn.x.is_finite() { Some(spawn.x) } else { None },
                    world_z: if spawn.z.is_finite() { Some(spawn.z) } else { None },
                });
            }

            if !spawn.prebuilt
                && !catalog_missing
                && !map
                    .aliases
                    .iter()
                    .any(|a| a.eq_ignore_ascii_case(&spawn.alias))
            {
                issues.push(MapIssue {
                    severity: MapIssueSeverity::Error,
                    code: "alias".into(),
                    chinese: format!("未知物件別名：{}", spawn.alias),
                    english: format!("Unknown object alias: {}", spawn.alias),
                    object_id: spawn.id,
                    event_index: -1,
                    world_x: if spawn.x.is_finite() { Some(spawn.x) } else { None },
                    world_z: if spawn.z.is_finite() { Some(spawn.z) } else { None },
                });
            }

            let tc_valid = (0..=20).contains(&spawn.count)
                && if spawn.count > 0 {
                    (0..=7).contains(&spawn.team)
                } else {
                    (-1..=15).contains(&spawn.team)
                };
            if !tc_valid {
                issues.push(MapIssue {
                    severity: MapIssueSeverity::Error,
                    code: "team-count".into(),
                    chinese: "物件隊伍或人數無效。".into(),
                    english: "Object team or unit count is invalid.".into(),
                    object_id: spawn.id,
                    event_index: -1,
                    world_x: if spawn.x.is_finite() { Some(spawn.x) } else { None },
                    world_z: if spawn.z.is_finite() { Some(spawn.z) } else { None },
                });
            }

            if obj.is_building && !obj.has_completed_template {
                issues.push(MapIssue {
                    severity: MapIssueSeverity::Warning,
                    code: "construction-site".into(),
                    chinese: format!("{} 缺完工範本，將儲存為腳本工地。", spawn.alias),
                    english: format!(
                        "{} has no completed template and will save as a construction site.",
                        spawn.alias
                    ),
                    object_id: spawn.id,
                    event_index: -1,
                    world_x: if spawn.x.is_finite() { Some(spawn.x) } else { None },
                    world_z: if spawn.z.is_finite() { Some(spawn.z) } else { None },
                });
            }
        }

        // 檢查整體事件清單
        if let Err(msg) = ScenarioEventValidator::validate(&map.events, &map.aliases) {
            issues.push(MapIssue {
                severity: MapIssueSeverity::Error,
                code: "event-validation".into(),
                english: format!("Event configuration is invalid; review the event list. ({msg})"),
                chinese: msg,
                object_id: Uuid::nil(),
                event_index: -1,
                world_x: None,
                world_z: None,
            });
        }

        for (index, item) in map.events.iter().enumerate() {
            if !item.enabled {
                continue;
            }
            for condition in &item.conditions {
                if !condition.target_id.is_nil() && !by_id.contains_key(&condition.target_id) {
                    issues.push(MapIssue {
                        severity: MapIssueSeverity::Error,
                        code: "event-target".into(),
                        chinese: format!("事件「{}」的目標已刪除。", item.name),
                        english: format!("Event '{}' references a deleted object.", item.name),
                        object_id: Uuid::nil(),
                        event_index: index as i32,
                        world_x: None,
                        world_z: None,
                    });
                }
            }
        }

        let valid: Vec<&MapCheckObject> = map
            .objects
            .iter()
            .filter(|obj| Self::valid_point(obj.spawn.x, obj.spawn.z))
            .collect();

        // 64 單位重疊檢查
        // Keep first-seen group order (C# GroupBy) so issue order is deterministic.
        let mut group_slot: HashMap<(i32, i32), usize> = HashMap::new();
        let mut grouped: Vec<Vec<&MapCheckObject>> = Vec::new();
        for obj in &valid {
            let key = (
                (obj.spawn.x / 64.0) as i32,
                (obj.spawn.z / 64.0) as i32,
            );
            let slot = *group_slot.entry(key).or_insert_with(|| {
                grouped.push(Vec::new());
                grouped.len() - 1
            });
            grouped[slot].push(obj);
        }
        for group in grouped {
            if group.len() > 1 {
                for obj in group {
                    issues.push(MapIssue {
                        severity: MapIssueSeverity::Warning,
                        code: "overlap".into(),
                        chinese: "多個放置物件錨點位於同一個 64 單位區域，請檢查重疊。".into(),
                        english: "Multiple placement anchors share a 64-unit area; inspect for overlap.".into(),
                        object_id: obj.spawn.id,
                        event_index: -1,
                        world_x: Some(obj.spawn.x),
                        world_z: Some(obj.spawn.z),
                    });
                }
            }
        }

        let starts: Vec<&MapCheckObject> = valid
            .iter()
            .copied()
            .filter(|obj| obj.spawn.team == 0 && obj.spawn.count > 0)
            .collect();

        // With no editor-placed objects, starts come from native template data (events alone do
        // not place a start troop); `MapDesignValidator` explains a fully untouched template.
        let native_only = map.objects.is_empty();
        if starts.is_empty() && !native_only {
            issues.push(MapIssue {
                severity: MapIssueSeverity::Warning,
                code: "no-start".into(),
                chinese: "未找到編輯器放置的玩家起始部隊；範本腳本可能另有生成。".into(),
                english: "No editor-placed player starting troop was found; template scripts may spawn one.".into(),
                object_id: Uuid::nil(),
                event_index: -1,
                world_x: None,
                world_z: None,
            });
        }

        if map.collision.is_none()
            || map.collision_size == 0
            || map.collision.as_ref().unwrap().len() != map.collision_size * map.collision_size
        {
            issues.push(MapIssue {
                severity: MapIssueSeverity::Warning,
                code: "no-collision".into(),
                chinese: "缺少有效通行層，無法檢查阻擋與連通性。".into(),
                english: "No valid collision layer is available for blockage/connectivity checks.".into(),
                object_id: Uuid::nil(),
                event_index: -1,
                world_x: None,
                world_z: None,
            });
            return issues;
        }

        let size = map.collision_size;
        if size > 1024 {
            issues.push(MapIssue {
                severity: MapIssueSeverity::Error,
                code: "collision-limit".into(),
                chinese: "通行網格超出診斷限制 (最大 1024)。".into(),
                english: "Collision grid exceeds diagnostic limits (max 1024).".into(),
                object_id: Uuid::nil(),
                event_index: -1,
                world_x: None,
                world_z: None,
            });
            return issues;
        }

        let has_height = map.heights.is_some()
            && map.height_size > 1
            && map.heights.as_ref().unwrap().len() == map.height_size * map.height_size
            && map.height_step.is_finite()
            && map.height_step > 0.0
            && map.water_level.is_finite();

        if !has_height {
            issues.push(MapIssue {
                severity: MapIssueSeverity::Warning,
                code: "no-height".into(),
                chinese: "缺少有效高度資料，連通估計只考慮通行層。".into(),
                english: "No valid height data is available; connectivity considers collision only.".into(),
                object_id: Uuid::nil(),
                event_index: -1,
                world_x: None,
                world_z: None,
            });
        }

        let collision = map.collision.as_ref().unwrap();
        let mut components = vec![-1i32; size * size];

        for y in 0..size {
            for x in 0..size {
                let mut submerged = false;
                if has_height {
                    let heights = map.heights.as_ref().unwrap();
                    let h_size = map.height_size;
                    let hx = (((x as f32 + 0.5) * (h_size - 1) as f32 / size as f32) as usize)
                        .min(h_size - 1);
                    let hy = (((y as f32 + 0.5) * (h_size - 1) as f32 / size as f32) as usize)
                        .min(h_size - 1);
                    submerged = heights[hy * h_size + hx] as f32 * map.height_step < map.water_level;
                }
                if collision[y * size + x] != 0 || submerged {
                    components[y * size + x] = -2;
                }
            }
        }

        let mut queue = vec![0usize; components.len()];
        let mut component = 0i32;
        for origin in 0..components.len() {
            if components[origin] != -1 {
                continue;
            }
            let mut head = 0usize;
            let mut tail = 0usize;
            queue[tail] = origin;
            tail += 1;
            components[origin] = component;

            while head < tail {
                let cell = queue[head];
                head += 1;
                let x = cell % size;
                let y = cell / size;

                if x > 0 {
                    let next = cell - 1;
                    if components[next] == -1 {
                        components[next] = component;
                        queue[tail] = next;
                        tail += 1;
                    }
                }
                if x + 1 < size {
                    let next = cell + 1;
                    if components[next] == -1 {
                        components[next] = component;
                        queue[tail] = next;
                        tail += 1;
                    }
                }
                if y > 0 {
                    let next = cell - size;
                    if components[next] == -1 {
                        components[next] = component;
                        queue[tail] = next;
                        tail += 1;
                    }
                }
                if y + 1 < size {
                    let next = cell + size;
                    if components[next] == -1 {
                        components[next] = component;
                        queue[tail] = next;
                        tail += 1;
                    }
                }
            }
            component += 1;
        }

        let cell_index = |x: f32, z: f32| -> usize {
            let cx = ((x / 16384.0 * size as f32) as usize).clamp(0, size - 1);
            let cz = ((z / 16384.0 * size as f32) as usize).clamp(0, size - 1);
            cz * size + cx
        };

        let mut reachable = HashSet::new();
        for start in &starts {
            let region = components[cell_index(start.spawn.x, start.spawn.z)];
            if region >= 0 {
                reachable.insert(region);
            } else {
                issues.push(MapIssue {
                    severity: MapIssueSeverity::Warning,
                    code: "blocked-start".into(),
                    chinese: "玩家起始部隊錨點位於阻擋區或水面下。".into(),
                    english: "A player starting troop anchor lies in blocked or submerged ground.".into(),
                    object_id: start.spawn.id,
                    event_index: -1,
                    world_x: Some(start.spawn.x),
                    world_z: Some(start.spawn.z),
                });
            }
        }

        for obj in valid.iter().filter(|obj| obj.is_building) {
            let region = components[cell_index(obj.spawn.x, obj.spawn.z)];
            if region < 0 {
                issues.push(MapIssue {
                    severity: MapIssueSeverity::Warning,
                    code: "blocked-building".into(),
                    chinese: "建築錨點位於阻擋區或水面下，請檢查放置位置。".into(),
                    english: "A building anchor lies in blocked or submerged ground; inspect placement.".into(),
                    object_id: obj.spawn.id,
                    event_index: -1,
                    world_x: Some(obj.spawn.x),
                    world_z: Some(obj.spawn.z),
                });
            } else if !reachable.is_empty() && !reachable.contains(&region) {
                issues.push(MapIssue {
                    severity: MapIssueSeverity::Warning,
                    code: "isolated-building".into(),
                    chinese: "建築所在通行區與玩家起始部隊不連通（估計）。".into(),
                    english: "A building's passable region is disconnected from player starting troops (estimate).".into(),
                    object_id: obj.spawn.id,
                    event_index: -1,
                    world_x: Some(obj.spawn.x),
                    world_z: Some(obj.spawn.z),
                });
            }
        }

        for (index, item) in map.events.iter().enumerate() {
            if !item.enabled {
                continue;
            }
            for area in item.conditions.iter().filter(|c| c.kind == 2) {
                if let Some(target) = by_id.get(&area.target_id) {
                    if !Self::valid_point(target.spawn.x, target.spawn.z)
                        || area.min_x < 0
                        || area.min_z < 0
                        || area.max_x > 16383
                        || area.max_z > 16383
                        || area.min_x > area.max_x
                        || area.min_z > area.max_z
                    {
                        continue;
                    }

                    let target_region = components[cell_index(target.spawn.x, target.spawn.z)];
                    if target_region < 0 {
                        continue;
                    }

                    let min_cx = ((area.min_x as f32 / 16384.0 * size as f32) as usize).clamp(0, size - 1);
                    let min_cz = ((area.min_z as f32 / 16384.0 * size as f32) as usize).clamp(0, size - 1);
                    let max_cx = ((area.max_x as f32 / 16384.0 * size as f32) as usize).clamp(0, size - 1);
                    let max_cz = ((area.max_z as f32 / 16384.0 * size as f32) as usize).clamp(0, size - 1);

                    let mut accessible = false;
                    'outer: for y in min_cz..=max_cz {
                        for x in min_cx..=max_cx {
                            if components[y * size + x] == target_region {
                                accessible = true;
                                break 'outer;
                            }
                        }
                    }

                    if !accessible {
                        issues.push(MapIssue {
                            severity: MapIssueSeverity::Warning,
                            code: "isolated-area".into(),
                            chinese: format!("事件「{}」的區域與目標物件不連通（估計）。", item.name),
                            english: format!("Event '{}' has an area disconnected from its target (estimate).", item.name),
                            object_id: target.spawn.id,
                            event_index: index as i32,
                            world_x: Some((area.min_x + area.max_x) as f32 / 2.0),
                            world_z: Some((area.min_z + area.max_z) as f32 / 2.0),
                        });
                    }
                }
            }
        }

        issues.extend(Self::route_coverage_issues(map));

        issues
    }

    /// 執行進階 NavMesh 連通性、道路間隙與部隊方陣通道評估，產生結構化分析報告與修復處方。
    pub fn analyze_nav_mesh(
        map: &MapCheckSnapshot,
        textures: Option<&[String]>,
        min_isolated_size: usize,
    ) -> NavMeshAnalysisResult {
        if map.collision.is_none()
            || map.collision_size == 0
            || map.collision.as_ref().unwrap().len() != map.collision_size * map.collision_size
        {
            return NavMeshAnalysisResult::empty();
        }

        let grid = match Self::passability_grid(map) {
            Some(g) => g,
            None => return NavMeshAnalysisResult::empty(),
        };

        let primary_seeds: Vec<(i32, i32)> = map
            .objects
            .iter()
            .filter(|o| o.spawn.team == 0 && (o.spawn.count > 0 || o.is_building))
            .filter(|o| Self::valid_point(o.spawn.x, o.spawn.z))
            .map(|o| grid.world_to_tile(o.spawn.x, o.spawn.z))
            .collect();

        let interest_points: Vec<(i32, i32)> = map
            .objects
            .iter()
            .filter(|o| Self::valid_point(o.spawn.x, o.spawn.z))
            .map(|o| grid.world_to_tile(o.spawn.x, o.spawn.z))
            .collect();

        let isolated = analyze_connectivity(
            &grid,
            &primary_seeds,
            &interest_points,
            min_isolated_size.max(1),
        );

        let tex_dimension = if let Some(tex) = textures {
            if !tex.is_empty() {
                let sqrt = (tex.len() as f32).sqrt().round() as usize;
                if sqrt * sqrt == tex.len() {
                    sqrt
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        let gaps = if tex_dimension > 0 && textures.is_some() {
            against_rome_navigation::detect_road_gaps(
                tex_dimension,
                textures.unwrap(),
                Some(&grid),
                4,
            )
            .unwrap_or_default()
        } else {
            vec![]
        };

        let mut repairs = against_rome_navigation::road_repairs(&gaps);

        for region in isolated.iter().filter(|r| r.bridge.is_some()) {
            if let Some(bridge) = against_rome_navigation::bridge_repair(
                region,
                "weg1",
                if tex_dimension > 0 { tex_dimension } else { 64 },
            ) {
                repairs.push(vec![bridge]);
            }
        }

        let requests = Self::scenario_route_requests(map);
        let squad_max = map
            .objects
            .iter()
            .filter(|o| o.spawn.count > 1 && Self::valid_point(o.spawn.x, o.spawn.z))
            .map(|o| o.spawn.count)
            .max()
            .unwrap_or(0);
        let report = Self::check_routes_on_grid(&grid, &requests, squad_max);
        let chokes = report.choke_points();

        NavMeshAnalysisResult {
            isolated_regions: isolated,
            road_gaps: gaps,
            formation_choke_points: chokes,
            repairs,
            routes: report.formation.routes,
            route_issues: report.route_issues,
        }
    }

    /// 將 NavMesh 分析報告轉為標準 MapIssue 項目，供地圖檢查面板顯示並支援座標定位。
    pub fn convert_to_issues(nav_result: &NavMeshAnalysisResult) -> Vec<MapIssue> {
        let mut issues = Vec::new();

        for region in &nav_result.isolated_regions {
            let contains_extra_zh = if region.contains_interest {
                " 區域內含有物件或建築！"
            } else {
                ""
            };
            let contains_extra_en = if region.contains_interest {
                " Area contains placed objects or buildings!"
            } else {
                ""
            };
            issues.push(MapIssue {
                severity: MapIssueSeverity::Warning,
                code: "isolated-land".into(),
                chinese: format!(
                    "發現孤立無法通達的陸地區域（共 {} 格），部隊無法前往。{}",
                    region.tile_count, contains_extra_zh
                ),
                english: format!(
                    "Isolated land region detected ({} tiles); troops cannot reach this area.{}",
                    region.tile_count, contains_extra_en
                ),
                object_id: Uuid::nil(),
                event_index: -1,
                world_x: Some(region.centroid.world_x),
                world_z: Some(region.centroid.world_z),
            });
        }

        for gap in &nav_result.road_gaps {
            let mid = gap.tiles.first().unwrap_or(&gap.start);
            issues.push(MapIssue {
                severity: MapIssueSeverity::Warning,
                code: "road-gap".into(),
                chinese: format!(
                    "偵測到道路中斷（間距 {} 格，{} 風格），建議補齊圖塊「{}」。",
                    gap.tiles.len(),
                    gap.style,
                    gap.texture
                ),
                english: format!(
                    "Road gap detected ({} tiles, {} style); recommend placing '{}'.",
                    gap.tiles.len(),
                    gap.style,
                    gap.texture
                ),
                object_id: Uuid::nil(),
                event_index: -1,
                world_x: Some(mid.world_x),
                world_z: Some(mid.world_z),
            });
        }

        // Corridor chokes are reported with route context in `route_issues`.
        for choke in nav_result
            .formation_choke_points
            .iter()
            .filter(|c| c.kind != TRAVEL_PATH_CHOKE)
        {
            issues.push(crate::route_check::bottleneck_issue(choke));
        }
        issues.extend(nav_result.route_issues.iter().cloned());

        issues
    }
}
