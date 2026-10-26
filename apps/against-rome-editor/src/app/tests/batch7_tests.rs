use super::*;

#[test]
fn cursor_tiles_follow_scenario_world_transform() {
    let rect = Rect::from_min_size(Pos2::new(-310.0, 47.0), Vec2::new(2048.0, 1024.0));
    for (x, z, expected) in [(0.0, 0.0, (0, 0)), (63.9, 64.0, (0, 1)),
        (8192.0, 4096.0, (128, 64)), (16384.0, 16384.0, (255, 255))] {
        let screen = crate::scenario_canvas::screen_from_world(rect, x, z);
        let world = crate::scenario_canvas::world_from_screen(rect, screen);
        assert_eq!(cursor_tile(world.0, world.1), expected);
    }
    // 3D 場景一單位為 256 世界單位；圖塊讀數仍採 64 世界單位。
    assert_eq!(cursor_tile(32.0 * 256.0, 16.0 * 256.0), (128, 64));
}

fn navigation_frame(app: &mut EditorApp, ctx: &egui::Context, key: egui::Key, shift: bool) {
    let modifiers = egui::Modifiers { shift, ..Default::default() };
    let _ = ctx.run(egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        modifiers,
        events: vec![egui::Event::PointerMoved(app.last_canvas_rect.center()),
            egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }],
        ..Default::default()
    }, |ctx| app.ui_frame(ctx));
}

#[test]
fn hovered_shift_arrow_pans_once_and_dialog_blocks_plain_arrow() {
    for shift in [false, true] {
        let mut app = EditorApp::initial_state();
        let ctx = egui::Context::default();
        gui_frame(&mut app, &ctx, vec![]);
        navigation_frame(&mut app, &ctx, egui::Key::ArrowRight, shift);
        assert_eq!(app.pan.x, if shift { -256.0 } else { -64.0 });
    }
    let mut app = EditorApp::initial_state();
    let ctx = egui::Context::default();
    gui_frame(&mut app, &ctx, vec![]);
    app.terrain_confirm = Some(TerrainBulkAction::Reset);
    navigation_frame(&mut app, &ctx, egui::Key::ArrowRight, false);
    assert_eq!(app.pan, Vec2::ZERO);
}

#[test]
fn navigation_preserves_text_focus_and_3d_view() {
    for key in [egui::Key::Home, egui::Key::PageUp, egui::Key::PageDown, egui::Key::ArrowRight] {
        assert_eq!(shortcut_action(key, false, true, false, true), None);
        assert_eq!(shortcut_action(key, false, false, false, true), None);
        let mut app = EditorApp::initial_state();
        app.view_mode = ViewMode::View3D;
        app.zoom = 2.0;
        app.pan = Vec2::new(5.0, 9.0);
        let ctx = egui::Context::default();
        navigation_frame(&mut app, &ctx, key, true);
        assert_eq!(app.zoom, 2.0);
        assert_eq!(app.pan, Vec2::new(5.0, 9.0));
    }
}

#[test]
fn view_controls_and_copy_loaded_folder_are_bilingual_and_headless() {
    for (language, reset, copy, zoom) in [
        (AppLanguage::English, "Reset 2D view", "Copy map folder", "2D zoom: 200%"),
        (AppLanguage::TraditionalChinese, "重設 2D 視角", "複製地圖資料夾", "2D 縮放: 200%"),
    ] {
        let mut app = EditorApp::initial_state();
        app.set_app_language(language);
        assert_eq!(app.map_folder_clipboard(), None);
        let loaded = PathBuf::from("D:/Maps/設計 ENDL_005");
        app.loaded_map_path = Some(loaded.clone());
        app.map_path_input = "尚未載入的其他路徑".into();
        app.zoom = 2.0;
        app.pan = Vec2::splat(80.0);
        let ctx = egui::Context::default();
        gui_frame(&mut app, &ctx, vec![]);
        let out = gui_frame(&mut app, &ctx, vec![]);
        assert!(gui_find(&out, zoom).is_some());
        let button = gui_find(&out, copy).unwrap();
        gui_frame(&mut app, &ctx, gui_pointer(button.center(), true));
        let out = gui_frame(&mut app, &ctx, gui_pointer(button.center(), false));
        assert_eq!(out.platform_output.copied_text, loaded.display().to_string());
        let button = gui_find(&out, reset).unwrap();
        gui_frame(&mut app, &ctx, gui_pointer(button.center(), true));
        gui_frame(&mut app, &ctx, gui_pointer(button.center(), false));
        assert_eq!((app.zoom, app.pan), (1.0, Vec2::ZERO));
        assert!(!app.session.as_ref().unwrap().is_dirty());
    }
}

