use super::*;

#[test]
fn designer_workflow_e2e() {
    let (root, _) = crate::skirmish_wizard_ui::tests::setup_test_game_fixture("designer_e2e");
    let aliases = root.join("SYSTEM/CLAK/cl_scint.ini");
    let catalog = std::fs::read_to_string(&aliases).unwrap().replace("HOUSE=", "GER_HAU00=");
    std::fs::write(aliases, catalog).unwrap();
    let mut app = EditorApp::initial_state();
    app.recovery.root = Some(root.join("recovery"));
    app.open_skirmish_wizard();
    let wizard = &mut app.skirmish_wizard;
    wizard.game_path_input = root.to_string_lossy().into_owned();
    wizard.source_slot = 1;
    wizard.target_slot = 5;
    wizard.target_name = "Designer workflow".into();
    wizard.briefing = "Hold the base until reinforcements arrive".into();
    wizard.players = 2;
    wizard.add_attack_waves = true;
    wizard.wave_alias = "TROOP".into();
    wizard.wave_count = 2;
    wizard.add_objectives = true;
    assert_eq!(wizard.template_kind, crate::skirmish_wizard_ui::SkirmishTemplateKind::FlatTemplate);
    app.execute_skirmish_wizard().unwrap();
    assert!(!app.environment.is_dirty());
    spawn_save_tests::save_ok(&mut app);
    let map = app.loaded_map_path.clone().unwrap();
    assert!(std::fs::canonicalize(&map).unwrap().starts_with(std::fs::canonicalize(&root).unwrap()));

    // 使用與畫筆相同的筆畫交易，保留其他地形頂點。
    let terrain = app.session.as_mut().unwrap();
    let before = terrain.heights()[0];
    terrain.apply_height_changes(&[crate::terrain::SampleChange { index: 0, before, after: before + 3 }]);
    assert!(terrain.commit_stroke());
    app.textures.load_session(&map).unwrap();
    let materials_before = app.textures.session_textures().unwrap().to_vec();
    let textures = app.textures.session_mut().unwrap();
    assert!(textures.stamp_texture(0, 0, "4BA___51").is_some());
    assert!(textures.commit_stroke());
    assert_ne!(app.textures.session_textures().unwrap(), materials_before);

    app.spawn_ui.sync_with_document(&app.scenario, Some(&map));
    app.spawn_ui.select_type("FigGerInf02_Zweihandaxt");
    app.spawn_ui.set_placement_values(0, 1, 0);
    app.spawn_ui.place_at_tile(&mut app.scenario, &spawn_save_tests::cx(), 10, 10);
    assert!(!app.spawn_ui.status().1, "{}", app.spawn_ui.status().0);
    spawn_save_tests::place_house(&mut app, (12, 12), 0);
    app.scenario.wave_generator.waves = 1;
    app.scenario.wave_generator.first = 600;
    app.scenario.generate_attack_waves().unwrap();
    let troop = app.scenario.document().spawns.iter().find(|s| s.alias == "GER_HAU00").unwrap().id;
    app.scenario.add_objective(None, Some(troop), 0).unwrap();
    app.run_design_check();
    let report = app.design_check.report.as_ref().unwrap();
    assert_eq!(report.error_count(), 0, "{report:?}");
    app.ready_to_test.open = true;
    let ready = app.evaluate_test_readiness();
    assert_eq!(ready.design_errors, 0);
    assert!(ready.unsaved_changes && !ready.is_ready);
    assert!(ready.has_victory && ready.has_defeat);
    assert_eq!(ready.total_waves, 3);
    assert_eq!(ready.waves_with_destination, 3);
    assert!(!ready.has_wave_not_hostile);
    let pending = app.recovery_pending();
    app.recovery.save(&map, true, pending).unwrap();
    app.recovery.flush();
    let snapshot = crate::recovery::snapshot_path(app.recovery.root.as_ref().unwrap(), &map);
    assert!(snapshot.exists());
    spawn_save_tests::save_ok(&mut app);
    app.recovery.flush();
    assert!(!snapshot.exists());
    assert!(!app.has_unsaved_changes());
    let heights = app.session.as_ref().unwrap().heights().to_vec();
    let materials = app.textures.session_textures().unwrap().to_vec();
    let document = app.scenario.document().clone();
    let objects = spawn_save_tests::data_objects(&map);
    assert!(document.spawns.iter().any(|s| s.prebuilt));
    assert!(document.events.iter().flat_map(|e| &e.actions).any(|a| a.kind == 1 && a.hostile));
    let briefing = app.ambience.briefing_title.clone();

    let mut loaded = EditorApp::initial_state();
    loaded.recovery.root = Some(root.join("recovery"));
    loaded.set_loaded_map(map.clone(), crate::storage::load_map(&map).unwrap()).unwrap();
    loaded.scenario.load_session(&map).unwrap();
    loaded.textures.load_session(&map).unwrap();
    assert_eq!(loaded.session.as_ref().unwrap().heights(), heights);
    assert_eq!(loaded.textures.session_textures().unwrap(), materials);
    assert_eq!(loaded.scenario.document(), &document);
    assert_eq!(spawn_save_tests::data_objects(&map).len(), objects.len());
    assert_eq!(loaded.ambience.briefing_title, briefing);
    assert_eq!(briefing, "Hold the base until reinforcements arrive");
    let ready = loaded.evaluate_test_readiness();
    assert_eq!(ready.design_errors, 0);
    assert!(!ready.unsaved_changes);
    assert!(ready.is_ready || ready.is_warning_only);

    let terrain = loaded.session.as_mut().unwrap();
    terrain.apply_height_changes(&[crate::terrain::SampleChange { index: 0, before: heights[0], after: heights[0] + 1 }]);
    assert!(terrain.commit_stroke());
    loaded.collect_pilot_receipts();
    assert!(loaded.undo_journal.can_undo());
    loaded.navigate_journal(false);
    assert_eq!(loaded.session.as_ref().unwrap().heights(), heights);
    assert_eq!(loaded.scenario.document(), &document);
    drop((app, loaded));
    std::fs::remove_dir_all(root).unwrap();
}

