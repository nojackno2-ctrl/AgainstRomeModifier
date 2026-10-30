use super::*;
use crate::undo_journal::{SubsystemId, UndoTarget};
use against_rome_scenario::{Action, BciImage, Event, ScenarioDocument};
use uuid::Uuid;

fn fixture() -> (EditorApp, PathBuf) {
    let game_root = temp_map_dir("design_check_game");
    let root = game_root.join("MAPS/design_check_focus");
    let clak = game_root.join("SYSTEM/CLAK");
    std::fs::create_dir_all(&clak).unwrap();
    std::fs::write(
        clak.join("cl_scint.ini"),
        "[ObjDefName]\nwolf=FigGerInf02_Zweihandaxt\nbear=FigGerInf02_Zweihandaxt\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("SCRIPT")).unwrap();
    std::fs::write(
        root.join("SCRIPT/ak_level.bci"),
        BciImage::idle().serialize(),
    )
    .unwrap();
    let document = ScenarioDocument {
        events: (0..3)
            .map(|i| Event {
                name: format!("定位事件 {i}"),
                actions: vec![Action {
                    kind: 3,
                    ..Default::default()
                }],
                ..Default::default()
            })
            .collect(),
        spawns: vec![
            against_rome_scenario::Spawn {
                id: Uuid::from_u128(0x1000_2000),
                alias: "wolf".into(),
                x: 1024.0,
                y: 10.0,
                z: 2048.0,
                count: 1,
                ..Default::default()
            },
            against_rome_scenario::Spawn {
                id: Uuid::from_u128(0x3000_4000),
                alias: "bear".into(),
                x: 4096.0,
                y: 15.0,
                z: 8192.0,
                count: 1,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    std::fs::write(root.join("arm_scenario.json"), document.to_json().unwrap()).unwrap();
    let mut app = EditorApp::initial_state();
    app.set_loaded_map(root.clone(), TerrainSession::create_blank(33, 32))
        .unwrap();
    app.scenario.load_session(&root).unwrap();
    app.last_canvas_rect = Rect::from_min_size(Pos2::new(30.0, 70.0), Vec2::new(800.0, 600.0));
    app.zoom = 2.0;
    (app, game_root)
}

/// 定位只能修改檢視與選取，不得改寫地圖內容、髒狀態或歷史。
fn assert_navigation_only(app: &mut EditorApp, navigate: impl FnOnce(&mut EditorApp)) {
    app.collect_pilot_receipts();
    let document = app.scenario.document().clone();
    let heights = app.session.as_ref().unwrap().heights().to_vec();
    let collision = app.session.as_ref().unwrap().collision().to_vec();
    let dirty = (app.has_unsaved_changes(), pending_panel_dirty_flags(app));
    let journal = app.undo_journal.clone();
    let tokens = [
        app.token(SubsystemId::Terrain),
        app.token(SubsystemId::Scenario),
    ];
    let history = (
        app.session.as_ref().unwrap().can_undo(),
        app.session.as_ref().unwrap().can_redo(),
        app.scenario.can_undo_events(),
        app.scenario.can_redo_events(),
    );
    navigate(app);
    app.collect_pilot_receipts();
    assert_eq!(*app.scenario.document(), document);
    assert_eq!(app.session.as_ref().unwrap().heights(), heights);
    assert_eq!(app.session.as_ref().unwrap().collision(), collision);
    assert_eq!(
        (app.has_unsaved_changes(), pending_panel_dirty_flags(app)),
        dirty
    );
    assert_eq!(app.undo_journal, journal);
    assert_eq!(
        [
            app.token(SubsystemId::Terrain),
            app.token(SubsystemId::Scenario)
        ],
        tokens
    );
    assert_eq!(
        (
            app.session.as_ref().unwrap().can_undo(),
            app.session.as_ref().unwrap().can_redo(),
            app.scenario.can_undo_events(),
            app.scenario.can_redo_events(),
        ),
        history
    );
}

#[test]
fn design_check_focus_3d_targets_world_position_and_consumes_requests_once() {
    let (mut app, root) = fixture();
    app.view_mode = ViewMode::View3D;
    app.pan = Vec2::new(17.0, -23.0);
    let pan = app.pan;
    let height = app.viewport_3d.camera.target.y;
    assert_navigation_only(&mut app, |app| {
        app.design_check.focus_request = Some((2048.0, 6144.0));
        app.design_check.event_request = Some(2);
        app.consume_design_check_focus();
        assert_eq!(app.viewport_3d.camera.target.x, 8.0);
        assert_eq!(app.viewport_3d.camera.target.z, 24.0);
        assert_eq!(app.viewport_3d.camera.target.y, height);
        assert_eq!(app.pan, pan);
        assert_eq!(app.scenario.selected, 2);
        assert!(app.scenario.open);
        assert!(app.design_check.focus_request.is_none());
        assert!(app.design_check.event_request.is_none());

        // 消耗後即使使用者改變檢視與選取，下次也不得重播請求。
        app.viewport_3d.center_on_world(256.0, 512.0);
        app.scenario.selected = 1;
        app.scenario.open = false;
        app.consume_design_check_focus();
        assert_eq!(app.viewport_3d.camera.target.x, 1.0);
        assert_eq!(app.viewport_3d.camera.target.z, 2.0);
        assert_eq!(app.scenario.selected, 1);
        assert!(!app.scenario.open);
    });
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn design_check_focus_2d_preserves_dimension_zoom_and_canvas_pan_calculation() {
    let (mut app, root) = fixture();
    app.view_mode = ViewMode::Heightmap;
    let target = app.viewport_3d.camera.target;
    assert_navigation_only(&mut app, |app| {
        // 未載入材質時沿用 64 個圖塊的預設尺寸，不以高度樣本數取代。
        app.design_check.focus_request = Some((4096.0, 12288.0));
        app.consume_design_check_focus();
        assert_eq!(app.pan, Vec2::new(300.0, -300.0));
        assert_eq!(app.viewport_3d.camera.target, target);
        assert!(app.design_check.focus_request.is_none());
        app.pan = Vec2::new(11.0, 19.0);
        app.consume_design_check_focus();
        assert_eq!(app.pan, Vec2::new(11.0, 19.0));
    });
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn design_check_focus_rejects_non_finite_positions_in_both_views() {
    let (mut app, root) = fixture();
    for mode in [
        ViewMode::Heightmap,
        ViewMode::Collision,
        ViewMode::Blended,
        ViewMode::View3D,
    ] {
        app.view_mode = mode;
        for position in [
            (f32::NAN, 1.0),
            (1.0, f32::NAN),
            (f32::INFINITY, 1.0),
            (1.0, f32::NEG_INFINITY),
        ] {
            let pan = app.pan;
            let target = app.viewport_3d.camera.target;
            let status = (app.status_message.clone(), app.status_is_error);
            assert_navigation_only(&mut app, |app| {
                app.design_check.focus_request = Some(position);
                app.consume_design_check_focus();
                assert!(app.design_check.focus_request.is_none());
                assert_eq!(app.pan, pan);
                assert_eq!(app.viewport_3d.camera.target, target);
                assert_eq!((app.status_message.clone(), app.status_is_error), status);
            });
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn design_check_focus_events_select_valid_indices_and_ignore_stale_indices() {
    let (mut app, root) = fixture();
    assert_navigation_only(&mut app, |app| {
        for index in [2, 0, 1] {
            app.scenario.open = false;
            app.design_check.event_request = Some(index);
            app.consume_design_check_focus();
            assert_eq!(app.scenario.selected, usize::try_from(index).unwrap());
            assert!(app.scenario.open);
            assert!(app.design_check.event_request.is_none());
        }
        for index in [
            i32::try_from(app.scenario.document().events.len()).unwrap(),
            i32::MAX,
            -1,
            i32::MIN,
        ] {
            app.design_check.event_request = Some(index);
            app.consume_design_check_focus();
            assert_eq!(app.scenario.selected, 1);
            assert!(app.design_check.event_request.is_none());
        }
    });
    // 空事件清單也必須能安全消耗索引 0。
    app.scenario = ScenarioUi::default();
    assert!(app.scenario.document().events.is_empty());
    assert_navigation_only(&mut app, |app| {
        app.design_check.event_request = Some(0);
        app.consume_design_check_focus();
        assert_eq!(app.scenario.selected, 0);
        assert!(app.design_check.event_request.is_none());
    });
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn design_check_focus_preserves_existing_dirty_edits_and_undo_history() {
    let (mut app, root) = fixture();
    let session = app.session.as_mut().unwrap();
    session.apply_height_changes(&[SampleChange {
        index: 0,
        before: 32,
        after: 40,
    }]);
    assert!(session.commit_stroke());
    app.scenario
        .add_events(&[Event {
            name: "尚未儲存的事件".into(),
            actions: vec![Action {
                kind: 4,
                ..Default::default()
            }],
            ..Default::default()
        }])
        .unwrap();
    assert!(app.has_unsaved_changes());
    assert!(app.scenario.can_undo_events());
    assert!(app.session.as_ref().unwrap().can_undo());
    app.view_mode = ViewMode::View3D;
    let disk = save_artifact_snapshot(&root);
    assert_navigation_only(&mut app, |app| {
        app.design_check.focus_request = Some((4096.0, 12288.0));
        app.design_check.event_request = Some(3);
        app.consume_design_check_focus();
        assert_eq!(app.scenario.selected, 3);
        assert_eq!(app.viewport_3d.camera.target.x, 16.0);
        assert_eq!(app.viewport_3d.camera.target.z, 48.0);
    });
    assert_eq!(save_artifact_snapshot(&root), disk);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn design_check_focus_selects_spawns_and_data_objects_with_viewport_centering() {
    let (mut app, root) = fixture();
    app.view_mode = ViewMode::View3D;

    // 1. 測試劇本生成物件 ID 定位：選取生成物、同步開啟 Spawns 面板並在 3D 視圖置中
    assert_navigation_only(&mut app, |app| {
        app.spawn_ui.open = false;
        app.design_check.object_request = Some(Uuid::from_u128(0x1000_2000));
        app.consume_design_check_focus();
        assert!(app.design_check.object_request.is_none());
        assert!(app.spawn_ui.open);
        assert_eq!(app.spawn_ui.selected_coordinates(), Some((1024.0, 2048.0)));
        assert_eq!(app.viewport_3d.camera.target.x, 4.0);
        assert_eq!(app.viewport_3d.camera.target.z, 8.0);
    });

    // 2. 測試 DATA 物件 ID (編碼為 1 << 127 | slot << 32 | uid)：選取 placement 物件、開啟 Objects 面板並置中
    let (mut app_data, root_data, map_data) = spawn_save_tests::fixture("focus_placement", &[(232, None)]);
    app_data.placement.load_session(&map_data).unwrap();
    assert!(app_data.placement.is_loaded());
    let (slot, uid) = (0usize, 1u32);
    let data_id = Uuid::from_u128((1u128 << 127) | ((slot as u128) << 32) | u128::from(uid));
    app_data.view_mode = ViewMode::View3D;

    assert_navigation_only(&mut app_data, |app| {
        app.placement.open = false;
        app.design_check.object_request = Some(data_id);
        app.consume_design_check_focus();
        assert!(app.design_check.object_request.is_none());
        assert!(app.placement.open);
        assert_eq!(app.placement.selected_object(), Some((slot, uid)));
    });

    std::fs::remove_dir_all(root_data).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
