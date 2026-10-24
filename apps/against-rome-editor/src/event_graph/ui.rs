//! 劇情事件節點圖 2D 畫布介面 (egui EventGraph UI)
//!
//! 提供節點視覺化畫布、視角平移、縮放、三次貝茲曲線連線、節點拖曳選取、端口連線互動與靜態診斷顯示。

use eframe::egui::{
    self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Vec2,
};
use std::collections::HashSet;
use uuid::Uuid;

use crate::event_graph::converter;
use crate::event_graph::model::{
    EventGraph, GraphDiagnostic, GraphDiagnosticSeverity, GraphNode, NodeKind, PortDirection,
    PortType,
};
use crate::event_graph::validator;

const DEFAULT_NODE_WIDTH: f32 = 220.0;
const DEFAULT_NODE_HEADER_HEIGHT: f32 = 28.0;
const PORT_ROW_HEIGHT: f32 = 24.0;
const PORT_CIRCLE_RADIUS: f32 = 5.5;

pub struct EventGraphUi {
    pub open: bool,
    pub graph: EventGraph,
    pub zoom: f32,
    pub pan: Vec2,
    pub selected_nodes: HashSet<Uuid>,
    pub diagnostics: Vec<GraphDiagnostic>,
    pub is_dirty: bool,
    pub status_message: String,

    // 互動狀態
    dragging_node_id: Option<Uuid>,
    connecting_port_id: Option<Uuid>,
    connecting_start_pos: Option<Pos2>,
    connecting_current_pos: Option<Pos2>,

    // 新增節點對話選單
    show_add_menu: bool,
    new_node_kind_selection: usize,

    // 關閉確認與套用狀態
    pub pending_close: bool,
    pub apply_requested: bool,
}

impl Default for EventGraphUi {
    fn default() -> Self {
        Self {
            open: false,
            graph: EventGraph::new(),
            zoom: 1.0,
            pan: Vec2::new(100.0, 100.0),
            selected_nodes: HashSet::new(),
            diagnostics: Vec::new(),
            is_dirty: false,
            status_message: String::new(),
            dragging_node_id: None,
            connecting_port_id: None,
            connecting_start_pos: None,
            connecting_current_pos: None,
            show_add_menu: false,
            new_node_kind_selection: 0,
            pending_close: false,
            apply_requested: false,
        }
    }
}

