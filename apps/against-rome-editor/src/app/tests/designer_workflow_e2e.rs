use super::*;

// 儲存建立新的復原界線；逐項驗證清空歷程後的 Undo 不會復活舊資料。

fn cycle_edit(app: &mut EditorApp, system: &str, cycle: usize) {
    match system {
        "terrain" => {
            let s = app.session.as_mut().unwrap();
            let before = s.heights()[0];
            s.apply_height_changes(&[crate::terrain::SampleChange { index: 0, before, after: before + 1 }]);
            assert!(s.commit_stroke());
        }
        "materials" => {
            let s = app.textures.session_mut().unwrap();
            assert!(s.stamp_texture(cycle, 0, "4BB___51").is_some());
            assert!(s.commit_stroke());
        }
        "nature" => {
            app.ensure_nature_data().unwrap();
            let item = app.nature_palette().first().cloned().unwrap();
            let old = app.placement.objects().into_iter().find(|o| i32::from(o.type_id) == item.type_id).unwrap();
            assert!(app.nature.session.remove(&[old.slot as i32], &[]));
            app.nature.session.plant(against_rome_environment::nature::NatureAddition::new(
                item.type_id, &item.name, 3000.0 + cycle as f32 * 256.0, 0.0, 3000.0, 0.0));
            assert!(app.nature.session.commit_stroke());
        }
        "placement" | "families" => {
            let o = app.placement.objects().into_iter().find(|o| o.parent.is_none()).unwrap();
            app.placement.select_object(Some((o.slot, o.uid)));
            app.placement.set_delta(["128".into(), "0".into(), "128".into()]);
            if system == "families" {
                let store = app.placement.store_mut().unwrap();
                let source = crate::family_ui::validate_native_source(store, o.slot, &Default::default()).unwrap();
                assert_eq!(source.light_count, 1);
                crate::family_ui::copy_family(store, &source, &Default::default(), [128 * 65536, 0, 128 * 65536]).unwrap();
                app.placement.note_external_store_edit();
            }
            else { app.placement.move_family().unwrap(); }
        }
        "scene" => app.scene.set_team(0, cycle as i32 + 1).unwrap(),
        "spawns" => {
            app.spawn_ui.sync_with_document(&app.scenario, app.loaded_map_path.as_deref());
            app.spawn_ui.select_type("FigGerInf02_Zweihandaxt");
            app.spawn_ui.set_placement_values(1, 1, 0);
            app.spawn_ui.place_at_tile(&mut app.scenario, &spawn_save_tests::cx(), 20 + cycle as i32, 20);
            assert!(!app.spawn_ui.status().1, "{}", app.spawn_ui.status().0);
        }
        "events" => { app.scenario.add_events(&[against_rome_scenario::Event {
            name: format!("循環 {cycle}"), actions: vec![against_rome_scenario::Action {
                kind: 1, team: 1, other_team: 2, hostile: true, ..Default::default()
            }], ..Default::default()
        }]).unwrap(); }
        "ambience" => {
            app.ambience.briefing_title = format!("Cycle {cycle}");
            app.ambience.briefing_body = format!("Briefing {cycle}");
            app.ambience.profile.boden.waterlevel = cycle as f32 + 1.0;
            app.ambience.mark_dirty();
        }
        "camera" => {
            app.campaign.start_camera.enabled = true;
            app.campaign.start_camera.x = 4096.0 + cycle as f64 * 256.0;
            app.apply_start_camera_candidate().unwrap();
        }
        _ => unreachable!(),
    }
    assert!(app.has_unsaved_changes(), "{system} {cycle}");
}

fn cycle_state(app: &EditorApp) -> Vec<String> {
    vec![format!("{:?}", app.session.as_ref().unwrap().heights()),
        format!("{:?}", app.textures.session_textures()),
        format!("{:?}", app.placement.objects()),
        serde_json::to_string(app.scenario.document()).unwrap(),
        format!("{:?}", app.scene.objects()),
        format!("{:?}", (&app.ambience.briefing_title, &app.ambience.briefing_body, app.ambience.profile.boden.waterlevel)),
        format!("{:?}", app.campaign.start_camera)]
}

fn cycle_load_layers(app: &mut EditorApp, map: &std::path::Path) {
    app.textures.load_session(map).unwrap();
    app.placement.load_session(map).unwrap();
}

fn subsystem_three_safe_saves_reload_and_save_boundary_undo(system: &str) {
        let root = workflow_fixture(&format!("cycles_{system}"));
        let mut app = wizard_map(&root, 5, false);
        let map = app.loaded_map_path.clone().unwrap();
        // 直接建立小型地景來源，避免聚落產生器的隨機森林搜尋干擾儲存測試。
        spawn_save_tests::write_data(&map, 200, &[(232, None), (234, Some(0)), (10, None)], 100);
        let mut columns = vec!["0".to_owned(); 53];
        columns[0] = "10".into();
        columns[1] = "1".into();
        columns[52] = "LanGerNad00_Tanne".into();
        let defs = root.join("SYSTEM/DATA_MP/DEFAULTS/objdef.dau");
        let mut text = std::fs::read_to_string(&defs).unwrap();
        text.push_str(&format!("\n{}\n", columns.join(",")));
        std::fs::write(defs, text).unwrap();
        // 根物件連結一盞光源，讓 Families & Effects 真正跨池複製並儲存。
        let path = map.join("DATA/objects.dat");
        let mut objects = std::fs::read(&path).unwrap();
        let light_ref = 16 + 200 * 79 + 200 * against_rome_placement::COLUMNS[..12].iter().sum::<usize>();
        objects[light_ref..light_ref + 2].copy_from_slice(&0u16.to_le_bytes());
        std::fs::write(path, objects).unwrap();
        let mut lights = std::fs::read(map.join("DATA/light.dat")).unwrap();
        lights[8..12].copy_from_slice(&1u32.to_le_bytes());
        let mut offset = 8;
        for (column, width) in against_rome_data::pool::WIDTHS.iter().enumerate() {
            if column >= 13 { lights[offset..offset + 1024 * width].fill(255); }
            offset += 1024 * width;
        }
        let owner = 8 + 1024 * against_rome_data::pool::WIDTHS[..13].iter().sum::<usize>();
        lights[owner..owner + 2].copy_from_slice(&0u16.to_le_bytes());
        std::fs::write(map.join("DATA/light.dat"), lights).unwrap();
        std::fs::write(map.join("cycle.sdl"), "[settlement]\nrefpos=1000,0,1000\n[object0000]\ndef=232\nnamedef=BauGerHau00_Haupthaus\npos=100,0,100\nteam=0\nangle=0\n").unwrap();
        app.scene.reset_session(Some(&map));
        cycle_load_layers(&mut app, &map);
        app.nature.names = None;
        app.nature.data = None;
        for cycle in 0..3 {
            eprintln!("Task41 {system}: 儲存循環 {}", cycle + 1);
            cycle_edit(&mut app, system, cycle);
            app.collect_pilot_receipts();
            let before_objects = spawn_save_tests::data_objects(&map).len();
            let nature = app.nature.session.additions().to_vec();
            let edited = cycle_state(&app);
            spawn_save_tests::save_ok(&mut app);
            assert!(!app.has_unsaved_changes(), "{system} {cycle}");
            let saved = cycle_state(&app);
            if system != "nature" {
                assert_eq!(saved, edited, "儲存保留編輯：{system} {cycle}");
            }
            let lights = std::fs::read(map.join("DATA/light.dat")).unwrap();
            if system == "families" {
                assert_eq!(against_rome_data::LightPool::from_payload(&lights).unwrap().active_slots().count(), cycle + 2);
            }
            let disk = map_bytes(&map);
            app.navigate_journal(false);
            assert_eq!(cycle_state(&app), saved, "儲存界線 Undo：{system} {cycle}");
            assert!(!app.has_unsaved_changes(), "{system} {cycle}");
            assert_eq!(map_bytes(&map), disk);
            // 即使 Undo 在新界線沒有項目，再次儲存仍須成功。
            spawn_save_tests::save_ok(&mut app);
            let mut loaded = reopen(&root, &map);
            cycle_load_layers(&mut loaded, &map);
            assert_eq!(cycle_state(&loaded), saved, "重載：{system} {cycle}");
            assert!(!loaded.has_unsaved_changes());
            if system == "nature" {
                let objects = spawn_save_tests::data_objects(&map);
                assert_eq!(objects.len(), before_objects);
                for a in nature { assert!(objects.iter().any(|o| o.position[0] == a.x && o.position[2] == a.z)); }
            }
        }
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
}

