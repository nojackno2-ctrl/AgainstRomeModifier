use super::*;
use crate::scenario_canvas::{screen_from_world, world_from_screen, PickTarget};
use against_rome_scenario::{Action, Event, ScenarioDocument, WorldPosition};

fn rectangle_fixture() -> (EditorApp, PathBuf, egui::Context) {
    let (mut app, root, ctx) = fixture();
    let mut doc = app.scenario.document().clone();
    let target_id = uuid::Uuid::new_v4();
    doc.spawns.push(against_rome_scenario::Spawn { id: target_id, alias: "unit".into(),
        count: 1, x: 4000., z: 5000., ..Default::default() });
    for event in &mut doc.events {
        event.conditions = vec![against_rome_scenario::Condition { kind: 2, target_id,
            min_x: 2000, min_z: 3000, max_x: 6000, max_z: 7000, ..Default::default() }];
    }
    let dir = root.join("MAPS/ENDL_006");
    std::fs::write(dir.join("arm_scenario.json"), doc.to_json().unwrap()).unwrap();
    app.scenario = ScenarioUi::default();
    app.scenario.load_session(&dir).unwrap();
    (app, root, ctx)
}

fn arm_rectangle(app: &mut EditorApp) {
    app.scenario.canvas.pick = Some(PickTarget::Condition { event: 0, condition: 0,
        snapshot: app.scenario.document().events[0].clone() });
}

fn bounds(app: &EditorApp) -> (i32, i32, i32, i32) {
    let c = &app.scenario.document().events[0].conditions[0];
    (c.min_x, c.min_z, c.max_x, c.max_z)
}

