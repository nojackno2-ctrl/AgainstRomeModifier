//! 安全儲存共用的阻擋清單與解決介面；所有操作沿用既有面板與交易。
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveBlockerKind {
    PendingPanel,
    MissingSession,
    Target,
    Validation,
    DataConflict,
    DesignError,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavePanel {
    EventGraph,
    ScenarioGraph,
    Maps,
    Environment,
    Landscape,
    Packaging,
    Campaign,
    AiPlanner,
    Toolbox,
    Terrain,
    Ambience,
    Objects,
    Nature,
    Scenario,
    Materials,
    Scene,
    DesignCheck,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveBlockerAction {
    OpenPanel,
    Apply,
    Discard,
    ApplyEnvironment(EnvApply),
    ApplyLandscape(LandscapeApply),
    ApplyErosion,
    ApplyLairEvents,
    ApplyAiPlan,
    ApplyObjectives,
    ApplyWaves,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveBlocker {
    pub kind: SaveBlockerKind,
    pub panel: SavePanel,
    pub english: String,
    pub chinese: String,
    pub actions: Vec<SaveBlockerAction>,
}
impl SaveBlocker {
    fn new(
        kind: SaveBlockerKind,
        panel: SavePanel,
        english: impl Into<String>,
        chinese: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            panel,
            english: english.into(),
            chinese: chinese.into(),
            actions: vec![SaveBlockerAction::OpenPanel],
        }
    }
}

/// 保留成功暫存的實際檔名，讓目標前檢與面板錯誤彼此獨立。
fn remember_targets<P: Into<PathBuf>>(
    targets: &mut Vec<(SavePanel, PathBuf)>,
    panel: SavePanel,
    files: Result<Vec<(P, Vec<u8>)>, String>,
) -> Result<(), String> {
    files.map(|files| targets.extend(files.into_iter().map(|(path, _)| (panel, path.into()))))
}

impl EditorApp {
    /// 只讀取與暫存候選資料；個別失敗不遮蔽其他面板，不改動編輯歷史。
    pub fn save_blockers(&self) -> Vec<SaveBlocker> {
        use SaveBlockerKind::*;
        use SavePanel::*;
        let mut blockers = Vec::new();
        let mut targets = Vec::new();
        for (dirty, panel, en, zh) in [
            (self.event_graph.is_dirty(), EventGraph, "EventGraph: unapplied edits; Apply to Scenario or discard before Safe Save", "事件圖：請先套用至劇本或捨棄草稿"),
            (self.scenario.graph_ui().is_dirty(), ScenarioGraph, "Scenario EventGraph: unapplied edits; Apply to Scenario or discard before Safe Save", "劇本事件圖：請先套用至劇本或捨棄草稿"),
            (self.maps.is_dirty(), Maps, "Maps: pending catalog/build settings have no map artifact serializer", "地圖：目錄或建置設定尚未處理"),
            (self.environment.is_dirty(), Environment, "Environment: pending flora/lair/settlement plans must be applied or discarded", "環境：植被、巢穴或聚落計畫尚未套用或捨棄"),
            (self.landscape.is_dirty(), Landscape, "Landscape: pending terrain/placement plans must be applied or discarded", "地景：地形或放置計畫尚未套用或捨棄"),
            (self.packaging.is_dirty(), Packaging, "Packaging: pending export/diff/budget state has no map artifact serializer", "封裝：匯出、差異或預算狀態尚未處理"),
            (self.campaign.has_unsupported_edits(), Campaign, "Campaign: pending objectives/waves/cinematics have no map-bound compile/save contract", "戰役：任務目標、波次或運鏡草稿尚未完成編譯與套用"),
            (self.ai_planner.is_dirty(), AiPlanner, "AI Planner: pending plan must be applied or discarded", "AI 規劃：計畫尚未套用或捨棄"),
            (self.toolbox.is_dirty(), Toolbox, "Toolbox: pending external save/backup review requires its own Apply/discard", "工具箱：外部存檔或備份審查尚未處理"),
        ] {
            if dirty {
                let mut b = SaveBlocker::new(PendingPanel, panel, en, zh);
                // 只提供已有確認流程的捨棄；外部存檔不從此處套用。
                if panel != Toolbox { b.actions.push(SaveBlockerAction::Discard); }
                if matches!(panel, EventGraph | ScenarioGraph) { b.actions.push(SaveBlockerAction::Apply); }
                if panel == Environment && self.placement.is_loaded() {
                    if !self.environment.generated_flora.is_empty() { b.actions.push(SaveBlockerAction::ApplyEnvironment(EnvApply::Flora)); }
                    if !self.environment.generated_lairs.is_empty() && self.environment.generated_lair_events.is_empty() { b.actions.push(SaveBlockerAction::ApplyEnvironment(EnvApply::Lairs)); }
                    if self.environment.last_distribution.is_some() { b.actions.push(SaveBlockerAction::ApplyEnvironment(EnvApply::Settlement)); }
                }
                if panel == Environment && !self.environment.generated_lair_events.is_empty() && self.scenario.is_loaded() && !self.scenario.graph_ui().is_dirty() { b.actions.push(SaveBlockerAction::ApplyLairEvents); }
                if panel == Landscape && self.session.is_some() {
                    for (pending, kind) in [(self.landscape.last_cliff_result.is_some(), LandscapeApply::Cliff), (self.landscape.last_river_plan.is_some(), LandscapeApply::River), (self.landscape.last_road_plan.is_some(), LandscapeApply::Road), (self.landscape.last_wall_plan.is_some(), LandscapeApply::Wall)] {
                        if pending { b.actions.push(SaveBlockerAction::ApplyLandscape(kind)); }
                    }
                    if self.landscape.last_erosion_plan.is_some() { b.actions.push(SaveBlockerAction::ApplyErosion); }
                }
                if panel == AiPlanner && self.session.is_some() && self.ai_planner.current_plan.is_some() && !self.ai_planner.is_generating() { b.actions.push(SaveBlockerAction::ApplyAiPlan); }
                if panel == Campaign && self.scenario.is_loaded() && !self.scenario.graph_ui().is_dirty() {
                    if !self.campaign.objectives.is_empty() { b.actions.push(SaveBlockerAction::ApplyObjectives); }
                    if self.campaign.last_wave_result.is_some() { b.actions.push(SaveBlockerAction::ApplyWaves); }
                }
                if self.current_map_is_original() {
                    b.actions.retain(|a| matches!(a, SaveBlockerAction::OpenPanel | SaveBlockerAction::Discard));
                }
                blockers.push(b);
            }
        }
        if self.session.is_none() {
            blockers.push(SaveBlocker::new(
                MissingSession,
                Terrain,
                "No terrain session to save",
                "尚未載入地形",
            ));
        }
        let path = PathBuf::from(self.map_path_input.trim());
        if path.as_os_str().is_empty() {
            blockers.push(SaveBlocker::new(
                Target,
                Maps,
                "Please specify target map directory path before saving",
                "請先指定地圖儲存目錄",
            ));
        }
        let mut check = |panel, result: Result<(), String>| {
            if let Err(e) = result {
                blockers.push(SaveBlocker::new(
                    Validation,
                    panel,
                    e.clone(),
                    format!("驗證未通過：{e}"),
                ));
            }
        };
        if let Some(s) = &self.session {
            targets.extend(["boden.bmp", "collision.bmp"].map(|p| (Terrain, PathBuf::from(p))));
            check(Terrain, crate::storage::encode_terrain(s).map(|_| ()));
            if let Some(e) = &self.emboss {
                if e.session.is_dirty(s.heights()) {
                    targets.push((Terrain, PathBuf::from("emboss.bmp")));
                    check(
                        Terrain,
                        e.session
                            .build_emboss(s.heights())
                            .map(|_| ())
                            .map_err(|e| format!("emboss.bmp relight: {e}")),
                    );
                }
            }
        }
        if self.ambience.is_dirty() {
            let current = serde_json::to_value(&self.ambience.profile);
            let defaults = serde_json::to_value(
                against_rome_ambience::atmosphere::WeatherPresets::create_clear_sky(),
            );
            check(Ambience, current.and_then(|v| defaults.map(|d| (v,d))).map_err(|e| e.to_string()).and_then(|(v,d)| {
                let fields: Vec<_> = v.as_object().ok_or("Ambience profile is not an object")?.iter().filter_map(|(k,v)| (k != "boden" && d.get(k) != Some(v)).then_some(k.as_str())).collect();
                if fields.is_empty() { Ok(()) } else { Err(format!("Ambience unsupported dirty profile fields (no save/reload serializer): {}", fields.join(", "))) }
            }));
            let (valid, errors) = against_rome_ambience::atmosphere::BodenIniSerializer::validate(
                &self.ambience.profile.boden,
            );
            if !valid {
                check(
                    Ambience,
                    Err(format!(
                        "Ambience boden.ini validation: {}",
                        errors.join("; ")
                    )),
                );
            }
        }
        if !path.as_os_str().is_empty() {
            if self.ambience.is_dirty() {
                targets
                    .extend(["boden.ini", "soundscape.sndz"].map(|p| (Ambience, PathBuf::from(p))));
                check(
                    Ambience,
                    self.ambience.stage_boden_ini_bytes(&path).map(|_| ()),
                );
                check(
                    Ambience,
                    self.ambience.stage_briefing_bytes(&path).map(|bytes| {
                        if bytes.is_some() {
                            targets.push((Ambience, PathBuf::from("TEXT/US/briefing.put")));
                        }
                    }),
                );
                let snapshot = against_rome_ambience::soundscape::SoundscapeSnapshot {
                    global_ambience_id: self.ambience.global_sound_id.clone(),
                    global_ambience_volume: self.ambience.global_volume,
                    zones: self.ambience.zones.clone(),
                };
                check(Ambience, against_rome_ambience::soundscape::SoundscapeBinaryStorage::serialize_to_binary(&snapshot).map(|_| ()).map_err(|e| format!("Ambience soundscape.sndz serialization: {e}")));
            }
            if self.placement.is_dirty() && !self.nature.session.is_dirty() {
                check(
                    Objects,
                    remember_targets(
                        &mut targets,
                        Objects,
                        self.placement.stage_transaction_files(&path),
                    ),
                );
            }
            if self.nature.session.is_dirty() {
                check(
                    Nature,
                    remember_targets(&mut targets, Nature, self.stage_nature_files(&path)),
                );
            }
            if self.textures.is_dirty() {
                check(
                    Materials,
                    remember_targets(
                        &mut targets,
                        Materials,
                        self.textures.stage_transaction_files(&path),
                    ),
                );
            }
            check(
                Scene,
                remember_targets(
                    &mut targets,
                    Scene,
                    self.scene.stage_transaction_files(&path),
                ),
            );
            if self.reset_auxiliary_layers {
                for name in ["vertex.bmp", "smooth.bmp"] {
                    let file = path.join(name);
                    if file.is_file() {
                        targets.push((Terrain, PathBuf::from(name)));
                        check(
                            Terrain,
                            std::fs::read(&file)
                                .map_err(|e| format!("{}: {e}", file.display()))
                                .and_then(|bytes| {
                                    crate::bmp::BmpImage::parse(&bytes)
                                        .map(|_| ())
                                        .map_err(|e| format!("Corrupted {}: {e}", file.display()))
                                }),
                        );
                    }
                }
            }
        }
        // DATA 候選與鏡頭互斥條件和原交易一致，不以清除 dirty 旗標解決。
        let scenario_dirty = self.scenario.dirty() || self.scenario.graph_ui().is_dirty();
        let spawn_plan = if scenario_dirty && !path.as_os_str().is_empty() {
            match self.preview_spawn_data_for_save(&path) {
                Ok(p) => p,
                Err(e) => {
                    check(Scenario, Err(e));
                    None
                }
            }
        } else {
            None
        };
        let spawn_data = spawn_plan.as_ref().is_some_and(|p| !p.files.is_empty());
        let staged = if scenario_dirty {
            self.scenario.stage_transaction_files_with(
                &path,
                spawn_plan
                    .as_ref()
                    .map(|p| (&p.scenario, p.active.as_slice())),
            )
        } else {
            Ok(Vec::new())
        };
        let staged = staged.and_then(|s| {
            self.campaign
                .stage_camera_candidate(&path, &s, self.scenario.known_aliases())
        });
        // 結束閉包借用後才加入衝突與診斷明細。
        if let Err(e) = &staged {
            check(Scenario, Err(e.clone()));
        }
        if let Some(plan) = &spawn_plan {
            targets.extend(plan.files.iter().map(|(p, _)| (Scenario, p.clone())));
        }
        if let Ok(files) = &staged {
            targets.extend(files.iter().map(|(p, _)| (Scenario, p.clone())));
        }
        blockers.extend(self.save_data_blockers(&path, spawn_data));
        if !path.as_os_str().is_empty() {
            let marker = staged.as_ref().is_ok_and(|files| {
                files.iter().any(|(p, _)| {
                    p == Path::new(against_rome_maps::CustomMapManifest::MARKER_FILE_NAME)
                })
            });
            if let Err(e) = crate::storage::preflight_transaction_tree_opt(&path, marker) {
                blockers.push(SaveBlocker::new(
                    Target,
                    Maps,
                    e.clone(),
                    format!("儲存目錄預檢未通過：{e}"),
                ));
            }
            for (panel, relative) in targets {
                if let Err(e) =
                    crate::storage::preflight_transaction_target_opt(&path.join(relative), marker)
                {
                    if !blockers.iter().any(|b| b.english == e) {
                        blockers.push(SaveBlocker::new(
                            Target,
                            panel,
                            e.clone(),
                            format!("儲存目標預檢未通過：{e}"),
                        ));
                    }
                }
            }
        }
        if self.session.is_some() && !path.as_os_str().is_empty() {
            if let Ok(staged) = staged {
                match self.candidate_diagnostics(&path, &staged) {
                    Err(e) => blockers.push(SaveBlocker::new(
                        DesignError,
                        DesignCheck,
                        e.clone(),
                        format!("設計檢查無法完成：{e}"),
                    )),
                    Ok(candidate) => {
                        for i in candidate.map_issues.into_iter().filter(|i| {
                            i.severity == against_rome_maps::diagnostics::MapIssueSeverity::Error
                        }) {
                            blockers.push(SaveBlocker::new(
                                DesignError,
                                DesignCheck,
                                format!("MapDiagnostics [{}]: {}", i.code, i.english),
                                format!("[{}] {}", i.code, i.chinese),
                            ));
                        }
                        match candidate.design {
                            Err(e) => blockers.push(SaveBlocker::new(DesignError, DesignCheck, e.clone(), format!("設計檢查未通過：{e}"))),
                            Ok(issues) => for i in issues.into_iter().filter(|i| i.severity == against_rome_maps::design_validator::DesignIssueSeverity::Error) {
                                blockers.push(SaveBlocker::new(DesignError, DesignCheck, format!("[{}] {}", i.code, i.english), format!("[{}] {}", i.code, i.chinese)));
                            }
                        }
                    }
                }
            }
        }
        blockers
    }

    /// 預覽與正式 DATA 候選都走同一組互斥檢查，正式候選不信任預覽結果。
    pub(super) fn save_data_blockers(&self, path: &Path, spawn_data: bool) -> Vec<SaveBlocker> {
        use SaveBlockerKind::*;
        use SavePanel::*;
        let mut blockers = Vec::new();
        if spawn_data && (self.placement.is_dirty() || self.nature.session.is_dirty()) {
            blockers.push(SaveBlocker::new(DataConflict, if self.placement.is_dirty() { Objects } else { Nature }, "Spawn placement DATA changes (prebuilt buildings / bound spawns) cannot be saved together with pending Objects (DATA) or Nature edits; save or discard those first", "生成物件的 DATA 變更不可與物件或自然地景變更同時儲存；請先處理其中一方"));
        }
        if spawn_data && self.campaign.camera_pending() {
            blockers.push(SaveBlocker::new(DataConflict, Campaign, "Start camera candidate and new prebuilt DATA objects cannot be saved in one transaction; apply one and save first", "開局鏡頭與新增預建物件不可在同一交易儲存"));
        }
        if self.campaign.camera_pending()
            && self
                .loaded_map_path
                .as_ref()
                .and_then(|p| std::fs::canonicalize(p).ok())
                != std::fs::canonicalize(&path).ok()
        {
            blockers.push(SaveBlocker::new(
                Target,
                Campaign,
                "Start camera save target differs from the loaded map",
                "開局鏡頭儲存目標與載入地圖不同",
            ));
        }
        blockers
    }

    pub(super) fn open_save_panel(&mut self, panel: SavePanel, ctx: &egui::Context) {
        use SavePanel::*;
        let title = match panel {
            EventGraph => {
                self.event_graph.open = true;
                "Visual Event Graph (劇情事件節點圖)"
            }
            ScenarioGraph => {
                self.scenario.open_graph();
                "Visual Event Graph (劇情事件節點圖)"
            }
            Maps | Terrain => {
                self.maps.open = true;
                "Maps & Diagnostics Studio"
            }
            Environment => {
                self.environment.open = true;
                "Environment & Settlement Studio"
            }
            Landscape => {
                self.landscape.open = true;
                "Landscape & Fortification Studio"
            }
            Packaging => {
                self.packaging.open = true;
                "Packaging, Diff & Budget Profiler"
            }
            Campaign => {
                self.campaign.open = true;
                "Campaign, Objectives & Cinematics Studio"
            }
            AiPlanner => {
                self.ai_planner.open = true;
                "AI Map Planning Studio (Ollama)"
            }
            Toolbox => {
                self.toolbox.open = true;
                match self.toolbox.current_tab {
                    crate::toolbox::ToolboxTab::Modifier => "EXE Modifier (遊戲補丁修改工具)",
                    crate::toolbox::ToolboxTab::SaveManager => {
                        "Save Game Manager (遊戲存檔管理工具)"
                    }
                }
            }
            Ambience => {
                self.ambience.open = true;
                "Ambience & Localization Studio (環境氛圍與在地化)"
            }
            Objects => {
                self.placement.open = true;
                "Rust 物件 / Family 工具"
            }
            Nature => {
                self.save_focus_nature = true;
                ctx.request_repaint();
                return;
            }
            Scenario => {
                self.scenario.open = true;
                "scenario_events_window"
            }
            Materials => {
                self.textures.open = true;
                "地圖材質 / Materials"
            }
            Scene => {
                self.scene.open = true;
                "場景 / Scene (SDL)"
            }
            DesignCheck => {
                self.run_design_check();
                tr(
                    self.loc.current_language(),
                    "Design Check (read-only)",
                    "設計檢查（唯讀）",
                )
            }
        };
        ctx.move_to_top(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new(title),
        ));
        ctx.request_repaint();
    }

    pub(super) fn resolve_save_blocker(
        &mut self,
        panel: SavePanel,
        action: SaveBlockerAction,
        ctx: &egui::Context,
    ) {
        // 每次重新檢查可用操作，避免對已改變或已解決的草稿執行舊按鈕。
        if !self
            .save_blockers()
            .iter()
            .any(|b| b.panel == panel && b.actions.contains(&action))
        {
            return;
        }
        self.open_save_panel(panel, ctx);
        match action {
            SaveBlockerAction::OpenPanel => (),
            SaveBlockerAction::Apply if panel == SavePanel::EventGraph => {
                self.event_graph.apply_requested = true;
                self.apply_toolbar_event_graph_request();
            }
            SaveBlockerAction::Apply if panel == SavePanel::ScenarioGraph => {
                if let Err(e) = self.scenario.apply_graph() {
                    self.status_message = e;
                    self.status_is_error = true;
                }
            }
            SaveBlockerAction::ApplyEnvironment(kind) => self.handle_environment_apply(kind),
            SaveBlockerAction::ApplyLandscape(kind) => self.handle_landscape_apply(kind),
            SaveBlockerAction::ApplyErosion => {
                self.landscape.request_erosion_apply();
                if let Some(plan) = self.landscape.take_erosion_apply_request() {
                    self.handle_erosion_apply(plan);
                }
            }
            SaveBlockerAction::ApplyLairEvents => {
                self.environment.request_lair_events_apply();
                self.apply_lair_events_request();
            }
            SaveBlockerAction::ApplyObjectives => {
                self.campaign.request_objective_apply();
                self.apply_campaign_objective_request();
            }
            SaveBlockerAction::ApplyWaves => {
                self.campaign.request_wave_merge();
                self.apply_campaign_wave_merge();
            }
            SaveBlockerAction::ApplyAiPlan => {
                self.sync_ai_materials();
                if let Some(session) = self.session.as_mut() {
                    match self.ai_planner.apply_plan_to_session(
                        session,
                        self.textures.session_mut(),
                        self.water_surface,
                    ) {
                        Ok(_) => {
                            self.textures.note_external_change();
                            self.texture_dirty = true;
                            self.navigation.invalidate();
                        }
                        Err(e) => {
                            self.ai_planner.status_message = e;
                            self.ai_planner.status_is_error = true;
                        }
                    }
                }
            }
            SaveBlockerAction::Discard => match panel {
                SavePanel::EventGraph => self.event_graph.request_close(),
                SavePanel::ScenarioGraph => self.scenario.graph_ui_mut().request_close(),
                SavePanel::Maps => self.maps.request_close(),
                SavePanel::Environment => self.environment.request_close(),
                SavePanel::Landscape => self.landscape.request_close(),
                SavePanel::Packaging => self.packaging.request_close(),
                SavePanel::Campaign => self.campaign.request_close(),
                SavePanel::AiPlanner => self.ai_planner.request_close(),
                _ => (),
            },
            _ => (),
        }
    }

    pub(super) fn save_now_from_blockers(&mut self) {
        if self.save_blockers().is_empty() {
            self.save_blockers_open = false;
            self.request_safe_save();
        }
    }

    pub(super) fn show_save_blockers(&mut self, ctx: &egui::Context) {
        if !self.save_blockers_open {
            return;
        }
        let blockers = self.save_blockers();
        let lang = self.loc.current_language();
        let mut open = true;
        let mut action = None;
        let mut save = false;
        egui::Window::new(tr(lang, "Cannot save yet", "尚無法儲存"))
            .open(&mut open)
            .default_width(620.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(420.0)
                    .show(ui, |ui| {
                        for (index, b) in blockers.iter().enumerate() {
                            ui.push_id(index, |ui| {
                                ui.label(if lang == AppLanguage::English {
                                    &b.english
                                } else {
                                    &b.chinese
                                });
                                ui.horizontal_wrapped(|ui| {
                                    for a in &b.actions {
                                        let label = match a {
                                            SaveBlockerAction::OpenPanel => {
                                                tr(lang, "Open panel", "開啟面板")
                                            }
                                            SaveBlockerAction::Apply => tr(lang, "Apply", "套用"),
                                            SaveBlockerAction::Discard => {
                                                tr(lang, "Discard…", "捨棄…")
                                            }
                                            SaveBlockerAction::ApplyEnvironment(
                                                EnvApply::Flora,
                                            ) => tr(lang, "Apply flora", "套用植被"),
                                            SaveBlockerAction::ApplyEnvironment(
                                                EnvApply::Lairs,
                                            ) => tr(lang, "Apply lairs", "套用巢穴"),
                                            SaveBlockerAction::ApplyEnvironment(
                                                EnvApply::Settlement,
                                            ) => tr(lang, "Apply settlement", "套用聚落"),
                                            SaveBlockerAction::ApplyLandscape(
                                                LandscapeApply::Cliff,
                                            ) => tr(lang, "Apply cliff", "套用懸崖"),
                                            SaveBlockerAction::ApplyLandscape(
                                                LandscapeApply::River,
                                            ) => tr(lang, "Apply river", "套用河流"),
                                            SaveBlockerAction::ApplyLandscape(
                                                LandscapeApply::Road,
                                            ) => tr(lang, "Apply road", "套用道路"),
                                            SaveBlockerAction::ApplyLandscape(
                                                LandscapeApply::Wall,
                                            ) => tr(lang, "Apply wall", "套用城牆"),
                                            SaveBlockerAction::ApplyErosion => {
                                                tr(lang, "Apply erosion", "套用侵蝕")
                                            }
                                            SaveBlockerAction::ApplyLairEvents => {
                                                tr(lang, "Apply lair events", "套用巢穴事件")
                                            }
                                            SaveBlockerAction::ApplyAiPlan => {
                                                tr(lang, "Apply AI plan", "套用 AI 計畫")
                                            }
                                            SaveBlockerAction::ApplyObjectives => {
                                                tr(lang, "Apply objectives", "套用任務目標")
                                            }
                                            SaveBlockerAction::ApplyWaves => {
                                                tr(lang, "Apply waves", "套用波次")
                                            }
                                        };
                                        if ui.button(label).clicked() {
                                            action = Some((b.panel, *a));
                                        }
                                    }
                                });
                                ui.separator();
                            });
                        }
                    });
                if blockers.is_empty() {
                    ui.label(tr(lang, "All blockers resolved.", "所有阻擋均已解決。"));
                }
                save = ui
                    .add_enabled(
                        blockers.is_empty(),
                        egui::Button::new(tr(lang, "Save now", "立即儲存")),
                    )
                    .clicked();
            });
        self.save_blockers_open = open;
        if let Some((panel, action)) = action {
            self.resolve_save_blocker(panel, action, ctx);
        }
        if save {
            self.save_now_from_blockers();
        }
    }
}
