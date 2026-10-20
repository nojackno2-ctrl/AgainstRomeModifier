use crate::ai::dotnet_unicode::ordinal_ignore_case_key;
use crate::ai::{CampaignCompilationResult, CampaignMissionCompiler, CampaignMissionPlan};
use crate::cinematics::{CinematicBciCompiler, CutsceneSequence, MapStartCamera};
use crate::objectives::{BciObjectiveCompilationResult, BciObjectiveCompiler, ObjectiveDependencyGraph};
use against_rome_scenario::{DataBindingSource, Event, ScenarioDocument, StartCamera};
use std::collections::HashSet;

/// 最大允許之情境事件上限（對應編輯器 256 事件上限）。
pub const MAX_SCENARIO_EVENTS: usize = 256;

/// 戰役全流程編譯輸入參數。
#[derive(Debug, Clone)]
pub struct CampaignPipelineOptions<'a> {
    pub mission_plan: Option<&'a CampaignMissionPlan>,
    pub objective_graph: Option<&'a ObjectiveDependencyGraph>,
    pub cutscenes: Option<&'a [CutsceneSequence]>,
    pub start_camera: Option<&'a MapStartCamera>,
    pub scenario: &'a ScenarioDocument,
    pub known_aliases: &'a [String],
    pub data_source: Option<&'a DataBindingSource>,
}

/// 戰役管線編譯成果。
#[derive(Debug, Clone)]
pub struct CampaignPipelineResult {
    pub success: bool,
    pub diagnostics: Vec<String>,
    pub scenario_document: ScenarioDocument,
    pub compiled_events: Vec<Event>,
    pub bci_bytes: Option<Vec<u8>>,
    pub resolved_camera: Option<MapStartCamera>,
}

/// 戰役整合編譯器：將 AI 任務企劃、任務目標依賴圖、過場運鏡與開局相機
/// 安全彙整至 ScenarioDocument 並編譯為符合原版規範的 BCI/PFIL 位元碼。
pub struct CampaignCompiler;

impl CampaignCompiler {
    fn failed(scenario: &ScenarioDocument, diagnostics: Vec<String>) -> CampaignPipelineResult {
        CampaignPipelineResult {
            success: false,
            diagnostics,
            scenario_document: scenario.clone(),
            compiled_events: vec![],
            bci_bytes: None,
            resolved_camera: None,
        }
    }

