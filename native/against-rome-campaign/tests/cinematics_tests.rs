use against_rome_campaign::*;
use against_rome_scenario::BciImage;

#[test]
fn test_camera_track_spline_planner_evaluation() {
    let wps = vec![
        CameraWaypoint::new(
            [1000.0, 0.0, 1000.0],
            30.0,
            45.0,
            82.0,
            0.0,
            CameraEasingType::Linear,
            None,
        ),
        CameraWaypoint::new(
            [2000.0, 0.0, 2000.0],
            40.0,
            90.0,
            50.0,
            4.0,
            CameraEasingType::Linear,
            None,
        ),
    ];

    let planner = CameraTrackSplinePlanner::new(&wps);
    assert_eq!(planner.count(), 2);
    assert!((planner.total_duration() - 4.0).abs() < 1e-3);

    let pose_start = planner.evaluate(0.0);
    assert!((pose_start.position[0] - 1000.0).abs() < 1e-1);
    assert!((pose_start.position[2] - 1000.0).abs() < 1e-1);

    let pose_end = planner.evaluate(4.0);
    assert!((pose_end.position[0] - 2000.0).abs() < 1e-1);
    assert!((pose_end.position[2] - 2000.0).abs() < 1e-1);

    let pose_mid = planner.evaluate(2.0);
    assert!(pose_mid.position[0] > 1000.0 && pose_mid.position[0] < 2000.0);
}

#[test]
fn test_camera_flight_previewer_lifecycle() {
    let mut previewer = CameraFlightPreviewer::new();
    assert_eq!(previewer.state, CameraPreviewState::Stopped);

    let seq = CutsceneSequence {
        name: "TestCutscene".to_string(),
        camera_waypoints: vec![
            CameraWaypoint::new(
                [1000.0, 0.0, 1000.0],
                30.0,
                45.0,
                82.0,
                0.0,
                CameraEasingType::Linear,
                None,
            ),
            CameraWaypoint::new(
                [2000.0, 0.0, 2000.0],
                30.0,
                45.0,
                82.0,
                2.0,
                CameraEasingType::Linear,
                None,
            ),
        ],
        subtitles: vec![SubtitleKeyframe::new(0.5, 1.0, "Narrator", "Welcome!")],
        ..Default::default()
    };

    let original_pose = CameraPose {
        position: [500.0, 0.0, 500.0],
        pitch_degrees: 30.0,
        yaw_degrees: 45.0,
        zoom: 82.0,
        time_seconds: 0.0,
    };

    previewer.play(seq, Some(original_pose));
    assert_eq!(previewer.state, CameraPreviewState::Playing);

    // Advance to 0.6s -> subtitle should be active
    previewer.update(0.6);
    assert_eq!(previewer.state, CameraPreviewState::Playing);
    let sub = previewer.current_active_subtitle().unwrap();
    assert_eq!(sub.text, "Welcome!");

    // Advance past total duration -> should stop and restore
    previewer.update(2.0);
    assert_eq!(previewer.state, CameraPreviewState::Stopped);
}

#[test]
fn test_cutscene_sequence_catalog_crud() {
    let mut catalog = CutsceneSequenceCatalog::new();
    assert_eq!(catalog.count(), 0);
    assert!(!catalog.is_dirty());

    let seq = CutsceneSequence {
        id: "seq_1".to_string(),
        name: "Intro".to_string(),
        ..Default::default()
    };
    assert!(catalog.add(seq).is_ok());
    assert_eq!(catalog.count(), 1);
    assert!(catalog.is_dirty());

    let retrieved = catalog.get_by_id("seq_1").unwrap();
    assert_eq!(retrieved.name, "Intro");

    assert!(catalog.remove_at(0).is_ok());
    assert_eq!(catalog.count(), 0);
}

#[test]
fn test_cinematic_bci_compiler_camera_calls() {
    let mut image = BciImage::idle();
    let original_main = image.main;
    let original_code_len = image.code.len();

    let calls = CinematicBciCompiler::compile_camera_calls(&mut image, 1.25, -2.5, 3000.0, 2.75).unwrap();
    assert!(!calls.is_empty());

    // Should not mutate image code or main directly
    assert_eq!(image.main, original_main);
    assert_eq!(image.code.len(), original_code_len);

    // Verify constants were registered
    let pos_const = image.native("s_lgcSetEnginePos").unwrap();
    let zoom_const = image.native("s_lgcSetEngineZoom").unwrap();
    assert!(calls.contains(&pos_const));
    assert!(calls.contains(&zoom_const));
}

