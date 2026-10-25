use super::*;
use crate::getting_started_ui::GettingStartedAction;
use crate::minimap_ui::{MinimapMarker, MinimapMarkerKind, MinimapNavResult, MinimapNavigator};

#[test]
fn test_minimap_markers_and_3d_navigation() {
    let mut app = EditorApp::initial_state();
    assert_eq!(app.view_mode, ViewMode::Blended);

    // 建立測試標記
    let marker_sel = MinimapMarker::selected(8192.0, 4096.0);
    assert_eq!(marker_sel.kind, MinimapMarkerKind::SelectedObject);
    assert_eq!(marker_sel.world_x, 8192.0);
    assert_eq!(marker_sel.world_z, 4096.0);

    let marker_cam = MinimapMarker::camera(2048.0, 1024.0);
    assert_eq!(marker_cam.kind, MinimapMarkerKind::Camera);

    let bounds = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(200.0, 200.0));
    let pt_sel = MinimapNavigator::world_to_minimap_point(marker_sel.world_x, marker_sel.world_z, bounds);
    assert_eq!(pt_sel, Pos2::new(100.0, 50.0));

    // 模擬在小地圖點擊 (100.0, 50.0) 產生導航結果
    let nav = MinimapNavResult {
        canvas_pan: Vec2::new(50.0, -25.0),
        world_x: 8192.0,
        world_z: 4096.0,
        tile_pos: Pos2::new(32.0, 16.0),
    };

    // 2D 畫布模式下，更新 pan
    app.pan = nav.canvas_pan;
    assert_eq!(app.pan, Vec2::new(50.0, -25.0));

    // 切換至 3D 視角模式下，模擬小地圖點擊跳轉同步更新 3D 攝影機目標
    app.view_mode = ViewMode::View3D;
    app.viewport_3d.center_on_world(nav.world_x, nav.world_z);
    assert_eq!(app.viewport_3d.camera.target.x, 32.0);
    assert_eq!(app.viewport_3d.camera.target.z, 16.0);
}

#[test]
fn test_frame_selection_shortcut_and_target() {
    let mut app = EditorApp::initial_state();

    // 1. 快捷鍵映射驗證：F 觸發 FrameSelection，受 text_focus 與修飾鍵保護
    assert_eq!(
        shortcut_action(egui::Key::F, false, false, false, false),
        Some(ShortcutAction::FrameSelection)
    );
    assert_eq!(
        shortcut_action(egui::Key::F, true, false, false, false),
        None
    );
    assert_eq!(
        shortcut_action(egui::Key::F, false, false, false, true),
        None
    );

    // 2. 未選取任何物件時，聚焦回傳 false，不改變鏡頭位置
    assert!(!app.frame_selection_target());

    // 3. 測試 3D 視角下的選取物件聚焦
    app.view_mode = ViewMode::View3D;
    // 初始鏡頭目標 (32.0, 0.0, 32.0)
    assert_eq!(app.viewport_3d.camera.target.x, 32.0);
    assert_eq!(app.viewport_3d.camera.target.z, 32.0);

    // 透過 viewport_3d.center_on_world 直接聚焦至指定世界座標
    app.viewport_3d.center_on_world(12288.0, 6144.0);
    assert_eq!(app.viewport_3d.camera.target.x, 48.0);
    assert_eq!(app.viewport_3d.camera.target.z, 24.0);

    // 4. 執行 handle_shortcut_action(ShortcutAction::FrameSelection)
    // 由於未選取物件，狀態列回報「尚未選取可聚焦之物件」
    app.handle_shortcut_action(ShortcutAction::FrameSelection);
    assert_eq!(
        app.status_message,
        tr(app.lang(), "No object selected to frame.", "尚未選取可聚焦之物件。")
    );
}

#[test]
fn test_getting_started_workflow_action_dispatch() {
    let mut app = EditorApp::initial_state();

    // 預設新手入門面板為關閉
    assert!(!app.getting_started.open);
    app.getting_started.open = true;
    assert!(app.getting_started.open);

    app.session = Some(TerrainSession::create_blank(257, 32));
    app.loaded_map_path = Some(PathBuf::from("workflow-map"));
    // 驗證步驟一動作分發
    assert!(!app.maps.open);
    app.handle_getting_started_action(GettingStartedAction::OpenMaps);
    assert!(app.maps.open);

    assert!(!app.map_properties_open);
    app.handle_getting_started_action(GettingStartedAction::OpenMapProperties);
    assert!(app.map_properties_open);

    // 驗證步驟二動作分發
    assert!(!app.toolbox.open);
    app.handle_getting_started_action(GettingStartedAction::OpenToolbox);
    assert!(app.toolbox.open);

    assert_eq!(app.view_mode, ViewMode::Blended);
    app.handle_getting_started_action(GettingStartedAction::Toggle3DView);
    assert_eq!(app.view_mode, ViewMode::View3D);
    app.handle_getting_started_action(GettingStartedAction::Toggle3DView);
    assert_eq!(app.view_mode, ViewMode::Blended);

    // 驗證步驟三動作分發
    assert!(!app.textures.open);
    app.handle_getting_started_action(GettingStartedAction::OpenTextures);
    assert!(app.textures.open);

    assert!(!app.navigation.open);
    app.handle_getting_started_action(GettingStartedAction::OpenNavigation);
    assert!(app.navigation.open);

    // 驗證步驟四動作分發
    assert!(!app.placement.open);
    app.handle_getting_started_action(GettingStartedAction::OpenPlacement);
    assert!(app.placement.open);

    assert!(!app.spawn_ui.open);
    app.handle_getting_started_action(GettingStartedAction::OpenSpawns);
    assert!(app.spawn_ui.open);

    // 驗證步驟五動作分發
    assert!(!app.scenario.open);
    app.handle_getting_started_action(GettingStartedAction::OpenScenario);
    assert!(app.scenario.open);

    app.minimap.open = false;
    assert!(!app.minimap.open);
    app.handle_getting_started_action(GettingStartedAction::OpenMinimap);
    assert!(app.minimap.open);
}
