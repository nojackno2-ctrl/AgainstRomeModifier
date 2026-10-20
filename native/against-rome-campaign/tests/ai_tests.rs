use against_rome_campaign::*;
use against_rome_scenario::{DataSlot, ScenarioDocument, Spawn};
use uuid::Uuid;

#[test]
fn test_ai_archetype_catalog_contains_standard_four_archetypes() {
    let all = AiArchetypeCatalog::all();
    assert!(all.len() >= 4);

    let nomadic = AiArchetypeCatalog::get(AiArchetypeCatalog::ID_NOMADIC_RAIDER).unwrap();
    assert_eq!(nomadic.preferred_tribe, "Hun");
    assert!(nomadic.aggressiveness >= 0.8);
    assert_eq!(nomadic.tactical_posture, TacticalPosture::HitAndRun);

    let roman = AiArchetypeCatalog::get(AiArchetypeCatalog::ID_ROMAN_FORTRESS).unwrap();
    assert_eq!(roman.preferred_tribe, "Roman");
    assert!(roman.defense_priority >= 0.9);
    assert_eq!(roman.tactical_posture, TacticalPosture::DefensiveHold);

    let germanic = AiArchetypeCatalog::get(AiArchetypeCatalog::ID_GERMANIC_SETTLEMENT).unwrap();
    assert_eq!(germanic.preferred_tribe, "German");
    assert!(germanic.economic_focus >= 0.8);

    let outpost = AiArchetypeCatalog::get(AiArchetypeCatalog::ID_BARBARIAN_OUTPOST).unwrap();
    assert_eq!(outpost.engagement_rule, EngagementRule::DefendTerritory);
}

#[test]
fn test_waypoint_path_planner_calculates_distances_and_progression() {
    let nodes = vec![
        WaypointNode::new(Uuid::new_v4(), 0, 1000.0, 1000.0),
        WaypointNode::new(Uuid::new_v4(), 1, 1000.0, 2000.0),
        WaypointNode::new(Uuid::new_v4(), 2, 2000.0, 2000.0),
    ];

    let ping_pong_path = WaypointPath::new(
        Uuid::new_v4(),
        "TestPingPong",
        WaypointMovementMode::PingPong,
        nodes,
    );
    ping_pong_path.validate().unwrap();

    let dist = WaypointPathPlanner::calculate_total_distance(&ping_pong_path);
    assert!((dist - 2000.0).abs() < 1e-3);

    let mut is_reversing = false;
    let mut next =
        WaypointPathPlanner::get_next_node_index(&ping_pong_path, 0, &mut is_reversing);
    assert_eq!(next, 1);
    assert!(!is_reversing);

    next = WaypointPathPlanner::get_next_node_index(&ping_pong_path, 1, &mut is_reversing);
    assert_eq!(next, 2);
    assert!(!is_reversing);

    // At end: should reverse
    next = WaypointPathPlanner::get_next_node_index(&ping_pong_path, 2, &mut is_reversing);
    assert_eq!(next, 1);
    assert!(is_reversing);

    // At start while reversing: should turn forward
    next = WaypointPathPlanner::get_next_node_index(&ping_pong_path, 0, &mut is_reversing);
    assert_eq!(next, 1);
    assert!(!is_reversing);
}

#[test]
fn test_waypoint_path_planner_loop_progression_cycles() {
    let nodes = vec![
        WaypointNode::new(Uuid::new_v4(), 0, 100.0, 100.0),
        WaypointNode::new(Uuid::new_v4(), 1, 100.0, 200.0),
        WaypointNode::new(Uuid::new_v4(), 2, 200.0, 100.0),
    ];

    let loop_path = WaypointPath::new(
        Uuid::new_v4(),
        "TestLoop",
        WaypointMovementMode::Loop,
        nodes,
    );
    loop_path.validate().unwrap();

    let mut reversing = false;
    assert_eq!(
        WaypointPathPlanner::get_next_node_index(&loop_path, 0, &mut reversing),
        1
    );
    assert_eq!(
        WaypointPathPlanner::get_next_node_index(&loop_path, 1, &mut reversing),
        2
    );
    assert_eq!(
        WaypointPathPlanner::get_next_node_index(&loop_path, 2, &mut reversing),
        0
    );
}

