use super::campaign::*;
use super::dotnet_unicode::{
    is_letter_or_digit_unit, ordinal_ignore_case_eq, ordinal_ignore_case_key,
};
use super::waypoint::WaypointPath;
use against_rome_scenario::{Action, Condition, Event, ScenarioDocument};
use std::collections::HashSet;

/// 戰役任務編譯結果，包含降階後的合法 ScenarioEvent 清單與診斷資訊。
#[derive(Debug, Clone)]
pub struct CampaignCompilationResult {
    pub success: bool,
    pub compiled_events: Vec<Event>,
    pub diagnostics: Vec<String>,
    pub bci_bytes: Option<Vec<u8>>,
}

/// 將高階戰役目標（波次進攻、定時增援、基地防守、刺殺/摧毀）
/// 編譯降階為原版合法 BCI 執行序與 ScenarioEvent 之編譯管線。
pub struct CampaignMissionCompiler;

impl CampaignMissionCompiler {
    /// 編譯戰役企劃為原版合法的 Event 清單。
    pub fn compile(
        plan: &CampaignMissionPlan,
        _waypoint_paths: Option<&[WaypointPath]>,
        known_aliases: &[String],
        existing_scenario: Option<&ScenarioDocument>,
    ) -> CampaignCompilationResult {
        let mut diagnostics = vec![];
        let mut compiled_events = vec![];

        // 1. 企劃層級自我驗證
        if let Err(e) = plan.validate() {
            diagnostics.push(format!(
                "[Error] 企劃總綱驗證失敗：{e} | Campaign plan validation failed: {e}"
            ));
            return CampaignCompilationResult {
                success: false,
                compiled_events,
                diagnostics,
                bci_bytes: None,
            };
        }

        // 2. 驗證所有部隊別名是否合法
        for wave in &plan.waves {
            if wave.assigned_path_id.is_some()
                || wave.target_object_id.is_some()
                || wave.squads.iter().any(|s| s.relative_delay_seconds != 0)
            {
                diagnostics.push(format!(
                    "[Error] 波次 {}：巡邏路徑、物件攻擊與部隊相對延遲尚未支援。 | Patrol paths, object attacks and relative delays are unsupported.",
                    wave.wave_index
                ));
            }
            for squad in &wave.squads {
                if !known_aliases
                    .iter()
                    .any(|a| ordinal_ignore_case_eq(a, &squad.alias))
                {
                    diagnostics.push(format!(
                        "[Error] 波次 {} 的部隊別名未知：{} | Wave {}: unknown unit alias {}",
                        wave.wave_index, squad.alias, wave.wave_index, squad.alias
                    ));
                }
            }
        }

        for reinf in &plan.reinforcements {
            if reinf.assigned_path_id.is_some()
                || reinf.squads.iter().any(|s| s.relative_delay_seconds != 0)
            {
                diagnostics.push(format!(
                    "[Error] 增援「{}」：巡邏路徑與部隊相對延遲尚未支援。 | Reinforcement {}: patrol paths and relative delays are unsupported.",
                    reinf.name, reinf.name
                ));
            }
            for squad in &reinf.squads {
                if !known_aliases
                    .iter()
                    .any(|a| ordinal_ignore_case_eq(a, &squad.alias))
                {
                    diagnostics.push(format!(
                        "[Error] 增援「{}」的部隊別名未知：{} | Reinforcement {}: unknown unit alias {}",
                        reinf.name, squad.alias, reinf.name, squad.alias
                    ));
                }
            }
        }

        // 3. 驗證目標物件持久 ID 是否存在於場景中
        let known_spawn_ids: HashSet<_> = existing_scenario
            .map(|s| s.spawns.iter().map(|sp| sp.id).collect())
            .unwrap_or_default();

        for obj in &plan.objectives {
            if let Some(target_id) = obj.target_id {
                if !target_id.is_nil() && existing_scenario.is_some() {
                    if !known_spawn_ids.contains(&target_id) {
                        diagnostics.push(format!(
                            "[Error] 目標「{}」指定的目標物件 ID ({}) 不存在於場景放置中。 | Objective {}: target object ID ({}) is not in the scene placement.",
                            obj.title, target_id, obj.title, target_id
                        ));
                    }
                }
            }
        }

        if diagnostics.iter().any(|d| d.starts_with("[Error]")) {
            return CampaignCompilationResult {
                success: false,
                compiled_events,
                diagnostics,
                bci_bytes: None,
            };
        }

        // 4. 編譯開局任務簡報與初始化事件
        if !plan.briefing.trim().is_empty() {
            let briefing_event = Event {
                name: format!("MissionBriefing_{}", sanitize_name(&plan.title)),
                delay_seconds: 2,
                repeat: false,
                enabled: true,
                actions: vec![Action {
                    kind: 0,
                    text: plan.briefing.clone(),
                    ..Default::default()
                }],
                conditions: vec![],
            };
            compiled_events.push(briefing_event);
        }

        // 5. 編譯定時增援（TimerReinforcement）
        for reinf in &plan.reinforcements {
            let mut actions = vec![];
            if !reinf.notification_text.trim().is_empty() {
                actions.push(Action {
                    kind: 0,
                    text: reinf.notification_text.clone(),
                    ..Default::default()
                });
            }

            for squad in &reinf.squads {
                actions.push(Action {
                    kind: 2,
                    alias: squad.alias.clone(),
                    team: squad.team,
                    count: squad.count,
                    x: reinf.spawn_x,
                    z: reinf.spawn_z,
                    ..Default::default()
                });
            }

            let reinf_event = Event {
                name: format!("Reinf_{}", sanitize_name(&reinf.name)),
                delay_seconds: reinf.trigger_delay_seconds,
                repeat: false,
                enabled: true,
                actions,
                conditions: vec![],
            };
            compiled_events.push(reinf_event);
        }

        // 6. 編譯進攻波次（WaveAttack）
        let mut sorted_waves = plan.waves.clone();
        sorted_waves.sort_by_key(|w| w.wave_index);

        for wave in sorted_waves {
            let mut actions = vec![];
            if !wave.announcement.trim().is_empty() {
                actions.push(Action {
                    kind: 0,
                    text: wave.announcement.clone(),
                    ..Default::default()
                });
            }

            for squad in &wave.squads {
                actions.push(Action {
                    kind: 2,
                    alias: squad.alias.clone(),
                    team: squad.team,
                    count: squad.count,
                    x: wave.spawn_x,
                    z: wave.spawn_z,
                    move_destination: wave.move_destination.clone(),
                    ..Default::default()
                });
            }

            let wave_event = Event {
                name: format!("Wave_{:02}", wave.wave_index),
                delay_seconds: wave.trigger_delay_seconds,
                repeat: false,
                enabled: true,
                actions,
                conditions: vec![],
            };
            compiled_events.push(wave_event);
        }

        // 7. 編譯勝敗目標（Objectives）
        for obj in &plan.objectives {
            match obj.objective_type {
                CampaignObjectiveType::DefendTarget => {
                    let target_id = obj.target_id.unwrap_or_default();
                    let defend_fail_event = Event {
                        name: format!("DefendFail_{}", sanitize_name(&obj.title)),
                        delay_seconds: 1,
                        repeat: false,
                        enabled: true,
                        conditions: vec![Condition {
                            kind: 1, // ObjectDeadOrRemoved
                            target_id,
                            ..Default::default()
                        }],
                        actions: vec![
                            Action {
                                kind: 0,
                                text: "Defense failed: the protected target was destroyed!".into(),
                                ..Default::default()
                            },
                            Action {
                                kind: 4, // Defeat
                                ..Default::default()
                            },
                        ],
                    };
                    compiled_events.push(defend_fail_event);
                }
                CampaignObjectiveType::DestroyTarget => {
                    let target_id = obj.target_id.unwrap_or_default();
                    let destroy_win_event = Event {
                        name: format!("DestroyWin_{}", sanitize_name(&obj.title)),
                        delay_seconds: 1,
                        repeat: false,
                        enabled: true,
                        conditions: vec![Condition {
                            kind: 1, // ObjectDeadOrRemoved
                            target_id,
                            ..Default::default()
                        }],
                        actions: vec![
                            Action {
                                kind: 0,
                                text: "Objective achieved: the target was destroyed!".into(),
                                ..Default::default()
                            },
                            Action {
                                kind: 3, // Victory
                                ..Default::default()
                            },
                        ],
                    };
                    compiled_events.push(destroy_win_event);
                }
                CampaignObjectiveType::SurviveTime => {
                    let survive_win_event = Event {
                        name: format!("SurviveWin_{}", sanitize_name(&obj.title)),
                        delay_seconds: obj.required_seconds,
                        repeat: false,
                        enabled: true,
                        conditions: vec![],
                        actions: vec![
                            Action {
                                kind: 0,
                                text: format!(
                                    "Defense successful: survived for {} seconds!",
                                    obj.required_seconds
                                ),
                                ..Default::default()
                            },
                            Action {
                                kind: 3, // Victory
                                ..Default::default()
                            },
                        ],
                    };
                    compiled_events.push(survive_win_event);
                }
                CampaignObjectiveType::ReachArea => {
                    let target_id = obj.target_id.unwrap_or_default();
                    let reach_win_event = Event {
                        name: format!("ReachWin_{}", sanitize_name(&obj.title)),
                        delay_seconds: 1,
                        repeat: false,
                        enabled: true,
                        conditions: vec![Condition {
                            kind: 2, // ObjectInArea
                            target_id,
                            min_x: obj.target_min_x,
                            min_z: obj.target_min_z,
                            max_x: obj.target_max_x,
                            max_z: obj.target_max_z,
                        }],
                        actions: vec![
                            Action {
                                kind: 0,
                                text: "Mission complete: the target unit reached the designated area!".into(),
                                ..Default::default()
                            },
                            Action {
                                kind: 3, // Victory
                                ..Default::default()
                            },
                        ],
                    };
                    compiled_events.push(reach_win_event);
                }
                CampaignObjectiveType::WaveSurvival => {
                    let last_wave_delay = plan
                        .waves
                        .iter()
                        .map(|w| w.trigger_delay_seconds)
                        .max()
                        .unwrap_or(0);
                    let survive_buffer = if obj.required_seconds > 0 {
                        obj.required_seconds
                    } else {
                        120
                    };
                    let wave_survival_event = Event {
                        name: format!("WaveSurvivalWin_{}", sanitize_name(&obj.title)),
                        // Match C# unchecked int addition; the final event validator
                        // rejects a wrapped negative delay instead of panicking.
                        delay_seconds: last_wave_delay.wrapping_add(survive_buffer),
                        repeat: false,
                        enabled: true,
                        conditions: vec![],
                        actions: vec![
                            Action {
                                kind: 0,
                                text: "Victory! All enemy attack waves have been repelled!".into(),
                                ..Default::default()
                            },
                            Action {
                                kind: 3, // Victory
                                ..Default::default()
                            },
                        ],
                    };
                    compiled_events.push(wave_survival_event);
                }
            }
        }

        // 8. 最終降階校驗（使用 ScenarioDocument 驗證規則）
        let mut check_doc = existing_scenario.cloned().unwrap_or_default();
        check_doc.events = compiled_events.clone();
        if let Err(e) = check_doc.validate(Some(known_aliases)) {
            diagnostics.push(format!(
                "[Error] 編譯後之 ScenarioEvent 驗證失敗：{e} | Compiled ScenarioEvent validation failed: {e}"
            ));
            return CampaignCompilationResult {
                success: false,
                compiled_events,
                diagnostics,
                bci_bytes: None,
            };
        }

        diagnostics.push(format!(
            "[Info] 成功編譯 {} 個戰役執行序事件。",
            compiled_events.len()
        ));
        CampaignCompilationResult {
            success: true,
            compiled_events,
            diagnostics,
            bci_bytes: None,
        }
    }

