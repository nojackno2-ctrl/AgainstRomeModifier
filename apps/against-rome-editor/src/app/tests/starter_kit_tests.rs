//! App-level tests of the Layout "Starter kit" action on an existing blank map (C#
//! `MapEditorForm.StarterKit.cs`): gate refusals, one-step undo/redo, the save-time import of the
//! main-house linked family into DATA, and the all-or-nothing rollback, driven through the real
//! `EditorApp` on synthetic temp-dir maps.

use super::spawn_save_tests::{cx, data_objects, place_house, save_ok, sync, write_data};
use super::*;
use against_rome_placement::ObjectStore;

const HOUSE: &str = "BauGerHau00_Haupthaus";
const UNIT: &str = "FigGerInf02_Zweihandaxt";

/// Game root with the starter aliases/objdefs, an official map with a linked house family and an
/// axeman, and a blank custom map `ENDL_005` (marker + exact idle scripts) loaded in the app.
fn fixture(tag: &str, live: &[(u16, Option<u16>)]) -> (EditorApp, PathBuf, PathBuf) {
    let root = temp_map_dir(&format!("starter_{tag}"));
    std::fs::create_dir_all(root.join("SYSTEM/CLAK")).unwrap();
    std::fs::create_dir_all(root.join("SYSTEM/DATA_MP/DEFAULTS")).unwrap();
    std::fs::write(
        root.join("SYSTEM/CLAK/cl_scint.ini"),
        format!("[ObjDefName]\nGER_HAU00={HOUSE}\nGER_INF02={UNIT}\n"),
    )
    .unwrap();
    let lines = [(232, HOUSE), (218, UNIT)].map(|(id, name)| {
        let mut fields = vec!["0".to_string(); 53];
        fields[0] = id.to_string();
        fields[52] = name.into();
        fields.join(",")
    });
    std::fs::write(root.join("SYSTEM/DATA_MP/DEFAULTS/objdef.dau"), lines.join("\n")).unwrap();
    write_data(&root.join("MAPS/TUTOR_01"), 6, &[(232, None), (234, Some(0)), (218, None)], 50);
    let map = root.join("MAPS/ENDL_005");
    write_data(&map, 12, live, 100);
    std::fs::create_dir_all(map.join("SCRIPT")).unwrap();
    std::fs::write(map.join(".arm_custom_map"), "{}").unwrap();
    let idle = against_rome_scenario::BciImage::idle().serialize();
    for name in ["ak_level.bci", "ak_level.arm_original"] {
        std::fs::write(map.join("SCRIPT").join(name), &idle).unwrap();
    }
    let mut app = EditorApp::initial_state();
    app.set_loaded_map(map.clone(), TerrainSession::create_blank(5, 32)).unwrap();
    app.water_surface = 0.0;
    app.scenario.load_session(&map).unwrap();
    app.spawn_ui.open = true;
    app.spawn_ui.sync_with_document(&app.scenario, Some(&map));
    (app, root, map)
}

fn kit_ok(app: &mut EditorApp) {
    assert!(app.apply_starter_kit(0, 8192.0, 8192.0), "{}", app.spawn_ui.status().0);
    assert!(!app.spawn_ui.status().1, "{}", app.spawn_ui.status().0);
}

