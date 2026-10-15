use crate::contract::is_payload_path;
use against_rome_scenario::model::ScenarioDocument;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreflightSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreflightCategory {
    StructuralBinary,
    Diagnostics,
    Playability,
    Sanitization,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapPreflightIssue {
    pub severity: PreflightSeverity,
    pub category: PreflightCategory,
    pub code: String,
    pub chinese: String,
    pub english: String,
    pub object_id: Uuid,
    pub event_index: i32,
    pub world_x: Option<f32>,
    pub world_z: Option<f32>,
}

impl MapPreflightIssue {
    pub fn new(
        severity: PreflightSeverity,
        category: PreflightCategory,
        code: impl Into<String>,
        chinese: impl Into<String>,
        english: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            category,
            code: code.into(),
            chinese: chinese.into(),
            english: english.into(),
            object_id: Uuid::nil(),
            event_index: -1,
            world_x: None,
            world_z: None,
        }
    }

    pub fn with_location(mut self, world_x: f32, world_z: f32) -> Self {
        self.world_x = Some(world_x);
        self.world_z = Some(world_z);
        self
    }

    pub fn with_object_id(mut self, id: Uuid) -> Self {
        self.object_id = id;
        self
    }
}

#[derive(Clone, Debug, Default)]
pub struct MapPreflightReport {
    pub issues: Vec<MapPreflightIssue>,
    pub files_to_package: Vec<String>,
    pub files_to_exclude: Vec<String>,
}

impl MapPreflightReport {
    pub fn can_export(&self) -> bool {
        self.issues.iter().all(|i| i.severity != PreflightSeverity::Error)
    }

    pub fn error_count(&self) -> usize {
        self.issues.iter().filter(|i| i.severity == PreflightSeverity::Error).count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues.iter().filter(|i| i.severity == PreflightSeverity::Warning).count()
    }

    pub fn info_count(&self) -> usize {
        self.issues.iter().filter(|i| i.severity == PreflightSeverity::Info).count()
    }
}

pub struct MapExportPreflightChecker;

const HEIGHT_DEPENDENT_CACHES: &[&str] = &[
    "skydens.dat",
    "visible.dat",
    "cliprect.dat",
    "shadows.dat",
];