impl EventGraphUi {
    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
        self.validate();
    }

    pub fn clear_transient(&mut self) {
        self.dragging_node_id = None;
        self.connecting_port_id = None;
        self.connecting_start_pos = None;
        self.connecting_current_pos = None;
        self.show_add_menu = false;
        self.pending_close = false;
        self.apply_requested = false;
    }

    pub fn validate(&mut self) {
        self.diagnostics = validator::validate(&self.graph, None, None);
        let errors = self
            .diagnostics
            .iter()
            .filter(|d| d.severity == GraphDiagnosticSeverity::Error)
            .count();
        let warnings = self
            .diagnostics
            .iter()
            .filter(|d| d.severity == GraphDiagnosticSeverity::Warning)
            .count();
        self.status_message = format!("診斷完成：{errors} 項錯誤，{warnings} 項警告。");
    }

    /// Validates graph structure and port connections before compilation.
    /// Fails closed if any edge references missing nodes, missing ports,
    /// incompatible directions/types, self loops, or violates single-edge port constraints.
    pub fn validate_graph_integrity(&self) -> Result<(), String> {
        let mut in_counts: std::collections::HashMap<Uuid, usize> = std::collections::HashMap::new();
        let mut out_counts: std::collections::HashMap<Uuid, usize> = std::collections::HashMap::new();

        for edge in &self.graph.edges {
            if edge.source_node_id == edge.target_node_id {
                return Err(format!("節點不可自我連線：節點 {}", edge.source_node_id));
            }
            let s_node = self
                .graph
                .find_node(edge.source_node_id)
                .ok_or_else(|| format!("連線參照了不存在的來源節點 {}", edge.source_node_id))?;
            let t_node = self
                .graph
                .find_node(edge.target_node_id)
                .ok_or_else(|| format!("連線參照了不存在的目標節點 {}", edge.target_node_id))?;

            let s_port = s_node
                .find_port(edge.source_port_id)
                .ok_or_else(|| format!("連線參照了不存在的來源端口 {}", edge.source_port_id))?;
            let t_port = t_node
                .find_port(edge.target_port_id)
                .ok_or_else(|| format!("連線參照了不存在的目標端口 {}", edge.target_port_id))?;

            if s_port.direction != PortDirection::Output || t_port.direction != PortDirection::Input {
                return Err(format!(
                    "連線端口方向不匹配：來源 {} ({:?}) -> 目標 {} ({:?})",
                    s_port.name, s_port.direction, t_port.name, t_port.direction
                ));
            }

            if s_port.port_type != t_port.port_type {
                return Err(format!(
                    "連線端口型別不相容：來源 {} ({:?}) 與 目標 {} ({:?})",
                    s_port.name, s_port.port_type, t_port.name, t_port.port_type
                ));
            }

            *in_counts.entry(t_port.id).or_insert(0) += 1;
            *out_counts.entry(s_port.id).or_insert(0) += 1;

            if !t_port.allow_multiple && in_counts[&t_port.id] > 1 {
                return Err(format!("目標端口 {} 不允許多條連線輸入。", t_port.name));
            }
            if !s_port.allow_multiple && out_counts[&s_port.id] > 1 {
                return Err(format!("來源端口 {} 不允許多條連線輸出。", s_port.name));
            }
        }
        Ok(())
    }

    /// Full static validation with scenario target objects and catalog aliases.
    /// Checks structural edge integrity, cycle detection, orphan/terminal action rules,
    /// spawn aliases and area bounds.
    pub fn validate_with_scenario(
        &mut self,
        scenario: &against_rome_scenario::ScenarioDocument,
        aliases: &[String],
    ) -> Result<(), String> {
        self.diagnostics.clear();

        // 1. 結構與端口連線完整性檢查
        if let Err(e) = self.validate_graph_integrity() {
            self.diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticSeverity::Error,
                "INVALID_LINK",
                e.clone(),
            ));
            self.status_message = format!("節點連線無效：{e}");
            return Err(e);
        }

        // 2. 節點語義與業務邏輯驗證
        let valid_targets: HashSet<Uuid> = scenario.spawns.iter().map(|s| s.id).collect();
        let alias_slice = if aliases.is_empty() {
            None
        } else {
            Some(aliases)
        };
        self.diagnostics = validator::validate(&self.graph, Some(&valid_targets), alias_slice);

        // 檢查是否有 Delay 節點（Against Rome 劇本動作序列不支援獨立 Delay 節點）
        for node in &self.graph.nodes {
            if matches!(node.kind, NodeKind::Delay { .. }) {
                self.diagnostics.push(
                    GraphDiagnostic::new(
                        GraphDiagnosticSeverity::Error,
                        "UNSUPPORTED_DELAY_NODE",
                        format!(
                            "劇本事件不支援 Delay 節點「{}」；請使用事件觸發節點本身的延遲秒數設定。",
                            node.title()
                        ),
                    )
                    .with_node(node.id),
                );
            }
        }

        let errors: Vec<_> = self
            .diagnostics
            .iter()
            .filter(|d| d.severity == GraphDiagnosticSeverity::Error)
            .collect();
        let warnings = self
            .diagnostics
            .iter()
            .filter(|d| d.severity == GraphDiagnosticSeverity::Warning)
            .count();

        if let Some(first_err) = errors.first() {
            self.status_message = format!("驗證錯誤 [{}]: {}", first_err.code, first_err.message);
            Err(format!("{}: {}", first_err.code, first_err.message))
        } else {
            self.status_message = format!("診斷通過：0 項錯誤，{warnings} 項警告。");
            Ok(())
        }
    }

    /// Compiles graph to scenario events after validating integrity and business rules.
    /// Fails closed if graph errors exist or compiled scenario cannot validate against aliases.
    pub fn compile_to_scenario_events_checked(
        &mut self,
        scenario: &against_rome_scenario::ScenarioDocument,
        aliases: &[String],
    ) -> Result<Vec<against_rome_scenario::Event>, String> {
        self.validate_with_scenario(scenario, aliases)?;
        let events = converter::to_scenario_events(&self.graph);

        let mut draft = scenario.clone();
        draft.events = events.clone();
        draft
            .validate(if aliases.is_empty() {
                None
            } else {
                Some(aliases)
            })
            .map_err(|e| format!("編譯後劇本驗證失敗：{e}"))?;

        Ok(events)
    }

    pub fn load_from_scenario_events(&mut self, events: &[against_rome_scenario::Event]) {
        self.graph = converter::from_scenario_events(events, None);
        self.selected_nodes.clear();
        self.is_dirty = false;
        self.clear_transient();
        self.validate();
    }

    fn locate_node(&mut self, id: Uuid, canvas: Rect) -> bool {
        let Some(node) = self.graph.nodes.iter().find(|n| n.id == id) else { return false; };
        self.pan = canvas.size() * 0.5 - get_node_bounds_world(node).center().to_vec2() * self.zoom;
        self.selected_nodes.clear();
        self.selected_nodes.insert(id);
        true
    }

    fn locate_button(&self, ui: &mut egui::Ui, id: Uuid) -> egui::Response {
        ui.add_enabled(self.graph.nodes.iter().any(|n| n.id == id),
            egui::Button::new("Locate / 定位"))
    }

    fn diagnostic_row(&self, ui: &mut egui::Ui, diagnostic: &GraphDiagnostic) -> bool {
        let mut locate = false;
        ui.horizontal(|ui| {
            ui.label(format!("[{}] {}", diagnostic.code, diagnostic.message));
            if let Some(id) = diagnostic.node_id {
                locate = self.locate_button(ui, id).clicked();
            }
        });
        locate
    }

    /// 清除地圖相關狀態，保留視角與使用者偏好。
    pub fn reset_session(&mut self) {
        self.graph = EventGraph::new();
        self.selected_nodes.clear();
        self.diagnostics.clear();
        self.is_dirty = false;
        self.status_message.clear();
        self.clear_transient();
    }

    /// Reloads or resets the event graph from the scenario document.
    /// If scenario events are present, converts them into visual graph nodes; if empty, cleanly resets.
    pub fn reload_from_scenario(&mut self, scenario: &against_rome_scenario::ScenarioDocument) {
        if scenario.events.is_empty() {
            self.reset_session();
        } else {
            let label_map: std::collections::HashMap<Uuid, String> = scenario
                .spawns
                .iter()
                .map(|s| (s.id, format!("{} / {}", s.alias, s.id)))
                .collect();
            self.graph = converter::from_scenario_events(
                &scenario.events,
                Some(&|id| {
                    label_map
                        .get(&id)
                        .cloned()
                        .unwrap_or_else(|| id.to_string())
                }),
            );
            self.selected_nodes.clear();
            self.is_dirty = false;
            self.clear_transient();
            self.validate();
        }
    }

    pub fn compile_to_scenario_events(&self) -> Vec<against_rome_scenario::Event> {
        converter::to_scenario_events(&self.graph)
    }

    /// Request closing the event graph editor.
    /// If changes are unsaved, opens pending close confirmation modal.
    pub fn request_close(&mut self) {
        if self.is_dirty {
            self.pending_close = true;
        } else {
            self.open = false;
            self.pending_close = false;
        }
    }

    /// Discards unsaved edits and closes the window.
    pub fn discard_and_close(&mut self) {
        self.is_dirty = false;
        self.open = false;
        self.pending_close = false;
        self.clear_transient();
    }

    /// Cancels close request and keeps the editor open.
    pub fn cancel_close(&mut self) {
        self.pending_close = false;
        self.open = true;
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if self.pending_close {
            egui::Window::new("未儲存的節點圖修改")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("節點圖中有未套用的修改。關閉將放棄這些變更。");
                    ui.horizontal(|ui| {
                        if ui.button("放棄修改並關閉").clicked() {
                            self.discard_and_close();
                        }
                        if ui.button("取消").clicked() {
                            self.cancel_close();
                        }
                    });
                });
        }

        if !self.open {
            return;
        }

        let mut open = self.open;
        egui::Window::new("Visual Event Graph (劇情事件節點圖)")
            .open(&mut open)
            .default_size(vec2_scaled(800.0, 500.0))
            .show(ctx, |ui| {
                // 頂部工具列
                ui.horizontal_wrapped(|ui| {
                    if ui.button("驗證 (Validate)").clicked() {
                        self.validate();
                    }
                    if ui
                        .add_enabled(
                            self.is_dirty,
                            egui::Button::new("套用至劇本 (Apply to Scenario)"),
                        )
                        .clicked()
                    {
                        self.apply_requested = true;
                    }
                    if ui
                        .add_enabled(
                            !self.selected_nodes.is_empty(),
                            egui::Button::new("中斷選取連線 (Disconnect Selected)"),
                        )
                        .clicked()
                    {
                        for &node_id in &self.selected_nodes {
                            let edges = self
                                .graph
                                .get_node_edges(node_id)
                                .iter()
                                .map(|e| e.id)
                                .collect::<Vec<_>>();
                            for edge_id in edges {
                                self.graph.disconnect(edge_id);
                            }
                        }
                        self.mark_dirty();
                    }
                    if ui
                        .add_enabled(
                            !self.selected_nodes.is_empty(),
                            egui::Button::new("刪除選取節點 (Delete Selected)"),
                        )
                        .clicked()
                    {
                        for node_id in self.selected_nodes.drain().collect::<Vec<_>>() {
                            self.graph.remove_node(node_id);
                        }
                        self.mark_dirty();
                    }
                    if ui.button("新增節點 (Add Node)...").clicked() {
                        self.show_add_menu = !self.show_add_menu;
                    }

                    ui.separator();
                    ui.label(format!("Zoom: {:.1}x", self.zoom));
                    if ui.button("重設視角 (Reset View)").clicked() {
                        self.zoom = 1.0;
                        self.pan = Vec2::new(100.0, 100.0);
                    }

                    if self.is_dirty {
                        ui.colored_label(Color32::YELLOW, "[未儲存變更]");
                    }
                });

                // 新增節點彈出區域
                if self.show_add_menu {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label("選擇節點型別:");
                            egui::ComboBox::from_id_source("add_node_type_combo")
                                .selected_text(match self.new_node_kind_selection {
                                    0 => "Event Trigger",
                                    1 => "Condition: Object Exists",
                                    2 => "Condition: Object Dead",
                                    3 => "Condition: Object In Area",
                                    4 => "Action: Show Message",
                                    5 => "Action: Diplomacy",
                                    6 => "Action: Spawn Unit",
                                    7 => "Action: Victory",
                                    8 => "Action: Defeat",
                                    _ => "Delay",
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.new_node_kind_selection, 0, "Event Trigger");
                                    ui.selectable_value(&mut self.new_node_kind_selection, 1, "Condition: Object Exists");
                                    ui.selectable_value(&mut self.new_node_kind_selection, 2, "Condition: Object Dead");
                                    ui.selectable_value(&mut self.new_node_kind_selection, 3, "Condition: Object In Area");
                                    ui.selectable_value(&mut self.new_node_kind_selection, 4, "Action: Show Message");
                                    ui.selectable_value(&mut self.new_node_kind_selection, 5, "Action: Diplomacy");
                                    ui.selectable_value(&mut self.new_node_kind_selection, 6, "Action: Spawn Unit");
                                    ui.selectable_value(&mut self.new_node_kind_selection, 7, "Action: Victory");
                                    ui.selectable_value(&mut self.new_node_kind_selection, 8, "Action: Defeat");
                                    ui.selectable_value(&mut self.new_node_kind_selection, 9, "Delay");
                                });

                            if ui.button("確認加入").clicked() {
                                let mut new_node = match self.new_node_kind_selection {
                                    0 => GraphNode::new_event_trigger("New Event", 10, false, true),
                                    1 => GraphNode::new_object_exists(Uuid::nil(), "Target"),
                                    2 => GraphNode::new_object_dead(Uuid::nil(), "Target"),
                                    3 => GraphNode::new_object_in_area(Uuid::nil(), 0, 0, 16383, 16383, "Target"),
                                    4 => GraphNode::new_message_action("Message text"),
                                    5 => GraphNode::new_diplomacy_action(0, 1, true),
                                    6 => GraphNode::new_spawn_unit_action("GER_INF01", 0, 8000.0, 8000.0, 10),
                                    7 => GraphNode::new_victory_action(),
                                    8 => GraphNode::new_defeat_action(),
                                    _ => GraphNode::new_delay(5),
                                };
                                new_node.x = 200.0 - self.pan.x;
                                new_node.y = 200.0 - self.pan.y;
                                self.graph.add_node(new_node);
                                self.show_add_menu = false;
                                self.mark_dirty();
                            }
                            if ui.button("取消").clicked() {
                                self.show_add_menu = false;
                            }
                        });
                    });
                }

                // 畫布主體區域
                let canvas_size = ui.available_size() - Vec2::new(0.0, if self.diagnostics.is_empty() { 60.0 } else { 180.0 });
                let (response, painter) = ui.allocate_painter(canvas_size.max(Vec2::new(200.0, 200.0)), Sense::click_and_drag());
                let canvas_rect = response.rect;

                // 背景網格自繪
                painter.rect_filled(canvas_rect, Rounding::ZERO, Color32::from_rgb(26, 26, 28));
                draw_grid(&painter, canvas_rect, self.pan, self.zoom);

                // 視角平移與縮放處理
                if response.dragged_by(egui::PointerButton::Middle) || (response.dragged_by(egui::PointerButton::Primary) && ctx.input(|i| i.modifiers.alt)) {
                    self.pan += response.drag_delta();
                }

                let hover_pos = response.hover_pos();
                if response.hovered() {
                    let scroll_delta = ctx.input(|i| i.raw_scroll_delta.y);
                    if scroll_delta != 0.0 {
                        let zoom_factor = if scroll_delta > 0.0 { 1.1 } else { 0.9 };
                        let new_zoom = (self.zoom * zoom_factor).clamp(0.25, 2.5);
                        if let Some(mouse) = hover_pos {
                            self.pan = mouse - canvas_rect.min - (mouse - canvas_rect.min - self.pan) * (new_zoom / self.zoom);
                        }
                        self.zoom = new_zoom;
                    }
                }

                // 世界座標與螢幕座標轉換閉包
                let world_to_screen = |wx: f32, wy: f32| -> Pos2 {
                    Pos2::new(canvas_rect.min.x + self.pan.x + wx * self.zoom, canvas_rect.min.y + self.pan.y + wy * self.zoom)
                };
                let _screen_to_world = |pos: Pos2| -> (f32, f32) {
                    ((pos.x - canvas_rect.min.x - self.pan.x) / self.zoom, (pos.y - canvas_rect.min.y - self.pan.y) / self.zoom)
                };

                // 1. 繪製現有連線 (Edges)
                for edge in &self.graph.edges {
                    let sp = self.graph.find_port(edge.source_port_id);
                    let tp = self.graph.find_port(edge.target_port_id);
                    if let (Some(source_port), Some(target_port)) = (sp, tp) {
                        let sn = self.graph.find_node(edge.source_node_id);
                        let tn = self.graph.find_node(edge.target_node_id);
                        if let (Some(s_node), Some(t_node)) = (sn, tn) {
                            let p1_world = get_port_center_world(s_node, source_port);
                            let p2_world = get_port_center_world(t_node, target_port);
                            let p1 = world_to_screen(p1_world.x, p1_world.y);
                            let p2 = world_to_screen(p2_world.x, p2_world.y);

                            let wire_color = match source_port.port_type {
                                PortType::Execution => Color32::from_rgb(80, 200, 255),
                                PortType::Condition => Color32::from_rgb(120, 240, 120),
                                PortType::Data => Color32::from_rgb(255, 200, 80),
                            };

                            draw_bezier_wire(&painter, p1, p2, wire_color, 2.0 * self.zoom);
                        }
                    }
                }

                // 2. 繪製拉線暫存 (Connecting Wire)
                if let (Some(start_pos), Some(cur_pos)) = (self.connecting_start_pos, self.connecting_current_pos) {
                    draw_bezier_wire(&painter, start_pos, cur_pos, Color32::WHITE, 2.5 * self.zoom);
                }

                // 3. 繪製各節點 (Nodes)
                let mut clicked_port_id = None;
                let mut clicked_node_id = None;

                for node in &self.graph.nodes {
                    let bounds_w = get_node_bounds_world(node);
                    let top_left = world_to_screen(bounds_w.min.x, bounds_w.min.y);
                    let node_rect = Rect::from_min_size(top_left, bounds_w.size() * self.zoom);

                    let is_selected = self.selected_nodes.contains(&node.id);
                    let border_color = if is_selected {
                        Color32::from_rgb(255, 215, 60)
                    } else {
                        Color32::from_rgb(60, 60, 65)
                    };

                    // 節點本體背景
                    painter.rect_filled(node_rect, Rounding::same(6.0), Color32::from_rgb(38, 38, 42));
                    painter.rect_stroke(node_rect, Rounding::same(6.0), Stroke::new(1.5_f32, border_color));

                    // 標題列
                    let header_height = DEFAULT_NODE_HEADER_HEIGHT * self.zoom;
                    let header_rect = Rect::from_min_size(top_left, Vec2::new(node_rect.width(), header_height));
                    let header_color = match node.category() {
                        "Trigger" => Color32::from_rgb(160, 60, 60),
                        "Condition" => Color32::from_rgb(40, 120, 60),
                        "Action" => Color32::from_rgb(45, 90, 160),
                        "Flow" => Color32::from_rgb(120, 90, 40),
                        _ => Color32::from_rgb(70, 70, 75),
                    };
                    painter.rect_filled(header_rect, Rounding::same(6.0), header_color);

                    painter.text(
                        header_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        node.title(),
                        egui::FontId::proportional(12.0 * self.zoom),
                        Color32::WHITE,
                    );

                    // 繪製連接埠 (Ports)
                    for port in &node.ports {
                        let pw = get_port_center_world(node, port);
                        let port_center = world_to_screen(pw.x, pw.y);
                        let p_radius = PORT_CIRCLE_RADIUS * self.zoom;

                        let port_color = match port.port_type {
                            PortType::Execution => Color32::from_rgb(80, 200, 255),
                            PortType::Condition => Color32::from_rgb(120, 240, 120),
                            PortType::Data => Color32::from_rgb(255, 200, 80),
                        };

                        painter.circle_filled(port_center, p_radius, port_color);
                        painter.circle_stroke(port_center, p_radius, Stroke::new(1.0_f32, Color32::BLACK));

                        // 連接埠文字
                        let text_pos = if port.direction == PortDirection::Input {
                            port_center + Vec2::new(8.0 * self.zoom, 0.0)
                        } else {
                            port_center - Vec2::new(8.0 * self.zoom, 0.0)
                        };
                        let align = if port.direction == PortDirection::Input {
                            egui::Align2::LEFT_CENTER
                        } else {
                            egui::Align2::RIGHT_CENTER
                        };
                        painter.text(
                            text_pos,
                            align,
                            &port.display_name,
                            egui::FontId::proportional(10.5 * self.zoom),
                            Color32::from_rgb(200, 200, 200),
                        );

                        // 檢查滑鼠是否點擊在端口半徑內
                        if let Some(mouse) = hover_pos {
                            if mouse.distance(port_center) <= (p_radius * 1.5).max(8.0) {
                                if response.clicked_by(egui::PointerButton::Primary) {
                                    clicked_port_id = Some(port.id);
                                }
                            }
                        }
                    }

                    // 檢查節點本體點選與拖曳
                    if let Some(mouse) = hover_pos {
                        if node_rect.contains(mouse) {
                            if response.clicked_by(egui::PointerButton::Primary) {
                                clicked_node_id = Some(node.id);
                            }
                            if response.dragged_by(egui::PointerButton::Primary) && self.dragging_node_id.is_none() && self.connecting_port_id.is_none() {
                                self.dragging_node_id = Some(node.id);
                            }
                        }
                    }
                }

                // 處理節點拖曳移動
                if let Some(drag_id) = self.dragging_node_id {
                    if response.dragged_by(egui::PointerButton::Primary) {
                        let delta = response.drag_delta() / self.zoom;
                        if let Some(node) = self.graph.find_node_mut(drag_id) {
                            node.x += delta.x;
                            node.y += delta.y;
                            self.is_dirty = true;
                        }
                    } else {
                        self.dragging_node_id = None;
                    }
                }

                // 處理點選節點
                if let Some(nid) = clicked_node_id {
                    if !ctx.input(|i| i.modifiers.shift || i.modifiers.ctrl) {
                        self.selected_nodes.clear();
                    }
                    if self.selected_nodes.contains(&nid) {
                        self.selected_nodes.remove(&nid);
                    } else {
                        self.selected_nodes.insert(nid);
                    }
                }

                // 處理端口連線拖放
                if let Some(pid) = clicked_port_id {
                    if let Some(start_pid) = self.connecting_port_id {
                        // 完成連線
                        if let Err(err) = self.graph.connect(start_pid, pid) {
                            self.status_message = format!("連線失敗：{err}");
                        } else {
                            self.status_message = "連線建立成功。".to_string();
                            self.mark_dirty();
                        }
                        self.connecting_port_id = None;
                        self.connecting_start_pos = None;
                        self.connecting_current_pos = None;
                    } else {
                        // 開始拉線
                        self.connecting_port_id = Some(pid);
                        if let Some(p) = self.graph.find_port(pid) {
                            if let Some(n) = self.graph.find_node(p.node_id) {
                                let pw = get_port_center_world(n, p);
                                self.connecting_start_pos = Some(world_to_screen(pw.x, pw.y));
                            }
                        }
                    }
                }

                if self.connecting_port_id.is_some() {
                    self.connecting_current_pos = hover_pos;
                    if response.clicked_by(egui::PointerButton::Secondary) || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.connecting_port_id = None;
                        self.connecting_start_pos = None;
                        self.connecting_current_pos = None;
                    }
                }

                let mut locate = None;
                egui::ScrollArea::vertical().id_source("graph_diagnostics").max_height(120.0).show(ui, |ui| {
                    for diagnostic in &self.diagnostics {
                        if self.diagnostic_row(ui, diagnostic) { locate = diagnostic.node_id; }
                    }
                });
                if let Some(id) = locate { self.locate_node(id, canvas_rect); }

                // 底部狀態列
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(&self.status_message);
                    if !self.diagnostics.is_empty() {
                        let err_count = self.diagnostics.iter().filter(|d| d.severity == GraphDiagnosticSeverity::Error).count();
                        if err_count > 0 {
                            ui.colored_label(Color32::from_rgb(255, 100, 100), format!("({err_count} errors)"));
                        }
                    }
                });
            });

        if !open {
            self.request_close();
        } else {
            self.open = true;
        }
    }
}

