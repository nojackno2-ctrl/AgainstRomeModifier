//! Custom map access validation and manifest tracking (`CustomMapAccess`, `CustomMapManifest`).

use std::fs;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Record for custom map metadata stored in `arm_custom_maps.json` and `.arm_custom_map`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CustomMapEntry {
    pub slot: i32,
    pub source_slot: i32,
    pub created_at: DateTime<Utc>,
    pub tool_version: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub standalone_level: bool,
}

impl CustomMapEntry {
    pub fn new(slot: i32, source_slot: i32, created_at: DateTime<Utc>, tool_version: impl Into<String>) -> Self {
        Self {
            slot,
            source_slot,
            created_at,
            tool_version: tool_version.into(),
            standalone_level: false,
        }
    }

    pub fn is_valid(&self) -> bool {
        (5..=999).contains(&self.slot)
            && (0..=999).contains(&self.source_slot)
            && !self.tool_version.trim().is_empty()
    }
}

/// Helper functions to parse and validate `ENDL_###` slot names.
pub fn parse_endl_slot(name: &str) -> Option<i32> {
    if name.len() == 8 && name[..5].eq_ignore_ascii_case("ENDL_") {
        name[5..].parse::<i32>().ok()
    } else {
        None
    }
}

/// Guard preventing accidental modification of retail game maps (`CustomMapAccess`).
pub struct CustomMapAccess;

impl CustomMapAccess {
    /// Determines whether the given map directory is an editable custom endless map (ENDL_005..999).
    pub fn is_editable_directory(map_directory: &Path) -> bool {
        let Ok(full_path) = map_directory.canonicalize().or_else(|_| Ok::<PathBuf, ()>(map_directory.to_path_buf())) else {
            return false;
        };
        let Some(file_name) = full_path.file_name().and_then(|s| s.to_str()) else {
            return false;
        };
        let Some(slot) = parse_endl_slot(file_name) else {
            return false;
        };
        if slot < 5 {
            return false;
        }

        let Some(parent) = full_path.parent() else {
            return false;
        };
        let Some(parent_name) = parent.file_name().and_then(|s| s.to_str()) else {
            return false;
        };
        if !parent_name.eq_ignore_ascii_case("MAPS") {
            return false;
        }

        CustomMapManifest::is_custom_map_directory(&full_path)
    }

    /// Requires the directory to be an editable custom map, returning an error otherwise.
    pub fn require_editable_directory(map_directory: &Path, game_path: Option<&Path>) -> Result<PathBuf, String> {
        let full_path = map_directory.canonicalize().unwrap_or_else(|_| map_directory.to_path_buf());
        if let Some(gp) = game_path {
            let expected_maps = gp.join("MAPS").canonicalize().unwrap_or_else(|_| gp.join("MAPS"));
            let parent = full_path.parent().map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()));
            if parent.as_ref() != Some(&expected_maps) {
                return Err("地圖資料夾不在選取的遊戲 MAPS 目錄，已取消儲存。".into());
            }
        }

        if !Self::is_editable_directory(&full_path) {
            return Err("編輯只允許 marker-backed 的 ENDL_005–999 自製地圖。".into());
        }

        Ok(full_path)
    }
}

/// Manifest tracking all custom maps created in the game's `MAPS` directory (`CustomMapManifest`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomMapManifest {
    entries: Vec<CustomMapEntry>,
}

impl CustomMapManifest {
    pub const FILE_NAME: &'static str = "arm_custom_maps.json";
    pub const MARKER_FILE_NAME: &'static str = ".arm_custom_map";

    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn entries(&self) -> &[CustomMapEntry] {
        &self.entries
    }

    /// Loads the manifest from `<gamePath>/MAPS/arm_custom_maps.json`.
    pub fn load(game_path: &Path) -> Result<Self, String> {
        let maps_path = game_path.join("MAPS");
        let manifest_path = maps_path.join(Self::FILE_NAME);
        if !manifest_path.exists() {
            return Ok(Self::new());
        }

        let content = fs::read_to_string(&manifest_path)
            .map_err(|e| format!("讀取自製地圖清單失敗 ({}): {e}", manifest_path.display()))?;
        let parsed: Vec<CustomMapEntry> = serde_json::from_str(&content)
            .map_err(|e| format!("解析自製地圖清單 JSON 失敗: {e}"))?;

        let valid_entries = parsed.into_iter().filter(|e| e.is_valid()).collect();
        Ok(Self { entries: valid_entries })
    }

    /// Registers a custom map entry, replacing any prior entry for the same slot.
    pub fn register(&mut self, entry: CustomMapEntry) -> Result<(), String> {
        if !entry.is_valid() {
            return Err("無效的自製地圖登記資料。".into());
        }
        self.entries.retain(|x| x.slot != entry.slot);
        self.entries.push(entry);
        self.entries.sort_by_key(|x| x.slot);
        Ok(())
    }

    /// Removes an entry by slot.
    pub fn remove(&mut self, slot: i32) {
        self.entries.retain(|x| x.slot != slot);
    }

    /// Saves the manifest to `<gamePath>/MAPS/arm_custom_maps.json`.
    pub fn save(&self, game_path: &Path) -> Result<(), String> {
        let maps_path = game_path.join("MAPS");
        if !maps_path.exists() {
            fs::create_dir_all(&maps_path)
                .map_err(|e| format!("建立 MAPS 目錄失敗: {e}"))?;
        }
        let manifest_path = maps_path.join(Self::FILE_NAME);
        let mut sorted = self.entries.clone();
        sorted.sort_by_key(|x| x.slot);
        let json = serde_json::to_string_pretty(&sorted)
            .map_err(|e| format!("序列化自製地圖清單失敗: {e}"))?;

        // Atomic write via temporary file
        let tmp_path = maps_path.join(format!("{}.tmp", Self::FILE_NAME));
        fs::write(&tmp_path, json.as_bytes())
            .map_err(|e| format!("寫入暫存清單失敗: {e}"))?;
        fs::rename(&tmp_path, &manifest_path)
            .map_err(|e| format!("替換自製地圖清單失敗: {e}"))?;

        Ok(())
    }

    /// Returns true if the map directory contains `.arm_custom_map`.
    pub fn is_custom_map_directory(path: &Path) -> bool {
        path.join(Self::MARKER_FILE_NAME).is_file()
    }

    /// Checks if a custom map directory has `standalone_level == true`.
    pub fn has_standalone_level(path: &Path) -> bool {
        if !CustomMapAccess::is_editable_directory(path) {
            return false;
        }
        let marker_file = path.join(Self::MARKER_FILE_NAME);
        if let Ok(content) = fs::read_to_string(&marker_file) {
            if let Ok(entry) = serde_json::from_str::<CustomMapEntry>(&content) {
                return entry.standalone_level;
            }
        }
        false
    }

    /// Checks if a file belongs to a custom map under `maps_path`.
    pub fn is_custom_map_file(maps_path: &Path, file_path: &Path) -> bool {
        let Ok(full_maps) = maps_path.canonicalize() else { return false; };
        let Ok(full_file) = file_path.canonicalize() else { return false; };
        let Ok(relative) = full_file.strip_prefix(&full_maps) else { return false; };
        let mut components = relative.components();
        let Some(std::path::Component::Normal(first)) = components.next() else { return false; };
        let map_dir = full_maps.join(first);
        Self::is_custom_map_directory(&map_dir)
    }
}
