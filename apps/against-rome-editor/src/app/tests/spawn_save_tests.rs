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
    std::fs::write(
        map.join(against_rome_maps::CustomMapManifest::MARKER_FILE_NAME),
        "test fixture",
    )
    .unwrap();
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

fn install_two_object_family_with_light(map: &Path, bound_uid: u32) -> uuid::Uuid {
    // Root slot 0 has a child in slot 1; the child's direct light link points at light slot 0.
    write_data(map, 12, &[(232, None), (233, Some(0))], 100);
    let mut objects = std::fs::read(map.join("DATA/objects.dat")).unwrap();
    objects[16 + 3..16 + 7].copy_from_slice(&bound_uid.to_le_bytes());
    let light_column = 16 + 12 * 79 + 12 * COLUMNS[..12].iter().sum::<usize>();
    objects[light_column + 2..light_column + 4].copy_from_slice(&0u16.to_le_bytes());
    std::fs::write(map.join("DATA/objects.dat"), objects).unwrap();

    let mut light = std::fs::read(map.join("DATA/light.dat")).unwrap();
    let widths = against_rome_data::pool::WIDTHS;
    let column = |index: usize| 8 + 1024 * widths[..index].iter().sum::<usize>();
    light[column(0)..column(0) + 4].copy_from_slice(&1u32.to_le_bytes());
    for (index, value) in [(1, 4021.0f32), (2, 28.0), (3, 4002.0)] {
        let offset = column(index);
        light[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (index, value) in [(13, 1i16), (14, -1), (15, -1)] {
        let offset = column(index);
        light[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    std::fs::write(map.join("DATA/light.dat"), light).unwrap();

    let mut document = against_rome_scenario::ScenarioDocument::from_json(
        &std::fs::read(map.join("arm_scenario.json")).unwrap(),
    )
    .unwrap();
    document.spawns[0].x = 4000.0;
    document.spawns[0].z = 4000.0;
    let root_id = document.spawns[0].id;
    std::fs::write(map.join("arm_scenario.json"), document.to_json().unwrap()).unwrap();
    root_id
}

fn install_two_object_family_with_particle_light(map: &Path, bound_uid: u32) -> uuid::Uuid {
    use against_rome_data::particle::{CAPACITY, CHILDREN, PAYLOAD_LEN};

    let root_id = install_two_object_family_with_light(map, bound_uid);
    let mut objects = std::fs::read(map.join("DATA/objects.dat")).unwrap();
    let column_offset = |column: usize| 16 + 12 * 79 + 12 * COLUMNS[..column].iter().sum::<usize>();
    // The root owns emitter 0; the child has no object-direct light backlink.
    objects[column_offset(11)..column_offset(11) + 2].copy_from_slice(&0u16.to_le_bytes());
    objects[column_offset(12) + 2..column_offset(12) + 4].copy_from_slice(&u16::MAX.to_le_bytes());
    std::fs::write(map.join("DATA/objects.dat"), objects).unwrap();

    let mut particle = vec![0u8; PAYLOAD_LEN];
    particle[..4].copy_from_slice(&1i32.to_le_bytes());
    particle[4..8].copy_from_slice(&(CAPACITY as i32).to_le_bytes());
    particle[8..12].copy_from_slice(&(CHILDREN as i32).to_le_bytes());
    let record = 12;
    particle[record..record + 2].copy_from_slice(&1u16.to_le_bytes());
    particle[record + 2..record + 6].copy_from_slice(&1i32.to_le_bytes());
    particle[record + 6..record + 8].copy_from_slice(&0i16.to_le_bytes());
    let child = record + 12;
    particle[child] = 1;
    for (axis, value) in [4021.75f32, 28.5, 4002.5].into_iter().enumerate() {
        let fixed = (value * 65536.0) as i32;
        let offset = child + 1 + axis * 4;
        particle[offset..offset + 4].copy_from_slice(&fixed.to_le_bytes());
    }
    particle[child + 33..child + 35].copy_from_slice(&0i16.to_le_bytes());
    std::fs::write(map.join("DATA/particle.dat"), particle).unwrap();

    // Particle-linked lights have no object owner/parent; the emitter child's link owns them.
    let mut light = std::fs::read(map.join("DATA/light.dat")).unwrap();
    let widths = against_rome_data::pool::WIDTHS;
    let column = |index: usize| 8 + 1024 * widths[..index].iter().sum::<usize>();
    for (index, value) in [(1, 4021.75f32), (2, 28.5), (3, 4002.5)] {
        let offset = column(index);
        light[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (index, value) in [(13, -1i16), (14, -1), (15, -1)] {
        let offset = column(index);
        light[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    std::fs::write(map.join("DATA/light.dat"), light).unwrap();
    root_id
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
fn prebuilt_spawn_and_start_camera_share_one_safe_save() {
    let (mut app, root, map) = fixture("prebuilt_with_camera", &[]);
    place_house(&mut app, (30, 30), 0);
    let spawn_id = app.scenario.document().spawns[0].id;
    app.scenario.add_events(&[against_rome_scenario::Event {
        name: "主屋區域勝利".into(),
        conditions: vec![against_rome_scenario::Condition {
            kind: 2, target_id: spawn_id, min_x: 7000, min_z: 7000,
            max_x: 9000, max_z: 9000, ..Default::default()
        }],
        actions: vec![against_rome_scenario::Action { kind: 3, ..Default::default() }],
        ..Default::default()
    }]).unwrap();
    app.campaign.start_camera.enabled = true;
    app.apply_start_camera_candidate().unwrap();
    let plan = app.preview_spawn_data_for_save(&map).unwrap().unwrap();
    let staged = app.scenario.stage_transaction_files_with(&map, Some((&plan.scenario, &plan.active))).unwrap();
    assert!(app.campaign.stage_camera_candidate_with_active(&map, &staged, app.scenario.known_aliases(), Some(&[])).is_err(), "候選 DATA 缺少綁定時不得編譯");
    let original_data = std::fs::read(map.join("DATA/objects.dat")).unwrap();
    let original_script = std::fs::read(map.join("SCRIPT/ak_level.bci")).unwrap();
    let blockers = app.save_blockers();
    assert!(blockers.is_empty(), "{blockers:?}");

    app.safe_save_current_using(|_| Err("模擬交易失敗".into()));
    assert!(app.status_is_error);
    assert!(app.campaign.camera_pending() && app.scenario.dirty());
    assert_eq!(std::fs::read(map.join("DATA/objects.dat")).unwrap(), original_data);
    assert_eq!(std::fs::read(map.join("SCRIPT/ak_level.bci")).unwrap(), original_script);
    assert!(!map.join("arm_scenario.json").exists());

    save_ok(&mut app);
    assert!(!app.campaign.camera_pending() && !app.scenario.dirty());
    let saved = against_rome_scenario::ScenarioDocument::from_json(&std::fs::read(map.join("arm_scenario.json")).unwrap()).unwrap();
    let binding = saved.data_slots.iter().find(|b| b.spawn_id == spawn_id).unwrap();
    assert!(saved.spawns.iter().any(|s| s.id == spawn_id && s.prebuilt));
    assert!(data_objects(&map).iter().any(|o| o.slot as i32 == binding.slot && o.uid == binding.uid));
    let camera: against_rome_scenario::StartCamera = serde_json::from_slice(&std::fs::read(map.join("arm_start_camera.json")).unwrap()).unwrap();
    assert!(camera.enabled);
    let script = std::fs::read(map.join("SCRIPT/ak_level.bci")).unwrap();
    against_rome_campaign::export::verify_exported_script(&original_script, &script, &saved, Some(&camera)).unwrap();
    let mut reloaded = EditorApp::initial_state();
    reloaded.set_loaded_map(map.clone(), crate::storage::load_map(&map).unwrap()).unwrap();
    reloaded.scenario.load_session(&map).unwrap();
    assert_eq!(reloaded.scenario.document(), &saved);
    assert_eq!(serde_json::to_value(reloaded.scenario.start_camera()).unwrap(), serde_json::to_value(&camera).unwrap());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn moved_bound_spawn_and_camera_refresh_loaded_placement() {
    let (mut app, root, map) = fixture("move_bound_with_camera", &[]);
    place_house(&mut app, (30, 30), 0);
    save_ok(&mut app);
    app.placement.load_session(&map).unwrap();
    sync(&mut app, &map);
    let spawn_id = app.scenario.document().spawns[0].id;
    let before = data_objects(&map)[0].clone();
    app.spawn_ui.select_all();
    app.spawn_ui.move_selected(&mut app.scenario, &cx(), 512.0, 0.0, false);
    assert!(!app.spawn_ui.status().1, "{}", app.spawn_ui.status().0);
    app.campaign.start_camera.enabled = true;
    app.apply_start_camera_candidate().unwrap();
    save_ok(&mut app);
    let saved = data_objects(&map);
    assert_eq!(saved.len(), 1);
    let binding = app.scenario.document().data_slots.iter().find(|b| b.spawn_id == spawn_id).unwrap();
    assert_eq!((saved[0].slot as i32, saved[0].uid), (binding.slot, binding.uid));
    assert_eq!(saved[0].position[0], before.position[0] + 512.0);
    assert_eq!(app.placement.objects(), saved);
    assert!(!app.placement.is_dirty() && !app.scenario.dirty());
    assert!(!app.campaign.camera_pending());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn bound_building_region_victory_event_survives_safe_save_and_reload() {
    let (mut app, root, map) = fixture("bound_region_victory", &[]);
    place_house(&mut app, (30, 30), 0);
    let target_id = app.scenario.document().spawns[0].id;
    app.scenario.add_events(&[against_rome_scenario::Event {
        name: "主屋區域勝利".into(),
        delay_seconds: 0,
        conditions: vec![against_rome_scenario::Condition {
            kind: 2,
            target_id,
            min_x: 7000,
            min_z: 7000,
            max_x: 9000,
            max_z: 9000,
        }],
        actions: vec![against_rome_scenario::Action { kind: 3, ..Default::default() }],
        ..Default::default()
    }]).unwrap();

    save_ok(&mut app);
    let expected_events = app.scenario.document().events.clone();
    let expected_spawn = app.scenario.document().spawns.iter().find(|s| s.id == target_id).unwrap().clone();
    assert!(expected_spawn.prebuilt, "Safe Save converts the placed house to a DATA object");
    let mut loaded = EditorApp::initial_state();
    loaded.set_loaded_map(map.clone(), TerrainSession::create_blank(5, 32)).unwrap();
    loaded.scenario.load_session(&map).unwrap();
    loaded.placement.load_session(&map).unwrap();

    let doc = loaded.scenario.document();
    assert_eq!(doc.events, expected_events);
    let spawn = doc.spawns.iter().find(|s| s.id == target_id).unwrap();
    assert_eq!(spawn, &expected_spawn);
    let binding = doc.data_slots.iter().find(|b| b.spawn_id == target_id).expect("region target stays bound to its DATA slot");
    let object = data_objects(&map).into_iter().find(|o| o.slot as i32 == binding.slot).unwrap();
    assert_eq!(object.uid, binding.uid);
    assert_eq!(object.team, 0);
    assert!(doc.events.iter().any(|e| e.conditions.iter().any(|c| c.kind == 2 && c.target_id == target_id)
        && e.actions.iter().any(|a| a.kind == 3)));
    let bci = std::fs::read(map.join("SCRIPT/ak_level.bci")).unwrap();
    against_rome_scenario::BciImage::parse(&bci).unwrap();

    drop((app, loaded));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn deleting_event_target_requires_disabling_event_before_save_and_reload() {
    use against_rome_scenario::{Action, Condition, Event};

    let (mut app, root, map) = fixture("delete_event_target", &[]);
    place_house(&mut app, (30, 30), 0);
    let target_id = app.scenario.document().spawns[0].id;
    app.scenario.add_events(&[Event {
        name: "被刪除主屋的存活事件".into(),
        conditions: vec![Condition {
            kind: 0,
            target_id,
            ..Default::default()
        }],
        actions: vec![Action { kind: 3, ..Default::default() }],
        ..Default::default()
    }]).unwrap();
    save_ok(&mut app);
    sync(&mut app, &map);

    app.spawn_ui.select_all();
    let english = cx();
    app.spawn_ui.delete_selected(&mut app.scenario, &english);
    assert!(app.spawn_ui.status().1, "啟用事件引用的目標必須拒絕刪除");
    assert_eq!(
        app.spawn_ui.status().0,
        "Cannot delete: an enabled event still references this object. Disable the event or choose another target first."
    );

    let mut traditional_chinese = cx();
    traditional_chinese.lang = AppLanguage::TraditionalChinese;
    app.spawn_ui.select_all();
    app.spawn_ui.delete_selected(&mut app.scenario, &traditional_chinese);
    assert!(app.spawn_ui.status().1);
    assert_eq!(
        app.spawn_ui.status().0,
        "無法刪除：啟用中的事件仍引用此物件。請先停用該事件或改選其他目標。"
    );
    assert_eq!(app.scenario.document().spawns.len(), 1, "拒絕後物件仍保留");
    assert_eq!(app.scenario.document().spawns[0].id, target_id);
    assert!(!app.scenario.dirty(), "拒絕後劇本文件仍維持乾淨");
    assert_eq!(data_objects(&map).len(), 1, "拒絕後 DATA 建物仍保留");

    assert_eq!(app.scenario.set_events_enabled(&[0], false).unwrap(), 1);
    app.spawn_ui.select_all();
    app.spawn_ui.delete_selected(&mut app.scenario, &cx());
    assert!(!app.spawn_ui.status().1, "{}", app.spawn_ui.status().0);
    assert!(app.scenario.document().spawns.is_empty(), "停用事件後可移除目標");
    save_ok(&mut app);
    assert!(data_objects(&map).is_empty(), "停用事件後重試可提交建物刪除");
    let saved = against_rome_scenario::ScenarioDocument::from_json(
        &std::fs::read(map.join("arm_scenario.json")).unwrap(),
    ).unwrap();
    assert!(saved.spawns.is_empty() && saved.data_slots.is_empty());
    assert_eq!(saved.events.len(), 1);
    assert!(!saved.events[0].enabled, "已停用的懸空目標事件應保留供設計者修復");

    let mut reloaded = EditorApp::initial_state();
    reloaded.set_loaded_map(map.clone(), TerrainSession::create_blank(5, 32)).unwrap();
    reloaded.scenario.load_session(&map).unwrap();
    reloaded.placement.load_session(&map).unwrap();
    assert!(reloaded.scenario.document().spawns.is_empty());
    assert!(!reloaded.scenario.document().events[0].enabled);
    assert_eq!(reloaded.scenario.document().events[0].conditions[0].target_id, target_id);

    drop((app, reloaded));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn four_tribes_all_team_slots_and_squad_targets_survive_two_safe_save_reload_cycles() {
    use against_rome_scenario::{Action, Condition, DataBindingSource, Event, ScenarioDocument};

    let (mut app, root, map) = fixture("four_tribes_targets_roundtrip", &[]);
    let houses = [
        ("GER_HAU00", HOUSE),
        ("ROM_HAU00", "BauRomHau00_Hauptzelt"),
        ("KEL_HAU00", "BauKelHau00_Haupthaus"),
        ("HUN_HAU00", "BauHunHau00_Haupthaus"),
    ];
    // 僅擴充本案例的真實別名；官方 DATA 範本與 objdef 仍只有德系主堡。
    let aliases = houses.iter().map(|(alias, name)| format!("{alias}={name}\n")).collect::<String>();
    std::fs::write(
        root.join("SYSTEM/CLAK/cl_scint.ini"),
        format!("[ObjDefName]\n{aliases}GER_INF02=FigGerInf02_Zweihandaxt\n"),
    ).unwrap();
    app.scenario.load_session(&map).unwrap();
    app.spawn_ui = Default::default();
    app.spawn_ui.open = true;
    sync(&mut app, &map);
    for (tribe, (alias, name)) in houses.iter().enumerate() {
        assert!(app.scenario.known_aliases().iter().any(|a| a == *alias), "劇本必須重載部族別名：{alias}");
        assert!(app.spawn_ui.catalog().iter().any(|t| t.name_def == *name), "放置目錄必須重載主堡：{name}");
        for team in 0..=8 {
            app.spawn_ui.select_type(name);
            app.spawn_ui.set_placement_values(team, 1, 0);
            app.spawn_ui.place_at_tile(&mut app.scenario, &cx(), 4 + team * 3, 4 + tribe as i32 * 6);
            assert!(!app.spawn_ui.status().1, "主堡放置失敗：{}", app.spawn_ui.status().0);
        }
    }
    app.spawn_ui.select_type("FigGerInf02_Zweihandaxt");
    // 人物／部隊的放置面板僅接受 team 0..7；逐隊驗證 script squad 不進入 DATA。
    for team in 0..=7 {
        app.spawn_ui.set_placement_values(team, 7, 0);
        app.spawn_ui.place_at_tile(&mut app.scenario, &cx(), 3 + team * 3, 28);
        assert!(!app.spawn_ui.status().1, "T{team} 部隊放置失敗：{}", app.spawn_ui.status().0);
    }

    let original_spawns = app.scenario.document().spawns.clone();
    assert_eq!(original_spawns.len(), 44, "四部族各九座主堡加八個隊伍槽位的部隊");
    let ids: std::collections::HashSet<_> = original_spawns.iter().map(|s| s.id).collect();
    assert_eq!(ids.len(), 44, "所有原始 UUID 必須唯一");
    assert!(original_spawns.iter().all(|s| !s.id.is_nil() && !s.prebuilt), "原始放置物件皆有 UUID 且尚未轉為 DATA");
    let squads: Vec<_> = original_spawns.iter().filter(|s| s.alias == "GER_INF02").collect();
    assert_eq!(squads.len(), 8, "八個隊伍槽位皆需有 script squad");
    for (team, squad) in squads.iter().enumerate() {
        assert_eq!((squad.team, squad.count), (team as i32, 7), "部隊需保留 T{team} 與多人數量並走 script 路徑");
    }
    let events: Vec<_> = original_spawns.iter().flat_map(|spawn| {
        [(0, 3, "存活勝利"), (1, 4, "死亡失敗")].map(|(condition, action, label)| Event {
            name: format!("{} T{} {label}", spawn.alias, spawn.team),
            delay_seconds: 0,
            conditions: vec![Condition { kind: condition, target_id: spawn.id, ..Default::default() }],
            actions: vec![Action { kind: action, ..Default::default() }],
            ..Default::default()
        })
    }).collect();
    assert_eq!(events.len(), 88, "44 個物件各有存活與死亡目標事件");
    app.scenario.add_events(&events).unwrap();
    let mut expected_spawns = original_spawns.clone();
    for spawn in &mut expected_spawns {
        spawn.prebuilt = spawn.alias == "GER_HAU00";
    }
    let mut first_files = None;
    let mut first_bindings = None;

    for cycle in 1..=2 {
        app.safe_save_current();
        assert!(!app.status_is_error, "第 {cycle} 次安全儲存失敗：{}", app.status_message);
        assert!(!app.scenario.dirty(), "第 {cycle} 次儲存後劇本應為乾淨狀態");
        let saved = ScenarioDocument::from_json(&std::fs::read(map.join("arm_scenario.json")).unwrap()).unwrap();
        assert_eq!(saved.spawns, expected_spawns, "第 {cycle} 次儲存保留原 UUID 與全部放置內容，僅德系轉為 DATA");
        assert_eq!(saved.events, events, "第 {cycle} 次儲存保留每個 UUID 的存活／死亡與勝敗事件");
        assert_eq!(app.scenario.document(), &saved, "第 {cycle} 次儲存後記憶體與磁碟劇本一致");
        assert_eq!(saved.data_slots.len(), 9, "只有九座德系主堡可綁定 DATA");
        if let Some(bindings) = &first_bindings {
            assert_eq!(&saved.data_slots, bindings, "第二次儲存保留 DATA slot／UID 與 UUID 關係");
        } else {
            first_bindings = Some(saved.data_slots.clone());
        }

        let objects = data_objects(&map);
        assert_eq!(objects.len(), 9, "fallback 主堡與多人部隊不得產生 DATA 物件");
        let source = DataBindingSource::load(&map).unwrap();
        let mut bound: Vec<_> = saved.data_slots.iter().map(|b| (b.slot, b.uid)).collect();
        bound.sort_unstable();
        assert_eq!(bound, source.active_bindings(), "每筆 active DATA 身分都恰有一筆劇本綁定");
        for spawn in &saved.spawns {
            let bindings: Vec<_> = saved.data_slots.iter().filter(|b| b.spawn_id == spawn.id).collect();
            if spawn.alias == "GER_HAU00" {
                assert_eq!(bindings.len(), 1, "德系主堡 UUID 必須唯一對應 DATA");
                let binding = bindings[0];
                let object = objects.iter().find(|o| o.slot as i32 == binding.slot).expect("綁定的 DATA slot 必須存在");
                assert_eq!(object.uid, binding.uid, "DATA UID 必須符合綁定");
                assert_eq!((object.type_id, i32::from(object.team)), (232, spawn.team), "DATA 型別與隊伍必須符合主堡");
                assert_eq!(object.position, [spawn.x, spawn.y, spawn.z], "DATA 位置必須符合原 UUID 的主堡");
            } else {
                assert!(bindings.is_empty() && !spawn.prebuilt, "無模板主堡與多人部隊必須保留 script 路徑：{}", spawn.alias);
            }
        }

        let mut loaded = EditorApp::initial_state();
        loaded.set_loaded_map(map.clone(), TerrainSession::create_blank(5, 32)).unwrap();
        loaded.scenario.load_session(&map).unwrap();
        loaded.placement.load_session(&map).unwrap();
        sync(&mut loaded, &map);
        assert_eq!(loaded.scenario.document(), &saved, "第 {cycle} 次全新重載保留 spawns／events／data_slots");
        assert!(!loaded.scenario.dirty() && !loaded.spawn_ui.session().is_dirty(), "全新重載不得留下未儲存狀態");
        against_rome_scenario::verify_binding_snapshot(loaded.scenario.document(), source.active_bindings()).unwrap();
        let original_bci = std::fs::read(map.join("SCRIPT/ak_level.arm_original")).unwrap();
        let compiled = against_rome_scenario::compile_script_verified(
            &original_bci,
            loaded.scenario.document(),
            Some(loaded.scenario.start_camera()),
            Some(loaded.scenario.known_aliases()),
            Some(source.active_bindings()),
        ).unwrap();
        let compiled_raw = if compiled.starts_with(b"PFIL") {
            against_rome_data::lzss::decompress_pfil(&compiled, None).unwrap()
        } else {
            compiled
        };
        source.verify_unchanged().unwrap();
        let disk_bci = std::fs::read(map.join("SCRIPT/ak_level.bci")).unwrap();
        against_rome_scenario::BciImage::parse(&disk_bci).unwrap();
        assert_eq!(compiled_raw, disk_bci, "第 {cycle} 次重載使用 active bindings 重編的 BCI 必須與磁碟逐位元組一致");
        let mut files = save_file_snapshot(&map);
        // 交易備份使用隨機名稱且保留各次舊內容；往返應比較正式地圖檔案。
        files.retain(|path, _| !path.to_string_lossy().contains(".arm_backup."));
        if let Some(expected) = &first_files {
            assert!(files == *expected, "第二次安全儲存與重開後正式地圖檔案必須逐位元組一致");
        } else {
            first_files = Some(files);
        }
        // 下一輪由全新載入的 app 再次安全儲存，驗證乾淨狀態的往返穩定性。
        app = loaded;
    }

    drop(app);
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
fn independent_objects_edit_and_prebuilt_spawn_share_safe_save_with_rollback_and_reload() {
    use against_rome_maps::TransactionFailurePoint as Fail;

    let (mut app, root, map) = fixture("objects_and_prebuilt", &[(232, None)]);
    app.placement.load_session(&map).unwrap();
    app.placement.store_mut().unwrap().set_team(0, 1, 3).unwrap();
    assert!(app.placement.is_dirty());

    place_house(&mut app, (30, 30), 0);
    let spawn_id = app.scenario.document().spawns[0].id;
    let before = save_file_snapshot(&root);

    // 交易失敗時保留磁碟內容與兩邊的編輯。
    app.safe_save_current_using(|tx| {
        let mut injected = std::mem::replace(
            tx,
            against_rome_maps::MapSaveTransaction::new("unused"),
        )
        .with_injected_failure(Fail::BeforeFinalCommit, "模擬交易失敗");
        injected.commit().map_err(|e| e.to_string())
    });
    assert!(app.status_is_error, "{}", app.status_message);
    assert_eq!(save_file_snapshot(&root), before);
    assert!(app.scenario.dirty() && app.placement.is_dirty());

    // 重試後確認無關物件編輯與預建綁定同時保留。
    save_ok(&mut app);
    assert!(!app.scenario.dirty() && !app.placement.is_dirty());
    let objects = data_objects(&map);
    let edited = objects.iter().find(|object| object.slot == 0).unwrap();
    assert_eq!(edited.team, 3);
    let saved = against_rome_scenario::ScenarioDocument::from_json(
        &std::fs::read(map.join("arm_scenario.json")).unwrap(),
    )
    .unwrap();
    assert!(saved.spawns.iter().any(|spawn| spawn.id == spawn_id && spawn.prebuilt));
    let binding = saved.data_slots.iter().find(|entry| entry.spawn_id == spawn_id).unwrap();
    assert!(objects
        .iter()
        .any(|object| object.slot as i32 == binding.slot && object.uid == binding.uid));
    assert_eq!(objects.len(), 2);
    assert_eq!(app.placement.objects(), objects);

    let mut reloaded = EditorApp::initial_state();
    reloaded
        .set_loaded_map(map.clone(), crate::storage::load_map(&map).unwrap())
        .unwrap();
    reloaded.placement.load_session(&map).unwrap();
    reloaded.scenario.load_session(&map).unwrap();
    let reloaded_objects = data_objects(&map);
    assert_eq!(reloaded.placement.objects(), reloaded_objects);
    assert_eq!(reloaded_objects.iter().find(|object| object.slot == 0).unwrap().team, 3);
    assert_eq!(reloaded.scenario.document(), &saved);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn editing_a_bound_spawn_family_blocks_another_spawn_data_change_without_writing() {
    let (mut app, root, map) = fixture("bound_family_conflict", &[]);
    place_house(&mut app, (30, 30), 0);
    save_ok(&mut app);
    sync(&mut app, &map);
    app.placement.load_session(&map).unwrap();

    let original_spawn_id = app.scenario.document().spawns[0].id;
    let binding = app
        .scenario
        .document()
        .data_slots
        .iter()
        .find(|entry| entry.spawn_id == original_spawn_id)
        .unwrap()
        .clone();
    assert!(app.spawn_ui.is_data_bound_index(0));

    // 修改既有綁定家族，再新增另一個預建物件。
    app.placement
        .store_mut()
        .unwrap()
        .set_family_team(binding.slot as usize, binding.uid, 3)
        .unwrap();
    place_house(&mut app, (35, 35), 0);
    assert!(app.placement.is_dirty() && app.scenario.dirty());
    let before = save_file_snapshot(&root);

    app.safe_save_current();
    assert!(app.status_is_error, "修改已儲存 bound family 時應拒絕合併：{}", app.status_message);
    assert!(app.status_message.contains("衝突"), "{}", app.status_message);
    assert_eq!(save_file_snapshot(&root), before);
    assert!(app.placement.is_dirty() && app.scenario.dirty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn nature_objects_and_prebuilt_spawn_share_safe_save_and_reload() {
    let (mut app, root, map) = fixture("nature_and_prebuilt", &[]);
    // 重用 Nature 測試資料，同時保留 spawn 型錄與官方主屋模板。
    nature_data_fixture(&map, 16);
    let mut house_columns = vec!["0".to_string(); 53];
    house_columns[0] = "232".into();
    house_columns[1] = "1".into();
    house_columns[52] = HOUSE.into();
    let mut tree_columns = vec!["0".to_string(); 53];
    tree_columns[0] = NATURE_TREE.to_string();
    tree_columns[1] = "1".into();
    tree_columns[52] = "LanGerNad00_Tanne".into();
    std::fs::write(
        root.join("SYSTEM/DATA_MP/DEFAULTS/objdef.dau"),
        format!(
            "[ObjectDefaults]\n{}\n{}\n",
            house_columns.join(","),
            tree_columns.join(",")
        ),
    )
    .unwrap();
    app.placement.load_session(&map).unwrap();

    app.nature_set_enabled(true);
    assert!(!app.status_is_error, "{}", app.status_message);
    app.nature_select_type(i32::from(NATURE_TREE));
    // 與種植模板無關的既有樹木編輯必須一併保留。
    app.placement.store_mut().unwrap().set_team(1, 2, 7).unwrap();
    let planted = app.nature_paint_tile(20, 20);
    app.nature_end_stroke();
    assert!(planted.planted > 0);
    assert!(app.nature_is_dirty());

    place_house(&mut app, (30, 30), 0);
    let spawn_id = app.scenario.document().spawns[0].id;
    assert!(app.scenario.dirty());
    save_ok(&mut app);
    assert!(!app.nature_is_dirty() && !app.scenario.dirty() && !app.placement.is_dirty());

    let saved = against_rome_scenario::ScenarioDocument::from_json(
        &std::fs::read(map.join("arm_scenario.json")).unwrap(),
    )
    .unwrap();
    assert!(saved.spawns.iter().any(|spawn| spawn.id == spawn_id && spawn.prebuilt));
    let binding = saved.data_slots.iter().find(|entry| entry.spawn_id == spawn_id).unwrap();
    let objects = data_objects(&map);
    assert!(objects
        .iter()
        .any(|object| object.slot as i32 == binding.slot && object.uid == binding.uid));
    assert_eq!(
        objects
            .iter()
            .filter(|object| object.type_id == NATURE_TREE)
            .count(),
        2 + planted.planted,
        "原有兩棵樹與 Nature 新增物件都應保留"
    );
    assert_eq!(objects.iter().find(|object| object.slot == 1).unwrap().team, 7);

    assert_eq!(app.placement.objects(), objects);

    let mut reloaded = EditorApp::initial_state();
    reloaded
        .set_loaded_map(map.clone(), crate::storage::load_map(&map).unwrap())
        .unwrap();
    reloaded.placement.load_session(&map).unwrap();
    reloaded.scenario.load_session(&map).unwrap();
    assert_eq!(reloaded.placement.objects(), objects);
    assert_eq!(reloaded.scenario.document(), &saved);
    assert!(reloaded
        .scenario
        .document()
        .data_slots
        .iter()
        .any(|entry| entry.spawn_id == spawn_id && entry.slot == binding.slot && entry.uid == binding.uid));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn moved_composite_spawn_diagnostics_use_final_candidate_with_independent_objects_edit() {
    let (mut app, root, map) = fixture("merged_family_diagnostics", &[]);
    place_house(&mut app, (30, 30), 0);
    save_ok(&mut app);
    let uid = app.scenario.document().data_slots[0].uid;
    let id = install_two_object_family_with_light(&map, uid);

    let mut loaded = EditorApp::initial_state();
    loaded.set_loaded_map(map.clone(), TerrainSession::create_blank(5, 32)).unwrap();
    loaded.scenario.load_session(&map).unwrap();
    loaded.placement.load_session(&map).unwrap();
    sync(&mut loaded, &map);
    loaded.placement.store_mut().unwrap().clone_family_fixed(0, uid, [2048 * 256, 0, 0]).unwrap();
    loaded.spawn_ui.select_all();
    loaded.spawn_ui.move_selected(&mut loaded.scenario, &cx(), 512.0, 0.0, false);
    assert!(!loaded.spawn_ui.status().1, "{}", loaded.spawn_ui.status().0);
    loaded.water_surface = 1000.0;

    let before = save_file_snapshot(&root);
    let plan = loaded.preview_spawn_data_for_save(&map).unwrap().unwrap();
    assert_eq!(plan.scenario.data_slots[0].uid, uid, "複合家族平移保留 UID");
    assert_eq!(plan.scenario.spawns[0].x, 4512.0);
    let staged = vec![(PathBuf::from("arm_scenario.json"), plan.scenario.to_json().unwrap())];
    let diagnostics = loaded.candidate_diagnostics(&map, &staged).unwrap();
    assert!(diagnostics.map_issues.iter().any(|issue| issue.code == "blocked-building"
        && issue.object_id == id && issue.world_x == Some(4512.0)),
        "診斷必須使用同筆交易中已平移的家族位置：{:?}", diagnostics.map_issues);
    assert_eq!(save_file_snapshot(&root), before, "預覽不得落盤");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn external_data_change_blocks_merged_spawn_save_without_losing_edits() {
    let (mut app, root, map) = fixture("merged_baseline_drift", &[(232, None)]);
    app.placement.load_session(&map).unwrap();
    app.placement.store_mut().unwrap().set_team(0, 1, 3).unwrap();
    place_house(&mut app, (30, 30), 0);
    let mut changed = std::fs::read(map.join("DATA/engine.dat")).unwrap();
    changed[34..38].copy_from_slice(&900u32.to_le_bytes());
    std::fs::write(map.join("DATA/engine.dat"), changed).unwrap();
    let before = save_file_snapshot(&root);
    assert!(!app.save_blockers().is_empty());
    app.safe_save_current();
    assert!(app.status_is_error, "外部 DATA 變更必須拒絕合併");
    assert_eq!(save_file_snapshot(&root), before);
    assert!(app.placement.is_dirty() && app.scenario.dirty());
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

#[test]
fn invalid_event_data_is_rejected_before_safe_save_without_losing_edits() {
    use against_rome_scenario::{Action, Condition, Event};

    fn valid_event(target_id: uuid::Uuid) -> Event {
        Event {
            name: "安全儲存拒絕後可修復".into(),
            conditions: vec![Condition {
                kind: 0,
                target_id,
                ..Default::default()
            }],
            actions: vec![Action {
                kind: 3,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    for (case, invalid_action, missing_target) in [
        (
            "invalid_team",
            Some(Action {
                kind: 2,
                alias: "TROOP".into(),
                team: 8,
                count: 1,
                ..Default::default()
            }),
            false,
        ),
        (
            "invalid_position",
            Some(Action {
                kind: 2,
                alias: "TROOP".into(),
                x: 16384.0,
                count: 1,
                ..Default::default()
            }),
            false,
        ),
        ("missing_target", None, true),
    ] {
        let (mut app, root, map) = fixture(&format!("invalid_event_{case}"), &[]);
        place_house(&mut app, (30, 30), 0);
        let target_id = app.scenario.document().spawns[0].id;
        let valid = valid_event(target_id);
        app.scenario.add_events(std::slice::from_ref(&valid)).unwrap();
        let before = save_file_snapshot(&root);

        let mut invalid = valid.clone();
        if let Some(action) = invalid_action {
            invalid.actions = vec![action];
        }
        if missing_target {
            invalid.conditions[0].target_id = uuid::Uuid::new_v4();
        }
        app.scenario.replace_event(0, invalid.clone()).unwrap();
        assert!(app.scenario.dirty(), "注入 {case} 後應保留未儲存編輯");

        app.safe_save_current();
        assert!(app.status_is_error, "{case} 必須被 Safe Save 拒絕：{}", app.status_message);
        assert!(app.status_message.contains("Safe Save Rejected"), "{}", app.status_message);
        assert_eq!(save_file_snapshot(&root), before, "{case} 驗證失敗不得改寫地圖檔案");
        assert!(app.scenario.dirty(), "拒絕 {case} 後劇本編輯仍須保留");
        assert_eq!(app.scenario.document().events[0], invalid, "拒絕後仍保留可修復的錯誤編輯");

        app.scenario.replace_event(0, valid.clone()).unwrap();
        save_ok(&mut app);
        assert!(!app.scenario.dirty(), "修復 {case} 後重試應建立乾淨儲存基準");
        let saved = against_rome_scenario::ScenarioDocument::from_json(
            &std::fs::read(map.join("arm_scenario.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(saved.events, vec![valid], "修復後事件必須正常寫入");
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn bound_spawn_edits_after_reload_keep_event_identity_and_reconcile_data_bindings() {
    use against_rome_scenario::{Action, Condition, Event};

    let (mut app, root, map) = fixture("post_reload_mutation", &[]);
    place_house(&mut app, (30, 30), 0);
    let original_id = app.scenario.document().spawns[0].id;
    let event = Event {
        name: "原主屋存活勝利".into(),
        conditions: vec![Condition {
            kind: 0,
            target_id: original_id,
            ..Default::default()
        }],
        actions: vec![Action {
            kind: 3,
            ..Default::default()
        }],
        ..Default::default()
    };
    app.scenario.add_events(std::slice::from_ref(&event)).unwrap();
    save_ok(&mut app);

    // 第二個 EditorApp 從磁碟重建場景與 DATA binding，再執行編輯。
    let mut reloaded = EditorApp::initial_state();
    reloaded.set_loaded_map(map.clone(), TerrainSession::create_blank(5, 32)).unwrap();
    reloaded.scenario.load_session(&map).unwrap();
    reloaded.placement.load_session(&map).unwrap();
    sync(&mut reloaded, &map);
    assert_eq!(reloaded.scenario.document().events, vec![event.clone()]);

    reloaded.spawn_ui.select_all();
    reloaded.spawn_ui.duplicate_selected(&mut reloaded.scenario, &cx());
    assert!(!reloaded.spawn_ui.status().1, "{}", reloaded.spawn_ui.status().0);
    let clone_id = reloaded
        .scenario
        .document()
        .spawns
        .iter()
        .find(|spawn| spawn.id != original_id)
        .expect("duplicate gets a new permanent UUID")
        .id;
    assert_ne!(clone_id, original_id);
    reloaded.spawn_ui.move_selected(&mut reloaded.scenario, &cx(), 512.0, 0.0, false);
    reloaded.spawn_ui.apply_batch(&mut reloaded.scenario, &cx(), Some(3), Some(90.0));
    assert!(!reloaded.spawn_ui.status().1, "{}", reloaded.spawn_ui.status().0);
    assert_eq!(reloaded.scenario.document().events[0].conditions[0].target_id, original_id);
    save_ok(&mut reloaded);

    let after_duplicate = against_rome_scenario::ScenarioDocument::from_json(
        &std::fs::read(map.join("arm_scenario.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(after_duplicate.events, vec![event.clone()], "duplicate must not retarget the original event");
    assert_eq!(after_duplicate.spawns.len(), 2);
    assert!(after_duplicate.spawns.iter().all(|spawn| spawn.prebuilt));
    assert_eq!(after_duplicate.data_slots.len(), 2, "both reloaded original and duplicate are DATA-bound");
    assert_eq!(data_objects(&map).len(), 2);
    assert_eq!(
        after_duplicate.data_slots.iter().map(|binding| binding.spawn_id).collect::<std::collections::HashSet<_>>(),
        [original_id, clone_id].into_iter().collect(),
    );
    let moved_clone = after_duplicate.spawns.iter().find(|spawn| spawn.id == clone_id).unwrap();
    assert_eq!(moved_clone.team, 3);
    assert_eq!(moved_clone.x, 7808.0 + 256.0 + 512.0, "duplicate offset plus the post-reload move");
    assert_eq!(moved_clone.angle, 90);

    // 同一組已重載物件在 script-only team 12 與 DATA team 1 間升降級，事件 UUID 不變。
    let mut after_reload = EditorApp::initial_state();
    after_reload.set_loaded_map(map.clone(), TerrainSession::create_blank(5, 32)).unwrap();
    after_reload.scenario.load_session(&map).unwrap();
    after_reload.placement.load_session(&map).unwrap();
    sync(&mut after_reload, &map);
    after_reload.spawn_ui.select_all();
    after_reload.spawn_ui.apply_batch(&mut after_reload.scenario, &cx(), Some(12), None);
    assert!(!after_reload.spawn_ui.status().1, "{}", after_reload.spawn_ui.status().0);
    save_ok(&mut after_reload);
    assert!(after_reload.scenario.document().spawns.iter().all(|spawn| !spawn.prebuilt));
    assert!(after_reload.scenario.document().data_slots.is_empty());
    assert!(data_objects(&map).is_empty(), "降級至 script team 後要移除 DATA 物件");
    assert_eq!(after_reload.scenario.document().events[0].conditions[0].target_id, original_id);

    let mut promoted = EditorApp::initial_state();
    promoted.set_loaded_map(map.clone(), TerrainSession::create_blank(5, 32)).unwrap();
    promoted.scenario.load_session(&map).unwrap();
    promoted.placement.load_session(&map).unwrap();
    sync(&mut promoted, &map);
    promoted.spawn_ui.select_all();
    promoted.spawn_ui.apply_batch(&mut promoted.scenario, &cx(), Some(1), None);
    assert!(!promoted.spawn_ui.status().1, "{}", promoted.spawn_ui.status().0);
    save_ok(&mut promoted);
    let final_doc = against_rome_scenario::ScenarioDocument::from_json(
        &std::fs::read(map.join("arm_scenario.json")).unwrap(),
    )
    .unwrap();
    assert!(final_doc.spawns.iter().all(|spawn| spawn.prebuilt && spawn.team == 1));
    assert_eq!(final_doc.events, vec![event], "升級 DATA binding 後原事件仍指向原 UUID");
    assert_eq!(final_doc.data_slots.len(), 2);
    assert_eq!(data_objects(&map).len(), 2);

    let mut final_reload = EditorApp::initial_state();
    final_reload.set_loaded_map(map.clone(), TerrainSession::create_blank(5, 32)).unwrap();
    final_reload.scenario.load_session(&map).unwrap();
    final_reload.placement.load_session(&map).unwrap();
    sync(&mut final_reload, &map);
    assert_eq!(final_reload.scenario.document(), &final_doc);
    against_rome_scenario::verify_binding_snapshot(
        final_reload.scenario.document(),
        against_rome_scenario::DataBindingSource::load(&map).unwrap().active_bindings(),
    )
    .unwrap();

    drop((app, reloaded, after_reload, promoted, final_reload));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn bound_family_with_child_light_survives_safe_save_reload_and_translation() {
    let (mut app, root, map) = fixture("family_light_reload_move", &[]);
    place_house(&mut app, (30, 30), 0);
    save_ok(&mut app);

    let bound_uid = app.scenario.document().data_slots[0].uid;
    let root_id = install_two_object_family_with_light(&map, bound_uid);

    let mut reloaded = EditorApp::initial_state();
    reloaded.set_loaded_map(map.clone(), TerrainSession::create_blank(5, 32)).unwrap();
    reloaded.scenario.load_session(&map).unwrap();
    reloaded.placement.load_session(&map).unwrap();
    sync(&mut reloaded, &map);
    assert_eq!(reloaded.scenario.document().data_slots.len(), 1, "only the family root gets a scenario binding");
    assert_eq!(reloaded.scenario.document().data_slots[0].spawn_id, root_id);
    assert_eq!(ObjectStore::load(&map).unwrap().family(0, bound_uid).unwrap().len(), 2);
    let before_light = against_rome_data::pool::LightPool::from_file_bytes(
        &std::fs::read(map.join("DATA/light.dat")).unwrap(),
    )
    .unwrap()
    .capture(0)
    .unwrap();
    assert_eq!(before_light.owner(), 1);

    reloaded.spawn_ui.select_all();
    reloaded.spawn_ui.move_selected(&mut reloaded.scenario, &cx(), 512.0, 0.0, false);
    assert!(!reloaded.spawn_ui.status().1, "{}", reloaded.spawn_ui.status().0);
    save_ok(&mut reloaded);

    let after = ObjectStore::load(&map).unwrap();
    let family = after.family(0, bound_uid).unwrap();
    assert_eq!(family.len(), 2, "the child remains attached to the bound root");
    assert_eq!(after.objects().iter().find(|o| o.slot == 0).unwrap().position[0], 4512.0);
    assert_eq!(after.objects().iter().find(|o| o.slot == 1).unwrap().position[0], 4532.0);
    let moved_light = against_rome_data::pool::LightPool::from_file_bytes(
        &std::fs::read(map.join("DATA/light.dat")).unwrap(),
    )
    .unwrap()
    .capture(0)
    .unwrap();
    assert_eq!(moved_light.position(), [4533.0, 8.0, 4002.0]);
    assert_eq!(moved_light.owner(), 1);
    let saved = against_rome_scenario::ScenarioDocument::from_json(
        &std::fs::read(map.join("arm_scenario.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(saved.data_slots.len(), 1);
    assert_eq!(saved.data_slots[0].spawn_id, root_id);
    assert_eq!(saved.spawns[0].x, 4512.0);

    drop((app, reloaded));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn enabled_event_protects_composite_family_and_disabled_event_allows_atomic_removal() {
    use against_rome_scenario::{Action, Condition, Event, ScenarioDocument};

    let (mut app, root, map) = fixture("family_event_delete_guard", &[]);
    place_house(&mut app, (30, 30), 0);
    save_ok(&mut app);
    let binding = app.scenario.document().data_slots[0].clone();
    let root_id = install_two_object_family_with_light(&map, binding.uid);

    let mut loaded = EditorApp::initial_state();
    loaded.set_loaded_map(map.clone(), TerrainSession::create_blank(5, 32)).unwrap();
    loaded.scenario.load_session(&map).unwrap();
    loaded.placement.load_session(&map).unwrap();
    sync(&mut loaded, &map);
    assert_eq!(ObjectStore::load(&map).unwrap().family(binding.slot as usize, binding.uid).unwrap().len(), 2);
    loaded.scenario.add_events(&[Event {
        name: "保護複合建築的啟用事件".into(),
        conditions: vec![Condition { kind: 0, target_id: root_id, ..Default::default() }],
        actions: vec![Action { kind: 3, ..Default::default() }],
        ..Default::default()
    }]).unwrap();
    save_ok(&mut loaded);
    sync(&mut loaded, &map);

    let objects_before = std::fs::read(map.join("DATA/objects.dat")).unwrap();
    let light_before = std::fs::read(map.join("DATA/light.dat")).unwrap();
    let document_before = loaded.scenario.document().clone();
    loaded.spawn_ui.select_all();
    loaded.spawn_ui.delete_selected(&mut loaded.scenario, &cx());
    assert!(loaded.spawn_ui.status().1, "enabled event must protect its composite target");
    assert_eq!(loaded.scenario.document(), &document_before);
    assert!(!loaded.scenario.dirty());
    assert_eq!(std::fs::read(map.join("DATA/objects.dat")).unwrap(), objects_before);
    assert_eq!(std::fs::read(map.join("DATA/light.dat")).unwrap(), light_before);
    assert_eq!(ObjectStore::load(&map).unwrap().family(binding.slot as usize, binding.uid).unwrap().len(), 2);

    assert_eq!(loaded.scenario.set_events_enabled(&[0], false).unwrap(), 1);
    loaded.spawn_ui.select_all();
    loaded.spawn_ui.delete_selected(&mut loaded.scenario, &cx());
    assert!(!loaded.spawn_ui.status().1, "{}", loaded.spawn_ui.status().0);
    assert!(loaded.scenario.document().spawns.is_empty());
    save_ok(&mut loaded);

    let after = ObjectStore::load(&map).unwrap();
    assert!(after.objects().is_empty(), "deleting the root removes its child family too");
    let light_pool = against_rome_data::pool::LightPool::from_file_bytes(
        &std::fs::read(map.join("DATA/light.dat")).unwrap(),
    ).unwrap();
    assert_eq!(light_pool.active_slots().count(), 0, "the child's linked light is removed with the family");
    let saved = ScenarioDocument::from_json(&std::fs::read(map.join("arm_scenario.json")).unwrap()).unwrap();
    assert!(saved.spawns.is_empty() && saved.data_slots.is_empty());
    assert_eq!(saved.events.len(), 1);
    assert!(!saved.events[0].enabled);
    assert_eq!(saved.events[0].conditions[0].target_id, root_id);

    let mut reloaded = EditorApp::initial_state();
    reloaded.set_loaded_map(map.clone(), TerrainSession::create_blank(5, 32)).unwrap();
    reloaded.scenario.load_session(&map).unwrap();
    reloaded.placement.load_session(&map).unwrap();
    sync(&mut reloaded, &map);
    assert!(reloaded.scenario.document().spawns.is_empty());
    assert!(reloaded.scenario.document().data_slots.is_empty());
    assert!(!reloaded.scenario.document().events[0].enabled);
    assert_eq!(reloaded.scenario.document().events[0].conditions[0].target_id, root_id);
    assert!(ObjectStore::load(&map).unwrap().objects().is_empty());

    drop((app, loaded, reloaded));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn particle_linked_light_family_moves_reloads_and_rolls_back_mult_pool_deletion() {
    use against_rome_data::particle::ParticlePool;
    use against_rome_maps::TransactionFailurePoint as Fail;

    let (mut app, root, map) = fixture("family_particle_light_rollback", &[]);
    place_house(&mut app, (30, 30), 0);
    save_ok(&mut app);
    let binding = app.scenario.document().data_slots[0].clone();
    let root_id = install_two_object_family_with_particle_light(&map, binding.uid);

    let mut loaded = EditorApp::initial_state();
    loaded.set_loaded_map(map.clone(), TerrainSession::create_blank(5, 32)).unwrap();
    loaded.scenario.load_session(&map).unwrap();
    loaded.placement.load_session(&map).unwrap();
    sync(&mut loaded, &map);
    let initial_particle = ParticlePool::from_file_bytes(&std::fs::read(map.join("DATA/particle.dat")).unwrap()).unwrap();
    let initial_child = initial_particle.capture(0).unwrap().child(0).unwrap();
    assert_eq!(initial_particle.capture(0).unwrap().owner(), 0);
    assert_eq!(initial_child.light_link(), 0);
    assert_eq!(ObjectStore::load(&map).unwrap().family(binding.slot as usize, binding.uid).unwrap().len(), 2);
    assert_eq!(loaded.scenario.document().data_slots.len(), 1);
    assert_eq!(loaded.scenario.document().data_slots[0].spawn_id, root_id);

    loaded.spawn_ui.select_all();
    loaded.spawn_ui.move_selected(&mut loaded.scenario, &cx(), 512.0, 0.0, false);
    assert!(!loaded.spawn_ui.status().1, "{}", loaded.spawn_ui.status().0);
    save_ok(&mut loaded);

    let moved_particle = ParticlePool::from_file_bytes(&std::fs::read(map.join("DATA/particle.dat")).unwrap()).unwrap();
    let moved_child = moved_particle.capture(0).unwrap().child(0).unwrap();
    let expected_fixed_x = initial_child.position_fixed()[0] + 512 * 65536;
    assert_eq!(moved_child.position_fixed()[0], expected_fixed_x);
    let moved_light = against_rome_data::pool::LightPool::from_file_bytes(
        &std::fs::read(map.join("DATA/light.dat")).unwrap(),
    ).unwrap().capture(0).unwrap();
    let expected_light: [f32; 3] = std::array::from_fn(|axis| (moved_child.position_fixed()[axis] >> 16) as f32);
    assert_eq!(moved_light.position(), expected_light, "particle-linked Light follows the native fixed-point child position");

    let mut reloaded = EditorApp::initial_state();
    reloaded.set_loaded_map(map.clone(), TerrainSession::create_blank(5, 32)).unwrap();
    reloaded.scenario.load_session(&map).unwrap();
    reloaded.placement.load_session(&map).unwrap();
    sync(&mut reloaded, &map);
    let round_trip = ParticlePool::from_file_bytes(&std::fs::read(map.join("DATA/particle.dat")).unwrap()).unwrap();
    assert_eq!(round_trip.capture(0).unwrap().child(0).unwrap().position_fixed(), moved_child.position_fixed());
    assert_eq!(reloaded.scenario.document().data_slots[0].spawn_id, root_id);

    reloaded.spawn_ui.select_all();
    reloaded.spawn_ui.delete_selected(&mut reloaded.scenario, &cx());
    assert!(!reloaded.spawn_ui.status().1, "{}", reloaded.spawn_ui.status().0);
    assert!(reloaded.scenario.document().spawns.is_empty());
    let before_failed_commit = save_file_snapshot(&root);
    reloaded.safe_save_current_using(|tx| {
        let mut injected = std::mem::replace(tx, against_rome_maps::MapSaveTransaction::new("unused"))
            .with_injected_failure(Fail::BeforeFinalCommit, "particle family deletion rollback");
        injected.commit().map_err(|e| e.to_string())
    });
    assert!(reloaded.status_is_error, "{}", reloaded.status_message);
    assert_eq!(save_file_snapshot(&root), before_failed_commit, "all DATA pools and scenario roll back together");
    assert!(reloaded.scenario.document().spawns.is_empty(), "failed save keeps the in-memory deletion for retry");
    assert!(ObjectStore::load(&map).unwrap().family(binding.slot as usize, binding.uid).unwrap().len() == 2);
    assert_eq!(ParticlePool::from_file_bytes(&std::fs::read(map.join("DATA/particle.dat")).unwrap()).unwrap().active_slots().count(), 1);

    save_ok(&mut reloaded);
    assert!(ObjectStore::load(&map).unwrap().objects().is_empty());
    assert_eq!(ParticlePool::from_file_bytes(&std::fs::read(map.join("DATA/particle.dat")).unwrap()).unwrap().active_slots().count(), 0);
    assert_eq!(against_rome_data::pool::LightPool::from_file_bytes(
        &std::fs::read(map.join("DATA/light.dat")).unwrap(),
    ).unwrap().active_slots().count(), 0);

    let mut final_reload = EditorApp::initial_state();
    final_reload.set_loaded_map(map.clone(), TerrainSession::create_blank(5, 32)).unwrap();
    final_reload.scenario.load_session(&map).unwrap();
    final_reload.placement.load_session(&map).unwrap();
    sync(&mut final_reload, &map);
    assert!(final_reload.scenario.document().spawns.is_empty());
    assert!(final_reload.scenario.document().data_slots.is_empty());
    assert_eq!(ParticlePool::from_file_bytes(&std::fs::read(map.join("DATA/particle.dat")).unwrap()).unwrap().active_slots().count(), 0);

    drop((app, loaded, reloaded, final_reload));
    std::fs::remove_dir_all(root).unwrap();
}
