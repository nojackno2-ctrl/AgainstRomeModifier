use super::models::*;
use std::collections::HashSet;
use uuid::Uuid;

/// 規則驗證結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectiveValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
}

impl ObjectiveValidationResult {
    pub fn success() -> Self {
        Self {
            is_valid: true,
            errors: vec![],
        }
    }

    pub fn failure(errors: Vec<String>) -> Self {
        Self {
            is_valid: false,
            errors,
        }
    }
}

/// 目標規則類型描述元。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectiveRuleDescriptor {
    pub kind: ObjectiveKind,
    pub display_name: &'static str,
    pub description: &'static str,
    pub requires_target_guid: bool,
    pub requires_area: bool,
    pub supports_hold_duration: bool,
    pub supports_time_limit: bool,
    pub default_category: ObjectiveCategory,
}

/// 戰役目標規則型錄（ObjectiveRuleCatalog）
pub struct ObjectiveRuleCatalog;

impl ObjectiveRuleCatalog {
    pub fn get_descriptor(kind: ObjectiveKind) -> Result<ObjectiveRuleDescriptor, String> {
        let desc = match kind {
            ObjectiveKind::EliminateAllEnemies => ObjectiveRuleDescriptor {
                kind,
                display_name: "全殲敵軍 (Eliminate Enemies)",
                description: "消滅指定敵方勢力的所有部隊或摧毀其主要軍事指揮中心。",
                requires_target_guid: false,
                requires_area: false,
                supports_hold_duration: false,
                supports_time_limit: true,
                default_category: ObjectiveCategory::Primary,
            },
            ObjectiveKind::Survival => ObjectiveRuleDescriptor {
                kind,
                display_name: "堅守陣地倒數計時 (Survival)",
                description: "守護關鍵基地或指揮官，在敵軍猛烈圍攻下堅守至計時歸零。",
                requires_target_guid: true,
                requires_area: false,
                supports_hold_duration: true,
                supports_time_limit: false,
                default_category: ObjectiveCategory::Primary,
            },
            ObjectiveKind::CaptureArea => ObjectiveRuleDescriptor {
                kind,
                display_name: "佔領特定區域 (Capture Area)",
                description: "匯出僅支援指定物件位於矩形區域（維持秒數為 0）；不檢查敵軍或隊伍佔領。",
                requires_target_guid: false,
                requires_area: true,
                supports_hold_duration: true,
                supports_time_limit: true,
                default_category: ObjectiveCategory::Primary,
            },
            ObjectiveKind::KingOfTheHill => ObjectiveRuleDescriptor {
                kind,
                display_name: "山丘之王 (King of the Hill)",
                description: "爭奪地圖中央樞紐據點，在限定時間內累計佔領時長率先達成者獲勝。",
                requires_target_guid: false,
                requires_area: true,
                supports_hold_duration: true,
                supports_time_limit: true,
                default_category: ObjectiveCategory::Primary,
            },
            ObjectiveKind::AssassinateTarget => ObjectiveRuleDescriptor {
                kind,
                display_name: "刺殺敵方英雄 (Assassinate Hero)",
                description: "擊殺敵軍特定將領、羅馬百夫長、祭司或酋長單位。",
                requires_target_guid: true,
                requires_area: false,
                supports_hold_duration: false,
                supports_time_limit: true,
                default_category: ObjectiveCategory::Primary,
            },
            ObjectiveKind::DestroyBuilding => ObjectiveRuleDescriptor {
                kind,
                display_name: "破壞關鍵建築 (Destroy Building)",
                description: "摧毀敵軍關鍵軍事建築、神廟要塞或補給倉庫。",
                requires_target_guid: true,
                requires_area: false,
                supports_hold_duration: false,
                supports_time_limit: true,
                default_category: ObjectiveCategory::Primary,
            },
            ObjectiveKind::EscortUnit => ObjectiveRuleDescriptor {
                kind,
                display_name: "護送商隊 (Escort Caravan)",
                description: "護送商隊或重要盟軍 VIP 單位安全抵達撤離區域，期間目標不可陣亡。",
                requires_target_guid: true,
                requires_area: true,
                supports_hold_duration: false,
                supports_time_limit: true,
                default_category: ObjectiveCategory::Primary,
            },
            ObjectiveKind::CustomScripted => ObjectiveRuleDescriptor {
                kind,
                display_name: "自訂複合腳本 (Custom Scripted)",
                description: "自由組合原生物件存在、陣亡或區域條件之高階目標。",
                requires_target_guid: false,
                requires_area: false,
                supports_hold_duration: true,
                supports_time_limit: true,
                default_category: ObjectiveCategory::Bonus,
            },
        };
        Ok(desc)
    }

