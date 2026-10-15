use super::compiler::BciObjectiveCompiler;
use super::graph::{ObjectiveDependencyGraph, ObjectiveDiagnostic};
use super::models::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 目標狀態變更事件引數。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectiveStateChangedEventArgs {
    pub objective_id: Uuid,
    pub title: String,
    pub previous_state: ObjectiveState,
    pub new_state: ObjectiveState,
    pub reason: String,
}

/// 單一目標即時進度快照。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectiveProgressSnapshot {
    pub objective_id: Uuid,
    pub title: String,
    pub kind: ObjectiveKind,
    pub category: ObjectiveCategory,
    pub state: ObjectiveState,
    pub progress_percentage: f32,
    pub elapsed_seconds: f32,
    pub remaining_time_limit_seconds: f32,
    pub current_hold_seconds: f32,
    pub target_hold_seconds: f32,
}

/// 沙盒整體運行狀態快照。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectiveSandboxSnapshot {
    pub campaign_status: CampaignResult,
    pub total_simulated_seconds: f32,
    pub objective_snapshots: Vec<ObjectiveProgressSnapshot>,
    pub message_log: Vec<String>,
}

/// 編輯器目標進度即時測試沙盒實作（ObjectiveSandboxSession）
#[derive(Debug, Clone, Default)]
pub struct ObjectiveSandboxSession {
    graph: Option<ObjectiveDependencyGraph>,
    states: HashMap<Uuid, ObjectiveState>,
    elapsed_seconds: HashMap<Uuid, f32>,
    hold_seconds: HashMap<Uuid, f32>,
    dead_objects: HashMap<Uuid, bool>,
    object_positions: HashMap<Uuid, (f32, f32)>,
    team_unit_counts: HashMap<i32, i32>,
    message_log: Vec<String>,
    state_changes: Vec<ObjectiveStateChangedEventArgs>,
    pub campaign_status: CampaignResult,
    pub total_simulated_seconds: f32,
}

