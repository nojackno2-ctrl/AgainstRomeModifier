//! AI Map Planning Studio UI: Driving Editor Brush Operations.
//! Replicates C# `AiMapPlanningDialog.cs` and integrates `native/against-rome-ai-planner`.

use against_rome_ai_planner::{
    AiEditorOperator, AiMapApplyResult, AiMapDesignRole, AiMapEditScope, AiMapFeature,
    AiMapPlan, AiMaterialOption, HttpTransport, MockHttpTransport,
    MultiAiMapPlanResult, MultiAiMapPlanner, MultiAiMapRoleRequest, OllamaMapPlanner, Rectangle,
};
use crate::terrain::TerrainSession;
use eframe::egui::{self, Color32, Vec2};
use std::sync::Arc;

pub struct AiPlannerUi {
    pub open: bool,
    pub ollama_url: String,
    pub selected_model: String,
    pub available_models: Vec<String>,
    pub prompt_description: String,

    // Scope bounds
    pub scope_x: i32,
    pub scope_y: i32,
    pub scope_width: i32,
    pub scope_height: i32,
    pub use_custom_scope: bool,

    // Multi-role selection
    pub role_terrain: bool,
    pub role_water: bool,
    pub role_materials: bool,

    // Generated plan & preview
    pub current_plan: Option<AiMapPlan>,
    pub plan_summary: String,
    pub last_apply_result: Option<AiMapApplyResult>,

    // Transport (allows custom or mock transport for offline testing)
    custom_transport: Option<Arc<dyn HttpTransport>>,

    // Dirty / guards
    pub dirty: bool,
    pub pending_discard: bool,
    pub status_message: String,
    pub status_is_error: bool,
}

impl Default for AiPlannerUi {
    fn default() -> Self {
        Self {
            open: false,
            ollama_url: "http://localhost:11434".to_string(),
            selected_model: "laguna-xs-2.1:latest".to_string(),
            available_models: vec!["laguna-xs-2.1:latest".to_string(), "llama3:latest".to_string()],
            prompt_description: "A highland valley with a meandering river and fortified hilltops.".to_string(),
            scope_x: 0,
            scope_y: 0,
            scope_width: 128,
            scope_height: 128,
            use_custom_scope: false,
            role_terrain: true,
            role_water: true,
            role_materials: true,
            current_plan: None,
            plan_summary: String::new(),
            last_apply_result: None,
            custom_transport: None,
            dirty: false,
            pending_discard: false,
            status_message: "Ready.".to_string(),
            status_is_error: false,
        }
    }
}

impl AiPlannerUi {
    pub fn is_dirty(&self) -> bool {
        self.dirty || self.current_plan.is_some()
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
        self.current_plan = None;
        self.plan_summary.clear();
        self.last_apply_result = None;
        self.pending_discard = false;
    }

    pub fn request_close(&mut self) {
        if self.is_dirty() {
            self.pending_discard = true;
        } else {
            self.open = false;
        }
    }