    /// 彙整所有戰役組件至 ScenarioDocument 並完成結構與別名驗證。
    pub fn compile(options: CampaignPipelineOptions) -> CampaignPipelineResult {
        let mut diagnostics = vec![];

        // 1. Preflight 檢查情境文件與 DATA 綁定源
        if let Err(e) = options
            .scenario
            .preflight_validate(Some(options.known_aliases), options.data_source)
        {
            diagnostics.push(format!("[Error] 情境 preflight 驗證失敗: {e}"));
            return CampaignPipelineResult {
                success: false,
                diagnostics,
                scenario_document: options.scenario.clone(),
                compiled_events: vec![],
                bci_bytes: None,
                resolved_camera: None,
            };
        }

        let mut incoming_events = vec![];

        // 2. 編譯 AI 戰役企劃 (Waves, Reinforcements, Objectives)
        if let Some(plan) = options.mission_plan {
            let ai_res: CampaignCompilationResult =
                CampaignMissionCompiler::compile(plan, None, options.known_aliases, Some(options.scenario));
            diagnostics.extend(ai_res.diagnostics);
            if !ai_res.success {
                return CampaignPipelineResult {
                    success: false,
                    diagnostics,
                    scenario_document: options.scenario.clone(),
                    compiled_events: vec![],
                    bci_bytes: None,
                    resolved_camera: None,
                };
            }
            incoming_events.extend(ai_res.compiled_events);
        }

        // 3. 編譯任務目標依賴圖 (Objectives)
        if let Some(graph) = options.objective_graph {
            let obj_res: BciObjectiveCompilationResult =
                BciObjectiveCompiler::compile(graph, Some(options.scenario), Some(options.known_aliases));
            for d in &obj_res.diagnostics {
                diagnostics.push(format!("[{:?}] {}: {}", d.severity, d.code, d.message));
            }
            if !obj_res.success {
                return CampaignPipelineResult {
                    success: false,
                    diagnostics,
                    scenario_document: options.scenario.clone(),
                    compiled_events: vec![],
                    bci_bytes: None,
                    resolved_camera: None,
                };
            }
            incoming_events.extend(obj_res.compiled_events);
        }

        // 4. 編譯過場動畫序列 (Cutscenes)
        let mut resolved_cam = options.start_camera.cloned();
        if let Some(cutscenes) = options.cutscenes {
            for seq in cutscenes {
                let cutscene_events = CinematicBciCompiler::compile_cutscene_events(seq);
                incoming_events.extend(cutscene_events);

                if resolved_cam.is_none() {
                    resolved_cam = CinematicBciCompiler::initial_start_camera(seq);
                }
            }
        }

        // 4b. 過場完結連結：on_complete_trigger_event 必須指向既有或本次產出的事件，
        //     不得靜默變成懸空文字訊息。原生 ABI 沒有「觸發事件」動作，故僅驗證連結、
        //     並在目標延遲與完結標記不一致時提出警告。
        if let Some(cutscenes) = options.cutscenes {
            let mut known_events: Vec<(&str, i32)> = options
                .scenario
                .events
                .iter()
                .map(|e| (e.name.as_str(), e.delay_seconds))
                .collect();
            known_events.extend(incoming_events.iter().map(|e| (e.name.as_str(), e.delay_seconds)));
            for seq in cutscenes {
                let Some(link) = seq.on_complete_trigger_event.as_deref() else {
                    continue;
                };
                let link = link.trim();
                let own_marker = format!("{}_完結觸發", seq.name);
                let target = known_events
                    .iter()
                    .find(|(n, _)| crate::ai::dotnet_unicode::ordinal_ignore_case_eq(n, link) && *n != own_marker);
                match target {
                    Some((name, delay)) => {
                        let marker = (seq.calculate_total_duration() as f64).round_ties_even() as i32;
                        if *delay != marker.max(1) {
                            diagnostics.push(format!(
                                "[Warning] 過場「{}」完結連結事件「{name}」延遲 {delay}s 與完結時間 {}s 不一致；原生腳本無事件觸發動作，不會自動對齊。",
                                seq.name,
                                marker.max(1)
                            ));
                        }
                    }
                    None => {
                        diagnostics.push(format!(
                            "[Error] 過場「{}」的完結連結事件「{link}」不存在於情境或本次產出事件。",
                            seq.name
                        ));
                        return Self::failed(options.scenario, diagnostics);
                    }
                }
            }
        }

        if let Some(cam) = resolved_cam.as_ref() {
            if let Err(e) = cam.validate() {
                diagnostics.push(format!("[Error] 開局相機無效: {e}"));
                return Self::failed(options.scenario, diagnostics);
            }
        }

        // 5. 事件名稱碰撞處理（對應 C# UniqueEventNames 邏輯）
        let base_events = options.scenario.events.clone();
        let mut used_names: HashSet<String> = base_events
            .iter()
            .map(|e| ordinal_ignore_case_key(&e.name))
            .collect();

        let mut deduped_incoming = Vec::with_capacity(incoming_events.len());
        for mut item in incoming_events {
            let original_name = item.name.clone();
            let mut name = original_name.clone();
            let mut counter = 2;
            while !used_names.insert(ordinal_ignore_case_key(&name)) {
                name = format!("{original_name}_{counter}");
                counter += 1;
            }
            item.name = name;
            deduped_incoming.push(item);
        }

        // 6. 檢查 256 事件上限容量
        let total_count = base_events.len() + deduped_incoming.len();
        if total_count > MAX_SCENARIO_EVENTS {
            diagnostics.push(format!(
                "[Error] 合併後事件總數 ({total_count}) 超出上限 ({MAX_SCENARIO_EVENTS}) | Merging would exceed the 256-event limit."
            ));
            return CampaignPipelineResult {
                success: false,
                diagnostics,
                scenario_document: options.scenario.clone(),
                compiled_events: vec![],
                bci_bytes: None,
                resolved_camera: None,
            };
        }

        // 7. 合併並排序事件：失敗條件優先於勝利條件
        let mut merged_events = base_events;
        merged_events.extend(deduped_incoming.clone());

        // 確保終端動作順序合法：失敗優先，勝利在後
        merged_events.sort_by_key(|e| {
            if e.actions.iter().any(|a| a.kind == 4) {
                0 // Defeat first
            } else if e.actions.iter().any(|a| a.kind == 3) {
                2 // Victory last
            } else {
                1 // Regular
            }
        });

        let mut final_scenario = options.scenario.clone();
        final_scenario.events = merged_events;

        if let Err(e) = final_scenario.validate(Some(options.known_aliases)) {
            diagnostics.push(format!("[Error] 合併後情境結構校驗失敗: {e}"));
            return CampaignPipelineResult {
                success: false,
                diagnostics,
                scenario_document: final_scenario,
                compiled_events: vec![],
                bci_bytes: None,
                resolved_camera: None,
            };
        }

        diagnostics.push(format!(
            "[Info] 成功整合戰役組件，產出 {} 個情境事件。",
            final_scenario.events.len()
        ));

        CampaignPipelineResult {
            success: true,
            diagnostics,
            scenario_document: final_scenario,
            compiled_events: deduped_incoming,
            bci_bytes: None,
            resolved_camera: resolved_cam,
        }
    }

    /// 完整戰役管線編譯並生成原版 ak_level.bci 位元碼。
    pub fn compile_to_bci(
        options: CampaignPipelineOptions,
        original_bci: &[u8],
    ) -> CampaignPipelineResult {
        let mut result = Self::compile(options.clone());
        if !result.success {
            return result;
        }

        let start_cam_opt: Option<StartCamera> = result.resolved_camera.as_ref().map(Into::into);

        let compile_res = if let Some(source) = options.data_source {
            source.compile_script(
                original_bci,
                &result.scenario_document,
                start_cam_opt.as_ref(),
                Some(options.known_aliases),
            )
        } else {
            against_rome_scenario::compile_script(
                original_bci,
                &result.scenario_document,
                start_cam_opt.as_ref(),
                Some(options.known_aliases),
            )
        };

        match compile_res {
            Ok(bytes) => {
                result.bci_bytes = Some(bytes);
                result
                    .diagnostics
                    .push("[Info] 成功完成戰役 BCI 位元碼編譯。".to_string());
                result
            }
            Err(e) => {
                result.success = false;
                result.diagnostics.push(format!("[Error] BCI 位元碼編譯失敗: {e}"));
                result
            }
        }
    }
}