#[test]
fn test_campaign_mission_compiler_compiles_objectives_and_waves() {
    let known_aliases = vec![
        "GER_INF00".to_string(),
        "ROM_INF01".to_string(),
        "HUN_KAVINF00".to_string(),
        "HUN_KAVSCH00".to_string(),
        "GER_HAU00".to_string(),
    ];

    let target_building_id = Uuid::new_v4();
    let target_general_id = Uuid::new_v4();

    let mut plan = CampaignMissionPlan::new(
        "Siege of Aquileia",
        "Protect our town center and withstand the Hun attacks!",
    );

    plan.objectives = vec![
        CampaignObjective::new(
            Uuid::new_v4(),
            CampaignObjectiveType::DefendTarget,
            "保護城鎮中心",
            "城鎮中心不可被毀",
        )
        .with_target(target_building_id),
        CampaignObjective::new(
            Uuid::new_v4(),
            CampaignObjectiveType::DestroyTarget,
            "斬首敵軍酋長",
            "擊殺敵軍騎兵統帥",
        )
        .with_target(target_general_id),
        CampaignObjective::new(
            Uuid::new_v4(),
            CampaignObjectiveType::SurviveTime,
            "堅守至黎明",
            "生存 1200 秒",
        )
        .with_seconds(1200),
    ];

    plan.waves = vec![
        WaveAttackDefinition::new(
            1,
            60,
            "Enemy vanguard approaching!",
            2000.0,
            2000.0,
            vec![WaveSquadDefinition::new("HUN_KAVINF00", 10, 2)],
        ),
        WaveAttackDefinition::new(
            2,
            180,
            "The second enemy wave is approaching!",
            2100.0,
            2100.0,
            vec![WaveSquadDefinition::new("HUN_KAVSCH00", 12, 2)],
        ),
    ];

    plan.reinforcements = vec![ReinforcementDefinition::new(
        Uuid::new_v4(),
        "羅馬同盟軍援軍",
        300,
        0,
        500.0,
        500.0,
        vec![WaveSquadDefinition::new("ROM_INF01", 15, 0)],
    )];

    plan.faction_profiles =
        vec![FactionAiProfile::new(2, AiArchetypeCatalog::ID_NOMADIC_RAIDER)];

    let mut scenario = ScenarioDocument::default();
    scenario.spawns = vec![
        Spawn {
            id: target_building_id,
            alias: "GER_HAU00".to_string(),
            x: 5000.0,
            z: 5000.0,
            team: 0,
            count: 0,
            prebuilt: true,
            ..Default::default()
        },
        Spawn {
            id: target_general_id,
            alias: "HUN_KAVINF00".to_string(),
            x: 3000.0,
            z: 3000.0,
            team: 2,
            count: 1,
            prebuilt: false,
            ..Default::default()
        },
    ];
    scenario.data_slots = vec![DataSlot {
        slot: 10,
        uid: 5001,
        spawn_id: target_building_id,
    }];

    let result = CampaignMissionCompiler::compile(&plan, None, &known_aliases, Some(&scenario));
    assert!(result.success, "Diagnostics: {:?}", result.diagnostics);
    assert!(!result.compiled_events.is_empty());

    // Verify Briefing event
    assert!(result
        .compiled_events
        .iter()
        .any(|e| e.name.starts_with("MissionBriefing")));

    // Verify Wave events
    assert!(result
        .compiled_events
        .iter()
        .any(|e| e.name.starts_with("Wave_01") && e.delay_seconds == 60));
    assert!(result
        .compiled_events
        .iter()
        .any(|e| e.name.starts_with("Wave_02") && e.delay_seconds == 180));

    // Verify Reinforcement event
    assert!(result
        .compiled_events
        .iter()
        .any(|e| e.name.starts_with("Reinf_") && e.delay_seconds == 300));

    // Verify DefendFail event (Defeat action, kind 4)
    let defend_fail = result
        .compiled_events
        .iter()
        .find(|e| e.name.starts_with("DefendFail_"))
        .unwrap();
    assert!(defend_fail.actions.iter().any(|a| a.kind == 4));

    // Verify DestroyWin event (Victory action, kind 3)
    let destroy_win = result
        .compiled_events
        .iter()
        .find(|e| e.name.starts_with("DestroyWin_"))
        .unwrap();
    assert!(destroy_win.actions.iter().any(|a| a.kind == 3));

    // Verify SurviveWin event (Victory action, kind 3)
    let survive_win = result
        .compiled_events
        .iter()
        .find(|e| e.name.starts_with("SurviveWin_"))
        .unwrap();
    assert_eq!(survive_win.delay_seconds, 1200);
    assert!(survive_win.actions.iter().any(|a| a.kind == 3));
}

