//! 劇情事件圖核心資料模型 (EventGraph Data Model)
//!
//! 定義節點 (GraphNode)、連接埠 (GraphPort)、連線 (GraphEdge) 以及事件圖容器 (EventGraph)。

use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortType {
    /// 邏輯執行流（白色/藍色連線）
    Execution,
    /// 條件信號流（綠色圓點連線）
    Condition,
    /// 資料傳遞（數值/字串/GUID）
    Data,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GraphDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphDiagnostic {
    pub severity: GraphDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub node_id: Option<Uuid>,
    pub port_id: Option<Uuid>,
    pub edge_id: Option<Uuid>,
}

impl GraphDiagnostic {
    pub fn new(
        severity: GraphDiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            node_id: None,
            port_id: None,
            edge_id: None,
        }
    }

    pub fn with_node(mut self, node_id: Uuid) -> Self {
        self.node_id = Some(node_id);
        self
    }

    pub fn with_port(mut self, port_id: Uuid) -> Self {
        self.port_id = Some(port_id);
        self
    }

    pub fn with_edge(mut self, edge_id: Uuid) -> Self {
        self.edge_id = Some(edge_id);
        self
    }
}

/// 節點連接埠，定義資料或控制信號的出入口。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphPort {
    pub id: Uuid,
    pub node_id: Uuid,
    pub name: String,
    pub display_name: String,
    pub port_type: PortType,
    pub direction: PortDirection,
    pub allow_multiple: bool,
}

impl GraphPort {
    pub fn new(
        node_id: Uuid,
        name: impl Into<String>,
        display_name: impl Into<String>,
        port_type: PortType,
        direction: PortDirection,
        allow_multiple: bool,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_id,
            name: name.into(),
            display_name: display_name.into(),
            port_type,
            direction,
            allow_multiple,
        }
    }

    /// 驗證此連接埠是否能與另一連接埠建立連線。
    pub fn can_connect_to(&self, other: &GraphPort) -> bool {
        if self.node_id == other.node_id {
            return false; // 禁止連接同節點端口
        }
        if self.direction == other.direction {
            return false; // 必須一進一出
        }
        if self.port_type != other.port_type {
            return false; // 類型必須相容
        }
        true
    }
}

/// 節點連線，連接來源埠與目標埠。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphEdge {
    pub id: Uuid,
    pub source_node_id: Uuid,
    pub source_port_id: Uuid,
    pub target_node_id: Uuid,
    pub target_port_id: Uuid,
}

impl GraphEdge {
    pub fn new(
        source_node_id: Uuid,
        source_port_id: Uuid,
        target_node_id: Uuid,
        target_port_id: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            source_node_id,
            source_port_id,
            target_node_id,
            target_port_id,
        }
    }
}

/// 節點類型與專屬資料
#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    EventTrigger {
        event_name: String,
        delay_seconds: i32,
        repeat: bool,
        enabled: bool,
    },
    ObjectExistsCondition {
        target_id: Uuid,
        target_label: String,
    },
    ObjectDeadCondition {
        target_id: Uuid,
        target_label: String,
    },
    ObjectInAreaCondition {
        target_id: Uuid,
        target_label: String,
        min_x: i32,
        min_z: i32,
        max_x: i32,
        max_z: i32,
    },
    MessageAction {
        message_text: String,
    },
    DiplomacyAction {
        team: i32,
        other_team: i32,
        hostile: bool,
    },
    SpawnUnitAction {
        alias: String,
        team: i32,
        spawn_x: f32,
        spawn_z: f32,
        count: i32,
    },
    VictoryAction,
    DefeatAction,
    Delay {
        seconds: i32,
    },
}

/// 邏輯節點結構體
#[derive(Clone, Debug, PartialEq)]
pub struct GraphNode {
    pub id: Uuid,
    pub x: f32,
    pub y: f32,
    pub kind: NodeKind,
    pub ports: Vec<GraphPort>,
}