#[test]
fn recovery_reports_pending_starter_kit_instead_of_claiming_scenario_restore() {
    let (mut app, root, map) = fixture("recovery_omitted", &[]);
    app.recovery.root = Some(root.join("editor"));
    kit_ok(&mut app);
    assert!(app.spawn_ui.pending_starter_kit().is_some());
    let pending = app.recovery_pending();
    assert!(pending["scenario"].is_null());
    assert!(pending["omitted_panels"].as_array().unwrap().iter().any(|v| v.as_str().unwrap().contains("starter kit")));
    let snapshot = crate::recovery::manifest(&map, &app.recovery.source, pending);
    assert!(!snapshot["included_parts"].as_array().unwrap().contains(&serde_json::json!("script_spawns")));
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

fn saved_scenario(map: &Path) -> against_rome_scenario::ScenarioDocument {
    against_rome_scenario::ScenarioDocument::from_json(&std::fs::read(map.join("arm_scenario.json")).unwrap()).unwrap()
}

#[test]
fn starter_kit_is_one_undo_step_and_nothing_is_written_before_save() {
    let (mut app, root, map) = fixture("undo", &[]);
    let before = save_file_snapshot(&root);
    kit_ok(&mut app);
    assert_eq!(save_file_snapshot(&root), before, "staged only");
    assert_eq!(app.spawn_ui.session().count(), 5);
    assert_eq!(app.spawn_ui.session().undo_depth(), 1, "one undo step for the whole kit");
    assert!(app.scenario.dirty());
    let doc = app.scenario.document();
    assert_eq!(doc.spawns.len(), 5);
    assert_eq!(doc.spawns[0].alias, "GER_HAU00");
    assert!(doc.spawns.iter().all(|s| s.team == 0 && s.raw_rotation.is_some() && s.count == 0));
    let (_, ids) = app.spawn_ui.pending_starter_kit().expect("kit pending");
    assert_eq!(ids.len(), 5);

    assert!(app.spawn_ui.undo(&mut app.scenario, &cx()));
    assert_eq!(app.spawn_ui.session().count(), 0);
    assert!(app.scenario.document().spawns.is_empty());
    assert!(app.spawn_ui.pending_starter_kit().is_none(), "no live kit placement, nothing to import");
    assert!(app.spawn_ui.undo(&mut app.scenario, &cx()) == false);

    assert!(app.spawn_ui.redo(&mut app.scenario, &cx()));
    assert_eq!(app.spawn_ui.session().count(), 5);
    assert!(app.spawn_ui.pending_starter_kit().is_some(), "the kit is retained across undo/redo");
    sync(&mut app, &map);
    assert_eq!(app.spawn_ui.session().count(), 5);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn gate_refuses_non_blank_maps_and_pending_edits_without_changes() {
    // DATA is not empty.
    let (mut app, root, _map) = fixture("gate_data", &[(232, None)]);
    let before = save_file_snapshot(&root);
    assert!(!app.apply_starter_kit(0, 8192.0, 8192.0));
    assert!(app.spawn_ui.status().1, "{}", app.spawn_ui.status().0);
    assert_eq!((app.spawn_ui.session().count(), app.scenario.dirty()), (0, false));
    assert_eq!(save_file_snapshot(&root), before);
    std::fs::remove_dir_all(root).unwrap();

    // The level script is not the idle bootstrap.
    let (mut app, root, map) = fixture("gate_script", &[]);
    std::fs::write(map.join("SCRIPT/ak_level.arm_original"), b"custom script").unwrap();
    assert!(!app.apply_starter_kit(0, 8192.0, 8192.0));
    assert!(app.spawn_ui.status().0.contains("閒置開局腳本"), "{}", app.spawn_ui.status().0);
    assert_eq!(app.spawn_ui.session().count(), 0);
    std::fs::remove_dir_all(root).unwrap();

    // Other staged placements must be undone first.
    let (mut app, root, _map) = fixture("gate_pending", &[]);
    place_house(&mut app, (10, 10), 0);
    assert!(!app.apply_starter_kit(0, 8192.0, 8192.0));
    assert!(app.spawn_ui.status().1);
    assert_eq!(app.spawn_ui.session().count(), 1, "the existing placement is untouched");
    assert!(app.spawn_ui.pending_starter_kit().is_none());
    std::fs::remove_dir_all(root).unwrap();

    // A second kit on top of the first is a non-blank scenario.
    let (mut app, root, _map) = fixture("gate_twice", &[]);
    kit_ok(&mut app);
    assert!(!app.apply_starter_kit(1, 4096.0, 4096.0));
    assert_eq!(app.spawn_ui.session().count(), 5);
    std::fs::remove_dir_all(root).unwrap();

    // Pending Objects (DATA) panel edits conflict with the save-time import.
    let (mut app, root, map) = fixture("gate_objects", &[(232, None)]);
    app.placement.load_session(&map).unwrap();
    app.placement.store_mut().unwrap().set_team(0, 1, 3).unwrap();
    assert!(app.placement.is_dirty());
    assert!(!app.apply_starter_kit(0, 8192.0, 8192.0));
    assert!(app.spawn_ui.status().0.contains("Objects (DATA)"), "{}", app.spawn_ui.status().0);
    assert_eq!(app.spawn_ui.session().count(), 0);
    std::fs::remove_dir_all(root).unwrap();

    // Invalid terrain (the kit would sit in water).
    let (mut app, root, _map) = fixture("gate_water", &[]);
    app.water_surface = 1000.0;
    assert!(!app.apply_starter_kit(0, 8192.0, 8192.0));
    assert!(app.spawn_ui.status().0.contains("水面"), "{}", app.spawn_ui.status().0);
    assert_eq!(app.spawn_ui.session().count(), 0);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn safe_save_imports_the_main_house_family_and_binds_all_five() {
    let (mut app, root, map) = fixture("save", &[]);
    assert!(app.apply_starter_kit(3, 8192.0, 8192.0), "{}", app.spawn_ui.status().0);
    save_ok(&mut app);

    let objects = data_objects(&map);
    assert_eq!(objects.len(), 6, "linked house family (232 + child 234) and 4 axemen");
    assert!(objects.iter().any(|o| o.type_id == 234));
    assert!(objects.iter().all(|o| o.team == 3));
    let saved = saved_scenario(&map);
    assert_eq!((saved.spawns.len(), saved.data_slots.len()), (5, 5));
    assert!(saved.spawns.iter().all(|s| s.prebuilt && s.raw_rotation.is_some()));
    let house = saved.spawns.iter().find(|s| s.alias == "GER_HAU00").unwrap();
    let binding = saved.data_slots.iter().find(|b| b.spawn_id == house.id).unwrap();
    let root_object = objects.iter().find(|o| o.slot as i32 == binding.slot).unwrap();
    assert_eq!((root_object.position[0], root_object.position[2]), (8192.0, 8192.0));
    assert_eq!(
        ObjectStore::load(&map).unwrap().family(binding.slot as usize, binding.uid).unwrap().len(),
        2
    );

    assert!(!app.scenario.dirty());
    sync(&mut app, &map);
    assert!(!app.spawn_ui.session().is_dirty());
    assert!(app.spawn_ui.pending_starter_kit().is_none(), "the import is done, nothing pending");
    assert_eq!(app.scenario.document().data_slots.len(), 5);
    // A saved map is no longer blank: a second kit is refused.
    assert!(!app.apply_starter_kit(0, 4096.0, 4096.0));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn moved_kit_is_translated_when_saved() {
    let (mut app, root, map) = fixture("moved", &[]);
    kit_ok(&mut app);
    app.spawn_ui.select_all();
    app.spawn_ui.move_selected(&mut app.scenario, &cx(), 512.0, 0.0, false);
    assert!(!app.spawn_ui.status().1, "{}", app.spawn_ui.status().0);
    save_ok(&mut app);
    let saved = saved_scenario(&map);
    let house = saved.spawns.iter().find(|s| s.alias == "GER_HAU00").unwrap();
    let binding = saved.data_slots.iter().find(|b| b.spawn_id == house.id).unwrap();
    let objects = data_objects(&map);
    let root_object = objects.iter().find(|o| o.slot as i32 == binding.slot).unwrap();
    assert_eq!(root_object.position[0], 8192.0 + 512.0);
    assert_eq!(objects.len(), 6);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_save_rolls_everything_back_and_keeps_the_staged_kit() {
    use against_rome_maps::TransactionFailurePoint as Fail;
    for point in [Fail::AfterPath("DATA/objects.dat".into()), Fail::BeforeFinalCommit] {
        let (mut app, root, map) = fixture("rollback", &[]);
        kit_ok(&mut app);
        let before = save_file_snapshot(&root);
        app.safe_save_current_using(|tx| {
            let mut injected = std::mem::replace(tx, against_rome_maps::MapSaveTransaction::new("unused"))
                .with_injected_failure(point, "starter kit rollback test");
            injected.commit().map_err(|e| e.to_string())
        });
        assert!(app.status_is_error, "{}", app.status_message);
        assert_eq!(save_file_snapshot(&root), before, "DATA, scenario and script are all untouched");
        assert!(app.scenario.dirty() && app.spawn_ui.session().is_dirty());
        assert!(app.spawn_ui.pending_starter_kit().is_some(), "the kit stays staged");
        assert!(data_objects(&map).is_empty());
        // The same staged kit saves cleanly afterwards.
        save_ok(&mut app);
        assert_eq!(data_objects(&map).len(), 6);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn save_time_gate_and_terrain_failures_write_nothing() {
    // The map stopped being blank after staging (e.g. objects were added on disk).
    let (mut app, root, map) = fixture("save_gate", &[]);
    kit_ok(&mut app);
    write_data(&map, 12, &[(232, None)], 100);
    let before = save_file_snapshot(&root);
    app.safe_save_current();
    assert!(app.status_is_error, "{}", app.status_message);
    assert!(app.status_message.contains("起始配置"), "{}", app.status_message);
    assert_eq!(save_file_snapshot(&root), before);
    assert!(app.scenario.dirty() && app.spawn_ui.pending_starter_kit().is_some());
    std::fs::remove_dir_all(root).unwrap();

    // Terrain became wet after staging: the final candidate is validated again.
    let (mut app, root, map) = fixture("save_water", &[]);
    kit_ok(&mut app);
    app.water_surface = 1000.0;
    let before = save_file_snapshot(&root);
    app.safe_save_current();
    assert!(app.status_is_error, "{}", app.status_message);
    assert!(app.status_message.contains("水面"), "{}", app.status_message);
    assert_eq!(save_file_snapshot(&root), before);
    assert!(data_objects(&map).is_empty());
    app.water_surface = 0.0;
    save_ok(&mut app);
    assert_eq!(data_objects(&map).len(), 6);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn deleting_every_kit_placement_drops_the_pending_import() {
    let (mut app, root, map) = fixture("deleted", &[]);
    kit_ok(&mut app);
    app.spawn_ui.select_all();
    app.spawn_ui.delete_selected(&mut app.scenario, &cx());
    assert!(!app.spawn_ui.status().1, "{}", app.spawn_ui.status().0);
    assert!(app.spawn_ui.pending_starter_kit().is_none());
    assert!(!app.scenario.dirty(), "back to the saved (empty) scenario");
    assert!(app.plan_spawn_data_for_save(&map).unwrap().is_none(), "no kit family is imported");
    assert!(data_objects(&map).is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn deleting_part_of_the_kit_imports_it_and_removes_the_deleted_families() {
    let (mut app, root, map) = fixture("partial", &[]);
    kit_ok(&mut app);
    // Delete only the main house (picked on the canvas); the four axemen stay.
    for dx in [-480.0, -160.0, 160.0, 480.0] {
        assert!(app.spawn_ui.canvas_press((8192.0 + dx, 8992.0), 10.0, true, false), "toggle the axeman off");
        app.spawn_ui.cancel_drag();
    }
    assert_eq!(app.spawn_ui.selection(), vec![0], "the house marker was picked");
    app.spawn_ui.delete_selected(&mut app.scenario, &cx());
    assert_eq!(app.spawn_ui.session().count(), 4);
    assert!(app.spawn_ui.pending_starter_kit().is_some(), "four kit placements are still alive");
    save_ok(&mut app);
    let objects = data_objects(&map);
    assert_eq!(objects.len(), 4, "four axemen kept, the deleted house family is not imported");
    assert_eq!(saved_scenario(&map).spawns.len(), 4);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn dialog_request_is_taken_once_in_world_units() {
    let (mut app, root, _map) = fixture("dialog", &[]);
    assert!(!app.spawn_ui.has_open_dialog());
    app.spawn_ui.set_placement_values(2, 1, 0);
    app.spawn_ui.open_starter_dialog();
    assert!(app.spawn_ui.has_open_dialog());
    assert!(app.spawn_ui.take_starter_request().is_none(), "nothing until the dialog is confirmed");
    std::fs::remove_dir_all(root).unwrap();
}