#[test]
fn test_campaign_mission_compiler_unsupported_briefing_returns_failure() {
    let plan = CampaignMissionPlan::new("Campaign", "中文簡報");
    let result = CampaignMissionCompiler::compile(&plan, None, &[], None);
    assert!(!result.success);
    assert!(result.diagnostics.iter().any(|d| d.starts_with("[Error]")));
}

#[test]
fn test_faction_ai_session_tracks_dirty_and_maintains_baseline() {
    let mut session = FactionAiSession::default();
    assert!(!session.is_dirty());

    let nodes = vec![
        WaypointNode::new(Uuid::new_v4(), 0, 100.0, 100.0),
        WaypointNode::new(Uuid::new_v4(), 1, 200.0, 200.0),
    ];
    let path = WaypointPath::new(Uuid::new_v4(), "Patrol Alpha", WaypointMovementMode::Loop, nodes);
    session.add_waypoint_path(path).unwrap();
    assert!(session.is_dirty());

    session.accept_changes();
    assert!(!session.is_dirty());
}

#[test]
fn test_campaign_mission_compiler_compile_to_bci() {
    let known_aliases = vec!["GER_INF00".to_string(), "HUN_KAVINF00".to_string()];
    let plan = CampaignMissionPlan {
        title: "FrontierSkirmish".to_string(),
        briefing: "Hold the line against incoming raiders!".to_string(),
        waves: vec![
            WaveAttackDefinition::new(
                1,
                30,
                "First wave arriving!",
                2000.0,
                2000.0,
                vec![WaveSquadDefinition::new("HUN_KAVINF00", 5, 1)],
            ),
        ],
        reinforcements: vec![
            ReinforcementDefinition::new(
                Uuid::new_v4(),
                "AuxiliarySupport",
                45,
                0,
                1000.0,
                1000.0,
                vec![WaveSquadDefinition::new("GER_INF00", 8, 0)],
            ),
        ],
        objectives: vec![
            CampaignObjective::new(
                Uuid::new_v4(),
                CampaignObjectiveType::SurviveTime,
                "SurviveAssault",
                "Hold for 300 seconds",
            )
            .with_seconds(300),
        ],
        ..Default::default()
    };

    let idle_bci = against_rome_scenario::BciImage::idle().serialize();
    let scenario = ScenarioDocument::default();
    let cam = MapStartCamera {
        x: 1500.0,
        z: 1500.0,
        zoom: 4.0,
        primitive: StartCameraPrimitive::CenterToPos,
        enabled: true,
        ..Default::default()
    };

    let result = CampaignMissionCompiler::compile_to_bci(
        &plan,
        &idle_bci,
        &scenario,
        &known_aliases,
        Some(&cam),
    );
    assert!(result.success, "Diagnostics: {:?}", result.diagnostics);
    assert!(result.bci_bytes.is_some());
    let bci_data = result.bci_bytes.unwrap();
    assert!(bci_data.len() > idle_bci.len());
    let parsed = against_rome_scenario::BciImage::parse(&bci_data);
    assert!(parsed.is_ok());
}