macro_rules! save_cycle_test {
    ($name:ident, $system:literal) => {
        #[test]
        fn $name() { subsystem_three_safe_saves_reload_and_save_boundary_undo($system); }
    };
}
save_cycle_test!(three_safe_saves_terrain, "terrain");
save_cycle_test!(three_safe_saves_materials, "materials");
save_cycle_test!(three_safe_saves_nature, "nature");
save_cycle_test!(three_safe_saves_placement, "placement");
save_cycle_test!(three_safe_saves_families_effects, "families");
save_cycle_test!(three_safe_saves_scene_sdl, "scene");
save_cycle_test!(three_safe_saves_script_spawns, "spawns");
save_cycle_test!(three_safe_saves_scenario_events, "events");
save_cycle_test!(three_safe_saves_ambience_briefing, "ambience");
save_cycle_test!(three_safe_saves_campaign_camera, "camera");

#[test]
fn large_map_near_event_limit_design_save_reload_timings() {
    let total = std::time::Instant::now();
    let root = workflow_fixture("stress_41");
    eprintln!("Task43 fixture 來源={:?}", total.elapsed());
    let mut app = wizard_map(&root, 5, false);
    eprintln!("Task42 fixture 建立／精靈={:?}", total.elapsed());
    let setup = std::time::Instant::now();
    let map = app.loaded_map_path.clone().unwrap();
    let live = vec![(232, None); 256];
    spawn_save_tests::write_data(&map, 512, &live, 1000);
    for (name, stride, extra) in [("anim", 25, false), ("gfxtype", 17, false), ("action", 25, true)] {
        let header = if extra { 12 } else { 8 };
        let mut bytes = vec![0; header + 512 * stride];
        bytes[..4].copy_from_slice(&1i32.to_le_bytes());
        bytes[4..8].copy_from_slice(&512i32.to_le_bytes());
        if extra { bytes[8..12].copy_from_slice(&6i32.to_le_bytes()); }
        std::fs::write(map.join(format!("DATA/{name}.dat")), bytes).unwrap();
    }
    app.placement.load_session(&map).unwrap();
    app.placement.select_all();
    app.placement.set_delta(["128".into(), "0".into(), "128".into()]);
    app.placement.move_family().unwrap();
    assert_eq!(app.session.as_ref().unwrap().vertex_size, 257);
    let spawns: Vec<_> = (0..512).map(|i| against_rome_scenario::Spawn {
        id: uuid::Uuid::new_v4(), alias: "TROOP".into(), team: 1, count: 1,
        x: 512.0 + (i % 32) as f32 * 256.0, z: 512.0 + (i / 32) as f32 * 256.0,
        ..Default::default()
    }).collect();
    eprintln!("Task42 DATA 建立／載入／移動={:?}", setup.elapsed());
    let scenario_start = std::time::Instant::now();
    app.scenario.apply_spawns(spawns).unwrap();
    let events: Vec<_> = (0..256).map(|i| against_rome_scenario::Event {
        name: format!("壓力事件 {i}"), delay_seconds: i,
        actions: vec![against_rome_scenario::Action { kind: 1, team: 1, other_team: 2, hostile: true, ..Default::default() }],
        ..Default::default()
    }).collect();
    app.scenario.add_events(&events).unwrap();
    eprintln!("Task42 劇本建構={:?}，fixture 總計={:?}", scenario_start.elapsed(), total.elapsed());
    let start = std::time::Instant::now();
    app.run_design_check();
    assert_eq!(app.design_check.report.as_ref().unwrap().error_count(), 0, "{:?}", app.design_check.report);
    let check = start.elapsed();
    let measured = std::time::Instant::now();
    let _summary = app.compute_balance_summary();
    eprintln!("Task43 平衡摘要={:?}", measured.elapsed());
    let measured = std::time::Instant::now();
    let _readiness = app.evaluate_test_readiness();
    eprintln!("Task43 準備測試={:?}", measured.elapsed());
    let measured = std::time::Instant::now();
    let staged = app.scenario.stage_transaction_files(&map).unwrap();
    eprintln!("Task43 BCI 編譯與 staging={:?}", measured.elapsed());
    assert_eq!(staged, app.scenario.stage_transaction_files(&map).unwrap());
    let measured = std::time::Instant::now();
    let pending = app.recovery_pending();
    assert!(app.recovery.save(&map, true, pending).unwrap());
    app.recovery.flush();
    assert!(app.recovery.poll().is_none());
    assert!(crate::recovery::snapshot_path(app.recovery.root.as_ref().unwrap(), &map).is_file());
    eprintln!("Task43 復原快照（含 flush）={:?}", measured.elapsed());
    let measured = std::time::Instant::now();
    spawn_save_tests::save_ok(&mut app);
    let save = measured.elapsed();
    let measured = std::time::Instant::now();
    let mut loaded = reopen(&root, &map);
    let reload = measured.elapsed();
    let ctx = egui::Context::default();
    let measured = std::time::Instant::now();
    let output = ctx.run(egui::RawInput::default(), |ctx| loaded.ui_frame(ctx));
    assert!(!output.shapes.is_empty());
    eprintln!("Task43 無頭首幀（不含 GPU）={:?}", measured.elapsed());
    let measured = std::time::Instant::now();
    let terrain = loaded.session.as_mut().unwrap();
    let baseline = terrain.heights().to_vec();
    terrain.paint_height(128.0, 128.0, 120.0, HeightBrushOp::Raise, 15, None, 0).unwrap();
    assert!(terrain.commit_stroke());
    let painted = terrain.heights().to_vec();
    eprintln!("Task43 大筆畫={:?}", measured.elapsed());
    let measured = std::time::Instant::now();
    assert!(terrain.undo());
    assert_eq!(terrain.heights(), baseline);
    eprintln!("Task43 大筆畫 undo={:?}", measured.elapsed());
    let measured = std::time::Instant::now();
    assert!(terrain.redo());
    assert_eq!(terrain.heights(), painted);
    eprintln!("Task43 大筆畫 redo={:?}", measured.elapsed());
    assert!(terrain.undo());
    eprintln!("Task41 257x257 / 256 DATA objects / 512 spawns / 256 events: Design Check={check:?}, Safe Save={save:?}, reload={reload:?}");
    assert!(start.elapsed() < std::time::Duration::from_secs(120));
    assert_eq!(loaded.scenario.document(), app.scenario.document());
    // 同一大型候選資料比較舊路徑與批次路徑的完整診斷文字位元組，並確認唯讀。
    let disk_before = map_bytes(&map);
    let store = app.placement.detached_store().unwrap();
    let snapshot = against_rome_maps::diagnostics::MapCheckSnapshot::new(Vec::new(), Vec::new(), Vec::new(), 0, None);
    let parity_start = std::time::Instant::now();
    let legacy = crate::design_check_ui::data_candidate_issues_legacy(&store, &snapshot, &mut None);
    let legacy_time = parity_start.elapsed();
    let parity_start = std::time::Instant::now();
    let batch = crate::design_check_ui::data_candidate_issues(&store, &snapshot, &mut None);
    eprintln!("Task42 相同 fixture 診斷：舊路徑={legacy_time:?}，批次={:?}", parity_start.elapsed());
    assert_eq!(format!("{legacy:?}").as_bytes(), format!("{batch:?}").as_bytes());
    assert_eq!(map_bytes(&map), disk_before);
    assert_eq!(loaded.scenario.document().events.len(), 256);
    assert_eq!(loaded.scenario.document().spawns.len(), 512);
    assert_eq!(spawn_save_tests::data_objects(&map), app.placement.objects());
    assert_eq!(loaded.session.as_ref().unwrap().heights(), app.session.as_ref().unwrap().heights());
    assert!(!loaded.has_unsaved_changes());
    // 自製 APT ZIP 與有效定義，量測真正的載入、解析、投影與尋路，禁止零售資產存取。
    std::fs::write(root.join("SYSTEM/clengine.ini"),
        "[DefaultPath]\nSYSTEM/DATA_MP/DEFAULTS\n[AptPath]\nmap/DATA\n").unwrap();
    std::fs::write(root.join("SYSTEM/cl_apt.ini"), "[AptNames]\n0,fixture.dat\n").unwrap();
    let mut columns = vec!["0"; 53];
    columns[0] = "232";
    columns[1] = "1";
    columns[52] = "BauGerHau00_Haupthaus";
    std::fs::write(root.join("SYSTEM/DATA_MP/DEFAULTS/objdef.dau"),
        format!("[ObjectDefaults]\n{}\n", columns.join(","))).unwrap();
    let mut apt = vec![0u8; 416 + 101 * 12];
    apt[..4].copy_from_slice(b"APAT");
    for (at, value) in [(4, 3u32), (12, 112), (36, 1), (384, 1), (396, 101)] {
        apt[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    for i in 0..101 {
        let at = 416 + i * 12;
        apt[at..at + 4].copy_from_slice(&1u32.to_le_bytes());
        apt[at + 4..at + 8].copy_from_slice(&(((i % 11) as i32 - 5) * 64).to_le_bytes());
    }
    // 沿用現有 Rust ZIP 匯出器，無新增相依套件或手刻 ZIP 格式。
    std::fs::write(map.join("DATA/fixture.dat"), apt).unwrap();
    let options = against_rome_packaging::exporter::ModExportOptions {
        clean_caches: false, force_export_on_errors: true, generate_thumbnail: false,
        ..Default::default()
    };
    against_rome_packaging::exporter::ModBundleExporter::export_to_zip(
        &map, &root.join("apt.dat"), Some(&options)).unwrap();
    std::fs::remove_file(map.join("DATA/fixture.dat")).unwrap();
    loaded.maps.game_path_input = root.to_string_lossy().into_owned();
    loaded.placement.load_session(&map).unwrap();
    loaded.placement.set_type_names(std::collections::HashMap::from([(232, "BauGerHau00_Haupthaus".into())]));
    loaded.navigation.open = true;
    let collision_before = loaded.session.as_ref().unwrap().collision().to_vec();
    let measured = std::time::Instant::now();
    loaded.update_navigation_objects_if_needed();
    let (route, fallbacks, cells) = loaded.navigation.analyze_apt_for_test(
        loaded.session.as_ref().unwrap(), loaded.water_surface).unwrap();
    eprintln!("Task43 APT 冷載入／256 建築／101 格占地／路徑={:?}", measured.elapsed());
    assert_eq!(fallbacks, 0);
    assert_eq!(cells, 256 * 101);
    assert!(route.path_cells.is_some_and(|count| count > 1));
    let measured = std::time::Instant::now();
    loaded.update_navigation_objects_if_needed();
    let (warm, _, _) = loaded.navigation.analyze_apt_for_test(
        loaded.session.as_ref().unwrap(), loaded.water_surface).unwrap();
    eprintln!("Task43 APT 快取／路徑={:?}", measured.elapsed());
    assert_eq!(warm.path_cells, route.path_cells);
    assert_eq!(warm.passable, route.passable);
    assert_eq!(loaded.session.as_ref().unwrap().collision(), collision_before);
    assert_eq!(map_bytes(&map), disk_before);
    drop((app, loaded));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unsaved_map_switch_prompt_discard_and_recovery_isolation() {
    let root = workflow_fixture("switch_41");
    let mut a = wizard_map(&root, 5, false);
    let b = wizard_map(&root, 6, false);
    let map_a = a.loaded_map_path.clone().unwrap();
    let map_b = b.loaded_map_path.clone().unwrap();
    let disk_a = map_bytes(&map_a);
    let disk_b = map_bytes(&map_b);
    cycle_edit(&mut a, "terrain", 0);
    cycle_edit(&mut a, "events", 0);
    let pending = a.recovery_pending();
    a.recovery.save(&map_a, true, pending).unwrap();
    a.recovery.flush();
    let snapshot = crate::recovery::snapshot_path(a.recovery.root.as_ref().unwrap(), &map_a);
    assert!(snapshot.exists());
    // 重啟開啟 B 時，A 的復原候選不得套用或顯示。
    let other = reopen(&root, &map_b);
    assert!(other.recovery.offer.is_none());
    assert!(!other.has_unsaved_changes());
    assert_eq!(other.scenario.document(), b.scenario.document());
    assert!(a.set_loaded_map(map_b.clone(), crate::storage::load_map(&map_b).unwrap()).is_err());
    assert_eq!(a.loaded_map_path.as_ref(), Some(&map_a));
    a.pending_action = Some(PendingAction::Load(map_b.clone()));
    let ctx = egui::Context::default();
    let _ = ctx.run(Default::default(), |ctx| a.handle_pending_action(ctx));
    let output = ctx.run(Default::default(), |ctx| a.handle_pending_action(ctx));
    assert_eq!(a.loaded_map_path.as_ref(), Some(&map_a));
    assert!(a.pending_action.is_some());
    let pos = output.shapes.iter().find_map(|s| match &s.shape {
        egui::epaint::Shape::Text(t) if t.galley.text() == tr(a.lang(), "Discard unsaved changes", "捨棄未儲存的變更") =>
            Some(Rect::from_min_size(t.pos, t.galley.size()).center()),
        _ => None,
    }).expect("顯示捨棄確認按鈕");
    for pressed in [true, false] {
        let _ = ctx.run(egui::RawInput { events: vec![egui::Event::PointerMoved(pos),
            egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }],
            ..Default::default() }, |ctx| a.handle_pending_action(ctx));
    }
    a.recovery.flush();
    assert!(a.pending_action.is_none());
    assert_eq!(a.loaded_map_path.as_ref(), Some(&map_b));
    assert!(!a.has_unsaved_changes());
    assert!(a.recovery.offer.is_none());
    assert!(!snapshot.exists());
    a.scenario.load_session(&map_b).unwrap();
    assert_eq!(a.scenario.document(), b.scenario.document());
    assert_eq!(a.session.as_ref().unwrap().heights(), b.session.as_ref().unwrap().heights());
    assert_eq!(map_bytes(&map_a), disk_a);
    assert_eq!(map_bytes(&map_b), disk_b);
    drop((a, b, other));
    std::fs::remove_dir_all(root).unwrap();
}

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
    wizard_map_custom(root, slot, blank, false)
}

fn wizard_map_custom(root: &std::path::Path, slot: i32, blank: bool, nature: bool) -> EditorApp {
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
    w.include_nature = nature;
    if blank { w.template_kind = crate::skirmish_wizard_ui::SkirmishTemplateKind::EmptyScene; }
    app.execute_skirmish_wizard().unwrap();
    if blank {
        let expected = app.scenario.document().clone();
        assert!(expected.spawns.iter().any(|s| s.prebuilt));
        assert!(app.scenario.stage_transaction_files(app.loaded_map_path.as_ref().unwrap()).is_err());
        app.collect_pilot_receipts();
        app.navigate_journal(false);
        assert!(app.scenario.document().spawns.is_empty());
        app.navigate_journal(true);
        assert_eq!(app.scenario.document(), &expected);
    }
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
    let reset_report = &app.maps.blank_last_result.as_ref().unwrap().reset_report;
    assert!(reset_report.reset_data_files.contains(&"DATA/objects.dat".to_string()));
    assert!(reset_report.reset_data_files.contains(&"DATA/position.dat".to_string()));
    assert!(reset_report.generated_script_files.contains(&"SCRIPT/ak_level.bci".to_string()));
    let map = app.loaded_map_path.clone().unwrap();
    let doc = app.scenario.document().clone();
    assert!(doc.spawns.iter().any(|s| s.prebuilt && s.alias == "GER_HAU00"));
    assert_eq!(doc.spawns.iter().filter(|s| s.alias == "GER_INF02").count(), 4);
    let objects = spawn_save_tests::data_objects(&map);
    assert!(!objects.is_empty());
    let active: Vec<_> = objects.iter().map(|o| (o.slot as i32, o.uid)).collect();
    against_rome_scenario::verify_binding_snapshot(&doc, &active).unwrap();
    for spawn in doc.spawns.iter().filter(|s| s.prebuilt) {
        assert!(doc.data_slots.iter().any(|b| b.spawn_id == spawn.id));
    }
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
    // Packaging 報告狀態不是地圖內容 dirty，不應阻擋匯出。
    app.packaging.dirty = true;
    app.packaging.request_export();
    app.process_packaging_export_request();
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

#[test]
fn packaging_export_rejects_source_and_game_maps_destinations_but_allows_external_zip() {
    use against_rome_packaging::exporter::ModBundleExporter;

    let root = workflow_fixture("packaging_destination_guard");
    let mut app = wizard_map(&root, 5, false);
    // Mark only this already-created TEMP fixture as a game root so detect_game_root
    // can distinguish its MAPS folder from an arbitrary directory named "MAPS".
    std::fs::write(root.join("Against_Rome.exe"), []).unwrap();
    let map = app.loaded_map_path.clone().unwrap();
    let before = map_bytes(&map);
    app.packaging.map_dir_input = map.to_string_lossy().into_owned();
    app.packaging.generate_thumbnail = false;

    let nested_target = map.join("nested/unsafe.zip");
    app.packaging.output_zip_input = nested_target.to_string_lossy().into_owned();
    app.packaging.request_export();
    app.process_packaging_export_request();
    assert!(!nested_target.exists());
    assert!(!map.join("nested").exists(), "rejected export must not create directories or temp files");
    assert!(app.packaging.status_is_error);
    assert!(app.packaging.status_message.contains("source map"));
    assert_eq!(map_bytes(&map), before);

    let maps_dir = root.join("MAPS");
    let maps_entries_before: std::collections::BTreeSet<_> = std::fs::read_dir(&maps_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    let game_maps_target = root.join("MAPS/unsafe.zip");
    app.packaging.output_zip_input = game_maps_target.to_string_lossy().into_owned();
    app.packaging.request_export();
    app.process_packaging_export_request();
    assert!(!game_maps_target.exists());
    assert!(app.packaging.status_is_error);
    assert!(app.packaging.status_message.contains("game MAPS"));
    let maps_entries_after: std::collections::BTreeSet<_> = std::fs::read_dir(&maps_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(maps_entries_after, maps_entries_before, "rejected export must create no ZIP or sibling temporary file");
    assert_eq!(map_bytes(&map), before);

    let external_zip = root.join("outside/export.zip");
    app.packaging.output_zip_input = external_zip.to_string_lossy().into_owned();
    app.packaging.request_export();
    app.process_packaging_export_request();
    assert!(external_zip.is_file(), "external destination should be created");
    let manifest = ModBundleExporter::verify_zip(&external_zip).unwrap();
    assert!(!manifest.files.is_empty());
    for entry in &manifest.files {
        let bytes = before.get(&PathBuf::from(&entry.relative_path)).unwrap();
        let hash: String = against_rome_data::sha256::sha256(bytes)
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect();
        assert_eq!(entry.sha256.to_uppercase(), hash, "{}", entry.relative_path);
    }
    assert_eq!(map_bytes(&map), before, "export must not alter source map files");
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn packaging_export_blocks_unsaved_loaded_map_and_safe_save_retry_contains_current_edits() {
    use against_rome_packaging::exporter::ModBundleExporter;

    let root = workflow_fixture("packaging_unsaved_guard");
    let mut app = wizard_map(&root, 5, false);
    let map = app.loaded_map_path.clone().unwrap();
    let before = map_bytes(&map);
    let zip = root.join("unsaved-export.zip");
    app.packaging.map_dir_input = map.to_string_lossy().into_owned();
    app.packaging.output_zip_input = zip.to_string_lossy().into_owned();
    app.packaging.generate_thumbnail = false;

    let terrain = app.session.as_mut().unwrap();
    let old_height = terrain.heights()[0];
    terrain.apply_height_changes(&[crate::terrain::SampleChange {
        index: 0,
        before: old_height,
        after: old_height + 1,
    }]);
    assert!(terrain.commit_stroke());
    app.scenario
        .add_events(&[against_rome_scenario::Event {
            name: "Unsaved export guard".into(),
            actions: vec![against_rome_scenario::Action {
                kind: 1,
                team: 1,
                other_team: 2,
                hostile: true,
                ..Default::default()
            }],
            ..Default::default()
        }])
        .unwrap();
    let can_undo_before = app.undo_journal.can_undo();
    assert!(app.has_unsaved_map_content());

    app.packaging.request_export();
    app.process_packaging_export_request();
    assert!(!zip.exists(), "blocked export must not create a package");
    assert_eq!(map_bytes(&map), before, "blocked export must not write map files");
    assert!(app.packaging.status_message.contains("Safe Save"));
    assert!(app.status_is_error || app.packaging.status_is_error);
    assert!(app.session.as_ref().unwrap().is_dirty());
    assert!(app.scenario.dirty());
    assert_eq!(app.undo_journal.can_undo(), can_undo_before);

    spawn_save_tests::save_ok(&mut app);
    assert!(!app.has_unsaved_map_content());
    app.packaging.request_export();
    app.process_packaging_export_request();
    assert!(zip.is_file(), "retry after Safe Save should produce the ZIP");
    let manifest = ModBundleExporter::verify_zip(&zip).unwrap();
    for name in ["boden.bmp", "arm_scenario.json"] {
        let saved = std::fs::read(map.join(name)).unwrap();
        let expected_hash: String = against_rome_data::sha256::sha256(&saved)
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect();
        let entry = manifest
            .files
            .iter()
            .find(|entry| entry.relative_path == name)
            .unwrap_or_else(|| panic!("ZIP manifest is missing {name}"));
        assert_eq!(entry.sha256.to_uppercase(), expected_hash, "{name}");
    }

    // A dirty editor may still package a distinct, explicitly selected disk map.
    let external_root = workflow_fixture("packaging_external_source");
    let external_app = wizard_map(&external_root, 6, false);
    let external_map = external_app.loaded_map_path.clone().unwrap();
    let external_zip = root.join("external-source.zip");
    let terrain = app.session.as_mut().unwrap();
    let old_height = terrain.heights()[1];
    terrain.apply_height_changes(&[crate::terrain::SampleChange {
        index: 1,
        before: old_height,
        after: old_height + 1,
    }]);
    assert!(terrain.commit_stroke());
    app.packaging.map_dir_input = external_map.to_string_lossy().into_owned();
    app.packaging.output_zip_input = external_zip.to_string_lossy().into_owned();
    app.packaging.request_export();
    app.process_packaging_export_request();
    assert!(external_zip.is_file(), "a distinct source map remains exportable");
    assert!(app.has_unsaved_map_content());
    drop(external_app);
    drop(app);
    std::fs::remove_dir_all(external_root).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn packaging_overwrite_confirmation_rechecks_map_dirty_state_before_replacing_zip() {
    let root = workflow_fixture("packaging_overwrite_dirty_guard");
    let mut app = wizard_map(&root, 5, false);
    let map = app.loaded_map_path.clone().unwrap();
    let zip = root.join("existing-export.zip");
    let previous_zip = b"previous verified package";
    std::fs::write(&zip, previous_zip).unwrap();
    app.packaging.map_dir_input = map.to_string_lossy().into_owned();
    app.packaging.output_zip_input = zip.to_string_lossy().into_owned();

    app.packaging.request_export();
    app.process_packaging_export_request();
    assert!(app.packaging.pending_overwrite_confirm);
    assert_eq!(std::fs::read(&zip).unwrap(), previous_zip);

    // A clean map switch resets the pending overwrite prompt rather than carrying its approval
    // to the newly loaded source.
    let mut next_map_app = wizard_map(&root, 6, false);
    let next_map = next_map_app.loaded_map_path.clone().unwrap();
    let next_session = next_map_app.session.take().unwrap();
    app.set_loaded_map(next_map.clone(), next_session).unwrap();
    assert!(!app.packaging.pending_overwrite_confirm);
    assert_eq!(std::fs::read(&zip).unwrap(), previous_zip);
    drop(next_map_app);
    app.packaging.map_dir_input = next_map.to_string_lossy().into_owned();
    app.packaging.output_zip_input = zip.to_string_lossy().into_owned();

    app.packaging.request_export();
    app.process_packaging_export_request();
    assert!(app.packaging.pending_overwrite_confirm);

    let terrain = app.session.as_mut().unwrap();
    let old_height = terrain.heights()[0];
    terrain.apply_height_changes(&[crate::terrain::SampleChange {
        index: 0,
        before: old_height,
        after: old_height + 1,
    }]);
    assert!(terrain.commit_stroke());
    app.packaging.confirm_overwrite_and_export();
    app.process_packaging_export_request();

    assert!(!app.packaging.pending_overwrite_confirm);
    assert!(app.packaging.status_message.contains("Safe Save"));
    assert_eq!(std::fs::read(&zip).unwrap(), previous_zip);
    assert!(app.has_unsaved_map_content());
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn packaging_overwrite_confirmation_rejects_changed_source_map() {
    let root = workflow_fixture("packaging_overwrite_source_guard");
    let other_root = workflow_fixture("packaging_overwrite_source_other");
    let mut app = wizard_map(&root, 5, false);
    let original_map = app.loaded_map_path.clone().unwrap();
    let other_app = wizard_map(&other_root, 6, false);
    let other_map = other_app.loaded_map_path.clone().unwrap();
    let zip = root.join("existing-export.zip");
    let previous_zip = b"previous verified package";
    std::fs::write(&zip, previous_zip).unwrap();
    app.packaging.map_dir_input = original_map.to_string_lossy().into_owned();
    app.packaging.output_zip_input = zip.to_string_lossy().into_owned();

    app.packaging.request_export();
    app.process_packaging_export_request();
    assert!(app.packaging.pending_overwrite_confirm);

    // Changing the source field while the prompt is open must not reuse approval for map A.
    app.packaging.map_dir_input = other_map.to_string_lossy().into_owned();
    app.packaging.confirm_overwrite_and_export();
    app.process_packaging_export_request();

    assert!(app.packaging.pending_overwrite_confirm);
    assert!(app.packaging.status_is_error);
    assert_eq!(std::fs::read(&zip).unwrap(), previous_zip);

    drop(other_app);
    drop(app);
    std::fs::remove_dir_all(other_root).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn existing_prebuilt_map_rejects_empty_missing_or_corrupt_data() {
    let root = workflow_fixture("binding_guard");
    let app = wizard_map(&root, 5, true);
    let map = app.loaded_map_path.clone().unwrap();
    let saved = app.scenario.document().clone();
    let mut unbound = saved.clone();
    unbound.data_slots.clear();
    let source = root.join("MAPS/ENDL_001");
    // 保留既存預放劇本，僅將物件池替換成已驗證的空白資料。
    let staging = root.join("MAPS/empty.tmp_arm");
    std::fs::create_dir_all(&staging).unwrap();
    fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap() {
            let e = e.unwrap();
            if e.path().is_dir() { copy_dir(&e.path(), &to.join(e.file_name())); }
            else { std::fs::copy(e.path(), to.join(e.file_name())).unwrap(); }
        }
    }
    copy_dir(&source, &staging);
    against_rome_maps::BlankMapContent::reset_staging_directory(&staging).unwrap();
    copy_dir(&staging.join("DATA"), &map.join("DATA"));
    for doc in [&saved, &unbound] {
        std::fs::write(map.join("arm_scenario.json"), serde_json::to_vec(doc).unwrap()).unwrap();
        assert!(crate::scenario_ui::ScenarioUi::default().load_session(&map).is_err());
    }
    let objects = map.join("DATA/objects.dat");
    std::fs::remove_file(&objects).unwrap();
    assert!(crate::scenario_ui::ScenarioUi::default().load_session(&map).is_err());
    std::fs::write(&objects, b"corrupt").unwrap();
    assert!(crate::scenario_ui::ScenarioUi::default().load_session(&map).is_err());
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn terrain_heavy_workflow_undo_redo_safe_save_reload_byte_identical() {
    use crate::terrain::{CollisionBrushOp, HeightBrushOp};

    let root = workflow_fixture("terrain_heavy_e2e");
    eprintln!("Task41 既有地形流程：fixture 完成");
    let mut app = wizard_map_custom(&root, 5, false, true);
    eprintln!("Task41 既有地形流程：精靈完成");
    let map = app.loaded_map_path.clone().unwrap();

    app.textures.load_session(&map).unwrap();
    app.placement.load_session(&map).unwrap();
    app.ensure_nature_data().unwrap();
    app.collect_pilot_receipts();
    app.undo_journal.clear();

    let clean_heights = app.session.as_ref().unwrap().heights().to_vec();
    let clean_collision = app.session.as_ref().unwrap().collision().to_vec();
    let clean_materials = app.textures.session_textures().unwrap().to_vec();
    let clean_nature = app.nature.session.capture();

    // 1. Raise
    let terrain = app.session.as_mut().unwrap();
    terrain.paint_height(20.0, 20.0, 5.0, HeightBrushOp::Raise, 15, None, 0).unwrap();
    assert!(terrain.commit_stroke());

    // 2. Lower
    let terrain = app.session.as_mut().unwrap();
    terrain.paint_height(45.0, 45.0, 5.0, HeightBrushOp::Lower, 10, None, 0).unwrap();
    assert!(terrain.commit_stroke());

    // 3. Flatten
    let terrain = app.session.as_mut().unwrap();
    terrain.paint_height(30.0, 30.0, 4.0, HeightBrushOp::Flatten, 12, Some(40), 0).unwrap();
    assert!(terrain.commit_stroke());

    // 4. Smooth
    let terrain = app.session.as_mut().unwrap();
    terrain.paint_height(20.0, 20.0, 6.0, HeightBrushOp::Smooth, 10, None, 0).unwrap();
    assert!(terrain.commit_stroke());

    // 5. Water
    let terrain = app.session.as_mut().unwrap();
    terrain.paint_water(55.0, 55.0, 4.0, 10, 2, 8.0).unwrap();
    assert!(terrain.commit_stroke());

    // 6. Passability brush (Collision Block & Clear)
    let terrain = app.session.as_mut().unwrap();
    terrain.paint_collision(15.0, 15.0, 3.0, CollisionBrushOp::Block).unwrap();
    assert!(terrain.commit_stroke());

    // 7. Material paint
    let textures = app.textures.session_mut().unwrap();
    assert!(textures.stamp_texture(6, 6, "4BB___51").is_some());
    assert!(textures.commit_stroke());

    // 8. Nature plant/remove
    let nature_item = app.nature_palette().first().cloned().expect("nature palette has landscape items");
    app.nature.session.plant(against_rome_environment::nature::NatureAddition::new(
        nature_item.type_id, &nature_item.name, 2560.0, 0.0, 2560.0, 0.0,
    ));
    assert!(app.nature.session.commit_stroke());
    let to_remove = against_rome_environment::nature::NatureAddition::new(
        nature_item.type_id, &nature_item.name, 2816.0, 0.0, 2816.0, 0.0,
    );
    app.nature.session.plant(to_remove.clone());
    assert!(app.nature.session.commit_stroke());
    assert!(app.nature.session.remove(&[], &[to_remove]));
    assert!(app.nature.session.commit_stroke());

    // 9. Cliff / road region tool
    let heights_snapshot = app.session.as_ref().unwrap().heights().to_vec();
    let _ = app.landscape.run_cliff_detection(257, Some(&heights_snapshot));
    app.landscape.clear_dirty();

    app.landscape.region.op = crate::landscape_ui::RegionOperation::SelectObjects;
    app.landscape.region.vertices_text = "0,0\n63,63".into();
    let known_names = app.textures.known_textures().to_vec();
    let cx = crate::landscape_ui::LandscapeContext {
        vertex_size: 257,
        heights: Some(app.session.as_ref().unwrap().heights()),
        textures: app.textures.session_textures(),
        texture_names: &known_names,
    };
    app.landscape.plan_region(&cx).unwrap();
    app.landscape.request_region_apply().unwrap();
    app.apply_region_request();
    assert!(!app.landscape.is_dirty());

    // 10. Unified undo all the way back to clean
    eprintln!("Task41 既有地形流程：開始 Undo");
    app.collect_pilot_receipts();
    assert!(app.undo_journal.can_undo());
    let mut undo_steps = 0;
    while app.undo_journal.can_undo() {
        undo_steps += 1;
        assert!(undo_steps <= 64, "統一復原未前進：{}", app.status_message);
        app.navigate_journal(false);
    }
    assert_eq!(app.session.as_ref().unwrap().heights(), clean_heights);
    assert_eq!(app.session.as_ref().unwrap().collision(), clean_collision);
    assert_eq!(app.textures.session_textures().unwrap(), clean_materials);
    assert_eq!(app.nature.session.capture(), clean_nature);
    assert!(!app.undo_journal.can_undo());

    // 11. Redo all
    assert!(app.undo_journal.can_redo());
    let mut redo_steps = 0;
    while app.undo_journal.can_redo() {
        redo_steps += 1;
        assert!(redo_steps <= 64, "統一重做未前進：{}", app.status_message);
        app.navigate_journal(true);
    }
    assert!(!app.undo_journal.can_redo());

    let pre_save_heights = app.session.as_ref().unwrap().heights().to_vec();
    let pre_save_collision = app.session.as_ref().unwrap().collision().to_vec();
    let pre_save_materials = app.textures.session_textures().unwrap().to_vec();
    let pre_save_nature_additions = app.nature.session.additions().to_vec();

    // 12. Safe Save
    spawn_save_tests::save_ok(&mut app);
    assert!(!app.has_unsaved_changes());

    // 13. Reload
    let mut loaded = reopen(&root, &map);
    loaded.textures.load_session(&map).unwrap();
    loaded.placement.load_session(&map).unwrap();

    // 14. Verify byte-identical state
    assert_eq!(loaded.session.as_ref().unwrap().heights(), pre_save_heights);
    assert_eq!(loaded.session.as_ref().unwrap().collision(), pre_save_collision);
    assert_eq!(loaded.textures.session_textures().unwrap(), pre_save_materials);
    let reloaded_objects = spawn_save_tests::data_objects(&map);
    for add in &pre_save_nature_additions {
        assert!(reloaded_objects.iter().any(|o| (o.position[0] - add.x).abs() < 0.1 && (o.position[2] - add.z).abs() < 0.1));
    }

    drop((app, loaded));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn placement_heavy_workflow_move_clone_delete_family_spawn_mirror_batch_edit() {
    let root = workflow_fixture("placement_heavy_e2e");
    let mut app = wizard_map(&root, 5, true);
    let map = app.loaded_map_path.clone().unwrap();

    app.placement.load_session(&map).unwrap();
    let objects_before = app.placement.objects();
    let house = objects_before.iter().find(|o| o.parent.is_none() && o.slot < 500).unwrap();
    let key = (house.slot, house.uid);
    let orig_x = house.position[0];
    let orig_z = house.position[2];
    app.placement.select_object(Some(key));

    // 1. DATA building move
    app.placement.set_delta(["256".into(), "0".into(), "256".into()]);
    app.placement.move_family().unwrap();
    let moved = app.placement.objects().into_iter().find(|o| o.slot == house.slot).unwrap();
    assert_eq!(moved.position[0], orig_x + 256.0);
    assert_eq!(moved.position[2], orig_z + 256.0);

    // 2. Family copy (clone family)
    app.placement.set_delta(["512".into(), "0".into(), "512".into()]);
    app.placement.clone_family().unwrap();
    let cloned_keys = app.placement.selected_objects();
    assert!(!cloned_keys.is_empty());

    // 3. DATA building delete
    app.placement.delete_selection().unwrap();
    assert!(app.placement.selected_objects().is_empty());

    // 4. Script spawn mirror copy
    app.spawn_ui.sync_with_document(&app.scenario, Some(&map));
    app.spawn_ui.select_type("FigGerInf02_Zweihandaxt");
    app.spawn_ui.set_placement_values(0, 1, 0);
    let cx = spawn_save_tests::cx();
    app.spawn_ui.place_at_tile(&mut app.scenario, &cx, 12, 12);
    assert!(!app.spawn_ui.status().1, "{}", app.spawn_ui.status().0);
    let count_before = app.scenario.document().spawns.len();
    app.spawn_ui.duplicate_opposite_selected(&mut app.scenario, &cx);
    assert!(!app.spawn_ui.status().1, "{}", app.spawn_ui.status().0);
    assert_eq!(app.scenario.document().spawns.len(), count_before + 1);
    let opposite_spawn = app.scenario.document().spawns.last().unwrap();
    assert_eq!(opposite_spawn.alias, "TROOP");

    // 5. Select-same-type batch edit
    let target_type = house.type_id;
    app.placement.select_object(Some((house.slot, house.uid)));
    let count = app.placement.select_all_of_type();
    assert!(count >= 1);
    app.placement.batch_edit(Some(3), Some(90.0)).unwrap();
    for obj in app.placement.objects().iter().filter(|o| o.type_id == target_type) {
        assert_eq!(obj.team, 3);
        assert!((obj.rotation_degrees - 90.0).abs() < 0.1);
    }

    // 6. Safe Save
    spawn_save_tests::save_ok(&mut app);

    // 7. Reload and verify DATA pools and scenario spawns/bindings
    let mut loaded = reopen(&root, &map);
    loaded.placement.load_session(&map).unwrap();
    let doc = loaded.scenario.document();
    let data_objs = spawn_save_tests::data_objects(&map);
    assert!(!data_objs.is_empty());
    let active: Vec<_> = data_objs.iter().map(|o| (o.slot as i32, o.uid)).collect();
    against_rome_scenario::verify_binding_snapshot(doc, &active).unwrap();
    assert_eq!(doc.spawns.len(), app.scenario.document().spawns.len());
    let loaded_house = data_objs.iter().find(|o| o.slot == house.slot).unwrap();
    assert_eq!(loaded_house.team, 3);
    assert!((loaded_house.rotation_degrees - 90.0).abs() < 0.1);

    drop((app, loaded));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn scenario_heavy_workflow_event_graph_templates_bulk_waves_objectives_bci() {
    let root = workflow_fixture("scenario_heavy_e2e");
    let mut app = wizard_map(&root, 5, false);
    let map = app.loaded_map_path.clone().unwrap();

    // 1. Event graph edits (add nodes, tidy, delete, undo)
    app.scenario.open_graph();
    let trigger = crate::event_graph::model::GraphNode::new_event_trigger("Graph Trigger", 0, false, true);
    let tr_id = trigger.id;
    let msg_node = crate::event_graph::model::GraphNode::new_message_action("Tidy Message");
    let msg_id = msg_node.id;
    let extra_node = crate::event_graph::model::GraphNode::new_message_action("To Delete");
    let extra_id = extra_node.id;
    app.scenario.graph_ui_mut().graph.add_node(trigger);
    app.scenario.graph_ui_mut().graph.add_node(msg_node);
    app.scenario.graph_ui_mut().graph.add_node(extra_node);
    app.scenario.graph_ui_mut().mark_dirty();
    let tr_out = app.scenario.graph_ui().graph.find_node(tr_id).unwrap().find_port_by_name("ExecOut").unwrap().id;
    let msg_in = app.scenario.graph_ui().graph.find_node(msg_id).unwrap().find_port_by_name("ExecIn").unwrap().id;
    app.scenario.graph_ui_mut().graph.connect(tr_out, msg_in).unwrap();

    // Tidy (auto_layout)
    app.scenario.graph_ui_mut().auto_layout();
    assert!(app.scenario.graph_ui().graph.find_node(tr_id).is_some());

    // Delete node and undo
    app.scenario.graph_ui_mut().selected_nodes.insert(extra_id);
    app.scenario.graph_ui_mut().delete_selected();
    assert!(app.scenario.graph_ui().graph.find_node(extra_id).is_none());
    assert!(app.scenario.graph_ui_mut().can_undo_draft());
    app.scenario.graph_ui_mut().undo_draft();
    assert!(app.scenario.graph_ui().graph.find_node(extra_id).is_some());

    // Apply graph to scenario
    let mut graph_ui = std::mem::take(app.scenario.graph_ui_mut());
    app.scenario.apply_from_graph_ui(&mut graph_ui).unwrap();
    *app.scenario.graph_ui_mut() = graph_ui;

    // 2. Event templates import
    let template_file = root.join("flow3_template.arm-events.json");
    let template_ev = against_rome_scenario::Event {
        name: "Imported Flow3 Event".into(),
        actions: vec![against_rome_scenario::Action {
            kind: 1, team: 1, other_team: 2, hostile: true, ..Default::default()
        }],
        ..Default::default()
    };
    let (template, _) = crate::event_templates::EventTemplate::capture(&[template_ev]);
    crate::layout_io::write_json_atomic(&template_file, &template.serialize().unwrap()).unwrap();
    let imported_count = app.scenario.import_events_from(&template_file, None).unwrap();
    assert_eq!(imported_count, 1);

    // 3. Bulk enable/disable
    let total = app.scenario.document().events.len();
    assert!(total >= 2);
    let all_indices: Vec<_> = (0..total).collect();
    assert_eq!(app.scenario.set_events_enabled(&all_indices, false).unwrap(), total);
    assert!(app.scenario.document().events.iter().all(|e| !e.enabled));
    assert_eq!(app.scenario.set_events_enabled(&all_indices, true).unwrap(), total);
    assert!(app.scenario.document().events.iter().all(|e| e.enabled));

    // 4. Wave generator
    app.scenario.wave_generator.waves = 2;
    app.scenario.wave_generator.first = 300;
    app.scenario.wave_generator.interval = 60;
    app.scenario.wave_generator.alias = "GER_INF02".into();
    app.scenario.generate_attack_waves().unwrap();

    // 5. Objective presets
    app.scenario.add_objective(Some(1200), None, 0).unwrap();

    // 6. Condition rectangle
    let spawn_target = uuid::Uuid::new_v4();
    app.scenario.apply_spawns(vec![against_rome_scenario::Spawn {
        alias: "GER_INF02".into(),
        x: 1000.0,
        z: 1000.0,
        team: 1,
        count: 1,
        angle: 0,
        y: 20.0,
        prebuilt: false,
        id: spawn_target,
        raw_rotation: None,
    }]).unwrap();
    app.scenario.add_events(&[against_rome_scenario::Event {
        name: "Condition Rectangle Event".into(),
        conditions: vec![against_rome_scenario::Condition {
            kind: 2,
            target_id: spawn_target,
            min_x: 500,
            min_z: 500,
            max_x: 2500,
            max_z: 2500,
        }],
        actions: vec![against_rome_scenario::Action {
            text: "Entered rectangle".into(),
            ..Default::default()
        }],
        ..Default::default()
    }]).unwrap();

    // 7. Safe Save and compile BCI
    spawn_save_tests::save_ok(&mut app);
    let expected_events = app.scenario.document().events.clone();
    let disk_bci = std::fs::read(map.join("SCRIPT/ak_level.bci")).unwrap();
    assert!(!disk_bci.is_empty());
    against_rome_scenario::BciImage::parse(&disk_bci).unwrap();

    // 8. Reload and verify events identical & BCI compiles again identically
    let mut loaded = reopen(&root, &map);
    assert_eq!(loaded.scenario.document().events, expected_events);
    let original_bci = std::fs::read(map.join("SCRIPT/ak_level.arm_original")).unwrap();
    let recompiled = against_rome_scenario::compile_script_verified(
        &original_bci,
        loaded.scenario.document(),
        Some(loaded.scenario.start_camera()),
        Some(loaded.scenario.known_aliases()),
        None,
    ).unwrap();
    let recompiled_raw = if recompiled.starts_with(b"PFIL") {
        against_rome_data::lzss::decompress_pfil(&recompiled, None).unwrap()
    } else {
        recompiled
    };
    assert_eq!(recompiled_raw, disk_bci);

    assert_eq!(loaded.scenario.set_events_enabled(&[0], false).unwrap(), 1);
    let staged = loaded.scenario.stage_transaction_files(&map).unwrap();
    let (_, bci_bytes) = staged.iter().find(|(p, _)| p.ends_with("ak_level.bci")).unwrap();
    against_rome_scenario::BciImage::parse(bci_bytes).unwrap();

    drop((app, loaded));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn readonly_original_map_attempts_refused_clean_safe_save_rejects() {
    let root = workflow_fixture("readonly_heavy_e2e");
    std::fs::write(root.join("Against_Rome.exe"), b"GAME_EXE").unwrap();
    let source = root.join("MAPS/ENDL_001");
    assert!(crate::app::is_original_map_dir(&source));
    let before_bytes = map_bytes(&source);

    let mut app = EditorApp::initial_state();
    app.recovery.root = Some(root.join("editor-data"));
    app.set_loaded_map(source.clone(), crate::storage::load_map(&source).unwrap()).unwrap();
    app.scenario.load_session(&source).unwrap();
    app.textures.load_session(&source).unwrap();
    app.placement.load_session(&source).unwrap();

    assert!(app.current_map_is_original());
    assert!(!app.has_unsaved_changes());

    // 1. Mutating Shortcuts are refused
    app.handle_shortcut_action(ShortcutAction::Save);
    assert!(app.status_is_error);
    assert!(!app.has_unsaved_changes());

    app.handle_shortcut_action(ShortcutAction::Delete);
    assert!(!app.has_unsaved_changes());

    app.handle_shortcut_action(ShortcutAction::Duplicate);
    assert!(!app.has_unsaved_changes());

    app.handle_shortcut_action(ShortcutAction::Nudge { dx: 1.0, dz: 1.0 });
    assert!(!app.has_unsaved_changes());

    assert!(!app.eyedrop_material_at_cursor());
    assert!(!app.has_unsaved_changes());

    assert!(!app.sample_flatten_at_cursor());
    assert!(!app.has_unsaved_changes());

    app.handle_shortcut_action(ShortcutAction::RegenerateSettlement);
    assert!(app.status_is_error);
    assert!(!app.has_unsaved_changes());

    // 2. Viewport 3D actions refused
    app.handle_viewport_3d_action(crate::viewport_3d::ViewportAction::Paint { grid_x: 20.0, grid_y: 20.0 });
    assert!(!app.terrain_is_dirty());

    app.active_tool = ActiveTool::Material;
    app.handle_viewport_3d_action(crate::viewport_3d::ViewportAction::Paint { grid_x: 20.0, grid_y: 20.0 });
    assert!(!app.textures.is_dirty());

    app.nature.enabled = true;
    app.handle_viewport_3d_action(crate::viewport_3d::ViewportAction::Paint { grid_x: 20.0, grid_y: 20.0 });
    assert!(!app.nature_is_dirty());

    // 3. Ramp point refused
    app.handle_ramp_point((10.0, 10.0));
    assert!(!app.terrain_is_dirty());

    // 4. Spawn placement refused
    let cx = crate::spawn_placement_ui::SpawnContext {
        lang: app.lang(),
        heights: Some(app.session.as_ref().unwrap().heights()),
        vertex_size: app.session.as_ref().unwrap().vertex_size,
        height_step: app.height_step,
        read_only: true,
    };
    app.spawn_ui.place_at_tile(&mut app.scenario, &cx, 10, 10);
    assert!(!app.scenario.dirty());

    // 6. Bulk / Macro operations refused
    assert!(!app.reset_terrain_now());
    assert!(!app.terrain_is_dirty());
    assert!(!app.apply_flat_terrain_now());
    assert!(!app.terrain_is_dirty());
    assert!(app.apply_settlement().is_err());
    assert!(!app.environment.is_dirty());

    // 7. Landscape tool operations refused
    app.handle_landscape_apply(crate::landscape_ui::LandscapeApply::Cliff);
    assert!(!app.terrain_is_dirty() && !app.textures.is_dirty());

    app.handle_landscape_apply(crate::landscape_ui::LandscapeApply::River);
    assert!(!app.terrain_is_dirty() && !app.textures.is_dirty());

    app.handle_landscape_apply(crate::landscape_ui::LandscapeApply::Road);
    assert!(!app.textures.is_dirty());

    app.handle_landscape_apply(crate::landscape_ui::LandscapeApply::Wall);
    assert!(!app.terrain_is_dirty());

    app.apply_region_request();
    assert!(!app.textures.is_dirty());

    // Assert completely clean
    assert!(!app.has_unsaved_changes());

    // 8. Safe Save refuses
    app.safe_save_current();
    assert!(app.status_is_error);
    assert!(app.status_message.contains("Safe Save Rejected") || app.status_message.contains("安全拒絕"));
    assert_eq!(map_bytes(&source), before_bytes);

    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
