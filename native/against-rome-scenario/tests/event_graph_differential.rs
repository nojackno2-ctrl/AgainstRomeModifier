use against_rome_scenario::{
    Action, Condition, Event, EventGraph, EventGraphConverter, EventGraphValidator,
    GraphDiagnosticSeverity, GraphNode, PortDirection, PortType, ScenarioEventSession,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;
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

fn execute_csharp_oracle(request: &OracleRequest) -> OracleResponse {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let temp_dir = std::env::temp_dir().join(format!("arm_eg_diff_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");

    let req_path = temp_dir.join("request.json");
    let resp_path = temp_dir.join("response.json");

    let req_bytes = serde_json::to_vec_pretty(request).expect("serialize request");
    std::fs::write(&req_path, req_bytes).expect("write request.json");

    let project_path = manifest_dir.join("tests/event_graph_oracle/EventGraphOracle.csproj");

    let output = Command::new("dotnet")
        .arg("run")
        .arg("--project")
        .arg(&project_path)
        .arg("-c")
        .arg("Release")
        .arg(&req_path)
        .arg(&resp_path)
        .output()
        .expect("failed to spawn dotnet");

    if !output.status.success() {
        panic!(
            "EventGraphOracle execution failed with code {:?}\nstdout: {}\nstderr: {}",
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

fn generate_synthetic_corpus() -> OracleRequest {
    let target1 = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    let target2 = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
    let target3 = Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap();

    // 1. FromScenarioEvents test cases
    let from_cases = vec![
        FromScenarioEventsTestCase {
            name: "empty_events".into(),
            events: vec![],
        },
        FromScenarioEventsTestCase {
            name: "single_trigger_simple".into(),
            events: vec![DtoEvent {
                name: "MissionStart".into(),
                delay_seconds: 5,
                repeat: false,
                enabled: true,
                conditions: vec![],
                actions: vec![],
            }],
        },
        FromScenarioEventsTestCase {
            name: "single_trigger_repeat".into(),
            events: vec![DtoEvent {
                name: "PeriodicReinforce".into(),
                delay_seconds: 30,
                repeat: true,
                enabled: true,
                conditions: vec![],
                actions: vec![],
            }],
        },
        FromScenarioEventsTestCase {
            name: "single_with_conditions_and_actions".into(),
            events: vec![DtoEvent {
                name: "DefeatBoss".into(),
                delay_seconds: 10,
                repeat: false,
                enabled: true,
                conditions: vec![
                    DtoCondition {
                        kind: 0,
                        target_id: target1,
                        min_x: 0,
                        min_z: 0,
                        max_x: 16383,
                        max_z: 16383,
                    },
                    DtoCondition {
                        kind: 1,
                        target_id: target2,
                        min_x: 0,
                        min_z: 0,
                        max_x: 16383,
                        max_z: 16383,
                    },
                    DtoCondition {
                        kind: 2,
                        target_id: target3,
                        min_x: 100,
                        min_z: 200,
                        max_x: 500,
                        max_z: 600,
                    },
                ],
                actions: vec![
                    DtoAction {
                        kind: 0,
                        text: "The chieftain has fallen!".into(),
                        team: 0,
                        other_team: 1,
                        hostile: true,
                        alias: "".into(),
                        x: 0.0,
                        z: 0.0,
                        count: 1,
                    },
                    DtoAction {
                        kind: 1,
                        text: "".into(),
                        team: 0,
                        other_team: 2,
                        hostile: false,
                        alias: "".into(),
                        x: 0.0,
                        z: 0.0,
                        count: 1,
                    },
                    DtoAction {
                        kind: 2,
                        text: "".into(),
                        team: 0,
                        other_team: 1,
                        hostile: true,
                        alias: "ROM_LEG01".into(),
                        x: 500.0,
                        z: 600.0,
                        count: 15,
                    },
                    DtoAction {
                        kind: 3,
                        text: "".into(),
                        team: 0,
                        other_team: 1,
                        hostile: true,
                        alias: "".into(),
                        x: 0.0,
                        z: 0.0,
                        count: 1,
                    },
                ],
            }],
        },
        FromScenarioEventsTestCase {
            name: "multi_event_layout_hierarchy".into(),
            events: vec![
                DtoEvent {
                    name: "Event1_StageA".into(),
                    delay_seconds: 0,
                    repeat: false,
                    enabled: true,
                    conditions: vec![DtoCondition {
                        kind: 0,
                        target_id: target1,
                        min_x: 0,
                        min_z: 0,
                        max_x: 16383,
                        max_z: 16383,
                    }],
                    actions: vec![DtoAction {
                        kind: 0,
                        text: "Stage A complete".into(),
                        team: 0,
                        other_team: 1,
                        hostile: true,
                        alias: "".into(),
                        x: 0.0,
                        z: 0.0,
                        count: 1,
                    }],
                },
                DtoEvent {
                    name: "Event2_StageB".into(),
                    delay_seconds: 15,
                    repeat: true,
                    enabled: true,
                    conditions: vec![
                        DtoCondition {
                            kind: 1,
                            target_id: target2,
                            min_x: 0,
                            min_z: 0,
                            max_x: 16383,
                            max_z: 16383,
                        },
                        DtoCondition {
                            kind: 2,
                            target_id: target3,
                            min_x: 200,
                            min_z: 300,
                            max_x: 800,
                            max_z: 900,
                        },
                    ],
                    actions: vec![
                        DtoAction {
                            kind: 2,
                            text: "".into(),
                            team: 1,
                            other_team: 0,
                            hostile: true,
                            alias: "GER_INF01".into(),
                            x: 1200.0,
                            z: 1400.0,
                            count: 8,
                        },
                        DtoAction {
                            kind: 4,
                            text: "".into(),
                            team: 0,
                            other_team: 1,
                            hostile: true,
                            alias: "".into(),
                            x: 0.0,
                            z: 0.0,
                            count: 1,
                        },
                    ],
                },
                DtoEvent {
                    name: "Event3_NoConditions".into(),
                    delay_seconds: 60,
                    repeat: false,
                    enabled: false,
                    conditions: vec![],
                    actions: vec![DtoAction {
                        kind: 0,
                        text: "Timeout".into(),
                        team: 0,
                        other_team: 1,
                        hostile: true,
                        alias: "".into(),
                        x: 0.0,
                        z: 0.0,
                        count: 1,
                    }],
                },
            ],
        },
    ];

    // 2. ToScenarioEvents test cases (including terminal nodes, flow branches, cycles)
    let tr1 = Uuid::parse_str("aaaaaaaa-0001-0001-0001-000000000001").unwrap();
    let c1 = Uuid::parse_str("aaaaaaaa-0001-0001-0001-000000000002").unwrap();
    let c2 = Uuid::parse_str("aaaaaaaa-0001-0001-0001-000000000007").unwrap();
    let a1 = Uuid::parse_str("aaaaaaaa-0001-0001-0001-000000000003").unwrap();
    let a2 = Uuid::parse_str("aaaaaaaa-0001-0001-0001-000000000004").unwrap();
    let a3 = Uuid::parse_str("aaaaaaaa-0001-0001-0001-000000000005").unwrap();
    let _a4 = Uuid::parse_str("aaaaaaaa-0001-0001-0001-000000000006").unwrap();

    let to_cases = vec![
        GraphTestCase {
            name: "to_events_standard_linear".into(),
            nodes: vec![
                DtoNode {
                    id: tr1,
                    kind: "Trigger".into(),
                    event_name: "SimpleLinear".into(),
                    delay_seconds: 10,
                    repeat: false,
                    enabled: true,
                    ..default_dto_node()
                },
                DtoNode {
                    id: c1,
                    kind: "ExistsCondition".into(),
                    target_id: target1,
                    target_label: "Chief".into(),
                    ..default_dto_node()
                },
                DtoNode {
                    id: a1,
                    kind: "MessageAction".into(),
                    message_text: "Hello".into(),
                    ..default_dto_node()
                },
                DtoNode {
                    id: a2,
                    kind: "DiplomacyAction".into(),
                    team: 0,
                    other_team: 2,
                    hostile: false,
                    ..default_dto_node()
                },
            ],
            edges: vec![
                DtoEdge {
                    source_node_id: c1,
                    source_port_name: "ConditionOut".into(),
                    target_node_id: tr1,
                    target_port_name: "ConditionsIn".into(),
                },
                DtoEdge {
                    source_node_id: tr1,
                    source_port_name: "ExecOut".into(),
                    target_node_id: a1,
                    target_port_name: "ExecIn".into(),
                },
                DtoEdge {
                    source_node_id: a1,
                    source_port_name: "ExecOut".into(),
                    target_node_id: a2,
                    target_port_name: "ExecIn".into(),
                },
            ],
        },
        GraphTestCase {
            name: "to_events_victory_termination".into(),
            nodes: vec![
                DtoNode {
                    id: tr1,
                    kind: "Trigger".into(),
                    event_name: "VictoryStop".into(),
                    delay_seconds: 5,
                    repeat: false,
                    enabled: true,
                    ..default_dto_node()
                },
                DtoNode {
                    id: a1,
                    kind: "MessageAction".into(),
                    message_text: "You won!".into(),
                    ..default_dto_node()
                },
                DtoNode {
                    id: a2,
                    kind: "VictoryAction".into(),
                    ..default_dto_node()
                },
                DtoNode {
                    id: a3,
                    kind: "SpawnUnitAction".into(),
                    alias: "UNREACHABLE".into(),
                    team: 0,
                    spawn_x: 100.0,
                    spawn_z: 100.0,
                    count: 1,
                    ..default_dto_node()
                },
            ],
            edges: vec![
                DtoEdge {
                    source_node_id: tr1,
                    source_port_name: "ExecOut".into(),
                    target_node_id: a1,
                    target_port_name: "ExecIn".into(),
                },
                DtoEdge {
                    source_node_id: a1,
                    source_port_name: "ExecOut".into(),
                    target_node_id: a2,
                    target_port_name: "ExecIn".into(),
                },
            ],
        },

        GraphTestCase {
            name: "to_events_defeat_termination".into(),
            nodes: vec![
                DtoNode {
                    id: tr1,
                    kind: "Trigger".into(),
                    event_name: "DefeatStop".into(),
                    delay_seconds: 0,
                    repeat: false,
                    enabled: true,
                    ..default_dto_node()
                },
                DtoNode {
                    id: a1,
                    kind: "DefeatAction".into(),
                    ..default_dto_node()
                },
            ],
            edges: vec![DtoEdge {
                source_node_id: tr1,
                source_port_name: "ExecOut".into(),
                target_node_id: a1,
                target_port_name: "ExecIn".into(),
            }],
        },
        GraphTestCase {
            name: "to_events_action_cycle_protection".into(),
            nodes: vec![
                DtoNode {
                    id: tr1,
                    kind: "Trigger".into(),
                    event_name: "CycleEvent".into(),
                    delay_seconds: 5,
                    repeat: false,
                    enabled: true,
                    ..default_dto_node()
                },
                DtoNode {
                    id: a1,
                    kind: "MessageAction".into(),
                    message_text: "LoopStep1".into(),
                    ..default_dto_node()
                },
                DtoNode {
                    id: a2,
                    kind: "MessageAction".into(),
                    message_text: "LoopStep2".into(),
                    ..default_dto_node()
                },
            ],
            edges: vec![
                DtoEdge {
                    source_node_id: tr1,
                    source_port_name: "ExecOut".into(),
                    target_node_id: a1,
                    target_port_name: "ExecIn".into(),
                },
                DtoEdge {
                    source_node_id: a1,
                    source_port_name: "ExecOut".into(),
                    target_node_id: a2,
                    target_port_name: "ExecIn".into(),
                },
                DtoEdge {
                    source_node_id: a2,
                    source_port_name: "ExecOut".into(),
                    target_node_id: a1,
                    target_port_name: "ExecIn".into(),
                },
            ],
        },
        GraphTestCase {
            name: "to_events_stops_at_unsupported_delay_node".into(),
            nodes: vec![
                DtoNode {
                    id: tr1,
                    kind: "Trigger".into(),
                    event_name: "DelayInterrupted".into(),
                    delay_seconds: 1,
                    repeat: false,
                    enabled: true,
                    ..default_dto_node()
                },
                DtoNode {
                    id: a1,
                    kind: "MessageAction".into(),
                    message_text: "BeforeDelay".into(),
                    ..default_dto_node()
                },
                DtoNode {
                    id: a2,
                    kind: "Delay".into(),
                    seconds: 5,
                    ..default_dto_node()
                },
                DtoNode {
                    id: a3,
                    kind: "MessageAction".into(),
                    message_text: "AfterDelay".into(),
                    ..default_dto_node()
                },
            ],
            edges: vec![
                DtoEdge {
                    source_node_id: tr1,
                    source_port_name: "ExecOut".into(),
                    target_node_id: a1,
                    target_port_name: "ExecIn".into(),
                },
                DtoEdge {
                    source_node_id: a1,
                    source_port_name: "ExecOut".into(),
                    target_node_id: a2,
                    target_port_name: "ExecIn".into(),
                },
                DtoEdge {
                    source_node_id: a2,
                    source_port_name: "ExecOut".into(),
                    target_node_id: a3,
                    target_port_name: "ExecIn".into(),
                },
            ],
        },
    ];

    // 3. Validator test cases
    let mut val_cases = Vec::new();

    // Valid clean graph
    val_cases.push(ValidatorTestCase {
        name: "val_clean_valid".into(),
        nodes: vec![
            DtoNode {
                id: tr1,
                kind: "Trigger".into(),
                event_name: "CleanEvent".into(),
                delay_seconds: 5,
                repeat: false,
                enabled: true,
                ..default_dto_node()
            },
            DtoNode {
                id: c1,
                kind: "ExistsCondition".into(),
                target_id: target1,
                target_label: "Chief".into(),
                ..default_dto_node()
            },
            DtoNode {
                id: a1,
                kind: "MessageAction".into(),
                message_text: "Clean Message".into(),
                ..default_dto_node()
            },
        ],
        edges: vec![
            DtoEdge {
                source_node_id: c1,
                source_port_name: "ConditionOut".into(),
                target_node_id: tr1,
                target_port_name: "ConditionsIn".into(),
            },
            DtoEdge {
                source_node_id: tr1,
                source_port_name: "ExecOut".into(),
                target_node_id: a1,
                target_port_name: "ExecIn".into(),
            },
        ],
        valid_target_ids: Some(vec![target1]),
        valid_aliases: Some(vec!["GER_INF01".into()]),
    });

    // Empty event name
    val_cases.push(ValidatorTestCase {
        name: "val_empty_event_name".into(),
        nodes: vec![DtoNode {
            id: tr1,
            kind: "Trigger".into(),
            event_name: "   ".into(),
            delay_seconds: 5,
            repeat: false,
            enabled: true,
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Invalid delay: negative
    val_cases.push(ValidatorTestCase {
        name: "val_negative_delay".into(),
        nodes: vec![DtoNode {
            id: tr1,
            kind: "Trigger".into(),
            event_name: "NegativeDelay".into(),
            delay_seconds: -1,
            repeat: false,
            enabled: true,
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Invalid delay: over 86400
    val_cases.push(ValidatorTestCase {
        name: "val_excessive_delay".into(),
        nodes: vec![DtoNode {
            id: tr1,
            kind: "Trigger".into(),
            event_name: "OverDayDelay".into(),
            delay_seconds: 90000,
            repeat: false,
            enabled: true,
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Repeat zero delay
    val_cases.push(ValidatorTestCase {
        name: "val_repeat_zero_delay".into(),
        nodes: vec![DtoNode {
            id: tr1,
            kind: "Trigger".into(),
            event_name: "RapidTick".into(),
            delay_seconds: 0,
            repeat: true,
            enabled: true,
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Terminal in repeat event: Victory
    val_cases.push(ValidatorTestCase {
        name: "val_terminal_victory_in_repeat".into(),
        nodes: vec![
            DtoNode {
                id: tr1,
                kind: "Trigger".into(),
                event_name: "RepeatWin".into(),
                delay_seconds: 10,
                repeat: true,
                enabled: true,
                ..default_dto_node()
            },
            DtoNode {
                id: a1,
                kind: "VictoryAction".into(),
                ..default_dto_node()
            },
        ],
        edges: vec![DtoEdge {
            source_node_id: tr1,
            source_port_name: "ExecOut".into(),
            target_node_id: a1,
            target_port_name: "ExecIn".into(),
        }],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Terminal in repeat event: Defeat
    val_cases.push(ValidatorTestCase {
        name: "val_terminal_defeat_in_repeat".into(),
        nodes: vec![
            DtoNode {
                id: tr1,
                kind: "Trigger".into(),
                event_name: "RepeatLoss".into(),
                delay_seconds: 10,
                repeat: true,
                enabled: true,
                ..default_dto_node()
            },
            DtoNode {
                id: a1,
                kind: "DefeatAction".into(),
                ..default_dto_node()
            },
        ],
        edges: vec![DtoEdge {
            source_node_id: tr1,
            source_port_name: "ExecOut".into(),
            target_node_id: a1,
            target_port_name: "ExecIn".into(),
        }],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Empty target GUID
    val_cases.push(ValidatorTestCase {
        name: "val_empty_target_guid".into(),
        nodes: vec![DtoNode {
            id: c1,
            kind: "ExistsCondition".into(),
            target_id: Uuid::nil(),
            target_label: "NilTarget".into(),
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Invalid target GUID (not found in valid_targets)
    val_cases.push(ValidatorTestCase {
        name: "val_invalid_target_guid".into(),
        nodes: vec![DtoNode {
            id: c1,
            kind: "ExistsCondition".into(),
            target_id: target2,
            target_label: "MissingTarget".into(),
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: Some(vec![target1]),
        valid_aliases: None,
    });

    // Invalid area bounds: min > max
    val_cases.push(ValidatorTestCase {
        name: "val_invalid_area_bounds_inverted".into(),
        nodes: vec![DtoNode {
            id: c1,
            kind: "AreaCondition".into(),
            target_id: target1,
            min_x: 500,
            min_z: 100,
            max_x: 200,
            max_z: 600,
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Invalid area bounds: negative min
    val_cases.push(ValidatorTestCase {
        name: "val_invalid_area_bounds_negative".into(),
        nodes: vec![DtoNode {
            id: c1,
            kind: "AreaCondition".into(),
            target_id: target1,
            min_x: -10,
            min_z: 0,
            max_x: 1000,
            max_z: 1000,
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Invalid area bounds: over 16383
    val_cases.push(ValidatorTestCase {
        name: "val_invalid_area_bounds_overflow".into(),
        nodes: vec![DtoNode {
            id: c1,
            kind: "AreaCondition".into(),
            target_id: target1,
            min_x: 0,
            min_z: 0,
            max_x: 20000,
            max_z: 1000,
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Dangling condition (no outgoing edges)
    val_cases.push(ValidatorTestCase {
        name: "val_dangling_condition".into(),
        nodes: vec![DtoNode {
            id: c1,
            kind: "DeadCondition".into(),
            target_id: target1,
            target_label: "OrphanDead".into(),
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Orphan action (no incoming exec edges)
    val_cases.push(ValidatorTestCase {
        name: "val_orphan_action".into(),
        nodes: vec![DtoNode {
            id: a1,
            kind: "MessageAction".into(),
            message_text: "Unreachable".into(),
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Empty message text
    val_cases.push(ValidatorTestCase {
        name: "val_empty_message_text".into(),
        nodes: vec![DtoNode {
            id: a1,
            kind: "MessageAction".into(),
            message_text: "   ".into(),
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Invalid diplomacy: same team
    val_cases.push(ValidatorTestCase {
        name: "val_diplomacy_same_team".into(),
        nodes: vec![DtoNode {
            id: a1,
            kind: "DiplomacyAction".into(),
            team: 1,
            other_team: 1,
            hostile: true,
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Invalid diplomacy: team out of range (8)
    val_cases.push(ValidatorTestCase {
        name: "val_diplomacy_team_overflow".into(),
        nodes: vec![DtoNode {
            id: a1,
            kind: "DiplomacyAction".into(),
            team: 8,
            other_team: 0,
            hostile: true,
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Invalid spawn: team < 0 or > 7
    val_cases.push(ValidatorTestCase {
        name: "val_spawn_invalid_team".into(),
        nodes: vec![DtoNode {
            id: a1,
            kind: "SpawnUnitAction".into(),
            team: 9,
            alias: "GER_INF01".into(),
            spawn_x: 100.0,
            spawn_z: 100.0,
            count: 5,
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Invalid spawn: count 0 or > 20
    val_cases.push(ValidatorTestCase {
        name: "val_spawn_count_bounds".into(),
        nodes: vec![
            DtoNode {
                id: a1,
                kind: "SpawnUnitAction".into(),
                team: 0,
                alias: "GER_INF01".into(),
                spawn_x: 100.0,
                spawn_z: 100.0,
                count: 0,
                ..default_dto_node()
            },
            DtoNode {
                id: a2,
                kind: "SpawnUnitAction".into(),
                team: 0,
                alias: "GER_INF01".into(),
                spawn_x: 100.0,
                spawn_z: 100.0,
                count: 25,
                ..default_dto_node()
            },
        ],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Invalid spawn coordinates: negative or overflow
    val_cases.push(ValidatorTestCase {
        name: "val_spawn_coordinates_overflow".into(),
        nodes: vec![DtoNode {
            id: a1,
            kind: "SpawnUnitAction".into(),
            team: 0,
            alias: "GER_INF01".into(),
            spawn_x: -50.0,
            spawn_z: 20000.0,
            count: 5,
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Invalid spawn alias: unknown
    val_cases.push(ValidatorTestCase {
        name: "val_spawn_unknown_alias".into(),
        nodes: vec![DtoNode {
            id: a1,
            kind: "SpawnUnitAction".into(),
            team: 0,
            alias: "MYSTERY_UNIT".into(),
            spawn_x: 500.0,
            spawn_z: 500.0,
            count: 5,
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: Some(vec!["ROM_LEG01".into(), "GER_INF01".into()]),
    });

    // Orphan delay and negative delay
    val_cases.push(ValidatorTestCase {
        name: "val_delay_orphan_and_negative".into(),
        nodes: vec![DtoNode {
            id: a1,
            kind: "Delay".into(),
            seconds: -5,
            ..default_dto_node()
        }],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Execution cycle: 2 nodes
    val_cases.push(ValidatorTestCase {
        name: "val_execution_cycle_two_nodes".into(),
        nodes: vec![
            DtoNode {
                id: a1,
                kind: "MessageAction".into(),
                message_text: "CycleNode1".into(),
                ..default_dto_node()
            },
            DtoNode {
                id: a2,
                kind: "MessageAction".into(),
                message_text: "CycleNode2".into(),
                ..default_dto_node()
            },
        ],
        edges: vec![
            DtoEdge {
                source_node_id: a1,
                source_port_name: "ExecOut".into(),
                target_node_id: a2,
                target_port_name: "ExecIn".into(),
            },
            DtoEdge {
                source_node_id: a2,
                source_port_name: "ExecOut".into(),
                target_node_id: a1,
                target_port_name: "ExecIn".into(),
            },
        ],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Execution cycle: 3 nodes
    val_cases.push(ValidatorTestCase {
        name: "val_execution_cycle_three_nodes".into(),
        nodes: vec![
            DtoNode {
                id: a1,
                kind: "MessageAction".into(),
                message_text: "N1".into(),
                ..default_dto_node()
            },
            DtoNode {
                id: a2,
                kind: "DiplomacyAction".into(),
                team: 0,
                other_team: 1,
                hostile: true,
                ..default_dto_node()
            },
            DtoNode {
                id: a3,
                kind: "Delay".into(),
                seconds: 5,
                ..default_dto_node()
            },
        ],
        edges: vec![
            DtoEdge {
                source_node_id: a1,
                source_port_name: "ExecOut".into(),
                target_node_id: a2,
                target_port_name: "ExecIn".into(),
            },
            DtoEdge {
                source_node_id: a2,
                source_port_name: "ExecOut".into(),
                target_node_id: a3,
                target_port_name: "ExecIn".into(),
            },
            DtoEdge {
                source_node_id: a3,
                source_port_name: "ExecOut".into(),
                target_node_id: a1,
                target_port_name: "ExecIn".into(),
            },
        ],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Compound complex graph
    val_cases.push(ValidatorTestCase {
        name: "val_compound_edge_cases".into(),
        nodes: vec![
            DtoNode {
                id: tr1,
                kind: "Trigger".into(),
                event_name: "Compound".into(),
                delay_seconds: 0,
                repeat: true,
                enabled: true,
                ..default_dto_node()
            },
            DtoNode {
                id: c1,
                kind: "ExistsCondition".into(),
                target_id: Uuid::nil(),
                ..default_dto_node()
            },
            DtoNode {
                id: a1,
                kind: "VictoryAction".into(),
                ..default_dto_node()
            },
            DtoNode {
                id: a2,
                kind: "Delay".into(),
                seconds: -1,
                ..default_dto_node()
            },
            DtoNode {
                id: a3,
                kind: "SpawnUnitAction".into(),
                alias: "INVALID".into(),
                team: 9,
                count: 50,
                spawn_x: -100.0,
                spawn_z: 25000.0,
                ..default_dto_node()
            },
        ],
        edges: vec![
            DtoEdge {
                source_node_id: tr1,
                source_port_name: "ExecOut".into(),
                target_node_id: a1,
                target_port_name: "ExecIn".into(),
            },
            DtoEdge {
                source_node_id: a2,
                source_port_name: "ExecOut".into(),
                target_node_id: a3,
                target_port_name: "ExecIn".into(),
            },
        ],
        valid_target_ids: Some(vec![target1]),
        valid_aliases: Some(vec!["ROM_LEG01".into()]),
    });

    // Condition limit exceeded (33 conditions connected to trigger)
    let mut cond_nodes = vec![DtoNode {
        id: tr1,
        kind: "Trigger".into(),
        event_name: "OverConditions".into(),
        delay_seconds: 5,
        repeat: false,
        enabled: true,
        ..default_dto_node()
    }];
    let mut cond_edges = Vec::new();
    for i in 0..33 {
        let cid = Uuid::new_v4();
        cond_nodes.push(DtoNode {
            id: cid,
            kind: "ExistsCondition".into(),
            target_id: target1,
            target_label: format!("T{i}"),
            ..default_dto_node()
        });
        cond_edges.push(DtoEdge {
            source_node_id: cid,
            source_port_name: "ConditionOut".into(),
            target_node_id: tr1,
            target_port_name: "ConditionsIn".into(),
        });
    }
    val_cases.push(ValidatorTestCase {
        name: "val_condition_limit_exceeded".into(),
        nodes: cond_nodes,
        edges: cond_edges,
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Action limit exceeded (33 actions in sequence)
    let mut act_nodes = vec![DtoNode {
        id: tr1,
        kind: "Trigger".into(),
        event_name: "OverActions".into(),
        delay_seconds: 5,
        repeat: false,
        enabled: true,
        ..default_dto_node()
    }];
    let mut act_edges = Vec::new();
    let mut prev_id = tr1;
    let mut prev_port = "ExecOut";
    for i in 0..33 {
        let aid = Uuid::new_v4();
        act_nodes.push(DtoNode {
            id: aid,
            kind: "MessageAction".into(),
            message_text: format!("Msg_{i}"),
            ..default_dto_node()
        });
        act_edges.push(DtoEdge {
            source_node_id: prev_id,
            source_port_name: prev_port.into(),
            target_node_id: aid,
            target_port_name: "ExecIn".into(),
        });
        prev_id = aid;
        prev_port = "ExecOut";
    }
    val_cases.push(ValidatorTestCase {
        name: "val_action_limit_exceeded".into(),
        nodes: act_nodes,
        edges: act_edges,
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Event limit exceeded (257 triggers in graph)
    let mut trig_nodes = Vec::new();
    for i in 0..257 {
        trig_nodes.push(DtoNode {
            id: Uuid::new_v4(),
            kind: "Trigger".into(),
            event_name: format!("Trig_{i}"),
            delay_seconds: 1,
            repeat: false,
            enabled: true,
            ..default_dto_node()
        });
    }
    val_cases.push(ValidatorTestCase {
        name: "val_event_limit_exceeded".into(),
        nodes: trig_nodes,
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Execution cycle involving trigger
    val_cases.push(ValidatorTestCase {
        name: "val_execution_cycle_with_trigger".into(),
        nodes: vec![
            DtoNode {
                id: tr1,
                kind: "Trigger".into(),
                event_name: "CycleTrig".into(),
                delay_seconds: 5,
                repeat: false,
                enabled: true,
                ..default_dto_node()
            },
            DtoNode {
                id: a1,
                kind: "MessageAction".into(),
                message_text: "CycleBack".into(),
                ..default_dto_node()
            },
        ],
        edges: vec![
            DtoEdge {
                source_node_id: tr1,
                source_port_name: "ExecOut".into(),
                target_node_id: a1,
                target_port_name: "ExecIn".into(),
            },
            DtoEdge {
                source_node_id: a1,
                source_port_name: "ExecOut".into(),
                target_node_id: tr1,
                target_port_name: "ExecIn".into(),
            },
        ],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // Multiple orphans and dangling conditions
    val_cases.push(ValidatorTestCase {
        name: "val_multiple_orphans_and_dangling".into(),
        nodes: vec![
            DtoNode {
                id: c1,
                kind: "ExistsCondition".into(),
                target_id: target1,
                target_label: "Dangling1".into(),
                ..default_dto_node()
            },
            DtoNode {
                id: c2,
                kind: "DeadCondition".into(),
                target_id: target2,
                target_label: "Dangling2".into(),
                ..default_dto_node()
            },
            DtoNode {
                id: a1,
                kind: "MessageAction".into(),
                message_text: "OrphanMsg1".into(),
                ..default_dto_node()
            },
            DtoNode {
                id: a2,
                kind: "DiplomacyAction".into(),
                team: 0,
                other_team: 1,
                hostile: true,
                ..default_dto_node()
            },
            DtoNode {
                id: a3,
                kind: "Delay".into(),
                seconds: 10,
                ..default_dto_node()
            },
        ],
        edges: vec![],
        valid_target_ids: None,
        valid_aliases: None,
    });

    // 4. ScenarioEventSession test cases
    let mut session_cases = Vec::new();


    // Session Test 1: Full lifecycle (Add, AddRange, Replace, RemoveAt, Duplicate, Undo, Redo, Reset, IsDirty)
    let ev1 = DtoEvent {
        name: "InitialAlpha".into(),
        delay_seconds: 5,
        repeat: false,
        enabled: true,
        conditions: vec![],
        actions: vec![],
    };
    let ev2 = DtoEvent {
        name: "InitialBeta".into(),
        delay_seconds: 10,
        repeat: true,
        enabled: true,
        conditions: vec![],
        actions: vec![],
    };
    let ev_extra1 = DtoEvent {
        name: "ExtraGamma".into(),
        delay_seconds: 2,
        repeat: false,
        enabled: true,
        conditions: vec![],
        actions: vec![],
    };
    let ev_extra2 = DtoEvent {
        name: "ExtraDelta".into(),
        delay_seconds: 8,
        repeat: false,
        enabled: true,
        conditions: vec![],
        actions: vec![],
    };
    let ev_repl = DtoEvent {
        name: "ReplacedAlpha".into(),
        delay_seconds: 99,
        repeat: false,
        enabled: true,
        conditions: vec![],
        actions: vec![],
    };

    session_cases.push(SessionTestCase {
        name: "session_lifecycle_and_dirty_tracking".into(),
        initial_events: vec![ev1.clone(), ev2.clone()],
        operations: vec![
            DtoSessionOp {
                op: "Add".into(),
                event: Some(ev_extra1.clone()),
                events: None,
                index: 0,
                suffix: "".into(),
            },
            DtoSessionOp {
                op: "AddRange".into(),
                event: None,
                events: Some(vec![ev_extra2.clone()]),
                index: 0,
                suffix: "".into(),
            },
            DtoSessionOp {
                op: "AcceptChanges".into(),
                event: None,
                events: None,
                index: 0,
                suffix: "".into(),
            },
            DtoSessionOp {
                op: "Replace".into(),
                event: Some(ev_repl.clone()),
                events: None,
                index: 0,
                suffix: "".into(),
            },
            DtoSessionOp {
                op: "Duplicate".into(),
                event: None,
                events: None,
                index: 1,
                suffix: "_copy".into(),
            },
            DtoSessionOp {
                op: "RemoveAt".into(),
                event: None,
                events: None,
                index: 2,
                suffix: "".into(),
            },
            DtoSessionOp {
                op: "Undo".into(),
                event: None,
                events: None,
                index: 0,
                suffix: "".into(),
            },
            DtoSessionOp {
                op: "Undo".into(),
                event: None,
                events: None,
                index: 0,
                suffix: "".into(),
            },
            DtoSessionOp {
                op: "Redo".into(),
                event: None,
                events: None,
                index: 0,
                suffix: "".into(),
            },
            DtoSessionOp {
                op: "Reset".into(),
                event: None,
                events: None,
                index: 0,
                suffix: "".into(),
            },
        ],
    });

    // Session Test 2: Duplicate and Move order preservation
    session_cases.push(SessionTestCase {
        name: "session_duplicate_and_move_order".into(),
        initial_events: vec![
            DtoEvent {
                name: "EventA".into(),
                delay_seconds: 1,
                repeat: false,
                enabled: true,
                conditions: vec![],
                actions: vec![],
            },
            DtoEvent {
                name: "EventB".into(),
                delay_seconds: 2,
                repeat: false,
                enabled: true,
                conditions: vec![],
                actions: vec![],
            },
            DtoEvent {
                name: "EventC".into(),
                delay_seconds: 3,
                repeat: false,
                enabled: true,
                conditions: vec![],
                actions: vec![],
            },
        ],
        operations: vec![
            // Duplicate at index 0 (should insert at 1)
            DtoSessionOp {
                op: "Duplicate".into(),
                event: None,
                events: None,
                index: 0,
                suffix: "_dup0".into(),
            },
            // Duplicate at last index 3 (should insert at 4)
            DtoSessionOp {
                op: "Duplicate".into(),
                event: None,
                events: None,
                index: 3,
                suffix: "_dup3".into(),
            },
            // Duplicate middle index 2 (should insert at 3)
            DtoSessionOp {
                op: "Duplicate".into(),
                event: None,
                events: None,
                index: 2,
                suffix: "_dup2".into(),
            },
            // Simulate move: remove at 1, undo, redo
            DtoSessionOp {
                op: "RemoveAt".into(),
                event: None,
                events: None,
                index: 1,
                suffix: "".into(),
            },
            DtoSessionOp {
                op: "Undo".into(),
                event: None,
                events: None,
                index: 0,
                suffix: "".into(),
            },
            DtoSessionOp {
                op: "Redo".into(),
                event: None,
                events: None,
                index: 0,
                suffix: "".into(),
            },
        ],
    });

    // Session Test 3: Duplicate truncation with long name (> 100 UTF-16 code units)
    let long_name = "X".repeat(95);
    session_cases.push(SessionTestCase {
        name: "session_duplicate_name_truncation".into(),
        initial_events: vec![DtoEvent {
            name: long_name,
            delay_seconds: 10,
            repeat: false,
            enabled: true,
            conditions: vec![],
            actions: vec![],
        }],
        operations: vec![
            DtoSessionOp {
                op: "Duplicate".into(),
                event: None,
                events: None,
                index: 0,
                suffix: "_LONG_SUFFIX_123456789".into(),
            },
            // Duplicate with suffix > 100 characters should fail
            DtoSessionOp {
                op: "Duplicate".into(),
                event: None,
                events: None,
                index: 0,
                suffix: "S".repeat(105),
            },
        ],
    });

    // Session Test 4: Capacity limits (256 events maximum)
    let mut batch255 = Vec::new();
    for i in 0..255 {
        batch255.push(DtoEvent {
            name: format!("Batch_{i}"),
            delay_seconds: i as i32,
            repeat: false,
            enabled: true,
            conditions: vec![],
            actions: vec![],
        });
    }

    session_cases.push(SessionTestCase {
        name: "session_capacity_limits".into(),
        initial_events: vec![],
        operations: vec![
            // Add 255 events
            DtoSessionOp {
                op: "AddRange".into(),
                event: None,
                events: Some(batch255),
                index: 0,
                suffix: "".into(),
            },
            // Add 1 event (now 256, at limit)
            DtoSessionOp {
                op: "Add".into(),
                event: Some(DtoEvent {
                    name: "Event256".into(),
                    delay_seconds: 256,
                    repeat: false,
                    enabled: true,
                    conditions: vec![],
                    actions: vec![],
                }),
                events: None,
                index: 0,
                suffix: "".into(),
            },
            // Attempt to add 1 more: should fail!
            DtoSessionOp {
                op: "Add".into(),
                event: Some(DtoEvent {
                    name: "Event257_Overflow".into(),
                    delay_seconds: 257,
                    repeat: false,
                    enabled: true,
                    conditions: vec![],
                    actions: vec![],
                }),
                events: None,
                index: 0,
                suffix: "".into(),
            },
            // Attempt to duplicate at limit 256: should fail!
            DtoSessionOp {
                op: "Duplicate".into(),
                event: None,
                events: None,
                index: 0,
                suffix: "_overflow".into(),
            },
        ],
    });

    OracleRequest {
        from_scenario_events_cases: from_cases,
        to_scenario_events_cases: to_cases,
        validator_cases: val_cases,
        session_cases,
    }
}

fn default_dto_node() -> DtoNode {
    DtoNode {
        id: Uuid::nil(),
        kind: "".into(),
        x: 0.0,
        y: 0.0,
        event_name: "".into(),
        delay_seconds: 0,
        repeat: false,
        enabled: true,
        target_id: Uuid::nil(),
        target_label: "".into(),
        min_x: 0,
        min_z: 0,
        max_x: 16383,
        max_z: 16383,
        message_text: "".into(),
        team: 0,
        other_team: 1,
        hostile: true,
        alias: "".into(),
        spawn_x: 0.0,
        spawn_z: 0.0,
        count: 1,
        seconds: 0,
    }
}

#[test]
fn csharp_event_graph_dotnet10_differential() {
    let request = generate_synthetic_corpus();
    let oracle_response = execute_csharp_oracle(&request);

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