#[test]
fn test_sanitize_name_matches_csharp_unicode_letter_behavior() {
    use against_rome_campaign::ai::compiler::sanitize_name;
    assert_eq!(sanitize_name("保護城鎮中心"), "保護城鎮中心");
    assert_eq!(sanitize_name("Siege of Aquileia!"), "Siege_of_Aquileia");
    assert_eq!(sanitize_name("  "), "Item");
    assert_eq!(sanitize_name(""), "Item");
    // C#: all-symbol text trims to empty (only whitespace falls back to "Item").
    assert_eq!(sanitize_name("!!!"), "");
    // C# truncates to 30 UTF-16 units.
    assert_eq!(sanitize_name(&"字".repeat(40)).chars().count(), 30);
    assert_eq!(sanitize_name(&"a".repeat(40)), "a".repeat(30));
}

#[test]
fn test_compiled_event_names_keep_unicode_objective_titles() {
    let target = Uuid::new_v4();
    let mut plan = CampaignMissionPlan::new("Campaign", "");
    plan.objectives = vec![CampaignObjective::new(
        Uuid::new_v4(),
        CampaignObjectiveType::DestroyTarget,
        "斬首酋長",
        "d",
    )
    .with_target(target)];
    let mut doc = ScenarioDocument::default();
    doc.spawns.push(Spawn {
        id: target,
        alias: "HUN_KAVINF00".into(),
        ..Default::default()
    });
    let result = CampaignMissionCompiler::compile(&plan, None, &["HUN_KAVINF00".to_string()], Some(&doc));
    assert!(result.success, "{:?}", result.diagnostics);
    assert_eq!(result.compiled_events[0].name, "DestroyWin_斬首酋長");
}

#[test]
fn test_wave_plan_parameters_build_plan_and_validation() {
    let mut params = WavePlanParameters::default();
    assert!(params.validate().is_ok());

    // Boundary validation
    params.waves = 0;
    assert!(params.validate().is_err());
    params.waves = 257;
    assert!(params.validate().is_err());
    params.waves = 3;

    params.interval_seconds = 0;
    assert!(params.validate().is_err());
    params.interval_seconds = 30;

    params.count_per_unit_type = 0;
    assert!(params.validate().is_err());
    params.count_per_unit_type = 21;
    assert!(params.validate().is_err());
    params.count_per_unit_type = 5;

    params.team = 8;
    assert!(params.validate().is_err());
    params.team = 1;

    params.spawn_x = 16384.0;
    assert!(params.validate().is_err());
    params.spawn_x = 1000.0;

    params.move_destination = Some((20000, 2000));
    assert!(params.validate().is_err());
    params.move_destination = Some((2000, 3000));
    assert!(params.validate().is_ok());

    let archetype = AiArchetypeCatalog::get(AiArchetypeCatalog::ID_NOMADIC_RAIDER).unwrap();
    let plan = params.build_plan(&archetype);
    assert_eq!(plan.waves.len(), 3);

    // Wave delays: first_delay=120, interval=30 -> 120, 150, 180
    assert_eq!(plan.waves[0].trigger_delay_seconds, 120);
    assert_eq!(plan.waves[1].trigger_delay_seconds, 150);
    assert_eq!(plan.waves[2].trigger_delay_seconds, 180);

    for (i, wave) in plan.waves.iter().enumerate() {
        assert_eq!(wave.wave_index, (i + 1) as i32);
        assert_eq!(wave.spawn_x, 1000.0);
        assert_eq!(wave.spawn_z, 1000.0);
        assert_eq!(
            wave.move_destination,
            Some(against_rome_scenario::WorldPosition {
                x: 2000,
                z: 3000
            })
        );
        assert!(!wave.squads.is_empty());
        for squad in &wave.squads {
            assert_eq!(squad.count, 5);
            assert_eq!(squad.team, 1);
            assert_eq!(squad.relative_delay_seconds, 0);
        }
    }
}

