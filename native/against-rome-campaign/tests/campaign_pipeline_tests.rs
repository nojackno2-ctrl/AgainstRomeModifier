use against_rome_campaign::*;
use against_rome_scenario::{
    Action, BciImage, DataBindingSource, DataSlot, Event, ScenarioDocument, Spawn,
};
use uuid::Uuid;

fn make_test_scenario(prebuilt_count: usize, active_uid: u32) -> ScenarioDocument {
    let mut doc = ScenarioDocument::default();
    for i in 0..prebuilt_count {
        let spawn_id = Uuid::new_v4();
        doc.spawns.push(Spawn {
            id: spawn_id,
            alias: "HOUSE".to_string(),
            x: 1000.0 + (i as f32) * 50.0,
            z: 2000.0 + (i as f32) * 50.0,
            team: 0,
            count: 0,
            angle: 0,
            y: 0.0,
            prebuilt: true,
            raw_rotation: None,
        });
        doc.data_slots.push(DataSlot {
            slot: i as i32 + 10,
            uid: active_uid + (i as u32),
            spawn_id,
        });
    }
    doc
}

fn make_test_data_file(active_slots: &[(i32, u32)]) -> Vec<u8> {
    let count = 100;
    let mut bytes = vec![0; 16 + count * 121];
    for (offset, value) in [(0, 1u32), (4, count as u32), (8, 30), (12, 30)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    for &(slot, uid) in active_slots {
        let s = slot as usize;
        bytes[16 + s * 79] = 1; // active
        bytes[19 + s * 79..23 + s * 79].copy_from_slice(&uid.to_le_bytes());
    }
    bytes
}

#[test]
fn test_unified_campaign_pipeline_compiles_all_components() {
    let aliases = vec![
        "HOUSE".to_string(),
        "GER_INF00".to_string(),
        "HUN_KAVINF00".to_string(),
    ];

    let target_boss_id = Uuid::new_v4();
    let mut scenario = make_test_scenario(1, 1001);
    scenario.spawns.push(Spawn {
        id: target_boss_id,
        alias: "GER_INF00".to_string(),
        x: 3000.0,
        z: 3000.0,
        team: 1,
        count: 5,
        angle: 45,
        y: 0.0,
        prebuilt: false,
        raw_rotation: None,
    });

    // 過場完結連結目標必須存在，否則管線會拒絕（不再產生懸空連結）。
    scenario.events.push(Event {
        name: "StartBattleEvent".into(),
        delay_seconds: 60,
        actions: vec![Action {
            kind: 0,
            text: "The battle begins".into(),
            ..Default::default()
        }],
        ..Default::default()
    });

    // 1. Mission plan
    let plan = CampaignMissionPlan {
        title: "GrandCampaign".into(),
        briefing: "Welcome to the grand battle!".into(),
        waves: vec![WaveAttackDefinition::new(
            1,
            60,
            "Enemy assault wave 1!",
            4000.0,
            4000.0,
            vec![WaveSquadDefinition::new("HUN_KAVINF00", 6, 2)],
        )],
        reinforcements: vec![ReinforcementDefinition::new(
            Uuid::new_v4(),
            "GuardReinforcement",
            90,
            0,
            1500.0,
            1500.0,
            vec![WaveSquadDefinition::new("GER_INF00", 10, 0)],
        )],
        objectives: vec![CampaignObjective::new(
            Uuid::new_v4(),
            CampaignObjectiveType::DestroyTarget,
            "EliminateBoss",
            "Defeat the enemy champion",
        )
        .with_target(target_boss_id)],
        ..Default::default()
    };

    // 2. Objective Graph
    let mut graph = ObjectiveDependencyGraph::new();
    let obj = ObjectiveDefinition {
        id: Uuid::new_v4(),
        title: "DefendTown".into(),
        description: "Keep our village standing".into(),
        kind: ObjectiveKind::Survival,
        category: ObjectiveCategory::Primary,
        parameters: ObjectiveRuleParameters {
            target_guids: vec![scenario.spawns[0].id],
            hold_duration_seconds: 600,
            ..Default::default()
        },
        reward: None,
        initial_state: ObjectiveState::Active,
        ui_order: 1.0,
    };
    graph.add_objective(obj).unwrap();

    // 3. Cutscene Sequence
    let cutscenes = vec![CutsceneSequence {
        name: "PrologueCinema".into(),
        camera_waypoints: vec![
            CameraWaypoint::new(
                [1100.0, 15.0, 2100.0],
                30.0,
                45.0,
                60.0,
                0.0,
                CameraEasingType::Linear,
                None,
            ),
            CameraWaypoint::new(
                [1800.0, 25.0, 2800.0],
                25.0,
                60.0,
                70.0,
                8.0,
                CameraEasingType::SmoothStep,
                None,
            ),
        ],
        subtitles: vec![SubtitleKeyframe::new(
            1.0,
            4.0,
            "Elder",
            "They are coming from the steppes...",
        )],
        on_complete_trigger_event: Some("StartBattleEvent".into()),
        ..Default::default()
    }];

    // 4. DataBindingSource
    let temp_dir = std::env::temp_dir().join(format!("arm-campaign-pipeline-{}", Uuid::new_v4()));
    std::fs::create_dir_all(temp_dir.join("DATA")).unwrap();
    let data_file = temp_dir.join("DATA/objects.dat");
    std::fs::write(&data_file, make_test_data_file(&[(10, 1001)])).unwrap();
    let data_source = DataBindingSource::load(&temp_dir).unwrap();

    let options = CampaignPipelineOptions {
        mission_plan: Some(&plan),
        objective_graph: Some(&graph),
        cutscenes: Some(&cutscenes),
        start_camera: None, // Will be extracted from cutscenes[0]
        scenario: &scenario,
        known_aliases: &aliases,
        data_source: Some(&data_source),
    };

    // Compile pipeline
    let result = CampaignCompiler::compile(options.clone());
    assert!(result.success, "Diagnostics: {:?}", result.diagnostics);
    assert!(!result.compiled_events.is_empty());
    assert!(result.resolved_camera.is_some());
    let resolved_cam = result.resolved_camera.unwrap();
    assert_eq!(resolved_cam.x, 1100.0);
    assert_eq!(resolved_cam.z, 2100.0);

    // Compile to BCI bytecode
    let idle_bci = BciImage::idle().serialize();
    let bci_result = CampaignCompiler::compile_to_bci(options, &idle_bci);
    assert!(bci_result.success, "Diagnostics: {:?}", bci_result.diagnostics);
    assert!(bci_result.bci_bytes.is_some());
    let bytes = bci_result.bci_bytes.unwrap();
    let parsed = BciImage::parse(&bytes);
    assert!(parsed.is_ok(), "Parsed BCI must be valid: {:?}", parsed.err());

    std::fs::remove_dir_all(temp_dir).unwrap();
}

#[test]
fn test_event_name_deduplication_appends_suffixes() {
    let aliases = vec!["HOUSE".to_string(), "GER_INF00".to_string()];
    let mut scenario = make_test_scenario(1, 2000);
    scenario.events.push(Event {
        name: "BattleIntro_字幕_1".into(),
        delay_seconds: 5,
        repeat: false,
        enabled: true,
        actions: vec![Action {
            kind: 0,
            text: "Existing event".into(),
            ..Default::default()
        }],
        conditions: vec![],
    });

    let cutscene = CutsceneSequence {
        name: "BattleIntro".into(),
        subtitles: vec![
            SubtitleKeyframe::new(0.0, 2.0, "Speaker", "First line"),
            SubtitleKeyframe::new(3.0, 2.0, "Speaker", "Second line"),
        ],
        ..Default::default()
    };

    let options = CampaignPipelineOptions {
        mission_plan: None,
        objective_graph: None,
        cutscenes: Some(&[cutscene]),
        start_camera: None,
        scenario: &scenario,
        known_aliases: &aliases,
        data_source: None,
    };

    let result = CampaignCompiler::compile(options);
    assert!(result.success);
    // Colliding name "BattleIntro_字幕_1" should be renamed to "BattleIntro_字幕_1_2"
    let names: Vec<String> = result
        .scenario_document
        .events
        .iter()
        .map(|e| e.name.clone())
        .collect();
    assert!(names.contains(&"BattleIntro_字幕_1".to_string()));
    assert!(names.contains(&"BattleIntro_字幕_1_2".to_string()));
    assert!(names.contains(&"BattleIntro_字幕_2".to_string()));
}

#[test]
fn test_max_events_cap_enforced() {
    let aliases = vec!["HOUSE".to_string()];
    let mut scenario = ScenarioDocument::default();
    // Fill scenario with 255 events
    for i in 0..255 {
        scenario.events.push(Event {
            name: format!("Event_{i}"),
            delay_seconds: 1,
            repeat: false,
            enabled: true,
            actions: vec![Action {
                kind: 0,
                text: "Hello".into(),
                ..Default::default()
            }],
            conditions: vec![],
        });
    }

    // Adding cutscene with 2 subtitles will exceed 256
    let cutscene = CutsceneSequence {
        name: "ExceedCap".into(),
        subtitles: vec![
            SubtitleKeyframe::new(0.0, 1.0, "", "1"),
            SubtitleKeyframe::new(2.0, 1.0, "", "2"),
        ],
        ..Default::default()
    };

    let options = CampaignPipelineOptions {
        mission_plan: None,
        objective_graph: None,
        cutscenes: Some(&[cutscene]),
        start_camera: None,
        scenario: &scenario,
        known_aliases: &aliases,
        data_source: None,
    };

    let result = CampaignCompiler::compile(options);
    assert!(!result.success);
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.contains("256-event limit")));
}

#[test]
fn test_dangling_cutscene_link_is_rejected() {
    let aliases = vec!["HOUSE".to_string()];
    let scenario = ScenarioDocument::default();
    let cutscene = CutsceneSequence {
        name: "Linked".into(),
        subtitles: vec![SubtitleKeyframe::new(0.0, 2.0, "", "Line")],
        on_complete_trigger_event: Some("MissingEvent".into()),
        ..Default::default()
    };
    let options = CampaignPipelineOptions {
        mission_plan: None,
        objective_graph: None,
        cutscenes: Some(&[cutscene]),
        start_camera: None,
        scenario: &scenario,
        known_aliases: &aliases,
        data_source: None,
    };
    let result = CampaignCompiler::compile(options);
    assert!(!result.success);
    assert!(result.diagnostics.iter().any(|d| d.contains("MissingEvent")));
    assert!(result.compiled_events.is_empty());
}
