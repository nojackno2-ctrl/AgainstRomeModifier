use super::archetype::FactionAiProfile;
use super::campaign::{
    CampaignMissionPlan, CampaignObjective, ReinforcementDefinition, WaveAttackDefinition,
};
use super::waypoint::WaypointPath;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// AI 與戰役企劃的快照資料結構
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampaignMissionSnapshot {
    pub plan: CampaignMissionPlan,
    pub waypoint_paths: Vec<WaypointPath>,
}

impl CampaignMissionSnapshot {
    pub fn empty() -> Self {
        Self {
            plan: CampaignMissionPlan::new("Default Mission", ""),
            waypoint_paths: vec![],
        }
    }
}

/// AI 與戰役任務編輯工作階段，負責管理路徑點、AI 勢力設定與戰役目標的編輯歷史與狀態追蹤
#[derive(Debug, Clone)]
pub struct FactionAiSession {
    plan: CampaignMissionPlan,
    waypoint_paths: Vec<WaypointPath>,
    baseline: CampaignMissionSnapshot,
}

impl Default for FactionAiSession {
    fn default() -> Self {
        Self::new(CampaignMissionSnapshot::empty())
    }
}

impl FactionAiSession {
    pub fn new(initial: CampaignMissionSnapshot) -> Self {
        let baseline = initial.clone();
        Self {
            plan: initial.plan,
            waypoint_paths: initial.waypoint_paths,
            baseline,
        }
    }

    pub fn module_id(&self) -> &'static str {
        "campaign_ai"
    }

    pub fn plan(&self) -> &CampaignMissionPlan {
        &self.plan
    }

    pub fn waypoint_paths(&self) -> &[WaypointPath] {
        &self.waypoint_paths
    }

    pub fn is_dirty(&self) -> bool {
        self.plan != self.baseline.plan || self.waypoint_paths != self.baseline.waypoint_paths
    }

    pub fn load(&mut self, snapshot: CampaignMissionSnapshot) {
        self.plan = snapshot.plan;
        self.waypoint_paths = snapshot.waypoint_paths;
        self.accept_changes();
    }

    pub fn capture(&self) -> CampaignMissionSnapshot {
        CampaignMissionSnapshot {
            plan: self.plan.clone(),
            waypoint_paths: self.waypoint_paths.clone(),
        }
    }

    pub fn accept_changes(&mut self) {
        self.baseline = self.capture();
    }

    pub fn reset(&mut self) {
        self.load(self.baseline.clone());
    }

    pub fn update_plan(&mut self, updated_plan: CampaignMissionPlan) -> Result<(), String> {
        updated_plan.validate()?;
        self.plan = updated_plan;
        Ok(())
    }

    pub fn add_waypoint_path(&mut self, path: WaypointPath) -> Result<(), String> {
        path.validate()?;
        if self.waypoint_paths.iter().any(|p| p.id == path.id) {
            return Err(format!("已存在相同 ID 的路徑：{}", path.id));
        }
        self.waypoint_paths.push(path);
        Ok(())
    }

    pub fn update_waypoint_path(&mut self, path: WaypointPath) -> Result<(), String> {
        path.validate()?;
        let index = self
            .waypoint_paths
            .iter()
            .position(|p| p.id == path.id)
            .ok_or_else(|| format!("找不到要更新的路徑：{}", path.id))?;
        self.waypoint_paths[index] = path;
        Ok(())
    }

    pub fn remove_waypoint_path(&mut self, path_id: Uuid) -> bool {
        if let Some(pos) = self.waypoint_paths.iter().position(|p| p.id == path_id) {
            self.waypoint_paths.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn find_path(&self, path_id: Uuid) -> Option<&WaypointPath> {
        self.waypoint_paths.iter().find(|p| p.id == path_id)
    }

    pub fn add_wave(&mut self, wave: WaveAttackDefinition) -> Result<(), String> {
        wave.validate()?;
        if self
            .plan
            .waves
            .iter()
            .any(|w| w.wave_index == wave.wave_index)
        {
            return Err(format!("已存在第 {} 波設定。", wave.wave_index));
        }
        self.plan.waves.push(wave);
        self.plan.waves.sort_by_key(|w| w.wave_index);
        Ok(())
    }

    pub fn add_objective(&mut self, objective: CampaignObjective) -> Result<(), String> {
        objective.validate()?;
        if self.plan.objectives.iter().any(|o| o.id == objective.id) {
            return Err(format!("已存在相同 ID 的戰役目標：{}", objective.id));
        }
        self.plan.objectives.push(objective);
        Ok(())
    }

    pub fn add_reinforcement(
        &mut self,
        reinforcement: ReinforcementDefinition,
    ) -> Result<(), String> {
        reinforcement.validate()?;
        if self
            .plan
            .reinforcements
            .iter()
            .any(|r| r.id == reinforcement.id)
        {
            return Err(format!("已存在相同 ID 的增援設定：{}", reinforcement.id));
        }
        self.plan.reinforcements.push(reinforcement);
        Ok(())
    }

    pub fn set_faction_profile(&mut self, profile: FactionAiProfile) -> Result<(), String> {
        profile.validate()?;
        self.plan.faction_profiles.retain(|f| f.team != profile.team);
        self.plan.faction_profiles.push(profile);
        self.plan.faction_profiles.sort_by_key(|f| f.team);
        Ok(())
    }
}