#[test]
fn test_compile_for_merge_success_and_destination() {
    let archetype = AiArchetypeCatalog::get(AiArchetypeCatalog::ID_ROMAN_FORTRESS).unwrap();
    let params = WavePlanParameters {
        archetype_id: AiArchetypeCatalog::ID_ROMAN_FORTRESS.to_string(),
        waves: 2,
        first_delay_seconds: 60,
        interval_seconds: 45,
        count_per_unit_type: 3,
        team: 2,
        spawn_x: 500.0,
        spawn_z: 600.0,
        move_destination: Some((1200, 1400)),
    };
    let plan = params.build_plan(&archetype);

    let mut scenario = ScenarioDocument::default();
    scenario.events.push(against_rome_scenario::Event {
        name: "Intro".into(),
        actions: vec![against_rome_scenario::Action {
            kind: 0,
            text: "Welcome".into(),
            ..Default::default()
        }],
        ..Default::default()
    });

    let known_aliases = vec![
        "ROM_INF00".to_string(),
        "ROM_INF01".to_string(),
        "ROM_SCH00".to_string(),
    ];

    let result = CampaignMissionCompiler::compile_for_merge(&plan, &known_aliases, &scenario);
    assert!(result.success, "Diagnostics: {:?}", result.diagnostics);
    assert_eq!(result.compiled_events.len(), 2);
    assert_eq!(result.compiled_events[0].name, "Wave_01");
    assert_eq!(result.compiled_events[1].name, "Wave_02");

    // Verify actions have correct move_destination
    for ev in &result.compiled_events {
        for action in &ev.actions {
            assert_eq!(
                action.move_destination,
                Some(against_rome_scenario::WorldPosition {
                    x: 1200,
                    z: 1400
                })
            );
            assert_eq!(action.x, 500.0);
            assert_eq!(action.z, 600.0);
            assert_eq!(action.team, 2);
            assert_eq!(action.count, 3);
        }
    }
}

#[test]
fn test_compile_for_merge_renames_event_name_collisions() {
    let archetype = AiArchetypeCatalog::get(AiArchetypeCatalog::ID_BARBARIAN_OUTPOST).unwrap();
    let params = WavePlanParameters {
        archetype_id: AiArchetypeCatalog::ID_BARBARIAN_OUTPOST.to_string(),
        waves: 2,
        first_delay_seconds: 10,
        interval_seconds: 20,
        count_per_unit_type: 1,
        team: 1,
        spawn_x: 100.0,
        spawn_z: 100.0,
        move_destination: None,
    };
    let plan = params.build_plan(&archetype);

    let mut scenario = ScenarioDocument::default();
    // Pre-existing events with collision on "Wave_01" (case-insensitive test) and "Wave_01_2"
    scenario.events.push(against_rome_scenario::Event {
        name: "wave_01".into(),
        actions: vec![against_rome_scenario::Action {
            kind: 0,
            text: "Wave 1".into(),
            ..Default::default()
        }],
        ..Default::default()
    });
    scenario.events.push(against_rome_scenario::Event {
        name: "Wave_01_2".into(),
        actions: vec![against_rome_scenario::Action {
            kind: 0,
            text: "Wave 1_2".into(),
            ..Default::default()
        }],
        ..Default::default()
    });

    let known_aliases = vec!["GER_INF01".to_string(), "GER_SCH00".to_string()];

    let result = CampaignMissionCompiler::compile_for_merge(&plan, &known_aliases, &scenario);
    assert!(result.success, "{:?}", result.diagnostics);
    assert_eq!(result.compiled_events.len(), 2);
    // Wave_01 collides with wave_01 and Wave_01_2, so it should become Wave_01_3
    assert_eq!(result.compiled_events[0].name, "Wave_01_3");
    // Wave_02 does not collide
    assert_eq!(result.compiled_events[1].name, "Wave_02");
}

