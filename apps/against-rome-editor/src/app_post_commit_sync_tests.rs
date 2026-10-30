//! 提交成功後的實際面板同步、內容防護與中英 UI 回歸。
use super::*;
use crate::terrain::SampleChange;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> (EditorApp, Temp, PathBuf) {
    let root = Temp(std::env::temp_dir().join(format!("arm-panel-sync-{}", uuid::Uuid::new_v4())));
    let map = root.0.join("MAPS/ENDL_005");
    std::fs::create_dir_all(map.join("SCRIPT")).unwrap();
    crate::storage::safe_save_map(&map, &mut TerrainSession::create_blank(5, 32)).unwrap();
    std::fs::write(
        map.join("SCRIPT/ak_level.bci"),
        against_rome_scenario::BciImage::idle().serialize(),
    )
    .unwrap();
    std::fs::write(
        map.join("boden.txt"),
        b"[Dimension]\r\n2\r\n[Texturen]\r\n4BA___51\r\n4BA___51\r\n4BA___51\r\n4BA___51\r\n",
    )
    .unwrap();
    let mut app = EditorApp::initial_state();
    app.recovery.root = Some(root.0.join("recovery"));
    app.set_loaded_map(map.clone(), crate::storage::load_map(&map).unwrap())
        .unwrap();
    app.scenario.load_session(&map).unwrap();
    add_event(&mut app, "TestA");
    (app, root, map)
}
fn add_event(app: &mut EditorApp, name: &str) {
    app.scenario
        .add_events(&[against_rome_scenario::Event {
            name: name.into(),
            actions: vec![against_rome_scenario::Action {
                text: name.into(),
                ..Default::default()
            }],
            ..Default::default()
        }])
        .unwrap();
}
fn save(app: &mut EditorApp) {
    app.safe_save_confirming_using(|_| true, |tx| tx.commit().map_err(|e| e.to_string()));
    assert!(app.post_commit_warning.is_some(), "{}", app.status_message);
    assert!(app.post_commit_sync.is_some());
}
fn retry_ok(app: &mut EditorApp) {
    app.retry_panel_sync();
    assert!(!app.status_is_error, "{}", app.status_message);
    assert!(app.post_commit_warning.is_none() && app.post_commit_sync.is_none());
}
#[test]
fn temporary_failure_retries_without_writes_and_clears_old_recovery_epoch() {
    let (mut app, _temp, map) = fixture();
    app.collect_pilot_receipts();
    let old_journal = app.undo_journal.clone();
    let pending = app.recovery_pending();
    app.recovery.save(&map, true, pending).unwrap();
    app.recovery.flush();
    let snapshot = crate::recovery::snapshot_path(app.recovery.root.as_ref().unwrap(), &map);
    assert!(snapshot.exists());
    app.panel_sync_faults.insert("scenario baseline");
    save(&mut app);
    assert!(app.scenario.dirty());
    assert_eq!(app.undo_journal, old_journal);
    let disk = read_tree(&map).unwrap();
    app.retry_panel_sync();
    assert!(app.status_is_error && app.post_commit_warning.is_some());
    assert_eq!(app.undo_journal, old_journal);
    app.panel_sync_faults.clear();
    retry_ok(&mut app);
    assert!(!app.scenario.dirty() && !app.has_unsaved_changes());
    assert!(!app.undo_journal.can_undo() && !app.undo_journal.can_redo());
    app.recovery.flush();
    assert!(!snapshot.exists());
    assert_eq!(
        read_tree(&map).unwrap(),
        disk,
        "重試不可重新寫入交易或產生備份"
    );
}
#[test]
fn same_length_corruption_after_commit_is_not_accepted_as_transaction_truth() {
    let (mut app, _temp, map) = fixture();
    let mut committed = Vec::new();
    app.safe_save_confirming_using(
        |_| true,
        |tx| {
            if let StagedAction::WriteBytes(bytes) =
                tx.find_entry("arm_scenario.json").unwrap().action()
            {
                committed = bytes.clone();
            }
            let report = tx.commit().map_err(|e| e.to_string())?;
            let changed = String::from_utf8(committed.clone())
                .unwrap()
                .replace("TestA", "TestB");
            assert_eq!(changed.len(), committed.len());
            std::fs::write(map.join("arm_scenario.json"), changed).unwrap();
            Ok(report)
        },
    );
    assert!(app.post_commit_warning.is_some());
    app.retry_panel_sync();
    assert!(
        app.status_message.contains("contents changed"),
        "{}",
        app.status_message
    );
    assert_eq!(app.scenario.document().events[0].name, "TestA");
    std::fs::write(map.join("arm_scenario.json"), committed).unwrap();
    retry_ok(&mut app);
    assert_eq!(app.scenario.document().events[0].name, "TestA");
}
#[test]
fn unchanged_baseline_dependency_and_external_alias_are_both_verified() {
    let (mut app, temp, map) = fixture();
    let dependency = map.join("DATA/gfxtype.dat");
    std::fs::create_dir_all(dependency.parent().unwrap()).unwrap();
    std::fs::write(&dependency, b"baseline").unwrap();
    let aliases = temp.0.join("SYSTEM/CLAK/cl_scint.ini");
    std::fs::create_dir_all(aliases.parent().unwrap()).unwrap();
    std::fs::write(&aliases, b"[ObjDefName]\n").unwrap();
    app.panel_sync_faults.insert("scenario baseline");
    save(&mut app);
    app.panel_sync_faults.clear();
    std::fs::write(&dependency, b"changed!").unwrap();
    app.retry_panel_sync();
    assert!(app.status_message.contains("contents changed"));
    std::fs::write(&dependency, b"baseline").unwrap();
    std::fs::write(&aliases, b"[ObjDefName] \n").unwrap();
    app.retry_panel_sync();
    assert!(app.status_message.contains("aliases changed"));
    std::fs::write(&aliases, b"[ObjDefName]\n").unwrap();
    retry_ok(&mut app);
}
#[test]
fn partial_success_only_retries_remaining_panel_and_preserves_other_new_edits() {
    let (mut app, _temp, map) = fixture();
    app.textures.load_session(&map).unwrap();
    app.textures
        .restore_recovery(&vec!["4BB___51".into(); 4])
        .unwrap();
    app.panel_sync_faults
        .extend(["scenario baseline", "texture baseline"]);
    save(&mut app);
    assert_eq!(app.post_commit_sync.as_ref().unwrap().failed.len(), 2);
    app.panel_sync_faults.remove("scenario baseline");
    app.retry_panel_sync();
    assert!(app.post_commit_warning.is_some());
    assert!(!app.scenario.dirty() && app.textures.is_dirty());
    assert_eq!(app.post_commit_sync.as_ref().unwrap().failed.len(), 1);
    add_event(&mut app, "New draft after partial sync");
    app.session
        .as_mut()
        .unwrap()
        .apply_height_changes(&[SampleChange {
            index: 0,
            before: 32,
            after: 71,
        }]);
    app.session.as_mut().unwrap().commit_stroke();
    let scenario_draft = app.scenario.recovery_state();
    app.panel_sync_faults.remove("texture baseline");
    retry_ok(&mut app);
    assert_eq!(
        app.scenario.recovery_state(),
        scenario_draft,
        "成功面板不可再次重載"
    );
    assert_eq!(app.session.as_ref().unwrap().heights()[0], 71);
    assert!(app.has_unsaved_changes() && !app.textures.is_dirty());
    assert_eq!(crate::storage::load_map(&map).unwrap().heights()[0], 32);
    app.recovery.flush();
    let snapshot = crate::recovery::snapshot_path(app.recovery.root.as_ref().unwrap(), &map);
    let value: Value = serde_json::from_slice(&std::fs::read(snapshot).unwrap()).unwrap();
    assert_eq!(value["pending"]["terrain"]["heights"][0], 71);
    assert_eq!(value["pending"]["scenario"], scenario_draft);
}
#[test]
fn new_failed_panel_draft_and_path_switch_refuse_retry_without_losing_edits() {
    let (mut app, _temp, map) = fixture();
    app.panel_sync_faults.insert("scenario baseline");
    save(&mut app);
    app.panel_sync_faults.clear();
    let original = app.scenario.sync_state();
    app.map_path_input = map.join("other").display().to_string();
    app.retry_panel_sync();
    assert!(app.status_message.contains("Map path changed"));
    assert_eq!(app.scenario.sync_state(), original);
    app.map_path_input = map.display().to_string();
    add_event(&mut app, "New draft");
    let draft = app.scenario.sync_state();
    for _ in 0..2 {
        app.retry_panel_sync();
        assert!(app.status_message.contains("Panel changed since commit"));
        assert_eq!(app.scenario.sync_state(), draft);
        assert!(app.scenario.dirty());
    }
}
#[test]
fn fingerprint_capture_failure_aborts_before_commit_and_does_not_bless_disk() {
    let (mut app, _temp, map) = fixture();
    // 一般檔案擋住來源依賴的目錄，read 必須回報錯誤，不能視同缺檔。
    std::fs::write(
        map.parent().unwrap().parent().unwrap().join("SYSTEM"),
        b"not a directory",
    )
    .unwrap();
    let disk = read_tree(&map).unwrap();
    app.safe_save_confirming_using(|_| true, |_| panic!("指紋擷取失敗不應進入 commit"));
    assert!(app.status_is_error);
    assert_eq!(read_tree(&map).unwrap(), disk);
    assert!(app.scenario.dirty() && app.post_commit_sync.is_none());
}
#[test]
fn retry_preserves_spawn_final_document_and_combined_camera_document() {
    use super::super::tests::spawn_save_tests as spawn;
    for camera in [false, true] {
        let (mut app, root, map) = spawn::fixture("panel_sync_final_document", &[]);
        let _temp = Temp(root);
        spawn::place_house(&mut app, (30, 30), 0);
        if camera {
            let mut view = app.campaign.start_camera.clone();
            view.enabled = true;
            app.campaign.capture_camera(view);
            let files = app.scenario.stage_transaction_files(&map).unwrap();
            app.campaign
                .apply_camera_candidate(&map, &files, app.scenario.known_aliases())
                .unwrap();
        }
        app.panel_sync_faults.insert("scenario baseline");
        save(&mut app);
        let saved =
            ScenarioDocument::from_json(&std::fs::read(map.join("arm_scenario.json")).unwrap())
                .unwrap();
        assert!(saved.spawns[0].prebuilt && !saved.data_slots.is_empty());
        app.panel_sync_faults.clear();
        retry_ok(&mut app);
        assert_eq!(app.scenario.document(), &saved);
        assert!(!app.scenario.dirty() && !app.campaign.camera_pending());
    }
}
fn render(app: &mut EditorApp, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 900.0))),
            events,
            ..Default::default()
        },
        |ctx| app.ui_frame(ctx),
    )
}
#[test]
fn chinese_and_english_retry_buttons_render_and_click_real_sync() {
    for (language, label) in [
        (AppLanguage::English, "Retry panel sync"),
        (AppLanguage::TraditionalChinese, "重試面板同步"),
    ] {
        let (mut app, _temp, _map) = fixture();
        app.loc.set_language(language);
        app.panel_sync_faults.insert("scenario baseline");
        save(&mut app);
        app.panel_sync_faults.clear();
        let ctx = egui::Context::default();
        let _ = render(&mut app, &ctx, vec![]);
        let output = render(&mut app, &ctx, vec![]);
        let pos = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("中英重試按鈕應顯示在主畫面");
        let _ = render(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        let _ = render(
            &mut app,
            &ctx,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        assert!(app.post_commit_warning.is_none(), "{}", app.status_message);
        assert!(!app.scenario.dirty());
        let output = render(&mut app, &ctx, vec![]);
        assert!(!output
            .shapes
            .iter()
            .any(|s| matches!(&s.shape, egui::epaint::Shape::Text(t) if t.galley.text() == label)));
    }
}

#[test]
fn unreadable_retry_source_keeps_original_expected_bytes_for_later_retry() {
    let (mut app, _temp, map) = fixture();
    app.panel_sync_faults.insert("scenario baseline");
    save(&mut app);
    app.panel_sync_faults.clear();
    let moved = map.with_file_name("held-map");
    std::fs::rename(&map, &moved).unwrap();
    std::fs::write(&map, b"not a directory").unwrap();
    app.retry_panel_sync();
    assert!(app.post_commit_warning.is_some() && app.scenario.dirty());
    std::fs::remove_file(&map).unwrap();
    std::fs::rename(&moved, &map).unwrap();
    retry_ok(&mut app);
}
#[test]
fn new_data_edits_are_detected_even_without_a_new_history_token() {
    use super::super::tests::spawn_save_tests as spawn;
    let (mut app, root, map) = spawn::fixture("retry_data_guard", &[(232, None)]);
    let _temp = Temp(root);
    app.placement.load_session(&map).unwrap();
    app.placement
        .store_mut()
        .unwrap()
        .set_team(0, 1, 3)
        .unwrap();
    app.panel_sync_faults.insert("placement baseline");
    save(&mut app);
    let token = app.placement.history_token();
    app.placement
        .store_mut()
        .unwrap()
        .set_team(0, 1, 4)
        .unwrap();
    assert_eq!(app.placement.history_token(), token);
    let draft = app.placement.sync_state();
    app.panel_sync_faults.clear();
    app.retry_panel_sync();
    assert!(app.post_commit_warning.is_some());
    assert!(app.status_message.contains("Panel changed since commit"));
    assert_eq!(app.placement.sync_state(), draft);
    let disk = against_rome_placement::ObjectStore::load(&map).unwrap();
    assert_eq!(disk.objects()[0].team, 3);
}
#[test]
fn new_material_and_scene_edits_are_preserved_by_failed_panel_guards() {
    for scene in [false, true] {
        let (mut app, _temp, map) = fixture();
        let fault = if scene {
            std::fs::write(
                map.join(against_rome_maps::CustomMapManifest::MARKER_FILE_NAME),
                b"",
            )
            .unwrap();
            std::fs::write(map.join("Endlos_01_Siedlung01.sdl"), "[settlement]\r\nrefpos=1000,50,2000\r\n[object0000]\r\ndef=1700\r\nnamedef=BauRomHau00_Haupthaus\r\npos=100,10,100\r\nteam=0\r\nangle=0\r\n").unwrap();
            app.scene.reset_session(Some(&map));
            app.scene.set_team(0, 1).unwrap();
            "scene baseline"
        } else {
            app.textures.load_session(&map).unwrap();
            app.textures
                .restore_recovery(&vec!["4BB___51".into(); 4])
                .unwrap();
            "texture baseline"
        };
        app.panel_sync_faults.insert(fault);
        save(&mut app);
        if scene {
            app.scene.set_team(0, 2).unwrap();
        } else {
            app.textures
                .restore_recovery(&vec!["4BC___51".into(); 4])
                .unwrap();
        }
        let scene_draft = app.scene.snapshot();
        let textures_draft = app.textures.sync_state();
        app.panel_sync_faults.clear();
        app.retry_panel_sync();
        assert!(app.post_commit_warning.is_some());
        assert!(app.status_message.contains("Panel changed since commit"));
        assert_eq!(app.scene.snapshot(), scene_draft);
        assert_eq!(app.textures.sync_state(), textures_draft);
    }
}
#[test]
fn unapplied_event_graph_draft_is_not_discarded_by_retry() {
    let (mut app, _temp, _map) = fixture();
    app.panel_sync_faults.insert("scenario baseline");
    save(&mut app);
    app.panel_sync_faults.clear();
    app.scenario.graph_ui.is_dirty = true;
    let draft = app.scenario.sync_state();
    app.retry_panel_sync();
    assert!(app.post_commit_warning.is_some());
    assert!(app.status_message.contains("Panel changed since commit"));
    assert_eq!(app.scenario.sync_state(), draft);
}