// 每個工作流程使用獨立 TEMP 遊戲與編輯器資料目錄，避免碰觸安裝檔或 APPDATA。
fn workflow_fixture(tag: &str) -> PathBuf {
    let (root, _) = crate::skirmish_wizard_ui::tests::setup_test_game_fixture(tag);
    let source = root.join("MAPS/ENDL_001");
    crate::storage::safe_save_map(&source, &mut TerrainSession::create_blank(257, 20)).unwrap();
    for name in ["minimap.bmp", "vertex.bmp", "smooth.bmp", "emboss.bmp"] {
        std::fs::copy(source.join("boden.bmp"), source.join(name)).unwrap();
    }
    std::fs::write(source.join("boden.txt"), format!("[Dimension]\n64\n\n[Texturen]\n{}", "GRAS_1\n".repeat(4096))).unwrap();
    std::fs::write(source.join("boden.ini"), "[Dimension]\n64\n[Waterlevel]\n0.0\n[Heightmapstep]\n4.0\n").unwrap();
    let aliases = root.join("SYSTEM/CLAK/cl_scint.ini");
    // 依原生資料池格式補齊空白初始化所需的來源檔，保留既有物件家族 fixture。
    let specs: &[(&str, usize, usize, &[i32])] = &[
        ("particle", against_rome_data::particle::PAYLOAD_LEN, 1024, &[64]),
        ("lager", 16 + 3200 * 45, 3200, &[6, 10]),
        ("biglager", 12 + 32 * 1601, 32, &[800]),
        ("way", 12 + 10 * 1030, 10, &[256]),
        ("anim", 8 + 200 * 25, 200, &[]),
        ("gfxtype", 8 + 200 * 17, 200, &[]),
        ("action", 12 + 200 * 25, 200, &[6]),
        ("hirarchy", 12 + 10 * 105, 10, &[50]),
        ("formatio", 8 + 10 * 25, 10, &[]),
        ("explos", 8 + 10 * 54, 10, &[]),
        ("hitex", 8 + 10 * 40, 10, &[]),
        ("flash", 12 + 10 * 233, 10, &[16]),
    ];
    for &(name, len, count, extra) in specs {
        let path = source.join(format!("DATA/{name}.dat"));
        if path.exists() { continue; }
        let mut bytes = vec![0; len];
        bytes[..4].copy_from_slice(&1i32.to_le_bytes());
        bytes[4..8].copy_from_slice(&(count as i32).to_le_bytes());
        for (i, value) in extra.iter().enumerate() {
            bytes[8 + i * 4..12 + i * 4].copy_from_slice(&value.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }
    let text = std::fs::read_to_string(&aliases).unwrap().replace("HOUSE=", "GER_HAU00=");
    std::fs::write(aliases, format!("{text}\nGER_INF02=FigGerInf02_Zweihandaxt\n")).unwrap();
    root
}

fn wizard_map(root: &std::path::Path, slot: i32, blank: bool) -> EditorApp {
    let mut app = EditorApp::initial_state();
    app.recovery.root = Some(root.join("editor-data"));
    app.open_skirmish_wizard();
    let w = &mut app.skirmish_wizard;
    w.game_path_input = root.to_string_lossy().into_owned();
    w.source_slot = 1;
    w.target_slot = slot;
    w.target_name = format!("Workflow {slot}");
    w.add_attack_waves = false;
    w.add_objectives = false;
    w.include_nature = false;
    if blank { w.template_kind = crate::skirmish_wizard_ui::SkirmishTemplateKind::EmptyScene; }
    app.execute_skirmish_wizard().unwrap();
    spawn_save_tests::save_ok(&mut app);
    app
}

fn reopen(root: &std::path::Path, map: &std::path::Path) -> EditorApp {
    let mut app = EditorApp::initial_state();
    app.recovery.root = Some(root.join("editor-data"));
    app.set_loaded_map(map.to_owned(), crate::storage::load_map(map).unwrap()).unwrap();
    app.scenario.load_session(map).unwrap();
    app
}

fn map_bytes(map: &std::path::Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn visit(base: &std::path::Path, dir: &std::path::Path, out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() { visit(base, &p, out); }
            else { out.insert(p.strip_prefix(base).unwrap().to_owned(), std::fs::read(p).unwrap()); }
        }
    }
    let mut out = Default::default();
    visit(map, map, &mut out);
    out
}

#[test]
fn experimental_blank_wizard_starter_save_reload_design_check() {
    let root = workflow_fixture("blank_e2e");
    let app = wizard_map(&root, 5, true);
    let map = app.loaded_map_path.clone().unwrap();
    let doc = app.scenario.document().clone();
    assert!(doc.spawns.iter().any(|s| s.prebuilt && s.alias == "GER_HAU00"));
    assert_eq!(doc.spawns.iter().filter(|s| s.alias == "GER_INF02").count(), 4);
    let objects = spawn_save_tests::data_objects(&map);
    assert!(!objects.is_empty());
    let mut loaded = reopen(&root, &map);
    assert_eq!(loaded.scenario.document(), &doc);
    assert_eq!(spawn_save_tests::data_objects(&map).len(), objects.len());
    loaded.run_design_check();
    let report = loaded.design_check.report.as_ref().unwrap();
    assert_eq!(report.error_count(), 0, "{report:?}");
    assert!(!loaded.has_unsaved_changes());
    drop((app, loaded));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn reuse_events_and_script_layout_across_maps_preserves_source() {
    use against_rome_scenario::{Action, Event, Spawn};
    let root = workflow_fixture("reuse_e2e");
    let mut a = wizard_map(&root, 5, false);
    let mut b = wizard_map(&root, 6, false);
    let map_a = a.loaded_map_path.clone().unwrap();
    let map_b = b.loaded_map_path.clone().unwrap();
    a.scenario.add_events(&[Event { name: "可攜外交".into(), actions: vec![Action {
        kind: 1, team: 2, other_team: 3, hostile: true, ..Default::default()
    }], ..Default::default() }]).unwrap();
    a.scenario.apply_spawns(vec![Spawn { id: uuid::Uuid::new_v4(), alias: "TROOP".into(),
        x: 2048., z: 3072., team: 2, count: 3, angle: 90, ..Default::default() }]).unwrap();
    spawn_save_tests::save_ok(&mut a);
    let before = map_bytes(&map_a);
    let events = root.join("reuse.arm-events.json");
    assert_eq!(a.scenario.export_events_to(&events).unwrap(), (1, 0));
    let ids = a.scenario.document().spawns.iter().map(|s| s.id).collect();
    let (layout, skipped) = crate::spawn_layout::SpawnLayout::capture(a.scenario.document(), &ids);
    assert_eq!(skipped, 0);
    let spawns = root.join("reuse.arm-spawns.json");
    crate::layout_io::write_json_atomic(&spawns, &layout.serialize().unwrap()).unwrap();
    assert_eq!(b.scenario.import_events_from(&events, Some((2, 5))).unwrap(), 1);
    let layout = crate::spawn_layout::SpawnLayout::deserialize(&crate::layout_io::read_json_file(&spawns).unwrap()).unwrap();
    let (imported, clamped) = layout.plan(b.scenario.known_aliases(), (4096., 5120.), Some((2, 5))).unwrap();
    assert_eq!(clamped, 0);
    assert_ne!(imported[0].id, a.scenario.document().spawns[0].id);
    b.scenario.apply_spawns(imported).unwrap();
    let expected = b.scenario.document().clone();
    spawn_save_tests::save_ok(&mut b);
    let loaded = reopen(&root, &map_b);
    assert_eq!(loaded.scenario.document(), &expected);
    let action = &expected.events.last().unwrap().actions[0];
    assert_eq!((action.team, action.other_team, action.hostile), (5, 3, true));
    let spawn = expected.spawns.last().unwrap();
    assert_eq!((spawn.team, spawn.x, spawn.z, spawn.count, spawn.angle), (5, 4096., 5120., 3, 90));
    assert_eq!(map_bytes(&map_a), before);
    drop((a, b, loaded));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn recovery_restart_restore_save_and_discard_preserve_disk_contract() {
    let root = workflow_fixture("recovery_e2e");
    let mut app = wizard_map(&root, 5, false);
    let map = app.loaded_map_path.clone().unwrap();
    let before = map_bytes(&map);
    let terrain = app.session.as_mut().unwrap();
    let height = terrain.heights()[0];
    terrain.apply_height_changes(&[crate::terrain::SampleChange { index: 0, before: height, after: height + 7 }]);
    assert!(terrain.commit_stroke());
    app.scenario.add_events(&[against_rome_scenario::Event { name: "復原事件".into(), actions: vec![against_rome_scenario::Action { kind: 1, team: 2, other_team: 3, hostile: true, ..Default::default() }], ..Default::default() }]).unwrap();
    let heights = app.session.as_ref().unwrap().heights().to_vec();
    let doc = app.scenario.document().clone();
    let pending = app.recovery_pending();
    assert!(app.recovery.save(&map, true, pending).unwrap());
    app.recovery.flush();
    assert!(app.recovery.poll().is_none());
    let snapshot = crate::recovery::snapshot_path(app.recovery.root.as_ref().unwrap(), &map);
    assert!(snapshot.is_file());
    drop(app);
    let mut restarted = reopen(&root, &map);
    assert!(!restarted.has_unsaved_changes());
    let offer = restarted.recovery.offer.take().unwrap().unwrap();
    restarted.restore_recovery(&offer).unwrap();
    assert_eq!(restarted.session.as_ref().unwrap().heights(), heights);
    assert_eq!(restarted.scenario.document(), &doc);
    assert!(restarted.has_unsaved_changes());
    assert_eq!(map_bytes(&map), before);
    spawn_save_tests::save_ok(&mut restarted);
    restarted.recovery.flush();
    assert!(!snapshot.exists());
    drop(restarted);
    let mut loaded = reopen(&root, &map);
    assert_eq!(loaded.session.as_ref().unwrap().heights(), heights);
    assert_eq!(loaded.scenario.document(), &doc);
    let saved = map_bytes(&map);
    loaded.scenario.add_events(&[against_rome_scenario::Event { name: "捨棄事件".into(), actions: vec![against_rome_scenario::Action { kind: 1, team: 2, other_team: 3, hostile: false, ..Default::default() }], ..Default::default() }]).unwrap();
    let pending = loaded.recovery_pending();
    assert!(loaded.recovery.save(&map, true, pending).unwrap());
    loaded.recovery.flush();
    drop(loaded);
    let mut discarded = reopen(&root, &map);
    assert!(discarded.recovery.offer.as_ref().unwrap().is_ok());
    discarded.clear_recovery();
    discarded.recovery.flush();
    assert!(!snapshot.exists());
    assert_eq!(discarded.scenario.document(), &doc);
    assert!(!discarded.has_unsaved_changes());
    assert_eq!(map_bytes(&map), saved);
    drop(discarded);
    let reopened = reopen(&root, &map);
    assert!(reopened.recovery.offer.is_none());
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn packaging_temp_map_zip_payload_hashes_and_safe_default_directory() {
    use against_rome_packaging::exporter::ModBundleExporter;
    let root = workflow_fixture("packaging_e2e");
    let mut app = wizard_map(&root, 5, false);
    let map = app.loaded_map_path.clone().unwrap();
    let before = map_bytes(&map);
    let default = crate::packaging_ui::default_export_zip_path(&map);
    assert!(!crate::packaging_ui::is_game_maps_dir(default.parent().unwrap()));
    assert!(!default.starts_with(root.join("MAPS")));
    let zip = root.join("export.zip");
    app.packaging.map_dir_input = map.to_string_lossy().into_owned();
    app.packaging.output_zip_input = zip.to_string_lossy().into_owned();
    app.packaging.generate_thumbnail = false;
    app.packaging.run_export().unwrap();
    assert!(zip.is_file());
    // verify_zip 實際讀取所有 ZIP payload 並驗證 SHA-256；再與來源逐檔比對。
    let manifest = ModBundleExporter::verify_zip(&zip).unwrap();
    assert!(!manifest.files.is_empty());
    for entry in &manifest.files {
        let bytes = before.get(&PathBuf::from(&entry.relative_path)).unwrap();
        let hash: String = against_rome_data::sha256::sha256(bytes).iter().map(|b| format!("{b:02X}")).collect();
        assert_eq!(entry.sha256.to_uppercase(), hash, "{}", entry.relative_path);
    }
    for name in ["boden.bmp", "collision.bmp", "SCRIPT/ak_level.bci"] {
        assert!(manifest.files.iter().any(|e| e.relative_path == name), "{name}");
    }
    assert_eq!(map_bytes(&map), before);
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
