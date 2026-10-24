use super::*;
use crate::save_blockers::{
    SaveBlockerAction as Action, SaveBlockerKind as Kind, SavePanel as Panel,
};

#[test]
fn save_blockers_all_file_targets_are_checked_before_save_now() {
    let (mut app, dir) = app_with_target("blocker_target_directories");
    for name in ["boden.bmp", "collision.bmp", "boden.ini"] {
        std::fs::create_dir(dir.join(name)).unwrap();
    }
    app.ambience.mark_dirty();
    let blockers = app.save_blockers();
    for name in ["boden.bmp", "collision.bmp", "boden.ini"] {
        assert!(
            blockers
                .iter()
                .any(|b| b.kind == Kind::Target && b.english.contains(name)),
            "{name}: {blockers:?}"
        );
    }
    app.save_blockers_open = true;
    let before = save_file_snapshot(&dir);
    app.save_now_from_blockers();
    assert!(app.save_blockers_open);
    assert_eq!(save_file_snapshot(&dir), before);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_blockers_embedded_graph_has_apply_and_confirmed_discard() {
    let (mut app, dir) = app_with_target("blocker_embedded_graph");
    app.scenario.load_session(&dir).unwrap();
    app.scenario.graph_ui_mut().mark_dirty();
    let blocker = app
        .save_blockers()
        .into_iter()
        .find(|b| b.panel == Panel::ScenarioGraph)
        .unwrap();
    assert!(blocker.actions.contains(&Action::Apply) && blocker.actions.contains(&Action::Discard));
    let ctx = egui::Context::default();
    app.resolve_save_blocker(Panel::ScenarioGraph, Action::Discard, &ctx);
    assert!(app.scenario.graph_ui().is_dirty() && app.scenario.graph_ui().pending_close);
    app.scenario.graph_ui_mut().cancel_close();
    app.resolve_save_blocker(Panel::ScenarioGraph, Action::Apply, &ctx);
    assert!(
        !app.scenario.graph_ui().is_dirty(),
        "{}",
        app.status_message
    );
    assert!(!app
        .save_blockers()
        .iter()
        .any(|b| b.panel == Panel::ScenarioGraph));
    std::fs::remove_dir_all(dir).unwrap();
}

fn app_with_target(tag: &str) -> (EditorApp, PathBuf) {
    let dir = temp_map_dir(tag);
    let mut app = EditorApp::initial_state();
    app.session = Some(TerrainSession::create_blank(5, 32));
    app.map_path_input = dir.display().to_string();
    (app, dir)
}

#[test]
fn save_blockers_map_errors_do_not_hide_scenario_preflight_errors() {
    let (mut app, dir) = app_with_target("blocker_parallel_diagnostics");
    app.session = Some(TerrainSession::create_blank(1026, 32));
    let document = against_rome_scenario::ScenarioDocument {
        spawns: vec![against_rome_scenario::Spawn {
            id: uuid::Uuid::new_v4(),
            alias: "UNKNOWN_PENDING_ALIAS".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    std::fs::write(
        dir.join("arm_scenario.json"),
        serde_json::to_vec(&document).unwrap(),
    )
    .unwrap();
    let blockers = app.save_blockers();
    assert!(blockers
        .iter()
        .any(|b| b.english.contains("collision-limit")));
    assert!(
        blockers
            .iter()
            .any(|b| b.english.contains("ScenarioSavePreflight")),
        "{blockers:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_blockers_multiple_include_design_errors_and_preserve_edits() {
    let (mut app, dir) = app_with_target("blockers_multiple");
    app.maps.mark_dirty();
    app.environment.mark_dirty();
    app.session = Some(TerrainSession::create_blank(1026, 32));
    let before = save_file_snapshot(&dir);
    let blockers = app.save_blockers();
    assert!(blockers.iter().any(|b| b.panel == Panel::Maps));
    assert!(blockers.iter().any(|b| b.panel == Panel::Environment));
    assert!(blockers
        .iter()
        .any(|b| b.kind == Kind::DesignError && b.english.contains("collision-limit")));
    assert!(blockers
        .iter()
        .all(|b| !b.english.is_empty() && !b.chinese.is_empty()));
    app.safe_save_current_using(|_| panic!("阻擋尚未解除不可提交"));
    assert!(app.save_blockers_open);
    assert!(app.maps.is_dirty() && app.environment.is_dirty());
    assert_eq!(save_file_snapshot(&dir), before);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_blockers_previous_rejections_table() {
    let cases: &[(Panel, fn(&mut EditorApp, &Path), &str)] = &[
        (
            Panel::EventGraph,
            |a, _| a.event_graph.mark_dirty(),
            "EventGraph",
        ),
        (Panel::Maps, |a, _| a.maps.mark_dirty(), "Maps"),
        (
            Panel::Environment,
            |a, _| a.environment.mark_dirty(),
            "Environment",
        ),
        (
            Panel::Landscape,
            |a, _| a.landscape.mark_dirty(),
            "Landscape",
        ),
        (
            Panel::Packaging,
            |a, _| a.packaging.mark_dirty(),
            "Packaging",
        ),
        (Panel::Campaign, |a, _| a.campaign.mark_dirty(), "Campaign"),
        (
            Panel::AiPlanner,
            |a, _| a.ai_planner.mark_dirty(),
            "AI Planner",
        ),
        (
            Panel::Toolbox,
            |a, p| pending_panel_edit(a, 4, p),
            "Toolbox",
        ),
        (
            Panel::Terrain,
            |a, _| a.session = None,
            "No terrain session",
        ),
        (
            Panel::Maps,
            |a, _| a.map_path_input.clear(),
            "Please specify target",
        ),
        (
            Panel::Ambience,
            |a, _| {
                a.ambience.profile.boden.waterlevel = f32::NAN;
                a.ambience.mark_dirty();
            },
            "Ambience",
        ),
        (
            Panel::Ambience,
            |a, _| {
                a.ambience.profile.preset_id = "preview".into();
                a.ambience.mark_dirty();
            },
            "unsupported dirty profile fields",
        ),
    ];
    for (panel, edit, message) in cases {
        let (mut app, dir) = app_with_target("blockers_table");
        edit(&mut app, &dir);
        let before = save_file_snapshot(&dir);
        assert!(
            app.save_blockers()
                .iter()
                .any(|b| b.panel == *panel && b.english.contains(message)),
            "{panel:?}"
        );
        let ctx = egui::Context::default();
        for blocker in app.save_blockers() {
            assert!(!blocker.actions.is_empty() || (!blocker.english.is_empty() && !blocker.chinese.is_empty()));
            assert!(blocker.actions.contains(&Action::OpenPanel), "{blocker:?}");
            app.resolve_save_blocker(blocker.panel, Action::OpenPanel, &ctx);
        }
        match panel {
            Panel::Ambience => assert!(app.ambience.open),
            Panel::AiPlanner => assert!(app.ai_planner.open),
            Panel::Packaging => assert!(app.packaging.open),
            _ => (),
        }
        app.safe_save_current_using(|_| panic!("既有拒絕條件不可放寬"));
        assert!(app.status_is_error, "{panel:?}");
        assert!(
            app.status_message.contains(message),
            "{}",
            app.status_message
        );
        assert_eq!(save_file_snapshot(&dir), before);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn save_blockers_open_panels_and_focus_existing_windows() {
    let (mut app, dir) = app_with_target("blockers_open");
    let ctx = egui::Context::default();
    for panel in [
        Panel::Spawns,
        Panel::Nature,
        Panel::ScenarioGraph,
        Panel::EventGraph,
        Panel::Maps,
        Panel::Environment,
        Panel::Landscape,
        Panel::Packaging,
        Panel::Campaign,
        Panel::AiPlanner,
        Panel::Toolbox,
        Panel::Ambience,
        Panel::Objects,
        Panel::Scenario,
        Panel::Materials,
        Panel::Scene,
        Panel::DesignCheck,
    ] {
        app.open_save_panel(panel, &ctx);
    }
    assert!(app.spawn_ui.open && app.save_focus_nature && app.scenario.graph_ui().open);
    assert!(app.event_graph.open && app.maps.open && app.environment.open && app.landscape.open);
    assert!(app.packaging.open && app.campaign.open && app.ai_planner.open && app.toolbox.open);
    assert!(
        app.ambience.open
            && app.placement.open
            && app.scenario.open
            && app.textures.open
            && app.scene.open
            && app.design_check.open
    );
    app.maps.mark_dirty();
    let _ = ctx.run(Default::default(), |ctx| app.show_studio_panels(ctx));
    let _ = ctx.run(Default::default(), |ctx| {
        app.show_studio_panels(ctx);
        app.open_save_panel(Panel::Maps, ctx);
    });
    assert_eq!(
        ctx.memory(|m| m.areas().top_layer_id(egui::Order::Middle)),
        Some(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("Maps & Diagnostics Studio")
        ))
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_blockers_apply_uses_graph_validation_and_recomputes() {
    let (mut app, dir, _) = start_camera_app(false);
    std::fs::write(
        dir.join("arm_scenario.json"),
        against_rome_scenario::model::ScenarioDocument::default()
            .to_json()
            .unwrap(),
    )
    .unwrap();
    app.scenario.load_session(&dir).unwrap();
    app.open_toolbar_event_graph();
    app.event_graph.mark_dirty();
    let ctx = egui::Context::default();
    app.resolve_save_blocker(Panel::EventGraph, Action::Apply, &ctx);
    assert!(!app.event_graph.is_dirty(), "{}", app.status_message);
    assert!(!app
        .save_blockers()
        .iter()
        .any(|b| b.panel == Panel::EventGraph));
    // 來源衝突仍由原函式拒絕，保留草稿。
    app.event_graph.mark_dirty();
    app.scenario
        .add_events(&[against_rome_scenario::Event {
            name: "conflict".into(),
            actions: vec![against_rome_scenario::Action {
                text: "conflict".into(),
                ..Default::default()
            }],
            ..Default::default()
        }])
        .unwrap();
    app.resolve_save_blocker(Panel::EventGraph, Action::Apply, &ctx);
    assert!(app.event_graph.is_dirty());
    assert!(app.status_is_error);
    assert!(app
        .save_blockers()
        .iter()
        .any(|b| b.panel == Panel::EventGraph));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_blockers_discard_requires_existing_confirmation_and_can_cancel() {
    let (mut app, dir) = app_with_target("blockers_discard");
    let ctx = egui::Context::default();
    app.event_graph.mark_dirty();
    app.resolve_save_blocker(Panel::EventGraph, Action::Discard, &ctx);
    assert!(app.event_graph.is_dirty() && app.event_graph.pending_close);
    app.event_graph.cancel_close();
    assert!(app.event_graph.is_dirty());
    app.resolve_save_blocker(Panel::EventGraph, Action::Discard, &ctx);
    app.event_graph.discard_and_close();
    assert!(!app
        .save_blockers()
        .iter()
        .any(|b| b.panel == Panel::EventGraph));
    app.environment.mark_dirty();
    app.resolve_save_blocker(Panel::Environment, Action::Discard, &ctx);
    assert!(app.environment.pending_discard && app.environment.is_dirty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_blockers_save_now_rechecks_then_uses_original_transaction() {
    let (mut app, dir) = app_with_target("blockers_save_now");
    app.maps.mark_dirty();
    app.save_blockers_open = true;
    app.save_now_from_blockers();
    assert!(app.save_blockers_open);
    assert!(!dir.join("boden.bmp").exists());
    app.maps.reset_session(None);
    assert!(app.save_blockers().is_empty(), "{:?}", app.save_blockers());
    app.save_now_from_blockers();
    if app.pending_save_warnings.is_some() {
        app.resolve_safe_save_warnings(true);
    }
    assert!(!app.status_is_error, "{}", app.status_message);
    assert!(!app.save_blockers_open);
    assert!(dir.join("boden.bmp").exists() && dir.join("collision.bmp").exists());
    assert!(load_map(&dir).is_ok());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_blockers_preview_keeps_spawn_history_selection_and_artifacts() {
    let (mut app, root, _) = spawn_save_tests::fixture("blocker_preview", &[]);
    spawn_save_tests::place_house(&mut app, (30, 30), 0);
    app.maps.mark_dirty();
    let token = app.spawn_ui.history_token();
    let selection = app.spawn_ui.selection();
    let snapshot = app.spawn_ui.session().capture();
    let before = save_file_snapshot(&root);
    for _ in 0..2 {
        assert!(!app.save_blockers().is_empty());
    }
    assert_eq!(app.spawn_ui.history_token(), token);
    assert_eq!(app.spawn_ui.selection(), selection);
    assert_eq!(app.spawn_ui.session().capture(), snapshot);
    assert_eq!(save_file_snapshot(&root), before);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn save_blockers_data_conflicts_are_reported_together() {
    let (mut app, root, map) = spawn_save_tests::fixture("blocker_data_conflicts", &[(232, None)]);
    app.placement.load_session(&map).unwrap();
    app.placement
        .store_mut()
        .unwrap()
        .set_team(0, 1, 3)
        .unwrap();
    spawn_save_tests::place_house(&mut app, (30, 30), 0);
    app.campaign.start_camera.enabled = true;
    app.apply_start_camera_candidate().unwrap();
    let before = save_file_snapshot(&root);
    let blockers = app.save_blockers();
    let ctx = egui::Context::default();
    for blocker in blockers.iter().filter(|b| b.kind == Kind::DataConflict) {
        assert!(blocker.actions.contains(&Action::OpenPanel));
        app.resolve_save_blocker(blocker.panel, Action::OpenPanel, &ctx);
    }
    assert!(app.placement.open && app.campaign.open);
    assert!(blockers
        .iter()
        .any(|b| b.kind == Kind::DataConflict && b.panel == Panel::Objects));
    assert!(blockers
        .iter()
        .any(|b| b.kind == Kind::DataConflict && b.panel == Panel::Campaign));
    app.safe_save_current_using(|_| panic!("兩項 DATA 衝突均不可略過"));
    assert_eq!(save_file_snapshot(&root), before);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn save_blockers_plan_apply_keeps_existing_validation_and_undo() {
    let (mut app, dir) = app_with_target("blocker_plan_apply");
    let ctx = egui::Context::default();
    app.ai_planner.current_plan = Some(against_rome_ai_planner::AiMapPlan {
        base_height: Some(55),
        ..Default::default()
    });
    app.ai_planner.mark_dirty();
    app.resolve_save_blocker(Panel::AiPlanner, Action::ApplyAiPlan, &ctx);
    assert!(app.ai_planner.current_plan.is_none());
    assert!(app.session.as_ref().unwrap().is_dirty() && app.session.as_ref().unwrap().can_undo());
    assert!(!app
        .save_blockers()
        .iter()
        .any(|b| b.panel == Panel::AiPlanner));
    app.ai_planner.current_plan = Some(against_rome_ai_planner::AiMapPlan {
        base_material: Some("missing".into()),
        ..Default::default()
    });
    app.ai_planner.mark_dirty();
    let heights = app.session.as_ref().unwrap().heights().to_vec();
    app.resolve_save_blocker(Panel::AiPlanner, Action::ApplyAiPlan, &ctx);
    assert!(app.ai_planner.current_plan.is_some() && app.ai_planner.status_is_error);
    assert_eq!(app.session.as_ref().unwrap().heights(), heights);
    app.ai_planner.reset_session();
    app.session = Some(TerrainSession::create_blank(257, 32));
    let plan = erosion_plan_for(&mut app, 0, 32, 91);
    app.landscape.last_erosion_plan = Some(plan);
    app.resolve_save_blocker(Panel::Landscape, Action::ApplyErosion, &ctx);
    assert_eq!(app.session.as_ref().unwrap().heights()[0], 91);
    assert!(app.session.as_ref().unwrap().can_undo());
    assert!(!app
        .save_blockers()
        .iter()
        .any(|b| b.panel == Panel::Landscape));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_blockers_dialog_is_bilingual_and_buttons_recompute() {
    fn frame(
        app: &mut EditorApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280.0, 800.0))),
                events,
                ..Default::default()
            },
            |ctx| app.show_save_blockers(ctx),
        )
    }
    fn click(app: &mut EditorApp, ctx: &egui::Context, label: &str) {
        frame(app, ctx, vec![]);
        let output = frame(app, ctx, vec![]);
        let pos = gui_find(&output, label)
            .unwrap_or_else(|| panic!("找不到按鈕 {label}"))
            .center();
        frame(app, ctx, gui_pointer(pos, true));
        frame(app, ctx, gui_pointer(pos, false));
    }
    let (mut app, dir) = app_with_target("blocker_dialog");
    app.set_app_language(AppLanguage::English);
    app.environment.mark_dirty();
    app.save_blockers_open = true;
    let ctx = egui::Context::default();
    frame(&mut app, &ctx, vec![]);
    let output = frame(&mut app, &ctx, vec![]);
    assert!(gui_find(&output, "Cannot save yet").is_some());
    click(&mut app, &ctx, "Open panel");
    assert!(app.environment.open);
    click(&mut app, &ctx, "Discard…");
    assert!(app.environment.pending_discard && app.environment.is_dirty());
    app.environment.reset_session();
    app.set_app_language(AppLanguage::TraditionalChinese);
    frame(&mut app, &ctx, vec![]);
    let output = frame(&mut app, &ctx, vec![]);
    assert!(gui_find(&output, "尚無法儲存").is_some());
    assert!(gui_find(&output, "所有阻擋均已解決。").is_some());
    click(&mut app, &ctx, "立即儲存");
    if app.pending_save_warnings.is_some() {
        app.resolve_safe_save_warnings(true);
    }
    assert!(dir.join("boden.bmp").exists(), "{}", app.status_message);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_blockers_weather_discard_keeps_confirmation() {
    let (mut app, dir) = app_with_target("blocker_weather_discard");
    app.ambience.profile.preset_id = "preview".into();
    app.ambience.mark_dirty();
    let blocker = app.save_blockers().into_iter().find(|b| b.panel == Panel::Ambience && b.english.contains("unsupported")).unwrap();
    assert!(blocker.actions.contains(&Action::OpenPanel));
    assert!(blocker.actions.contains(&Action::Discard));
    app.resolve_save_blocker(Panel::Ambience, Action::Discard, &egui::Context::default());
    assert!(app.ambience.open && app.ambience.pending_discard && app.ambience.is_dirty());
    app.ambience.pending_discard = false;
    assert!(app.ambience.is_dirty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_blockers_nature_data_conflict_opens_nature() {
    let (mut app, root) = nature_app("blocker_nature_route");
    app.nature_set_enabled(true);
    app.nature_select_type(i32::from(NATURE_TREE));
    app.nature.density = NatureDensity::Dense;
    assert!(nature_stroke(&mut app, &[(10, 10)]).planted > 0);
    let path = PathBuf::from(&app.map_path_input);
    let blockers = app.save_data_blockers(&path, true);
    let blocker = blockers.iter().find(|b| b.panel == Panel::Nature).unwrap();
    assert!(blocker.actions.contains(&Action::OpenPanel));
    app.open_save_panel(blocker.panel, &egui::Context::default());
    assert!(app.save_focus_nature);
    std::fs::remove_dir_all(root).unwrap();
}
