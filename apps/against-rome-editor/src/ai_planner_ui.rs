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
    AiMapPlanPreviewBuilder,
    AiMapRoleProgress, AiMapRoleStatus, AiMaterialOption, CancelToken, HttpTransport,
    MultiAiMapPlanResult, MultiAiMapPlanner, MultiAiMapRoleRequest, OllamaMapPlanner,
};
use against_rome_textures::session::TerrainBlendEditSession;
use eframe::egui::{self, Color32, Vec2};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::hash::{Hash, Hasher};

struct VisualPreviewCache {
    key: u64,
    texture: egui::TextureHandle,
    changes: AiMapApplyResult,
}

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

enum ModelRefreshMsg {
    Done { url: String, result: Result<Vec<String>, String> },
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
    pub english: bool,
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
    /// Paintable materials of the loaded map, offered to the Materials specialist. Filled by the
    /// app from the Textures catalog; plans may only name these ids.
    pub material_options: Vec<AiMaterialOption>,

    // Generated plan & preview
    pub current_plan: Option<AiMapPlan>,
    pub plan_summary: String,
    visual_preview: Option<VisualPreviewCache>,
    pub last_apply_result: Option<AiMapApplyResult>,
    /// Incremented by every successful Apply, so the host can refresh dependent views.
    pub applied_revision: u64,

    // Background generation state
    job: Option<GenerationJob>,
    model_refresh: Option<Receiver<ModelRefreshMsg>>,
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
            english: false,
            ollama_url: "http://localhost:11434".to_string(),
            role_models: std::array::from_fn(|_| DEFAULT_MODEL.to_string()),
            available_models: vec![DEFAULT_MODEL.to_string(), "llama3:latest".to_string()],
            prompt_description: "A highland valley with a meandering river and fortified hilltops.".to_string(),
            bounds_text: "0,0,64,64".to_string(),
            locks_text: String::new(),
            edit_passability: false,
            role_checks: [true; 3],
            material_options: Vec::new(),
            current_plan: None,
            plan_summary: String::new(),
            visual_preview: None,
            last_apply_result: None,
            applied_revision: 0,
            job: None,
            model_refresh: None,
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

pub fn tl<'a>(english: bool, zh: &'a str, en: &'a str) -> &'a str {
    if english {
        en
    } else {
        zh
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
        self.visual_preview = None;
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

    pub fn role_label_localized(role: AiMapDesignRole, english: bool) -> &'static str {
        match role {
            AiMapDesignRole::Terrain => tl(english, "地形 (Terrain)", "Terrain"),
            AiMapDesignRole::Water => tl(english, "水域 (Water)", "Water"),
            AiMapDesignRole::Materials => tl(english, "材質 (Materials)", "Materials"),
        }
    }

    pub fn status_label_localized(status: AiMapRoleStatus, english: bool) -> &'static str {
        match status {
            AiMapRoleStatus::Succeeded => tl(english, "成功", "Succeeded"),
            AiMapRoleStatus::Failed => tl(english, "失敗", "Failed"),
            AiMapRoleStatus::Cancelled => tl(english, "已取消", "Cancelled"),
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
        if self.model_refresh.is_some() { return; }
        let url = self.ollama_url.clone();
        let planner = OllamaMapPlanner::new(Some(&url), self.custom_transport.clone());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || { let _ = tx.send(ModelRefreshMsg::Done { url, result: planner.list_models() }); });
        self.model_refresh = Some(rx);
        self.status_message = tl(self.english, "正在連線至 Ollama 並列出模型…", "Connecting to Ollama and listing models…").into();
        self.status_is_error = false;
    }