impl GraphNode {
    pub fn new_event_trigger(
        name: impl Into<String>,
        delay_seconds: i32,
        repeat: bool,
        enabled: bool,
    ) -> Self {
        let id = Uuid::new_v4();
        let ports = vec![
            GraphPort::new(id, "ExecIn", "In", PortType::Execution, PortDirection::Input, false),
            GraphPort::new(id, "ConditionsIn", "Conditions (AND)", PortType::Condition, PortDirection::Input, true),
            GraphPort::new(id, "ExecOut", "Triggered", PortType::Execution, PortDirection::Output, false),
        ];
        Self {
            id,
            x: 0.0,
            y: 0.0,
            kind: NodeKind::EventTrigger {
                event_name: name.into(),
                delay_seconds,
                repeat,
                enabled,
            },
            ports,
        }
    }

    pub fn new_object_exists(target_id: Uuid, label: impl Into<String>) -> Self {
        let id = Uuid::new_v4();
        let ports = vec![
            GraphPort::new(id, "ConditionOut", "Condition", PortType::Condition, PortDirection::Output, true),
        ];
        Self {
            id,
            x: 0.0,
            y: 0.0,
            kind: NodeKind::ObjectExistsCondition {
                target_id,
                target_label: label.into(),
            },
            ports,
        }
    }

    pub fn new_object_dead(target_id: Uuid, label: impl Into<String>) -> Self {
        let id = Uuid::new_v4();
        let ports = vec![
            GraphPort::new(id, "ConditionOut", "Condition", PortType::Condition, PortDirection::Output, true),
        ];
        Self {
            id,
            x: 0.0,
            y: 0.0,
            kind: NodeKind::ObjectDeadCondition {
                target_id,
                target_label: label.into(),
            },
            ports,
        }
    }

    pub fn new_object_in_area(
        target_id: Uuid,
        min_x: i32,
        min_z: i32,
        max_x: i32,
        max_z: i32,
        label: impl Into<String>,
    ) -> Self {
        let id = Uuid::new_v4();
        let ports = vec![
            GraphPort::new(id, "ConditionOut", "Condition", PortType::Condition, PortDirection::Output, true),
        ];
        Self {
            id,
            x: 0.0,
            y: 0.0,
            kind: NodeKind::ObjectInAreaCondition {
                target_id,
                target_label: label.into(),
                min_x,
                min_z,
                max_x,
                max_z,
            },
            ports,
        }
    }

    pub fn new_message_action(text: impl Into<String>) -> Self {
        let id = Uuid::new_v4();
        let ports = vec![
            GraphPort::new(id, "ExecIn", "In", PortType::Execution, PortDirection::Input, false),
            GraphPort::new(id, "ExecOut", "Out", PortType::Execution, PortDirection::Output, false),
        ];
        Self {
            id,
            x: 0.0,
            y: 0.0,
            kind: NodeKind::MessageAction {
                message_text: text.into(),
            },
            ports,
        }
    }

    pub fn new_diplomacy_action(team: i32, other_team: i32, hostile: bool) -> Self {
        let id = Uuid::new_v4();
        let ports = vec![
            GraphPort::new(id, "ExecIn", "In", PortType::Execution, PortDirection::Input, false),
            GraphPort::new(id, "ExecOut", "Out", PortType::Execution, PortDirection::Output, false),
        ];
        Self {
            id,
            x: 0.0,
            y: 0.0,
            kind: NodeKind::DiplomacyAction {
                team,
                other_team,
                hostile,
            },
            ports,
        }
    }

    pub fn new_spawn_unit_action(
        alias: impl Into<String>,
        team: i32,
        spawn_x: f32,
        spawn_z: f32,
        count: i32,
    ) -> Self {
        let id = Uuid::new_v4();
        let ports = vec![
            GraphPort::new(id, "ExecIn", "In", PortType::Execution, PortDirection::Input, false),
            GraphPort::new(id, "ExecOut", "Out", PortType::Execution, PortDirection::Output, false),
        ];
        Self {
            id,
            x: 0.0,
            y: 0.0,
            kind: NodeKind::SpawnUnitAction {
                alias: alias.into(),
                team,
                spawn_x,
                spawn_z,
                count,
            },
            ports,
        }
    }

