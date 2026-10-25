//! Headless tests for Ready to Test Checklist, Subsystem Change Summary, and Test Instructions.

use against_rome_ambience::Language as AppLanguage;
use against_rome_editor_app::{
    ready_to_test_ui::SubsystemState,
    EditorApp,
};
use against_rome_scenario::{Action, BciImage, Condition, Event, Spawn, WorldPosition};
use eframe::egui;
use std::path::PathBuf;

struct TempMapFixture {
    path: PathBuf,
}

impl TempMapFixture {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("arm_ready_test_{}_{}", name, uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(path.join("SCRIPT")).unwrap();
        std::fs::write(path.join("SCRIPT/ak_level.bci"), BciImage::idle().serialize()).unwrap();
        Self { path }
    }
}

impl Drop for TempMapFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn test_readiness_evaluation_with_objectives_and_waves() {
    let fixture = TempMapFixture::new("objectives_waves");
    let mut app = EditorApp::default();
    app.scenario.load_session(&fixture.path).unwrap();

    let scout_id = uuid::Uuid::new_v4();
    app.scenario
        .apply_spawns(vec![Spawn {
            id: scout_id,
            alias: "GER_INF01".into(),
            team: 0,
            count: 5,
            x: 2200.0,
            z: 2200.0,
            ..Default::default()
        }])
        .unwrap();

    // 建立具備勝負條件、進攻波次與區域條件之劇本事件
    let wave_event = Event {
        name: "Wave 1 Attack".into(),
        enabled: true,
        delay_seconds: 60,
        actions: vec![Action {
            kind: 2, // SpawnUnit
            team: 1,
            alias: "GER_INF01".into(),
            count: 12,
            move_destination: Some(WorldPosition { x: 4000, z: 4000 }),
            ..Default::default()
        }],
        ..Default::default()
    };

    let victory_event = Event {
        name: "Victory Condition".into(),
        enabled: true,
        delay_seconds: 300,
        actions: vec![Action {
            kind: 3, // Victory
            ..Default::default()
        }],
        ..Default::default()
    };

    let defeat_event = Event {
        name: "Defeat Condition".into(),
        enabled: true,
        delay_seconds: 1,
        actions: vec![Action {
            kind: 4, // Defeat
            ..Default::default()
        }],
        ..Default::default()
    };

    let area_event = Event {
        name: "Ambush Area".into(),
        enabled: true,
        delay_seconds: 10,
        conditions: vec![Condition {
            kind: 2, // Area rect
            target_id: scout_id,
            min_x: 2000,
            min_z: 2000,
            max_x: 2500,
            max_z: 2500,
        }],
        actions: vec![Action {
            kind: 1, // Message
            text: "Enemy ambush!".into(),
            ..Default::default()
        }],
        ..Default::default()
    };

    app.scenario
        .add_events(&[wave_event, victory_event, defeat_event, area_event])
        .unwrap();

    let report = app.evaluate_test_readiness();
    assert!(report.has_victory, "Victory condition must be detected");
    assert!(report.has_defeat, "Defeat condition must be detected");
    assert_eq!(report.total_waves, 1, "One attack wave should be detected");
    assert_eq!(report.waves_with_destination, 1, "Wave destination is set");
}

#[test]
fn test_subsystem_change_summary_distinguishes_draft_and_modified() {
    let mut app = EditorApp::default();
    assert_eq!(app.count_modified_subsystems(), 0);

    // 1. 未套用草稿 (PendingDraft)
    app.landscape.mark_dirty();
    let changes = app.subsystem_change_summary();
    let landscape = changes.iter().find(|c| c.id == "landscape").unwrap();
    assert_eq!(landscape.state, SubsystemState::PendingDraft);
    assert!(landscape.details_zh.contains("未套用"));
    assert!(landscape.details_en.contains("Pending"));

    // 2. 未儲存變更 (UnsavedChanges)
    app.ambience.mark_dirty();
    let changes2 = app.subsystem_change_summary();
    let ambience = changes2.iter().find(|c| c.id == "ambience").unwrap();
    assert_eq!(ambience.state, SubsystemState::UnsavedChanges);
    assert!(ambience.details_zh.contains("已更動"));
    assert!(ambience.details_en.contains("modified"));

    assert_eq!(app.count_modified_subsystems(), 2);
}

