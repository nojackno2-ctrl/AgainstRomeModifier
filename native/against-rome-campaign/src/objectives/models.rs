use against_rome_scenario::{Action, Condition};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 戰役目標種類，涵蓋《反抗羅馬》常見的戰術與劇情戰役玩法。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObjectiveKind {
    /// 全殲敵軍：消滅指定敵對陣營的所有部隊或主要軍事實體。
    EliminateAllEnemies,
    /// 堅守陣地倒數計時：守護關鍵物件（基地、陣地、橋樑），在指定秒數內存活。
    Survival,
    /// 佔領特定區域：派遣部隊進駐目標矩形或圓形區域，並維持佔領指定秒數。
    CaptureArea,
    /// 山丘之王：爭奪戰略高地或樞紐，累計佔領維持時長率先達標者勝。
    KingOfTheHill,
    /// 刺殺敵方英雄：擊殺敵對特定關鍵指揮官或英雄單位。
    AssassinateTarget,
    /// 破壞關鍵建築：摧毀指定要塞、神廟、奇觀或軍事哨所。
    DestroyBuilding,
    /// 護送商隊/VIP：引導或保護重要人物/商隊安全抵達目的區域，全程不可陣亡。
    EscortUnit,
    /// 自訂腳本條件：由多重原生物件狀態與位置條件所構成的複合目標。
    CustomScripted,
}

/// 目標類別屬性：區分主線、獎勵與失敗判據。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObjectiveCategory {
    /// 主要目標：戰役獲勝所必須達成的核心主線目標。
    Primary,
    /// 次要/獎勵目標（Bonus）：可選任務，達成可獲得援軍、資源或戰術加成，失敗不影響主線勝利。
    Bonus,
    /// 關鍵失敗判據：若此條件滿足（例如守護對象死亡、老家被平），立即判定戰役失敗。
    FailureCriterion,
}

/// 戰役目標的即時運作狀態機。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObjectiveState {
    /// 隱藏未揭露：玩家在任務日誌中看不見（或標記為「???」），等待前置劇情觸發揭曉。
    Hidden,
    /// 未解鎖/等待中：已列在日誌中但尚未激活，等待前置目標達成後解鎖。
    Inactive,
    /// 進行中：目標已激活，計時器與條件監控正即時運行中。
    Active,
    /// 已達成：玩家已順利完成該目標。
    Completed,
    /// 已失敗：該目標未達成（例如被護送對象死亡或逾時）。
    Failed,
    /// 已捨棄：因互斥分支路線（Mutually Exclusive）被選擇而失效的旁支目標。
    Abandoned,
}

/// 目標依賴關係類型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DependencyRelation {
    /// 前置解鎖依賴：前置目標達成後，目標從 Inactive 解鎖為 Active。
    Prerequisite,
    /// 複合與（Composite AND）：所有子目標皆達成後，父目標自動達成。
    CompositeAnd,
    /// 複合或（Composite OR）：任一子目標達成後，父目標即達成。
    CompositeOr,
    /// 互斥分支（Mutually Exclusive）：來源目標一旦達成，目標將被標記為 Abandoned。
    MutuallyExclusive,
}

/// 整體戰役狀態評估結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum CampaignResult {
    /// 戰役持續進行中。
    #[default]
    InProgress,
    /// 主線目標全數達成，戰役勝利！
    Victory,
    /// 觸發失敗條件或關鍵主線失敗，戰役失敗！
    Defeat,
}

/// 區域邊界定義：支援矩形世界坐標與圓形範圍定義。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectiveAreaBounds {
    pub min_x: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_z: i32,
    pub center_x: f32,
    pub center_z: f32,
    pub radius: f32,
    pub is_circle: bool,
}

impl Default for ObjectiveAreaBounds {
    fn default() -> Self {
        Self {
            min_x: 0,
            min_z: 0,
            max_x: 16383,
            max_z: 16383,
            center_x: 8192.0,
            center_z: 8192.0,
            radius: 0.0,
            is_circle: false,
        }
    }
}

impl ObjectiveAreaBounds {
    /// 建立標準矩形區域。
    pub fn from_rectangle(min_x: i32, min_z: i32, max_x: i32, max_z: i32) -> Self {
        let actual_min_x = min_x.min(max_x);
        let actual_min_z = min_z.min(max_z);
        let actual_max_x = min_x.max(max_x);
        let actual_max_z = min_z.max(max_z);
        Self {
            min_x: actual_min_x,
            min_z: actual_min_z,
            max_x: actual_max_x,
            max_z: actual_max_z,
            center_x: (actual_min_x + actual_max_x) as f32 * 0.5,
            center_z: (actual_min_z + actual_max_z) as f32 * 0.5,
            radius: 0.0,
            is_circle: false,
        }
    }

