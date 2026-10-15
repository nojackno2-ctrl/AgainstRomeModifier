use crate::text_encoding;
use against_rome_scenario::model::{Event, ScenarioDocument};
use std::collections::HashSet;

/// 事件清單驗證邏輯（純邏輯，對應 C# `ScenarioEventValidator`）。
pub struct ScenarioEventValidator;

impl ScenarioEventValidator {
    pub fn validate(events: &[Event], aliases: &[String]) -> Result<(), String> {
        if events.len() > 256 {
            return Err("事件上限為 256 個。".into());
        }
        Self::validate_conditions(events, None)?;
        Self::validate_terminal_actions(events)?;
        Self::validate_orders(events)?;

        for item in events {
            if item.name.trim().is_empty() || item.name.chars().count() > 100 {
                return Err("事件名稱必須是 1–100 個字元。".into());
            }
            if !(0..=86400).contains(&item.delay_seconds) || (item.repeat && item.delay_seconds == 0) {
                return Err("計時必須介於 0 與 86400 秒；重複事件至少間隔 1 秒。".into());
            }
            if item.actions.is_empty() || item.actions.len() > 32 {
                return Err("每個事件必須有 1–32 個動作。".into());
            }
            for action in &item.actions {
                if !(0..=4).contains(&action.kind) {
                    return Err("不支援的事件動作。".into());
                }
                match action.kind {
                    0 => {
                        // Message
                        if action.text.trim().is_empty()
                            || action.text.contains('\0')
                            || !text_encoding::can_encode_game_text(&action.text)
                            || text_encoding::game_text_byte_count(&action.text) > 1000
                        {
                            return Err("訊息必須是遊戲支援的文字，長度上限為 1000 位元組。".into());
                        }
                    }
                    1 => {
                        // Diplomacy
                        if !(0..=7).contains(&action.team)
                            || !(0..=7).contains(&action.other_team)
                            || action.team == action.other_team
                        {
                            return Err("外交需要兩個不同隊伍（0–7）。".into());
                        }
                    }
                    2 => {
                        // SpawnUnit
                        if !(0..=7).contains(&action.team)
                            || !(1..=20).contains(&action.count)
                            || !action.x.is_finite()
                            || !action.z.is_finite()
                            || !(0.0..=16383.0).contains(&action.x)
                            || !(0.0..=16383.0).contains(&action.z)
                        {
                            return Err("部隊需要隊伍 0–7、人數 1–20 與有效的地圖座標。".into());
                        }
                        if !aliases
                            .iter()
                            .any(|a| a.eq_ignore_ascii_case(&action.alias))
                        {
                            return Err(format!("未知的部隊別名：{}", action.alias));
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    pub fn validate_terminal_actions(events: &[Event]) -> Result<(), String> {
        for item in events {
            let terminal = item
                .actions
                .iter()
                .position(|action| action.kind == 3 || action.kind == 4);
            if let Some(idx) = terminal {
                if item.repeat || idx != item.actions.len() - 1 {
                    return Err("勝敗動作必須是單次事件的最後一個動作。".into());
                }
            }
        }
        Ok(())
    }

    pub fn validate_orders(events: &[Event]) -> Result<(), String> {
        for item in events {
            for action in &item.actions {
                if let Some(dest) = &action.move_destination {
                    if action.kind != 2
                        || dest.x < 0
                        || dest.x > 16383
                        || dest.z < 0
                        || dest.z > 16383
                    {
                        return Err(
                            "移動目的地僅適用於生成部隊，X/Z 必須介於 0 與 16383。".into()
                        );
                    }
                }
            }
        }
        Ok(())
    }

    pub fn validate_conditions(
        events: &[Event],
        scenario: Option<&ScenarioDocument>,
    ) -> Result<(), String> {
        for item in events {
            if item.conditions.len() > 32 {
                return Err("每個事件最多有 32 個條件。".into());
            }
            for condition in &item.conditions {
                if !(0..=2).contains(&condition.kind) || condition.target_id.is_nil() {
                    return Err("物件條件需要有效種類與目標 ID。".into());
                }
                if condition.kind == 2
                    && (condition.min_x < 0
                        || condition.min_z < 0
                        || condition.max_x > 16383
                        || condition.max_z > 16383
                        || condition.min_x > condition.max_x
                        || condition.min_z > condition.max_z)
                {
                    return Err("區域需要有序且介於 0–16383 的 X/Z 邊界。".into());
                }
                if let Some(sc) = scenario {
                    if !item.enabled {
                        continue;
                    }
                    let target = sc.spawns.iter().find(|s| s.id == condition.target_id);
                    if target.is_none() {
                        return Err(format!(
                            "事件「{}」的目標物件已刪除，請重新選擇或停用事件。",
                            item.name
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

/// 在開始檔案交易之前檢查可由編輯快照確定的錯誤；DATA 綁定於重建後再驗證。
pub struct ScenarioSavePreflight;

impl ScenarioSavePreflight {
    pub fn validate(scenario: &ScenarioDocument, aliases: &[String]) -> Result<(), String> {
        ScenarioEventValidator::validate(&scenario.events, aliases)?;
        let mut ids = HashSet::new();
        for spawn in &scenario.spawns {
            if spawn.id.is_nil() || !ids.insert(spawn.id) {
                return Err("放置物件缺少持久 ID 或 ID 重複。".into());
            }
            if !spawn.prebuilt
                && !aliases
                    .iter()
                    .any(|a| a.eq_ignore_ascii_case(&spawn.alias))
            {
                return Err(format!("未知的物件別名：{}", spawn.alias));
            }
            if !spawn.x.is_finite()
                || !spawn.y.is_finite()
                || !spawn.z.is_finite()
                || !(0.0..=16383.0).contains(&spawn.x)
                || !(0.0..=16383.0).contains(&spawn.z)
            {
                return Err("放置物件的位置必須是有效地圖座標（X/Z：0–16383）。".into());
            }
            let valid_tc = if spawn.count > 0 {
                (1..=20).contains(&spawn.count) && (0..=7).contains(&spawn.team)
            } else {
                spawn.count == 0 && (-1..=15).contains(&spawn.team)
            };
            if !valid_tc {
                return Err("部隊需要隊伍 0–7、人數 1–20；其他物件需要隊伍 -1–15。".into());
            }
        }

        for item in scenario.events.iter().filter(|item| item.enabled) {
            for condition in &item.conditions {
                if !condition.target_id.is_nil() && !ids.contains(&condition.target_id) {
                    return Err(format!(
                        "事件「{}」的目標物件已刪除，請重新選擇或停用事件。",
                        item.name
                    ));
                }
            }
        }

        Ok(())
    }
}
