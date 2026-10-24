use crate::{scenario_ui::ScenarioUi, spawn_placement_ui::team_color};
use against_rome_scenario::{Event, WorldPosition};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};

#[derive(Clone)]
pub(crate) enum PickTarget {
    Condition { event: usize, condition: usize, snapshot: Event },
    Event { event: usize, action: usize, destination: bool, snapshot: Event },
    Wave { destination: bool },
}

pub(crate) struct ScenarioCanvas {
    pub show: bool,
    pub pick: Option<PickTarget>,
    holding: bool,
    drag: Option<(WorldPosition, WorldPosition)>,
}

impl Default for ScenarioCanvas {
    fn default() -> Self {
        Self { show: true, pick: None, holding: false, drag: None }
    }
}

/// 與既有 2D 畫布共用 16384 世界範圍及 map_rect 的縮放、平移。
pub(crate) fn world_from_screen(rect: Rect, p: Pos2) -> (f32, f32) {
    ((p.x - rect.min.x) / rect.width() * 16384.,
     (p.y - rect.min.y) / rect.height() * 16384.)
}

pub(crate) fn screen_from_world(rect: Rect, x: f32, z: f32) -> Pos2 {
    Pos2::new(rect.min.x + x / 16384. * rect.width(), rect.min.y + z / 16384. * rect.height())
}

impl ScenarioUi {
    pub(crate) fn cancel_canvas_pick(&mut self, ctx: &egui::Context, en: bool) -> Option<String> {
        if self.canvas.pick.is_some() && ctx.input(|i|
            i.key_pressed(egui::Key::Escape) || i.pointer.secondary_pressed()) {
            self.canvas.pick = None;
            self.canvas.drag = None;
            self.canvas.holding = ctx.input(|i| i.pointer.primary_down() || i.pointer.secondary_down());
            return Some(if en { "Map pick cancelled." } else { "已取消地圖取點。" }.into());
        }
        None
    }

