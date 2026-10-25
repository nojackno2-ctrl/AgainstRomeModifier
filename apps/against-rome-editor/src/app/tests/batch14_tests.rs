use super::*;
use crate::getting_started_ui::GettingStartedAction;
use crate::minimap_ui::MinimapNavigator;

#[test]
fn frame_selected_data_object_uses_world_scale_and_keeps_history() {
    let (mut app, root, map) = spawn_save_tests::fixture("batch14_frame", &[(232, None)]);
    app.placement.load_session(&map).unwrap();
    app.placement.select_object(Some((0, 1)));
    app.last_canvas_rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 500.0));
    for dimension in [193, 257] {
        app.session = Some(TerrainSession::create_blank(dimension, 32));
        let token = app.session.as_ref().unwrap().history_token();
        app.view_mode = ViewMode::Blended;
        app.handle_shortcut_action(ShortcutAction::FrameSelection);
        let map_rect = Rect::from_center_size(app.last_canvas_rect.center() + app.pan, egui::Vec2::splat(500.0 * app.zoom));
        let world = crate::scenario_canvas::world_from_screen(map_rect, app.last_canvas_rect.center());
        assert!((world.0 - 4000.0).abs() < 0.01 && (world.1 - 4000.0).abs() < 0.01);
        app.view_mode = ViewMode::View3D;
        app.handle_shortcut_action(ShortcutAction::FrameSelection);
        assert_eq!(app.viewport_3d.camera.target.x, 4000.0 / 256.0);
        app.event_graph.open = true;
        app.viewport_3d.center_on_world(12000.0, 8000.0);
        let before = app.viewport_3d.camera.target;
        app.handle_shortcut_action(ShortcutAction::FrameSelection);
        assert_eq!(app.viewport_3d.camera.target, before);
        app.event_graph.open = false;
        assert_eq!(app.session.as_ref().unwrap().history_token(), token);
        assert!(!app.session.as_ref().unwrap().is_dirty());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn continuous_navigation_matches_canvas_world_for_all_grid_sizes() {
    let size = egui::vec2(800.0, 500.0);
    for dimension in [193, 257] {
        for zoom in [0.5, 1.0, 3.0] {
            for (x, z) in [(0.0, 0.0), (8192.0, 8192.0), (12288.0, 4096.0)] {
                let mut app = EditorApp::initial_state();
                app.session = Some(TerrainSession::create_blank(dimension, 32));
                let pan = MinimapNavigator::focus_world_pan(x, z, size, zoom);
                let map = Rect::from_center_size(Pos2::new(400.0, 250.0) + pan, egui::Vec2::splat(500.0 * zoom));
                let world = crate::scenario_canvas::world_from_screen(map, Pos2::new(400.0, 250.0));
                assert!((world.0 - x).abs() < 0.01 && (world.1 - z).abs() < 0.01);
                let mini = Rect::from_min_size(Pos2::new(10.0, 20.0), egui::vec2(200.0, 200.0));
                let p = MinimapNavigator::world_to_minimap_point(x, z, mini);
                assert_eq!(MinimapNavigator::point_to_world(p, mini), Some((x, z)));
            }
        }
    }
    assert!(MinimapNavigator::point_to_world(Pos2::new(f32::NAN, 0.0), Rect::EVERYTHING).is_none());
}

#[test]
fn workflow_requires_map_and_respects_modal_blockers() {
    for lang in [AppLanguage::English, AppLanguage::TraditionalChinese] {
        let mut app = EditorApp::initial_state();
        app.set_app_language(lang);
        app.handle_getting_started_action(GettingStartedAction::OpenMapProperties);
        assert!(!app.map_properties_open);
        assert_eq!(app.status_message, tr(lang, "Load a map first.", "請先載入地圖。"));
        app.handle_getting_started_action(GettingStartedAction::OpenMaps);
        assert!(app.maps.open);
        app.session = Some(TerrainSession::create_blank(257, 32));
        app.loaded_map_path = Some(PathBuf::from("workflow-map"));
        app.event_graph.open = true;
        app.handle_getting_started_action(GettingStartedAction::OpenMapProperties);
        assert!(!app.map_properties_open);
        assert!(!app.frame_selection_target());
        app.event_graph.open = false;
        app.handle_getting_started_action(GettingStartedAction::OpenMapProperties);
        assert!(app.map_properties_open);
    }
    assert_eq!(crate::viewport_3d::camera_key_action(egui::Key::F, false, false), None);
    assert_eq!(shortcut_action(egui::Key::F, false, false, false, true), None);
}
