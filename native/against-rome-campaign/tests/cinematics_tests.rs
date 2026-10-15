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
