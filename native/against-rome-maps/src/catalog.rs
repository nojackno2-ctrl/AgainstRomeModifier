//! Catalogs for retail and custom endless and game maps (`EndlessMapCatalog`, `GameMapCatalog`).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use crate::custom_map::{parse_endl_slot, CustomMapAccess};
use crate::text_docs::PutTextDocument;

/// Information about an endless map slot (`EndlessMapInfo`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndlessMapInfo {
    pub slot: i32,
    pub directory_path: PathBuf,
    pub is_custom: bool,
    pub display_name: Option<String>,
}

impl EndlessMapInfo {
    pub fn id(&self) -> String {
        format!("ENDL_{:03}", self.slot)
    }
}

/// Catalog managing endless map discovery and slot allocation (`EndlessMapCatalog`).
#[derive(Clone, Debug, Default)]
pub struct EndlessMapCatalog;

impl EndlessMapCatalog {
    pub fn new() -> Self {
        Self
    }

    pub fn validate_game_path(game_path: &Path) -> Result<PathBuf, String> {
        if game_path.as_os_str().is_empty() {
            return Err("未提供遊戲路徑。".into());
        }
        let full = game_path.canonicalize().map_err(|_| format!("找不到遊戲路徑: {}", game_path.display()))?;
        if !full.is_dir() {
            return Err(format!("找不到遊戲路徑: {}", full.display()));
        }
        Ok(full)
    }

    pub fn list(&self, game_path: &Path) -> Result<Vec<EndlessMapInfo>, String> {
        let valid_game = Self::validate_game_path(game_path)?;
        let maps_path = valid_game.join("MAPS");
        if !maps_path.exists() {
            return Ok(Vec::new());
        }

        let mut result = Vec::new();
        let read_dir = fs::read_dir(&maps_path)
            .map_err(|e| format!("無法列舉 MAPS 目錄: {e}"))?;

        for entry in read_dir.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if let Some(slot) = parse_endl_slot(name) {
                let is_custom = CustomMapAccess::is_editable_directory(&path);
                let display_name = try_read_title(&path);
                result.push(EndlessMapInfo {
                    slot,
                    directory_path: path,
                    is_custom,
                    display_name,
                });
            }
        }

        result.sort_by_key(|x| x.slot);
        Ok(result)
    }

    pub fn get_next_free_slot(&self, game_path: &Path) -> Result<i32, String> {
        let list = self.list(game_path)?;
        let occupied: HashSet<i32> = list.into_iter().map(|x| x.slot).collect();
        for slot in 5..=999 {
            if !occupied.contains(&slot) {
                return Ok(slot);
            }
        }
        Err("沒有可用的 ENDL_005 至 ENDL_999 地圖槽位。".into())
    }

    pub fn require(&self, game_path: &Path, slot: i32) -> Result<EndlessMapInfo, String> {
        if !(0..=999).contains(&slot) {
            return Err(format!("槽位超出範圍: {slot}"));
        }
        let list = self.list(game_path)?;
        list.into_iter()
            .find(|x| x.slot == slot)
            .ok_or_else(|| format!("找不到 ENDL_{:03}。", slot))
    }
}

/// Information about any game map (`GameMapInfo`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameMapInfo {
    pub id: String,
    pub directory_path: PathBuf,
    pub is_custom: bool,
    pub display_name: Option<String>,
    pub category: String,
}

impl GameMapInfo {
    pub fn endless_slot(&self) -> Option<i32> {
        parse_endl_slot(&self.id)
    }
}

/// Catalog listing all campaign, historical, multiplayer, tutorial, and endless maps (`GameMapCatalog`).
#[derive(Clone, Debug, Default)]
pub struct GameMapCatalog;

impl GameMapCatalog {
    pub fn new() -> Self {
        Self
    }

    pub fn list(&self, game_path: &Path) -> Result<Vec<GameMapInfo>, String> {
        let valid_game = EndlessMapCatalog::validate_game_path(game_path)?;
        let maps_path = valid_game.join("MAPS");
        if !maps_path.exists() {
            return Ok(Vec::new());
        }

        let mut result = Vec::new();
        let read_dir = fs::read_dir(&maps_path)
            .map_err(|e| format!("無法列舉 MAPS 目錄: {e}"))?;

        for entry in read_dir.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if !is_safe_map_id(name) {
                continue;
            }
            if !path.join("boden.txt").is_file() || !path.join("minimap.bmp").is_file() {
                continue;
            }

            let is_custom = CustomMapAccess::is_editable_directory(&path);
            let display_name = try_read_title(&path);
            let cat = category(name);

            result.push(GameMapInfo {
                id: name.to_string(),
                directory_path: path,
                is_custom,
                display_name,
                category: cat.to_string(),
            });
        }

        result.sort_by(|a, b| {
            category_order(&a.category)
                .cmp(&category_order(&b.category))
                .then_with(|| a.id.to_ascii_lowercase().cmp(&b.id.to_ascii_lowercase()))
        });

        Ok(result)
    }

    pub fn require(&self, game_path: &Path, map_id: &str) -> Result<GameMapInfo, String> {
        if !is_safe_map_id(map_id) {
            return Err("無效的地圖代號。".into());
        }
        let list = self.list(game_path)?;
        list.into_iter()
            .find(|x| x.id.eq_ignore_ascii_case(map_id))
            .ok_or_else(|| format!("找不到地圖: {map_id}"))
    }
}

fn is_safe_map_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn category(id: &str) -> &'static str {
    let upper = id.to_ascii_uppercase();
    if upper.starts_with("KAMP_") {
        "劇情戰役"
    } else if upper.starts_with("HIST_") {
        "歷史戰役"
    } else if upper.starts_with("TUTOR_") {
        "教學"
    } else if upper.starts_with("MP_") {
        "多人地圖"
    } else if upper.starts_with("ENDL_") {
        "無盡模式"
    } else {
        "其他地圖"
    }
}

fn category_order(category: &str) -> i32 {
    match category {
        "劇情戰役" => 0,
        "歷史戰役" => 1,
        "教學" => 2,
        "無盡模式" => 3,
        "多人地圖" => 4,
        _ => 5,
    }
}

fn try_read_title(map_directory: &Path) -> Option<String> {
    let path = map_directory.join("TEXT").join("US").join("briefing.put");
    if !path.exists() {
        return None;
    }
    PutTextDocument::load(&path).ok().and_then(|doc| doc.get_value("briefing_titel_1"))
}
