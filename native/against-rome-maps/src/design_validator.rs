use against_rome_scenario::model::{Event, Spawn};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 設計檢查問題的嚴重程度。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesignIssueSeverity {
    Warning,
    Error,
}

/// 地圖設計檢查問題項目（包含物件別名、隊伍與座標資訊）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapDesignIssue {
    pub severity: DesignIssueSeverity,
    pub code: String,
    pub chinese: String,
    pub english: String,
    pub object_alias: String,
    pub team: i32,
    pub world_x: Option<f32>,
    pub world_z: Option<f32>,
    pub object_id: Uuid,
}

/// 設計檢查請求模型。
#[derive(Clone, Debug)]
pub struct MapDesignValidationRequest {
    pub objects: Vec<Spawn>,
    pub events: Vec<Event>,
    pub collision: Option<Vec<u8>>,
    pub collision_size: usize,
    pub heights: Option<Vec<u8>>,
    pub height_size: usize,
    pub height_step: f32,
    pub water_level: f32,
    pub map_dimension: f32,
}

impl Default for MapDesignValidationRequest {
    fn default() -> Self {
        Self {
            objects: vec![],
            events: vec![],
            collision: None,
            collision_size: 0,
            heights: None,
            height_size: 0,
            height_step: 4.0,
            water_level: 0.0,
            map_dimension: 16384.0,
        }
    }
}

pub const DEFAULT_MAP_DIMENSION: f32 = 16384.0;

/// 地圖設計層級的可玩性驗證邏輯（純邏輯，無 UI 相依）。
/// 檢查項目包含：
/// 1. 物件是否超出地圖邊界 (0..16383)。
/// 2. 放置物件是否位於水域或通行阻擋格。
/// 3. 各參戰勢力/玩家隊伍是否缺少主堡（Haupthaus）或缺少部隊/單位。
/// 4. 劇本是否缺少勝利或失敗事件。
pub struct MapDesignValidator;

impl MapDesignValidator {
    /// 依雙線性內插計算地形原始高度（0..255），與 TerrainHeightEditSession.SampleHeight / TerrainHeightField.SampleRaw 邏輯一致。
    pub fn sample_bilinear_raw_height(
        heights: &[u8],
        vertex_size: usize,
        norm_x: f32,
        norm_z: f32,
    ) -> f32 {
        if heights.is_empty() || vertex_size < 2 || heights.len() != vertex_size * vertex_size {
            return 0.0;
        }
        let gx = (norm_x * (vertex_size - 1) as f32).clamp(0.0, (vertex_size - 1) as f32);
        let gz = (norm_z * (vertex_size - 1) as f32).clamp(0.0, (vertex_size - 1) as f32);
        let left = (gx as usize).min(vertex_size - 2);
        let top = (gz as usize).min(vertex_size - 2);
        let tx = gx - left as f32;
        let tz = gz - top as f32;
        let a = heights[top * vertex_size + left] as f32;
        let b = heights[top * vertex_size + left + 1] as f32;
        let c = heights[(top + 1) * vertex_size + left] as f32;
        let d = heights[(top + 1) * vertex_size + left + 1] as f32;
        (a + (b - a) * tx) * (1.0 - tz) + (c + (d - c) * tx) * tz
    }

    /// 判斷別名或名稱是否為部族主堡（Haupthaus / Hauptzelt）。
    pub fn is_main_house(alias_or_name: &str) -> bool {
        let s = alias_or_name.trim();
        if s.is_empty() {
            return false;
        }
        let s_upper = s.to_ascii_uppercase();

        // 1. 標準別名：GER_HAU00, ROM_HAU00, KEL_HAU00, HUN_HAU00
        if s_upper.ends_with("_HAU00")
            || s_upper.ends_with("_HAU")
            || s_upper.contains("HAU00")
        {
            return true;
        }

        // 2. 原版德文 namedef：BauGerHau00_Haupthaus, BauRomHau00_Hauptzelt, BauKelHau00, BauHunHau00
        if s_upper.contains("HAUPTHAUS") || s_upper.contains("HAUPTZELT") {
            return true;
        }

        if s_upper.starts_with("BAU") && s_upper.contains("HAU") {
            return true;
        }

        false
    }

