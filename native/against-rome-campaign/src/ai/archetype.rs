use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// AI 接戰與遭遇規則
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngagementRule {
    /// 視線內主動攻擊任何敵人
    AttackOnSight,
    /// 僅在領地/警戒範圍內交戰，敵軍逃離則放棄追擊
    DefendTerritory,
    /// 受到攻擊前保持中立被動
    PassiveUntilAttacked,
    /// 遭攻擊後持續反擊直到威脅消除
    RetaliateOnly,
}

/// 戰術姿態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TacticalPosture {
    /// 主動突擊衝鋒
    AggressiveRush,
    /// 均衡推進與陣形掩護
    BalancedFormation,
    /// 固守原地與盾牆防線
    DefensiveHold,
    /// 游擊拉扯與騎射騷擾
    HitAndRun,
}

/// 目標優先級
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetPriority {
    /// 距離最近的敵對目標
    NearestEnemy,
    /// 玩家聚落主建築（城鎮中心）
    PlayerSettlementCenter,
    /// 經濟生產與資源採集點
    EconomicStructures,
    /// 落單或脆弱部隊
    WeakestSquad,
    /// 重型防禦塔與軍事哨站
    MilitaryStructures,
}

/// 兵種偏好與權重
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitPreferenceWeight {
    pub alias: String,
    pub weight: i32,
    pub min_count: i32,
    pub max_count: i32,
}

impl UnitPreferenceWeight {
    pub fn new(alias: impl Into<String>, weight: i32, min_count: i32, max_count: i32) -> Self {
        Self {
            alias: alias.into(),
            weight,
            min_count,
            max_count,
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.alias.trim().is_empty()
            && (1..=100).contains(&self.weight)
            && (1..=20).contains(&self.min_count)
            && self.max_count >= self.min_count
            && self.max_count <= 20
    }
}

/// AI 戰略原型設定檔
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiArchetypeProfile {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub preferred_tribe: String,
    pub aggressiveness: f32,
    pub expansion_desire: f32,
    pub economic_focus: f32,
    pub defense_priority: f32,
    pub patrol_radius: f32,
    pub retreat_health_ratio: f32,
    pub min_raid_interval_seconds: i32,
    pub max_raid_interval_seconds: i32,
    pub garrison_cap: i32,
    pub engagement_rule: EngagementRule,
    pub tactical_posture: TacticalPosture,
    pub target_priority: TargetPriority,
    pub unit_preferences: Vec<UnitPreferenceWeight>,
}

impl AiArchetypeProfile {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("AI 原型 ID 不能為空。".into());
        }
        if self.display_name.trim().is_empty() {
            return Err("AI 原型名稱不能為空。".into());
        }
        if !(0.0..=1.0).contains(&self.aggressiveness) {
            return Err("侵略度必須介於 0.0 與 1.0。".into());
        }
        if !(0.0..=1.0).contains(&self.expansion_desire) {
            return Err("擴張欲必須介於 0.0 與 1.0。".into());
        }
        if !(0.0..=1.0).contains(&self.economic_focus) {
            return Err("經濟重心必須介於 0.0 與 1.0。".into());
        }
        if !(0.0..=1.0).contains(&self.defense_priority) {
            return Err("防守優先級必須介於 0.0 與 1.0。".into());
        }
        if !(100.0..=16384.0).contains(&self.patrol_radius) {
            return Err("巡邏警戒半徑必須介於 100 與 16384。".into());
        }
        if !(0.0..=0.8).contains(&self.retreat_health_ratio) {
            return Err("撤退血量比率必須介於 0.0 與 0.8。".into());
        }
        if self.min_raid_interval_seconds < 5
            || self.max_raid_interval_seconds < self.min_raid_interval_seconds
        {
            return Err("突襲間隔設定不合法。".into());
        }
        if !(0..=100).contains(&self.garrison_cap) {
            return Err("駐軍上限必須介於 0 與 100。".into());
        }
        for pref in &self.unit_preferences {
            if !pref.is_valid() {
                return Err(format!("兵種權重設定不合法：{}", pref.alias));
            }
        }
        Ok(())
    }
}

