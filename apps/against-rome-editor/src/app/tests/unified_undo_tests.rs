use super::*;
use super::spawn_save_tests::{fixture, place_house};
use crate::undo_journal::{SubsystemId, UndoTarget};

fn terrain(app: &mut EditorApp, after: u8) {
    let s = app.session.as_mut().unwrap();
    s.apply_height_changes(&[crate::terrain::SampleChange { index: 0, before: s.heights()[0], after }]);
    assert!(s.commit_stroke());
}
fn snapshot(app: &EditorApp) -> (Vec<u8>, Vec<u8>, against_rome_environment::nature::NatureSnapshot) {
    (app.session.as_ref().unwrap().heights().to_vec(),
     serde_json::to_vec(app.scenario.document()).unwrap(), app.nature.session.capture())
}
fn tokens(app: &EditorApp) -> Vec<u64> {
    [SubsystemId::Terrain, SubsystemId::Spawns, SubsystemId::Scenario, SubsystemId::Nature]
        .into_iter().map(|id| app.token(id)).collect()
}
fn setup(tag: &str) -> (EditorApp, PathBuf, egui::Context) {
    let (mut app, root, map) = fixture(tag, &[]);
    std::fs::write(map.join(".arm_custom_map"), r#"{"Slot":5,"SourceSlot":0,"CreatedAt":"2026-10-24T00:00:00+00:00","ToolVersion":"test"}"#).unwrap();
    app.set_app_language(AppLanguage::English);
    app.collect_pilot_receipts();
    app.undo_journal.clear();
    (app, root, egui::Context::default())
}

#[test]
fn lifecycle_load_blank_close_and_reset_clear_pending_journal_receipts() {
    let (mut app, root, ctx) = setup("step5_lifecycle");
    let map = app.loaded_map_path.clone().unwrap();
    terrain(&mut app, 40);
    app.install_loaded_map(map.clone(), TerrainSession::create_blank(5, 32)).unwrap();
    app.collect_pilot_receipts();
    assert!(!app.undo_journal.can_undo() && !app.undo_journal.can_redo());
    terrain(&mut app, 41);
    assert!(app.reset_terrain_now());
    app.collect_pilot_receipts();
    assert!(!app.undo_journal.can_undo() && !app.undo_journal.can_redo());
    terrain(&mut app, 42);
    app.install_blank_map(5);
    app.collect_pilot_receipts();
    assert!(!app.undo_journal.can_undo());
    terrain(&mut app, 43);
    app.perform_action(PendingAction::Close, &ctx);
    app.collect_pilot_receipts();
    assert!(!app.undo_journal.can_undo());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn explicit_discard_clears_journal_before_blank_map_switch() {
    let (mut app, root, ctx) = setup("step5_discard");
    terrain(&mut app, 44);
    app.collect_pilot_receipts();
    assert!(app.undo_journal.can_undo());
    app.pending_action = Some(PendingAction::Blank(5));
    let _ = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
    let output = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
    let pos = gui_find(&output, "Discard unsaved changes").unwrap().center();
    for pressed in [true, false] {
        let _ = ctx.run(egui::RawInput { events: gui_pointer(pos, pressed), ..Default::default() },
            |ctx| app.handle_pending_action(ctx));
    }
    assert!(app.pending_action.is_none());
    app.collect_pilot_receipts();
    assert!(!app.undo_journal.can_undo() && !app.undo_journal.can_redo());
    assert!(app.loaded_map_path.is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn blank_terrain_toolbar_restores_heights_materials_and_auxiliary_together() {
    let (mut app, root, _ctx) = setup("step5_flat");
    let map = app.loaded_map_path.clone().unwrap();
    std::fs::write(map.join("boden.txt"), "[Dimension]\n2\n[Texturen]\n4BA___51\n4BB___51\n4BA___51\n4BB___51\n").unwrap();
    app.textures.load_session(&map).unwrap();
    app.emboss = Some(EmbossBaseline {
        raw: crate::bmp::BmpImage::new_solid(5, 5, 100, 100, 100),
        session: against_rome_editor::emboss::EmbossSession::new(5, vec![32; 25], Some(vec![100; 25])).unwrap(),
    });
    let heights = app.session.as_ref().unwrap().heights().to_vec();
    let materials = app.textures.session_textures().unwrap().to_vec();
    assert!(!app.reset_auxiliary_layers);
    assert!(app.apply_flat_terrain_now());
    let flat_heights = app.session.as_ref().unwrap().heights().to_vec();
    let flat_materials = app.textures.session_textures().unwrap().to_vec();
    assert_ne!(flat_heights, heights);
    assert!(app.reset_auxiliary_layers);
    assert!(app.emboss.as_ref().unwrap().session.is_relight_pending());
    let journal = app.undo_journal.clone();
    assert!(app.apply_flat_terrain_now());
    assert_eq!(app.undo_journal, journal, "重複套用不應新增空操作");
    app.navigate_journal(false);
    assert_eq!(app.session.as_ref().unwrap().heights(), heights, "{}; {journal:?}", app.status_message);
    assert_eq!(app.textures.session_textures().unwrap(), materials);
    assert!(!app.reset_auxiliary_layers);
    assert!(!app.emboss.as_ref().unwrap().session.is_relight_pending());
    assert!(!app.undo_journal.can_undo(), "{}", app.status_message);
    app.navigate_journal(true);
    assert_eq!(app.session.as_ref().unwrap().heights(), flat_heights);
    assert_eq!(app.textures.session_textures().unwrap(), flat_materials);
    assert!(app.reset_auxiliary_layers);
    assert!(app.emboss.as_ref().unwrap().session.is_relight_pending());
    assert!(!app.undo_journal.can_redo());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unified_toolbar_interleaved_reverse_order_and_redo() {
    let (mut app, root, ctx) = setup("step4a_order");
    let mut states = vec![(tokens(&app), snapshot(&app))];
    terrain(&mut app, 40);
    states.push((tokens(&app), snapshot(&app)));
    place_house(&mut app, (30, 30), 0);
    states.push((tokens(&app), snapshot(&app)));
    app.scenario.add_events(&[against_rome_scenario::Event { name: "交錯事件".into(), actions: vec![against_rome_scenario::Action { text: "step4a".into(), ..Default::default() }], ..Default::default() }]).unwrap();
    states.push((tokens(&app), snapshot(&app)));
    app.nature.session.plant(against_rome_environment::nature::NatureAddition::new(1, "Tree", 10.0, 0.0, 10.0, 0.0));
    assert!(app.nature.session.commit_stroke());
    states.push((tokens(&app), snapshot(&app)));
    app.active_tool = ActiveTool::Material;
    for expected in states[..4].iter().rev() {
        gui_click(&mut app, &ctx, "Undo (Ctrl+Z)");
        assert_eq!(&(tokens(&app), snapshot(&app)), expected, "狀態：{} spawn={:?} loaded={} dirty={} readonly={}", app.status_message, app.spawn_ui.status(), app.scenario.is_loaded(), app.scenario.dirty(), app.current_map_is_original());
    }
    for expected in &states[1..] {
        gui_click(&mut app, &ctx, "Redo (Ctrl+Y)");
        assert_eq!(&(tokens(&app), snapshot(&app)), expected, "狀態：{} spawn={:?} loaded={} dirty={} readonly={}", app.status_message, app.spawn_ui.status(), app.scenario.is_loaded(), app.scenario.dirty(), app.current_map_is_original());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unified_toolbar_local_undo_desync_falls_back_safely() {
    let (mut app, root, ctx) = setup("step4a_desync");
    terrain(&mut app, 40);
    app.scenario.add_events(&[against_rome_scenario::Event { name: "區域復原".into(), actions: vec![against_rome_scenario::Action { text: "step4a".into(), ..Default::default() }], ..Default::default() }]).unwrap();
    app.collect_pilot_receipts();
    assert!(app.scenario.undo_events());
    let scenario = app.scenario.history_token();
    gui_click(&mut app, &ctx, "Undo (Ctrl+Z)");
    assert_eq!(app.scenario.history_token(), scenario);
    assert_ne!(app.session.as_ref().unwrap().heights()[0], 40);
    assert!(app.status_message.contains("out of sync"));
    assert!(!app.undo_journal.can_undo());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unified_toolbar_barrier_uses_old_routing() {
    let (mut app, root, ctx) = setup("step4a_barrier");
    terrain(&mut app, 40);
    app.collect_pilot_receipts();
    app.placement.note_external_store_edit();
    gui_click(&mut app, &ctx, "Undo (Ctrl+Z)");
    assert!(!app.undo_journal.can_undo());
    assert_ne!(app.session.as_ref().unwrap().heights()[0], 40);
    std::fs::remove_dir_all(root).unwrap();
}

fn key_frame(app: &mut EditorApp, ctx: &egui::Context, key: egui::Key, shift: bool) {
    let modifiers = egui::Modifiers { ctrl: true, shift, ..Default::default() };
    let _ = ctx.run(egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280.0, 800.0))),
        modifiers,
        events: vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }],
        ..Default::default()
    }, |ctx| app.ui_frame(ctx));
    gui_frame(app, ctx, vec![egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers }]);
}

#[test]
fn unified_shortcuts_respect_text_focus_and_shift_redo() {
    let (mut app, root, ctx) = setup("step4a_focus");
    terrain(&mut app, 40);
    gui_frame(&mut app, &ctx, vec![]);
    // 使用真正的 TextEdit 註冊鍵盤焦點，再由完整編輯器 frame 處理快捷鍵。
    let id = egui::Id::new("step4a_text");
    let mut text = String::from("文字");
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add(egui::TextEdit::singleline(&mut text).id(id)).request_focus();
        });
    });
    assert!(ctx.wants_keyboard_input());
    key_frame(&mut app, &ctx, egui::Key::Z, false);
    assert_eq!(app.session.as_ref().unwrap().heights()[0], 40);
    ctx.memory_mut(|m| m.surrender_focus(id));
    gui_frame(&mut app, &ctx, vec![]);
    key_frame(&mut app, &ctx, egui::Key::Z, false);
    assert_ne!(app.session.as_ref().unwrap().heights()[0], 40);
    key_frame(&mut app, &ctx, egui::Key::Z, true);
    assert_eq!(app.session.as_ref().unwrap().heights()[0], 40);
    key_frame(&mut app, &ctx, egui::Key::Z, false);
    key_frame(&mut app, &ctx, egui::Key::Y, false);
    assert_eq!(app.session.as_ref().unwrap().heights()[0], 40);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unified_settlement_is_atomic_between_terrain_edits() {
    for include_nature in [true, false] {
        let (mut app, root, _) = super::settlement_apply_tests::fixture("step4a_settlement", include_nature, 384);
        let ctx = egui::Context::default();
        app.set_app_language(AppLanguage::English);
        app.collect_pilot_receipts();
        app.undo_journal.clear();
        let baseline = (snapshot(&app), app.placement.objects());
        terrain(&mut app, 40);
        let before = (snapshot(&app), app.placement.objects());
        app.apply_settlement().unwrap();
        let applied = (snapshot(&app), app.placement.objects());
        assert_ne!(applied, before);
        terrain(&mut app, 50);
        let after = (snapshot(&app), app.placement.objects());
        for (expected, depth) in [(&applied, 1), (&before, 0), (&baseline, 0)] {
            app.texture_dirty = false;
            gui_click(&mut app, &ctx, "Undo (Ctrl+Z)");
            assert_eq!(&(snapshot(&app), app.placement.objects()), expected, "{}", app.status_message);
            assert_eq!(app.settlement_undo.len(), depth);
            assert_eq!(app.settlement_redo.len(), 1 - depth);
            assert_eq!(app.status_message, "Unified undo history applied.");
            assert!(!app.is_painting);
        }
        for (expected, depth) in [(&before, 0), (&applied, 1), (&after, 1)] {
            gui_click(&mut app, &ctx, "Redo (Ctrl+Y)");
            assert_eq!(&(snapshot(&app), app.placement.objects()), expected, "{}", app.status_message);
            assert_eq!(app.settlement_undo.len(), depth);
            assert_eq!(app.settlement_redo.len(), 1 - depth);
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn unified_navigation_invalidates_terrain_view() {
    let (mut app, root, _) = setup("step4a_invalidation");
    terrain(&mut app, 40);
    for forward in [false, true] {
        app.texture_dirty = false;
        app.navigate_journal(forward);
        assert!(app.texture_dirty);
        assert!(!app.is_painting);
        assert_eq!(app.status_message, "Unified undo history applied.");
    }
    std::fs::remove_dir_all(root).unwrap();
}
fn event(app: &mut EditorApp) {
    app.scenario.add_events(&[against_rome_scenario::Event {
        name: "面板復原測試".into(),
        actions: vec![against_rome_scenario::Action { text: "step4b".into(), ..Default::default() }],
        ..Default::default()
    }]).unwrap();
}

#[test]
fn unified_panel_newest_then_toolbar_keeps_chronology_and_redo() {
    let (mut app, root, ctx) = setup("step4b_newest");
    let baseline = snapshot(&app);
    terrain(&mut app, 40);
    let terrain_only = snapshot(&app);
    event(&mut app);
    let edited = snapshot(&app);
    app.spawn_ui.open = false;
    app.scenario.open = true;
    app.scenario.history_request = Some(false);
    gui_frame(&mut app, &ctx, vec![]);
    assert_eq!(snapshot(&app), terrain_only);
    assert!(app.pilot_history_consistent());
    app.navigate_journal(false);
    assert_eq!(snapshot(&app), baseline);
    app.navigate_journal(true);
    assert_eq!(snapshot(&app), terrain_only);
    app.scenario.history_request = Some(true);
    app.consume_panel_history_requests();
    assert_eq!(snapshot(&app), edited);
    assert!(app.pilot_history_consistent());
    assert!(!app.undo_journal.can_redo());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unified_panel_buried_step_preserves_other_tools_and_redo_order() {
    let (mut app, root, _) = setup("step4b_buried");
    let baseline = snapshot(&app);
    event(&mut app);
    let event_only = snapshot(&app);
    terrain(&mut app, 40);
    let edited = snapshot(&app);
    assert!(app.navigate_panel(SubsystemId::Scenario, false));
    assert_eq!(app.session.as_ref().unwrap().heights()[0], 40);
    assert_eq!(snapshot(&app).1, baseline.1);
    assert!(app.pilot_history_consistent());
    app.navigate_journal(false);
    assert_eq!(snapshot(&app), baseline);
    app.navigate_journal(true);
    assert_eq!(app.session.as_ref().unwrap().heights()[0], 40);
    app.navigate_journal(true);
    assert_eq!(snapshot(&app), edited);
    // Redo 堆疊中事件埋在地形下方，局部重做仍只操作事件。
    app.navigate_journal(false);
    app.navigate_journal(false);
    assert!(app.navigate_panel(SubsystemId::Scenario, true));
    assert_eq!(snapshot(&app), event_only);
    assert!(app.pilot_history_consistent());
    app.navigate_journal(true);
    assert_eq!(snapshot(&app), edited);
    app.navigate_journal(false);
    assert_eq!(snapshot(&app), event_only);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unified_panel_token_mismatch_clears_with_local_only_fallback() {
    let (mut app, root, _) = setup("step4b_mismatch");
    event(&mut app);
    terrain(&mut app, 40);
    app.collect_pilot_receipts();
    assert!(app.scenario.undo_events());
    assert!(app.navigate_panel(SubsystemId::Scenario, true));
    assert_eq!(app.session.as_ref().unwrap().heights()[0], 40);
    assert!(!app.undo_journal.can_undo() && !app.undo_journal.can_redo());
    assert!(app.status_message.contains("unified history cleared"));
    assert!(!app.status_is_error);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unified_family_copy_uses_placement_receipt_and_shared_panel_navigation() {
    let (mut app, root, map) = fixture("step4b_family", &[(232, None)]);
    app.placement.load_session(&map).unwrap();
    app.collect_pilot_receipts();
    app.undo_journal.clear();
    // 群組複製為單一 store transaction，由 DATA adapter 記錄。
    let baseline = app.placement.objects();
    let store = app.placement.store_mut().unwrap();
    let source = crate::family_ui::validate_native_source(store, baseline[0].slot, &Default::default()).unwrap();
    crate::family_ui::copy_family(store, &source, &Default::default(), [100, 0, 0]).unwrap();
    app.placement.record_edit(1);
    let copied = app.placement.objects();
    terrain(&mut app, 40);
    app.family.history_request = Some(false);
    app.consume_panel_history_requests();
    assert_eq!(app.placement.objects(), baseline);
    assert_eq!(app.session.as_ref().unwrap().heights()[0], 40);
    assert!(app.pilot_history_consistent());
    app.navigate_journal(false);
    app.navigate_journal(true);
    app.family.history_request = Some(true);
    app.consume_panel_history_requests();
    assert_eq!(app.placement.objects(), copied);
    assert!(app.pilot_history_consistent());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn unified_panel_settlement_atomic_undo_redo_then_toolbar() {
    for id in [SubsystemId::Nature, SubsystemId::Placement] {
        for include_nature in [true, false] {
            for buried in [true, false] {
                let (mut app, root, _) = super::settlement_apply_tests::fixture("step4b_settlement", include_nature, 384);
                app.set_app_language(AppLanguage::English);
                app.collect_pilot_receipts();
                app.undo_journal.clear();
                let baseline = (snapshot(&app), app.placement.objects());
                terrain(&mut app, 40);
                let before = (snapshot(&app), app.placement.objects());
                app.apply_settlement().unwrap();
                let applied = (snapshot(&app), app.placement.objects());
                assert_ne!(applied.1, before.1);
                if include_nature { assert_ne!(applied.0.2, before.0.2); }
                // 埋藏的 settlement 無法移動頂端收據，必須清除而非拆成半筆。
                if buried { event(&mut app); }
                for forward in [false, true, false] {
                    assert!(app.navigate_panel(id, forward));
                    let expected = if forward { &applied } else { &before };
                    assert_eq!(app.placement.objects(), expected.1);
                    assert_eq!(app.nature.session.capture(), expected.0.2);
                    assert_eq!(app.settlement_undo.len(), usize::from(forward));
                    assert_eq!(app.settlement_redo.len(), usize::from(!forward));
                    if buried {
                        assert!(!app.undo_journal.can_undo() && !app.undo_journal.can_redo());
                        assert!(app.status_message.contains("unified history cleared"));
                    } else {
                        assert!(app.undo_journal.consistent_tokens(|id| app.token(id)));
                    }
                }
                if !buried {
                    app.navigate_journal(false);
                    assert_eq!((snapshot(&app), app.placement.objects()), baseline);
                    app.navigate_journal(true);
                    assert_eq!((snapshot(&app), app.placement.objects()), before);
                    app.navigate_journal(true);
                    assert_eq!((snapshot(&app), app.placement.objects()), applied);
                    assert_eq!((app.settlement_undo.len(), app.settlement_redo.len()), (1, 0));
                }
                std::fs::remove_dir_all(root).unwrap();
            }
        }
    }
}
