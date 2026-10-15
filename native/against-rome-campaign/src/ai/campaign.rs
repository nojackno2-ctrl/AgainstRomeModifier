use super::archetype::FactionAiProfile;
use against_rome_scenario::WorldPosition;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

/// 戰役任務目標類型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CampaignObjectiveType {
    /// 防守存活指定時間
    SurviveTime,
    /// 守護特定建築或 VIP 部隊（目標被毀即失敗）
    DefendTarget,
    /// 摧毀指定目標建築或擊殺敵將（達成即獲勝）
    DestroyTarget,
    /// 抵禦並消滅所有進攻波次
    WaveSurvival,
    /// 護送或引導部隊進入特定矩形區域
    ReachArea,
}

/// 戰役目標定義
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampaignObjective {
    pub id: Uuid,
    pub objective_type: CampaignObjectiveType,
    pub title: String,
    pub description: String,
    pub target_id: Option<Uuid>,
    pub target_min_x: i32,
    pub target_min_z: i32,
    pub target_max_x: i32,
    pub target_max_z: i32,
    pub required_seconds: i32,
    pub is_primary: bool,
    pub is_hidden: bool,
}

impl CampaignObjective {
    pub fn new(
        id: Uuid,
        objective_type: CampaignObjectiveType,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id,
            objective_type,
            title: title.into(),
            description: description.into(),
            target_id: None,
            target_min_x: 0,
            target_min_z: 0,
            target_max_x: 16383,
            target_max_z: 16383,
            required_seconds: 0,
            is_primary: true,
            is_hidden: false,
        }
    }

    pub fn with_target(mut self, target_id: Uuid) -> Self {
        self.target_id = Some(target_id);
        self
    }

    pub fn with_seconds(mut self, seconds: i32) -> Self {
        self.required_seconds = seconds;
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_nil() {
            return Err("戰役目標必須具備持久 ID。".into());
        }
        if self.title.trim().is_empty() {
            return Err("戰役目標標題不能為空。".into());
        }
        if matches!(
            self.objective_type,
            CampaignObjectiveType::DefendTarget | CampaignObjectiveType::DestroyTarget
        ) {
            match self.target_id {
                Some(id) if !id.is_nil() => {}
                _ => {
                    return Err(format!(
                        "目標類型「{:?}」必須指定目標物件 ID。",
                        self.objective_type
                    ))
                }
            }
        }
        if self.objective_type == CampaignObjectiveType::SurviveTime && self.required_seconds <= 0 {
            return Err("存活時間目標必須指定大於 0 的秒數。".into());
        }
        if self.objective_type == CampaignObjectiveType::ReachArea {
            match self.target_id {
                Some(id) if !id.is_nil() => {}
                _ => return Err("區域到達目標必須指定護送目標物件 ID。".into()),
            }
            if self.target_min_x < 0
                || self.target_min_z < 0
                || self.target_max_x > 16383
                || self.target_max_z > 16383
                || self.target_min_x > self.target_max_x
                || self.target_min_z > self.target_max_z
            {
                return Err("目標區域邊界必須合法且介於 0–16383。".into());
            }
        }
        Ok(())
    }
}

/// 波次或增援部隊配置
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaveSquadDefinition {
    pub alias: String,
    pub count: i32,
    pub team: i32,
    pub relative_delay_seconds: i32,
}