    fn summarize(plan: &AiMapPlan) -> String {
        let mut text = plan.summary.clone().unwrap_or_default();
        if let Some(h) = plan.base_height {
            text.push_str(&format!("
Base height: {h}"));
        }
        text.push_str(&format!("
Features: {}", plan.features.len()));
        for f in &plan.features {
            text.push_str(&format!(
                "
- {} @ ({},{}) r={}",
                f.feature_type.as_deref().unwrap_or("?"),
                f.x,
                f.y,
                f.radius
            ));
        }
        text
    }

    pub fn set_custom_transport(&mut self, transport: Arc<dyn HttpTransport>) {
        self.custom_transport = Some(transport);
    }

    pub fn refresh_models(&mut self) {
        let planner = OllamaMapPlanner::new(Some(&self.ollama_url), self.custom_transport.clone());
        match planner.list_models() {
            Ok(models) => {
                if !models.is_empty() {
                    self.selected_model = models[0].clone();
                }
                self.available_models = models;
                self.status_message = format!("Discovered {} Ollama models.", self.available_models.len());
                self.status_is_error = false;
            }
            Err(e) => {
                self.status_message = format!("Failed to list Ollama models: {e}");
                self.status_is_error = true;
            }
        }
    }

    pub fn generate_ai_plan(&mut self, water_level: f32) -> Result<(), String> {
        let planner = OllamaMapPlanner::new(Some(&self.ollama_url), self.custom_transport.clone());
        let materials = vec![
            AiMaterialOption::new("GRAS1", "Grass"),
            AiMaterialOption::new("ERDE", "Dirt"),
            AiMaterialOption::new("FELS", "Rock"),
            AiMaterialOption::new("FLUSS1", "River"),
        ];

        match planner.generate_plan(&self.selected_model, &self.prompt_description, &materials, water_level) {
            Ok((mut plan, _raw_json)) => {
                if self.use_custom_scope {
                    plan.edit_scope = Some(AiMapEditScope::new(
                        Rectangle::new(self.scope_x, self.scope_y, self.scope_width, self.scope_height),
                        Vec::new(),
                        false,
                    )?);
                }

                self.plan_summary = Self::summarize(&plan);
                self.current_plan = Some(plan);
                self.dirty = true;
                self.status_message = "AI plan generated successfully!".to_string();
                self.status_is_error = false;
                Ok(())
            }
            Err(e) => {
                self.status_message = format!("AI Plan Generation Failed: {e}");
                self.status_is_error = true;
                Err(e)
            }
        }
    }

    pub fn apply_plan_to_session(&mut self, session: &mut TerrainSession, water_level: f32) -> Result<AiMapApplyResult, String> {
        let Some(plan) = &self.current_plan else {
            return Err("No AI plan generated to apply.".into());
        };

        let dimension = session.vertex_size;
        let result = AiEditorOperator::apply(
            plan,
            Some(session.core_mut()),
            None,
            dimension,
            water_level,
            true,
            None,
            None,
            None,
        );

        self.status_message = format!(
            "Applied plan: {} height samples modified, {} collision pixels updated, {} operations.",
            result.height_samples_changed,
            result.collision_pixels_changed,
            result.operation_count()
        );
        self.status_is_error = false;
        self.last_apply_result = Some(result.clone());
        self.dirty = true;
        Ok(result)
    }

    pub fn show(&mut self, ctx: &egui::Context, session: Option<&mut TerrainSession>, water_level: f32) {
        if !self.open {
            return;
        }

        let mut is_open = self.open;
        egui::Window::new("AI Map Planning Studio (Ollama)")
            .open(&mut is_open)
            .min_size(Vec2::new(760.0, 560.0))
            .show(ctx, |ui| {
                ui.heading("Multi-AI Map Planning & Brush Operator");
                ui.label("Directs AI LLMs to design terrain features, executing changes exclusively through verified /brush editor operations.");

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

                egui::Grid::new("ai_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label("Ollama Endpoint URL:");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.ollama_url);
                        if ui.button("Connect & List Models").clicked() {
                            self.refresh_models();
                        }
                    });
                    ui.end_row();

                    ui.label("Active Model:");
                    egui::ComboBox::from_id_source("ai_model_select")
                        .selected_text(&self.selected_model)
                        .show_ui(ui, |ui| {
                            for m in &self.available_models {
                                ui.selectable_value(&mut self.selected_model, m.clone(), m);
                            }
                        });
                    ui.end_row();

                    ui.label("Multi-Role Collaboration:");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.role_terrain, "Macro Terrain");
                        ui.checkbox(&mut self.role_water, "Hydrology & Rivers");
                        ui.checkbox(&mut self.role_materials, "Floor Materials");
                    });
                    ui.end_row();

                    ui.label("Edit Scope Bounds:");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.use_custom_scope, "Restrict to Region");
                        if self.use_custom_scope {
                            ui.add(egui::DragValue::new(&mut self.scope_x).prefix("X: "));
                            ui.add(egui::DragValue::new(&mut self.scope_y).prefix("Y: "));
                            ui.add(egui::DragValue::new(&mut self.scope_width).prefix("W: "));
                            ui.add(egui::DragValue::new(&mut self.scope_height).prefix("H: "));
                        }
                    });
                    ui.end_row();
                });

                ui.add_space(6.0);
                ui.label("Design Description / Prompt:");
                ui.text_edit_multiline(&mut self.prompt_description);

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Generate AI Map Plan").clicked() {
                        let _ = self.generate_ai_plan(water_level);
                    }

                    let can_apply = self.current_plan.is_some() && session.is_some();
                    if ui.add_enabled(can_apply, egui::Button::new("Apply Plan to Map Terrain")).clicked() {
                        if let Some(s) = session {
                            let _ = self.apply_plan_to_session(s, water_level);
                        }
                    }
                });

                ui.separator();
                if let Some(plan) = &self.current_plan {
                    ui.heading("Plan Preview:");
                    ui.label(format!("Features: {} (Base Height: {:?})", plan.features.len(), plan.base_height));
                    if !self.plan_summary.is_empty() {
                        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                            ui.monospace(&self.plan_summary);
                        });
                    }
                } else {
                    ui.label("No AI plan generated. Click 'Generate AI Map Plan' to start.");
                }

                if let Some(apply_res) = &self.last_apply_result {
                    ui.separator();
                    ui.heading("Execution Report:");
                    ui.label(format!("Modified Height Samples: {}", apply_res.height_samples_changed));
                    ui.label(format!("Modified Collision Pixels: {}", apply_res.collision_pixels_changed));
                    egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                        for log in &apply_res.operation_log {
                            ui.label(log);
                        }
                    });
                }
            });

        if !is_open {
            self.request_close();
        }

        if self.pending_discard {
            let mut close_modal = false;
            let mut confirm = false;

            egui::Window::new("Discard Unsaved AI Plan?")
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Closing will discard currently generated AI plan features.");
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use against_rome_ai_planner::HttpResponse;

    #[test]
    fn test_ai_planner_ui_initial_state_and_dirty_guards() {
        let mut ui = AiPlannerUi::default();
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
    fn test_ai_planner_ui_generation_with_mock_transport() {
        let mut ui = AiPlannerUi::default();
        let mock_transport = Arc::new(MockHttpTransport::new(|_method, url, _body| {
            if url.ends_with("/api/tags") {
                Ok(HttpResponse::new(200, r#"{"models":[{"name":"mock-model:latest"}]}"#))
            } else {
                // Ollama /api/chat envelope carrying a valid plan
                let plan = serde_json::json!({
                    "baseHeight": 45,
                    "features": [
                        {"type": "mountain", "x": 20, "y": 20, "radius": 8, "amount": 30}
                    ]
                });
                let response_body = serde_json::json!({
                    "message": {"role": "assistant", "content": plan.to_string()}
                });
                Ok(HttpResponse::new(200, response_body.to_string()))
            }
        }));

        ui.set_custom_transport(mock_transport);
        ui.refresh_models();
        assert_eq!(ui.available_models, vec!["mock-model:latest".to_string()]);

        assert!(ui.generate_ai_plan(0.0).is_ok());
        assert!(ui.current_plan.is_some());
        assert!(ui.is_dirty());
    }

    #[test]
    fn test_ai_planner_ui_apply_to_terrain_session() {
        let mut ui = AiPlannerUi::default();
        let mut plan = AiMapPlan::default();
        plan.base_height = Some(40);
        ui.current_plan = Some(plan);

        let mut session = TerrainSession::create_blank(33, 32);
        let res = ui.apply_plan_to_session(&mut session, 0.0);
        assert!(res.is_ok());
        assert!(ui.last_apply_result.is_some());
        assert!(ui.is_dirty());
    }
}
