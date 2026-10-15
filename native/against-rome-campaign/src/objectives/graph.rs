use super::catalog::ObjectiveRuleCatalog;
use super::models::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

/// 目標依賴圖診斷嚴重性。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObjectiveDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

/// 目標依賴圖診斷訊息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectiveDiagnostic {
    pub severity: ObjectiveDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub objective_id: Option<Uuid>,
}

impl ObjectiveDiagnostic {
    pub fn new(
        severity: ObjectiveDiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            objective_id: None,
        }
    }

    pub fn with_id(
        severity: ObjectiveDiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
        objective_id: Uuid,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            objective_id: Some(objective_id),
        }
    }
}

/// 階層式戰役目標依賴圖（ObjectiveDependencyGraph）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObjectiveDependencyGraph {
    objectives: HashMap<Uuid, ObjectiveDefinition>,
    objective_order: Vec<Uuid>,
    dependencies: Vec<ObjectiveDependency>,
}

impl ObjectiveDependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn objectives(&self) -> Vec<&ObjectiveDefinition> {
        self.objective_order
            .iter()
            .filter_map(|id| self.objectives.get(id))
            .collect()
    }

    pub fn dependencies(&self) -> &[ObjectiveDependency] {
        &self.dependencies
    }

    pub fn add_objective(&mut self, objective: ObjectiveDefinition) -> Result<(), String> {
        if self.objectives.contains_key(&objective.id) {
            return Err(format!("目標 ID {} 已存在於圖中。", objective.id));
        }
        self.objective_order.push(objective.id);
        self.objectives.insert(objective.id, objective);
        Ok(())
    }

    pub fn remove_objective(&mut self, objective_id: Uuid) -> bool {
        if self.objectives.remove(&objective_id).is_some() {
            self.objective_order.retain(|id| *id != objective_id);
            self.dependencies.retain(|d| {
                d.source_objective_id != objective_id && d.target_objective_id != objective_id
            });
            true
        } else {
            false
        }
    }

    pub fn find_objective(&self, objective_id: Uuid) -> Option<&ObjectiveDefinition> {
        self.objectives.get(&objective_id)
    }

    pub fn add_dependency(
        &mut self,
        source_id: Uuid,
        target_id: Uuid,
        relation: DependencyRelation,
    ) -> bool {
        if !self.objectives.contains_key(&source_id) || !self.objectives.contains_key(&target_id) {
            return false;
        }
        if source_id == target_id {
            return false;
        }
        if self.dependencies.iter().any(|d| {
            d.source_objective_id == source_id
                && d.target_objective_id == target_id
                && d.relation == relation
        }) {
            return true;
        }
        self.dependencies
            .push(ObjectiveDependency::new(source_id, target_id, relation));
        true
    }

    pub fn remove_dependency(&mut self, source_id: Uuid, target_id: Uuid) -> bool {
        let initial_len = self.dependencies.len();
        self.dependencies.retain(|d| {
            !(d.source_objective_id == source_id && d.target_objective_id == target_id)
        });
        self.dependencies.len() < initial_len
    }

    pub fn get_prerequisites(&self, target_id: Uuid) -> Vec<&ObjectiveDefinition> {
        self.dependencies
            .iter()
            .filter(|d| {
                d.target_objective_id == target_id && d.relation == DependencyRelation::Prerequisite
            })
            .filter_map(|d| self.objectives.get(&d.source_objective_id))
            .collect()
    }

    pub fn get_dependents(&self, source_id: Uuid) -> Vec<&ObjectiveDefinition> {
        self.dependencies
            .iter()
            .filter(|d| {
                d.source_objective_id == source_id && d.relation == DependencyRelation::Prerequisite
            })
            .filter_map(|d| self.objectives.get(&d.target_objective_id))
            .collect()
    }

    pub fn get_composite_children(
        &self,
        parent_id: Uuid,
        relation: DependencyRelation,
    ) -> Vec<&ObjectiveDefinition> {
        self.dependencies
            .iter()
            .filter(|d| d.target_objective_id == parent_id && d.relation == relation)
            .filter_map(|d| self.objectives.get(&d.source_objective_id))
            .collect()
    }

    pub fn get_mutually_exclusive_objectives(
        &self,
        objective_id: Uuid,
    ) -> Vec<&ObjectiveDefinition> {
        let mut ids = HashSet::new();
        for d in &self.dependencies {
            if d.relation == DependencyRelation::MutuallyExclusive {
                if d.source_objective_id == objective_id {
                    ids.insert(d.target_objective_id);
                } else if d.target_objective_id == objective_id {
                    ids.insert(d.source_objective_id);
                }
            }
        }
        ids.into_iter()
            .filter_map(|id| self.objectives.get(&id))
            .collect()
    }

    // Graph Algorithms

    pub fn detect_cycles(&self) -> Vec<Vec<Uuid>> {
        let mut cycles = vec![];
        let mut visited: HashMap<Uuid, i32> = HashMap::new(); // 0: unvisited, 1: visiting, 2: visited
        let mut path = vec![];

        for &id in &self.objective_order {
            if visited.get(&id).copied().unwrap_or(0) == 0 {
                self.dfs_cycles(id, &mut visited, &mut path, &mut cycles);
            }
        }

        cycles
    }

    fn dfs_cycles(
        &self,
        current: Uuid,
        visited: &mut HashMap<Uuid, i32>,
        path: &mut Vec<Uuid>,
        cycles: &mut Vec<Vec<Uuid>>,
    ) {
        visited.insert(current, 1);
        path.push(current);

        let neighbors: Vec<Uuid> = self
            .dependencies
            .iter()
            .filter(|d| {
                d.source_objective_id == current
                    && matches!(
                        d.relation,
                        DependencyRelation::Prerequisite
                            | DependencyRelation::CompositeAnd
                            | DependencyRelation::CompositeOr
                    )
            })
            .map(|d| d.target_objective_id)
            .collect();

        for neighbor in neighbors {
            let neighbor_visited = visited.get(&neighbor).copied().unwrap_or(0);
            if neighbor_visited == 1 {
                // 環路
                if let Some(pos) = path.iter().position(|id| *id == neighbor) {
                    let mut cycle = path[pos..].to_vec();
                    cycle.push(neighbor);
                    cycles.push(cycle);
                }
            } else if neighbor_visited == 0 {
                self.dfs_cycles(neighbor, visited, path, cycles);
            }
        }

        path.pop();
        visited.insert(current, 2);
    }

    pub fn get_topological_order(&self) -> Result<Vec<ObjectiveDefinition>, String> {
        let cycles = self.detect_cycles();
        if !cycles.is_empty() {
            return Err("目標依賴圖存在循環依賴，無法執行拓撲排序。".into());
        }

        let mut in_degree: HashMap<Uuid, usize> =
            self.objective_order.iter().map(|k| (*k, 0)).collect();
        for dep in &self.dependencies {
            if matches!(
                dep.relation,
                DependencyRelation::Prerequisite
                    | DependencyRelation::CompositeAnd
                    | DependencyRelation::CompositeOr
            ) {
                if let Some(deg) = in_degree.get_mut(&dep.target_objective_id) {
                    *deg += 1;
                }
            }
        }

        let mut queue: VecDeque<Uuid> = self
            .objective_order
            .iter()
            .copied()
            .filter(|k| in_degree.get(k) == Some(&0))
            .collect();

        let mut order = vec![];
        while let Some(id) = queue.pop_front() {
            if let Some(obj) = self.objectives.get(&id) {
                order.push(obj.clone());
            }

            for dep in &self.dependencies {
                if dep.source_objective_id == id
                    && matches!(
                        dep.relation,
                        DependencyRelation::Prerequisite
                            | DependencyRelation::CompositeAnd
                            | DependencyRelation::CompositeOr
                    )
                {
                    if let Some(deg) = in_degree.get_mut(&dep.target_objective_id) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dep.target_objective_id);
                        }
                    }
                }
            }
        }

        Ok(order)
    }

    pub fn validate(&self) -> Vec<ObjectiveDiagnostic> {
        let mut diagnostics = vec![];

        if self.objectives.is_empty() {
            diagnostics.push(ObjectiveDiagnostic::new(
                ObjectiveDiagnosticSeverity::Warning,
                "OBJ_EMPTY",
                "目前戰役目標圖未包含任何目標。",
            ));
            return diagnostics;
        }

        // 1. 個別目標參數驗證
        for obj in self.objectives.values() {
            let result = ObjectiveRuleCatalog::validate(obj);
            if !result.is_valid {
                for err in result.errors {
                    diagnostics.push(ObjectiveDiagnostic::with_id(
                        ObjectiveDiagnosticSeverity::Error,
                        "OBJ_PARAM_INVALID",
                        format!("[{}] {err}", obj.title),
                        obj.id,
                    ));
                }
            }
        }

        // 2. 循環依賴檢查
        let cycles = self.detect_cycles();
        if !cycles.is_empty() {
            for cycle in cycles {
                let path_str: Vec<String> = cycle
                    .iter()
                    .map(|id| {
                        self.objectives
                            .get(id)
                            .map(|o| o.title.as_str())
                            .unwrap_or("Unknown")
                            .to_string()
                    })
                    .collect();
                diagnostics.push(ObjectiveDiagnostic::new(
                    ObjectiveDiagnosticSeverity::Error,
                    "OBJ_CYCLE_DETECTED",
                    format!("偵測到循環依賴路徑：{}", path_str.join(" -> ")),
                ));
            }
        }

        // 3. 主線目標存在性驗證
        let primary_count = self
            .objectives
            .values()
            .filter(|o| o.category == ObjectiveCategory::Primary)
            .count();
        if primary_count == 0 {
            diagnostics.push(ObjectiveDiagnostic::new(
                ObjectiveDiagnosticSeverity::Error,
                "OBJ_NO_PRIMARY",
                "戰役目標圖必須至少包含一個主要主線目標（Primary Objective）。",
            ));
        }

        // 4. 懸空依賴或未引用檢查
        for dep in &self.dependencies {
            if !self.objectives.contains_key(&dep.source_objective_id) {
                diagnostics.push(ObjectiveDiagnostic::new(
                    ObjectiveDiagnosticSeverity::Error,
                    "OBJ_DANGLING_DEP",
                    format!("依賴來源 ID {} 不存在。", dep.source_objective_id),
                ));
            }
            if !self.objectives.contains_key(&dep.target_objective_id) {
                diagnostics.push(ObjectiveDiagnostic::new(
                    ObjectiveDiagnosticSeverity::Error,
                    "OBJ_DANGLING_DEP",
                    format!("依賴目標 ID {} 不存在。", dep.target_objective_id),
                ));
            }
        }

        diagnostics
    }

    // State Progression Engine

    pub fn propagate_states(
        &self,
        current_states: &HashMap<Uuid, ObjectiveState>,
    ) -> HashMap<Uuid, ObjectiveState> {
        let mut states = current_states.clone();

        for obj in self.objectives.values() {
            states.entry(obj.id).or_insert(obj.initial_state);
        }

        let mut changed = true;
        let max_passes = self.objectives.len() * 2 + 5;
        let mut pass = 0;

        while changed && pass < max_passes {
            pass += 1;
            changed = false;

            for obj in self.objectives.values() {
                let cur_state = states[&obj.id];

                // 1. 處理互斥路線（若任一互斥目標已完成，此目標自動標記為 Abandoned）
                if matches!(
                    cur_state,
                    ObjectiveState::Active | ObjectiveState::Inactive | ObjectiveState::Hidden
                ) {
                    let exclusive_nodes = self.get_mutually_exclusive_objectives(obj.id);
                    if exclusive_nodes.iter().any(|other| {
                        states.get(&other.id) == Some(&ObjectiveState::Completed)
                    }) {
                        states.insert(obj.id, ObjectiveState::Abandoned);
                        changed = true;
                        continue;
                    }
                }

                // 2. 處理前置解鎖（Prerequisites -> Inactive / Hidden 轉為 Active）
                if matches!(cur_state, ObjectiveState::Inactive | ObjectiveState::Hidden) {
                    let prereqs = self.get_prerequisites(obj.id);
                    if !prereqs.is_empty()
                        && prereqs.iter().all(|p| {
                            states.get(&p.id) == Some(&ObjectiveState::Completed)
                        })
                    {
                        states.insert(obj.id, ObjectiveState::Active);
                        changed = true;
                        continue;
                    }
                }

                // 3. 處理複合與（Composite AND 父節點自動達成）
                if cur_state == ObjectiveState::Active {
                    let and_children =
                        self.get_composite_children(obj.id, DependencyRelation::CompositeAnd);
                    if !and_children.is_empty()
                        && and_children.iter().all(|c| {
                            states.get(&c.id) == Some(&ObjectiveState::Completed)
                        })
                    {
                        states.insert(obj.id, ObjectiveState::Completed);
                        changed = true;
                        continue;
                    }

                    // 4. 處理複合或（Composite OR 父節點任一達成）
                    let or_children =
                        self.get_composite_children(obj.id, DependencyRelation::CompositeOr);
                    if !or_children.is_empty()
                        && or_children.iter().any(|c| {
                            states.get(&c.id) == Some(&ObjectiveState::Completed)
                        })
                    {
                        states.insert(obj.id, ObjectiveState::Completed);
                        changed = true;
                        continue;
                    }
                }
            }
        }

        states
    }

    pub fn evaluate_campaign_result(
        &self,
        states: &HashMap<Uuid, ObjectiveState>,
    ) -> CampaignResult {
        // 1. 檢查關鍵失敗條件（FailureCriterion）
        for obj in self
            .objectives
            .values()
            .filter(|o| o.category == ObjectiveCategory::FailureCriterion)
        {
            if states.get(&obj.id) == Some(&ObjectiveState::Failed) {
                return CampaignResult::Defeat;
            }
        }

        // 2. 檢查主線目標（Primary）
        let primary_objectives: Vec<_> = self
            .objectives
            .values()
            .filter(|o| {
                o.category == ObjectiveCategory::Primary
                    && states.get(&o.id) != Some(&ObjectiveState::Abandoned)
            })
            .collect();

        if primary_objectives.is_empty() {
            return CampaignResult::InProgress;
        }

        // 若任一未被捨棄的主線目標已判定失敗，且無替代路徑，則戰役失敗
        if primary_objectives
            .iter()
            .any(|o| states.get(&o.id) == Some(&ObjectiveState::Failed))
        {
            return CampaignResult::Defeat;
        }

        // 若所有主線目標皆已 Completed，則戰役勝利！
        if primary_objectives
            .iter()
            .all(|o| states.get(&o.id) == Some(&ObjectiveState::Completed))
        {
            return CampaignResult::Victory;
        }

        CampaignResult::InProgress
    }

    pub fn clear(&mut self) {
        self.dependencies.clear();
        self.objective_order.clear();
        self.objectives.clear();
    }
}
