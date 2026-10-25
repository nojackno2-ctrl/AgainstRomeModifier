use super::*;
use crate::terrain::SampleChange;

#[test]
fn status_bar_active_tool_and_brush_size_bilingual() {
    let mut app = EditorApp::initial_state();
    assert_eq!(app.active_tool, ActiveTool::HeightRaise);
    assert_eq!(app.brush_radius, 12.0);

    // Initial state (Raise, radius 12)
    assert_eq!(app.active_tool_status_text(AppLanguage::English), "Tool: Raise (Radius: 12)");
    assert_eq!(app.active_tool_status_text(AppLanguage::TraditionalChinese), "工具: 升高 (半徑: 12)");

    // Change radius
    app.brush_radius = 5.0;
    assert_eq!(app.active_tool_status_text(AppLanguage::English), "Tool: Raise (Radius: 5)");
    assert_eq!(app.active_tool_status_text(AppLanguage::TraditionalChinese), "工具: 升高 (半徑: 5)");

    // Terrain tools
    let cases = [
        (ActiveTool::HeightLower, "Tool: Lower (Radius: 5)", "工具: 降低 (半徑: 5)"),
        (ActiveTool::HeightFlatten, "Tool: Flatten (Radius: 5)", "工具: 整平 (半徑: 5)"),
        (ActiveTool::HeightSmooth, "Tool: Smooth (Radius: 5)", "工具: 平滑 (半徑: 5)"),
        (ActiveTool::HeightRoughen, "Tool: Roughen (Radius: 5)", "工具: 粗糙 (半徑: 5)"),
        (ActiveTool::Water, "Tool: Water (Radius: 5)", "工具: 水面 (半徑: 5)"),
        (ActiveTool::CollisionBlock, "Tool: Block (Radius: 5)", "工具: 阻擋通行 (半徑: 5)"),
        (ActiveTool::CollisionClear, "Tool: Clear (Radius: 5)", "工具: 清除阻擋 (半徑: 5)"),
        (ActiveTool::Material, "Tool: Material (Size: 1)", "工具: 地表材質 (尺寸: 1)"),
    ];
    for (tool, en, zh) in cases {
        app.active_tool = tool;
        assert_eq!(app.active_tool_status_text(AppLanguage::English), en);
        assert_eq!(app.active_tool_status_text(AppLanguage::TraditionalChinese), zh);
    }

    // Nature tool
    app.nature.enabled = true;
    app.nature.operation = NatureOperation::Plant;
    app.nature.brush_size = 3;
    assert_eq!(app.active_tool_status_text(AppLanguage::English), "Tool: Nature (Plant) (Radius: 3) | Density: Normal");
    assert_eq!(app.active_tool_status_text(AppLanguage::TraditionalChinese), "工具: 自然 (種植) (半徑: 3) | 密度: 一般");

    app.nature.operation = NatureOperation::Remove;
    app.nature.brush_size = 7;
    assert_eq!(app.active_tool_status_text(AppLanguage::English), "Tool: Nature (Remove) (Radius: 7)");
    assert_eq!(app.active_tool_status_text(AppLanguage::TraditionalChinese), "工具: 自然 (清除) (半徑: 7)");
}

