//! 故事地圖「The Last Granary」建置工具（預設忽略）。
//!
//! 透過編輯器公開 API 載入 `AR_STORY_MAP_DIR` 指向的自製地圖工作副本，
//! 檢查地形高度與通行碰撞後，建立完整可遊玩劇本（開局簡報＋敵對外交、三波進攻、
//! 中央矩形偵察提示、8:00 存活勝利、主屋被摧毀失敗），執行 Design Check、
//! Safe Save 並重新載入驗證。
//!
//! 執行：
//! `$env:AR_STORY_MAP_DIR='<工作副本>'; cargo test -p against-rome-editor-app --locked --test story_map_builder -- --ignored --nocapture`

use against_rome_ambience::Language as AppLanguage;
use against_rome_editor_app::{
    app::EditorApp,
    design_check_ui::report_rows,
    load_map,
};
use against_rome_maps::design_validator::MapDesignValidator;
use against_rome_maps::diagnostics::MapIssueSeverity;
use against_rome_scenario::{Action, Condition, Event, WorldPosition};
use std::path::PathBuf;

/// 檢查世界座標 (wx, wz) 及其周圍 3x3 碰撞格是否為可行走陸地。
fn is_passable_land(app: &EditorApp, wx: f32, wz: f32) -> bool {
    let Some(session) = app.session.as_ref() else {
        return false;
    };
    if !(256.0..=16127.0).contains(&wx) || !(256.0..=16127.0).contains(&wz) {
        return false;
    }
    let Some(h) = app.height_at_world(wx, wz) else {
        return false;
    };
    let settings = app.effective_map_settings();
    let world_h = f32::from(h) * settings.height_step;
    if world_h <= settings.water_world + 1.0 {
        return false;
    }
    let csize = session.collision_size;
    let collision = session.collision();
    if csize == 0 || collision.len() != csize * csize {
        return false;
    }
    let cx = ((wx / 16384.0) * csize as f32)
        .floor()
        .clamp(0.0, (csize - 1) as f32) as isize;
    let cz = ((wz / 16384.0) * csize as f32)
        .floor()
        .clamp(0.0, (csize - 1) as f32) as isize;
    for dz in -1..=1 {
        for dx in -1..=1 {
            let nx = cx + dx;
            let nz = cz + dz;
            if nx < 0 || nz < 0 || nx >= csize as isize || nz >= csize as isize {
                return false;
            }
            if collision[(nz as usize) * csize + (nx as usize)] != 0 {
                return false;
            }
        }
    }
    true
}

/// 從候選網格中尋找距離玩家主屋足夠遠且為可行走陸地的邊緣生成點。
fn find_edge_spawn_point(app: &EditorApp, base_x: f64, base_z: f64) -> (f32, f32) {
    for z_step in (1280..=5120).step_by(256) {
        for x_step in (1280..=5120).step_by(256) {
            let wx = x_step as f32;
            let wz = z_step as f32;
            let dist_sq = (f64::from(wx) - base_x).powi(2) + (f64::from(wz) - base_z).powi(2);
            if dist_sq >= 5000.0_f64.powi(2) && is_passable_land(app, wx, wz) {
                return (wx, wz);
            }
        }
    }
    panic!("no passable land spawn point found near far map edge");
}

/// 在地圖中央 (8192, 8192) 附近尋找可行走陸地，作為偵察矩形中心。
fn find_center_scout_point(app: &EditorApp) -> (i32, i32) {
    for radius in (0..=2048).step_by(256) {
        for dz in (-radius..=radius).step_by(256) {
            for dx in (-radius..=radius).step_by(256) {
                let wx = 8192 + dx;
                let wz = 8192 + dz;
                if is_passable_land(app, wx as f32, wz as f32) {
                    return (wx, wz);
                }
            }
        }
    }
    panic!("no passable land found near map center (8192, 8192)");
}

