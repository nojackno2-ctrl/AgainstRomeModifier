//! 劇情事件圖靜態分析器 (EventGraph Validator)
//!
//! 1. 偵測 Execution 迴路與死鎖 (EXECUTION_CYCLE_DETECTED)。
//! 2. 偵測孤立動作 (ORPHAN_ACTION) 與懸空條件 (DANGLING_CONDITION)。
//! 3. 驗證終止動作（勝利/失敗）合規性 (TERMINAL_IN_REPEAT_EVENT, ACTION_AFTER_TERMINAL)。
//! 4. 檢驗目標物件 GUID 是否有效或已遭刪除 (EMPTY_TARGET_GUID, INVALID_TARGET_GUID)。
//! 5. 檢驗部隊別名與數值邊界合規性 (INVALID_SPAWN_ALIAS, INVALID_SPAWN_COORDINATES 等)。

use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::event_graph::model::{
    EventGraph, GraphDiagnostic, GraphDiagnosticSeverity, NodeKind, PortType,
};

pub fn validate(
    graph: &EventGraph,
    valid_target_ids: Option<&HashSet<Uuid>>,
    valid_aliases: Option<&[String]>,
) -> Vec<GraphDiagnostic> {
    let mut diagnostics = Vec::new();

    // 1. 數量限制檢查
    let trigger_count = graph
        .nodes
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::EventTrigger { .. }))
        .count();
    if trigger_count > 256 {
        diagnostics.push(GraphDiagnostic::new(
            GraphDiagnosticSeverity::Error,
            "EVENT_LIMIT_EXCEEDED",
            format!("圖中事件觸發節點數量為 {trigger_count}，超過遊戲上限 256 個。"),
        ));
    }

    // 2. 循環依賴與死鎖偵測 (Cycle Detection on Execution Flow)
    detect_execution_cycles(graph, &mut diagnostics);

    // 3. 逐節點深入驗證
    for node in &graph.nodes {
        match &node.kind {
            NodeKind::EventTrigger {
                event_name,
                delay_seconds,
                repeat,
                ..
            } => {
                validate_trigger_node(
                    graph,
                    node.id,
                    event_name,
                    *delay_seconds,
                    *repeat,
                    &mut diagnostics,
                );
            }
            NodeKind::ObjectExistsCondition { target_id, .. }
            | NodeKind::ObjectDeadCondition { target_id, .. } => {
                validate_condition_node(
                    graph,
                    node.id,
                    *target_id,
                    None,
                    &node.title(),
                    valid_target_ids,
                    &mut diagnostics,
                );
            }
            NodeKind::ObjectInAreaCondition {
                target_id,
                min_x,
                min_z,
                max_x,
                max_z,
                ..
            } => {
                validate_condition_node(
                    graph,
                    node.id,
                    *target_id,
                    Some((*min_x, *min_z, *max_x, *max_z)),
                    &node.title(),
                    valid_target_ids,
                    &mut diagnostics,
                );
            }
            NodeKind::MessageAction { message_text } => {
                validate_action_common(graph, node.id, &node.title(), &mut diagnostics);
                if message_text.trim().is_empty() {
                    diagnostics.push(
                        GraphDiagnostic::new(
                            GraphDiagnosticSeverity::Error,
                            "EMPTY_MESSAGE_TEXT",
                            "訊息動作文字不可為空。",
                        )
                        .with_node(node.id),
                    );
                }
            }
            NodeKind::DiplomacyAction {
                team,
                other_team,
                ..
            } => {
                validate_action_common(graph, node.id, &node.title(), &mut diagnostics);
                if *team < 0 || *team > 7 || *other_team < 0 || *other_team > 7 || *team == *other_team
                {
                    diagnostics.push(
                        GraphDiagnostic::new(
                            GraphDiagnosticSeverity::Error,
                            "INVALID_DIPLOMACY_TEAMS",
                            format!(
                                "外交動作需要兩個不同隊伍（0–7），當前設定為隊伍 {team} 與隊伍 {other_team}。"
                            ),
                        )
                        .with_node(node.id),
                    );
                }
            }
            NodeKind::SpawnUnitAction {
                alias,
                team,
                spawn_x,
                spawn_z,
                count,
            } => {
                validate_action_common(graph, node.id, &node.title(), &mut diagnostics);
                if *team < 0 || *team > 7 {
                    diagnostics.push(
                        GraphDiagnostic::new(
                            GraphDiagnosticSeverity::Error,
                            "INVALID_SPAWN_TEAM",
                            format!("生成部隊隊伍必須介於 0 與 7，當前為 {team}。"),
                        )
                        .with_node(node.id),
                    );
                }
                if *count < 1 || *count > 20 {
                    diagnostics.push(
                        GraphDiagnostic::new(
                            GraphDiagnosticSeverity::Error,
                            "INVALID_SPAWN_COUNT",
                            format!("生成部隊人數必須介於 1 與 20，當前為 {count}。"),
                        )
                        .with_node(node.id),
                    );
                }
                if !spawn_x.is_finite()
                    || !spawn_z.is_finite()
                    || *spawn_x < 0.0
                    || *spawn_x > 16383.0
                    || *spawn_z < 0.0
                    || *spawn_z > 16383.0
                {
                    diagnostics.push(
                        GraphDiagnostic::new(
                            GraphDiagnosticSeverity::Error,
                            "INVALID_SPAWN_COORDINATES",
                            format!(
                                "生成部隊座標必須介於 0 與 16383 之間，當前為 ({spawn_x}, {spawn_z})。"
                            ),
                        )
                        .with_node(node.id),
                    );
                }
                if let Some(aliases) = valid_aliases {
                    if !aliases.iter().any(|a| a.eq_ignore_ascii_case(alias)) {
                        diagnostics.push(
                            GraphDiagnostic::new(
                                GraphDiagnosticSeverity::Error,
                                "INVALID_SPAWN_ALIAS",
                                format!("未知的部隊別名：{alias}。"),
                            )
                            .with_node(node.id),
                        );
                    }
                }
            }
            NodeKind::VictoryAction | NodeKind::DefeatAction => {
                validate_action_common(graph, node.id, &node.title(), &mut diagnostics);
            }
            NodeKind::Delay { seconds } => {
                let incoming = node
                    .find_port_by_name("ExecIn")
                    .map(|p| graph.get_incoming_edges(p.id))
                    .unwrap_or_default();
                if incoming.is_empty() {
                    diagnostics.push(
                        GraphDiagnostic::new(
                            GraphDiagnosticSeverity::Warning,
                            "ORPHAN_DELAY",
                            format!("延遲節點「{}」沒有前置執行連線。", node.title()),
                        )
                        .with_node(node.id),
                    );
                }
                if *seconds < 0 {
                    diagnostics.push(
                        GraphDiagnostic::new(
                            GraphDiagnosticSeverity::Error,
                            "NEGATIVE_DELAY",
                            "延遲時間不能為負數。",
                        )
                        .with_node(node.id),
                    );
                }
            }
        }
    }

    diagnostics
}

