//! Safe endless map cloner with atomic rollback (`EndlessMapCloner`).

use std::fs;
use std::path::Path;
use chrono::Utc;
use crate::catalog::{EndlessMapCatalog, EndlessMapInfo, GameMapCatalog};
use crate::custom_map::{CustomMapEntry, CustomMapManifest};
use crate::text_docs::{PutTextDocument, SdlDocument};
use crate::text_encoding::can_encode_game_text;

pub struct EndlessMapCloner {
    catalog: EndlessMapCatalog,
}

impl EndlessMapCloner {
    pub fn new() -> Self {
        Self {
            catalog: EndlessMapCatalog::new(),
        }
    }

    pub fn clone_map(
        &self,
        game_path: &Path,
        source_slot: i32,
        new_slot: i32,
        new_name: &str,
    ) -> Result<EndlessMapInfo, String> {
        let valid_game = EndlessMapCatalog::validate_game_path(game_path)?;
        let source = self.catalog.require(&valid_game, source_slot)?;
        self.clone_core(
            &valid_game,
            &source.id(),
            &source.directory_path,
            source_slot,
            new_slot,
            new_name,
            None,
            None,
        )
    }

    pub fn clone_from_map_id(
        &self,
        game_path: &Path,
        source_map_id: &str,
        new_slot: i32,
        new_name: &str,
    ) -> Result<EndlessMapInfo, String> {
        let valid_game = EndlessMapCatalog::validate_game_path(game_path)?;
        let source = GameMapCatalog::new().require(&valid_game, source_map_id)?;
        self.clone_core(
            &valid_game,
            &source.id,
            &source.directory_path,
            source.endless_slot().unwrap_or(0),
            new_slot,
            new_name,
            None,
            None,
        )
    }

