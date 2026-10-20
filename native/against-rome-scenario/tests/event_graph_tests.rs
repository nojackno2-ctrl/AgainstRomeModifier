use against_rome_scenario::*;
use uuid::Uuid;

fn make_test_spawn(id: Uuid, alias: &str, x: f32, z: f32, team: i32) -> Spawn {
    Spawn {
        alias: alias.into(),
        x,
        z,
        team,
        count: 10,
        angle: 0,
        y: 0.0,
        prebuilt: false,
        id,
        raw_rotation: None,
    }
}

#[test]
fn event_graph_node_and_port_operations_work_correctly() {
    let mut graph = EventGraph::new();
    let trigger = EventTriggerNode::new("TestTrigger", 10, false, true);
    let trigger_exec_in = trigger.exec_in.id;
    let trigger_conditions_in = trigger.conditions_in.id;
    let trigger_exec_out = trigger.exec_out.id;

    let msg = MessageActionNode::new("Hello World");
    let msg_id = msg.id;
    let msg_exec_in = msg.exec_in.id;

    let target_id = Uuid::new_v4();
    let cond = ObjectExistsConditionNode::new(target_id, "Chief");
    let cond_out = cond.condition_out.id;

    assert!(graph.add_node(GraphNode::Trigger(trigger)).is_ok());
    assert!(graph.add_node(GraphNode::MessageAction(msg)).is_ok());
    assert!(graph.add_node(GraphNode::ExistsCondition(cond)).is_ok());
    assert_eq!(graph.nodes.len(), 3);

    // Connecting Condition -> ConditionsIn
    let cond_edge = graph.connect(cond_out, trigger_conditions_in);
    assert!(cond_edge.is_ok());

    // Connecting Trigger.ExecOut -> Msg.ExecIn
    let exec_edge = graph.connect(trigger_exec_out, msg_exec_in);
    assert!(exec_edge.is_ok());

    // Self-connection should fail
    assert!(graph.connect(trigger_exec_in, trigger_exec_out).is_err());

    // Incompatible port types (Condition -> Execution) should fail
    assert!(graph.connect(cond_out, msg_exec_in).is_err());

    // Multiple outgoing edges on ExecOut should fail
    let msg2 = MessageActionNode::new("Second");
    let msg2_exec_in = msg2.exec_in.id;
    assert!(graph.add_node(GraphNode::MessageAction(msg2)).is_ok());
    assert!(graph.connect(trigger_exec_out, msg2_exec_in).is_err());

    // Removing a node should clean up associated edges
    assert!(graph.remove_node(msg_id));
    assert_eq!(graph.nodes.len(), 3);
    assert_eq!(graph.edges.len(), 1); // Only cond -> trigger remains

    // Disconnect by ports
    assert!(graph.disconnect_ports(cond_out, trigger_conditions_in));
    assert_eq!(graph.edges.len(), 0);
}

#[test]
fn event_graph_converter_roundtrip_is_lossless() {
    let target1 = Uuid::new_v4();
    let target2 = Uuid::new_v4();

    let original_events = vec![
        Event {
            name: "Stage1_KillEnemies".into(),
            delay_seconds: 5,
            repeat: false,
            enabled: true,
            conditions: vec![
                Condition {
                    kind: 1, // DeadOrRemoved
                    target_id: target1,
                    min_x: 0,
                    min_z: 0,
                    max_x: 16383,
                    max_z: 16383,
                },
                Condition {
                    kind: 2, // InArea
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
                    text: "Enemies eliminated!".into(),
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
                    alias: "GER_INF01".into(),
                    team: 0,
                    x: 4500.0,
                    z: 5500.0,
                    count: 8,
                    move_destination: Some(WorldPosition { x: 5000, z: 6000 }),
                    ..Default::default()
                },
            ],
        },
        Event {
            name: "Stage2_VictoryCheck".into(),
            delay_seconds: 15,
            repeat: false,
            enabled: true,
            conditions: vec![Condition {
                kind: 0, // Exists
                target_id: target2,
                min_x: 0,
                min_z: 0,
                max_x: 16383,
                max_z: 16383,
            }],
            actions: vec![
                Action {
                    kind: 0,
                    text: "Victory achieved!".into(),
                    ..Default::default()
                },
                Action {
                    kind: 3, // Victory
                    ..Default::default()
                },
            ],
        },
    ];

    // Convert to graph
    let graph = EventGraphConverter::from_scenario_events(&original_events);
    assert_eq!(
        graph.nodes.iter().filter(|n| matches!(n, GraphNode::Trigger(_))).count(),
        2
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|n| matches!(
                n,
                GraphNode::ExistsCondition(_)
                    | GraphNode::DeadCondition(_)
                    | GraphNode::AreaCondition(_)
            ))
            .count(),
        3
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|n| matches!(
                n,
                GraphNode::MessageAction(_)
                    | GraphNode::DiplomacyAction(_)
                    | GraphNode::SpawnUnitAction(_)
                    | GraphNode::VictoryAction(_)
                    | GraphNode::DefeatAction(_)
            ))
            .count(),
        5
    );

    // Convert back to ScenarioEvents
    let compiled = EventGraphConverter::to_scenario_events(&graph).expect("conversion to events");
    assert_eq!(original_events, compiled);

    // JSON serialization roundtrip
    let serialized = serde_json::to_string_pretty(&graph).expect("serialize graph");
    let deserialized: EventGraph = serde_json::from_str(&serialized).expect("deserialize graph");
    assert_eq!(graph, deserialized);
}