impl ObjectiveSandboxSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn export_diagnostics(&self) -> Vec<ObjectiveDiagnostic> {
        match &self.graph {
            Some(graph) => BciObjectiveCompiler::compile(graph, None, None).diagnostics,
            None => vec![],
        }
    }

    pub fn state_changes(&self) -> &[ObjectiveStateChangedEventArgs] {
        &self.state_changes
    }

    pub fn message_log(&self) -> &[String] {
        &self.message_log
    }

    pub fn initialize(&mut self, graph: ObjectiveDependencyGraph) {
        self.graph = Some(graph);
        self.reset();
    }

    pub fn reset(&mut self) {
        self.total_simulated_seconds = 0.0;
        self.campaign_status = CampaignResult::InProgress;
        self.states.clear();
        self.elapsed_seconds.clear();
        self.hold_seconds.clear();
        self.dead_objects.clear();
        self.object_positions.clear();
        self.team_unit_counts.clear();
        self.message_log.clear();
        self.state_changes.clear();

        if let Some(ref graph) = self.graph {
            for obj in graph.objectives() {
                self.states.insert(obj.id, obj.initial_state);
                self.elapsed_seconds.insert(obj.id, 0.0);
                self.hold_seconds.insert(obj.id, 0.0);
            }
            self.propagate_and_evaluate();
        }
    }

    pub fn tick(&mut self, delta_seconds: f32) {
        if self.graph.is_none()
            || self.campaign_status != CampaignResult::InProgress
            || delta_seconds <= 0.0
        {
            return;
        }

        self.total_simulated_seconds += delta_seconds;

        let graph = self.graph.as_ref().unwrap().clone();
        for obj in graph.objectives() {
            if self.states.get(&obj.id).copied() != Some(ObjectiveState::Active) {
                continue;
            }

            let elapsed = self.elapsed_seconds.entry(obj.id).or_insert(0.0);
            *elapsed += delta_seconds;
            let cur_elapsed = *elapsed;

            // 1. 時限超時判定（若有設定且非堅守目標）
            if obj.parameters.time_limit_seconds > 0
                && obj.kind != ObjectiveKind::Survival
                && cur_elapsed >= obj.parameters.time_limit_seconds as f32
            {
                self.transition_state(obj, ObjectiveState::Failed, "目標逾時未完成。");
                continue;
            }

            // 2. 堅守目標進度（Survival）
            if obj.kind == ObjectiveKind::Survival
                && obj.category != ObjectiveCategory::FailureCriterion
            {
                let target_guid = obj.parameters.target_guids.first().copied();
                let is_target_dead = target_guid
                    .map(|g| self.dead_objects.get(&g).copied().unwrap_or(false))
                    .unwrap_or(false);

                if is_target_dead {
                    self.transition_state(obj, ObjectiveState::Failed, "守護目標已陣亡。");
                    continue;
                }

                let hold = self.hold_seconds.entry(obj.id).or_insert(0.0);
                *hold += delta_seconds;
                if *hold >= obj.parameters.hold_duration_seconds as f32 {
                    self.transition_state(
                        obj,
                        ObjectiveState::Completed,
                        &format!(
                            "成功堅守陣地 {} 秒！",
                            obj.parameters.hold_duration_seconds
                        ),
                    );
                    continue;
                }
            }

            // 3. 佔領區域維持進度（CaptureArea / KingOfTheHill）
            if matches!(
                obj.kind,
                ObjectiveKind::CaptureArea | ObjectiveKind::KingOfTheHill
            ) {
                let target_guid = obj.parameters.target_guids.first().copied();
                let mut in_area = false;

                if let Some(target_guid) = target_guid {
                    if let Some(&pos) = self.object_positions.get(&target_guid) {
                        if let Some(ref area) = obj.parameters.area {
                            in_area = area.contains(pos.0, pos.1);
                        }
                    }
                }

                if in_area {
                    let hold = self.hold_seconds.entry(obj.id).or_insert(0.0);
                    *hold += delta_seconds;
                    if *hold >= obj.parameters.hold_duration_seconds as f32 {
                        self.transition_state(
                            obj,
                            ObjectiveState::Completed,
                            &format!(
                                "成功佔領區域維持 {} 秒！",
                                obj.parameters.hold_duration_seconds
                            ),
                        );
                    }
                }
            }
        }

        self.propagate_and_evaluate();
    }

    pub fn simulate_object_death(&mut self, target_id: Uuid) {
        if self.graph.is_none() || target_id.is_nil() {
            return;
        }
        self.dead_objects.insert(target_id, true);

        let graph = self.graph.as_ref().unwrap().clone();
        for obj in graph.objectives() {
            if self.states.get(&obj.id).copied() != Some(ObjectiveState::Active) {
                continue;
            }

            // 1. 刺殺/破壞關鍵目標
            if matches!(
                obj.kind,
                ObjectiveKind::AssassinateTarget | ObjectiveKind::DestroyBuilding
            ) {
                if !obj.parameters.target_guids.is_empty()
                    && obj
                        .parameters
                        .target_guids
                        .iter()
                        .all(|id| self.dead_objects.get(id).copied().unwrap_or(false))
                {
                    self.transition_state(
                        obj,
                        ObjectiveState::Completed,
                        &format!("關鍵目標 {target_id} 已被殲滅！"),
                    );
                }
            }

            // 2. 守護資產陣亡 -> 失敗判據觸發
            if obj.kind == ObjectiveKind::Survival
                || obj.category == ObjectiveCategory::FailureCriterion
            {
                if obj.parameters.target_guids.contains(&target_id) {
                    self.transition_state(
                        obj,
                        ObjectiveState::Failed,
                        &format!("重要守護目標 {target_id} 已遭摧毀！"),
                    );
                }
            }

            // 3. VIP 陣亡 -> 護送失敗
            if obj.kind == ObjectiveKind::EscortUnit {
                if obj.parameters.target_guids.contains(&target_id) {
                    self.transition_state(
                        obj,
                        ObjectiveState::Failed,
                        &format!("護送對象 {target_id} 途中遇害！"),
                    );
                }
            }
        }

        self.propagate_and_evaluate();
    }

    pub fn simulate_object_move(&mut self, target_id: Uuid, world_x: f32, world_z: f32) {
        if self.graph.is_none() || target_id.is_nil() {
            return;
        }
        self.object_positions.insert(target_id, (world_x, world_z));

        let graph = self.graph.as_ref().unwrap().clone();
        for obj in graph.objectives() {
            if self.states.get(&obj.id).copied() != Some(ObjectiveState::Active) {
                continue;
            }

            // 護送商隊抵達終點區域
            if obj.kind == ObjectiveKind::EscortUnit
                && obj.parameters.target_guids.contains(&target_id)
            {
                if let Some(ref area) = obj.parameters.area {
                    if area.contains(world_x, world_z) {
                        self.transition_state(
                            obj,
                            ObjectiveState::Completed,
                            &format!(
                                "商隊已平安抵達撤離區域 ({world_x:.0}, {world_z:.0})！"
                            ),
                        );
                    }
                }
            }
        }

        self.propagate_and_evaluate();
    }

    pub fn simulate_team_unit_count(&mut self, team: i32, remaining_count: i32) {
        if self.graph.is_none() {
            return;
        }
        self.team_unit_counts.insert(team, remaining_count);

        let graph = self.graph.as_ref().unwrap().clone();
        for obj in graph.objectives() {
            if self.states.get(&obj.id).copied() != Some(ObjectiveState::Active) {
                continue;
            }

            if obj.kind == ObjectiveKind::EliminateAllEnemies && obj.parameters.target_team == team
            {
                if remaining_count <= 0 {
                    self.transition_state(
                        obj,
                        ObjectiveState::Completed,
                        &format!("敵軍隊伍 {team} 的所有部隊已被全殲！"),
                    );
                }
            }
        }

        self.propagate_and_evaluate();
    }

    fn transition_state(&mut self, obj: &ObjectiveDefinition, new_state: ObjectiveState, reason: &str) {
        let old_state = self.states.get(&obj.id).copied().unwrap_or(obj.initial_state);
        if old_state == new_state {
            return;
        }

        self.states.insert(obj.id, new_state);
        let log_msg = format!(
            "[{:.1}s] 目標「{}」狀態：{:?} -> {:?} ({})",
            self.total_simulated_seconds, obj.title, old_state, new_state, reason
        );
        self.message_log.push(log_msg);

        self.state_changes.push(ObjectiveStateChangedEventArgs {
            objective_id: obj.id,
            title: obj.title.clone(),
            previous_state: old_state,
            new_state,
            reason: reason.to_string(),
        });

        if new_state == ObjectiveState::Completed {
            if let Some(ref reward) = obj.reward {
                if !reward.completion_message.trim().is_empty() {
                    let reward_msg = format!("[獎勵] {}", reward.completion_message);
                    self.message_log.push(reward_msg);
                }
            }
        }
    }

    fn propagate_and_evaluate(&mut self) {
        let Some(ref graph) = self.graph else { return };

        let updated = graph.propagate_states(&self.states);
        for (id, state) in updated {
            let old_state = self.states.get(&id).copied().unwrap_or(ObjectiveState::Inactive);
            if old_state != state {
                self.states.insert(id, state);
                if let Some(obj) = graph.find_objective(id) {
                    self.state_changes.push(ObjectiveStateChangedEventArgs {
                        objective_id: id,
                        title: obj.title.clone(),
                        previous_state: old_state,
                        new_state: state,
                        reason: "依賴圖動態解鎖/狀態傳遞".to_string(),
                    });
                }
            }
        }

        let result = graph.evaluate_campaign_result(&self.states);
        if result != self.campaign_status {
            self.campaign_status = result;
            let final_msg = match result {
                CampaignResult::Victory => format!(
                    "戰役全勝！所有主要戰役目標均已順利達成！(耗時 {:.1}s)",
                    self.total_simulated_seconds
                ),
                CampaignResult::Defeat => format!(
                    "戰役失敗！未能保衛關鍵目標或主線任務失敗。(耗時 {:.1}s)",
                    self.total_simulated_seconds
                ),
                _ => String::new(),
            };
            if !final_msg.is_empty() {
                self.message_log.push(final_msg);
            }
        }
    }

    pub fn get_progress(&self, objective_id: Uuid) -> Option<ObjectiveProgressSnapshot> {
        let graph = self.graph.as_ref()?;
        let obj = graph.find_objective(objective_id)?;

        let state = self.states.get(&objective_id).copied().unwrap_or(obj.initial_state);
        let elapsed = self.elapsed_seconds.get(&objective_id).copied().unwrap_or(0.0);
        let hold = self.hold_seconds.get(&objective_id).copied().unwrap_or(0.0);
        let target_hold = obj.parameters.hold_duration_seconds as f32;
        let remaining_limit = if obj.parameters.time_limit_seconds > 0 {
            (obj.parameters.time_limit_seconds as f32 - elapsed).max(0.0)
        } else {
            0.0
        };

        let percentage = match state {
            ObjectiveState::Completed => 100.0,
            ObjectiveState::Failed => 0.0,
            ObjectiveState::Active => {
                if target_hold > 0.0 {
                    (hold / target_hold * 100.0).clamp(0.0, 99.0)
                } else {
                    50.0
                }
            }
            _ => 0.0,
        };

        Some(ObjectiveProgressSnapshot {
            objective_id: obj.id,
            title: obj.title.clone(),
            kind: obj.kind,
            category: obj.category,
            state,
            progress_percentage: percentage,
            elapsed_seconds: elapsed,
            remaining_time_limit_seconds: remaining_limit,
            current_hold_seconds: hold,
            target_hold_seconds: target_hold,
        })
    }

    pub fn capture_snapshot(&self) -> ObjectiveSandboxSnapshot {
        let mut progress_list = vec![];
        if let Some(ref graph) = self.graph {
            for obj in graph.objectives() {
                if let Some(p) = self.get_progress(obj.id) {
                    progress_list.push(p);
                }
            }
        }

        ObjectiveSandboxSnapshot {
            campaign_status: self.campaign_status,
            total_simulated_seconds: self.total_simulated_seconds,
            objective_snapshots: progress_list,
            message_log: self.message_log.clone(),
        }
    }
}