    /// 判斷別名或名稱是否為戰鬥部隊或人物單位。
    pub fn is_unit_alias(alias_or_name: &str) -> bool {
        let s = alias_or_name.trim();
        if s.is_empty() {
            return false;
        }
        let s_upper = s.to_ascii_uppercase();

        // 標準兵種別名：步兵 INF、弓兵 ARC、騎兵 CAV/KAV/REI、英雄 HER、矛兵 SPI 等
        if s_upper.contains("_INF")
            || s_upper.contains("_ARC")
            || s_upper.contains("_CAV")
            || s_upper.contains("_KAV")
            || s_upper.contains("_REI")
            || s_upper.contains("_HER")
            || s_upper.contains("_SPI")
        {
            return true;
        }

        // Namedef 前綴判定：Ver...Ico 為部隊圖示，Fig 為人物（排除 FigTie 動物）
        if s_upper.starts_with("VER") && s_upper.contains("ICO") {
            return true;
        }

        if s_upper.starts_with("FIG") && !s_upper.starts_with("FIGTIE") {
            return true;
        }

        false
    }

    /// 執行地圖設計檢查並回傳所有發現的警告與問題。
    pub fn validate(request: &MapDesignValidationRequest) -> Vec<MapDesignIssue> {
        let mut issues = Vec::new();
        // Nothing authored in the editor: bases, troops and victory logic live in the
        // template's native DATA pools and BCI script, which this validator cannot see.
        if request.objects.is_empty() && request.events.is_empty() {
            issues.push(MapDesignIssue {
                severity: DesignIssueSeverity::Warning,
                code: "scenario-native-only".into(),
                chinese: "此地圖沒有編輯器放置的物件或事件；起始部隊、主堡與勝負條件都來自範本的原生資料與腳本，編輯器未檢查。若要自訂劇本，請放置物件並加入勝利／失敗事件。".into(),
                english: "This map has no editor-placed objects or events; starting troops, main houses and victory/defeat come from the template's native data and scripts and were not checked. To author a scenario, place objects and add Victory/Defeat events.".into(),
                object_alias: String::new(),
                team: -1,
                world_x: None,
                world_z: None,
                object_id: Uuid::nil(),
            });
            return issues;
        }
        let dim = if request.map_dimension > 0.0 {
            request.map_dimension
        } else {
            DEFAULT_MAP_DIMENSION
        };

        // --- 1. 物件座標與邊界檢查 ---
        for spawn in &request.objects {
            let out_of_bounds = !spawn.x.is_finite()
                || !spawn.z.is_finite()
                || spawn.x < 0.0
                || spawn.x > dim
                || spawn.z < 0.0
                || spawn.z > dim;

            if out_of_bounds {
                issues.push(MapDesignIssue {
                    severity: DesignIssueSeverity::Warning,
                    code: "bounds".into(),
                    chinese: format!(
                        "物件「{}」（隊伍 {}，座標 X={:.0}, Z={:.0}）超出地圖邊界 (0..{:.0})。",
                        spawn.alias, spawn.team, spawn.x, spawn.z, dim
                    ),
                    english: format!(
                        "Object '{}' (Team {}, Position X={:.0}, Z={:.0}) is outside map bounds (0..{:.0}).",
                        spawn.alias, spawn.team, spawn.x, spawn.z, dim
                    ),
                    object_alias: spawn.alias.clone(),
                    team: spawn.team,
                    world_x: if spawn.x.is_finite() { Some(spawn.x) } else { None },
                    world_z: if spawn.z.is_finite() { Some(spawn.z) } else { None },
                    object_id: spawn.id,
                });
            }
        }

        // --- 2. 地形層檢查：水下與阻擋格判定 ---
        let mut col_size = request.collision_size;
        if col_size == 0 {
            if let Some(col) = &request.collision {
                let sqrt = (col.len() as f32).sqrt().round() as usize;
                if sqrt * sqrt == col.len() {
                    col_size = sqrt;
                }
            }
        }

        let mut height_size = request.height_size;
        if height_size == 0 {
            if let Some(h) = &request.heights {
                let sqrt = (h.len() as f32).sqrt().round() as usize;
                if sqrt * sqrt == h.len() {
                    height_size = sqrt;
                }
            }
        }

        let mut has_collision = request
            .collision
            .as_ref()
            .map_or(false, |c| col_size > 0 && c.len() == col_size * col_size);
        let mut has_heights = request.heights.as_ref().map_or(false, |h| {
            height_size > 1
                && h.len() == height_size * height_size
                && request.height_step.is_finite()
                && request.height_step > 0.0
                && request.water_level.is_finite()
        });

        // 若高度圖與碰撞圖同時存在，兩者尺寸必須相容 (collision = boden - 1 = dimension * 4 對應 dimension * 4 + 1)
        // 若尺寸不一致，優雅略過兩者檢查，避免座標錯位或誤報
        if has_collision && has_heights && col_size != height_size - 1 {
            has_collision = false;
            has_heights = false;
        }

        for spawn in &request.objects {
            if !spawn.x.is_finite()
                || !spawn.z.is_finite()
                || spawn.x < 0.0
                || spawn.x > dim
                || spawn.z < 0.0
                || spawn.z > dim
            {
                continue; // 越界物件由第一條規則回報
            }

            let norm_x = (spawn.x / dim).clamp(0.0, 1.0);
            let norm_z = (spawn.z / dim).clamp(0.0, 1.0);

            // 水下判定（採用雙線性內插）
            if has_heights {
                let raw_height = Self::sample_bilinear_raw_height(
                    request.heights.as_ref().unwrap(),
                    height_size,
                    norm_x,
                    norm_z,
                );
                let ground_height = raw_height * request.height_step;
                if ground_height < request.water_level {
                    issues.push(MapDesignIssue {
                        severity: DesignIssueSeverity::Warning,
                        code: "water".into(),
                        chinese: format!(
                            "物件「{}」（隊伍 {}，座標 X={:.0}, Z={:.0}）位於水下（地面高度 {:.1} 低於水面高度 {:.1}）。",
                            spawn.alias, spawn.team, spawn.x, spawn.z, ground_height, request.water_level
                        ),
                        english: format!(
                            "Object '{}' (Team {}, Position X={:.0}, Z={:.0}) is submerged under water (ground height {:.1} < water level {:.1}).",
                            spawn.alias, spawn.team, spawn.x, spawn.z, ground_height, request.water_level
                        ),
                        object_alias: spawn.alias.clone(),
                        team: spawn.team,
                        world_x: Some(spawn.x),
                        world_z: Some(spawn.z),
                        object_id: spawn.id,
                    });
                }
            }

            // 通行阻擋判定（對應至實際碰撞網格尺寸）
            if has_collision {
                let cx = ((norm_x * col_size as f32) as usize).clamp(0, col_size - 1);
                let cz = ((norm_z * col_size as f32) as usize).clamp(0, col_size - 1);
                let col_val = request.collision.as_ref().unwrap()[cz * col_size + cx];
                if col_val != 0 {
                    issues.push(MapDesignIssue {
                        severity: DesignIssueSeverity::Warning,
                        code: "impassable".into(),
                        chinese: format!(
                            "物件「{}」（隊伍 {}，座標 X={:.0}, Z={:.0}）位於不可通行的阻擋格上（碰撞值 {}）。",
                            spawn.alias, spawn.team, spawn.x, spawn.z, col_val
                        ),
                        english: format!(
                            "Object '{}' (Team {}, Position X={:.0}, Z={:.0}) is placed on an impassable cell (collision value {}).",
                            spawn.alias, spawn.team, spawn.x, spawn.z, col_val
                        ),
                        object_alias: spawn.alias.clone(),
                        team: spawn.team,
                        world_x: Some(spawn.x),
                        world_z: Some(spawn.z),
                        object_id: spawn.id,
                    });
                }
            }
        }

        // --- 3. 參戰勢力/玩家隊伍完整性檢查（主堡與部隊） ---
        let mut candidate_teams = std::collections::BTreeSet::new();
        candidate_teams.insert(0);
        for s in &request.objects {
            if (0..=7).contains(&s.team) {
                candidate_teams.insert(s.team);
            }
        }
        for e in request.events.iter().filter(|ev| ev.enabled) {
            for a in e.actions.iter().filter(|act| act.kind == 2) {
                if (0..=7).contains(&a.team) {
                    candidate_teams.insert(a.team);
                }
            }
        }

        for &team in &candidate_teams {
            let team_objects: Vec<&Spawn> =
                request.objects.iter().filter(|s| s.team == team).collect();
            let loc_x = team_objects.first().map(|s| s.x);
            let loc_z = team_objects.first().map(|s| s.z);
            let loc_zh = match (loc_x, loc_z) {
                (Some(x), Some(z)) => format!("（位於 X={:.0}, Z={:.0} 附近）", x, z),
                _ => String::new(),
            };
            let loc_en = match (loc_x, loc_z) {
                (Some(x), Some(z)) => format!(" (near X={:.0}, Z={:.0})", x, z),
                _ => String::new(),
            };

            // 檢查主堡：只針對玩家隊伍與有放置物件的勢力；僅由事件生成的進攻波次不需要基地
            let has_main_house = team_objects.iter().any(|s| Self::is_main_house(&s.alias));
            let needs_base = team == 0 || !team_objects.is_empty();
            if needs_base && !has_main_house {
                issues.push(MapDesignIssue {
                    severity: DesignIssueSeverity::Warning,
                    code: "team-no-main-house".into(),
                    chinese: format!("隊伍 {team}{loc_zh} 缺少主堡（Haupthaus），無法作為完整勢力基地運作。"),
                    english: format!("Team {team}{loc_en} has no main house (Haupthaus) and cannot function as a complete base."),
                    object_alias: String::new(),
                    team,
                    world_x: loc_x,
                    world_z: loc_z,
                    object_id: Uuid::nil(),
                });
            }

            // 檢查起始部隊/單位
            let has_placed_units = team_objects
                .iter()
                .any(|s| s.count > 0 || Self::is_unit_alias(&s.alias));
            let has_event_units = request.events.iter().filter(|e| e.enabled).any(|e| {
                e.actions
                    .iter()
                    .any(|a| a.kind == 2 && a.team == team && a.count > 0)
            });

            if !has_placed_units && !has_event_units {
                issues.push(MapDesignIssue {
                    severity: DesignIssueSeverity::Warning,
                    code: "team-no-units".into(),
                    chinese: format!("隊伍 {team}{loc_zh} 沒有任何起始部隊或生成單位。"),
                    english: format!("Team {team}{loc_en} has no starting troops or spawned units."),
                    object_alias: String::new(),
                    team,
                    world_x: loc_x,
                    world_z: loc_z,
                    object_id: Uuid::nil(),
                });
            }
        }

        // --- 4. 劇本勝負判定事件檢查 ---
        let enabled_events: Vec<&Event> = request.events.iter().filter(|e| e.enabled).collect();
        let has_victory = enabled_events
            .iter()
            .any(|e| e.actions.iter().any(|a| a.kind == 3));
        let has_defeat = enabled_events
            .iter()
            .any(|e| e.actions.iter().any(|a| a.kind == 4));

        if !has_victory && !has_defeat {
            issues.push(MapDesignIssue {
                severity: DesignIssueSeverity::Warning,
                code: "scenario-no-win-loss".into(),
                chinese: "劇本未設定任何勝利或失敗事件（Scenario 缺少 Victory 與 Defeat 動作），地圖在遊戲中將無法判定勝負或結束任務。".into(),
                english: "Scenario has neither victory nor defeat events (missing Victory and Defeat actions); map cannot evaluate win/loss or complete the mission.".into(),
                object_alias: String::new(),
                team: -1,
                world_x: None,
                world_z: None,
                object_id: Uuid::nil(),
            });
        } else if !has_victory {
            issues.push(MapDesignIssue {
                severity: DesignIssueSeverity::Warning,
                code: "scenario-no-victory".into(),
                chinese: "劇本未設定任何勝利事件（Scenario 缺少 Victory 動作），玩家將無法達成任務目標獲得勝利。".into(),
                english: "Scenario has no victory event (missing Victory action); player cannot achieve mission victory.".into(),
                object_alias: String::new(),
                team: -1,
                world_x: None,
                world_z: None,
                object_id: Uuid::nil(),
            });
        } else if !has_defeat {
            issues.push(MapDesignIssue {
                severity: DesignIssueSeverity::Warning,
                code: "scenario-no-defeat".into(),
                chinese: "劇本未設定任何失敗事件（Scenario 缺少 Defeat 動作），玩家全滅時可能無法正常判定失敗。".into(),
                english: "Scenario has no defeat event (missing Defeat action); mission loss cannot be evaluated when player is defeated.".into(),
                object_alias: String::new(),
                team: -1,
                world_x: None,
                world_z: None,
                object_id: Uuid::nil(),
            });
        }

        issues
    }
}
