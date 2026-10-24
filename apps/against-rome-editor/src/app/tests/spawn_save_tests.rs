//! App-level tests of the spawn panel's save-time DATA transaction (C# `TrySaveMap` prebuilt
//! block): prebuilt conversion of new buildings, bound-spawn edits, family copies and the
//! all-or-nothing rollback, driven through the real `EditorApp` Safe Save on synthetic maps.

use super::*;
use against_rome_placement::{ObjectStore, COLUMNS, SEGMENTS};

const HOUSE: &str = "BauGerHau00_Haupthaus";

pub(in crate::app) fn write_data(map: &Path, n: usize, live: &[(u16, Option<u16>)], cursor: u32) {
    std::fs::create_dir_all(map.join("DATA")).unwrap();
    let mut o = vec![0u8; 16 + n * (79 + COLUMNS.iter().sum::<usize>())];
    for (i, v) in [1u32, n as u32, 30, 30].iter().enumerate() {
        o[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    let mut p = vec![0u8; 8 + n * 3 * 17];
    p[..4].copy_from_slice(&1u32.to_le_bytes());
    p[4..8].copy_from_slice(&((n * 3) as u32).to_le_bytes());
    let mut d = vec![0u8; 8 + n * SEGMENTS.iter().sum::<usize>()];
    d[..4].copy_from_slice(&1u32.to_le_bytes());
    d[4..8].copy_from_slice(&(n as u32).to_le_bytes());
    for s in 0..n {
        let r = 16 + s * 79;
        for off in [67, 69, 71, 73, 77] {
            o[r + off..r + off + 2].fill(255);
        }
        for c in 0..18 {
            if COLUMNS[c] == 2 {
                let off = 16 + n * 79 + n * COLUMNS[..c].iter().sum::<usize>() + s * COLUMNS[c];
                o[off..off + 2].fill(255);
            }
        }
        if let Some(&(typ, parent)) = live.get(s) {
            o[r] = 1;
            o[r + 3..r + 7].copy_from_slice(&(s as u32 + 1).to_le_bytes());
            o[r + 75..r + 77].copy_from_slice(&typ.to_le_bytes());
            for off in [67, 69] {
                o[r + off..r + off + 2].copy_from_slice(&(s as u16).to_le_bytes());
            }
            for (c, v) in [(1, s as u16), (13, parent.unwrap_or(65535))] {
                let off = 16 + n * 79 + n * COLUMNS[..c].iter().sum::<usize>() + s * COLUMNS[c];
                o[off..off + 2].copy_from_slice(&v.to_le_bytes());
            }
            let off = 8 + s * 17;
            p[off] = 1;
            for (a, v) in [4000. + s as f32 * 20., 20., 4000.].iter().enumerate() {
                p[off + 1 + a * 4..off + 5 + a * 4].copy_from_slice(&v.to_le_bytes());
            }
            p[off + 13..off + 17].copy_from_slice(&0f32.to_le_bytes());
            d[8 + s * 9] = 1;
        }
    }
    let mut l = vec![0u8; against_rome_data::pool::PAYLOAD_LEN];
    l[..4].copy_from_slice(&1u32.to_le_bytes());
    l[4..8].copy_from_slice(&1024u32.to_le_bytes());
    let mut e = vec![0u8; 94];
    e[..4].copy_from_slice(&1u32.to_le_bytes());
    e[34..38].copy_from_slice(&cursor.to_le_bytes());
    for (name, b) in [("objects", o), ("position", p), ("objdata", d), ("light", l), ("engine", e)] {
        std::fs::write(map.join(format!("DATA/{name}.dat")), b).unwrap();
    }
}

/// Game root with alias + objdef catalogs, one official map holding a house template, and a
/// custom map `ENDL_005` with an idle level script.
pub(in crate::app) fn fixture(tag: &str, live: &[(u16, Option<u16>)]) -> (EditorApp, PathBuf, PathBuf) {
    let root = temp_map_dir(&format!("spawn_save_{tag}"));
    std::fs::create_dir_all(root.join("SYSTEM/CLAK")).unwrap();
    std::fs::create_dir_all(root.join("SYSTEM/DATA_MP/DEFAULTS")).unwrap();
    std::fs::write(
        root.join("SYSTEM/CLAK/cl_scint.ini"),
        format!("[ObjDefName]\nHOUSE={HOUSE}\nOTHER=BauRomOther\nTROOP=FigGerInf02_Zweihandaxt\n"),
    )
    .unwrap();
    let mut columns = vec!["0".to_string(); 53];
    columns[0] = "232".into();
    columns[1] = "1".into();
    columns[52] = HOUSE.into();
    std::fs::write(
        root.join("SYSTEM/DATA_MP/DEFAULTS/objdef.dau"),
        format!("[ObjectDefaults]\n{}\n", columns.join(",")),
    )
    .unwrap();
    write_data(&root.join("MAPS/OFFICIAL_A"), 6, &[(232, None)], 50);
    let map = root.join("MAPS/ENDL_005");
    write_data(&map, 12, live, 100);
    std::fs::create_dir_all(map.join("SCRIPT")).unwrap();
    std::fs::write(map.join("SCRIPT/ak_level.bci"), against_rome_scenario::BciImage::idle().serialize()).unwrap();
    let mut app = EditorApp::initial_state();
    app.set_loaded_map(map.clone(), TerrainSession::create_blank(5, 32)).unwrap();
    app.scenario.load_session(&map).unwrap();
    app.spawn_ui.open = true;
    app.spawn_ui.sync_with_document(&app.scenario, Some(&map));
    (app, root, map)
}

pub(in crate::app) fn cx() -> SpawnContext<'static> {
    SpawnContext {
        lang: AppLanguage::English,
        heights: None,
        vertex_size: 0,
        height_step: 4.0,
        read_only: false,
    }
}

pub(in crate::app) fn place_house(app: &mut EditorApp, tile: (i32, i32), team: i32) {
    app.spawn_ui.select_type(HOUSE);
    app.spawn_ui.set_placement_values(team, 1, 0);
    app.spawn_ui.place_at_tile(&mut app.scenario, &cx(), tile.0, tile.1);
    assert!(!app.spawn_ui.status().1, "{}", app.spawn_ui.status().0);
}

pub(in crate::app) fn sync(app: &mut EditorApp, map: &Path) {
    app.spawn_ui.sync_with_document(&app.scenario, Some(map));
}

pub(in crate::app) fn data_objects(map: &Path) -> Vec<against_rome_placement::WorldObject> {
    ObjectStore::load(map).unwrap().objects()
}

pub(in crate::app) fn save_ok(app: &mut EditorApp) {
    app.safe_save_current();
    assert!(!app.status_is_error, "{}", app.status_message);
}

#[test]
fn new_building_is_saved_as_a_prebuilt_data_object_with_binding() {
    let (mut app, root, map) = fixture("prebuilt", &[]);
    place_house(&mut app, (30, 30), 0);
    assert!(!app.scenario.document().spawns[0].prebuilt, "script spawn while editing");
    save_ok(&mut app);

    let objects = data_objects(&map);
    assert_eq!(objects.len(), 1);
    assert_eq!((objects[0].type_id, objects[0].team), (232, 0));
    assert_eq!((objects[0].position[0], objects[0].position[2]), (7808.0, 7808.0));
    let saved = against_rome_scenario::ScenarioDocument::from_json(&std::fs::read(map.join("arm_scenario.json")).unwrap()).unwrap();
    assert_eq!(saved.spawns.len(), 1);
    assert!(saved.spawns[0].prebuilt);
    assert_eq!(saved.data_slots.len(), 1);
    assert_eq!((saved.data_slots[0].slot, saved.data_slots[0].uid), (objects[0].slot as i32, objects[0].uid));
    assert!(!app.scenario.dirty());
    assert_eq!(app.scenario.document().data_slots.len(), 1, "baseline reloaded from the saved files");
    sync(&mut app, &map);
    assert!(!app.spawn_ui.session().is_dirty());
    assert!(app.spawn_ui.is_data_bound_index(0));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_commit_after_data_staging_writes_nothing_and_keeps_the_edit() {
    use against_rome_maps::TransactionFailurePoint as Fail;
    for point in [Fail::AfterPath("DATA/objects.dat".into()), Fail::BeforeFinalCommit] {
        let (mut app, root, map) = fixture("rollback", &[]);
        place_house(&mut app, (20, 20), 1);
        let before = save_file_snapshot(&root);
        app.safe_save_current_using(|tx| {
            let mut injected = std::mem::replace(tx, against_rome_maps::MapSaveTransaction::new("unused"))
                .with_injected_failure(point, "spawn data rollback test");
            injected.commit().map_err(|e| e.to_string())
        });
        assert!(app.status_is_error, "{}", app.status_message);
        assert_eq!(save_file_snapshot(&root), before, "DATA, scenario and script are all untouched");
        assert!(app.scenario.dirty() && app.spawn_ui.session().is_dirty(), "the edit is kept");
        assert!(data_objects(&map).is_empty());
        // The same edit saves cleanly afterwards.
        save_ok(&mut app);
        assert_eq!(data_objects(&map).len(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn buildings_outside_teams_0_to_8_stay_script_spawns() {
    let (mut app, root, map) = fixture("scriptteam", &[]);
    // A building on team 12 is script-created and never touches DATA.
    place_house(&mut app, (10, 10), 12);
    let before = save_file_snapshot(&map.join("DATA"));
    save_ok(&mut app);
    assert_eq!(save_file_snapshot(&map.join("DATA")), before, "no DATA transaction for script spawns");
    let saved = against_rome_scenario::ScenarioDocument::from_json(&std::fs::read(map.join("arm_scenario.json")).unwrap()).unwrap();
    assert!(!saved.spawns[0].prebuilt && saved.data_slots.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn bound_spawn_is_editable_and_moves_its_data_object() {
    let (mut app, root, map) = fixture("move", &[]);
    place_house(&mut app, (30, 30), 0);
    save_ok(&mut app);
    sync(&mut app, &map);

    app.spawn_ui.select_all();
    app.spawn_ui.move_selected(&mut app.scenario, &cx(), 512.0, 0.0, false);
    assert!(!app.spawn_ui.status().1, "{}", app.spawn_ui.status().0);
    save_ok(&mut app);
    let objects = data_objects(&map);
    assert_eq!(objects.len(), 1, "moved, not duplicated");
    assert_eq!(objects[0].position[0], 7808.0 + 512.0);
    let saved = against_rome_scenario::ScenarioDocument::from_json(&std::fs::read(map.join("arm_scenario.json")).unwrap()).unwrap();
    assert_eq!(saved.spawns[0].x, 7808.0 + 512.0);
    assert_eq!(saved.data_slots.len(), 1);

    // Team and direction of an ordinary (singleton) bound building are rebuilt from the template.
    sync(&mut app, &map);
    app.spawn_ui.select_all();
    app.spawn_ui.apply_batch(&mut app.scenario, &cx(), Some(3), Some(90.0));
    assert!(!app.spawn_ui.status().1, "{}", app.spawn_ui.status().0);
    save_ok(&mut app);
    let objects = data_objects(&map);
    assert_eq!((objects.len(), objects[0].team), (1, 3));
    assert!((objects[0].rotation_degrees - 90.0).abs() < 0.01);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn duplicating_and_deleting_bound_spawns_copies_and_removes_data() {
    let (mut app, root, map) = fixture("copy", &[]);
    place_house(&mut app, (30, 30), 0);
    save_ok(&mut app);
    sync(&mut app, &map);

    app.spawn_ui.select_all();
    app.spawn_ui.duplicate_selected(&mut app.scenario, &cx());
    assert!(!app.spawn_ui.status().1, "{}", app.spawn_ui.status().0);
    save_ok(&mut app);
    let objects = data_objects(&map);
    assert_eq!(objects.len(), 2, "the duplicate is a second DATA object");
    let saved = against_rome_scenario::ScenarioDocument::from_json(&std::fs::read(map.join("arm_scenario.json")).unwrap()).unwrap();
    assert_eq!((saved.spawns.len(), saved.data_slots.len()), (2, 2));
    assert!(saved.spawns.iter().all(|s| s.prebuilt));

    sync(&mut app, &map);
    app.spawn_ui.select_all();
    app.spawn_ui.delete_selected(&mut app.scenario, &cx());
    assert!(!app.spawn_ui.status().1, "{}", app.spawn_ui.status().0);
    save_ok(&mut app);
    assert!(data_objects(&map).is_empty(), "deleting the spawns removes their DATA objects");
    let saved = against_rome_scenario::ScenarioDocument::from_json(&std::fs::read(map.join("arm_scenario.json")).unwrap()).unwrap();
    assert!(saved.spawns.is_empty() && saved.data_slots.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn pending_objects_panel_edits_block_spawn_data_changes_without_writing() {
    let (mut app, root, map) = fixture("conflict", &[(232, None)]);
    app.placement.load_session(&map).unwrap();
    app.placement.store_mut().unwrap().set_team(0, 1, 3).unwrap();
    assert!(app.placement.is_dirty());
    place_house(&mut app, (30, 30), 0);
    let before = save_file_snapshot(&root);
    app.safe_save_current();
    assert!(app.status_is_error, "{}", app.status_message);
    assert!(app.status_message.contains("Spawn placement DATA"), "{}", app.status_message);
    assert_eq!(save_file_snapshot(&root), before);
    assert!(app.scenario.dirty() && app.placement.is_dirty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn landscape_points_follow_nature_edits_and_feed_the_spawn_panel() {
    let (mut app, root, map) = fixture("landscape", &[]);
    app.refresh_spawn_landscape(Some(&map));
    assert_eq!(app.spawn_ui.landscape_count(), 0, "no Lan* objects on the map");
    app.spawn_landscape = Some((map.clone(), vec![(5, 1000.0, 1000.0), (6, 3000.0, 3000.0)]));
    app.refresh_spawn_landscape(Some(&map));
    assert_eq!(app.spawn_ui.landscape_count(), 2);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn building_without_any_template_falls_back_to_a_construction_site() {
    let (mut app, root, map) = fixture("notemplate", &[]);
    // BauRomOther has no objdef id and no official/current template: script creation fallback.
    app.spawn_ui.select_type("BauRomOther");
    app.spawn_ui.set_placement_values(0, 1, 0);
    app.spawn_ui.place_at_tile(&mut app.scenario, &cx(), 12, 12);
    assert!(!app.spawn_ui.status().1, "{}", app.spawn_ui.status().0);
    let before = save_file_snapshot(&map.join("DATA"));
    save_ok(&mut app);
    assert_eq!(save_file_snapshot(&map.join("DATA")), before);
    let saved = against_rome_scenario::ScenarioDocument::from_json(&std::fs::read(map.join("arm_scenario.json")).unwrap()).unwrap();
    assert_eq!(saved.spawns.len(), 1);
    assert!(!saved.spawns[0].prebuilt && saved.data_slots.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}
