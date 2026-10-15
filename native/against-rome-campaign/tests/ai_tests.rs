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