fn vec2_scaled(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

fn draw_grid(painter: &egui::Painter, rect: Rect, pan: Vec2, zoom: f32) {
    let grid_step = 40.0 * zoom;
    if grid_step < 5.0 {
        return;
    }

    let start_x = rect.min.x + (pan.x % grid_step);
    let start_y = rect.min.y + (pan.y % grid_step);

    let line_color = Color32::from_rgb(34, 34, 38);
    let stroke = Stroke::new(1.0_f32, line_color);

    let mut x = start_x;
    while x <= rect.max.x {
        if x >= rect.min.x {
            painter.line_segment([Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)], stroke);
        }
        x += grid_step;
    }

    let mut y = start_y;
    while y <= rect.max.y {
        if y >= rect.min.y {
            painter.line_segment([Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)], stroke);
        }
        y += grid_step;
    }
}

fn draw_bezier_wire(painter: &egui::Painter, p1: Pos2, p2: Pos2, color: Color32, width: f32) {
    let dx = (p2.x - p1.x).abs() * 0.5;
    let cp1 = p1 + Vec2::new(dx.max(40.0), 0.0);
    let cp2 = p2 - Vec2::new(dx.max(40.0), 0.0);

    let stroke = Stroke::new(width, color);
    let bezier = egui::epaint::CubicBezierShape::from_points_stroke([p1, cp1, cp2, p2], false, Color32::TRANSPARENT, stroke);
    painter.add(bezier);
}

