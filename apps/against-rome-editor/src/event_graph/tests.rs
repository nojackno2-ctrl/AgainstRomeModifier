//! 劇情事件圖單元測試 (EventGraph Tests)
//!
//! 完整移植自 C# `EventGraphTests.cs`，包含連接埠驗證、無損雙向轉換、靜態分析與死鎖循環檢測。

use std::collections::HashSet;
use uuid::Uuid;
use against_rome_scenario::{Action, Condition, Event};

use crate::event_graph::converter;
use crate::event_graph::model::{
    EventGraph, GraphDiagnosticSeverity, GraphNode, NodeKind,
};
use crate::event_graph::validator;

fn valid_aliases() -> Vec<String> {
    vec![
        "GER_INF01".to_string(),
        "ROM_LEG01".to_string(),
        "CEL_INF01".to_string(),
    ]
}

#[test]
fn test_node_and_port_operations_work_correctly() {
    let mut graph = EventGraph::new();
    let trigger = GraphNode::new_event_trigger("TestTrigger", 10, false, true);
    let msg = GraphNode::new_message_action("Hello World");
    let cond = GraphNode::new_object_exists(Uuid::new_v4(), "Cond");

    let trigger_id = trigger.id;
    let msg_id = msg.id;
    let cond_id = cond.id;

    graph.add_node(trigger);
    graph.add_node(msg);
    graph.add_node(cond);

    assert_eq!(graph.nodes.len(), 3);

    let trigger_node = graph.find_node(trigger_id).unwrap();
    let cond_node = graph.find_node(cond_id).unwrap();
    let msg_node = graph.find_node(msg_id).unwrap();

    let cond_out = cond_node.find_port_by_name("ConditionOut").unwrap().id;
    let trigger_cond_in = trigger_node.find_port_by_name("ConditionsIn").unwrap().id;
    let trigger_exec_in = trigger_node.find_port_by_name("ExecIn").unwrap().id;
    let trigger_exec_out = trigger_node.find_port_by_name("ExecOut").unwrap().id;
    let msg_exec_in = msg_node.find_port_by_name("ExecIn").unwrap().id;

    // 1. 測試連接 Condition -> ConditionsIn
    let cond_edge = graph.connect(cond_out, trigger_cond_in);
    assert!(cond_edge.is_ok());

    // 2. 測試連接 Trigger.ExecOut -> Msg.ExecIn
    let exec_edge = graph.connect(trigger_exec_out, msg_exec_in);
    assert!(exec_edge.is_ok());

    // 3. 測試不可連接同節點端口
    let self_connect = graph.connect(trigger_exec_in, trigger_exec_out);
    assert!(self_connect.is_err());

    // 4. 測試類型不相容不可連接 (Condition -> Execution)
    let invalid_type = graph.connect(cond_out, msg_exec_in);
    assert!(invalid_type.is_err());

    // 5. 測試移除節點會同步清理關聯連線
    assert!(graph.remove_node(msg_id));
    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(graph.edges.len(), 1); // 只剩 cond -> trigger
}