fn print_events(label: &str, events: &[Event]) {
    println!("=== {label} ({} events) ===", events.len());
    for (idx, ev) in events.iter().enumerate() {
        println!(
            "[{idx}] {} | delay={}s ({}:{:02}) | repeat={} | enabled={} | conditions={} | actions={}",
            ev.name,
            ev.delay_seconds,
            ev.delay_seconds / 60,
            ev.delay_seconds % 60,
            ev.repeat,
            ev.enabled,
            ev.conditions.len(),
            ev.actions.len()
        );
        for (ci, c) in ev.conditions.iter().enumerate() {
            println!(
                "    cond[{ci}]: kind={} target_id={} rect=({}, {})..=({}, {})",
                c.kind, c.target_id, c.min_x, c.min_z, c.max_x, c.max_z
            );
        }
        for (ai, a) in ev.actions.iter().enumerate() {
            match a.kind {
                0 => println!("    act[{ai}]: Message team={} text={:?}", a.team, a.text),
                1 => println!(
                    "    act[{ai}]: Diplomacy team={} other_team={} hostile={}",
                    a.team, a.other_team, a.hostile
                ),
                2 => println!(
                    "    act[{ai}]: SpawnUnit team={} alias={} count={} pos=({}, {}) dest={:?}",
                    a.team, a.alias, a.count, a.x, a.z, a.move_destination
                ),
                3 => println!("    act[{ai}]: Victory"),
                4 => println!("    act[{ai}]: Defeat"),
                other => println!("    act[{ai}]: Unknown({other})"),
            }
        }
    }
}

