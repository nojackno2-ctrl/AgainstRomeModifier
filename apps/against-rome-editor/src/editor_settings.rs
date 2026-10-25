//! 編輯器工作階段設定與面板狀態持久化（儲存於 `%APPDATA%\AgainstRomeModifier\editor\editor-panels.json`）。
//! 嚴格禁止寫入遊戲安裝目錄或地圖目錄。
use std::collections::BTreeSet;
use std::path::PathBuf;

/// 支援記憶開啟狀態的設計師專用面板識別碼清單
pub const KNOWN_PANELS: &[&str] = &[
    "maps",
    "environment",
    "landscape",
    "packaging",
    "campaign",
    "ai_planner",
    "ambience",
    "modifier",
    "navigation",
    "minimap",
    "placement",
    "scenario",
    "spawns",
    "textures",
    "scene",
    "console",
    "tech_doc",
    "balance_summary",
    "getting_started",
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditorSettings {
    pub open_panels: BTreeSet<String>,
    file: Option<PathBuf>,
}

impl EditorSettings {
    /// 載入設定檔；若檔案不存在則返回空白預設狀態；嚴格拒絕地圖目錄。
    pub fn load(file: PathBuf) -> Result<Self, String> {
        if crate::packaging_ui::is_game_maps_dir(&file) {
            return Err("Refusing to load editor settings from game maps directory / 拒絕從地圖目錄載入設定".into());
        }
        let mut result = Self {
            open_panels: BTreeSet::new(),
            file: Some(file.clone()),
        };
        if !file.exists() {
            return Ok(result);
        }
        let text = crate::layout_io::read_json_file(&file)?;
        let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        if json["version"].as_u64() != Some(1) {
            return Err("Unsupported editor settings version / 不支援的編輯器設定檔版本".into());
        }
        if let Some(panels) = json["open_panels"].as_array() {
            for item in panels {
                if let Some(name) = item.as_str() {
                    let trimmed = name.trim();
                    if KNOWN_PANELS.contains(&trimmed) {
                        result.open_panels.insert(trimmed.to_string());
                    }
                }
            }
        }
        Ok(result)
    }

    /// 將目前面板狀態原子寫入設定檔；嚴格拒絕地圖目錄。
    pub fn save(&self) -> Result<(), String> {
        if let Some(file) = &self.file {
            if crate::packaging_ui::is_game_maps_dir(file) {
                return Err("Refusing to save editor settings into game maps directory / 拒絕儲存設定至地圖目錄".into());
            }
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let list: Vec<&str> = self.open_panels.iter().map(|s| s.as_str()).collect();
            let json = serde_json::json!({
                "version": 1,
                "open_panels": list
            });
            crate::layout_io::write_json_atomic(file, &json.to_string())?;
        }
        Ok(())
    }

    /// 檢查特定面板是否為開啟狀態
    pub fn is_panel_open(&self, panel: &str) -> bool {
        self.open_panels.contains(panel)
    }

    /// 設定單一面板開關狀態並立即儲存
    pub fn set_panel_open(&mut self, panel: &str, open: bool) -> Result<(), String> {
        let changed = if open {
            if KNOWN_PANELS.contains(&panel) {
                self.open_panels.insert(panel.to_string())
            } else {
                false
            }
        } else {
            self.open_panels.remove(panel)
        };
        if changed {
            self.save()?;
        }
        Ok(())
    }

    /// 從目前的面板開啟清單比對並同步；若有變動則自動儲存，返回是否有狀態更動。
    pub fn sync_panels(&mut self, current: &[(&str, bool)]) -> Result<bool, String> {
        let mut new_set = BTreeSet::new();
        for &(name, open) in current {
            if open && KNOWN_PANELS.contains(&name) {
                new_set.insert(name.to_string());
            }
        }
        if new_set != self.open_panels {
            self.open_panels = new_set;
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_memory_only_and_invalid_files_are_preserved() {
        let mut defaults = EditorSettings::default();
        assert!(defaults.file.is_none());
        defaults.set_panel_open("maps", true).unwrap();
        assert!(defaults.file.is_none());
        let dir = std::env::temp_dir().join(format!("arm_settings_invalid_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("editor-panels.json");
        for payload in ["{broken", r#"{"version":99,"open_panels":["maps"]}"#] {
            std::fs::write(&file, payload).unwrap();
            assert!(EditorSettings::load(file.clone()).is_err());
            assert_eq!(std::fs::read_to_string(&file).unwrap(), payload);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unchanged_snapshot_never_recreates_a_deleted_file() {
        let dir = std::env::temp_dir().join(format!("arm_settings_sync_{}", uuid::Uuid::new_v4()));
        let file = dir.join("editor-panels.json");
        let mut settings = EditorSettings::load(file.clone()).unwrap();
        assert!(settings.sync_panels(&[("maps", true)]).unwrap());
        std::fs::remove_file(&file).unwrap();
        assert!(!settings.sync_panels(&[("maps", true)]).unwrap());
        assert!(!file.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn editor_settings_roundtrip_prunes_unknown_and_refuses_maps_dir() {
        let temp_dir = std::env::temp_dir().join(format!("arm_test_settings_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let settings_path = temp_dir.join("editor-panels.json");

        // 1. Initial load from non-existent file
        let mut settings = EditorSettings::load(settings_path.clone()).unwrap();
        assert!(settings.open_panels.is_empty());

        // 2. Add known panels and save
        settings.set_panel_open("campaign", true).unwrap();
        settings.set_panel_open("ai_planner", true).unwrap();
        settings.set_panel_open("minimap", true).unwrap();
        settings.set_panel_open("unknown_panel", true).unwrap(); // should not be added
        assert_eq!(settings.open_panels.len(), 3);
        assert!(settings.is_panel_open("campaign"));
        assert!(settings.is_panel_open("ai_planner"));
        assert!(settings.is_panel_open("minimap"));
        assert!(!settings.is_panel_open("unknown_panel"));

        // 3. Reload from disk
        let reloaded = EditorSettings::load(settings_path.clone()).unwrap();
        assert_eq!(reloaded.open_panels, settings.open_panels);

        // 4. Test sync_panels
        let snapshot = [
            ("campaign", true),
            ("ai_planner", false),
            ("ambience", true),
        ];
        let changed = settings.sync_panels(&snapshot).unwrap();
        assert!(changed);
        assert_eq!(settings.open_panels.len(), 2);
        assert!(settings.is_panel_open("campaign"));
        assert!(settings.is_panel_open("ambience"));
        assert!(!settings.is_panel_open("ai_planner"));

        // Sync again with same snapshot -> unchanged
        assert!(!settings.sync_panels(&snapshot).unwrap());

        // 5. Corrupted version or payload handling
        std::fs::write(&settings_path, r#"{"version":2,"open_panels":["campaign"]}"#).unwrap();
        assert!(EditorSettings::load(settings_path.clone()).is_err());

        // 6. Refuse MAPS directory
        let maps_dir = temp_dir.join("MAPS").join("TEST_001");
        std::fs::create_dir_all(&maps_dir).unwrap();
        let maps_settings = maps_dir.join("editor-panels.json");
        assert!(EditorSettings::load(maps_settings).is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
