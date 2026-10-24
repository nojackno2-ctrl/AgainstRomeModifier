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