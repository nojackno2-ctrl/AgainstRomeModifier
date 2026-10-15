use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Error, ErrorKind, Read, Result};
use std::path::Path;

pub const MANIFEST_FILE_NAME: &str = "manifest.json";
pub const CURRENT_SCHEMA_VERSION: &str = "1.1";
pub const CUSTOM_MAPS_REGISTRY_FILE: &str = "arm_custom_maps.json";
pub const CUSTOM_MAP_MARKER_FILE: &str = ".arm_custom_map";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapPackageLocalization {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub briefing_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_names: Option<Vec<String>>,
}

impl MapPackageLocalization {
    pub fn new(
        title: impl Into<String>,
        subtitle: Option<String>,
        briefing_text: Option<String>,
        team_names: Option<Vec<String>>,
    ) -> Self {
        Self {
            title: title.into(),
            subtitle,
            briefing_text,
            team_names,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapRecommendedPlayers {
    pub min_players: i32,
    pub max_players: i32,
    pub optimal_players: i32,
    pub team_configurations: String,
    pub faction_restrictions: String,
}

impl Default for MapRecommendedPlayers {
    fn default() -> Self {
        Self {
            min_players: 1,
            max_players: 4,
            optimal_players: 2,
            team_configurations: "1v1, 2v2, FFA".to_string(),
            faction_restrictions: "Any".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapPackageDimensions {
    pub grid_width: i32,
    pub grid_height: i32,
    pub world_units_width: f32,
    pub world_units_height: f32,
    pub height_step: f32,
    pub water_level: f32,
}

impl Default for MapPackageDimensions {
    fn default() -> Self {
        Self {
            grid_width: 256,
            grid_height: 256,
            world_units_width: 16384.0,
            world_units_height: 16384.0,
            height_step: 4.0,
            water_level: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MapFileCategory {
    TerrainHeight,
    TerrainEmboss,
    TerrainVertex,
    TerrainSmooth,
    Collision,
    Minimap,
    TextureBlending,
    DataPool,
    Script,
    Localization,
    SettlementSdl,
    ScenarioJson,
    CustomMarker,
    Thumbnail,
    Documentation,
    Auxiliary,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapPackageFileEntry {
    pub relative_path: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub category: MapFileCategory,
    pub is_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapCompatibilityPolicy {
    pub preferred_slot: i32,
    pub allow_dynamic_slot_remapping: bool,
    pub standalone_level: bool,
    pub min_game_version: String,
    pub target_patch: String,
}

impl Default for MapCompatibilityPolicy {
    fn default() -> Self {
        Self {
            preferred_slot: 5,
            allow_dynamic_slot_remapping: true,
            standalone_level: false,
            min_game_version: "1.0".to_string(),
            target_patch: "None".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CustomMapEntry {
    pub slot: i32,
    pub source_slot: i32,
    pub created_at: String,
    pub tool_version: String,
    #[serde(default)]
    pub standalone_level: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapPackageManifest {
    pub schema_version: String,
    pub payload_layout: String,
    pub source_map_id: String,
    pub package_id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    pub author: String,
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub description: String,
    #[serde(default)]
    pub localizations: BTreeMap<String, MapPackageLocalization>,
    pub players: MapRecommendedPlayers,
    pub dimensions: MapPackageDimensions,
    #[serde(default)]
    pub game_modes: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub compatibility: MapCompatibilityPolicy,
    #[serde(default)]
    pub files: Vec<MapPackageFileEntry>,
    pub package_checksum: String,
}

impl Default for MapPackageManifest {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            schema_version: CURRENT_SCHEMA_VERSION.to_string(),
            payload_layout: "map-relative".to_string(),
            source_map_id: "ENDL_005".to_string(),
            package_id: String::new(),
            title: String::new(),
            subtitle: None,
            author: "Community Modder".to_string(),
            version: "1.0.0".to_string(),
            created_at: now,
            modified_at: now,
            description: String::new(),
            localizations: BTreeMap::new(),
            players: MapRecommendedPlayers::default(),
            dimensions: MapPackageDimensions::default(),
            game_modes: vec!["Endless".to_string()],
            tags: Vec::new(),
            compatibility: MapCompatibilityPolicy::default(),
            files: Vec::new(),
            package_checksum: String::new(),
        }
    }
}

impl MapPackageManifest {
    pub fn validate(&self) -> std::result::Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.package_id.trim().is_empty() {
            errors.push("PackageId 不能為空。".to_string());
        }
        if self.title.trim().is_empty() {
            errors.push("Title 地圖名稱不能為空。".to_string());
        }
        if self.compatibility.preferred_slot < 5 || self.compatibility.preferred_slot > 999 {
            errors.push(format!(
                "PreferredSlot ({}) 必須在 5 至 999 範圍內，原廠槽位 0..4 禁止作為發布槽位。",
                self.compatibility.preferred_slot
            ));
        }
        if self.players.min_players < 1 || self.players.max_players < self.players.min_players {
            errors.push("玩家人數配置無效 (MinPlayers 需 >= 1 且 MaxPlayers >= MinPlayers)。".to_string());
        }
        if self.dimensions.grid_width <= 0 || self.dimensions.grid_height <= 0 {
            errors.push("地圖網格尺寸必須大於 0。".to_string());
        }
        if self.files.is_empty() {
            errors.push("Files 檔案清單不能為空。".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn compute_package_checksum(&self) -> String {
        let standalone_str = if self.compatibility.standalone_level {
            "True"
        } else {
            "False"
        };
        let remapping_str = if self.compatibility.allow_dynamic_slot_remapping {
            "True"
        } else {
            "False"
        };

        let mut buf = format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}x{}|{}|",
            self.schema_version,
            self.payload_layout,
            self.source_map_id,
            self.title,
            standalone_str,
            remapping_str,
            self.package_id,
            self.version,
            self.dimensions.grid_width,
            self.dimensions.grid_height,
            self.compatibility.preferred_slot
        );

        let mut sorted_files = self.files.clone();
        sorted_files.sort_by(|a, b| a.relative_path.to_ascii_lowercase().cmp(&b.relative_path.to_ascii_lowercase()));

        for file in sorted_files {
            buf.push_str(&file.relative_path.to_ascii_lowercase());
            buf.push(':');
            buf.push_str(&file.size_bytes.to_string());
            buf.push(':');
            buf.push_str(&file.sha256);
            buf.push(';');
        }

        let hash = against_rome_data::sha256::sha256(buf.as_bytes());
        hash.iter().map(|b| format!("{:02X}", b)).collect()
    }

    pub fn to_json(&mut self) -> std::result::Result<String, serde_json::Error> {
        self.package_checksum = self.compute_package_checksum();
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self> {
        if json.trim().is_empty() {
            return Err(Error::new(ErrorKind::InvalidData, "JSON 字串為空"));
        }
        serde_json::from_str(json).map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))
    }

    pub fn create_from_directory(
        map_directory: &Path,
        package_id: Option<&str>,
        author: Option<&str>,
        version: Option<&str>,
        preferred_slot: Option<i32>,
    ) -> Result<Self> {
        if !map_directory.exists() || !map_directory.is_dir() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("地圖目錄不存在: {}", map_directory.display()),
            ));
        }

        let dir_name = map_directory
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "custom_map".to_string());

        let pkg_id = match package_id {
            Some(id) if !id.trim().is_empty() => id.to_string(),
            _ => dir_name.to_ascii_lowercase(),
        };

        let now = Utc::now();
        let mut manifest = Self {
            package_id: pkg_id,
            author: author.unwrap_or("Community Modder").to_string(),
            version: version.unwrap_or("1.0.0").to_string(),
            created_at: now,
            modified_at: now,
            ..Default::default()
        };

        // 讀取標題與多語系簡報
        let us_briefing = map_directory.join("TEXT").join("US").join("briefing.put");
        if us_briefing.exists() {
            let put_map = parse_put_file(&us_briefing)?;
            manifest.title = put_map
                .get("briefing_titel_1")
                .cloned()
                .unwrap_or_else(|| dir_name.clone());
            manifest.subtitle = put_map.get("briefing_titel_2").cloned();
            manifest.description = put_map.get("briefing_text").cloned().unwrap_or_default();

            let mut team_names = Vec::new();
            for i in 0..8 {
                let key = format!("briefing_text_teamname{}", i);
                if let Some(team) = put_map.get(&key) {
                    if !team.trim().is_empty() {
                        team_names.push(team.clone());
                    }
                }
            }
            let teams_opt = if team_names.is_empty() {
                None
            } else {
                Some(team_names)
            };

            manifest.localizations.insert(
                "US".to_string(),
                MapPackageLocalization::new(
                    &manifest.title,
                    manifest.subtitle.clone(),
                    Some(manifest.description.clone()),
                    teams_opt,
                ),
            );
        } else {
            manifest.title = dir_name.clone();
        }

        // 讀取水面與地形配置
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
        manifest.dimensions = MapPackageDimensions {
            water_level,
            ..Default::default()
        };

        // 掃描槽位資訊
        let slot = preferred_slot.unwrap_or_else(|| extract_slot_from_directory(map_directory));
        let standalone = has_standalone_marker(map_directory);

        manifest.compatibility = MapCompatibilityPolicy {
            preferred_slot: slot,
            standalone_level: standalone,
            ..Default::default()
        };
        manifest.source_map_id = format!("ENDL_{:03}", slot);

        // 掃描檔案清單
        manifest.scan_files(map_directory)?;
        manifest.package_checksum = manifest.compute_package_checksum();

        Ok(manifest)
    }

    pub fn scan_files(&mut self, map_directory: &Path) -> Result<()> {
        self.files.clear();
        let mut collected = Vec::new();
        collect_files_recursive(map_directory, map_directory, &mut collected)?;

        for (relative, full_path) in collected {
            let normalized = relative.replace('\\', "/");
            if should_exclude_from_files(&normalized) {
                continue;
            }

            let category = categorize_file(&normalized);
            let mut file = File::open(&full_path)?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;

            let hash_bytes = against_rome_data::sha256::sha256(&buf);
            let hex_hash: String = hash_bytes.iter().map(|b| format!("{:02X}", b)).collect();

            self.files.push(MapPackageFileEntry {
                relative_path: normalized,
                size_bytes: buf.len() as i64,
                sha256: hex_hash,
                category,
                is_required: is_required_file(category),
            });
        }

        Ok(())
    }
}

fn collect_files_recursive(
    root: &Path,
    dir: &Path,
    collected: &mut Vec<(String, std::path::PathBuf)>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(root, &path, collected)?;
        } else if path.is_file() {
            if let Ok(rel) = path.strip_prefix(root) {
                collected.push((rel.to_string_lossy().to_string(), path));
            }
        }
    }
    Ok(())
}

pub fn should_exclude_from_files(relative_path: &str) -> bool {
    let name = relative_path.split('/').last().unwrap_or(relative_path);
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".tmp_arm")
        || lower.ends_with(".bak")
        || lower.ends_with(".log")
        || lower == "thumbs.db"
        || lower == ".ds_store"
        || lower == "sdl_placed_objects.put"
    {
        return true;
    }
    false
}

pub fn categorize_file(relative_path: &str) -> MapFileCategory {
    let normalized = relative_path.replace('\\', "/").to_ascii_lowercase();
    let name = normalized.split('/').last().unwrap_or(&normalized);

    if name == "boden.bmp" {
        return MapFileCategory::TerrainHeight;
    }
    if name == "emboss.bmp" {
        return MapFileCategory::TerrainEmboss;
    }
    if name == "vertex.bmp" {
        return MapFileCategory::TerrainVertex;
    }
    if name == "smooth.bmp" {
        return MapFileCategory::TerrainSmooth;
    }
    if name == "collision.bmp" {
        return MapFileCategory::Collision;
    }
    if name == "minimap.bmp" {
        return MapFileCategory::Minimap;
    }
    if name == "floortex.dat" || name == "textures.txt" {
        return MapFileCategory::TextureBlending;
    }
    if normalized.starts_with("data/") {
        return MapFileCategory::DataPool;
    }
    if normalized.starts_with("script/") {
        return MapFileCategory::Script;
    }
    if normalized.starts_with("text/") {
        return MapFileCategory::Localization;
    }
    if name.ends_with(".sdl") {
        return MapFileCategory::SettlementSdl;
    }
    if name == "arm_scenario.json" {
        return MapFileCategory::ScenarioJson;
    }
    if name == CUSTOM_MAP_MARKER_FILE {
        return MapFileCategory::CustomMarker;
    }
    if name.starts_with("thumbnail.") {
        return MapFileCategory::Thumbnail;
    }
    if name.ends_with(".txt") || name.ends_with(".md") {
        return MapFileCategory::Documentation;
    }

    MapFileCategory::Auxiliary
}

pub fn is_required_file(category: MapFileCategory) -> bool {
    matches!(
        category,
        MapFileCategory::TerrainHeight
            | MapFileCategory::Collision
            | MapFileCategory::DataPool
            | MapFileCategory::Script
            | MapFileCategory::Localization
    )
}

pub fn extract_slot_from_directory(directory: &Path) -> i32 {
    if let Some(file_name) = directory.file_name() {
        let s = file_name.to_string_lossy();
        if s.starts_with("ENDL_") || s.starts_with("endl_") {
            if let Ok(slot) = s[5..].parse::<i32>() {
                return slot;
            }
        }
    }
    5
}

pub fn has_standalone_marker(path: &Path) -> bool {
    let marker_path = path.join(CUSTOM_MAP_MARKER_FILE);
    if marker_path.exists() {
        if let Ok(content) = fs::read_to_string(&marker_path) {
            if let Ok(entry) = serde_json::from_str::<CustomMapEntry>(&content) {
                return entry.standalone_level;
            }
        }
    }
    false
}

pub fn parse_put_file(path: &Path) -> Result<BTreeMap<String, String>> {
    let raw = fs::read(path)?;
    // Windows-1251 / ISO-8859-1 / UTF-8 fallback
    let text = match String::from_utf8(raw.clone()) {
        Ok(s) => s,
        Err(_) => {
            let (decoded, _) = encoding_rs::WINDOWS_1251.decode_without_bom_handling(&raw);
            decoded.to_string()
        }
    };

    let mut map = BTreeMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with("//") {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let mut key = k.trim();
            if let Some(stripped) = key.strip_prefix("var:") {
                key = stripped.trim();
            }
            let mut val = v.trim();
            if val.ends_with(';') {
                val = val[..val.len() - 1].trim();
            }
            if val.starts_with('"') && val.ends_with('"') && val.len() >= 2 {
                val = &val[1..val.len() - 1];
            }
            map.insert(key.to_string(), val.to_string());
        }
    }
    Ok(map)
}
