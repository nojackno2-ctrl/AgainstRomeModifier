use against_rome_campaign::cinematics::{
    camera::{
        CameraEasingType, CameraFlightPreviewer, CameraPose, CameraPreviewState,
        CameraTrackSplinePlanner, CameraWaypoint, MapStartCamera, StartCameraPrimitive,
    },
    cutscene::{
        CutsceneFxEventKind, CutsceneFxKeyframe, CutsceneSequence, SubtitleKeyframe,
    },
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct OracleRequest {
    easing_queries: Vec<EasingQuery>,
    catmull_rom_queries: Vec<CatmullRomQuery>,
    planner_queries: Vec<PlannerQuery>,
    sample_path_queries: Vec<SamplePathQuery>,
    previewer_queries: Vec<PreviewerQuery>,
    start_camera_validation_queries: Vec<StartCameraValidationQuery>,
    start_camera_json_queries: Vec<StartCameraJsonQuery>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct EasingQuery {
    easing_type: i32,
    t: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CatmullRomQuery {
    p0: [f32; 3],
    p1: [f32; 3],
    p2: [f32; 3],
    p3: [f32; 3],
    t: f32,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct WaypointDto {
    position: [f32; 3],
    pitch_degrees: f32,
    yaw_degrees: f32,
    zoom: f32,
    duration_from_previous: f32,
    easing: i32,
    label: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct PoseDto {
    position: [f32; 3],
    pitch_degrees: f32,
    yaw_degrees: f32,
    zoom: f32,
    time_seconds: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlannerQuery {
    name: String,
    waypoints: Vec<WaypointDto>,
    query_times: Vec<f32>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SamplePathQuery {
    name: String,
    waypoints: Vec<WaypointDto>,
    sample_count: i32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PreviewerQuery {
    name: String,
    waypoints: Vec<WaypointDto>,
    subtitles: Vec<SubtitleDto>,
    fx_events: Vec<FxEventDto>,
    restore_camera_on_complete: bool,
    auto_restore_camera_on_stop: bool,
    initial_camera_pose: Option<PoseDto>,
    playback_speed: f32,
    steps: Vec<PreviewerStepDto>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SubtitleDto {
    start_time: f32,
    duration: f32,
    speaker: String,
    text: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct FxEventDto {
    time_seconds: f32,
    kind: i32,
    parameter: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PreviewerStepDto {
    action: String,
    value: f32,
    flag: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct StartCameraValidationQuery {
    x: String,
    y: String,
    z: String,
    zoom: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct StartCameraJsonQuery {
    enabled: bool,
    x: f64,
    y: f64,
    z: f64,
    zoom: f64,
    primitive: i32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct OracleResponse {
    easing_results: Vec<f32>,
    catmull_rom_results: Vec<[f32; 3]>,
    planner_results: Vec<PlannerResult>,
    sample_path_results: Vec<SamplePathResult>,
    previewer_results: Vec<PreviewerResult>,
    start_camera_validation_results: Vec<StartCameraValidationResult>,
    start_camera_json_results: Vec<StartCameraJsonResult>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlannerResult {
    name: String,
    total_duration: f32,
    count: usize,
    poses: Vec<PoseDto>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SamplePathResult {
    name: String,
    samples: Vec<PoseDto>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PreviewerResult {
    name: String,
    step_results: Vec<PreviewerStepResult>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PreviewerStepResult {
    state: i32,
    current_time: f32,
    last_pose: PoseDto,
    active_subtitle_text: Option<String>,
    triggered_fx_count: usize,
    playback_finished: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct StartCameraValidationResult {
    is_valid: bool,
    error_message: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct StartCameraJsonResult {
    serialized_json: String,
    deserialized_enabled: bool,
    deserialized_x: f64,
    deserialized_y: f64,
    deserialized_z: f64,
    deserialized_zoom: f64,
    deserialized_primitive: i32,
}

fn run_oracle(request: &OracleRequest) -> OracleResponse {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let temp_dir = std::env::temp_dir().join(format!("arm_cam_diff_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");

    let req_path = temp_dir.join("request.json");
    let resp_path = temp_dir.join("response.json");

    let req_bytes = serde_json::to_vec_pretty(request).expect("serialize request");
    std::fs::write(&req_path, req_bytes).expect("write request.json");

    let project_path = manifest_dir.join("tests/oracle_camera_track/CameraTrackOracle.csproj");

    let output = Command::new("dotnet")
        .arg("run")
        .arg("--project")
        .arg(&project_path)
        .arg("-c")
        .arg("Release")
        .arg(&req_path)
        .arg(&resp_path)
        .output()
        .expect("failed to spawn dotnet CameraTrackOracle");

    if !output.status.success() {
        panic!(
            "CameraTrackOracle failed with code {:?}\nstdout: {}\nstderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let resp_bytes = std::fs::read(&resp_path).expect("read response.json");
    let resp: OracleResponse = serde_json::from_slice(&resp_bytes).expect("deserialize response");

    let _ = std::fs::remove_dir_all(&temp_dir);
    resp
}

fn to_rust_easing(code: i32) -> CameraEasingType {
    match code {
        0 => CameraEasingType::Linear,
        1 => CameraEasingType::SmoothStep,
        2 => CameraEasingType::EaseInQuad,
        3 => CameraEasingType::EaseOutQuad,
        4 => CameraEasingType::EaseInOutCubic,
        _ => panic!("unknown easing code {code}"),
    }
}

fn to_rust_waypoint(dto: &WaypointDto) -> CameraWaypoint {
    CameraWaypoint::new(
        dto.position,
        dto.pitch_degrees,
        dto.yaw_degrees,
        dto.zoom,
        dto.duration_from_previous,
        to_rust_easing(dto.easing),
        dto.label.clone(),
    )
}

fn assert_vec3_near(a: [f32; 3], b: [f32; 3], eps: f32, msg: &str) {
    let dx = (a[0] - b[0]).abs();
    let dy = (a[1] - b[1]).abs();
    let dz = (a[2] - b[2]).abs();
    assert!(
        dx <= eps && dy <= eps && dz <= eps,
        "{msg}: a={a:?} b={b:?} (deltas: {dx}, {dy}, {dz}; max allowed {eps})"
    );
}

fn assert_pose_near(a: &CameraPose, b: &PoseDto, eps: f32, msg: &str) {
    assert_vec3_near(a.position, b.position, eps, &format!("{msg} position"));
    let dpitch = (a.pitch_degrees - b.pitch_degrees).abs();
    let dyaw = (a.yaw_degrees - b.yaw_degrees).abs();
    let dzoom = (a.zoom - b.zoom).abs();
    let dtime = (a.time_seconds - b.time_seconds).abs();
    assert!(dpitch <= eps, "{msg} pitch: got {} want {}", a.pitch_degrees, b.pitch_degrees);
    // Yaw wrap check: 0 and 360 are equivalent
    let yaw_diff = dyaw.min((dyaw - 360.0).abs());
    assert!(yaw_diff <= eps, "{msg} yaw: got {} want {}", a.yaw_degrees, b.yaw_degrees);
    assert!(dzoom <= eps, "{msg} zoom: got {} want {}", a.zoom, b.zoom);
    assert!(dtime <= eps, "{msg} time: got {} want {}", a.time_seconds, b.time_seconds);
}

#[test]
fn test_csharp_dotnet10_camera_track_differential() {
    let mut request = OracleRequest {
        easing_queries: Vec::new(),
        catmull_rom_queries: Vec::new(),
        planner_queries: Vec::new(),
        sample_path_queries: Vec::new(),
        previewer_queries: Vec::new(),
        start_camera_validation_queries: Vec::new(),
        start_camera_json_queries: Vec::new(),
    };

    // 1. Easing queries
    let t_values = [-0.5f32, 0.0, 0.001, 0.1, 0.25, 0.3333, 0.5, 0.6667, 0.75, 0.999, 1.0, 1.5];
    for easing_code in 0..5 {
        for &t in &t_values {
            request.easing_queries.push(EasingQuery {
                easing_type: easing_code,
                t,
            });
        }
    }

    // 2. Catmull-Rom spline curves
    let test_points: Vec<([f32; 3], [f32; 3], [f32; 3], [f32; 3])> = vec![
        // Collinear straight line
        ([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [20.0, 0.0, 0.0], [30.0, 0.0, 0.0]),
        // 3D curve with elevation and diagonal
        ([1000.0, 20.0, 1000.0], [2000.0, 80.0, 3000.0], [4000.0, 150.0, 5000.0], [7000.0, 30.0, 6000.0]),
        // U-turn
        ([0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [100.0, 0.0, 100.0], [0.0, 0.0, 100.0]),
        // Duplicate start points (p0 == p1)
        ([500.0, 10.0, 500.0], [500.0, 10.0, 500.0], [1500.0, 50.0, 2500.0], [3000.0, 10.0, 4000.0]),
        // Duplicate middle points (p1 == p2)
        ([100.0, 0.0, 100.0], [1000.0, 50.0, 1000.0], [1000.0, 50.0, 1000.0], [2000.0, 0.0, 2000.0]),
        // Duplicate end points (p2 == p3)
        ([100.0, 0.0, 100.0], [1000.0, 50.0, 1000.0], [2000.0, 100.0, 2000.0], [2000.0, 100.0, 2000.0]),
        // All identical points
        ([8192.0, 50.0, 8192.0], [8192.0, 50.0, 8192.0], [8192.0, 50.0, 8192.0], [8192.0, 50.0, 8192.0]),
        // Very small segment distance (< 1e-4)
        ([10.0, 10.0, 10.0], [10.00001, 10.00001, 10.0], [10.00002, 10.00002, 10.0], [10.00005, 10.00005, 10.0]),
    ];
    let cr_t_values = [0.0f32, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0, 1.2];
    for &(p0, p1, p2, p3) in &test_points {
        for &t in &cr_t_values {
            request.catmull_rom_queries.push(CatmullRomQuery { p0, p1, p2, p3, t });
        }
    }

    // 3. Planner queries
    // 3a. Empty waypoints
    request.planner_queries.push(PlannerQuery {
        name: "empty_planner".into(),
        waypoints: vec![],
        query_times: vec![-1.0, 0.0, 1.0, 5.0, 10.0],
    });

    // 3b. Single waypoint
    request.planner_queries.push(PlannerQuery {
        name: "single_waypoint".into(),
        waypoints: vec![WaypointDto {
            position: [1200.0, 50.0, 3400.0],
            pitch_degrees: 42.0,
            yaw_degrees: 135.0,
            zoom: 65.0,
            duration_from_previous: 0.0,
            easing: 0,
            label: Some("SoloPoint".into()),
        }],
        query_times: vec![-5.0, 0.0, 2.0, 100.0],
    });

    // 3c. Two waypoints with zero duration
    request.planner_queries.push(PlannerQuery {
        name: "zero_duration_segment".into(),
        waypoints: vec![
            WaypointDto {
                position: [1000.0, 0.0, 1000.0],
                pitch_degrees: 30.0,
                yaw_degrees: 45.0,
                zoom: 80.0,
                duration_from_previous: 0.0,
                easing: 0,
                label: None,
            },
            WaypointDto {
                position: [2000.0, 0.0, 2000.0],
                pitch_degrees: 30.0,
                yaw_degrees: 45.0,
                zoom: 80.0,
                duration_from_previous: 0.0, // should clamp to 0.001
                easing: 0,
                label: None,
            },
        ],
        query_times: vec![0.0, 0.0005, 0.001, 0.5],
    });

    // 3d. Angle unwrapping across 350° -> 10° and 10° -> 350°
    request.planner_queries.push(PlannerQuery {
        name: "yaw_unwrapping_shortest_arc".into(),
        waypoints: vec![
            WaypointDto {
                position: [1000.0, 0.0, 1000.0],
                pitch_degrees: 30.0,
                yaw_degrees: 350.0,
                zoom: 82.0,
                duration_from_previous: 0.0,
                easing: 0, // Linear
                label: None,
            },
            WaypointDto {
                position: [2000.0, 0.0, 2000.0],
                pitch_degrees: 30.0,
                yaw_degrees: 10.0,
                zoom: 82.0,
                duration_from_previous: 2.0,
                easing: 0, // Linear
                label: None,
            },
            WaypointDto {
                position: [3000.0, 0.0, 3000.0],
                pitch_degrees: 30.0,
                yaw_degrees: 350.0,
                zoom: 82.0,
                duration_from_previous: 2.0,
                easing: 0, // Linear
                label: None,
            },
        ],
        query_times: vec![0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0],
    });

    // 3e. Multi-waypoint full complex cinematic spline
    request.planner_queries.push(PlannerQuery {
        name: "complex_cinematic_track".into(),
        waypoints: vec![
            WaypointDto {
                position: [1000.0, 10.0, 1000.0],
                pitch_degrees: 20.0,
                yaw_degrees: 45.0,
                zoom: 90.0,
                duration_from_previous: 0.0,
                easing: 1, // SmoothStep
                label: Some("Intro".into()),
            },
            WaypointDto {
                position: [3000.0, 120.0, 2500.0],
                pitch_degrees: 60.0,
                yaw_degrees: 90.0,
                zoom: 50.0,
                duration_from_previous: 4.0,
                easing: 2, // EaseInQuad
                label: Some("Peak".into()),
            },
            WaypointDto {
                position: [5500.0, 40.0, 4800.0],
                pitch_degrees: 85.0,
                yaw_degrees: 180.0,
                zoom: 30.0,
                duration_from_previous: 3.5,
                easing: 3, // EaseOutQuad
                label: Some("Dive".into()),
            },
            WaypointDto {
                position: [8000.0, 200.0, 7500.0],
                pitch_degrees: 15.0,
                yaw_degrees: 300.0,
                zoom: 70.0,
                duration_from_previous: 5.0,
                easing: 4, // EaseInOutCubic
                label: Some("Final".into()),
            },
        ],
        query_times: vec![
            -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.5, 9.0, 10.5, 12.0, 12.5, 15.0,
        ],
    });

    // 4. Sample path queries
    request.sample_path_queries.push(SamplePathQuery {
        name: "sample_empty".into(),
        waypoints: vec![],
        sample_count: 5,
    });
    request.sample_path_queries.push(SamplePathQuery {
        name: "sample_single".into(),
        waypoints: vec![WaypointDto {
            position: [2000.0, 0.0, 2000.0],
            pitch_degrees: 30.0,
            yaw_degrees: 45.0,
            zoom: 82.0,
            duration_from_previous: 0.0,
            easing: 1,
            label: None,
        }],
        sample_count: 5,
    });
    request.sample_path_queries.push(SamplePathQuery {
        name: "sample_track_11".into(),
        waypoints: vec![
            WaypointDto {
                position: [1000.0, 0.0, 1000.0],
                pitch_degrees: 30.0,
                yaw_degrees: 45.0,
                zoom: 80.0,
                duration_from_previous: 0.0,
                easing: 1,
                label: None,
            },
            WaypointDto {
                position: [3000.0, 100.0, 2000.0],
                pitch_degrees: 45.0,
                yaw_degrees: 90.0,
                zoom: 60.0,
                duration_from_previous: 4.0,
                easing: 1,
                label: None,
            },
            WaypointDto {
                position: [5000.0, 50.0, 4000.0],
                pitch_degrees: 35.0,
                yaw_degrees: 120.0,
                zoom: 70.0,
                duration_from_previous: 3.0,
                easing: 1,
                label: None,
            },
        ],
        sample_count: 11,
    });

    // 5. Previewer queries
    request.previewer_queries.push(PreviewerQuery {
        name: "flight_preview_full_lifecycle".into(),
        waypoints: vec![
            WaypointDto {
                position: [1000.0, 0.0, 1000.0],
                pitch_degrees: 30.0,
                yaw_degrees: 45.0,
                zoom: 82.0,
                duration_from_previous: 0.0,
                easing: 1,
                label: None,
            },
            WaypointDto {
                position: [2000.0, 50.0, 2000.0],
                pitch_degrees: 45.0,
                yaw_degrees: 90.0,
                zoom: 60.0,
                duration_from_previous: 4.0,
                easing: 1,
                label: None,
            },
        ],
        subtitles: vec![
            SubtitleDto {
                start_time: 1.0,
                duration: 2.0,
                speaker: "Leader".into(),
                text: "Advance!".into(),
            },
        ],
        fx_events: vec![
            FxEventDto {
                time_seconds: 1.5,
                kind: 2, // FadeIn
                parameter: "white".into(),
            },
        ],
        restore_camera_on_complete: true,
        auto_restore_camera_on_stop: true,
        initial_camera_pose: Some(PoseDto {
            position: [9999.0, 99.0, 9999.0],
            pitch_degrees: 30.0,
            yaw_degrees: 45.0,
            zoom: 82.0,
            time_seconds: 0.0,
        }),
        playback_speed: 1.0,
        steps: vec![
            PreviewerStepDto { action: "update".into(), value: 0.5, flag: false }, // t = 0.5 (no sub)
            PreviewerStepDto { action: "update".into(), value: 1.0, flag: false }, // t = 1.5 (sub active, fx triggered)
            PreviewerStepDto { action: "pause".into(), value: 0.0, flag: false },  // paused
            PreviewerStepDto { action: "resume".into(), value: 0.0, flag: false }, // resumed
            PreviewerStepDto { action: "scrub".into(), value: 2.5, flag: false },  // scrubbed to 2.5
            PreviewerStepDto { action: "update".into(), value: 2.0, flag: false }, // t = 4.5 -> end reached -> restored!
        ],
    });

    // 6. StartCamera validation queries
    request.start_camera_validation_queries = vec![
        StartCameraValidationQuery { x: "8192.0".into(), y: "0.0".into(), z: "8192.0".into(), zoom: "5.0".into() },     // valid default
        StartCameraValidationQuery { x: "0.0".into(), y: "-100.0".into(), z: "0.0".into(), zoom: "0.0".into() },        // valid min
        StartCameraValidationQuery { x: "16383.0".into(), y: "5000.0".into(), z: "16383.0".into(), zoom: "9.0".into() },// valid max
        StartCameraValidationQuery { x: "-0.1".into(), y: "0.0".into(), z: "8192.0".into(), zoom: "5.0".into() },       // invalid X < 0
        StartCameraValidationQuery { x: "16383.1".into(), y: "0.0".into(), z: "8192.0".into(), zoom: "5.0".into() },    // invalid X > 16383
        StartCameraValidationQuery { x: "8192.0".into(), y: "0.0".into(), z: "-1.0".into(), zoom: "5.0".into() },       // invalid Z < 0
        StartCameraValidationQuery { x: "8192.0".into(), y: "0.0".into(), z: "16384.0".into(), zoom: "5.0".into() },    // invalid Z > 16383
        StartCameraValidationQuery { x: "8192.0".into(), y: "0.0".into(), z: "8192.0".into(), zoom: "-0.5".into() },    // invalid zoom < 0
        StartCameraValidationQuery { x: "8192.0".into(), y: "0.0".into(), z: "8192.0".into(), zoom: "9.5".into() },     // invalid zoom > 9
        StartCameraValidationQuery { x: "NaN".into(), y: "0.0".into(), z: "8192.0".into(), zoom: "5.0".into() },        // NaN X
        StartCameraValidationQuery { x: "8192.0".into(), y: "Infinity".into(), z: "8192.0".into(), zoom: "5.0".into() }, // Inf Y
    ];

    // 7. StartCamera JSON queries
    request.start_camera_json_queries = vec![
        StartCameraJsonQuery {
            enabled: true,
            x: 8192.0,
            y: 0.0,
            z: 8192.0,
            zoom: 5.0,
            primitive: 0, // CenterToPos
        },
        StartCameraJsonQuery {
            enabled: true,
            x: 12512.5,
            y: 35.0,
            z: 14816.5,
            zoom: 7.25,
            primitive: 1, // DirectEnginePos
        },
        StartCameraJsonQuery {
            enabled: false,
            x: 4096.0,
            y: 10.0,
            z: 4096.0,
            zoom: 3.0,
            primitive: 0,
        },
    ];

    // RUN THE ORACLE!
    let oracle_resp = run_oracle(&request);

    // ==========================================
    // VERIFICATION 1: Easing Functions Parity
    // ==========================================
    assert_eq!(request.easing_queries.len(), oracle_resp.easing_results.len());
    for (q, &expected) in request.easing_queries.iter().zip(&oracle_resp.easing_results) {
        let rust_easing = to_rust_easing(q.easing_type);
        let actual = CameraTrackSplinePlanner::apply_easing(q.t, rust_easing);
        let delta = (actual - expected).abs();
        assert!(
            delta < 1e-6,
            "Easing parity mismatch for {:?} at t={}: got {actual} want {expected}",
            rust_easing,
            q.t
        );
    }

    // ==========================================
    // VERIFICATION 2: Catmull-Rom Spline Parity
    // ==========================================
    assert_eq!(request.catmull_rom_queries.len(), oracle_resp.catmull_rom_results.len());
    for (q, &expected) in request.catmull_rom_queries.iter().zip(&oracle_resp.catmull_rom_results) {
        let actual = CameraTrackSplinePlanner::evaluate_centripetal_catmull_rom(q.p0, q.p1, q.p2, q.p3, q.t);
        assert_vec3_near(
            actual,
            expected,
            0.01,
            &format!("Catmull-Rom mismatch at t={}", q.t),
        );
    }

    // ==========================================
    // VERIFICATION 3: Planner Evaluation Parity
    // ==========================================
    assert_eq!(request.planner_queries.len(), oracle_resp.planner_results.len());
    for (q, o_res) in request.planner_queries.iter().zip(&oracle_resp.planner_results) {
        let rust_waypoints: Vec<CameraWaypoint> = q.waypoints.iter().map(to_rust_waypoint).collect();
        let planner = CameraTrackSplinePlanner::new(&rust_waypoints);

        assert_eq!(planner.count(), o_res.count, "planner count for {}", q.name);
        let d_total = (planner.total_duration() - o_res.total_duration).abs();
        assert!(
            d_total < 1e-4,
            "TotalDuration mismatch for {}: got {} want {}",
            q.name,
            planner.total_duration(),
            o_res.total_duration
        );

        assert_eq!(q.query_times.len(), o_res.poses.len());
        for (&t, expected_pose) in q.query_times.iter().zip(&o_res.poses) {
            let actual_pose = planner.evaluate(t);
            assert_pose_near(&actual_pose, expected_pose, 1e-3, &format!("{}[t={}]", q.name, t));
        }
    }

    // ==========================================
    // VERIFICATION 4: SamplePath Parity
    // ==========================================
    assert_eq!(request.sample_path_queries.len(), oracle_resp.sample_path_results.len());
    for (q, o_res) in request.sample_path_queries.iter().zip(&oracle_resp.sample_path_results) {
        let rust_waypoints: Vec<CameraWaypoint> = q.waypoints.iter().map(to_rust_waypoint).collect();
        let planner = CameraTrackSplinePlanner::new(&rust_waypoints);
        let actual_samples = planner.sample_path(q.sample_count as usize);

        assert_eq!(actual_samples.len(), o_res.samples.len(), "Sample count for {}", q.name);
        for (i, (actual, expected)) in actual_samples.iter().zip(&o_res.samples).enumerate() {
            assert_pose_near(actual, expected, 1e-3, &format!("{}[sample {}]", q.name, i));
        }
    }

    // ==========================================
    // VERIFICATION 5: Flight Previewer Lifecycle
    // ==========================================
    assert_eq!(request.previewer_queries.len(), oracle_resp.previewer_results.len());
    for (q, o_res) in request.previewer_queries.iter().zip(&oracle_resp.previewer_results) {
        let rust_waypoints: Vec<CameraWaypoint> = q.waypoints.iter().map(to_rust_waypoint).collect();
        let rust_subtitles: Vec<SubtitleKeyframe> = q
            .subtitles
            .iter()
            .map(|s| SubtitleKeyframe::new(s.start_time, s.duration, &s.speaker, &s.text))
            .collect();
        let rust_fx: Vec<CutsceneFxKeyframe> = q
            .fx_events
            .iter()
            .map(|f| {
                let kind = match f.kind {
                    0 => CutsceneFxEventKind::LetterboxOn,
                    1 => CutsceneFxEventKind::LetterboxOff,
                    2 => CutsceneFxEventKind::FadeIn,
                    3 => CutsceneFxEventKind::FadeOut,
                    4 => CutsceneFxEventKind::PlayAudio,
                    _ => CutsceneFxEventKind::TriggerScenarioEvent,
                };
                CutsceneFxKeyframe::new(f.time_seconds, kind, &f.parameter)
            })
            .collect();

        let seq = CutsceneSequence {
            name: q.name.clone(),
            camera_waypoints: rust_waypoints,
            subtitles: rust_subtitles,
            fx_events: rust_fx,
            restore_camera_on_complete: q.restore_camera_on_complete,
            ..Default::default()
        };

        let initial_pose = q.initial_camera_pose.as_ref().map(|p| CameraPose {
            position: p.position,
            pitch_degrees: p.pitch_degrees,
            yaw_degrees: p.yaw_degrees,
            zoom: p.zoom,
            time_seconds: p.time_seconds,
        });

        let mut previewer = CameraFlightPreviewer::new();
        previewer.auto_restore_camera_on_stop = q.auto_restore_camera_on_stop;
        previewer.playback_speed = q.playback_speed;
        previewer.play(seq, initial_pose);

        for (step_idx, (step, expected_step)) in q.steps.iter().zip(&o_res.step_results).enumerate() {
            match step.action.as_str() {
                "update" => previewer.update(step.value),
                "scrub" => previewer.scrub_to(step.value),
                "pause" => previewer.pause(),
                "resume" => previewer.resume(),
                "stop" => previewer.stop(step.flag),
                _ => {}
            }

            let expected_state = match expected_step.state {
                0 => CameraPreviewState::Stopped,
                1 => CameraPreviewState::Playing,
                2 => CameraPreviewState::Paused,
                _ => panic!("unknown state code {}", expected_step.state),
            };
            assert_eq!(
                previewer.state, expected_state,
                "{}[step {} state]", q.name, step_idx
            );

            let d_time = (previewer.current_time - expected_step.current_time).abs();
            assert!(
                d_time < 1e-4,
                "{}[step {} time]: got {} want {}",
                q.name,
                step_idx,
                previewer.current_time,
                expected_step.current_time
            );

            assert_pose_near(
                &previewer.last_camera_pose,
                &expected_step.last_pose,
                1e-3,
                &format!("{}[step {} last_pose]", q.name, step_idx),
            );

            let actual_sub_text = previewer
                .current_active_subtitle()
                .map(|s| s.text.clone());
            assert_eq!(
                actual_sub_text, expected_step.active_subtitle_text,
                "{}[step {} subtitle]", q.name, step_idx
            );

            assert_eq!(
                previewer.triggered_fx_events().len(),
                expected_step.triggered_fx_count,
                "{}[step {} fx_count]", q.name, step_idx
            );

            assert_eq!(
                previewer.playback_finished,
                expected_step.playback_finished,
                "{}[step {} playback_finished]", q.name, step_idx
            );
        }
    }

    // ==========================================
    // VERIFICATION 6: MapStartCamera Validation
    // ==========================================
    assert_eq!(
        request.start_camera_validation_queries.len(),
        oracle_resp.start_camera_validation_results.len()
    );
    for (q, o_res) in request
        .start_camera_validation_queries
        .iter()
        .zip(&oracle_resp.start_camera_validation_results)
    {
        let x: f64 = q.x.parse().unwrap();
        let y: f64 = q.y.parse().unwrap();
        let z: f64 = q.z.parse().unwrap();
        let zoom: f64 = q.zoom.parse().unwrap();
        let cam = MapStartCamera {
            enabled: true,
            x,
            y,
            z,
            zoom,
            primitive: StartCameraPrimitive::CenterToPos,
        };
        let val_res = cam.validate();
        assert_eq!(
            val_res.is_ok(),
            o_res.is_valid,
            "Validation mismatch for ({}, {}, {}, {}): Rust={:?} C#={:?}",
            q.x,
            q.y,
            q.z,
            q.zoom,
            val_res,
            o_res.error_message
        );
    }

    // ==========================================
    // VERIFICATION 7: MapStartCamera JSON Cross-Parity
    // ==========================================
    assert_eq!(
        request.start_camera_json_queries.len(),
        oracle_resp.start_camera_json_results.len()
    );
    for (q, o_res) in request
        .start_camera_json_queries
        .iter()
        .zip(&oracle_resp.start_camera_json_results)
    {
        // 7a. Test Rust reading JSON produced by C#
        let from_csharp: Result<MapStartCamera, _> = serde_json::from_str(&o_res.serialized_json);
        assert!(
            from_csharp.is_ok(),
            "Rust failed to parse C# JSON: {:?}\nJSON was: {}",
            from_csharp.err(),
            o_res.serialized_json
        );
        let parsed_csharp = from_csharp.unwrap();
        assert_eq!(parsed_csharp.enabled, q.enabled);
        assert_eq!(parsed_csharp.x, q.x);
        assert_eq!(parsed_csharp.y, q.y);
        assert_eq!(parsed_csharp.z, q.z);
        assert_eq!(parsed_csharp.zoom, q.zoom);
        assert_eq!(
            parsed_csharp.primitive as i32,
            q.primitive,
            "Primitive code mismatch when parsing C# JSON"
        );

        // 7b. Test Rust serializing and verifying deserialization matches
        let rust_cam = MapStartCamera {
            enabled: q.enabled,
            x: q.x,
            y: q.y,
            z: q.z,
            zoom: q.zoom,
            primitive: if q.primitive == 1 {
                StartCameraPrimitive::DirectEnginePos
            } else {
                StartCameraPrimitive::CenterToPos
            },
        };
        let rust_json = serde_json::to_string_pretty(&rust_cam).expect("serialize rust cam");
        // Verify C#'s deserialized results match what we expect
        assert_eq!(o_res.deserialized_enabled, q.enabled);
        assert_eq!(o_res.deserialized_x, q.x);
        assert_eq!(o_res.deserialized_y, q.y);
        assert_eq!(o_res.deserialized_z, q.z);
        assert_eq!(o_res.deserialized_zoom, q.zoom);
        assert_eq!(o_res.deserialized_primitive, q.primitive);

        // Verify roundtrip in Rust
        let roundtrip_rust: MapStartCamera = serde_json::from_str(&rust_json).expect("roundtrip");
        assert_eq!(roundtrip_rust, rust_cam);
    }
}
