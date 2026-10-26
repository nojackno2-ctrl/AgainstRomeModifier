use super::*;
use crate::undo_journal::{SubsystemId, SubsystemStep};

#[derive(Clone, PartialEq)]
pub(super) enum ControlState {
    Water { preview: f32, level: f32, dirty: bool, other: String },
    Camera(crate::campaign_ui::CameraHistoryState),
    Briefing { title: String, subtitle: String, body: String, teams: [String; crate::ambience_ui::BRIEFING_TEAM_COUNT], dirty: bool },
}
impl ControlState {
    fn id(&self) -> SubsystemId {
        match self { Self::Water { .. } => SubsystemId::WaterControl, Self::Camera(_) => SubsystemId::StartCamera, Self::Briefing { .. } => SubsystemId::BriefingText }
    }
    fn changed(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Water { preview: a, level: b, .. }, Self::Water { preview: c, level: d, .. }) => a != c || b != d,
            (
                Self::Briefing { title: a, subtitle: b, body: c, teams: d, .. },
                Self::Briefing { title: e, subtitle: f, body: g, teams: h, .. },
            ) => a != e || b != f || c != g || d != h,
            _ => self != other,
        }
    }
}
#[derive(Default)]
struct History {
    token: u64,
    undo: Vec<(u64, u64, ControlState, ControlState)>,
    redo: Vec<(u64, u64, ControlState, ControlState)>,
    dragging: bool,
}
#[derive(Default)]
pub(super) struct ControlHistory { water: History, camera: History, briefing: History }
impl ControlHistory {
    pub(super) fn end_gestures(&mut self) { self.water.dragging = false; self.camera.dragging = false; self.briefing.dragging = false; }
    fn get(&self, id: SubsystemId) -> &History {
        match id { SubsystemId::WaterControl => &self.water, SubsystemId::StartCamera => &self.camera, _ => &self.briefing }
    }
    fn get_mut(&mut self, id: SubsystemId) -> &mut History {
        match id { SubsystemId::WaterControl => &mut self.water, SubsystemId::StartCamera => &mut self.camera, _ => &mut self.briefing }
    }
    pub(super) fn token(&self, id: SubsystemId) -> u64 { self.get(id).token }
}
impl EditorApp {
    fn water_other_signature(&self) -> String {
        let mut profile = self.ambience.profile.clone();
        profile.boden.waterlevel = 0.0;
        format!("{profile:?}")
    }
    pub(super) fn capture_water_control(&self) -> ControlState {
        ControlState::Water { preview: self.water_surface, level: self.ambience.profile.boden.waterlevel,
            dirty: self.ambience.dirty, other: self.water_other_signature() }
    }
    pub(super) fn capture_camera_control(&self) -> ControlState {
        ControlState::Camera(self.campaign.camera_history_state())
    }
    pub(super) fn capture_briefing_control(&self) -> ControlState {
        ControlState::Briefing {
            title: self.ambience.briefing_title.clone(),
            subtitle: self.ambience.briefing_subtitle.clone(),
            body: self.ambience.briefing_body.clone(),
            teams: self.ambience.briefing_team_names.clone(),
            dirty: self.ambience.dirty,
        }
    }
    fn restore_control(&mut self, state: ControlState) {
        match state {
            ControlState::Water { preview, level, dirty, other } => {
                self.water_surface = preview;
                self.ambience.profile.boden.waterlevel = level;
                self.ambience.dirty = dirty || self.water_other_signature() != other;
            }
            ControlState::Camera(camera) => self.campaign.restore_camera_history(camera),
            ControlState::Briefing { title, subtitle, body, teams, dirty } => {
                self.ambience.briefing_title = title;
                self.ambience.briefing_subtitle = subtitle;
                self.ambience.briefing_body = body;
                self.ambience.briefing_team_names = teams;
                self.ambience.dirty = dirty;
            }
        }
    }
    /// UI 共用的交易界線；記錄水位、鏡頭與任務簡報欄位變更。
    pub(super) fn finish_control_edit(&mut self, ctx: &egui::Context, before: ControlState) {
        let id = before.id();
        let after = match id {
            SubsystemId::WaterControl => self.capture_water_control(),
            SubsystemId::StartCamera => self.capture_camera_control(),
            _ => self.capture_briefing_control(),
        };
        let down = ctx.input(|i| i.pointer.primary_down());
        if !before.changed(&after) {
            if !down { self.control_history.get_mut(id).dragging = false; }
            return;
        }
        // 預覽與筆刷參數在空白地圖或原版唯讀地圖仍可使用；持久化水位與鏡頭維持唯讀。
        let preview_only = matches!((&before, &after),
            (ControlState::Water { level: a, .. }, ControlState::Water { level: b, .. }) if a == b);
        if self.current_map_is_original()
            && (id == SubsystemId::BriefingText || !preview_only)
        {
            self.restore_control(before);
            return;
        }
        self.collect_pilot_receipts();
        let history = self.control_history.get_mut(id);
        let merge = (history.dragging || id == SubsystemId::BriefingText)
            && self.undo_journal.ends_with_step(id, history.token);
        history.dragging = down;
        history.redo.clear();
        if merge {
            if let Some(edit) = history.undo.last_mut() { edit.3 = after; }
        } else {
            let token = against_rome_textures::history::next_history_token();
            let previous = history.token;
            history.undo.push((previous, token, before, after));
            history.token = token;
            self.undo_journal.record_compound(vec![SubsystemStep { subsystem: id, before_token: previous, after_token: token }]);
        }
    }
    pub(super) fn navigate_control(&mut self, id: SubsystemId, forward: bool) -> bool {
        if self.current_map_is_original()
            && matches!(id, SubsystemId::StartCamera | SubsystemId::BriefingText)
        {
            return false;
        }
        let history = self.control_history.get_mut(id);
        history.dragging = false;
        let source = if forward { &mut history.redo } else { &mut history.undo };
        let Some(edit) = source.pop() else { return false; };
        history.token = if forward { edit.1 } else { edit.0 };
        let state = if forward { edit.3.clone() } else { edit.2.clone() };
        if forward { history.undo.push(edit); } else { history.redo.push(edit); }
        self.restore_control(state);
        true
    }
}