    pub fn all_descriptors() -> Vec<ObjectiveRuleDescriptor> {
        vec![
            Self::get_descriptor(ObjectiveKind::EliminateAllEnemies).unwrap(),
            Self::get_descriptor(ObjectiveKind::Survival).unwrap(),
            Self::get_descriptor(ObjectiveKind::CaptureArea).unwrap(),
            Self::get_descriptor(ObjectiveKind::KingOfTheHill).unwrap(),
            Self::get_descriptor(ObjectiveKind::AssassinateTarget).unwrap(),
            Self::get_descriptor(ObjectiveKind::DestroyBuilding).unwrap(),
            Self::get_descriptor(ObjectiveKind::EscortUnit).unwrap(),
            Self::get_descriptor(ObjectiveKind::CustomScripted).unwrap(),
        ]
    }

    /// 驗證目標定義之參數完整性與邏輯有效性。
    pub fn validate(definition: &ObjectiveDefinition) -> ObjectiveValidationResult {
        let mut errors = vec![];

        if definition.title.trim().is_empty() {
            errors.push("目標標題不可為空。".to_string());
        }
        if definition.title.chars().count() > 120 {
            errors.push("目標標題長度上限為 120 字元。".to_string());
        }

        let descriptor = match Self::get_descriptor(definition.kind) {
            Ok(d) => d,
            Err(_) => return ObjectiveValidationResult::failure(vec!["Unknown objective kind.".into()]),
        };

        let param = &definition.parameters;
        if !(0..=7).contains(&param.player_team) {
            errors.push("玩家隊伍編號必須介於 0–7 之間。".to_string());
        }
        if !(0..=7).contains(&param.target_team) {
            errors.push("目標隊伍編號必須介於 0–7 之間。".to_string());
        }

        if descriptor.requires_target_guid {
            if param.target_guids.is_empty() || param.target_guids.iter().any(|g| g.is_nil()) {
                errors.push(format!(
                    "規則「{}」需要至少一個有效的目標物件 GUID。",
                    descriptor.display_name
                ));
            }
        }

        if descriptor.requires_area {
            if let Some(ref area) = param.area {
                if area.min_x < 0
                    || area.min_z < 0
                    || area.max_x > 16383
                    || area.max_z > 16383
                    || area.min_x > area.max_x
                    || area.min_z > area.max_z
                {
                    errors.push("區域邊界必須介於世界座標 0–16383 且 Min 必須不大於 Max。".to_string());
                }
            } else {
                errors.push(format!(
                    "規則「{}」必須設定有效的目標區域範圍。",
                    descriptor.display_name
                ));
            }
        }

        if descriptor.supports_hold_duration && param.hold_duration_seconds < 0 {
            errors.push("維持秒數不可為負數。".to_string());
        }

        if descriptor.supports_time_limit && param.time_limit_seconds < 0 {
            errors.push("時限秒數不可為負數。".to_string());
        }

        if definition.kind == ObjectiveKind::Survival
            && definition.category != ObjectiveCategory::FailureCriterion
            && param.hold_duration_seconds <= 0
        {
            errors.push("堅守陣地目標（Survival）必須指定大於 0 的堅守秒數。".to_string());
        }

        if definition.kind == ObjectiveKind::CaptureArea && param.hold_duration_seconds < 0 {
            errors.push("佔領目標維持秒數不可為負數。".to_string());
        }

        if errors.is_empty() {
            ObjectiveValidationResult::success()
        } else {
            ObjectiveValidationResult::failure(errors)
        }
    }

