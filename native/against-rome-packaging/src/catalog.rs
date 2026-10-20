//! Parity with C# `EndlessMapCatalog.List/Require`: enumerate `MAPS/ENDL_###` slots for design tools.
use crate::contract::reject_links;
use crate::exporter::{MAX_CUSTOM_SLOT, MIN_CUSTOM_SLOT};
use crate::manifest::{parse_put_file, CUSTOM_MAP_MARKER_FILE};
use std::fs;
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndlessMapInfo {
    pub slot: i32,
    pub directory_path: PathBuf,
    /// Slot >= 5 with a custom-map marker file (C# `CustomMapAccess.IsEditableDirectory`).
    pub is_custom: bool,
    /// `briefing_titel_1` from `TEXT/US/briefing.put`; `None` when missing or unreadable.
    pub display_name: Option<String>,
}

impl EndlessMapInfo {
    pub fn id(&self) -> String {
        format!("ENDL_{:03}", self.slot)
    }
}

/// Matches `^ENDL_(\d{3})$` case-insensitively.
fn parse_slot_dir_name(name: &str) -> Option<i32> {
    let b = name.as_bytes();
    if b.len() == 8 && name.is_char_boundary(5) && name[..5].eq_ignore_ascii_case("ENDL_") && b[5..].iter().all(u8::is_ascii_digit) {
        name[5..].parse().ok()
    } else {
        None
    }
}

fn validate_game_path(game_path: &Path) -> Result<PathBuf> {
    if game_path.as_os_str().is_empty() || game_path.to_string_lossy().trim().is_empty() {
        return Err(Error::new(ErrorKind::InvalidInput, "未提供遊戲路徑。"));
    }
    if !game_path.is_dir() {
        return Err(Error::new(
            ErrorKind::NotFound,
            format!("找不到遊戲路徑: {}", game_path.display()),
        ));
    }
    Ok(game_path.to_path_buf())
}

fn try_read_title(map_dir: &Path) -> Option<String> {
    let path = map_dir.join("TEXT").join("US").join("briefing.put");
    if !path.is_file() {
        return None;
    }
    parse_put_file(&path).ok()?.remove("briefing_titel_1")
}

/// Lists `ENDL_###` directories under `<game>/MAPS`, ordered by slot. A missing `MAPS` yields an empty list.
pub fn list_endless_maps(game_path: &Path) -> Result<Vec<EndlessMapInfo>> {
    let maps_root = validate_game_path(game_path)?.join("MAPS");
    if !maps_root.is_dir() {
        return Ok(Vec::new());
    }
    reject_links(&maps_root)?;
    let mut maps = Vec::new();
    for entry in fs::read_dir(&maps_root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(slot) = parse_slot_dir_name(&entry.file_name().to_string_lossy()) else {
            continue;
        };
        maps.push(EndlessMapInfo {
            slot,
            is_custom: slot >= MIN_CUSTOM_SLOT && path.join(CUSTOM_MAP_MARKER_FILE).is_file(),
            display_name: try_read_title(&path),
            directory_path: path,
        });
    }
    maps.sort_by_key(|m| m.slot);
    Ok(maps)
}

/// Returns the map in `slot`; `InvalidInput` for slots outside 0..=999, `NotFound` when absent.
pub fn require_endless_map(game_path: &Path, slot: i32) -> Result<EndlessMapInfo> {
    if !(0..=MAX_CUSTOM_SLOT).contains(&slot) {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!("槽位超出範圍 (0-{}): {}", MAX_CUSTOM_SLOT, slot),
        ));
    }
    list_endless_maps(game_path)?
        .into_iter()
        .find(|m| m.slot == slot)
        .ok_or_else(|| Error::new(ErrorKind::NotFound, format!("找不到 ENDL_{:03}。", slot)))
}