impl WaveSquadDefinition {
    pub fn new(alias: impl Into<String>, count: i32, team: i32) -> Self {
        Self {
            alias: alias.into(),
            count,
            team,
            relative_delay_seconds: 0,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.alias.trim().is_empty() {
            return Err("部隊別名不能為空。".into());
        }
        if !(1..=20).contains(&self.count) {
            return Err("部隊人數必須介於 1 與 20 之間。".into());
        }
        if !(0..=7).contains(&self.team) {
            return Err("部隊隊伍編號必須介於 0 與 7 之間。".into());
        }
        if self.relative_delay_seconds < 0 {
            return Err("相對延遲秒數不能為負數。".into());
        }
        Ok(())
    }
}

/// 進攻波次定義
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveAttackDefinition {
    pub wave_index: i32,
    pub trigger_delay_seconds: i32,
    pub announcement: String,
    pub spawn_x: f32,
    pub spawn_z: f32,
    pub squads: Vec<WaveSquadDefinition>,
    pub assigned_path_id: Option<Uuid>,
    pub target_object_id: Option<Uuid>,
    pub move_destination: Option<WorldPosition>,
}

impl WaveAttackDefinition {
    pub fn new(
        wave_index: i32,
        trigger_delay_seconds: i32,
        announcement: impl Into<String>,
        spawn_x: f32,
        spawn_z: f32,
        squads: Vec<WaveSquadDefinition>,
    ) -> Self {
        Self {
            wave_index,
            trigger_delay_seconds,
            announcement: announcement.into(),
            spawn_x,
            spawn_z,
            squads,
            assigned_path_id: None,
            target_object_id: None,
            move_destination: None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.wave_index < 1 {
            return Err("波次序號必須 >= 1。".into());
        }
        if self.trigger_delay_seconds < 0 {
            return Err("波次觸發延遲不能為負數。".into());
        }
        if !self.spawn_x.is_finite()
            || !self.spawn_z.is_finite()
            || !(0.0..=16383.0).contains(&self.spawn_x)
            || !(0.0..=16383.0).contains(&self.spawn_z)
        {
            return Err("生成點座標必須介於 0 與 16383 之間。".into());
        }
        if self.squads.is_empty() {
            return Err(format!("第 {} 波至少需要包含一個部隊編制。", self.wave_index));
        }
        for squad in &self.squads {
            squad.validate()?;
        }
        Ok(())
    }
}

/// 定時增援定義
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReinforcementDefinition {
    pub id: Uuid,
    pub name: String,
    pub trigger_delay_seconds: i32,
    pub team: i32,
    pub spawn_x: f32,
    pub spawn_z: f32,
    pub squads: Vec<WaveSquadDefinition>,
    pub notification_text: String,
    pub assigned_path_id: Option<Uuid>,
}

impl ReinforcementDefinition {
    pub fn new(
        id: Uuid,
        name: impl Into<String>,
        trigger_delay_seconds: i32,
        team: i32,
        spawn_x: f32,
        spawn_z: f32,
        squads: Vec<WaveSquadDefinition>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            trigger_delay_seconds,
            team,
            spawn_x,
            spawn_z,
            squads,
            notification_text: String::new(),
            assigned_path_id: None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_nil() {
            return Err("增援項目必須具備持久 ID。".into());
        }
        if self.name.trim().is_empty() {
            return Err("增援名稱不能為空。".into());
        }
        if self.trigger_delay_seconds < 0 {
            return Err("增援觸發延遲不能為負數。".into());
        }
        if !(0..=7).contains(&self.team) {
            return Err("增援隊伍必須介於 0 與 7。".into());
        }
        if !self.spawn_x.is_finite()
            || !self.spawn_z.is_finite()
            || !(0.0..=16383.0).contains(&self.spawn_x)
            || !(0.0..=16383.0).contains(&self.spawn_z)
        {
            return Err("生成點座標必須介於 0 與 16383 之間。".into());
        }
        if self.squads.is_empty() {
            return Err(format!("增援「{}」至少需要包含一個部隊編制。", self.name));
        }
        for squad in &self.squads {
            squad.validate()?;
        }
        Ok(())
    }
}

impl PartialEq for WaveAttackDefinition {
    fn eq(&self, other: &Self) -> bool {
        self.wave_index == other.wave_index
            && self.trigger_delay_seconds == other.trigger_delay_seconds
            && self.announcement == other.announcement
            && self.spawn_x == other.spawn_x
            && self.spawn_z == other.spawn_z
            && self.squads == other.squads
            && self.assigned_path_id == other.assigned_path_id
            && self.target_object_id == other.target_object_id
            && match (&self.move_destination, &other.move_destination) {
                (Some(a), Some(b)) => a.x == b.x && a.z == b.z,
                (None, None) => true,
                _ => false,
            }
    }
}

/// 高階戰役企劃總綱
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampaignMissionPlan {
    pub title: String,
    pub briefing: String,
    pub objectives: Vec<CampaignObjective>,
    pub waves: Vec<WaveAttackDefinition>,
    pub reinforcements: Vec<ReinforcementDefinition>,
    pub faction_profiles: Vec<FactionAiProfile>,
}

impl CampaignMissionPlan {
    pub fn new(title: impl Into<String>, briefing: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            briefing: briefing.into(),
            objectives: vec![],
            waves: vec![],
            reinforcements: vec![],
            faction_profiles: vec![],
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.title.trim().is_empty() {
            return Err("戰役任務標題不能為空。".into());
        }
        let mut obj_ids = HashSet::new();
        for obj in &self.objectives {
            obj.validate()?;
            if !obj_ids.insert(obj.id) {
                return Err(format!("包含重複的戰役目標 ID：{}", obj.id));
            }
        }

        let mut wave_indices = HashSet::new();
        for wave in &self.waves {
            wave.validate()?;
            if !wave_indices.insert(wave.wave_index) {
                return Err(format!("包含重複的波次序號：{}", wave.wave_index));
            }
        }

        let mut reinf_ids = HashSet::new();
        for reinf in &self.reinforcements {
            reinf.validate()?;
            if !reinf_ids.insert(reinf.id) {
                return Err(format!("包含重複的增援 ID：{}", reinf.id));
            }
        }

        for faction in &self.faction_profiles {
            faction.validate()?;
        }

        Ok(())
    }
}