#[test]
fn test_event_graph_converter_round_trip_is_lossless() {
    let target1 = Uuid::new_v4();
    let target2 = Uuid::new_v4();

    let original_events = vec![
        Event {
            name: "Stage1_KillEnemies".to_string(),
            delay_seconds: 5,
            repeat: false,
            enabled: true,
            conditions: vec![
                Condition {
                    kind: 1, // ObjectDeadOrRemoved
                    target_id: target1,
                    ..Default::default()
                },
                Condition {
                    kind: 2, // ObjectInArea
                    target_id: target2,
                    min_x: 100,
                    min_z: 200,
                    max_x: 500,
                    max_z: 600,
                },
            ],
            actions: vec![
                Action {
                    kind: 0, // Message
                    text: "Enemies eliminated!".to_string(),
                    ..Default::default()
                },
                Action {
                    kind: 1, // Diplomacy
                    team: 0,
                    other_team: 1,
                    hostile: false,
                    ..Default::default()
                },
                Action {
                    kind: 2, // SpawnUnit
                    alias: "GER_INF01".to_string(),
                    team: 0,
                    x: 4500.0,
                    z: 5500.0,
                    count: 8,
                    ..Default::default()
                },
            ],
        },
        Event {
            name: "Stage2_VictoryCheck".to_string(),
            delay_seconds: 15,
            repeat: false,
            enabled: true,
            conditions: vec![Condition {
                kind: 0, // ObjectExists
                target_id: target2,
                ..Default::default()
            }],
            actions: vec![
                Action {
                    kind: 0,
                    text: "Victory achieved!".to_string(),
                    ..Default::default()
                },
                Action {
                    kind: 3, // Victory
                    ..Default::default()
                },
            ],
        },
    ];

    // 轉換為圖
    let graph = converter::from_scenario_events(&original_events, None);

    let trigger_count = graph
        .nodes
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::EventTrigger { .. }))
        .count();
    let condition_count = graph
        .nodes
        .iter()
        .filter(|n| {
            matches!(
                n.kind,
                NodeKind::ObjectExistsCondition { .. }
                    | NodeKind::ObjectDeadCondition { .. }
                    | NodeKind::ObjectInAreaCondition { .. }
            )
        })
        .count();
    let action_count = graph
        .nodes
        .iter()
        .filter(|n| {
            matches!(
                n.kind,
                NodeKind::MessageAction { .. }
                    | NodeKind::DiplomacyAction { .. }
                    | NodeKind::SpawnUnitAction { .. }
                    | NodeKind::VictoryAction
                    | NodeKind::DefeatAction
            )
        })
        .count();

    assert_eq!(trigger_count, 2);
    assert_eq!(condition_count, 3);
    assert_eq!(action_count, 5);

    // 編譯回 ScenarioEvent 清單
    let compiled_events = converter::to_scenario_events(&graph);

    assert_eq!(original_events.len(), compiled_events.len());
    for i in 0..original_events.len() {
        let orig = &original_events[i];
        let comp = &compiled_events[i];

        assert_eq!(orig.name, comp.name);
        assert_eq!(orig.delay_seconds, comp.delay_seconds);
        assert_eq!(orig.repeat, comp.repeat);
        assert_eq!(orig.enabled, comp.enabled);

        assert_eq!(orig.conditions.len(), comp.conditions.len());
        for c in 0..orig.conditions.len() {
            assert_eq!(orig.conditions[c].kind, comp.conditions[c].kind);
            assert_eq!(orig.conditions[c].target_id, comp.conditions[c].target_id);
            assert_eq!(orig.conditions[c].min_x, comp.conditions[c].min_x);
            assert_eq!(orig.conditions[c].min_z, comp.conditions[c].min_z);
            assert_eq!(orig.conditions[c].max_x, comp.conditions[c].max_x);
            assert_eq!(orig.conditions[c].max_z, comp.conditions[c].max_z);
        }

        assert_eq!(orig.actions.len(), comp.actions.len());
        for a in 0..orig.actions.len() {
            assert_eq!(orig.actions[a].kind, comp.actions[a].kind);
            assert_eq!(orig.actions[a].text, comp.actions[a].text);
            assert_eq!(orig.actions[a].team, comp.actions[a].team);
            assert_eq!(orig.actions[a].other_team, comp.actions[a].other_team);
            assert_eq!(orig.actions[a].hostile, comp.actions[a].hostile);
            assert_eq!(orig.actions[a].alias, comp.actions[a].alias);
            assert_eq!(orig.actions[a].x, comp.actions[a].x);
            assert_eq!(orig.actions[a].z, comp.actions[a].z);
            assert_eq!(orig.actions[a].count, comp.actions[a].count);
        }
    }
}

#[test]
fn test_event_graph_validator_detects_cycles() {
    let mut graph = EventGraph::new();
    let trigger = GraphNode::new_event_trigger("CycleTrigger", 10, false, true);
    let msg1 = GraphNode::new_message_action("Step 1");
    let msg2 = GraphNode::new_message_action("Step 2");

    let msg1_id = msg1.id;
    let msg2_id = msg2.id;

    graph.add_node(trigger);
    graph.add_node(msg1);
    graph.add_node(msg2);

    let m1_out = graph.find_node(msg1_id).unwrap().find_port_by_name("ExecOut").unwrap().id;
    let m1_in = graph.find_node(msg1_id).unwrap().find_port_by_name("ExecIn").unwrap().id;
    let m2_out = graph.find_node(msg2_id).unwrap().find_port_by_name("ExecOut").unwrap().id;
    let m2_in = graph.find_node(msg2_id).unwrap().find_port_by_name("ExecIn").unwrap().id;

    // msg1 -> msg2
    assert!(graph.connect(m1_out, m2_in).is_ok());

    let diags1 = validator::validate(&graph, None, None);
    assert!(!diags1.iter().any(|d| d.code == "EXECUTION_CYCLE_DETECTED"));

    // msg2 -> msg1形成迴路
    assert!(graph.connect(m2_out, m1_in).is_ok());

    let diags2 = validator::validate(&graph, None, None);
    assert!(diags2.iter().any(|d| d.code == "EXECUTION_CYCLE_DETECTED" && d.severity == GraphDiagnosticSeverity::Error));
}

