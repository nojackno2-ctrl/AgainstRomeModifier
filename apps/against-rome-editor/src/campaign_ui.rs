//! Campaign, Objectives, AI Waves & Cinematics Studio UI.
//! Replicates C# `ObjectiveStudioDialog.cs`, `CampaignWaveDialog.cs`, `MapEditorForm.StartCamera.cs`
//! and integrates `native/against-rome-campaign`.

use against_rome_campaign::{
    ai::{
        AiArchetypeCatalog, CampaignCompilationResult, CampaignMissionCompiler,
        CampaignMissionPlan, WaveAttackDefinition, WaveSquadDefinition,
    },
    cinematics::{
        CameraEasingType, CameraTrackSplinePlanner, CameraWaypoint, CinematicBciCompiler,
        CutsceneSequence, CutsceneSequenceCatalog, MapStartCamera, StartCameraPrimitive,
        SubtitleKeyframe,
    },
    objectives::{
        BciObjectiveCompiler, ObjectiveCategory, ObjectiveDefinition, ObjectiveDependencyGraph,
        ObjectiveKind, ObjectiveReward,
    },
};
use against_rome_scenario::{model::{Event, ScenarioDocument}, WorldPosition};
use eframe::egui::{self, Color32, Vec2};
use uuid::Uuid;

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
    pub wave_aliases_text: String,

    // --- Camera & Cinematics state ---
    pub start_camera: MapStartCamera,
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
            .map(|p| p.unit_preferences.iter().map(|u| u.alias.clone()).collect::<Vec<_>>().join("\n"))
            .unwrap_or_default();

        let cutscenes = CutsceneSequenceCatalog::new();

        Self {
            open: false,
            current_tab: CampaignTab::ObjectivesStudio,
            objectives: Vec::new(),
            selected_objective_idx: None,
            obj_title_input: "Defeat Enemy Base".to_string(),
            obj_desc_input: "Destroy the enemy main fortress and eliminate their forces.".to_string(),
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
            camera_waypoints: vec![
                CameraWaypoint::new([1000.0, 0.0, 1000.0], 30.0, 45.0, 82.0, 0.0, CameraEasingType::SmoothStep, None),
                CameraWaypoint::new([5000.0, 0.0, 5000.0], 40.0, 90.0, 82.0, 5.0, CameraEasingType::SmoothStep, None),
                CameraWaypoint::new([8000.0, 0.0, 8000.0], 30.0, 135.0, 82.0, 5.0, CameraEasingType::SmoothStep, None),
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
        self.dirty = false;
        self.objectives.clear();
        self.last_compiled_objective_events.clear();
        self.last_wave_result = None;
        self.last_cinematic_events.clear();
        self.last_objective_diagnostics.clear();
        self.flight_pose_count = 0;
        self.pending_discard = false;
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
    pub fn compile_objectives_to_events(&mut self, scenario: Option<&ScenarioDocument>, aliases: &[String]) -> Result<(), String> {
        let mut graph = ObjectiveDependencyGraph::new();
        for obj in &self.objectives {
            graph.add_objective(obj.clone())?;
        }

        let result = BciObjectiveCompiler::compile(&graph, scenario, (!aliases.is_empty()).then_some(aliases));
        self.last_objective_diagnostics = result
            .diagnostics
            .iter()
            .map(|d| format!("[{:?}] {}: {}", d.severity, d.code, d.message))
            .collect();
        if !result.success {
            let err = format!(
                "Objective compilation failed: {}",
                self.last_objective_diagnostics.first().cloned().unwrap_or_else(|| result.summary.clone())
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

    pub fn compile_wave_attack(&mut self, scenario: Option<&ScenarioDocument>, live_aliases: &[String]) -> Result<(), String> {
        let archetype = AiArchetypeCatalog::get(&self.selected_archetype_id).map_err(|e| {
            self.status_message = e.clone();
            self.status_is_error = true;
            e
        })?;

        let mut waves = Vec::new();
        for i in 1..=self.wave_count {
            let delay = self.first_delay_seconds + (i - 1) * self.interval_seconds;
            let squads: Vec<WaveSquadDefinition> = archetype
                .unit_preferences
                .iter()
                .map(|p| WaveSquadDefinition::new(p.alias.clone(), self.squad_count, self.wave_team))
                .collect();

            let mut wave = WaveAttackDefinition::new(
                i,
                delay,
                format!("Wave {i}"),
                self.wave_spawn_x,
                self.wave_spawn_z,
                squads,
            );
            if self.move_after_spawn {
                wave.move_destination = Some(WorldPosition {
                    x: self.dest_x.round() as i32,
                    z: self.dest_z.round() as i32,
                });
            }
            waves.push(wave);
        }

        let mut plan = CampaignMissionPlan::new("AI Wave Campaign", "");
        plan.waves = waves;
        let aliases: Vec<String> = if live_aliases.is_empty() {
            self.wave_aliases_text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect()
        } else {
            live_aliases.to_vec()
        };

        let result = CampaignMissionCompiler::compile(&plan, None, &aliases, scenario);
        if result.success {
            self.status_message = format!(
                "Campaign AI Wave compilation succeeded: {} waves, {} events.",
                self.wave_count,
                result.compiled_events.len()
            );
            self.status_is_error = false;
            self.last_wave_result = Some(result);
            self.dirty = true;
            Ok(())
        } else {
            let err = format!("Wave compilation failed: {}", result.diagnostics.join("; "));
            self.status_message = err.clone();
            self.status_is_error = true;
            Err(err)
        }
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
            sequence
                .subtitles
                .push(SubtitleKeyframe::new(0.0, 4.0, "", self.cutscene_subtitle.clone()));
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

    pub fn show(&mut self, ctx: &egui::Context, scenario: Option<&ScenarioDocument>, aliases: &[String]) {
        if !self.open {
            return;
        }

        let mut is_open = self.open;
        egui::Window::new("Campaign, Objectives & Cinematics Studio")
            .open(&mut is_open)
            .min_size(Vec2::new(760.0, 560.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.current_tab, CampaignTab::ObjectivesStudio, "Objectives Studio");
                    ui.selectable_value(&mut self.current_tab, CampaignTab::AiWaves, "AI Waves & Missions");
                    ui.selectable_value(&mut self.current_tab, CampaignTab::CameraCinematics, "Camera & Cinematics");
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
                    CampaignTab::ObjectivesStudio => self.render_objectives_tab(ui, scenario, aliases),
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

    fn render_objectives_tab(&mut self, ui: &mut egui::Ui, scenario: Option<&ScenarioDocument>, aliases: &[String]) {
        ui.heading("Campaign Objectives Studio");
        ui.label("Designs primary, bonus, and failure objectives with dependency graphs and BCI compilation.");

        ui.add_space(6.0);
        egui::Grid::new("obj_input_grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
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
                    ui.selectable_value(&mut self.obj_kind, ObjectiveKind::EliminateAllEnemies, "Eliminate All Enemies");
                    ui.selectable_value(&mut self.obj_kind, ObjectiveKind::Survival, "Survival (Hold Timer)");
                    ui.selectable_value(&mut self.obj_kind, ObjectiveKind::CaptureArea, "Capture Area");
                    ui.selectable_value(&mut self.obj_kind, ObjectiveKind::AssassinateTarget, "Assassinate Target");
                    ui.selectable_value(&mut self.obj_kind, ObjectiveKind::DestroyBuilding, "Destroy Building");
                    ui.selectable_value(&mut self.obj_kind, ObjectiveKind::EscortUnit, "Escort Unit");
                });
            ui.end_row();

            ui.label("Category:");
            egui::ComboBox::from_id_source("obj_cat_select")
                .selected_text(format!("{:?}", self.obj_category))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.obj_category, ObjectiveCategory::Primary, "Primary Objective");
                    ui.selectable_value(&mut self.obj_category, ObjectiveCategory::Bonus, "Bonus / Secondary");
                    ui.selectable_value(&mut self.obj_category, ObjectiveCategory::FailureCriterion, "Failure Criterion");
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
        ui.label(format!("Configured Objectives: {} (Compiled Events: {})", self.objectives.len(), self.last_compiled_objective_events.len()));
        for d in &self.last_objective_diagnostics {
            ui.colored_label(Color32::from_rgb(255, 180, 50), d);
        }
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            for (idx, obj) in self.objectives.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("{}. [{:?}] {} - {}", idx + 1, obj.category, obj.title, obj.description));
                });
            }
        });
    }

    fn render_waves_tab(&mut self, ui: &mut egui::Ui, scenario: Option<&ScenarioDocument>, aliases: &[String]) {
        ui.heading("Campaign Wave Attack & AI Planner");
        ui.label("Configures recurring attack waves, tactical posture profiles, and waypoint routes.");

        ui.add_space(6.0);
        egui::Grid::new("wave_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("AI Archetype:");
            egui::ComboBox::from_id_source("archetype_select")
                .selected_text(&self.selected_archetype_id)
                .show_ui(ui, |ui| {
                    for arch in &self.archetypes {
                        ui.selectable_value(
                            &mut self.selected_archetype_id,
                            arch.id.clone(),
                            format!("{} ({})", arch.display_name, arch.id),
                        );
                    }
                });
            ui.end_row();

            ui.label("Known unit aliases:");
            ui.text_edit_multiline(&mut self.wave_aliases_text);
            ui.end_row();

            ui.label("Wave Count:");
            ui.add(egui::DragValue::new(&mut self.wave_count).clamp_range(1..=32));
            ui.end_row();

            ui.label("First Delay / Interval (s):");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.first_delay_seconds).prefix("First: "));
                ui.add(egui::DragValue::new(&mut self.interval_seconds).prefix("Interval: "));
            });
            ui.end_row();

            ui.label("Squad Unit Count:");
            ui.add(egui::DragValue::new(&mut self.squad_count).clamp_range(1..=20));
            ui.end_row();

            ui.label("Team (0..7):");
            ui.add(egui::DragValue::new(&mut self.wave_team).clamp_range(0..=7));
            ui.end_row();

            ui.label("Spawn Location (X, Z):");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.wave_spawn_x).prefix("X: "));
                ui.add(egui::DragValue::new(&mut self.wave_spawn_z).prefix("Z: "));
            });
            ui.end_row();

            ui.label("Move after Spawn:");
            ui.checkbox(&mut self.move_after_spawn, "");
            ui.end_row();

            if self.move_after_spawn {
                ui.label("Destination (X, Z):");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut self.dest_x).prefix("X: "));
                    ui.add(egui::DragValue::new(&mut self.dest_z).prefix("Z: "));
                });
                ui.end_row();
            }
        });

        ui.add_space(8.0);
        if ui.button("Compile AI Waves").clicked() {
            let _ = self.compile_wave_attack(scenario, aliases);
        }

        if let Some(res) = &self.last_wave_result {
            ui.separator();
            ui.label(format!("Compilation Success: {}", res.success));
            ui.label(format!("Compiled events: {}, diagnostics: {}", res.compiled_events.len(), res.diagnostics.len()));
        }
    }

    fn render_camera_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Map Start Camera & Cinematic Tracks");
        ui.label("Specifies the map intro viewport camera position and multi-waypoint cinematic flight paths.");

        ui.add_space(6.0);
        egui::Grid::new("camera_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("Start Camera Enabled:");
            ui.checkbox(&mut self.start_camera.enabled, "");
            ui.end_row();

            ui.label("Start Position (X, Y, Z):");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.start_camera.x).prefix("X: "));
                ui.add(egui::DragValue::new(&mut self.start_camera.y).prefix("Y: "));
                ui.add(egui::DragValue::new(&mut self.start_camera.z).prefix("Z: "));
            });
            ui.end_row();

            ui.label("Start Zoom:");
            ui.add(egui::Slider::new(&mut self.start_camera.zoom, 0.5..=10.0));
            ui.end_row();

            ui.label("Primitive:");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.start_camera.primitive, StartCameraPrimitive::CenterToPos, "CenterToPos");
                ui.selectable_value(&mut self.start_camera.primitive, StartCameraPrimitive::DirectEnginePos, "DirectEnginePos");
            });
            ui.end_row();

            ui.label("Intro subtitle:");
            ui.text_edit_singleline(&mut self.cutscene_subtitle);
            ui.end_row();
        });

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
            ui.label(format!("Compiled Cinematic Events: {}", self.last_cinematic_events.len()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(compiled.is_ok(), "{compiled:?} {:?}", ui.last_objective_diagnostics);
        assert!(!ui.last_compiled_objective_events.is_empty());
        assert!(ui.is_dirty());
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
    fn test_campaign_ui_cinematic_flight_planning() {
        let mut ui = CampaignUi::default();
        assert!(ui.compile_cinematics_flight().is_ok());
        assert!(!ui.last_cinematic_events.is_empty());
        assert!(ui.is_dirty());
    }
}