    pub(crate) fn pick_prompt(&self, en: bool) -> Option<&'static str> {
        self.canvas.pick.as_ref().map(|target| {
            if matches!(target, PickTarget::Condition { .. }) {
                return if en { "Draw rectangle: left-drag the 2D canvas; Esc/right-click cancels." }
                else { "框選矩形：在 2D 畫布拖曳左鍵；Esc／右鍵取消。" };
            }
            let destination = match target {
                PickTarget::Event { destination, .. } | PickTarget::Wave { destination } => *destination,
                PickTarget::Condition { .. } => false,
            };
            match (en, destination) {
                (true, false) => "Pick spawn: left-click the 2D canvas; Esc/right-click cancels.",
                (true, true) => "Pick destination: left-click the 2D canvas; Esc/right-click cancels.",
                (false, false) => "選取生成點：左鍵點擊 2D 畫布；Esc／右鍵取消。",
                (false, true) => "選取目的地：左鍵點擊 2D 畫布；Esc／右鍵取消。",
            }
        })
    }

    /// 取點手勢直到放開左鍵都獨占畫布，避免後續影格塗畫或框選。
    pub(crate) fn canvas_pick_input(&mut self, ctx: &egui::Context, response: &egui::Response,
        rect: Rect, en: bool) -> (bool, Option<(String, bool)>) {
        let owns = self.canvas.pick.is_some() || self.canvas.holding;
        if self.canvas.holding {
            self.canvas.holding = ctx.input(|i| i.pointer.primary_down() || i.pointer.secondary_down());
        }
        if matches!(self.canvas.pick, Some(PickTarget::Condition { .. })) {
            let point = ctx.input(|i| i.pointer.interact_pos()).map(|pos| {
                let (x, z) = world_from_screen(rect, pos);
                WorldPosition { x: x.round().clamp(0., 16383.) as i32,
                    z: z.round().clamp(0., 16383.) as i32 }
            });
            if response.hovered() && ctx.input(|i| i.pointer.primary_pressed()) {
                self.canvas.drag = point.clone().map(|p| (p.clone(), p));
                self.canvas.holding = true;
            }
            if let Some((_, end)) = &mut self.canvas.drag {
                if let Some(point) = point { *end = point; }
            }
            if ctx.input(|i| i.pointer.primary_released()) {
                if let Some((start, end)) = self.canvas.drag.take() {
                    let target = self.canvas.pick.take();
                    let status = if start.x == end.x || start.z == end.z {
                        (if en { "Zero-size rectangle cancelled." } else { "已取消零大小矩形。" }.into(), false)
                    } else if let Some(PickTarget::Condition { event, condition, snapshot }) = target {
                        let result = self.document().events.get(event).filter(|e| **e == snapshot).cloned()
                            .and_then(|mut item| {
                                let c = item.conditions.get_mut(condition).filter(|c| c.kind == 2)?;
                                c.min_x = start.x.min(end.x); c.max_x = start.x.max(end.x);
                                c.min_z = start.z.min(end.z); c.max_z = start.z.max(end.z);
                                Some(item)
                            }).ok_or(()).and_then(|item| self.replace_event(event, item).map_err(|_| ()));
                        match result {
                            Ok(()) => (if en { "Rectangle updated; Save to write it." } else { "已更新矩形；請存檔以寫入變更。" }.into(), false),
                            Err(()) => (if en { "Rectangle target changed; draw again." } else { "矩形目標已變更；請重新框選。" }.into(), true),
                        }
                    } else { return (true, None); };
                    return (true, Some(status));
                }
            }
            return (true, None);
        }
        if self.canvas.pick.is_some() && response.hovered() && ctx.input(|i| i.pointer.primary_pressed()) {
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                let (x, z) = world_from_screen(rect, pos);
                let point = WorldPosition { x: x.round().clamp(0., 16383.) as i32,
                    z: z.round().clamp(0., 16383.) as i32 };
                self.canvas.holding = true;
                let result = self.apply_canvas_pick(point.clone());
                let status = match result {
                    Ok(wave) => (if en {
                        format!("Picked X/Z: {}, {}. {}", point.x, point.z,
                            if wave { "Wave form updated; generate waves to apply." } else { "Event updated; Save to write it." })
                    } else {
                        format!("已選取 X/Z：{}, {}。{}", point.x, point.z,
                            if wave { "已更新波次表單；產生波次後套用。" } else { "已更新事件；請存檔以寫入變更。" })
                    }, false),
                    Err(()) => (if en { "Pick target changed; pick again." } else { "取點目標已變更；請重新取點。" }.into(), true),
                };
                return (true, Some(status));
            }
        }
        (owns, None)
    }

    fn apply_canvas_pick(&mut self, point: WorldPosition) -> Result<bool, ()> {
        match self.canvas.pick.take().ok_or(())? {
            PickTarget::Condition { .. } => Err(()),
            PickTarget::Wave { destination } => {
                let g = &mut self.wave_generator;
                if destination { g.destination = point; }
                else { g.x = point.x as f32; g.z = point.z as f32; }
                Ok(true)
            }
            PickTarget::Event { event, action, destination, snapshot } => {
                let mut item = self.document().events.get(event).filter(|e| **e == snapshot).cloned().ok_or(())?;
                let a = item.actions.get_mut(action).filter(|a| a.kind == 2).ok_or(())?;
                if destination { a.move_destination = Some(point); }
                else { a.x = point.x as f32; a.z = point.z as f32; }
                self.replace_event(event, item).map_err(|_| ())?;
                Ok(false)
            }
        }
    }

    pub(crate) fn paint_canvas(&self, painter: &egui::Painter, rect: Rect, en: bool) {
        if !self.open || !self.canvas.show { return; }
        for (index, e) in self.document().events.iter().enumerate().filter(|(_, e)| e.enabled) {
            for c in e.conditions.iter().filter(|c| c.kind == 2) {
                paint_rectangle(painter, rect, (c.min_x, c.min_z), (c.max_x, c.max_z),
                    &e.name, index == self.selected);
            }
            for a in e.actions.iter().filter(|a| a.kind == 2) {
                paint_spawn(painter, rect, a.x, a.z, a.team, &e.name, a.move_destination.as_ref(), false);
            }
        }
        if let Some((start, end)) = &self.canvas.drag {
            paint_rectangle(painter, rect, (start.x, start.z), (end.x, end.z),
                if en { "Draw rectangle" } else { "框選矩形" }, true);
        }
        if self.open && self.wave_generator.initialized {
            let g = &self.wave_generator;
            paint_spawn(painter, rect, g.x, g.z, g.team,
                if en { "Pending wave" } else { "待產生波次" }, Some(&g.destination), true);
        }
    }
}

fn paint_rectangle(painter: &egui::Painter, rect: Rect, start: (i32, i32), end: (i32, i32),
    label: &str, selected: bool) {
    let bounds = Rect::from_two_pos(screen_from_world(rect, start.0 as f32, start.1 as f32),
        screen_from_world(rect, end.0 as f32, end.1 as f32));
    let color = if selected { Color32::YELLOW } else { Color32::LIGHT_BLUE };
    painter.rect_filled(bounds, 0., color.linear_multiply(if selected { 0.22 } else { 0.12 }));
    painter.rect_stroke(bounds, 0., Stroke::new(if selected { 3_f32 } else { 1.5_f32 }, color));
    painter.text(bounds.min + egui::vec2(4., 4.), egui::Align2::LEFT_TOP,
        label, egui::FontId::proportional(13.), color);
}

fn paint_spawn(painter: &egui::Painter, rect: Rect, x: f32, z: f32, team: i32,
    label: &str, destination: Option<&WorldPosition>, pending: bool) {
    if !x.is_finite() || !z.is_finite() { return; }
    let p = screen_from_world(rect, x, z);
    let color = team_color(team);
    if let Some(d) = destination {
        let end = screen_from_world(rect, d.x as f32, d.z as f32);
        painter.arrow(p, end - p, Stroke::new(if pending { 1_f32 } else { 2_f32 }, color));
        painter.circle_stroke(end, 4., Stroke::new(1_f32, color));
    }
    if pending {
        painter.rect_stroke(Rect::from_center_size(p, egui::vec2(16., 16.)), 0., Stroke::new(2_f32, Color32::WHITE));
        painter.circle_stroke(p, 5., Stroke::new(2_f32, color));
    } else {
        painter.circle_filled(p, 5., color);
    }
    painter.text(p + egui::vec2(10., -10.), egui::Align2::LEFT_BOTTOM,
        label, egui::FontId::proportional(13.), color);
}
