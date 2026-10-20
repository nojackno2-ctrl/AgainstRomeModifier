use against_rome_maps::*;
use chrono::Utc;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(base: &Path, cur: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(cur).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                out.insert(path.strip_prefix(base).unwrap().to_path_buf(), fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

#[test]
fn rollback_scope_creation_error_leaves_source_and_manifest_untouched() {
    let game = std::env::temp_dir().join(format!("arm_test_del_inject_{}", Uuid::new_v4().simple()));
    let maps = game.join("MAPS");
    let map_dir = maps.join("ENDL_005");
    fs::create_dir_all(&map_dir).unwrap();
    fs::write(map_dir.join("level.put"), "var:map_name =\"Custom\";\r\n").unwrap();
    fs::write(map_dir.join("map.sdl"), "[settlement]\r\nrefpos=1.0,0.0,1.0\r\n").unwrap();

    let entry = CustomMapEntry::new(5, 1, Utc::now(), "test");
    fs::write(map_dir.join(".arm_custom_map"), serde_json::to_string(&entry).unwrap()).unwrap();
    let mut manifest = CustomMapManifest::new();
    manifest.register(entry).unwrap();
    manifest.save(&game).unwrap();

    let before = snapshot(&game);
    let manifest_path = maps.join("arm_custom_maps.json");
    let manifest_bytes = fs::read(&manifest_path).unwrap();

    let mut factory_calls = 0;
    let err = EndlessMapDeleter::new()
        .delete_with_rollback_factory(&game, 5, || {
            factory_calls += 1;
            // 工廠被呼叫時，來源必須尚未被改名或刪除。
            assert!(map_dir.is_dir(), "rollback 建立前來源目錄不應被移動");
            assert!(!maps.join("ENDL_005.deleting_arm").exists());
            Err("injected rollback scope failure".to_string())
        })
        .unwrap_err();

    assert_eq!(factory_calls, 1);
    assert!(err.contains("injected rollback scope failure"), "unexpected error: {err}");
    assert!(map_dir.is_dir());
    assert!(!maps.join("ENDL_005.deleting_arm").exists(), "不應遺留 .deleting_arm");
    assert_eq!(fs::read(&manifest_path).unwrap(), manifest_bytes);
    assert_eq!(snapshot(&game), before, "來源目錄與 manifest 應逐位元組不變");

    fs::remove_dir_all(&game).unwrap();
}
