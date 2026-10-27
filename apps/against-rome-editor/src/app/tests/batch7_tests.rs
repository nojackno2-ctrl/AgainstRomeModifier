use super::*;

fn gui_frame_at_size(
    app: &mut EditorApp,
    ctx: &egui::Context,
    size: Vec2,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| app.ui_frame(ctx),
    )
}

fn gui_find_prefix(output: &egui::FullOutput, prefix: &str) -> Option<Rect> {
    output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::epaint::Shape::Text(text) if text.galley.text().starts_with(prefix) => {
            Some(Rect::from_min_size(text.pos, text.galley.size()))
        }
        _ => None,
    })
}

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
fn partial_floor_atlas_diagnostic_names_fallback_tiles_and_copies_in_both_languages() {
    let missing = ["missing_tile", "decode_error_tile"];
    for (language, expected_message, copy_button) in [
        (
            AppLanguage::English,
            "3D floor atlas loaded with fallback colors for these textures: missing_tile, decode_error_tile",
            "Copy 3D diagnostic",
        ),
        (
            AppLanguage::TraditionalChinese,
            "3D 地表圖集已載入，但以下材質使用替代色：missing_tile, decode_error_tile",
            "複製 3D 診斷",
        ),
    ] {
        let layer = against_rome_textures::terrain_layer::TerrainLayer {
            width: 128,
            height: 128,
            argb: vec![0xFF12_3456; 128 * 128],
            green: vec![0; 128 * 128],
        };
        let atlas = against_rome_textures::FloorTextureAtlas::create_with_decoder(
            ["available_tile", missing[0], missing[1]],
            |name| (name == "available_tile").then(|| layer.clone()),
        )
        .unwrap();
        assert_eq!(atlas.count(), 3);
        assert_eq!(atlas.unavailable_names(), missing);

        let mut app = EditorApp::initial_state();
        app.view_mode = ViewMode::View3D;
        app.set_app_language(language);
        app.install_3d_floor_atlas(atlas);
        assert_eq!(app.status_message, expected_message);
        assert!(app.status_is_error);

        let ctx = egui::Context::default();
        let output = gui_frame(&mut app, &ctx, vec![]);
        assert!(gui_find(&output, &expected_message).is_some());
        let button = gui_find(&output, copy_button).expect("partial 3D diagnostic copy button");
        gui_frame(&mut app, &ctx, gui_pointer(button.center(), true));
        let output = gui_frame(&mut app, &ctx, gui_pointer(button.center(), false));
        assert_eq!(output.platform_output.copied_text, expected_message);

        let other_language = if language == AppLanguage::English {
            AppLanguage::TraditionalChinese
        } else {
            AppLanguage::English
        };
        app.set_app_language(other_language);
        assert_eq!(
            app.status_message,
            EditorApp::floor_atlas_partial_fallback_message(
                &missing.map(str::to_string),
                other_language == AppLanguage::English,
            )
        );

        let complete_atlas = against_rome_textures::FloorTextureAtlas::create_with_decoder(
            &["available_tile"],
            |_| Some(layer.clone()),
        )
        .unwrap();
        app.install_3d_floor_atlas(complete_atlas);
        assert!(app.status_message.is_empty());
        assert!(!app.status_is_error);
    }
}