    /// 收取背景模型探索結果；使用者切換端點後，捨棄舊端點的延遲回應。
    pub fn poll_model_refresh(&mut self) -> bool {
        let Some(rx) = &self.model_refresh else { return false };
        match rx.try_recv() {
            Ok(ModelRefreshMsg::Done { url, result }) => {
                self.model_refresh = None;
                if url != self.ollama_url {
                    self.status_message = tl(self.english, "Ollama 端點已變更，請重新列出模型。", "Ollama endpoint changed; list models again to refresh.").into();
                    self.status_is_error = false;
                    return false;
                }
                match result {
                    Ok(models) => {
                        for model in &mut self.role_models {
                            if model.trim().is_empty() {
                                if let Some(first) = models.first() { *model = first.clone(); }
                            }
                        }
                        self.status_message = if models.is_empty() {
                            tl(self.english, "找不到 Ollama 模型，請先安裝模型或手動輸入名稱。", "No Ollama models found. Install one or enter a known model name.").into()
                        } else if self.english { format!("Discovered {} Ollama models.", models.len()) }
                        else { format!("已找到 {} 個 Ollama 模型。", models.len()) };
                        self.available_models = models;
                        self.status_is_error = false;
                    }
                    Err(e) => {
                        self.status_message = if self.english { format!("Failed to list Ollama models: {e}") }
                            else { format!("列出 Ollama 模型失敗：{e}") };
                        self.status_is_error = true;
                    }
                }
                false
            }
            Err(TryRecvError::Empty) => true,
            Err(TryRecvError::Disconnected) => {
                self.model_refresh = None;
                self.status_message = tl(self.english, "模型探索工作意外停止，請重試。", "Model discovery worker stopped unexpectedly; retry.").into();
                self.status_is_error = true;
                false
            }
        }
    }

    /// 無視窗測試用的等待包裝；互動 UI 每幀輪詢，不會阻塞。
    #[cfg(test)]
    fn wait_model_refresh(&mut self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while self.poll_model_refresh() {
            if Instant::now() >= deadline { return false; }
            std::thread::sleep(Duration::from_millis(2));
        }
        true
    }

    fn compute_inputs_key(&self) -> String {
        format!(
            "{}|{:?}|{:?}|{}|{}|{}|{}",
            self.ollama_url, self.role_models, self.role_checks, self.bounds_text, self.locks_text, self.edit_passability,
            self.prompt_description
        )
    }

