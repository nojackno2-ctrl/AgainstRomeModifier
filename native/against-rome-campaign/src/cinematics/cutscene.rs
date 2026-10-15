use super::spline::CameraWaypoint;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 過場動畫演員指令類型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CutsceneUnitOrderKind {
    /// 下令前往座標點 (對應原生 s_conMoveTo)。
    MoveTo,
    /// 下令攻擊指定位置/目標 (對應原生 s_attacksObj / s_conCombat)。
    AttackTarget,
    /// 原地下令警戒/防守。
    HoldGround,
    /// 切換隊形 (對應原生 s_getFormDef / 陣形幾何)。
    ChangeFormation,
    /// 播放特定動作動畫。
    PlayAnimation,
}

impl Default for CutsceneUnitOrderKind {
    fn default() -> Self {
        Self::MoveTo
    }
}

/// 過場特效/事件標記類型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CutsceneFxEventKind {
    /// 寬銀幕黑邊開啟 (Cinema Letterbox)。
    LetterboxOn,
    /// 寬銀幕黑邊關閉。
    LetterboxOff,
    /// 黑幕淡入 (Fade In)。
    FadeIn,
    /// 黑幕淡出 (Fade Out)。
    FadeOut,
    /// 切換背景音樂或播放環境音。
    PlayAudio,
    /// 觸發原生戰役腳本事件 (ScenarioEvent Trigger)。
    TriggerScenarioEvent,
}

impl Default for CutsceneFxEventKind {
    fn default() -> Self {
        Self::LetterboxOn
    }
}

/// 對白與字幕關鍵影格 (Subtitle Keyframe)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtitleKeyframe {
    pub start_time_seconds: f32,
    pub duration_seconds: f32,
    pub speaker: String,
    pub text: String,
    pub voice_sample_alias: Option<String>,
}

impl Default for SubtitleKeyframe {
    fn default() -> Self {
        Self {
            start_time_seconds: 0.0,
            duration_seconds: 4.0,
            speaker: String::new(),
            text: String::new(),
            voice_sample_alias: None,
        }
    }
}

impl SubtitleKeyframe {
    pub fn new(
        start_time: f32,
        duration: f32,
        speaker: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            start_time_seconds: start_time.max(0.0),
            duration_seconds: duration.max(0.5),
            speaker: speaker.into(),
            text: text.into(),
            voice_sample_alias: None,
        }
    }

    pub fn end_time_seconds(&self) -> f32 {
        self.start_time_seconds + self.duration_seconds
    }
}

/// 部隊演員指令關鍵影格 (Unit Order Keyframe)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnitOrderKeyframe {
    pub time_seconds: f32,
    pub unit_id: Uuid,
    pub unit_alias: String,
    pub order: CutsceneUnitOrderKind,
    pub target_x: f32,
    pub target_z: f32,
    pub formation_index: i32,
}

impl Default for UnitOrderKeyframe {
    fn default() -> Self {
        Self {
            time_seconds: 0.0,
            unit_id: Uuid::nil(),
            unit_alias: String::new(),
            order: CutsceneUnitOrderKind::MoveTo,
            target_x: 8192.0,
            target_z: 8192.0,
            formation_index: 0,
        }
    }
}

impl UnitOrderKeyframe {
    pub fn new(
        time: f32,
        unit_alias: impl Into<String>,
        order: CutsceneUnitOrderKind,
        target_x: f32,
        target_z: f32,
    ) -> Self {
        Self {
            time_seconds: time.max(0.0),
            unit_id: Uuid::nil(),
            unit_alias: unit_alias.into(),
            order,
            target_x,
            target_z,
            formation_index: 0,
        }
    }
}

/// 畫面導演特效與環境事件關鍵影格 (Cutscene FX Keyframe)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CutsceneFxKeyframe {
    pub time_seconds: f32,
    pub kind: CutsceneFxEventKind,
    pub parameter: String,
}

impl Default for CutsceneFxKeyframe {
    fn default() -> Self {
        Self {
            time_seconds: 0.0,
            kind: CutsceneFxEventKind::LetterboxOn,
            parameter: String::new(),
        }
    }
}

impl CutsceneFxKeyframe {
    pub fn new(time: f32, kind: CutsceneFxEventKind, parameter: impl Into<String>) -> Self {
        Self {
            time_seconds: time.max(0.0),
            kind,
            parameter: parameter.into(),
        }
    }
}

/// 完整歷史戰役過場動畫序列 (Cutscene Sequence)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CutsceneSequence {
    pub id: String,
    pub name: String,
    pub camera_waypoints: Vec<CameraWaypoint>,
    pub subtitles: Vec<SubtitleKeyframe>,
    pub unit_orders: Vec<UnitOrderKeyframe>,
    pub fx_events: Vec<CutsceneFxKeyframe>,
    pub auto_letterbox: bool,
    pub disable_player_control: bool,
    pub restore_camera_on_complete: bool,
    pub on_complete_trigger_event: Option<String>,
}

impl Default for CutsceneSequence {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().as_simple().to_string(),
            name: "未命名過場序列".to_string(),
            camera_waypoints: vec![],
            subtitles: vec![],
            unit_orders: vec![],
            fx_events: vec![],
            auto_letterbox: true,
            disable_player_control: true,
            restore_camera_on_complete: true,
            on_complete_trigger_event: None,
        }
    }
}

impl CutsceneSequence {
    pub fn calculate_total_duration(&self) -> f32 {
        let mut camera_duration = 0.0f32;
        for i in 1..self.camera_waypoints.len() {
            camera_duration += 0.001f32.max(self.camera_waypoints[i].duration_from_previous);
        }

        let max_subtitle = self
            .subtitles
            .iter()
            .map(|s| s.end_time_seconds())
            .fold(0.0f32, f32::max);
        let max_unit_order = self
            .unit_orders
            .iter()
            .map(|u| u.time_seconds)
            .fold(0.0f32, f32::max);
        let max_fx = self
            .fx_events
            .iter()
            .map(|e| e.time_seconds)
            .fold(0.0f32, f32::max);

        camera_duration
            .max(max_subtitle)
            .max(max_unit_order)
            .max(max_fx)
    }
}