#[test]
fn test_event_graph_validator_detects_orphan_actions_and_dangling_conditions() {
    let mut graph = EventGraph::new();
    let trigger = GraphNode::new_event_trigger("EmptyTrigger", 10, false, true);
    let orphan = GraphNode::new_message_action("Never Called");
    let dangling = GraphNode::new_object_exists(Uuid::new_v4(), "Dangling");

    graph.add_node(trigger);
    graph.add_node(orphan);
    graph.add_node(dangling);

    let diags = validator::validate(&graph, None, None);

    assert!(diags.iter().any(|d| d.code == "EMPTY_TRIGGER_FLOW" && d.severity == GraphDiagnosticSeverity::Warning));
    assert!(diags.iter().any(|d| d.code == "ORPHAN_ACTION" && d.severity == GraphDiagnosticSeverity::Warning));
    assert!(diags.iter().any(|d| d.code == "DANGLING_CONDITION" && d.severity == GraphDiagnosticSeverity::Warning));
}

#[test]
fn test_event_graph_validator_detects_invalid_target_guids_and_aliases() {
    let mut graph = EventGraph::new();
    let trigger = GraphNode::new_event_trigger("Trigger", 10, false, true);
    let empty_guid_cond = GraphNode::new_object_exists(Uuid::nil(), "Empty");
    let nonexistent_guid_cond = GraphNode::new_object_exists(Uuid::new_v4(), "NonExistent");
    let invalid_spawn = GraphNode::new_spawn_unit_action("NON_EXISTENT_UNIT", 99, 20000.0, -50.0, 50);

    let trigger_id = trigger.id;
    let c1_id = empty_guid_cond.id;
    let c2_id = nonexistent_guid_cond.id;
    let s_id = invalid_spawn.id;

    graph.add_node(trigger);
    graph.add_node(empty_guid_cond);
    graph.add_node(nonexistent_guid_cond);
    graph.add_node(invalid_spawn);

    let trig_cond_in = graph.find_node(trigger_id).unwrap().find_port_by_name("ConditionsIn").unwrap().id;
    let trig_exec_out = graph.find_node(trigger_id).unwrap().find_port_by_name("ExecOut").unwrap().id;
    let c1_out = graph.find_node(c1_id).unwrap().find_port_by_name("ConditionOut").unwrap().id;
    let c2_out = graph.find_node(c2_id).unwrap().find_port_by_name("ConditionOut").unwrap().id;
    let s_in = graph.find_node(s_id).unwrap().find_port_by_name("ExecIn").unwrap().id;

    let _ = graph.connect(c1_out, trig_cond_in);
    let _ = graph.connect(c2_out, trig_cond_in);
    let _ = graph.connect(trig_exec_out, s_in);

    let mut valid_targets = HashSet::new();
    valid_targets.insert(Uuid::new_v4()); // 不包含 nonexistent_guid_cond

    let aliases = valid_aliases();
    let diags = validator::validate(&graph, Some(&valid_targets), Some(&aliases));

    assert!(diags.iter().any(|d| d.code == "EMPTY_TARGET_GUID"));
    assert!(diags.iter().any(|d| d.code == "INVALID_TARGET_GUID"));
    assert!(diags.iter().any(|d| d.code == "INVALID_SPAWN_ALIAS"));
    assert!(diags.iter().any(|d| d.code == "INVALID_SPAWN_TEAM"));
    assert!(diags.iter().any(|d| d.code == "INVALID_SPAWN_COUNT"));
    assert!(diags.iter().any(|d| d.code == "INVALID_SPAWN_COORDINATES"));
}

#[test]
fn test_event_graph_validator_validates_terminal_actions() {
    let mut graph = EventGraph::new();
    let trigger = GraphNode::new_event_trigger("RepeatVictory", 5, true, true);
    let victory = GraphNode::new_victory_action();

    let trigger_id = trigger.id;
    let victory_id = victory.id;

    graph.add_node(trigger);
    graph.add_node(victory);

    let t_out = graph.find_node(trigger_id).unwrap().find_port_by_name("ExecOut").unwrap().id;
    let v_in = graph.find_node(victory_id).unwrap().find_port_by_name("ExecIn").unwrap().id;

    assert!(graph.connect(t_out, v_in).is_ok());

    let diags = validator::validate(&graph, None, None);
    assert!(diags.iter().any(|d| d.code == "TERMINAL_IN_REPEAT_EVENT" && d.severity == GraphDiagnosticSeverity::Error));
}
