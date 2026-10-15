//! 無損雙向轉換器 (EventGraph Converter)
//!
//! 1. 將 `ScenarioEvent` (`against_rome_scenario::Event`) 清單轉換為 `EventGraph` 視覺化節點圖（含階層式自動排版）。
//! 2. 將 `EventGraph` 編譯回符合 Against Rome 規格的 `ScenarioEvent` 清單。

use std::collections::HashSet;
use uuid::Uuid;
use against_rome_scenario::{Action, Condition, Event};

use crate::event_graph::model::{EventGraph, GraphNode, NodeKind};

const LAYOUT_START_Y: f32 = 60.0;
const CONDITION_COLUMN_X: f32 = 60.0;
const TRIGGER_COLUMN_X: f32 = 360.0;
const ACTION_COLUMN_START_X: f32 = 660.0;
const ACTION_SPACING_X: f32 = 260.0;
const ROW_SPACING_Y: f32 = 240.0;
const CONDITION_SPACING_Y: f32 = 90.0;

/// 將現有 ScenarioEvent 清單轉換為 EventGraph。
pub fn from_scenario_events(
    events: &[Event],
    target_label_resolver: Option<&dyn Fn(Uuid) -> String>,
) -> EventGraph {
    let mut graph = EventGraph::new();
    if events.is_empty() {
        return graph;
    }

    let mut current_y = LAYOUT_START_Y;

    for item in events {
        let event_start_y = current_y;

        // 1. 建立觸發節點
        let mut trigger_node = GraphNode::new_event_trigger(
            &item.name,
            item.delay_seconds,
            item.repeat,
            item.enabled,
        );
        trigger_node.x = TRIGGER_COLUMN_X;
        trigger_node.y = event_start_y;
        let _trigger_id = trigger_node.id;
        let conditions_in_port_id = trigger_node.find_port_by_name("ConditionsIn").unwrap().id;
        let trigger_exec_out_port_id = trigger_node.find_port_by_name("ExecOut").unwrap().id;
        graph.add_node(trigger_node);

        // 2. 建立條件節點並連線至觸發節點
        let mut cond_y = event_start_y;
        for cond in &item.conditions {
            let label = target_label_resolver
                .map(|f| f(cond.target_id))
                .unwrap_or_else(|| cond.target_id.to_string());

            let mut cond_node = match cond.kind {
                0 => GraphNode::new_object_exists(cond.target_id, label),
                1 => GraphNode::new_object_dead(cond.target_id, label),
                2 => GraphNode::new_object_in_area(
                    cond.target_id,
                    cond.min_x,
                    cond.min_z,
                    cond.max_x,
                    cond.max_z,
                    label,
                ),
                _ => GraphNode::new_object_exists(cond.target_id, label),
            };

            cond_node.x = CONDITION_COLUMN_X;
            cond_node.y = cond_y;
            let cond_out_port_id = cond_node.find_port_by_name("ConditionOut").unwrap().id;
            graph.add_node(cond_node);

            let _ = graph.connect(cond_out_port_id, conditions_in_port_id);
            cond_y += CONDITION_SPACING_Y;
        }

        // 3. 建立動作節點並形成執行序列連線
        let mut previous_exec_out = Some(trigger_exec_out_port_id);
        let mut action_x = ACTION_COLUMN_START_X;

        for action in &item.actions {
            let mut action_node = match action.kind {
                0 => GraphNode::new_message_action(&action.text),
                1 => GraphNode::new_diplomacy_action(action.team, action.other_team, action.hostile),
                2 => GraphNode::new_spawn_unit_action(
                    &action.alias,
                    action.team,
                    action.x,
                    action.z,
                    action.count,
                ),
                3 => GraphNode::new_victory_action(),
                4 => GraphNode::new_defeat_action(),
                _ => GraphNode::new_message_action(&action.text),
            };

            action_node.x = action_x;
            action_node.y = event_start_y;
            let action_exec_in = action_node.find_port_by_name("ExecIn").map(|p| p.id);
            let action_exec_out = action_node.find_port_by_name("ExecOut").map(|p| p.id);
            graph.add_node(action_node);

            if let (Some(prev_out), Some(act_in)) = (previous_exec_out, action_exec_in) {
                let _ = graph.connect(prev_out, act_in);
            }

            previous_exec_out = action_exec_out;
            action_x += ACTION_SPACING_X;
        }

        // 計算下一個事件的 Y 起始位置
        let row_height = ROW_SPACING_Y.max((cond_y - event_start_y + 60.0).max(180.0));
        current_y += row_height;
    }

    graph
}