#[test]
fn scenario_canvas_rectangle_directions_release_one_undo_no_terrain_or_selection() {
    let (mut app, root, ctx) = rectangle_fixture();
    app.zoom = 0.6;
    app.pan = egui::vec2(18., -12.);
    gui_frame(&mut app, &ctx, vec![]);
    let before = app.scenario.document().clone();
    let heights = app.session.as_ref().unwrap().heights().to_vec();
    let collision = app.session.as_ref().unwrap().collision().to_vec();
    let disk = save_artifact_snapshot(&root.join("MAPS/ENDL_006"));
    for (start, end) in [((4100.2, 5100.8), (9200.6, 10200.3)),
        ((9200.6, 5100.8), (4100.2, 10200.3)),
        ((4100.2, 10200.3), (9200.6, 5100.8)),
        ((9200.6, 10200.3), (4100.2, 5100.8))] {
        for selection in [false, true] {
            app.active_tool = ActiveTool::HeightRaise;
            app.box_select = selection;
            arm_rectangle(&mut app);
            let start = screen_from_world(app.last_map_rect, start.0, start.1);
            let end = screen_from_world(app.last_map_rect, end.0, end.1);
            gui_frame(&mut app, &ctx, gui_pointer(start, true));
            gui_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            assert_eq!(*app.scenario.document(), before);
            assert!(!app.scenario.can_undo_events());
            gui_frame(&mut app, &ctx, gui_pointer(end, false));
            assert_eq!(bounds(&app), (4100, 5101, 9201, 10200));
            assert!(app.status_message.contains("Rectangle updated"));
            assert_eq!(app.session.as_ref().unwrap().heights(), heights);
            assert_eq!(app.session.as_ref().unwrap().collision(), collision);
            assert!(!app.is_painting && app.box_drag_start.is_none());
            assert_eq!(app.box_select, selection);
            assert!(app.scenario.undo_events());
            assert_eq!(*app.scenario.document(), before);
            assert!(!app.scenario.can_undo_events());
        }
    }
    assert_eq!(save_artifact_snapshot(&root.join("MAPS/ENDL_006")), disk);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn scenario_canvas_rectangle_clamps_outside_map_and_canvas() {
    let (mut app, root, ctx) = rectangle_fixture();
    app.zoom = 0.4;
    gui_frame(&mut app, &ctx, vec![]);
    for reverse in [false, true] {
        arm_rectangle(&mut app);
        let low = app.last_map_rect.min - egui::vec2(20., 20.);
        let high = app.last_map_rect.max + egui::vec2(20., 20.);
        assert!(app.last_canvas_rect.contains(low) && app.last_canvas_rect.contains(high));
        let (start, end) = if reverse { (high, low) } else { (low, high) };
        gui_frame(&mut app, &ctx, gui_pointer(start, true));
        gui_frame(&mut app, &ctx, gui_pointer(end, false));
        assert_eq!(bounds(&app), (0, 0, 16383, 16383));
        assert!(app.scenario.undo_events());
        assert!(!app.scenario.can_undo_events());
    }
    arm_rectangle(&mut app);
    let start = app.last_map_rect.center();
    let end = app.last_canvas_rect.max + egui::vec2(100., 100.);
    gui_frame(&mut app, &ctx, gui_pointer(start, true));
    gui_frame(&mut app, &ctx, gui_pointer(end, false));
    assert_eq!(bounds(&app), (8192, 8192, 16383, 16383));
    assert!(app.scenario.undo_events());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn scenario_canvas_rectangle_cancel_zero_size_and_stale_snapshot() {
    let (mut app, root, ctx) = rectangle_fixture();
    let before = app.scenario.document().clone();
    let heights = app.session.as_ref().unwrap().heights().to_vec();
    app.active_tool = ActiveTool::HeightRaise;
    app.box_select = true;
    let start = app.last_map_rect.center();
    for en in [true, false] {
        app.set_app_language(if en { AppLanguage::English } else { AppLanguage::TraditionalChinese });
        for right in [false, true] {
            arm_rectangle(&mut app);
            gui_frame(&mut app, &ctx, gui_pointer(start, true));
            gui_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start + egui::vec2(20., 30.))]);
            let cancel = if right { egui::Event::PointerButton { pos: start,
                button: egui::PointerButton::Secondary, pressed: true, modifiers: egui::Modifiers::NONE }
            } else { egui::Event::Key { key: egui::Key::Escape, physical_key: None,
                pressed: true, repeat: false, modifiers: egui::Modifiers::NONE } };
            gui_frame(&mut app, &ctx, vec![cancel]);
            gui_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start + egui::vec2(50., 50.))]);
            gui_frame(&mut app, &ctx, gui_pointer(start, false));
            let release = if right { egui::Event::PointerButton { pos: start,
                button: egui::PointerButton::Secondary, pressed: false, modifiers: egui::Modifiers::NONE }
            } else { egui::Event::Key { key: egui::Key::Escape, physical_key: None,
                pressed: false, repeat: false, modifiers: egui::Modifiers::NONE } };
            gui_frame(&mut app, &ctx, vec![release]);
            assert!(app.status_message.contains(if en { "cancelled" } else { "已取消" }));
            assert!(app.scenario.canvas.pick.is_none());
            assert_eq!(*app.scenario.document(), before);
            assert!(!app.scenario.can_undo_events());
        }
        for delta in [egui::vec2(0., 0.), egui::vec2(30., 0.), egui::vec2(0., 30.), egui::vec2(0.001, 0.001)] {
            arm_rectangle(&mut app);
            gui_frame(&mut app, &ctx, gui_pointer(start, true));
            gui_frame(&mut app, &ctx, gui_pointer(start + delta, false));
            assert!(app.status_message.contains(if en { "Zero-size" } else { "零大小" }));
            assert!(app.scenario.canvas.pick.is_none());
            assert_eq!(*app.scenario.document(), before);
            assert!(!app.scenario.can_undo_events());
        }
    }
    assert_eq!(app.session.as_ref().unwrap().heights(), heights);
    assert!(app.box_drag_start.is_none());
    arm_rectangle(&mut app);
    gui_frame(&mut app, &ctx, gui_pointer(start, true));
    let mut changed = before.events[0].clone();
    changed.name = "Changed during drag".into();
    app.scenario.replace_event(0, changed).unwrap();
    let after = app.scenario.document().clone();
    gui_frame(&mut app, &ctx, gui_pointer(start + egui::vec2(40., 50.), false));
    assert!(app.status_is_error && app.status_message.contains("目標已變更"));
    assert_eq!(*app.scenario.document(), after);
    assert!(app.scenario.undo_events());
    assert_eq!(*app.scenario.document(), before);
    assert!(!app.scenario.can_undo_events());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn scenario_canvas_rectangle_overlay_highlight_visibility_and_roundtrip() {
    let (mut app, root, ctx) = rectangle_fixture();
    app.scenario.open = true;
    let expected = Rect::from_two_pos(screen_from_world(app.last_map_rect, 2000., 3000.),
        screen_from_world(app.last_map_rect, 6000., 7000.));
    let out = overlay_frame(&app);
    assert!(out.shapes.iter().any(|s| matches!(&s.shape, egui::epaint::Shape::Rect(r)
        if r.rect == expected && r.fill.a() > 0 && r.fill.a() < 255)));
    assert!(out.shapes.iter().any(|s| matches!(&s.shape, egui::epaint::Shape::Rect(r)
        if r.rect == expected && r.stroke.color == egui::Color32::YELLOW && r.stroke.width == 3.)));
    assert!(gui_find(&out, "Canvas wave").is_some());
    assert!(gui_find(&out, "Disabled wave").is_none());
    app.scenario.selected = 1;
    assert!(overlay_frame(&app).shapes.iter().any(|s| matches!(&s.shape, egui::epaint::Shape::Rect(r)
        if r.rect == expected && r.stroke.color == egui::Color32::LIGHT_BLUE && r.stroke.width == 1.5)));
    app.scenario.canvas.show = false;
    assert!(overlay_frame(&app).shapes.is_empty());
    app.scenario.canvas.show = true;
    app.scenario.open = false;
    assert!(overlay_frame(&app).shapes.is_empty());
    arm_rectangle(&mut app);
    let start = screen_from_world(app.last_map_rect, 4200., 5200.);
    let end = screen_from_world(app.last_map_rect, 9400., 10400.);
    gui_frame(&mut app, &ctx, gui_pointer(start, true));
    gui_frame(&mut app, &ctx, gui_pointer(end, false));
    let doc = app.scenario.document().clone();
    let parsed = ScenarioDocument::from_json(&doc.to_json().unwrap()).unwrap();
    assert_eq!(parsed, doc);
    let dir = root.join("MAPS/ENDL_006");
    std::fs::write(dir.join("arm_scenario.json"), doc.to_json().unwrap()).unwrap();
    let mut reloaded = ScenarioUi::default();
    reloaded.load_session(&dir).unwrap();
    app.scenario = reloaded;
    app.scenario.open = true;
    let expected = Rect::from_two_pos(start, end);
    assert!(overlay_frame(&app).shapes.iter().any(|s| matches!(&s.shape, egui::epaint::Shape::Rect(r)
        if r.rect == expected && r.stroke.width > 0.)));
    assert_eq!(bounds(&app), (4200, 5200, 9400, 10400));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn scenario_canvas_rectangle_bilingual_draw_buttons_and_preview() {
    for en in [true, false] {
        let (mut app, root, ctx) = rectangle_fixture();
        app.set_app_language(if en { AppLanguage::English } else { AppLanguage::TraditionalChinese });
        gui_click(&mut app, &ctx, if en { "Scenario / Camera" } else { "劇本／鏡頭" });
        for _ in 0..20 { gui_frame(&mut app, &ctx, vec![]); }
        gui_click(&mut app, &ctx, if en { "Draw on map" } else { "在地圖上框選" });
        assert!(matches!(app.scenario.canvas.pick, Some(PickTarget::Condition { event: 0, condition: 0, .. })));
        app.scenario.open = false;
        gui_frame(&mut app, &ctx, vec![]);
        assert!(app.status_message.contains(if en { "Draw rectangle" } else { "框選矩形" }));
        let start = app.last_map_rect.center();
        gui_frame(&mut app, &ctx, gui_pointer(start, true));
        gui_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start + egui::vec2(30., 40.))]);
        app.scenario.open = true;
        assert!(gui_find(&overlay_frame(&app), if en { "Draw rectangle" } else { "框選矩形" }).is_some());
        app.scenario.canvas.show = false;
        assert!(overlay_frame(&app).shapes.is_empty());
        app.scenario.open = false;
        gui_frame(&mut app, &ctx, gui_pointer(start + egui::vec2(30., 40.), false));
        assert!(app.scenario.can_undo_events());
        std::fs::remove_dir_all(root).unwrap();
    }
}

