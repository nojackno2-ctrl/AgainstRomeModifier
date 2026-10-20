use crate::{Action, Condition, Event, Result, WorldPosition};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortType {
    /// Logical execution flow (arrow connection)
    Execution,
    /// Condition signal flow
    Condition,
    /// Data transmission
    Data,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GraphDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphDiagnostic {
    pub severity: GraphDiagnosticSeverity,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_id: Option<Uuid>,
}

impl GraphDiagnostic {
    pub fn new(
        severity: GraphDiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
        node_id: Option<Uuid>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            node_id,
            port_id: None,
            edge_id: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
        name: &str,
        display_name: &str,
        port_type: PortType,
        direction: PortDirection,
        allow_multiple: bool,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_id,
            name: name.to_string(),
            display_name: display_name.to_string(),
            port_type,
            direction,
            allow_multiple,
        }
    }

    pub fn can_connect_to(&self, other: &GraphPort) -> bool {
        if self.node_id == other.node_id {
            return false;
        }
        if self.direction == other.direction {
            return false;
        }
        if self.port_type != other.port_type {
            return false;
        }
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventTriggerNode {
    pub id: Uuid,
    pub x: f32,
    pub y: f32,
    pub event_name: String,
    pub delay_seconds: i32,
    pub repeat: bool,
    pub enabled: bool,
    pub exec_in: GraphPort,
    pub conditions_in: GraphPort,
    pub exec_out: GraphPort,
}

impl EventTriggerNode {
    pub fn new(event_name: &str, delay_seconds: i32, repeat: bool, enabled: bool) -> Self {
        let id = Uuid::new_v4();
        Self {
            id,
            x: 0.0,
            y: 0.0,
            event_name: event_name.to_string(),
            delay_seconds,
            repeat,
            enabled,
            exec_in: GraphPort::new(id, "ExecIn", "In", PortType::Execution, PortDirection::Input, false),
            conditions_in: GraphPort::new(
                id,
                "ConditionsIn",
                "Conditions (AND)",
                PortType::Condition,
                PortDirection::Input,
                true,
            ),
            exec_out: GraphPort::new(
                id,
                "ExecOut",
                "Triggered",
                PortType::Execution,
                PortDirection::Output,
                false,
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObjectExistsConditionNode {
    pub id: Uuid,
    pub x: f32,
    pub y: f32,
    pub target_id: Uuid,
    pub target_label: String,
    pub condition_out: GraphPort,
}

impl ObjectExistsConditionNode {
    pub fn new(target_id: Uuid, target_label: &str) -> Self {
        let id = Uuid::new_v4();
        Self {
            id,
            x: 0.0,
            y: 0.0,
            target_id,
            target_label: target_label.to_string(),
            condition_out: GraphPort::new(
                id,
                "ConditionOut",
                "Condition",
                PortType::Condition,
                PortDirection::Output,
                true,
            ),
        }
    }

    pub fn to_condition(&self) -> Condition {
        Condition {
            kind: 0,
            target_id: self.target_id,
            min_x: 0,
            min_z: 0,
            max_x: 16383,
            max_z: 16383,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObjectDeadConditionNode {
    pub id: Uuid,
    pub x: f32,
    pub y: f32,
    pub target_id: Uuid,
    pub target_label: String,
    pub condition_out: GraphPort,
}

impl ObjectDeadConditionNode {
    pub fn new(target_id: Uuid, target_label: &str) -> Self {
        let id = Uuid::new_v4();
        Self {
            id,
            x: 0.0,
            y: 0.0,
            target_id,
            target_label: target_label.to_string(),
            condition_out: GraphPort::new(
                id,
                "ConditionOut",
                "Condition",
                PortType::Condition,
                PortDirection::Output,
                true,
            ),
        }
    }

    pub fn to_condition(&self) -> Condition {
        Condition {
            kind: 1,
            target_id: self.target_id,
            min_x: 0,
            min_z: 0,
            max_x: 16383,
            max_z: 16383,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObjectInAreaConditionNode {
    pub id: Uuid,
    pub x: f32,
    pub y: f32,
    pub target_id: Uuid,
    pub target_label: String,
    pub min_x: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_z: i32,
    pub condition_out: GraphPort,
}

impl ObjectInAreaConditionNode {
    pub fn new(
        target_id: Uuid,
        min_x: i32,
        min_z: i32,
        max_x: i32,
        max_z: i32,
        target_label: &str,
    ) -> Self {
        let id = Uuid::new_v4();
        Self {
            id,
            x: 0.0,
            y: 0.0,
            target_id,
            target_label: target_label.to_string(),
            min_x,
            min_z,
            max_x,
            max_z,
            condition_out: GraphPort::new(
                id,
                "ConditionOut",
                "Condition",
                PortType::Condition,
                PortDirection::Output,
                true,
            ),
        }
    }

    pub fn to_condition(&self) -> Condition {
        Condition {
            kind: 2,
            target_id: self.target_id,
            min_x: self.min_x,
            min_z: self.min_z,
            max_x: self.max_x,
            max_z: self.max_z,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MessageActionNode {
    pub id: Uuid,
    pub x: f32,
    pub y: f32,
    pub message_text: String,
    pub exec_in: GraphPort,
    pub exec_out: GraphPort,
}

impl MessageActionNode {
    pub fn new(text: &str) -> Self {
        let id = Uuid::new_v4();
        Self {
            id,
            x: 0.0,
            y: 0.0,
            message_text: text.to_string(),
            exec_in: GraphPort::new(id, "ExecIn", "In", PortType::Execution, PortDirection::Input, false),
            exec_out: GraphPort::new(id, "ExecOut", "Out", PortType::Execution, PortDirection::Output, false),
        }
    }

    pub fn to_action(&self) -> Action {
        Action {
            kind: 0,
            text: self.message_text.clone(),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiplomacyActionNode {
    pub id: Uuid,
    pub x: f32,
    pub y: f32,
    pub team: i32,
    pub other_team: i32,
    pub hostile: bool,
    pub exec_in: GraphPort,
    pub exec_out: GraphPort,
}

impl DiplomacyActionNode {
    pub fn new(team: i32, other_team: i32, hostile: bool) -> Self {
        let id = Uuid::new_v4();
        Self {
            id,
            x: 0.0,
            y: 0.0,
            team,
            other_team,
            hostile,
            exec_in: GraphPort::new(id, "ExecIn", "In", PortType::Execution, PortDirection::Input, false),
            exec_out: GraphPort::new(id, "ExecOut", "Out", PortType::Execution, PortDirection::Output, false),
        }
    }

    pub fn to_action(&self) -> Action {
        Action {
            kind: 1,
            team: self.team,
            other_team: self.other_team,
            hostile: self.hostile,
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpawnUnitActionNode {
    pub id: Uuid,
    pub x: f32,
    pub y: f32,
    pub alias: String,
    pub team: i32,
    pub spawn_x: f32,
    pub spawn_z: f32,
    pub count: i32,
    pub move_destination: Option<WorldPosition>,
    pub exec_in: GraphPort,
    pub exec_out: GraphPort,
}

impl SpawnUnitActionNode {
    pub fn new(alias: &str, team: i32, x: f32, z: f32, count: i32) -> Self {
        let id = Uuid::new_v4();
        Self {
            id,
            x: 0.0,
            y: 0.0,
            alias: alias.to_string(),
            team,
            spawn_x: x,
            spawn_z: z,
            count,
            move_destination: None,
            exec_in: GraphPort::new(id, "ExecIn", "In", PortType::Execution, PortDirection::Input, false),
            exec_out: GraphPort::new(id, "ExecOut", "Out", PortType::Execution, PortDirection::Output, false),
        }
    }

    pub fn to_action(&self) -> Action {
        Action {
            kind: 2,
            alias: self.alias.clone(),
            team: self.team,
            x: self.spawn_x,
            z: self.spawn_z,
            count: self.count,
            move_destination: self.move_destination.clone(),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VictoryActionNode {
    pub id: Uuid,
    pub x: f32,
    pub y: f32,
    pub exec_in: GraphPort,
}

impl VictoryActionNode {
    pub fn new() -> Self {
        let id = Uuid::new_v4();
        Self {
            id,
            x: 0.0,
            y: 0.0,
            exec_in: GraphPort::new(id, "ExecIn", "In", PortType::Execution, PortDirection::Input, false),
        }
    }

    pub fn to_action(&self) -> Action {
        Action {
            kind: 3,
            ..Default::default()
        }
    }
}

impl Default for VictoryActionNode {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DefeatActionNode {
    pub id: Uuid,
    pub x: f32,
    pub y: f32,
    pub exec_in: GraphPort,
}

impl DefeatActionNode {
    pub fn new() -> Self {
        let id = Uuid::new_v4();
        Self {
            id,
            x: 0.0,
            y: 0.0,
            exec_in: GraphPort::new(id, "ExecIn", "In", PortType::Execution, PortDirection::Input, false),
        }
    }

    pub fn to_action(&self) -> Action {
        Action {
            kind: 4,
            ..Default::default()
        }
    }
}

impl Default for DefeatActionNode {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DelayNode {
    pub id: Uuid,
    pub x: f32,
    pub y: f32,
    pub seconds: i32,
    pub exec_in: GraphPort,
    pub exec_out: GraphPort,
}

impl DelayNode {
    pub fn new(seconds: i32) -> Self {
        let id = Uuid::new_v4();
        Self {
            id,
            x: 0.0,
            y: 0.0,
            seconds,
            exec_in: GraphPort::new(id, "ExecIn", "In", PortType::Execution, PortDirection::Input, false),
            exec_out: GraphPort::new(id, "ExecOut", "Out", PortType::Execution, PortDirection::Output, false),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GraphNode {
    Trigger(EventTriggerNode),
    ExistsCondition(ObjectExistsConditionNode),
    DeadCondition(ObjectDeadConditionNode),
    AreaCondition(ObjectInAreaConditionNode),
    MessageAction(MessageActionNode),
    DiplomacyAction(DiplomacyActionNode),
    SpawnUnitAction(SpawnUnitActionNode),
    VictoryAction(VictoryActionNode),
    DefeatAction(DefeatActionNode),
    Delay(DelayNode),
}

impl GraphNode {
    pub fn id(&self) -> Uuid {
        match self {
            Self::Trigger(n) => n.id,
            Self::ExistsCondition(n) => n.id,
            Self::DeadCondition(n) => n.id,
            Self::AreaCondition(n) => n.id,
            Self::MessageAction(n) => n.id,
            Self::DiplomacyAction(n) => n.id,
            Self::SpawnUnitAction(n) => n.id,
            Self::VictoryAction(n) => n.id,
            Self::DefeatAction(n) => n.id,
            Self::Delay(n) => n.id,
        }
    }

    pub fn title(&self) -> String {
        match self {
            Self::Trigger(n) => {
                if n.event_name.trim().is_empty() {
                    "Scenario Event".into()
                } else {
                    format!("Event: {}", n.event_name)
                }
            }
            Self::ExistsCondition(_) => "Condition: Object Exists".into(),
            Self::DeadCondition(_) => "Condition: Object Dead/Removed".into(),
            Self::AreaCondition(_) => "Condition: Object In Area".into(),
            Self::MessageAction(_) => "Action: Show Message".into(),
            Self::DiplomacyAction(n) => {
                format!(
                    "Action: Diplomacy (T{} {} T{})",
                    n.team,
                    if n.hostile { "vs" } else { "with" },
                    n.other_team
                )
            }
            Self::SpawnUnitAction(n) => format!("Action: Spawn Unit ({} x{})", n.alias, n.count),
            Self::VictoryAction(_) => "Action: Victory".into(),
            Self::DefeatAction(_) => "Action: Defeat".into(),
            Self::Delay(n) => format!("Delay: {}s", n.seconds),
        }
    }

    pub fn category(&self) -> &'static str {
        match self {
            Self::Trigger(_) => "Trigger",
            Self::ExistsCondition(_) | Self::DeadCondition(_) | Self::AreaCondition(_) => "Condition",
            Self::MessageAction(_)
            | Self::DiplomacyAction(_)
            | Self::SpawnUnitAction(_)
            | Self::VictoryAction(_)
            | Self::DefeatAction(_) => "Action",
            Self::Delay(_) => "Flow",
        }
    }

    pub fn ports(&self) -> Vec<&GraphPort> {
        match self {
            Self::Trigger(n) => vec![&n.exec_in, &n.conditions_in, &n.exec_out],
            Self::ExistsCondition(n) => vec![&n.condition_out],
            Self::DeadCondition(n) => vec![&n.condition_out],
            Self::AreaCondition(n) => vec![&n.condition_out],
            Self::MessageAction(n) => vec![&n.exec_in, &n.exec_out],
            Self::DiplomacyAction(n) => vec![&n.exec_in, &n.exec_out],
            Self::SpawnUnitAction(n) => vec![&n.exec_in, &n.exec_out],
            Self::VictoryAction(n) => vec![&n.exec_in],
            Self::DefeatAction(n) => vec![&n.exec_in],
            Self::Delay(n) => vec![&n.exec_in, &n.exec_out],
        }
    }

    pub fn find_port(&self, port_id: Uuid) -> Option<&GraphPort> {
        self.ports().into_iter().find(|p| p.id == port_id)
    }

    pub fn find_port_by_name(&self, name: &str) -> Option<&GraphPort> {
        self.ports()
            .into_iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    pub fn position(&self) -> (f32, f32) {
        match self {
            Self::Trigger(n) => (n.x, n.y),
            Self::ExistsCondition(n) => (n.x, n.y),
            Self::DeadCondition(n) => (n.x, n.y),
            Self::AreaCondition(n) => (n.x, n.y),
            Self::MessageAction(n) => (n.x, n.y),
            Self::DiplomacyAction(n) => (n.x, n.y),
            Self::SpawnUnitAction(n) => (n.x, n.y),
            Self::VictoryAction(n) => (n.x, n.y),
            Self::DefeatAction(n) => (n.x, n.y),
            Self::Delay(n) => (n.x, n.y),
        }
    }

    pub fn set_position(&mut self, x: f32, y: f32) {
        match self {
            Self::Trigger(n) => {
                n.x = x;
                n.y = y;
            }
            Self::ExistsCondition(n) => {
                n.x = x;
                n.y = y;
            }
            Self::DeadCondition(n) => {
                n.x = x;
                n.y = y;
            }
            Self::AreaCondition(n) => {
                n.x = x;
                n.y = y;
            }
            Self::MessageAction(n) => {
                n.x = x;
                n.y = y;
            }
            Self::DiplomacyAction(n) => {
                n.x = x;
                n.y = y;
            }
            Self::SpawnUnitAction(n) => {
                n.x = x;
                n.y = y;
            }
            Self::VictoryAction(n) => {
                n.x = x;
                n.y = y;
            }
            Self::DefeatAction(n) => {
                n.x = x;
                n.y = y;
            }
            Self::Delay(n) => {
                n.x = x;
                n.y = y;
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EventGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

impl EventGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, node: GraphNode) -> Result<()> {
        let id = node.id();
        if self.nodes.iter().any(|n| n.id() == id) {
            return Err(format!("節點 ID {id} 已存在於圖中。"));
        }
        self.nodes.push(node);
        Ok(())
    }

    pub fn remove_node(&mut self, node_id: Uuid) -> bool {
        let before_nodes = self.nodes.len();
        self.nodes.retain(|n| n.id() != node_id);
        if self.nodes.len() < before_nodes {
            self.edges
                .retain(|e| e.source_node_id != node_id && e.target_node_id != node_id);
            true
        } else {
            false
        }
    }

    pub fn find_node(&self, node_id: Uuid) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.id() == node_id)
    }

    pub fn find_node_mut(&mut self, node_id: Uuid) -> Option<&mut GraphNode> {
        self.nodes.iter_mut().find(|n| n.id() == node_id)
    }

    pub fn find_port(&self, port_id: Uuid) -> Option<&GraphPort> {
        for node in &self.nodes {
            if let Some(port) = node.find_port(port_id) {
                return Some(port);
            }
        }
        None
    }

    pub fn connect(&mut self, source_port_id: Uuid, target_port_id: Uuid) -> Result<GraphEdge> {
        let p1 = self
            .find_port(source_port_id)
            .cloned()
            .ok_or_else(|| "找不到指定的連接埠。".to_string())?;
        let p2 = self
            .find_port(target_port_id)
            .cloned()
            .ok_or_else(|| "找不到指定的連接埠。".to_string())?;

        // Ensure direction is Output -> Input
        let (from, to) = if p1.direction == PortDirection::Input && p2.direction == PortDirection::Output {
            (p2, p1)
        } else {
            (p1, p2)
        };

        if !from.can_connect_to(&to) {
            return Err(format!(
                "無法連接連接埠：{} ({:?}/{:?}) 與 {} ({:?}/{:?}) 不相容。",
                from.name, from.port_type, from.direction, to.name, to.port_type, to.direction
            ));
        }

        if !from.allow_multiple && self.edges.iter().any(|e| e.source_port_id == from.id) {
            return Err(format!("連接埠 {} 不允許輸出多條連線。", from.name));
        }

        if !to.allow_multiple && self.edges.iter().any(|e| e.target_port_id == to.id) {
            return Err(format!("連接埠 {} 不允許輸入多條連線。", to.name));
        }

        if self
            .edges
            .iter()
            .any(|e| e.source_port_id == from.id && e.target_port_id == to.id)
        {
            return Err("連線已存在。".into());
        }

        let edge = GraphEdge::new(from.node_id, from.id, to.node_id, to.id);
        self.edges.push(edge.clone());
        Ok(edge)
    }

    pub fn disconnect(&mut self, edge_id: Uuid) -> bool {
        let before = self.edges.len();
        self.edges.retain(|e| e.id != edge_id);
        self.edges.len() < before
    }

    pub fn disconnect_ports(&mut self, port_a_id: Uuid, port_b_id: Uuid) -> bool {
        let before = self.edges.len();
        self.edges.retain(|e| {
            !(e.source_port_id == port_a_id && e.target_port_id == port_b_id)
                && !(e.source_port_id == port_b_id && e.target_port_id == port_a_id)
        });
        self.edges.len() < before
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

    pub fn has_chained_triggers(&self) -> bool {
        for node in &self.nodes {
            if let GraphNode::Trigger(trigger) = node {
                if !self.get_incoming_edges(trigger.exec_in.id).is_empty() {
                    return true;
                }
            }
        }
        false
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.edges.clear();
    }
}

pub struct EventGraphConverter;

impl EventGraphConverter {
    pub const LAYOUT_START_Y: f32 = 60.0;
    pub const CONDITION_COLUMN_X: f32 = 60.0;
    pub const TRIGGER_COLUMN_X: f32 = 360.0;
    pub const ACTION_COLUMN_START_X: f32 = 660.0;
    pub const ACTION_SPACING_X: f32 = 260.0;
    pub const ROW_SPACING_Y: f32 = 240.0;
    pub const CONDITION_SPACING_Y: f32 = 90.0;

    pub fn from_scenario_events(events: &[Event]) -> EventGraph {
        Self::from_scenario_events_with_labels(events, |id| id.to_string())
    }

    pub fn from_scenario_events_with_labels<F: Fn(Uuid) -> String>(
        events: &[Event],
        target_label_resolver: F,
    ) -> EventGraph {
        let mut graph = EventGraph::new();
        let mut current_y = Self::LAYOUT_START_Y;

        for item in events {
            let event_start_y = current_y;

            // 1. Trigger node
            let mut trigger_node =
                EventTriggerNode::new(&item.name, item.delay_seconds, item.repeat, item.enabled);
            trigger_node.x = Self::TRIGGER_COLUMN_X;
            trigger_node.y = event_start_y;
            let _trigger_id = trigger_node.id;
            let conditions_in_id = trigger_node.conditions_in.id;
            let trigger_exec_out_id = trigger_node.exec_out.id;
            let _ = graph.add_node(GraphNode::Trigger(trigger_node));

            // 2. Condition nodes
            let mut cond_y = event_start_y;
            for cond in &item.conditions {
                let target_label = target_label_resolver(cond.target_id);
                let (cond_node, cond_out_id) = match cond.kind {
                    1 => {
                        let mut n = ObjectDeadConditionNode::new(cond.target_id, &target_label);
                        n.x = Self::CONDITION_COLUMN_X;
                        n.y = cond_y;
                        let out_id = n.condition_out.id;
                        (GraphNode::DeadCondition(n), out_id)
                    }
                    2 => {
                        let mut n = ObjectInAreaConditionNode::new(
                            cond.target_id,
                            cond.min_x,
                            cond.min_z,
                            cond.max_x,
                            cond.max_z,
                            &target_label,
                        );
                        n.x = Self::CONDITION_COLUMN_X;
                        n.y = cond_y;
                        let out_id = n.condition_out.id;
                        (GraphNode::AreaCondition(n), out_id)
                    }
                    _ => {
                        let mut n = ObjectExistsConditionNode::new(cond.target_id, &target_label);
                        n.x = Self::CONDITION_COLUMN_X;
                        n.y = cond_y;
                        let out_id = n.condition_out.id;
                        (GraphNode::ExistsCondition(n), out_id)
                    }
                };
                let _ = graph.add_node(cond_node);
                let _ = graph.connect(cond_out_id, conditions_in_id);
                cond_y += Self::CONDITION_SPACING_Y;
            }

            // 3. Action nodes
            let mut previous_exec_out = Some(trigger_exec_out_id);
            let mut action_x = Self::ACTION_COLUMN_START_X;

            for action in &item.actions {
                let (action_node, exec_in_id, exec_out_id) = match action.kind {
                    0 => {
                        let mut n = MessageActionNode::new(&action.text);
                        n.x = action_x;
                        n.y = event_start_y;
                        let in_id = n.exec_in.id;
                        let out_id = Some(n.exec_out.id);
                        (GraphNode::MessageAction(n), in_id, out_id)
                    }
                    1 => {
                        let mut n =
                            DiplomacyActionNode::new(action.team, action.other_team, action.hostile);
                        n.x = action_x;
                        n.y = event_start_y;
                        let in_id = n.exec_in.id;
                        let out_id = Some(n.exec_out.id);
                        (GraphNode::DiplomacyAction(n), in_id, out_id)
                    }
                    2 => {
                        let mut n = SpawnUnitActionNode::new(
                            &action.alias,
                            action.team,
                            action.x,
                            action.z,
                            action.count,
                        );
                        n.move_destination = action.move_destination.clone();
                        n.x = action_x;
                        n.y = event_start_y;
                        let in_id = n.exec_in.id;
                        let out_id = Some(n.exec_out.id);
                        (GraphNode::SpawnUnitAction(n), in_id, out_id)
                    }
                    3 => {
                        let mut n = VictoryActionNode::new();
                        n.x = action_x;
                        n.y = event_start_y;
                        let in_id = n.exec_in.id;
                        (GraphNode::VictoryAction(n), in_id, None)
                    }
                    4 => {
                        let mut n = DefeatActionNode::new();
                        n.x = action_x;
                        n.y = event_start_y;
                        let in_id = n.exec_in.id;
                        (GraphNode::DefeatAction(n), in_id, None)
                    }
                    _ => {
                        let mut n = MessageActionNode::new(&action.text);
                        n.x = action_x;
                        n.y = event_start_y;
                        let in_id = n.exec_in.id;
                        let out_id = Some(n.exec_out.id);
                        (GraphNode::MessageAction(n), in_id, out_id)
                    }
                };

                let _ = graph.add_node(action_node);
                if let Some(prev_out) = previous_exec_out {
                    let _ = graph.connect(prev_out, exec_in_id);
                }
                previous_exec_out = exec_out_id;
                action_x += Self::ACTION_SPACING_X;
            }

            let row_height = (cond_y - event_start_y + 60.0)
                .max(180.0)
                .max(Self::ROW_SPACING_Y);
            current_y += row_height;
        }

        graph
    }

    pub fn to_scenario_events(graph: &EventGraph) -> Result<Vec<Event>> {
        let mut result = Vec::new();

        for node in &graph.nodes {
            if let GraphNode::Trigger(trigger) = node {
                // 1. Collect conditions
                let mut conditions = Vec::new();
                let incoming_cond_edges = graph.get_incoming_edges(trigger.conditions_in.id);
                for edge in incoming_cond_edges {
                    if let Some(source_node) = graph.find_node(edge.source_node_id) {
                        match source_node {
                            GraphNode::ExistsCondition(c) => conditions.push(c.to_condition()),
                            GraphNode::DeadCondition(c) => conditions.push(c.to_condition()),
                            GraphNode::AreaCondition(c) => conditions.push(c.to_condition()),
                            _ => {}
                        }
                    }
                }

                // 2. Collect actions along exec_out
                let mut actions = Vec::new();
                let mut current_exec_port = Some(trigger.exec_out.id);
                let mut visited = HashSet::new();

                while let Some(port_id) = current_exec_port {
                    let outgoing = graph.get_outgoing_edges(port_id);
                    if outgoing.is_empty() {
                        break;
                    }
                    let edge = outgoing[0];
                    let next_node_id = edge.target_node_id;
                    if !visited.insert(next_node_id) {
                        break; // Cycle guard
                    }

                    if let Some(next_node) = graph.find_node(next_node_id) {
                        match next_node {
                            GraphNode::MessageAction(a) => {
                                actions.push(a.to_action());
                                current_exec_port = Some(a.exec_out.id);
                            }
                            GraphNode::DiplomacyAction(a) => {
                                actions.push(a.to_action());
                                current_exec_port = Some(a.exec_out.id);
                            }
                            GraphNode::SpawnUnitAction(a) => {
                                actions.push(a.to_action());
                                current_exec_port = Some(a.exec_out.id);
                            }
                            GraphNode::VictoryAction(a) => {
                                actions.push(a.to_action());
                                current_exec_port = None;
                            }
                            GraphNode::DefeatAction(a) => {
                                actions.push(a.to_action());
                                current_exec_port = None;
                            }
                            _ => {
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }

                result.push(Event {
                    name: trigger.event_name.clone(),
                    delay_seconds: trigger.delay_seconds,
                    repeat: trigger.repeat,
                    enabled: trigger.enabled,
                    actions,
                    conditions,
                });
            }
        }

        Ok(result)
    }
}

pub struct EventGraphValidator;

impl EventGraphValidator {
    pub fn validate(
        graph: &EventGraph,
        valid_target_ids: Option<&HashSet<Uuid>>,
        valid_aliases: Option<&[String]>,
    ) -> Vec<GraphDiagnostic> {
        let mut diagnostics = Vec::new();

        // 1. Trigger count check
        let trigger_count = graph
            .nodes
            .iter()
            .filter(|n| matches!(n, GraphNode::Trigger(_)))
            .count();
        if trigger_count > 256 {
            diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticSeverity::Error,
                "EVENT_LIMIT_EXCEEDED",
                format!("圖中事件觸發節點數量為 {trigger_count}，超過遊戲上限 256 個。"),
                None,
            ));
        }

        // 2. Cycle detection on execution flow
        Self::detect_execution_cycles(graph, &mut diagnostics);

        // 3. Node-by-node validation
        for node in &graph.nodes {
            match node {
                GraphNode::Trigger(trigger) => {
                    Self::validate_trigger(graph, trigger, &mut diagnostics);
                }
                GraphNode::ExistsCondition(c) => {
                    Self::validate_condition(
                        graph,
                        c.id,
                        c.target_id,
                        None,
                        &c.condition_out,
                        &c.target_label,
                        "Condition: Object Exists",
                        valid_target_ids,
                        &mut diagnostics,
                    );
                }
                GraphNode::DeadCondition(c) => {
                    Self::validate_condition(
                        graph,
                        c.id,
                        c.target_id,
                        None,
                        &c.condition_out,
                        &c.target_label,
                        "Condition: Object Dead/Removed",
                        valid_target_ids,
                        &mut diagnostics,
                    );
                }
                GraphNode::AreaCondition(c) => {
                    Self::validate_condition(
                        graph,
                        c.id,
                        c.target_id,
                        Some((c.min_x, c.min_z, c.max_x, c.max_z)),
                        &c.condition_out,
                        &c.target_label,
                        "Condition: Object In Area",
                        valid_target_ids,
                        &mut diagnostics,
                    );
                }
                GraphNode::MessageAction(a) => {
                    Self::validate_orphan_action(graph, a.id, "Action: Show Message", &a.exec_in, &mut diagnostics);
                    if a.message_text.trim().is_empty() {
                        diagnostics.push(GraphDiagnostic::new(
                            GraphDiagnosticSeverity::Error,
                            "EMPTY_MESSAGE_TEXT",
                            "訊息動作文字不可為空。",
                            Some(a.id),
                        ));
                    }
                }
                GraphNode::DiplomacyAction(a) => {
                    Self::validate_orphan_action(graph, a.id, "Action: Diplomacy", &a.exec_in, &mut diagnostics);
                    if !(0..=7).contains(&a.team)
                        || !(0..=7).contains(&a.other_team)
                        || a.team == a.other_team
                    {
                        diagnostics.push(GraphDiagnostic::new(
                            GraphDiagnosticSeverity::Error,
                            "INVALID_DIPLOMACY_TEAMS",
                            format!(
                                "外交動作需要兩個不同隊伍（0–7），當前設定為隊伍 {} 與隊伍 {}。",
                                a.team, a.other_team
                            ),
                            Some(a.id),
                        ));
                    }
                }
                GraphNode::SpawnUnitAction(a) => {
                    Self::validate_orphan_action(graph, a.id, "Action: Spawn Unit", &a.exec_in, &mut diagnostics);
                    if !(0..=7).contains(&a.team) {
                        diagnostics.push(GraphDiagnostic::new(
                            GraphDiagnosticSeverity::Error,
                            "INVALID_SPAWN_TEAM",
                            format!("生成部隊隊伍必須介於 0 與 7，當前為 {}。", a.team),
                            Some(a.id),
                        ));
                    }
                    if !(1..=20).contains(&a.count) {
                        diagnostics.push(GraphDiagnostic::new(
                            GraphDiagnosticSeverity::Error,
                            "INVALID_SPAWN_COUNT",
                            format!("生成部隊人數必須介於 1 與 20，當前為 {}。", a.count),
                            Some(a.id),
                        ));
                    }
                    if !a.spawn_x.is_finite()
                        || !a.spawn_z.is_finite()
                        || !(0.0..=16383.0).contains(&a.spawn_x)
                        || !(0.0..=16383.0).contains(&a.spawn_z)
                    {
                        diagnostics.push(GraphDiagnostic::new(
                            GraphDiagnosticSeverity::Error,
                            "INVALID_SPAWN_COORDINATES",
                            format!(
                                "生成部隊座標必須介於 0 與 16383 之間，當前為 ({}, {})。",
                                a.spawn_x, a.spawn_z
                            ),
                            Some(a.id),
                        ));
                    }
                    if let Some(aliases) = valid_aliases {
                        if !aliases.iter().any(|alias| alias.eq_ignore_ascii_case(&a.alias)) {
                            diagnostics.push(GraphDiagnostic::new(
                                GraphDiagnosticSeverity::Error,
                                "INVALID_SPAWN_ALIAS",
                                format!("未知的部隊別名：{}。", a.alias),
                                Some(a.id),
                            ));
                        }
                    }
                }
                GraphNode::VictoryAction(a) => {
                    Self::validate_orphan_action(graph, a.id, "Action: Victory", &a.exec_in, &mut diagnostics);
                }
                GraphNode::DefeatAction(a) => {
                    Self::validate_orphan_action(graph, a.id, "Action: Defeat", &a.exec_in, &mut diagnostics);
                }
                GraphNode::Delay(d) => {
                    let incoming = graph.get_incoming_edges(d.exec_in.id);
                    if incoming.is_empty() {
                        diagnostics.push(GraphDiagnostic::new(
                            GraphDiagnosticSeverity::Warning,
                            "ORPHAN_DELAY",
                            format!("延遲節點「Delay: {}s」沒有前置執行連線。", d.seconds),
                            Some(d.id),
                        ));
                    }
                    if d.seconds < 0 {
                        diagnostics.push(GraphDiagnostic::new(
                            GraphDiagnosticSeverity::Error,
                            "NEGATIVE_DELAY",
                            "延遲時間不能為負數。",
                            Some(d.id),
                        ));
                    }
                }
            }
        }

        diagnostics
    }

    fn validate_trigger(
        graph: &EventGraph,
        trigger: &EventTriggerNode,
        diagnostics: &mut Vec<GraphDiagnostic>,
    ) {
        if trigger.event_name.trim().is_empty() {
            diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticSeverity::Error,
                "EMPTY_EVENT_NAME",
                "事件觸發節點名稱不可為空。",
                Some(trigger.id),
            ));
        }

        if !(0..=86400).contains(&trigger.delay_seconds) {
            diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticSeverity::Error,
                "INVALID_DELAY",
                format!(
                    "事件「{}」延遲秒數必須介於 0 與 86400 秒。",
                    trigger.event_name
                ),
                Some(trigger.id),
            ));
        }

        if trigger.repeat && trigger.delay_seconds == 0 {
            diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticSeverity::Error,
                "REPEAT_ZERO_DELAY",
                format!(
                    "重複執行的事件「{}」間隔時間至少必須為 1 秒，否則將造成每一幀觸發之死鎖問題。",
                    trigger.event_name
                ),
                Some(trigger.id),
            ));
        }

        let incoming_conditions = graph.get_incoming_edges(trigger.conditions_in.id);
        if incoming_conditions.len() > 32 {
            diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticSeverity::Error,
                "CONDITION_LIMIT_EXCEEDED",
                format!(
                    "事件「{}」連接了 {} 個條件，超過上限 32 個。",
                    trigger.event_name,
                    incoming_conditions.len()
                ),
                Some(trigger.id),
            ));
        }

        let outgoing_exec = graph.get_outgoing_edges(trigger.exec_out.id);
        if outgoing_exec.is_empty() {
            diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticSeverity::Warning,
                "EMPTY_TRIGGER_FLOW",
                format!(
                    "事件觸發節點「{}」未連接任何後續動作，觸發時將無實質效果。",
                    trigger.event_name
                ),
                Some(trigger.id),
            ));
        } else {
            // Count actions along chain
            let mut action_count = 0;
            let mut current_port = Some(trigger.exec_out.id);
            let mut visited = HashSet::new();

            while let Some(port_id) = current_port {
                let next_edges = graph.get_outgoing_edges(port_id);
                if next_edges.is_empty() {
                    break;
                }
                let next_node_id = next_edges[0].target_node_id;
                if !visited.insert(next_node_id) {
                    break;
                }

                if let Some(next_node) = graph.find_node(next_node_id) {
                    match next_node {
                        GraphNode::MessageAction(a) => {
                            action_count += 1;
                            current_port = Some(a.exec_out.id);
                        }
                        GraphNode::DiplomacyAction(a) => {
                            action_count += 1;
                            current_port = Some(a.exec_out.id);
                        }
                        GraphNode::SpawnUnitAction(a) => {
                            action_count += 1;
                            current_port = Some(a.exec_out.id);
                        }
                        GraphNode::VictoryAction(a) => {
                            action_count += 1;
                            if trigger.repeat {
                                diagnostics.push(GraphDiagnostic::new(
                                    GraphDiagnosticSeverity::Error,
                                    "TERMINAL_IN_REPEAT_EVENT",
                                    format!(
                                        "事件「{}」設定為重複執行，但其動作鏈中包含勝利/失敗終止動作。",
                                        trigger.event_name
                                    ),
                                    Some(a.id),
                                ));
                            }
                            current_port = None;
                        }
                        GraphNode::DefeatAction(a) => {
                            action_count += 1;
                            if trigger.repeat {
                                diagnostics.push(GraphDiagnostic::new(
                                    GraphDiagnosticSeverity::Error,
                                    "TERMINAL_IN_REPEAT_EVENT",
                                    format!(
                                        "事件「{}」設定為重複執行，但其動作鏈中包含勝利/失敗終止動作。",
                                        trigger.event_name
                                    ),
                                    Some(a.id),
                                ));
                            }
                            current_port = None;
                        }
                        GraphNode::Delay(d) => {
                            current_port = Some(d.exec_out.id);
                        }
                        _ => break,
                    }
                } else {
                    break;
                }
            }

            if action_count > 32 {
                diagnostics.push(GraphDiagnostic::new(
                    GraphDiagnosticSeverity::Error,
                    "ACTION_LIMIT_EXCEEDED",
                    format!(
                        "事件「{}」動作數量為 {}，超過上限 32 個。",
                        trigger.event_name, action_count
                    ),
                    Some(trigger.id),
                ));
            }
        }
    }

    fn validate_orphan_action(
        graph: &EventGraph,
        action_id: Uuid,
        action_title: &str,
        exec_in: &GraphPort,
        diagnostics: &mut Vec<GraphDiagnostic>,
    ) {
        let incoming = graph.get_incoming_edges(exec_in.id);
        if incoming.is_empty() {
            diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticSeverity::Warning,
                "ORPHAN_ACTION",
                format!("動作節點「{action_title}」沒有前置執行連線，永遠不會被執行。"),
                Some(action_id),
            ));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_condition(
        graph: &EventGraph,
        cond_id: Uuid,
        target_id: Uuid,
        area_bounds: Option<(i32, i32, i32, i32)>,
        condition_out: &GraphPort,
        _target_label: &str,
        title: &str,
        valid_target_ids: Option<&HashSet<Uuid>>,
        diagnostics: &mut Vec<GraphDiagnostic>,
    ) {
        if target_id.is_nil() {
            diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticSeverity::Error,
                "EMPTY_TARGET_GUID",
                format!("條件節點「{title}」尚未指定目標物件 GUID。"),
                Some(cond_id),
            ));
        } else if let Some(targets) = valid_target_ids {
            if !targets.contains(&target_id) {
                diagnostics.push(GraphDiagnostic::new(
                    GraphDiagnosticSeverity::Error,
                    "INVALID_TARGET_GUID",
                    format!(
                        "條件節點「{title}」引用的目標物件 GUID ({target_id}) 在地圖中不存在或已遭刪除。"
                    ),
                    Some(cond_id),
                ));
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
                diagnostics.push(GraphDiagnostic::new(
                    GraphDiagnosticSeverity::Error,
                    "INVALID_AREA_BOUNDS",
                    format!(
                        "區域條件座標範圍無效：[{min_x}, {min_z}] 至 [{max_x}, {max_z}] 必須在 0–16383 範圍內且 Min <= Max。"
                    ),
                    Some(cond_id),
                ));
            }
        }

        let outgoing = graph.get_outgoing_edges(condition_out.id);
        if outgoing.is_empty() {
            diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticSeverity::Warning,
                "DANGLING_CONDITION",
                format!("條件節點「{title}」未連接至任何事件觸發節點，屬於懸空條件。"),
                Some(cond_id),
            ));
        }
    }

    fn detect_execution_cycles(graph: &EventGraph, diagnostics: &mut Vec<GraphDiagnostic>) {
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

        // Color map: 0 = White, 1 = Gray, 2 = Black
        let mut color: HashMap<Uuid, u8> = HashMap::new();
        for node in &graph.nodes {
            color.insert(node.id(), 0);
        }

        for node in &graph.nodes {
            let node_id = node.id();
            if color.get(&node_id).copied().unwrap_or(0) == 0 {
                let mut path = Vec::new();
                if Self::dfs_cycle(node_id, &adj, &mut color, &mut path) {
                    diagnostics.push(GraphDiagnostic::new(
                        GraphDiagnosticSeverity::Error,
                        "EXECUTION_CYCLE_DETECTED",
                        format!(
                            "偵測到執行流死循環（包含節點 {}），將導致遊戲腳本引擎陷入無窮迴圈或死鎖。",
                            node.title()
                        ),
                        Some(node_id),
                    ));
                }
            }
        }
    }

    fn dfs_cycle(
        u: Uuid,
        adj: &HashMap<Uuid, Vec<Uuid>>,
        color: &mut HashMap<Uuid, u8>,
        path: &mut Vec<Uuid>,
    ) -> bool {
        color.insert(u, 1); // Gray
        path.push(u);

        if let Some(neighbors) = adj.get(&u) {
            for &v in neighbors {
                let c = color.get(&v).copied().unwrap_or(0);
                if c == 1 {
                    return true;
                }
                if c == 0 && Self::dfs_cycle(v, adj, color, path) {
                    return true;
                }
            }
        }

        color.insert(u, 2); // Black
        path.pop();
        false
    }
}

/// Headless scenario event editing session matching C# `ScenarioEventSession`.
/// Provides undo/redo, dirty tracking, event limit enforcement, and baseline rollback.
#[derive(Clone, Debug)]
pub struct ScenarioEventSession {
    events: Vec<Event>,
    baseline: Vec<Event>,
    undo: Vec<Vec<Event>>,
    redo: Vec<Vec<Event>>,
}

impl Default for ScenarioEventSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ScenarioEventSession {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            baseline: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn with_events(events: Vec<Event>) -> Self {
        let baseline = events.clone();
        Self {
            events,
            baseline,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn count(&self) -> usize {
        self.events.len()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn baseline(&self) -> &[Event] {
        &self.baseline
    }

    pub fn is_dirty(&self) -> bool {
        self.events != self.baseline
    }

    pub fn load(&mut self, snapshot: &[Event]) {
        self.events = snapshot.to_vec();
        self.undo.clear();
        self.redo.clear();
        self.accept_changes();
    }

    pub fn capture(&self) -> Vec<Event> {
        self.events.clone()
    }

    pub fn accept_changes(&mut self) {
        self.baseline = self.events.clone();
    }

    pub fn accept_baseline(&mut self, snapshot: &[Event]) {
        self.baseline = snapshot.to_vec();
    }

    pub fn reset(&mut self) {
        self.events = self.baseline.clone();
        self.undo.clear();
        self.redo.clear();
    }

    pub fn add(&mut self, item: Event) -> Result<usize> {
        let index = self.events.len();
        self.add_range(std::slice::from_ref(&item))?;
        Ok(index)
    }

    pub fn add_range(&mut self, items: &[Event]) -> Result<()> {
        if items.len() > 256usize.saturating_sub(self.events.len()) {
            return Err("事件上限為 256 個。".into());
        }
        if items.is_empty() {
            return Ok(());
        }
        self.remember();
        self.events.extend_from_slice(items);
        Ok(())
    }

    pub fn replace_all(&mut self, items: &[Event]) -> Result<()> {
        if items.len() > 256 {
            return Err("事件上限為 256 個。".into());
        }
        self.remember();
        self.events = items.to_vec();
        Ok(())
    }

    pub fn replace(&mut self, index: usize, item: Event) -> Result<()> {
        if index >= self.events.len() {
            return Err("event index out of bounds".into());
        }
        self.remember();
        self.events[index] = item;
        Ok(())
    }

    pub fn remove_at(&mut self, index: usize) -> Result<Event> {
        if index >= self.events.len() {
            return Err("event index out of bounds".into());
        }
        self.remember();
        Ok(self.events.remove(index))
    }

    pub fn duplicate(&mut self, index: usize, suffix: &str) -> Result<usize> {
        if self.events.len() >= 256 {
            return Err("事件上限為 256 個。".into());
        }
        if suffix.encode_utf16().count() > 100 {
            return Err("副本名稱後綴過長。".into());
        }
        let source = self.events.get(index).ok_or("event index out of bounds")?;
        let max_base_len = 100usize.saturating_sub(suffix.encode_utf16().count());
        let mut base_name = String::new();
        let mut utf16_count = 0;
        for c in source.name.chars() {
            let c_len = c.len_utf16();
            if utf16_count + c_len > max_base_len {
                break;
            }
            base_name.push(c);
            utf16_count += c_len;
        }
        base_name.push_str(suffix);
        let mut copy = source.clone();
        copy.name = base_name;
        self.remember();
        let insert_idx = index + 1;
        self.events.insert(insert_idx, copy);
        Ok(insert_idx)
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(self.events.clone());
            self.events = prev;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.events.clone());
            self.events = next;
            true
        } else {
            false
        }
    }

    fn remember(&mut self) {
        self.undo.push(self.events.clone());
        self.redo.clear();
    }
}