/// 將 EventGraph 編譯回標準 ScenarioEvent 清單。
pub fn to_scenario_events(graph: &EventGraph) -> Vec<Event> {
    let mut result = Vec::new();

    for node in &graph.nodes {
        let (name, delay_seconds, repeat, enabled) = match &node.kind {
            NodeKind::EventTrigger {
                event_name,
                delay_seconds,
                repeat,
                enabled,
            } => (event_name.clone(), *delay_seconds, *repeat, *enabled),
            _ => continue,
        };

        let conditions_in = node.find_port_by_name("ConditionsIn");
        let exec_out = node.find_port_by_name("ExecOut");

        // 1. 收集條件
        let mut conditions = Vec::new();
        if let Some(cond_port) = conditions_in {
            let incoming_edges = graph.get_incoming_edges(cond_port.id);
            for edge in incoming_edges {
                if let Some(source_node) = graph.find_node(edge.source_node_id) {
                    match &source_node.kind {
                        NodeKind::ObjectExistsCondition { target_id, .. } => {
                            conditions.push(Condition {
                                kind: 0,
                                target_id: *target_id,
                                ..Default::default()
                            });
                        }
                        NodeKind::ObjectDeadCondition { target_id, .. } => {
                            conditions.push(Condition {
                                kind: 1,
                                target_id: *target_id,
                                ..Default::default()
                            });
                        }
                        NodeKind::ObjectInAreaCondition {
                            target_id,
                            min_x,
                            min_z,
                            max_x,
                            max_z,
                            ..
                        } => {
                            conditions.push(Condition {
                                kind: 2,
                                target_id: *target_id,
                                min_x: *min_x,
                                min_z: *min_z,
                                max_x: *max_x,
                                max_z: *max_z,
                            });
                        }
                        _ => {}
                    }
                }
            }
        }

        // 2. 收集動作序列（沿 ExecOut 遍歷）
        let mut actions = Vec::new();
        let mut current_exec_port_id = exec_out.map(|p| p.id);
        let mut visited_nodes = HashSet::new();

        while let Some(port_id) = current_exec_port_id {
            let outgoing = graph.get_outgoing_edges(port_id);
            if outgoing.is_empty() {
                break;
            }

            let next_node_id = outgoing[0].target_node_id;
            if visited_nodes.contains(&next_node_id) {
                break; // 避免循環
            }
            visited_nodes.insert(next_node_id);

            let next_node = match graph.find_node(next_node_id) {
                Some(n) => n,
                None => break,
            };

            match &next_node.kind {
                NodeKind::MessageAction { message_text } => {
                    actions.push(Action {
                        kind: 0,
                        text: message_text.clone(),
                        ..Default::default()
                    });
                    current_exec_port_id = next_node.find_port_by_name("ExecOut").map(|p| p.id);
                }
                NodeKind::DiplomacyAction {
                    team,
                    other_team,
                    hostile,
                } => {
                    actions.push(Action {
                        kind: 1,
                        team: *team,
                        other_team: *other_team,
                        hostile: *hostile,
                        ..Default::default()
                    });
                    current_exec_port_id = next_node.find_port_by_name("ExecOut").map(|p| p.id);
                }
                NodeKind::SpawnUnitAction {
                    alias,
                    team,
                    spawn_x,
                    spawn_z,
                    count,
                } => {
                    actions.push(Action {
                        kind: 2,
                        alias: alias.clone(),
                        team: *team,
                        x: *spawn_x,
                        z: *spawn_z,
                        count: *count,
                        ..Default::default()
                    });
                    current_exec_port_id = next_node.find_port_by_name("ExecOut").map(|p| p.id);
                }
                NodeKind::VictoryAction => {
                    actions.push(Action {
                        kind: 3,
                        ..Default::default()
                    });
                    current_exec_port_id = None; // Terminal action
                }
                NodeKind::DefeatAction => {
                    actions.push(Action {
                        kind: 4,
                        ..Default::default()
                    });
                    current_exec_port_id = None; // Terminal action
                }
                _ => break,
            }
        }

        result.push(Event {
            name,
            delay_seconds,
            repeat,
            enabled,
            actions,
            conditions,
        });
    }

    result
}
