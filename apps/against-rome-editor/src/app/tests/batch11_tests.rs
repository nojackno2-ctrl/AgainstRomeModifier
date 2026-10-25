use super::*;

#[test]
fn height_grid_units_edges_and_invalid_coordinates() {
    for size in [193, 257] {
        let mut app = EditorApp::initial_state();
        let mut session = TerrainSession::create_blank(size, 12);
        session.apply_height_changes(&[SampleChange { index: 50 * size + 100, before: 12, after: 99 }]);
        app.session = Some(session);
        let spacing = 16384.0 / (size - 1) as f32;
        assert_eq!(app.height_at_world(100.0 * spacing, 50.0 * spacing), Some(99));
        // 與 Paint 的 round_ties_even 保持一致，半格不得跳到下一個奇數頂點。
        if size == 257 {
            assert_eq!(app.height_at_world(100.5 * spacing, 50.0 * spacing), Some(99));
        }
        assert_eq!(app.height_at_world(16384.0, 16384.0), Some(12));
        assert_eq!(app.height_at_world(f32::NAN, 0.0), None);
        assert_eq!(app.height_at_world(0.0, f32::INFINITY), None);
        app.height_step = 7.5;
        app.canvas_cursor = Some((100.0 * spacing, 50.0 * spacing, 0, 0));
        assert!(app.cursor_readout_text(AppLanguage::English).unwrap().ends_with("World Y: 742.5"));
        assert!(app.cursor_readout_text(AppLanguage::TraditionalChinese).unwrap().ends_with("世界 Y: 742.5"));
    }
}

#[test]
fn cursor_tools_are_settings_only_and_respect_guards() {
    for language in [AppLanguage::English, AppLanguage::TraditionalChinese] {
        let mut app = EditorApp::initial_state();
        app.set_app_language(language);
        app.session = Some(TerrainSession::create_blank(257, 71));
        app.canvas_cursor = Some((6400.0, 3200.0, 100, 50));
        let token = app.session.as_ref().unwrap().history_token();
        assert_eq!(app.cursor_coordinates_text().as_deref(), Some("6400.0, 3200.0"));
        assert!(app.sample_flatten_at_cursor());
        assert_eq!(app.flatten_target, 71);
        assert!(!app.sample_flatten_target);
        assert_eq!(app.active_tool, ActiveTool::HeightFlatten);
        assert!(!app.session.as_ref().unwrap().is_dirty());
        assert_eq!(app.session.as_ref().unwrap().history_token(), token);
        assert!(!app.session.as_ref().unwrap().can_undo());
        app.session.as_mut().unwrap().apply_height_changes(&[SampleChange { index: 20 * 257 + 20, before: 71, after: 10 }]);
        app.session.as_mut().unwrap().commit_stroke();
        app.brush_strength = 255;
        app.handle_viewport_3d_action(ViewportAction::Paint { grid_x: 20.0, grid_y: 20.0 });
        assert!(!app.sample_flatten_at_cursor());
        app.handle_viewport_3d_action(ViewportAction::CommitStroke);
        assert_eq!(app.session.as_ref().unwrap().heights()[20 * 257 + 20], 71);
        app.perform_undo();
        assert_eq!(app.session.as_ref().unwrap().heights()[20 * 257 + 20], 10);
        app.perform_redo();
        assert_eq!(app.session.as_ref().unwrap().heights()[20 * 257 + 20], 71);
        app.terrain_confirm = Some(TerrainBulkAction::Reset);
        assert!(!app.sample_flatten_at_cursor());
        assert_eq!(app.cursor_coordinates_text(), None);
        app.terrain_confirm = None;
        app.loaded_map_path = Some(PathBuf::from("C:/Maps/TUTOR_01"));
        app.map_path_input = "C:/Maps/TUTOR_01".into();
        assert!(app.current_map_is_original());
        assert!(!app.sample_flatten_at_cursor());
        assert!(!app.eyedrop_material_at_cursor());
    }
    for (key, action) in [(egui::Key::H, ShortcutAction::SampleFlattenTarget), (egui::Key::K, ShortcutAction::CopyCursor)] {
        assert_eq!(shortcut_action(key, false, false, false, false), Some(action));
        assert_eq!(shortcut_action(key, false, false, false, true), None);
        assert_eq!(shortcut_action(key, true, false, false, false), None);
    }
}