fn fixture() -> (EditorApp, PathBuf, egui::Context) {
    let root = temp_map_dir("scenario_canvas");
    let dir = root.join("MAPS/ENDL_006");
    std::fs::create_dir_all(dir.join("SCRIPT")).unwrap();
    std::fs::write(dir.join("SCRIPT/ak_level.bci"), against_rome_scenario::BciImage::idle().serialize()).unwrap();
    let action = Action { kind: 2, alias: "unit".into(), team: 2, count: 3, x: 4000., z: 5000.,
        move_destination: Some(WorldPosition { x: 7000, z: 8000 }), ..Default::default() };
    let doc = ScenarioDocument { events: vec![
        Event { name: "Canvas wave".into(), actions: vec![action.clone()], ..Default::default() },
        Event { name: "Disabled wave".into(), enabled: false, actions: vec![action], ..Default::default() },
    ], ..Default::default() };
    std::fs::write(dir.join("arm_scenario.json"), doc.to_json().unwrap()).unwrap();
    let mut app = EditorApp::initial_state();
    app.set_loaded_map(dir.clone(), TerrainSession::create_blank(33, 32)).unwrap();
    app.scenario.load_session(&dir).unwrap();
    let ctx = egui::Context::default();
    for _ in 0..3 { gui_frame(&mut app, &ctx, vec![]); }
    (app, root, ctx)
}

