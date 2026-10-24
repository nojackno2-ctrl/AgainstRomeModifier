use crate::{scenario_ui::ScenarioUi, spawn_placement_ui::team_color};
use against_rome_scenario::{Event, WorldPosition};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};

#[derive(Clone)]
pub(crate) enum PickTarget {
    Event { event: usize, action: usize, destination: bool, snapshot: Event },
    Wave { destination: bool },
}

pub(crate) struct ScenarioCanvas {
    pub show: bool,
    pub pick: Option<PickTarget>,
    holding: bool,
}

impl Default for ScenarioCanvas {
    fn default() -> Self {
        Self { show: true, pick: None, holding: false }
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
            self.canvas.holding = ctx.input(|i| i.pointer.primary_down() || i.pointer.secondary_down());
            return Some(if en { "Map pick cancelled." } else { "已取消地圖取點。" }.into());
        }
        None
    }

    pub(crate) fn pick_prompt(&self, en: bool) -> Option<&'static str> {
        self.canvas.pick.as_ref().map(|target| {
            let destination = match target {
                PickTarget::Event { destination, .. } | PickTarget::Wave { destination } => *destination,
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
        for e in self.document().events.iter().filter(|e| e.enabled) {
            for a in e.actions.iter().filter(|a| a.kind == 2) {
                paint_spawn(painter, rect, a.x, a.z, a.team, &e.name, a.move_destination.as_ref(), false);
            }
        }
        if self.open && self.wave_generator.initialized {
            let g = &self.wave_generator;
            paint_spawn(painter, rect, g.x, g.z, g.team,
                if en { "Pending wave" } else { "待產生波次" }, Some(&g.destination), true);
        }
    }
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