fn get_node_bounds_world(node: &GraphNode) -> Rect {
    let input_count = node.ports.iter().filter(|p| p.direction == PortDirection::Input).count();
    let output_count = node.ports.iter().filter(|p| p.direction == PortDirection::Output).count();
    let rows = 1.max(input_count.max(output_count));
    let height = DEFAULT_NODE_HEADER_HEIGHT + rows as f32 * PORT_ROW_HEIGHT + 12.0;
    Rect::from_min_size(Pos2::new(node.x, node.y), Vec2::new(DEFAULT_NODE_WIDTH, height))
}

fn get_port_center_world(node: &GraphNode, port: &crate::event_graph::model::GraphPort) -> Pos2 {
    let bounds = get_node_bounds_world(node);
    let dir_ports: Vec<_> = node.ports.iter().filter(|p| p.direction == port.direction).collect();
    let index = dir_ports.iter().position(|p| p.id == port.id).unwrap_or(0);

    let y = bounds.min.y + DEFAULT_NODE_HEADER_HEIGHT + 12.0 + index as f32 * PORT_ROW_HEIGHT + PORT_ROW_HEIGHT * 0.5;
    let x = if port.direction == PortDirection::Input {
        bounds.min.x
    } else {
        bounds.max.x
    };
    Pos2::new(x, y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use against_rome_scenario::{Action, Condition, Event, ScenarioDocument, Spawn};
    use crate::event_graph::model::GraphEdge;

    fn sample_doc_and_alias() -> (ScenarioDocument, Vec<String>) {
        let spawn_id = Uuid::new_v4();
        let doc = ScenarioDocument {
            version: 6,
            spawns: vec![Spawn {
                id: spawn_id,
                alias: "GER_INF01".into(),
                x: 8000.0,
                z: 8000.0,
                team: 0,
                count: 10,
                ..Default::default()
            }],
            events: vec![Event {
                name: "Test Event".into(),
                delay_seconds: 5,
                repeat: false,
                enabled: true,
                conditions: vec![Condition {
                    kind: 0,
                    target_id: spawn_id,
                    ..Default::default()
                }],
                actions: vec![
                    Action {
                        kind: 0,
                        text: "Initial Message".into(),
                        ..Default::default()
                    },
                    Action {
                        kind: 2,
                        alias: "GER_INF01".into(),
                        team: 0,
                        x: 8100.0,
                        z: 8100.0,
                        count: 4,
                        ..Default::default()
                    },
                ],
            }],
            ..Default::default()
        };
        let aliases = vec!["GER_INF01".into(), "ROM_LEG01".into()];
        (doc, aliases)
    }

    #[test]
    fn test_graph_ui_reload_from_scenario_and_roundtrip() {
        let (doc, aliases) = sample_doc_and_alias();
        let mut ui = EventGraphUi::default();

        ui.reload_from_scenario(&doc);
        assert!(!ui.is_dirty());
        assert_eq!(ui.graph.nodes.len(), 4); // 1 trigger + 1 condition + 2 actions
        assert_eq!(ui.graph.edges.len(), 3); // 1 cond->trigger + 2 exec links

        // Edit an action node: change message text
        let mut found = false;
        for node in &mut ui.graph.nodes {
            if let NodeKind::MessageAction { message_text } = &mut node.kind {
                *message_text = "Modified in Graph".into();
                found = true;
                break;
            }
        }
        assert!(found);
        ui.mark_dirty();
        assert!(ui.is_dirty());

        let compiled = ui.compile_to_scenario_events_checked(&doc, &aliases).unwrap();
        assert_eq!(compiled.len(), 1);
        assert_eq!(compiled[0].name, "Test Event");
        assert_eq!(compiled[0].actions.len(), 2);
        assert_eq!(compiled[0].actions[0].text, "Modified in Graph");
        assert_eq!(compiled[0].actions[1].alias, "GER_INF01");
    }

    #[test]
    fn test_graph_ui_invalid_links_fail_closed() {
        let (doc, aliases) = sample_doc_and_alias();
        let mut ui = EventGraphUi::default();
        ui.reload_from_scenario(&doc);

        // 1. Edge referencing nonexistent port
        let mut bad_ui = EventGraphUi::default();
        bad_ui.graph = ui.graph.clone();
        bad_ui.graph.edges.push(GraphEdge::new(
            bad_ui.graph.nodes[0].id,
            Uuid::new_v4(),
            bad_ui.graph.nodes[1].id,
            Uuid::new_v4(),
        ));
        assert!(bad_ui.validate_graph_integrity().is_err());
        assert!(bad_ui.compile_to_scenario_events_checked(&doc, &aliases).is_err());

        // 2. Self-loop edge
        let mut loop_ui = EventGraphUi::default();
        loop_ui.graph = ui.graph.clone();
        let node0 = &loop_ui.graph.nodes[0];
        let p_in = node0.find_port_by_name("ExecIn").unwrap().id;
        let p_out = node0.find_port_by_name("ExecOut").unwrap().id;
        loop_ui.graph.edges.push(GraphEdge::new(node0.id, p_out, node0.id, p_in));
        assert!(loop_ui.validate_graph_integrity().is_err());
        assert!(loop_ui.compile_to_scenario_events_checked(&doc, &aliases).is_err());

        // 3. Port type mismatch: connect Condition port to Execution port
        let mut mismatch_ui = EventGraphUi::default();
        mismatch_ui.graph = ui.graph.clone();
        let cond_node = mismatch_ui.graph.nodes.iter().find(|n| matches!(n.kind, NodeKind::ObjectExistsCondition { .. })).unwrap();
        let act_node = mismatch_ui.graph.nodes.iter().find(|n| matches!(n.kind, NodeKind::MessageAction { .. })).unwrap();
        let cond_out = cond_node.find_port_by_name("ConditionOut").unwrap().id;
        let act_in = act_node.find_port_by_name("ExecIn").unwrap().id;
        mismatch_ui.graph.edges.push(GraphEdge::new(cond_node.id, cond_out, act_node.id, act_in));
        assert!(mismatch_ui.validate_graph_integrity().is_err());

        // 4. Direction mismatch: Input to Input
        let mut dir_ui = EventGraphUi::default();
        dir_ui.graph = ui.graph.clone();
        let trig_node = &dir_ui.graph.nodes[0];
        let t_in = trig_node.find_port_by_name("ExecIn").unwrap().id;
        dir_ui.graph.edges.push(GraphEdge::new(trig_node.id, t_in, act_node.id, act_in));
        assert!(dir_ui.validate_graph_integrity().is_err());

        // 5. Multiple incoming connections to single-input ExecIn
        let mut multi_ui = EventGraphUi::default();
        multi_ui.graph = ui.graph.clone();
        let (act_id, act_exec_in) = {
            let act = multi_ui.graph.nodes.iter().find(|n| matches!(n.kind, NodeKind::MessageAction { .. })).unwrap();
            (act.id, act.find_port_by_name("ExecIn").unwrap().id)
        };
        let extra_trigger = GraphNode::new_event_trigger("Extra", 0, false, true);
        let extra_exec_out = extra_trigger.find_port_by_name("ExecOut").unwrap().id;
        let extra_id = extra_trigger.id;
        multi_ui.graph.add_node(extra_trigger);
        multi_ui.graph.edges.push(GraphEdge::new(extra_id, extra_exec_out, act_id, act_exec_in));
        assert!(multi_ui.validate_graph_integrity().is_err());
    }

    #[test]
    fn test_graph_ui_dirty_close_behavior() {
        let mut ui = EventGraphUi::default();
        ui.open = true;

        // Clean close
        ui.request_close();
        assert!(!ui.open);
        assert!(!ui.pending_close);

        // Dirty close requested
        ui.open = true;
        ui.mark_dirty();
        assert!(ui.is_dirty());
        ui.request_close();
        assert!(ui.open); // Remains open because modal is shown
        assert!(ui.pending_close);

        // Cancel close
        ui.cancel_close();
        assert!(ui.open);
        assert!(!ui.pending_close);
        assert!(ui.is_dirty());

        // Discard and close
        ui.request_close();
        assert!(ui.pending_close);
        ui.discard_and_close();
        assert!(!ui.open);
        assert!(!ui.pending_close);
        assert!(!ui.is_dirty());

        // Headless egui test for pending_close dialog
        let ctx = egui::Context::default();
        ui.open = true;
        ui.mark_dirty();
        ui.request_close();
        ctx.begin_frame(Default::default());
        ui.show(&ctx);
        let out = ctx.end_frame();
        assert!(!out.shapes.is_empty());
    }
}

#[cfg(test)]
mod small_ux_tests {
    use super::*;
    #[test]
    fn locate_selects_centers_and_missing_is_disabled() {
        let mut ui = EventGraphUi::default();
        let mut node = GraphNode::new_message_action("test");
        node.x = 1300.0;
        node.y = -500.0;
        let id = node.id;
        let center = get_node_bounds_world(&node).center();
        ui.graph.nodes.push(node);
        ui.zoom = 1.75;
        ui.selected_nodes.insert(Uuid::nil());
        let canvas = Rect::from_min_size(Pos2::new(40.0, 90.0), Vec2::new(800.0, 500.0));
        assert!(ui.locate_node(id, canvas));
        assert_eq!(ui.selected_nodes, HashSet::from([id]));
        assert!((canvas.min + ui.pan + center.to_vec2() * ui.zoom).distance(canvas.center()) < 0.001);
        let pan = ui.pan;
        ui.graph.nodes.clear();
        assert!(!ui.locate_node(id, canvas));
        assert_eq!(ui.pan, pan);
        assert_eq!(ui.selected_nodes, HashSet::from([id]));
        let ctx = egui::Context::default();
        let diagnostic = GraphDiagnostic::new(GraphDiagnosticSeverity::Error, "missing", "test").with_node(id);
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |egui_ui| {
                assert!(!ui.locate_button(egui_ui, id).enabled());
                assert!(!ui.diagnostic_row(egui_ui, &diagnostic));
            });
        });
    }
}