    pub fn new_victory_action() -> Self {
        let id = Uuid::new_v4();
        let ports = vec![
            GraphPort::new(id, "ExecIn", "In", PortType::Execution, PortDirection::Input, false),
        ];
        Self {
            id,
            x: 0.0,
            y: 0.0,
            kind: NodeKind::VictoryAction,
            ports,
        }
    }

    pub fn new_defeat_action() -> Self {
        let id = Uuid::new_v4();
        let ports = vec![
            GraphPort::new(id, "ExecIn", "In", PortType::Execution, PortDirection::Input, false),
        ];
        Self {
            id,
            x: 0.0,
            y: 0.0,
            kind: NodeKind::DefeatAction,
            ports,
        }
    }

    pub fn new_delay(seconds: i32) -> Self {
        let id = Uuid::new_v4();
        let ports = vec![
            GraphPort::new(id, "ExecIn", "In", PortType::Execution, PortDirection::Input, false),
            GraphPort::new(id, "ExecOut", "Out", PortType::Execution, PortDirection::Output, false),
        ];
        Self {
            id,
            x: 0.0,
            y: 0.0,
            kind: NodeKind::Delay { seconds },
            ports,
        }
    }

    pub fn title(&self) -> String {
        match &self.kind {
            NodeKind::EventTrigger { event_name, .. } => {
                if event_name.trim().is_empty() {
                    "Scenario Event".to_string()
                } else {
                    format!("Event: {event_name}")
                }
            }
            NodeKind::ObjectExistsCondition { .. } => "Condition: Object Exists".to_string(),
            NodeKind::ObjectDeadCondition { .. } => "Condition: Object Dead/Removed".to_string(),
            NodeKind::ObjectInAreaCondition { .. } => "Condition: Object In Area".to_string(),
            NodeKind::MessageAction { .. } => "Action: Show Message".to_string(),
            NodeKind::DiplomacyAction { team, other_team, hostile } => {
                format!("Action: Diplomacy (T{team} {} T{other_team})", if *hostile { "vs" } else { "with" })
            }
            NodeKind::SpawnUnitAction { alias, count, .. } => {
                format!("Action: Spawn Unit ({alias} x{count})")
            }
            NodeKind::VictoryAction => "Action: Victory".to_string(),
            NodeKind::DefeatAction => "Action: Defeat".to_string(),
            NodeKind::Delay { seconds } => format!("Delay: {seconds}s"),
        }
    }

    pub fn category(&self) -> &'static str {
        match &self.kind {
            NodeKind::EventTrigger { .. } => "Trigger",
            NodeKind::ObjectExistsCondition { .. }
            | NodeKind::ObjectDeadCondition { .. }
            | NodeKind::ObjectInAreaCondition { .. } => "Condition",
            NodeKind::MessageAction { .. }
            | NodeKind::DiplomacyAction { .. }
            | NodeKind::SpawnUnitAction { .. }
            | NodeKind::VictoryAction
            | NodeKind::DefeatAction => "Action",
            NodeKind::Delay { .. } => "Flow",
        }
    }

    pub fn find_port(&self, port_id: Uuid) -> Option<&GraphPort> {
        self.ports.iter().find(|p| p.id == port_id)
    }

    pub fn find_port_by_name(&self, name: &str) -> Option<&GraphPort> {
        self.ports.iter().find(|p| p.name.eq_ignore_ascii_case(name))
    }
}

/// 劇情事件節點圖邏輯容器 (EventGraph)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EventGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

