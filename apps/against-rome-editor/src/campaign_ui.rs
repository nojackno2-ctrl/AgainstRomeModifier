//! Campaign, Objectives, AI Waves & Cinematics Studio UI.
//! Replicates C# `ObjectiveStudioDialog.cs`, `CampaignWaveDialog.cs`, `MapEditorForm.StartCamera.cs`
//! and integrates `native/against-rome-campaign`.

use crate::scenario_ui::ScenarioUi;
use against_rome_campaign::{
    ai::{
        AiArchetypeCatalog, CampaignCompilationResult, CampaignMissionCompiler,
        CampaignMissionPlan, WavePlanParameters,
    },
    cinematics::{
        CameraEasingType, CameraTrackSplinePlanner, CameraWaypoint, CinematicBciCompiler,
        CutsceneSequence, CutsceneSequenceCatalog, MapStartCamera, MapStartCameraInjector,
        StartCameraPrimitive, SubtitleKeyframe,
    },
    objectives::{
        BciObjectiveCompiler, ObjectiveCategory, ObjectiveDefinition, ObjectiveDependencyGraph,
        ObjectiveKind, ObjectiveReward,
    },
};
use against_rome_scenario::model::{Event, ScenarioDocument};
#[cfg(test)]
use against_rome_scenario::{model::Action, WorldPosition};
use eframe::egui::{self, Color32, Vec2};
use std::path::{Path, PathBuf};
use uuid::Uuid;

struct StartCameraCandidate {
    map: PathBuf,
    camera: MapStartCamera,
    watched: Vec<(PathBuf, Option<Vec<u8>>)>,
    original: Vec<u8>,
}