#[test]
fn test_compile_for_merge_enforces_256_event_limit() {
    let archetype = AiArchetypeCatalog::get(AiArchetypeCatalog::ID_NOMADIC_RAIDER).unwrap();
    let params = WavePlanParameters {
        archetype_id: AiArchetypeCatalog::ID_NOMADIC_RAIDER.to_string(),
        waves: 2,
        first_delay_seconds: 10,
        interval_seconds: 20,
        count_per_unit_type: 1,
        team: 1,
        spawn_x: 100.0,
        spawn_z: 100.0,
        move_destination: None,
    };
    let plan = params.build_plan(&archetype);

    let mut scenario = ScenarioDocument::default();
    // Fill scenario with 255 events
    for i in 0..255 {
        scenario.events.push(against_rome_scenario::Event {
            name: format!("Ev_{i}"),
            ..Default::default()
        });
    }

    let known_aliases = vec!["HUN_KAVINF00".to_string(), "HUN_KAVSCH00".to_string()];

    // 255 + 2 = 257 > 256, should fail
    let result = CampaignMissionCompiler::compile_for_merge(&plan, &known_aliases, &scenario);
    assert!(!result.success);
    assert!(result.compiled_events.is_empty());
    assert!(result.diagnostics.iter().any(|d| d.contains("256")));
}

#[test]
fn test_compile_for_merge_rejects_unsupported_fields() {
    let known_aliases = vec!["ROM_LEGION".to_string()];
    let scenario = ScenarioDocument::default();

    // 1. Faction profiles present
    let mut plan = CampaignMissionPlan::new("Test", "Desc");
    plan.faction_profiles.push(FactionAiProfile::new(
        1,
        AiArchetypeCatalog::ID_ROMAN_FORTRESS,
    ));
    let result = CampaignMissionCompiler::compile_for_merge(&plan, &known_aliases, &scenario);
    assert!(!result.success);
    assert!(result.diagnostics[0].contains("尚不支援勢力 AI"));

    // 2. Wave with assigned_path_id
    let mut plan2 = CampaignMissionPlan::new("Test2", "Desc");
    let mut wave =
        WaveAttackDefinition::new(1, 10, "Wave_01", 100.0, 100.0, vec![]);
    wave.assigned_path_id = Some(Uuid::new_v4());
    plan2.waves.push(wave);
    let result2 = CampaignMissionCompiler::compile_for_merge(&plan2, &known_aliases, &scenario);
    assert!(!result2.success);
    assert!(result2.diagnostics[0].contains("尚不支援勢力 AI"));

    // 3. Wave squad with relative_delay_seconds != 0
    let mut plan3 = CampaignMissionPlan::new("Test3", "Desc");
    let mut wave3 =
        WaveAttackDefinition::new(1, 10, "Wave_01", 100.0, 100.0, vec![]);
    wave3.squads.push(WaveSquadDefinition {
        alias: "ROM_LEGION".to_string(),
        count: 5,
        team: 1,
        relative_delay_seconds: 15,
    });
    plan3.waves.push(wave3);
    let result3 = CampaignMissionCompiler::compile_for_merge(&plan3, &known_aliases, &scenario);
    assert!(!result3.success);
    assert!(result3.diagnostics[0].contains("尚不支援勢力 AI"));

    // 4. Reinforcement with relative_delay_seconds != 0
    let mut plan4 = CampaignMissionPlan::new("Test4", "Desc");
    let mut reinf = ReinforcementDefinition::new(Uuid::new_v4(), "Reinf", 30, 1, 100.0, 100.0, vec![]);
    reinf.squads.push(WaveSquadDefinition {
        alias: "ROM_LEGION".to_string(),
        count: 5,
        team: 1,
        relative_delay_seconds: 10,
    });
    plan4.reinforcements.push(reinf);
    let result4 = CampaignMissionCompiler::compile_for_merge(&plan4, &known_aliases, &scenario);
    assert!(!result4.success);
    assert!(result4.diagnostics[0].contains("尚不支援勢力 AI"));
}