impl EventGraph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: GraphNode) {
        if self.nodes.iter().any(|n| n.id == node.id) {
            panic!("節點 ID {} 已存在於圖中。", node.id);
        }
        self.nodes.push(node);
    }

    pub fn remove_node(&mut self, node_id: Uuid) -> bool {
        let initial_len = self.nodes.len();
        self.nodes.retain(|n| n.id != node_id);
        if self.nodes.len() < initial_len {
            self.edges.retain(|e| e.source_node_id != node_id && e.target_node_id != node_id);
            true
        } else {
            false
        }
    }

    pub fn find_node(&self, node_id: Uuid) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.id == node_id)
    }

    pub fn find_node_mut(&mut self, node_id: Uuid) -> Option<&mut GraphNode> {
        self.nodes.iter_mut().find(|n| n.id == node_id)
    }

    pub fn find_port(&self, port_id: Uuid) -> Option<&GraphPort> {
        for node in &self.nodes {
            if let Some(port) = node.find_port(port_id) {
                return Some(port);
            }
        }
        None
    }

    pub fn connect(
        &mut self,
        source_port_id: Uuid,
        target_port_id: Uuid,
    ) -> Result<GraphEdge, String> {
        let p1 = self.find_port(source_port_id).ok_or("找不到指定的來源連接埠。")?.clone();
        let p2 = self.find_port(target_port_id).ok_or("找不到指定的目標連接埠。")?.clone();

        // 確保方向為 Output -> Input
        let (from, to) = match (p1.direction, p2.direction) {
            (PortDirection::Output, PortDirection::Input) => (p1, p2),
            (PortDirection::Input, PortDirection::Output) => (p2, p1),
            _ => {
                return Err(format!(
                    "無法連接連接埠：{} ({:?}) 與 {} ({:?}) 方向不相容。",
                    p1.name, p1.direction, p2.name, p2.direction
                ));
            }
        };

        if !from.can_connect_to(&to) {
            return Err(format!(
                "無法連接連接埠：{} ({:?}/{:?}) 與 {} ({:?}/{:?}) 不相容。",
                from.name, from.port_type, from.direction, to.name, to.port_type, to.direction
            ));
        }

        // 檢查多重連線限制
        if !from.allow_multiple && self.edges.iter().any(|e| e.source_port_id == from.id) {
            return Err(format!("連接埠 {} 不允許輸出多條連線。", from.name));
        }

        if !to.allow_multiple && self.edges.iter().any(|e| e.target_port_id == to.id) {
            return Err(format!("連接埠 {} 不允許輸入多條連線。", to.name));
        }

        // 避免重複連線
        if self.edges.iter().any(|e| e.source_port_id == from.id && e.target_port_id == to.id) {
            return Err("連線已存在。".to_string());
        }

        let edge = GraphEdge::new(from.node_id, from.id, to.node_id, to.id);
        self.edges.push(edge);
        Ok(edge)
    }

    pub fn disconnect(&mut self, edge_id: Uuid) -> bool {
        let initial_len = self.edges.len();
        self.edges.retain(|e| e.id != edge_id);
        self.edges.len() < initial_len
    }

    pub fn disconnect_ports(&mut self, port_a: Uuid, port_b: Uuid) -> bool {
        let initial_len = self.edges.len();
        self.edges.retain(|e| {
            !((e.source_port_id == port_a && e.target_port_id == port_b)
                || (e.source_port_id == port_b && e.target_port_id == port_a))
        });
        self.edges.len() < initial_len
    }

    pub fn get_incoming_edges(&self, port_id: Uuid) -> Vec<&GraphEdge> {
        self.edges.iter().filter(|e| e.target_port_id == port_id).collect()
    }

    pub fn get_outgoing_edges(&self, port_id: Uuid) -> Vec<&GraphEdge> {
        self.edges.iter().filter(|e| e.source_port_id == port_id).collect()
    }

    pub fn get_node_edges(&self, node_id: Uuid) -> Vec<&GraphEdge> {
        self.edges
            .iter()
            .filter(|e| e.source_node_id == node_id || e.target_node_id == node_id)
            .collect()
    }

    pub fn clear(&mut self) {
        self.edges.clear();
        self.nodes.clear();
    }
}