#[test]
fn long_3d_fallback_diagnostic_keeps_status_controls_visible_in_narrow_window() {
    let diagnostic = format!(
        "3D floor atlas loaded with fallback colors for these textures: {}",
        (0..24)
            .map(|index| format!("unavailable_floor_texture_{index:02}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let size = Vec2::new(680.0, 720.0);
    let mut app = EditorApp::initial_state();
    app.view_mode = ViewMode::View3D;
    app.set_app_language(AppLanguage::English);
    app.status_message = diagnostic.clone();
    app.status_is_error = true;
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);

    let first = gui_frame_at_size(&mut app, &ctx, size, vec![]);
    let diagnostic_rect = gui_find_prefix(&first, "3D floor atlas loaded with fallback colors")
        .expect("truncated diagnostic remains visible");
    assert!(diagnostic_rect.width() <= size.x * 0.4, "diagnostic must be width-limited: {diagnostic_rect:?}");
    let copy_rect = gui_find(&first, "Copy 3D diagnostic").expect("copy action remains visible");
    let tool_text = app.active_tool_status_text(AppLanguage::English);
    let tool_rect = gui_find(&first, &tool_text).expect("active tool remains visible");
    for rect in [diagnostic_rect, copy_rect, tool_rect] {
        assert!(rect.min.x >= 0.0 && rect.max.x <= size.x, "status control clipped: {rect:?}");
        assert!(rect.min.y >= 0.0 && rect.max.y <= size.y, "status control clipped: {rect:?}");
    }

    let hovered = gui_frame_at_size(
        &mut app,
        &ctx,
        size,
        vec![egui::Event::PointerMoved(diagnostic_rect.center())],
    );
    assert!(gui_find(&hovered, &diagnostic).is_some(), "hover exposes the full diagnostic");

    gui_frame_at_size(
        &mut app,
        &ctx,
        size,
        gui_pointer(copy_rect.center(), true),
    );
    let copied = gui_frame_at_size(
        &mut app,
        &ctx,
        size,
        gui_pointer(copy_rect.center(), false),
    );
    assert_eq!(copied.platform_output.copied_text, diagnostic);
}

#[test]
fn oversized_floor_atlas_uses_cpu_fallback_and_reports_gl_limit() {
    let layer = against_rome_textures::terrain_layer::TerrainLayer {
        width: 128,
        height: 128,
        argb: vec![0xFF12_3456; 128 * 128],
        green: vec![0; 128 * 128],
    };
    let atlas = || {
        against_rome_textures::FloorTextureAtlas::create_with_decoder(
            &["available_tile"],
            |_| Some(layer.clone()),
        )
        .unwrap()
    };
    for (language, expected_message, copy_button) in [
        (
            AppLanguage::English,
            "3D floor atlas is 2048×2048, above this device's GL_MAX_TEXTURE_SIZE limit of 1024; using CPU fallback colors.",
            "Copy 3D diagnostic",
        ),
        (
            AppLanguage::TraditionalChinese,
            "3D 地表圖集大小為 2048×2048，超出此裝置的 GL_MAX_TEXTURE_SIZE 上限 1024；目前改用 CPU 替代色呈現。",
            "複製 3D 診斷",
        ),
    ] {
        let mut app = EditorApp::initial_state();
        app.view_mode = ViewMode::View3D;
        app.set_app_language(language);
        app.set_floor_atlas_gl_capability(FloorAtlasGlCapability::MaxTextureSize(1024));
        app.install_3d_floor_atlas(atlas());
        assert_eq!(app.status_message, expected_message);
        assert!(app.status_is_error);
        assert!(app.viewport_3d.floor_atlas.is_some());

        let ctx = egui::Context::default();
        app.viewport_3d.sync_floor_atlas_texture(&ctx);
        assert!(app.viewport_3d.floor_atlas_texture.is_none());
        app.view_mode = ViewMode::Blended;
        let output = gui_frame(&mut app, &ctx, vec![]);
        assert!(gui_find(&output, expected_message).is_none());
        assert!(gui_find(&output, copy_button).is_none());
        app.view_mode = ViewMode::View3D;
        let output = gui_frame(&mut app, &ctx, vec![]);
        let button = gui_find(&output, copy_button).expect("oversize 3D diagnostic copy button");
        gui_frame(&mut app, &ctx, gui_pointer(button.center(), true));
        let output = gui_frame(&mut app, &ctx, gui_pointer(button.center(), false));
        assert_eq!(output.platform_output.copied_text, expected_message);

        let other_language = if language == AppLanguage::English {
            AppLanguage::TraditionalChinese
        } else {
            AppLanguage::English
        };
        app.set_app_language(other_language);
        assert!(app.status_message.contains("2048×2048"));
        assert!(app.status_message.contains("1024"));
        assert_ne!(app.status_message, expected_message);
    }

    let mut compatible_app = EditorApp::initial_state();
    compatible_app.view_mode = ViewMode::View3D;
    compatible_app
        .set_floor_atlas_gl_capability(FloorAtlasGlCapability::MaxTextureSize(2048));
    compatible_app.install_3d_floor_atlas(atlas());
    assert!(!EditorApp::is_3d_material_diagnostic(&compatible_app.status_message));
    assert!(!compatible_app.status_is_error);
    let ctx = egui::Context::default();
    compatible_app
        .viewport_3d
        .sync_floor_atlas_texture(&ctx);
    assert!(compatible_app.viewport_3d.floor_atlas_texture.is_some());

    let mut unknown_app = EditorApp::initial_state();
    unknown_app.view_mode = ViewMode::View3D;
    unknown_app.set_app_language(AppLanguage::English);
    unknown_app.set_floor_atlas_gl_capability(FloorAtlasGlCapability::Unknown);
    unknown_app.install_3d_floor_atlas(atlas());
    assert_eq!(
        unknown_app.status_message,
        "3D floor atlas is 2048×2048, but the OpenGL texture limit is unknown; using CPU fallback colors."
    );
    let ctx = egui::Context::default();
    unknown_app.viewport_3d.sync_floor_atlas_texture(&ctx);
    assert!(unknown_app.viewport_3d.floor_atlas_texture.is_none());
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
    let layer = against_rome_textures::terrain_layer::TerrainLayer {
        width: 128,
        height: 128,
        argb: vec![0xFF12_3456; 128 * 128],
        green: vec![0; 128 * 128],
    };
    let full_atlas = || {
        against_rome_textures::FloorTextureAtlas::create_with_decoder(
            vec!["4BA___51"],
            |_| Some(layer.clone()),
        )
        .unwrap()
    };
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
        app.install_3d_floor_atlas(full_atlas());
        assert!(app.viewport_3d.floor_atlas.is_some());
        assert!(app.status_message.is_empty());
        assert!(!app.status_is_error);
    }

    let mut app = EditorApp::initial_state();
    let unrelated = "3D floor textures unavailable / 3D 地表材質無法載入：invalid archive";
    app.status_message = unrelated.into();
    app.status_is_error = true;
    app.install_3d_floor_atlas(full_atlas());
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
    app.set_app_language(AppLanguage::English);
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
    assert_eq!(
        app.status_message,
        EditorApp::floor_atlas_partial_fallback_message(
            &["4BA___51".to_string()],
            true,
        )
    );
    assert!(app.status_is_error);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn minimum_window_size_hud_and_controls_remain_unclipped() {
    for (lang, mode) in [
        (AppLanguage::English, ViewMode::View3D),
        (AppLanguage::TraditionalChinese, ViewMode::View3D),
        (AppLanguage::English, ViewMode::Blended),
        (AppLanguage::TraditionalChinese, ViewMode::Blended),
    ] {
        let mut app = EditorApp::initial_state();
        app.set_app_language(lang);
        app.view_mode = mode;
        app.session = Some(TerrainSession::create_blank(257, 32));
        app.viewport_3d.hover_object = Some((15, 20300));
        app.viewport_3d.hover_tile = Some((128, 128));

        let ctx = egui::Context::default();
        let size = Vec2::new(800.0, 600.0);
        let output = gui_frame_at_size(&mut app, &ctx, size, vec![]);

        if mode == ViewMode::View3D {
            let vp_rect = app
                .viewport_3d
                .last_rect
                .expect("3D viewport rect must exist");
            assert!(vp_rect.min.x >= 0.0 && vp_rect.max.x <= size.x);
            assert!(vp_rect.min.y >= 0.0 && vp_rect.max.y <= size.y);
            assert!(
                vp_rect.width() >= 200.0,
                "viewport width must be usable: {vp_rect:?}"
            );
            assert!(
                vp_rect.height() >= 100.0,
                "viewport height must be usable: {vp_rect:?}"
            );

            // Verify HUD text is fully contained within the 3D viewport canvas bounds
            let hud_prefix = if lang == AppLanguage::English {
                "Controls: Right-drag: Orbit"
            } else {
                "操作指南：右鍵拖曳：旋轉視角"
            };
            let hud_shape = output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::epaint::Shape::Text(t) if t.galley.text().starts_with(hud_prefix) => {
                        Some((
                            Rect::from_min_size(t.pos, t.galley.size()),
                            s.clip_rect,
                            t.galley.text(),
                        ))
                    }
                    _ => None,
                })
                .expect("HUD overlay text must be rendered");

            let (hud_rect, clip_rect, hud_text) = hud_shape;
            assert!(
                hud_rect.min.x >= vp_rect.min.x + 7.5,
                "HUD min x out of bounds: {hud_rect:?} vs {vp_rect:?}"
            );
            assert!(
                hud_rect.max.x <= vp_rect.max.x,
                "HUD text must not overflow 3D viewport or clip rect: {hud_rect:?} vs {vp_rect:?} clip={clip_rect:?}"
            );
            assert!(
                hud_rect.max.y <= vp_rect.max.y,
                "HUD text must fit vertically within viewport: {hud_rect:?} vs {vp_rect:?}"
            );
            assert!(
                hud_rect.min.y >= vp_rect.min.y,
                "HUD text must not overflow the top of the viewport: {hud_rect:?} vs {vp_rect:?}"
            );
            assert!(
                clip_rect.contains_rect(hud_rect),
                "HUD text must fit within its actual painter clip: {hud_rect:?} vs {clip_rect:?}"
            );
            // Verify full content is preserved in the wrapped galley
            let paint_label = if lang == AppLanguage::English {
                "Click: Paint / Pick Object"
            } else {
                "左鍵點選：繪製／選取物件"
            };
            let viewport_label = if lang == AppLanguage::English {
                "3D Terrain Viewport:"
            } else {
                "3D 地形視角："
            };
            assert!(hud_text.contains(paint_label));
            assert!(hud_text.contains(viewport_label));
        }

        // Verify top-level status bar and tool text do not overflow horizontal bounds
        for s in &output.shapes {
            if let egui::epaint::Shape::Text(t) = &s.shape {
                let r = Rect::from_min_size(t.pos, t.galley.size());
                assert!(
                    r.max.x <= size.x + 1.0,
                    "Text shape {r:?} exceeds window width {size:?}: {:?}",
                    t.galley.text()
                );
            }
        }
    }
}