#[test]
fn event_graph_validator_detects_cycles() {
    let mut graph = EventGraph::new();
    let trigger = EventTriggerNode::new("CycleTrigger", 5, false, true);
    let msg1 = MessageActionNode::new("Step 1");
    let msg2 = MessageActionNode::new("Step 2");

    let t_out = trigger.exec_out.id;
    let m1_in = msg1.exec_in.id;
    let m1_out = msg1.exec_out.id;
    let m2_in = msg2.exec_in.id;
    let m2_out = msg2.exec_out.id;

    graph.add_node(GraphNode::Trigger(trigger)).unwrap();
    graph.add_node(GraphNode::MessageAction(msg1)).unwrap();
    graph.add_node(GraphNode::MessageAction(msg2)).unwrap();

    assert!(graph.connect(t_out, m1_in).is_ok());
    assert!(graph.connect(m1_out, m2_in).is_ok());

    let diags = EventGraphValidator::validate(&graph, None, None);
    assert!(!diags.iter().any(|d| d.code == "EXECUTION_CYCLE_DETECTED"));

    // Introduce cycle: m2 -> m1
    // First, disconnect t_out -> m1_in because m1_in only allows single connection
    graph.disconnect_ports(t_out, m1_in);
    assert!(graph.connect(m2_out, m1_in).is_ok());

    let diags = EventGraphValidator::validate(&graph, None, None);
    assert!(diags.iter().any(|d| d.code == "EXECUTION_CYCLE_DETECTED"
        && d.severity == GraphDiagnosticSeverity::Error));
}

#[test]
fn event_graph_validator_detects_orphan_actions_and_dangling_conditions() {
    let mut graph = EventGraph::new();
    let trigger = EventTriggerNode::new("EmptyTrigger", 10, false, true);
    let orphan_action = MessageActionNode::new("Never Called");
    let dangling_cond = ObjectExistsConditionNode::new(Uuid::new_v4(), "Dangling");

    graph.add_node(GraphNode::Trigger(trigger)).unwrap();
    graph.add_node(GraphNode::MessageAction(orphan_action)).unwrap();
    graph.add_node(GraphNode::ExistsCondition(dangling_cond)).unwrap();

    let diags = EventGraphValidator::validate(&graph, None, None);
    assert!(diags.iter().any(|d| d.code == "EMPTY_TRIGGER_FLOW"
        && d.severity == GraphDiagnosticSeverity::Warning));
    assert!(diags.iter().any(|d| d.code == "ORPHAN_ACTION"
        && d.severity == GraphDiagnosticSeverity::Warning));
    assert!(diags.iter().any(|d| d.code == "DANGLING_CONDITION"
        && d.severity == GraphDiagnosticSeverity::Warning));
}