impl MapExportPreflightChecker {
    pub fn check(
        map_directory: &Path,
        scenario: Option<&ScenarioDocument>,
        known_aliases: Option<&[String]>,
    ) -> std::io::Result<MapPreflightReport> {
        if !map_directory.exists() || !map_directory.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("地圖目錄不存在: {}", map_directory.display()),
            ));
        }

        let mut report = MapPreflightReport::default();

        // 階段一：檔案結構與二進位體檢
        Self::check_file_structure(map_directory, &mut report);

        // 階段二：語意與幾何診斷
        Self::check_semantic_diagnostics(map_directory, scenario, known_aliases, &mut report);

        // 階段三：可玩性與開局完整性
        Self::check_playability(map_directory, scenario, &mut report);

        // 階段四：封包衛生性排查
        Self::audit_package_sanitization(map_directory, &mut report);

        Ok(report)
    }

    fn check_file_structure(map_directory: &Path, report: &mut MapPreflightReport) {
        let boden_bmp = map_directory.join("boden.bmp");
        if !boden_bmp.exists() {
            report.issues.push(MapPreflightIssue::new(
                PreflightSeverity::Error,
                PreflightCategory::StructuralBinary,
                "missing-boden",
                "缺少地圖核心高度檔案 boden.bmp。",
                "Missing core terrain height file boden.bmp.",
            ));
        } else {
            Self::validate_bmp_header(&boden_bmp, 257, 257, "boden.bmp", report);
        }

        let collision_bmp = map_directory.join("collision.bmp");
        if !collision_bmp.exists() {
            report.issues.push(MapPreflightIssue::new(
                PreflightSeverity::Error,
                PreflightCategory::StructuralBinary,
                "missing-collision",
                "缺少通行阻擋層 collision.bmp。",
                "Missing collision layer collision.bmp.",
            ));
        } else {
            Self::validate_bmp_header(&collision_bmp, 256, 256, "collision.bmp", report);
        }

        let boden_ini = map_directory.join("boden.ini");
        if !boden_ini.exists() {
            report.issues.push(MapPreflightIssue::new(
                PreflightSeverity::Error,
                PreflightCategory::StructuralBinary,
                "missing-boden-ini",
                "缺少地形設定檔案 boden.ini。",
                "Missing terrain configuration boden.ini.",
            ));
        }

        let us_briefing = map_directory.join("TEXT").join("US").join("briefing.put");
        if !us_briefing.exists() {
            report.issues.push(MapPreflightIssue::new(
                PreflightSeverity::Error,
                PreflightCategory::StructuralBinary,
                "missing-briefing",
                "缺少主要語系簡報檔 TEXT/US/briefing.put。",
                "Missing main localization file TEXT/US/briefing.put.",
            ));
        }

        let objects_dat = map_directory.join("DATA").join("objects.dat");
        if !objects_dat.exists() {
            report.issues.push(MapPreflightIssue::new(
                PreflightSeverity::Error,
                PreflightCategory::StructuralBinary,
                "missing-objects-dat",
                "缺少世界物件定義檔 DATA/objects.dat。",
                "Missing world objects definition DATA/objects.dat.",
            ));
        }

        let level_bci = map_directory.join("SCRIPT").join("ak_level.bci");
        if !level_bci.exists() {
            report.issues.push(MapPreflightIssue::new(
                PreflightSeverity::Error,
                PreflightCategory::StructuralBinary,
                "missing-ak-level",
                "缺少關卡主腳本 SCRIPT/ak_level.bci。",
                "Missing level script SCRIPT/ak_level.bci.",
            ));
        }

        let has_sdl = if let Ok(entries) = fs::read_dir(map_directory) {
            entries.flatten().any(|e| {
                e.path().is_file()
                    && e.path()
                        .extension()
                        .map_or(false, |ext| ext.eq_ignore_ascii_case("sdl"))
            })
        } else {
            false
        };

        if !has_sdl {
            report.issues.push(MapPreflightIssue::new(
                PreflightSeverity::Error,
                PreflightCategory::StructuralBinary,
                "missing-sdl",
                "地圖目錄未找到任何聚落或場景 SDL 檔案。",
                "No settlement or scene SDL files found.",
            ));
        }
    }

    fn validate_bmp_header(
        path: &Path,
        expected_width: i32,
        expected_height: i32,
        label: &str,
        report: &mut MapPreflightReport,
    ) {
        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(e) => {
                report.issues.push(MapPreflightIssue::new(
                    PreflightSeverity::Error,
                    PreflightCategory::StructuralBinary,
                    "bmp-read-error",
                    format!("讀取 {label} 失敗: {e}"),
                    format!("Failed reading {label}: {e}"),
                ));
                return;
            }
        };

        let len = file.metadata().map(|m| m.len()).unwrap_or(0);
        if len < 54 {
            report.issues.push(MapPreflightIssue::new(
                PreflightSeverity::Error,
                PreflightCategory::StructuralBinary,
                "corrupt-bmp-header",
                format!("{label} 檔案長度小於 54 位元組，BMP 標頭損壞。"),
                format!("{label} file length is less than 54 bytes; BMP header corrupt."),
            ));
            return;
        }

        let mut header = [0u8; 54];
        if file.read_exact(&mut header).is_err() {
            report.issues.push(MapPreflightIssue::new(
                PreflightSeverity::Error,
                PreflightCategory::StructuralBinary,
                "bmp-read-error",
                format!("讀取 {label} 標頭失敗。"),
                format!("Failed reading {label} header."),
            ));
            return;
        }

        if header[0] != b'B' || header[1] != b'M' {
            report.issues.push(MapPreflightIssue::new(
                PreflightSeverity::Error,
                PreflightCategory::StructuralBinary,
                "invalid-bmp-magic",
                format!("{label} 不是合法的 BMP 格式（缺少 BM 標記）。"),
                format!("{label} is not a valid BMP format (missing BM signature)."),
            ));
            return;
        }

        let width = i32::from_le_bytes(header[18..22].try_into().unwrap());
        let height = i32::from_le_bytes(header[22..26].try_into().unwrap());

        if width != expected_width || height != expected_height {
            report.issues.push(MapPreflightIssue::new(
                PreflightSeverity::Error,
                PreflightCategory::StructuralBinary,
                "invalid-bmp-dimensions",
                format!("{label} 尺寸為 {width}x{height}，與標準要求 {expected_width}x{expected_height} 不符。"),
                format!("{label} dimensions are {width}x{height}; expected {expected_width}x{expected_height}."),
            ));
        }
    }

    fn check_semantic_diagnostics(
        map_directory: &Path,
        passed_scenario: Option<&ScenarioDocument>,
        known_aliases: Option<&[String]>,
        report: &mut MapPreflightReport,
    ) {
        let loaded_scenario: Option<ScenarioDocument> = if passed_scenario.is_none() {
            let sc_path = map_directory.join("arm_scenario.json");
            if sc_path.exists() {
                match fs::read_to_string(&sc_path) {
                    Ok(text) => match serde_json::from_str::<ScenarioDocument>(&text) {
                        Ok(sc) => Some(sc),
                        Err(e) => {
                            report.issues.push(MapPreflightIssue::new(
                                PreflightSeverity::Error,
                                PreflightCategory::Diagnostics,
                                "corrupted-scenario-json",
                                format!("無法解析場景設定檔 arm_scenario.json: {e}"),
                                format!("Corrupted arm_scenario.json: {e}"),
                            ));
                            None
                        }
                    },
                    Err(e) => {
                        report.issues.push(MapPreflightIssue::new(
                            PreflightSeverity::Error,
                            PreflightCategory::Diagnostics,
                            "corrupted-scenario-json",
                            format!("無法讀取 arm_scenario.json: {e}"),
                            format!("Cannot read arm_scenario.json: {e}"),
                        ));
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        let scenario = passed_scenario.or(loaded_scenario.as_ref());

        let collision = Self::read_bmp_green_channel(&map_directory.join("collision.bmp"), 256, 256);
        let heights = Self::read_bmp_green_channel(&map_directory.join("boden.bmp"), 257, 257);

        let mut water_level = 0.0f32;
        let boden_ini = map_directory.join("boden.ini");
        if boden_ini.exists() {
            if let Ok(content) = fs::read_to_string(&boden_ini) {
                for line in content.lines() {
                    if let Some((k, v)) = line.split_once('=') {
                        if k.trim().eq_ignore_ascii_case("Waterlevel") {
                            if let Ok(parsed) = v.trim().parse::<f32>() {
                                water_level = parsed;
                            }
                        }
                    }
                }
            }
        }

        if let Some(sc) = scenario {
            for spawn in &sc.spawns {
                let is_building = spawn.alias.to_ascii_lowercase().starts_with("bau");

                // Alias check
                if let Some(catalog) = known_aliases {
                    let found = catalog.iter().any(|a| a.eq_ignore_ascii_case(&spawn.alias));
                    if !found {
                        report.issues.push(
                            MapPreflightIssue::new(
                                PreflightSeverity::Error,
                                PreflightCategory::Diagnostics,
                                "alias",
                                format!("未知的物件別名: {}", spawn.alias),
                                format!("Unknown object alias: {}", spawn.alias),
                            )
                            .with_location(spawn.x, spawn.z)
                            .with_object_id(spawn.id),
                        );
                    }
                } else if !spawn.alias.trim().is_empty() {
                    report.issues.push(
                        MapPreflightIssue::new(
                            PreflightSeverity::Warning,
                            PreflightCategory::Diagnostics,
                            "alias",
                            format!("未核實的物件別名（缺素材目錄）: {}", spawn.alias),
                            format!("Unverified object alias (missing catalog): {}", spawn.alias),
                        )
                        .with_location(spawn.x, spawn.z)
                        .with_object_id(spawn.id),
                    );
                }

                // Coordinates check
                if spawn.x < 0.0 || spawn.x > 16384.0 || spawn.z < 0.0 || spawn.z > 16384.0 {
                    report.issues.push(
                        MapPreflightIssue::new(
                            PreflightSeverity::Error,
                            PreflightCategory::Diagnostics,
                            "coordinates",
                            format!("物件坐標越界: ({}, {})", spawn.x, spawn.z),
                            format!("Object coordinates out of bounds: ({}, {})", spawn.x, spawn.z),
                        )
                        .with_location(spawn.x, spawn.z)
                        .with_object_id(spawn.id),
                    );
                }

                // Team count check
                if spawn.team < 0 || spawn.team > 7 {
                    report.issues.push(
                        MapPreflightIssue::new(
                            PreflightSeverity::Error,
                            PreflightCategory::Diagnostics,
                            "team-count",
                            format!("無效的隊伍編號: {}", spawn.team),
                            format!("Invalid team number: {}", spawn.team),
                        )
                        .with_location(spawn.x, spawn.z)
                        .with_object_id(spawn.id),
                    );
                }

                // Blocked check
                if let Some(ref col) = collision {
                    let cx = ((spawn.x / 64.0) as usize).min(255);
                    let cz = ((spawn.z / 64.0) as usize).min(255);
                    let is_blocked = col[cz * 256 + cx] != 0;

                    let mut submerged = false;
                    if let Some(ref hts) = heights {
                        let hx = ((spawn.x / 16384.0 * 256.0) as usize).min(256);
                        let hz = ((spawn.z / 16384.0 * 256.0) as usize).min(256);
                        let elev = hts[hz * 257 + hx] as f32 * 4.0;
                        if elev < water_level {
                            submerged = true;
                        }
                    }

                    if is_blocked || submerged {
                        let code = if is_building {
                            "blocked-building"
                        } else {
                            "blocked-start"
                        };
                        report.issues.push(
                            MapPreflightIssue::new(
                                PreflightSeverity::Error,
                                PreflightCategory::Diagnostics,
                                code,
                                format!("單位或建築落於障礙或水下: {}", spawn.alias),
                                format!("Unit or building located on obstacle or water: {}", spawn.alias),
                            )
                            .with_location(spawn.x, spawn.z)
                            .with_object_id(spawn.id),
                        );
                    }
                }
            }

            // Event validation
            for (evt_idx, event) in sc.events.iter().enumerate() {
                if event.actions.is_empty() && event.conditions.is_empty() {
                    report.issues.push(MapPreflightIssue {
                        severity: PreflightSeverity::Error,
                        category: PreflightCategory::Diagnostics,
                        code: "event-validation".to_string(),
                        chinese: format!("事件 {} 缺乏條件與動作", event.name),
                        english: format!("Event {} lacks conditions and actions", event.name),
                        object_id: Uuid::nil(),
                        event_index: evt_idx as i32,
                        world_x: None,
                        world_z: None,
                    });
                }
            }
        }
    }

    fn check_playability(
        map_directory: &Path,
        scenario: Option<&ScenarioDocument>,
        report: &mut MapPreflightReport,
    ) {
        let mut has_player_start = false;

        if let Some(sc) = scenario {
            has_player_start = sc.spawns.iter().any(|s| {
                s.team == 0 && (s.count > 0 || s.alias.to_ascii_lowercase().starts_with("bau"))
            });
        }

        if !has_player_start {
            // Check SDL files for team = 0
            if let Ok(entries) = fs::read_dir(map_directory) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file()
                        && path
                            .extension()
                            .map_or(false, |ext| ext.eq_ignore_ascii_case("sdl"))
                    {
                        if let Ok(content) = fs::read_to_string(&path) {
                            for line in content.lines() {
                                let trimmed = line.trim();
                                if let Some((k, v)) = trimmed.split_once('=') {
                                    if k.trim().eq_ignore_ascii_case("team") && v.trim() == "0" {
                                        has_player_start = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    if has_player_start {
                        break;
                    }
                }
            }
        }

        if !has_player_start {
            report.issues.push(MapPreflightIssue::new(
                PreflightSeverity::Warning,
                PreflightCategory::Playability,
                "no-player-presence",
                "地圖未檢測到 Team 0（人類玩家）的起始部隊或聚落主基地；玩家可能在進入地圖時無單位可操作。",
                "No starting unit or town hall detected for Team 0 (human player); player may have no units to control.",
            ));
        }

        if let Some(sc) = scenario {
            if !sc.events.is_empty() {
                // Kind 3 = Victory, Kind 4 = Defeat
                let has_terminal = sc.events.iter().filter(|e| e.enabled).any(|e| {
                    e.actions.iter().any(|a| a.kind == 3 || a.kind == 4)
                });

                if !has_terminal {
                    report.issues.push(MapPreflightIssue::new(
                        PreflightSeverity::Warning,
                        PreflightCategory::Playability,
                        "no-terminal-event",
                        "地圖設定了腳本事件，但未包含任何勝利 (Victory) 或失敗 (Defeat) 結算行動；戰役可能無法正常通關結算。",
                        "Scenario events are defined but lack Victory or Defeat terminal actions; scenario might never conclude.",
                    ));
                }
            }
        }
    }

    fn audit_package_sanitization(map_directory: &Path, report: &mut MapPreflightReport) {
        let mut collected = Vec::new();
        let _ = collect_files(map_directory, map_directory, &mut collected);

        let cache_set: HashSet<&str> = HEIGHT_DEPENDENT_CACHES.iter().copied().collect();

        for (rel, _) in collected {
            let normalized = rel.replace('\\', "/");
            let file_name = normalized.split('/').last().unwrap_or(&normalized).to_string();
            let lower_name = file_name.to_ascii_lowercase();

            if cache_set.contains(lower_name.as_str()) {
                report.files_to_exclude.push(normalized);
                report.issues.push(MapPreflightIssue::new(
                    PreflightSeverity::Info,
                    PreflightCategory::Sanitization,
                    "height-cache-excluded",
                    format!("偵測到引擎動態重算快取檔 {file_name}；已標記為排除，減少模組包體積並防止快取錯亂。"),
                    format!("Detected engine runtime cache {file_name}; marked for exclusion to reduce size and prevent cache mismatch."),
                ));
                continue;
            }

            if lower_name.ends_with(".tmp_arm")
                || lower_name.ends_with(".bak")
                || lower_name.ends_with(".log")
                || lower_name == "thumbs.db"
                || lower_name == ".ds_store"
                || lower_name == "sdl_placed_objects.put"
            {
                report.files_to_exclude.push(normalized.clone());
                report.issues.push(MapPreflightIssue::new(
                    PreflightSeverity::Info,
                    PreflightCategory::Sanitization,
                    "temporary-file-excluded",
                    format!("偵測到本地暫存或無用檔案 {normalized}；發布時將自動排除。"),
                    format!("Detected temporary/junk file {normalized}; automatically excluded from package."),
                ));
                continue;
            }

            if !is_payload_path(&normalized) {
                report.files_to_exclude.push(normalized.clone());
                report.issues.push(MapPreflightIssue::new(
                    PreflightSeverity::Info,
                    PreflightCategory::Sanitization,
                    "non-payload-excluded",
                    format!("排除槽位外素材、marker 或不支援檔案：{normalized}"),
                    format!("Excluded non-payload file: {normalized}"),
                ));
                continue;
            }

            report.files_to_package.push(normalized);
        }
    }

    pub fn read_bmp_green_channel(path: &Path, expected_width: usize, expected_height: usize) -> Option<Vec<u8>> {
        let mut file = File::open(path).ok()?;
        let mut header = [0u8; 54];
        file.read_exact(&mut header).ok()?;

        if header[0] != b'B' || header[1] != b'M' {
            return None;
        }

        let width = i32::from_le_bytes(header[18..22].try_into().ok()?) as usize;
        let height = i32::from_le_bytes(header[22..26].try_into().ok()?) as usize;
        if width != expected_width || height != expected_height {
            return None;
        }

        let stride = (width * 3 + 3) & !3;
        let mut green = vec![0u8; width * height];
        let mut row = vec![0u8; stride];

        for y in 0..height {
            let row_idx = height - 1 - y;
            file.seek(SeekFrom::Start(54 + (row_idx * stride) as u64)).ok()?;
            file.read_exact(&mut row).ok()?;

            for x in 0..width {
                green[y * width + x] = row[x * 3 + 1];
            }
        }

        Some(green)
    }
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, std::path::PathBuf)>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, out)?;
        } else if path.is_file() {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push((rel.to_string_lossy().to_string(), path));
            }
        }
    }
    Ok(())
}