    pub fn clone_prepared<F>(
        &self,
        game_path: &Path,
        source_map_id: &str,
        new_slot: i32,
        new_name: &str,
        prepare: F,
        standalone_level: Option<bool>,
    ) -> Result<EndlessMapInfo, String>
    where
        F: FnOnce(&Path) -> Result<(), String> + 'static,
    {
        let valid_game = EndlessMapCatalog::validate_game_path(game_path)?;
        let source = GameMapCatalog::new().require(&valid_game, source_map_id)?;
        self.clone_core(
            &valid_game,
            &source.id,
            &source.directory_path,
            source.endless_slot().unwrap_or(0),
            new_slot,
            new_name,
            Some(Box::new(prepare)),
            standalone_level,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn clone_core(
        &self,
        normalized_game_path: &Path,
        source_map_id: &str,
        source_directory: &Path,
        source_slot: i32,
        new_slot: i32,
        new_name: &str,
        prepare: Option<Box<dyn FnOnce(&Path) -> Result<(), String>>>,
        standalone_level: Option<bool>,
    ) -> Result<EndlessMapInfo, String> {
        if !(5..=999).contains(&new_slot) {
            return Err("自製地圖槽位必須在 ENDL_005 至 ENDL_999。".into());
        }
        if new_name.trim().is_empty() {
            return Err("請輸入地圖名稱。".into());
        }
        if new_name.chars().any(|c| "/\\:*?\"<>|".contains(c)) {
            return Err("地圖名稱含不允許的字元。".into());
        }
        if !can_encode_game_text(new_name) {
            return Err("地圖名稱包含遊戲無法儲存的字元。請改用英文、數字或原版地圖使用的字元。".into());
        }

        let maps_path = normalized_game_path.join("MAPS");
        let map_id = format!("ENDL_{:03}", new_slot);
        let destination = maps_path.join(&map_id);
        let temporary = maps_path.join(format!("{}.tmp_arm", map_id));

        if destination.exists() || temporary.exists() {
            return Err(format!("目標地圖槽位已存在或有待清理暫存資料夾: {map_id}"));
        }

        let mut moved = false;
        let result = (|| -> Result<EndlessMapInfo, String> {
            copy_directory_recursive(source_directory, &temporary)?;
            verify_copy(source_directory, &temporary)?;
            rewrite_known_files(&temporary, source_map_id, &map_id, new_name)?;

            if let Some(prep) = prepare {
                prep(&temporary)?;
            }

            let marker = CustomMapEntry {
                slot: new_slot,
                source_slot,
                created_at: Utc::now(),
                tool_version: env!("CARGO_PKG_VERSION").to_string(),
                standalone_level: standalone_level.unwrap_or_else(|| CustomMapManifest::has_standalone_level(source_directory)),
            };

            let marker_json = serde_json::to_string_pretty(&marker)
                .map_err(|e| format!("序列化自製地圖標記失敗: {e}"))?;
            fs::write(temporary.join(CustomMapManifest::MARKER_FILE_NAME), marker_json.as_bytes())
                .map_err(|e| format!("寫入標記檔案失敗: {e}"))?;

            fs::rename(&temporary, &destination)
                .map_err(|e| format!("移動暫存地圖至目標失敗: {e}"))?;
            moved = true;

            let mut manifest = CustomMapManifest::load(normalized_game_path)?;
            manifest.register(marker)?;
            manifest.save(normalized_game_path)?;

            self.catalog.require(normalized_game_path, new_slot)
        })();

        match result {
            Ok(info) => Ok(info),
            Err(e) => {
                if moved && destination.exists() {
                    let _ = fs::remove_dir_all(&destination);
                }
                if temporary.exists() {
                    let _ = fs::remove_dir_all(&temporary);
                }
                Err(e)
            }
        }
    }
}

pub fn rewrite_known_files(
    directory: &Path,
    old_map_id: &str,
    new_map_id: &str,
    new_name: &str,
) -> Result<(), String> {
    let briefing = directory.join("TEXT").join("US").join("briefing.put");
    if briefing.exists() {
        if let Ok(mut put) = PutTextDocument::load(&briefing) {
            put.set_value("briefing_titel_1", new_name)?;
            put.save()?;
        }
    }

    if let Ok(entries) = fs::read_dir(directory) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            let lower = name.to_ascii_lowercase();
            if lower.starts_with("endlos_") && lower.contains("_siedlung") && lower.ends_with(".sdl") {
                if let Ok(mut sdl) = SdlDocument::load(&path) {
                    sdl.rewrite_map_path(old_map_id, new_map_id);
                    sdl.save()?;
                }
            }
        }
    }

    Ok(())
}

fn copy_directory_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if !dst.exists() {
        fs::create_dir_all(dst).map_err(|e| format!("建立目錄失敗: {e}"))?;
    }
    let entries = fs::read_dir(src).map_err(|e| format!("讀取來源目錄失敗: {e}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_directory_recursive(&path, &target)?;
        } else {
            fs::copy(&path, &target).map_err(|e| format!("複製檔案失敗 ({}): {e}", path.display()))?;
        }
    }
    Ok(())
}

fn collect_file_metrics(dir: &Path) -> Result<(usize, u64), String> {
    let mut count = 0;
    let mut total_len = 0;
    let entries = fs::read_dir(dir).map_err(|e| format!("讀取目錄失敗: {e}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let (sub_count, sub_len) = collect_file_metrics(&path)?;
            count += sub_count;
            total_len += sub_len;
        } else if let Ok(meta) = fs::metadata(&path) {
            count += 1;
            total_len += meta.len();
        }
    }
    Ok((count, total_len))
}

fn verify_copy(src: &Path, dst: &Path) -> Result<(), String> {
    let (src_count, src_len) = collect_file_metrics(src)?;
    let (dst_count, dst_len) = collect_file_metrics(dst)?;
    if src_count != dst_count || src_len != dst_len {
        Err("地圖複製驗證失敗：檔案數或總大小不一致。".into())
    } else {
        Ok(())
    }
}
