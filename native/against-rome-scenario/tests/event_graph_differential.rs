use against_rome_scenario::{
    Action, Condition, Event, EventGraph, EventGraphConverter, EventGraphValidator,
    GraphDiagnosticSeverity, GraphNode, PortDirection, PortType, ScenarioEventSession,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DtoCondition {
    pub kind: i32,
    pub target_id: Uuid,
    #[serde(default)]
    pub min_x: i32,
    #[serde(default)]
    pub min_z: i32,
    #[serde(default = "default_max_coord")]
    pub max_x: i32,
    #[serde(default = "default_max_coord")]
    pub max_z: i32,
}

fn default_max_coord() -> i32 {
    16383
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DtoAction {
    pub kind: i32,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub team: i32,
    #[serde(default = "default_other_team")]
    pub other_team: i32,
    #[serde(default = "default_true")]
    pub hostile: bool,
    #[serde(default)]
    pub alias: String,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub z: f32,
    #[serde(default = "default_count")]
    pub count: i32,
}

fn default_other_team() -> i32 {
    1
}
fn default_true() -> bool {
    true
}
fn default_count() -> i32 {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DtoEvent {
    pub name: String,
    #[serde(default)]
    pub delay_seconds: i32,
    #[serde(default)]
    pub repeat: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub conditions: Vec<DtoCondition>,
    #[serde(default)]
    pub actions: Vec<DtoAction>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DtoNode {
    pub id: Uuid,
    pub kind: String,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub event_name: String,
    #[serde(default)]
    pub delay_seconds: i32,
    #[serde(default)]
    pub repeat: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub target_id: Uuid,
    #[serde(default)]
    pub target_label: String,
    #[serde(default)]
    pub min_x: i32,
    #[serde(default)]
    pub min_z: i32,
    #[serde(default = "default_max_coord")]
    pub max_x: i32,
    #[serde(default = "default_max_coord")]
    pub max_z: i32,
    #[serde(default)]
    pub message_text: String,
    #[serde(default)]
    pub team: i32,
    #[serde(default = "default_other_team")]
    pub other_team: i32,
    #[serde(default = "default_true")]
    pub hostile: bool,
    #[serde(default)]
    pub alias: String,
    #[serde(default)]
    pub spawn_x: f32,
    #[serde(default)]
    pub spawn_z: f32,
    #[serde(default = "default_count")]
    pub count: i32,
    #[serde(default)]
    pub seconds: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DtoEdge {
    pub source_node_id: Uuid,
    pub source_port_name: String,
    pub target_node_id: Uuid,
    pub target_port_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct FromScenarioEventsTestCase {
    pub name: String,
    pub events: Vec<DtoEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GraphTestCase {
    pub name: String,
    pub nodes: Vec<DtoNode>,
    pub edges: Vec<DtoEdge>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ValidatorTestCase {
    pub name: String,
    pub nodes: Vec<DtoNode>,
    pub edges: Vec<DtoEdge>,
    pub valid_target_ids: Option<Vec<Uuid>>,
    pub valid_aliases: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DtoSessionOp {
    pub op: String,
    pub event: Option<DtoEvent>,
    pub events: Option<Vec<DtoEvent>>,
    #[serde(default)]
    pub index: usize,
    #[serde(default)]
    pub suffix: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SessionTestCase {
    pub name: String,
    pub initial_events: Vec<DtoEvent>,
    pub operations: Vec<DtoSessionOp>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OracleRequest {
    pub from_scenario_events_cases: Vec<FromScenarioEventsTestCase>,
    pub to_scenario_events_cases: Vec<GraphTestCase>,
    pub validator_cases: Vec<ValidatorTestCase>,
    pub session_cases: Vec<SessionTestCase>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ObservedPort {
    pub id: Uuid,
    pub name: String,
    pub display_name: String,
    pub port_type: i32,
    pub direction: i32,
    pub allow_multiple: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ObservedNode {
    pub id: Uuid,
    pub category: String,
    pub title: String,
    pub x: f32,
    pub y: f32,
    pub ports: Vec<ObservedPort>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ObservedEdge {
    pub source_node_id: Uuid,
    pub source_port_name: String,
    pub target_node_id: Uuid,
    pub target_port_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct FromScenarioEventsResult {
    pub name: String,
    pub nodes: Vec<ObservedNode>,
    pub edges: Vec<ObservedEdge>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ToScenarioEventsResult {
    pub name: String,
    pub events: Vec<DtoEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ObservedDiagnostic {
    pub severity: i32,
    pub code: String,
    pub message: String,
    pub node_id: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ValidatorResult {
    pub name: String,
    pub diagnostics: Vec<ObservedDiagnostic>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ObservedSessionStep {
    pub op: String,
    pub success: bool,
    pub error: Option<String>,
    pub count: usize,
    pub is_dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub event_names: Vec<String>,
    pub baseline_names: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SessionResult {
    pub name: String,
    pub steps: Vec<ObservedSessionStep>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OracleResponse {
    pub from_scenario_events_results: Vec<FromScenarioEventsResult>,
    pub to_scenario_events_results: Vec<ToScenarioEventsResult>,
    pub validator_results: Vec<ValidatorResult>,
    pub session_results: Vec<SessionResult>,
}


fn to_rust_event(dto: &DtoEvent) -> Event {
    Event {
        name: dto.name.clone(),
        delay_seconds: dto.delay_seconds,
        repeat: dto.repeat,
        enabled: dto.enabled,
        conditions: dto
            .conditions
            .iter()
            .map(|c| Condition {
                kind: c.kind,
                target_id: c.target_id,
                min_x: c.min_x,
                min_z: c.min_z,
                max_x: c.max_x,
                max_z: c.max_z,
            })
            .collect(),
        actions: dto
            .actions
            .iter()
            .map(|a| Action {
                kind: a.kind,
                text: a.text.clone(),
                team: a.team,
                other_team: a.other_team,
                hostile: a.hostile,
                alias: a.alias.clone(),
                x: a.x,
                z: a.z,
                count: a.count,
                move_destination: None,
            })
            .collect(),
    }
}

fn build_rust_graph(nodes_dto: &[DtoNode], edges_dto: &[DtoEdge]) -> EventGraph {
    let mut graph = EventGraph::new();

    for d in nodes_dto {
        let node = match d.kind.as_str() {
            "Trigger" => {
                let mut n = against_rome_scenario::EventTriggerNode::new(
                    &d.event_name,
                    d.delay_seconds,
                    d.repeat,
                    d.enabled,
                );
                n.id = d.id;
                n.x = d.x;
                n.y = d.y;
                n.exec_in.node_id = d.id;
                n.conditions_in.node_id = d.id;
                n.exec_out.node_id = d.id;
                GraphNode::Trigger(n)
            }
            "ExistsCondition" => {
                let mut n =
                    against_rome_scenario::ObjectExistsConditionNode::new(d.target_id, &d.target_label);
                n.id = d.id;
                n.x = d.x;
                n.y = d.y;
                n.condition_out.node_id = d.id;
                GraphNode::ExistsCondition(n)
            }
            "DeadCondition" => {
                let mut n =
                    against_rome_scenario::ObjectDeadConditionNode::new(d.target_id, &d.target_label);
                n.id = d.id;
                n.x = d.x;
                n.y = d.y;
                n.condition_out.node_id = d.id;
                GraphNode::DeadCondition(n)
            }
            "AreaCondition" => {
                let mut n = against_rome_scenario::ObjectInAreaConditionNode::new(
                    d.target_id,
                    d.min_x,
                    d.min_z,
                    d.max_x,
                    d.max_z,
                    &d.target_label,
                );
                n.id = d.id;
                n.x = d.x;
                n.y = d.y;
                n.condition_out.node_id = d.id;
                GraphNode::AreaCondition(n)
            }
            "MessageAction" => {
                let mut n = against_rome_scenario::MessageActionNode::new(&d.message_text);
                n.id = d.id;
                n.x = d.x;
                n.y = d.y;
                n.exec_in.node_id = d.id;
                n.exec_out.node_id = d.id;
                GraphNode::MessageAction(n)
            }
            "DiplomacyAction" => {
                let mut n = against_rome_scenario::DiplomacyActionNode::new(
                    d.team,
                    d.other_team,
                    d.hostile,
                );
                n.id = d.id;
                n.x = d.x;
                n.y = d.y;
                n.exec_in.node_id = d.id;
                n.exec_out.node_id = d.id;
                GraphNode::DiplomacyAction(n)
            }
            "SpawnUnitAction" => {
                let mut n = against_rome_scenario::SpawnUnitActionNode::new(
                    &d.alias, d.team, d.spawn_x, d.spawn_z, d.count,
                );
                n.id = d.id;
                n.x = d.x;
                n.y = d.y;
                n.exec_in.node_id = d.id;
                n.exec_out.node_id = d.id;
                GraphNode::SpawnUnitAction(n)
            }
            "VictoryAction" => {
                let mut n = against_rome_scenario::VictoryActionNode::new();
                n.id = d.id;
                n.x = d.x;
                n.y = d.y;
                n.exec_in.node_id = d.id;
                GraphNode::VictoryAction(n)
            }
            "DefeatAction" => {
                let mut n = against_rome_scenario::DefeatActionNode::new();
                n.id = d.id;
                n.x = d.x;
                n.y = d.y;
                n.exec_in.node_id = d.id;
                GraphNode::DefeatAction(n)
            }
            "Delay" => {
                let mut n = against_rome_scenario::DelayNode::new(d.seconds);
                n.id = d.id;
                n.x = d.x;
                n.y = d.y;
                n.exec_in.node_id = d.id;
                n.exec_out.node_id = d.id;
                GraphNode::Delay(n)
            }
            other => panic!("unknown node kind: {other}"),
        };
        graph.add_node(node).unwrap();
    }

    for e in edges_dto {
        let sp_id = {
            let sn = graph.find_node(e.source_node_id).unwrap();
            sn.find_port_by_name(&e.source_port_name).unwrap().id
        };
        let tp_id = {
            let tn = graph.find_node(e.target_node_id).unwrap();
            tn.find_port_by_name(&e.target_port_name).unwrap().id
        };
        let _ = graph.connect(sp_id, tp_id);
    }

    graph
}

/// Corpus and results frozen from the (since removed) C# EventGraph oracle (.NET 10). The request keeps the
/// exact node/edge ids that were fed to the oracle (they were random), so it must be loaded, not regenerated.
fn load_frozen_oracle() -> (OracleRequest, OracleResponse) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let read = |name: &str| {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).map(<[u8]>::to_vec).unwrap_or(bytes)
    };
    (
        serde_json::from_slice(&read("event_graph_oracle_request.json")).expect("frozen request"),
        serde_json::from_slice(&read("event_graph_oracle_response.json")).expect("frozen response"),
    )
}

#[test]
fn csharp_event_graph_frozen_differential() {
    let (request, oracle_response) = load_frozen_oracle();

    // 1. Compare FromScenarioEvents
    assert_eq!(
        request.from_scenario_events_cases.len(),
        oracle_response.from_scenario_events_results.len()
    );

    for (tc, csharp_res) in request
        .from_scenario_events_cases
        .iter()
        .zip(&oracle_response.from_scenario_events_results)
    {
        assert_eq!(&tc.name, &csharp_res.name);

        let rust_events: Vec<Event> = tc.events.iter().map(to_rust_event).collect();
        let rust_graph = EventGraphConverter::from_scenario_events(&rust_events);

        // Compare Node counts
        assert_eq!(
            rust_graph.nodes.len(),
            csharp_res.nodes.len(),
            "Node count mismatch in FromScenarioEvents case '{}'",
            tc.name
        );

        // Match each node by order and compare properties
        for (i, (r_node, c_node)) in rust_graph.nodes.iter().zip(&csharp_res.nodes).enumerate() {
            assert_eq!(
                r_node.category(),
                c_node.category,
                "Node #{i} category mismatch in '{}'",
                tc.name
            );
            assert_eq!(
                r_node.title(),
                c_node.title,
                "Node #{i} title mismatch in '{}'",
                tc.name
            );
            let (rx, ry) = r_node.position();
            assert!(
                (rx - c_node.x).abs() < 0.01 && (ry - c_node.y).abs() < 0.01,
                "Node #{i} position mismatch in '{}': Rust=({rx},{ry}) C#=({},{}); title={}",
                tc.name,
                c_node.x,
                c_node.y,
                r_node.title()
            );

            // Compare Ports
            let r_ports = r_node.ports();
            assert_eq!(
                r_ports.len(),
                c_node.ports.len(),
                "Node #{i} port count mismatch in '{}'",
                tc.name
            );
            for (rp, cp) in r_ports.iter().zip(&c_node.ports) {
                assert_eq!(rp.name, cp.name, "Port name mismatch in '{}'", tc.name);
                assert_eq!(
                    rp.display_name, cp.display_name,
                    "Port display_name mismatch in '{}'",
                    tc.name
                );
                assert_eq!(
                    match rp.port_type {
                        PortType::Execution => 0,
                        PortType::Condition => 1,
                        PortType::Data => 2,
                    },
                    cp.port_type,
                    "Port type mismatch in '{}'",
                    tc.name
                );
                assert_eq!(
                    match rp.direction {
                        PortDirection::Input => 0,
                        PortDirection::Output => 1,
                    },
                    cp.direction,
                    "Port direction mismatch in '{}'",
                    tc.name
                );
                assert_eq!(
                    rp.allow_multiple, cp.allow_multiple,
                    "Port allow_multiple mismatch in '{}'",
                    tc.name
                );
            }
        }

        // Compare Edges
        assert_eq!(
            rust_graph.edges.len(),
            csharp_res.edges.len(),
            "Edge count mismatch in FromScenarioEvents case '{}'",
            tc.name
        );

        // Map topological edge signatures: "{source_title}:{source_port} -> {target_title}:{target_port}"
        let mut rust_edge_sigs: Vec<String> = rust_graph
            .edges
            .iter()
            .map(|e| {
                let sn = rust_graph.find_node(e.source_node_id).unwrap();
                let tn = rust_graph.find_node(e.target_node_id).unwrap();
                let sp = sn.find_port(e.source_port_id).unwrap();
                let tp = tn.find_port(e.target_port_id).unwrap();
                format!("{}:{} -> {}:{}", sn.title(), sp.name, tn.title(), tp.name)
            })
            .collect();
        rust_edge_sigs.sort();

        let mut csharp_edge_sigs: Vec<String> = csharp_res
            .edges
            .iter()
            .map(|e| {
                let sn = csharp_res
                    .nodes
                    .iter()
                    .find(|n| n.id == e.source_node_id)
                    .unwrap();
                let tn = csharp_res
                    .nodes
                    .iter()
                    .find(|n| n.id == e.target_node_id)
                    .unwrap();
                format!(
                    "{}:{} -> {}:{}",
                    sn.title, e.source_port_name, tn.title, e.target_port_name
                )
            })
            .collect();
        csharp_edge_sigs.sort();

        assert_eq!(
            rust_edge_sigs, csharp_edge_sigs,
            "Edge signatures mismatch in FromScenarioEvents case '{}'",
            tc.name
        );
    }

    // 2. Compare ToScenarioEvents
    assert_eq!(
        request.to_scenario_events_cases.len(),
        oracle_response.to_scenario_events_results.len()
    );

    for (tc, csharp_res) in request
        .to_scenario_events_cases
        .iter()
        .zip(&oracle_response.to_scenario_events_results)
    {
        assert_eq!(&tc.name, &csharp_res.name);

        let rust_graph = build_rust_graph(&tc.nodes, &tc.edges);
        let rust_events = EventGraphConverter::to_scenario_events(&rust_graph).unwrap();

        assert_eq!(
            rust_events.len(),
            csharp_res.events.len(),
            "ToScenarioEvents event count mismatch in '{}'",
            tc.name
        );

        for (i, (rev, cev)) in rust_events.iter().zip(&csharp_res.events).enumerate() {
            assert_eq!(rev.name, cev.name, "Event #{i} name mismatch in '{}'", tc.name);
            assert_eq!(
                rev.delay_seconds, cev.delay_seconds,
                "Event #{i} delay mismatch in '{}'",
                tc.name
            );
            assert_eq!(
                rev.repeat, cev.repeat,
                "Event #{i} repeat mismatch in '{}'",
                tc.name
            );
            assert_eq!(
                rev.enabled, cev.enabled,
                "Event #{i} enabled mismatch in '{}'",
                tc.name
            );

            assert_eq!(
                rev.conditions.len(),
                cev.conditions.len(),
                "Event #{i} condition count mismatch in '{}'",
                tc.name
            );
            for (rc, cc) in rev.conditions.iter().zip(&cev.conditions) {
                assert_eq!(rc.kind, cc.kind, "Condition kind mismatch in '{}'", tc.name);
                assert_eq!(
                    rc.target_id, cc.target_id,
                    "Condition target_id mismatch in '{}'",
                    tc.name
                );
                assert_eq!(rc.min_x, cc.min_x);
                assert_eq!(rc.min_z, cc.min_z);
                assert_eq!(rc.max_x, cc.max_x);
                assert_eq!(rc.max_z, cc.max_z);
            }

            assert_eq!(
                rev.actions.len(),
                cev.actions.len(),
                "Event #{i} action count mismatch in '{}'",
                tc.name
            );
            for (ra, ca) in rev.actions.iter().zip(&cev.actions) {
                assert_eq!(ra.kind, ca.kind, "Action kind mismatch in '{}'", tc.name);
                assert_eq!(ra.text, ca.text, "Action text mismatch in '{}'", tc.name);
                assert_eq!(ra.team, ca.team, "Action team mismatch in '{}'", tc.name);
                assert_eq!(
                    ra.other_team, ca.other_team,
                    "Action other_team mismatch in '{}'",
                    tc.name
                );
                assert_eq!(
                    ra.hostile, ca.hostile,
                    "Action hostile mismatch in '{}'",
                    tc.name
                );
                assert_eq!(ra.alias, ca.alias, "Action alias mismatch in '{}'", tc.name);
                assert_eq!(ra.count, ca.count, "Action count mismatch in '{}'", tc.name);
                assert!((ra.x - ca.x).abs() < 0.01, "Action x mismatch in '{}'", tc.name);
                assert!((ra.z - ca.z).abs() < 0.01, "Action z mismatch in '{}'", tc.name);
            }
        }
    }

    // 3. Compare EventGraphValidator
    assert_eq!(
        request.validator_cases.len(),
        oracle_response.validator_results.len()
    );

    for (tc, csharp_res) in request
        .validator_cases
        .iter()
        .zip(&oracle_response.validator_results)
    {
        assert_eq!(&tc.name, &csharp_res.name);

        let rust_graph = build_rust_graph(&tc.nodes, &tc.edges);
        let valid_targets: Option<HashSet<Uuid>> =
            tc.valid_target_ids.as_ref().map(|v| v.iter().copied().collect());
        let rust_diags = EventGraphValidator::validate(
            &rust_graph,
            valid_targets.as_ref(),
            tc.valid_aliases.as_deref(),
        );

        assert_eq!(
            rust_diags.len(),
            csharp_res.diagnostics.len(),
            "Diagnostic count mismatch in '{}': Rust={:?} C#={:?}",
            tc.name,
            rust_diags.iter().map(|d| &d.code).collect::<Vec<_>>(),
            csharp_res.diagnostics.iter().map(|d| &d.code).collect::<Vec<_>>()
        );

        let mut r_diag_keys: Vec<(u8, String)> = rust_diags
            .iter()
            .map(|d| {
                let s = match d.severity {
                    GraphDiagnosticSeverity::Info => 0,
                    GraphDiagnosticSeverity::Warning => 1,
                    GraphDiagnosticSeverity::Error => 2,
                };
                (s, d.code.clone())
            })
            .collect();
        r_diag_keys.sort();

        let mut c_diag_keys: Vec<(u8, String)> = csharp_res
            .diagnostics
            .iter()
            .map(|d| (d.severity as u8, d.code.clone()))
            .collect();
        c_diag_keys.sort();

        assert_eq!(
            r_diag_keys, c_diag_keys,
            "Diagnostic codes/severity mismatch in '{}'",
            tc.name
        );
    }

    // 4. Compare ScenarioEventSession
    assert_eq!(
        request.session_cases.len(),
        oracle_response.session_results.len()
    );

    for (tc, csharp_res) in request
        .session_cases
        .iter()
        .zip(&oracle_response.session_results)
    {
        assert_eq!(&tc.name, &csharp_res.name);

        let mut session = ScenarioEventSession::new();
        if !tc.initial_events.is_empty() {
            let init: Vec<Event> = tc.initial_events.iter().map(to_rust_event).collect();
            session.load(&init);
        }

        assert_eq!(
            tc.operations.len(),
            csharp_res.steps.len(),
            "Session step count mismatch in '{}'",
            tc.name
        );

        for (op_idx, (op, c_step)) in tc.operations.iter().zip(&csharp_res.steps).enumerate() {
            let rust_success = match op.op.as_str() {
                "Add" => {
                    let ev = to_rust_event(op.event.as_ref().unwrap());
                    session.add(ev).is_ok()
                }
                "AddRange" => {
                    let items: Vec<Event> = op
                        .events
                        .as_ref()
                        .unwrap()
                        .iter()
                        .map(to_rust_event)
                        .collect();
                    session.add_range(&items).is_ok()
                }
                "Replace" => {
                    let ev = to_rust_event(op.event.as_ref().unwrap());
                    session.replace(op.index, ev).is_ok()
                }
                "ReplaceAll" => {
                    let items: Vec<Event> = op
                        .events
                        .as_ref()
                        .unwrap()
                        .iter()
                        .map(to_rust_event)
                        .collect();
                    session.replace_all(&items).is_ok()
                }
                "RemoveAt" => session.remove_at(op.index).is_ok(),
                "Duplicate" => session.duplicate(op.index, &op.suffix).is_ok(),
                "Undo" => session.undo(),
                "Redo" => session.redo(),
                "Reset" => {
                    session.reset();
                    true
                }
                "AcceptChanges" => {
                    session.accept_changes();
                    true
                }
                "AcceptBaseline" => {
                    let items: Vec<Event> = op
                        .events
                        .as_ref()
                        .unwrap()
                        .iter()
                        .map(to_rust_event)
                        .collect();
                    session.accept_baseline(&items);
                    true
                }
                "Load" => {
                    let items: Vec<Event> = op
                        .events
                        .as_ref()
                        .unwrap()
                        .iter()
                        .map(to_rust_event)
                        .collect();
                    session.load(&items);
                    true
                }
                other => panic!("unknown session op: {other}"),
            };

            assert_eq!(
                rust_success, c_step.success,
                "Op #{op_idx} '{}' success mismatch in '{}'",
                op.op, tc.name
            );

            assert_eq!(
                session.count(),
                c_step.count,
                "Op #{op_idx} '{}' count mismatch in '{}'",
                op.op,
                tc.name
            );

            assert_eq!(
                session.is_dirty(),
                c_step.is_dirty,
                "Op #{op_idx} '{}' is_dirty mismatch in '{}'",
                op.op,
                tc.name
            );

            assert_eq!(
                session.can_undo(),
                c_step.can_undo,
                "Op #{op_idx} '{}' can_undo mismatch in '{}'",
                op.op,
                tc.name
            );

            assert_eq!(
                session.can_redo(),
                c_step.can_redo,
                "Op #{op_idx} '{}' can_redo mismatch in '{}'",
                op.op,
                tc.name
            );

            let rust_names: Vec<String> = session.capture().into_iter().map(|e| e.name).collect();
            assert_eq!(
                rust_names, c_step.event_names,
                "Op #{op_idx} '{}' event names mismatch in '{}'",
                op.op, tc.name
            );

            let rust_baseline_names: Vec<String> =
                session.baseline().iter().map(|e| e.name.clone()).collect();
            assert_eq!(
                rust_baseline_names, c_step.baseline_names,
                "Op #{op_idx} '{}' baseline names mismatch in '{}'",
                op.op, tc.name
            );
        }
    }

    println!(
        ".NET 10 differential parity confirmed: {} FromScenarioEvents cases, {} ToScenarioEvents cases, {} Validator cases, {} Session cases ({} total session operations).",
        request.from_scenario_events_cases.len(),
        request.to_scenario_events_cases.len(),
        request.validator_cases.len(),
        request.session_cases.len(),
        request.session_cases.iter().map(|s| s.operations.len()).sum::<usize>()
    );
}