    /// Export eligibility, separate from the offline graph/sandbox parameter validation.
    pub fn validate_for_compilation(definition: &ObjectiveDefinition) -> ObjectiveValidationResult {
        let mut errors = vec![];
        let p = &definition.parameters;

        if definition.id.is_nil() || definition.initial_state != ObjectiveState::Active {
            errors.push("Export requires a persistent ID and an initially active objective.".to_string());
        }
        if matches!(
            definition.kind,
            ObjectiveKind::EliminateAllEnemies | ObjectiveKind::KingOfTheHill
        ) {
            errors.push("Team-wide elimination and accumulated area control have no verified ScenarioEvent mapping.".to_string());
        }
        if p.time_limit_seconds != 0 || p.required_count != 1 || !p.custom_data.is_empty() {
            errors.push("Timeouts, kill/resource counts and custom data are simulation-only and cannot be silently ignored.".to_string());
        }
        if let Some(ref area) = p.area {
            if area.is_circle {
                errors.push("Only rectangular areas are supported; circles cannot be approximated by their bounding box.".to_string());
            }
        }
        if definition.category == ObjectiveCategory::FailureCriterion
            && definition.kind != ObjectiveKind::Survival
        {
            errors.push("Only protected-object loss is supported as a failure criterion.".to_string());
        }
        if matches!(
            definition.kind,
            ObjectiveKind::Survival | ObjectiveKind::EscortUnit
        ) && definition.category == ObjectiveCategory::Bonus
        {
            errors.push("Bonus survival/escort needs a failed-state latch to suppress later rewards; unsupported.".to_string());
        }
        if matches!(
            definition.kind,
            ObjectiveKind::Survival | ObjectiveKind::EscortUnit | ObjectiveKind::CaptureArea
        ) {
            if p.target_guids.len() != 1 || p.target_guids[0].is_nil() {
                errors.push("This rule requires exactly one explicit target GUID.".to_string());
            }
        }
        let unique_targets: HashSet<_> = p.target_guids.iter().collect();
        if p.target_guids.len() > 32
            || p.target_guids.iter().any(|id| id.is_nil())
            || unique_targets.len() != p.target_guids.len()
        {
            errors.push("Target lists must contain at most 32 distinct, nonempty GUIDs; no truncation is allowed.".to_string());
        }
        if definition.kind != ObjectiveKind::Survival && p.hold_duration_seconds != 0 {
            errors.push("Area dwell/accumulation and custom hold timers are unsupported; DelaySeconds is a level-start deadline, not time spent satisfying a condition.".to_string());
        }
        if p.hold_duration_seconds > 86400 {
            errors.push("Survival duration cannot exceed the existing compiler limit of 86400 seconds.".to_string());
        }
        if definition.category == ObjectiveCategory::FailureCriterion
            && (p.hold_duration_seconds != 0 || definition.reward.is_some())
        {
            errors.push("A protected-object failure criterion cannot have a completion timer or reward.".to_string());
        }
        if definition.kind == ObjectiveKind::CustomScripted {
            if !(1..=32).contains(&p.custom_conditions.len())
                || !p.target_guids.is_empty()
                || p.area.is_some()
            {
                errors.push("Custom objectives require 1-32 explicit ScenarioConditions and no unused targets/area.".to_string());
            }
            for c in &p.custom_conditions {
                if !(0..=2).contains(&c.kind) {
                    errors.push("Unsupported condition kind.".to_string());
                }
            }
        } else if !p.custom_conditions.is_empty() {
            errors.push("Custom conditions are only supported by CustomScripted objectives.".to_string());
        }
        if p.area.is_some()
            && !matches!(
                definition.kind,
                ObjectiveKind::EscortUnit | ObjectiveKind::CaptureArea
            )
        {
            errors.push("This objective kind does not use an area.".to_string());
        }
        if let Some(ref reward) = definition.reward {
            if reward.actions().iter().any(|a| a.kind == 3 || a.kind == 4) {
                errors.push("Reward actions cannot terminate the mission; terminal results are owned by the objective compiler.".to_string());
            }
        }

        if errors.is_empty() {
            ObjectiveValidationResult::success()
        } else {
            ObjectiveValidationResult::failure(errors)
        }
    }

    // Standard Template Factories

    pub fn create_eliminate_enemies(
        title: impl Into<String>,
        description: impl Into<String>,
        target_team: i32,
        player_team: i32,
        time_limit_seconds: i32,
        category: ObjectiveCategory,
        reward: Option<ObjectiveReward>,
    ) -> ObjectiveDefinition {
        ObjectiveDefinition {
            id: Uuid::new_v4(),
            title: title.into(),
            description: description.into(),
            kind: ObjectiveKind::EliminateAllEnemies,
            category,
            reward,
            parameters: ObjectiveRuleParameters {
                player_team,
                target_team,
                time_limit_seconds,
                ..Default::default()
            },
            initial_state: ObjectiveState::Active,
            ui_order: 0.0,
        }
    }

