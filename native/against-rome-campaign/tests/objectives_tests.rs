use against_rome_campaign::*;
use against_rome_scenario::{BciImage, Condition, DataSlot, ScenarioDocument, Spawn};
use uuid::Uuid;

fn make_test_graph(objective: ObjectiveDefinition) -> ObjectiveDependencyGraph {
    let mut graph = ObjectiveDependencyGraph::new();
    graph.add_objective(objective).unwrap();
    graph
}

fn make_test_objective(kind: ObjectiveKind) -> ObjectiveDefinition {
    ObjectiveDefinition {
        id: Uuid::new_v4(),
        title: "Goal".to_string(),
        description: "Test Objective".to_string(),
        kind,
        category: ObjectiveCategory::Primary,
        parameters: ObjectiveRuleParameters {
            target_guids: vec![Uuid::new_v4()],
            hold_duration_seconds: if kind == ObjectiveKind::Survival { 10 } else { 0 },
            area: if matches!(kind, ObjectiveKind::CaptureArea | ObjectiveKind::EscortUnit) {
                Some(ObjectiveAreaBounds::from_rectangle(100, 200, 300, 400))
            } else {
                None
            },
            custom_conditions: if kind == ObjectiveKind::CustomScripted {
                vec![Condition {
                    kind: 0, // ObjectExists
                    target_id: Uuid::new_v4(),
                    ..Default::default()
                }]
            } else {
                vec![]
            },
            ..Default::default()
        },
        reward: None,
        initial_state: ObjectiveState::Active,
        ui_order: 0.0,
    }
}

#[test]
fn test_supported_rules_compile_to_bci() {
    for kind in [
        ObjectiveKind::AssassinateTarget,
        ObjectiveKind::DestroyBuilding,
        ObjectiveKind::Survival,
        ObjectiveKind::EscortUnit,
        ObjectiveKind::CaptureArea,
        ObjectiveKind::CustomScripted,
    ] {
        let mut objective = make_test_objective(kind);
        if kind == ObjectiveKind::CustomScripted {
            objective.parameters.target_guids.clear();
        }
        let mut ids = objective.parameters.target_guids.clone();
        for c in &objective.parameters.custom_conditions {
            ids.push(c.target_id);
        }

        let mut scenario = ScenarioDocument::default();
        for (i, id) in ids.iter().enumerate() {
            scenario.spawns.push(Spawn {
                id: *id,
                alias: "HOUSE".to_string(),
                x: 0.0,
                z: 0.0,
                team: 0,
                prebuilt: true,
                ..Default::default()
            });
            scenario.data_slots.push(DataSlot {
                slot: i as i32 + 42,
                uid: (i as u32) + 123,
                spawn_id: *id,
            });
        }

        let idle_image = BciImage::idle();
        let original_bytes = idle_image.serialize();
        let graph = make_test_graph(objective);

        let result = BciObjectiveCompiler::compile_to_bci(
            &graph,
            &original_bytes,
            &scenario,
            &[],
        );
        assert!(
            result.success,
            "Failed for kind {:?}: {:?}",
            kind, result.diagnostics
        );
        assert!(result.bci_bytes.is_some());
    }
}

#[test]
fn test_unsupported_kinds_are_rejected() {
    for kind in [
        ObjectiveKind::EliminateAllEnemies,
        ObjectiveKind::KingOfTheHill,
    ] {
        let objective = make_test_objective(kind);
        let graph = make_test_graph(objective);
        let result = BciObjectiveCompiler::compile(&graph, None, None);
        assert!(!result.success, "Kind {:?} should be rejected", kind);
    }
}

#[test]
fn test_objective_graph_cycle_detection() {
    let mut graph = ObjectiveDependencyGraph::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();

    let mut obj_a = make_test_objective(ObjectiveKind::AssassinateTarget);
    obj_a.id = a;
    let mut obj_b = make_test_objective(ObjectiveKind::DestroyBuilding);
    obj_b.id = b;
    let mut obj_c = make_test_objective(ObjectiveKind::Survival);
    obj_c.id = c;

    graph.add_objective(obj_a).unwrap();
    graph.add_objective(obj_b).unwrap();
    graph.add_objective(obj_c).unwrap();

    graph.add_dependency(a, b, DependencyRelation::Prerequisite);
    graph.add_dependency(b, c, DependencyRelation::Prerequisite);
    assert!(graph.detect_cycles().is_empty());

    // Introduce cycle: c -> a
    graph.add_dependency(c, a, DependencyRelation::Prerequisite);
    let cycles = graph.detect_cycles();
    assert!(!cycles.is_empty());
}

#[test]
fn test_objective_sandbox_state_propagation() {
    let mut graph = ObjectiveDependencyGraph::new();
    let parent_id = Uuid::new_v4();
    let child1_id = Uuid::new_v4();
    let child2_id = Uuid::new_v4();

    let mut parent = make_test_objective(ObjectiveKind::AssassinateTarget);
    parent.id = parent_id;
    parent.category = ObjectiveCategory::Primary;
    parent.initial_state = ObjectiveState::Active;

    let mut child1 = make_test_objective(ObjectiveKind::AssassinateTarget);
    child1.id = child1_id;
    child1.category = ObjectiveCategory::Bonus;
    child1.initial_state = ObjectiveState::Active;
    let target1 = child1.parameters.target_guids[0];

    let mut child2 = make_test_objective(ObjectiveKind::AssassinateTarget);
    child2.id = child2_id;
    child2.category = ObjectiveCategory::Bonus;
    child2.initial_state = ObjectiveState::Active;
    let target2 = child2.parameters.target_guids[0];

    graph.add_objective(parent).unwrap();
    graph.add_objective(child1).unwrap();
    graph.add_objective(child2).unwrap();

    graph.add_dependency(child1_id, parent_id, DependencyRelation::CompositeAnd);
    graph.add_dependency(child2_id, parent_id, DependencyRelation::CompositeAnd);

    let mut sandbox = ObjectiveSandboxSession::new();
    sandbox.initialize(graph);

    assert_eq!(
        sandbox.get_progress(parent_id).unwrap().state,
        ObjectiveState::Active
    );
    assert_eq!(sandbox.campaign_status, CampaignResult::InProgress);

    // Complete child 1 by killing target1
    sandbox.simulate_object_death(target1);
    assert_eq!(
        sandbox.get_progress(child1_id).unwrap().state,
        ObjectiveState::Completed
    );
    assert_eq!(
        sandbox.get_progress(parent_id).unwrap().state,
        ObjectiveState::Active
    );

    // Complete child 2 by killing target2 -> Composite AND should complete parent
    sandbox.simulate_object_death(target2);
    assert_eq!(
        sandbox.get_progress(child2_id).unwrap().state,
        ObjectiveState::Completed
    );
    assert_eq!(
        sandbox.get_progress(parent_id).unwrap().state,
        ObjectiveState::Completed
    );
    assert_eq!(sandbox.campaign_status, CampaignResult::Victory);
}