    /// 建立圓形半徑區域（內部自動計算對應的外接矩形供原生引擎比對）。
    pub fn from_circle(center_x: f32, center_z: f32, radius: f32) -> Self {
        let min_x = (center_x - radius).clamp(0.0, 16383.0) as i32;
        let max_x = (center_x + radius).clamp(0.0, 16383.0) as i32;
        let min_z = (center_z - radius).clamp(0.0, 16383.0) as i32;
        let max_z = (center_z + radius).clamp(0.0, 16383.0) as i32;
        Self {
            min_x,
            min_z,
            max_x,
            max_z,
            center_x,
            center_z,
            radius,
            is_circle: true,
        }
    }

    /// 檢查指定坐標是否位於區域內。
    pub fn contains(&self, x: f32, z: f32) -> bool {
        let xi = x as i32;
        let zi = z as i32;
        if xi < self.min_x || xi > self.max_x || zi < self.min_z || zi > self.max_z {
            return false;
        }
        if !self.is_circle || self.radius <= 0.0 {
            return true;
        }
        let dx = x - self.center_x;
        let dz = z - self.center_z;
        (dx * dx + dz * dz) <= (self.radius * self.radius)
    }
}

/// 目標完成時發放的自訂獎勵或動作序列。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObjectiveReward {
    pub completion_message: String,
    pub extra_actions: Vec<Action>,
}

impl ObjectiveReward {
    pub fn new(completion_message: impl Into<String>, extra_actions: Vec<Action>) -> Self {
        Self {
            completion_message: completion_message.into(),
            extra_actions,
        }
    }

    pub fn actions(&self) -> &[Action] {
        &self.extra_actions
    }
}

/// 戰役目標規則參數模型。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectiveRuleParameters {
    /// 目標物件 GUID 清單（英雄、建築、VIP、守護標的）。
    pub target_guids: Vec<Uuid>,
    /// 防守或維持佔領秒數（例如 Survival 需撐過 180 秒，或佔領需維持 30 秒）。
    pub hold_duration_seconds: i32,
    /// 目的區域或佔領區域範圍。
    pub area: Option<ObjectiveAreaBounds>,
    /// 玩家所屬隊伍編號（0–7，預設 0）。
    pub player_team: i32,
    /// 敵方或目標所屬隊伍編號（0–7，預設 1）。
    pub target_team: i32,
    /// 時限秒數（0 代表無限制，逾時將導致目標判定失敗）。
    pub time_limit_seconds: i32,
    /// 需求數量（例如需消滅之小隊數、或佔領點數）。
    pub required_count: i32,
    /// 自訂擴充原生條件組。
    pub custom_conditions: Vec<Condition>,
    /// 擴充自訂資料（如自訂標籤或腳本變數名稱）。
    pub custom_data: String,
}

impl Default for ObjectiveRuleParameters {
    fn default() -> Self {
        Self {
            target_guids: vec![],
            hold_duration_seconds: 0,
            area: None,
            player_team: 0,
            target_team: 1,
            time_limit_seconds: 0,
            required_count: 1,
            custom_conditions: vec![],
            custom_data: String::new(),
        }
    }
}

/// 目標依賴連線定義。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectiveDependency {
    pub source_objective_id: Uuid,
    pub target_objective_id: Uuid,
    pub relation: DependencyRelation,
}

impl ObjectiveDependency {
    pub fn new(
        source_objective_id: Uuid,
        target_objective_id: Uuid,
        relation: DependencyRelation,
    ) -> Self {
        Self {
            source_objective_id,
            target_objective_id,
            relation,
        }
    }
}

/// 戰役目標核心定義模型。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectiveDefinition {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub kind: ObjectiveKind,
    pub category: ObjectiveCategory,
    pub parameters: ObjectiveRuleParameters,
    pub reward: Option<ObjectiveReward>,
    pub initial_state: ObjectiveState,
    pub ui_order: f32,
}

impl Default for ObjectiveDefinition {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            title: "New Objective".to_string(),
            description: String::new(),
            kind: ObjectiveKind::EliminateAllEnemies,
            category: ObjectiveCategory::Primary,
            parameters: ObjectiveRuleParameters::default(),
            reward: None,
            initial_state: ObjectiveState::Active,
            ui_order: 0.0,
        }
    }
}
