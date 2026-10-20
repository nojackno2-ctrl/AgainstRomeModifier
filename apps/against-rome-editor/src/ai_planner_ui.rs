//! AI Map Planning Studio UI: Driving Editor Brush Operations.
//! Replicates C# `AiMapPlanningDialog.cs` and integrates `native/against-rome-ai-planner`.
//!
//! Generation runs on a background worker thread (never blocks the UI). The worker reports
//! per-role progress over an `mpsc` channel and observes a [`CancelToken`]; the UI polls the
//! channel each frame. Plans produced after an input change or a Cancel are never applicable.

use crate::terrain::TerrainSession;
use against_rome_ai_planner::multi_planner::GenerateRolePlanFn;
use against_rome_ai_planner::{
    AiEditorOperator, AiMapApplyResult, AiMapDesignRole, AiMapEditScope, AiMapPlan,
    AiMapRoleProgress, AiMapRoleStatus, AiMaterialOption, CancelToken, HttpTransport,
    MultiAiMapPlanResult, MultiAiMapPlanner, MultiAiMapRoleRequest, OllamaMapPlanner,
};
use eframe::egui::{self, Color32, Vec2};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Role order matches C# `AiMapDesignRole` / the three model boxes of the dialog.
pub const ROLES: [AiMapDesignRole; 3] = [
    AiMapDesignRole::Terrain,
    AiMapDesignRole::Water,
    AiMapDesignRole::Materials,
];

const DEFAULT_MODEL: &str = "laguna-xs-2.1:latest";

enum WorkerMsg {
    Progress(AiMapRoleProgress),
    Done(Result<MultiAiMapPlanResult, String>),
}

struct GenerationJob {
    cancel: CancelToken,
    rx: Receiver<WorkerMsg>,
    /// Input version captured at start; a mismatch on completion invalidates the result.
    input_version: u64,
    scope: AiMapEditScope,
}

pub struct AiPlannerUi {
    pub open: bool,
    pub ollama_url: String,
    /// One model per role (Terrain, Water, Materials), like the three C# model boxes.
    pub role_models: [String; 3],
    pub available_models: Vec<String>,
    pub prompt_description: String,

    /// Editable area as `X,Y,寬,高` (C# `BoundsBox`).
    pub bounds_text: String,
    /// AI locked areas, one `X,Y,寬,高` per line (C# `LocksBox`).
    pub locks_text: String,
    /// C# `PassabilityCheck` ("允許修改通行"); only effective when the Terrain role is enabled.
    pub edit_passability: bool,

    /// Role checkboxes (Terrain, Water, Materials).
    pub role_checks: [bool; 3],

    // Generated plan & preview
    pub current_plan: Option<AiMapPlan>,
    pub plan_summary: String,
    pub last_apply_result: Option<AiMapApplyResult>,

    // Background generation state
    job: Option<GenerationJob>,
    pub progress_done: usize,
    pub progress_total: usize,
    input_version: u64,
    inputs_key: String,

    // Transport / planner injection (allows custom or fake planners for offline testing)
    custom_transport: Option<Arc<dyn HttpTransport>>,
    custom_generator: Option<GenerateRolePlanFn>,

    // Dirty / guards
    pub dirty: bool,
    pub pending_discard: bool,
    pub status_message: String,
    pub status_is_error: bool,
}

impl Default for AiPlannerUi {
    fn default() -> Self {
        let mut ui = Self {
            open: false,
            ollama_url: "http://localhost:11434".to_string(),
            role_models: std::array::from_fn(|_| DEFAULT_MODEL.to_string()),
            available_models: vec![DEFAULT_MODEL.to_string(), "llama3:latest".to_string()],
            prompt_description: "A highland valley with a meandering river and fortified hilltops.".to_string(),
            bounds_text: "0,0,64,64".to_string(),
            locks_text: String::new(),
            edit_passability: false,
            role_checks: [true; 3],
            current_plan: None,
            plan_summary: String::new(),
            last_apply_result: None,
            job: None,
            progress_done: 0,
            progress_total: 0,
            input_version: 0,
            inputs_key: String::new(),
            custom_transport: None,
            custom_generator: None,
            dirty: false,
            pending_discard: false,
            status_message: "Ready.".to_string(),
            status_is_error: false,
        };
        ui.inputs_key = ui.compute_inputs_key();
        ui
    }
}