#[test]
fn three_d_material_fallback_diagnostic_can_be_copied_in_both_languages() {
    for (language, diagnostic, button) in [
        (
            AppLanguage::English,
            "3D floor texture archive floortex.dat not found; showing relief fallback.",
            "Copy 3D diagnostic",
        ),
        (
            AppLanguage::TraditionalChinese,
            "找不到 3D 地表材質檔 floortex.dat；目前以地形起伏色彩替代顯示。",
            "複製 3D 診斷",
        ),
    ] {
        let mut app = EditorApp::initial_state();
        app.set_app_language(language);
        app.view_mode = ViewMode::View3D;
        app.status_message = diagnostic.into();
        let ctx = egui::Context::default();
        let settled = gui_frame(&mut app, &ctx, vec![]);
        let button_rect = gui_find(&settled, button).expect("3D diagnostic copy button");
        gui_frame(&mut app, &ctx, gui_pointer(button_rect.center(), true));
        let output = gui_frame(&mut app, &ctx, gui_pointer(button_rect.center(), false));
        assert_eq!(output.platform_output.copied_text, diagnostic);
        assert!(!app.status_is_error);
    }

    let mut app = EditorApp::initial_state();
    app.status_message = "Unified undo history applied.".into();
    let ctx = egui::Context::default();
    let output = gui_frame(&mut app, &ctx, vec![]);
    assert!(gui_find(&output, "Copy 3D diagnostic").is_none());
    assert!(gui_find(&output, "複製 3D 診斷").is_none());
}

#[test]
fn three_d_material_diagnostic_is_only_shown_in_3d_view() {
    let mut app = EditorApp::initial_state();
    app.set_app_language(AppLanguage::TraditionalChinese);
    app.status_message = EditorApp::floor_atlas_fallback_message(false).into();
    let ctx = egui::Context::default();

    let output_2d = gui_frame(&mut app, &ctx, vec![]);
    assert!(gui_find(&output_2d, app.status_message.as_str()).is_none());
    assert!(gui_find(&output_2d, "複製 3D 診斷").is_none());

    app.view_mode = ViewMode::View3D;
    let output_3d = gui_frame(&mut app, &ctx, vec![]);
    assert!(gui_find(&output_3d, app.status_message.as_str()).is_some());
    assert!(gui_find(&output_3d, "複製 3D 診斷").is_some());
}

#[test]
fn three_d_material_fallback_status_tracks_language_switches() {
    let mut app = EditorApp::initial_state();
    app.status_message = EditorApp::floor_atlas_fallback_message(false).into();

    app.set_app_language(AppLanguage::English);
    assert_eq!(
        app.status_message,
        EditorApp::floor_atlas_fallback_message(true)
    );

    app.set_app_language(AppLanguage::TraditionalChinese);
    assert_eq!(
        app.status_message,
        EditorApp::floor_atlas_fallback_message(false)
    );

    app.status_message = "3D floor textures unavailable / 3D 地表材質無法載入: invalid archive".into();
    app.set_app_language(AppLanguage::English);
    assert_eq!(
        app.status_message,
        "3D floor textures unavailable / 3D 地表材質無法載入: invalid archive"
    );
}

#[test]
fn successful_3d_floor_atlas_clears_only_exact_fallback_statuses() {
    for (language, fallback) in [
        (
            AppLanguage::English,
            "3D floor texture archive floortex.dat not found; showing relief fallback.",
        ),
        (
            AppLanguage::TraditionalChinese,
            "找不到 3D 地表材質檔 floortex.dat；目前以地形起伏色彩替代顯示。",
        ),
    ] {
        let mut app = EditorApp::initial_state();
        app.set_app_language(language);
        app.status_message = fallback.into();
        app.install_3d_floor_atlas(
            against_rome_textures::FloorTextureAtlas::create_with_decoder(vec!["4BA___51"], |_| None)
                .unwrap(),
        );
        assert!(app.viewport_3d.floor_atlas.is_some());
        assert!(app.status_message.is_empty());
        assert!(!app.status_is_error);
    }

    let mut app = EditorApp::initial_state();
    let unrelated = "3D floor textures unavailable / 3D 地表材質無法載入：invalid archive";
    app.status_message = unrelated.into();
    app.status_is_error = true;
    app.install_3d_floor_atlas(
        against_rome_textures::FloorTextureAtlas::create_with_decoder(vec!["4BA___51"], |_| None)
            .unwrap(),
    );
    assert_eq!(app.status_message, unrelated);
    assert!(app.status_is_error);
}

#[test]
fn repaired_floortex_archive_at_same_path_retries_atlas_load() {
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "arm-floor-atlas-retry-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let map_dir = root.join("MAPS").join("TEST");
    fs::create_dir_all(&map_dir).unwrap();
    fs::write(
        map_dir.join("boden.txt"),
        "[Dimension]\n2\n[Texturen]\n4BA___51\n4BA___51\n4BA___51\n4BA___51\n",
    )
    .unwrap();

    let archive_path = root.join("floortex.dat");
    fs::write(&archive_path, b"broken archive").unwrap();
    let mut app = EditorApp::initial_state();
    app.loaded_map_path = Some(map_dir);
    app.refresh_3d_floor_atlas();
    assert!(app.viewport_3d.floor_atlas.is_none());
    assert!(app.status_is_error);
    let first_error = app.status_message.clone();

    // An unchanged failed archive is memoized, so rendering another frame does not retry it.
    app.refresh_3d_floor_atlas();
    assert_eq!(app.status_message, first_error);
    assert!(app.viewport_3d.floor_atlas.is_none());

    // Replace the same path with a valid empty ZIP. Its length changes even on filesystems
    // whose modification timestamps have coarse resolution, invalidating the failed signature.
    fs::write(
        &archive_path,
        b"PK\x05\x06\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00",
    )
    .unwrap();
    app.refresh_3d_floor_atlas();
    assert!(app.viewport_3d.floor_atlas.is_some());
    assert!(app.status_message.is_empty());
    assert!(!app.status_is_error);

    fs::remove_dir_all(root).unwrap();
}