#[test]
#[ignore = "needs AR_STORY_MAP_DIR pointing at a scratch working copy of ENDL_005"]
fn build_the_last_granary_story_map() {
    let dir = PathBuf::from(
        std::env::var("AR_STORY_MAP_DIR").expect("set AR_STORY_MAP_DIR to a working copy"),
    );
    assert!(
        !dir.to_string_lossy().to_ascii_lowercase().contains("program files"),
        "refusing to save into an installed game directory"
    );

    let session = load_map(&dir).expect("load working copy");
    let mut app = EditorApp::initial_state();
    app.set_loaded_map(dir.clone(), session).expect("bind map");
    app.map_path_input = dir.display().to_string();
    app.scenario.load_session(&dir).expect("load scenario");

    let (base_x, base_z) = app
        .player_main_house_position()
        .expect("ENDL_005 must have a team 0 main house");
    let base_dest = WorldPosition {
        x: base_x.round() as i32,
        z: base_z.round() as i32,
    };
    println!(
        "Player main house at ({}, {}), height={:?}",
        base_dest.x,
        base_dest.z,
        app.height_at_world(base_dest.x as f32, base_dest.z as f32)
    );

    let main_house_id = app
        .scenario
        .document()
        .spawns
        .iter()
        .find(|s| s.team == 0 && MapDesignValidator::is_main_house(&s.alias))
        .map(|s| s.id)
        .expect("ENDL_005 must have a team 0 main house spawn");

    let player_infantry_id = app
        .scenario
        .document()
        .spawns
        .iter()
        .find(|s| s.team == 0 && MapDesignValidator::is_unit_alias(&s.alias))
        .map(|s| s.id)
        .expect("ENDL_005 must have a team 0 unit spawn");

    let (spawn_x, spawn_z) = find_edge_spawn_point(&app, base_x, base_z);
    println!(
        "Selected enemy edge spawn at ({}, {}), height={:?}",
        spawn_x,
        spawn_z,
        app.height_at_world(spawn_x, spawn_z)
    );

    let (scout_cx, scout_cz) = find_center_scout_point(&app);
    let (min_x, max_x) = (scout_cx - 256, scout_cx + 256);
    let (min_z, max_z) = (scout_cz - 256, scout_cz + 256);
    println!(
        "Selected center scout rectangle: ({min_x}, {min_z})..=({max_x}, {max_z})"
    );

    // 清除既有測試事件，透過編輯器公開 API 建立「The Last Granary」故事事件。
    let story_events = vec![
        Event {
            name: "Story_01_Briefing_And_Hostility".into(),
            delay_seconds: 0,
            repeat: false,
            enabled: true,
            actions: vec![
                Action {
                    kind: 0,
                    team: 0,
                    text: "The Last Granary: Hold the village for 8 minutes until reinforcements arrive! Defend your main house.".into(),
                    ..Default::default()
                },
                Action {
                    kind: 1,
                    team: 0,
                    other_team: 1,
                    hostile: true,
                    ..Default::default()
                },
            ],
            conditions: vec![],
        },
        Event {
            name: "Story_02_Wave_1".into(),
            delay_seconds: 60,
            repeat: false,
            enabled: true,
            actions: vec![
                Action {
                    kind: 0,
                    team: 0,
                    text: "Warning: A small enemy raiding party is approaching from the northwest!".into(),
                    ..Default::default()
                },
                Action {
                    kind: 2,
                    team: 1,
                    alias: "HUN_KAVINF00".into(),
                    x: spawn_x,
                    z: spawn_z,
                    count: 5,
                    move_destination: Some(base_dest.clone()),
                    ..Default::default()
                },
            ],
            conditions: vec![],
        },
        Event {
            name: "Story_03_Wave_2".into(),
            delay_seconds: 180,
            repeat: false,
            enabled: true,
            actions: vec![
                Action {
                    kind: 0,
                    team: 0,
                    text: "Warning: A stronger enemy force with horsemen and archers is marching on the village!".into(),
                    ..Default::default()
                },
                Action {
                    kind: 2,
                    team: 1,
                    alias: "HUN_KAVINF00".into(),
                    x: spawn_x,
                    z: spawn_z,
                    count: 6,
                    move_destination: Some(base_dest.clone()),
                    ..Default::default()
                },
                Action {
                    kind: 2,
                    team: 1,
                    alias: "HUN_KAVSCH00".into(),
                    x: spawn_x,
                    z: spawn_z,
                    count: 3,
                    move_destination: Some(base_dest.clone()),
                    ..Default::default()
                },
            ],
            conditions: vec![],
        },
        Event {
            name: "Story_04_Wave_3".into(),
            delay_seconds: 300,
            repeat: false,
            enabled: true,
            actions: vec![
                Action {
                    kind: 0,
                    team: 0,
                    text: "Warning: The main enemy warband has arrived! Hold the granary!".into(),
                    ..Default::default()
                },
                Action {
                    kind: 2,
                    team: 1,
                    alias: "HUN_KAVINF00".into(),
                    x: spawn_x,
                    z: spawn_z,
                    count: 8,
                    move_destination: Some(base_dest.clone()),
                    ..Default::default()
                },
                Action {
                    kind: 2,
                    team: 1,
                    alias: "HUN_KAVSCH00".into(),
                    x: spawn_x,
                    z: spawn_z,
                    count: 5,
                    move_destination: Some(base_dest),
                    ..Default::default()
                },
            ],
            conditions: vec![],
        },
        Event {
            name: "Story_05_Center_Scout_Hint".into(),
            delay_seconds: 0,
            repeat: false,
            enabled: true,
            actions: vec![Action {
                kind: 0,
                team: 0,
                text: "Scouts report enemy riders gathering beyond the central ridge. Watch the northwest approach!".into(),
                ..Default::default()
            }],
            conditions: vec![Condition {
                kind: 2,
                target_id: player_infantry_id,
                min_x,
                min_z,
                max_x,
                max_z,
            }],
        },
    ];

    app.scenario
        .replace_events(story_events)
        .expect("replace story events");

    assert!(
        app.scenario
            .add_objective(Some(480), None, 0)
            .expect("add 8:00 survival victory"),
        "survival victory should be added"
    );
    assert!(
        app.scenario
            .add_objective(None, Some(main_house_id), 0)
            .expect("add main house destroyed defeat"),
        "main house defeat should be added"
    );

    // 執行 Design Check，確認 0 errors。
    app.run_design_check();
    let report = app
        .design_check
        .report
        .as_ref()
        .expect("design check report");
    let extra_errors = app
        .design_check
        .extra_issues
        .iter()
        .filter(|i| i.severity == MapIssueSeverity::Error)
        .count();
    println!(
        "Design Check: {} errors, {} warnings (extra_errors={})",
        report.error_count(),
        report.warning_count(),
        extra_errors
    );
    for row in report_rows(report, &app.design_check.extra_issues, AppLanguage::English) {
        println!(
            "  [{:?}] {}: {}",
            row.severity, row.code, row.english
        );
    }
    assert_eq!(report.error_count(), 0, "design check reported errors");
    assert_eq!(extra_errors, 0, "design check reported extra errors");

    // Safe Save
    app.safe_save_current();
    println!("status: {}", app.status_message);
    assert!(!app.status_is_error, "{}", app.status_message);
    assert!(!app.has_unsaved_changes(), "{}", app.status_message);

    // 重新載入驗證
    let reloaded = load_map(&dir).expect("reload saved map");
    let mut again = EditorApp::initial_state();
    again.set_loaded_map(dir.clone(), reloaded).expect("rebind saved map");
    again.map_path_input = dir.display().to_string();
    again.scenario.load_session(&dir).expect("reload scenario");
    assert!(!again.has_unsaved_changes());

    let events = &again.scenario.document().events;
    assert_eq!(events.len(), 7, "expected 7 story events");
    print_events("The Last Granary", events);
}