#[test]
fn test_compile_rejects_unsupported_reinforcement_fields() {
    let known_aliases = vec!["ROM_LEGION".to_string()];
    let scenario = ScenarioDocument::default();

    // Reinforcement with assigned_path_id in standard compile()
    let mut plan = CampaignMissionPlan::new("Test", "Desc");
    let mut reinf = ReinforcementDefinition::new(Uuid::new_v4(), "Reinf", 30, 1, 100.0, 100.0, vec![]);
    reinf.assigned_path_id = Some(Uuid::new_v4());
    reinf.squads.push(WaveSquadDefinition {
        alias: "ROM_LEGION".to_string(),
        count: 5,
        team: 1,
        relative_delay_seconds: 0,
    });
    plan.reinforcements.push(reinf);
    let result = CampaignMissionCompiler::compile(&plan, None, &known_aliases, Some(&scenario));
    assert!(!result.success);
    assert!(result.diagnostics.iter().any(|d| d.contains("巡邏路徑")));

    // Reinforcement with relative_delay_seconds != 0 in standard compile()
    let mut plan2 = CampaignMissionPlan::new("Test2", "Desc");
    let mut reinf2 = ReinforcementDefinition::new(Uuid::new_v4(), "Reinf2", 30, 1, 100.0, 100.0, vec![]);
    reinf2.squads.push(WaveSquadDefinition {
        alias: "ROM_LEGION".to_string(),
        count: 5,
        team: 1,
        relative_delay_seconds: 20,
    });
    plan2.reinforcements.push(reinf2);
    let result2 = CampaignMissionCompiler::compile(&plan2, None, &known_aliases, Some(&scenario));
    assert!(!result2.success);
    assert!(result2.diagnostics.iter().any(|d| d.contains("相對延遲")));
}