    fn visual_preview_key(
        plan: &AiMapPlan,
        session: &TerrainSession,
        blend: Option<&TerrainBlendEditSession>,
        water_level: f32,
    ) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        serde_json::to_vec(plan).unwrap_or_default().hash(&mut hasher);
        session.heights().hash(&mut hasher);
        session.collision().hash(&mut hasher);
        blend.map(|b| b.current_textures()).hash(&mut hasher);
        session.vertex_size.hash(&mut hasher);
        session.collision_size.hash(&mut hasher);
        water_level.to_bits().hash(&mut hasher);
        hasher.finish()
    }

    fn update_visual_preview(
        &mut self,
        ctx: &egui::Context,
        session: Option<&TerrainSession>,
        blend: Option<&TerrainBlendEditSession>,
        water_level: f32,
    ) {
        let (Some(plan), Some(session)) = (self.current_plan.as_ref(), session) else {
            self.visual_preview = None;
            return;
        };
        let key = Self::visual_preview_key(plan, session, blend, water_level);
        if self.visual_preview.as_ref().is_some_and(|preview| preview.key == key) {
            return;
        }
        let preview = AiMapPlanPreviewBuilder::build(
            plan,
            session.core(),
            blend,
            Self::tile_dimension(session.vertex_size),
            water_level,
            None,
        );
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [preview.image.width, preview.image.height],
            &preview.image.pixels,
        );
        let texture = ctx.load_texture("ai_plan_visual_preview", image, egui::TextureOptions::NEAREST);
        self.visual_preview = Some(VisualPreviewCache { key, texture, changes: preview.changes });
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
            self.visual_preview = None;
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
        self.visual_preview = None;
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

        if self.role_checks[2] && self.material_options.is_empty() {
            return fail(
                self,
                "The Materials specialist needs the map's material catalog; load the map's textures (boden.txt and floortex.dat) or untick Materials.".to_string(),
            );
        }
        let materials = self.material_options.clone();

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
                    let outside = Self::features_outside_scope(plan, &scope);
                    if outside > 0 {
                        review.push_str(&format!(
                            "Warning: {outside} of {} features are anchored outside the edit area or inside a lock and will be rejected on Apply.\n",
                            plan.features.len()
                        ));
                    }
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
        // Real local inference takes minutes per role; 60 s aborted genuine runs.
        if !self.wait_generation(Duration::from_secs(20 * 60)) {
            self.cancel_generation();
            return Err("Generation timed out.".into());
        }
        if self.current_plan.is_some() {
            Ok(())
        } else {
            Err(self.status_message.clone())
        }
    }

    /// Material tiles per side for a height grid (`vertex = tiles * 4 + 1`).
    pub fn tile_dimension(vertex_size: usize) -> usize {
        ((vertex_size.saturating_sub(1)) / 4).max(1)
    }

    /// Features whose anchor tile is outside the edit area or inside a lock; Apply rejects them.
    pub fn features_outside_scope(plan: &AiMapPlan, scope: &AiMapEditScope) -> usize {
        plan.features
            .iter()
            .filter(|f| !scope.allows_tile(f.x, f.y))
            .count()
    }

    pub fn apply_plan_to_session(
        &mut self,
        session: &mut TerrainSession,
        mut blend: Option<&mut TerrainBlendEditSession>,
        water_level: f32,
    ) -> Result<AiMapApplyResult, String> {
        if self.job.is_some() {
            return Err("Generation is still running.".into());
        }
        let Some(plan) = &self.current_plan else {
            return Err("No AI plan generated to apply.".into());
        };
        let wants_materials = plan.base_material.is_some()
            || plan.features.iter().any(|f| f.material.as_deref().is_some_and(|m| !m.trim().is_empty()));
        if wants_materials && blend.is_none() {
            return Err("This plan paints materials; load the map's textures (boden.txt) before applying.".into());
        }

        // The operator works in material tiles (4 height cells each; 64 on a 257-vertex map),
        // the same units as the prompt's coordinates and the edit-area box.
        let dimension = Self::tile_dimension(session.vertex_size);
        // Accumulate every stroke and commit once per layer, so one Undo of each layer reverts
        // the whole plan like the other editor tools.
        let result = AiEditorOperator::apply(
            plan,
            Some(session.core_mut()),
            blend.as_deref_mut(),
            dimension,
            water_level,
            false,
            None,
            None,
            None,
        );
        session.commit_stroke();
        if let Some(b) = blend {
            b.commit_stroke();
        }

        let changed = result.height_samples_changed > 0
            || result.collision_pixels_changed > 0
            || result.material_strokes > result.rejected_material_strokes;
        self.status_message = if changed {
            format!(
                "Applied plan: {} height samples, {} collision pixels, {} of {} material strokes, {} operations.",
                result.height_samples_changed,
                result.collision_pixels_changed,
                result.material_strokes - result.rejected_material_strokes,
                result.material_strokes,
                result.operation_count()
            )
        } else {
            format!(
                "Applied plan changed nothing: all {} operations fell outside the edit area or locks. Generate again or widen the area.",
                result.operation_count()
            )
        };
        self.status_is_error = !changed;
        self.last_apply_result = Some(result.clone());
        self.applied_revision += 1;
        // C#: the plan is consumed by Apply (no double apply). Its edits now live in the
        // terrain/material sessions, which carry their own dirty state and undo, so the
        // planner itself holds nothing unsaved and must not block Safe Save.
        self.current_plan = None;
        self.visual_preview = None;
        self.dirty = false;
        Ok(result)
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        session: Option<&mut TerrainSession>,
        blend: Option<&mut TerrainBlendEditSession>,
        water_level: f32,
    ) {
        if !self.open {
            return;
        }
        self.poll_generation();
        let model_refreshing = self.poll_model_refresh();
        self.sync_inputs();
        self.update_visual_preview(ctx, session.as_deref(), blend.as_deref(), water_level);
        let busy = self.job.is_some();
        let has_session = session.is_some();
        if busy {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if model_refreshing { ctx.request_repaint_after(Duration::from_millis(100)); }

        let mut is_open = self.open;
        let title = tl(
            self.english,
            "AI 地圖企劃工作室 (Ollama) / AI Planner Studio",
            "AI Map Planning Studio (Ollama)",
        );
        egui::Window::new(title)
            .open(&mut is_open)
            .min_size(Vec2::new(760.0, 560.0))
            .show(ctx, |ui| {
                ui.heading(tl(self.english, "多角色 AI 製圖與筆刷執行器", "Multi-AI Map Planning & Brush Operator"));
                ui.label(tl(
                    self.english,
                    "引導本機 AI LLM 規劃地形特徵，完全透過通過驗證的編輯器筆刷操作執行變更。",
                    "Directs AI LLMs to design terrain features, executing changes exclusively through verified /brush editor operations.",
                ));

                ui.separator();
                if !self.status_message.is_empty() {
                    let color = if self.status_is_error {
                        Color32::from_rgb(250, 100, 100)
                    } else {
                        Color32::from_rgb(120, 220, 120)
                    };
                    ui.colored_label(color, self.localized_status_message());
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
                        ui.label(tl(self.english, "Ollama 端點 URL：", "Ollama Endpoint URL:"));
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.ollama_url);
                            if ui.add_enabled(!model_refreshing, egui::Button::new(tl(self.english, "連線並列出模型", "Connect & List Models"))).clicked() {
                                self.refresh_models();
                            }
                        });
                        ui.end_row();

                        for (i, role) in ROLES.iter().enumerate() {
                            ui.checkbox(&mut self.role_checks[i], Self::role_label_localized(*role, self.english));
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

                        ui.label(tl(self.english, "生效區域 (X, Y, 寬, 高)：", "Area X,Y,width,height:"));
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.bounds_text);
                            ui.checkbox(&mut self.edit_passability, tl(self.english, "編輯通行度", "Edit passability"));
                        });
                        ui.end_row();

                        ui.label(tl(self.english, "AI 鎖定區域（每行一個區域）：", "AI locks (one area per line):"));
                        ui.add(
                            egui::TextEdit::multiline(&mut self.locks_text)
                                .desired_rows(3)
                                .hint_text("X,Y,width,height"),
                        );
                        ui.end_row();
                    });

                    ui.add_space(6.0);
                    ui.label(tl(self.english, "企劃設計描述／提示詞：", "Design Description / Prompt:"));
                    ui.text_edit_multiline(&mut self.prompt_description);
                });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(!busy, egui::Button::new(tl(self.english, "產生企劃／重試", "Generate / Retry"))).clicked() {
                        let _ = self.start_generation(water_level);
                    }
                    if ui.add_enabled(busy, egui::Button::new(tl(self.english, "取消產生", "Cancel generation"))).clicked() {
                        self.cancel_generation();
                    }

                    let can_apply = self.current_plan.is_some() && session.is_some() && !busy;
                    if ui.add_enabled(can_apply, egui::Button::new(tl(self.english, "套用 AI 企劃至地形", "Apply Plan to Map Terrain"))).clicked() {
                        if let Some(s) = session {
                            if let Err(e) = self.apply_plan_to_session(s, blend, water_level) {
                                self.status_message = e;
                                self.status_is_error = true;
                            }
                        }
                    }
                });

                ui.separator();
                if let Some(plan) = &self.current_plan {
                    ui.heading(tl(self.english, "企劃預覽：", "Plan Preview:"));
                    ui.label(if self.english {
                        format!("Features: {} (Base Height: {:?}, Base Material: {:?})", plan.features.len(), plan.base_height, plan.base_material)
                    } else {
                        format!("地形特徵：{}（基礎高度：{:?}，基礎材質：{:?}）", plan.features.len(), plan.base_height, plan.base_material)
                    });
                    if let Some(preview) = &self.visual_preview {
                        ui.horizontal_top(|ui| {
                            let side = ui.available_height().min(220.0).max(120.0);
                            ui.image((preview.texture.id(), Vec2::splat(side)));
                            ui.vertical(|ui| {
                                ui.label(tl(self.english,
                                    "地圖預覽：綠為地形，藍色水域依深度加深，橙為材質變更，紅為阻擋，紫為遭拒特徵，灰為保護區。",
                                    "Map preview: green terrain; blue water darkens with depth; orange material changes, red blocked cells, purple rejected features, grey protected areas."));
                                ui.label(format!("{} {}", tl(self.english, "預計高度取樣點：", "Height samples:"), preview.changes.height_samples_changed));
                                ui.label(format!("{} {}", tl(self.english, "預計阻擋像素：", "Collision pixels:"), preview.changes.collision_pixels_changed));
                                ui.label(format!("{} {}", tl(self.english, "材質筆刷：", "Material strokes:"), preview.changes.material_strokes));
                            });
                        });
                    } else if !has_session {
                        ui.label(tl(self.english, "載入地圖地形後即可產生視覺預覽。", "Load map terrain to build the visual preview."));
                    }
                    if !self.plan_summary.is_empty() {
                        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                            ui.monospace(&self.plan_summary);
                        });
                    }
                } else if !busy {
                    ui.label(tl(self.english, "尚未產生 AI 企劃。請點選「產生企劃／重試」開始。", "No AI plan generated. Click 'Generate / Retry' to start."));
                }

                if let Some(apply_res) = &self.last_apply_result {
                    ui.separator();
                    ui.heading(tl(self.english, "執行報告：", "Execution Report:"));
            ui.label(format!("{} {}", tl(self.english, "已修改高度取樣點：", "Modified Height Samples:"), apply_res.height_samples_changed));
            ui.label(format!("{} {}", tl(self.english, "已修改阻擋像素：", "Modified Collision Pixels:"), apply_res.collision_pixels_changed));
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

            let modal_title = tl(
                self.english,
                "放棄未儲存的 AI 企劃？",
                "Discard Unsaved AI Plan?",
            );
            egui::Window::new(modal_title)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(tl(
                        self.english,
                        "關閉視窗將放棄目前產生的 AI 企劃特徵（並取消任何執行中的生成）。",
                        "Closing will discard currently generated AI plan features (and cancel any running generation).",
                    ));
                    ui.horizontal(|ui| {
                        if ui.button(tl(self.english, "放棄並關閉", "Discard & Close")).clicked() {
                            confirm = true;
                        }
                        if ui.button(tl(self.english, "繼續編輯", "Keep Editing")).clicked() {
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

    fn localized_status_message(&self) -> &str {
        if self.status_message == "Ready." {
            tl(self.english, "就緒。", "Ready.")
        } else {
            &self.status_message
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

    /// Planner with real catalog-style material ids, as the app provides after loading textures.
    fn test_ui() -> AiPlannerUi {
        let mut ui = AiPlannerUi::default();
        ui.material_options = vec![
            AiMaterialOption::new("B5", "Sand"),
            AiMaterialOption::new("B3", "Grey Rock"),
        ];
        ui
    }

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
                base_material: Some("B5".into()),
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
        let mut ui = test_ui();
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
        let mut ui = test_ui();
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
        assert!(ui.wait_model_refresh(Duration::from_secs(2)));
        assert_eq!(ui.available_models, vec!["mock-model:latest".to_string()]);

        assert!(ui.generate_ai_plan(0.0).is_ok());
        assert!(ui.current_plan.is_some());
        assert!(ui.is_dirty());
    }

    #[test]
    fn model_discovery_is_background_and_discards_results_for_old_endpoint() {
        let gate = Arc::new(Gate { open: Mutex::new(false), cv: Condvar::new(), entered: AtomicUsize::new(0) });
        let worker_gate = gate.clone();
        let transport = Arc::new(MockHttpTransport::new(move |_method, _url, _body| {
            worker_gate.entered.fetch_add(1, Ordering::SeqCst);
            let mut open = worker_gate.open.lock().unwrap();
            while !*open { open = worker_gate.cv.wait(open).unwrap(); }
            Ok(HttpResponse::new(200, r#"{"models":[{"name":"old-endpoint-model"}]}"#))
        }));
        let mut ui = test_ui();
        ui.set_custom_transport(transport);
        let original_models = ui.available_models.clone();

        ui.refresh_models();
        assert!(!ui.is_generating(), "模型探索不應被算成地圖生成工作");
        assert!(ui.poll_model_refresh(), "尚未收到結果時應保持背景工作狀態");
        while gate.entered.load(Ordering::SeqCst) == 0 { std::thread::sleep(Duration::from_millis(1)); }
        ui.ollama_url = "http://localhost:11435".into();
        ui.sync_inputs();
        release(&gate);
        assert!(ui.wait_model_refresh(Duration::from_secs(2)));
        assert_eq!(ui.available_models, original_models, "不可用舊端點結果覆蓋模型清單");
        assert!(ui.status_message.contains("端點已變更"), "{}", ui.status_message);
    }

    #[test]
    fn visual_plan_preview_renders_without_mutating_map_or_history_and_refreshes_on_edit() {
        let mut ui = test_ui();
        ui.open = true;
        ui.current_plan = Some(AiMapPlan {
            base_height: Some(45),
            ..Default::default()
        });
        let mut session = TerrainSession::create_blank(33, 32);
        let original_heights = session.heights().to_vec();
        let original_collision = session.collision().to_vec();
        let ctx = egui::Context::default();

        let _ = ctx.run(Default::default(), |ctx| ui.show(ctx, Some(&mut session), None, 0.0));
        let first = ui.visual_preview.as_ref().expect("rendered visual preview");
        assert!(first.changes.height_samples_changed > 0);
        let first_key = first.key;
        let first_texture = first.texture.id();
        assert_eq!(session.heights(), original_heights);
        assert_eq!(session.collision(), original_collision);
        assert!(!session.is_dirty());
        assert!(!session.can_undo());

        let _ = ctx.run(Default::default(), |ctx| ui.show(ctx, Some(&mut session), None, 0.0));
        let cached = ui.visual_preview.as_ref().unwrap();
        assert_eq!(cached.key, first_key);
        assert_eq!(cached.texture.id(), first_texture, "unchanged source should reuse its texture");

        let before = session.heights()[0];
        session.apply_height_changes(&[crate::terrain::SampleChange { index: 0, before, after: before + 1 }]);
        assert!(session.commit_stroke());
        let edited_heights = session.heights().to_vec();
        let undo_available = session.can_undo();
        let _ = ctx.run(Default::default(), |ctx| ui.show(ctx, Some(&mut session), None, 0.0));
        assert_ne!(ui.visual_preview.as_ref().unwrap().key, first_key);
        assert_eq!(session.heights(), edited_heights);
        assert_eq!(session.can_undo(), undo_available, "preview must preserve the user's history");
    }

    #[test]
    fn test_ai_planner_ui_apply_to_terrain_session() {
        let mut ui = test_ui();
        let mut plan = AiMapPlan::default();
        plan.base_height = Some(40);
        ui.current_plan = Some(plan);

        let mut session = TerrainSession::create_blank(33, 32);
        let res = ui.apply_plan_to_session(&mut session, None, 0.0);
        assert!(res.is_ok());
        assert!(ui.last_apply_result.is_some());
        assert!(!ui.is_dirty(), "an applied plan no longer blocks Safe Save");
        assert!(session.is_dirty());
        // The plan is consumed (C# `_plan = null`): no double apply.
        assert!(ui.current_plan.is_none());
        assert!(ui.apply_plan_to_session(&mut session, None, 0.0).is_err());
    }

    #[test]
    fn applying_a_multi_stroke_plan_is_one_undo_step() {
        let plan: AiMapPlan = serde_json::from_value(serde_json::json!({
            "baseHeight": 40,
            "features": [
                {"type": "mountain", "x": 10, "y": 10, "radius": 5, "amount": 30},
                {"type": "valley", "x": 22, "y": 22, "radius": 5, "amount": 20}
            ]
        }))
        .unwrap();
        let mut ui = test_ui();
        ui.current_plan = Some(plan);
        let mut session = TerrainSession::create_blank(257, 32);
        let original = session.heights().to_vec();
        let result = ui.apply_plan_to_session(&mut session, None, 0.0).unwrap();
        assert!(result.operation_count() >= 3, "{}", result.operation_count());
        let applied = session.heights().to_vec();
        assert_ne!(applied, original);
        assert!(session.undo());
        assert_eq!(session.heights(), &original[..], "one undo reverts the whole plan");
        assert!(!session.can_undo());
        assert!(session.redo());
        assert_eq!(session.heights(), &applied[..]);
    }

    /// Plan captured from a real laguna-xs-2.1 run (`tests/ai_planner_live.rs`). The model was
    /// asked for X/Y 4..27 but anchored both features at x=32: review must warn, and an Apply
    /// that changes nothing must say so instead of reporting success.
    const REAL_LAGUNA_PLAN: &str = r#"{"summary":"t","features":[{"type":"hill","location":"center","x":32,"y":32,"radius":8,"amount":45},{"type":"valley","location":"north-center","x":32,"y":20,"radius":6,"amount":10}]}"#;

    #[test]
    fn real_laguna_plan_outside_edit_area_is_flagged_and_reported() {
        let mut plan: AiMapPlan = serde_json::from_str(REAL_LAGUNA_PLAN).unwrap();
        let scope = AiMapEditScope::parse("4,4,24,24", "", false).unwrap();
        assert_eq!(AiPlannerUi::features_outside_scope(&plan, &scope), 2);
        plan.edit_scope = Some(scope);
        let mut ui = test_ui();
        ui.current_plan = Some(plan);
        let mut session = TerrainSession::create_blank(257, 60);
        let result = ui.apply_plan_to_session(&mut session, None, 0.0).unwrap();
        assert_eq!(result.height_samples_changed, 0);
        assert!(ui.status_is_error);
        assert!(ui.status_message.contains("changed nothing"), "{}", ui.status_message);
        assert!(!session.can_undo(), "an empty apply adds no undo step");
    }

    #[test]
    fn plan_coordinates_are_material_tiles_not_height_vertices() {
        assert_eq!(AiPlannerUi::tile_dimension(257), 64);
        assert_eq!(AiPlannerUi::tile_dimension(193), 48);
        let mut plan: AiMapPlan = serde_json::from_str(REAL_LAGUNA_PLAN).unwrap();
        plan.edit_scope = Some(AiMapEditScope::parse("0,0,64,64", "", false).unwrap());
        let mut ui = test_ui();
        ui.current_plan = Some(plan);
        let mut session = TerrainSession::create_blank(257, 60);
        let result = ui.apply_plan_to_session(&mut session, None, 0.0).unwrap();
        assert!(result.height_samples_changed > 0, "{result:#?}");
        // Tile (32,32) is the map centre, vertex ~(130,130); vertex (32,32) stays untouched.
        let h = |x: usize, y: usize| session.heights()[y * 257 + x];
        assert!(h(130, 130) > 60, "centre raised: {}", h(130, 130));
        assert_eq!(h(32, 32), 60);
    }

    #[test]
    fn fake_planner_generation_attaches_scope_locks_and_passability() {
        let prompts = Arc::new(Mutex::new(Vec::new()));
        let mut ui = test_ui();
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
        assert_eq!(plan.base_material.as_deref(), Some("B5"));
        assert!(ui.plan_summary.contains("Editable-area base height: 60"));
        assert!(ui.plan_summary.contains("Editable-area base material: B5"));
        assert!(ui.plan_summary.contains("2 locks"));
        assert!(ui.dirty);

        let prompts = prompts.lock().unwrap();
        assert_eq!(prompts.len(), 3);
        assert!(prompts[0].contains("X=8..31, Y=8..31"));
        assert!(prompts[0].contains("10,10,4,4; 20,20,2,2"));
        assert!(!prompts[0].contains("passability is protected"));
    }

    #[test]
    fn materials_role_offers_only_catalog_ids_and_needs_a_catalog() {
        let offered = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
        let seen = offered.clone();
        let mut ui = AiPlannerUi::default();
        ui.set_custom_generator(Arc::new(move |_m, prompt, mats: &[AiMaterialOption], _w| {
            seen.lock().unwrap().push(mats.iter().map(|m| m.id.clone()).collect());
            let plan = fake_plan_for(prompt);
            Ok((plan.clone(), serde_json::to_string(&plan).unwrap()))
        }));
        ui.role_checks = [false, false, true];
        let err = ui.start_generation(0.0).unwrap_err();
        assert!(err.contains("material catalog"), "{err}");
        assert!(offered.lock().unwrap().is_empty());

        ui.material_options = vec![AiMaterialOption::new("B5", "Sand")];
        assert!(ui.generate_ai_plan(0.0).is_ok());
        assert_eq!(*offered.lock().unwrap(), vec![vec!["B5".to_string()]]);
    }

    #[test]
    fn material_plan_without_textures_is_refused_before_touching_heights() {
        let mut ui = test_ui();
        ui.current_plan = Some(AiMapPlan {
            base_height: Some(50),
            base_material: Some("B5".into()),
            ..Default::default()
        });
        let mut session = TerrainSession::create_blank(257, 32);
        let err = ui.apply_plan_to_session(&mut session, None, 0.0).unwrap_err();
        assert!(err.contains("textures"), "{err}");
        assert!(!session.can_undo());
        assert!(ui.current_plan.is_some(), "a refused apply keeps the plan");
    }

    #[test]
    fn passability_requires_terrain_role_and_default_protects_it() {
        let prompts = Arc::new(Mutex::new(Vec::new()));
        let mut ui = test_ui();
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
        let mut ui = test_ui();
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
        let mut ui = test_ui();
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
        let mut ui = test_ui();
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
        let mut ui = test_ui();
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
        let mut ui = test_ui();
        ui.set_custom_generator(instant_generator(prompts));
        ui.generate_ai_plan(0.0).unwrap();
        assert!(ui.current_plan.is_some());
        ui.locks_text = "0,0,2,2".into();
        ui.sync_inputs();
        assert!(ui.current_plan.is_none());
        assert!(ui.plan_summary.is_empty());

        ui.generate_ai_plan(0.0).unwrap();
        assert!(ui.current_plan.is_some());
        ui.ollama_url.push_str("/new-endpoint");
        ui.sync_inputs();
        assert!(ui.current_plan.is_none(), "切換 Ollama 端點後必須使舊方案失效");
    }

    #[test]
    fn partial_failure_is_reported_but_plan_remains_reviewable() {
        let mut ui = test_ui();
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
        let mut ui = test_ui();
        ui.set_custom_generator(Arc::new(|_m, _p, _mats, _w| Err("down".to_string())));
        assert!(ui.generate_ai_plan(0.0).is_err());
        assert!(ui.current_plan.is_none());
        assert!(ui.status_is_error);
        assert!(!ui.dirty);
    }

    #[test]
    fn reset_session_cancels_running_job() {
        let gate = Arc::new(Gate { open: Mutex::new(false), cv: Condvar::new(), entered: AtomicUsize::new(0) });
        let mut ui = test_ui();
        ui.set_custom_generator(gated_generator(gate.clone()));
        ui.start_generation(0.0).unwrap();
        ui.reset_session();
        assert!(!ui.is_generating());
        assert!(!ui.is_dirty());
        release(&gate);
    }

    #[test]
    fn test_ai_planner_bilingual_titles_and_labels() {
        let mut ui = test_ui();
        assert!(!ui.english);
        assert_eq!(AiPlannerUi::role_label_localized(AiMapDesignRole::Terrain, false), "地形 (Terrain)");
        assert_eq!(AiPlannerUi::role_label_localized(AiMapDesignRole::Terrain, true), "Terrain");
        assert_eq!(AiPlannerUi::status_label_localized(AiMapRoleStatus::Succeeded, false), "成功");
        assert_eq!(AiPlannerUi::status_label_localized(AiMapRoleStatus::Succeeded, true), "Succeeded");
        assert_eq!(tl(false, "企劃預覽：", "Plan Preview:"), "企劃預覽：");
        assert_eq!(tl(true, "企劃預覽：", "Plan Preview:"), "Plan Preview:");
        ui.english = true;
        assert!(ui.english);
    }

    #[test]
    fn test_ai_planner_ready_status_is_localized_in_rendered_ui() {
        let mut ui = test_ui();
        ui.open = true;
        let chinese = crate::batch20_localization_tests::render(|ctx| {
            ui.show(ctx, None, None, 0.0);
        });
        assert!(chinese.contains("就緒。"), "{chinese}");
        assert!(!chinese.contains("Ready."), "{chinese}");

        ui.english = true;
        let english = crate::batch20_localization_tests::render(|ctx| {
            ui.show(ctx, None, None, 0.0);
        });
        assert!(english.contains("Ready."), "{english}");
        assert!(!english.contains("就緒。"), "{english}");
    }
}