#[test]
fn test_map_start_camera_injector() {
    let mut image = BciImage::idle();
    let original_main = image.main;

    let camera = MapStartCamera {
        enabled: true,
        x: 4000.0,
        y: 10.0,
        z: 4000.0,
        zoom: 4.5,
        primitive: StartCameraPrimitive::CenterToPos,
    };

    MapStartCameraInjector::inject(&mut image, &camera).unwrap();

    // Main should point to shim now
    assert_ne!(image.main, original_main);

    // Last instructions of shim should be OpJump to original main
    let last_op = image.code[image.code.len() - 2];
    assert_eq!(last_op, MapStartCameraInjector::OP_JUMP);
}

#[test]
fn test_cutscene_spline_initial_camera_extraction() {
    let mut seq = CutsceneSequence {
        name: "IntroFlyover".to_string(),
        camera_waypoints: vec![
            CameraWaypoint::new(
                [1500.5, 20.0, 3500.5],
                35.0,
                45.0,
                55.0,
                0.0,
                CameraEasingType::Linear,
                Some("StartPoint".into()),
            ),
            CameraWaypoint::new(
                [2500.0, 30.0, 4500.0],
                30.0,
                60.0,
                60.0,
                5.0,
                CameraEasingType::SmoothStep,
                Some("EndPoint".into()),
            ),
        ],
        ..Default::default()
    };

    let start_cam = CinematicBciCompiler::initial_start_camera(&seq).unwrap();
    assert!(start_cam.enabled);
    assert_eq!(start_cam.x, 1500.5);
    assert_eq!(start_cam.y, 20.0);
    assert_eq!(start_cam.z, 3500.5);
    assert!((start_cam.zoom - 5.5).abs() < 1e-4);
    assert_eq!(start_cam.primitive, StartCameraPrimitive::DirectEnginePos);

    // Empty sequence returns None
    seq.camera_waypoints.clear();
    assert!(CinematicBciCompiler::initial_start_camera(&seq).is_none());
}

#[test]
fn test_cutscene_events_compilation_and_sequence_to_bci() {
    let seq = CutsceneSequence {
        name: "BattleIntro".to_string(),
        camera_waypoints: vec![
            CameraWaypoint::new(
                [1200.0, 0.0, 1400.0],
                30.0,
                45.0,
                50.0,
                0.0,
                CameraEasingType::Linear,
                None,
            ),
        ],
        subtitles: vec![
            SubtitleKeyframe::new(1.0, 3.0, "Leader", "To battle!"),
            SubtitleKeyframe::new(5.0, 2.0, "", "Charge!"),
        ],
        unit_orders: vec![
            UnitOrderKeyframe::new(
                2.0,
                "GER_INF00",
                CutsceneUnitOrderKind::MoveTo,
                3000.0,
                4000.0,
            ),
        ],
        on_complete_trigger_event: Some("StartBattleTimer".into()),
        ..Default::default()
    };

    let events = CinematicBciCompiler::compile_cutscene_events(&seq);
    assert_eq!(events.len(), 4); // 2 subtitles + 1 unit order + 1 on_complete
    assert_eq!(events[0].name, "BattleIntro_字幕_1");
    assert_eq!(events[0].delay_seconds, 1);
    assert_eq!(events[1].name, "BattleIntro_字幕_2");
    assert_eq!(events[1].delay_seconds, 5);
    assert_eq!(events[2].name, "BattleIntro_部隊指令_1");
    assert_eq!(events[2].delay_seconds, 2);
    assert_eq!(events[3].name, "BattleIntro_完結觸發");

    // Test compiling directly to BCI
    let idle_bci = BciImage::idle().serialize();
    let scenario = against_rome_scenario::ScenarioDocument::default();
    let aliases = vec!["GER_INF00".to_string()];

    let bci_result = CinematicBciCompiler::compile_sequence_to_bci(
        &seq,
        &idle_bci,
        &scenario,
        &aliases,
        None,
    );
    assert!(bci_result.is_ok(), "{:?}", bci_result.err());
    let bci_bytes = bci_result.unwrap();
    assert!(bci_bytes.len() > idle_bci.len());
    let parsed = BciImage::parse(&bci_bytes);
    assert!(parsed.is_ok());
}

#[test]
fn test_bankers_rounding_parity() {
    let mut image = BciImage::idle();
    // 100.5 rounds to 100 with banker's rounding (even), 101.5 rounds to 102
    let cam1 = MapStartCamera {
        x: 100.5,
        z: 101.5,
        zoom: 3.0,
        primitive: StartCameraPrimitive::CenterToPos,
        enabled: true,
        ..Default::default()
    };
    let words = CinematicBciCompiler::compile_start_camera_calls(&mut image, &cam1).unwrap();
    // In CenterToPos: words[1] is z (102), words[3] is x (100)
    assert_eq!(words[1], 102);
    assert_eq!(words[3], 100);
}