    /// 將戰役任務企劃降階為 ScenarioEvent 並編譯進原版 ak_level.bci 位元碼。
    pub fn compile_to_bci(
        plan: &CampaignMissionPlan,
        original: &[u8],
        scenario: &ScenarioDocument,
        known_aliases: &[String],
        camera: Option<&crate::MapStartCamera>,
    ) -> CampaignCompilationResult {
        let mut result = Self::compile(plan, None, known_aliases, Some(scenario));
        if !result.success {
            return result;
        }

        let mut scenario_copy = scenario.clone();
        scenario_copy.events.extend(result.compiled_events.clone());

        let start_cam_opt: Option<against_rome_scenario::StartCamera> = camera.map(Into::into);

        match against_rome_scenario::compile_script(
            original,
            &scenario_copy,
            start_cam_opt.as_ref(),
            Some(known_aliases),
        ) {
            Ok(bytes) => {
                result.bci_bytes = Some(bytes);
                result
                    .diagnostics
                    .push("[Info] 成功編譯戰役企劃為 BCI 位元碼。".to_string());
                result
            }
            Err(e) => {
                result.success = false;
                result.diagnostics.push(format!("[Error] BCI 編譯失敗: {e}"));
                result
            }
        }
    }

    /// 編譯戰役企劃並合併至既有劇本（對應 C# `CampaignWaveDialog.CompileForMerge` / `MapEditorForm.ApplyCampaignWaves`）：
    /// 1. 拒絕未支援之企劃欄位（勢力 AI 設定、波次/增援之路徑、攻擊目標、部隊相對延遲）。
    /// 2. 調用 `compile` 進行波次降階編譯。
    /// 3. 對編譯生成事件名稱進行 `OrdinalIgnoreCase` 序數大小寫不敏感去重（碰撞時附加 `_2`, `_3` 等後綴）。
    /// 4. 強制檢查合併後事件總數不超過原版引擎之 256 事件上限。
    /// 5. 針對合併後之整體 ScenarioDocument 進行完整結構、別名與條件驗證。
    pub fn compile_for_merge(
        plan: &CampaignMissionPlan,
        known_aliases: &[String],
        scenario: &ScenarioDocument,
    ) -> CampaignCompilationResult {
        if !plan.faction_profiles.is_empty()
            || plan.waves.iter().any(|w| {
                w.assigned_path_id.is_some()
                    || w.target_object_id.is_some()
                    || w.squads.iter().any(|s| s.relative_delay_seconds != 0)
            })
            || plan.reinforcements.iter().any(|r| {
                r.assigned_path_id.is_some()
                    || r.squads.iter().any(|s| s.relative_delay_seconds != 0)
            })
        {
            return CampaignCompilationResult {
                success: false,
                compiled_events: vec![],
                diagnostics: vec![
                    "[Error] 尚不支援勢力 AI、路徑、攻擊目標與部隊相對延遲。 | Faction AI, paths, attack targets and relative squad delays are unsupported.".to_string(),
                ],
                bci_bytes: None,
            };
        }

        let mut result = Self::compile(plan, None, known_aliases, Some(scenario));
        if !result.success {
            return result;
        }

        let mut used = HashSet::new();
        for ev in &scenario.events {
            used.insert(ordinal_ignore_case_key(&ev.name));
        }

        let mut deduplicated = Vec::with_capacity(result.compiled_events.len());
        for item in result.compiled_events {
            let mut name = item.name.clone();
            let mut n = 2;
            while !used.insert(ordinal_ignore_case_key(&name)) {
                name = format!("{}_{}", item.name, n);
                n += 1;
            }
            let mut renamed = item;
            renamed.name = name;
            deduplicated.push(renamed);
        }

        if scenario.events.len() + deduplicated.len() > 256 {
            return CampaignCompilationResult {
                success: false,
                compiled_events: vec![],
                diagnostics: vec![
                    "[Error] 合併後會超過 256 個事件上限。 | Merging would exceed the 256-event limit.".to_string(),
                ],
                bci_bytes: None,
            };
        }

        let mut check_doc = scenario.clone();
        check_doc.events.extend(deduplicated.clone());
        if let Err(e) = check_doc.validate(Some(known_aliases)) {
            let mut diags = result.diagnostics;
            diags.push(format!(
                "[Error] 合併事件後驗證失敗：{e} | Merged events validation failed: {e}"
            ));
            return CampaignCompilationResult {
                success: false,
                compiled_events: vec![],
                diagnostics: diags,
                bci_bytes: None,
            };
        }

        result.compiled_events = deduplicated;
        result
    }
}

/// 對應 C# `CampaignMissionCompiler.SanitizeName`：僅空白字串回傳 "Item"；
/// 其餘逐 UTF-16 code unit 將非 `char.IsLetterOrDigit` 者換成 `_`（surrogate 單元恆為非字母，
/// 故補充平面字元如 emoji、𠀀 各變成兩個 `_`，與 C# 一致）、修剪兩端 `_`、截斷至 30 個 UTF-16 單元。
/// 全為符號時與 C# 相同回傳空字串。分類表由真 .NET 8 oracle 產生（見 `dotnet_unicode`）。
pub fn sanitize_name(raw: &str) -> String {
    if raw.trim().is_empty() {
        return "Item".to_string();
    }
    let mapped: String = raw
        .encode_utf16()
        .map(|u| if is_letter_or_digit_unit(u) { u } else { b'_' as u16 })
        .map(|u| char::from_u32(u as u32).unwrap_or('_'))
        .collect();
    mapped.trim_matches('_').chars().take(30).collect()
}
