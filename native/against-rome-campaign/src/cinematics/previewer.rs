use super::cutscene::{CutsceneFxKeyframe, CutsceneSequence, SubtitleKeyframe};
use super::spline::{CameraPose, CameraTrackSplinePlanner};
use std::collections::HashSet;

/// 運鏡試飛預覽器播放狀態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CameraPreviewState {
    /// 停止狀態。
    #[default]
    Stopped,
    /// 正在播放試飛。
    Playing,
    /// 暫停播放。
    Paused,
}

/// 3D 視圖即時運鏡預覽器 (Camera Flight Previewer)
#[derive(Debug, Clone, Default)]
pub struct CameraFlightPreviewer {
    spline_planner: CameraTrackSplinePlanner,
    active_sequence: Option<CutsceneSequence>,
    saved_original_camera_pose: Option<CameraPose>,
    current_active_subtitle: Option<SubtitleKeyframe>,
    triggered_fx_indices: HashSet<usize>,
    triggered_fx_events: Vec<CutsceneFxKeyframe>,
    pub state: CameraPreviewState,
    pub current_time: f32,
    pub playback_speed: f32,
    pub auto_restore_camera_on_stop: bool,
    pub last_camera_pose: CameraPose,
    pub playback_finished: bool,
}

impl CameraFlightPreviewer {
    pub fn new() -> Self {
        Self {
            spline_planner: CameraTrackSplinePlanner::default(),
            active_sequence: None,
            saved_original_camera_pose: None,
            current_active_subtitle: None,
            triggered_fx_indices: HashSet::new(),
            triggered_fx_events: vec![],
            state: CameraPreviewState::Stopped,
            current_time: 0.0,
            playback_speed: 1.0,
            auto_restore_camera_on_stop: true,
            last_camera_pose: CameraPose::default(),
            playback_finished: false,
        }
    }

    pub fn total_duration(&self) -> f32 {
        self.spline_planner.total_duration()
    }

    pub fn active_sequence(&self) -> Option<&CutsceneSequence> {
        self.active_sequence.as_ref()
    }

    pub fn current_active_subtitle(&self) -> Option<&SubtitleKeyframe> {
        self.current_active_subtitle.as_ref()
    }

    pub fn triggered_fx_events(&self) -> &[CutsceneFxKeyframe] {
        &self.triggered_fx_events
    }

    pub fn saved_original_camera_pose(&self) -> Option<CameraPose> {
        self.saved_original_camera_pose
    }

    pub fn play(&mut self, sequence: CutsceneSequence, current_camera_pose: Option<CameraPose>) {
        self.spline_planner = CameraTrackSplinePlanner::new(&sequence.camera_waypoints);
        self.active_sequence = Some(sequence);
        self.saved_original_camera_pose = current_camera_pose;
        self.triggered_fx_indices.clear();
        self.triggered_fx_events.clear();
        self.current_time = 0.0;
        self.state = CameraPreviewState::Playing;
        self.playback_finished = false;

        self.evaluate_frame(0.0);
    }

    pub fn pause(&mut self) {
        if self.state == CameraPreviewState::Playing {
            self.state = CameraPreviewState::Paused;
        }
    }

    pub fn resume(&mut self) {
        if self.state == CameraPreviewState::Paused {
            self.state = CameraPreviewState::Playing;
        }
    }

    pub fn stop(&mut self, restore_camera: bool) {
        self.state = CameraPreviewState::Stopped;
        self.current_time = 0.0;
        self.current_active_subtitle = None;

        if restore_camera && self.auto_restore_camera_on_stop {
            if let Some(original) = self.saved_original_camera_pose {
                self.last_camera_pose = original;
            }
        }
    }

    pub fn scrub_to(&mut self, time_seconds: f32) {
        if self.active_sequence.is_none() {
            return;
        }
        let total = self.total_duration();
        let clamped = time_seconds.clamp(0.0, total);
        self.current_time = clamped;
        self.evaluate_frame(clamped);
    }

    pub fn update(&mut self, delta_time_seconds: f32) {
        if self.state != CameraPreviewState::Playing || self.active_sequence.is_none() {
            return;
        }
        if delta_time_seconds <= 0.0 {
            return;
        }

        let advance = delta_time_seconds * 0.01f32.max(self.playback_speed);
        self.current_time += advance;

        let total = self.total_duration();
        if self.current_time >= total {
            self.current_time = total;
            self.evaluate_frame(self.current_time);
            self.state = CameraPreviewState::Stopped;
            self.playback_finished = true;

            if let Some(ref seq) = self.active_sequence {
                if seq.restore_camera_on_complete && self.auto_restore_camera_on_stop {
                    if let Some(original) = self.saved_original_camera_pose {
                        self.last_camera_pose = original;
                    }
                }
            }
            return;
        }

        self.evaluate_frame(self.current_time);
    }

    fn evaluate_frame(&mut self, time: f32) {
        // 1. 樣條姿態評估
        self.last_camera_pose = self.spline_planner.evaluate(time);

        let Some(ref seq) = self.active_sequence else { return };

        // 2. 字幕軌道比對
        self.current_active_subtitle = seq
            .subtitles
            .iter()
            .find(|s| time >= s.start_time_seconds && time <= s.end_time_seconds())
            .cloned();

        // 3. 特效與事件軌道觸發
        for (i, fx) in seq.fx_events.iter().enumerate() {
            if time >= fx.time_seconds && self.triggered_fx_indices.insert(i) {
                self.triggered_fx_events.push(fx.clone());
            }
        }
    }
}
