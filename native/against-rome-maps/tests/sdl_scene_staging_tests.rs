use against_rome_maps::*;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const SDL: &str = "[settlement]\r\nrefpos=1000.0,50.0,2000.0\r\n\r\n[object0000]\r\ndef=1700\r\nnamedef=BauRomHau00_Haupthaus\r\npos=100.00,10.00,100.00\r\nteam=0\r\nangle=0.00\r\n";
const SDL_NAME: &str = "Endlos_01_Siedlung01.sdl";

fn fixture() -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("arm_sdl_stage_{}", Uuid::new_v4().simple()));
    let map_dir = root.join("MAPS").join("ENDL_005");
    fs::create_dir_all(&map_dir).unwrap();
    fs::write(map_dir.join(CustomMapManifest::MARKER_FILE_NAME), b"").unwrap();
    fs::write(map_dir.join(SDL_NAME), SDL).unwrap();
    (root, map_dir)
}

fn scene_object() -> MapSceneObject {
    MapSceneObject {
        name: "BauRomHau00_Haupthaus".into(),
        world_x: 0.0,
        world_y: 0.0,
        world_z: 0.0,
        team: 0,
        source_file: SDL_NAME.into(),
        object_index: 0,
        local_x: 100.0,
        local_y: 10.0,
        local_z: 100.0,
        angle: Some(0.0),
        has_native_saved_position: false,
    }
}

fn read(dir: &Path) -> Vec<u8> {
    fs::read(dir.join(SDL_NAME)).unwrap()
}