fn arm_event(app: &mut EditorApp, destination: bool) {
    app.scenario.canvas.pick = Some(PickTarget::Event { event: 0, action: 0, destination,
        snapshot: app.scenario.document().events[0].clone() });
}

fn overlay_frame(app: &EditorApp) -> egui::FullOutput {
    let ctx = egui::Context::default();
    ctx.run(egui::RawInput::default(), |ctx| {
        let painter = ctx.layer_painter(egui::LayerId::background());
        app.scenario.paint_canvas(&painter, app.last_map_rect, app.scenario.english);
    })
}

#[test]
fn scenario_canvas_transform_and_json_roundtrip() {
    for rect in [Rect::from_min_size(Pos2::new(319., -123.), egui::vec2(927., 927.)),
        Rect::from_min_size(Pos2::new(-400., 220.), egui::vec2(2500., 2500.))] {
        for (x, z) in [(0., 0.), (16383., 16383.), (5123.25, 7359.75)] {
            let world = world_from_screen(rect, screen_from_world(rect, x, z));
            assert!((world.0 - x).abs() < 0.01 && (world.1 - z).abs() < 0.01);
        }
    }
    let (mut app, root, ctx) = fixture();
    app.zoom = 1.3;
    app.pan = egui::vec2(-67., 24.);
    gui_frame(&mut app, &ctx, vec![]);
    arm_event(&mut app, true);
    let pos = screen_from_world(app.last_map_rect, 8123.6, 6543.3);
    gui_frame(&mut app, &ctx, gui_pointer(pos, true));
    gui_frame(&mut app, &ctx, gui_pointer(pos, false));
    let doc = app.scenario.document().clone();
    assert_eq!(doc.events[0].actions[0].move_destination, Some(WorldPosition { x: 8124, z: 6543 }));
    let roundtrip = ScenarioDocument::from_json(&doc.to_json().unwrap()).unwrap();
    assert_eq!(doc, roundtrip);
    let dir = root.join("MAPS/ENDL_006");
    std::fs::write(dir.join("arm_scenario.json"), doc.to_json().unwrap()).unwrap();
    let mut reloaded = ScenarioUi::default();
    reloaded.load_session(&dir).unwrap();
    assert_eq!(reloaded.document(), &doc);
    assert!(app.scenario.undo_events());
    assert!(!app.scenario.can_undo_events());
    assert!(app.scenario.redo_events());
    assert_eq!(*app.scenario.document(), doc);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn scenario_canvas_pointer_consumes_press_hold_release_and_one_undo() {
    let (mut app, root, ctx) = fixture();
    let before = app.scenario.document().clone();
    let heights = app.session.as_ref().unwrap().heights().to_vec();
    let collision = app.session.as_ref().unwrap().collision().to_vec();
    let disk = save_artifact_snapshot(&root.join("MAPS/ENDL_006"));
    app.active_tool = ActiveTool::HeightRaise;
    app.zoom = 0.65;
    app.pan = egui::vec2(25., -15.);
    gui_frame(&mut app, &ctx, vec![]);
    arm_event(&mut app, false);
    let pos = screen_from_world(app.last_map_rect, 9345.6, 4321.2);
    gui_frame(&mut app, &ctx, gui_pointer(pos, true));
    gui_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pos + egui::vec2(25., 20.))]);
    gui_frame(&mut app, &ctx, gui_pointer(pos + egui::vec2(25., 20.), false));
    assert_eq!((app.scenario.document().events[0].actions[0].x,
        app.scenario.document().events[0].actions[0].z), (9346., 4321.));
    assert_eq!(app.session.as_ref().unwrap().heights(), heights);
    assert_eq!(app.session.as_ref().unwrap().collision(), collision);
    assert!(!app.is_painting && !app.spawn_ui.is_capturing());
    assert!(app.status_message.contains("Event updated"));
    assert!(app.scenario.undo_events());
    assert_eq!(*app.scenario.document(), before);
    assert!(!app.scenario.can_undo_events());
    assert_eq!(save_artifact_snapshot(&root.join("MAPS/ENDL_006")), disk);

    app.box_select = true;
    arm_event(&mut app, true);
    gui_frame(&mut app, &ctx, gui_pointer(pos, true));
    gui_frame(&mut app, &ctx, gui_pointer(pos, false));
    assert!(app.box_select && app.box_drag_start.is_none());
    assert!(app.scenario.undo_events());
    assert!(!app.scenario.can_undo_events());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn scenario_canvas_cancel_and_clamp_with_real_pointer() {
    let (mut app, root, ctx) = fixture();
    let before = app.scenario.document().clone();
    let heights = app.session.as_ref().unwrap().heights().to_vec();
    let pos = app.last_map_rect.center();
    for en in [true, false] {
        app.set_app_language(if en { AppLanguage::English } else { AppLanguage::TraditionalChinese });
        for right in [false, true] {
            arm_event(&mut app, true);
            gui_frame(&mut app, &ctx, vec![]);
            assert!(app.status_message.contains(if en { "Pick destination" } else { "選取目的地" }));
            let events = if right {
                vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                    pos, button: egui::PointerButton::Secondary, pressed: true, modifiers: egui::Modifiers::NONE }]
            } else { vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None,
                pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }] };
            gui_frame(&mut app, &ctx, events);
            if right {
                gui_frame(&mut app, &ctx, vec![egui::Event::PointerButton {
                    pos, button: egui::PointerButton::Secondary, pressed: false, modifiers: egui::Modifiers::NONE }]);
            } else {
                gui_frame(&mut app, &ctx, vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None,
                    pressed: false, repeat: false, modifiers: egui::Modifiers::NONE }]);
            }
            assert!(app.status_message.contains(if en { "cancelled" } else { "已取消" }));
            assert!(app.scenario.canvas.pick.is_none());
            assert_eq!(*app.scenario.document(), before);
            assert!(!app.scenario.can_undo_events());
        }
    }
    app.zoom = 0.4;
    gui_frame(&mut app, &ctx, vec![]);
    arm_event(&mut app, true);
    let outside = app.last_map_rect.max + egui::vec2(10., 10.);
    assert!(app.last_canvas_rect.contains(outside));
    gui_frame(&mut app, &ctx, gui_pointer(outside, true));
    gui_frame(&mut app, &ctx, gui_pointer(outside, false));
    assert_eq!(app.scenario.document().events[0].actions[0].move_destination,
        Some(WorldPosition { x: 16383, z: 16383 }));
    assert!(app.scenario.undo_events());
    arm_event(&mut app, false);
    let outside = app.last_map_rect.min - egui::vec2(10., 10.);
    gui_frame(&mut app, &ctx, gui_pointer(outside, true));
    gui_frame(&mut app, &ctx, gui_pointer(outside, false));
    let a = &app.scenario.document().events[0].actions[0];
    assert_eq!((a.x, a.z), (0., 0.));
    assert_eq!(app.session.as_ref().unwrap().heights(), heights);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn scenario_canvas_overlay_and_wave_form_buttons() {
    let (mut app, root, ctx) = fixture();
    assert!(app.scenario.canvas.show);
    assert!(!app.scenario.open);
    assert!(overlay_frame(&app).shapes.is_empty());
    gui_click(&mut app, &ctx, "Scenario / Camera");
    let out = overlay_frame(&app);
    assert!(gui_find(&out, "Canvas wave").is_some());
    assert!(gui_find(&out, "Disabled wave").is_none());
    let start = screen_from_world(app.last_map_rect, 4000., 5000.);
    let end = screen_from_world(app.last_map_rect, 7000., 8000.);
    let color = crate::spawn_placement_ui::team_color(2);
    assert_eq!(out.shapes.iter().filter(|s| matches!(&s.shape,
        egui::epaint::Shape::Circle(c) if c.center == start && c.fill == color)).count(), 1);
    assert!(out.shapes.iter().any(|s| matches!(&s.shape,
        egui::epaint::Shape::LineSegment { points, stroke } if *points == [start, end] && stroke.color == color)));
    gui_click(&mut app, &ctx, "Show scenario overlay");
    let out = overlay_frame(&app);
    assert!(gui_find(&out, "Canvas wave").is_none());
    gui_click(&mut app, &ctx, "Show scenario overlay");
    app.scenario.open = false;
    assert!(overlay_frame(&app).shapes.is_empty());
    gui_click(&mut app, &ctx, "Scenario / Camera");
    assert!(gui_find(&overlay_frame(&app), "Canvas wave").is_some());
    gui_click(&mut app, &ctx, "Attack wave generator");
    for _ in 0..20 { gui_frame(&mut app, &ctx, vec![]); }
    let before = app.scenario.document().clone();
    for destination in [false, true] {
        gui_click(&mut app, &ctx, if destination { "Pick on map (destination)" } else { "Pick on map (spawn)" });
        assert!(matches!(app.scenario.canvas.pick, Some(PickTarget::Wave { destination: d }) if d == destination));
        app.scenario.open = false;
        assert!(overlay_frame(&app).shapes.is_empty());
        gui_frame(&mut app, &ctx, vec![]);
        let pos = screen_from_world(app.last_map_rect, 9123.6, 6543.2);
        gui_frame(&mut app, &ctx, gui_pointer(pos, true));
        gui_frame(&mut app, &ctx, gui_pointer(pos, false));
        let g = &app.scenario.wave_generator;
        if destination { assert_eq!(g.destination, WorldPosition { x: 9124, z: 6543 }); }
        else { assert_eq!((g.x, g.z), (9124., 6543.)); }
        assert_eq!(*app.scenario.document(), before);
        assert!(!app.scenario.can_undo_events());
        assert!(app.status_message.contains("Wave form updated"));
        app.scenario.open = true;
        gui_frame(&mut app, &ctx, vec![]);
    }
    let out = overlay_frame(&app);
    assert!(gui_find(&out, "Pending wave").is_some());
    assert_eq!(app.scenario.wave_generator.x, 9124.);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn scenario_canvas_chinese_overlay_and_pick_buttons() {
    for wave in [false, true] {
        let (mut app, root, ctx) = fixture();
        app.set_app_language(AppLanguage::TraditionalChinese);
        gui_click(&mut app, &ctx, "顯示劇本標記");
        assert!(!app.scenario.canvas.show);
        gui_click(&mut app, &ctx, "顯示劇本標記");
        assert!(app.scenario.canvas.show);
        gui_click(&mut app, &ctx, "劇本／鏡頭");
        if wave { gui_click(&mut app, &ctx, "進攻波次產生器"); }
        for _ in 0..20 { gui_frame(&mut app, &ctx, vec![]); }
        for destination in [false, true] {
            gui_click(&mut app, &ctx, if destination {
                "在地圖上點選（目的地）"
            } else { "在地圖上點選（生成點）" });
            match &app.scenario.canvas.pick {
                Some(PickTarget::Wave { destination: d }) if wave => assert_eq!(*d, destination),
                Some(PickTarget::Event { event: 0, action: 0, destination: d, .. }) if !wave => {
                    assert_eq!(*d, destination);
                }
                _ => panic!("取點按鈕未選取預期的 X/Z 目標"),
            }
            app.scenario.open = false;
            gui_frame(&mut app, &ctx, vec![]);
            let pos = screen_from_world(app.last_map_rect, 9000., 10000.);
            gui_frame(&mut app, &ctx, gui_pointer(pos, true));
            gui_frame(&mut app, &ctx, gui_pointer(pos, false));
            if wave {
                let g = &app.scenario.wave_generator;
                if destination { assert_eq!(g.destination, WorldPosition { x: 9000, z: 10000 }); }
                else { assert_eq!((g.x, g.z), (9000., 10000.)); }
            } else {
                let a = &app.scenario.document().events[0].actions[0];
                if destination { assert_eq!(a.move_destination, Some(WorldPosition { x: 9000, z: 10000 })); }
                else { assert_eq!((a.x, a.z), (9000., 10000.)); }
            }
            app.scenario.open = true;
            gui_frame(&mut app, &ctx, vec![]);
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn scenario_canvas_event_buttons_and_stale_target() {
    let (mut app, root, ctx) = fixture();
    let before = app.scenario.document().clone();
    gui_click(&mut app, &ctx, "Scenario / Camera");
    for _ in 0..20 { gui_frame(&mut app, &ctx, vec![]); }
    for destination in [false, true] {
        gui_click(&mut app, &ctx, if destination { "Pick on map (destination)" } else { "Pick on map (spawn)" });
        assert!(matches!(app.scenario.canvas.pick,
            Some(PickTarget::Event { event: 0, action: 0, destination: d, .. }) if d == destination));
        app.scenario.open = false;
        gui_frame(&mut app, &ctx, vec![]);
        let pos = screen_from_world(app.last_map_rect, 10000.3, 12000.8);
        gui_frame(&mut app, &ctx, gui_pointer(pos, true));
        gui_frame(&mut app, &ctx, gui_pointer(pos, false));
        let a = &app.scenario.document().events[0].actions[0];
        if destination { assert_eq!(a.move_destination, Some(WorldPosition { x: 10000, z: 12001 })); }
        else { assert_eq!((a.x, a.z), (10000., 12001.)); }
        assert!(app.scenario.undo_events());
        assert_eq!(*app.scenario.document(), before);
        assert!(!app.scenario.can_undo_events());
        app.scenario.open = true;
        gui_frame(&mut app, &ctx, vec![]);
    }
    app.scenario.open = false;
    gui_frame(&mut app, &ctx, vec![]);
    arm_event(&mut app, true);
    app.scenario.remove_event(0).unwrap();
    let after_delete = app.scenario.document().clone();
    let pos = app.last_map_rect.center();
    gui_frame(&mut app, &ctx, gui_pointer(pos, true));
    gui_frame(&mut app, &ctx, gui_pointer(pos, false));
    assert!(app.status_is_error && app.status_message.contains("target changed"));
    assert_eq!(*app.scenario.document(), after_delete);
    assert!(app.scenario.undo_events());
    assert_eq!(*app.scenario.document(), before);
    assert!(!app.scenario.can_undo_events());
    std::fs::remove_dir_all(root).unwrap();
}
