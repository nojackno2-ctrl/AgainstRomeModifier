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