#[test]
fn stages_changed_file_bytes_without_touching_disk_then_commits() {
    let (root, map_dir) = fixture();
    let saved = vec![scene_object()];
    let mut moved = scene_object();
    moved.team = 3;
    moved.local_x = 150.5;
    let current = vec![moved];

    let staged =
        SdlSceneEditService::stage_changes(&map_dir, &saved, &current, None, None, None).unwrap();
    assert_eq!(staged.len(), 1);
    assert_eq!(staged[0].relative_path, SDL_NAME);
    let text = String::from_utf8(staged[0].bytes.clone()).unwrap();
    assert!(text.contains("team=3"), "{text}");
    assert!(text.contains("pos=150.50,10.00,100.00"), "{text}");
    assert_eq!(read(&map_dir), SDL.as_bytes(), "staging must not write");

    let mut tx = MapSaveTransaction::for_custom_map(&map_dir, None).unwrap();
    tx.stage_file(&staged[0].relative_path, staged[0].bytes.clone())
        .unwrap();
    tx.commit().unwrap();
    assert_eq!(read(&map_dir), staged[0].bytes);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn save_changes_writes_staged_bytes() {
    let (root, map_dir) = fixture();
    let saved = vec![scene_object()];
    let mut moved = scene_object();
    moved.team = 2;
    let current = vec![moved];

    let staged =
        SdlSceneEditService::stage_changes(&map_dir, &saved, &current, None, None, None).unwrap();
    SdlSceneEditService::save_changes(&map_dir, &saved, &current, None, None, None).unwrap();
    assert_eq!(read(&map_dir), staged[0].bytes);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn no_op_stages_nothing_and_leaves_disk_unchanged() {
    let (root, map_dir) = fixture();
    let objects = vec![scene_object()];

    let staged =
        SdlSceneEditService::stage_changes(&map_dir, &objects, &objects, None, None, None).unwrap();
    assert!(staged.is_empty());
    SdlSceneEditService::save_changes(&map_dir, &objects, &objects, None, None, None).unwrap();
    assert_eq!(read(&map_dir), SDL.as_bytes());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn validation_failure_leaves_disk_unchanged() {
    let (root, map_dir) = fixture();
    let saved = vec![scene_object()];
    let mut bad = scene_object();
    bad.team = 99;
    let current = vec![bad];

    assert!(
        SdlSceneEditService::stage_changes(&map_dir, &saved, &current, None, None, None).is_err()
    );
    assert!(
        SdlSceneEditService::save_changes(&map_dir, &saved, &current, None, None, None).is_err()
    );
    assert_eq!(read(&map_dir), SDL.as_bytes());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn serialization_failure_leaves_disk_unchanged() {
    let (root, map_dir) = fixture();
    let objects = vec![scene_object()];
    // The translation edits the document first; the addition's missing template then fails.
    let translations = [SdlSettlementTranslation {
        source_file: SDL_NAME.into(),
        delta_x: 1.0,
        delta_y: 0.0,
        delta_z: 0.0,
    }];
    let additions = [SdlSceneObjectAddition {
        source_file: SDL_NAME.into(),
        template_index: 7,
        team: 0,
        local_x: 0.0,
        local_y: 0.0,
        local_z: 0.0,
        angle: None,
        target_file: None,
    }];

    let result = SdlSceneEditService::save_changes(
        &map_dir,
        &objects,
        &objects,
        None,
        Some(&additions),
        Some(&translations),
    );
    assert!(result.is_err());
    assert_eq!(read(&map_dir), SDL.as_bytes());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn additions_from_same_template_apply_values_independently() {
    let (root, map_dir) = fixture();
    let objects = vec![scene_object()];
    let addition = |team, x, angle| SdlSceneObjectAddition {
        source_file: SDL_NAME.into(),
        template_index: 0,
        team,
        local_x: x,
        local_y: 10.0,
        local_z: 20.0,
        angle,
        target_file: None,
    };
    let additions = [addition(3, 11.0, Some(45.0)), addition(5, 22.0, Some(90.0))];

    let staged = SdlSceneEditService::stage_changes(
        &map_dir,
        &objects,
        &objects,
        None,
        Some(&additions),
        None,
    )
    .unwrap();
    let doc = SdlDocument::from_text(&String::from_utf8(staged[0].bytes.clone()).unwrap());
    let objs = doc.objects();
    assert_eq!(objs.len(), 3);
    for (sec, (team, pos, angle)) in objs[1..].iter().zip([
        ("3", "11.00,10.00,20.00", "45.00"),
        ("5", "22.00,10.00,20.00", "90.00"),
    ]) {
        assert_eq!(sec.get_value("team"), Some(team));
        assert_eq!(sec.get_value("pos"), Some(pos));
        assert_eq!(sec.get_value("angle"), Some(angle));
        assert_eq!(sec.get_value("namedef"), Some("BauRomHau00_Haupthaus"));
    }

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn angle_some_to_none_removes_angle_key_only() {
    let (root, map_dir) = fixture();
    let saved = vec![scene_object()];
    let mut cleared = scene_object();
    cleared.angle = None;
    cleared.team = 2;

    let staged =
        SdlSceneEditService::stage_changes(&map_dir, &saved, &[cleared], None, None, None).unwrap();
    let text = String::from_utf8(staged[0].bytes.clone()).unwrap();
    assert_eq!(
        text,
        "[settlement]\r\nrefpos=1000.0,50.0,2000.0\r\n\r\n[object0000]\r\ndef=1700\r\nnamedef=BauRomHau00_Haupthaus\r\npos=100.00,10.00,100.00\r\nteam=2\r\n"
    );
    assert!(SdlDocument::from_text(&text).objects()[0]
        .get_value("angle")
        .is_none());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn remove_object_value_preserves_comments_order_and_other_sections() {
    let text = "; head\n[object0000]\nangle=1\n[object0001]\ndef=1\n; note\nAngle = 5.00\npos=1,2,3\nteam=0";
    let mut doc = SdlDocument::from_text(text);
    doc.remove_object_value(1, "angle").unwrap();
    assert_eq!(
        doc.text(),
        "; head\n[object0000]\nangle=1\n[object0001]\ndef=1\n; note\npos=1,2,3\nteam=0"
    );
    doc.remove_object_value(1, "angle").unwrap();
    assert!(doc.remove_object_value(9, "angle").is_err());
}