impl AiPlannerUi {
    pub fn is_dirty(&self) -> bool {
        self.dirty || self.current_plan.is_some() || self.job.is_some()
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn clear_dirty(&mut self) {
        self.reset_session();
    }

    pub fn is_generating(&self) -> bool {
        self.job.is_some()
    }

    /// Resets all map-bound generated AI plans, summaries, and apply results.
    /// Preserves user preferences (open state, ollama url, models, prompt description, scope, roles).
    pub fn reset_session(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancel.cancel();
        }
        self.dirty = false;
        self.current_plan = None;
        self.plan_summary.clear();
        self.last_apply_result = None;
        self.pending_discard = false;
        self.progress_done = 0;
        self.progress_total = 0;
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

    fn role_label(role: AiMapDesignRole) -> &'static str {
        match role {
            AiMapDesignRole::Terrain => "Terrain",
            AiMapDesignRole::Water => "Water",
            AiMapDesignRole::Materials => "Materials",
        }
    }

    fn status_label(status: AiMapRoleStatus) -> &'static str {
        match status {
            AiMapRoleStatus::Succeeded => "Succeeded",
            AiMapRoleStatus::Failed => "Failed",
            AiMapRoleStatus::Cancelled => "Cancelled",
        }
    }

    /// Port of C# `FormatReview`: per-role diagnostics, then summary, editable-area base
    /// height / base material and the feature list.
    pub fn format_review(result: &MultiAiMapPlanResult) -> String {
        let mut text = String::new();
        for role in &result.roles {
            text.push_str(&format!(
                "{} / {}: {} ({})\n",
                Self::role_label(role.role),
                role.model,
                Self::status_label(role.status),
                role.accepted_features
            ));
            if let Some(e) = &role.error {
                text.push_str(e);
                text.push('\n');
            }
            for w in &role.warnings {
                text.push_str(w);
                text.push('\n');
            }
        }
        let Some(plan) = &result.plan else {
            return text;
        };
        text.push('\n');
        text.push_str(plan.summary.as_deref().unwrap_or(""));
        text.push('\n');
        if let Some(h) = plan.base_height {
            text.push_str(&format!("Editable-area base height: {h}\n"));
        }
        if let Some(m) = &plan.base_material {
            text.push_str(&format!("Editable-area base material: {m}\n"));
        }
        for f in &plan.features {
            let from = f
                .location
                .clone()
                .unwrap_or_else(|| format!("({},{})", f.x, f.y));
            let to = f.to_location.clone().unwrap_or_else(|| match (f.x2, f.y2) {
                (Some(x2), y2) => format!("({},{})", x2, y2.map(|v| v.to_string()).unwrap_or_default()),
                _ => "—".to_string(),
            });
            text.push_str(&format!(
                "{}: {} → {}; radius={}; amount={}; material={}\n",
                f.feature_type.as_deref().unwrap_or("?"),
                from,
                to,
                f.radius,
                f.amount.map(|v| v.to_string()).unwrap_or_default(),
                f.material.as_deref().unwrap_or("")
            ));
        }
        text
    }

    pub fn set_custom_transport(&mut self, transport: Arc<dyn HttpTransport>) {
        self.custom_transport = Some(transport);
    }

    /// Inject a per-role planner (model, prompt, materials, water) -> (plan, raw). Used by tests
    /// and by hosts that do not want to talk to Ollama.
    pub fn set_custom_generator(&mut self, generator: GenerateRolePlanFn) {
        self.custom_generator = Some(generator);
    }

    pub fn refresh_models(&mut self) {
        let planner = OllamaMapPlanner::new(Some(&self.ollama_url), self.custom_transport.clone());
        match planner.list_models() {
            Ok(models) => {
                for m in &mut self.role_models {
                    if m.trim().is_empty() {
                        if let Some(first) = models.first() {
                            *m = first.clone();
                        }
                    }
                }
                self.status_message = if models.is_empty() {
                    "No Ollama models found. Install one or enter a known model name.".to_string()
                } else {
                    format!("Discovered {} Ollama models.", models.len())
                };
                self.available_models = models;
                self.status_is_error = false;
            }
            Err(e) => {
                self.status_message = format!("Failed to list Ollama models: {e}");
                self.status_is_error = true;
            }
        }
    }

    fn compute_inputs_key(&self) -> String {
        format!(
            "{:?}|{:?}|{}|{}|{}|{}",
            self.role_models, self.role_checks, self.bounds_text, self.locks_text, self.edit_passability,
            self.prompt_description
        )
    }

    /// Detects edits to roles/models/area/locks/passability/description. Like C# `InvalidatePlan`,
    /// an input change makes any existing (or in-flight) plan stale.
    pub fn sync_inputs(&mut self) {
        let key = self.compute_inputs_key();
        if key == self.inputs_key {
            return;
        }
        self.inputs_key = key;
        self.input_version += 1;
        if self.current_plan.is_some() {
            self.current_plan = None;
            self.plan_summary.clear();
            self.status_message =
                "Roles, area, locks or description changed; the old plan is invalid. Generate again.".to_string();
            self.status_is_error = false;
        }
    }

    fn make_generator(&self) -> GenerateRolePlanFn {
        if let Some(g) = &self.custom_generator {
            return g.clone();
        }
        let planner = Arc::new(OllamaMapPlanner::new(
            Some(&self.ollama_url),
            self.custom_transport.clone(),
        ));
        Arc::new(move |model, prompt, materials, water| planner.generate_plan(model, prompt, materials, water))
    }

    /// Parses the area/lock inputs exactly like C# (passability only when Terrain role is on).
    pub fn parse_scope(&self) -> Result<AiMapEditScope, String> {
        AiMapEditScope::parse(
            &self.bounds_text,
            &self.locks_text,
            self.edit_passability && self.role_checks[0],
        )
    }

    /// Starts generation on a background thread and returns immediately.
    pub fn start_generation(&mut self, water_level: f32) -> Result<(), String> {
        if self.job.is_some() {
            return Err("Generation already running.".into());
        }
        self.sync_inputs();
        self.current_plan = None;
        self.plan_summary.clear();
        let fail = |this: &mut Self, msg: String| {
            this.status_message = format!("Generation failed to start: {msg}");
            this.status_is_error = true;
            Err(msg)
        };

        let requests: Vec<MultiAiMapRoleRequest> = ROLES
            .iter()
            .enumerate()
            .filter(|(i, _)| self.role_checks[*i])
            .map(|(i, role)| MultiAiMapRoleRequest::new(*role, self.role_models[i].trim()))
            .collect();
        if requests.is_empty() {
            return fail(self, "Select at least one specialist.".to_string());
        }
        let scope = match self.parse_scope() {
            Ok(s) => s,
            Err(e) => return fail(self, e),
        };

        let b = *scope.bounds();
        let mut description = format!(
            "{}\nUse global tile coordinates inside X={}..{}, Y={}..{}. Protected rectangles (X,Y,width,height): {}",
            self.prompt_description,
            b.x,
            b.right() - 1,
            b.y,
            b.bottom() - 1,
            scope
                .locked()
                .iter()
                .map(|r| format!("{},{},{},{}", r.x, r.y, r.width, r.height))
                .collect::<Vec<_>>()
                .join("; ")
        );
        if !scope.edit_passability() {
            description.push_str("\nDo not create blocked or passable features; passability is protected.");
        }

        let materials = vec![
            AiMaterialOption::new("GRAS1", "Grass"),
            AiMaterialOption::new("ERDE", "Dirt"),
            AiMaterialOption::new("FELS", "Rock"),
            AiMaterialOption::new("FLUSS1", "River"),
        ];

        let generator = self.make_generator();
        let cancel = CancelToken::new();
        let (tx, rx) = mpsc::channel::<WorkerMsg>();
        let worker_cancel = cancel.clone();
        let worker_requests = requests.clone();
        std::thread::spawn(move || {
            let planner = MultiAiMapPlanner::new(generator);
            let progress_tx = tx.clone();
            let mut cb = move |p: AiMapRoleProgress| {
                let _ = progress_tx.send(WorkerMsg::Progress(p));
            };
            let result = planner.generate_plan_cancellable(
                &worker_requests,
                &description,
                &materials,
                water_level,
                &worker_cancel,
                Some(&mut cb),
            );
            let _ = tx.send(WorkerMsg::Done(result));
        });

        self.progress_done = 0;
        self.progress_total = requests.len();
        self.job = Some(GenerationJob {
            cancel,
            rx,
            input_version: self.input_version,
            scope,
        });
        self.status_message = "Terrain, water and materials plan in order, one local inference at a time; this may take several minutes.".to_string();
        self.status_is_error = false;
        Ok(())
    }

    /// Cancels the running generation. The UI is released immediately; a provider that finishes
    /// afterwards is ignored.
    pub fn cancel_generation(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancel.cancel();
            self.status_message = "Generation cancelled; you can retry.".to_string();
            self.status_is_error = false;
        }
    }

    /// Drains worker messages (call every frame). Returns true while generation is running.
    pub fn poll_generation(&mut self) -> bool {
        let mut finished: Option<Result<MultiAiMapPlanResult, String>> = None;
        let mut disconnected = false;
        if let Some(job) = &self.job {
            loop {
                match job.rx.try_recv() {
                    Ok(WorkerMsg::Progress(p)) => {
                        self.progress_done = p.completed_roles.min(self.progress_total);
                        let role = Self::role_label(p.role);
                        self.status_message = match (p.finished, p.status) {
                            (true, Some(s)) => format!(
                                "Completed {}/{}: {} / {} ({})",
                                p.completed_roles,
                                self.progress_total,
                                role,
                                p.model,
                                Self::status_label(s)
                            ),
                            _ => format!(
                                "Planning {} / {}; completed {}/{}.",
                                role, p.model, p.completed_roles, self.progress_total
                            ),
                        };
                        self.status_is_error = false;
                    }
                    Ok(WorkerMsg::Done(r)) => {
                        finished = Some(r);
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }
        if disconnected {
            self.job = None;
            self.status_message = "Generation worker stopped unexpectedly; retry.".to_string();
            self.status_is_error = true;
            return false;
        }
        if let Some(result) = finished {
            let job = self.job.take().expect("job present when finished");
            self.finish_generation(job, result);
            return false;
        }
        self.job.is_some()
    }

    /// Blocks until the worker finishes (headless callers / tests). Returns false on timeout.
    pub fn wait_generation(&mut self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while self.poll_generation() {
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        true
    }

    fn finish_generation(&mut self, job: GenerationJob, result: Result<MultiAiMapPlanResult, String>) {
        if job.cancel.is_cancelled() {
            self.status_message = "Generation cancelled; you can retry.".to_string();
            return;
        }
        if job.input_version != self.input_version {
            self.plan_summary.clear();
            self.status_message =
                "Roles, area, locks or description changed; the old plan is invalid. Generate again.".to_string();
            self.status_is_error = false;
            return;
        }
        match result {
            Err(e) => {
                self.status_message = format!("AI Plan Generation Failed: {e}");
                self.status_is_error = true;
            }
            Ok(res) if res.is_cancelled => {
                self.status_message = "Generation cancelled; you can retry.".to_string();
                self.status_is_error = false;
            }
            Ok(mut res) => {
                let scope = job.scope;
                let mut review = Self::format_review(&res);
                let b = *scope.bounds();
                review.push_str(&format!(
                    "\nEdit area: {},{},{},{}; {} locks. Shared boundary vertices are preserved; locks protect this AI operation only.\n",
                    b.x,
                    b.y,
                    b.width,
                    b.height,
                    scope.locked().len()
                ));
                if !scope.edit_passability() {
                    review.push_str("Passability is protected; blocked/passable features will not be applied.\n");
                }
                self.progress_done = res.roles.len().min(self.progress_total);
                self.status_message = if res.plan.is_none() {
                    self.status_is_error = true;
                    "All specialists failed. Review diagnostics and retry.".to_string()
                } else if res.has_failures() {
                    self.status_is_error = true;
                    "Some specialists failed: this is a partial plan. Review diagnostics before applying or retrying."
                        .to_string()
                } else {
                    self.status_is_error = false;
                    "Plan generated and awaiting review. Click Apply to change the map.".to_string()
                };
                if let Some(plan) = res.plan.as_mut() {
                    plan.edit_scope = Some(scope);
                    self.dirty = true;
                }
                self.plan_summary = review;
                self.current_plan = res.plan;
            }
        }
    }

    /// Blocking convenience wrapper (headless/tests): start, then wait for completion.
    pub fn generate_ai_plan(&mut self, water_level: f32) -> Result<(), String> {
        self.start_generation(water_level)?;
        if !self.wait_generation(Duration::from_secs(60)) {
            self.cancel_generation();
            return Err("Generation timed out.".into());
        }
        if self.current_plan.is_some() {
            Ok(())
        } else {
            Err(self.status_message.clone())
        }
    }

    pub fn apply_plan_to_session(&mut self, session: &mut TerrainSession, water_level: f32) -> Result<AiMapApplyResult, String> {
        if self.job.is_some() {
            return Err("Generation is still running.".into());
        }
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
        // C#: the plan is consumed by Apply (no double apply); the map is now dirty.
        self.current_plan = None;
        self.dirty = true;
        Ok(result)
    }

    pub fn show(&mut self, ctx: &egui::Context, session: Option<&mut TerrainSession>, water_level: f32) {
        if !self.open {
            return;
        }
        self.poll_generation();
        self.sync_inputs();
        let busy = self.job.is_some();
        if busy {
            ctx.request_repaint_after(Duration::from_millis(100));
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
                }
                if busy || self.progress_total > 0 {
                    ui.add(
                        egui::ProgressBar::new(if self.progress_total == 0 {
                            0.0
                        } else {
                            self.progress_done as f32 / self.progress_total as f32
                        })
                        .text(format!("{}/{}", self.progress_done, self.progress_total)),
                    );
                }
                ui.separator();

                ui.add_enabled_ui(!busy, |ui| {
                    egui::Grid::new("ai_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                        ui.label("Ollama Endpoint URL:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.ollama_url);
                            if ui.button("Connect & List Models").clicked() {
                                self.refresh_models();
                            }
                        });
                        ui.end_row();

                        for (i, role) in ROLES.iter().enumerate() {
                            ui.checkbox(&mut self.role_checks[i], Self::role_label(*role));
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut self.role_models[i]);
                                egui::ComboBox::from_id_source(("ai_model_select", i))
                                    .selected_text("▼")
                                    .show_ui(ui, |ui| {
                                        for m in &self.available_models {
                                            ui.selectable_value(&mut self.role_models[i], m.clone(), m);
                                        }
                                    });
                            });
                            ui.end_row();
                        }

                        ui.label("Area X,Y,width,height:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.bounds_text);
                            ui.checkbox(&mut self.edit_passability, "Edit passability");
                        });
                        ui.end_row();

                        ui.label("AI locks (one area per line):");
                        ui.add(
                            egui::TextEdit::multiline(&mut self.locks_text)
                                .desired_rows(3)
                                .hint_text("X,Y,width,height"),
                        );
                        ui.end_row();
                    });

                    ui.add_space(6.0);
                    ui.label("Design Description / Prompt:");
                    ui.text_edit_multiline(&mut self.prompt_description);
                });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(!busy, egui::Button::new("Generate / Retry")).clicked() {
                        let _ = self.start_generation(water_level);
                    }
                    if ui.add_enabled(busy, egui::Button::new("Cancel generation")).clicked() {
                        self.cancel_generation();
                    }

                    let can_apply = self.current_plan.is_some() && session.is_some() && !busy;
                    if ui.add_enabled(can_apply, egui::Button::new("Apply Plan to Map Terrain")).clicked() {
                        if let Some(s) = session {
                            let _ = self.apply_plan_to_session(s, water_level);
                        }
                    }
                });

                ui.separator();
                if let Some(plan) = &self.current_plan {
                    ui.heading("Plan Preview:");
                    ui.label(format!(
                        "Features: {} (Base Height: {:?}, Base Material: {:?})",
                        plan.features.len(),
                        plan.base_height,
                        plan.base_material
                    ));
                    if !self.plan_summary.is_empty() {
                        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                            ui.monospace(&self.plan_summary);
                        });
                    }
                } else if !busy {
                    ui.label("No AI plan generated. Click 'Generate / Retry' to start.");
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
                    ui.label("Closing will discard currently generated AI plan features (and cancel any running generation).");
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
    use against_rome_ai_planner::plan::AiMapFeature;
    use against_rome_ai_planner::{HttpResponse, MockHttpTransport};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Condvar, Mutex};

    fn fake_plan_for(prompt: &str) -> AiMapPlan {
        if prompt.contains("Terrain specialist") {
            AiMapPlan {
                base_height: Some(60),
                features: vec![AiMapFeature {
                    feature_type: Some("hill".into()),
                    location: Some("north".into()),
                    radius: 4,
                    ..Default::default()
                }],
                ..Default::default()
            }
        } else if prompt.contains("Water specialist") {
            AiMapPlan {
                features: vec![AiMapFeature {
                    feature_type: Some("lake".into()),
                    location: Some("south".into()),
                    radius: 4,
                    ..Default::default()
                }],
                ..Default::default()
            }
        } else {
            AiMapPlan {
                base_material: Some("GRAS1".into()),
                features: vec![AiMapFeature {
                    feature_type: Some("material".into()),
                    location: Some("east".into()),
                    radius: 4,
                    material: Some("ERDE".into()),
                    ..Default::default()
                }],
                ..Default::default()
            }
        }
    }

    fn instant_generator(prompts: Arc<Mutex<Vec<String>>>) -> GenerateRolePlanFn {
        Arc::new(move |_m, prompt, _mats, _w| {
            prompts.lock().unwrap().push(prompt.to_string());
            let plan = fake_plan_for(prompt);
            let raw = serde_json::to_string(&plan).unwrap();
            Ok((plan, raw))
        })
    }

    /// A generator that blocks until released, to prove the UI thread is not blocked.
    struct Gate {
        open: Mutex<bool>,
        cv: Condvar,
        entered: AtomicUsize,
    }

    fn gated_generator(gate: Arc<Gate>) -> GenerateRolePlanFn {
        Arc::new(move |_m, prompt, _mats, _w| {
            gate.entered.fetch_add(1, Ordering::SeqCst);
            let mut open = gate.open.lock().unwrap();
            while !*open {
                open = gate.cv.wait(open).unwrap();
            }
            drop(open);
            let plan = fake_plan_for(prompt);
            let raw = serde_json::to_string(&plan).unwrap();
            Ok((plan, raw))
        })
    }

    fn release(gate: &Gate) {
        *gate.open.lock().unwrap() = true;
        gate.cv.notify_all();
    }

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
        // The plan is consumed (C# `_plan = null`): no double apply.
        assert!(ui.current_plan.is_none());
        assert!(ui.apply_plan_to_session(&mut session, 0.0).is_err());
    }

    #[test]
    fn fake_planner_generation_attaches_scope_locks_and_passability() {
        let prompts = Arc::new(Mutex::new(Vec::new()));
        let mut ui = AiPlannerUi::default();
        ui.set_custom_generator(instant_generator(prompts.clone()));
        ui.bounds_text = "8,8,24,24".into();
        ui.locks_text = "10,10,4,4\n\n 20 , 20 , 2 , 2 \n".into();
        ui.edit_passability = true;

        assert!(ui.generate_ai_plan(0.0).is_ok());
        let plan = ui.current_plan.as_ref().unwrap();
        let scope = plan.edit_scope.as_ref().unwrap();
        assert_eq!(scope.bounds().x, 8);
        assert_eq!(scope.locked().len(), 2);
        assert!(scope.edit_passability());
        assert!(!scope.allows_tile(11, 11));
        assert!(scope.allows_tile(9, 9));
        assert!(!scope.allows_tile(40, 40));
        assert_eq!(plan.base_height, Some(60));
        assert_eq!(plan.base_material.as_deref(), Some("GRAS1"));
        assert!(ui.plan_summary.contains("Editable-area base height: 60"));
        assert!(ui.plan_summary.contains("Editable-area base material: GRAS1"));
        assert!(ui.plan_summary.contains("2 locks"));
        assert!(ui.dirty);

        let prompts = prompts.lock().unwrap();
        assert_eq!(prompts.len(), 3);
        assert!(prompts[0].contains("X=8..31, Y=8..31"));
        assert!(prompts[0].contains("10,10,4,4; 20,20,2,2"));
        assert!(!prompts[0].contains("passability is protected"));
    }

    #[test]
    fn passability_requires_terrain_role_and_default_protects_it() {
        let prompts = Arc::new(Mutex::new(Vec::new()));
        let mut ui = AiPlannerUi::default();
        ui.set_custom_generator(instant_generator(prompts.clone()));
        ui.edit_passability = true;
        ui.role_checks = [false, true, true];
        assert!(!ui.parse_scope().unwrap().edit_passability());
        assert!(ui.generate_ai_plan(0.0).is_ok());
        assert_eq!(prompts.lock().unwrap().len(), 2);
        assert!(prompts.lock().unwrap()[0].contains("passability is protected"));
        assert!(ui.plan_summary.contains("Passability is protected"));
    }

    #[test]
    fn invalid_inputs_fail_before_spawning_and_keep_ui_usable() {
        let mut ui = AiPlannerUi::default();
        ui.bounds_text = "0,0,65,64".into();
        assert!(ui.start_generation(0.0).is_err());
        assert!(!ui.is_generating());
        assert!(ui.status_is_error);

        ui.bounds_text = "0,0,64,64".into();
        ui.locks_text = "1,2,3".into();
        assert!(ui.start_generation(0.0).is_err());

        ui.locks_text.clear();
        ui.role_checks = [false; 3];
        assert!(ui.start_generation(0.0).unwrap_err().contains("at least one"));
    }

    #[test]
    fn generation_runs_in_background_with_progress_and_does_not_block() {
        let gate = Arc::new(Gate { open: Mutex::new(false), cv: Condvar::new(), entered: AtomicUsize::new(0) });
        let mut ui = AiPlannerUi::default();
        ui.set_custom_generator(gated_generator(gate.clone()));

        // Returns immediately although the planner is blocked.
        ui.start_generation(0.0).unwrap();
        assert!(ui.is_generating());
        assert!(ui.is_dirty());
        assert!(ui.generate_ai_plan(0.0).is_err(), "no second job while running");
        assert!(ui.poll_generation());
        assert!(ui.current_plan.is_none());
        assert_eq!(ui.progress_total, 3);

        release(&gate);
        assert!(ui.wait_generation(Duration::from_secs(10)));
        assert!(!ui.is_generating());
        assert_eq!(ui.progress_done, 3);
        assert!(ui.current_plan.is_some());
        assert!(!ui.status_is_error);
    }

    #[test]
    fn cancel_releases_ui_and_discards_late_result() {
        let gate = Arc::new(Gate { open: Mutex::new(false), cv: Condvar::new(), entered: AtomicUsize::new(0) });
        let mut ui = AiPlannerUi::default();
        ui.set_custom_generator(gated_generator(gate.clone()));
        ui.start_generation(0.0).unwrap();
        while gate.entered.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
        ui.cancel_generation();
        assert!(!ui.is_generating());
        assert!(ui.status_message.contains("cancelled"));
        // Provider finishes late: nothing may become applicable.
        release(&gate);
        std::thread::sleep(Duration::from_millis(50));
        assert!(!ui.poll_generation());
        assert!(ui.current_plan.is_none());
        // Retry after cancel works.
        let prompts = Arc::new(Mutex::new(Vec::new()));
        ui.set_custom_generator(instant_generator(prompts));
        assert!(ui.generate_ai_plan(0.0).is_ok());
    }

    #[test]
    fn input_change_during_generation_invalidates_result() {
        let gate = Arc::new(Gate { open: Mutex::new(false), cv: Condvar::new(), entered: AtomicUsize::new(0) });
        let mut ui = AiPlannerUi::default();
        ui.set_custom_generator(gated_generator(gate.clone()));
        ui.start_generation(0.0).unwrap();
        ui.prompt_description.push_str(" more rivers");
        ui.sync_inputs();
        release(&gate);
        assert!(ui.wait_generation(Duration::from_secs(10)));
        assert!(ui.current_plan.is_none());
        assert!(ui.status_message.contains("invalid"));
    }

    #[test]
    fn editing_inputs_invalidates_existing_plan() {
        let prompts = Arc::new(Mutex::new(Vec::new()));
        let mut ui = AiPlannerUi::default();
        ui.set_custom_generator(instant_generator(prompts));
        ui.generate_ai_plan(0.0).unwrap();
        assert!(ui.current_plan.is_some());
        ui.locks_text = "0,0,2,2".into();
        ui.sync_inputs();
        assert!(ui.current_plan.is_none());
        assert!(ui.plan_summary.is_empty());
    }

    #[test]
    fn partial_failure_is_reported_but_plan_remains_reviewable() {
        let mut ui = AiPlannerUi::default();
        ui.set_custom_generator(Arc::new(|_m, prompt, _mats, _w| {
            if prompt.contains("Water specialist") {
                return Err("boom".to_string());
            }
            let plan = fake_plan_for(prompt);
            let raw = serde_json::to_string(&plan).unwrap();
            Ok((plan, raw))
        }));
        assert!(ui.generate_ai_plan(0.0).is_ok());
        assert!(ui.status_message.contains("partial plan"));
        assert!(ui.plan_summary.contains("Failed"));
        assert!(ui.plan_summary.contains("boom"));
    }

    #[test]
    fn all_roles_failing_yields_no_plan() {
        let mut ui = AiPlannerUi::default();
        ui.set_custom_generator(Arc::new(|_m, _p, _mats, _w| Err("down".to_string())));
        assert!(ui.generate_ai_plan(0.0).is_err());
        assert!(ui.current_plan.is_none());
        assert!(ui.status_is_error);
        assert!(!ui.dirty);
    }

    #[test]
    fn reset_session_cancels_running_job() {
        let gate = Arc::new(Gate { open: Mutex::new(false), cv: Condvar::new(), entered: AtomicUsize::new(0) });
        let mut ui = AiPlannerUi::default();
        ui.set_custom_generator(gated_generator(gate.clone()));
        ui.start_generation(0.0).unwrap();
        ui.reset_session();
        assert!(!ui.is_generating());
        assert!(!ui.is_dirty());
        release(&gate);
    }
}