#[test]
fn event_graph_validator_detects_terminal_action_in_repeat_and_unsupported_delay() {
    let mut graph = EventGraph::new();
    let trigger = EventTriggerNode::new("RepeatVictory", 5, true, true);
    let victory = VictoryActionNode::new();

    let t_out = trigger.exec_out.id;
    let v_in = victory.exec_in.id;

    graph.add_node(GraphNode::Trigger(trigger)).unwrap();
    graph.add_node(GraphNode::VictoryAction(victory)).unwrap();
    graph.connect(t_out, v_in).unwrap();

    let diags = EventGraphValidator::validate(&graph, None, None);
    assert!(diags.iter().any(|d| d.code == "TERMINAL_IN_REPEAT_EVENT"
        && d.severity == GraphDiagnosticSeverity::Error));

    // Adding DelayNode: C# documents DelayNode as unsupported on apply
    let delay = DelayNode::new(3);
    graph.add_node(GraphNode::Delay(delay)).unwrap();

    let mut doc = ScenarioDocument::default();
    let apply_res = doc.apply_event_graph(&graph, None);
    assert!(apply_res.is_err());
    assert_eq!(
        apply_res.unwrap_err(),
        "Fix graph errors and disconnected nodes before applying. Delay nodes and chained triggers are unsupported."
    );
}

#[test]
fn apply_event_graph_rejects_chained_triggers_and_rolls_back() {
    let mut doc = ScenarioDocument::default();
    let orig_event = Event {
        name: "Initial".into(),
        delay_seconds: 10,
        repeat: false,
        enabled: true,
        actions: vec![Action {
            kind: 0,
            text: "Hello".into(),
            ..Default::default()
        }],
        conditions: vec![],
    };
    doc.events.push(orig_event.clone());

    // Create a graph with chained triggers (Trigger1 -> Trigger2 via ExecIn)
    let mut graph = EventGraph::new();
    let trigger1 = EventTriggerNode::new("T1", 5, false, true);
    let trigger2 = EventTriggerNode::new("T2", 5, false, true);
    let t1_out = trigger1.exec_out.id;
    let t2_in = trigger2.exec_in.id;

    graph.add_node(GraphNode::Trigger(trigger1)).unwrap();
    graph.add_node(GraphNode::Trigger(trigger2)).unwrap();
    graph.connect(t1_out, t2_in).unwrap();

    assert!(graph.has_chained_triggers());

    let res = doc.apply_event_graph(&graph, None);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err(),
        "Fix graph errors and disconnected nodes before applying. Delay nodes and chained triggers are unsupported."
    );

    // Failure rollback check: doc.events must be untouched!
    assert_eq!(doc.events.len(), 1);
    assert_eq!(doc.events[0], orig_event);
}

#[test]
fn apply_valid_event_graph_succeeds_and_preserves_targets() {
    let mut doc = ScenarioDocument::default();
    let spawn_id = Uuid::new_v4();
    doc.spawns.push(make_test_spawn(spawn_id, "GER_INF01", 1000.0, 2000.0, 0));

    let events = vec![Event {
        name: "MissionStart".into(),
        delay_seconds: 2,
        repeat: false,
        enabled: true,
        conditions: vec![Condition {
            kind: 0,
            target_id: spawn_id,
            min_x: 0,
            min_z: 0,
            max_x: 16383,
            max_z: 16383,
        }],
        actions: vec![Action {
            kind: 0,
            text: "Defend the camp!".into(),
            ..Default::default()
        }],
    }];

    let graph = EventGraphConverter::from_scenario_events(&events);
    assert!(doc.apply_event_graph(&graph, Some(&["GER_INF01".into()])).is_ok());
    assert_eq!(doc.events, events);

    // Apply with invalid alias should fail and roll back
    let bad_events = vec![Event {
        name: "BadSpawn".into(),
        delay_seconds: 2,
        repeat: false,
        enabled: true,
        conditions: vec![],
        actions: vec![Action {
            kind: 2,
            alias: "UNKNOWN_UNIT".into(),
            team: 0,
            count: 5,
            x: 100.0,
            z: 100.0,
            ..Default::default()
        }],
    }];
    let bad_graph = EventGraphConverter::from_scenario_events(&bad_events);
    let fail_res = doc.apply_event_graph(&bad_graph, Some(&["GER_INF01".into()]));
    assert!(fail_res.is_err());
    // Verification: doc.events remains the previous valid events
    assert_eq!(doc.events, events);
}