#[test]
fn status_bar_height_readout_under_cursor_bilingual() {
    let mut app = EditorApp::initial_state();
    assert_eq!(app.canvas_cursor, None);
    assert_eq!(app.cursor_readout_text(AppLanguage::English), None);
    assert_eq!(app.cursor_readout_text(AppLanguage::TraditionalChinese), None);

    // When session is None: height is gracefully omitted
    app.session = None;
    app.canvas_cursor = Some((8192.0, 4096.0, 128, 64));
    assert_eq!(
        app.cursor_readout_text(AppLanguage::English).unwrap(),
        "Cursor: X: 8192, Z: 4096 | Tile: (128, 64)"
    );
    assert_eq!(
        app.cursor_readout_text(AppLanguage::TraditionalChinese).unwrap(),
        "游標: X: 8192, Z: 4096 | 圖塊: (128, 64)"
    );

    // Load a blank terrain session with uniform height 42
    let session = TerrainSession::create_blank(257, 42);
    app.session = Some(session);

    assert_eq!(app.height_at_world(8192.0, 4096.0), Some(42));
    assert_eq!(
        app.cursor_readout_text(AppLanguage::English).unwrap(),
        "Cursor: X: 8192, Z: 4096 | Tile: (128, 64) | Height: 42 | World Y: 168.0"
    );
    assert_eq!(
        app.cursor_readout_text(AppLanguage::TraditionalChinese).unwrap(),
        "游標: X: 8192, Z: 4096 | 圖塊: (128, 64) | 高度: 42 | 世界 Y: 168.0"
    );

    // Apply specific height changes to vertex (100, 50)
    let index = 50 * 257 + 100;
    app.session.as_mut().unwrap().apply_height_changes(&[SampleChange {
        index,
        before: 42,
        after: 99,
    }]);

    let wx = 100.0 * 64.0;
    let wz = 50.0 * 64.0;
    assert_eq!(app.height_at_world(wx, wz), Some(99));
    app.canvas_cursor = Some((wx, wz, 100, 50));
    assert_eq!(
        app.cursor_readout_text(AppLanguage::English).unwrap(),
        "Cursor: X: 6400, Z: 3200 | Tile: (100, 50) | Height: 99 | World Y: 396.0"
    );
    assert_eq!(
        app.cursor_readout_text(AppLanguage::TraditionalChinese).unwrap(),
        "游標: X: 6400, Z: 3200 | 圖塊: (100, 50) | 高度: 99 | 世界 Y: 396.0"
    );

    // Clamping on extreme and negative coordinates without panicking
    assert_eq!(app.height_at_world(-500.0, -100.0), Some(42));
    assert_eq!(app.height_at_world(20000.0, 30000.0), Some(42));
}

#[test]
fn eyedropper_shortcut_and_cursor_sampling() {
    use egui::Key;

    // Pure shortcut mapping
    assert_eq!(
        shortcut_action(Key::I, false, false, false, false),
        Some(ShortcutAction::Eyedropper)
    );
    // Suppressed by modifiers or text focus
    assert_eq!(shortcut_action(Key::I, true, false, false, false), None);
    assert_eq!(shortcut_action(Key::I, false, true, false, false), None);
    assert_eq!(shortcut_action(Key::I, false, false, true, false), None);
    assert_eq!(shortcut_action(Key::I, false, false, false, true), None);

    let mut app = EditorApp::initial_state();

    // 1. Cursor is None: eyedrop prompts user to hover cursor over map
    assert!(!app.eyedrop_material_at_cursor());
    assert!(app.status_message.contains("Hover cursor over the map to pick material"));

    // 2. Dialog / confirm blocks shortcuts
    app.canvas_cursor = Some((1024.0, 1024.0, 16, 16));
    app.terrain_confirm = Some(TerrainBulkAction::Reset);
    assert!(app.shortcuts_blocked());
    assert!(!app.eyedrop_material_at_cursor());
    app.terrain_confirm = None;

    // 3. No map/session loaded
    app.session = None;
    assert!(!app.eyedrop_material_at_cursor());
    assert!(app.status_is_error);
    assert!(app.status_message.contains("No map loaded"));

    // 4. Session loaded
    let session = TerrainSession::create_blank(257, 32);
    app.session = Some(session);

    // Enable nature mode first, to verify eyedropper switches to Material tool and disables nature
    app.nature.enabled = true;
    app.active_tool = ActiveTool::HeightRaise;

    // Eyedrop with no texture session loaded sets informative status
    assert!(!app.eyedrop_material_at_cursor());
    assert!(app.status_message.contains("No material texture data available"));

    // Verify shortcuts_blocked() check in handle_shortcut_action
    app.terrain_confirm = Some(TerrainBulkAction::Reset);
    let tool_before = app.active_tool;
    app.handle_shortcut_action(ShortcutAction::Eyedropper);
    assert_eq!(app.active_tool, tool_before);
}
