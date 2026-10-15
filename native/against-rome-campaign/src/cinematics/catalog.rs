use super::cutscene::CutsceneSequence;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

/// 歷史戰役過場動畫目錄模組 (Cutscene Sequence Catalog)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CutsceneSequenceCatalog {
    sequences: Vec<CutsceneSequence>,
    #[serde(skip)]
    baseline: Vec<CutsceneSequence>,
}

impl CutsceneSequenceCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_sequences(sequences: Vec<CutsceneSequence>) -> Self {
        let baseline = sequences.clone();
        Self {
            sequences,
            baseline,
        }
    }

    pub fn module_id(&self) -> &'static str {
        "cutscenes"
    }

    pub fn count(&self) -> usize {
        self.sequences.len()
    }

    pub fn sequences(&self) -> &[CutsceneSequence] {
        &self.sequences
    }

    pub fn baseline(&self) -> &[CutsceneSequence] {
        &self.baseline
    }

    pub fn is_dirty(&self) -> bool {
        if self.sequences.len() != self.baseline.len() {
            return true;
        }
        for (a, b) in self.sequences.iter().zip(&self.baseline) {
            if a.id != b.id
                || a.name != b.name
                || a.auto_letterbox != b.auto_letterbox
                || a.disable_player_control != b.disable_player_control
                || a.restore_camera_on_complete != b.restore_camera_on_complete
                || a.on_complete_trigger_event != b.on_complete_trigger_event
                || a.camera_waypoints.len() != b.camera_waypoints.len()
                || a.subtitles.len() != b.subtitles.len()
                || a.unit_orders.len() != b.unit_orders.len()
                || a.fx_events.len() != b.fx_events.len()
            {
                return true;
            }
        }
        false
    }

    pub fn load(&mut self, snapshot: Vec<CutsceneSequence>) {
        self.sequences = snapshot;
        self.accept_changes();
    }

    pub fn capture(&self) -> Vec<CutsceneSequence> {
        self.sequences.clone()
    }

    pub fn accept_changes(&mut self) {
        self.baseline = self.sequences.clone();
    }

    pub fn accept_baseline(&mut self, snapshot: Vec<CutsceneSequence>) {
        self.baseline = snapshot;
    }

    pub fn reset(&mut self) {
        self.sequences = self.baseline.clone();
    }

    pub fn add(&mut self, item: CutsceneSequence) -> Result<usize, String> {
        if self.sequences.len() >= 64 {
            return Err("單張地圖過場動畫上限為 64 個。".into());
        }
        self.sequences.push(item);
        Ok(self.sequences.len() - 1)
    }

    pub fn replace(&mut self, index: usize, item: CutsceneSequence) -> Result<(), String> {
        if index >= self.sequences.len() {
            return Err(format!("Index out of range: {index}"));
        }
        self.sequences[index] = item;
        Ok(())
    }

    pub fn remove_at(&mut self, index: usize) -> Result<(), String> {
        if index >= self.sequences.len() {
            return Err(format!("Index out of range: {index}"));
        }
        self.sequences.remove(index);
        Ok(())
    }

    pub fn duplicate(&mut self, index: usize, suffix: &str) -> Result<usize, String> {
        if index >= self.sequences.len() {
            return Err(format!("Index out of range: {index}"));
        }
        if self.sequences.len() >= 64 {
            return Err("單張地圖過場動畫上限為 64 個。".into());
        }

        let mut copy = self.sequences[index].clone();
        copy.id = Uuid::new_v4().as_simple().to_string();
        copy.name = format!("{}{}", copy.name, suffix);

        let new_index = index + 1;
        self.sequences.insert(new_index, copy);
        Ok(new_index)
    }

    pub fn get_by_id(&self, id: &str) -> Option<&CutsceneSequence> {
        self.sequences
            .iter()
            .find(|s| s.id.eq_ignore_ascii_case(id))
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut ids = HashSet::new();

        for seq in &self.sequences {
            if seq.name.trim().is_empty() || seq.name.chars().count() > 100 {
                return Err(format!(
                    "過場序列「{}」名稱無效，必須介於 1–100 個字元。",
                    seq.name
                ));
            }
            if !ids.insert(seq.id.to_lowercase()) {
                return Err(format!("發現重複的過場序列識別碼：{}。", seq.id));
            }

            // 航點檢驗
            if seq.camera_waypoints.len() > 0 && seq.camera_waypoints.len() < 2 {
                return Err(format!(
                    "過場「{}」的相機航跡至少需要 2 個路徑點才能構成平滑飛行。",
                    seq.name
                ));
            }

            for wp in &seq.camera_waypoints {
                if !(0.0..=16384.0).contains(&wp.position[0])
                    || !(0.0..=16384.0).contains(&wp.position[2])
                {
                    return Err(format!(
                        "過場「{}」中航點座標 ({}, {}) 超出地圖邊界 (0..16384)。",
                        seq.name, wp.position[0], wp.position[2]
                    ));
                }
                if !(5.0..=89.0).contains(&wp.pitch_degrees) {
                    return Err(format!(
                        "過場「{}」中航點俯仰角 ({}°) 超出有效視野範圍 (5°..89°)。",
                        seq.name, wp.pitch_degrees
                    ));
                }
                if !(1.0..=500.0).contains(&wp.zoom) {
                    return Err(format!(
                        "過場「{}」中航點焦距距離 ({}) 異常 (有效範圍 1..500)。",
                        seq.name, wp.zoom
                    ));
                }
            }

            // 字幕檢驗
            for sub in &seq.subtitles {
                if sub.text.trim().is_empty() || sub.text.chars().count() > 1000 {
                    return Err(format!(
                        "過場「{}」中的字幕對白文字為空或長度超過 1000 字元。",
                        seq.name
                    ));
                }
                if sub.duration_seconds <= 0.0 {
                    return Err(format!(
                        "過場「{}」中的字幕持續時間必須大於 0 秒。",
                        seq.name
                    ));
                }
            }

            // 部隊調度檢驗
            for order in &seq.unit_orders {
                if !(0.0..=16384.0).contains(&order.target_x)
                    || !(0.0..=16384.0).contains(&order.target_z)
                {
                    return Err(format!(
                        "過場「{}」中的部隊移動目標座標超出邊界。",
                        seq.name
                    ));
                }
            }
        }

        Ok(())
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&self.sequences).map_err(|e| e.to_string())
    }

    pub fn from_json(json: &str) -> Self {
        if json.trim().is_empty() {
            return Self::new();
        }
        let list: Vec<CutsceneSequence> = serde_json::from_str(json).unwrap_or_default();
        Self::from_sequences(list)
    }
}