#[test]
fn scenario_document_authoring_helpers_and_rollback() {
    let mut doc = ScenarioDocument::default();
    assert_eq!(doc.events.len(), 0);

    // Add event
    let event1 = Event {
        name: "Opening".into(),
        delay_seconds: 10,
        repeat: false,
        enabled: true,
        actions: vec![
            Action {
                kind: 0,
                text: "Step 1".into(),
                ..Default::default()
            },
            Action {
                kind: 0,
                text: "Step 2".into(),
                ..Default::default()
            },
            Action {
                kind: 3, // Victory
                ..Default::default()
            },
        ],
        conditions: vec![],
    };
    assert_eq!(doc.add_event(event1.clone()).unwrap(), 0);
    assert_eq!(doc.events.len(), 1);

    // Reorder actions: Move Victory from index 2 to index 1 should fail
    // because terminal action must be the last action!
    let move_fail = doc.move_event_action(0, 2, -1);
    assert!(move_fail.is_err());
    // Rollback check: order must still be Step 1, Step 2, Victory
    assert_eq!(doc.events[0].actions[2].kind, 3);
    assert_eq!(doc.events[0].actions[1].text, "Step 2");

    // Reorder non-terminal actions: swap Step 1 (0) and Step 2 (1)
    let move_ok = doc.move_event_action(0, 0, 1);
    assert_eq!(move_ok.unwrap(), 1);
    assert_eq!(doc.events[0].actions[0].text, "Step 2");
    assert_eq!(doc.events[0].actions[1].text, "Step 1");

    // Duplicate event
    let dup_idx = doc.duplicate_event(0, " (copy)").unwrap();
    assert_eq!(dup_idx, 1);
    assert_eq!(doc.events.len(), 2);
    assert_eq!(doc.events[1].name, "Opening (copy)");

    // Suffix overflow truncation test (name capped at 100 chars)
    let long_name = "A".repeat(95);
    doc.events[0].name = long_name;
    let dup_idx2 = doc.duplicate_event(0, " (copy)").unwrap();
    assert_eq!(doc.events[dup_idx2].name.len(), 100);
    assert!(doc.events[dup_idx2].name.ends_with(" (copy)"));

    // Replace event with invalid event should fail and roll back
    let invalid_event = Event {
        name: "".into(), // empty name is invalid
        delay_seconds: 10,
        repeat: false,
        enabled: true,
        actions: vec![Action::default()],
        conditions: vec![],
    };
    assert!(doc.replace_event(0, invalid_event).is_err());
    assert!(!doc.events[0].name.is_empty());

    // Remove event
    let removed = doc.remove_event(0).unwrap();
    assert_eq!(removed.name, "A".repeat(95));
}

#[test]
fn scenario_event_session_undo_redo_and_dirty_tracking() {
    let mut session = ScenarioEventSession::new();
    let seed = Event {
        name: "Initial".into(),
        delay_seconds: 10,
        repeat: false,
        enabled: true,
        actions: vec![Action {
            kind: 0,
            text: "Hello".into(),
            ..Default::default()
        }],
        conditions: vec![],
    };

    session.load(&[seed.clone()]);
    assert_eq!(session.count(), 1);
    assert!(!session.is_dirty());
    assert!(!session.can_undo());
    assert!(!session.can_redo());

    // Modify event
    let mut modified = seed.clone();
    modified.delay_seconds = 20;
    session.replace(0, modified).unwrap();
    assert!(session.is_dirty());
    assert!(session.can_undo());

    // Undo restores clean state
    assert!(session.undo());
    assert_eq!(session.capture()[0].delay_seconds, 10);
    assert!(!session.is_dirty());
    assert!(session.can_redo());

    // Redo restores modified state
    assert!(session.redo());
    assert_eq!(session.capture()[0].delay_seconds, 20);
    assert!(session.is_dirty());

    // Duplicate
    let dup_idx = session.duplicate(0, " (copy)").unwrap();
    assert_eq!(dup_idx, 1);
    assert_eq!(session.count(), 2);
    assert_eq!(session.capture()[1].name, "Initial (copy)");

    // Accept changes updates baseline
    session.accept_changes();
    assert!(!session.is_dirty());

    // Reset rolls back to baseline
    session.remove_at(0).unwrap();
    assert!(session.is_dirty());
    session.reset();
    assert_eq!(session.count(), 2);
    assert!(!session.is_dirty());

    // 256 event limit
    let fill = seed.clone();
    while session.count() < 256 {
        session.add(fill.clone()).unwrap();
    }
    assert_eq!(session.count(), 256);
    assert!(session.add(fill.clone()).is_err());
    assert!(session.duplicate(0, " (copy)").is_err());
}