/// 勢力指派的 AI 設定
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactionAiProfile {
    pub team: i32,
    pub archetype_id: String,
    pub custom_overrides: Option<AiArchetypeProfile>,
    pub base_settlement_id: Option<Uuid>,
    pub enabled: bool,
}

impl FactionAiProfile {
    pub fn new(team: i32, archetype_id: impl Into<String>) -> Self {
        Self {
            team,
            archetype_id: archetype_id.into(),
            custom_overrides: None,
            base_settlement_id: None,
            enabled: true,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(0..=7).contains(&self.team) {
            return Err("隊伍編號必須介於 0 與 7。".into());
        }
        if self.archetype_id.trim().is_empty() {
            return Err("勢力 AI 必須指定原型 ID。".into());
        }
        if let Some(overrides) = &self.custom_overrides {
            overrides.validate()?;
        }
        Ok(())
    }
}

/// 標準 AI 戰略原型目錄
pub struct AiArchetypeCatalog;

impl AiArchetypeCatalog {
    pub const ID_NOMADIC_RAIDER: &'static str = "NomadicRaider";
    pub const ID_ROMAN_FORTRESS: &'static str = "RomanFortress";
    pub const ID_GERMANIC_SETTLEMENT: &'static str = "GermanicSettlement";
    pub const ID_BARBARIAN_OUTPOST: &'static str = "BarbarianOutpost";

    pub fn all() -> Vec<AiArchetypeProfile> {
        vec![
            Self::create_nomadic_raider(),
            Self::create_roman_fortress(),
            Self::create_germanic_settlement(),
            Self::create_barbarian_outpost(),
        ]
    }

    pub fn find(id: &str) -> Option<AiArchetypeProfile> {
        if id.trim().is_empty() {
            return None;
        }
        if id.eq_ignore_ascii_case(Self::ID_NOMADIC_RAIDER) {
            Some(Self::create_nomadic_raider())
        } else if id.eq_ignore_ascii_case(Self::ID_ROMAN_FORTRESS) {
            Some(Self::create_roman_fortress())
        } else if id.eq_ignore_ascii_case(Self::ID_GERMANIC_SETTLEMENT) {
            Some(Self::create_germanic_settlement())
        } else if id.eq_ignore_ascii_case(Self::ID_BARBARIAN_OUTPOST) {
            Some(Self::create_barbarian_outpost())
        } else {
            None
        }
    }

    pub fn get(id: &str) -> Result<AiArchetypeProfile, String> {
        Self::find(id).ok_or_else(|| format!("找不到指定的 AI 戰略原型：{id}"))
    }

    pub fn create_nomadic_raider() -> AiArchetypeProfile {
        AiArchetypeProfile {
            id: Self::ID_NOMADIC_RAIDER.to_string(),
            display_name: "進攻型遊牧部落".to_string(),
            description: "高度侵略性的遊牧掠奪者。偏好高速騎兵與騎射騷擾，定時組織突襲掠奪邊界聚落，不注重固定防禦建築，遭遇劣勢時快速撤退脫離。".to_string(),
            preferred_tribe: "Hun".to_string(),
            aggressiveness: 0.90,
            expansion_desire: 0.30,
            economic_focus: 0.15,
            defense_priority: 0.20,
            patrol_radius: 3500.0,
            retreat_health_ratio: 0.35,
            min_raid_interval_seconds: 45,
            max_raid_interval_seconds: 90,
            garrison_cap: 4,
            engagement_rule: EngagementRule::AttackOnSight,
            tactical_posture: TacticalPosture::HitAndRun,
            target_priority: TargetPriority::EconomicStructures,
            unit_preferences: vec![
                UnitPreferenceWeight::new("HUN_KAVINF00", 60, 5, 15),
                UnitPreferenceWeight::new("HUN_KAVSCH00", 40, 5, 10),
            ],
        }
    }