#[test]
fn test_staggered_reinforcements_fail_closed_parity_and_differential_boundaries() {
    let known_aliases = vec!["GER_INF01".to_string(), "ROM_LEGION".to_string()];
    let scenario = ScenarioDocument::default();

    // 1. Differential match with C# ScenarioWaveOrdersTests.cs:
    // C# line 77: Assert.False(CampaignMissionCompiler.Compile(plan with { Waves = [wave with { Squads = [new("GER_INF01", 10, 1, 1)] }] }, null, Aliases).Success);
    let mut wave_csharp = WaveAttackDefinition::new(1, 0, "", 8000.0, 8000.0, vec![]);
    wave_csharp.squads.push(WaveSquadDefinition {
        alias: "GER_INF01".to_string(),
        count: 10,
        team: 1,
        relative_delay_seconds: 1,
    });
    let mut plan_csharp_wave = CampaignMissionPlan::new("Orders", "");
    plan_csharp_wave.waves.push(wave_csharp);

    let res_wave_compile = CampaignMissionCompiler::compile(&plan_csharp_wave, None, &known_aliases, Some(&scenario));
    assert!(!res_wave_compile.success, "Wave with non-zero relative_delay_seconds must fail in compile()");
    assert!(res_wave_compile.diagnostics.iter().any(|d| d.contains("相對延遲")));

    let res_wave_merge = CampaignMissionCompiler::compile_for_merge(&plan_csharp_wave, &known_aliases, &scenario);
    assert!(!res_wave_merge.success, "Wave with non-zero relative_delay_seconds must fail in compile_for_merge()");
    assert!(res_wave_merge.compiled_events.is_empty(), "Failed merge must maintain atomicity and return empty events");
    assert!(res_wave_merge.diagnostics[0].contains("部隊相對延遲"));

    // 2. Reinforcement staggered wave parity (Reinforcement squads with relative_delay_seconds > 0)
    let mut reinf = ReinforcementDefinition::new(Uuid::new_v4(), "AlliedReinf", 60, 1, 4000.0, 4000.0, vec![]);
    reinf.squads.push(WaveSquadDefinition {
        alias: "ROM_LEGION".to_string(),
        count: 10,
        team: 1,
        relative_delay_seconds: 0,
    });
    reinf.squads.push(WaveSquadDefinition {
        alias: "ROM_LEGION".to_string(),
        count: 5,
        team: 1,
        relative_delay_seconds: 15,
    });
    let mut plan_reinf = CampaignMissionPlan::new("ReinfPlan", "Briefing");
    plan_reinf.reinforcements.push(reinf);

    let res_reinf_compile = CampaignMissionCompiler::compile(&plan_reinf, None, &known_aliases, Some(&scenario));
    assert!(!res_reinf_compile.success, "Reinforcement with non-zero relative_delay_seconds must fail in compile()");
    assert!(res_reinf_compile.diagnostics.iter().any(|d| d.contains("相對延遲")));

    let res_reinf_merge = CampaignMissionCompiler::compile_for_merge(&plan_reinf, &known_aliases, &scenario);
    assert!(!res_reinf_merge.success, "Reinforcement with non-zero relative_delay_seconds must fail in compile_for_merge()");
    assert!(res_reinf_merge.compiled_events.is_empty(), "Failed merge must maintain atomicity and return empty events");
    assert!(res_reinf_merge.diagnostics[0].contains("部隊相對延遲"));

    // 3. Boundary: relative_delay_seconds == 0 succeeds cleanly in compile and compile_for_merge
    let mut reinf_zero = ReinforcementDefinition::new(Uuid::new_v4(), "ValidReinf", 60, 1, 4000.0, 4000.0, vec![]);
    reinf_zero.squads.push(WaveSquadDefinition {
        alias: "ROM_LEGION".to_string(),
        count: 10,
        team: 1,
        relative_delay_seconds: 0,
    });
    let mut plan_zero = CampaignMissionPlan::new("ValidPlan", "");
    plan_zero.reinforcements.push(reinf_zero);

    let res_zero_compile = CampaignMissionCompiler::compile(&plan_zero, None, &known_aliases, Some(&scenario));
    assert!(res_zero_compile.success, "Zero relative delay must succeed in compile()");
    assert_eq!(res_zero_compile.compiled_events.len(), 1);
    assert_eq!(res_zero_compile.compiled_events[0].name, "Reinf_ValidReinf");
    assert_eq!(res_zero_compile.compiled_events[0].delay_seconds, 60);

    let res_zero_merge = CampaignMissionCompiler::compile_for_merge(&plan_zero, &known_aliases, &scenario);
    assert!(res_zero_merge.success, "Zero relative delay must succeed in compile_for_merge()");
    assert_eq!(res_zero_merge.compiled_events.len(), 1);
    assert_eq!(res_zero_merge.compiled_events[0].name, "Reinf_ValidReinf");

    // 4. Boundary: negative relative_delay_seconds fails plan validation (ArgumentOutOfRangeException in C#)
    let mut squad_neg = WaveSquadDefinition::new("ROM_LEGION", 5, 1);
    squad_neg.relative_delay_seconds = -1;
    assert!(squad_neg.validate().is_err(), "Negative relative delay must fail validation");

    let reinf_neg = ReinforcementDefinition::new(Uuid::new_v4(), "NegReinf", 60, 1, 4000.0, 4000.0, vec![squad_neg]);
    let mut plan_neg = CampaignMissionPlan::new("NegPlan", "");
    plan_neg.reinforcements.push(reinf_neg);
    let res_neg_compile = CampaignMissionCompiler::compile(&plan_neg, None, &known_aliases, Some(&scenario));
    assert!(!res_neg_compile.success);
    assert!(res_neg_compile.diagnostics[0].contains("企劃總綱驗證失敗"));
}