#[test]
fn test_generate_and_copy_test_instructions_contains_map_details() {
    let fixture = TempMapFixture::new("instructions");
    let mut app = EditorApp::default();
    app.scenario.load_session(&fixture.path).unwrap();
    app.map_path_input = "MAPS/ENDL_005".into();

    let scout = uuid::Uuid::new_v4();
    app.scenario
        .apply_spawns(vec![Spawn {
            id: scout,
            alias: "GER_INF01".into(),
            team: 0,
            count: 3,
            x: 1200.0,
            z: 2200.0,
            ..Default::default()
        }])
        .unwrap();

    // 新增敵方波次與區域觸發
    let wave = Event {
        name: "Cavalry Raid".into(),
        enabled: true,
        delay_seconds: 120,
        actions: vec![Action {
            kind: 2,
            team: 2,
            alias: "GER_INF01".into(),
            count: 8,
            move_destination: Some(WorldPosition { x: 5000, z: 5000 }),
            ..Default::default()
        }],
        ..Default::default()
    };

    let area = Event {
        name: "River Crossing Alert".into(),
        enabled: true,
        delay_seconds: 5,
        conditions: vec![Condition {
            kind: 2,
            target_id: scout,
            min_x: 1024,
            min_z: 2048,
            max_x: 1536,
            max_z: 2560,
        }],
        actions: vec![Action {
            kind: 1,
            text: "Crossing alert!".into(),
            ..Default::default()
        }],
        ..Default::default()
    };

    app.scenario.add_events(&[wave, area]).unwrap();

    // 驗證中文指引格式
    let zh = app.generate_test_instructions(AppLanguage::TraditionalChinese);
    assert!(zh.contains("# 遊戲內驗收清單：ENDL_005"));
    assert!(zh.contains("MAPS/ENDL_005"));
    assert!(zh.contains("%TEMP%"));
    assert!(zh.contains("Cavalry Raid"));
    assert!(zh.contains("2:00"));
    assert!(zh.contains("River Crossing Alert"));
    assert!(zh.contains("1024, 2048 ~ 1536, 2560"));
    assert!(zh.contains("| A. 進攻波次 | [ ] 通過 / [ ] 失敗 | |"));

    // 驗證英文指引格式
    let en = app.generate_test_instructions(AppLanguage::English);
    assert!(en.contains("# In-Game Test Checklist: ENDL_005"));
    assert!(en.contains("Prerequisites"));
    assert!(en.contains("Check A: Attack Waves"));
    assert!(en.contains("Cavalry Raid"));
    assert!(en.contains("River Crossing Alert"));
    assert!(en.contains("| A. Attack Waves | [ ] Pass / [ ] Fail | |"));

    // 測試複製至剪貼簿
    let ctx = egui::Context::default();
    app.copy_test_instructions(&ctx);
    let copied = ctx.output(|o| o.copied_text.clone());
    assert_eq!(copied, app.ready_to_test.last_copied_instructions.unwrap());
}

#[test]
fn test_ready_to_test_window_headless_bilingual_renders_cleanly() {
    let mut app = EditorApp::default();
    app.ready_to_test.open = true;
    app.ready_to_test.show_instructions_preview = true;
    let ctx = egui::Context::default();

    // 測試中文介面無崩潰
    app.set_app_language(AppLanguage::TraditionalChinese);
    let _ = ctx.run(Default::default(), |ctx| {
        app.show_ready_to_test(ctx);
    });
    assert!(app.ready_to_test.open);
    assert!(app.ready_to_test.cached_readiness.is_some());

    // 測試英文介面無崩潰
    app.set_app_language(AppLanguage::English);
    let _ = ctx.run(Default::default(), |ctx| {
        app.show_ready_to_test(ctx);
    });
    assert!(app.ready_to_test.open);
}