fn camera_input(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Typed outcome of merging campaign waves into a ScenarioDocument.
#[derive(Clone, Debug)]
pub struct CampaignWaveMergeOutcome {
    /// The compilation result containing compiled events and diagnostics.
    pub compilation: CampaignCompilationResult,
    /// Number of new events successfully merged into the scenario document.
    pub added_events_count: usize,
    /// Total number of events in the scenario document after merging.
    pub total_events_after_merge: usize,
}

/// Error returned when wave validation or merge compilation fails.
#[derive(Clone, Debug, PartialEq)]
pub struct CampaignWaveMergeError {
    /// Summary error message.
    pub message: String,
    /// Detailed diagnostic messages.
    pub diagnostics: Vec<String>,
}

impl std::fmt::Display for CampaignWaveMergeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CampaignWaveMergeError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampaignTab {
    ObjectivesStudio,
    AiWaves,
    CameraCinematics,
}

pub struct CampaignUi {
    pub open: bool,
    pub current_tab: CampaignTab,

    // --- Objectives Studio state ---
    pub objectives: Vec<ObjectiveDefinition>,
    pub selected_objective_idx: Option<usize>,
    pub obj_title_input: String,
    pub obj_desc_input: String,
    pub obj_kind: ObjectiveKind,
    pub obj_category: ObjectiveCategory,
    pub obj_hold_duration: i32,
    /// Target object GUID (hero/building/VIP); required by most objective kinds.
    pub obj_target_guid: String,
    pub last_objective_diagnostics: Vec<String>,
    pub obj_reward_msg: String,
    pub last_compiled_objective_events: Vec<Event>,

    // --- Campaign Waves & AI state ---
    pub archetypes: Vec<against_rome_campaign::ai::AiArchetypeProfile>,
    pub selected_archetype_id: String,
    pub wave_count: i32,
    pub first_delay_seconds: i32,
    pub interval_seconds: i32,
    pub squad_count: i32,
    pub wave_team: i32,
    pub wave_spawn_x: f32,
    pub wave_spawn_z: f32,
    pub move_after_spawn: bool,
    pub dest_x: f32,
    pub dest_z: f32,
    pub last_wave_result: Option<CampaignCompilationResult>,
    /// Set by the "Add to Scenario" button; the app drains it and merges through the scenario session.
    wave_merge_requested: bool,
    pub wave_aliases_text: String,

    // --- Camera & Cinematics state ---
    pub start_camera: MapStartCamera,
    /// Explicit source for a new script work copy; never the active map's save target.
    start_camera_source: String,
    camera_dirty: bool,
    camera_candidate: Option<StartCameraCandidate>,
    camera_apply_requested: bool,
    camera_capture_requested: bool,
    pub cutscene_catalog: CutsceneSequenceCatalog,
    pub camera_waypoints: Vec<CameraWaypoint>,
    pub cutscene_subtitle: String,
    pub flight_pose_count: usize,
    pub last_cinematic_events: Vec<Event>,

    // Dirty / guards
    pub dirty: bool,
    pub pending_discard: bool,
    pub status_message: String,
    pub status_is_error: bool,
}

impl Default for CampaignUi {
    fn default() -> Self {
        let archetypes = AiArchetypeCatalog::all();
        let default_arch_id = archetypes
            .first()
            .map(|p| p.id.clone())
            .unwrap_or_else(|| AiArchetypeCatalog::ID_NOMADIC_RAIDER.to_string());
        let default_aliases = archetypes
            .first()
            .map(|p| {
                p.unit_preferences
                    .iter()
                    .map(|u| u.alias.clone())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        let cutscenes = CutsceneSequenceCatalog::new();

        Self {
            open: false,
            current_tab: CampaignTab::ObjectivesStudio,
            objectives: Vec::new(),
            selected_objective_idx: None,
            obj_title_input: "Defeat Enemy Base".to_string(),
            obj_desc_input: "Destroy the enemy main fortress and eliminate their forces."
                .to_string(),
            obj_kind: ObjectiveKind::EliminateAllEnemies,
            obj_category: ObjectiveCategory::Primary,
            obj_hold_duration: 0,
            obj_target_guid: String::new(),
            last_objective_diagnostics: Vec::new(),
            obj_reward_msg: "Victory achieved!".to_string(),
            last_compiled_objective_events: Vec::new(),
            archetypes,
            selected_archetype_id: default_arch_id,
            wave_count: 3,
            first_delay_seconds: 60,
            interval_seconds: 60,
            squad_count: 5,
            wave_team: 1,
            wave_spawn_x: 8000.0,
            wave_spawn_z: 8000.0,
            move_after_spawn: true,
            dest_x: 4000.0,
            dest_z: 4000.0,
            last_wave_result: None,
            wave_merge_requested: false,
            wave_aliases_text: default_aliases,
            start_camera: MapStartCamera {
                enabled: true,
                x: 8192.0,
                y: 0.0,
                z: 8192.0,
                zoom: MapStartCamera::DEFAULT_ZOOM,
                primitive: StartCameraPrimitive::CenterToPos,
            },
            cutscene_catalog: cutscenes,
            start_camera_source: String::new(),
            camera_dirty: false,
            camera_candidate: None,
            camera_apply_requested: false,
            camera_capture_requested: false,
            camera_waypoints: vec![
                CameraWaypoint::new(
                    [1000.0, 0.0, 1000.0],
                    30.0,
                    45.0,
                    82.0,
                    0.0,
                    CameraEasingType::SmoothStep,
                    None,
                ),
                CameraWaypoint::new(
                    [5000.0, 0.0, 5000.0],
                    40.0,
                    90.0,
                    82.0,
                    5.0,
                    CameraEasingType::SmoothStep,
                    None,
                ),
                CameraWaypoint::new(
                    [8000.0, 0.0, 8000.0],
                    30.0,
                    135.0,
                    82.0,
                    5.0,
                    CameraEasingType::SmoothStep,
                    None,
                ),
            ],
            cutscene_subtitle: "The enemy approaches!".to_string(),
            flight_pose_count: 0,
            last_cinematic_events: Vec::new(),
            dirty: false,
            pending_discard: false,
            status_message: "Ready.".to_string(),
            status_is_error: false,
        }
    }
}

impl CampaignUi {
    pub fn is_dirty(&self) -> bool {
        self.camera_dirty || self.camera_candidate.is_some() || self.has_unsupported_edits()
    }

    pub(crate) fn has_unsupported_edits(&self) -> bool {
        self.dirty
            || !self.objectives.is_empty()
            || self.last_wave_result.is_some()
            || !self.last_cinematic_events.is_empty()
            || self.flight_pose_count > 0
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn clear_dirty(&mut self) {
        self.reset_session();
    }

    /// Resets all map-bound campaign objectives, compiled events, wave compiler results,
    /// start camera, camera waypoints, and flight pose count.
    /// Preserves user preferences (open state, current tab, archetype profiles, wave input templates).
    pub fn reset_session(&mut self) {
        self.dirty = false;
        self.pending_discard = false;
        self.objectives.clear();
        self.selected_objective_idx = None;
        self.last_compiled_objective_events.clear();
        self.last_wave_result = None;
        self.wave_merge_requested = false;
        self.last_cinematic_events.clear();
        self.last_objective_diagnostics.clear();
        self.camera_waypoints.clear();
        self.flight_pose_count = 0;
        self.start_camera_source.clear();
        self.camera_dirty = false;
        self.camera_candidate = None;
        self.camera_apply_requested = false;
        self.camera_capture_requested = false;
        self.start_camera = MapStartCamera {
            enabled: false,
            x: 8192.0,
            y: 0.0,
            z: 8192.0,
            zoom: MapStartCamera::DEFAULT_ZOOM,
            primitive: StartCameraPrimitive::CenterToPos,
        };
        self.status_message = "Ready.".to_string();
        self.status_is_error = false;
    }

    pub fn request_close(&mut self) {
        if self.is_dirty() {
            self.pending_discard = true;
        } else {
            self.open = false;
        }
    }

    pub fn add_objective(&mut self) {
        let mut obj = ObjectiveDefinition::default();
        obj.title = self.obj_title_input.clone();
        obj.description = self.obj_desc_input.clone();
        obj.kind = self.obj_kind;
        obj.category = self.obj_category;
        obj.parameters.hold_duration_seconds = self.obj_hold_duration;
        if let Ok(guid) = Uuid::parse_str(self.obj_target_guid.trim()) {
            obj.parameters.target_guids.push(guid);
        }

        if !self.obj_reward_msg.trim().is_empty() {
            obj.reward = Some(ObjectiveReward::new(self.obj_reward_msg.clone(), vec![]));
        }

        self.objectives.push(obj);
        self.dirty = true;
        self.status_message = format!("Added objective '{}'.", self.obj_title_input);
        self.status_is_error = false;
    }

    /// Objective events reference spawns by GUID, so the live scenario is required for validation.
    pub fn compile_objectives_to_events(
        &mut self,
        scenario: Option<&ScenarioDocument>,
        aliases: &[String],
    ) -> Result<(), String> {
        let mut graph = ObjectiveDependencyGraph::new();
        for obj in &self.objectives {
            graph.add_objective(obj.clone())?;
        }

        let result = BciObjectiveCompiler::compile(
            &graph,
            scenario,
            (!aliases.is_empty()).then_some(aliases),
        );
        self.last_objective_diagnostics = result
            .diagnostics
            .iter()
            .map(|d| format!("[{:?}] {}: {}", d.severity, d.code, d.message))
            .collect();
        if !result.success {
            let err = format!(
                "Objective compilation failed: {}",
                self.last_objective_diagnostics
                    .first()
                    .cloned()
                    .unwrap_or_else(|| result.summary.clone())
            );
            self.status_message = err.clone();
            self.status_is_error = true;
            return Err(err);
        }
        self.last_compiled_objective_events = result.compiled_events;
        self.dirty = true;
        self.status_message = format!(
            "Compiled {} objectives into {} scenario events.",
            self.objectives.len(),
            self.last_compiled_objective_events.len()
        );
        self.status_is_error = false;
        Ok(())
    }

    /// Extracts canonical `WavePlanParameters` from current UI state.
    pub fn wave_parameters(&self) -> WavePlanParameters {
        let mut params = WavePlanParameters::new(
            self.selected_archetype_id.clone(),
            self.wave_count,
            self.first_delay_seconds,
            self.interval_seconds,
            self.squad_count,
            self.wave_team,
            self.wave_spawn_x,
            self.wave_spawn_z,
        );
        if self.move_after_spawn {
            params.move_destination =
                Some((self.dest_x.round() as i32, self.dest_z.round() as i32));
        }
        params
    }

    /// Resolves wave unit aliases: uses live aliases if non-empty, otherwise parses `wave_aliases_text`.
    pub fn resolved_wave_aliases(&self, live_aliases: &[String]) -> Vec<String> {
        if !live_aliases.is_empty() {
            live_aliases.to_vec()
        } else {
            self.wave_aliases_text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect()
        }
    }

    /// Builds a canonical `CampaignMissionPlan` from current parameters and archetype.
    pub fn build_wave_plan(&self) -> Result<CampaignMissionPlan, String> {
        let params = self.wave_parameters();
        params.validate().map_err(|e| {
            format!("波次參數驗證失敗：{e} | Wave parameters validation failed: {e}")
        })?;
        let archetype = AiArchetypeCatalog::get(&self.selected_archetype_id)
            .map_err(|e| format!("AI 原型無效：{e} | Invalid AI archetype: {e}"))?;
        Ok(params.build_plan(&archetype))
    }

    /// Compiles campaign waves against a scenario document (read-only preview/verification).
    ///
    /// If `scenario` is `None`, an empty default `ScenarioDocument` is used.
    /// Uses `CampaignMissionCompiler::compile_for_merge` for canonical validation and deduplication.
    pub fn compile_wave_attack(
        &mut self,
        scenario: Option<&ScenarioDocument>,
        live_aliases: &[String],
    ) -> Result<CampaignCompilationResult, String> {
        let plan = self.build_wave_plan().map_err(|e| {
            self.status_message = e.clone();
            self.status_is_error = true;
            e
        })?;

        let aliases = self.resolved_wave_aliases(live_aliases);
        let default_doc;
        let target_doc = match scenario {
            Some(s) => s,
            None => {
                default_doc = ScenarioDocument::default();
                &default_doc
            }
        };

        let result = CampaignMissionCompiler::compile_for_merge(&plan, &aliases, target_doc);
        self.last_wave_result = Some(result.clone());

        if result.success {
            self.status_message = format!(
                "Campaign AI Wave compilation succeeded: {} waves, {} events.",
                self.wave_count,
                result.compiled_events.len()
            );
            self.status_is_error = false;
            self.dirty = true;
            Ok(result)
        } else {
            let err = format!("Wave compilation failed: {}", result.diagnostics.join("; "));
            self.status_message = err.clone();
            self.status_is_error = true;
            Err(err)
        }
    }

    /// Compiles and merges a given `CampaignMissionPlan` into `scenario`.
    ///
    /// Replicates C# `MapEditorForm.ApplyCampaignWaves`.
    /// Validates and compiles with `compile_for_merge`. If any error occurs (unsupported fields,
    /// validation failure, or exceeding 256 events), `scenario` is GUARANTEED to remain unmutated (fail-closed).
    pub fn apply_plan_to_document(
        &mut self,
        plan: &CampaignMissionPlan,
        scenario: &mut ScenarioDocument,
        live_aliases: &[String],
    ) -> Result<CampaignWaveMergeOutcome, CampaignWaveMergeError> {
        let aliases = self.resolved_wave_aliases(live_aliases);
        let result = CampaignMissionCompiler::compile_for_merge(plan, &aliases, scenario);
        self.last_wave_result = Some(result.clone());

        if !result.success {
            let err_msg = format!(
                "Wave compile/merge failed: {}",
                result.diagnostics.join("; ")
            );
            self.status_message = err_msg.clone();
            self.status_is_error = true;
            return Err(CampaignWaveMergeError {
                message: err_msg,
                diagnostics: result.diagnostics,
            });
        }

        let added_count = result.compiled_events.len();
        scenario.events.extend(result.compiled_events.clone());
        self.dirty = true;
        self.status_message = format!(
            "Campaign AI Wave compilation succeeded: {} waves, {} events merged (total: {}).",
            self.wave_count,
            added_count,
            scenario.events.len(),
        );
        self.status_is_error = false;

        Ok(CampaignWaveMergeOutcome {
            compilation: result,
            added_events_count: added_count,
            total_events_after_merge: scenario.events.len(),
        })
    }

    /// Compiles current wave UI parameters and merges them into `scenario`.
    ///
    /// Exposes a merge-safe document update contract matching C# `ApplyCampaignWaves`.
    /// If parameter validation, archetype resolution, or compilation fails, `scenario` is NOT mutated.
    pub fn apply_waves_to_document(
        &mut self,
        scenario: &mut ScenarioDocument,
        live_aliases: &[String],
    ) -> Result<CampaignWaveMergeOutcome, CampaignWaveMergeError> {
        let plan = match self.build_wave_plan() {
            Ok(p) => p,
            Err(e) => {
                self.status_message = e.clone();
                self.status_is_error = true;
                return Err(CampaignWaveMergeError {
                    message: e.clone(),
                    diagnostics: vec![format!("[Error] {e}")],
                });
            }
        };

        self.apply_plan_to_document(&plan, scenario, live_aliases)
    }

    /// Alias for `apply_waves_to_document`.
    pub fn apply_wave_attack(
        &mut self,
        scenario: &mut ScenarioDocument,
        live_aliases: &[String],
    ) -> Result<CampaignWaveMergeOutcome, CampaignWaveMergeError> {
        self.apply_waves_to_document(scenario, live_aliases)
    }

    pub fn request_wave_merge(&mut self) {
        self.wave_merge_requested = true;
    }

    /// Returns true once if the user asked to merge the compiled AI waves into the scenario.
    pub fn take_wave_merge_request(&mut self) -> bool {
        std::mem::take(&mut self.wave_merge_requested)
    }

    /// Recompiles the current wave parameters against the live scenario and merges the events
    /// through `ScenarioUi::add_events` (validation, 256-event cap, one undo step). On any failure
    /// the scenario document and its event session are left untouched.
    pub fn merge_waves_into_scenario(
        &mut self,
        scenario: &mut ScenarioUi,
    ) -> Result<CampaignWaveMergeOutcome, CampaignWaveMergeError> {
        let fail = |this: &mut Self, message: String, diagnostics: Vec<String>| {
            this.status_message = message.clone();
            this.status_is_error = true;
            CampaignWaveMergeError {
                message,
                diagnostics,
            }
        };
        let plan = match self.build_wave_plan() {
            Ok(p) => p,
            Err(e) => return Err(fail(self, e.clone(), vec![format!("[Error] {e}")])),
        };
        let aliases = self.resolved_wave_aliases(scenario.known_aliases());
        let result =
            CampaignMissionCompiler::compile_for_merge(&plan, &aliases, scenario.document());
        self.last_wave_result = Some(result.clone());
        if !result.success {
            let msg = format!(
                "Wave compile/merge failed: {}",
                result.diagnostics.join("; ")
            );
            return Err(fail(self, msg, result.diagnostics));
        }
        let added = result.compiled_events.len();
        if added > 0 {
            if let Err(e) = scenario.add_events(&result.compiled_events) {
                let msg = format!("Wave merge rejected by scenario: {e}");
                return Err(fail(self, msg, vec![format!("[Error] {e}")]));
            }
        }
        let total = scenario.document().events.len();
        self.last_wave_result = None;
        self.dirty = false;
        self.status_message =
            format!("Merged {added} AI wave events into the scenario (total: {total}).");
        self.status_is_error = false;
        Ok(CampaignWaveMergeOutcome {
            compilation: result,
            added_events_count: added,
            total_events_after_merge: total,
        })
    }

    pub fn compile_cinematics_flight(&mut self) -> Result<(), String> {
        if self.camera_waypoints.len() < 2 {
            return Err("At least 2 camera waypoints are required.".into());
        }
        let planner = CameraTrackSplinePlanner::new(&self.camera_waypoints);
        let poses = planner.sample_path(30);

        let mut sequence = CutsceneSequence {
            name: "Flight".to_string(),
            camera_waypoints: self.camera_waypoints.clone(),
            ..Default::default()
        };
        if !self.cutscene_subtitle.trim().is_empty() {
            sequence.subtitles.push(SubtitleKeyframe::new(
                0.0,
                4.0,
                "",
                self.cutscene_subtitle.clone(),
            ));
        }
        let events = CinematicBciCompiler::compile_to_scenario_events(&sequence);

        self.flight_pose_count = poses.len();
        self.last_cinematic_events = events;
        self.dirty = true;
        self.status_message = format!(
            "Planned spline flight: {} camera poses ({:.1}s); {} subtitle events compiled (camera moves are preview-only).",
            poses.len(),
            planner.total_duration(),
            self.last_cinematic_events.len()
        );
        self.status_is_error = false;
        Ok(())
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        scenario: Option<&ScenarioDocument>,
        aliases: &[String],
    ) {
        if !self.open {
            return;
        }

        let mut is_open = self.open;
        egui::Window::new("Campaign, Objectives & Cinematics Studio")
            .open(&mut is_open)
            .min_size(Vec2::new(760.0, 560.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.current_tab,
                        CampaignTab::ObjectivesStudio,
                        "Objectives Studio",
                    );
                    ui.selectable_value(
                        &mut self.current_tab,
                        CampaignTab::AiWaves,
                        "AI Waves & Missions",
                    );
                    ui.selectable_value(
                        &mut self.current_tab,
                        CampaignTab::CameraCinematics,
                        "Camera & Cinematics",
                    );
                });
                ui.separator();

                if !self.status_message.is_empty() {
                    let color = if self.status_is_error {
                        Color32::from_rgb(250, 100, 100)
                    } else {
                        Color32::from_rgb(120, 220, 120)
                    };
                    ui.colored_label(color, &self.status_message);
                    ui.separator();
                }

                match self.current_tab {
                    CampaignTab::ObjectivesStudio => {
                        self.render_objectives_tab(ui, scenario, aliases)
                    }
                    CampaignTab::AiWaves => self.render_waves_tab(ui, scenario, aliases),
                    CampaignTab::CameraCinematics => self.render_camera_tab(ui),
                }
            });

        if !is_open {
            self.request_close();
        }

        if self.pending_discard {
            let mut close_modal = false;
            let mut confirm = false;

            egui::Window::new("Discard Unsaved Campaign Edits?")
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Closing will discard currently configured objectives, wave plans, and cinematic tracks.");
                    ui.horizontal(|ui| {
                        if ui.button("Discard & Close").clicked() {
                            confirm = true;
                        }
                        if ui.button("Keep Editing").clicked() {
                            close_modal = true;
                        }
                    });
                });

            if confirm {
                self.clear_dirty();
                self.open = false;
            } else if close_modal {
                self.pending_discard = false;
            }
        }
    }

    fn render_objectives_tab(
        &mut self,
        ui: &mut egui::Ui,
        scenario: Option<&ScenarioDocument>,
        aliases: &[String],
    ) {
        ui.heading("Campaign Objectives Studio");
        ui.label("Designs primary, bonus, and failure objectives with dependency graphs and BCI compilation.");

        ui.add_space(6.0);
        egui::Grid::new("obj_input_grid")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Title:");
                ui.text_edit_singleline(&mut self.obj_title_input);
                ui.end_row();

                ui.label("Description:");
                ui.text_edit_multiline(&mut self.obj_desc_input);
                ui.end_row();

                ui.label("Objective Kind:");
                egui::ComboBox::from_id_source("obj_kind_select")
                    .selected_text(format!("{:?}", self.obj_kind))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.obj_kind,
                            ObjectiveKind::EliminateAllEnemies,
                            "Eliminate All Enemies",
                        );
                        ui.selectable_value(
                            &mut self.obj_kind,
                            ObjectiveKind::Survival,
                            "Survival (Hold Timer)",
                        );
                        ui.selectable_value(
                            &mut self.obj_kind,
                            ObjectiveKind::CaptureArea,
                            "Capture Area",
                        );
                        ui.selectable_value(
                            &mut self.obj_kind,
                            ObjectiveKind::AssassinateTarget,
                            "Assassinate Target",
                        );
                        ui.selectable_value(
                            &mut self.obj_kind,
                            ObjectiveKind::DestroyBuilding,
                            "Destroy Building",
                        );
                        ui.selectable_value(
                            &mut self.obj_kind,
                            ObjectiveKind::EscortUnit,
                            "Escort Unit",
                        );
                    });
                ui.end_row();

                ui.label("Category:");
                egui::ComboBox::from_id_source("obj_cat_select")
                    .selected_text(format!("{:?}", self.obj_category))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.obj_category,
                            ObjectiveCategory::Primary,
                            "Primary Objective",
                        );
                        ui.selectable_value(
                            &mut self.obj_category,
                            ObjectiveCategory::Bonus,
                            "Bonus / Secondary",
                        );
                        ui.selectable_value(
                            &mut self.obj_category,
                            ObjectiveCategory::FailureCriterion,
                            "Failure Criterion",
                        );
                    });
                ui.end_row();

                ui.label("Target object GUID:");
                ui.text_edit_singleline(&mut self.obj_target_guid);
                ui.end_row();

                ui.label("Hold Duration (s):");
                ui.add(egui::DragValue::new(&mut self.obj_hold_duration).clamp_range(0..=86400));
                ui.end_row();

                ui.label("Reward Message:");
                ui.text_edit_singleline(&mut self.obj_reward_msg);
                ui.end_row();
            });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Add Objective").clicked() {
                self.add_objective();
            }
            if ui.button("Compile to Scenario Events").clicked() {
                let _ = self.compile_objectives_to_events(scenario, aliases);
            }
        });

        ui.separator();
        ui.label(format!(
            "Configured Objectives: {} (Compiled Events: {})",
            self.objectives.len(),
            self.last_compiled_objective_events.len()
        ));
        for d in &self.last_objective_diagnostics {
            ui.colored_label(Color32::from_rgb(255, 180, 50), d);
        }
        egui::ScrollArea::vertical()
            .max_height(200.0)
            .show(ui, |ui| {
                for (idx, obj) in self.objectives.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{}. [{:?}] {} - {}",
                            idx + 1,
                            obj.category,
                            obj.title,
                            obj.description
                        ));
                    });
                }
            });
    }

    fn render_waves_tab(
        &mut self,
        ui: &mut egui::Ui,
        scenario: Option<&ScenarioDocument>,
        aliases: &[String],
    ) {
        ui.heading("Campaign Wave Attack & AI Planner");
        ui.label(
            "Configures recurring attack waves, tactical posture profiles, and waypoint routes.",
        );

        ui.add_space(6.0);
        egui::Grid::new("wave_settings")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("AI Archetype:");
                egui::ComboBox::from_id_source("archetype_select")
                    .selected_text(&self.selected_archetype_id)
                    .show_ui(ui, |ui| {
                        for arch in &self.archetypes {
                            if ui
                                .selectable_value(
                                    &mut self.selected_archetype_id,
                                    arch.id.clone(),
                                    format!("{} ({})", arch.display_name, arch.id),
                                )
                                .clicked()
                            {
                                self.wave_aliases_text = arch
                                    .unit_preferences
                                    .iter()
                                    .map(|u| u.alias.clone())
                                    .collect::<Vec<_>>()
                                    .join("\n");
                            }
                        }
                    });
                ui.end_row();

                ui.label("Known unit aliases:");
                ui.text_edit_multiline(&mut self.wave_aliases_text);
                ui.end_row();

                ui.label("Wave Count (1..256):");
                ui.add(egui::DragValue::new(&mut self.wave_count).clamp_range(1..=256));
                ui.end_row();

                ui.label("First Delay / Interval (s):");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.first_delay_seconds)
                            .prefix("First: ")
                            .clamp_range(0..=86400),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.interval_seconds)
                            .prefix("Interval: ")
                            .clamp_range(1..=86400),
                    );
                });
                ui.end_row();

                ui.label("Squad Unit Count (1..20):");
                ui.add(egui::DragValue::new(&mut self.squad_count).clamp_range(1..=20));
                ui.end_row();

                ui.label("Team (0..7):");
                ui.add(egui::DragValue::new(&mut self.wave_team).clamp_range(0..=7));
                ui.end_row();

                ui.label("Spawn Location (X, Z):");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.wave_spawn_x)
                            .prefix("X: ")
                            .clamp_range(0.0..=16383.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.wave_spawn_z)
                            .prefix("Z: ")
                            .clamp_range(0.0..=16383.0),
                    );
                });
                ui.end_row();

                ui.label("Move after Spawn:");
                ui.checkbox(&mut self.move_after_spawn, "");
                ui.end_row();

                if self.move_after_spawn {
                    ui.label("Destination (X, Z):");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut self.dest_x)
                                .prefix("X: ")
                                .clamp_range(0.0..=16383.0),
                        );
                        ui.add(
                            egui::DragValue::new(&mut self.dest_z)
                                .prefix("Z: ")
                                .clamp_range(0.0..=16383.0),
                        );
                    });
                    ui.end_row();
                }
            });

        ui.add_space(8.0);
        if ui.button("Compile AI Waves").clicked() {
            let _ = self.compile_wave_attack(scenario, aliases);
        }
        if self
            .last_wave_result
            .as_ref()
            .is_some_and(|r| r.success && !r.compiled_events.is_empty())
            && scenario.is_some()
            && ui.button("Add to Scenario").clicked()
        {
            self.request_wave_merge();
        }

        if let Some(res) = &self.last_wave_result {
            ui.separator();
            let status_text = if res.success {
                format!(
                    "Compilation Success: {} compiled events",
                    res.compiled_events.len()
                )
            } else {
                "Compilation Failed".to_string()
            };
            ui.label(status_text);
            for d in &res.diagnostics {
                let color = if d.starts_with("[Error]") {
                    Color32::from_rgb(250, 100, 100)
                } else if d.starts_with("[Warn]") {
                    Color32::from_rgb(255, 180, 50)
                } else {
                    Color32::from_rgb(120, 220, 120)
                };
                ui.colored_label(color, d);
            }
            if res.success && !res.compiled_events.is_empty() {
                egui::ScrollArea::vertical()
                    .max_height(140.0)
                    .show(ui, |ui| {
                        for ev in &res.compiled_events {
                            let action_summaries: Vec<String> = ev
                                .actions
                                .iter()
                                .map(|a| {
                                    let mut s =
                                        format!("{} × {} (team {})", a.alias, a.count, a.team);
                                    if let Some(dest) = &a.move_destination {
                                        s.push_str(&format!(" → ({}, {})", dest.x, dest.z));
                                    }
                                    s
                                })
                                .collect();
                            ui.label(format!(
                                "{}: {}s — {}",
                                ev.name,
                                ev.delay_seconds,
                                action_summaries.join(", ")
                            ));
                        }
                    });
            }
        }
    }

    pub(crate) fn take_camera_requests(&mut self) -> (bool, bool) {
        (
            std::mem::take(&mut self.camera_apply_requested),
            std::mem::take(&mut self.camera_capture_requested),
        )
    }

    pub(crate) fn capture_camera(&mut self, camera: MapStartCamera) {
        self.start_camera = camera;
        self.camera_dirty = true;
        self.status_message = "Captured current 3D view; Apply and Safe Save to persist.".into();
        self.status_is_error = false;
    }

    pub(crate) fn camera_pending(&self) -> bool {
        self.camera_candidate.is_some()
    }

    pub(crate) fn load_map_camera(&mut self, map: &Path) -> Result<(), String> {
        let camera = match camera_input(&map.join("arm_start_camera.json"))? {
            Some(bytes) => serde_json::from_slice::<MapStartCamera>(
                bytes.strip_prefix(&[239, 187, 191]).unwrap_or(&bytes),
            )
            .map_err(|e| e.to_string())?,
            None => MapStartCamera {
                enabled: false,
                ..Default::default()
            },
        };
        camera.validate()?;
        self.start_camera = camera;
        Ok(())
    }

    /// Publish only a fully validated candidate, preserving the prior candidate on failure.
    pub(crate) fn apply_camera_candidate(
        &mut self,
        map: &Path,
        scenario_files: &[(PathBuf, Vec<u8>)],
        aliases: &[String],
    ) -> Result<(), String> {
        let previous = self.camera_candidate.take();
        let previous_dirty = self.camera_dirty;
        let result = (|| {
            self.prepare_camera_candidate(map)?;
            self.stage_camera_candidate(map, scenario_files, aliases)?;
            Ok(())
        })();
        if result.is_err() {
            self.camera_candidate = previous;
            self.camera_dirty = previous_dirty;
        }
        result
    }

    /// Snapshot inputs without writes. The app owns the whole-map transaction.
    fn prepare_camera_candidate(&mut self, map: &Path) -> Result<(), String> {
        self.start_camera.validate()?;
        crate::storage::preflight_transaction_tree_opt(map, false)?;
        let map = std::fs::canonicalize(map).map_err(|e| e.to_string())?;
        let mut watched = Vec::new();
        for rel in [
            "SCRIPT/ak_level.bci",
            "SCRIPT/ak_level.arm_original",
            "arm_start_camera.json",
            "arm_scenario.json",
        ] {
            watched.push((PathBuf::from(rel), camera_input(&map.join(rel))?));
        }
        for pool in [
            "objects", "objdata", "position", "anim", "gfxtype", "action", "engine", "light",
            "particle", "lager", "biglager",
        ] {
            let rel = PathBuf::from(format!("DATA/{pool}.dat"));
            watched.push((rel.clone(), camera_input(&map.join(&rel))?));
        }
        let original = watched[1]
            .1
            .as_ref()
            .or(watched[0].1.as_ref())
            .ok_or("Loaded map has no original BCI source")?
            .clone();
        self.compile_start_camera_script(&original)?;
        self.camera_candidate = Some(StartCameraCandidate {
            map,
            camera: self.start_camera.clone(),
            watched,
            original,
        });
        self.camera_dirty = true;
        Ok(())
    }

    /// Recompile camera and events together from the immutable original, never the patched BCI.
    pub(crate) fn stage_camera_candidate(
        &self,
        map: &Path,
        scenario_files: &[(PathBuf, Vec<u8>)],
        aliases: &[String],
    ) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
        if self.camera_dirty && self.camera_candidate.is_none() {
            return Err("Start camera: Apply to the loaded map before Safe Save".into());
        }
        let Some(candidate) = &self.camera_candidate else {
            return Ok(scenario_files.to_vec());
        };
        if std::fs::canonicalize(map).map_err(|e| e.to_string())? != candidate.map
            || self.start_camera != candidate.camera
        {
            return Err(
                "Start camera candidate is stale or belongs to another map; Apply again".into(),
            );
        }
        for (rel, bytes) in &candidate.watched {
            if camera_input(&candidate.map.join(rel))? != *bytes {
                return Err(format!(
                    "Start camera source changed externally: {}; Apply again",
                    rel.display()
                ));
            }
        }
        let find = |rel: &str| {
            scenario_files
                .iter()
                .find(|(p, _)| p == Path::new(rel))
                .map(|(_, b)| b.as_slice())
        };
        if let Some(bytes) = find("SCRIPT/ak_level.arm_original") {
            if bytes != candidate.original {
                return Err("Scenario and camera original sources differ".into());
            }
        }
        // Simultaneous camera edits in Scenario must be resolved explicitly.
        if let Some(bytes) = find("arm_start_camera.json") {
            let scenario_camera: MapStartCamera =
                serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
            let original_camera = match &candidate.watched[2].1 {
                Some(b) => serde_json::from_slice::<MapStartCamera>(
                    b.strip_prefix(&[239, 187, 191]).unwrap_or(b),
                )
                .map_err(|e| e.to_string())?,
                None => MapStartCamera {
                    enabled: false,
                    ..Default::default()
                },
            };
            if scenario_camera != original_camera && scenario_camera != candidate.camera {
                return Err("Conflicting start camera edits in Scenario and Campaign".into());
            }
        }
        let mut document = match find("arm_scenario.json").or(candidate.watched[3].1.as_deref()) {
            Some(b) => ScenarioDocument::from_json(b)?,
            None => ScenarioDocument::default(),
        };
        document.version = 6;
        let active =
            if !document.data_slots.is_empty() || document.spawns.iter().any(|s| s.prebuilt) {
                let store =
                    against_rome_placement::ObjectStore::load(map).map_err(|e| e.to_string())?;
                Some(
                    store
                        .objects()
                        .iter()
                        .map(|o| (o.slot as i32, o.uid))
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            };
        let camera = against_rome_scenario::StartCamera::from(&candidate.camera);
        let script = against_rome_scenario::compile_script_verified(
            &candidate.original,
            &document,
            Some(&camera),
            if aliases.is_empty() {
                None
            } else {
                Some(aliases)
            },
            active.as_deref(),
        )?;
        against_rome_campaign::export::verify_exported_script(
            &candidate.original,
            &script,
            &document,
            Some(&camera),
        )?;
        let mut files = scenario_files.to_vec();
        for (rel, bytes) in [
            (
                "arm_scenario.json",
                serde_json::to_vec_pretty(&document).map_err(|e| e.to_string())?,
            ),
            (
                "arm_start_camera.json",
                serde_json::to_vec_pretty(&camera).map_err(|e| e.to_string())?,
            ),
            ("SCRIPT/ak_level.bci", script),
        ] {
            files.retain(|(p, _)| p != Path::new(rel));
            files.push((PathBuf::from(rel), bytes));
        }
        // Preserve an existing immutable backup byte-for-byte; create it only on first save.
        files.retain(|(p, _)| p != Path::new("SCRIPT/ak_level.arm_original"));
        if candidate.watched[1].1.is_none() {
            files.push((
                PathBuf::from("SCRIPT/ak_level.arm_original"),
                candidate.original.clone(),
            ));
        }
        Ok(files)
    }

    pub(crate) fn camera_saved(&mut self) {
        self.camera_candidate = None;
        self.camera_dirty = false;
        self.status_message = "Start camera saved in whole-map transaction.".into();
        self.status_is_error = false;
    }

    /// Compile from the supplied original on every export, so repeated exports do not
    /// accumulate camera shims. The native injector owns all camera bytecode generation.
    fn compile_start_camera_script(&self, source: &[u8]) -> Result<Vec<u8>, String> {
        self.start_camera.validate()?;
        let raw = if source.starts_with(b"PFIL") {
            against_rome_data::lzss::decompress_pfil(source, None).map_err(|e| e.to_string())?
        } else {
            source.to_vec()
        };
        let mut image = against_rome_scenario::BciImage::parse(&raw)?;
        MapStartCameraInjector::inject(&mut image, &self.start_camera)?;
        Ok(image.serialize())
    }

    /// Export a standalone raw BCI work copy. Existing files (including the source)
    /// cannot be overwritten. This deliberately does not apply to the loaded map or
    /// clear dirty state belonging to other campaign tools.
    fn export_start_camera_script(&mut self, target: &std::path::Path) -> Result<(), String> {
        let result = (|| {
            let source =
                std::fs::read(self.start_camera_source.trim()).map_err(|e| e.to_string())?;
            let bytes = self.compile_start_camera_script(&source)?;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target)
                .map_err(|e| e.to_string())?;
            use std::io::Write;
            if let Err(e) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
                drop(file);
                let _ = std::fs::remove_file(target);
                return Err(e.to_string());
            }
            Ok(())
        })();
        match &result {
            Ok(()) => {
                self.status_message = format!(
                    "Start camera script work copy exported: {}. Loaded map unchanged.",
                    target.display()
                );
                self.status_is_error = false;
            }
            Err(e) => {
                self.status_message = format!("Start camera export failed: {e}");
                self.status_is_error = true;
            }
        }
        result
    }

    fn render_camera_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Map Start Camera & Cinematic Tracks");
        ui.label("Specifies the map intro viewport camera position and multi-waypoint cinematic flight paths.");

        ui.add_space(6.0);
        let camera_before = self.start_camera.clone();
        egui::Grid::new("camera_settings")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Start Camera Enabled:");
                ui.checkbox(&mut self.start_camera.enabled, "");
                ui.end_row();

                ui.label("Start Position (X, Y, Z):");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.start_camera.x)
                            .prefix("X: ")
                            .clamp_range(0.0..=16383.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.start_camera.y)
                            .prefix("Y: ")
                            .clamp_range(-500.0..=5000.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.start_camera.z)
                            .prefix("Z: ")
                            .clamp_range(0.0..=16383.0),
                    );
                });
                ui.end_row();

                ui.label("Start Zoom:");
                ui.add(egui::Slider::new(&mut self.start_camera.zoom, 0.0..=9.0));
                ui.end_row();

                ui.label("Primitive:");
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.start_camera.primitive,
                        StartCameraPrimitive::CenterToPos,
                        "CenterToPos",
                    );
                    ui.selectable_value(
                        &mut self.start_camera.primitive,
                        StartCameraPrimitive::DirectEnginePos,
                        "DirectEnginePos",
                    );
                });
                ui.end_row();

                ui.label("Intro subtitle:");
                ui.text_edit_singleline(&mut self.cutscene_subtitle);
                ui.end_row();
            });

        if self.start_camera != camera_before {
            self.camera_dirty = true;
        }

        ui.horizontal(|ui| {
            if ui.button("Capture Current 3D View").clicked() {
                self.camera_capture_requested = true;
            }
            if ui.button("Apply Start Camera to Loaded Map").clicked() {
                self.camera_apply_requested = true;
            }
        });
        ui.label("Apply prepares a candidate. Safe Save writes camera, events and terrain together. Native zoom capture uses 5, as in C#.");
        ui.label("Export a new script work copy from an original BCI (raw or PFIL). Output is raw BCI; the loaded map is not saved. Disabling the camera adds no shim; it does not remove a camera already in the source.");
        ui.horizontal(|ui| {
            ui.label("Original script:");
            ui.text_edit_singleline(&mut self.start_camera_source);
            if ui.button("Browse BCI…").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("BCI script", &["bci", "arm_original"])
                    .pick_file()
                {
                    self.start_camera_source = path.to_string_lossy().into_owned();
                }
            }
        });
        if ui
            .add_enabled(
                !self.start_camera_source.trim().is_empty(),
                egui::Button::new("Export Start Camera Script Copy…"),
            )
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("BCI script", &["bci"])
                .set_file_name("start_camera_preview.bci")
                .save_file()
            {
                let _ = self.export_start_camera_script(&path);
            }
        }

        if ui.button("Validate Start Camera").clicked() {
            match self.start_camera.validate() {
                Ok(()) => {
                    self.status_message = "Start camera is valid.".to_string();
                    self.status_is_error = false;
                }
                Err(e) => {
                    self.status_message = format!("Start camera invalid: {e}");
                    self.status_is_error = true;
                }
            }
        }

        ui.separator();
        ui.heading("Cinematic Flight Tracks");
        ui.label(format!("Waypoints: {}", self.camera_waypoints.len()));

        if ui.button("Compile Spline Flight Path").clicked() {
            let _ = self.compile_cinematics_flight();
        }

        if self.flight_pose_count > 0 {
            ui.label(format!("Sampled camera poses: {}", self.flight_pose_count));
        }
        if !self.last_cinematic_events.is_empty() {
            ui.label(format!(
                "Compiled Cinematic Events: {}",
                self.last_cinematic_events.len()
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_camera_work_copy_uses_native_calls_and_returns_to_original_main() {
        use against_rome_scenario::BciImage;
        let original = BciImage::idle();
        let source = original.serialize();
        let mut ui = CampaignUi::default();
        for primitive in [
            StartCameraPrimitive::CenterToPos,
            StartCameraPrimitive::DirectEnginePos,
        ] {
            for zoom in [0.0, 9.0] {
                ui.start_camera.primitive = primitive;
                ui.start_camera.zoom = zoom;
                ui.start_camera.x = 100.5;
                ui.start_camera.z = 101.5;
                let bytes = ui.compile_start_camera_script(&source).unwrap();
                let image = BciImage::parse(&bytes).unwrap();
                assert_eq!(&image.code[..original.code.len()], &original.code);
                assert_eq!(image.main, original.code.len() as i32 * 4);
                assert_eq!(image.dtor, original.dtor);
                let mut expected = original.clone();
                let calls = CinematicBciCompiler::compile_start_camera_calls(
                    &mut expected,
                    &ui.start_camera,
                )
                .unwrap();
                assert_eq!(
                    &image.code[original.code.len()..image.code.len() - 2],
                    &calls
                );
                assert_eq!(image.constants, expected.constants);
                let jump_at = (image.code.len() as i32 - 2) * 4;
                assert_eq!(
                    image.code[image.code.len() - 2],
                    MapStartCameraInjector::OP_JUMP
                );
                assert_eq!(jump_at + 8 + image.code.last().unwrap(), original.main);
                assert_eq!(ui.compile_start_camera_script(&source).unwrap(), bytes);
            }
        }
        ui.start_camera.enabled = false;
        assert_eq!(ui.compile_start_camera_script(&source).unwrap(), source);
    }

    #[test]
    fn start_camera_pfil_source_matches_raw_source() {
        let source = against_rome_scenario::BciImage::idle().serialize();
        // Synthetic literal-only PFIL fixture, not an output serializer.
        let mut pfil = vec![0; 64];
        pfil[..4].copy_from_slice(b"PFIL");
        pfil[16..20].copy_from_slice(&(source.len() as u32).to_le_bytes());
        for chunk in source.chunks(8) {
            pfil.push(((1u16 << chunk.len()) - 1) as u8);
            pfil.extend_from_slice(chunk);
        }
        let ui = CampaignUi::default();
        assert_eq!(
            ui.compile_start_camera_script(&pfil).unwrap(),
            ui.compile_start_camera_script(&source).unwrap()
        );
    }

    struct CameraExportDir(std::path::PathBuf);
    impl CameraExportDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("arm-camera-ui-{}", Uuid::new_v4()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for CameraExportDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn start_camera_export_preserves_source_existing_targets_and_other_dirty_work() {
        let dir = CameraExportDir::new();
        let source_path = dir.0.join("original.bci");
        let source = against_rome_scenario::BciImage::idle().serialize();
        std::fs::write(&source_path, &source).unwrap();
        let mut ui = CampaignUi::default();
        ui.start_camera_source = source_path.to_string_lossy().into_owned();
        ui.add_objective();
        let output = dir.0.join("copy.bci");
        ui.export_start_camera_script(&output).unwrap();
        assert!(!ui.status_is_error);
        assert!(ui.is_dirty());
        assert_eq!(ui.objectives.len(), 1);
        let saved = std::fs::read(&output).unwrap();
        ui.start_camera.x = 2048.0;
        assert!(ui.export_start_camera_script(&output).is_err());
        assert!(ui.status_is_error);
        assert_eq!(std::fs::read(&output).unwrap(), saved);
        assert!(ui.export_start_camera_script(&source_path).is_err());
        assert_eq!(std::fs::read(&source_path).unwrap(), source);
        let second = dir.0.join("second.bci");
        ui.export_start_camera_script(&second).unwrap();
        assert_eq!(
            std::fs::read(second).unwrap(),
            ui.compile_start_camera_script(&source).unwrap()
        );
        ui.reset_session();
        assert!(ui.start_camera_source.is_empty());
    }

    #[test]
    fn start_camera_export_rejects_invalid_input_before_creating_output() {
        let dir = CameraExportDir::new();
        let source_path = dir.0.join("original.bci");
        let target = dir.0.join("copy.bci");
        let mut ui = CampaignUi::default();
        ui.start_camera_source = source_path.to_string_lossy().into_owned();
        assert!(ui.export_start_camera_script(&target).is_err());
        assert!(!target.exists());
        std::fs::write(&source_path, b"invalid BCI").unwrap();
        assert!(ui.export_start_camera_script(&target).is_err());
        assert!(!target.exists());
        std::fs::write(&source_path, b"PFIL").unwrap();
        assert!(ui.export_start_camera_script(&target).is_err());
        assert!(!target.exists());
        let source = against_rome_scenario::BciImage::idle().serialize();
        std::fs::write(&source_path, &source).unwrap();
        for (x, y, zoom) in [
            (f64::NAN, 0.0, 5.0),
            (0.0, f64::INFINITY, 5.0),
            (0.0, 0.0, 10.0),
        ] {
            ui.start_camera.x = x;
            ui.start_camera.y = y;
            ui.start_camera.zoom = zoom;
            assert!(ui.export_start_camera_script(&target).is_err());
            assert!(ui.status_is_error);
            assert!(!target.exists());
            assert_eq!(std::fs::read(&source_path).unwrap(), source);
        }
    }

    #[test]
    fn test_campaign_ui_initial_state_and_dirty_guards() {
        let mut ui = CampaignUi::default();
        assert!(!ui.open);
        assert!(!ui.is_dirty());

        ui.mark_dirty();
        assert!(ui.is_dirty());

        ui.request_close();
        assert!(ui.pending_discard);

        ui.clear_dirty();
        assert!(!ui.is_dirty());
        assert!(!ui.pending_discard);
    }

    #[test]
    fn test_campaign_ui_add_objective_and_compile() {
        let mut ui = CampaignUi::default();
        ui.obj_title_input = "Protect Fortress".to_string();
        ui.obj_kind = ObjectiveKind::Survival;
        ui.obj_hold_duration = 180;
        ui.obj_target_guid = Uuid::new_v4().to_string();
        ui.add_objective();

        assert_eq!(ui.objectives.len(), 1);
        let mut scenario = ScenarioDocument::default();
        let mut spawn = against_rome_scenario::model::Spawn::default();
        spawn.id = ui.objectives[0].parameters.target_guids[0];
        spawn.alias = "GER_INF00".to_string();
        scenario.spawns.push(spawn);
        let compiled = ui.compile_objectives_to_events(Some(&scenario), &[]);
        assert!(
            compiled.is_ok(),
            "{compiled:?} {:?}",
            ui.last_objective_diagnostics
        );
        assert!(!ui.last_compiled_objective_events.is_empty());
        assert!(ui.is_dirty());
    }

    #[test]
    fn merge_waves_goes_through_scenario_session_with_undo_redo() {
        let mut ui = CampaignUi::default();
        ui.wave_count = 2;
        let mut scenario = ScenarioUi::default();
        let out = ui.merge_waves_into_scenario(&mut scenario).unwrap();
        assert!(out.added_events_count > 0);
        assert_eq!(
            scenario.document().events.len(),
            out.total_events_after_merge
        );
        assert!(scenario.dirty() && scenario.can_undo_events());
        assert!(ui.last_wave_result.is_none() && !ui.status_is_error);
        assert!(scenario.undo_events());
        assert!(scenario.document().events.is_empty());
        assert!(scenario.redo_events());
        assert_eq!(scenario.document().events.len(), out.added_events_count);
    }

    #[test]
    fn merge_waves_over_256_cap_or_bad_params_leaves_scenario_untouched() {
        let mut ui = CampaignUi::default();
        ui.wave_count = 2;
        let mut scenario = ScenarioUi::default();
        let filler: Vec<Event> = (0..255)
            .map(|i| Event {
                name: format!("f{i}"),
                actions: vec![Action {
                    text: "m".into(),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .collect();
        scenario.add_events(&filler).unwrap();
        let before = scenario.document().events.clone();
        assert!(ui.merge_waves_into_scenario(&mut scenario).is_err());
        assert!(ui.status_is_error);
        assert_eq!(scenario.document().events, before);
        assert!(scenario.undo_events());
        assert!(scenario.document().events.is_empty());
        assert!(!scenario.undo_events());

        let mut bad = CampaignUi::default();
        bad.wave_count = 0;
        let mut fresh = ScenarioUi::default();
        assert!(bad.merge_waves_into_scenario(&mut fresh).is_err());
        assert!(fresh.document().events.is_empty() && !fresh.can_undo_events());
    }

    #[test]
    fn test_campaign_ui_wave_compilation() {
        let mut ui = CampaignUi::default();
        ui.wave_count = 2;
        assert!(ui.compile_wave_attack(None, &[]).is_ok());
        assert!(ui.last_wave_result.as_ref().unwrap().success);
        assert!(ui.is_dirty());
    }

    #[test]
    fn test_wave_merge_alias_collision() {
        let mut ui = CampaignUi::default();
        ui.wave_count = 2; // will generate Wave_01 and Wave_02

        let mut scenario = ScenarioDocument::default();
        // Add an existing event named "Wave_01" to trigger collision handling
        scenario.events.push(Event {
            name: "Wave_01".to_string(),
            delay_seconds: 30,
            repeat: false,
            enabled: true,
            actions: vec![Action {
                kind: 0,
                text: "Existing initial wave".to_string(),
                ..Default::default()
            }],
            conditions: vec![],
        });

        let outcome = ui
            .apply_waves_to_document(&mut scenario, &[])
            .expect("merge must succeed with deduplicated event names");

        assert_eq!(outcome.added_events_count, 2);
        assert_eq!(outcome.total_events_after_merge, 3);
        assert_eq!(scenario.events.len(), 3);

        // Original event is preserved unmutated
        assert_eq!(scenario.events[0].name, "Wave_01");
        assert_eq!(scenario.events[0].actions[0].text, "Existing initial wave");

        // Colliding wave event is deduplicated to "Wave_01_2"
        assert_eq!(scenario.events[1].name, "Wave_01_2");
        // Non-colliding wave event is "Wave_02"
        assert_eq!(scenario.events[2].name, "Wave_02");

        // Verify that the scenario is valid with known aliases
        let aliases = ui.resolved_wave_aliases(&[]);
        assert!(scenario.validate(Some(&aliases)).is_ok());
    }

    #[test]
    fn test_wave_merge_event_cap() {
        let mut ui = CampaignUi::default();
        ui.wave_count = 2; // will attempt to add 2 events

        let mut scenario = ScenarioDocument::default();
        // Fill scenario with 255 valid events
        for i in 0..255 {
            scenario.events.push(Event {
                name: format!("Event_{i:03}"),
                delay_seconds: 1,
                repeat: false,
                enabled: true,
                actions: vec![Action {
                    kind: 0,
                    text: format!("Message {i}"),
                    ..Default::default()
                }],
                conditions: vec![],
            });
        }
        assert_eq!(scenario.events.len(), 255);

        // 255 + 2 = 257 > 256: merge must fail and scenario must NOT be mutated!
        let err = ui
            .apply_waves_to_document(&mut scenario, &[])
            .expect_err("merging beyond 256 events must fail");

        assert!(
            err.diagnostics.iter().any(|d| d.contains("256")) || err.message.contains("256"),
            "Error should report 256 event cap: {:?}",
            err
        );

        // Verify that the input document was NOT mutated (fail-closed)
        assert_eq!(scenario.events.len(), 255);
        assert_eq!(scenario.events[0].name, "Event_000");
        assert_eq!(scenario.events[254].name, "Event_254");
    }

    #[test]
    fn test_wave_merge_invalid_field() {
        let mut ui = CampaignUi::default();
        let mut scenario = ScenarioDocument::default();
        scenario.events.push(Event {
            name: "InitialEvent".to_string(),
            delay_seconds: 5,
            repeat: false,
            enabled: true,
            actions: vec![Action {
                kind: 0,
                text: "Keep me intact".to_string(),
                ..Default::default()
            }],
            conditions: vec![],
        });

        // 1. Invalid wave count: 0 (must be 1..=256)
        ui.wave_count = 0;
        let err = ui
            .apply_waves_to_document(&mut scenario, &[])
            .expect_err("wave_count 0 must fail");
        assert!(err.message.contains("波次數") || err.message.contains("Wave parameters"));
        assert_eq!(
            scenario.events.len(),
            1,
            "scenario must not be mutated on invalid wave count"
        );

        // 2. Invalid spawn coordinate: negative
        ui.wave_count = 2;
        ui.wave_spawn_x = -50.0;
        let err = ui
            .apply_waves_to_document(&mut scenario, &[])
            .expect_err("negative spawn coordinate must fail");
        assert!(err.message.contains("座標") || err.message.contains("coordinate"));
        assert_eq!(
            scenario.events.len(),
            1,
            "scenario must not be mutated on invalid coordinate"
        );

        // 3. Invalid team number: 99 (valid is 0..=7)
        ui.wave_spawn_x = 8000.0;
        ui.wave_team = 99;
        let err = ui
            .apply_waves_to_document(&mut scenario, &[])
            .expect_err("invalid team number must fail");
        assert!(err.message.contains("隊伍") || err.message.contains("team"));
        assert_eq!(
            scenario.events.len(),
            1,
            "scenario must not be mutated on invalid team"
        );

        // 4. Unsupported field in CampaignMissionPlan via apply_plan_to_document (e.g., assigned path)
        ui.wave_team = 1;
        let archetype = AiArchetypeCatalog::get(&ui.selected_archetype_id).unwrap();
        let mut unsupported_plan = ui.wave_parameters().build_plan(&archetype);
        unsupported_plan.waves[0].assigned_path_id = Some(Uuid::new_v4());

        let err = ui
            .apply_plan_to_document(&unsupported_plan, &mut scenario, &[])
            .expect_err("unsupported patrol path must fail merge");
        assert!(err
            .diagnostics
            .iter()
            .any(|d| d.contains("尚未支援") || d.contains("unsupported")));
        assert_eq!(
            scenario.events.len(),
            1,
            "scenario must not be mutated on unsupported plan field"
        );
        assert_eq!(scenario.events[0].name, "InitialEvent");
    }

    #[test]
    fn test_wave_merge_valid_merge() {
        let mut ui = CampaignUi::default();
        ui.wave_count = 3;
        ui.first_delay_seconds = 45;
        ui.interval_seconds = 30;
        ui.squad_count = 6;
        ui.wave_team = 2;
        ui.wave_spawn_x = 5000.0;
        ui.wave_spawn_z = 6000.0;
        ui.move_after_spawn = true;
        ui.dest_x = 2000.0;
        ui.dest_z = 3000.0;

        let mut scenario = ScenarioDocument::default();
        let outcome = ui
            .apply_waves_to_document(&mut scenario, &[])
            .expect("valid wave merge must succeed");

        assert_eq!(outcome.added_events_count, 3);
        assert_eq!(outcome.total_events_after_merge, 3);
        assert_eq!(scenario.events.len(), 3);

        // Check Wave 1: delay 45, spawn squad team 2 count 6 at (5000, 6000), move destination (2000, 3000)
        assert_eq!(scenario.events[0].name, "Wave_01");
        assert_eq!(scenario.events[0].delay_seconds, 45);
        for action in &scenario.events[0].actions {
            assert_eq!(action.kind, 2);
            assert_eq!(action.team, 2);
            assert_eq!(action.count, 6);
            assert_eq!(action.x, 5000.0);
            assert_eq!(action.z, 6000.0);
            assert_eq!(
                action.move_destination,
                Some(WorldPosition { x: 2000, z: 3000 })
            );
        }

        // Check Wave 2: delay 45 + 30 = 75
        assert_eq!(scenario.events[1].name, "Wave_02");
        assert_eq!(scenario.events[1].delay_seconds, 75);

        // Check Wave 3: delay 45 + 60 = 105
        assert_eq!(scenario.events[2].name, "Wave_03");
        assert_eq!(scenario.events[2].delay_seconds, 105);

        // Validate final scenario document structure
        let aliases = ui.resolved_wave_aliases(&[]);
        assert!(scenario.validate(Some(&aliases)).is_ok());

        assert!(ui.is_dirty());
        assert!(!ui.status_is_error);
    }

    #[test]
    fn test_campaign_ui_cinematic_flight_planning() {
        let mut ui = CampaignUi::default();
        assert!(ui.compile_cinematics_flight().is_ok());
        assert!(!ui.last_cinematic_events.is_empty());
        assert!(ui.is_dirty());
    }
}