fn validate_trigger_node(
    graph: &EventGraph,
    node_id: Uuid,
    event_name: &str,
    delay_seconds: i32,
    repeat: bool,
    diagnostics: &mut Vec<GraphDiagnostic>,
) {
    if event_name.trim().is_empty() {
        diagnostics.push(
            GraphDiagnostic::new(
                GraphDiagnosticSeverity::Error,
                "EMPTY_EVENT_NAME",
                "事件觸發節點名稱不可為空。",
            )
            .with_node(node_id),
        );
    }

    if !(0..=86400).contains(&delay_seconds) {
        diagnostics.push(
            GraphDiagnostic::new(
                GraphDiagnosticSeverity::Error,
                "INVALID_DELAY",
                format!("事件「{event_name}」延遲秒數必須介於 0 與 86400 秒。"),
            )
            .with_node(node_id),
        );
    }

    if repeat && delay_seconds == 0 {
        diagnostics.push(
            GraphDiagnostic::new(
                GraphDiagnosticSeverity::Error,
                "REPEAT_ZERO_DELAY",
                format!(
                    "重複執行的事件「{event_name}」間隔時間至少必須為 1 秒，否則將造成每一幀觸發之死鎖問題。"
                ),
            )
            .with_node(node_id),
        );
    }

    let node = match graph.find_node(node_id) {
        Some(n) => n,
        None => return,
    };

    // 檢查條件連線數量
    if let Some(cond_port) = node.find_port_by_name("ConditionsIn") {
        let incoming_conditions = graph.get_incoming_edges(cond_port.id);
        if incoming_conditions.len() > 32 {
            diagnostics.push(
                GraphDiagnostic::new(
                    GraphDiagnosticSeverity::Error,
                    "CONDITION_LIMIT_EXCEEDED",
                    format!(
                        "事件「{event_name}」連接了 {} 個條件，超過上限 32 個。",
                        incoming_conditions.len()
                    ),
                )
                .with_node(node_id),
            );
        }
    }

    // 檢查動作序列
    let exec_out = node.find_port_by_name("ExecOut");
    let outgoing_exec = exec_out.map(|p| graph.get_outgoing_edges(p.id)).unwrap_or_default();

    if outgoing_exec.is_empty() {
        diagnostics.push(
            GraphDiagnostic::new(
                GraphDiagnosticSeverity::Warning,
                "EMPTY_TRIGGER_FLOW",
                format!("事件觸發節點「{event_name}」未連接任何後續動作，觸發時將無實質效果。"),
            )
            .with_node(node_id),
        );
    } else {
        let mut action_count = 0;
        let mut current_port_id = exec_out.map(|p| p.id);
        let mut visited = HashSet::new();

        while let Some(port_id) = current_port_id {
            let next_edges = graph.get_outgoing_edges(port_id);
            if next_edges.is_empty() {
                break;
            }

            let next_node_id = next_edges[0].target_node_id;
            if visited.contains(&next_node_id) {
                break;
            }
            visited.insert(next_node_id);

            let next_node = match graph.find_node(next_node_id) {
                Some(n) => n,
                None => break,
            };

            match &next_node.kind {
                NodeKind::MessageAction { .. }
                | NodeKind::DiplomacyAction { .. }
                | NodeKind::SpawnUnitAction { .. } => {
                    action_count += 1;
                    current_port_id = next_node.find_port_by_name("ExecOut").map(|p| p.id);
                }
                NodeKind::VictoryAction | NodeKind::DefeatAction => {
                    action_count += 1;
                    if repeat {
                        diagnostics.push(
                            GraphDiagnostic::new(
                                GraphDiagnosticSeverity::Error,
                                "TERMINAL_IN_REPEAT_EVENT",
                                format!(
                                    "事件「{event_name}」設定為重複執行，但其動作鏈中包含勝利/失敗終止動作。"
                                ),
                            )
                            .with_node(next_node.id),
                        );
                    }
                    if let Some(out_p) = next_node.find_port_by_name("ExecOut") {
                        if !graph.get_outgoing_edges(out_p.id).is_empty() {
                            diagnostics.push(
                                GraphDiagnostic::new(
                                    GraphDiagnosticSeverity::Error,
                                    "ACTION_AFTER_TERMINAL",
                                    "勝利/失敗終止動作後不可串接其他動作。",
                                )
                                .with_node(next_node.id),
                            );
                        }
                    }
                    current_port_id = None;
                }
                NodeKind::Delay { .. } => {
                    current_port_id = next_node.find_port_by_name("ExecOut").map(|p| p.id);
                }
                _ => break,
            }
        }

        if action_count > 32 {
            diagnostics.push(
                GraphDiagnostic::new(
                    GraphDiagnosticSeverity::Error,
                    "ACTION_LIMIT_EXCEEDED",
                    format!("事件「{event_name}」動作數量為 {action_count}，超過上限 32 個。"),
                )
                .with_node(node_id),
            );
        }
    }
}

