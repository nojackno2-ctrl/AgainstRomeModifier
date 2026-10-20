//! Safe endless map deleter with atomic rollback (`EndlessMapDeleter`).

use std::fs;
use std::path::Path;
use crate::catalog::EndlessMapCatalog;
use crate::custom_map::CustomMapManifest;
use crate::rollback::FileRollbackScope;

pub struct EndlessMapDeleter {
    catalog: EndlessMapCatalog,
}

impl EndlessMapDeleter {
    pub fn new() -> Self {
        Self {
            catalog: EndlessMapCatalog::new(),
        }
    }

    pub fn delete(&self, game_path: &Path, slot: i32) -> Result<(), String> {
        self.delete_with_rollback_factory(game_path, slot, FileRollbackScope::new)
    }

    /// 與 `delete` 相同，但 rollback scope 由 `make_rollback` 建立，供測試注入建立錯誤。
    /// 工廠必須在任何 rename／刪除之前被呼叫。
    #[doc(hidden)]
    pub fn delete_with_rollback_factory<F>(&self, game_path: &Path, slot: i32, make_rollback: F) -> Result<(), String>
    where
        F: FnOnce() -> Result<FileRollbackScope, String>,
    {
        if (0..5).contains(&slot) {
            return Err("只能刪除自製地圖；原廠地圖 ENDL_000–004 不可刪除。".into());
        }

        let normalized_game_path = EndlessMapCatalog::validate_game_path(game_path)?;
        let map = self.catalog.require(&normalized_game_path, slot)?;

        if !map.is_custom || !CustomMapManifest::is_custom_map_directory(&map.directory_path) {
            return Err("只能刪除由地圖編輯器建立的自製地圖。".into());
        }

        let mut manifest = CustomMapManifest::load(&normalized_game_path)?;
        if !manifest.entries().iter().any(|entry| entry.slot == slot) {
            return Err("地圖未登記在自製地圖 manifest，已取消刪除。".into());
        }

        let maps_path = normalized_game_path.join("MAPS").canonicalize().map_err(|e| e.to_string())?;
        let map_path = map.directory_path.canonicalize().map_err(|e| e.to_string())?;

        let parent = map_path.parent().and_then(|p| p.canonicalize().ok());
        if parent.as_ref() != Some(&maps_path) {
            return Err("地圖資料夾不在預期的位置，已取消刪除。".into());
        }

        let temporary = map_path.with_extension("deleting_arm");
        if temporary.exists() {
            return Err(format!("地圖刪除暫存資料夾已存在，請先檢查: {}", temporary.display()));
        }

        // 先建立 rollback scope：失敗時尚未改名，不會遺留 .deleting_arm 目錄。
        let mut rollback = make_rollback()?;

        fs::rename(&map_path, &temporary).map_err(|e| format!("移動待刪除資料夾失敗: {e}"))?;
        let perform_delete = (|| -> Result<(), String> {
            manifest.remove(slot);
            manifest.save_with_rollback(&normalized_game_path, Some(&mut rollback))?;
            fs::remove_dir_all(&temporary).map_err(|e| format!("刪除暫存地圖資料夾失敗: {e}"))?;
            rollback.commit();
            Ok(())
        })();

        match perform_delete {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = rollback.restore_all();
                if !map_path.exists() && temporary.exists() {
                    let _ = fs::rename(&temporary, &map_path);
                }
                Err(e)
            }
        }
    }
}
