use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

/// 路徑移動拓撲模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WaypointMovementMode {
    /// 循環巡邏（A -> B -> C -> A）
    Loop,
    /// 往返折返巡邏（A -> B -> C -> B -> A）
    PingPong,
    /// 埋伏待命（駐守原點，觸發後沿路線突擊）
    Ambush,
    /// 單向行軍（抵達終點後就地防守）
    OneWay,
}

/// 部隊行軍姿態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WaypointStance {
    /// 標準行軍（標準隊形與速限）
    NormalMarch,
    /// 快速疾行（脫離戰鬥、全速推進）
    FastMarch,
    /// 戒備警戒（搜尋視野內敵人、邊走邊偵查）
    AggressivePatrol,
    /// 隱蔽埋伏（降低可見度、原地待命）
    StealthHold,
}

/// 路徑點節點定義
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaypointNode {
    pub id: Uuid,
    pub order_index: i32,
    pub world_x: f32,
    pub world_z: f32,
    pub tolerance_radius: f32,
    pub dwell_time_seconds: i32,
    pub stance: WaypointStance,
    pub action_trigger_name: Option<String>,
}

impl WaypointNode {
    pub fn new(id: Uuid, order_index: i32, world_x: f32, world_z: f32) -> Self {
        Self {
            id,
            order_index,
            world_x,
            world_z,
            tolerance_radius: 150.0,
            dwell_time_seconds: 0,
            stance: WaypointStance::NormalMarch,
            action_trigger_name: None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_nil() {
            return Err("路徑點節點必須具備持久 ID。".into());
        }
        if !self.world_x.is_finite()
            || !self.world_z.is_finite()
            || !(0.0..=16383.0).contains(&self.world_x)
            || !(0.0..=16383.0).contains(&self.world_z)
        {
            return Err("路徑點座標必須介於 0 與 16383 之間。".into());
        }
        if !(10.0..=2000.0).contains(&self.tolerance_radius) {
            return Err("容差半徑必須介於 10 與 2000 之間。".into());
        }
        if !(0..=3600).contains(&self.dwell_time_seconds) {
            return Err("停留時間必須介於 0 與 3600 秒之間。".into());
        }
        Ok(())
    }
}

/// 巡邏與進攻路線（節點鏈）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaypointPath {
    pub id: Uuid,
    pub name: String,
    pub mode: WaypointMovementMode,
    pub nodes: Vec<WaypointNode>,
    pub assigned_spawn_ids: Vec<Uuid>,
    pub break_on_combat: bool,
    pub return_to_start_on_lost_target: bool,
    pub ambush_trigger_radius: f32,
    pub loop_count: i32,
}

impl WaypointPath {
    pub fn new(
        id: Uuid,
        name: impl Into<String>,
        mode: WaypointMovementMode,
        nodes: Vec<WaypointNode>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            mode,
            nodes,
            assigned_spawn_ids: vec![],
            break_on_combat: true,
            return_to_start_on_lost_target: true,
            ambush_trigger_radius: 800.0,
            loop_count: -1,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_nil() {
            return Err("路徑必須具備持久 ID。".into());
        }
        if self.name.trim().is_empty() {
            return Err("路徑名稱不能為空。".into());
        }
        if self.nodes.len() < 2 {
            return Err(format!("路徑「{}」至少需要 2 個路徑點節點。", self.name));
        }
        if !(50.0..=8000.0).contains(&self.ambush_trigger_radius) {
            return Err("埋伏警戒半徑必須介於 50 與 8000 之間。".into());
        }

        let mut ids = HashSet::new();
        for node in &self.nodes {
            node.validate()?;
            if !ids.insert(node.id) {
                return Err(format!("路徑「{}」包含重複的節點 ID。", self.name));
            }
        }
        Ok(())
    }
}

/// 部隊巡邏與進攻路線規劃輔助器
pub struct WaypointPathPlanner;

impl WaypointPathPlanner {
    /// 計算路徑總長度（以地圖世界座標單位為準）
    pub fn calculate_total_distance(path: &WaypointPath) -> f32 {
        if path.nodes.len() < 2 {
            return 0.0;
        }

        let mut total = 0.0f32;
        for i in 0..path.nodes.len() - 1 {
            let dx = path.nodes[i + 1].world_x - path.nodes[i].world_x;
            let dz = path.nodes[i + 1].world_z - path.nodes[i].world_z;
            total += (dx * dx + dz * dz).sqrt();
        }

        if path.mode == WaypointMovementMode::Loop && path.nodes.len() >= 3 {
            let dx = path.nodes[0].world_x - path.nodes[path.nodes.len() - 1].world_x;
            let dz = path.nodes[0].world_z - path.nodes[path.nodes.len() - 1].world_z;
            total += (dx * dx + dz * dz).sqrt();
        }

        total
    }

    /// 推進計算下一個節點索引
    pub fn get_next_node_index(
        path: &WaypointPath,
        current_index: usize,
        is_reversing: &mut bool,
    ) -> usize {
        if path.nodes.is_empty() {
            return 0;
        }
        if path.nodes.len() == 1 {
            return 0;
        }

        match path.mode {
            WaypointMovementMode::Loop => (current_index + 1) % path.nodes.len(),
            WaypointMovementMode::PingPong => {
                if *is_reversing {
                    if current_index == 0 {
                        *is_reversing = false;
                        1
                    } else {
                        current_index - 1
                    }
                } else if current_index >= path.nodes.len() - 1 {
                    *is_reversing = true;
                    path.nodes.len().saturating_sub(2)
                } else {
                    current_index + 1
                }
            }
            WaypointMovementMode::OneWay | WaypointMovementMode::Ambush => {
                std::cmp::min(current_index + 1, path.nodes.len() - 1)
            }
        }
    }

    /// 檢查整條路徑上所有節點的通行性
    pub fn validate_passability<F>(
        path: &WaypointPath,
        is_passable_predicate: Option<F>,
    ) -> Vec<(usize, String)>
    where
        F: Fn(f32, f32) -> bool,
    {
        let mut issues = vec![];

        for (i, node) in path.nodes.iter().enumerate() {
            if !(0.0..=16383.0).contains(&node.world_x) || !(0.0..=16383.0).contains(&node.world_z)
            {
                issues.push((
                    i,
                    format!(
                        "節點 {} 座標 ({:.0}, {:.0}) 超出地圖範圍。",
                        i, node.world_x, node.world_z
                    ),
                ));
                continue;
            }

            if let Some(ref is_passable) = is_passable_predicate {
                if !is_passable(node.world_x, node.world_z) {
                    issues.push((
                        i,
                        format!(
                            "節點 {} 座標 ({:.0}, {:.0}) 位於不可通行區域（如深水或陡峭山壁）。",
                            i, node.world_x, node.world_z
                        ),
                    ));
                }
            }
        }

        issues
    }
}