fn validate_condition_node(
    graph: &EventGraph,
    node_id: Uuid,
    target_id: Uuid,
    area_bounds: Option<(i32, i32, i32, i32)>,
    title: &str,
    valid_target_ids: Option<&HashSet<Uuid>>,
    diagnostics: &mut Vec<GraphDiagnostic>,
) {
    if target_id.is_nil() {
        diagnostics.push(
            GraphDiagnostic::new(
                GraphDiagnosticSeverity::Error,
                "EMPTY_TARGET_GUID",
                format!("條件節點「{title}」尚未指定目標物件 GUID。"),
            )
            .with_node(node_id),
        );
    } else if let Some(valid_ids) = valid_target_ids {
        if !valid_ids.contains(&target_id) {
            diagnostics.push(
                GraphDiagnostic::new(
                    GraphDiagnosticSeverity::Error,
                    "INVALID_TARGET_GUID",
                    format!("條件節點「{title}」引用的目標物件 GUID ({target_id}) 在地圖中不存在或已遭刪除。"),
                )
                .with_node(node_id),
            );
        }
    }

    if let Some((min_x, min_z, max_x, max_z)) = area_bounds {
        if min_x < 0
            || min_z < 0
            || max_x > 16383
            || max_z > 16383
            || min_x > max_x
            || min_z > max_z
        {
            diagnostics.push(
                GraphDiagnostic::new(
                    GraphDiagnosticSeverity::Error,
                    "INVALID_AREA_BOUNDS",
                    format!(
                        "區域條件座標範圍無效：[{min_x}, {min_z}] 至 [{max_x}, {max_z}] 必須在 0–16383 範圍內且 Min <= Max。"
                    ),
                )
                .with_node(node_id),
            );
        }
    }

    let node = match graph.find_node(node_id) {
        Some(n) => n,
        None => return,
    };

    if let Some(out_port) = node.find_port_by_name("ConditionOut") {
        if graph.get_outgoing_edges(out_port.id).is_empty() {
            diagnostics.push(
                GraphDiagnostic::new(
                    GraphDiagnosticSeverity::Warning,
                    "DANGLING_CONDITION",
                    format!("條件節點「{title}」未連接至任何事件觸發節點，屬於懸空條件。"),
                )
                .with_node(node_id),
            );
        }
    }
}

