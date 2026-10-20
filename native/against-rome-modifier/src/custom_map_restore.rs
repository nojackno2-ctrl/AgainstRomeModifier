//! Full-restore transaction for custom maps, ported from `CustomMapRestoreService`.

use std::{io, path::Path};

use against_rome_maps::{
    CustomMapManifest, EndlessMapCatalog, EndlessMapDeleter, FileRollbackScope,
};

use crate::{cli::commit_plan, services::PatchFilePlan};

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// Validates and returns the custom map slots that a full restore will delete.
/// This runs before any restore writes so a missing registration or stale delete
/// staging directory cannot leave a partially restored game.
pub fn deletion_slots(game_path: &Path, preserve_custom_maps: bool) -> io::Result<Vec<i32>> {
    if preserve_custom_maps {
        return Ok(Vec::new());
    }

    let catalog = EndlessMapCatalog::new();
    let maps = catalog.list(game_path).map_err(invalid)?;
    let custom: Vec<_> = maps.into_iter().filter(|map| map.is_custom).collect();
    if custom.is_empty() {
        return Ok(Vec::new());
    }

    let manifest = CustomMapManifest::load(game_path).map_err(invalid)?;
    let mut slots = Vec::with_capacity(custom.len());
    for map in custom {
        if !manifest
            .entries()
            .iter()
            .any(|entry| entry.slot == map.slot)
        {
            return Err(invalid(format!(
                "自製地圖未登記，已取消完整還原: {}",
                map.id()
            )));
        }
        if map.directory_path.with_extension("deleting_arm").exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("地圖刪除暫存資料夾已存在: {}", map.id()),
            ));
        }
        slots.push(map.slot);
    }
    Ok(slots)
}

/// Commits the full restore and custom-map deletions as one rollback scope.
/// Map deletion continues to use the maps crate's marker, manifest, path and
/// symlink guards; the outer snapshot makes a later deletion failure restore
/// earlier deleted maps and all files changed by the restore plan.
pub fn commit_restore(
    game_path: &Path,
    plan: &PatchFilePlan,
    preserve_custom_maps: bool,
    slots: &[i32],
) -> io::Result<()> {
    let current_slots = deletion_slots(game_path, preserve_custom_maps)?;
    if current_slots != slots {
        return Err(invalid("完整還原前自製地圖清單已變更，請重新預覽後再試。"));
    }
    if slots.is_empty() {
        return commit_plan(plan);
    }

    let normalized_game = EndlessMapCatalog::validate_game_path(game_path).map_err(invalid)?;
    let catalog = EndlessMapCatalog::new();
    let mut map_dirs = Vec::with_capacity(slots.len());
    for &slot in slots {
        let map = catalog.require(&normalized_game, slot).map_err(invalid)?;
        if !map.is_custom {
            return Err(invalid(format!(
                "只能在完整還原時刪除已登記的自製地圖: {}",
                map.id()
            )));
        }
        map_dirs.push(map.directory_path);
    }

    let mut rollback = FileRollbackScope::new().map_err(invalid)?;
    let manifest_path = normalized_game
        .join("MAPS")
        .join(CustomMapManifest::FILE_NAME);
    rollback.track_file(&manifest_path).map_err(invalid)?;
    for path in plan.files.keys().chain(plan.deletes.iter()) {
        rollback.track_file(path).map_err(invalid)?;
    }
    for path in &map_dirs {
        rollback.track_directory(path).map_err(invalid)?;
    }

    let result = (|| {
        commit_plan(plan)?;
        let deleter = EndlessMapDeleter::new();
        for &slot in slots {
            deleter.delete(&normalized_game, slot).map_err(invalid)?;
        }
        Ok::<(), io::Error>(())
    })();

    match result {
        Ok(()) => {
            rollback.commit();
            Ok(())
        }
        Err(primary) => match rollback.restore_all() {
            Ok(()) => Err(primary),
            Err(rollback_error) => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("{primary}; custom-map restore rollback also failed: {rollback_error}"),
            )),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use against_rome_maps::{CustomMapEntry, CustomMapManifest};
    use chrono::Utc;
    use std::{
        collections::BTreeMap,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct TempGame(PathBuf);

    impl TempGame {
        fn new(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "arm-custom-restore-{name}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(path.join("MAPS")).unwrap();
            Self(path)
        }

        fn add_custom_map(&self, slot: i32, registered: bool) {
            let maps_path = self.0.join("MAPS");
            let map_path = maps_path.join(format!("ENDL_{slot:03}"));
            fs::create_dir_all(&map_path).unwrap();
            let entry = CustomMapEntry::new(slot, 1, Utc::now(), "test");
            fs::write(
                map_path.join(CustomMapManifest::MARKER_FILE_NAME),
                serde_json::to_vec(&entry).unwrap(),
            )
            .unwrap();
            fs::write(map_path.join("keep.dat"), b"map contents").unwrap();
            if registered {
                let mut manifest = CustomMapManifest::load(&self.0).unwrap();
                manifest.register(entry).unwrap();
                manifest.save(&self.0).unwrap();
            }
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempGame {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn preserved_maps_are_not_scheduled_for_deletion() {
        let game = TempGame::new("preserve");
        game.add_custom_map(5, true);
        assert!(deletion_slots(game.path(), true).unwrap().is_empty());
        assert!(game.path().join("MAPS/ENDL_005").is_dir());
    }

    #[test]
    fn unregistered_custom_map_rejects_full_delete_preflight() {
        let game = TempGame::new("unregistered");
        game.add_custom_map(5, false);
        assert!(deletion_slots(game.path(), false).is_err());
        assert!(game.path().join("MAPS/ENDL_005/keep.dat").is_file());
    }

    #[test]
    fn stale_delete_staging_directory_rejects_full_delete_preflight() {
        let game = TempGame::new("stale-staging");
        game.add_custom_map(5, true);
        fs::create_dir(game.path().join("MAPS/ENDL_005.deleting_arm")).unwrap();
        assert!(deletion_slots(game.path(), false).is_err());
        assert!(game.path().join("MAPS/ENDL_005/keep.dat").is_file());
    }

    #[test]
    fn full_restore_deletes_registered_maps_and_restores_files() {
        let game = TempGame::new("delete");
        game.add_custom_map(5, true);
        fs::create_dir_all(game.path().join("SYSTEM")).unwrap();
        fs::write(game.path().join("SYSTEM/value.dat"), b"patched").unwrap();
        let mut files = BTreeMap::new();
        files.insert(game.path().join("SYSTEM/value.dat"), b"original".to_vec());
        let plan = PatchFilePlan {
            files,
            deletes: Vec::new(),
        };
        let slots = deletion_slots(game.path(), false).unwrap();

        commit_restore(game.path(), &plan, false, &slots).unwrap();

        assert_eq!(
            fs::read(game.path().join("SYSTEM/value.dat")).unwrap(),
            b"original"
        );
        assert!(!game.path().join("MAPS/ENDL_005").exists());
        let manifest = CustomMapManifest::load(game.path()).unwrap();
        assert!(manifest.entries().is_empty());
    }
}