    pub fn create_survival(
        title: impl Into<String>,
        description: impl Into<String>,
        defend_target_guid: Uuid,
        hold_duration_seconds: i32,
        player_team: i32,
        target_team: i32,
        category: ObjectiveCategory,
        reward: Option<ObjectiveReward>,
    ) -> ObjectiveDefinition {
        ObjectiveDefinition {
            id: Uuid::new_v4(),
            title: title.into(),
            description: description.into(),
            kind: ObjectiveKind::Survival,
            category,
            reward,
            parameters: ObjectiveRuleParameters {
                target_guids: vec![defend_target_guid],
                hold_duration_seconds,
                player_team,
                target_team,
                ..Default::default()
            },
            initial_state: ObjectiveState::Active,
            ui_order: 0.0,
        }
    }

    pub fn create_capture_area(
        title: impl Into<String>,
        description: impl Into<String>,
        area: ObjectiveAreaBounds,
        hold_duration_seconds: i32,
        player_team: i32,
        time_limit_seconds: i32,
        category: ObjectiveCategory,
        reward: Option<ObjectiveReward>,
        target_guid: Option<Uuid>,
    ) -> ObjectiveDefinition {
        ObjectiveDefinition {
            id: Uuid::new_v4(),
            title: title.into(),
            description: description.into(),
            kind: ObjectiveKind::CaptureArea,
            category,
            reward,
            parameters: ObjectiveRuleParameters {
                target_guids: target_guid.map(|g| vec![g]).unwrap_or_default(),
                area: Some(area),
                hold_duration_seconds,
                player_team,
                time_limit_seconds,
                ..Default::default()
            },
            initial_state: ObjectiveState::Active,
            ui_order: 0.0,
        }
    }

    pub fn create_assassinate_target(
        title: impl Into<String>,
        description: impl Into<String>,
        target_hero_guid: Uuid,
        target_team: i32,
        player_team: i32,
        time_limit_seconds: i32,
        category: ObjectiveCategory,
        reward: Option<ObjectiveReward>,
    ) -> ObjectiveDefinition {
        ObjectiveDefinition {
            id: Uuid::new_v4(),
            title: title.into(),
            description: description.into(),
            kind: ObjectiveKind::AssassinateTarget,
            category,
            reward,
            parameters: ObjectiveRuleParameters {
                target_guids: vec![target_hero_guid],
                target_team,
                player_team,
                time_limit_seconds,
                ..Default::default()
            },
            initial_state: ObjectiveState::Active,
            ui_order: 0.0,
        }
    }

    pub fn create_destroy_building(
        title: impl Into<String>,
        description: impl Into<String>,
        building_guid: Uuid,
        target_team: i32,
        player_team: i32,
        time_limit_seconds: i32,
        category: ObjectiveCategory,
        reward: Option<ObjectiveReward>,
    ) -> ObjectiveDefinition {
        ObjectiveDefinition {
            id: Uuid::new_v4(),
            title: title.into(),
            description: description.into(),
            kind: ObjectiveKind::DestroyBuilding,
            category,
            reward,
            parameters: ObjectiveRuleParameters {
                target_guids: vec![building_guid],
                target_team,
                player_team,
                time_limit_seconds,
                ..Default::default()
            },
            initial_state: ObjectiveState::Active,
            ui_order: 0.0,
        }
    }

    pub fn create_escort_unit(
        title: impl Into<String>,
        description: impl Into<String>,
        vip_guid: Uuid,
        dest_area: ObjectiveAreaBounds,
        time_limit_seconds: i32,
        player_team: i32,
        category: ObjectiveCategory,
        reward: Option<ObjectiveReward>,
    ) -> ObjectiveDefinition {
        ObjectiveDefinition {
            id: Uuid::new_v4(),
            title: title.into(),
            description: description.into(),
            kind: ObjectiveKind::EscortUnit,
            category,
            reward,
            parameters: ObjectiveRuleParameters {
                target_guids: vec![vip_guid],
                area: Some(dest_area),
                time_limit_seconds,
                player_team,
                ..Default::default()
            },
            initial_state: ObjectiveState::Active,
            ui_order: 0.0,
        }
    }

    pub fn create_defend_failure_criterion(
        title: impl Into<String>,
        description: impl Into<String>,
        critical_asset_guid: Uuid,
        player_team: i32,
    ) -> ObjectiveDefinition {
        ObjectiveDefinition {
            id: Uuid::new_v4(),
            title: title.into(),
            description: description.into(),
            kind: ObjectiveKind::Survival,
            category: ObjectiveCategory::FailureCriterion,
            parameters: ObjectiveRuleParameters {
                target_guids: vec![critical_asset_guid],
                player_team,
                ..Default::default()
            },
            initial_state: ObjectiveState::Active,
            reward: None,
            ui_order: 0.0,
        }
    }
}