    pub fn create_roman_fortress() -> AiArchetypeProfile {
        AiArchetypeProfile {
            id: Self::ID_ROMAN_FORTRESS.to_string(),
            display_name: "防守型羅馬堡壘".to_string(),
            description: "以軍事要塞與防衛為重心的羅馬軍團。維持龐大軍團駐軍、箭塔防衛網，陣形堅固。僅在領地受到威脅時出動重裝步兵迎擊，並執行組織化反擊。".to_string(),
            preferred_tribe: "Roman".to_string(),
            aggressiveness: 0.25,
            expansion_desire: 0.20,
            economic_focus: 0.50,
            defense_priority: 0.95,
            patrol_radius: 2200.0,
            retreat_health_ratio: 0.15,
            min_raid_interval_seconds: 120,
            max_raid_interval_seconds: 240,
            garrison_cap: 20,
            engagement_rule: EngagementRule::DefendTerritory,
            tactical_posture: TacticalPosture::DefensiveHold,
            target_priority: TargetPriority::MilitaryStructures,
            unit_preferences: vec![
                UnitPreferenceWeight::new("ROM_INF00", 50, 10, 20),
                UnitPreferenceWeight::new("ROM_INF01", 30, 8, 15),
                UnitPreferenceWeight::new("ROM_SCH00", 20, 6, 12),
            ],
        }
    }

    pub fn create_germanic_settlement() -> AiArchetypeProfile {
        AiArchetypeProfile {
            id: Self::ID_GERMANIC_SETTLEMENT.to_string(),
            display_name: "經濟擴張型日耳曼聚落".to_string(),
            description: "注重人口增長、農耕伐木與資源儲備的日耳曼/凱爾特部落。優先發展聚落建築，組建民兵護送隊，並具備向外建立分聚落與拓荒的戰略傾向。".to_string(),
            preferred_tribe: "German".to_string(),
            aggressiveness: 0.45,
            expansion_desire: 0.85,
            economic_focus: 0.90,
            defense_priority: 0.55,
            patrol_radius: 2800.0,
            retreat_health_ratio: 0.25,
            min_raid_interval_seconds: 150,
            max_raid_interval_seconds: 300,
            garrison_cap: 12,
            engagement_rule: EngagementRule::RetaliateOnly,
            tactical_posture: TacticalPosture::BalancedFormation,
            target_priority: TargetPriority::NearestEnemy,
            unit_preferences: vec![
                UnitPreferenceWeight::new("GER_INF00", 45, 6, 16),
                UnitPreferenceWeight::new("GER_INF01", 35, 6, 14),
                UnitPreferenceWeight::new("GER_KAVINF00", 20, 4, 10),
            ],
        }
    }

    pub fn create_barbarian_outpost() -> AiArchetypeProfile {
        AiArchetypeProfile {
            id: Self::ID_BARBARIAN_OUTPOST.to_string(),
            display_name: "中立野蠻人巡邏哨".to_string(),
            description: "地圖中立或敵對哨站。部隊沿固定路徑點循環巡邏，守護周邊寶箱或隘口；攻擊進入警戒範圍的任何勢力，超出脫離範圍時自動回防巡邏起點。".to_string(),
            preferred_tribe: "Neutral".to_string(),
            aggressiveness: 0.60,
            expansion_desire: 0.00,
            economic_focus: 0.00,
            defense_priority: 0.80,
            patrol_radius: 1600.0,
            retreat_health_ratio: 0.10,
            min_raid_interval_seconds: 60,
            max_raid_interval_seconds: 120,
            garrison_cap: 6,
            engagement_rule: EngagementRule::DefendTerritory,
            tactical_posture: TacticalPosture::AggressiveRush,
            target_priority: TargetPriority::NearestEnemy,
            unit_preferences: vec![
                UnitPreferenceWeight::new("GER_INF01", 60, 4, 10),
                UnitPreferenceWeight::new("GER_SCH00", 40, 4, 8),
            ],
        }
    }
}