fn validate_action_common(
    graph: &EventGraph,
    node_id: Uuid,
    title: &str,
    diagnostics: &mut Vec<GraphDiagnostic>,
) {
    let node = match graph.find_node(node_id) {
        Some(n) => n,
        None => return,
    };
    if let Some(in_port) = node.find_port_by_name("ExecIn") {
        if graph.get_incoming_edges(in_port.id).is_empty() {
            diagnostics.push(
                GraphDiagnostic::new(
                    GraphDiagnosticSeverity::Warning,
                    "ORPHAN_ACTION",
                    format!("動作節點「{title}」沒有前置執行連線，永遠不會被執行。"),
                )
                .with_node(node_id),
            );
        }
    }
}

fn detect_execution_cycles(graph: &EventGraph, diagnostics: &mut Vec<GraphDiagnostic>) {
    // 建立 Execution 連線鄰接表
    let mut adj: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for edge in &graph.edges {
        let source_port = graph.find_port(edge.source_port_id);
        let target_port = graph.find_port(edge.target_port_id);
        if let (Some(sp), Some(tp)) = (source_port, target_port) {
            if sp.port_type == PortType::Execution && tp.port_type == PortType::Execution {
                adj.entry(edge.source_node_id)
                    .or_default()
                    .push(edge.target_node_id);
            }
        }
    }

    // DFS 三色標記法：0=White(未訪), 1=Gray(當前路徑), 2=Black(已完成)
    let mut color: HashMap<Uuid, u8> = HashMap::new();
    for node in &graph.nodes {
        color.insert(node.id, 0);
    }

    for node in &graph.nodes {
        if color.get(&node.id).copied().unwrap_or(0) == 0 {
            if dfs_cycle(node.id, &adj, &mut color) {
                diagnostics.push(
                    GraphDiagnostic::new(
                        GraphDiagnosticSeverity::Error,
                        "EXECUTION_CYCLE_DETECTED",
                        format!(
                            "偵測到執行流死循環（包含節點 {}），將導致遊戲腳本引擎陷入無窮迴圈或死鎖。",
                            node.title()
                        ),
                    )
                    .with_node(node.id),
                );
            }
        }
    }
}

fn dfs_cycle(u: Uuid, adj: &HashMap<Uuid, Vec<Uuid>>, color: &mut HashMap<Uuid, u8>) -> bool {
    color.insert(u, 1); // Gray

    if let Some(neighbors) = adj.get(&u) {
        for &v in neighbors {
            match color.get(&v).copied().unwrap_or(0) {
                1 => return true, // 找到環！
                0 => {
                    if dfs_cycle(v, adj, color) {
                        return true;
                    }
                }
                _ => {}
            }
        }
    }

    color.insert(u, 2); // Black
    false
}
