//! Against Rome 原生 Rust 桌面地圖編輯器主介面 (egui/eframe)

#[path = "app_recovery.rs"]
mod app_recovery;
pub mod save_blockers;

use against_rome_environment::nature::{
    map_nature_regions, NatureCatalogItem, NatureCategory, NatureDensity, NatureEditSession,
    NatureHeightField, NatureOperation, NaturePaletteFilter, NatureStrokeDriver,
    NatureStrokeReport, NatureStrokeSettings, PlacedNatureObject,
};
use against_rome_environment::DotNetRandom;
use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use against_rome_ambience::{Language as AppLanguage, Loc};
use crate::ambience_ui::AmbienceUi;
use crate::minimap_ui::MinimapUi;
use crate::modifier_ui::ModifierUi;
use crate::scene_ui::SceneUi;
use crate::storage::load_map;
#[cfg(test)]
use crate::storage::safe_save_map;
use crate::tech_doc_ui::TechDocUi;
use crate::terrain::{BrushPath, CollisionBrushOp, HeightBrushOp, TerrainSession};
use crate::textures_ui::TexturesUi;
use crate::viewport_3d::{
    expand_troops_for_3d, load_formation_definition, TroopCatalog, Viewport3d, ViewportAction,
};
use against_rome_ai_planner::formation::FormationLayoutDefinition;
use crate::{
    ai_planner_ui::AiPlannerUi,
    campaign_ui::CampaignUi,
    environment_ui::{EnvApply, EnvironmentUi},
    landscape_ui::{LandscapeApply, LandscapeContext, LandscapeUi},
    maps_ui::{BlankMapBuildIntent, MapsContext, MapsUi},
    packaging_ui::PackagingUi,
};
use crate::{console::ConsoleUi, event_graph::EventGraphUi};
use crate::{map_settings::MapSettings, navigation::NavigationUi, toolbox::ToolboxUi};
use crate::{placement_ui::PlacementUi, scenario_ui::ScenarioUi};
use crate::spawn_placement_ui::{SpawnContext, SpawnPlacementUi};
use crate::{layout_io::LayoutContext, placement_ui::LayoutRequest};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Heightmap,
    Collision,
    Blended,
    View3D,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveTool {
    HeightRaise,
    HeightLower,
    HeightFlatten,
    HeightSmooth,
    HeightRoughen,
    Water,
    CollisionBlock,
    CollisionClear,
    /// Interactive terrain material painting (C# texture tool).
    Material,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PendingAction {
    Load(PathBuf),
    Blank(usize),
    BuildBlank(BlankMapBuildIntent),
    Close,
}

/// Original `emboss.bmp` layer plus the C# `TerrainHeightEditSession` light/baseline state.
/// Present only when the loaded map has an emboss layer; absent means Safe Save never writes one.
#[derive(Clone)]
struct EmbossBaseline {
    raw: crate::bmp::BmpImage,
    session: against_rome_editor::emboss::EmbossSession,
}

/// Result of [`EditorApp::candidate_diagnostics`].
pub(crate) struct CandidateDiagnostics {
    pub map_issues: Vec<against_rome_maps::diagnostics::MapIssue>,
    pub design: Result<Vec<against_rome_maps::design_validator::MapDesignIssue>, String>,
}

/// 獨立事件圖載入時的劇本快照；完整比對可避免套用覆蓋後續修改或重新載入。
#[derive(PartialEq)]
struct ToolbarGraphSource {
    map: Option<PathBuf>,
    document: against_rome_scenario::ScenarioDocument,
    aliases: Vec<String>,
    saved_document: against_rome_scenario::ScenarioDocument,
}

#[cfg(test)]
#[path = "../tests/toolbar_event_graph/mod.rs"]
mod toolbar_event_graph_tests;

pub struct EditorApp {
    recovery: crate::recovery::Recovery,
    recovery_deferred: bool,
    recent_maps: crate::recent_maps::RecentMaps,
    pub map_path_input: String,
    pub(crate) loaded_map_path: Option<PathBuf>,
    pub session: Option<TerrainSession>,
    emboss: Option<EmbossBaseline>,
    pub status_message: String,
    pub status_is_error: bool,

    // 筆刷參數
    pub active_tool: ActiveTool,
    pub brush_radius: f32,
    pub brush_strength: u8,
    pub flatten_target: u8,
    pub roughness_seed: i32,
    pub sample_flatten_target: bool,
    pub ramp_width: f32,
    pub ramp_falloff: f32,
    ramp_selecting: bool,
    ramp_start: Option<(f32, f32)>,
    /// Box-select tool (C# `_boxSelectButton`): one drag selects placed objects, then the tool turns off.
    pub box_select: bool,
    box_drag_start: Option<Pos2>,
    pub water_surface: f32,
    pub water_bed: u8,

    // 視圖參數
    pub view_mode: ViewMode,
    pub zoom: f32,
    pub pan: Vec2,
    pub canvas_cursor: Option<(f32, f32, i32, i32)>,

    // 紋理快取
    texture: Option<TextureHandle>,
    texture_dirty: bool,
    is_painting: bool,
    stroke_path: BrushPath,
    pub pending_action: Option<PendingAction>,
    close_allowed: bool,
    save_blockers_open: bool,
    save_focus_nature: bool,
    pending_save_warnings: Option<(
        PathBuf,
        Vec<against_rome_maps::design_validator::MapDesignIssue>,
    )>,
    toolbox: ToolboxUi,
    navigation: NavigationUi,
    pub(crate) height_step: f32,
    pub(crate) placement: PlacementUi,
    family: crate::family_ui::FamilyUi,
    family_open: bool,
    pub map_properties_open: bool,
    family_history_key: (usize, usize, usize),
    pub(crate) scenario: ScenarioUi,
    spawn_ui: SpawnPlacementUi,
    /// DATA landscape objects `(slot, x, z)` of the map (building clearance warning), per map.
    spawn_landscape: Option<(PathBuf, Vec<(i32, f32, f32)>)>,
    textures: TexturesUi,
    undo_journal: crate::undo_journal::UndoJournal,
    auxiliary_undo: Vec<(bool, Option<EmbossBaseline>)>,
    auxiliary_redo: Vec<(bool, Option<EmbossBaseline>)>,
    scene: SceneUi,
    event_graph: EventGraphUi,
    toolbar_graph_source: Option<ToolbarGraphSource>,
    console: ConsoleUi,
    pub maps: MapsUi,
    pub(crate) environment: EnvironmentUi,
    landscape: LandscapeUi,
    packaging: PackagingUi,
    campaign: CampaignUi,
    ai_planner: AiPlannerUi,
    /// Edit area + material revision the AI material options were computed for.
    ai_materials_key: String,
    /// Map for which the Material brush already tried to load boden.txt on demand.
    material_autoload_tried: Option<PathBuf>,
    /// Map for which the Scenario panel already tried to load its script on demand.
    pub(crate) scenario_autoload_tried: Option<PathBuf>,
    pub ambience: AmbienceUi,
    pub modifier: ModifierUi,
    pub viewport_3d: Viewport3d,
    pub tech_doc: TechDocUi,
    pub minimap: MinimapUi,
    pub(crate) design_check: crate::design_check_ui::DesignCheckUi,
    pub(crate) balance_summary: crate::balance_summary_ui::BalanceSummaryUi,
    nature: NatureWorkflow,
    settlement_undo: Vec<SettlementEdit>,
    settlement_redo: Vec<SettlementEdit>,
    /// App-level UI language (zh-TW / en) persisted through the shared `Loc` settings file.
    loc: Loc,
    last_canvas_rect: Rect,
    last_map_rect: Rect,
    /// C# `_showGrid` / `_showObjects` display toggles (2D canvas and 3D viewport).
    pub show_grid: bool,
    pub show_objects: bool,
    /// C# `_resetAuxiliaryLayers`: Flat Terrain resets `vertex.bmp` / `smooth.bmp` on the next save.
    reset_auxiliary_layers: bool,
    /// Pending confirm dialog of a bulk terrain action (C# `MessageBox`).
    terrain_confirm: Option<TerrainBulkAction>,
    /// Squad catalog and formation layout feeding the 3D view's formation members.
    troops: TroopDisplay,
    pub(crate) skirmish_wizard: crate::skirmish_wizard_ui::SkirmishWizardUi,
}

/// Background-loaded squad catalog plus the fingerprint of the spawns last expanded for the 3D
/// view (C# `ExpandTroopsFor3D`; the C# editor builds the SDL template catalog lazily too).
#[derive(Default)]
struct TroopDisplay {
    game: Option<PathBuf>,
    loading: Option<std::sync::mpsc::Receiver<Result<(TroopCatalog, FormationLayoutDefinition), String>>>,
    data: Option<(TroopCatalog, FormationLayoutDefinition)>,
    key: Option<u64>,
}

/// Bulk terrain actions that need confirmation (C# `ResetTerrain` / `ApplyBlankTerrain(confirm)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainBulkAction {
    /// 還原地表: discard unsaved terrain changes (not undoable, like C#).
    Reset,
    /// 重設平坦地形: flatten everything just above the water level (undoable per layer).
    Flat,
}

impl EditorApp {
    /// 以目前地圖的玩家主屋為目標；沒有主屋時使用已啟用的玩家起始鏡頭。
    pub(crate) fn player_main_house_target_opt(
        &self,
        path: Option<&Path>,
    ) -> Option<against_rome_scenario::WorldPosition> {
        use against_rome_maps::design_validator::MapDesignValidator;
        use against_rome_scenario::WorldPosition;
        let position = |x: f64, z: f64| {
            (x.is_finite() && z.is_finite() && (0.0..=16383.0).contains(&x)
                && (0.0..=16383.0).contains(&z))
                .then(|| WorldPosition { x: x.round() as i32, z: z.round() as i32 })
        };
        let document = self.scenario.document();
        let objects = if self.placement.is_loaded() {
            self.placement.objects()
        } else if let Some(path) = path {
            against_rome_placement::ObjectStore::load(path)
                .map(|s| s.objects())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        for spawn in &document.spawns {
            if !MapDesignValidator::is_main_house(&spawn.alias) {
                continue;
            }
            let binding = document.data_slots.iter().find(|b| b.spawn_id == spawn.id);
            let (team, x, z) = if let Some(binding) = binding {
                let Some(object) = objects.iter().find(|o| o.slot as i32 == binding.slot && o.uid == binding.uid) else {
                    continue;
                };
                (i32::from(object.team), object.position[0], object.position[2])
            } else {
                (spawn.team, spawn.x, spawn.z)
            };
            if team == 0 {
                if let Some(target) = position(x as f64, z as f64) { return Some(target); }
            }
        }
        // 原生地圖的主屋可能只存在 DATA，未收錄於編輯器劇本。
        if let Some(path) = path {
            if let Some(game) = path.parent().and_then(Path::parent) {
                if let Ok(definitions) = against_rome_maps::NativeObjectDefinitions::load(game) {
                    for object in objects.iter().filter(|o| o.team == 0 && o.parent.is_none()) {
                        if definitions.get(&i32::from(object.type_id))
                            .and_then(|d| d.name.as_deref()).is_some_and(MapDesignValidator::is_main_house)
                        {
                            if let Some(target) = position(object.position[0] as f64, object.position[2] as f64) {
                                return Some(target);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub(crate) fn player_main_house_target(&self, path: &Path) -> Option<against_rome_scenario::WorldPosition> {
        self.player_main_house_target_opt(Some(path))
    }

    pub(crate) fn wave_player_target(&self, path: &Path) -> Option<against_rome_scenario::WorldPosition> {
        if let Some(target) = self.player_main_house_target(path) {
            return Some(target);
        }
        let position = |x: f64, z: f64| {
            (x.is_finite() && z.is_finite() && (0.0..=16383.0).contains(&x)
                && (0.0..=16383.0).contains(&z))
                .then(|| against_rome_scenario::WorldPosition { x: x.round() as i32, z: z.round() as i32 })
        };
        let camera = &self.campaign.start_camera;
        camera.enabled.then(|| position(camera.x, camera.z)).flatten()
    }

    pub fn player_main_house_position(&self) -> Option<(f64, f64)> {
        let path = self.loaded_map_path.as_deref();
        self.player_main_house_target_opt(path)
            .map(|target| (target.x as f64, target.z as f64))
    }

    pub fn center_start_camera_on_player_main_house(&mut self) -> bool {
        if let Some((x, z)) = self.player_main_house_position() {
            self.campaign.start_camera.enabled = true;
            self.campaign.start_camera.x = x;
            self.campaign.start_camera.z = z;
            self.campaign.camera_dirty = true;
            let _ = self.apply_start_camera_candidate_from_ui();
            true
        } else {
            false
        }
    }


    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load the user's installed CJK font, without redistributing font files.
        #[cfg(windows)]
        if let Some(windows) = std::env::var_os("WINDIR") {
            for name in ["msjh.ttc", "msyh.ttc"] {
                if let Ok(bytes) = std::fs::read(PathBuf::from(&windows).join("Fonts").join(name)) {
                    let mut fonts = egui::FontDefinitions::default();
                    fonts
                        .font_data
                        .insert("system-cjk".into(), egui::FontData::from_owned(bytes));
                    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                        fonts
                            .families
                            .entry(family)
                            .or_default()
                            .push("system-cjk".into());
                    }
                    cc.egui_ctx.set_fonts(fonts);
                    break;
                }
            }
        }
        #[cfg(not(windows))]
        let _ = cc;
        let mut app = Self::initial_state();
        if let Some(dir) = crate::storage::editor_data_dir() {
            match crate::recent_maps::RecentMaps::load(dir.join("recent-maps.json")) {
                Ok(recent) => app.recent_maps = recent,
                Err(e) => {
                    app.status_message = format!("Recent maps unavailable / 最近地圖無法讀取：{e}");
                    app.status_is_error = true;
                }
            }
        }
        app.modifier = ModifierUi::persisted(crate::modifier_ui::default_state_dir());
        if let Some(path) = app_settings_path() {
            app.loc = Loc::load(path, "");
            app.ambience.app_language = app.loc.current_language();
        }
        app
    }

    pub fn initial_state() -> Self {
        // 預設建立一張空白 257x257 地圖以便立即可用
        let session = TerrainSession::create_blank(257, 32);

        Self {
            recent_maps: Default::default(),
            map_path_input: String::new(),
            loaded_map_path: None,
            session: Some(session),
            emboss: None,
            status_message: "Ready. Blank 257x257 map initialized.".to_string(),
            status_is_error: false,
            active_tool: ActiveTool::HeightRaise,
            brush_radius: 12.0,
            brush_strength: 8,
            flatten_target: 32,
            roughness_seed: 42,
            sample_flatten_target: true,
            ramp_width: 4.0,
            ramp_falloff: 2.0,
            ramp_selecting: false,
            ramp_start: None,
            box_select: false,
            box_drag_start: None,
            water_surface: 48.0,
            water_bed: 24,
            view_mode: ViewMode::Blended,
            zoom: 1.0,
            pan: Vec2::ZERO,
            canvas_cursor: None,
            texture: None,
            texture_dirty: true,
            is_painting: false,
            stroke_path: BrushPath::default(),
            pending_action: None,
            close_allowed: false,
            recovery: Default::default(),
            recovery_deferred: false,
            save_blockers_open: false,
            save_focus_nature: false,
            pending_save_warnings: None,
            toolbox: ToolboxUi::default(),
            navigation: NavigationUi::default(),
            height_step: 4.0,
            placement: PlacementUi::default(),
            family: crate::family_ui::FamilyUi::default(),
            family_open: false,
            map_properties_open: false,
            family_history_key: (0, 0, 0),
            scenario: ScenarioUi::default(),
            spawn_ui: SpawnPlacementUi::default(),
            spawn_landscape: None,
            textures: TexturesUi::default(),
            undo_journal: Default::default(),
            auxiliary_undo: Vec::new(),
            auxiliary_redo: Vec::new(),
            scene: SceneUi::default(),
            event_graph: EventGraphUi::default(),
            toolbar_graph_source: None,
            console: ConsoleUi::default(),
            maps: MapsUi::default(),
            environment: EnvironmentUi::default(),
            landscape: LandscapeUi::default(),
            packaging: PackagingUi::default(),
            campaign: CampaignUi::default(),
            ai_planner: AiPlannerUi::default(),
            ai_materials_key: String::new(),
            material_autoload_tried: None,
            scenario_autoload_tried: None,
            ambience: AmbienceUi::default(),
            modifier: ModifierUi::default(),
            viewport_3d: Viewport3d::new(),
            tech_doc: TechDocUi::default(),
            minimap: MinimapUi::default(),
            design_check: Default::default(),
            balance_summary: Default::default(),
            nature: NatureWorkflow::default(),
            settlement_undo: Vec::new(),
            settlement_redo: Vec::new(),
            loc: Loc::new(AppLanguage::English),
            last_canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)),
            last_map_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 600.0)),
            show_grid: true,
            show_objects: true,
            reset_auxiliary_layers: false,
            terrain_confirm: None,
            troops: TroopDisplay::default(),
            skirmish_wizard: crate::skirmish_wizard_ui::SkirmishWizardUi::default(),
        }
    }
}

impl Default for EditorApp {
    fn default() -> Self {
        Self::initial_state()
    }
}

impl EditorApp {
    /// Noninteractive save, like C# TrySaveMap: design warnings may proceed, errors may not.
    /// The toolbar uses request_safe_save for the interactive design-warning confirmation.
    pub fn safe_save_current(&mut self) {
        self.safe_save_current_using(|tx| tx.commit().map_err(|e| e.to_string()));
    }

    fn safe_save_current_using(
        &mut self,
        commit: impl FnOnce(
            &mut against_rome_maps::MapSaveTransaction,
        ) -> Result<against_rome_maps::MapSaveReport, String>,
    ) {
        self.pending_save_warnings = None;
        self.safe_save_confirming_using(|_| true, commit);
    }

    fn safe_save_confirming_using(
        &mut self,
        confirm_design: impl FnOnce(&[against_rome_maps::design_validator::MapDesignIssue]) -> bool,
        commit: impl FnOnce(
            &mut against_rome_maps::MapSaveTransaction,
        ) -> Result<against_rome_maps::MapSaveReport, String>,
    ) {
        let result = self.save_transaction(confirm_design, commit);
        match result {
            Ok(report) => {
                self.status_message = format!(
                    "Saved safely in one transaction ({} files, {} cache deletions).",
                    report.written_files.len(),
                    report.deleted_files.len()
                );
                self.status_is_error = false;
            }
            Err(err) => {
                self.status_message = format!("Safe Save Rejected: {err}; unsaved changes kept.");
                self.status_is_error = true;
            }
        }
    }

    /// Placement/Ambience Save buttons only raise requests; both collapse into one central
    /// Safe Save so the whole-map transaction stays the only product path that writes the map.
    fn consume_panel_save_requests(&mut self) {
        let placement = self.placement.take_save_request();
        let ambience = self.ambience.take_save_request();
        if placement || ambience {
            self.request_safe_save();
        }
    }

    fn request_safe_save(&mut self) {
        self.confirm_safe_save(None);
    }

    fn confirm_safe_save(
        &mut self,
        approved: Option<(
            PathBuf,
            Vec<against_rome_maps::design_validator::MapDesignIssue>,
        )>,
    ) {
        self.pending_save_warnings = None;
        let path = PathBuf::from(self.map_path_input.trim());
        let mut pending = None;
        self.safe_save_confirming_using(
            |issues| {
                if approved
                    .as_ref()
                    .is_some_and(|(p, old)| p == &path && old == issues)
                {
                    true
                } else {
                    pending = Some((path.clone(), issues.to_vec()));
                    false
                }
            },
            |tx| tx.commit().map_err(|e| e.to_string()),
        );
        if let Some(pending) = pending {
            self.pending_save_warnings = Some(pending);
            self.status_message =
                "Safe Save awaiting design warning confirmation; unsaved changes kept.".into();
            self.status_is_error = false;
        }
    }

    fn resolve_safe_save_warnings(&mut self, proceed: bool) {
        let Some(pending) = self.pending_save_warnings.take() else {
            return;
        };
        if proceed {
            // Rebuild and revalidate the candidate; confirmation never bypasses errors.
            self.confirm_safe_save(Some(pending));
        } else {
            self.status_message = "Safe Save cancelled; unsaved changes kept.".into();
            self.status_is_error = false;
            self.maps.open = true;
            self.maps.current_tab = crate::maps_ui::MapsTab::Diagnostics;
        }
    }

    /// Inspect the effective candidate, even when the scenario panel has never been opened.
    /// Staged JSON wins over saved JSON; clean/unloaded panel defaults must not hide saved spawns.
    fn save_diagnostics(
        &mut self,
        path: &Path,
        staged_scenario: &[(PathBuf, Vec<u8>)],
    ) -> Result<Vec<against_rome_maps::design_validator::MapDesignIssue>, String> {
        use against_rome_maps::diagnostics::MapIssueSeverity;
        let candidate = self.candidate_diagnostics(path, staged_scenario)?;
        self.maps.diagnostic_issues = candidate.map_issues;
        if let Some(blocking) = self
            .maps
            .diagnostic_issues
            .iter()
            .find(|issue| issue.severity == MapIssueSeverity::Error)
        {
            self.maps.open = true;
            self.maps.current_tab = crate::maps_ui::MapsTab::Diagnostics;
            return Err(format!(
                "MapDiagnostics [{}]: {}",
                blocking.code, blocking.english
            ));
        }
        candidate.design
    }

    /// Memory-only counterpart of Safe Save's scenario staging for the Design Check panel:
    /// dirty scenario/events/camera win over saved JSON. Returns the map path and staged files.
    pub(crate) fn check_candidate_inputs(
        &self,
    ) -> Result<(PathBuf, Vec<(PathBuf, Vec<u8>)>), String> {
        let path = if self.map_path_input.trim().is_empty() {
            self.loaded_map_path.clone().unwrap_or_default()
        } else {
            PathBuf::from(self.map_path_input.trim())
        };
        let staged = if self.scenario.dirty() || self.scenario.graph_ui().is_dirty() {
            self.scenario
                .stage_transaction_files(&path)
                .map_err(|e| format!("Scenario: {e}"))?
        } else {
            Vec::new()
        };
        let staged =
            self.campaign
                .stage_camera_candidate(&path, &staged, self.scenario.known_aliases())?;
        Ok((path, staged))
    }

    /// Safe Save 與唯讀設計檢查共用候選資料，同時回報地圖與劇本檢查結果。
    pub(crate) fn candidate_diagnostics(
        &self,
        path: &Path,
        staged_scenario: &[(PathBuf, Vec<u8>)],
    ) -> Result<CandidateDiagnostics, String> {
        self.candidate_diagnostics_with_pending_data(path, staged_scenario, false)
    }

    pub(crate) fn candidate_diagnostics_with_pending_data(
        &self,
        path: &Path,
        staged_scenario: &[(PathBuf, Vec<u8>)],
        check_pending_data: bool,
    ) -> Result<CandidateDiagnostics, String> {
        use against_rome_maps::{
            design_validator::{MapDesignValidationRequest, MapDesignValidator},
            diagnostics::{MapCheckObject, MapCheckSnapshot, MapDiagnostics},
            preflight::ScenarioSavePreflight,
        };
        use against_rome_scenario::model::ScenarioDocument;

        let mut document = if let Some((_, bytes)) = staged_scenario
            .iter()
            .find(|(p, _)| p == Path::new("arm_scenario.json"))
        {
            ScenarioDocument::from_json(bytes)?
        } else {
            match std::fs::read(path.join("arm_scenario.json")) {
                Ok(bytes) => ScenarioDocument::from_json(&bytes)?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => ScenarioDocument::default(),
                Err(e) => return Err(format!("Cannot read saved scenario: {e}")),
            }
        };
        // Placement edits change DATA, not scenario JSON. Use their pending positions/teams
        // for bound spawns rather than diagnosing their stale saved positions.
        if self.placement.is_dirty() {
            let objects = self.placement.objects();
            for binding in document.data_slots.iter().filter(|b| !b.spawn_id.is_nil()) {
                let spawn = document
                    .spawns
                    .iter_mut()
                    .find(|s| s.id == binding.spawn_id)
                    .ok_or("Diagnostics: missing bound scenario spawn")?;
                let object = objects.iter().find(|o| o.slot as i32 == binding.slot && o.uid == binding.uid)
                    .ok_or("Diagnostics: pending placement removed or replaced a bound scenario object")?;
                [spawn.x, spawn.y, spawn.z] = object.position;
                spawn.team = i32::from(object.team);
            }
        }
        let aliases = if self.scenario.dirty() {
            self.scenario.known_aliases().to_vec()
        } else {
            path.parent()
                .and_then(Path::parent)
                .and_then(|root| against_rome_scenario::load_aliases(root).ok())
                .unwrap_or_default()
        };
        let terrain = self
            .session
            .as_ref()
            .ok_or("No terrain session to diagnose")?;
        let snapshot = MapCheckSnapshot {
            objects: document
                .spawns
                .iter()
                .map(|s| MapCheckObject::new(s.clone(), s.count == 0, s.prebuilt))
                .collect(),
            events: document.events.clone(),
            aliases: aliases.clone(),
            collision_size: terrain.collision_size,
            collision: Some(terrain.collision().to_vec()),
            height_size: terrain.vertex_size,
            heights: Some(terrain.heights().to_vec()),
            height_step: self.height_step,
            water_level: if self.ambience.is_dirty() {
                self.ambience.profile.boden.waterlevel
            } else {
                self.water_surface
            },
        };
        let mut footprint_resolver = path.parent().and_then(Path::parent)
            .and_then(|root| crate::building_footprint::FootprintResolver::load(root).ok());
        let mut footprints = std::collections::HashMap::new();
        if let Ok(store) = self.placement.detached_store().or_else(|_| against_rome_placement::ObjectStore::load(path)) {
            let objects = store.objects();
            for binding in &document.data_slots {
                if let Some(object) = objects.iter().find(|o| o.slot as i32 == binding.slot && o.uid == binding.uid) {
                    if let Some(stencil) = footprint_resolver.as_mut().and_then(|r| r.resolve_building(i32::from(object.type_id))) {
                        footprints.insert(binding.spawn_id, stencil.cells.clone());
                    }
                }
            }
        }
        let mut map_issues = MapDiagnostics::check_with_footprints(&snapshot, &footprints);
        if check_pending_data && (self.placement.is_dirty() || self.nature.session.is_dirty()) {
            let pending = (|| {
                let candidate = self.replay_nature_candidate(
                    path, &self.nature.session, self.placement.detached_store()?,
                )?;
                let saved = against_rome_placement::ObjectStore::load(path)?;
                Ok::<_, String>((candidate, saved))
            })();
            match pending {
                Ok((candidate, saved)) => {
                    let baseline = crate::design_check_ui::data_candidate_issues(&saved, &snapshot, &mut footprint_resolver);
                    for mut issue in crate::design_check_ui::data_candidate_issues(&candidate, &snapshot, &mut footprint_resolver) {
                        if !baseline.contains(&issue) {
                            issue.chinese.push_str("（未儲存）");
                            issue.english.push_str(" (unsaved)");
                        }
                        if !map_issues.contains(&issue) {
                            map_issues.push(issue);
                        }
                    }
                }
                Err(error) => map_issues.push(against_rome_maps::diagnostics::MapIssue::new(
                    against_rome_maps::diagnostics::MapIssueSeverity::Warning,
                    "pending-data-unchecked",
                    format!("未儲存的物件／自然地景變更無法檢查：{error}"),
                    format!("Unsaved Objects/Nature edits could not be checked: {error}"),
                )),
            }
        }
        let design = ScenarioSavePreflight::validate(&document, &aliases)
            .map_err(|e| format!("ScenarioSavePreflight: {e}"))
            .map(|_| {
                MapDesignValidator::validate(&MapDesignValidationRequest {
                    objects: document.spawns,
                    events: document.events,
                    collision: snapshot.collision,
                    collision_size: snapshot.collision_size,
                    heights: snapshot.heights,
                    height_size: snapshot.height_size,
                    height_step: snapshot.height_step,
                    water_level: snapshot.water_level,
                    ..Default::default()
                })
            });
        Ok(CandidateDiagnostics { map_issues, design })
    }

    fn save_transaction(
        &mut self,
        confirm_design: impl FnOnce(&[against_rome_maps::design_validator::MapDesignIssue]) -> bool,
        commit: impl FnOnce(
            &mut against_rome_maps::MapSaveTransaction,
        ) -> Result<against_rome_maps::MapSaveReport, String>,
    ) -> Result<against_rome_maps::MapSaveReport, String> {
        use against_rome_ambience::soundscape::{SoundscapeBinaryStorage, SoundscapeSnapshot};
        // A brush stroke still in flight must be part of the candidate, never half-recorded.
        self.nature_end_stroke();
        let blockers = self.save_blockers();
        if !blockers.is_empty() {
            self.save_blockers_open = true;
            return Err(format!(
                "unsupported dirty panel state / save blockers: {}",
                blockers.iter().map(|b| b.english.as_str()).collect::<Vec<_>>().join("; ")
            ));
        }
        let session = self.session.as_ref().ok_or("No terrain session to save")?;
        let path = PathBuf::from(self.map_path_input.trim());
        if path.as_os_str().is_empty() {
            return Err("Please specify target map directory path before saving".into());
        }
        let (height, collision) = crate::storage::encode_terrain(session)?;
        let mut tx = against_rome_maps::MapSaveTransaction::new(&path).with_create_if_missing(true);
        tx.stage_file("boden.bmp", height.encode_bmp())?;
        tx.stage_file("collision.bmp", collision.encode_bmp())?;
        // C# parity: emboss.bmp is rebuilt only when heights changed and an original layer exists.
        let staged_emboss = match &self.emboss {
            Some(emboss) if emboss.session.is_dirty(session.heights()) => emboss
                .session
                .build_emboss(session.heights())
                .map_err(|e| format!("emboss.bmp relight: {e}"))?
                .map(|green| {
                    let mut image = emboss.raw.clone();
                    image.update_green_channel(&green);
                    (image, green)
                }),
            _ => None,
        };
        if let Some((image, _)) = &staged_emboss {
            tx.stage_file("emboss.bmp", image.encode_bmp())?;
        }
        // C# `_resetAuxiliaryLayers`: Flat Terrain resets vertex colours and the smoothing mask.
        let aux_reset = self.reset_auxiliary_layers;
        if aux_reset {
            for (name, value) in [("vertex.bmp", 255u8), ("smooth.bmp", 0u8)] {
                let file = path.join(name);
                if !file.is_file() {
                    continue;
                }
                let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
                let mut image = crate::bmp::BmpImage::parse(&bytes)
                    .map_err(|e| format!("Corrupted {}: {e}", file.display()))?;
                fill_layer_value(&mut image, value);
                tx.stage_file(name, image.encode_bmp())?;
            }
        }
        if session.is_dirty() {
            tx.stage_height_cache_invalidation()?;
        }
        let ambience_dirty = self.ambience.is_dirty();
        if ambience_dirty {
            tx.stage_file("boden.ini", self.ambience.stage_boden_ini_bytes(&path)?)?;
            let snapshot = SoundscapeSnapshot {
                global_ambience_id: self.ambience.global_sound_id.clone(),
                global_ambience_volume: self.ambience.global_volume,
                zones: self.ambience.zones.clone(),
            };
            tx.stage_file(
                "soundscape.sndz",
                SoundscapeBinaryStorage::serialize_to_binary(&snapshot)
                    .map_err(|e| format!("Ambience soundscape.sndz serialization: {e}"))?,
            )?;
            // Native location TEXT/US/briefing.put; None means no briefing field changed.
            if let Some(bytes) = self.ambience.stage_briefing_bytes(&path)? {
                tx.stage_file("TEXT/US/briefing.put", bytes)?;
            }
        }
        let placement_dirty = self.placement.is_dirty();
        let nature_dirty = self.nature.session.is_dirty();
        let staged_placement = if placement_dirty && !nature_dirty {
            self.placement.stage_transaction_files(&path)?
        } else {
            Vec::new()
        };
        for (rel_path, bytes) in &staged_placement {
            tx.stage_file(rel_path, bytes.clone())?;
        }
        let staged_nature = if nature_dirty {
            self.stage_nature_files(&path)?
        } else {
            Vec::new()
        };
        for (rel_path, bytes) in &staged_nature {
            tx.stage_file(rel_path, bytes.clone())?;
        }
        let scenario_dirty = self.scenario.dirty() || self.scenario.graph_ui().is_dirty();
        // Save-time DATA transaction of the spawn panel (C# prebuilt block): new buildings become
        // DATA objects, bound spawns move/rebuild, duplicates copy their family. Detached: it only
        // yields bytes to stage, so a later failure leaves every file untouched.
        let spawn_plan = if scenario_dirty {
            self.plan_spawn_data_for_save(&path)?
        } else {
            None
        };
        let spawn_data_staged = spawn_plan.as_ref().is_some_and(|p| !p.files.is_empty());
        let blockers = self.save_data_blockers(&path, spawn_data_staged);
        if !blockers.is_empty() {
            self.save_blockers_open = true;
            return Err(blockers.iter().map(|b| b.english.as_str()).collect::<Vec<_>>().join("; "));
        }
        for (rel_path, bytes) in spawn_plan.iter().flat_map(|p| p.files.iter()) {
            tx.stage_file(rel_path, bytes.clone())?;
        }
        let staged_scenario = if scenario_dirty {
            self.scenario.stage_transaction_files_with(
                &path,
                spawn_plan.as_ref().map(|p| (&p.scenario, p.active.as_slice())),
            )?
        } else {
            Vec::new()
        };
        let camera_pending = self.campaign.camera_pending();
        let staged_scenario = self.campaign.stage_camera_candidate(
            &path,
            &staged_scenario,
            self.scenario.known_aliases(),
        )?;
        for (rel_path, bytes) in &staged_scenario {
            tx.stage_file(rel_path, bytes.clone())?;
        }
        let textures_dirty = self.textures.is_dirty();
        let staged_textures = if textures_dirty {
            self.textures.stage_transaction_files(&path)?
        } else {
            Vec::new()
        };
        for (rel_path, bytes) in &staged_textures {
            tx.stage_file(rel_path, bytes.clone())?;
        }
        let scene_dirty = self.scene.is_dirty();
        for (rel_path, bytes) in self.scene.stage_transaction_files(&path)? {
            tx.stage_file(rel_path, bytes)?;
        }
        // Staging above is memory-only. All diagnostics/preflight/confirmation gates must
        // finish before backups, cache deletion, directory creation or commit can write.
        let design_issues = self.save_diagnostics(&path, &staged_scenario)?;
        if !design_issues.is_empty() && !confirm_design(&design_issues) {
            return Err("Design warning confirmation declined".into());
        }
        // Native cleanup scans the map recursively. Reject pre-existing recovery files and
        // links throughout that tree so a failed save cannot delete unrelated evidence.
        let staged_marker = tx.has_staged(against_rome_maps::CustomMapManifest::MARKER_FILE_NAME)
            || tx.staged_entries().iter().any(|e| {
                e.relative_path()
                    == Path::new(against_rome_maps::CustomMapManifest::MARKER_FILE_NAME)
                    && !e.is_deletion()
            });
        crate::storage::preflight_transaction_tree_opt(&path, staged_marker)?;
        let entries = tx.staged_entries().to_vec();
        for entry in &entries {
            let target = path.join(entry.relative_path());
            crate::storage::preflight_transaction_target_opt(&target, staged_marker)?;
            if target.is_file() {
                let original =
                    std::fs::read(&target).map_err(|e| format!("{}: {e}", target.display()))?;
                // Backups themselves participate in rollback; never overwrite old backups.
                let mut backup = entry.relative_path().as_os_str().to_os_string();
                backup.push(format!(".arm_backup.{}", uuid::Uuid::new_v4().simple()));
                let backup = PathBuf::from(backup);
                if path.join(&backup).try_exists().map_err(|e| e.to_string())? {
                    return Err(format!("Backup already exists: {}", backup.display()));
                }
                tx.stage_file(backup, original)?;
            }
        }
        tx.validate().map_err(|e| e.to_string())?;
        let report = commit(&mut tx)?;
        // Publish in-memory baselines only after *all* disk artifacts committed.
        let session = self.session.as_mut().expect("session validated above");
        session.raw_boden_bmp = Some(height);
        session.raw_collision_bmp = Some(collision);
        if let (Some(emboss), Some((image, green))) = (self.emboss.as_mut(), staged_emboss) {
            emboss
                .session
                .commit_baseline(session.heights(), Some(&green))
                .map_err(|e| format!("emboss baseline: {e}"))?;
            emboss.raw = image;
        }
        session.commit_baseline();
        if aux_reset {
            self.reset_auxiliary_layers = false;
        }
        if ambience_dirty {
            self.ambience.clear_dirty();
            self.ambience.status_message = "Ambience saved in whole-map transaction.".into();
            self.ambience.status_is_error = false;
        }
        if placement_dirty && !nature_dirty {
            self.placement.commit_baseline(&path)?;
        }
        if nature_dirty {
            // The committed objects.dat is the new baseline for both Nature and the DATA panel.
            self.nature.accept_saved();
            if self.placement.is_loaded() {
                self.placement.commit_baseline(&path)?;
            }
        }
        self.settlement_undo.clear();
        self.settlement_redo.clear();
        if spawn_data_staged {
            // The committed DATA is the new baseline for the Objects panel and the clearance cache.
            if self.placement.is_loaded() {
                self.placement.commit_baseline(&path)?;
            }
            self.spawn_landscape = None;
        }
        if camera_pending {
            self.scenario.load_session(&path)?;
            self.campaign.camera_saved();
        } else if scenario_dirty {
            self.scenario
                .commit_baseline_with(&path, spawn_plan.as_ref().map(|p| &p.scenario))?;
        }
        if textures_dirty {
            self.textures.commit_baseline(&path)?;
        }
        if scene_dirty {
            self.scene.commit_baseline(&path)?;
        }
        self.clear_recovery();
        self.reset_journal_epoch();
        if let Ok(source) = crate::recovery::fingerprint(&path) { self.recovery.source = source; }
        self.recovery_deferred = false;
        Ok(report)
    }

    /// Returns a reference to any pending action awaiting clean-state or discard resolution.
    pub fn pending_action(&self) -> Option<&PendingAction> {
        self.pending_action.as_ref()
    }

    pub(crate) fn require_clean_map_switch(&self) -> Result<(), String> {
        if self.has_unsaved_changes() {
            return Err("Map switch rejected: unsaved changes; save first or use the UI discard confirmation.".into());
        }
        Ok(())
    }

    /// Install a loaded map only when every current map session is clean.
    /// Rejection leaves the app unchanged; explicit discard is handled by the UI.
    pub fn set_loaded_map(&mut self, path: PathBuf, session: TerrainSession) -> Result<(), String> {
        self.require_clean_map_switch()?;
        self.install_loaded_map(path, session)?;
        Ok(())
    }

    /// Prevalidates map files transactionally before modifying any editor panel state.
    fn prevalidate_map_load(path: &Path) -> Result<(), String> {
        if !path.is_dir() {
            return Err(format!(
                "Target map path is not a directory: {}",
                path.display()
            ));
        }
        // Validate Day/Night BMP if present
        let daynight_path = path.join("daynight.bmp");
        if daynight_path.is_file() {
            let bytes = std::fs::read(&daynight_path)
                .map_err(|e| format!("Failed to read {}: {e}", daynight_path.display()))?;
            against_rome_scene::lighting::MapLightingDayNight::parse(&bytes)
                .map_err(|e| format!("Corrupted {}: {e}", daynight_path.display()))?;
        }
        // Validate Soundscape SNDZ if present
        let sndz_path = path.join("soundscape.sndz");
        if sndz_path.is_file() {
            let bytes = std::fs::read(&sndz_path)
                .map_err(|e| format!("Failed to read {}: {e}", sndz_path.display()))?;
            against_rome_ambience::soundscape::SoundscapeBinaryStorage::deserialize_from_binary(
                &bytes,
            )
            .map_err(|e| format!("Corrupted {}: {e}", sndz_path.display()))?;
        }
        // Validate Briefing PUT scripts if present in SCRIPT/
        let script_dir = path.join("SCRIPT");
        if script_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&script_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("briefing") && name.ends_with(".put") {
                        let text = std::fs::read_to_string(entry.path()).map_err(|e| {
                            format!("Failed to read {}: {e}", entry.path().display())
                        })?;
                        against_rome_ambience::localization::BriefingTextTable::parse(&text)
                            .map_err(|e| format!("Corrupted {}: {e}", entry.path().display()))?;
                    }
                }
            }
        }
        // Validate Materials (boden.txt) if present
        let boden_txt = path.join("boden.txt");
        if boden_txt.is_file() {
            let bytes = std::fs::read(&boden_txt)
                .map_err(|e| format!("Failed to read {}: {e}", boden_txt.display()))?;
            against_rome_textures::BodenTexturesDocument::from_bytes(&bytes)
                .map_err(|e| format!("Corrupted {}: {e}", boden_txt.display()))?;
        }
        // Validate Scenario (SCRIPT/ak_level.bci or arm_original) if present
        let bci = script_dir.join("ak_level.bci");
        let arm_orig = script_dir.join("ak_level.arm_original");
        let script_path = if arm_orig.is_file() {
            Some(arm_orig)
        } else if bci.is_file() {
            Some(bci)
        } else {
            None
        };
        if let Some(sp) = script_path {
            let bytes =
                std::fs::read(&sp).map_err(|e| format!("Failed to read {}: {e}", sp.display()))?;
            let raw = if bytes.starts_with(b"PFIL") {
                against_rome_data::lzss::decompress_pfil(&bytes, None)
                    .map_err(|e| format!("Decompression failed for {}: {e}", sp.display()))?
            } else {
                bytes
            };
            against_rome_scenario::BciImage::parse(&raw)
                .map_err(|e| format!("Corrupted script {}: {e}", sp.display()))?;
        }
        Ok(())
    }

    /// Reads the optional original `emboss.bmp` as the relight baseline (C# `_embossLayer`).
    fn load_emboss_baseline(
        path: &Path,
        terrain: &TerrainSession,
    ) -> Result<Option<EmbossBaseline>, String> {
        let file = path.join("emboss.bmp");
        if !file.is_file() {
            return Ok(None);
        }
        let bytes =
            std::fs::read(&file).map_err(|e| format!("Failed to read {}: {e}", file.display()))?;
        let raw = crate::bmp::BmpImage::parse(&bytes)
            .map_err(|e| format!("Corrupted {}: {e}", file.display()))?;
        if raw.width != terrain.vertex_size || raw.height != terrain.vertex_size {
            return Err(format!(
                "{} size {}x{} does not match boden.bmp {}",
                file.display(),
                raw.width,
                raw.height,
                terrain.vertex_size
            ));
        }
        let session = against_rome_editor::emboss::EmbossSession::new(
            terrain.vertex_size,
            terrain.original_heights().to_vec(),
            Some(raw.extract_green_channel()),
        )
        .map_err(|e| format!("{}: {e}", file.display()))?;
        Ok(Some(EmbossBaseline { raw, session }))
    }

    // Only called after a clean-state check or the UI's explicit discard confirmation.
    fn install_loaded_map(&mut self, path: PathBuf, session: TerrainSession) -> Result<(), String> {
        Self::prevalidate_map_load(&path)?;
        let emboss = Self::load_emboss_baseline(&path, &session)?;

        // Keep map-bound documents unloaded until their panels explicitly load them.
        // Retail DATA pools and editor ObjectStore documents are different formats; map
        // opening must not reinterpret native game pools as editable placement data.
        // Ambience loading is the last fallible document load, so perform it before resetting
        // the other panels. AmbienceUi stages its parse and leaves itself untouched on failure.
        let mut loaded_camera = CampaignUi::default();
        loaded_camera.load_map_camera(&path)?;
        self.ambience.load_from_map(&path)?;
        self.reset_journal_epoch();
        self.save_blockers_open = false;
        self.save_focus_nature = false;
        self.pending_save_warnings = None;
        self.placement.reset_session(Some(&path));
        self.family.reset_session();
        self.nature.reset_map_bound();
        self.settlement_undo.clear();
        self.settlement_redo.clear();
        self.textures.reset_session(Some(&path));
        self.scene.reset_session(Some(&path));
        self.scenario.reset_session(Some(&path));

        // Install terrain session and settings
        self.session = Some(session);
        self.emboss = emboss;
        self.reset_auxiliary_layers = false;
        self.terrain_confirm = None;
        self.loaded_map_path = Some(path.clone());
        self.recovery.open(&path);
        self.recovery_deferred = false;
        self.map_path_input = path.to_string_lossy().into_owned();
        self.status_is_error = false;
        self.status_message = format!("Loaded {}", path.display());
        let settings = match MapSettings::load(&path) {
            Ok(s) => s,
            Err(e) => {
                self.status_is_error = true;
                self.status_message = format!("Map loaded; water settings disabled: {e}");
                MapSettings::default()
            }
        };
        self.height_step = settings.height_step;
        self.water_surface = settings.water_surface();
        self.water_bed = settings.suggested_bed();

        // Rebuild / reset derived panels in dependency order
        self.event_graph.reset_session();
        self.toolbar_graph_source = None;
        self.maps.reset_session(Some(&path));
        self.environment.reset_session();
        self.landscape.reset_session();
        self.packaging.reset_session(Some(&path));
        self.campaign.reset_session();
        self.campaign.start_camera = loaded_camera.start_camera;
        self.ai_planner.reset_session();
        self.toolbox.clear_transient_reviews();

        self.navigation.invalidate();
        self.texture_dirty = true;
        self.is_painting = false;
        self.stroke_path.clear();
        self.ramp_selecting = false;
        self.ramp_start = None;
        if let Err(e) = self.recent_maps.remember(&path) {
            self.status_message = format!("Map loaded; recent maps not saved / 地圖已載入；最近地圖未儲存：{e}");
            self.status_is_error = true;
        }
        Ok(())
    }

    fn install_blank_map(&mut self, size: usize) {
        self.reset_journal_epoch();
        self.recovery.offer = None;
        self.recovery.last = None;
        self.save_blockers_open = false;
        self.save_focus_nature = false;
        self.recovery.source = serde_json::Value::Null;
        self.recovery_deferred = false;
        self.pending_save_warnings = None;
        self.loaded_map_path = None;
        self.session = Some(TerrainSession::create_blank(size, 32));
        self.emboss = None;
        self.reset_auxiliary_layers = false;
        self.terrain_confirm = None;
        self.status_message = format!("Created blank {size}x{size} terrain");
        self.status_is_error = false;
        self.map_path_input.clear();
        self.height_step = 4.0;
        self.water_surface = 0.0;
        self.water_bed = 0;

        // Reset base documents
        self.placement.reset_session(None);
        self.family.reset_session();
        self.nature.reset_map_bound();
        self.settlement_undo.clear();
        self.settlement_redo.clear();
        self.textures.reset_session(None);
        self.scene.reset_session(None);
        self.scenario.reset_session(None);
        self.ambience.reset_session();

        // Rebuild / reset derived panels
        self.event_graph.reset_session();
        self.toolbar_graph_source = None;
        self.maps.reset_session(None);
        self.environment.reset_session();
        self.landscape.reset_session();
        self.packaging.reset_session(None);
        self.campaign.reset_session();
        self.ai_planner.reset_session();
        self.toolbox.clear_transient_reviews();

        self.navigation.invalidate();
        self.texture_dirty = true;
        self.is_painting = false;
        self.stroke_path.clear();
        self.ramp_selecting = false;
        self.ramp_start = None;
    }

    pub(crate) fn build_and_install_blank_map(
        &mut self,
        game_path: &Path,
        spec: &against_rome_maps::blank_builder::BlankMapSpec,
    ) -> Result<against_rome_maps::blank_builder::BlankMapBuildResult, String> {
        let result =
            against_rome_maps::blank_builder::BlankMapBuilder::build_with_spec(game_path, spec)?;
        result.verify_disk_artifacts(game_path)?;
        let loaded = load_map(&result.map_path)?;
        self.install_loaded_map(result.map_path.clone(), loaded)?;
        self.maps.refresh_catalog();
        self.maps.blank_last_result = Some(result.clone());
        self.maps.status_message = format!(
            "Blank map ENDL_{:03} created and verified successfully!",
            spec.target_slot
        );
        self.maps.status_is_error = false;
        Ok(result)
    }

    /// Build and open a blank map. Reject dirty sessions before building or changing any files.
    pub fn create_blank_map(
        &mut self,
        game_path: &Path,
        spec: &against_rome_maps::blank_builder::BlankMapSpec,
    ) -> Result<against_rome_maps::blank_builder::BlankMapBuildResult, String> {
        self.require_clean_map_switch()?;
        self.build_and_install_blank_map(game_path, spec)
    }

    /// Execute a blank map build intent. Reject dirty sessions before building or changing any files.
    pub fn execute_build_intent(
        &mut self,
        intent: &BlankMapBuildIntent,
    ) -> Result<against_rome_maps::blank_builder::BlankMapBuildResult, String> {
        if intent.flat_template {
            return Err("The Flat Template intent opens a cloned template; queue it as PendingAction::BuildBlank".into());
        }
        self.create_blank_map(&intent.game_path, &intent.spec)
    }

    /// C# "Flat Template" (`MapSelectionForm.CreateBlankMap` + `_startWithBlankTerrain`): clones the
    /// template unchanged (settlements, scripts and DATA kept), opens the copy and immediately
    /// flattens its terrain without a confirm dialog. Nothing is saved until the designer saves.
    pub(crate) fn build_and_install_flat_template(
        &mut self,
        game_path: &Path,
        spec: &against_rome_maps::blank_builder::BlankMapSpec,
    ) -> Result<(against_rome_maps::catalog::EndlessMapInfo, Option<String>), String> {
        let source_slot = spec
            .source_map_id
            .to_ascii_uppercase()
            .strip_prefix("ENDL_")
            .and_then(|slot| slot.parse::<i32>().ok())
            .ok_or_else(|| format!("Flat Template source must be an ENDL_### map: {}", spec.source_map_id))?;
        let info = against_rome_maps::cloner::EndlessMapCloner::new().clone_map(
            game_path,
            source_slot,
            spec.target_slot,
            &spec.target_name,
        )?;
        let mut briefing_warning: Option<String> = None;
        if let Some(briefing_text) = &spec.briefing_text {
            let briefing = info.directory_path.join("TEXT").join("US").join("briefing.put");
            let write_res: Result<(), String> = if !briefing.is_file() {
                Err(format!("briefing.put not found or is a directory: {}", briefing.display()))
            } else {
                against_rome_maps::text_docs::PutTextDocument::load(&briefing)
                    .and_then(|mut put| {
                        put.set_value("briefing_titel_1", briefing_text)?;
                        put.save()
                    })
            };
            if let Err(err) = write_res {
                briefing_warning = Some(format!(
                    "Warning: Failed to update briefing file ({err}); briefing title kept in memory (unsaved) / 警告：簡報檔案寫入失敗（{err}）；簡報標題暫存於記憶體（未儲存）"
                ));
            }
        }
        let loaded = load_map(&info.directory_path)?;
        self.install_loaded_map(info.directory_path.clone(), loaded)?;
        if let Some(briefing_text) = &spec.briefing_text {
            if briefing_warning.is_some() {
                self.ambience.briefing_title = briefing_text.clone();
                self.ambience.dirty = true;
            } else {
                self.ambience.dirty = false;
            }
        }
        self.maps.refresh_catalog();
        if !self.apply_flat_terrain_now() {
            return Err(format!(
                "Template {} was created but could not be flattened: {}",
                info.id(),
                self.status_message
            ));
        }
        if let Some(warning) = &briefing_warning {
            self.status_message = warning.clone();
            self.status_is_error = false;
            self.maps.status_message = format!(
                "Flat template {} created; terrain flattened (unsaved). {warning}",
                info.id()
            );
            self.maps.status_is_error = false;
        } else {
            self.maps.status_message = format!(
                "Flat template {} created; terrain flattened (unsaved). Shape it, then Save.",
                info.id()
            );
            self.maps.status_is_error = false;
        }
        Ok((info, briefing_warning))
    }

    /// Queue a typed blank map build intent into the pending action pipeline.
    pub fn queue_build_intent(&mut self, intent: BlankMapBuildIntent) {
        self.pending_action = Some(PendingAction::BuildBlank(intent));
    }

    fn perform_action(&mut self, action: PendingAction, ctx: &egui::Context) {
        match action {
            PendingAction::Load(path) => match load_map(&path) {
                Ok(loaded) => {
                    if let Err(error) = self.install_loaded_map(path, loaded) {
                        self.status_message = format!("Load failed: {error}");
                        self.status_is_error = true;
                        return;
                    }
                }
                Err(error) => {
                    self.status_message = format!("Load failed: {error}");
                    self.status_is_error = true;
                    return;
                }
            },
            PendingAction::Blank(size) => {
                self.install_blank_map(size);
            }
            PendingAction::BuildBlank(intent) if intent.flat_template => {
                match self.build_and_install_flat_template(&intent.game_path, &intent.spec) {
                    Ok((info, briefing_warning)) => {
                        if let Some(warning) = briefing_warning {
                            self.status_message = warning;
                        } else {
                            self.status_message = format!(
                                "Flat template {} created and opened; terrain flattened (unsaved).",
                                info.id()
                            );
                        }
                        self.status_is_error = false;
                    }
                    Err(error) => {
                        self.status_message = format!("Flat template failed: {error}");
                        self.status_is_error = true;
                        self.maps.status_message = format!("平坦範本建置失敗: {error}");
                        self.maps.status_is_error = true;
                        return;
                    }
                }
            }
            PendingAction::BuildBlank(intent) => {
                match self.build_and_install_blank_map(&intent.game_path, &intent.spec) {
                    Ok(result) => {
                        self.status_message =
                            format!("Built and loaded blank map {}", result.map_path.display());
                        self.status_is_error = false;
                    }
                    Err(error) => {
                        self.status_message = format!("Build failed: {error}");
                        self.status_is_error = true;
                        self.maps.status_message = format!("建置失敗: {error}");
                        self.maps.status_is_error = true;
                        return;
                    }
                }
            }
            PendingAction::Close => {
                self.reset_journal_epoch();
                self.close_allowed = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }
        self.is_painting = false;
        self.stroke_path.clear();
        self.ramp_selecting = false;
        self.ramp_start = None;
        self.texture_dirty = true;
    }

    pub fn handle_pending_action(&mut self, ctx: &egui::Context) {
        if self.pending_action.is_none() {
            return;
        }
        if !self.has_unsaved_changes() {
            let action = self.pending_action.take().unwrap();
            self.perform_action(action, ctx);
            return;
        }
        let mut discard = false;
        let mut cancel = false;
        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("Discard unsaved changes? Closing also discards pending DATA and scenario edits.");
                ui.horizontal(|ui| {
                    discard = ui.button("Discard unsaved changes").clicked();
                    cancel = ui.button("Keep editing").clicked();
                });
            });
        if discard {
            self.reset_journal_epoch();
            self.clear_recovery();
            self.recovery_deferred = false;
            let action = self.pending_action.take().unwrap();
            self.perform_action(action, ctx);
        } else if cancel {
            self.pending_action = None;
        }
    }

    /// Merges campaign AI waves into the scenario via its event session (undoable, capped, fail-closed).
    fn apply_campaign_wave_merge(&mut self) {
        if self.campaign.take_wave_merge_request() {
            let _ = self.campaign.merge_waves_into_scenario(&mut self.scenario);
        }
    }

    fn apply_campaign_objective_request(&mut self) {
        if self.campaign.take_objective_apply_request() {
            let _ = self
                .campaign
                .apply_objectives_to_scenario(&mut self.scenario);
        }
    }

    /// Merges pending Neutral Lair events into the scenario on an explicit Apply Lair Events request.
    fn apply_lair_events_request(&mut self) {
        if self.environment.take_lair_events_apply_request() {
            let _ = self.environment.apply_lair_events(&mut self.scenario);
        }
    }

    /// Designer panels backed by the native maps/environment/landscape/packaging/campaign/ai-planner/ambience crates.
    pub fn show_studio_panels(&mut self, ctx: &egui::Context) {
        let dim = self.session.as_ref().map_or(0, |s| s.vertex_size);
        let water = self.water_surface;
        let mut load_req: Option<PathBuf> = None;
        {
            let cx = MapsContext {
                scenario: Some(self.scenario.document()),
                aliases: self.scenario.known_aliases(),
                heights: self.session.as_ref().map(|s| s.heights()),
                height_size: dim,
                collision: self.session.as_ref().map(|s| s.collision()),
                height_step: self.height_step,
                water_level: water,
            };
            let mut cb = |p: PathBuf| {
                load_req = Some(p);
            };
            self.maps.show(ctx, &cx, Some(&mut cb));
        }
        if let Some(p) = load_req {
            self.pending_action = Some(PendingAction::Load(p));
        }
        if let Some(intent) = self.maps.take_pending_build_intent() {
            self.pending_action = Some(PendingAction::BuildBlank(intent));
        }
        if self.maps.request_skirmish_wizard {
            self.maps.request_skirmish_wizard = false;
            self.open_skirmish_wizard();
        }
        self.environment.english = self.lang() == AppLanguage::English;
        self.environment.show(
            ctx,
            dim,
            self.session.as_ref().map(|s| s.heights()),
            self.session.as_ref().map(|s| (s.collision_size, s.collision())),
            water,
            self.height_step,
        );
        if let Some(apply) = self.environment.take_apply_request() {
            self.handle_environment_apply(apply);
        }
        self.apply_lair_events_request();
        self.pump_layout_requests();

        let textures = self.textures.session_textures();
        let texture_names = self.textures.known_textures();
        let lcx = LandscapeContext {
            vertex_size: dim,
            heights: self.session.as_ref().map(|s| s.heights()),
            textures,
            texture_names,
        };
        self.landscape.show(ctx, &lcx, Some(&mut self.ambience));
        if let Some(plan) = self.landscape.take_erosion_apply_request() {
            self.handle_erosion_apply(plan);
        }
        if let Some(apply) = self.landscape.take_apply_request() {
            self.handle_landscape_apply(apply);
        }
        self.apply_region_request();

        self.packaging.english = self.lang() == AppLanguage::English;
        self.packaging.show(
            ctx,
            dim,
            self.session.as_ref().map(|s| s.heights()),
            self.session.as_ref().map(|s| s.collision()),
            Some(&self.scenario.document().spawns),
            crate::packaging_ui::LiveDiffSessions::from_sessions(
                &self.textures,
                &self.placement,
                &self.scenario,
            ),
        );
        self.campaign.show(
            ctx,
            Some(self.scenario.document()),
            self.scenario.known_aliases(),
        );
        self.apply_campaign_objective_request();
        self.apply_campaign_wave_merge();
        self.apply_campaign_camera_requests();
        let map_path = if self.map_path_input.trim().is_empty() {
            None
        } else {
            Some(std::path::Path::new(self.map_path_input.trim()))
        };
        self.ambience.app_language = self.loc.current_language();
        self.ambience.show(ctx, map_path, water);
        if self.ambience.app_language != self.loc.current_language() {
            let chosen = self.ambience.app_language;
            self.set_app_language(chosen);
        }
        self.modifier.show(ctx);

        let had_session = self.session.is_some();
        let before = self.session.as_ref().is_some_and(|s| s.is_dirty());
        self.sync_ai_materials();
        let ai_applied = self.ai_planner.applied_revision;
        self.ai_planner
            .show(ctx, self.session.as_mut(), self.textures.session_mut(), water);
        if self.ai_planner.applied_revision != ai_applied {
            self.textures.note_external_change();
            self.texture_dirty = true;
        }
        self.show_family_window(ctx);
        self.show_toolbar_event_graph(ctx);
        self.show_skirmish_wizard(ctx);
        if had_session && self.session.as_ref().is_some_and(|s| s.is_dirty()) && !before {
            self.texture_dirty = true;
            self.navigation.invalidate();
        }
    }

    /// 首次開啟時以目前的劇本資料載入獨立事件圖。
    fn open_toolbar_event_graph(&mut self) {
        self.event_graph.open = true;
        if !self.scenario.is_loaded()
            && !self.event_graph.is_dirty()
            && !self.event_graph.apply_requested
            && !self.scenario.dirty()
            && !self.scenario.graph_ui().is_dirty()
            && !self.scenario.graph_ui().apply_requested
        {
            let result = self.loaded_map_path.as_deref()
                .ok_or_else(|| "EventGraph: no loaded map scenario".to_string())
                .and_then(|path| self.scenario.load_session(path));
            if let Err(error) = result {
                self.toolbar_graph_error(error);
                return;
            }
        }
        if !self.event_graph.is_dirty() && !self.event_graph.apply_requested {
            // 放棄後再次開啟時，即使劇本未變更也必須重新載入。
            self.toolbar_graph_source = None;
        }
        self.sync_toolbar_event_graph();
        if !self.scenario.is_loaded() {
            self.toolbar_graph_error("EventGraph: no loaded scenario; graph edits kept".into());
        }
    }

    fn toolbar_graph_source(&self) -> Result<Option<ToolbarGraphSource>, String> {
        if !self.scenario.is_loaded() {
            return Ok(None);
        }
        Ok(Some(ToolbarGraphSource {
            map: self.loaded_map_path.clone(),
            document: self.scenario.document().clone(),
            aliases: self.scenario.known_aliases().to_vec(),
            saved_document: self.scenario.saved_document()?,
        }))
    }

    /// 劇本重新載入時不得覆寫尚未套用的修改或排隊中的套用要求。
    fn sync_toolbar_event_graph(&mut self) {
        if self.event_graph.is_dirty() || self.event_graph.apply_requested {
            return;
        }
        let source = match self.toolbar_graph_source() {
            Ok(source) => source,
            Err(error) => {
                self.toolbar_graph_error(error);
                return;
            }
        };
        if source != self.toolbar_graph_source {
            if source.is_some() {
                self.scenario.sync_to_graph_ui(&mut self.event_graph);
            } else {
                self.event_graph.reset_session();
            }
            self.toolbar_graph_source = source;
        }
    }

    fn toolbar_graph_error(&mut self, error: String) {
        self.event_graph.status_message = error.clone();
        self.status_message = error;
        self.status_is_error = true;
    }

    fn apply_toolbar_event_graph_request(&mut self) {
        if !std::mem::take(&mut self.event_graph.apply_requested) {
            return;
        }
        let source = self.toolbar_graph_source();
        let result = match source {
            Err(error) => Err(error),
            Ok(None) => Err("EventGraph: no loaded scenario; graph edits kept".into()),
            Ok(Some(source)) if Some(&source) != self.toolbar_graph_source.as_ref() => {
                Err("EventGraph: scenario changed since graph sync; graph edits kept. Resolve the conflict or discard and reopen the graph.".into())
            }
            Ok(Some(_)) if self.toolbar_graph_source.is_none() => {
                Err("EventGraph: scenario changed since graph sync; graph edits kept. Resolve the conflict or discard and reopen the graph.".into())
            }
            Ok(Some(_)) => self.scenario.apply_from_graph_ui(&mut self.event_graph),
        };
        match result {
            Ok(()) => {
                // 劇本面板使用另一份事件圖；保留其中尚未套用的修改。
                if !self.scenario.graph_ui().is_dirty() && !self.scenario.graph_ui().apply_requested {
                    let _ = self.scenario.sync_to_graph();
                }
                match self.toolbar_graph_source() {
                    Ok(source) => self.toolbar_graph_source = source,
                    Err(error) => self.toolbar_graph_error(error),
                }
                self.event_graph.status_message = "Applied to Scenario; use Safe Save to write the map.".into();
                self.status_message = self.event_graph.status_message.clone();
                self.status_is_error = false;
            }
            Err(error) => self.toolbar_graph_error(error),
        }
    }

    fn show_toolbar_event_graph(&mut self, ctx: &egui::Context) {
        if self.event_graph.open {
            self.sync_toolbar_event_graph();
        }
        self.event_graph.show(ctx);
        self.apply_toolbar_event_graph_request();
    }

    /// Loads objdef names (cached per game root) so placement team edits use category limits.
    fn refresh_placement_type_names(&mut self) {
        if self.placement.has_type_names() || !self.placement.is_loaded() {
            return;
        }
        let Some(game) = self
            .placement
            .loaded_map()
            .and_then(|m| m.parent())
            .and_then(Path::parent)
            .map(Path::to_path_buf)
        else {
            return;
        };
        if let Ok(definitions) = against_rome_maps::NativeObjectDefinitions::load(&game) {
            let names: HashMap<i32, String> = definitions
                .into_iter()
                .filter_map(|(id, d)| d.name.filter(|n| !n.is_empty()).map(|n| (id, n)))
                .collect();
            self.placement.set_type_names(names);
        }
    }

    /// Families & Effects window (saved lights, particles, native family copy).
    fn show_family_window(&mut self, ctx: &egui::Context) {
        // Any placement edit (including undo/redo/load) invalidates the cached tables.
        let key = (
            self.placement.object_count(),
            self.placement.undo_depth(),
            self.placement.redo_depth(),
        );
        if key != self.family_history_key {
            self.family_history_key = key;
            self.family.invalidate();
        }
        if !self.family_open {
            return;
        }
        let english = self.lang() == AppLanguage::English;
        let map_dir = self.placement.loaded_map().map(Path::to_path_buf).or_else(|| {
            let t = self.map_path_input.trim();
            (!t.is_empty()).then(|| PathBuf::from(t))
        });
        let game_dir = map_dir.as_deref().and_then(crate::packaging_ui::game_dir_for_map);
        let can_edit = map_dir.as_deref().is_some_and(|d| !is_original_map_dir(d));
        let bound: BTreeSet<usize> = self
            .scenario
            .document()
            .data_slots
            .iter()
            .filter_map(|d| usize::try_from(d.slot).ok())
            .collect();
        let mut open = self.family_open;
        let family = &mut self.family;
        let placement = &mut self.placement;
        egui::Window::new(tr(self.loc.current_language(), "Families & Effects", "群組與特效"))
            .open(&mut open)
            .default_size(Vec2::new(640.0, 560.0))
            .show(ctx, |ui| {
                family.show(
                    ui,
                    crate::family_ui::FamilyCtx {
                        store: placement.store_mut(),
                        map_dir: map_dir.as_deref(),
                        game_dir: game_dir.as_deref(),
                        can_edit,
                        english,
                        bound_root_slots: Some(&bound),
                    },
                );
            });
        self.family_open = open;
        if self.family.take_store_changed() {
            self.placement.record_edit(1);
            self.navigation.invalidate();
        }
    }

    /// Applies a pending Region Tools request (texture fill or object selection).
    fn apply_region_request(&mut self) {
        let objects: Vec<(usize, f32, f32)> = self
            .placement
            .objects()
            .iter()
            .map(|o| (o.slot, o.position[0], o.position[2]))
            .collect();
        let outcome = self
            .landscape
            .apply_pending_region(self.textures.session_mut(), || objects);
        if outcome.textures_changed > 0 {
            self.texture_dirty = true;
            self.navigation.invalidate();
        }
        if let Some(ids) = outcome.selected_objects {
            let keys: Vec<(usize, u32)> = self
                .placement
                .objects()
                .iter()
                .filter(|o| ids.contains(&o.slot))
                .map(|o| (o.slot, o.uid))
                .collect();
            self.placement.select_objects(&keys);
        }
    }

    /// 2D canvas hooks for Region Tools: drag a rectangle and paint the selection overlay.
    fn handle_region_canvas(&mut self, response: &egui::Response, painter: &egui::Painter, map_rect: Rect) {
        let dim = self.nature_dimension();
        let to_screen = |x: f32, y: f32| {
            Pos2::new(
                map_rect.min.x + x / dim as f32 * map_rect.width(),
                map_rect.min.y + y / dim as f32 * map_rect.height(),
            )
        };
        if self.landscape.region_pick_active() {
            let tile_at = |p: Pos2| {
                Some((
                    ((p.x - map_rect.min.x) / map_rect.width() * dim as f32).floor() as i32,
                    ((p.y - map_rect.min.y) / map_rect.height() * dim as f32).floor() as i32,
                ))
            };
            if self.landscape.region_drag_input(response, dim.max(1) as usize, tile_at).is_some() {
                self.status_message = "Region selection updated from canvas.".into();
                self.status_is_error = false;
            }
        }
        self.landscape.paint_region_overlay(painter, to_screen);
    }

    fn handle_landscape_apply(&mut self, apply: LandscapeApply) {
        match apply {
            LandscapeApply::Cliff => {
                let Some(detection) = self.landscape.last_cliff_result.clone() else {
                    self.landscape
                        .apply_failed("No cliff detection result available.");
                    return;
                };
                let Some(terrain) = self.session.as_mut() else {
                    self.landscape.apply_failed("No terrain session loaded.");
                    return;
                };
                let known_names = self.textures.known_textures().to_vec();
                let known = crate::design_apply::TextureNames::new(&known_names);
                let options = against_rome_landscape::cliff::CliffPlannerOptions {
                    generate_scree: self.landscape.cliff_generate_scree,
                    ..Default::default()
                };
                let Some(textures_session) = self.textures.session_mut() else {
                    self.landscape.apply_failed(
                        "Materials session (boden.txt) must be loaded to apply cliffs.",
                    );
                    return;
                };
                match crate::design_apply::apply_cliffs(
                    terrain,
                    textures_session,
                    &known,
                    &known_names,
                    &detection,
                    &options,
                ) {
                    Ok(report) => {
                        self.texture_dirty = true;
                        self.navigation.invalidate();
                        self.landscape
                            .plan_applied(LandscapeApply::Cliff, &report.summary());
                    }
                    Err(e) => self.landscape.apply_failed(e),
                }
            }
            LandscapeApply::River => {
                let Some(plan) = self.landscape.last_river_plan.clone() else {
                    self.landscape.apply_failed("No river plan available.");
                    return;
                };
                let Some(terrain) = self.session.as_mut() else {
                    self.landscape.apply_failed("No terrain session loaded.");
                    return;
                };
                let known_names = self.textures.known_textures().to_vec();
                let known = crate::design_apply::TextureNames::new(&known_names);
                let Some(textures_session) = self.textures.session_mut() else {
                    self.landscape.apply_failed(
                        "Materials session (boden.txt) must be loaded to apply river.",
                    );
                    return;
                };
                match crate::design_apply::apply_river(terrain, textures_session, &known, &plan) {
                    Ok(report) => {
                        self.texture_dirty = true;
                        self.navigation.invalidate();
                        self.landscape
                            .plan_applied(LandscapeApply::River, &report.summary());
                    }
                    Err(e) => self.landscape.apply_failed(e),
                }
            }
            LandscapeApply::Road => {
                let Some(plan) = self.landscape.last_road_plan.clone() else {
                    self.landscape.apply_failed("No road plan available.");
                    return;
                };
                let known_names = self.textures.known_textures().to_vec();
                let known = crate::design_apply::TextureNames::new(&known_names);
                let Some(textures_session) = self.textures.session_mut() else {
                    self.landscape.apply_failed(
                        "Materials session (boden.txt) must be loaded to apply road.",
                    );
                    return;
                };
                match crate::design_apply::apply_road(textures_session, &known, &plan) {
                    Ok(report) => {
                        self.texture_dirty = true;
                        self.landscape
                            .plan_applied(LandscapeApply::Road, &report.summary());
                    }
                    Err(e) => self.landscape.apply_failed(e),
                }
            }
            LandscapeApply::Wall => {
                let Some(plan) = self.landscape.last_wall_plan.clone() else {
                    self.landscape.apply_failed("No wall plan available.");
                    return;
                };
                let Some(terrain) = self.session.as_mut() else {
                    self.landscape.apply_failed("No terrain session loaded.");
                    return;
                };
                let templates =
                    match crate::design_apply::parse_templates(&self.landscape.wall_templates_text)
                    {
                        Ok(t) => t,
                        Err(e) => {
                            self.landscape
                                .apply_failed(format!("Invalid template definitions: {e}"));
                            return;
                        }
                    };
                let resolve = crate::design_apply::resolver(&templates);
                let (tile_dim, _) =
                    crate::landscape_ui::LandscapeContext::plain(terrain.vertex_size, None)
                        .geometry();
                let edits = match crate::design_apply::plan_wall_edits(
                    terrain, tile_dim, &plan, &resolve,
                ) {
                    Ok(edits) => edits,
                    Err(e) => {
                        self.landscape.apply_failed(e);
                        return;
                    }
                };
                let blocked = crate::design_apply::commit_wall_collision(terrain, &edits);
                let mut foundations = 0;
                if !edits.foundations.is_empty() {
                    let known_names = self.textures.known_textures().to_vec();
                    let known = crate::design_apply::TextureNames::new(&known_names);
                    if let Some(textures_session) = self.textures.session_mut() {
                        foundations = crate::design_apply::commit_wall_foundations(
                            textures_session,
                            &known,
                            &edits,
                        );
                    }
                }
                let mut placed_count = 0;
                let mut placement_note = String::new();
                if !edits.unresolved.is_empty() {
                    let list: Vec<_> = edits.unresolved.iter().cloned().collect();
                    placement_note
                        .push_str(&format!(" (unresolved templates: {})", list.join(", ")));
                }
                if !edits.requests.is_empty() {
                    if self.placement.is_loaded() {
                        match self.placement.apply_requests(&edits.requests) {
                            Ok(batch) => {
                                placed_count = batch.placed;
                                if !batch.skipped.is_empty() {
                                    placement_note.push_str(&format!(
                                        " (skipped: {})",
                                        batch.skipped.join("; ")
                                    ));
                                }
                            }
                            Err(e) => {
                                placement_note
                                    .push_str(&format!(" (object placement failed: {e})"));
                            }
                        }
                    } else {
                        placement_note
                            .push_str(" (objects skipped: load DATA in Objects panel first)");
                    }
                } else if !plan.placements.is_empty() && !self.placement.is_loaded() {
                    placement_note.push_str(" (objects skipped: load DATA in Objects panel first)");
                }
                self.texture_dirty = true;
                self.navigation.invalidate();
                let summary = format!(
                    "wall applied: {blocked} collision pixels, {foundations} foundation tiles, {placed_count} objects placed{placement_note}"
                );
                self.landscape.plan_applied(LandscapeApply::Wall, &summary);
            }
        }
    }

    fn handle_erosion_apply(&mut self, plan: crate::landscape_ui::ErosionPlan) {
        let Some(terrain) = self.session.as_mut() else {
            self.landscape.apply_failed("No terrain session loaded.");
            return;
        };

        // The preview is calculated against a specific heightmap. Reject it if another
        // tool changed any source sample while the dialog remained open.
        if plan
            .changes
            .iter()
            .any(|change| terrain.heights().get(change.index).copied() != Some(change.before))
        {
            self.landscape.apply_failed(
                "Terrain changed after the erosion preview; calculate the plan again.",
            );
            return;
        }

        let changes: Vec<_> = plan
            .changes
            .iter()
            .map(|change| against_rome_editor::terrain::SampleChange {
                index: change.index,
                before: change.before,
                after: change.after,
            })
            .collect();
        let applied = terrain.apply_height_changes(&changes);
        if applied.is_empty() {
            self.landscape.erosion_applied("no height samples changed");
            return;
        }
        terrain.commit_stroke();
        self.texture_dirty = true;
        self.navigation.invalidate();
        self.landscape
            .erosion_applied(&format!("{} height samples modified", applied.len()));
    }

    fn handle_ramp_point(&mut self, point: (f32, f32)) {
        let Some(start) = self.ramp_start.take() else {
            self.ramp_start = Some(point);
            self.status_message = format!(
                "Ramp start selected at ({:.0}, {:.0}); choose the endpoint.",
                point.0, point.1
            );
            self.status_is_error = false;
            return;
        };
        if start == point {
            self.ramp_start = Some(start);
            return;
        }
        let Some(terrain) = self.session.as_mut() else {
            self.ramp_start = Some(start);
            self.status_message = "Ramp rejected: no terrain session loaded.".into();
            self.status_is_error = true;
            return;
        };
        let parameters = against_rome_editor::terrain::RampParameters {
            width: self.ramp_width,
            falloff: self.ramp_falloff,
            ..Default::default()
        };
        match terrain.apply_ramp(start, point, &parameters) {
            Ok(changes) if !changes.is_empty() => {
                self.ramp_selecting = false;
                self.texture_dirty = true;
                self.navigation.invalidate();
                self.status_message =
                    format!("Ramp applied: {} height samples modified.", changes.len());
                self.status_is_error = false;
            }
            Ok(_) => {
                self.ramp_start = Some(start);
                self.status_message =
                    "Ramp made no visible change; choose another endpoint.".into();
                self.status_is_error = false;
            }
            Err(error) => {
                self.ramp_start = Some(start);
                self.status_message = format!("Ramp rejected: {error}");
                self.status_is_error = true;
            }
        }
    }

    fn cancel_ramp_selection(&mut self) {
        self.ramp_selecting = false;
        self.ramp_start = None;
        self.status_message = "Ramp selection cancelled.".into();
        self.status_is_error = false;
    }

    /// Centers the 2D canvas on a world position (C# `LocateMapIssue` -> `FocusTile`).
    pub fn focus_world_position(&mut self, world_x: f32, world_z: f32) {
        if !(world_x.is_finite() && world_z.is_finite()) {
            return;
        }
        let canvas = self.last_canvas_rect;
        let side = canvas.width().min(canvas.height()) * self.zoom;
        let world = self.nature_dimension() as f32 * 256.0;
        // map_rect centre = canvas centre + pan; the point sits at (norm - 0.5) * side from it.
        self.pan = Vec2::new(
            -(world_x / world - 0.5) * side,
            -(world_z / world - 0.5) * side,
        );
        self.status_message = format!(
            "Focused tile ({:.1}, {:.1}).",
            world_x / 256.0,
            world_z / 256.0
        );
        self.status_is_error = false;
    }

    /// Applies a Locate request from the Design Check issue list.
    fn consume_design_check_focus(&mut self) {
        if let Some((x, z)) = self.design_check.focus_request.take() {
            self.focus_world_position(x, z);
        }
        if self.design_check.event_request.take().is_some() {
            self.scenario.open = true;
        }
    }

    /// Starts or cancels the box-select tool (needs loaded DATA; 2D canvas only).
    pub fn toggle_box_select(&mut self) {
        if self.box_select {
            self.cancel_box_select();
        } else if !self.placement.is_loaded() {
            self.status_message = "Load DATA in the Objects panel before box select.".into();
            self.status_is_error = true;
        } else {
            self.box_select = true;
            self.box_drag_start = None;
            self.ramp_selecting = false;
            self.status_message = "Drag to select placed objects (Shift adds); Esc cancels.".into();
            self.status_is_error = false;
        }
    }

    pub fn cancel_box_select(&mut self) {
        self.box_select = false;
        self.box_drag_start = None;
    }

    /// Finishes a box drag between two screen points over the displayed map.
    pub fn finish_box_select(&mut self, a: Pos2, b: Pos2, map_rect: Rect, additive: bool) -> usize {
        let dimension = self.nature_dimension();
        let rect = crate::placement_ui::screen_box_to_tile_rect(
            (a.x, a.y),
            (b.x, b.y),
            (map_rect.min.x, map_rect.min.y),
            (map_rect.width(), map_rect.height()),
            dimension,
        );
        let count = self.placement.select_in_tile_rect(rect, additive);
        self.box_select = false;
        self.box_drag_start = None;
        self.status_message = format!(
            "Box selected {count} object(s) in tiles [{},{}) x [{},{}).",
            rect.left, rect.right, rect.top, rect.bottom
        );
        self.status_is_error = false;
        count
    }

    /// 2D canvas input for the box-select tool: drag a rectangle, release to select.
    fn handle_box_select_input(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
        painter: &egui::Painter,
        map_rect: Rect,
    ) {
        let world = self.nature_dimension() as f32 * 256.0;
        for object in self.placement.objects() {
            let nx = object.position[0] / world;
            let nz = object.position[2] / world;
            let point = Pos2::new(
                map_rect.min.x + nx * map_rect.width(),
                map_rect.min.y + nz * map_rect.height(),
            );
            let color = if self.placement.is_selected((object.slot, object.uid)) {
                Color32::YELLOW
            } else {
                Color32::from_rgba_unmultiplied(120, 200, 255, 160)
            };
            painter.circle_filled(point, 2.0, color);
        }
        let pointer = ctx.input(|i| (i.pointer.latest_pos(), i.pointer.primary_down(), i.modifiers.shift));
        if self.box_drag_start.is_none() && response.drag_started_by(egui::PointerButton::Primary) {
            self.box_drag_start = ctx
                .input(|i| i.pointer.press_origin())
                .or(pointer.0)
                .map(|p| map_rect.clamp(p));
        }
        let Some(start) = self.box_drag_start else {
            return;
        };
        let Some(current) = pointer.0.map(|p| map_rect.clamp(p)) else {
            return;
        };
        let rect = Rect::from_two_pos(start, current);
        painter.rect(
            rect,
            0.0,
            Color32::from_rgba_unmultiplied(255, 220, 80, 40),
            Stroke::new(1.5_f32, Color32::YELLOW),
        );
        if !pointer.1 {
            self.finish_box_select(start, current, map_rect, pointer.2);
        }
    }

    /// Objdef names (cached per game root) plus the Environment template list, for layouts.
    fn layout_name_table(&mut self) -> Vec<(i32, String)> {
        let mut table: Vec<(i32, String)> = Vec::new();
        let game = self
            .placement
            .loaded_map()
            .and_then(|m| m.parent())
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        if let Some(game) = game {
            if !matches!(&self.nature.names, Some((root, _)) if *root == game) {
                if let Ok(definitions) = against_rome_maps::NativeObjectDefinitions::load(&game) {
                    let names: HashMap<i32, String> = definitions
                        .into_iter()
                        .filter_map(|(id, d)| d.name.filter(|n| !n.is_empty()).map(|n| (id, n)))
                        .collect();
                    if !names.is_empty() {
                        self.nature.names = Some((game.clone(), names));
                    }
                }
            }
            if let Some((root, names)) = &self.nature.names {
                if *root == game {
                    table.extend(names.iter().map(|(id, name)| (*id, name.clone())));
                    table.sort_by_key(|(id, _)| *id);
                }
            }
        }
        if let Ok(templates) = crate::design_apply::parse_templates(&self.environment.flora_templates_text) {
            for (id, name) in templates {
                if !table.iter().any(|(existing, _)| *existing == id) {
                    table.push((id, name));
                }
            }
        }
        table
    }

    /// Forwards Environment-panel layout buttons and runs queued layout exports/applies with the
    /// current terrain and objdef names.
    fn pump_layout_requests(&mut self) {
        if let Some(rect) = self.environment.take_layout_export_request() {
            self.placement.request_layout(LayoutRequest::ExportNature(rect));
        }
        if self.environment.take_layout_import_request() {
            self.placement.begin_layout_import();
        }
        if !self.placement.has_layout_requests() {
            return;
        }
        let names = self.layout_name_table();
        let dim = self.session.as_ref().map_or(0, |s| s.vertex_size);
        let step = self.height_step;
        let heights = self.session.as_ref().map(|s| s.heights());
        let ground = move |x: f32, z: f32| crate::environment_ui::ground_height(heights, dim, x, z, step);
        let ctx = LayoutContext { names: &names, ground: &ground };
        self.placement.process_layout_requests(&ctx);
        self.status_message = self.placement.layout_status().to_string();
        self.status_is_error = self.status_message.contains("failed");
    }

    fn handle_environment_apply(&mut self, apply: EnvApply) {
        if !self.placement.is_loaded() {
            self.environment
                .apply_failed("Load DATA in the Objects panel first before applying placements.");
            return;
        }
        match apply {
            EnvApply::Flora => {
                let requests = self.environment.flora_requests();
                if requests.is_empty() {
                    self.environment
                        .apply_failed("No flora generated to apply.");
                    return;
                }
                match self.placement.apply_requests(&requests) {
                    Ok(batch) => {
                        let note = if !batch.skipped.is_empty() {
                            format!(" (skipped: {})", batch.skipped.join("; "))
                        } else {
                            String::new()
                        };
                        self.environment
                            .plan_queued(EnvApply::Flora, batch.placed, &note);
                    }
                    Err(e) => self.environment.apply_failed(e),
                }
            }
            EnvApply::Lairs => {
                let dim = self.session.as_ref().map_or(0, |s| s.vertex_size);
                let heights = self.session.as_ref().map(|s| s.heights());
                match self
                    .environment
                    .lair_requests(dim, heights, self.height_step)
                {
                    Ok((requests, unresolved)) => {
                        if !unresolved.is_empty() {
                            self.environment.apply_failed(format!(
                                "Add cloneable templates for all neutral animal types before applying: {}",
                                unresolved.join(", ")
                            ));
                            return;
                        }
                        if requests.is_empty() {
                            self.environment.apply_failed(format!(
                                "No neutral animal templates could be resolved{}",
                                if unresolved.is_empty() {
                                    String::new()
                                } else {
                                    format!(": {}", unresolved.join(", "))
                                }
                            ));
                            return;
                        }
                        let available_templates = self
                            .placement
                            .objects()
                            .into_iter()
                            .filter(|o| {
                                o.parent.is_none() && o.position.iter().all(|v| v.is_finite())
                            })
                            .map(|o| o.type_id)
                            .collect::<std::collections::BTreeSet<_>>();
                        let missing_templates = requests
                            .iter()
                            .filter(|r| {
                                u16::try_from(r.type_id)
                                    .map_or(true, |type_id| !available_templates.contains(&type_id))
                            })
                            .map(|r| r.name.clone())
                            .collect::<std::collections::BTreeSet<_>>();
                        if !missing_templates.is_empty() {
                            self.environment.apply_failed(format!(
                                "Neutral animal templates are not cloneable from this map: {}",
                                missing_templates.into_iter().collect::<Vec<_>>().join(", ")
                            ));
                            return;
                        }
                        match self.placement.apply_requests(&requests) {
                            Ok(batch) => {
                                let mut notes = Vec::new();
                                if !batch.skipped.is_empty() {
                                    notes.push(batch.skipped.join("; "));
                                }
                                if !unresolved.is_empty() {
                                    notes.push(format!(
                                        "unresolved animal templates: {}",
                                        unresolved.join(", ")
                                    ));
                                }
                                let note = if notes.is_empty() {
                                    String::new()
                                } else {
                                    format!(" ({})", notes.join("; "))
                                };
                                self.environment
                                    .plan_queued(EnvApply::Lairs, batch.placed, &note);
                            }
                            Err(e) => self.environment.apply_failed(e),
                        }
                    }
                    Err(e) => self.environment.apply_failed(e),
                }
            }
            EnvApply::Settlement => {
                match self.apply_settlement() {
                    Ok(count) => self.environment.plan_queued(EnvApply::Settlement, count, " Placement and Nature share one undo step."),
                    Err(e) => self.environment.apply_failed(e),
                }
            }
        }
    }

    pub(crate) fn apply_settlement(&mut self) -> Result<usize, String> {
        if self.current_map_is_original() {
            return Err("Settlement: original maps are read-only".into());
        }
        let loaded = self
            .loaded_map_path
            .as_ref()
            .ok_or("Settlement: load the current map first")?;
        let placement_map = self
            .placement
            .loaded_map()
            .ok_or("Settlement: load DATA first")?;
        let loaded = std::fs::canonicalize(loaded).map_err(|e| e.to_string())?;
        if std::fs::canonicalize(self.map_path_input.trim()).map_err(|e| e.to_string())? != loaded
            || std::fs::canonicalize(placement_map).map_err(|e| e.to_string())? != loaded
        {
            return Err("Settlement: terrain and DATA must belong to the current map".into());
        }
        if self.nature.driver.is_active() {
            return Err("Settlement: finish the current Nature stroke first".into());
        }
        if let Err(catalog_error) = self.ensure_nature_data_with_placement(true) {
            let terrain = self.session.as_ref().ok_or("Settlement: load terrain first")?;
            let unresolved = self.environment.settlement_plan(
                &[], terrain.vertex_size, Some(terrain.heights()),
                Some((terrain.collision_size, terrain.collision())),
                self.water_surface, self.height_step,
            ).err().unwrap_or_else(|| "Settlement catalog unavailable".into());
            return Err(format!("{unresolved}; {catalog_error}"));
        }
        let names = &self
            .nature
            .names
            .as_ref()
            .ok_or("Settlement: missing objdef catalog")?
            .1;
        let catalog: Vec<_> = self
            .placement
            .objects()
            .into_iter()
            .filter(|o| o.parent.is_none() && o.position.iter().all(|v| v.is_finite()))
            .filter_map(|o| {
                names
                    .get(&i32::from(o.type_id))
                    .map(|n| (i32::from(o.type_id), n.clone()))
            })
            .collect();
        let terrain = self
            .session
            .as_ref()
            .ok_or("Settlement: load terrain first")?;
        let (requests, additions) = self.environment.settlement_plan(
            &catalog,
            terrain.vertex_size,
            Some(terrain.heights()),
            Some((terrain.collision_size, terrain.collision())),
            self.water_surface,
            self.height_step,
        )?;
        if requests.is_empty() && additions.is_empty() {
            return Err("Settlement: empty generated plan".into());
        }
        self.collect_pilot_receipts();
        let before = self.placement.capture_state();
        let after = self.placement.settlement_candidate(&requests)?;
        let nature_before = self.nature.session.clone();
        let mut nature_after = nature_before.clone();
        nature_after.plant_many(&additions)?;
        // Replay Nature onto the detached Placement candidate as well: shared pool capacity,
        // links and late failures must be proven before either live session is published.
        let mut detached = PlacementUi::default();
        detached.restore_state(&after);
        let map = self
            .placement
            .loaded_map()
            .ok_or("Settlement: load DATA first")?;
        self.replay_nature_candidate(map, &nature_after, detached.detached_store()?)?;
        self.placement.restore_state(&after);
        self.nature.session = nature_after.clone();
        self.nature.last_action = false;
        self.settlement_undo.push(SettlementEdit {
            before,
            after,
            nature_before,
            nature_after,
            placement_changed: !requests.is_empty(),
            nature_changed: !additions.is_empty(),
        });
        self.settlement_redo.clear();
        // 複合操作只旁錄一次，子系統收據由既有原子歷程接管。
        self.placement.take_history_receipts();
        self.nature.session.take_history_receipts();
        let depth = self.settlement_undo.len() as u64;
        self.undo_journal.record_compound(vec![crate::undo_journal::SubsystemStep {
            subsystem: crate::undo_journal::SubsystemId::Other,
            before_token: depth - 1,
            after_token: depth,
        }]);
        Ok(requests.len() + additions.len())
    }

    pub fn has_settlement_undo(&self) -> bool {
        !self.settlement_undo.is_empty()
    }

    fn settlement_history(&mut self, redo: bool) -> bool {
        let stack = if redo {
            &self.settlement_redo
        } else {
            &self.settlement_undo
        };
        let Some(edit) = stack.last() else {
            return false;
        };
        let (expected, nature_expected) = if redo {
            (&edit.before, &edit.nature_before)
        } else {
            (&edit.after, &edit.nature_after)
        };
        let matches = if redo {
            self.placement.matches_state(expected)
        } else {
            self.placement.matches_undo_boundary(expected)
        };
        if !matches || self.nature.session.capture() != nature_expected.capture() {
            return false;
        }
        let placement = if edit.placement_changed {
            match self.placement.history_candidate(redo) {
                Ok(state) => state,
                Err(_) => return false,
            }
        } else {
            self.placement.capture_state()
        };
        let mut nature = self.nature.session.clone();
        if edit.nature_changed && !(if redo { nature.redo() } else { nature.undo() }) {
            return false;
        }
        let mut edit = if redo {
            self.settlement_redo.pop().unwrap()
        } else {
            self.settlement_undo.pop().unwrap()
        };
        self.placement.restore_state(&placement);
        self.nature.session = nature;
        if redo {
            edit.after = self.placement.capture_state();
            edit.nature_after = self.nature.session.clone();
        } else {
            edit.before = self.placement.capture_state();
            edit.nature_before = self.nature.session.clone();
        }
        self.nature.last_action = false;
        if redo {
            self.settlement_undo.push(edit);
        } else {
            self.settlement_redo.push(edit);
        }
        self.status_message = if redo {
            "Settlement redo performed."
        } else {
            "Settlement undo performed."
        }
        .into();
        self.status_is_error = false;
        true
    }

    fn current_view_start_camera(&self) -> Result<against_rome_campaign::MapStartCamera, String> {
        if self.loaded_map_path.is_none()
            || self.session.is_none()
            || self.view_mode != ViewMode::View3D
        {
            return Err("Capture requires a loaded map and the current 3D view".into());
        }
        let target = self.viewport_3d.camera.target;
        if !target.x.is_finite() || !target.y.is_finite() || !target.z.is_finite() {
            return Err("Current view has invalid camera coordinates".into());
        }
        Ok(against_rome_campaign::MapStartCamera {
            enabled: true,
            x: (target.x as f64 * 256.0).clamp(0.0, 16383.0),
            y: (target.y as f64).clamp(-500.0, 5000.0),
            z: (target.z as f64 * 256.0).clamp(0.0, 16383.0),
            zoom: 5.0,
            primitive: against_rome_campaign::StartCameraPrimitive::CenterToPos,
        })
    }

    fn apply_campaign_camera_requests(&mut self) {
        let (apply, capture) = self.campaign.take_camera_requests();
        if capture {
            match self.current_view_start_camera() {
                Ok(camera) => {
                    self.campaign.capture_camera(camera);
                }
                Err(e) => {
                    self.campaign.status_message = e;
                    self.campaign.status_is_error = true;
                }
            }
        }
        if apply {
            let _ = self.apply_start_camera_candidate_from_ui();
        }
    }

    pub(crate) fn apply_start_camera_candidate_from_ui(&mut self) -> Result<(), String> {
        let result = self.apply_start_camera_candidate();
        match &result {
            Ok(()) => {
                let msg = tr(
                    self.lang(),
                    "Start camera candidate ready; use Safe Save to write.",
                    "起始鏡頭候選已備妥，請使用安全儲存寫入。",
                );
                self.campaign.status_message = msg.into();
                self.campaign.status_is_error = false;
                self.status_message = msg.into();
                self.status_is_error = false;
            }
            Err(e) => {
                let msg = format!(
                    "{}: {e}",
                    tr(self.lang(), "Start camera Apply rejected", "起始鏡頭套用失敗")
                );
                self.campaign.status_message = msg.clone();
                self.campaign.status_is_error = true;
                self.status_message = msg;
                self.status_is_error = true;
            }
        }
        result
    }

    fn apply_start_camera_candidate(&mut self) -> Result<(), String> {
        let path = self
            .loaded_map_path
            .as_ref()
            .ok_or("Load a map before applying the start camera")?;
        if self.session.is_none()
            || std::fs::canonicalize(path).map_err(|e| e.to_string())?
                != std::fs::canonicalize(self.map_path_input.trim()).map_err(|e| e.to_string())?
        {
            return Err("Start camera target must be the currently loaded map".into());
        }
        let scenario_files = if self.scenario.dirty() || self.scenario.graph_ui().is_dirty() {
            self.scenario.stage_transaction_files(path)?
        } else {
            Vec::new()
        };
        self.campaign
            .apply_camera_candidate(path, &scenario_files, self.scenario.known_aliases())
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.is_dirty())
            || self.session.as_ref().zip(self.emboss.as_ref()).is_some_and(|(s, e)| e.session.is_dirty(s.heights()))
            || self.reset_auxiliary_layers
            || self.placement.is_dirty()
            || self.nature.session.is_dirty()
            || self.scenario.dirty()
            || self.scenario.graph_ui().is_dirty()
            || self.textures.is_dirty()
            || self.scene.is_dirty()
            || self.event_graph.is_dirty()
            || self.maps.is_dirty()
            || self.environment.is_dirty()
            || self.landscape.is_dirty()
            || self.packaging.is_dirty()
            || self.campaign.is_dirty()
            || self.ai_planner.is_dirty()
            || self.ambience.is_dirty()
            || self.toolbox.is_dirty()
    }

    fn update_texture(&mut self, ctx: &egui::Context) {
        let Some(session) = &self.session else { return };
        let w = session.vertex_size;
        let h = session.vertex_size;
        let mut pixels = Vec::with_capacity(w * h);

        let col_scale = session.collision_size as f32 / (session.vertex_size.max(2) - 1) as f32;

        for y in 0..h {
            for x in 0..w {
                let height = session.heights()[y * w + x];
                let col_x =
                    ((x as f32 * col_scale).round() as usize).min(session.collision_size - 1);
                let col_y =
                    ((y as f32 * col_scale).round() as usize).min(session.collision_size - 1);
                let is_blocked = session.collision()[col_y * session.collision_size + col_x] > 0;

                let color = match self.view_mode {
                    ViewMode::Heightmap => {
                        // 地形高程調色盤：綠色平原 -> 土黃丘陵 -> 雪白峰頂
                        if height < 64 {
                            Color32::from_rgb(20 + height, 80 + height * 2, 20 + height)
                        } else if height < 160 {
                            Color32::from_rgb(100 + (height - 64), 140 + (height - 64) / 2, 50)
                        } else {
                            let light = 160 + ((height as u32 - 160) * 95 / 95) as u8;
                            Color32::from_rgb(light, light, light)
                        }
                    }
                    ViewMode::Collision => {
                        if is_blocked {
                            Color32::from_rgb(220, 40, 40) // 紅色阻擋
                        } else {
                            Color32::from_rgb(30, 30, 30) // 暗黑可通行
                        }
                    }
                    ViewMode::Blended | ViewMode::View3D => {
                        let base_g = (height as f32 * 0.7) as u8 + 40;
                        if is_blocked {
                            Color32::from_rgb(200, base_g / 2, base_g / 2) // 阻擋染紅
                        } else {
                            Color32::from_rgb(base_g / 2, base_g, base_g / 3)
                        }
                    }
                };
                pixels.push(color);
            }
        }

        let image = ColorImage {
            size: [w, h],
            pixels,
        };

        if let Some(tex) = &mut self.texture {
            tex.set(image, egui::TextureOptions::NEAREST);
        } else {
            self.texture =
                Some(ctx.load_texture("terrain-view", image, egui::TextureOptions::NEAREST));
        }
        self.texture_dirty = false;
    }

    pub fn handle_viewport_3d_action(&mut self, action: ViewportAction) {
        match action {
            ViewportAction::None => {}
            ViewportAction::CancelTerrainPick => {
                if self.ramp_selecting {
                    self.cancel_ramp_selection();
                }
            }
            ViewportAction::PickTerrain { grid_x, grid_y } => {
                if self.ramp_selecting && self.pending_action.is_none() {
                    self.handle_ramp_point((grid_x, grid_y));
                }
            }
            ViewportAction::SelectObject(key) => {
                self.placement.select_object(key);
            }
            ViewportAction::Paint { grid_x, grid_y } if self.nature.enabled => {
                if self.pending_action.is_some() || self.ramp_selecting {
                    return;
                }
                if let Some((tx, ty)) = self.nature_tile_from_grid(grid_x, grid_y) {
                    self.nature_paint_tile(tx, ty);
                }
            }
            ViewportAction::CommitStroke if self.nature.driver.is_active() => {
                self.nature_end_stroke();
            }
            ViewportAction::Paint { grid_x, grid_y } if self.active_tool == ActiveTool::Material => {
                if self.pending_action.is_some() || self.ramp_selecting {
                    return;
                }
                let english = self.lang() == AppLanguage::English;
                let Some(span) = self.session.as_ref().map(|s| (s.vertex_size - 1) as f32) else {
                    return;
                };
                self.textures
                    .paint_material_at_grid(grid_x, grid_y, span, english);
            }
            ViewportAction::Paint { grid_x, grid_y } => {
                if self.pending_action.is_some() || self.ramp_selecting {
                    return;
                }
                if !self.is_painting {
                    self.stroke_path.clear();
                }
                self.is_painting = true;
                self.nature.last_action = false;
                let tool = self.active_tool;
                let radius = self.brush_radius;
                let strength = self.brush_strength;
                let seed = self.roughness_seed;

                let points = self.stroke_path.advance((
                    grid_x.round_ties_even() as i32,
                    grid_y.round_ties_even() as i32,
                ));

                if let Some(session_mut) = &mut self.session {
                    for (x, y) in points {
                        let gx = x as f32;
                        let gy = y as f32;
                        if tool == ActiveTool::HeightFlatten
                            && self.stroke_path.flatten_target.is_none()
                        {
                            self.stroke_path.flatten_target = Some(if self.sample_flatten_target {
                                session_mut.heights()
                                    [y as usize * session_mut.vertex_size + x as usize]
                            } else {
                                self.flatten_target
                            });
                        }
                        let flat_target = self.stroke_path.flatten_target;
                        let result = match tool {
                            // Material is handled by its own branch before this brush loop.
                            ActiveTool::Material => continue,
                            ActiveTool::HeightRaise => session_mut.paint_height(
                                gx,
                                gy,
                                radius,
                                HeightBrushOp::Raise,
                                strength,
                                None,
                                seed,
                            ),
                            ActiveTool::HeightLower => session_mut.paint_height(
                                gx,
                                gy,
                                radius,
                                HeightBrushOp::Lower,
                                strength,
                                None,
                                seed,
                            ),
                            ActiveTool::HeightFlatten => session_mut.paint_height(
                                gx,
                                gy,
                                radius,
                                HeightBrushOp::Flatten,
                                strength,
                                flat_target,
                                seed,
                            ),
                            ActiveTool::HeightSmooth => session_mut.paint_height(
                                gx,
                                gy,
                                radius,
                                HeightBrushOp::Smooth,
                                strength,
                                None,
                                seed,
                            ),
                            ActiveTool::HeightRoughen => session_mut.paint_height(
                                gx,
                                gy,
                                radius,
                                HeightBrushOp::Roughen,
                                strength,
                                None,
                                seed,
                            ),
                            ActiveTool::Water if self.water_surface < 4.0 => {
                                Err(against_rome_editor::terrain::Error::InvalidParameters)
                            }
                            ActiveTool::Water => session_mut
                                .paint_water(
                                    gx,
                                    gy,
                                    radius,
                                    strength,
                                    self.water_bed,
                                    self.water_surface,
                                )
                                .map(|s| s.heights),
                            ActiveTool::CollisionBlock => {
                                let col_scale = session_mut.collision_size as f32
                                    / (session_mut.vertex_size - 1) as f32;
                                session_mut.paint_collision(
                                    gx * col_scale,
                                    gy * col_scale,
                                    radius * col_scale,
                                    CollisionBrushOp::Block,
                                )
                            }
                            ActiveTool::CollisionClear => {
                                let col_scale = session_mut.collision_size as f32
                                    / (session_mut.vertex_size - 1) as f32;
                                session_mut.paint_collision(
                                    gx * col_scale,
                                    gy * col_scale,
                                    radius * col_scale,
                                    CollisionBrushOp::Clear,
                                )
                            }
                        };
                        if let Err(error) = result {
                            self.status_message = format!("Brush rejected: {error}");
                            self.status_is_error = true;
                        }
                    }
                    self.texture_dirty = true;
                }
            }
            ViewportAction::CommitStroke => {
                self.textures.end_material_stroke();
                if self.is_painting {
                    self.is_painting = false;
                    if let Some(session_mut) = &mut self.session {
                        session_mut.commit_stroke();
                    }
                    self.stroke_path.clear();
                    self.texture_dirty = true;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Nature Plant/Remove brush (port of C# `MapEditorForm.Nature.cs`)
//
// The native `NatureStrokeDriver` plans each brush dab. This workflow owns the pending
// `NatureEditSession` and, on Safe Save, replays it onto a freshly loaded `ObjectStore` so the
// committed additions/removals reach `DATA/*.dat` through the same whole-map transaction as every
// other panel. Anything that cannot be proven (objdef names, template objects, slot identity,
// linked/shared data, pool capacity) aborts the save with the candidate and session untouched.
// ---------------------------------------------------------------------------------------------

/// One tile = 256 world units (`SdlSceneCatalog.WorldUnitsPerMapPixel * 4`).
const NATURE_TILE_WORLD: f32 = 256.0;
/// 16384 world units / 256 per tile; used when no texture session reports its own dimension.
const NATURE_DEFAULT_DIMENSION: i32 = 64;
const NATURE_WORLD_SIZE: f32 = 16384.0;
/// Map-bound snapshot of the landscape objects the brush may see, taken from the DATA session.
struct NatureMapData {
    map: PathBuf,
    level: Vec<PlacedNatureObject>,
    /// slot -> (uid, type id): proves at save time that a removed slot is still the same object.
    identity: HashMap<i32, (u32, u16)>,
    positions: HashMap<i32, (f32, f32)>,
    /// Root landscape types present on the map, i.e. types that can be cloned as templates.
    catalog: Vec<NatureCatalogItem>,
    regions: BTreeSet<String>,
}

struct SettlementEdit {
    before: crate::placement_ui::PlacementState,
    after: crate::placement_ui::PlacementState,
    nature_before: NatureEditSession,
    nature_after: NatureEditSession,
    placement_changed: bool,
    nature_changed: bool,
}

struct NatureWorkflow {
    enabled: bool,
    operation: NatureOperation,
    brush_size: i32,
    density: NatureDensity,
    mix_species: bool,
    show_other_regions: bool,
    category: Option<NatureCategory>,
    selected_type: Option<i32>,
    session: NatureEditSession,
    driver: NatureStrokeDriver,
    random: DotNetRandom,
    /// objdef names keyed by the game root they were read from.
    names: Option<(PathBuf, HashMap<i32, String>)>,
    data: Option<NatureMapData>,
    /// Nature was the last thing edited, so Ctrl+Z targets it before the terrain history.
    last_action: bool,
}

impl Default for NatureWorkflow {
    fn default() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos() as i32);
        Self {
            enabled: false,
            operation: NatureOperation::Plant,
            brush_size: 1,
            density: NatureDensity::Normal,
            mix_species: false,
            show_other_regions: false,
            category: None,
            selected_type: None,
            session: NatureEditSession::new_with_history_token_allocator(
                against_rome_textures::history::next_history_token,
            ),
            driver: NatureStrokeDriver::new(),
            random: DotNetRandom::new(seed),
            names: None,
            data: None,
            last_action: false,
        }
    }
}

impl NatureWorkflow {
    /// Drops everything bound to the previous map; brush preferences survive.
    fn reset_map_bound(&mut self) {
        self.session.clear();
        self.driver = NatureStrokeDriver::new();
        self.names = None;
        self.data = None;
        self.last_action = false;
    }

    /// A committed Safe Save made the disk the new baseline; the snapshot is rebuilt lazily.
    fn accept_saved(&mut self) {
        self.session.accept_changes();
        self.driver = NatureStrokeDriver::new();
        self.data = None;
        self.last_action = false;
    }
}

impl EditorApp {
    fn nature_fail(&mut self, message: impl Into<String>) {
        self.status_message = format!("Nature: {}", message.into());
        self.status_is_error = true;
    }

    /// Turn the brush on/off. Turning it off closes any open stroke; edits stay pending.
    pub fn nature_set_enabled(&mut self, enabled: bool) {
        if !enabled {
            self.nature_end_stroke();
        }
        self.nature.enabled = enabled;
        if enabled {
            match self.ensure_nature_data() {
                Ok(()) => {
                    self.status_message = "Nature brush ready.".into();
                    self.status_is_error = false;
                }
                Err(e) => self.nature_fail(e),
            }
        }
    }

    pub fn nature_set_operation(&mut self, operation: NatureOperation) {
        self.nature_end_stroke();
        self.nature.operation = operation;
    }

    pub fn nature_select_type(&mut self, type_id: i32) {
        self.nature.selected_type = Some(type_id);
    }

    pub fn nature_is_dirty(&self) -> bool {
        self.nature.session.is_dirty()
    }

    /// Pending additions / removed existing slots.
    pub fn nature_pending_counts(&self) -> (usize, usize) {
        (
            self.nature.session.additions().len(),
            self.nature.session.removed_slots().len(),
        )
    }

    /// (Re)builds the catalog and level-object snapshot from the loaded DATA session. While
    /// Nature edits are pending the snapshot is frozen so slot identities stay provable.
    fn ensure_nature_data(&mut self) -> Result<(), String> {
        self.ensure_nature_data_with_placement(false)
    }

    fn ensure_nature_data_with_placement(&mut self, allow_placement: bool) -> Result<(), String> {
        if self.placement.is_dirty() && !allow_placement {
            return Err("the Objects (DATA) session has unsaved edits; save or discard them before using the Nature brush".into());
        }
        let map = self.placement.loaded_map().map(Path::to_path_buf).ok_or(
            "catalog/object data unavailable: load the map's DATA in the Objects panel first",
        )?;
        if self.nature.session.is_dirty() {
            return match &self.nature.data {
                Some(data) if data.map == map => Ok(()),
                _ => Err(
                    "pending Nature edits no longer match the loaded DATA; save or discard them"
                        .into(),
                ),
            };
        }
        let game = map
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .ok_or("catalog unavailable: the map folder is not inside a game MAPS directory")?;
        if !matches!(&self.nature.names, Some((root, _)) if *root == game) {
            let definitions = against_rome_maps::NativeObjectDefinitions::load(&game)
                .map_err(|e| format!("catalog unavailable (objdef): {e}"))?;
            let names: HashMap<i32, String> = definitions
                .into_iter()
                .filter_map(|(id, d)| d.name.filter(|n| !n.is_empty()).map(|n| (id, n)))
                .collect();
            if names.is_empty() {
                return Err(format!(
                    "catalog unavailable: no object names found under {}",
                    game.display()
                ));
            }
            self.nature.names = Some((game, names));
        }
        let names = &self.nature.names.as_ref().expect("names cached above").1;
        let mut data = NatureMapData {
            map,
            level: Vec::new(),
            identity: HashMap::new(),
            positions: HashMap::new(),
            catalog: Vec::new(),
            regions: BTreeSet::new(),
        };
        let mut roots: std::collections::BTreeMap<i32, &String> = Default::default();
        for object in self.placement.objects() {
            let type_id = i32::from(object.type_id);
            let Some(name) = names.get(&type_id) else {
                continue;
            };
            if !(name.len() >= 3 && name[..3].eq_ignore_ascii_case("Lan")) {
                continue;
            }
            let [x, y, z] = object.position;
            if !(x.is_finite() && y.is_finite() && z.is_finite()) {
                continue;
            }
            let linked = object.parent.is_some();
            let rotation = if object.rotation_degrees.is_finite() {
                object.rotation_degrees.to_radians()
            } else {
                0.0
            };
            let slot = object.slot as i32;
            data.level.push(PlacedNatureObject::with_transform(
                slot,
                type_id,
                name.as_str(),
                x,
                y,
                z,
                rotation,
                i32::from(object.team),
                linked,
            ));
            data.identity.insert(slot, (object.uid, object.type_id));
            data.positions.insert(slot, (x, z));
            if !linked {
                roots.entry(type_id).or_insert(name);
            }
        }
        data.catalog = roots
            .into_iter()
            .map(|(id, name)| NatureCatalogItem::new(id, name.as_str()))
            .collect();
        data.regions = map_nature_regions(data.level.iter().map(|o| o.name.as_str()));
        self.nature.data = Some(data);
        Ok(())
    }

    /// Catalog filtered by category and the map's dominant regions (C# `RefreshNatureTypes`).
    pub fn nature_palette(&self) -> Vec<NatureCatalogItem> {
        let Some(data) = &self.nature.data else {
            return Vec::new();
        };
        NaturePaletteFilter::new()
            .with_category(self.nature.category)
            .with_dominant_regions(data.regions.clone())
            .with_show_other_regions(self.nature.show_other_regions)
            .filter_and_sort(&data.catalog)
            .into_iter()
            .cloned()
            .collect()
    }

    fn nature_selected_item(&self, palette: &[NatureCatalogItem]) -> Option<NatureCatalogItem> {
        self.nature
            .selected_type
            .and_then(|id| palette.iter().find(|i| i.type_id == id))
            .or_else(|| palette.first())
            .cloned()
    }

    fn nature_dimension(&mut self) -> i32 {
        self.textures
            .session_mut()
            .map(|s| s.tile_dimension() as i32)
            .filter(|d| *d > 0)
            .unwrap_or(NATURE_DEFAULT_DIMENSION)
    }

    /// Vertex-grid position (0..=vertex_size-1) to a texture tile.
    fn nature_tile_from_grid(&mut self, grid_x: f32, grid_y: f32) -> Option<(i32, i32)> {
        let span = (self.session.as_ref()?.vertex_size.checked_sub(1)?).max(1) as f32;
        let dimension = self.nature_dimension();
        let tile = |g: f32| ((g / span * dimension as f32).floor() as i32).clamp(0, dimension - 1);
        (grid_x.is_finite() && grid_y.is_finite()).then(|| (tile(grid_x), tile(grid_y)))
    }

    /// One brush dab at a tile; the first dab of a stroke refreshes the object snapshot.
    /// Fails closed (status error, no edit) when catalog/template/object data is unavailable.
    pub fn nature_paint_tile(&mut self, tile_x: i32, tile_y: i32) -> NatureStrokeReport {
        if !self.nature.driver.is_active() || self.nature.data.is_none() {
            if let Err(e) = self.ensure_nature_data() {
                self.nature_fail(e);
                return NatureStrokeReport::default();
            }
        }
        let dimension = self.nature_dimension();
        if tile_x < 0 || tile_y < 0 || tile_x >= dimension || tile_y >= dimension {
            return NatureStrokeReport::default();
        }
        let palette = self.nature_palette();
        let selected = self.nature_selected_item(&palette);
        let height_step = self.height_step;
        let Some(terrain) = self.session.as_ref() else {
            self.nature_fail("no terrain session loaded");
            return NatureStrokeReport::default();
        };
        let settings = NatureStrokeSettings {
            operation: self.nature.operation,
            brush_size: self.nature.brush_size.max(1),
            density: self.nature.density,
            dimension,
            tile_world_size: NATURE_TILE_WORLD,
            mix_species: self.nature.mix_species,
            selected,
            heights: Some(NatureHeightField {
                vertex_size: terrain.vertex_size,
                heights: terrain.heights(),
                height_step,
            }),
        };
        let nature = &mut self.nature;
        let Some(data) = nature.data.as_ref() else {
            self.status_message = "Nature: catalog/object data unavailable".into();
            self.status_is_error = true;
            return NatureStrokeReport::default();
        };
        let report = nature.driver.paint(
            &mut nature.session,
            &settings,
            &data.level,
            &palette,
            &mut nature.random,
            tile_x,
            tile_y,
        );
        if report.missing_selection {
            self.status_message =
                "Nature: pick a landscape type first (the catalog has no matching entry).".into();
            self.status_is_error = true;
        } else if report.changed() {
            self.status_message = format!(
                "Nature: planted {}, removed {} existing / {} pending objects.",
                report.planted, report.removed_slots, report.removed_additions
            );
            self.status_is_error = false;
        }
        report
    }

    /// Ends the open stroke as exactly one undo step.
    pub fn nature_end_stroke(&mut self) -> bool {
        let committed = self.nature.driver.end(&mut self.nature.session);
        if committed {
            self.nature.last_action = true;
        }
        committed
    }

    /// Esc: drop the stroke in progress without leaving an undo step.
    pub fn nature_cancel_stroke(&mut self) {
        let cancelled = self.nature.session.cancel_stroke();
        self.nature.driver.end(&mut self.nature.session);
        if cancelled {
            self.status_message = "Nature stroke cancelled.".into();
            self.status_is_error = false;
        }
    }

    /// Ctrl+Z target: true when Nature handled the undo.
    pub fn nature_undo(&mut self) -> bool {
        if self.settlement_history(false) { return true; }
        self.nature_end_stroke();
        if !(self.nature.last_action && self.nature.session.can_undo()) {
            return false;
        }
        self.nature.session.undo();
        self.status_message = "Nature undo performed.".into();
        self.status_is_error = false;
        true
    }

    pub fn nature_redo(&mut self) -> bool {
        if self.settlement_history(true) { return true; }
        self.nature_end_stroke();
        if !(self.nature.last_action && self.nature.session.can_redo()) {
            return false;
        }
        self.nature.session.redo();
        self.status_message = "Nature redo performed.".into();
        self.status_is_error = false;
        true
    }

    /// Drops every pending Nature edit back to the last saved baseline.
    pub fn nature_discard(&mut self) {
        self.nature.driver = NatureStrokeDriver::new();
        self.nature.session.reset();
        self.nature.last_action = false;
        self.status_message = "Nature edits discarded.".into();
        self.status_is_error = false;
    }

    /// Replay Nature onto a detached current Placement store and serialize DATA once.
    /// Independent conflicting workflows retain their existing fail-closed behavior.
    fn stage_nature_files(&self, target_map: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
        if self.placement.is_dirty() && self.settlement_undo.is_empty() && self.settlement_redo.is_empty() {
            return Err("Nature: the Objects (DATA) session has unsaved edits; save or discard them before saving Nature changes".into());
        }
        let store = self.replay_nature_candidate(target_map, &self.nature.session, self.placement.detached_store()?)?;
        self.placement.stage_candidate_store(target_map, store)
    }

    fn replay_nature_candidate(
        &self, target_map: &Path, session: &NatureEditSession,
        mut store: against_rome_placement::ObjectStore,
    ) -> Result<against_rome_placement::ObjectStore, String> {
        use crate::design_apply::{apply_placements, PlacementRequest};
        if !session.is_dirty() {
            return Ok(store);
        }
        let loaded = self
            .placement
            .loaded_map()
            .ok_or("Nature: no DATA session is loaded for this map")?;
        let same_path =
            |a: &Path, b: &Path| match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
                (Ok(a), Ok(b)) => a == b,
                _ => a == b,
            };
        if !same_path(loaded, target_map) {
            return Err(format!(
                "Nature: edits belong to '{}' and cannot be saved to '{}'",
                loaded.display(),
                target_map.display()
            ));
        }
        let data = self
            .nature
            .data
            .as_ref()
            .filter(|d| same_path(&d.map, target_map))
            .ok_or("Nature: catalog/object data unavailable; cannot verify pending edits")?;

        let disk = against_rome_placement::ObjectStore::load(target_map)
            .map_err(|e| format!("Nature: cannot load DATA objects: {e}"))?;
        let current_disk: HashMap<i32, (u32, u16)> = disk.objects().iter()
            .map(|o| (o.slot as i32, (o.uid, o.type_id))).collect();
        let current: HashMap<i32, (u32, u16)> = store
            .objects()
            .iter()
            .map(|o| (o.slot as i32, (o.uid, o.type_id)))
            .collect();
        let mut families = Vec::new();
        for slot in session.removed_slots() {
            let expected = data.identity.get(slot).ok_or_else(|| {
                format!("Nature: removed slot {slot} is not a known landscape object")
            })?;
            if current.get(slot) != Some(expected) || current_disk.get(slot) != Some(expected) {
                return Err(format!(
                    "Nature: slot {slot} changed on disk since it was selected; reload the map and redo the removal"
                ));
            }
            families.push((*slot as usize, expected.0));
        }
        // Add before removing so templates survive even when a whole type is erased.
        let requests: Vec<PlacementRequest> = session
            .additions()
            .iter()
            .map(|a| PlacementRequest {
                type_id: a.type_id,
                name: a.name.clone(),
                x: a.x,
                y: a.y,
                z: a.z,
                rotation_deg: a.rotation.to_degrees(),
                team: None,
            })
            .collect();
        let mut rest = requests.as_slice();
        while !rest.is_empty() {
            let batch = apply_placements(&mut store, rest)
                .map_err(|e| format!("Nature: cannot add objects: {e}"))?;
            if batch.consumed == 0 || batch.placed != batch.consumed || !batch.skipped.is_empty() {
                return Err(format!(
                    "Nature: additions rejected (missing template or invalid position): {}",
                    batch.skipped.join("; ")
                ));
            }
            rest = &rest[batch.consumed..];
        }
        store
            .remove_families(&families)
            .map_err(|e| format!("Nature: removal rejected (linked, shared or stale data): {e}"))?;
        Ok(store)
    }

    /// Pending additions (green) and removed existing objects (red) over the 2D map.
    fn draw_nature_overlay(&self, painter: &egui::Painter, map_rect: Rect) {
        let to_screen = |x: f32, z: f32| {
            Pos2::new(
                map_rect.min.x + x / NATURE_WORLD_SIZE * map_rect.width(),
                map_rect.min.y + z / NATURE_WORLD_SIZE * map_rect.height(),
            )
        };
        for addition in self.nature.session.additions() {
            painter.circle_filled(to_screen(addition.x, addition.z), 2.0, Color32::LIGHT_GREEN);
        }
        if let Some(data) = &self.nature.data {
            for slot in self.nature.session.removed_slots() {
                if let Some(&(x, z)) = data.positions.get(slot) {
                    painter.circle_stroke(
                        to_screen(x, z),
                        3.0,
                        Stroke::new(1.0_f32, Color32::LIGHT_RED),
                    );
                }
            }
        }
    }

    fn show_nature_panel(&mut self, ui: &mut egui::Ui) {
        if std::mem::take(&mut self.save_focus_nature) {
            ui.scroll_to_cursor(Some(egui::Align::Center));
        }
        ui.heading("Nature Brush");
        let mut enabled = self.nature.enabled;
        if ui
            .checkbox(&mut enabled, "Nature brush (replaces terrain brush)")
            .changed()
        {
            self.nature_set_enabled(enabled);
        }
        if !self.nature.enabled {
            return;
        }
        let mut operation = self.nature.operation;
        ui.horizontal(|ui| {
            ui.selectable_value(&mut operation, NatureOperation::Plant, "Plant");
            ui.selectable_value(&mut operation, NatureOperation::Remove, "Remove");
        });
        if operation != self.nature.operation {
            self.nature_set_operation(operation);
        }
        ui.add(egui::Slider::new(&mut self.nature.brush_size, 1..=16).text("Brush size (tiles)"));
        if self.nature.operation == NatureOperation::Plant {
            ui.horizontal(|ui| {
                ui.label("Density:");
                for (density, label) in [
                    (NatureDensity::Sparse, "Sparse"),
                    (NatureDensity::Normal, "Normal"),
                    (NatureDensity::Dense, "Dense"),
                ] {
                    ui.selectable_value(&mut self.nature.density, density, label);
                }
            });
            ui.checkbox(&mut self.nature.mix_species, "Mix species in category");
        }
        if self.nature.data.is_none() {
            if let Err(e) = self.ensure_nature_data() {
                ui.colored_label(Color32::from_rgb(240, 80, 80), format!("Unavailable: {e}"));
                return;
            }
        }
        ui.checkbox(
            &mut self.nature.show_other_regions,
            "Show other regions' objects",
        );
        let categories: Vec<NatureCategory> = self
            .nature
            .data
            .as_ref()
            .map(|d| {
                NatureCategory::ALL
                    .into_iter()
                    .filter(|c| d.catalog.iter().any(|i| i.category() == *c))
                    .collect()
            })
            .unwrap_or_default();
        egui::ComboBox::from_label("Category")
            .selected_text(self.nature.category.map_or("All", |c| c.display_name(true)))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.nature.category, None, "All");
                for category in categories {
                    ui.selectable_value(
                        &mut self.nature.category,
                        Some(category),
                        category.display_name(true),
                    );
                }
            });
        let palette = self.nature_palette();
        let selected = self.nature_selected_item(&palette).map(|i| i.type_id);
        egui::ScrollArea::vertical()
            .id_source("nature_types")
            .max_height(160.0)
            .show(ui, |ui| {
                for item in &palette {
                    if ui
                        .selectable_label(selected == Some(item.type_id), &item.name)
                        .clicked()
                    {
                        self.nature.selected_type = Some(item.type_id);
                    }
                }
            });
        let (planted, removed) = self.nature_pending_counts();
        ui.small(format!(
            "Pending: +{planted} planted, -{removed} existing removed."
        ));
        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.nature.session.can_undo() || !self.settlement_undo.is_empty(), egui::Button::new("Undo"))
                .clicked()
            {
                self.nature.last_action = true;
                self.navigate_panel(crate::undo_journal::SubsystemId::Nature, false);
            }
            if ui
                .add_enabled(self.nature.session.can_redo() || !self.settlement_redo.is_empty(), egui::Button::new("Redo"))
                .clicked()
            {
                self.nature.last_action = true;
                self.navigate_panel(crate::undo_journal::SubsystemId::Nature, true);
            }
            if ui
                .add_enabled(self.nature.session.is_dirty(), egui::Button::new("Discard"))
                .clicked()
            {
                self.nature_discard();
            }
        });
        ui.small("Drag on the map. Remove never touches script-linked or non-landscape objects. Save with Safe Save.");
    }
}

/// Shared with the C# editor: `%APPDATA%\AgainstRomeModifier\settings.json`.
fn app_settings_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|appdata| {
        PathBuf::from(appdata)
            .join("AgainstRomeModifier")
            .join("settings.json")
    })
}

/// Picks the English or Traditional Chinese text for the current app language.
pub fn tr(lang: AppLanguage, en: &'static str, zh: &'static str) -> &'static str {
    if lang == AppLanguage::English {
        en
    } else {
        zh
    }
}

// ---------------------------------------------------------------------------------------------
// Undo / Redo routing, Reset Terrain and Flat Terrain (C# `MapEditorForm.Terrain.cs`)
// ---------------------------------------------------------------------------------------------

impl EditorApp {
    /// True when any history the toolbar Undo button / Ctrl+Z can act on exists.
    pub fn can_undo_any(&self) -> bool {
        !self.settlement_undo.is_empty()
            || (self.spawn_ui.is_active() && self.spawn_ui.session().can_undo())
            || (self.nature.last_action && self.nature.session.can_undo())
            || (self.active_tool == ActiveTool::Material && self.textures.can_material_undo())
            || self.session.as_ref().is_some_and(|s| s.can_undo())
    }

    pub fn can_redo_any(&self) -> bool {
        !self.settlement_redo.is_empty()
            || (self.spawn_ui.is_active() && self.spawn_ui.session().can_redo())
            || (self.nature.last_action && self.nature.session.can_redo())
            || (self.active_tool == ActiveTool::Material && self.textures.can_material_redo())
            || self.session.as_ref().is_some_and(|s| s.can_redo())
    }

    /// Ctrl+Z and the toolbar Undo: spawn placement, nature brush, Material strokes (only while
    /// the Material tool is active), then the terrain history.
    pub fn perform_undo(&mut self) {
        if self.settlement_history(false) { return; }
        if self.spawn_ui.is_active() && self.spawn_ui.session().can_undo() {
            let read_only = self.current_map_is_original();
            let cx = Self::spawn_cx(&self.session, self.loc.current_language(), self.height_step, read_only);
            self.spawn_ui.undo(&mut self.scenario, &cx);
        } else if self.nature_undo() {
        } else if self.active_tool == ActiveTool::Material && self.textures.material_undo() {
            self.status_message = "Material undo performed.".into();
            self.status_is_error = false;
        } else if let Some(session) = &mut self.session {
            if session.undo() {
                self.is_painting = false;
                self.stroke_path.clear();
                self.texture_dirty = true;
                self.status_message = "Undo performed.".to_string();
                self.status_is_error = false;
            }
        }
    }

    /// Ctrl+Y and the toolbar Redo (same routing as [`Self::perform_undo`]).
    pub fn perform_redo(&mut self) {
        if self.settlement_history(true) { return; }
        if self.spawn_ui.is_active() && self.spawn_ui.session().can_redo() {
            let read_only = self.current_map_is_original();
            let cx = Self::spawn_cx(&self.session, self.loc.current_language(), self.height_step, read_only);
            self.spawn_ui.redo(&mut self.scenario, &cx);
        } else if self.nature_redo() {
        } else if self.active_tool == ActiveTool::Material && self.textures.material_redo() {
            self.status_message = "Material redo performed.".into();
            self.status_is_error = false;
        } else if let Some(session) = &mut self.session {
            if session.redo() {
                self.is_painting = false;
                self.stroke_path.clear();
                self.texture_dirty = true;
                self.status_message = "Redo performed.".to_string();
                self.status_is_error = false;
            }
        }
    }

    /// C# `_resetTerrainButton.Enabled`: custom map with unsaved terrain, material or reset state.
    pub fn can_reset_terrain(&self) -> bool {
        !self.current_map_is_original()
            && (self.session.as_ref().is_some_and(|s| s.is_dirty())
                || self.textures.is_dirty()
                || self.reset_auxiliary_layers
                || self.emboss.as_ref().is_some_and(|e| e.session.is_relight_pending()))
    }

    /// C# `_blankTerrainButton.Enabled`: any editable (custom) map with a terrain session.
    pub fn can_apply_flat_terrain(&self) -> bool {
        self.session.is_some() && !self.current_map_is_original()
    }

    /// Opens the confirm dialog of a bulk terrain action (no-op when the action is unavailable).
    pub fn request_terrain_action(&mut self, action: TerrainBulkAction) {
        let allowed = match action {
            TerrainBulkAction::Reset => self.can_reset_terrain(),
            TerrainBulkAction::Flat => self.can_apply_flat_terrain(),
        };
        if allowed {
            self.terrain_confirm = Some(action);
        }
    }

    /// C# `ResetTerrain` after confirmation: drops unsaved heights, collision, textures, nature
    /// edits and the pending lighting / auxiliary-layer reset. Like C#, this is the discard itself
    /// and therefore leaves no undo step.
    pub fn reset_terrain_now(&mut self) -> bool {
        let lang = self.loc.current_language();
        if self.current_map_is_original() {
            self.set_status(
                tr(lang, "Original maps are read-only.", "原版地圖唯讀。").into(),
                true,
            );
            return false;
        }
        self.nature_end_stroke();
        self.textures.end_material_stroke();
        if let Some(session) = self.session.as_mut() {
            session.cancel_stroke();
            session.reset_to_baseline();
        }
        if let Some(emboss) = self.emboss.as_mut() {
            emboss.session.reset_to_baseline();
        }
        self.reset_auxiliary_layers = false;
        self.nature_discard();
        self.textures.reset_to_baseline();
        self.reset_journal_epoch();
        self.clear_recovery();
        self.is_painting = false;
        self.stroke_path.clear();
        self.texture_dirty = true;
        self.navigation.invalidate();
        self.set_status(
            tr(
                lang,
                "Unsaved terrain changes discarded (heights, passability, materials).",
                "已放棄尚未儲存的地表變更（材質、高度、通行區域）。",
            )
            .into(),
            false,
        );
        true
    }

    /// 空白地形：高度設為略高於水面，填滿基底材質、清除阻擋與可移除地景，
    /// 並於下次儲存重設光照、頂點色彩及平滑；保留聚落與腳本。
    /// 高度、碰撞、材質、自然物件與輔助圖層由統一 journal 合為一步。
    pub fn apply_flat_terrain_now(&mut self) -> bool {
        let lang = self.loc.current_language();
        if self.session.is_none() {
            return false;
        }
        if self.current_map_is_original() {
            self.set_status(
                tr(lang, "Original maps are read-only; copy to a custom map first.", "原版地圖唯讀；請先複製為自訂地圖。").into(),
                true,
            );
            return false;
        }
        self.nature_end_stroke();
        self.textures.end_material_stroke();
        self.finish_history_gestures();
        // 先載入相依 session 並消費 DATA 載入障壁，避免抹除本次操作的收據。
        if let Some(path) = self.loaded_map_path.clone() {
            if !self.placement.is_loaded() && !self.placement.is_dirty() {
                let _ = self.placement.load_session(&path);
            }
            if self.textures.session_textures().is_none() {
                let _ = self.textures.load_session(&path);
            }
        }
        self.collect_pilot_receipts();
        let compound_start = self.undo_journal.checkpoint();
        let auxiliary_before = (self.reset_auxiliary_layers, self.emboss.clone());
        // Heights: water level + 20 (C# `Math.Round(water) + 20`, banker's rounding).
        let level = ((self.water_surface.round_ties_even() as i32).saturating_add(20)).clamp(0, 255) as u8;
        if let Some(session) = self.session.as_mut() {
            session.commit_stroke();
            session.core_mut().transform_heights(|_, _, _| f32::from(level));
            let size = session.collision_size as f32;
            let _ = session.paint_collision(size / 2.0, size / 2.0, size, CollisionBrushOp::Clear);
            session.commit_stroke();
        }
        if let Some(emboss) = self.emboss.as_mut() {
            emboss.session.apply_blank_terrain();
        }
        // 相依 session 已在複合操作前嘗試載入。
        let material = self.textures.apply_blank_material();
        self.reset_auxiliary_layers = true;
        // Scenery: removable nature objects and pending additions (needs the DATA session).
        let mut scenery = tr(lang, "scenery cleared", "地景已清除");
        match self.ensure_nature_data() {
            Ok(()) => {
                let nature = &mut self.nature;
                if let Some(data) = nature.data.as_ref() {
                    against_rome_environment::apply_blank_terrain_nature_clearance(&data.level, &mut nature.session);
                    nature.last_action = true;
                }
            }
            Err(_) => scenery = tr(lang, "scenery not cleared (object data unavailable)", "地景未清除（物件資料不可用）"),
        }
        self.is_painting = false;
        self.stroke_path.clear();
        self.texture_dirty = true;
        self.navigation.invalidate();
        let material_note = match &material {
            Some(id) => format!("{} {id}", tr(lang, "material", "材質")),
            None => tr(lang, "no material layer", "無材質層").to_string(),
        };
        self.collect_pilot_receipts();
        if auxiliary_before.0 != self.reset_auxiliary_layers
            || auxiliary_before.1.as_ref().map(|e| e.session.is_relight_pending())
                != self.emboss.as_ref().map(|e| e.session.is_relight_pending())
        {
            let before_token = self.auxiliary_undo.len() as u64;
            self.auxiliary_undo.push(auxiliary_before);
            self.auxiliary_redo.clear();
            self.undo_journal.record_compound(vec![crate::undo_journal::SubsystemStep {
                subsystem: crate::undo_journal::SubsystemId::Auxiliary,
                before_token,
                after_token: before_token + 1,
            }]);
        }
        if let Err(reason) = self.undo_journal.compound_since(compound_start) {
            self.set_status(format!("Blank terrain history barrier / 空白地形已清除統一歷程：{reason}"), false);
            return true;
        }
        self.set_status(
            format!(
                "{} ({material_note}; {scenery}). {}",
                tr(
                    lang,
                    "Flat terrain prepared; settlements and scripts are kept",
                    "已準備平坦地形，聚落與腳本保留",
                ),
                tr(lang, "Shape the terrain, then Save.", "完成地形編輯後按「儲存」。"),
            ),
            false,
        );
        true
    }

    /// Confirmation dialog for the bulk terrain actions (C# `MessageBox.Show(... YesNo)`).
    fn show_terrain_confirm(&mut self, ctx: &egui::Context) {
        let Some(action) = self.terrain_confirm else {
            return;
        };
        let lang = self.loc.current_language();
        let (title, message) = match action {
            TerrainBulkAction::Reset => (
                tr(lang, "Reset Terrain", "還原地表"),
                tr(
                    lang,
                    "Discard unsaved terrain changes (textures, heights, passability)?\nMap metadata, environment settings and scene objects will not be affected.",
                    "要放棄這次尚未儲存的地表變更（材質、高度、通行區域）嗎？\n地圖名稱、環境設定與場景物件不會受影響。",
                ),
            ),
            TerrainBulkAction::Flat => (
                tr(lang, "Reset Flat Terrain", "重設平坦地形"),
                tr(
                    lang,
                    "Flatten the whole terrain?\nHeights, ground material, blocked areas, vertex colors, smoothing and lighting are reset when you Save. Removable scenery is cleared; settlements and scripts are kept. You can still use \"Reset Terrain\" before saving.",
                    "要整平整張地形嗎？\n高度、地表材質、阻擋區、頂點色、平滑遮罩與光照會在儲存時重設，可移除地景會清除；聚落與腳本保留。儲存前仍可用「還原地表」放棄。",
                ),
            ),
        };
        let mut decision = None;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(message);
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, "Yes", "是")).clicked() {
                        decision = Some(true);
                    }
                    if ui.button(tr(lang, "No", "否")).clicked() {
                        decision = Some(false);
                    }
                });
            });
        if let Some(yes) = decision {
            self.terrain_confirm = None;
            if yes {
                match action {
                    TerrainBulkAction::Reset => {
                        self.reset_terrain_now();
                    }
                    TerrainBulkAction::Flat => {
                        self.apply_flat_terrain_now();
                    }
                }
            }
        }
    }
}

/// 編輯器圖塊為 256 × 256；每格 64 世界單位，邊界歸最後一格。
fn cursor_tile(x: f32, z: f32) -> (i32, i32) {
    (((x / 64.0).floor() as i32).clamp(0, 255),
     ((z / 64.0).floor() as i32).clamp(0, 255))
}
/// Editor-wide keyboard shortcuts (C# `HandleShortcut` / `HandlePlacementWorkflowKey`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShortcutAction {
    Save,
    TerrainTool(ActiveTool),
    BrushRadius(f32),
    DesignCheck,
    Delete,
    SelectAll,
    Duplicate,
    Nudge { dx: f32, dz: f32 },
    CanvasZoom(f32),
    CanvasPan { dx: f32, dy: f32 },
    CanvasResetView,
    Eyedropper,
}

/// Keys polled each frame for [`shortcut_action`].
const SHORTCUT_KEYS: [egui::Key; 24] = [
    egui::Key::Num1, egui::Key::Num2, egui::Key::Num3,
    egui::Key::Num4, egui::Key::Num5, egui::Key::Num6,
    egui::Key::Num7, egui::Key::Num8, egui::Key::Num9,
    egui::Key::OpenBracket, egui::Key::CloseBracket, egui::Key::F5,
    egui::Key::S,
    egui::Key::A,
    egui::Key::D,
    egui::Key::I,
    egui::Key::Delete,
    egui::Key::ArrowLeft,
    egui::Key::ArrowRight,
    egui::Key::ArrowUp,
    egui::Key::ArrowDown,
    egui::Key::PageUp,
    egui::Key::PageDown,
    egui::Key::Home,
];

/// Pure key -> action mapping. `text_focus` (a text field wants the keyboard) suppresses everything
/// except Ctrl+S. Nudge is Ctrl+Arrow, 256 units (64 with Shift), Up/Left negative as in C#.
pub fn shortcut_action(
    key: egui::Key,
    ctrl: bool,
    shift: bool,
    alt: bool,
    text_focus: bool,
) -> Option<ShortcutAction> {
    use egui::Key;
    if alt {
        return None;
    }
    if ctrl && key == Key::S {
        return Some(ShortcutAction::Save);
    }
    if text_focus {
        return None;
    }
    if !ctrl && !shift {
        let action = match key {
            Key::Num1 => Some(ShortcutAction::TerrainTool(ActiveTool::HeightRaise)),
            Key::Num2 => Some(ShortcutAction::TerrainTool(ActiveTool::HeightLower)),
            Key::Num3 => Some(ShortcutAction::TerrainTool(ActiveTool::HeightFlatten)),
            Key::Num4 => Some(ShortcutAction::TerrainTool(ActiveTool::HeightSmooth)),
            Key::Num5 => Some(ShortcutAction::TerrainTool(ActiveTool::HeightRoughen)),
            Key::Num6 => Some(ShortcutAction::TerrainTool(ActiveTool::Water)),
            Key::Num7 => Some(ShortcutAction::TerrainTool(ActiveTool::CollisionBlock)),
            Key::Num8 => Some(ShortcutAction::TerrainTool(ActiveTool::CollisionClear)),
            Key::Num9 => Some(ShortcutAction::TerrainTool(ActiveTool::Material)),
            Key::I => Some(ShortcutAction::Eyedropper),
            Key::OpenBracket => Some(ShortcutAction::BrushRadius(-1.0)),
            Key::CloseBracket => Some(ShortcutAction::BrushRadius(1.0)),
            Key::F5 => Some(ShortcutAction::DesignCheck),
            Key::PageUp => Some(ShortcutAction::CanvasZoom(1.15)),
            Key::PageDown => Some(ShortcutAction::CanvasZoom(1.0 / 1.15)),
            Key::Home => Some(ShortcutAction::CanvasResetView),
            _ => None,
        };
        if action.is_some() { return action; }
    }
    match key {
        Key::A if ctrl => Some(ShortcutAction::SelectAll),
        Key::D if ctrl => Some(ShortcutAction::Duplicate),
        Key::Delete if !ctrl && !shift => Some(ShortcutAction::Delete),
        Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown if ctrl => {
            let step = if shift { 64.0 } else { 256.0 };
            Some(match key {
                Key::ArrowLeft => ShortcutAction::Nudge { dx: -step, dz: 0.0 },
                Key::ArrowRight => ShortcutAction::Nudge { dx: step, dz: 0.0 },
                Key::ArrowUp => ShortcutAction::Nudge { dx: 0.0, dz: -step },
                _ => ShortcutAction::Nudge { dx: 0.0, dz: step },
            })
        }
        Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown if !ctrl && shift => {
            let step = 256.0;
            Some(match key {
                Key::ArrowLeft => ShortcutAction::CanvasPan { dx: step, dy: 0.0 },
                Key::ArrowRight => ShortcutAction::CanvasPan { dx: -step, dy: 0.0 },
                Key::ArrowUp => ShortcutAction::CanvasPan { dx: 0.0, dy: step },
                _ => ShortcutAction::CanvasPan { dx: 0.0, dy: -step },
            })
        }
        _ => None,
    }
}

impl EditorApp {
    /// Game root (the folder holding `MAPS`) of the loaded map, if it lives in one.
    fn troop_game_root(&self) -> Option<PathBuf> {
        let map = Path::new(self.map_path_input.trim());
        let root = map.parent()?.parent()?;
        root.join("MAPS").is_dir().then(|| root.to_path_buf())
    }

    /// Keeps `viewport_3d.troop_objects` in sync with the scenario spawns: loads the squad catalog
    /// and `formdef.dau` once per game root on a worker thread, then re-expands whenever a spawn
    /// changes. Display only; never touches the scenario, SDL or DATA.
    fn refresh_troop_objects(&mut self, ctx: &egui::Context) {
        use std::hash::{Hash, Hasher};
        if !self.show_objects {
            self.viewport_3d.troop_objects.clear();
            self.troops.key = None;
            return;
        }
        let game = self.troop_game_root();
        if game != self.troops.game {
            self.troops = TroopDisplay { game: game.clone(), ..Default::default() };
            self.viewport_3d.troop_objects.clear();
            if let Some(game) = game {
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let loaded = TroopCatalog::from_game(&game)
                        .map(|catalog| (catalog, load_formation_definition(&game)));
                    let _ = tx.send(loaded);
                });
                self.troops.loading = Some(rx);
            }
        }
        if let Some(rx) = self.troops.loading.take() {
            match rx.try_recv() {
                Ok(Ok(data)) => self.troops.data = Some(data),
                Ok(Err(_)) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {}
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    self.troops.loading = Some(rx);
                    ctx.request_repaint_after(std::time::Duration::from_millis(250));
                }
            }
        }
        let Some((catalog, definition)) = &self.troops.data else {
            return;
        };
        let spawns = &self.scenario.document().spawns;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for spawn in spawns {
            spawn.alias.hash(&mut hasher);
            [spawn.x, spawn.y, spawn.z].map(f32::to_bits).hash(&mut hasher);
            (spawn.team, spawn.count, spawn.angle).hash(&mut hasher);
        }
        let key = hasher.finish();
        if self.troops.key != Some(key) {
            self.viewport_3d.troop_objects = expand_troops_for_3d(spawns, catalog, definition);
            self.troops.key = Some(key);
        }
    }
}

/// Minimum on-screen cell size (px) for the 2D grid (C# `cellWidth >= 6`).
const GRID_MIN_CELL_PX: f32 = 6.0;

/// Grid line segments over the map rectangle: `dimension + 1` vertical and horizontal lines, each as
/// a shadow line (1 px right/below) plus a light line, or nothing when cells are too small to read.
pub(crate) fn canvas_grid_lines(map_rect: Rect, dimension: usize) -> Vec<([Pos2; 2], bool)> {
    if dimension == 0 {
        return Vec::new();
    }
    let cell_w = map_rect.width() / dimension as f32;
    let cell_h = map_rect.height() / dimension as f32;
    if !(cell_w >= GRID_MIN_CELL_PX && cell_h >= GRID_MIN_CELL_PX) {
        return Vec::new();
    }
    let mut lines = Vec::with_capacity((dimension + 1) * 4);
    for i in 0..=dimension {
        let px = map_rect.min.x + i as f32 * cell_w;
        let py = map_rect.min.y + i as f32 * cell_h;
        // `true` marks the dark shadow line, `false` the light highlight (C# draws both).
        lines.push(([Pos2::new(px + 1.0, map_rect.top()), Pos2::new(px + 1.0, map_rect.bottom())], true));
        lines.push(([Pos2::new(px, map_rect.top()), Pos2::new(px, map_rect.bottom())], false));
        lines.push(([Pos2::new(map_rect.left(), py + 1.0), Pos2::new(map_rect.right(), py + 1.0)], true));
        lines.push(([Pos2::new(map_rect.left(), py), Pos2::new(map_rect.right(), py)], false));
    }
    lines
}

fn draw_canvas_grid(painter: &egui::Painter, map_rect: Rect, dimension: usize) {
    let shadow = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0, 0, 0, 70));
    let light = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 55));
    for (points, is_shadow) in canvas_grid_lines(map_rect, dimension) {
        painter.line_segment(points, if is_shadow { shadow } else { light });
    }
}

/// C# `TerrainLayerFiles.EncodeWithGreen` for a constant layer: pixels whose green already equals
/// `value` keep their colour, every other pixel becomes grey `value`.
fn fill_layer_value(image: &mut crate::bmp::BmpImage, value: u8) {
    for pixel in image.bgr_pixels.chunks_exact_mut(3) {
        if pixel[1] != value {
            pixel.fill(value);
        }
    }
}

/// Retail maps are `ENDL_001..ENDL_004`; custom maps start at slot 5 (C# `IsCustom`).
/// Folders that do not follow the `ENDL_nnn` naming are treated as editable.
pub fn is_original_map_dir(dir: &Path) -> bool {
    // C# gates edits on CustomMapAccess.IsEditableDirectory: any map that lives
    // in a game MAPS folder but is not a registered custom map is original.
    let in_maps_folder = dir
        .parent()
        .and_then(Path::file_name)
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case("MAPS"));
    in_maps_folder && !against_rome_maps::CustomMapAccess::is_editable_directory(dir)
}

impl EditorApp {
    /// While the AI planner is open, load the map's materials on demand (like Flat Terrain) and
    /// offer the Materials specialist only catalog ids that can border every material already in
    /// the edit area: AI strokes are protected and cannot auto-bridge, so others always fail.
    pub(crate) fn sync_ai_materials(&mut self) {
        if !self.ai_planner.open {
            return;
        }
        if self.textures.session_textures().is_none() {
            if let Some(path) = self.loaded_map_path.clone() {
                let _ = self.textures.load_session(&path);
            }
        }
        let key = format!(
            "{}|{}|{}",
            self.ai_planner.bounds_text,
            self.textures.material_revision(),
            self.textures.session_textures().is_some()
        );
        if key == self.ai_materials_key {
            return;
        }
        self.ai_materials_key = key;
        let options = match self.ai_planner.parse_scope() {
            Ok(scope) => {
                let b = *scope.bounds();
                let clamp = |v: i32| v.max(0) as usize;
                self.textures
                    .palette_materials_blendable_in(clamp(b.x), clamp(b.y), clamp(b.width), clamp(b.height))
            }
            Err(_) => Vec::new(),
        };
        self.ai_planner.material_options = options
            .iter()
            .map(|m| against_rome_ai_planner::AiMaterialOption::new(&m.id, &m.name_en))
            .collect();
    }

    pub(crate) fn lang(&self) -> AppLanguage {
        self.loc.current_language()
    }

    fn set_app_language(&mut self, language: AppLanguage) {
        self.loc.set_language(language);
        self.ambience.app_language = language;
    }

    fn current_map_is_original(&self) -> bool {
        is_original_map_dir(Path::new(self.map_path_input.trim()))
    }

    fn set_status(&mut self, message: String, is_error: bool) {
        self.status_message = message;
        self.status_is_error = is_error;
    }

    /// Per-frame data for the spawn placement panel (borrows only the terrain session).
    fn spawn_cx(
        session: &Option<TerrainSession>,
        lang: AppLanguage,
        height_step: f32,
        read_only: bool,
    ) -> SpawnContext<'_> {
        SpawnContext {
            lang,
            heights: session.as_ref().map(|s| s.heights()),
            vertex_size: session.as_ref().map_or(0, |s| s.vertex_size),
            height_step,
            read_only,
        }
    }

    /// Scenario spawn placement window (C# placement tab); edits flow into the scenario document.
    fn show_spawn_panel(&mut self, ctx: &egui::Context) {
        let map_path = if self.map_path_input.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(self.map_path_input.trim()))
        };
        let read_only = is_original_map_dir(map_path.as_deref().unwrap_or(Path::new("")));
        self.refresh_spawn_landscape(map_path.as_deref());
        let cx = Self::spawn_cx(&self.session, self.loc.current_language(), self.height_step, read_only);
        self.spawn_ui.show(ctx, &mut self.scenario, &cx, map_path.as_deref());
        if let Some((team, x, z)) = self.spawn_ui.take_starter_request() {
            self.apply_starter_kit(team, x, z);
        }
    }

    /// Layout > Starter kit (C# `ApplyStarterKit`): stages 1 main house + 4 axemen on the loaded
    /// blank custom map as one spawn undo step; Safe Save imports the house family into DATA.
    /// Nothing changes when any gate or validation fails.
    pub fn apply_starter_kit(&mut self, team: i32, x: f32, z: f32) -> bool {
        let lang = self.loc.current_language();
        let map = PathBuf::from(self.map_path_input.trim());
        let blocked = if self.placement.is_dirty() || self.nature.session.is_dirty() || self.scene.is_dirty() {
            Some(tr(
                lang,
                "Save or discard the pending Objects (DATA), Nature and scene edits before adding the starter kit.",
                "請先儲存或放棄待存的物件 (DATA)、地景與場景編輯，再加入起始配置。",
            ))
        } else if map.as_os_str().is_empty() {
            Some(tr(lang, "Load a custom blank map first.", "請選取自製空白地圖。"))
        } else {
            None
        };
        if let Some(message) = blocked {
            self.spawn_ui.set_status(message, true);
            return false;
        }
        let Some(session) = self.session.as_ref() else {
            self.spawn_ui.set_status(
                tr(lang, "The map has no trusted terrain data; cannot validate the starter kit.", "地圖缺少可信地形高度資料，無法驗證起始配置。"),
                true,
            );
            return false;
        };
        let read_only = is_original_map_dir(&map);
        let terrain = against_rome_maps::spawn_data::StarterKitTerrain {
            heights: session.heights(),
            vertex_size: session.vertex_size,
            height_step: self.height_step,
            collision: session.collision(),
            collision_size: session.collision_size,
            water_world: self.water_surface * self.height_step,
        };
        let cx = Self::spawn_cx(&self.session, lang, self.height_step, read_only);
        self.spawn_ui.apply_starter_kit(&mut self.scenario, &cx, &map, &terrain, team, x, z)
    }

    /// Feeds the spawn panel the landscape objects of the map's DATA (objdef name `Lan*`), minus
    /// pending Nature removals plus pending Nature additions (C# `NatureDisplayObjects`).
    fn refresh_spawn_landscape(&mut self, map: Option<&Path>) {
        if !self.spawn_ui.open {
            return;
        }
        let Some(map) = map else {
            self.spawn_ui.set_landscape(Vec::new());
            return;
        };
        if !matches!(&self.spawn_landscape, Some((cached, _)) if cached == map) {
            let names: HashMap<i32, String> = map
                .parent()
                .and_then(Path::parent)
                .and_then(|game| against_rome_maps::NativeObjectDefinitions::load(game).ok())
                .map(|definitions| {
                    definitions
                        .into_iter()
                        .filter_map(|(id, d)| d.name.filter(|n| !n.is_empty()).map(|n| (id, n)))
                        .collect()
                })
                .unwrap_or_default();
            let objects = if self.placement.loaded_map() == Some(map) {
                self.placement.objects()
            } else {
                against_rome_placement::ObjectStore::load(map)
                    .map(|store| store.objects())
                    .unwrap_or_default()
            };
            self.spawn_landscape =
                Some((map.to_path_buf(), crate::spawn_placement::landscape_points(&objects, &names)));
        }
        let removed = self.nature.session.removed_slots();
        let mut points: Vec<(f32, f32)> = self
            .spawn_landscape
            .iter()
            .flat_map(|(_, base)| base.iter())
            .filter(|(slot, _, _)| !removed.contains(slot))
            .map(|&(_, x, z)| (x, z))
            .collect();
        points.extend(self.nature.session.additions().iter().map(|a| (a.x, a.z)));
        self.spawn_ui.set_landscape(points);
    }

    /// 2D canvas gestures of the spawn panel (outside place mode): a press on a marker selects it
    /// (Ctrl toggles, Shift adds), dragging moves the whole selection as one undo step on release.
    /// A press that misses every marker is left to the active brush.
    fn handle_spawn_canvas(
        ctx: &egui::Context,
        response: &egui::Response,
        map_rect: Rect,
        tool_busy: bool,
        cx: &SpawnContext,
        spawn_ui: &mut SpawnPlacementUi,
        scenario: &mut ScenarioUi,
    ) {
        if !spawn_ui.open || spawn_ui.place_mode() || tool_busy {
            if spawn_ui.is_capturing() {
                spawn_ui.cancel_drag();
            }
            return;
        }
        let world_size = 16384.0_f32;
        let to_world = |p: Pos2| crate::scenario_canvas::world_from_screen(map_rect, p);
        let units_per_pixel = world_size / map_rect.width().max(1.0);
        let (pressed, down, pointer, modifiers) = ctx.input(|i| {
            (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.interact_pos(), i.modifiers)
        });
        if pressed && response.hovered() {
            if let Some(position) = pointer.filter(|p| map_rect.contains(*p)) {
                spawn_ui.canvas_press(
                    to_world(position),
                    10.0 * units_per_pixel,
                    modifiers.ctrl || modifiers.command,
                    modifiers.shift,
                );
            }
        } else if spawn_ui.is_capturing() {
            if down {
                if let Some(position) = pointer {
                    spawn_ui.canvas_drag(to_world(position), 4.0 * units_per_pixel);
                }
            } else {
                spawn_ui.canvas_release(scenario, cx);
            }
        }
    }

    /// C# prebuilt block of `TrySaveMap` for this save: marks the placements that become DATA
    /// objects, then plans the detached DATA candidate (templates, family copies, bindings).
    /// 復原後面板可能已同步為 clean，仍須比較已儲存與目前劇本並同步放置物件。
    fn plan_spawn_data_for_save(
        &mut self,
        map: &Path,
    ) -> Result<Option<against_rome_maps::spawn_data::SpawnDataPlan>, String> {
        self.spawn_ui.sync_with_document(&self.scenario, Some(map));
        self.plan_spawn_data_candidate(map, &self.spawn_ui)
    }

    /// 診斷使用獨立面板快照，避免清除真正面板的選取、草稿或復原歷史。
    fn preview_spawn_data_for_save(
        &self,
        map: &Path,
    ) -> Result<Option<against_rome_maps::spawn_data::SpawnDataPlan>, String> {
        let mut preview = SpawnPlacementUi::default();
        if !self.spawn_ui.catalog().is_empty() && self.loaded_map_path.as_deref() == Some(map) {
            preview.set_catalog(self.spawn_ui.catalog().to_vec());
        }
        preview.sync_with_document(&self.scenario, Some(map));
        self.plan_spawn_data_candidate(map, &preview)
    }

    fn plan_spawn_data_candidate(
        &self,
        map: &Path,
        placements: &SpawnPlacementUi,
    ) -> Result<Option<against_rome_maps::spawn_data::SpawnDataPlan>, String> {
        use against_rome_maps::spawn_data::{mark_prebuilt, SpawnDataCatalog};
        let previous = self.scenario.saved_document()?;
        if !placements.session().is_dirty() && !self.spawn_ui.session().is_dirty() && previous.spawns == self.scenario.document().spawns {
            return Ok(None);
        }
        let mut next = self.scenario.document().clone();
        let placed = placements.session().objects();
        mark_prebuilt(&mut next.spawns, placed);
        if !next.spawns.iter().any(|s| s.prebuilt) && previous.data_slots.is_empty() {
            return Ok(None);
        }
        let catalog = map
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| "the map folder is not inside a game MAPS directory".to_string())
            .and_then(SpawnDataCatalog::from_game);
        let kit = self.spawn_ui.pending_starter_kit();
        let catalog = match catalog {
            Ok(catalog) => catalog,
            // Without game catalogs nothing can be bound to DATA: keep the C#-less script
            // fallback for new buildings, but never edit around existing DATA bindings or a
            // staged starter kit (which must be imported, never silently dropped).
            Err(_) if previous.data_slots.is_empty() && kit.is_none() => return Ok(None),
            Err(e) => return Err(format!("Spawn DATA: game catalogs unavailable: {e}")),
        };
        // Starter kit import (C# `ImportPendingStarterKit`) is validated against the live terrain.
        let session = self.session.as_ref();
        let terrain = session.map(|s| against_rome_maps::spawn_data::StarterKitTerrain {
            heights: s.heights(),
            vertex_size: s.vertex_size,
            height_step: self.height_step,
            collision: s.collision(),
            collision_size: s.collision_size,
            water_world: self.water_surface * self.height_step,
        });
        let pending = match (&kit, &terrain) {
            (Some((kit, ids)), Some(terrain)) => {
                Some(against_rome_maps::spawn_data::PendingStarterKit { kit, ids, terrain })
            }
            (Some(_), None) => return Err("Spawn DATA: starter kit needs trusted terrain data".into()),
            _ => None,
        };
        against_rome_maps::spawn_data::plan_spawn_data_with_kit(map, &catalog, &previous, &next, placed, pending.as_ref())
            .map_err(|e| format!("Spawn DATA: {e}"))
    }

    /// Returns the terrain height sample (0..=255) at the given world coordinates (0..=16384).
    pub fn height_at_world(&self, wx: f32, wz: f32) -> Option<u8> {
        let session = self.session.as_ref()?;
        if session.vertex_size < 2 || session.heights().is_empty() {
            return None;
        }
        let span = (session.vertex_size - 1) as f32;
        let gx = (wx / 16384.0 * span).round().clamp(0.0, span) as usize;
        let gz = (wz / 16384.0 * span).round().clamp(0.0, span) as usize;
        session.heights().get(gz * session.vertex_size + gx).copied()
    }

    /// Formats the active tool name and brush size for the status bar.
    pub fn active_tool_status_text(&self, lang: AppLanguage) -> String {
        let tool_prefix = tr(lang, "Tool", "工具");
        if self.nature.enabled {
            let op_str = match self.nature.operation {
                NatureOperation::Plant => tr(lang, "Nature (Plant)", "自然 (種植)"),
                NatureOperation::Remove => tr(lang, "Nature (Remove)", "自然 (清除)"),
            };
            let radius_label = tr(lang, "Radius", "半徑");
            return format!("{tool_prefix}: {op_str} ({radius_label}: {})", self.nature.brush_size);
        }

        match self.active_tool {
            ActiveTool::Material => {
                let name = tr(lang, "Material", "地表材質");
                let size_label = tr(lang, "Size", "尺寸");
                let size = self.textures.material_brush_tiles();
                format!("{tool_prefix}: {name} ({size_label}: {size})")
            }
            ActiveTool::HeightRaise => {
                let name = tr(lang, "Raise", "升高");
                let r_label = tr(lang, "Radius", "半徑");
                format!("{tool_prefix}: {name} ({r_label}: {:.0})", self.brush_radius)
            }
            ActiveTool::HeightLower => {
                let name = tr(lang, "Lower", "降低");
                let r_label = tr(lang, "Radius", "半徑");
                format!("{tool_prefix}: {name} ({r_label}: {:.0})", self.brush_radius)
            }
            ActiveTool::HeightFlatten => {
                let name = tr(lang, "Flatten", "整平");
                let r_label = tr(lang, "Radius", "半徑");
                format!("{tool_prefix}: {name} ({r_label}: {:.0})", self.brush_radius)
            }
            ActiveTool::HeightSmooth => {
                let name = tr(lang, "Smooth", "平滑");
                let r_label = tr(lang, "Radius", "半徑");
                format!("{tool_prefix}: {name} ({r_label}: {:.0})", self.brush_radius)
            }
            ActiveTool::HeightRoughen => {
                let name = tr(lang, "Roughen", "粗糙");
                let r_label = tr(lang, "Radius", "半徑");
                format!("{tool_prefix}: {name} ({r_label}: {:.0})", self.brush_radius)
            }
            ActiveTool::Water => {
                let name = tr(lang, "Water", "水面");
                let r_label = tr(lang, "Radius", "半徑");
                format!("{tool_prefix}: {name} ({r_label}: {:.0})", self.brush_radius)
            }
            ActiveTool::CollisionBlock => {
                let name = tr(lang, "Block", "阻擋通行");
                let r_label = tr(lang, "Radius", "半徑");
                format!("{tool_prefix}: {name} ({r_label}: {:.0})", self.brush_radius)
            }
            ActiveTool::CollisionClear => {
                let name = tr(lang, "Clear", "清除阻擋");
                let r_label = tr(lang, "Radius", "半徑");
                format!("{tool_prefix}: {name} ({r_label}: {:.0})", self.brush_radius)
            }
        }
    }

    /// Formats cursor world, tile, and elevation readout for the status bar.
    pub fn cursor_readout_text(&self, lang: AppLanguage) -> Option<String> {
        let (wx, wz, tx, tz) = self.canvas_cursor?;
        let prefix = tr(lang, "Cursor", "游標");
        let tile_label = tr(lang, "Tile", "圖塊");
        let base = format!("{prefix}: X: {wx:.0}, Z: {wz:.0} | {tile_label}: ({tx}, {tz})");
        Some(if let Some(h) = self.height_at_world(wx, wz) {
            let height_label = tr(lang, "Height", "高度");
            format!("{base} | {height_label}: {h}")
        } else {
            base
        })
    }

    /// Eyedropper: samples the floor material under the cursor (2D canvas or 3D view)
    /// and switches to the Material tool. Respects shortcuts_blocked().
    pub fn eyedrop_material_at_cursor(&mut self) -> bool {
        if self.shortcuts_blocked() {
            return false;
        }
        let lang = self.lang();
        let english = lang == AppLanguage::English;
        let Some((wx, wz, _, _)) = self.canvas_cursor else {
            let msg = tr(
                lang,
                "Hover cursor over the map to pick material",
                "請將游標移至地圖上方以吸取材質",
            );
            self.set_status(msg.to_string(), false);
            return false;
        };

        let Some(session) = &self.session else {
            let msg = tr(
                lang,
                "No map loaded to pick material from",
                "尚未載入地圖，無法吸取材質",
            );
            self.set_status(msg.to_string(), true);
            return false;
        };

        let span = (session.vertex_size.saturating_sub(1).max(1)) as f32;
        let gx = (wx / 16384.0 * span).clamp(0.0, span);
        let gy = (wz / 16384.0 * span).clamp(0.0, span);

        if self.textures.sample_material_at_grid(gx, gy, span, english) {
            self.active_tool = ActiveTool::Material;
            self.nature.enabled = false;
            let picked_name = self
                .textures
                .active_material_or_stamp()
                .unwrap_or("material");
            let msg = if english {
                format!("Picked material: {picked_name}")
            } else {
                format!("已吸取地表材質: {picked_name}")
            };
            self.set_status(msg, false);
            true
        } else {
            let notice = self.textures.material_notice().map(str::to_string).unwrap_or_else(|| {
                tr(
                    lang,
                    "No material texture data available at this location",
                    "該位置沒有可讀取的地表材質資料",
                ).to_string()
            });
            self.set_status(notice, false);
            false
        }
    }

    /// Dispatches one shortcut. Mutating actions are refused on retail maps like the C# editor.
    pub fn handle_shortcut_action(&mut self, action: ShortcutAction) {
        if self.shortcuts_blocked() { return; }
        let lang = self.lang();
        let read_only = self.current_map_is_original();
        let read_only_msg = tr(
            lang,
            "Original maps are read-only; copy to a custom map first.",
            "原廠地圖為唯讀，無法儲存；請先複製為自製地圖。",
        );
        match action {
            ShortcutAction::TerrainTool(tool) => {
                self.active_tool = tool;
            }
            ShortcutAction::BrushRadius(delta) => {
                if !self.nature.enabled && self.active_tool != ActiveTool::Material {
                    self.brush_radius = (self.brush_radius + delta).clamp(1.0, 64.0);
                }
            }
            ShortcutAction::Eyedropper => {
                self.eyedrop_material_at_cursor();
            }
            ShortcutAction::DesignCheck => self.run_design_check(),
            ShortcutAction::CanvasZoom(factor) => {
                if self.view_mode != ViewMode::View3D {
                    self.zoom = (self.zoom * factor).clamp(0.25, 10.0);
                }
            }
            ShortcutAction::CanvasPan { dx, dy } => {
                if self.view_mode != ViewMode::View3D {
                    self.pan += egui::Vec2::new(dx, dy);
                }
            }
            ShortcutAction::CanvasResetView => {
                if self.view_mode != ViewMode::View3D {
                    self.zoom = 1.0;
                    self.pan = egui::Vec2::ZERO;
                }
            }
            ShortcutAction::Save => {
                if read_only {
                    self.set_status(read_only_msg.into(), true);
                } else {
                    self.request_safe_save();
                }
            }
            ShortcutAction::Delete if self.scene.scene_pick_active() && self.scene.selected_ref().is_some() => {
                if read_only {
                    self.set_status(read_only_msg.into(), true);
                    return;
                }
                if let Err(err) = self.scene.toggle_delete_selected() {
                    self.scene.status = err;
                }
            }
            ShortcutAction::SelectAll if self.spawn_ui.is_active() => {
                self.spawn_ui.select_all();
            }
            ShortcutAction::Delete | ShortcutAction::Duplicate | ShortcutAction::Nudge { .. }
                if self.spawn_ui.is_active() && self.spawn_ui.has_selection() =>
            {
                let read_only = self.current_map_is_original();
                let cx = Self::spawn_cx(&self.session, self.loc.current_language(), self.height_step, read_only);
                self.spawn_ui.handle_shortcut(action, &mut self.scenario, &cx);
            }
            ShortcutAction::SelectAll => {
                if self.placement.is_loaded() {
                    self.placement.select_all();
                }
            }
            ShortcutAction::Delete | ShortcutAction::Duplicate | ShortcutAction::Nudge { .. } => {
                if !self.placement.is_loaded() || self.placement.selected_objects().is_empty() {
                    return;
                }
                if read_only {
                    self.set_status(read_only_msg.into(), true);
                    return;
                }
                let result = match action {
                    ShortcutAction::Delete => self.placement.delete_selection(),
                    ShortcutAction::Duplicate => self.placement.duplicate_selection(),
                    ShortcutAction::Nudge { dx, dz } => self.placement.nudge_selection(dx, dz),
                    _ => Ok(()),
                };
                if let Err(err) = result {
                    self.set_status(format!("Placement shortcut failed: {err}"), true);
                }
            }
        }
    }

    /// "Test in game": saves a dirty custom map first, then launches `Against_Rome.exe`.
    pub fn test_in_game(&mut self) {
        self.test_in_game_using(&mut crate::packaging_ui::spawn_game_process);
    }

    pub fn test_in_game_using(&mut self, spawn: crate::packaging_ui::GameSpawner<'_>) {
        let lang = self.lang();
        let text = self.map_path_input.trim().to_string();
        let map_dir = PathBuf::from(&text);
        let Some(game_dir) = (!text.is_empty())
            .then(|| crate::packaging_ui::game_dir_for_map(&map_dir))
            .flatten()
        else {
            self.set_status(
                tr(
                    lang,
                    "Please select a map to preview.",
                    "請先選擇要預覽的地圖。",
                )
                .into(),
                true,
            );
            return;
        };
        if !self.current_map_is_original() && self.has_unsaved_changes() {
            self.safe_save_current();
            if self.has_unsaved_changes() {
                let msg = tr(
                    lang,
                    "Game not launched: the map could not be saved.",
                    "未啟動遊戲：地圖尚未成功儲存。",
                );
                self.status_message = format!("{msg} {}", self.status_message);
                self.status_is_error = true;
                return;
            }
        }
        match crate::packaging_ui::launch_game(&game_dir, spawn) {
            Ok(_) => {
                let name = map_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let msg = if lang == AppLanguage::English {
                    format!(
                        "Game launched. To test the custom map, enter Endless Mode and select {name}."
                    )
                } else {
                    format!("遊戲已啟動。若要測試自製地圖，請進入「無盡模式」並選擇 {name}。")
                };
                self.set_status(msg, false);
            }
            Err(err) => self.set_status(err, true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::SampleChange;

    mod batch7_tests;
    mod batch10_tests;
    mod save_blockers_tests;
    mod unified_undo_tests;
    mod objectives_apply_tests;
    mod scenario_canvas_tests;
    mod settlement_apply_tests;
    pub(super) mod spawn_save_tests;
    mod starter_kit_tests;

    fn start_camera_app(pfil: bool) -> (EditorApp, PathBuf, Vec<u8>) {
        let dir = temp_map_dir("start_camera");
        std::fs::create_dir_all(dir.join("SCRIPT")).unwrap();
        let raw = against_rome_scenario::BciImage::idle().serialize();
        let original = if pfil {
            {
                let mut pfil = vec![0; 64];
                pfil[..4].copy_from_slice(b"PFIL");
                pfil[16..20].copy_from_slice(&(raw.len() as u32).to_le_bytes());
                for chunk in raw.chunks(8) {
                    pfil.push(((1u16 << chunk.len()) - 1) as u8);
                    pfil.extend_from_slice(chunk);
                }
                pfil
            }
        } else {
            raw
        };
        std::fs::write(dir.join("SCRIPT/ak_level.bci"), &original).unwrap();
        let mut app = EditorApp::initial_state();
        app.set_loaded_map(dir.clone(), TerrainSession::create_blank(5, 32))
            .unwrap();
        (app, dir, original)
    }

    #[test]
    fn start_camera_apply_and_repeat_save_disable_preserve_original_raw_and_pfil() {
        for pfil in [false, true] {
            let (mut app, dir, original) = start_camera_app(pfil);
            assert!(!app.campaign.start_camera.enabled);
            let before = save_file_snapshot(&dir);
            app.campaign.start_camera.enabled = true;
            app.campaign.start_camera.x = 12512.0;
            app.apply_start_camera_candidate().unwrap();
            assert_eq!(save_file_snapshot(&dir), before, "Apply is memory only");
            assert!(app.has_unsaved_changes());
            assert!(app.require_clean_map_switch().is_err());
            app.safe_save_current();
            assert!(!app.status_is_error, "{}", app.status_message);
            assert!(!app.has_unsaved_changes());
            let script = std::fs::read(dir.join("SCRIPT/ak_level.bci")).unwrap();
            assert_eq!(script.starts_with(b"PFIL"), pfil);
            assert_ne!(script, original);
            assert_eq!(
                std::fs::read(dir.join("SCRIPT/ak_level.arm_original")).unwrap(),
                original
            );
            app.apply_start_camera_candidate().unwrap();
            app.safe_save_current();
            assert!(!app.status_is_error, "{}", app.status_message);
            assert_eq!(
                std::fs::read(dir.join("SCRIPT/ak_level.bci")).unwrap(),
                script
            );
            app.campaign.start_camera.enabled = false;
            app.apply_start_camera_candidate().unwrap();
            app.safe_save_current();
            assert!(!app.status_is_error, "{}", app.status_message);
            assert_eq!(
                std::fs::read(dir.join("SCRIPT/ak_level.bci")).unwrap(),
                original
            );
            assert_eq!(
                std::fs::read(dir.join("SCRIPT/ak_level.arm_original")).unwrap(),
                original
            );
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn start_camera_and_pending_scenario_events_share_transaction_and_reload() {
        let (mut app, dir, original) = start_camera_app(false);
        app.scenario.load_session(&dir).unwrap();
        app.scenario
            .add_events(&[against_rome_scenario::Event {
                name: "Timer".into(),
                delay_seconds: 2,
                actions: vec![against_rome_scenario::Action {
                    text: "Hello".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }])
            .unwrap();
        app.campaign.start_camera.enabled = true;
        app.apply_start_camera_candidate().unwrap();
        app.safe_save_current();
        assert!(!app.status_is_error, "{}", app.status_message);
        assert_eq!(app.scenario.document().events.len(), 1);
        assert!(!app.scenario.dirty());
        let script = std::fs::read(dir.join("SCRIPT/ak_level.bci")).unwrap();
        let camera = against_rome_scenario::StartCamera::from(&app.campaign.start_camera);
        let expected = against_rome_scenario::compile_script_verified(
            &original,
            app.scenario.document(),
            Some(&camera),
            None,
            None,
        )
        .unwrap();
        assert_eq!(script, expected);
        let mut reloaded = EditorApp::initial_state();
        reloaded
            .set_loaded_map(dir.clone(), TerrainSession::create_blank(5, 32))
            .unwrap();
        assert_eq!(reloaded.campaign.start_camera, app.campaign.start_camera);
        reloaded.campaign.start_camera.enabled = false;
        reloaded.apply_start_camera_candidate().unwrap();
        reloaded.safe_save_current();
        assert!(!reloaded.status_is_error, "{}", reloaded.status_message);
        assert_eq!(reloaded.scenario.document().events.len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn start_camera_failure_rolls_back_script_metadata_backup_and_keeps_candidate() {
        use against_rome_maps::TransactionFailurePoint as Fail;
        for point in [
            Fail::AfterPath("SCRIPT/ak_level.bci".into()),
            Fail::BeforeFinalCommit,
        ] {
            let (mut app, dir, _) = start_camera_app(false);
            app.campaign.start_camera.enabled = true;
            app.apply_start_camera_candidate().unwrap();
            edit_save_terrain(&mut app);
            let before = save_file_snapshot(&dir);
            app.safe_save_current_using(|tx| {
                let mut injected =
                    std::mem::replace(tx, against_rome_maps::MapSaveTransaction::new("unused"))
                        .with_injected_failure(point, "camera rollback test");
                injected.commit().map_err(|e| e.to_string())
            });
            assert!(app.status_is_error, "{}", app.status_message);
            assert_eq!(save_file_snapshot(&dir), before);
            assert!(app.campaign.camera_pending());
            assert!(app.session.as_ref().unwrap().is_dirty());
            app.safe_save_current();
            assert!(!app.status_is_error, "{}", app.status_message);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn start_camera_rejects_stale_sources_draft_wrong_target_and_other_pending_work() {
        for source in [
            "SCRIPT/ak_level.bci",
            "SCRIPT/ak_level.arm_original",
            "arm_scenario.json",
            "arm_start_camera.json",
            "DATA/objects.dat",
        ] {
            let (mut app, dir, _) = start_camera_app(false);
            app.campaign.start_camera.enabled = true;
            app.apply_start_camera_candidate().unwrap();
            std::fs::create_dir_all(dir.join(source).parent().unwrap()).unwrap();
            std::fs::write(dir.join(source), b"external change").unwrap();
            let before = save_file_snapshot(&dir);
            app.safe_save_current();
            assert!(app.status_is_error, "{}", app.status_message);
            assert_eq!(save_file_snapshot(&dir), before);
            assert!(app.campaign.camera_pending());
            std::fs::remove_dir_all(dir).unwrap();
        }
        let (mut app, dir, _) = start_camera_app(false);
        app.campaign.start_camera.enabled = true;
        app.apply_start_camera_candidate().unwrap();
        let other = temp_map_dir("camera_wrong_target");
        app.map_path_input = other.display().to_string();
        assert!(app.apply_start_camera_candidate().is_err());
        app.safe_save_current();
        assert!(app.status_is_error);
        assert!(save_file_snapshot(&other).is_empty());
        app.map_path_input = dir.display().to_string();
        app.campaign.start_camera.x += 32.0;
        app.safe_save_current();
        assert!(app.status_is_error);
        app.apply_start_camera_candidate().unwrap();
        app.campaign.mark_dirty();
        let before = save_file_snapshot(&dir);
        app.safe_save_current();
        assert!(app.status_is_error && app.campaign.has_unsupported_edits());
        assert_eq!(save_file_snapshot(&dir), before);
        std::fs::remove_dir_all(dir).unwrap();
        std::fs::remove_dir_all(other).unwrap();
    }

    #[test]
    fn start_camera_invalid_apply_preserves_prior_candidate_and_pending_scenario() {
        let (mut app, dir, original) = start_camera_app(false);
        app.campaign.start_camera.enabled = true;
        app.apply_start_camera_candidate().unwrap();
        let before = save_file_snapshot(&dir);
        app.campaign.start_camera.zoom = 10.0;
        assert!(app.apply_start_camera_candidate().is_err());
        assert!(app.campaign.camera_pending());
        app.campaign.start_camera.zoom = 5.0;
        // The old, valid candidate survived the failed Apply.
        app.safe_save_current();
        assert!(!app.status_is_error, "{}", app.status_message);
        // An existing immutable backup must not even be staged for replacement.
        app.apply_start_camera_candidate().unwrap();
        app.safe_save_current_using(|tx| {
            assert!(!tx.has_staged("SCRIPT/ak_level.arm_original"));
            tx.commit().map_err(|e| e.to_string())
        });
        assert!(!app.status_is_error, "{}", app.status_message);
        assert_eq!(
            std::fs::read(dir.join("SCRIPT/ak_level.arm_original")).unwrap(),
            original
        );
        assert_ne!(save_file_snapshot(&dir), before);
        std::fs::remove_dir_all(dir).unwrap();

        let (mut app, dir, _) = start_camera_app(false);
        let mut document = against_rome_scenario::ScenarioDocument::default();
        document.spawns.push(against_rome_scenario::Spawn {
            prebuilt: true,
            alias: "GER_INF01".into(),
            ..Default::default()
        });
        std::fs::write(
            dir.join("arm_scenario.json"),
            serde_json::to_vec(&document).unwrap(),
        )
        .unwrap();
        let before = save_file_snapshot(&dir);
        app.campaign.start_camera.enabled = true;
        assert!(
            app.apply_start_camera_candidate().is_err(),
            "prebuilt slots need verified DATA"
        );
        assert!(!app.campaign.camera_pending());
        assert_eq!(save_file_snapshot(&dir), before);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn start_camera_capture_uses_real_3d_target_and_requires_loaded_3d_view() {
        let mut blank = EditorApp::initial_state();
        blank.view_mode = ViewMode::View3D;
        assert!(blank.current_view_start_camera().is_err());
        assert!(blank.apply_start_camera_candidate().is_err());
        let (mut app, dir, _) = start_camera_app(false);
        assert!(app.current_view_start_camera().is_err());
        app.view_mode = ViewMode::View3D;
        app.viewport_3d.camera.target = against_rome_scene::Vec3::new(12.5, 72.0, 20.0);
        let camera = app.current_view_start_camera().unwrap();
        assert_eq!(
            (camera.x, camera.y, camera.z, camera.zoom),
            (3200.0, 72.0, 5120.0, 5.0)
        );
        app.campaign.capture_camera(camera);
        assert!(app.has_unsaved_changes());
        let before = save_file_snapshot(&dir);
        app.safe_save_current();
        assert!(app.status_is_error, "must Apply first");
        assert_eq!(save_file_snapshot(&dir), before);
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn erosion_plan_for(
        app: &mut EditorApp,
        index: usize,
        before: u8,
        after: u8,
    ) -> crate::landscape_ui::ErosionPlan {
        let heights = app.session.as_ref().unwrap().heights().to_vec();
        let mut plan = app
            .landscape
            .run_erosion_planning(257, Some(&heights))
            .unwrap();
        plan.changes = vec![against_rome_landscape::TerrainSampleChange::new(
            index, before, after,
        )];
        plan
    }

    #[test]
    fn erosion_apply_uses_terrain_undo_history_and_invalidates_derived_views() {
        let mut app = EditorApp::initial_state();
        let plan = erosion_plan_for(&mut app, 0, 32, 91);
        app.handle_erosion_apply(plan);

        let terrain = app.session.as_ref().unwrap();
        assert_eq!(terrain.heights()[0], 91);
        assert!(terrain.can_undo());
        assert!(terrain.is_dirty());
        assert!(app.texture_dirty);
        assert!(app
            .landscape
            .status_message
            .contains("1 height samples modified"));

        assert!(app.session.as_mut().unwrap().undo());
        assert_eq!(app.session.as_ref().unwrap().heights()[0], 32);
    }

    #[test]
    fn erosion_apply_rejects_preview_when_source_height_changed() {
        let mut app = EditorApp::initial_state();
        let plan = erosion_plan_for(&mut app, 0, 32, 91);
        app.session
            .as_mut()
            .unwrap()
            .apply_height_changes(&[SampleChange {
                index: 0,
                before: 32,
                after: 44,
            }]);
        app.session.as_mut().unwrap().commit_stroke();
        let before_dirty = app.session.as_ref().unwrap().is_dirty();

        app.handle_erosion_apply(plan);

        assert_eq!(app.session.as_ref().unwrap().heights()[0], 44);
        assert_eq!(app.session.as_ref().unwrap().is_dirty(), before_dirty);
        assert!(app.landscape.status_is_error);
        assert!(app.landscape.last_erosion_plan.is_some());
    }

    #[test]
    fn two_point_ramp_ui_applies_one_undoable_terrain_operation() {
        let mut terrain = TerrainSession::create_blank(65, 32);
        let start_index = 20 * 65 + 10;
        let end_index = 20 * 65 + 50;
        terrain.apply_height_changes(&[
            SampleChange {
                index: start_index,
                before: 32,
                after: 12,
            },
            SampleChange {
                index: end_index,
                before: 32,
                after: 96,
            },
        ]);
        terrain.commit_stroke();
        terrain.commit_baseline();

        let mut app = EditorApp::initial_state();
        app.session = Some(terrain);
        app.ramp_selecting = true;
        app.handle_ramp_point((10.0, 20.0));
        assert_eq!(app.ramp_start, Some((10.0, 20.0)));
        assert!(!app.session.as_ref().unwrap().can_undo());

        app.handle_ramp_point((50.0, 20.0));

        let terrain = app.session.as_ref().unwrap();
        assert!(!app.ramp_selecting);
        assert!(terrain.is_dirty());
        assert!(terrain.can_undo());
        assert!(app.texture_dirty);
        assert!(app.status_message.contains("Ramp applied"));
        assert!(app.session.as_mut().unwrap().undo());
        assert_eq!(app.session.as_ref().unwrap().heights()[start_index], 12);
        assert_eq!(app.session.as_ref().unwrap().heights()[end_index], 96);
    }

    fn temp_map_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("arm_app_{tag}_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn recent_maps_only_track_successful_loads_and_keep_dirty_switch_guard() {
        let root = temp_map_dir("recent_load");
        let a = root.join("a");
        let b = root.join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let mut app = EditorApp::initial_state();
        app.set_loaded_map(a.clone(), TerrainSession::create_blank(257, 32)).unwrap();
        assert_eq!(app.recent_maps.paths, vec![std::fs::canonicalize(&a).unwrap()]);
        app.ambience.mark_dirty();
        assert!(app.set_loaded_map(b.clone(), TerrainSession::create_blank(257, 32)).is_err());
        assert_eq!(app.recent_maps.paths.len(), 1);
        app.install_loaded_map(b.clone(), TerrainSession::create_blank(257, 32)).unwrap();
        assert_eq!(app.recent_maps.paths[0], std::fs::canonicalize(&b).unwrap());
        std::fs::write(a.join("daynight.bmp"), "invalid").unwrap();
        let before = app.recent_maps.paths.clone();
        assert!(app.install_loaded_map(a, TerrainSession::create_blank(257, 32)).is_err());
        assert_eq!(app.recent_maps.paths, before);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recent_menu_uses_existing_dirty_map_confirmation() {
        let root = temp_map_dir("recent_menu");
        let a = root.join("a");
        let b = root.join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let mut app = EditorApp::initial_state();
        app.set_loaded_map(a.clone(), TerrainSession::create_blank(33, 32)).unwrap();
        app.recent_maps.remember(&b).unwrap();
        app.ambience.mark_dirty();
        let ctx = egui::Context::default();
        gui_click(&mut app, &ctx, "Recent maps");
        let label = std::fs::canonicalize(&b).unwrap().display().to_string();
        gui_click(&mut app, &ctx, &label);
        assert_eq!(app.loaded_map_path.as_deref(), Some(a.as_path()));
        assert!(matches!(&app.pending_action, Some(PendingAction::Load(p)) if *p == std::fs::canonicalize(&b).unwrap()));
        assert!(app.ambience.is_dirty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn safe_save_with_dirty_ambience_success_clears_dirty() {
        let dir = temp_map_dir("save_ok");
        let mut app = EditorApp::initial_state();
        app.map_path_input = dir.to_string_lossy().into_owned();
        app.ambience.mark_dirty();
        app.safe_save_current();
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(app.status_message.starts_with("Saved safely"));
        assert!(!app.ambience.is_dirty());
        assert!(dir.join("boden.ini").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_flat_build_keeps_briefing() {
        let temp_root = temp_map_dir("flat_briefing");
        let src = temp_root.join("MAPS").join("ENDL_001");
        std::fs::create_dir_all(&src).unwrap();
        let mut terrain = TerrainSession::create_blank(257, 50);
        safe_save_map(&src, &mut terrain).unwrap();
        std::fs::write(src.join("level.put"), "var:map_name =\"Template\";\r\n").unwrap();
        std::fs::write(src.join("briefing.put"), "var:briefing_titel_1 =\"Original Title\";\r\n").unwrap();
        let us_dir = src.join("TEXT").join("US");
        std::fs::create_dir_all(&us_dir).unwrap();
        std::fs::write(us_dir.join("briefing.put"), "var:briefing_titel_1 =\"Original Title\";\r\n").unwrap();
        std::fs::write(src.join("map.sdl"), "[settlement]\r\nrefpos=100.0,0.0,100.0\r\n").unwrap();
        std::fs::write(src.join("boden.ini"), "[Dimension]\r\n64\r\n[Waterlevel]\r\n30.0\r\n[Heightmapstep]\r\n2.0\r\n").unwrap();
        let mut boden_txt = String::from("[Dimension]\n64\n\n[Texturen]\n");
        for _ in 0..4096 {
            boden_txt.push_str("GRAS_1\n");
        }
        std::fs::write(src.join("boden.txt"), boden_txt).unwrap();

        let mut app = EditorApp::initial_state();
        app.maps.game_path_input = temp_root.to_string_lossy().to_string();
        app.maps.blank_source_slot = 1;
        app.maps.blank_target_name = "Flat Alpine".to_string();
        app.maps.blank_briefing = "Alpine conquest mission briefing.".to_string();

        let intent = app.maps.request_build_flat_template().expect("flat template intent");
        assert_eq!(
            intent.spec.briefing_text.as_deref(),
            Some("Alpine conquest mission briefing.")
        );

        let (info, briefing_warning) = app
            .build_and_install_flat_template(&intent.game_path, &intent.spec)
            .expect("build flat template");
        assert!(briefing_warning.is_none());
        assert_eq!(app.ambience.briefing_title, "Alpine conquest mission briefing.");
        assert!(!app.ambience.dirty, "success path must leave dirty=false");
        assert!(!app.status_message.contains("Warning:"), "success path must not have warning: {}", app.status_message);

        let saved_put = std::fs::read_to_string(info.directory_path.join("TEXT").join("US").join("briefing.put"))
            .or_else(|_| std::fs::read_to_string(info.directory_path.join("SCRIPT").join("briefing.put")))
            .or_else(|_| std::fs::read_to_string(info.directory_path.join("briefing.put")))
            .expect("read briefing.put");
        assert!(saved_put.contains("Alpine conquest mission briefing."));

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_flat_template_briefing_failure_path_sets_warning_and_dirty() {
        let temp_root = temp_map_dir("flat_briefing_fail");
        let src = temp_root.join("MAPS").join("ENDL_001");
        std::fs::create_dir_all(&src).unwrap();
        let mut terrain = TerrainSession::create_blank(257, 50);
        safe_save_map(&src, &mut terrain).unwrap();
        std::fs::write(src.join("level.put"), "var:map_name =\"Template\";\r\n").unwrap();
        // Do not create TEXT/US/briefing.put so write fails in build_and_install_flat_template
        std::fs::create_dir_all(src.join("TEXT").join("US")).unwrap();
        std::fs::write(src.join("map.sdl"), "[settlement]\r\nrefpos=100.0,0.0,100.0\r\n").unwrap();
        std::fs::write(src.join("boden.ini"), "[Dimension]\r\n64\r\n[Waterlevel]\r\n30.0\r\n[Heightmapstep]\r\n2.0\r\n").unwrap();
        let mut boden_txt = String::from("[Dimension]\n64\n\n[Texturen]\n");
        for _ in 0..4096 {
            boden_txt.push_str("GRAS_1\n");
        }
        std::fs::write(src.join("boden.txt"), boden_txt).unwrap();

        let mut app = EditorApp::initial_state();
        app.maps.game_path_input = temp_root.to_string_lossy().to_string();
        app.maps.blank_source_slot = 1;
        app.maps.blank_target_name = "Flat Alpine Fail".to_string();
        app.maps.blank_briefing = "Alpine conquest mission briefing.".to_string();

        let intent = app.maps.request_build_flat_template().expect("flat template intent");
        let (info, briefing_warning) = app
            .build_and_install_flat_template(&intent.game_path, &intent.spec)
            .expect("map build still succeeds on briefing failure");
        assert!(briefing_warning.is_some());

        assert_eq!(info.id(), "ENDL_005");
        assert_eq!(app.ambience.briefing_title, "Alpine conquest mission briefing.");
        assert!(app.ambience.dirty, "failed file write must keep ambience.dirty = true");
        assert!(
            app.status_message.contains("Warning") && app.status_message.contains("警告"),
            "status bar warning must be reported: {}",
            app.status_message
        );

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_perform_action_flat_template_clears_stale_warning() {
        let temp_root = temp_map_dir("flat_stale_warn");
        let src = temp_root.join("MAPS").join("ENDL_001");
        std::fs::create_dir_all(&src).unwrap();
        let mut terrain = TerrainSession::create_blank(257, 50);
        safe_save_map(&src, &mut terrain).unwrap();
        std::fs::write(src.join("level.put"), "var:map_name =\"Template\";\r\n").unwrap();
        std::fs::create_dir_all(src.join("TEXT").join("US")).unwrap();
        std::fs::write(src.join("TEXT").join("US").join("briefing.put"), "var:briefing_titel_1=\"Original\"\r\n").unwrap();
        std::fs::write(src.join("map.sdl"), "[settlement]\r\nrefpos=100.0,0.0,100.0\r\n").unwrap();
        std::fs::write(src.join("boden.ini"), "[Dimension]\r\n64\r\n[Waterlevel]\r\n30.0\r\n[Heightmapstep]\r\n2.0\r\n").unwrap();
        let mut boden_txt = String::from("[Dimension]\n64\n\n[Texturen]\n");
        for _ in 0..4096 {
            boden_txt.push_str("GRAS_1\n");
        }
        std::fs::write(src.join("boden.txt"), boden_txt).unwrap();

        let mut app = EditorApp::initial_state();
        app.maps.game_path_input = temp_root.to_string_lossy().to_string();
        app.maps.blank_source_slot = 1;
        app.maps.blank_target_name = "Flat Stale Test".to_string();
        // Stale warning from an earlier unrelated action
        app.status_message = "Warning: Some stale unrelated warning from earlier / 警告：先前不相干的舊警告".to_string();

        let intent = app.maps.request_build_flat_template().expect("flat template intent");
        let ctx = egui::Context::default();
        app.perform_action(PendingAction::BuildBlank(intent), &ctx);

        assert!(!app.status_is_error);
        assert!(app.status_message.contains("created and opened; terrain flattened (unsaved)"));
        assert!(!app.status_message.contains("stale unrelated warning"));

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_map_properties_updates_ambience_and_safe_save() {
        let dir = temp_map_dir("map_props_save");
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.loaded_map_path = Some(dir.clone());
        app.map_path_input = dir.to_string_lossy().into_owned();
        app.map_properties_open = true;

        // Render Map Properties window using egui context to verify UI integration
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            app.show_map_properties(ctx);
        });

        // Edit title and briefing in Map properties: reuses existing Ambience state
        app.ambience.briefing_title = "Prop Window Title".to_string();
        app.ambience.briefing_body = "Prop Window Description".to_string();
        app.ambience.dirty = true;

        assert!(app.ambience.is_dirty(), "dirty flag must be set when editing properties");

        app.safe_save_current();
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(!app.ambience.is_dirty(), "save clears dirty flag");

        // Verify native location TEXT/US/briefing.put was written with the updated title and body
        let briefing_path = dir.join("TEXT").join("US").join("briefing.put");
        assert!(briefing_path.is_file(), "TEXT/US/briefing.put must be saved to disk");
        let put_doc = against_rome_maps::text_docs::PutTextDocument::load(&briefing_path).unwrap();
        assert_eq!(put_doc.get_value("briefing_titel_1").as_deref(), Some("Prop Window Title"));
        assert_eq!(put_doc.get_value("briefing_text").as_deref(), Some("Prop Window Description"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_map_properties_center_start_camera_on_player_main_house() {
        let (mut app, dir, _) = start_camera_app(false);
        app.campaign.start_camera.enabled = false;
        app.campaign.start_camera.x = 100.0;
        app.campaign.start_camera.z = 200.0;
        assert!(!app.campaign.camera_dirty);

        // When no player main house exists:
        assert_eq!(app.player_main_house_position(), None);
        assert!(!app.center_start_camera_on_player_main_house(), "center button must be disabled/inactive when no house");
        assert_eq!(app.campaign.start_camera.x, 100.0);
        assert_eq!(app.campaign.start_camera.z, 200.0);

        // UI rendering with no main house (disabled button with tooltip):
        app.map_properties_open = true;
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            app.show_map_properties(ctx);
        });

        // Add a team-0 main house spawn:
        let spawn = against_rome_scenario::Spawn {
            id: uuid::Uuid::new_v4(),
            alias: "GER_HAU00".into(),
            team: 0,
            x: 4500.0,
            y: 0.0,
            z: 6200.0,
            count: 1,
            ..Default::default()
        };
        app.scenario.load_session(&dir).unwrap();
        app.scenario.apply_spawns(vec![spawn]).unwrap();

        assert_eq!(app.player_main_house_position(), Some((4500.0, 6200.0)));
        assert!(app.center_start_camera_on_player_main_house());
        assert_eq!(app.campaign.start_camera.x, 4500.0);
        assert_eq!(app.campaign.start_camera.z, 6200.0);
        assert!(app.campaign.camera_dirty, "camera dirty flag must be set on center");

        // UI rendering with main house (enabled button):
        let _ = ctx.run(Default::default(), |ctx| {
            app.show_map_properties(ctx);
        });

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_map_properties_camera_safe_save_workflow() {
        let root = temp_map_dir("camera_safe_save_root");
        let dir = root.join("MAPS").join("ENDL_006");
        std::fs::create_dir_all(dir.join("SCRIPT")).unwrap();
        std::fs::create_dir_all(root.join("SYSTEM/CLAK")).unwrap();
        std::fs::write(root.join("SYSTEM/CLAK/cl_scint.ini"), "[ObjDefName]\nGER_HAU00=BauGerHau00\n").unwrap();
        let original = against_rome_scenario::BciImage::idle().serialize();
        std::fs::write(dir.join("SCRIPT/ak_level.bci"), &original).unwrap();

        let doc = against_rome_scenario::ScenarioDocument {
            spawns: vec![against_rome_scenario::Spawn {
                id: uuid::Uuid::new_v4(),
                alias: "GER_HAU00".into(),
                team: 0,
                x: 4500.0,
                y: 0.0,
                z: 6200.0,
                count: 1,
                ..Default::default()
            }],
            ..Default::default()
        };
        std::fs::write(dir.join("arm_scenario.json"), serde_json::to_string(&doc).unwrap()).unwrap();

        let mut app = EditorApp::initial_state();
        app.set_loaded_map(dir.clone(), TerrainSession::create_blank(33, 32)).unwrap();
        app.scenario.load_session(&dir).unwrap();
        assert!(!app.scenario.dirty());
        assert_eq!(app.player_main_house_position(), Some((4500.0, 6200.0)));

        app.map_properties_open = true;

        // 1. Change start camera via Map properties (sets camera_dirty = true, candidate is None)
        app.campaign.start_camera.enabled = true;
        app.campaign.start_camera.x = 1200.0;
        app.campaign.start_camera.z = 3400.0;
        app.campaign.camera_dirty = true;
        assert!(!app.campaign.camera_candidate_ready());

        // Safe Save is blocked before applying
        app.safe_save_current();
        assert!(app.status_is_error, "Safe Save must be blocked without apply");
        assert!(app.status_message.contains("Apply to the loaded map before Safe Save"));

        // 2. Click Apply in Map properties:
        app.apply_start_camera_candidate_from_ui().unwrap();
        assert!(app.campaign.camera_candidate_ready());

        // Safe Save now succeeds and writes camera
        app.safe_save_current();
        assert!(!app.status_is_error, "Safe Save must succeed after Apply: {}", app.status_message);
        assert!(!app.campaign.camera_dirty);
        assert!(!app.campaign.camera_pending());

        let written_cam: against_rome_campaign::MapStartCamera = serde_json::from_str(
            &std::fs::read_to_string(dir.join("arm_start_camera.json")).expect("arm_start_camera.json written"),
        ).unwrap();
        assert!(written_cam.enabled);
        assert_eq!(written_cam.x, 1200.0);
        assert_eq!(written_cam.z, 3400.0);
        let bci_bytes = std::fs::read(dir.join("SCRIPT/ak_level.bci")).unwrap();
        assert_ne!(bci_bytes, original, "BCI script must be updated with camera shim");

        // 3. Center on player main house automatically applies candidate:
        assert!(app.center_start_camera_on_player_main_house());
        assert_eq!(app.campaign.start_camera.x, 4500.0);
        assert_eq!(app.campaign.start_camera.z, 6200.0);
        assert!(app.campaign.camera_dirty);
        assert!(app.campaign.camera_candidate_ready());

        // Safe Save succeeds and writes the centered camera
        app.safe_save_current();
        assert!(!app.status_is_error, "Safe Save must succeed after center on main house: {}", app.status_message);

        let written_cam2: against_rome_campaign::MapStartCamera = serde_json::from_str(
            &std::fs::read_to_string(dir.join("arm_start_camera.json")).expect("arm_start_camera.json written"),
        ).unwrap();
        assert!(written_cam2.enabled);
        assert_eq!(written_cam2.x, 4500.0);
        assert_eq!(written_cam2.z, 6200.0);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn panel_save_requests_route_through_one_central_safe_save() {
        let dir = temp_map_dir("panel_request");
        let mut app = EditorApp::initial_state();
        app.map_path_input = dir.to_string_lossy().into_owned();

        // No request: nothing is written and no status change.
        app.consume_panel_save_requests();
        assert!(!dir.join("boden.ini").exists());

        // Ambience request alone: consumed, saved through the transaction, dirty cleared.
        app.ambience.mark_dirty();
        app.ambience.request_save();
        app.consume_panel_save_requests();
        // The interactive route may stop for design-warning confirmation; accept it.
        app.resolve_safe_save_warnings(true);
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(
            app.status_message.starts_with("Saved safely"),
            "{}",
            app.status_message
        );
        assert!(!app.ambience.is_dirty());
        assert!(dir.join("boden.ini").is_file());
        assert!(!app.ambience.take_save_request());

        // Both panels requesting collapse into a single request that is fully consumed.
        app.ambience.request_save();
        app.placement.request_save();
        app.consume_panel_save_requests();
        assert!(!app.ambience.take_save_request());
        assert!(!app.placement.take_save_request());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn panel_save_request_failure_keeps_dirty_and_never_auto_saves_on_idle() {
        let dir = temp_map_dir("panel_request_fail");
        std::fs::create_dir_all(dir.join("boden.ini")).unwrap();
        let before = save_file_snapshot(&dir);
        let mut app = EditorApp::initial_state();
        app.map_path_input = dir.to_string_lossy().into_owned();
        app.ambience.mark_dirty();

        // Dirty without a request never writes.
        app.consume_panel_save_requests();
        assert_eq!(save_file_snapshot(&dir), before);

        app.ambience.request_save();
        app.consume_panel_save_requests();
        app.resolve_safe_save_warnings(true);
        assert!(app.status_is_error, "{}", app.status_message);
        assert!(app.ambience.is_dirty());
        assert_eq!(save_file_snapshot(&dir), before);
        // The request was consumed: a failed save is not retried automatically.
        assert!(!app.ambience.take_save_request());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn safe_save_preflight_failure_keeps_all_artifacts_and_dirty() {
        let dir = temp_map_dir("save_preflight");
        std::fs::write(dir.join("boden.bmp"), b"original height").unwrap();
        std::fs::create_dir_all(dir.join("boden.ini")).unwrap();
        let before = save_file_snapshot(&dir);
        let mut app = EditorApp::initial_state();
        app.map_path_input = dir.to_string_lossy().into_owned();
        app.ambience.mark_dirty();
        app.safe_save_current();
        assert!(app.status_is_error);
        assert!(
            app.status_message.contains("target is a directory"),
            "{}",
            app.status_message
        );
        assert!(app.status_message.contains("boden.ini"));
        assert!(app.ambience.is_dirty());
        assert!(app.session.as_ref().unwrap().raw_boden_bmp.is_none());
        assert_eq!(save_file_snapshot(&dir), before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn save_file_snapshot(dir: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
        fn visit(root: &Path, dir: &Path, out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    visit(root, &path, out);
                } else {
                    out.insert(
                        path.strip_prefix(root).unwrap().to_owned(),
                        std::fs::read(path).unwrap(),
                    );
                }
            }
        }
        let mut out = std::collections::BTreeMap::new();
        visit(dir, dir, &mut out);
        out
    }

    fn edit_save_terrain(app: &mut EditorApp) {
        let terrain = app.session.as_mut().unwrap();
        terrain.apply_height_changes(&[SampleChange {
            index: 0,
            before: terrain.heights()[0],
            after: 99,
        }]);
        terrain
            .paint_collision(0.5, 0.5, 0.5, CollisionBrushOp::Block)
            .unwrap();
        assert_eq!(terrain.collision()[0], 255);
        terrain.commit_stroke();
    }

    // Include directory inventory, file bytes, modification times and read-only flags:
    // an aborted gate must not even create staging/backup directories or rewrite equal bytes.
    fn save_artifact_snapshot(
        root: &Path,
    ) -> std::collections::BTreeMap<PathBuf, (bool, Vec<u8>, std::time::SystemTime, bool)> {
        fn visit(
            root: &Path,
            path: &Path,
            out: &mut std::collections::BTreeMap<
                PathBuf,
                (bool, Vec<u8>, std::time::SystemTime, bool),
            >,
        ) {
            let meta = std::fs::metadata(path).unwrap();
            out.insert(
                path.strip_prefix(root).unwrap().to_owned(),
                (
                    meta.is_dir(),
                    if meta.is_file() {
                        std::fs::read(path).unwrap()
                    } else {
                        vec![]
                    },
                    meta.modified().unwrap(),
                    meta.permissions().readonly(),
                ),
            );
            if meta.is_dir() {
                for entry in std::fs::read_dir(path).unwrap() {
                    visit(root, &entry.unwrap().path(), out);
                }
            }
        }
        let mut out = std::collections::BTreeMap::new();
        visit(root, root, &mut out);
        out
    }

    #[test]
    fn safe_save_diagnostics_error_precedes_commit_and_keeps_all_sessions_and_artifacts() {
        let (mut app, dir, _) = start_camera_app(false);
        pending_placement_fixture(&dir);
        app.placement.load_session(&dir).unwrap();
        app.placement.select_object(Some((0, 1)));
        app.placement
            .set_delta(["10".into(), "0".into(), "0".into()]);
        app.placement.move_family().unwrap();
        app.scenario.load_session(&dir).unwrap();
        app.scenario
            .add_events(&[against_rome_scenario::Event {
                name: "pending message".into(),
                actions: vec![against_rome_scenario::Action {
                    text: "hello".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }])
            .unwrap();
        // The manual design validator only warns here. MapDiagnostics must still run
        // and reject its independent collision-limit Error before the commit callback.
        app.session = Some(TerrainSession::create_blank(1026, 32));
        edit_save_terrain(&mut app);
        app.ambience.mark_dirty();
        std::fs::write(dir.join("skydens.dat"), b"retain height cache").unwrap();
        std::fs::create_dir_all(dir.join("empty/evidence")).unwrap();
        let before = save_artifact_snapshot(&dir);
        let flags = pending_panel_dirty_flags(&app);
        let undo_depth = app.placement.undo_depth();
        let events = app.scenario.document().clone();

        app.safe_save_current_using(|_| panic!("diagnostics failure must never reach commit"));

        assert!(app.status_is_error);
        assert!(
            app.status_message
                .contains("MapDiagnostics [collision-limit]"),
            "{}",
            app.status_message
        );
        assert_eq!(save_artifact_snapshot(&dir), before);
        assert_eq!(pending_panel_dirty_flags(&app), flags);
        assert!(app.session.as_ref().unwrap().is_dirty());
        assert!(app.session.as_ref().unwrap().can_undo());
        assert!(app.session.as_ref().unwrap().raw_boden_bmp.is_none());
        assert_eq!(app.placement.undo_depth(), undo_depth);
        assert_eq!(app.scenario.document(), &events);
        assert!(app.scenario.can_undo_events());
        assert!(app.ambience.is_dirty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn safe_save_diagnostics_reads_saved_spawns_with_clean_or_unopened_scenario() {
        for load_panel in [false, true] {
            let (mut app, dir, _) = start_camera_app(false);
            let document = against_rome_scenario::ScenarioDocument {
                spawns: vec![against_rome_scenario::Spawn {
                    id: uuid::Uuid::new_v4(),
                    alias: "unknown_not_in_catalog".into(),
                    x: 8000.0,
                    z: 8000.0,
                    count: 1,
                    ..Default::default()
                }],
                ..Default::default()
            };
            std::fs::write(
                dir.join("arm_scenario.json"),
                serde_json::to_vec(&document).unwrap(),
            )
            .unwrap();
            if load_panel {
                app.scenario.load_session(&dir).unwrap();
            }
            assert!(!app.scenario.dirty());
            edit_save_terrain(&mut app);
            let before = save_artifact_snapshot(&dir);
            app.safe_save_current_using(|_| panic!("unverifiable saved alias must block every save"));
            // The TEMP fixture has no game root, so the catalog itself is reported missing.
            assert!(
                app.status_message.contains("MapDiagnostics [alias-catalog-missing]"),
                "{}",
                app.status_message
            );
            assert_eq!(save_artifact_snapshot(&dir), before);
            assert!(app.session.as_ref().unwrap().is_dirty());
            assert!(!app.scenario.dirty());
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn safe_save_design_warning_cancel_keeps_artifacts_dirty_and_continue_commits() {
        let (mut app, dir, _) = start_camera_app(false);
        let document = against_rome_scenario::ScenarioDocument {
            spawns: vec![against_rome_scenario::Spawn {
                id: uuid::Uuid::new_v4(),
                alias: "BAU_HAU".into(),
                prebuilt: true,
                x: 8000.0,
                z: 8000.0,
                ..Default::default()
            }],
            ..Default::default()
        };
        std::fs::write(
            dir.join("arm_scenario.json"),
            serde_json::to_vec(&document).unwrap(),
        )
        .unwrap();
        edit_save_terrain(&mut app);
        // The pending water level, rather than the toolbar's old preview level, submerges
        // this building. The saved scenario must be checked despite its unopened panel.
        app.ambience.profile.boden.waterlevel = 500.0;
        app.ambience.mark_dirty();
        std::fs::write(dir.join("skydens.dat"), b"cached original").unwrap();
        std::fs::create_dir_all(dir.join("empty/evidence")).unwrap();
        let before = save_artifact_snapshot(&dir);
        app.request_safe_save();
        assert!(app.pending_save_warnings.is_some());
        assert!(app
            .pending_save_warnings
            .as_ref()
            .unwrap()
            .1
            .iter()
            .any(|i| i.code == "water"));
        assert!(app
            .maps
            .diagnostic_issues
            .iter()
            .any(|i| i.code == "blocked-building"));
        assert!(app
            .maps
            .diagnostic_issues
            .iter()
            .any(|i| i.code == "no-start"));
        assert_eq!(save_artifact_snapshot(&dir), before);
        app.resolve_safe_save_warnings(false);
        assert!(app.pending_save_warnings.is_none());
        assert!(app.status_message.contains("cancelled"));
        assert_eq!(save_artifact_snapshot(&dir), before);
        assert!(app.session.as_ref().unwrap().is_dirty());
        assert!(app.session.as_ref().unwrap().can_undo());
        assert!(app.ambience.is_dirty());
        app.request_safe_save();
        app.resolve_safe_save_warnings(true);
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(app.status_message.starts_with("Saved safely"));
        assert!(!app.has_unsaved_changes());
        assert!(!dir.join("skydens.dat").exists());
        assert_eq!(load_map(&dir).unwrap().heights()[0], 99);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn safe_save_warning_confirmation_cannot_bypass_new_diagnostics_error() {
        let (mut app, dir, _) = start_camera_app(false);
        app.request_safe_save();
        assert!(app.pending_save_warnings.is_some());
        app.session = Some(TerrainSession::create_blank(1026, 32));
        edit_save_terrain(&mut app);
        let before = save_artifact_snapshot(&dir);
        app.resolve_safe_save_warnings(true);
        assert!(app.status_is_error);
        assert!(app
            .status_message
            .contains("MapDiagnostics [collision-limit]"));
        assert!(app.pending_save_warnings.is_none());
        assert_eq!(save_artifact_snapshot(&dir), before);
        assert!(app.session.as_ref().unwrap().is_dirty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn safe_save_gate_uses_staged_events_and_pending_terrain_not_stale_saved_state() {
        let (mut app, dir, _) = start_camera_app(false);
        // Saved event has an unknown alias; deleting it in the active session repairs it.
        let document = against_rome_scenario::ScenarioDocument {
            events: vec![against_rome_scenario::Event {
                name: "remove this".into(),
                actions: vec![against_rome_scenario::Action {
                    kind: 2,
                    alias: "unknown".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        std::fs::write(
            dir.join("arm_scenario.json"),
            serde_json::to_vec(&document).unwrap(),
        )
        .unwrap();
        app.scenario.load_session(&dir).unwrap();
        app.scenario.remove_event(0).unwrap();
        let before = save_artifact_snapshot(&dir);
        app.safe_save_confirming_using(|_| false, |_| panic!("cancelled candidate must not write"));
        assert!(
            app.status_message.contains("confirmation declined"),
            "{}",
            app.status_message
        );
        assert_eq!(save_artifact_snapshot(&dir), before);
        assert!(app.scenario.dirty());
        assert!(app.scenario.can_undo_events());
        app.safe_save_current();
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(!app.scenario.dirty());
        assert!(against_rome_scenario::ScenarioDocument::from_json(
            &std::fs::read(dir.join("arm_scenario.json")).unwrap()
        )
        .unwrap()
        .events
        .is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn safe_save_gate_fails_closed_on_unreadable_saved_scenario_and_missing_destination() {
        let dir = temp_map_dir("diagnostics_fail_closed");
        let mut app = EditorApp::initial_state();
        app.map_path_input = dir.display().to_string();
        edit_save_terrain(&mut app);
        std::fs::write(dir.join("arm_scenario.json"), b"invalid JSON").unwrap();
        let before = save_artifact_snapshot(&dir);
        app.safe_save_current_using(|_| panic!("cannot ignore corrupt saved scenario"));
        assert!(app.status_is_error);
        assert_eq!(save_artifact_snapshot(&dir), before);
        assert!(app.session.as_ref().unwrap().is_dirty());
        app.map_path_input = dir.join("not-created").display().to_string();
        app.session = Some(TerrainSession::create_blank(1026, 32));
        edit_save_terrain(&mut app);
        app.safe_save_current_using(|_| panic!("error must precede destination creation"));
        assert!(app
            .status_message
            .contains("MapDiagnostics [collision-limit]"));
        assert_eq!(save_artifact_snapshot(&dir), before);
        assert!(app.session.as_ref().unwrap().is_dirty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn safe_save_gate_uses_pending_bound_placement_and_rejects_missing_binding() {
        for missing in [false, true] {
            let dir = temp_map_dir("diagnostics_bound_placement");
            pending_placement_fixture(&dir);
            let id = uuid::Uuid::new_v4();
            let document = against_rome_scenario::ScenarioDocument {
                spawns: vec![against_rome_scenario::Spawn {
                    id,
                    alias: "BAU_HAU".into(),
                    prebuilt: true,
                    x: 0.5,
                    z: 0.5,
                    ..Default::default()
                }],
                data_slots: vec![against_rome_scenario::model::DataSlot {
                    spawn_id: id,
                    slot: if missing { 100 } else { 0 },
                    uid: 1,
                }],
                ..Default::default()
            };
            std::fs::write(
                dir.join("arm_scenario.json"),
                serde_json::to_vec(&document).unwrap(),
            )
            .unwrap();
            let mut app = EditorApp::initial_state();
            app.session = Some(TerrainSession::create_blank(5, 32));
            app.map_path_input = dir.display().to_string();
            app.placement.load_session(&dir).unwrap();
            app.placement.select_object(Some((0, 1)));
            app.placement
                .set_delta(["8000".into(), "0".into(), "8000".into()]);
            app.placement.move_family().unwrap();
            edit_save_terrain(&mut app); // old anchor's cell is blocked, new one is clear
            let before = save_artifact_snapshot(&dir);
            app.safe_save_confirming_using(|_| false, |_| panic!("gate must not write"));
            assert_eq!(save_artifact_snapshot(&dir), before);
            assert!(app.placement.is_dirty());
            assert!(app.placement.undo_depth() > 0);
            assert!(app.session.as_ref().unwrap().is_dirty());
            if missing {
                assert!(
                    app.status_message.contains("bound scenario object"),
                    "{}",
                    app.status_message
                );
            } else {
                assert!(
                    app.status_message.contains("confirmation declined"),
                    "{}",
                    app.status_message
                );
                assert!(!app
                    .maps
                    .diagnostic_issues
                    .iter()
                    .any(|i| i.code == "blocked-building"));
            }
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    fn design_check_codes(report: &crate::design_check_ui::DesignCheckReport) -> Vec<String> {
        match report {
            crate::design_check_ui::DesignCheckReport::Checked {
                map_issues,
                design_issues,
                ..
            } => map_issues
                .iter()
                .map(|i| i.code.clone())
                .chain(design_issues.iter().map(|i| i.code.clone()))
                .collect(),
            other => panic!("check unavailable: {other:?}"),
        }
    }

    #[test]
    fn design_check_pending_data_failure_preserves_report() {
        let dir = temp_map_dir("pending_data_failure");
        pending_placement_fixture(&dir);
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.map_path_input = dir.display().to_string();
        app.placement.load_session(&dir).unwrap();
        app.placement.select_object(Some((0, 1)));
        app.placement.set_delta(["200".into(), "0".into(), "0".into()]);
        app.placement.move_family().unwrap();
        // 保留檔案但破壞格式，觸發 ObjectStore 載入時的封閉失敗檢查。
        std::fs::write(dir.join("DATA/objects.dat"), []).unwrap();
        let error = against_rome_placement::ObjectStore::load(&dir).err().unwrap();
        let baseline = app.candidate_diagnostics(&dir, &[]).unwrap();
        let report = app.design_check_report();
        let codes = design_check_codes(&report);
        assert!(codes.iter().any(|code| code != "pending-data-unchecked"));
        let crate::design_check_ui::DesignCheckReport::Checked { map_issues, .. } = report else {
            panic!("候選失敗後仍應提供檢查報告");
        };
        let warnings: Vec<_> = map_issues.iter()
            .filter(|issue| issue.code == "pending-data-unchecked").collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].severity, against_rome_maps::diagnostics::MapIssueSeverity::Warning);
        assert_eq!(warnings[0].english, format!("Unsaved Objects/Nature edits could not be checked: {error}"));
        assert_eq!(warnings[0].chinese, format!("未儲存的物件／自然地景變更無法檢查：{error}"));
        assert!(baseline.map_issues.iter().all(|issue| map_issues.contains(issue)));
        assert!(!baseline.map_issues.iter().any(|issue| issue.code == "pending-data-unchecked"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn design_check_pending_data_overlap_undo_is_read_only() {
        let dir = temp_map_dir("pending_data_overlap");
        pending_placement_fixture(&dir);
        let file = dir.join("DATA/position.dat");
        let mut bytes = std::fs::read(&file).unwrap();
        for slot in 0..3 {
            bytes[9 + slot * 17..13 + slot * 17]
                .copy_from_slice(&(100.0 + slot as f32 * 200.0).to_le_bytes());
        }
        std::fs::write(file, bytes).unwrap();
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.map_path_input = dir.display().to_string();
        app.placement.load_session(&dir).unwrap();
        let before = save_artifact_snapshot(&dir);
        let clean = app.design_check_report();
        assert!(!design_check_codes(&clean).contains(&"overlap".to_string()));
        app.placement.select_object(Some((0, 1)));
        app.placement.set_delta(["200".into(), "0".into(), "0".into()]);
        app.placement.move_family().unwrap();
        let pending = app.design_check_report();
        let crate::design_check_ui::DesignCheckReport::Checked { map_issues, .. } = pending else {
            panic!("候選檢查無法執行");
        };
        let overlap: Vec<_> = map_issues.iter().filter(|i| i.code == "overlap").collect();
        assert_eq!(overlap.len(), 2);
        assert!(overlap.iter().all(|i| i.english.ends_with(" (unsaved)") && i.chinese.ends_with("（未儲存）")));
        assert_eq!(save_artifact_snapshot(&dir), before);
        app.placement.undo().unwrap();
        assert!(!design_check_codes(&app.design_check_report()).contains(&"overlap".to_string()));
        assert_eq!(save_artifact_snapshot(&dir), before);
        assert_eq!(app.design_check_report(), clean);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn design_check_uses_pending_placement_and_leaves_disk_and_sessions_untouched() {
        let dir = temp_map_dir("design_check_placement");
        pending_placement_fixture(&dir);
        let id = uuid::Uuid::new_v4();
        let document = against_rome_scenario::ScenarioDocument {
            spawns: vec![against_rome_scenario::Spawn {
                id,
                alias: "BAU_HAU".into(),
                prebuilt: true,
                x: 0.5,
                z: 0.5,
                ..Default::default()
            }],
            data_slots: vec![against_rome_scenario::model::DataSlot {
                spawn_id: id,
                slot: 0,
                uid: 1,
            }],
            ..Default::default()
        };
        std::fs::write(
            dir.join("arm_scenario.json"),
            serde_json::to_vec(&document).unwrap(),
        )
        .unwrap();
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.map_path_input = dir.display().to_string();
        app.placement.load_session(&dir).unwrap();
        edit_save_terrain(&mut app); // the saved anchor cell is blocked
                                     // Saved placement position sits on the blocked cell.
        assert!(design_check_codes(&app.design_check_report())
            .iter()
            .any(|c| c == "blocked-building"));
        // Pending placement moves the bound object to a clear cell.
        app.placement.select_object(Some((0, 1)));
        app.placement
            .set_delta(["8000".into(), "0".into(), "8000".into()]);
        app.placement.move_family().unwrap();
        let before = save_artifact_snapshot(&dir);
        let flags = pending_panel_dirty_flags(&app);
        let undo_depth = app.placement.undo_depth();
        let status = app.status_message.clone();
        app.run_design_check();
        let report = app.design_check.report.clone().unwrap();
        assert!(
            !design_check_codes(&report)
                .iter()
                .any(|c| c == "blocked-building"),
            "{report:?}"
        );
        assert!(app.design_check.open);
        assert_eq!(save_artifact_snapshot(&dir), before);
        assert_eq!(pending_panel_dirty_flags(&app), flags);
        assert_eq!(app.placement.undo_depth(), undo_depth);
        assert!(app.session.as_ref().unwrap().is_dirty());
        assert!(app.session.as_ref().unwrap().can_undo());
        assert_eq!(app.status_message, status);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn headless_design_check_loads_scenario_and_writes_nothing() {
        use crate::design_check_ui::{format_issue_row, headless_design_check};
        let (_, dir, _) = start_camera_app(false);
        let before = save_artifact_snapshot(&dir);
        let rows = headless_design_check(
            &dir,
            TerrainSession::create_blank(5, 32),
            AppLanguage::English,
        )
        .unwrap();
        use against_rome_maps::diagnostics::MapIssueSeverity;
        // The fixture has an idle BCI and no authored objects/events.
        let native = rows.iter().find(|r| r.code == "scenario-native-only");
        assert_eq!(native.map(|r| r.severity), Some(MapIssueSeverity::Warning), "{rows:?}");
        assert!(format_issue_row(&rows[0], AppLanguage::English).starts_with('['));
        let blocked = headless_design_check(
            &dir,
            TerrainSession::create_blank(1026, 32),
            AppLanguage::English,
        )
        .unwrap();
        assert!(blocked.iter().any(|r| r.severity == MapIssueSeverity::Error));
        assert_eq!(save_artifact_snapshot(&dir), before);
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap().flatten() {
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    fn gui_frame(app: &mut EditorApp, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280.0, 800.0))),
                events,
                ..Default::default()
            },
            |ctx| app.ui_frame(ctx),
        )
    }

    fn gui_find(output: &egui::FullOutput, label: &str) -> Option<Rect> {
        output.shapes.iter().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.text() == label => {
                Some(Rect::from_min_size(t.pos, t.galley.size()))
            }
            _ => None,
        })
    }

    fn gui_pointer(pos: Pos2, pressed: bool) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE },
        ]
    }

    /// Clicks the painted label exactly like a mouse: press and release on its centre.
    fn gui_click(app: &mut EditorApp, ctx: &egui::Context, label: &str) -> egui::FullOutput {
        let settled = gui_frame(app, ctx, vec![]);
        let pos = gui_find(&settled, label).unwrap_or_else(|| panic!("no painted label {label:?}")).center();
        gui_frame(app, ctx, gui_pointer(pos, true));
        gui_frame(app, ctx, gui_pointer(pos, false));
        gui_frame(app, ctx, vec![])
    }

    /// Headless GUI click-through of the full editor frame on a TEMP copy of a retail map:
    /// real pointer events against the real widget layout (no screen control). Not a visual
    /// check: it proves the toolbar, canvas brush, undo/redo, Safe Save and language switch work
    /// through the UI. `cargo test -p against-rome-editor-app --lib gui_click_through -- --ignored`
    #[test]
    #[ignore = "copies a map from the installed game"]
    fn gui_click_through_design_check_brush_undo_save_reload_language() {
        let game = PathBuf::from(std::env::var("ARM_AI_GAME_ROOT").unwrap_or_else(|_| r"C:\Program Files (x86)\Against Rome".into()));
        let root = temp_map_dir("gui_click_root");
        let dir = root.join("MAPS").join("ENDL_005");
        copy_tree(&game.join("MAPS/ENDL_005"), &dir);
        std::fs::create_dir_all(root.join("SYSTEM/CLAK")).unwrap();
        std::fs::copy(game.join("SYSTEM/CLAK/cl_scint.ini"), root.join("SYSTEM/CLAK/cl_scint.ini")).unwrap();
        std::fs::copy(game.join("floortex.dat"), root.join("floortex.dat")).unwrap();
        let mut app = EditorApp::initial_state();
        app.set_loaded_map(dir.clone(), crate::storage::load_map(&dir).unwrap()).unwrap();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            gui_frame(&mut app, &ctx, vec![]);
        }

        // 1. Toolbar Design Check opens the read-only report; ENDL_005 is clean.
        gui_click(&mut app, &ctx, "Design Check");
        assert!(app.design_check.open);
        let report = app.design_check.report.clone().expect("report");
        assert!(report.is_clean(), "{report:?}");
        app.design_check.open = false;

        // 2. Raise brush + drag across the map canvas.
        gui_click(&mut app, &ctx, "Raise (Height) [1]");
        let original = app.session.as_ref().unwrap().heights().to_vec();
        let map = app.last_map_rect;
        let (a, b) = (map.lerp_inside(egui::vec2(0.45, 0.5)), map.lerp_inside(egui::vec2(0.55, 0.5)));
        gui_frame(&mut app, &ctx, gui_pointer(a, true));
        for step in 1..=10 {
            let p = a + (b - a) * (step as f32 / 10.0);
            gui_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(p)]);
        }
        gui_frame(&mut app, &ctx, gui_pointer(b, false));
        gui_frame(&mut app, &ctx, vec![]);
        let painted = app.session.as_ref().unwrap().heights().to_vec();
        let changed = original.iter().zip(&painted).filter(|(x, y)| x != y).count();
        assert!(changed > 0, "canvas drag raised nothing");
        assert!(app.session.as_ref().unwrap().is_dirty());

        // 3. Toolbar Undo / Redo.
        gui_click(&mut app, &ctx, "Undo (Ctrl+Z)");
        assert_eq!(app.session.as_ref().unwrap().heights(), &original[..]);
        gui_click(&mut app, &ctx, "Redo (Ctrl+Y)");
        assert_eq!(app.session.as_ref().unwrap().heights(), &painted[..]);

        // 4. Toolbar Safe Save, then a fresh load sees the brush stroke.
        let out = gui_click(&mut app, &ctx, "Safe Save");
        if app.pending_save_warnings.is_some() {
            let texts: Vec<String> = out.shapes.iter().filter_map(|s| match &s.shape { egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()), _ => None }).collect();
            panic!("unexpected save warning dialog: {texts:?}");
        }
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(!app.session.as_ref().unwrap().is_dirty(), "{}", app.status_message);
        assert_eq!(crate::storage::load_map(&dir).unwrap().heights(), &painted[..]);

        // 4b. Material brush (loaded on demand): palette pick + canvas drag paints boden.txt.
        let out = gui_click(&mut app, &ctx, "Material (terrain paint) [9]");
        let before = app.textures.session_textures().expect("materials loaded on demand").to_vec();
        if gui_find(&out, "Dirt Road").is_none() {
            let texts: Vec<String> = out.shapes.iter().filter_map(|s| match &s.shape { egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()), _ => None }).collect();
            panic!("Dirt Road not in palette: {texts:?}");
        }
        let picked = gui_click(&mut app, &ctx, "Dirt Road");
        eprintln!("after pick: {:?}", picked.shapes.iter().filter_map(|s| match &s.shape { egui::epaint::Shape::Text(t) if t.galley.text().starts_with("Active brush") => Some(t.galley.text().to_string()), _ => None }).collect::<Vec<_>>());
        let map = app.last_map_rect;
        let (a, b) = (map.lerp_inside(egui::vec2(0.30, 0.30)), map.lerp_inside(egui::vec2(0.40, 0.30)));
        gui_frame(&mut app, &ctx, gui_pointer(a, true));
        for step in 1..=10 {
            gui_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(a + (b - a) * (step as f32 / 10.0))]);
        }
        gui_frame(&mut app, &ctx, gui_pointer(b, false));
        gui_frame(&mut app, &ctx, vec![]);
        let after = app.textures.session_textures().unwrap().to_vec();
        let tiles = before.iter().zip(&after).filter(|(x, y)| x != y).count();
        eprintln!("material tiles painted through the UI: {tiles}; status: {}; notice: {:?}; map rect {:?}", app.status_message, app.textures.material_notice(), map);
        assert!(tiles > 0, "material drag painted nothing: {}", app.status_message);
        gui_click(&mut app, &ctx, "Undo (Ctrl+Z)");
        assert_eq!(app.textures.session_textures().unwrap(), &before[..]);
        gui_click(&mut app, &ctx, "Redo (Ctrl+Y)");
        assert_eq!(app.textures.session_textures().unwrap(), &after[..]);
        gui_click(&mut app, &ctx, "Safe Save");
        assert!(app.pending_save_warnings.is_none() && !app.status_is_error, "{}", app.status_message);
        let mut reloaded = TexturesUi::default();
        reloaded.load_session(&dir).unwrap();
        assert_eq!(reloaded.session_textures().unwrap(), &after[..]);

        // 4c. Search the palette like a designer ("green"), pick a bridge-tier grass and paint it
        // with the default auto transition.
        let out = gui_frame(&mut app, &ctx, vec![]);
        let search = gui_find(&out, "Search grass, sand, mud, rock...").expect("palette search box").center();
        gui_frame(&mut app, &ctx, gui_pointer(search, true));
        gui_frame(&mut app, &ctx, gui_pointer(search, false));
        gui_frame(&mut app, &ctx, vec![egui::Event::Text("green".into())]);
        let out = gui_frame(&mut app, &ctx, vec![]);
        let texts: Vec<String> = out.shapes.iter().filter_map(|s| match &s.shape { egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()), _ => None }).collect();
        let palette: Vec<&String> = texts.iter().skip_while(|t| !t.starts_with("Active brush")).collect();
        eprintln!("search 'green' palette: {palette:?}");
        let grass = palette
            .iter()
            .find(|t| t.contains("Green") && t.ends_with("Grass"))
            .map(|t| t.to_string())
            .unwrap_or_else(|| panic!("no green grass offered: {palette:?}"));
        gui_click(&mut app, &ctx, &grass);
        let before = app.textures.session_textures().unwrap().to_vec();
        let (a, b) = (map.lerp_inside(egui::vec2(0.65, 0.70)), map.lerp_inside(egui::vec2(0.75, 0.70)));
        gui_frame(&mut app, &ctx, gui_pointer(a, true));
        for step in 1..=10 {
            gui_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(a + (b - a) * (step as f32 / 10.0))]);
        }
        gui_frame(&mut app, &ctx, gui_pointer(b, false));
        gui_frame(&mut app, &ctx, vec![]);
        let changed = before.iter().zip(app.textures.session_textures().unwrap()).filter(|(x, y)| x != y).count();
        eprintln!("{grass}: {changed} tiles; notice {:?}", app.textures.material_notice());
        assert!(changed > 0, "{grass} painted nothing: {:?}", app.textures.material_notice());

        // 5. Language toggle repaints the toolbar in Traditional Chinese.
        let out = gui_click(&mut app, &ctx, "中");
        assert!(gui_find(&out, "Design Check").is_none(), "toolbar still English");
        eprintln!(
            "zh toolbar: {:?}",
            out.shapes.iter().filter_map(|s| match &s.shape { egui::epaint::Shape::Text(t) if t.pos.y < 60.0 => Some(t.galley.text().to_string()), _ => None }).collect::<Vec<_>>()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    /// Design Check's quick fix turns a neutral attack wave hostile with one diplomacy event
    /// (one undo step), clicked through the real panel.
    #[test]
    fn design_check_quick_fix_makes_wave_team_hostile() {
        let root = temp_map_dir("hostile_fix_root");
        let dir = root.join("MAPS").join("ENDL_006");
        std::fs::create_dir_all(dir.join("SCRIPT")).unwrap();
        std::fs::create_dir_all(root.join("SYSTEM/CLAK")).unwrap();
        std::fs::write(root.join("SYSTEM/CLAK/cl_scint.ini"), "[ObjDefName]\nHUN_KAVINF00=FigHunKav00\n").unwrap();
        std::fs::write(dir.join("SCRIPT/ak_level.bci"), against_rome_scenario::BciImage::idle().serialize()).unwrap();
        let mut app = EditorApp::initial_state();
        app.set_loaded_map(dir.clone(), TerrainSession::create_blank(33, 32)).unwrap();
        app.scenario.load_session(&dir).unwrap();
        app.scenario
            .add_events(&[against_rome_scenario::Event {
                name: "Wave_01".into(),
                delay_seconds: 10,
                actions: vec![against_rome_scenario::Action {
                    kind: 2,
                    team: 1,
                    alias: "HUN_KAVINF00".into(),
                    x: 4000.0,
                    z: 4000.0,
                    count: 3,
                    ..Default::default()
                }],
                ..Default::default()
            }])
            .unwrap();
        let ctx = egui::Context::default();
        gui_click(&mut app, &ctx, "Design Check");
        assert!(design_codes(&app).contains(&"wave-not-hostile".to_string()), "{:?}", design_codes(&app));
        gui_click(&mut app, &ctx, "Make wave teams hostile");
        let events = app.scenario.document().events.clone();
        assert_eq!(events.len(), 2, "{events:?}");
        let war = &events[1].actions[0];
        assert_eq!((war.kind, war.team, war.other_team, war.hostile), (1, 0, 1, true));
        assert!(!design_codes(&app).contains(&"wave-not-hostile".to_string()), "{:?}", design_codes(&app));
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(app.scenario.undo_events());
        assert_eq!(app.scenario.document().events.len(), 1, "one undo removes the fix");
        std::fs::remove_dir_all(root).unwrap();
    }

    /// 真實面板產生、目的地預設、夾限預覽、設計檢查及單步復原。
    #[test]
    fn attack_wave_generator_panel() {
        use against_rome_scenario::WorldPosition;
        let root = temp_map_dir("wave_generator_panel");
        let dir = root.join("MAPS/ENDL_006");
        std::fs::create_dir_all(dir.join("SCRIPT")).unwrap();
        std::fs::create_dir_all(root.join("SYSTEM/CLAK")).unwrap();
        std::fs::write(root.join("SYSTEM/CLAK/cl_scint.ini"),
            "[ObjDefName]\nHUN_KAVINF00=FigHunKav00\n").unwrap();
        std::fs::write(dir.join("SCRIPT/ak_level.bci"), against_rome_scenario::BciImage::idle().serialize()).unwrap();
        let mut app = EditorApp::initial_state();
        app.set_loaded_map(dir.clone(), TerrainSession::create_blank(33, 32)).unwrap();
        app.campaign.start_camera.enabled = true;
        app.campaign.start_camera.x = 6000.;
        app.campaign.start_camera.z = 7000.;
        let disk = save_artifact_snapshot(&dir);
        let ctx = egui::Context::default();
        gui_click(&mut app, &ctx, "Scenario / Camera");
        gui_click(&mut app, &ctx, "Attack wave generator");
        for _ in 0..20 { gui_frame(&mut app, &ctx, vec![]); }
        let out = gui_frame(&mut app, &ctx, vec![]);
        assert!(gui_find(&out, "5 waves, 10..20 units, 60s..300s (1–20 units per wave)").is_some());
        assert_eq!(app.scenario.wave_generator.destination, WorldPosition { x: 6000, z: 7000 });
        gui_click(&mut app, &ctx, "Map edge");
        app.scenario.wave_generator.destination = WorldPosition { x: 1, z: 2 };
        gui_click(&mut app, &ctx, "Player base");
        gui_click(&mut app, &ctx, "Generate attack waves");
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(app.status_message.contains("Save"));
        let events = app.scenario.document().events.clone();
        assert_eq!(events.len(), 6);
        assert_eq!(events[0], crate::scenario_ui::hostile_wave_event(1));
        for (i, event) in events[1..].iter().enumerate() {
            assert_eq!(event.name, format!("Wave_{:02}", i + 1));
            assert_eq!(event.delay_seconds, 60 + i as i32 * 60);
            assert!(event.enabled && !event.repeat && event.conditions.is_empty());
            let a = &event.actions[0];
            assert_eq!((a.kind, a.team, a.count), (2, 1, (10 + i as i32 * 5).min(20)));
            assert_eq!((a.x, a.z), (0., 8000.));
            assert_eq!(a.alias, "HUN_KAVINF00");
            assert_eq!(a.move_destination, Some(WorldPosition { x: 6000, z: 7000 }));
        }
        let codes = design_codes(&app);
        assert!(!codes.iter().any(|c| c == "wave-not-hostile" || c == "wave-no-destination"), "{codes:?}");
        assert!(app.scenario.undo_events());
        assert!(app.scenario.document().events.is_empty());
        assert!(!app.scenario.can_undo_events());
        assert!(app.scenario.redo_events());
        assert_eq!(app.scenario.document().events, events);
        assert_eq!(save_artifact_snapshot(&dir), disk);
        let out = gui_click(&mut app, &ctx, "中");
        assert!(gui_find(&out, "進攻波次產生器").is_some());
        std::fs::remove_dir_all(root).unwrap();
    }

    /// 三種預設、所有敵隊、編譯與設計檢查；每次新增皆可單步復原。
    #[test]
    fn objective_presets_compile_check_and_undo() {
        use against_rome_scenario::{BciImage, Spawn};
        let root = temp_map_dir("objective_presets");
        let dir = root.join("MAPS/ENDL_006");
        std::fs::create_dir_all(dir.join("SCRIPT")).unwrap();
        std::fs::create_dir_all(root.join("SYSTEM/CLAK")).unwrap();
        std::fs::write(root.join("SYSTEM/CLAK/cl_scint.ini"),
            "[ObjDefName]\nGER_HAU00=BauGerHau00_Haupthaus\n").unwrap();
        let original = BciImage::idle().serialize();
        std::fs::write(dir.join("SCRIPT/ak_level.bci"), &original).unwrap();
        let mut app = EditorApp::initial_state();
        app.set_loaded_map(dir.clone(), TerrainSession::create_blank(33, 32)).unwrap();
        app.scenario.load_session(&dir).unwrap();
        let houses: Vec<_> = (0..=7).map(|team| Spawn { alias: "GER_HAU00".into(), team,
            x: 4000., z: 4000., id: uuid::Uuid::new_v4(), ..Default::default() }).collect();
        app.scenario.apply_spawns(houses.clone()).unwrap();
        let errors = app.design_check_report().error_count();
        let disk = save_artifact_snapshot(&dir);
        for team in -1..=7 {
            let seconds = (team == -1).then_some(123);
            let target = (team >= 0).then(|| houses[team as usize].id);
            assert!(app.scenario.add_objective(seconds, target, team.max(0)).unwrap());
            let event = &app.scenario.document().events[0];
            assert!(event.enabled && !event.repeat);
            assert_eq!(event.delay_seconds, if team == -1 { 123 } else { 0 });
            assert_eq!(event.actions.len(), 1);
            assert_eq!(event.actions[0].kind, if team == 0 { 4 } else { 3 });
            if let Some(target) = target {
                assert_eq!(event.conditions.len(), 1);
                assert_eq!((event.conditions[0].kind, event.conditions[0].target_id), (1, target));
                let graph = against_rome_scenario::graph::ObjectDeadConditionNode::new(target, "主屋");
                assert_eq!(graph.to_condition(), event.conditions[0]);
            } else { assert!(event.conditions.is_empty()); }
            let aliases = vec!["GER_HAU00".to_string()];
            let compiled = against_rome_scenario::compile(&original, app.scenario.document(), None, Some(&aliases)).unwrap();
            BciImage::parse(&compiled).unwrap();
            assert!(app.design_check_report().error_count() <= errors);
            assert!(!app.scenario.add_objective(seconds, target, team.max(0)).unwrap());
            assert!(app.scenario.undo_events());
            assert!(app.scenario.document().events.is_empty());
            assert!(!app.scenario.can_undo_events());
            assert!(app.scenario.redo_events());
            assert!(app.scenario.undo_events());
        }
        assert_eq!(save_artifact_snapshot(&dir), disk);
        std::fs::remove_dir_all(root).unwrap();
    }

    /// 真實雙語面板、去重狀態與僅原生 DATA 主屋的停用行為。
    #[test]
    fn objective_presets_panel_and_native_only_disabled() {
        let root = temp_map_dir("objective_panel");
        let dir = root.join("MAPS/ENDL_006");
        std::fs::create_dir_all(dir.join("SCRIPT")).unwrap();
        std::fs::write(dir.join("SCRIPT/ak_level.bci"), against_rome_scenario::BciImage::idle().serialize()).unwrap();
        make_test_placement_fixture(&dir, 10001);
        std::fs::create_dir_all(root.join("SYSTEM/DATA_MP/DEFAULTS")).unwrap();
        let mut fields = vec!["0".to_string(); 53];
        fields[52] = "BauGerHau00_Haupthaus".into();
        std::fs::write(root.join("SYSTEM/DATA_MP/DEFAULTS/objdef.dau"),
            format!("[ObjectDefaults]\n{}\n", fields.join(","))).unwrap();
        let mut app = EditorApp::initial_state();
        app.set_loaded_map(dir.clone(), TerrainSession::create_blank(33, 32)).unwrap();
        let ctx = egui::Context::default();
        gui_click(&mut app, &ctx, "Scenario / Camera");
        gui_click(&mut app, &ctx, "Objective presets");
        for _ in 0..20 { gui_frame(&mut app, &ctx, vec![]); }
        let out = gui_frame(&mut app, &ctx, vec![]);
        assert!(gui_find(&out, "No main-house Spawn").is_some());
        gui_click(&mut app, &ctx, "Player main house destroyed: Defeat");
        gui_click(&mut app, &ctx, "Enemy main house destroyed: Victory");
        assert!(app.scenario.document().events.is_empty());
        assert!(!app.scenario.can_undo_events());
        assert!(app.scenario.add_objective(None, None, 0).is_err());
        gui_click(&mut app, &ctx, "Add survival victory");
        assert_eq!(app.scenario.document().events.len(), 1);
        assert!(app.status_message.contains("Save"), "{}", app.status_message);
        assert!(!app.status_is_error);
        let out = gui_frame(&mut app, &ctx, vec![]);
        assert!(gui_find(&out, "Already present").is_some());
        gui_click(&mut app, &ctx, "Add survival victory");
        assert_eq!(app.scenario.document().events.len(), 1);
        assert!(app.scenario.undo_events());
        assert!(!app.scenario.can_undo_events());
        let out = gui_click(&mut app, &ctx, "中");
        assert!(gui_find(&out, "勝負條件預設").is_some());
        std::fs::remove_dir_all(root).unwrap();
    }

    /// 外交僅接受啟用的敵對動作，且兩個方向均可辨識；名稱避免大小寫碰撞。
    #[test]
    fn attack_wave_generator_diplomacy_names_and_undo() {
        use against_rome_scenario::{Action, Event, WorldPosition};
        for mode in ["none", "hostile", "reverse", "peace", "disabled", "other"] {
            let root = temp_map_dir("wave_generator_diplomacy");
            let dir = root.join("MAPS/ENDL_006");
            std::fs::create_dir_all(dir.join("SCRIPT")).unwrap();
            std::fs::write(dir.join("SCRIPT/ak_level.bci"), against_rome_scenario::BciImage::idle().serialize()).unwrap();
            let mut app = EditorApp::initial_state();
            app.set_loaded_map(dir.clone(), TerrainSession::create_blank(33, 32)).unwrap();
            app.scenario.load_session(&dir).unwrap();
            let message = Action { text: "Existing event".into(), ..Default::default() };
            let mut prior = vec![Event { name: "wave_01".into(), actions: vec![message.clone()], ..Default::default() },
                Event { name: "Wave_03".into(), actions: vec![message], ..Default::default() }];
            if mode != "none" {
                let mut e = crate::scenario_ui::hostile_wave_event(if mode == "other" { 2 } else { 7 });
                if mode == "reverse" { e.actions[0].team = 7; e.actions[0].other_team = 0; }
                if mode == "peace" { e.actions[0].hostile = false; }
                if mode == "disabled" { e.enabled = false; }
                prior.push(e);
            }
            app.scenario.add_events(&prior).unwrap();
            let g = &mut app.scenario.wave_generator;
            g.team = 7; g.alias = "HUN_KAVINF00".into(); g.waves = 3;
            g.first = 0; g.interval = 10; g.base = 0; g.increase = i32::MAX;
            g.destination = WorldPosition { x: 16383, z: 0 };
            assert_eq!(app.scenario.generate_attack_waves().unwrap(), 3);
            let events = &app.scenario.document().events;
            let needs = !matches!(mode, "hostile" | "reverse");
            assert_eq!(events.len(), prior.len() + 3 + usize::from(needs));
            let waves = &events[events.len()-3..];
            for (i, e) in waves.iter().enumerate() {
                assert_eq!(e.name, ["Wave_02", "Wave_04", "Wave_05"][i]);
                assert_eq!(e.delay_seconds, i as i32 * 10);
                assert_eq!(e.actions[0].count, if i == 0 { 1 } else { 20 });
                assert_eq!(e.actions[0].move_destination, Some(WorldPosition { x: 16383, z: 0 }));
            }
            let mut names = std::collections::HashSet::new();
            assert!(events.iter().all(|e| names.insert(e.name.to_lowercase())));
            app.scenario.generate_attack_waves().unwrap();
            let mut names = std::collections::HashSet::new();
            assert!(app.scenario.document().events.iter().all(|e| names.insert(e.name.to_lowercase())));
            assert!(app.scenario.undo_events());
            assert!(app.scenario.undo_events());
            assert_eq!(app.scenario.document().events, prior);
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    /// 時間、別名、座標與事件容量錯誤均不留下半批事件或額外復原步驟。
    #[test]
    fn attack_wave_generator_rejects_invalid_batch_atomically() {
        let root = temp_map_dir("wave_generator_invalid");
        let dir = root.join("MAPS/ENDL_006");
        std::fs::create_dir_all(dir.join("SCRIPT")).unwrap();
        std::fs::write(dir.join("SCRIPT/ak_level.bci"), against_rome_scenario::BciImage::idle().serialize()).unwrap();
        let mut app = EditorApp::initial_state();
        app.set_loaded_map(dir.clone(), TerrainSession::create_blank(33, 32)).unwrap();
        app.scenario.load_session(&dir).unwrap();
        let mut valid = app.scenario.wave_generator.clone();
        valid.alias = "HUN_KAVINF00".into();
        for mode in ["time", "alias", "coordinate", "capacity", "team", "waves"] {
            app.scenario.wave_generator = valid.clone();
            let g = &mut app.scenario.wave_generator;
            match mode {
                "time" => { g.first = i32::MAX; g.interval = i32::MAX; }
                "alias" => g.alias.clear(),
                "coordinate" => g.x = f32::NAN,
                "capacity" => g.waves = 255,
                "team" => g.team = 0,
                _ => g.waves = 0,
            }
            if mode == "capacity" {
                app.scenario.add_events(&[crate::scenario_ui::hostile_wave_event(2)]).unwrap();
            }
            let before = app.scenario.document().events.clone();
            assert!(app.scenario.generate_attack_waves().is_err(), "{mode}");
            assert_eq!(app.scenario.document().events, before);
            if mode == "capacity" { assert!(app.scenario.undo_events()); }
            assert!(!app.scenario.can_undo_events());
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    /// 波次目的地修正涵蓋面板點擊、主屋優先、鏡頭備援與無目標略過。
    #[test]
    fn design_check_quick_fix_sends_waves_to_player_base() {
        use against_rome_scenario::{Action, Event, Spawn, WorldPosition};
        for mode in ["house", "native", "pending_native", "camera", "no_target"] {
            let root = temp_map_dir("wave_destination_root");
            let dir = root.join("MAPS/ENDL_006");
            std::fs::create_dir_all(dir.join("SCRIPT")).unwrap();
            std::fs::create_dir_all(root.join("SYSTEM/CLAK")).unwrap();
            std::fs::write(root.join("SYSTEM/CLAK/cl_scint.ini"),
                "[ObjDefName]\nHUN_KAVINF00=FigHunKav00\nGER_HAU00=BauGerHau00_Haupthaus\n").unwrap();
            std::fs::write(dir.join("SCRIPT/ak_level.bci"), against_rome_scenario::BciImage::idle().serialize()).unwrap();
            let mut app = EditorApp::initial_state();
            app.set_loaded_map(dir.clone(), TerrainSession::create_blank(33, 32)).unwrap();
            app.scenario.load_session(&dir).unwrap();
            let action = |team, destination| Action {
                kind: 2, team, alias: "HUN_KAVINF00".into(), count: 3,
                x: 4000.0, z: 4000.0, move_destination: destination, ..Default::default()
            };
            app.scenario.add_events(&[
                Event { name: "Wave_01".into(), actions: vec![action(1, None), action(2, None),
                    action(0, None), action(3, Some(WorldPosition { x: 100, z: 200 }))], ..Default::default() },
                Event { name: "Wave_02".into(), actions: vec![action(7, None)], ..Default::default() },
                Event { name: "Disabled".into(), enabled: false, actions: vec![action(4, None)], ..Default::default() },
            ]).unwrap();
            // 敵方主屋不可作為玩家的進攻目標。
            app.scenario.apply_spawns(vec![Spawn { alias: "GER_HAU00".into(), team: if mode == "house" { 0 } else { 1 },
                x: 6000.0, z: 7000.0, count: 0, id: uuid::Uuid::new_v4(), ..Default::default() }]).unwrap();
            app.campaign.start_camera.enabled = mode != "no_target";
            app.campaign.start_camera.x = 8000.0;
            app.campaign.start_camera.z = 9000.0;
            if mode == "native" || mode == "pending_native" {
                make_test_placement_fixture(&dir, 10001);
                std::fs::create_dir_all(root.join("SYSTEM/DATA_MP/DEFAULTS")).unwrap();
                let mut fields = vec!["0".to_string(); 53];
                fields[52] = "BauGerHau00_Haupthaus".into();
                std::fs::write(root.join("SYSTEM/DATA_MP/DEFAULTS/objdef.dau"), format!("[ObjectDefaults]\n{}\n", fields.join(","))).unwrap();
                if mode == "pending_native" {
                    app.placement.load_session(&dir).unwrap();
                    app.placement.select_object(Some((0, 10001)));
                    app.placement.set_delta(["1000".into(), "0".into(), "2000".into()]);
                    app.placement.move_family().unwrap();
                }
            }
            let disk_before = save_artifact_snapshot(&dir);
            let before = app.scenario.document().events.clone();
            let ctx = egui::Context::default();
            gui_click(&mut app, &ctx, "Design Check");
            assert!(design_codes(&app).contains(&"wave-no-destination".to_string()));
            gui_click(&mut app, &ctx, "Send waves toward the player base");
            if mode == "no_target" {
                assert_eq!(app.scenario.document().events, before);
                assert!(app.status_message.contains("Skipped"), "{}", app.status_message);
                assert!(design_codes(&app).contains(&"wave-no-destination".to_string()));
                assert!(app.scenario.undo_events());
                assert!(app.scenario.document().events.is_empty(), "略過不可新增復原紀錄");
            } else {
                let target = match mode {
                    "house" => WorldPosition { x: 6000, z: 7000 },
                    "native" => WorldPosition { x: 1, z: 0 },
                    "pending_native" => WorldPosition { x: 1001, z: 2000 },
                    _ => WorldPosition { x: 8000, z: 9000 },
                };
                let mut expected = before.clone();
                for (event, action) in [(0, 0), (0, 1), (1, 0)] {
                    expected[event].actions[action].move_destination = Some(target.clone());
                }
                assert_eq!(app.scenario.document().events, expected);
                assert!(!design_codes(&app).contains(&"wave-no-destination".to_string()));
                assert!(!app.status_is_error && app.status_message.contains("Save"), "{}", app.status_message);
                assert!(app.scenario.dirty());
                assert_eq!(app.fix_wave_destinations().unwrap(), 0);
                assert!(app.scenario.undo_events());
                assert_eq!(app.scenario.document().events, before, "所有波次一步復原");
                app.run_design_check();
                assert!(design_codes(&app).contains(&"wave-no-destination".to_string()));
                assert!(app.scenario.redo_events());
                assert_eq!(app.scenario.document().events, expected);
            }
            assert_eq!(save_artifact_snapshot(&dir), disk_before, "修正前後不可寫入地圖檔案");
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    /// Opening the Scenario panel on an opened map loads its script on demand; events added from
    /// the panel must then pass Design Check instead of "劇本尚未載入來源".
    #[test]
    fn scenario_panel_loads_the_open_maps_script_on_demand() {
        let (mut app, dir, _) = start_camera_app(false);
        assert!(!app.scenario.is_loaded());
        let ctx = egui::Context::default();
        gui_click(&mut app, &ctx, "Scenario / Camera");
        assert!(app.scenario.is_loaded(), "{}", app.status_message);
        assert!(!app.has_unsaved_changes(), "loading the script is not an edit");
        let out = gui_click(&mut app, &ctx, "Add event");
        assert_eq!(app.scenario.document().events.len(), 1);
        assert_eq!(app.scenario.document().events[0].name, "Event 1");
        assert!(gui_find(&out, "Show message").is_some(), "English action list");
        assert!(gui_find(&out, "新增事件").is_none(), "panel follows the English UI");
        assert!(matches!(
            app.design_check_report(),
            crate::design_check_ui::DesignCheckReport::Checked { .. }
        ));
        // The panel follows the language toggle; event data is untouched.
        let out = gui_click(&mut app, &ctx, "中");
        assert!(gui_find(&out, "新增事件").is_some() && gui_find(&out, "顯示訊息").is_some());
        assert!(gui_find(&out, "Add event").is_none());
        assert_eq!(app.scenario.document().events[0].name, "Event 1");
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// The Spawns panel needs the scenario too; opening it must not send the designer to the
    /// Scenario panel first.
    #[test]
    fn spawns_panel_loads_the_open_maps_script_on_demand() {
        let (mut app, dir, _) = start_camera_app(false);
        let ctx = egui::Context::default();
        let out = gui_click(&mut app, &ctx, "Spawns");
        assert!(app.spawn_ui.open);
        assert!(app.scenario.is_loaded(), "{}", app.status_message);
        let out = gui_frame(&mut app, &ctx, vec![]);
        let _ = &out;
        assert!(!gui_texts(&out, 0.0).contains("No scenario loaded"), "{}", gui_texts(&out, 0.0));
        assert!(!app.has_unsaved_changes());
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Choosing the Material brush on an opened map must load its boden.txt on demand (like
    /// Flat Terrain), not tell the designer to "load a map with boden.txt".
    #[test]
    fn material_brush_mode_loads_the_open_maps_materials_on_demand() {
        let dir = temp_map_dir("material_mode_autoload");
        std::fs::write(
            dir.join("boden.txt"),
            b"[Dimension]\n2\n[Texturen]\n4BA___51\n4BA___51\n4BA___51\n4BB___51\n",
        )
        .unwrap();
        let mut app = EditorApp::initial_state();
        app.set_loaded_map(dir.clone(), TerrainSession::create_blank(9, 32)).unwrap();
        assert!(app.textures.session_textures().is_none());
        let ctx = egui::Context::default();
        let out = gui_click(&mut app, &ctx, "Material (terrain paint) [9]");
        assert_eq!(app.active_tool, ActiveTool::Material);
        assert!(app.textures.session_textures().is_some(), "{}", app.status_message);
        assert!(gui_find(&out, "Load a map with boden.txt to paint materials.").is_none());
        assert!(!app.has_unsaved_changes(), "loading materials is not an edit");
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn design_codes(app: &EditorApp) -> Vec<String> {
        match app.design_check.report.as_ref().expect("report") {
            crate::design_check_ui::DesignCheckReport::Checked { map_issues, design_issues, .. } => map_issues
                .iter()
                .map(|i| i.code.clone())
                .chain(design_issues.iter().map(|i| i.code.clone()))
                .collect(),
            other => panic!("check unavailable: {other:?}"),
        }
    }

    /// Headless click-through of scenario authoring on a TEMP copy of the untouched ENDL_000
    /// template: add Victory and Defeat events in the Scenario panel, re-run Design Check and
    /// Safe Save, then reload. Proves the editor half of win/loss; the game half is in-game.
    #[test]
    #[ignore = "copies a map from the installed game"]
    fn gui_click_through_authors_victory_and_defeat_events() {
        let game = PathBuf::from(std::env::var("ARM_AI_GAME_ROOT").unwrap_or_else(|_| r"C:\Program Files (x86)\Against Rome".into()));
        let root = temp_map_dir("gui_scenario_root");
        let dir = root.join("MAPS").join("ENDL_000");
        copy_tree(&game.join("MAPS/ENDL_000"), &dir);
        std::fs::create_dir_all(root.join("SYSTEM/CLAK")).unwrap();
        std::fs::copy(game.join("SYSTEM/CLAK/cl_scint.ini"), root.join("SYSTEM/CLAK/cl_scint.ini")).unwrap();
        let script_before = std::fs::read(dir.join("SCRIPT/ak_level.bci")).unwrap();
        let mut app = EditorApp::initial_state();
        app.set_loaded_map(dir.clone(), crate::storage::load_map(&dir).unwrap()).unwrap();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            gui_frame(&mut app, &ctx, vec![]);
        }

        gui_click(&mut app, &ctx, "Design Check");
        assert!(design_codes(&app).contains(&"scenario-native-only".to_string()));
        app.design_check.open = false;

        gui_click(&mut app, &ctx, "Scenario / Camera");
        for kind in ["Victory", "Defeat"] {
            gui_click(&mut app, &ctx, "Add event");
            gui_click(&mut app, &ctx, "Show message");
            gui_click(&mut app, &ctx, kind);
        }
        let kinds: Vec<Vec<i32>> = app.scenario.document().events.iter().map(|e| e.actions.iter().map(|a| a.kind).collect()).collect();
        assert_eq!(kinds, vec![vec![3], vec![4]], "victory then defeat");

        gui_click(&mut app, &ctx, "Design Check");
        let codes = design_codes(&app);
        assert!(!codes.iter().any(|c| c == "scenario-native-only" || c.starts_with("scenario-no-")), "{codes:?}");
        assert_eq!(app.design_check.report.as_ref().unwrap().error_count(), 0, "{codes:?}");
        app.design_check.open = false;

        // The template's base and troops are native DATA: no guessed team warnings, so Safe Save
        // goes straight through without the confirmation dialog.
        gui_click(&mut app, &ctx, "Safe Save");
        assert!(app.pending_save_warnings.is_none(), "unexpected design warnings: {:?}", app.pending_save_warnings.as_ref().map(|p| p.1.iter().map(|i| i.code.clone()).collect::<Vec<_>>()));
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(!app.has_unsaved_changes(), "{}", app.status_message);

        let script_after = std::fs::read(dir.join("SCRIPT/ak_level.bci")).unwrap();
        assert_ne!(script_after, script_before, "events compiled into the level script");
        assert_eq!(std::fs::read(dir.join("SCRIPT/ak_level.arm_original")).unwrap(), script_before);
        let mut reopened = EditorApp::initial_state();
        reopened.set_loaded_map(dir.clone(), crate::storage::load_map(&dir).unwrap()).unwrap();
        reopened.scenario.load_session(&dir).unwrap();
        let kinds: Vec<Vec<i32>> = reopened.scenario.document().events.iter().map(|e| e.actions.iter().map(|a| a.kind).collect()).collect();
        assert_eq!(kinds, vec![vec![3], vec![4]]);
        std::fs::remove_dir_all(root).unwrap();
    }

    fn gui_texts(out: &egui::FullOutput, min_y: f32) -> String {
        out.shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) if t.pos.y >= min_y => Some(format!("{}@({:.0},{:.0})", t.galley.text(), t.pos.x, t.pos.y)),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" | ")
    }


    /// Headless click-through of placement on a TEMP custom slot cloned from ENDL_000: open Spawns (script
    /// loads on demand), place a main house and a troop, Design Check, Safe Save and reload.
    #[test]
    #[ignore = "copies a map from the installed game"]
    fn gui_click_through_places_house_and_troop_and_saves() {
        let game = PathBuf::from(std::env::var("ARM_AI_GAME_ROOT").unwrap_or_else(|_| r"C:\Program Files (x86)\Against Rome".into()));
        let root = temp_map_dir("gui_spawn_root");
        // A custom slot, as New Map creates it: ENDL_000 content plus the custom-map marker.
        let dir = root.join("MAPS").join("ENDL_006");
        copy_tree(&game.join("MAPS/ENDL_000"), &dir);
        std::fs::write(dir.join(".arm_custom_map"), r#"{"Slot": 6, "SourceSlot": 0, "CreatedAt": "2026-10-22T00:00:00+00:00", "ToolVersion": "test"}"#).unwrap();
        std::fs::create_dir_all(root.join("SYSTEM/CLAK")).unwrap();
        std::fs::copy(game.join("SYSTEM/CLAK/cl_scint.ini"), root.join("SYSTEM/CLAK/cl_scint.ini")).unwrap();
        let mut app = EditorApp::initial_state();
        app.set_loaded_map(dir.clone(), crate::storage::load_map(&dir).unwrap()).unwrap();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            gui_frame(&mut app, &ctx, vec![]);
        }
        gui_click(&mut app, &ctx, "Spawns");
        assert!(app.scenario.is_loaded(), "{}", app.status_message);
        // The starter kit is reserved for blank maps (this template has native DATA), so place a
        // main house and a troop the regular way: pick a type, "Place on map", click the canvas.
        let map = app.last_map_rect;
        // "Place on map" is a toggle that stays on after a placement.
        let mut place = |app: &mut EditorApp, row: &str, at: egui::Vec2, toggle: bool| {
            gui_click(app, &ctx, row);
            if toggle {
                gui_click(app, &ctx, "Place on map");
            }
            let pos = map.lerp_inside(at);
            gui_frame(app, &ctx, gui_pointer(pos, true));
            gui_frame(app, &ctx, gui_pointer(pos, false));
            gui_frame(app, &ctx, vec![])
        };
        let out = place(&mut app, "Germans Haupthaus  (GER_HAU00)", egui::vec2(0.5, 0.5), true);
        // Search for a German infantry type, as a designer would.
        let search = gui_find(&out, "Name or alias (space = AND)").expect("type search").center();
        gui_frame(&mut app, &ctx, gui_pointer(search, true));
        gui_frame(&mut app, &ctx, gui_pointer(search, false));
        gui_frame(&mut app, &ctx, vec![egui::Event::Text("GER INF".into())]);
        let out = gui_frame(&mut app, &ctx, vec![]);
        let row = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.text().contains("(GER_INF") => Some(t.galley.text().to_string()),
                _ => None,
            })
            .next()
            .unwrap_or_else(|| panic!("no infantry row: {}", gui_texts(&out, 380.0)));
        place(&mut app, &row, egui::vec2(0.53, 0.5), false);
        let spawns: Vec<String> = app.scenario.document().spawns.iter().map(|s| s.alias.clone()).collect();
        eprintln!("placed: {spawns:?}; panel: {:?}", app.spawn_ui.status());
        assert_eq!(spawns.len(), 2, "house + troop: {spawns:?} / {:?}", app.spawn_ui.status());
        assert!(app.has_unsaved_changes());

        gui_click(&mut app, &ctx, "Design Check");
        let report = app.design_check.report.clone().unwrap();
        let codes = design_codes(&app);
        // No official map has a reusable GER main-house template, so (as documented in the
        // panel and in C#) the new main house saves as a script construction site.
        assert!(codes.contains(&"construction-site".to_string()), "{codes:?}");
        assert_eq!(report.error_count(), 0, "{codes:?}");
        assert!(!codes.iter().any(|c| c == "team-no-main-house" || c == "team-no-units" || c == "no-start"), "{codes:?}");
        app.design_check.open = false;

        gui_click(&mut app, &ctx, "Safe Save");
        if app.pending_save_warnings.is_some() {
            gui_click(&mut app, &ctx, "Continue saving");
        }
        assert!(!app.status_is_error && !app.has_unsaved_changes(), "{}", app.status_message);
        let mut reopened = EditorApp::initial_state();
        reopened.set_loaded_map(dir.clone(), crate::storage::load_map(&dir).unwrap()).unwrap();
        reopened.scenario.load_session(&dir).unwrap();
        let reloaded: Vec<String> = reopened.scenario.document().spawns.iter().map(|s| s.alias.clone()).collect();
        assert_eq!(reloaded, spawns);
        std::fs::remove_dir_all(root).unwrap();
    }

    /// Retail gate (read-only): on ENDL_005's B8 ground most materials have no native corner
    /// tiles, so AI material strokes failed. Every material offered for the edit area must paint.
    #[test]
    #[ignore = "reads the installed game"]
    fn retail_ai_offered_materials_all_paint_on_endl_005() {
        let game = PathBuf::from(std::env::var("ARM_AI_GAME_ROOT").unwrap_or_else(|_| r"C:\Program Files (x86)\Against Rome".into()));
        let mut textures = TexturesUi::default();
        textures.load_session(&game.join("MAPS/ENDL_005")).unwrap();
        let offered = textures.palette_materials_blendable_in(8, 8, 48, 48);
        let ids: Vec<&str> = offered.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"PFAD"), "{ids:?}");
        assert!(!ids.contains(&"B5"), "sand has no B8 corner tiles: {ids:?}");
        let session = textures.session_mut().unwrap();
        for id in ids.iter().filter(|id| **id != "B8") {
            let result = session.paint_circle(32.5, 20.5, 2.5, id, true, false, None).unwrap();
            assert!(result.succeeded && !result.texture_changes.is_empty(), "{id} did not paint");
            session.commit_stroke();
            session.undo();
        }
    }

    /// Live gate: real Ollama Water + Materials specialists on a TEMP copy of a retail map
    /// (map, floortex.dat and the alias catalog are copied; the install is only read).
    /// `cargo test -p against-rome-editor-app --lib live_ollama_water_materials -- --ignored --nocapture`
    /// Env: `ARM_AI_GAME_ROOT` (default the retail install), `ARM_AI_MAP` (default ENDL_005).
    #[test]
    #[ignore = "requires a local Ollama server and an installed game to copy from"]
    fn live_ollama_water_materials_apply_undo_save_reload() {
        let game = PathBuf::from(std::env::var("ARM_AI_GAME_ROOT").unwrap_or_else(|_| r"C:\Program Files (x86)\Against Rome".into()));
        let map_id = std::env::var("ARM_AI_MAP").unwrap_or_else(|_| "ENDL_005".into());
        let root = temp_map_dir("ai_live_root");
        let dir = root.join("MAPS").join(&map_id);
        copy_tree(&game.join("MAPS").join(&map_id), &dir);
        std::fs::copy(game.join("floortex.dat"), root.join("floortex.dat")).unwrap();
        std::fs::create_dir_all(root.join("SYSTEM/CLAK")).unwrap();
        std::fs::copy(game.join("SYSTEM/CLAK/cl_scint.ini"), root.join("SYSTEM/CLAK/cl_scint.ini")).unwrap();

        let mut app = EditorApp::initial_state();
        app.set_loaded_map(dir.clone(), crate::storage::load_map(&dir).unwrap()).unwrap();
        app.ai_planner.open = true;
        app.sync_ai_materials();
        assert!(!app.ai_planner.material_options.is_empty(), "catalog materials offered");
        // ARM_AI_MATERIALS_ONLY / ARM_AI_PROMPT narrow a run to collect material evidence.
        app.ai_planner.role_checks = if std::env::var("ARM_AI_MATERIALS_ONLY").is_ok() { [false, false, true] } else { [false, true, true] };
        app.ai_planner.bounds_text = "8,8,48,48".into();
        app.ai_planner.prompt_description = std::env::var("ARM_AI_PROMPT").unwrap_or_else(|_| "Add a small lake near the middle of the area and paint a dirt road (PFAD) leading to its shore.".into());
        let water = app.water_surface;
        app.ai_planner.start_generation(water).unwrap();
        assert!(app.ai_planner.wait_generation(std::time::Duration::from_secs(20 * 60)), "timed out");
        eprintln!("status: {}\n{}", app.ai_planner.status_message, app.ai_planner.plan_summary);
        let plan = app.ai_planner.current_plan.clone().expect("plan");
        eprintln!("plan: {}", serde_json::to_string(&plan).unwrap());

        let heights_before = app.session.as_ref().unwrap().heights().to_vec();
        let textures_before = app.textures.session_textures().unwrap().to_vec();
        let result = app
            .ai_planner
            .apply_plan_to_session(app.session.as_mut().unwrap(), app.textures.session_mut(), water)
            .unwrap();
        eprintln!("{result:#?}");
        let heights_after = app.session.as_ref().unwrap().heights().to_vec();
        let textures_after = app.textures.session_textures().unwrap().to_vec();
        let changed_tiles = textures_before.iter().zip(&textures_after).filter(|(a, b)| a != b).count();
        eprintln!("material tiles changed: {changed_tiles}; height samples changed: {}", result.height_samples_changed);
        let dim = (textures_before.len() as f64).sqrt() as usize;
        for (i, (a, b)) in textures_before.iter().zip(&textures_after).enumerate() {
            let (tx, ty) = (i % dim, i / dim);
            if !(8..56).contains(&tx) || !(8..56).contains(&ty) {
                assert_eq!(a, b, "material tile ({tx},{ty}) outside the edit area changed");
            }
        }
        assert!(changed_tiles > 0 || heights_after != heights_before, "plan changed nothing");

        // Material layer: one undo reverts every AI material stroke, redo restores it.
        if changed_tiles > 0 {
            assert!(app.textures.material_undo());
            assert_eq!(app.textures.session_textures().unwrap(), &textures_before[..]);
            assert!(app.textures.material_redo());
            assert_eq!(app.textures.session_textures().unwrap(), &textures_after[..]);
        }

        app.safe_save_current();
        assert!(!app.status_is_error, "{}", app.status_message);
        let mut reopened = EditorApp::initial_state();
        reopened.set_loaded_map(dir.clone(), crate::storage::load_map(&dir).unwrap()).unwrap();
        assert_eq!(reopened.session.as_ref().unwrap().heights(), &heights_after[..]);
        reopened.textures.load_session(&dir).unwrap();
        assert_eq!(reopened.textures.session_textures().unwrap(), &textures_after[..]);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn design_check_uses_pending_scenario_events_and_keeps_errors_distinct() {
        let (mut app, dir, _) = start_camera_app(false);
        app.scenario.load_session(&dir).unwrap();
        let saved = app.design_check_report();
        assert!(design_check_codes(&saved)
            .iter()
            .any(|c| c == "scenario-native-only"));
        let action = |kind| against_rome_scenario::Action {
            kind,
            ..Default::default()
        };
        app.scenario
            .add_events(&[3, 4].map(|kind| against_rome_scenario::Event {
                name: format!("end{kind}"),
                actions: vec![action(kind)],
                ..Default::default()
            }))
            .unwrap();
        let before = save_artifact_snapshot(&dir);
        let flags = pending_panel_dirty_flags(&app);
        let events = app.scenario.document().clone();
        let report = app.design_check_report();
        assert!(
            !design_check_codes(&report)
                .iter()
                .any(|c| c.starts_with("scenario-no-") || c == "scenario-native-only"),
            "{report:?}"
        );
        assert_eq!(save_artifact_snapshot(&dir), before);
        assert_eq!(pending_panel_dirty_flags(&app), flags);
        assert_eq!(app.scenario.document(), &events);
        assert!(app.scenario.can_undo_events());
        // A blocking diagnostic stays an Error, distinct from the warnings.
        app.session = Some(TerrainSession::create_blank(1026, 32));
        let report = app.design_check_report();
        assert!(report.error_count() > 0, "{report:?}");
        assert!(!report.is_clean());
        assert_eq!(save_artifact_snapshot(&dir), before);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn safe_save_multifile_failure_restores_every_byte_and_preserves_edit_history() {
        use against_rome_maps::TransactionFailurePoint as Fail;
        for existing in [false, true] {
            for point in [
                Fail::DuringReplacement(0),
                Fail::DuringReplacement(1),
                Fail::AfterPath("soundscape.sndz".into()),
                Fail::BeforeFinalCommit,
            ] {
                let dir = temp_map_dir("transaction_rollback");
                let mut app = EditorApp::initial_state();
                app.session = Some(TerrainSession::create_blank(5, 32));
                app.map_path_input = dir.display().to_string();
                if existing {
                    app.ambience.mark_dirty();
                    app.safe_save_current();
                    assert!(!app.status_is_error, "{}", app.status_message);
                }
                std::fs::write(dir.join("skydens.dat"), b"cached original").unwrap();
                std::fs::write(dir.join("boden.bmp.arm_backup"), b"older recovery evidence")
                    .unwrap();
                std::fs::write(dir.join("unrelated.dat"), b"keep this").unwrap();
                std::fs::create_dir_all(dir.join("TEXT/US")).unwrap();
                std::fs::write(
                    dir.join("TEXT/US/briefing.put"),
                    briefing_fixture_with_extras(),
                )
                .unwrap();
                edit_save_terrain(&mut app);
                app.ambience.global_sound_id = "rain_new".into();
                app.ambience.profile.boden.waterlevel = 61.0;
                app.ambience.briefing_title = "Changed".into();
                app.ambience.briefing_subtitle = "Changed subtitle".into();
                app.ambience.briefing_team_names[1] = "Changed team".into();
                app.ambience.briefing_body = "Changed briefing".into();
                app.ambience.mark_dirty();
                let before = save_file_snapshot(&dir);
                let terrain = app.session.as_ref().unwrap();
                let raw = (
                    terrain.raw_boden_bmp.clone(),
                    terrain.raw_collision_bmp.clone(),
                );
                let baseline = terrain.original_heights().to_vec();
                app.safe_save_current_using(|tx| {
                    // Fail after *actual* writes in the same coordinator used by Safe Save.
                    let mut injected =
                        std::mem::replace(tx, against_rome_maps::MapSaveTransaction::new("unused"))
                            .with_injected_failure(point, "app integration failure");
                    injected.commit().map_err(|e| e.to_string())
                });
                assert!(app.status_is_error, "{}", app.status_message);
                assert!(
                    app.status_message.contains("CleanlyRestored"),
                    "{}",
                    app.status_message
                );
                assert_eq!(save_file_snapshot(&dir), before);
                let terrain = app.session.as_mut().unwrap();
                assert!(terrain.is_dirty());
                assert!(terrain.can_undo());
                assert_eq!(terrain.raw_boden_bmp, raw.0);
                assert_eq!(terrain.raw_collision_bmp, raw.1);
                assert_eq!(terrain.original_heights(), baseline);
                assert!(terrain.undo());
                assert_eq!(terrain.heights()[0], 32);
                assert!(terrain.redo());
                assert_eq!(terrain.heights()[0], 99);
                assert!(app.ambience.is_dirty());
                assert_eq!(app.ambience.briefing_title, "Changed");
                // Retry succeeds, persisted terrain and ambience reload together.
                app.safe_save_current();
                assert!(!app.status_is_error, "{}", app.status_message);
                assert!(!app.has_unsaved_changes());
                assert!(!dir.join("skydens.dat").exists());
                let terrain = load_map(&dir).unwrap();
                assert_eq!(terrain.heights()[0], 99);
                assert_eq!(terrain.collision()[0], 255);
                let mut ambience = AmbienceUi::default();
                ambience.load_from_map(&dir).unwrap();
                assert_eq!(ambience.global_sound_id, "rain_new");
                assert_eq!(ambience.profile.boden.waterlevel, 61.0);
                assert_eq!(ambience.briefing_title, "Changed");
                assert_eq!(ambience.briefing_body, "Changed briefing");
                assert_eq!(ambience.briefing_subtitle, "Changed subtitle");
                assert_eq!(ambience.briefing_team_names[1], "Changed team");
                let saved = against_rome_ambience::localization::BriefingTextTable::parse(
                    &String::from_utf8(std::fs::read(dir.join("TEXT/US/briefing.put")).unwrap())
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(saved.debriefing_win(), Some("Extra win text"));
                assert!(!dir.join("SCRIPT").join("briefing_us.put").exists());
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }

    const EMBOSS_SIZE: usize = 7;

    fn emboss_fixture_heights() -> Vec<u8> {
        (0..EMBOSS_SIZE * EMBOSS_SIZE)
            .map(|i| {
                let (x, y) = (i % EMBOSS_SIZE, i / EMBOSS_SIZE);
                20 + ((x * x * 7 + y * 13 + x * y * 3) % 50) as u8
            })
            .collect()
    }

    /// Map dir whose emboss green is an exact linear function of the height gradient,
    /// so the fitted light model is usable. Blue/red are distinctive to prove preservation.
    fn emboss_fixture_dir(tag: &str, with_emboss: bool) -> (PathBuf, Vec<u8>) {
        let dir = temp_map_dir(tag);
        let heights = emboss_fixture_heights();
        let mut boden = crate::bmp::BmpImage::new_solid(EMBOSS_SIZE, EMBOSS_SIZE, 1, 0, 2);
        boden.update_green_channel(&heights);
        std::fs::write(dir.join("boden.bmp"), boden.encode_bmp()).unwrap();
        let green: Vec<u8> = (0..heights.len())
            .map(|i| {
                let (gx, gy) = against_rome_editor::emboss::gradient(
                    &heights,
                    EMBOSS_SIZE,
                    i % EMBOSS_SIZE,
                    i / EMBOSS_SIZE,
                );
                against_rome_editor::emboss::quantize_f32(128.0 + 2.0 * gx - 1.5 * gy)
            })
            .collect();
        if with_emboss {
            let mut emboss = crate::bmp::BmpImage::new_solid(EMBOSS_SIZE, EMBOSS_SIZE, 7, 0, 9);
            emboss.update_green_channel(&green);
            std::fs::write(dir.join("emboss.bmp"), emboss.encode_bmp()).unwrap();
        }
        (dir, green)
    }

    fn emboss_app(dir: &Path) -> EditorApp {
        let mut app = EditorApp::initial_state();
        app.set_loaded_map(dir.to_path_buf(), load_map(dir).unwrap())
            .unwrap();
        app
    }

    fn edit_center_height(app: &mut EditorApp) {
        let terrain = app.session.as_mut().unwrap();
        terrain.apply_height_changes(&[SampleChange {
            index: 24,
            before: terrain.heights()[24],
            after: 120,
        }]);
        terrain.commit_stroke();
    }

    #[test]
    fn reset_terrain_discards_unsaved_heights_collision_and_flat_relight() {
        let (dir, _) = emboss_fixture_dir("reset_terrain", true);
        let mut app = emboss_app(&dir);
        assert!(!app.can_reset_terrain(), "nothing to reset yet");
        edit_center_height(&mut app);
        assert!(app.can_reset_terrain());
        app.request_terrain_action(TerrainBulkAction::Reset);
        assert_eq!(app.terrain_confirm, Some(TerrainBulkAction::Reset));
        assert!(app.apply_flat_terrain_now());
        assert!(app.reset_auxiliary_layers);
        assert!(app.emboss.as_ref().unwrap().session.is_relight_pending());

        assert!(app.reset_terrain_now());
        let session = app.session.as_ref().unwrap();
        assert_eq!(session.heights(), emboss_fixture_heights().as_slice());
        assert!(!session.is_dirty() && !session.can_undo(), "reset is the discard itself");
        assert!(!app.reset_auxiliary_layers);
        assert!(!app.emboss.as_ref().unwrap().session.is_relight_pending());
        assert!(!app.can_reset_terrain());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn flat_terrain_is_one_undo_step_and_resets_aux_layers_on_save() {
        let (dir, _) = emboss_fixture_dir("flat_terrain", true);
        for (name, green) in [("vertex.bmp", 10u8), ("smooth.bmp", 200u8)] {
            let mut layer = crate::bmp::BmpImage::new_solid(EMBOSS_SIZE, EMBOSS_SIZE, 3, 0, 4);
            layer.update_green_channel(&vec![green; EMBOSS_SIZE * EMBOSS_SIZE]);
            std::fs::write(dir.join(name), layer.encode_bmp()).unwrap();
        }
        let mut app = emboss_app(&dir);
        app.water_surface = 5.0;
        {
            let terrain = app.session.as_mut().unwrap();
            terrain.paint_collision(3.5, 0.5, 0.4, CollisionBrushOp::Block).unwrap();
            terrain.commit_stroke();
        }
        assert!(app.apply_flat_terrain_now(), "{}", app.status_message);
        let session = app.session.as_ref().unwrap();
        assert!(session.heights().iter().all(|h| *h == 25), "water 5 + 20");
        assert!(session.collision().iter().all(|c| *c == 0));
        assert!(app.reset_auxiliary_layers && app.has_unsaved_changes());

        // Heights and collision of the flatten are a single undo step on top of the earlier stroke.
        let session = app.session.as_mut().unwrap();
        assert!(session.undo());
        assert_eq!(session.heights(), emboss_fixture_heights().as_slice());
        assert_eq!(session.collision()[3], 255);
        assert!(session.redo());
        assert!(session.heights().iter().all(|h| *h == 25));

        app.safe_save_current();
        assert!(!app.status_is_error, "{}", app.status_message);
        let vertex = crate::bmp::BmpImage::parse(&std::fs::read(dir.join("vertex.bmp")).unwrap()).unwrap();
        let smooth = crate::bmp::BmpImage::parse(&std::fs::read(dir.join("smooth.bmp")).unwrap()).unwrap();
        assert!(vertex.extract_green_channel().iter().all(|g| *g == 255));
        assert!(smooth.extract_green_channel().iter().all(|g| *g == 0));
        assert!(!app.reset_auxiliary_layers, "flag clears after a committed save");
        let emboss = crate::bmp::BmpImage::parse(&std::fs::read(dir.join("emboss.bmp")).unwrap()).unwrap();
        let flat = emboss.extract_green_channel();
        assert!(flat.iter().all(|g| *g == flat[0]), "flat terrain gets one uniform light value");
        assert!(!app.emboss.as_ref().unwrap().session.is_relight_pending());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn flat_terrain_paints_the_most_common_base_material() {
        let dir = temp_map_dir("flat_material");
        std::fs::write(
            dir.join("boden.txt"),
            b"[Dimension]\n2\n[Texturen]\n4BA___51\n4BA___51\n4BA___51\n4BB___51\n",
        )
        .unwrap();
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.loaded_map_path = Some(dir.clone());
        app.map_path_input = dir.display().to_string();
        app.textures.load_session(&dir).unwrap();
        assert!(app.apply_flat_terrain_now(), "{}", app.status_message);
        let textures = app.textures.session_textures().unwrap();
        assert!(textures.iter().all(|t| t.eq_ignore_ascii_case("4BA___51")), "{textures:?}");
        assert!(app.textures.is_dirty());
        // Reset Terrain brings every texture (and the heights) back.
        assert!(app.reset_terrain_now());
        let textures = app.textures.session_textures().unwrap();
        assert!(textures[3].eq_ignore_ascii_case("4BB___51"));
        assert!(!app.textures.is_dirty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn terrain_bulk_actions_refuse_original_maps() {
        let root = temp_map_dir("flat_original");
        let dir = root.join("MAPS").join("ENDL_001");
        std::fs::create_dir_all(&dir).unwrap();
        let heights = emboss_fixture_heights();
        let mut boden = crate::bmp::BmpImage::new_solid(EMBOSS_SIZE, EMBOSS_SIZE, 1, 0, 2);
        boden.update_green_channel(&heights);
        std::fs::write(dir.join("boden.bmp"), boden.encode_bmp()).unwrap();
        let mut app = emboss_app(&dir);
        assert!(app.current_map_is_original());
        assert!(!app.can_apply_flat_terrain() && !app.can_reset_terrain());
        app.request_terrain_action(TerrainBulkAction::Flat);
        assert_eq!(app.terrain_confirm, None);
        assert!(!app.apply_flat_terrain_now() && app.status_is_error);
        assert!(!app.reset_terrain_now());
        assert_eq!(app.session.as_ref().unwrap().heights(), heights.as_slice());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn toolbar_undo_redo_route_to_material_strokes_when_material_tool_is_active() {
        let dir = temp_map_dir("toolbar_material_undo");
        std::fs::write(
            dir.join("boden.txt"),
            b"[Dimension]\n2\n[Texturen]\n4BA___51\n4BA___51\n4BA___51\n4BB___51\n",
        )
        .unwrap();
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.map_path_input = dir.display().to_string();
        app.textures.load_session(&dir).unwrap();
        let texture_session = app.textures.session_mut().unwrap();
        assert!(texture_session.stamp_texture(0, 0, "4BB___51").is_some());
        assert!(texture_session.commit_stroke());
        assert!(!app.can_undo_any(), "material history is only reachable from the Material tool");

        app.active_tool = ActiveTool::Material;
        assert!(app.can_undo_any() && !app.can_redo_any());
        app.perform_undo();
        assert!(app.textures.session_textures().unwrap()[0].eq_ignore_ascii_case("4BA___51"));
        assert!(app.can_redo_any() && !app.can_undo_any());
        app.perform_redo();
        assert!(app.textures.session_textures().unwrap()[0].eq_ignore_ascii_case("4BB___51"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn troop_display_follows_object_toggle_and_loaded_catalog() {
        use crate::viewport_3d::TroopObject;
        let ctx = egui::Context::default();
        let mut app = EditorApp::initial_state();
        let stale = TroopObject {
            spawn_index: 0,
            name: "FigX".into(),
            world: [0.0; 3],
            team: 0,
            angle: 0.0,
            is_marker: false,
        };
        app.viewport_3d.troop_objects = vec![stale];
        app.show_objects = false;
        app.refresh_troop_objects(&ctx);
        assert!(app.viewport_3d.troop_objects.is_empty(), "hidden objects drop the troop display");

        // With a loaded catalog the expansion tracks the scenario (empty scenario -> nothing to show).
        app.show_objects = true;
        app.troops.data = Some((
            TroopCatalog::new(Vec::new(), Default::default()),
            FormationLayoutDefinition::native_default(),
        ));
        app.viewport_3d.troop_objects = vec![TroopObject {
            spawn_index: 9,
            name: "stale".into(),
            world: [0.0; 3],
            team: 0,
            angle: 0.0,
            is_marker: true,
        }];
        app.refresh_troop_objects(&ctx);
        assert!(app.viewport_3d.troop_objects.is_empty());
        assert!(app.troops.key.is_some());
    }

    #[test]
    fn canvas_grid_follows_cell_size_and_toggle_defaults() {
        let app = EditorApp::initial_state();
        assert!(app.show_grid && app.show_objects, "C# checkboxes default to checked");
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(640.0, 640.0));
        let lines = canvas_grid_lines(rect, 64);
        assert_eq!(lines.len(), 65 * 4, "64 cells -> 65 lines per axis, shadow + light each");
        assert_eq!(lines[1].0, [Pos2::new(10.0, 20.0), Pos2::new(10.0, 660.0)]);
        assert!(lines[0].1 && !lines[1].1);
        // 5 px cells are below the C# readability threshold, so no grid is drawn.
        assert!(canvas_grid_lines(Rect::from_min_size(Pos2::ZERO, Vec2::splat(320.0)), 64).is_empty());
        assert!(canvas_grid_lines(rect, 0).is_empty());
    }

    #[test]
    fn safe_save_height_edit_stages_relit_emboss_and_syncs_baseline() {
        let (dir, green) = emboss_fixture_dir("emboss_ok", true);
        let mut app = emboss_app(&dir);
        edit_center_height(&mut app);
        let edited = app.session.as_ref().unwrap().heights().to_vec();
        let expected = against_rome_editor::emboss::EmbossSession::new(
            EMBOSS_SIZE,
            emboss_fixture_heights(),
            Some(green.clone()),
        )
        .unwrap()
        .build_emboss(&edited)
        .unwrap()
        .unwrap();
        assert!(app.emboss.as_ref().unwrap().session.light().is_usable());
        assert_ne!(expected, green, "fixture must change at least one vertex");
        let mut staged = false;
        app.safe_save_current_using(|tx| {
            staged = tx.has_staged("emboss.bmp") && tx.has_staged("boden.bmp");
            std::mem::replace(tx, against_rome_maps::MapSaveTransaction::new("unused"))
                .commit()
                .map_err(|e| e.to_string())
        });
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(staged);
        let saved =
            crate::bmp::BmpImage::parse(&std::fs::read(dir.join("emboss.bmp")).unwrap()).unwrap();
        assert_eq!(saved.extract_green_channel(), expected);
        assert!(saved.bgr_pixels.chunks(3).all(|p| p[0] == 7 && p[2] == 9));
        let baseline = app.emboss.as_ref().unwrap();
        assert_eq!(baseline.session.baseline_heights(), edited);
        assert_eq!(baseline.session.saved_emboss(), Some(expected.as_slice()));
        assert_eq!(
            baseline.raw.encode_bmp(),
            std::fs::read(dir.join("emboss.bmp")).unwrap()
        );
        // Nothing changed since: a second save must not stage emboss.bmp again.
        let before = std::fs::read(dir.join("emboss.bmp")).unwrap();
        app.safe_save_current_using(|tx| {
            assert!(!tx.has_staged("emboss.bmp"));
            std::mem::replace(tx, against_rome_maps::MapSaveTransaction::new("unused"))
                .commit()
                .map_err(|e| e.to_string())
        });
        assert_eq!(std::fs::read(dir.join("emboss.bmp")).unwrap(), before);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn safe_save_without_original_emboss_never_creates_one() {
        let (dir, _) = emboss_fixture_dir("emboss_missing", false);
        let mut app = emboss_app(&dir);
        assert!(app.emboss.is_none());
        edit_center_height(&mut app);
        app.safe_save_current_using(|tx| {
            assert!(tx.has_staged("boden.bmp"));
            assert!(!tx.has_staged("emboss.bmp"));
            std::mem::replace(tx, against_rome_maps::MapSaveTransaction::new("unused"))
                .commit()
                .map_err(|e| e.to_string())
        });
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(!dir.join("emboss.bmp").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn safe_save_collision_only_edit_leaves_emboss_untouched() {
        let (dir, _) = emboss_fixture_dir("emboss_noop", true);
        let original = std::fs::read(dir.join("emboss.bmp")).unwrap();
        let mut app = emboss_app(&dir);
        let terrain = app.session.as_mut().unwrap();
        terrain
            .paint_collision(0.5, 0.5, 0.5, CollisionBrushOp::Block)
            .unwrap();
        terrain.commit_stroke();
        assert!(terrain.is_dirty());
        app.safe_save_current_using(|tx| {
            assert!(tx.has_staged("collision.bmp"));
            assert!(!tx.has_staged("emboss.bmp"));
            std::mem::replace(tx, against_rome_maps::MapSaveTransaction::new("unused"))
                .commit()
                .map_err(|e| e.to_string())
        });
        assert!(!app.status_is_error, "{}", app.status_message);
        assert_eq!(std::fs::read(dir.join("emboss.bmp")).unwrap(), original);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn safe_save_emboss_rolls_back_with_boden_and_keeps_dirty_state() {
        use against_rome_maps::TransactionFailurePoint as Fail;
        for point in [
            Fail::AfterPath("emboss.bmp".into()),
            Fail::AfterPath("boden.bmp".into()),
            Fail::BeforeFinalCommit,
        ] {
            let (dir, green) = emboss_fixture_dir("emboss_rollback", true);
            let mut app = emboss_app(&dir);
            edit_center_height(&mut app);
            let edited = app.session.as_ref().unwrap().heights().to_vec();
            let before = save_file_snapshot(&dir);
            app.safe_save_current_using(|tx| {
                assert!(tx.has_staged("emboss.bmp"));
                let mut injected =
                    std::mem::replace(tx, against_rome_maps::MapSaveTransaction::new("unused"))
                        .with_injected_failure(point.clone(), "emboss injected failure");
                injected.commit().map_err(|e| e.to_string())
            });
            assert!(app.status_is_error, "{}", app.status_message);
            assert_eq!(save_file_snapshot(&dir), before, "{point:?}");
            let terrain = app.session.as_ref().unwrap();
            assert!(terrain.is_dirty() && terrain.can_undo());
            let baseline = app.emboss.as_ref().unwrap();
            assert_eq!(
                baseline.session.baseline_heights(),
                emboss_fixture_heights()
            );
            assert_eq!(baseline.session.saved_emboss(), Some(green.as_slice()));
            assert!(baseline.session.heights_dirty(&edited));
            // Retry commits both layers together.
            app.safe_save_current();
            assert!(!app.status_is_error, "{}", app.status_message);
            let saved =
                crate::bmp::BmpImage::parse(&std::fs::read(dir.join("emboss.bmp")).unwrap())
                    .unwrap();
            assert_ne!(saved.extract_green_channel(), green);
            assert_eq!(load_map(&dir).unwrap().heights()[24], 120);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    fn briefing_fixture_with_extras() -> String {
        let mut table = against_rome_ambience::localization::BriefingTextTable::create_empty();
        table.set_title1("Original");
        table.set_briefing_text("Original briefing");
        table.set_debriefing_win("Extra win text");
        table.to_script_text()
    }

    #[test]
    fn safe_save_staged_briefing_failure_leaves_no_terrain_ambience_or_briefing() {
        use against_rome_maps::TransactionFailurePoint as Fail;
        for point in [
            Fail::AfterPath("TEXT/US/briefing.put".into()),
            Fail::AfterPath("soundscape.sndz".into()),
            Fail::BeforeFinalCommit,
        ] {
            // Fresh map: every staged artifact (terrain, boden.ini, SNDZ, briefing) is new.
            let dir = temp_map_dir("briefing_rollback");
            let mut app = EditorApp::initial_state();
            app.session = Some(TerrainSession::create_blank(5, 32));
            app.map_path_input = dir.display().to_string();
            edit_save_terrain(&mut app);
            app.ambience.profile.boden.waterlevel = 61.0;
            app.ambience.briefing_title = "New title".into();
            app.ambience.mark_dirty();
            let before = save_file_snapshot(&dir);
            app.safe_save_current_using(|tx| {
                assert!(tx.has_staged("boden.bmp"));
                assert!(tx.has_staged("boden.ini"));
                assert!(tx.has_staged("soundscape.sndz"));
                assert!(tx.has_staged("TEXT/US/briefing.put"));
                let mut injected =
                    std::mem::replace(tx, against_rome_maps::MapSaveTransaction::new("unused"))
                        .with_injected_failure(point.clone(), "briefing stage failure");
                injected.commit().map_err(|e| e.to_string())
            });
            assert!(app.status_is_error, "{}", app.status_message);
            assert_eq!(save_file_snapshot(&dir), before, "{point:?}");
            for rel in [
                "boden.bmp",
                "boden.ini",
                "soundscape.sndz",
                "TEXT/US/briefing.put",
            ] {
                assert!(!dir.join(rel).exists(), "{rel} left behind for {point:?}");
            }
            assert!(app.ambience.is_dirty());
            assert!(app.session.as_ref().unwrap().is_dirty());
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn safe_save_rejects_each_unsupported_dirty_panel_before_any_write() {
        let unsupported_panels: [(usize, &str); 8] = [
            (3, "EventGraph"),
            (4, "Toolbox"),
            (5, "Maps"),
            (6, "Environment"),
            (7, "Landscape"),
            (8, "Packaging"),
            (9, "Campaign"),
            (10, "AI Planner"),
        ];
        for (panel, name) in unsupported_panels {
            let dir = temp_map_dir("unsupported_save");
            let mut app = EditorApp::initial_state();
            app.map_path_input = dir.display().to_string();
            pending_panel_edit(&mut app, panel, &dir);
            edit_save_terrain(&mut app);
            app.ambience.mark_dirty();
            let before = save_file_snapshot(&dir);
            let flags = pending_panel_dirty_flags(&app);
            app.safe_save_current();
            assert!(app.status_is_error);
            assert!(app.status_message.contains("unsupported dirty panel state"));
            assert!(app.status_message.contains(name), "{}", app.status_message);
            assert_eq!(save_file_snapshot(&dir), before);
            assert_eq!(pending_panel_dirty_flags(&app), flags);
            assert!(app.session.as_ref().unwrap().is_dirty());
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn safe_save_with_dirty_loaded_scenario_commits_bci_and_baseline() {
        let dir = temp_map_dir("scenario_save_ok");
        std::fs::create_dir_all(dir.join("SCRIPT")).unwrap();
        std::fs::write(
            dir.join("SCRIPT/ak_level.bci"),
            against_rome_scenario::BciImage::idle().serialize(),
        )
        .unwrap();
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.map_path_input = dir.display().to_string();
        app.scenario.load_session(&dir).unwrap();
        app.scenario.open = true;
        pending_panel_click(&mut app, &egui::Context::default(), 1, "新增事件");
        assert!(app.scenario.dirty());

        app.safe_save_current();

        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(!app.scenario.dirty());
        assert!(dir.join("arm_scenario.json").is_file());
        assert!(dir.join("arm_start_camera.json").is_file());
        assert!(dir.join("SCRIPT/ak_level.arm_original").is_file());
        assert!(dir.join("SCRIPT/ak_level.bci").is_file());
        assert!(std::fs::read_dir(dir.join("SCRIPT")).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".arm_backup.")
        }));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn scenario_graph_only_edits_are_unsaved_and_safe_save_rejects_unapplied_graph() {
        let dir = temp_map_dir("scenario_graph_dirty");
        let mut app = EditorApp::initial_state();
        app.map_path_input = dir.display().to_string();
        app.scenario.graph_ui_mut().mark_dirty();
        assert!(app.has_unsaved_changes());
        assert!(pending_panel_dirty_flags(&app)[1]);
        let before = save_file_snapshot(&dir);

        app.safe_save_current();

        assert!(app.status_is_error);
        assert!(
            app.status_message.contains("劇本尚未載入來源")
                || app.status_message.contains("未套用"),
            "{}",
            app.status_message
        );
        assert_eq!(save_file_snapshot(&dir), before);
        assert!(app.scenario.graph_ui().is_dirty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn safe_save_with_dirty_loaded_textures_commits_boden_and_baseline() {
        let dir = temp_map_dir("textures_save_ok");
        let original = b"[Dimension]\n2\n[Texturen]\n4BA___51\n4BA___51\n4BA___51\n4BB___51\n";
        std::fs::write(dir.join("boden.txt"), original).unwrap();
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.map_path_input = dir.display().to_string();
        app.textures.load_session(&dir).unwrap();
        let texture_session = app.textures.session_mut().unwrap();
        assert!(texture_session.stamp_texture(0, 0, "4BB___51").is_some());
        assert!(texture_session.commit_stroke());
        assert!(app.textures.is_dirty());

        app.safe_save_current();

        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(!app.textures.is_dirty());
        let saved = std::fs::read(dir.join("boden.txt")).unwrap();
        assert_ne!(saved, original);
        assert!(String::from_utf8_lossy(&saved).contains("4BB___51"));
        assert!(std::fs::read_dir(&dir).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("boden.txt.arm_backup.")
        }));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn combined_scenario_and_texture_transaction_failure_restores_disk_and_dirty_state() {
        use against_rome_maps::TransactionFailurePoint as Fail;
        let dir = temp_map_dir("scenario_textures_rollback");
        std::fs::create_dir_all(dir.join("SCRIPT")).unwrap();
        std::fs::write(
            dir.join("SCRIPT/ak_level.bci"),
            against_rome_scenario::BciImage::idle().serialize(),
        )
        .unwrap();
        let original_textures =
            b"[Dimension]\n2\n[Texturen]\n4BA___51\n4BA___51\n4BA___51\n4BB___51\n";
        std::fs::write(dir.join("boden.txt"), original_textures).unwrap();
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.map_path_input = dir.display().to_string();
        app.scenario.load_session(&dir).unwrap();
        app.scenario.open = true;
        pending_panel_click(&mut app, &egui::Context::default(), 1, "新增事件");
        app.textures.load_session(&dir).unwrap();
        let texture_session = app.textures.session_mut().unwrap();
        assert!(texture_session.stamp_texture(0, 0, "4BB___51").is_some());
        assert!(texture_session.commit_stroke());
        edit_save_terrain(&mut app);
        let before_disk = save_file_snapshot(&dir);

        app.safe_save_current_using(|tx| {
            let mut injected =
                std::mem::replace(tx, against_rome_maps::MapSaveTransaction::new("unused"))
                    .with_injected_failure(
                        Fail::BeforeFinalCommit,
                        "scenario/texture rollback verification",
                    );
            injected.commit().map_err(|e| e.to_string())
        });

        assert!(app.status_is_error, "{}", app.status_message);
        assert!(
            app.status_message.contains("CleanlyRestored"),
            "{}",
            app.status_message
        );
        assert_eq!(save_file_snapshot(&dir), before_disk);
        assert!(app.session.as_ref().unwrap().is_dirty());
        assert!(app.scenario.dirty());
        assert!(app.textures.is_dirty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    const SCENE_SDL: &str = "[settlement]
refpos=1000.0,50.0,2000.0

[object0000]
def=1700
namedef=BauRomHau00_Haupthaus
pos=100.00,10.00,100.00
team=0
angle=0.00
";

    fn scene_app(tag: &str) -> (EditorApp, PathBuf) {
        let dir = temp_map_dir(tag).join("MAPS").join("ENDL_005");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(against_rome_maps::CustomMapManifest::MARKER_FILE_NAME),
            b"",
        )
        .unwrap();
        std::fs::write(dir.join("Endlos_01_Siedlung01.sdl"), SCENE_SDL).unwrap();
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.map_path_input = dir.display().to_string();
        app.loaded_map_path = Some(dir.clone());
        app.scene.reset_session(Some(&dir));
        (app, dir)
    }

    #[test]
    fn safe_save_commits_scene_edits_and_clears_scene_dirty() {
        let (mut app, dir) = scene_app("scene_save_ok");
        assert!(!app.has_unsaved_changes() || app.session.as_ref().unwrap().is_dirty());
        app.scene.set_team(0, 4).unwrap();
        app.scene
            .duplicate_object(0, 1, [5.0, 6.0, 7.0], Some(45.0))
            .unwrap();
        assert!(app.has_unsaved_changes());
        assert!(app.require_clean_map_switch().is_err());

        app.safe_save_current();

        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(!app.scene.is_dirty());
        let objects = against_rome_maps::SdlSceneCatalog::load_directory(&dir).unwrap();
        assert_eq!(objects.len(), 2);
        assert!(objects.iter().any(|o| o.team == 4));
        assert!(objects.iter().any(|o| o.team == 1 && o.angle == Some(45.0)));
        std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap()).unwrap();
    }

    #[test]
    fn safe_save_scene_failure_restores_files_and_keeps_scene_dirty_and_undo() {
        use against_rome_maps::TransactionFailurePoint as Fail;
        let (mut app, dir) = scene_app("scene_save_fail");
        app.scene.set_team(0, 4).unwrap();
        let before = save_file_snapshot(&dir);

        app.safe_save_current_using(|tx| {
            let mut injected =
                std::mem::replace(tx, against_rome_maps::MapSaveTransaction::new("unused"))
                    .with_injected_failure(Fail::BeforeFinalCommit, "scene rollback");
            injected.commit().map_err(|e| e.to_string())
        });

        assert!(app.status_is_error, "{}", app.status_message);
        assert!(
            app.status_message.contains("CleanlyRestored"),
            "{}",
            app.status_message
        );
        assert_eq!(save_file_snapshot(&dir), before);
        assert!(app.scene.is_dirty() && app.scene.can_undo());
        std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap()).unwrap();
    }

    #[test]
    fn safe_save_dirty_scene_rejects_different_target_without_writes() {
        let (mut app, dir) = scene_app("scene_target_a");
        let other = temp_map_dir("scene_target_b");
        app.scene.set_team(0, 4).unwrap();
        app.map_path_input = other.display().to_string();

        app.safe_save_current();

        assert!(app.status_is_error, "{}", app.status_message);
        assert!(
            app.status_message.contains("differs"),
            "{}",
            app.status_message
        );
        assert!(app.scene.is_dirty());
        assert_eq!(std::fs::read_dir(&other).unwrap().count(), 0);
        std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap()).unwrap();
        std::fs::remove_dir_all(other).unwrap();
    }

    #[test]
    fn combined_scenario_and_texture_transaction_saves_both_and_clears_baselines() {
        let dir = temp_map_dir("scenario_textures_save_ok");
        std::fs::create_dir_all(dir.join("SCRIPT")).unwrap();
        std::fs::write(
            dir.join("SCRIPT/ak_level.bci"),
            against_rome_scenario::BciImage::idle().serialize(),
        )
        .unwrap();
        let original_textures =
            b"[Dimension]\n2\n[Texturen]\n4BA___51\n4BA___51\n4BA___51\n4BB___51\n";
        std::fs::write(dir.join("boden.txt"), original_textures).unwrap();
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.map_path_input = dir.display().to_string();
        app.scenario.load_session(&dir).unwrap();
        app.scenario.open = true;
        pending_panel_click(&mut app, &egui::Context::default(), 1, "新增事件");
        app.textures.load_session(&dir).unwrap();
        let texture_session = app.textures.session_mut().unwrap();
        assert!(texture_session.stamp_texture(0, 0, "4BB___51").is_some());
        assert!(texture_session.commit_stroke());
        edit_save_terrain(&mut app);

        app.safe_save_current();

        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(!app.session.as_ref().unwrap().is_dirty());
        assert!(!app.scenario.dirty());
        assert!(!app.textures.is_dirty());
        assert!(dir.join("arm_scenario.json").is_file());
        assert!(dir.join("arm_start_camera.json").is_file());
        assert!(dir.join("SCRIPT/ak_level.arm_original").is_file());
        assert!(dir.join("SCRIPT/ak_level.bci").is_file());
        assert_ne!(
            std::fs::read(dir.join("boden.txt")).unwrap(),
            original_textures
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn safe_save_with_dirty_placement_success_clears_dirty_and_roundtrips() {
        let dir = temp_map_dir("placement_save_ok");
        pending_placement_fixture(&dir);
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.map_path_input = dir.display().to_string();

        // 1. Explicitly load Placement session and open window
        app.placement.open = true;
        app.placement.load_session(&dir).unwrap();
        assert!(app.placement.is_loaded());
        assert!(!app.placement.is_dirty());

        // 2. Edit an object: snap to tile modifies position.dat
        let object_before = app.placement.objects()[0].clone();
        app.placement
            .select_object(Some((object_before.slot, object_before.uid)));
        let ctx = egui::Context::default();
        pending_panel_click(&mut app, &ctx, 0, "Snap to tile");
        assert!(app.placement.is_dirty());
        assert!(app.has_unsaved_changes());
        let pos_edited = app.placement.objects()[0].position;
        assert_ne!(pos_edited, object_before.position);

        // 3. Safe save current app
        app.safe_save_current();
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(app.status_message.starts_with("Saved safely"));
        assert!(!app.placement.is_dirty());
        assert!(!app.has_unsaved_changes());

        // 4. Roundtrip reload: new PlacementUi instance loads the saved map
        let mut reloaded = PlacementUi::default();
        reloaded.load_session(&dir).unwrap();
        assert!(!reloaded.is_dirty());
        let reloaded_objs = reloaded.objects();
        assert_eq!(reloaded_objs.len(), 3);
        assert_eq!(reloaded_objs[0].position, pos_edited);
        assert_eq!(reloaded_objs[0].slot, object_before.slot);
        assert_eq!(reloaded_objs[0].uid, object_before.uid);

        // 5. Verify PFIL container encoding roundtrip
        // Replace position.dat with a synthetic PFIL container, edit, and safe_save
        let orig_pos_bytes = std::fs::read(dir.join("DATA/position.dat")).unwrap();
        let mut pfil_pos = vec![0u8; 64];
        pfil_pos[..4].copy_from_slice(b"PFIL");
        pfil_pos[16..20].copy_from_slice(&(orig_pos_bytes.len() as u32).to_le_bytes());
        for part in orig_pos_bytes.chunks(8) {
            let ctrl = ((1u16 << part.len()) - 1) as u8;
            pfil_pos.push(ctrl);
            pfil_pos.extend_from_slice(part);
        }
        std::fs::write(dir.join("DATA/position.dat"), &pfil_pos).unwrap();

        // Reload PFIL placement into app, edit it, and safe_save
        app.placement.load_session(&dir).unwrap();
        assert!(!app.placement.is_dirty());
        app.placement.select_object(Some((0, 1)));
        app.placement
            .set_delta(["10".into(), "0".into(), "0".into()]);
        pending_panel_click(&mut app, &ctx, 0, "Move family");
        assert!(app.placement.is_dirty());
        let pos_pfil_edited = app.placement.objects()[0].position;

        app.safe_save_current();
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(!app.placement.is_dirty());

        // Verify disk position.dat still has PFIL magic and decodes properly
        let saved_disk_pfil = std::fs::read(dir.join("DATA/position.dat")).unwrap();
        assert!(saved_disk_pfil.starts_with(b"PFIL"));
        let mut pfil_reloaded = PlacementUi::default();
        pfil_reloaded.load_session(&dir).unwrap();
        assert_eq!(pfil_reloaded.objects()[0].position, pos_pfil_edited);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn safe_save_placement_failure_restores_disk_bytes_and_preserves_dirty_undo() {
        use against_rome_maps::TransactionFailurePoint as Fail;
        let dir = temp_map_dir("placement_fail_restore");
        pending_placement_fixture(&dir);
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.map_path_input = dir.display().to_string();

        app.placement.open = true;
        app.placement.load_session(&dir).unwrap();
        let before_disk = save_file_snapshot(&dir);

        // Edit placement
        app.placement.select_object(Some((0, 1)));
        let ctx = egui::Context::default();
        pending_panel_click(&mut app, &ctx, 0, "Snap to tile");
        assert!(app.placement.is_dirty());
        let undo_count = app.placement.undo_depth();
        assert!(undo_count > 0);

        // Inject transaction failure at BeforeFinalCommit
        app.safe_save_current_using(|tx| {
            let mut injected =
                std::mem::replace(tx, against_rome_maps::MapSaveTransaction::new("unused"))
                    .with_injected_failure(Fail::BeforeFinalCommit, "simulated disk failure");
            injected.commit().map_err(|e| e.to_string())
        });

        assert!(app.status_is_error, "{}", app.status_message);
        assert!(
            app.status_message.contains("CleanlyRestored"),
            "{}",
            app.status_message
        );

        // All disk files restored to before-edit bytes
        assert_eq!(save_file_snapshot(&dir), before_disk);

        // In-memory placement state remains dirty and undo stack is completely preserved
        assert!(app.placement.is_dirty());
        assert_eq!(app.placement.undo_depth(), undo_count);
        assert!(app.has_unsaved_changes());

        // Undo successfully rolls back the edit in memory
        app.placement.undo().unwrap();
        assert_eq!(app.placement.undo_depth(), undo_count - 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn safe_save_placement_dirty_fails_closed_when_another_panel_is_dirty() {
        let dir = temp_map_dir("placement_other_dirty_fails");
        pending_placement_fixture(&dir);
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.map_path_input = dir.display().to_string();

        app.placement.open = true;
        app.placement.load_session(&dir).unwrap();
        app.placement.select_object(Some((0, 1)));
        let ctx = egui::Context::default();
        pending_panel_click(&mut app, &ctx, 0, "Snap to tile");
        assert!(app.placement.is_dirty());

        // Dirty another panel (Scenario)
        app.scenario.open = true;
        pending_panel_click(&mut app, &ctx, 1, "新增事件");
        assert!(app.scenario.dirty());

        let before_disk = save_file_snapshot(&dir);
        app.safe_save_current();

        assert!(app.status_is_error);
        assert!(
            app.status_message.contains("劇本尚未載入來源"),
            "{}",
            app.status_message
        );

        // Disk unchanged, dirty state kept
        assert_eq!(save_file_snapshot(&dir), before_disk);
        assert!(app.placement.is_dirty());
        assert!(app.scenario.dirty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn safe_save_placement_fail_closed_on_unsupported_states() {
        let dir_a = temp_map_dir("placement_state_a");
        let dir_b = temp_map_dir("placement_state_b");
        pending_placement_fixture(&dir_a);
        pending_placement_fixture(&dir_b);

        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.map_path_input = dir_b.display().to_string();

        // 1. Mismatched map path: placement loaded from dir_a, saving to dir_b
        app.placement.open = true;
        app.placement.load_session(&dir_a).unwrap();
        app.placement.select_object(Some((0, 1)));
        let ctx = egui::Context::default();
        pending_panel_click(&mut app, &ctx, 0, "Snap to tile");
        assert!(app.placement.is_dirty());

        let before_a = save_file_snapshot(&dir_a);
        let before_b = save_file_snapshot(&dir_b);
        app.safe_save_current();

        assert!(app.status_is_error);
        assert!(
            app.status_message
                .contains("cannot be saved to different map path"),
            "{}",
            app.status_message
        );
        assert_eq!(save_file_snapshot(&dir_a), before_a);
        assert_eq!(save_file_snapshot(&dir_b), before_b);
        assert!(app.placement.is_dirty());

        // 2. Native retail pool guard: if DATA/objects.dat is retail pool format (stride 56)
        let retail_dir = temp_map_dir("placement_retail");
        pending_placement_fixture(&retail_dir);
        let count = 2u32;
        let mut retail_objects = vec![0u8; 8 + count as usize * 56];
        retail_objects[..4].copy_from_slice(&1u32.to_le_bytes());
        retail_objects[4..8].copy_from_slice(&count.to_le_bytes());
        std::fs::write(retail_dir.join("DATA/objects.dat"), &retail_objects).unwrap();

        app.map_path_input = retail_dir.display().to_string();
        app.safe_save_current();
        assert!(app.status_is_error);
        assert!(app.placement.is_dirty());

        let _ = std::fs::remove_dir_all(&dir_a);
        let _ = std::fs::remove_dir_all(&dir_b);
        let _ = std::fs::remove_dir_all(&retail_dir);
    }

    #[test]
    fn safe_save_serialization_and_recovery_preflights_never_create_destination() {
        let root = temp_map_dir("serialization_save");
        let dir = root.join("not-created");
        let mut app = EditorApp::initial_state();
        app.map_path_input = dir.display().to_string();
        app.ambience.profile.boden.waterlevel = f32::NAN;
        app.ambience.mark_dirty();
        app.safe_save_current();
        assert!(app.status_is_error);
        assert!(app.status_message.contains("boden.ini validation"));
        assert!(!dir.exists());
        assert!(app.ambience.is_dirty());
        app.ambience.profile.boden.waterlevel = 48.0;
        app.ambience.global_volume = f32::NAN;
        app.safe_save_current();
        assert!(app.status_is_error);
        assert!(app.status_message.contains("soundscape.sndz serialization"));
        assert!(!dir.exists());
        app.ambience.global_volume = 0.6;
        app.ambience.profile.wind_speed = 29.0;
        app.safe_save_current();
        assert!(app.status_is_error);
        assert!(app.status_message.contains("wind_speed"));
        assert!(!dir.exists());
        app.ambience.profile.wind_speed =
            against_rome_ambience::atmosphere::WeatherPresets::create_clear_sky().wind_speed;
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("precious.tmp_tx.evidence"), b"do not delete").unwrap();
        let before = save_file_snapshot(&dir);
        app.safe_save_current();
        assert!(app.status_is_error);
        assert!(app.status_message.contains("recovery/lock path"));
        assert_eq!(save_file_snapshot(&dir), before);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn safe_save_tiered_path_policy_enforcement() {
        use against_rome_maps::CustomMapManifest;

        let root = temp_map_dir("tiered_save_policy");

        // 1. External standalone map project: arbitrary name, no marker, safe_save succeeds
        let ext_dir = root.join("independent_projects").join("my_creative_map");
        let mut app_ext = EditorApp::initial_state();
        app_ext.session = Some(TerrainSession::create_blank(5, 32));
        app_ext.map_path_input = ext_dir.display().to_string();
        app_ext.safe_save_current();
        assert!(!app_ext.status_is_error, "{}", app_ext.status_message);
        assert!(ext_dir.join("boden.bmp").is_file());
        assert!(ext_dir.join("collision.bmp").is_file());

        // 2. Standard install path: Program Files (x86)\Against Rome
        let std_game = root.join("Program Files (x86)").join("Against Rome");
        let std_maps = std_game.join("MAPS");
        std::fs::create_dir_all(&std_maps).unwrap();

        // 2a. Unmarked ENDL_005 rejected
        let std_endl_005 = std_maps.join("ENDL_005");
        let mut app_unmarked = EditorApp::initial_state();
        app_unmarked.session = Some(TerrainSession::create_blank(5, 32));
        app_unmarked.map_path_input = std_endl_005.display().to_string();
        app_unmarked.safe_save_current();
        assert!(app_unmarked.status_is_error);
        assert!(
            app_unmarked.status_message.contains("安全拒絕")
                || app_unmarked.status_message.contains("Security Guard")
        );

        // 2b. Marker-backed ENDL_005 succeeds
        std::fs::create_dir_all(&std_endl_005).unwrap();
        let marker_json =
            r#"{"Slot":5,"SourceSlot":1,"CreatedAt":"2026-10-18T00:00:00Z","ToolVersion":"1.0.0"}"#;
        std::fs::write(
            std_endl_005.join(CustomMapManifest::MARKER_FILE_NAME),
            marker_json,
        )
        .unwrap();

        let mut app_marked = EditorApp::initial_state();
        app_marked.session = Some(TerrainSession::create_blank(5, 32));
        app_marked.map_path_input = std_endl_005.display().to_string();
        app_marked.safe_save_current();
        assert!(!app_marked.status_is_error, "{}", app_marked.status_message);
        assert!(std_endl_005.join("boden.bmp").is_file());

        // 3. Non-standard install path: D:\Games\Against Rome
        let nonstd_game = root.join("Games").join("Against Rome");
        let nonstd_maps = nonstd_game.join("MAPS");
        std::fs::create_dir_all(&nonstd_maps).unwrap();
        std::fs::write(nonstd_game.join("Against_Rome.exe"), b"GAME_EXE").unwrap();

        // 3a. Retail ENDL_001 in non-standard install rejected
        let nonstd_endl_001 = nonstd_maps.join("ENDL_001");
        std::fs::create_dir_all(&nonstd_endl_001).unwrap();
        let mut app_endl_001 = EditorApp::initial_state();
        app_endl_001.session = Some(TerrainSession::create_blank(5, 32));
        app_endl_001.map_path_input = nonstd_endl_001.display().to_string();
        app_endl_001.safe_save_current();
        assert!(app_endl_001.status_is_error);
        assert!(
            app_endl_001.status_message.contains("安全拒絕")
                || app_endl_001.status_message.contains("Security Guard")
        );
        assert!(!nonstd_endl_001.join("boden.bmp").exists());

        // 3b. Retail TUTOR_01 in non-standard install rejected
        let nonstd_tutor = nonstd_maps.join("TUTOR_01");
        std::fs::create_dir_all(&nonstd_tutor).unwrap();
        let mut app_tutor = EditorApp::initial_state();
        app_tutor.session = Some(TerrainSession::create_blank(5, 32));
        app_tutor.map_path_input = nonstd_tutor.display().to_string();
        app_tutor.safe_save_current();
        assert!(app_tutor.status_is_error);
        assert!(!nonstd_tutor.join("boden.bmp").exists());

        // 3c. Marker-backed ENDL_007 in non-standard install succeeds
        let nonstd_endl_007 = nonstd_maps.join("ENDL_007");
        std::fs::create_dir_all(&nonstd_endl_007).unwrap();
        let marker_007_json =
            r#"{"Slot":7,"SourceSlot":1,"CreatedAt":"2026-10-18T00:00:00Z","ToolVersion":"1.0.0"}"#;
        std::fs::write(
            nonstd_endl_007.join(CustomMapManifest::MARKER_FILE_NAME),
            marker_007_json,
        )
        .unwrap();

        let mut app_007 = EditorApp::initial_state();
        app_007.session = Some(TerrainSession::create_blank(5, 32));
        app_007.map_path_input = nonstd_endl_007.display().to_string();
        app_007.safe_save_current();
        assert!(!app_007.status_is_error, "{}", app_007.status_message);
        assert!(nonstd_endl_007.join("boden.bmp").is_file());

        let _ = std::fs::remove_dir_all(root);
    }

    fn pending_placement_fixture(map: &Path) {
        use against_rome_placement::{COLUMNS, SEGMENTS};
        use std::fs;
        let data = map.join("DATA");
        fs::create_dir_all(&data).unwrap();
        let n = 3usize;
        let mut objects = vec![0u8; 16 + n * (79 + COLUMNS.iter().sum::<usize>())];
        objects[..4].copy_from_slice(&1u32.to_le_bytes());
        objects[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        objects[8..12].copy_from_slice(&30u32.to_le_bytes());
        objects[12..16].copy_from_slice(&30u32.to_le_bytes());
        for slot in 0..n {
            let r = 16 + slot * 79;
            objects[r] = 1;
            objects[r + 3..r + 7].copy_from_slice(&(slot as u32 + 1).to_le_bytes());
            for offset in [67, 69] {
                objects[r + offset..r + offset + 2].copy_from_slice(&(slot as u16).to_le_bytes());
            }
            for offset in [71, 73, 77] {
                objects[r + offset..r + offset + 2].fill(255);
            }
            for col in 0..18 {
                let offset =
                    16 + n * 79 + n * COLUMNS[..col].iter().sum::<usize>() + slot * COLUMNS[col];
                if COLUMNS[col] == 2 {
                    objects[offset..offset + 2].fill(255);
                }
                if col == 1 {
                    objects[offset..offset + 2].copy_from_slice(&(slot as u16).to_le_bytes());
                }
            }
        }
        let mut position = vec![0u8; 8 + n * 17];
        position[..4].copy_from_slice(&1u32.to_le_bytes());
        position[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        for slot in 0..n {
            position[8 + slot * 17] = 1;
            position[9 + slot * 17..13 + slot * 17]
                .copy_from_slice(&(slot as f32 * 20.0 + 0.5).to_le_bytes());
        }
        let mut objdata = vec![0u8; 8 + n * SEGMENTS.iter().sum::<usize>()];
        objdata[..4].copy_from_slice(&1u32.to_le_bytes());
        objdata[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        for (name, bytes) in [
            ("objects", objects),
            ("position", position),
            ("objdata", objdata),
        ] {
            fs::write(data.join(format!("{name}.dat")), bytes).unwrap();
        }
    }

    // Drive public panel UI where loading/editing has no programmatic entry point.
    fn pending_panel_frame(
        app: &mut EditorApp,
        ctx: &egui::Context,
        panel: usize,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1800.0, 1400.0))),
                events,
                ..Default::default()
            },
            |ctx| match panel {
                0 => app.placement.show(ctx),
                1 => app.scenario.show(ctx),
                2 => app.textures.show(ctx),
                3 => app.show_studio_panels(ctx),
                4 => app.toolbox.show(ctx),
                _ => unreachable!(),
            },
        )
    }

    fn pending_panel_click_at(app: &mut EditorApp, ctx: &egui::Context, panel: usize, pos: Pos2) {
        for pressed in [true, false] {
            pending_panel_frame(
                app,
                ctx,
                panel,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }

    fn pending_panel_label(
        app: &mut EditorApp,
        ctx: &egui::Context,
        panel: usize,
        label: &str,
    ) -> Rect {
        // Settle the window's first-frame sizing before locating painted text.
        pending_panel_frame(app, ctx, panel, vec![]);
        let output = pending_panel_frame(app, ctx, panel, vec![]);
        output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.text() == label => {
                    Some(Rect::from_min_size(t.pos, t.galley.size()))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("panel {panel}: missing {label}"))
    }

    fn pending_panel_click(app: &mut EditorApp, ctx: &egui::Context, panel: usize, label: &str) {
        let pos = pending_panel_label(app, ctx, panel, label).center();
        pending_panel_click_at(app, ctx, panel, pos);
    }

    #[test]
    fn packaging_compare_button_captures_live_app_sessions_without_safe_save() {
        use crate::bmp::BmpImage;
        use crate::packaging_ui::{DiffVerdict, PackagingTab};
        let (mut app, root, map) = spawn_save_tests::fixture("packaging_live", &[(232, None)]);
        let _temp = PendingPanelTemp(root);
        app.spawn_ui.open = false;
        std::fs::write(
            map.join("boden.bmp"),
            BmpImage::new_solid(5, 5, 0, 32, 0).encode_bmp(),
        )
        .unwrap();
        std::fs::write(
            map.join("collision.bmp"),
            BmpImage::new_solid(4, 4, 0, 0, 0).encode_bmp(),
        )
        .unwrap();
        std::fs::write(
            map.join("boden.txt"),
            format!("[Dimension]\n4\n[Texturen]\n{}", "4BA___51\n".repeat(16)),
        )
        .unwrap();
        std::fs::write(
            map.join("arm_scenario.json"),
            app.scenario.document().to_json().unwrap(),
        )
        .unwrap();
        app.textures.load_session(&map).unwrap();
        app.placement.load_session(&map).unwrap();
        app.packaging.load_saved_baseline(&map).unwrap();
        app.textures
            .session_mut()
            .unwrap()
            .stamp_texture(0, 0, "4BB___51")
            .unwrap();
        app.textures.session_mut().unwrap().commit_stroke();
        app.placement
            .store_mut()
            .unwrap()
            .set_family_team(0, 1, 3)
            .unwrap();
        let event = serde_json::from_str(
            r#"{"Name":"LiveAppEvent","Actions":[{"Kind":0,"Text":"unsaved"}]}"#,
        )
        .unwrap();
        app.scenario.add_events(&[event]).unwrap();
        let texture_bytes = std::fs::read(map.join("boden.txt")).unwrap();
        let object_bytes = std::fs::read(map.join("DATA/objects.dat")).unwrap();
        let scenario_bytes = std::fs::read(map.join("arm_scenario.json")).unwrap();
        app.packaging.current_tab = PackagingTab::MapDiff;
        app.packaging.open = true;
        let ctx = egui::Context::default();
        pending_panel_click(&mut app, &ctx, 3, "Compare Working vs Baseline");
        assert_eq!(app.packaging.diff_verdict, Some(DiffVerdict::Differences));
        let report = app.packaging.diff_report.as_ref().unwrap();
        assert_eq!(report.textures.changed_tile_count, 1);
        assert_eq!(report.objects.modified.len(), 1);
        assert_eq!(report.events.added.len(), 1);
        assert_eq!(std::fs::read(map.join("boden.txt")).unwrap(), texture_bytes);
        assert_eq!(
            std::fs::read(map.join("DATA/objects.dat")).unwrap(),
            object_bytes
        );
        assert_eq!(
            std::fs::read(map.join("arm_scenario.json")).unwrap(),
            scenario_bytes
        );
        assert!(app.textures.is_dirty() && app.scenario.dirty());
    }

    fn pending_panel_path(
        app: &mut EditorApp,
        ctx: &egui::Context,
        panel: usize,
        label: &str,
        path: &Path,
    ) {
        let rect = pending_panel_label(app, ctx, panel, label);
        // These labels immediately precede their single-line path field.
        pending_panel_click_at(
            app,
            ctx,
            panel,
            Pos2::new(rect.left() + 40.0, rect.bottom() + 12.0),
        );
        pending_panel_frame(
            app,
            ctx,
            panel,
            vec![egui::Event::Text(path.display().to_string())],
        );
    }

    struct PendingPanelTemp(PathBuf);
    impl Drop for PendingPanelTemp {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn pending_panel_dirty_flags(app: &EditorApp) -> [bool; 12] {
        [
            app.placement.is_dirty(),
            app.scenario.dirty() || app.scenario.graph_ui().is_dirty(),
            app.textures.is_dirty(),
            app.event_graph.is_dirty(),
            app.toolbox.is_dirty(),
            app.maps.is_dirty(),
            app.environment.is_dirty(),
            app.landscape.is_dirty(),
            app.packaging.is_dirty(),
            app.campaign.is_dirty(),
            app.ai_planner.is_dirty(),
            app.ambience.is_dirty(),
        ]
    }

    fn pending_panel_edit(app: &mut EditorApp, panel: usize, dir: &Path) {
        let ctx = egui::Context::default();
        match panel {
            0 => {
                pending_placement_fixture(dir);
                app.placement.open = true;
                pending_panel_path(app, &ctx, panel, "MAP 完整路徑（包含 DATA 的資料夾）", dir);
                pending_panel_click(app, &ctx, panel, "Load DATA");
                assert!(app.placement.is_loaded());
                let object = app.placement.objects()[0].clone();
                app.placement.select_object(Some((object.slot, object.uid)));
                pending_panel_click(app, &ctx, panel, "Snap to tile");
                assert_ne!(app.placement.objects()[0].position, object.position);
            }
            1 => {
                app.scenario.open = true;
                pending_panel_click(app, &ctx, panel, "新增事件");
                assert_eq!(app.scenario.document().events.len(), 1);
            }
            2 => {
                std::fs::write(
                    dir.join("boden.txt"),
                    b"[Dimension]\n2\n[Texturen]\n4BA___51\n4BA___51\n4BA___51\n4BB___51\n",
                )
                .unwrap();
                app.textures.open = true;
                pending_panel_path(
                    app,
                    &ctx,
                    panel,
                    "MAP 資料夾（boden.txt）；材質保存與地形 / DATA 分開。",
                    dir,
                );
                pending_panel_click(app, &ctx, panel, "載入材質層");
                let session = app.textures.session_mut().expect("loaded texture session");
                assert!(session.stamp_texture(0, 0, "4BB___51").is_some());
                assert!(session.commit_stroke());
            }
            3 => {
                app.event_graph.graph.add_node(
                    crate::event_graph::model::GraphNode::new_message_action("guard regression"),
                );
                app.event_graph.mark_dirty();
            }
            4 => {
                let save = dir.join("SAVE/SAVE_001");
                std::fs::create_dir_all(&save).unwrap();
                std::fs::write(
                    save.join("save.ini"),
                    b"[titel]\nGuard\n[orglevelname]\nENDL_000\n",
                )
                .unwrap();
                app.toolbox.open_save_manager();
                pending_panel_path(app, &ctx, panel, "Game root", dir);
                pending_panel_path(app, &ctx, panel, "Backups root", &dir.join("backups"));
                pending_panel_click(app, &ctx, panel, "Scan saves / backups");
                pending_panel_click(
                    app,
                    &ctx,
                    panel,
                    "SAVE_001 | Guard | ENDL_000 | parsed=true",
                );
                app.toolbox
                    .review_save(crate::toolbox::SaveAction::Backup)
                    .unwrap();
            }
            5 => app.maps.mark_dirty(),
            6 => app.environment.mark_dirty(),
            7 => app.landscape.mark_dirty(),
            8 => app.packaging.mark_dirty(),
            9 => app.campaign.mark_dirty(),
            10 => app.ai_planner.mark_dirty(),
            11 => app.ambience.mark_dirty(),
            _ => unreachable!(),
        }
    }

    // Capture session identity, editable documents, history flags and switch-related app state.
    fn map_switch_snapshot(app: &EditorApp) -> Vec<String> {
        let terrain = app.session.as_ref().unwrap();
        vec![
            format!(
                "{:?}",
                (
                    terrain as *const TerrainSession,
                    terrain.vertex_size,
                    terrain.collision_size,
                    terrain.heights(),
                    terrain.collision(),
                    terrain.is_dirty(),
                    terrain.can_undo(),
                    terrain.can_redo()
                )
            ),
            format!(
                "{:?}",
                (
                    terrain.raw_boden_bmp.as_ref().map(|b| b.encode_bmp()),
                    terrain.raw_collision_bmp.as_ref().map(|b| b.encode_bmp())
                )
            ),
            format!(
                "{:?}",
                (
                    &app.map_path_input,
                    &app.status_message,
                    app.status_is_error,
                    app.height_step,
                    app.water_surface,
                    app.water_bed,
                    app.texture_dirty,
                    app.is_painting,
                    app.close_allowed,
                    app.view_mode,
                    app.pan,
                    app.zoom
                )
            ),
            format!(
                "{:?}",
                app.pending_action.as_ref().map(|a| match a {
                    PendingAction::Load(p) => format!("load:{}", p.display()),
                    PendingAction::Blank(s) => format!("blank:{s}"),
                    PendingAction::BuildBlank(intent) => format!(
                        "build_blank:{}:{}",
                        intent.game_path.display(),
                        intent.spec.target_slot
                    ),
                    PendingAction::Close => "close".into(),
                })
            ),
            format!("{:?}", pending_panel_dirty_flags(app)),
            format!("{:?}", app.placement.objects()),
            serde_json::to_string(app.scenario.document()).unwrap(),
            format!("{:?}", app.textures.session_textures()),
            format!("{:?}", app.event_graph.graph),
            app.toolbox.status().to_owned(),
            serde_json::to_string(&app.ambience.profile).unwrap(),
            format!(
                "{:?}",
                (
                    &app.ambience.briefing_title,
                    &app.ambience.briefing_body,
                    &app.ambience.status_message,
                    app.ambience.status_is_error
                )
            ),
        ]
    }

    #[test]
    fn public_map_switch_apis_reject_each_dirty_session_without_changing_app() {
        for dirty_kind in 0..13 {
            let dir = PendingPanelTemp(temp_map_dir("public_switch"));
            let mut app = EditorApp::initial_state();
            if dirty_kind == 12 {
                app.session
                    .as_mut()
                    .unwrap()
                    .apply_height_changes(&[SampleChange {
                        index: 0,
                        before: 32,
                        after: 71,
                    }]);
                assert!(app.session.as_ref().unwrap().is_dirty());
                assert_eq!(pending_panel_dirty_flags(&app), [false; 12]);
            } else {
                pending_panel_edit(&mut app, dirty_kind, &dir.0);
                let mut expected = [false; 12];
                expected[dirty_kind] = true;
                assert_eq!(pending_panel_dirty_flags(&app), expected);
                assert!(!app.session.as_ref().unwrap().is_dirty());
            }
            app.map_path_input = dir.0.to_string_lossy().into_owned();
            app.status_message = "retain current status".into();
            app.texture_dirty = false;
            app.pending_action = Some(PendingAction::Blank(193));
            let before = map_switch_snapshot(&app);
            let err = app
                .set_loaded_map(
                    dir.0.join("replacement"),
                    TerrainSession::create_blank(193, 99),
                )
                .unwrap_err();
            assert!(err.contains("unsaved changes"), "{dirty_kind}: {err}");
            assert_eq!(map_switch_snapshot(&app), before, "dirty kind {dirty_kind}");

            // Even an invalid root/spec must fail at the dirty guard, before builder validation.
            let game = dir.0.join("missing-game");
            let spec = against_rome_maps::blank_builder::BlankMapSpec::new("ENDL_000", 7, "Guard");
            let err = app
                .create_blank_map(&game, &spec)
                .err()
                .expect("dirty build rejected");
            assert!(err.contains("unsaved changes"), "{dirty_kind}: {err}");
            assert_eq!(map_switch_snapshot(&app), before, "dirty kind {dirty_kind}");
            assert!(!game.exists());
        }
    }

    #[test]
    fn public_map_switch_rejects_dirty_panel_without_terrain() {
        let dir = PendingPanelTemp(temp_map_dir("public_no_terrain"));
        let mut app = EditorApp::initial_state();
        app.session = None;
        app.ambience
            .load_preset(against_rome_ambience::atmosphere::WeatherPresets::STORM_ID);
        assert!(app.ambience.is_dirty());
        let profile = serde_json::to_vec(&app.ambience.profile).unwrap();
        let status = app.status_message.clone();
        assert!(app
            .set_loaded_map(dir.0.clone(), TerrainSession::create_blank(193, 77))
            .unwrap_err()
            .contains("unsaved changes"));
        let spec = against_rome_maps::blank_builder::BlankMapSpec::new("ENDL_000", 7, "Guard");
        assert!(app
            .create_blank_map(&dir.0, &spec)
            .err()
            .unwrap()
            .contains("unsaved changes"));
        assert!(app.session.is_none());
        assert!(app.map_path_input.is_empty());
        assert_eq!(app.status_message, status);
        assert!(!app.status_is_error);
        assert!(app.ambience.is_dirty());
        assert_eq!(serde_json::to_vec(&app.ambience.profile).unwrap(), profile);
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 0);
    }

    #[test]
    fn public_set_loaded_map_accepts_clean_session() {
        let dir = PendingPanelTemp(temp_map_dir("public_clean"));
        std::fs::write(
            dir.0.join("boden.ini"),
            b"[Heightmapstep]\n2.0\n[Waterlevel]\n30.0\n",
        )
        .unwrap();
        let mut app = EditorApp::initial_state();
        app.set_loaded_map(dir.0.clone(), TerrainSession::create_blank(193, 77))
            .unwrap();
        assert_eq!(app.session.as_ref().unwrap().vertex_size, 193);
        assert_eq!(app.session.as_ref().unwrap().heights()[0], 77);
        assert_eq!(app.map_path_input, dir.0.to_string_lossy());
        assert_eq!(app.height_step, 2.0);
        assert_eq!(app.water_surface, 15.0);
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(!app.has_unsaved_changes());
    }

    #[test]
    fn ui_explicit_discard_still_loads_over_dirty_sessions() {
        let dir = PendingPanelTemp(temp_map_dir("discard_load"));
        let mut terrain = TerrainSession::create_blank(193, 77);
        safe_save_map(&dir.0, &mut terrain).unwrap();
        for terrain_dirty in [false, true] {
            let mut app = EditorApp::initial_state();
            if terrain_dirty {
                app.session
                    .as_mut()
                    .unwrap()
                    .apply_height_changes(&[SampleChange {
                        index: 0,
                        before: 32,
                        after: 71,
                    }]);
            } else {
                pending_panel_edit(&mut app, 1, &dir.0);
            }
            app.pending_action = Some(PendingAction::Load(dir.0.clone()));
            let ctx = egui::Context::default();
            let _ = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
            let output = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
            let pos = output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::epaint::Shape::Text(t)
                        if t.galley.text() == "Discard unsaved changes" =>
                    {
                        Some(Rect::from_min_size(t.pos, t.galley.size()).center())
                    }
                    _ => None,
                })
                .expect("discard button");
            for pressed in [true, false] {
                let _ = ctx.run(
                    egui::RawInput {
                        events: vec![
                            egui::Event::PointerMoved(pos),
                            egui::Event::PointerButton {
                                pos,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        ..Default::default()
                    },
                    |ctx| app.handle_pending_action(ctx),
                );
            }
            assert!(app.pending_action.is_none());
            assert_eq!(app.session.as_ref().unwrap().vertex_size, 193);
            assert_eq!(app.session.as_ref().unwrap().heights()[0], 77);
            assert_eq!(app.map_path_input, dir.0.to_string_lossy());
            assert!(!app.status_is_error, "{}", app.status_message);
        }
    }

    fn assert_pending_non_terrain_guard(action_kind: usize) {
        let names = [
            "placement",
            "scenario",
            "textures",
            "event_graph",
            "toolbox",
            "maps",
            "environment",
            "landscape",
            "packaging",
            "campaign",
            "ai_planner",
            "ambience",
        ];
        for (panel, name) in names.iter().enumerate() {
            // Independent session for every action/category: terrain never masks a panel's dirty flag.
            let dir = PendingPanelTemp(temp_map_dir("pending_panel"));
            let missing_map = dir.0.join("missing-map");
            let mut app = EditorApp::initial_state();
            assert!(!app.has_unsaved_changes());
            pending_panel_edit(&mut app, panel, &dir.0);
            let mut expected_dirty = [false; 12];
            expected_dirty[panel] = true;
            assert_eq!(pending_panel_dirty_flags(&app), expected_dirty, "{name}");
            assert!(!app.session.as_ref().unwrap().is_dirty(), "{name}");
            app.map_path_input = "original-map-path".into();
            app.status_message = "original-status".into();
            app.status_is_error = false;
            let terrain = app.session.as_ref().unwrap();
            let terrain_ptr = terrain as *const TerrainSession;
            let heights = terrain.heights().to_vec();
            let objects = format!("{:?}", app.placement.objects());
            let scenario = serde_json::to_vec(app.scenario.document()).unwrap();
            let textures = app.textures.session_textures().map(|t| t.to_vec());
            let graph = format!("{:?}", app.event_graph.graph);
            let toolbox_status = app.toolbox.status().to_owned();
            app.pending_action = Some(match action_kind {
                0 => PendingAction::Load(missing_map.clone()),
                1 => PendingAction::Blank(193),
                2 => PendingAction::Close,
                3 => PendingAction::BuildBlank(BlankMapBuildIntent::new(
                    missing_map.clone(),
                    against_rome_maps::blank_builder::BlankMapSpec::new("ENDL_000", 7, "Guard"),
                )),
                _ => unreachable!(),
            });
            let ctx = egui::Context::default();
            let _ = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
            let output = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
            assert!(
                match (&app.pending_action, action_kind) {
                    (Some(PendingAction::Load(path)), 0) => path == &missing_map,
                    (Some(PendingAction::Blank(size)), 1) => *size == 193,
                    (Some(PendingAction::Close), 2) => true,
                    (Some(PendingAction::BuildBlank(intent)), 3) => intent.game_path == missing_map,
                    _ => false,
                },
                "{name}: action must be retained"
            );
            let text: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::epaint::Shape::Text(t) => Some(t.galley.text()),
                    _ => None,
                })
                .collect();
            assert!(text.contains(&"Discard unsaved changes"), "{name}");
            assert!(text.contains(&"Keep editing"), "{name}");
            assert!(!app.close_allowed, "{name}");
            assert_eq!(app.map_path_input, "original-map-path", "{name}");
            // A missing Load would report failure; Blank would change terrain size and status.
            assert_eq!(app.status_message, "original-status", "{name}");
            assert!(!app.status_is_error, "{name}");
            let terrain = app.session.as_ref().unwrap();
            assert_eq!(terrain as *const TerrainSession, terrain_ptr, "{name}");
            assert_eq!(terrain.vertex_size, 257, "{name}");
            assert_eq!(terrain.heights(), heights, "{name}");
            assert!(!terrain.is_dirty(), "{name}");
            assert_eq!(pending_panel_dirty_flags(&app), expected_dirty, "{name}");
            assert_eq!(format!("{:?}", app.placement.objects()), objects, "{name}");
            assert_eq!(
                serde_json::to_vec(app.scenario.document()).unwrap(),
                scenario,
                "{name}"
            );
            assert_eq!(
                app.textures.session_textures().map(|t| t.to_vec()),
                textures,
                "{name}"
            );
            assert_eq!(format!("{:?}", app.event_graph.graph), graph, "{name}");
            assert_eq!(app.toolbox.status(), toolbox_status, "{name}");
            assert!(app.has_unsaved_changes(), "{name}");
            assert!(!missing_map.exists());
        }
    }

    #[test]
    fn pending_non_terrain_load_requires_explicit_discard() {
        assert_pending_non_terrain_guard(0);
    }

    #[test]
    fn pending_non_terrain_blank_requires_explicit_discard() {
        assert_pending_non_terrain_guard(1);
    }

    #[test]
    fn pending_non_terrain_close_requires_explicit_discard() {
        assert_pending_non_terrain_guard(2);
    }

    #[test]
    fn pending_non_terrain_build_blank_requires_explicit_discard() {
        assert_pending_non_terrain_guard(3);
    }

    #[test]
    fn pending_clean_load_and_blank_are_allowed() {
        let ctx = egui::Context::default();
        let dir = PendingPanelTemp(temp_map_dir("pending_clean"));
        let mut terrain = TerrainSession::create_blank(193, 77);
        safe_save_map(&dir.0, &mut terrain).unwrap();
        let mut app = EditorApp::initial_state();
        assert!(!app.has_unsaved_changes());
        app.pending_action = Some(PendingAction::Load(dir.0.clone()));
        let _ = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
        assert!(app.pending_action.is_none());
        assert!(!app.status_is_error, "{}", app.status_message);
        assert_eq!(app.session.as_ref().unwrap().vertex_size, 193);
        assert_eq!(app.session.as_ref().unwrap().heights()[0], 77);
        assert_eq!(app.map_path_input, dir.0.to_string_lossy());
        assert!(!app.has_unsaved_changes());

        let mut app = EditorApp::initial_state();
        assert!(!app.has_unsaved_changes());
        app.map_path_input = "old-path".into();
        app.pending_action = Some(PendingAction::Blank(193));
        let _ = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
        assert!(app.pending_action.is_none());
        assert!(!app.status_is_error);
        assert_eq!(app.session.as_ref().unwrap().vertex_size, 193);
        assert_eq!(app.status_message, "Created blank 193x193 terrain");
        assert!(app.map_path_input.is_empty());
    }

    #[test]
    fn dirty_session_blocks_new_and_open_until_explicit_discard() {
        let mut app = EditorApp::initial_state();
        app.session
            .as_mut()
            .unwrap()
            .apply_height_changes(&[SampleChange {
                index: 0,
                before: 32,
                after: 99,
            }]);
        let ctx = egui::Context::default();
        for action in [
            PendingAction::Blank(193),
            PendingAction::Load(PathBuf::from("not-a-map")),
            PendingAction::BuildBlank(BlankMapBuildIntent::new(
                "missing-game",
                against_rome_maps::blank_builder::BlankMapSpec::new("ENDL_000", 7, "Guard"),
            )),
            PendingAction::Close,
        ] {
            app.pending_action = Some(action);
            let _ = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
            assert!(app.pending_action.is_some());
            assert_eq!(app.session.as_ref().unwrap().heights()[0], 99);
            assert!(!app.close_allowed);
        }
        // The exact action executed by the dialog's explicit discard button.
        app.pending_action = Some(PendingAction::Blank(193));
        let action = app.pending_action.take().unwrap();
        app.perform_action(action, &ctx);
        assert_eq!(app.session.as_ref().unwrap().vertex_size, 193);
    }

    #[test]
    fn dirty_build_blank_intent_cancels_cleanly_without_filesystem_mutation() {
        let temp_dir =
            std::env::temp_dir().join(format!("arm_cancel_test_{}", uuid::Uuid::new_v4()));
        let mut app = EditorApp::initial_state();
        app.session
            .as_mut()
            .unwrap()
            .apply_height_changes(&[SampleChange {
                index: 0,
                before: 32,
                after: 105,
            }]);
        assert!(app.has_unsaved_changes());

        let target_game = temp_dir.join("game_root");
        let spec =
            against_rome_maps::blank_builder::BlankMapSpec::new("ENDL_000", 9, "CancelGuard");
        let intent = BlankMapBuildIntent::new(target_game.clone(), spec);

        app.queue_build_intent(intent.clone());
        assert_eq!(
            app.pending_action(),
            Some(&PendingAction::BuildBlank(intent))
        );

        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
        let output = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));

        // Click "Keep editing" (Cancel)
        let pos = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.text() == "Keep editing" => {
                    Some(Rect::from_min_size(t.pos, t.galley.size()).center())
                }
                _ => None,
            })
            .expect("cancel button");
        for pressed in [true, false] {
            let _ = ctx.run(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ctx| app.handle_pending_action(ctx),
            );
        }

        assert!(app.pending_action().is_none());
        assert!(app.has_unsaved_changes(), "session must remain dirty");
        assert_eq!(app.session.as_ref().unwrap().heights()[0], 105);
        assert!(
            !target_game.exists(),
            "no filesystem mutation on cancelled build intent"
        );
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn public_execute_build_intent_rejects_dirty_session_without_mutation() {
        let temp_dir =
            std::env::temp_dir().join(format!("arm_public_intent_test_{}", uuid::Uuid::new_v4()));
        let mut app = EditorApp::initial_state();
        app.ambience
            .load_preset(against_rome_ambience::atmosphere::WeatherPresets::STORM_ID);
        assert!(app.ambience.is_dirty());
        assert!(app.has_unsaved_changes());

        let target_game = temp_dir.join("game_root");
        let spec =
            against_rome_maps::blank_builder::BlankMapSpec::new("ENDL_000", 9, "PublicGuard");
        let intent = BlankMapBuildIntent::new(target_game.clone(), spec);

        let err = app.execute_build_intent(&intent).unwrap_err();
        assert!(err.contains("unsaved changes"), "must reject: {err}");
        assert!(
            !target_game.exists(),
            "no filesystem mutation on rejected build intent"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn campaign_wave_merge_request_updates_scenario_session() {
        let mut app = EditorApp::initial_state();
        app.campaign.wave_count = 2;
        app.apply_campaign_wave_merge();
        assert!(app.scenario.document().events.is_empty());
        app.campaign.request_wave_merge();
        app.apply_campaign_wave_merge();
        let n = app.scenario.document().events.len();
        assert!(n > 0 && app.scenario.can_undo_events() && app.scenario.dirty());
        app.apply_campaign_wave_merge();
        assert_eq!(app.scenario.document().events.len(), n);
        assert!(app.scenario.undo_events());
        assert!(app.scenario.document().events.is_empty());
    }

    fn lair_test_event(name: &str) -> against_rome_environment::wild_lair::ScenarioEvent {
        use against_rome_environment::wild_lair::ScenarioAction;
        let mut e = against_rome_environment::wild_lair::ScenarioEvent::new(name, 60, true, true);
        e.actions.push(ScenarioAction::spawn_unit(1, "GER_INF01", 4000.0, 6000.0, 2));
        e
    }

    fn app_with_loaded_scenario_and_lair_events(tag: &str, n: usize) -> (EditorApp, PathBuf) {
        let dir = temp_map_dir(tag);
        std::fs::create_dir_all(dir.join("SCRIPT")).unwrap();
        std::fs::write(
            dir.join("SCRIPT/ak_level.bci"),
            against_rome_scenario::BciImage::idle().serialize(),
        )
        .unwrap();
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.map_path_input = dir.display().to_string();
        app.scenario.load_session(&dir).unwrap();
        app.environment.add_neutral_lair().unwrap();
        app.environment.generated_lair_events =
            (0..n).map(|i| lair_test_event(&format!("LAIR_t{i}"))).collect();
        app.environment.lair_event_counts = vec![n];
        (app, dir)
    }

    #[test]
    fn lair_events_apply_request_merges_once_with_one_undo_step() {
        let (mut app, dir) = app_with_loaded_scenario_and_lair_events("lair_apply_ok", 2);
        app.apply_lair_events_request();
        assert!(app.scenario.document().events.is_empty(), "no request, no merge");

        app.environment.request_lair_events_apply();
        app.apply_lair_events_request();
        assert!(
            !app.environment.status_is_error,
            "{}",
            app.environment.status_message
        );
        let events = &app.scenario.document().events;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].actions[0].kind, 2);
        assert_eq!(events[0].actions[0].alias, "GER_INF01");
        assert!(events[0].repeat && events[0].enabled);
        assert!(app.environment.generated_lair_events.is_empty());
        assert_eq!(app.environment.lair_event_counts, vec![0]);
        assert_eq!(app.environment.generated_lairs.len(), 1, "lairs stay pending");
        assert!(app.scenario.dirty() && app.scenario.can_undo_events());

        // The request was consumed; a repeated pass must not merge again.
        app.apply_lair_events_request();
        assert_eq!(app.scenario.document().events.len(), 2);

        assert!(app.scenario.undo_events());
        assert!(app.scenario.document().events.is_empty());
        assert!(!app.scenario.can_undo_events());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn lair_events_apply_refusals_keep_pending_state_and_block_saves() {
        // Unloaded scenario.
        let mut app = EditorApp::initial_state();
        app.environment.add_neutral_lair().unwrap();
        app.environment.generated_lair_events = vec![lair_test_event("LAIR_a")];
        app.environment.lair_event_counts = vec![1];
        app.environment.request_lair_events_apply();
        app.apply_lair_events_request();
        assert!(app.environment.status_is_error);
        assert_eq!(app.environment.generated_lair_events.len(), 1);
        assert_eq!(app.environment.lair_event_counts, vec![1]);
        assert!(app.scenario.document().events.is_empty() && !app.scenario.can_undo_events());

        // Dirty graph.
        let (mut app, dir) = app_with_loaded_scenario_and_lair_events("lair_apply_graph", 1);
        app.scenario.graph_ui_mut().mark_dirty();
        app.environment.request_lair_events_apply();
        app.apply_lair_events_request();
        assert!(app.environment.status_is_error);
        assert_eq!(app.environment.generated_lair_events.len(), 1);
        assert!(app.scenario.document().events.is_empty() && !app.scenario.can_undo_events());
        let _ = std::fs::remove_dir_all(dir);

        // 256-event cap, then saves stay blocked while lair state is pending.
        let (mut app, dir) = app_with_loaded_scenario_and_lair_events("lair_apply_cap", 2);
        let filler: Vec<_> = (0..255)
            .map(|i| against_rome_scenario::Event {
                name: format!("f{i}"),
                actions: vec![against_rome_scenario::Action {
                    text: "m".into(),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .collect();
        app.scenario.add_events(&filler).unwrap();
        app.environment.request_lair_events_apply();
        app.apply_lair_events_request();
        assert!(app.environment.status_is_error);
        assert!(app.environment.status_message.contains("limit"));
        assert_eq!(app.environment.generated_lair_events.len(), 2);
        assert_eq!(app.environment.lair_event_counts, vec![2]);
        assert_eq!(app.scenario.document().events.len(), 255);
        assert!(app.environment.is_dirty() && app.has_unsaved_changes());
        let before = std::fs::read_dir(&dir).unwrap().count();
        app.safe_save_current();
        assert!(app.status_is_error, "{}", app.status_message);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), before);
        assert!(!dir.join("arm_scenario.json").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn lair_events_rejected_by_scenario_validation_keep_pending_events() {
        let (mut app, dir) = app_with_loaded_scenario_and_lair_events("lair_apply_invalid", 1);
        // A condition on a spawn the scenario does not know is refused by add_events validation.
        app.environment.generated_lair_events[0].conditions.push(
            against_rome_environment::wild_lair::ScenarioCondition::new(
                against_rome_environment::wild_lair::ScenarioConditionKind::ObjectExists,
                uuid::Uuid::new_v4(),
            ),
        );
        app.environment.request_lair_events_apply();
        app.apply_lair_events_request();
        assert!(app.environment.status_is_error);
        assert_eq!(app.environment.generated_lair_events.len(), 1);
        assert_eq!(app.environment.lair_event_counts, vec![1]);
        assert!(app.scenario.document().events.is_empty() && !app.scenario.can_undo_events());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn studio_panels_render_and_guard_close() {
        let mut app = EditorApp::initial_state();
        let ctx = egui::Context::default();
        app.maps.open = true;
        app.environment.open = true;
        app.landscape.open = true;
        app.packaging.open = true;
        app.campaign.open = true;
        app.ai_planner.open = true;
        app.ambience.open = true;
        app.modifier.open = true;
        let _ = ctx.run(Default::default(), |ctx| app.show_studio_panels(ctx));
        assert!(!app.has_unsaved_changes());

        type Mark = fn(&mut EditorApp);
        let marks: [Mark; 7] = [
            |a| a.maps.mark_dirty(),
            |a| a.environment.mark_dirty(),
            |a| a.landscape.mark_dirty(),
            |a| a.packaging.mark_dirty(),
            |a| a.campaign.mark_dirty(),
            |a| a.ai_planner.mark_dirty(),
            |a| a.ambience.mark_dirty(),
        ];
        for mark in marks {
            let mut app = EditorApp::initial_state();
            mark(&mut app);
            assert!(app.has_unsaved_changes());
            app.pending_action = Some(PendingAction::Close);
            let _ = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
            assert!(app.pending_action.is_some());
            assert!(!app.close_allowed);
        }
    }

    #[test]
    fn landscape_apply_wall_without_objects_session_commits_collision_and_reports_note() {
        let mut app = EditorApp::initial_state();
        let ctx = egui::Context::default();
        app.landscape.wall_waypoints_text = "4,4\n8,4\n8,8".into();
        let cx = LandscapeContext::plain(257, app.session.as_ref().map(|s| s.heights()));
        app.landscape.plan_wall(&cx).unwrap();
        assert!(app.landscape.last_wall_plan.is_some());
        app.landscape.apply_requested = Some(LandscapeApply::Wall);

        let _ = ctx.run(Default::default(), |ctx| app.show_studio_panels(ctx));
        assert!(app.landscape.status_message.contains("wall applied"));
        assert!(app.landscape.status_message.contains("objects skipped"));
        assert!(app.session.as_ref().unwrap().is_dirty());
    }

    #[test]
    fn environment_apply_guards_unloaded_objects_session() {
        let mut app = EditorApp::initial_state();
        let ctx = egui::Context::default();
        app.environment.generate_flora_scatter(257, None, 0.0);
        assert!(!app.environment.generated_flora.is_empty());
        app.environment.apply_requested = Some(EnvApply::Flora);

        let _ = ctx.run(Default::default(), |ctx| app.show_studio_panels(ctx));
        assert!(app.environment.status_is_error);
        assert!(app
            .environment
            .status_message
            .contains("Load DATA in the Objects panel"));
    }

    #[test]
    fn neutral_wildlife_apply_clones_template_into_objects_session() {
        let dir = PendingPanelTemp(temp_map_dir("neutral_wildlife_apply"));
        make_test_map(&dir.0, 10001, "NeutralWildlife", "NeutralWildlife");
        make_test_placement_fixture_with_free_slot(&dir.0, 10001);

        let mut app = EditorApp::initial_state();
        let loaded = load_map(&dir.0).unwrap();
        app.set_loaded_map(dir.0.clone(), loaded).unwrap();
        app.placement.load_session(&dir.0).unwrap();
        let initial_count = app.placement.object_count();
        app.environment.flora_templates_text = "0,FigTieWol00_Wilder_Wolf".into();
        app.environment.lair_world_x = 256.0;
        app.environment.lair_world_z = 320.0;
        app.environment.add_neutral_lair().unwrap();

        app.handle_environment_apply(EnvApply::Lairs);

        assert!(
            !app.environment.status_is_error,
            "{}",
            app.environment.status_message
        );
        assert_eq!(app.placement.object_count(), initial_count + 1);
        let wolf = app.placement.objects().pop().unwrap();
        assert_eq!(wolf.type_id, 0);
        assert_eq!(wolf.team, 8);
        assert_eq!(wolf.position[0], 256.0);
        assert_eq!(wolf.position[2], 320.0);
        assert!(app.placement.is_dirty());
        assert!(app.environment.generated_lairs.is_empty());
        assert!(app
            .environment
            .status_message
            .contains("Queued 1 objects for the Objects session"));
        app.safe_save_current();
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(!app.placement.is_dirty());
        let saved = against_rome_placement::ObjectStore::load(&dir.0)
            .unwrap()
            .objects();
        let saved_wolf = saved.iter().find(|o| o.uid == wolf.uid).unwrap();
        assert_eq!(saved_wolf.team, 8);
        assert_eq!(saved_wolf.position[0], 256.0);
        assert_eq!(saved_wolf.position[2], 320.0);
    }

    #[test]
    fn neutral_wildlife_apply_rejects_unresolved_mixed_templates_without_mutation() {
        let dir = PendingPanelTemp(temp_map_dir("neutral_wildlife_unresolved"));
        make_test_map(&dir.0, 10001, "NeutralWildlife", "NeutralWildlife");
        make_test_placement_fixture_with_free_slot(&dir.0, 10001);

        let mut app = EditorApp::initial_state();
        let loaded = load_map(&dir.0).unwrap();
        app.set_loaded_map(dir.0.clone(), loaded).unwrap();
        app.placement.load_session(&dir.0).unwrap();
        let initial_count = app.placement.object_count();
        app.environment.flora_templates_text = "0,FigTieWol00_Wilder_Wolf".into();
        app.environment.add_neutral_lair().unwrap();
        app.environment.selected_lair_id = "ALL_BAE00".into();
        app.environment.lair_world_x = 512.0;
        app.environment.add_neutral_lair().unwrap();

        app.handle_environment_apply(EnvApply::Lairs);

        assert!(app.environment.status_is_error);
        assert!(app.environment.status_message.contains("FigTieBae00_Baer"));
        assert_eq!(app.placement.object_count(), initial_count);
        assert!(!app.placement.is_dirty());
        assert_eq!(app.environment.generated_lairs.len(), 2);
    }

    #[test]
    fn test_viewport_3d_mode_selection_and_headless_render() {
        let mut app = EditorApp::initial_state();
        let ctx = egui::Context::default();
        app.view_mode = ViewMode::View3D;
        assert_eq!(app.view_mode, ViewMode::View3D);

        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if app.view_mode == ViewMode::View3D {
                    if let Some(session) = &app.session {
                        let placement_objects = app.placement.objects();
                        let selected_obj = app.placement.selected_object();
                        let action = app.viewport_3d.show(
                            ui,
                            ctx,
                            session,
                            app.height_step,
                            app.water_surface,
                            app.brush_radius,
                            &placement_objects,
                            selected_obj,
                        );
                        app.handle_viewport_3d_action(action);
                    }
                }
            });
        });

        assert!(!app.has_unsaved_changes());
    }

    #[test]
    fn test_viewport_3d_action_routes_tools_and_preserves_dirty_undo() {
        let mut app = EditorApp::initial_state();
        app.view_mode = ViewMode::View3D;
        app.active_tool = ActiveTool::HeightRaise;
        app.brush_strength = 10;
        app.brush_radius = 4.0;

        let initial_h = app.session.as_ref().unwrap().heights()[64 * 257 + 64];

        app.handle_viewport_3d_action(ViewportAction::Paint {
            grid_x: 64.0,
            grid_y: 64.0,
        });
        assert!(app.is_painting);

        app.handle_viewport_3d_action(ViewportAction::CommitStroke);
        assert!(!app.is_painting);

        assert!(app.has_unsaved_changes());
        assert!(app.session.as_ref().unwrap().is_dirty());
        assert!(app.session.as_ref().unwrap().can_undo());

        let raised_h = app.session.as_ref().unwrap().heights()[64 * 257 + 64];
        assert!(
            raised_h > initial_h,
            "raised_h {raised_h} should exceed initial_h {initial_h}"
        );

        app.view_mode = ViewMode::Blended;
        assert!(app.has_unsaved_changes());
        app.view_mode = ViewMode::View3D;
        assert!(app.has_unsaved_changes());

        app.session.as_mut().unwrap().undo();
        let restored_h = app.session.as_ref().unwrap().heights()[64 * 257 + 64];
        assert_eq!(restored_h, initial_h);
    }

    #[test]
    fn test_viewport_3d_camera_manipulation() {
        let mut app = EditorApp::initial_state();
        let initial_yaw = app.viewport_3d.camera.yaw_degrees();
        let initial_pitch = app.viewport_3d.camera.pitch_degrees();

        app.viewport_3d.orbit(15.0, 10.0);
        assert_ne!(app.viewport_3d.camera.yaw_degrees(), initial_yaw);
        assert_ne!(app.viewport_3d.camera.pitch_degrees(), initial_pitch);

        let initial_dist = app.viewport_3d.camera.distance();
        app.viewport_3d.zoom(true);
        assert!(app.viewport_3d.camera.distance() < initial_dist);

        app.viewport_3d.reset_camera(64.0);
        assert_eq!(app.viewport_3d.camera.yaw_degrees(), 45.0);
        assert_eq!(app.viewport_3d.camera.pitch_degrees(), 30.0);
    }

    #[test]
    fn test_viewport_3d_shares_placement_state_and_select_action() {
        let mut app = EditorApp::initial_state();
        assert_eq!(app.placement.selected_object(), None);

        // Selecting via 3D viewport routes to shared placement state
        app.handle_viewport_3d_action(ViewportAction::SelectObject(Some((1, 42))));
        // When no store is loaded, invalid keys are safely rejected/cleared by placement_ui
        assert_eq!(app.placement.selected_object(), None);

        // Deselecting via 3D viewport also routes cleanly
        app.handle_viewport_3d_action(ViewportAction::SelectObject(None));
        assert_eq!(app.placement.selected_object(), None);
    }

    // Drive real egui pointer frames through the viewport ray picker and app route.
    fn ramp_viewport_frame(
        app: &mut EditorApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> ViewportAction {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))),
            events,
            ..Default::default()
        };
        let mut action = ViewportAction::None;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.viewport_3d.terrain_pick_mode = app.ramp_selecting;
                action = app.viewport_3d.show(
                    ui,
                    ctx,
                    app.session.as_ref().unwrap(),
                    4.0,
                    50.0,
                    4.0,
                    &[],
                    None,
                );
            });
        });
        app.handle_viewport_3d_action(action.clone());
        action
    }

    fn ramp_pointer(pos: Pos2, button: egui::PointerButton, pressed: bool) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: Default::default(),
            },
        ]
    }

    #[test]
    fn viewport_3d_ray_pick_to_ramp_click_and_drag_are_single_undo_operations() {
        for drag in [false, true] {
            let mut app = EditorApp::initial_state();
            let mut terrain = TerrainSession::create_blank(257, 32);
            let changes: Vec<_> = (0..257 * 257)
                .map(|index| SampleChange {
                    index,
                    before: 32,
                    after: (16 + (index % 257).pow(2) / 8192) as u8,
                })
                .collect();
            terrain.apply_height_changes(&changes);
            terrain.commit_stroke();
            terrain.commit_baseline();
            let before = terrain.heights().to_vec();
            let mut expected = TerrainSession::create_blank(257, 32);
            expected.apply_height_changes(&changes);
            expected.commit_stroke();
            expected.commit_baseline();
            app.session = Some(terrain);
            app.ramp_selecting = true;
            let ctx = egui::Context::default();
            ramp_viewport_frame(&mut app, &ctx, vec![]);
            let rect = app.viewport_3d.last_rect.unwrap();
            let start_pos = rect.center() - egui::vec2(60.0, 0.0);
            let end_pos = rect.center() + egui::vec2(60.0, 0.0);
            let first = ramp_viewport_frame(
                &mut app,
                &ctx,
                ramp_pointer(start_pos, egui::PointerButton::Primary, true),
            );
            let start = first
                .picked_terrain()
                .expect("press must ray-pick the ramp start");
            assert_eq!(app.ramp_start, Some(start));
            assert_eq!(app.session.as_ref().unwrap().heights(), before);
            assert!(!app.session.as_ref().unwrap().can_undo());
            if !drag {
                assert_eq!(
                    ramp_viewport_frame(
                        &mut app,
                        &ctx,
                        ramp_pointer(start_pos, egui::PointerButton::Primary, false)
                    ),
                    ViewportAction::None
                );
            } else {
                assert_eq!(
                    ramp_viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end_pos)]),
                    ViewportAction::None
                );
                assert_eq!(app.session.as_ref().unwrap().heights(), before);
            }
            let second = ramp_viewport_frame(
                &mut app,
                &ctx,
                ramp_pointer(end_pos, egui::PointerButton::Primary, !drag),
            );
            let end = second
                .picked_terrain()
                .expect("endpoint must come from a terrain ray hit");
            assert_ne!(start, end);
            let changes = expected
                .apply_ramp(
                    start,
                    end,
                    &against_rome_editor::terrain::RampParameters {
                        width: app.ramp_width,
                        falloff: app.ramp_falloff,
                        ..Default::default()
                    },
                )
                .unwrap();
            assert!(!changes.is_empty());
            assert_eq!(app.session.as_ref().unwrap().heights(), expected.heights());
            assert!(!app.is_painting);
            assert!(!app.ramp_selecting);
            assert!(app.texture_dirty);
            assert!(app.session.as_mut().unwrap().undo());
            assert_eq!(app.session.as_ref().unwrap().heights(), before);
            assert!(!app.session.as_ref().unwrap().can_undo());
            assert!(app.session.as_mut().unwrap().redo());
            assert_eq!(app.session.as_ref().unwrap().heights(), expected.heights());
        }
    }

    #[test]
    fn viewport_3d_ramp_cancel_miss_and_pending_action_preserve_terrain() {
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(257, 32));
        app.ramp_selecting = true;
        let ctx = egui::Context::default();
        ramp_viewport_frame(&mut app, &ctx, vec![]);
        let center = app.viewport_3d.last_rect.unwrap().center();
        ramp_viewport_frame(
            &mut app,
            &ctx,
            ramp_pointer(center, egui::PointerButton::Primary, true),
        );
        let start = app.ramp_start.unwrap();
        // Releasing outside the canvas must not complete a ramp using stale hover.
        assert_eq!(
            ramp_viewport_frame(
                &mut app,
                &ctx,
                ramp_pointer(Pos2::new(-50.0, -50.0), egui::PointerButton::Primary, false)
            ),
            ViewportAction::None
        );
        assert_eq!(app.ramp_start, Some(start));
        app.handle_viewport_3d_action(ViewportAction::Paint {
            grid_x: 100.0,
            grid_y: 100.0,
        });
        assert!(!app.is_painting);
        app.pending_action = Some(PendingAction::Close);
        app.handle_viewport_3d_action(ViewportAction::PickTerrain {
            grid_x: 120.0,
            grid_y: 120.0,
        });
        assert_eq!(app.ramp_start, Some(start));
        app.pending_action = None;
        ramp_viewport_frame(
            &mut app,
            &ctx,
            ramp_pointer(center, egui::PointerButton::Secondary, true),
        );
        assert_eq!(
            ramp_viewport_frame(
                &mut app,
                &ctx,
                ramp_pointer(center, egui::PointerButton::Secondary, false)
            ),
            ViewportAction::CancelTerrainPick
        );
        assert!(!app.ramp_selecting);
        assert_eq!(app.ramp_start, None);
        assert!(!app.session.as_ref().unwrap().is_dirty());
        assert!(!app.session.as_ref().unwrap().can_undo());
    }

    #[test]
    fn test_viewport_3d_terrain_pick_routes_to_ramp_without_painting() {
        let mut app = EditorApp::initial_state();
        app.view_mode = ViewMode::View3D;
        app.ramp_selecting = true;

        app.handle_viewport_3d_action(ViewportAction::PickTerrain {
            grid_x: 23.0,
            grid_y: 41.0,
        });

        assert_eq!(app.ramp_start, Some((23.0, 41.0)));
        assert!(app.ramp_selecting);
        assert!(!app.is_painting);

        // A normal terrain pick outside Ramp mode must not start a brush stroke.
        app.ramp_selecting = false;
        app.handle_viewport_3d_action(ViewportAction::PickTerrain {
            grid_x: 24.0,
            grid_y: 42.0,
        });
        assert_eq!(app.ramp_start, Some((23.0, 41.0)));
        assert!(!app.is_painting);
    }

    #[test]
    fn test_tech_doc_app_integration() {
        let mut app = EditorApp::initial_state();
        assert!(!app.tech_doc.is_open());

        app.tech_doc.open();
        assert!(app.tech_doc.is_open());

        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            app.tech_doc.show(ctx);
        });
        assert!(app.tech_doc.is_open());
        assert!(!app.has_unsaved_changes());

        app.tech_doc.close();
        assert!(!app.tech_doc.is_open());
    }

    fn make_test_placement_fixture(map: &Path, base_uid: u32) {
        make_test_placement_fixture_slots(map, base_uid, 2);
    }

    fn make_test_placement_fixture_with_free_slot(map: &Path, base_uid: u32) {
        make_test_placement_fixture_slots(map, base_uid, 3);
    }

    fn make_test_placement_fixture_slots(map: &Path, base_uid: u32, slots: usize) {
        use against_rome_placement::{COLUMNS, SEGMENTS};
        use std::fs;
        let data = map.join("DATA");
        fs::create_dir_all(&data).unwrap();
        let n = slots;
        let mut objects = vec![0u8; 16 + n * (79 + COLUMNS.iter().sum::<usize>())];
        objects[..4].copy_from_slice(&1u32.to_le_bytes());
        objects[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        objects[8..12].copy_from_slice(&30u32.to_le_bytes());
        objects[12..16].copy_from_slice(&30u32.to_le_bytes());
        for slot in 0..n {
            let r = 16 + slot * 79;
            objects[r] = u8::from(slot < 2);
            objects[r + 3..r + 7].copy_from_slice(&(base_uid + slot as u32).to_le_bytes());
            for offset in [67, 69] {
                objects[r + offset..r + offset + 2].copy_from_slice(&(slot as u16).to_le_bytes());
            }
            for offset in [71, 73, 77] {
                objects[r + offset..r + offset + 2].fill(255);
            }
            for col in 0..18 {
                let offset =
                    16 + n * 79 + n * COLUMNS[..col].iter().sum::<usize>() + slot * COLUMNS[col];
                if COLUMNS[col] == 2 {
                    objects[offset..offset + 2].fill(255);
                }
                if col == 1 {
                    objects[offset..offset + 2].copy_from_slice(&(slot as u16).to_le_bytes());
                }
            }
        }
        let mut position = vec![0u8; 8 + n * 17];
        position[..4].copy_from_slice(&1u32.to_le_bytes());
        position[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        for slot in 0..n {
            position[8 + slot * 17] = u8::from(slot < 2);
            position[9 + slot * 17..13 + slot * 17]
                .copy_from_slice(&(slot as f32 * 20.0 + 0.5).to_le_bytes());
        }
        let mut objdata = vec![0u8; 8 + n * SEGMENTS.iter().sum::<usize>()];
        objdata[..4].copy_from_slice(&1u32.to_le_bytes());
        objdata[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        for (name, bytes) in [
            ("objects", objects),
            ("position", position),
            ("objdata", objdata),
        ] {
            fs::write(data.join(format!("{name}.dat")), bytes).unwrap();
        }
    }

    fn make_test_daynight_bmp() -> Vec<u8> {
        let width = 24u32;
        let height = 6u32;
        let stride = ((width * 3 + 3) / 4) * 4;
        let image_size = stride * height;
        let file_size = 54 + image_size;
        let mut bmp = vec![0u8; file_size as usize];
        bmp[0..2].copy_from_slice(b"BM");
        bmp[2..6].copy_from_slice(&file_size.to_le_bytes());
        bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&width.to_le_bytes());
        bmp[22..26].copy_from_slice(&height.to_le_bytes());
        bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
        bmp[28..30].copy_from_slice(&24u16.to_le_bytes());
        bmp[34..38].copy_from_slice(&image_size.to_le_bytes());
        for b in &mut bmp[54..] {
            *b = 120;
        }
        bmp
    }

    fn make_test_soundscape(zone_name: &str, sound_id: &str) -> Vec<u8> {
        use against_rome_ambience::soundscape::{
            SoundscapeBinaryStorage, SoundscapeSnapshot, SoundscapeZone,
        };
        let snapshot = SoundscapeSnapshot {
            global_ambience_id: sound_id.to_string(),
            global_ambience_volume: 0.75,
            zones: vec![SoundscapeZone {
                name: zone_name.to_string(),
                sound_def_id: sound_id.to_string(),
                ..Default::default()
            }],
        };
        SoundscapeBinaryStorage::serialize_to_binary(&snapshot).unwrap()
    }

    fn make_test_briefing(title: &str, body: &str) -> String {
        use against_rome_ambience::localization::BriefingTextTable;
        let mut table = BriefingTextTable::create_empty();
        table.set_title1(title);
        table.set_briefing_text(body);
        table.to_script_text()
    }

    fn make_test_map(dir: &Path, base_uid: u32, zone_name: &str, briefing_title: &str) {
        let mut terrain = TerrainSession::create_blank(193, 77);
        safe_save_map(dir, &mut terrain).unwrap();
        std::fs::write(
            dir.join("boden.ini"),
            b"[Heightmapstep]\n2.0\n[Waterlevel]\n30.0\n",
        )
        .unwrap();
        make_test_placement_fixture(dir, base_uid);
        std::fs::write(dir.join("daynight.bmp"), make_test_daynight_bmp()).unwrap();
        std::fs::write(
            dir.join("soundscape.sndz"),
            make_test_soundscape(zone_name, &format!("SND_{zone_name}")),
        )
        .unwrap();
        let script_dir = dir.join("SCRIPT");
        std::fs::create_dir_all(&script_dir).unwrap();
        std::fs::write(
            script_dir.join("briefing.put"),
            make_test_briefing(briefing_title, "Mission description"),
        )
        .unwrap();
        std::fs::write(dir.join("boden.txt"), b"; boden\r\n[Dimension]\r\n2\r\n[Texturen]\r\n4BA___51\r\n4BA___51\r\n4BA___51\r\n4BA___51\r\n").unwrap();
    }

    #[test]
    fn test_map_switch_a_to_b_clears_all_map_a_documents_and_plans_while_preserving_preferences() {
        let dir_a = PendingPanelTemp(temp_map_dir("switch_a"));
        let dir_b = PendingPanelTemp(temp_map_dir("switch_b"));
        make_test_map(&dir_a.0, 10001, "Zone_A", "Briefing_A");
        make_test_map(&dir_b.0, 20001, "Zone_B", "Briefing_B");

        let mut app = EditorApp::initial_state();
        let loaded_a = load_map(&dir_a.0).unwrap();
        app.set_loaded_map(dir_a.0.clone(), loaded_a).unwrap();

        // Objects, materials, and scenario documents are explicit panel loads.
        app.placement.load_session(&dir_a.0).unwrap();
        app.textures.load_session(&dir_a.0).unwrap();
        app.scenario.load_session(&dir_a.0).unwrap();

        // Verify Map A documents loaded
        assert_eq!(app.placement.objects()[0].uid, 10001);
        assert_eq!(app.ambience.zones[0].name, "Zone_A");
        assert_eq!(app.ambience.briefing_title, "Briefing_A");

        // Set user preferences
        app.placement.open = true;
        app.placement
            .set_delta(["12.5".into(), "0".into(), "-5.5".into()]);
        app.textures.open = true;
        app.scenario.open = true;
        app.event_graph.open = true;
        app.maps.open = true;
        app.environment.open = true;
        app.landscape.open = true;
        app.packaging.open = true;
        app.campaign.open = true;
        app.ai_planner.open = true;
        app.view_mode = ViewMode::View3D;
        app.brush_radius = 24.0;

        // Set Map A-bound transient & pending states
        app.placement.select_object(Some((0, 10001)));
        let event = against_rome_scenario::Event {
            name: "Evt A".into(),
            delay_seconds: 5,
            repeat: false,
            enabled: true,
            actions: vec![],
            conditions: vec![],
        };
        app.event_graph.load_from_scenario_events(&[event]);
        let node_ids_a: Vec<uuid::Uuid> =
            app.event_graph.graph.nodes.iter().map(|n| n.id).collect();
        assert!(!node_ids_a.is_empty());

        app.landscape.last_cliff_result =
            Some(against_rome_landscape::cliff::CliffDetectionResult {
                dimension: 64,
                cliff_cells: vec![],
                contours: vec![],
            });
        app.landscape.apply_requested = Some(LandscapeApply::Cliff);
        app.environment.generated_flora =
            vec![against_rome_environment::nature::NatureAddition::new(
                1, "Tree", 10.0, 0.0, 10.0, 0.0,
            )];
        app.environment.apply_requested = Some(EnvApply::Flora);
        app.ai_planner.current_plan = Some(against_rome_ai_planner::AiMapPlan::default());
        app.ai_planner.plan_summary = "Plan for A".into();
        app.packaging.diff_summary_text = "Diff for A".into();
        app.campaign.objectives =
            vec![against_rome_campaign::objectives::ObjectiveDefinition::default()];
        app.maps.diagnostic_issues = vec![against_rome_maps::diagnostics::MapIssue::new(
            against_rome_maps::diagnostics::MapIssueSeverity::Warning,
            "MAP_A_ISSUE",
            "說明",
            "Description",
        )];

        // Generated results and pending plans are unsaved work: retain them until
        // the user explicitly approves Discard through the normal switch dialog.
        assert!(app.has_unsaved_changes());
        let loaded_b = load_map(&dir_b.0).unwrap();
        assert!(app.set_loaded_map(dir_b.0.clone(), loaded_b).is_err());
        assert_eq!(app.map_path_input, dir_a.0.to_string_lossy());
        assert!(!app.environment.generated_flora.is_empty());
        app.pending_action = Some(PendingAction::Load(dir_b.0.clone()));
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
        let output = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
        let pos = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.text() == "Discard unsaved changes" => {
                    Some(Rect::from_min_size(t.pos, t.galley.size()).center())
                }
                _ => None,
            })
            .expect("discard button");
        for pressed in [true, false] {
            let _ = ctx.run(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ctx| app.handle_pending_action(ctx),
            );
        }
        assert!(app.pending_action.is_none());
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(!app.has_unsaved_changes());

        // Verify Map A documents are completely gone and replaced by Map B
        assert_eq!(app.map_path_input, dir_b.0.to_string_lossy());
        assert_eq!(app.placement.loaded_map(), None);
        assert!(app.placement.objects().is_empty());
        assert_eq!(app.placement.selected_object(), None);

        assert_eq!(app.ambience.zones.len(), 1);
        assert_eq!(app.ambience.zones[0].name, "Zone_B");
        assert_eq!(app.ambience.briefing_title, "Briefing_B");

        for id in node_ids_a {
            assert!(!app.event_graph.graph.nodes.iter().any(|n| n.id == id));
        }

        // Verify derived panels were reset
        assert!(app.landscape.last_cliff_result.is_none());
        assert!(app.landscape.apply_requested.is_none());
        assert!(app.environment.generated_flora.is_empty());
        assert!(app.environment.apply_requested.is_none());
        assert!(app.ai_planner.current_plan.is_none());
        assert!(app.ai_planner.plan_summary.is_empty());
        assert!(app.packaging.diff_summary_text.is_empty());
        assert!(app.campaign.objectives.is_empty());
        assert!(app.maps.diagnostic_issues.is_empty());

        // Verify user preferences are preserved
        assert!(app.placement.open);
        assert_eq!(app.placement.delta(), &["12.5", "0", "-5.5"]);
        assert!(app.textures.open);
        assert!(app.scenario.open);
        assert!(app.event_graph.open);
        assert!(app.maps.open);
        assert!(app.environment.open);
        assert!(app.landscape.open);
        assert!(app.packaging.open);
        assert!(app.campaign.open);
        assert!(app.ai_planner.open);
        assert_eq!(app.view_mode, ViewMode::View3D);
        assert_eq!(app.brush_radius, 24.0);
    }

    #[test]
    fn test_map_switch_missing_optional_ambience_uses_clean_defaults() {
        let dir_a = PendingPanelTemp(temp_map_dir("missing_amb_a"));
        let dir_b = PendingPanelTemp(temp_map_dir("missing_amb_b"));
        make_test_map(&dir_a.0, 10001, "Zone_A", "Briefing_A");

        // Map B has valid terrain and boden.ini, but no soundscape, no daynight, no briefing
        let mut terrain_b = TerrainSession::create_blank(193, 77);
        safe_save_map(&dir_b.0, &mut terrain_b).unwrap();
        std::fs::write(
            dir_b.0.join("boden.ini"),
            b"[Heightmapstep]\n2.0\n[Waterlevel]\n30.0\n",
        )
        .unwrap();

        let mut app = EditorApp::initial_state();
        let loaded_a = load_map(&dir_a.0).unwrap();
        app.set_loaded_map(dir_a.0.clone(), loaded_a).unwrap();
        assert_eq!(app.ambience.zones.len(), 1);

        let loaded_b = load_map(&dir_b.0).unwrap();
        app.set_loaded_map(dir_b.0.clone(), loaded_b).unwrap();

        assert!(app.ambience.zones.is_empty());
        assert!(app.ambience.lighting_context.day_night.is_none());
        assert!(app.ambience.briefing_title.is_empty());
        assert!(app.ambience.briefing_body.is_empty());
        assert!(!app.ambience.is_dirty());
    }

    #[test]
    fn test_map_switch_corrupt_ambience_aborts_and_preserves_map_a() {
        let dir_a = PendingPanelTemp(temp_map_dir("corrupt_amb_a"));
        let dir_c = PendingPanelTemp(temp_map_dir("corrupt_amb_c"));
        make_test_map(&dir_a.0, 10001, "Zone_A", "Briefing_A");

        // Map C has valid terrain, but corrupt soundscape.sndz
        let mut terrain_c = TerrainSession::create_blank(193, 77);
        safe_save_map(&dir_c.0, &mut terrain_c).unwrap();
        std::fs::write(
            dir_c.0.join("boden.ini"),
            b"[Heightmapstep]\n2.0\n[Waterlevel]\n30.0\n",
        )
        .unwrap();
        std::fs::write(
            dir_c.0.join("soundscape.sndz"),
            b"INVALID_GARBAGE_HEADER_BYTES",
        )
        .unwrap();

        let mut app = EditorApp::initial_state();
        let loaded_a = load_map(&dir_a.0).unwrap();
        app.set_loaded_map(dir_a.0.clone(), loaded_a).unwrap();
        app.placement.load_session(&dir_a.0).unwrap();

        let loaded_c = load_map(&dir_c.0).unwrap();
        let err = app.set_loaded_map(dir_c.0.clone(), loaded_c).unwrap_err();
        assert!(
            err.contains("Corrupted") && err.contains("soundscape.sndz"),
            "{err}"
        );

        // Map A remains completely intact and unpolluted
        assert_eq!(app.map_path_input, dir_a.0.to_string_lossy());
        assert_eq!(app.ambience.zones.len(), 1);
        assert_eq!(app.ambience.zones[0].name, "Zone_A");
        assert_eq!(app.placement.objects()[0].uid, 10001);
        assert!(!app.has_unsaved_changes());
    }

    #[test]
    fn test_map_switch_corrupt_daynight_aborts_before_resetting_other_panels() {
        let dir_a = PendingPanelTemp(temp_map_dir("corrupt_daynight_a"));
        let dir_b = PendingPanelTemp(temp_map_dir("corrupt_daynight_b"));
        make_test_map(&dir_a.0, 10001, "Zone_A", "Briefing_A");

        let mut terrain_b = TerrainSession::create_blank(193, 77);
        safe_save_map(&dir_b.0, &mut terrain_b).unwrap();
        std::fs::write(dir_b.0.join("daynight.bmp"), b"broken daynight table").unwrap();

        let mut app = EditorApp::initial_state();
        app.set_loaded_map(dir_a.0.clone(), load_map(&dir_a.0).unwrap())
            .unwrap();
        app.placement.load_session(&dir_a.0).unwrap();
        app.textures.load_session(&dir_a.0).unwrap();
        let old_heights = app.session.as_ref().unwrap().heights().to_vec();
        let old_texture_doc = app.textures.session_textures().map(|x| x.to_vec());

        let err = app
            .set_loaded_map(dir_b.0.clone(), load_map(&dir_b.0).unwrap())
            .unwrap_err();
        assert!(
            err.contains("Corrupted") && err.contains("daynight.bmp"),
            "{err}"
        );
        assert_eq!(app.map_path_input, dir_a.0.to_string_lossy());
        assert_eq!(app.session.as_ref().unwrap().heights(), old_heights);
        assert_eq!(app.placement.objects()[0].uid, 10001);
        assert_eq!(app.placement.loaded_map(), Some(dir_a.0.as_path()));
        assert_eq!(
            app.textures.session_textures().map(|x| x.to_vec()),
            old_texture_doc
        );
        assert_eq!(app.ambience.zones[0].name, "Zone_A");
        assert!(!app.has_unsaved_changes());
    }

    #[test]
    fn test_blank_map_switch_resets_all_panel_documents_and_preserves_preferences() {
        let dir_a = PendingPanelTemp(temp_map_dir("blank_switch_a"));
        make_test_map(&dir_a.0, 10001, "Zone_A", "Briefing_A");

        let mut app = EditorApp::initial_state();
        let loaded_a = load_map(&dir_a.0).unwrap();
        app.set_loaded_map(dir_a.0.clone(), loaded_a).unwrap();

        // Set user preferences and transient plans
        app.placement.open = true;
        app.placement
            .set_delta(["1.0".into(), "2.0".into(), "3.0".into()]);
        app.landscape.open = true;
        app.landscape.apply_requested = Some(LandscapeApply::River);
        app.environment.generated_flora =
            vec![against_rome_environment::nature::NatureAddition::new(
                1, "Tree", 10.0, 0.0, 10.0, 0.0,
            )];
        app.ai_planner.plan_summary = "AI summary".into();

        let ctx = egui::Context::default();
        app.perform_action(PendingAction::Blank(257), &ctx);

        assert_eq!(app.session.as_ref().unwrap().vertex_size, 257);
        assert!(app.map_path_input.is_empty());
        assert!(app.placement.objects().is_empty());
        assert!(app.placement.loaded_map().is_none());
        assert!(app.ambience.zones.is_empty());
        assert!(app.ambience.briefing_title.is_empty());
        assert!(app.ambience.lighting_context.day_night.is_none());
        assert!(app.landscape.apply_requested.is_none());
        assert!(app.environment.generated_flora.is_empty());
        assert!(app.ai_planner.plan_summary.is_empty());

        // User preferences preserved
        assert!(app.placement.open);
        assert_eq!(app.placement.delta(), &["1.0", "2.0", "3.0"]);
        assert!(app.landscape.open);
        assert!(!app.has_unsaved_changes());
    }

    #[test]
    fn test_ui_explicit_discard_cleanly_switches_to_map_b_without_leakage() {
        let dir_a = PendingPanelTemp(temp_map_dir("discard_leak_a"));
        let dir_b = PendingPanelTemp(temp_map_dir("discard_leak_b"));
        make_test_map(&dir_a.0, 10001, "Zone_A", "Briefing_A");
        make_test_map(&dir_b.0, 20001, "Zone_B", "Briefing_B");

        let mut app = EditorApp::initial_state();
        let loaded_a = load_map(&dir_a.0).unwrap();
        app.set_loaded_map(dir_a.0.clone(), loaded_a).unwrap();
        app.placement.load_session(&dir_a.0).unwrap();

        // Dirty Map A
        app.session
            .as_mut()
            .unwrap()
            .apply_height_changes(&[SampleChange {
                index: 0,
                before: 77,
                after: 88,
            }]);
        app.placement.select_object(Some((0, 10001)));
        app.landscape.last_cliff_result =
            Some(against_rome_landscape::cliff::CliffDetectionResult {
                dimension: 64,
                cliff_cells: vec![],
                contours: vec![],
            });
        app.landscape.apply_requested = Some(LandscapeApply::Cliff);
        assert!(app.has_unsaved_changes());

        // Queue switch to Map B
        app.pending_action = Some(PendingAction::Load(dir_b.0.clone()));

        // Simulate discard confirmation in perform_action
        let ctx = egui::Context::default();
        let action = app.pending_action.take().unwrap();
        app.perform_action(action, &ctx);

        // Verify Map B is installed and Map A dirty/transient state is completely cleared
        assert_eq!(app.map_path_input, dir_b.0.to_string_lossy());
        assert_eq!(app.ambience.zones[0].name, "Zone_B");
        assert!(app.placement.objects().is_empty());
        assert_eq!(app.placement.loaded_map(), None);
        assert_eq!(app.placement.selected_object(), None);
        assert!(app.landscape.last_cliff_result.is_none());
        assert!(app.landscape.apply_requested.is_none());
        assert!(!app.has_unsaved_changes());

        // The Objects panel remains explicit: loading B after the switch yields only B data.
        app.placement.load_session(&dir_b.0).unwrap();
        let uids: Vec<u32> = app.placement.objects().iter().map(|o| o.uid).collect();
        assert!(uids.contains(&20001));
        assert!(!uids.contains(&10001));
    }

    // ---------------------------------------------------------------- Nature brush workflow

    const NATURE_TREE: u16 = 10;
    const NATURE_GRASS: u16 = 11;
    const NATURE_LINKED: u16 = 12;
    const NATURE_HOUSE: u16 = 20;

    /// Game root with an objdef naming the fixture types; `with_objdef = false` omits it.
    fn nature_game(tag: &str, with_objdef: bool) -> (PathBuf, PathBuf) {
        let root = temp_map_dir(tag);
        let map = root.join("game").join("MAPS").join("NAT1");
        std::fs::create_dir_all(&map).unwrap();
        if with_objdef {
            let defaults = root.join("game/SYSTEM/DATA_MP/DEFAULTS");
            std::fs::create_dir_all(&defaults).unwrap();
            let mut text = String::from("[ObjectDefaults]\n");
            for (id, name) in [
                (NATURE_TREE, "LanGerNad00_Tanne"),
                (NATURE_GRASS, "LanGerGra01_Gras"),
                (NATURE_LINKED, "LanGerStr01_Strauch"),
                (NATURE_HOUSE, "BuiGerHaus01"),
            ] {
                let mut columns = vec!["0".to_string(); 52];
                columns[0] = id.to_string();
                columns[1] = "1".into();
                columns.push(name.into());
                text.push_str(&columns.join(","));
                text.push('\n');
            }
            std::fs::write(defaults.join("objdef.dau"), text).unwrap();
        }
        (root, map)
    }

    /// Editor-format DATA: slots 0 tree, 1 far tree, 2 grass, 3 house, 4 landscape child of the
    /// house; slots 5.. are free capacity for clones.
    fn nature_data_fixture(map: &Path, capacity: usize) {
        use against_rome_placement::{COLUMNS, SEGMENTS};
        let data = map.join("DATA");
        std::fs::create_dir_all(&data).unwrap();
        let n = capacity;
        let mut objects = vec![0u8; 16 + n * (79 + COLUMNS.iter().sum::<usize>())];
        objects[..4].copy_from_slice(&1u32.to_le_bytes());
        objects[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        objects[8..12].copy_from_slice(&30u32.to_le_bytes());
        objects[12..16].copy_from_slice(&30u32.to_le_bytes());
        let actives: [(u16, [f32; 3], Option<u16>); 5] = [
            (NATURE_TREE, [900.0, 0.0, 900.0], None),
            (NATURE_TREE, [4000.0, 0.0, 4000.0], None),
            (NATURE_GRASS, [950.0, 0.0, 950.0], None),
            (NATURE_HOUSE, [880.0, 0.0, 880.0], None),
            (NATURE_LINKED, [900.0, 0.0, 880.0], Some(3)),
        ];
        for slot in 0..n {
            let r = 16 + slot * 79;
            for offset in [67, 69] {
                objects[r + offset..r + offset + 2].copy_from_slice(&(slot as u16).to_le_bytes());
            }
            for offset in [71, 73, 77] {
                objects[r + offset..r + offset + 2].fill(255);
            }
            for col in 0..18 {
                let offset =
                    16 + n * 79 + n * COLUMNS[..col].iter().sum::<usize>() + slot * COLUMNS[col];
                if COLUMNS[col] == 2 {
                    objects[offset..offset + 2].fill(255);
                }
                if col == 1 {
                    objects[offset..offset + 2].copy_from_slice(&(slot as u16).to_le_bytes());
                }
                if col == 13 {
                    if let Some(Some(parent)) = actives.get(slot).map(|a| a.2) {
                        objects[offset..offset + 2].copy_from_slice(&parent.to_le_bytes());
                    }
                }
            }
            if let Some((type_id, _, _)) = actives.get(slot) {
                objects[r] = 1;
                objects[r + 3..r + 7].copy_from_slice(&(slot as u32 + 1).to_le_bytes());
                objects[r + 75..r + 77].copy_from_slice(&type_id.to_le_bytes());
            }
        }
        let mut position = vec![0u8; 8 + n * 17];
        position[..4].copy_from_slice(&1u32.to_le_bytes());
        position[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        for (slot, (_, xyz, _)) in actives.iter().enumerate() {
            position[8 + slot * 17] = 1;
            for (axis, v) in xyz.iter().enumerate() {
                position[9 + slot * 17 + axis * 4..13 + slot * 17 + axis * 4]
                    .copy_from_slice(&v.to_le_bytes());
            }
        }
        let mut objdata = vec![0u8; 8 + n * SEGMENTS.iter().sum::<usize>()];
        objdata[..4].copy_from_slice(&1u32.to_le_bytes());
        objdata[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        for (name, bytes) in [
            ("objects", objects),
            ("position", position),
            ("objdata", objdata),
        ] {
            std::fs::write(data.join(format!("{name}.dat")), bytes).unwrap();
        }
    }

    fn nature_app(tag: &str) -> (EditorApp, PathBuf) {
        let (root, map) = nature_game(tag, true);
        nature_data_fixture(&map, 16);
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.map_path_input = map.display().to_string();
        app.placement.load_session(&map).unwrap();
        app.nature.random = DotNetRandom::new(1234);
        (app, root)
    }

    fn nature_stroke(app: &mut EditorApp, tiles: &[(i32, i32)]) -> NatureStrokeReport {
        let mut total = NatureStrokeReport::default();
        for &(x, y) in tiles {
            let report = app.nature_paint_tile(x, y);
            total.planted += report.planted;
            total.removed_slots += report.removed_slots;
            total.removed_additions += report.removed_additions;
            total.visited.extend(report.visited);
        }
        app.nature_end_stroke();
        total
    }

    fn nature_active_objects(map: &Path) -> Vec<against_rome_placement::WorldObject> {
        against_rome_placement::ObjectStore::load(map)
            .unwrap()
            .objects()
    }

    #[test]
    fn nature_brush_catalog_comes_from_map_templates_and_objdef_names() {
        let (mut app, root) = nature_app("nature_catalog");
        app.nature_set_enabled(true);
        assert!(!app.status_is_error, "{}", app.status_message);
        let names: Vec<_> = app.nature_palette().into_iter().map(|i| i.name).collect();
        // Root landscape types only: the house and the linked child are not templates.
        assert_eq!(names, ["LanGerGra01_Gras", "LanGerNad00_Tanne"]);
        app.nature.category = Some(NatureCategory::Tree);
        assert_eq!(app.nature_palette().len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn nature_brush_plant_remove_are_single_undoable_strokes() {
        let (mut app, root) = nature_app("nature_edit_undo");
        app.nature_set_enabled(true);
        app.nature_select_type(i32::from(NATURE_TREE));
        app.nature.brush_size = 3;
        app.nature.density = NatureDensity::Dense;

        // A fast drag skips tiles; the driver fills the gap and the stroke is one undo step.
        let planted = nature_stroke(&mut app, &[(10, 10), (14, 10)]);
        assert_eq!(planted.visited.len(), 5, "gap tiles filled");
        let (added, removed) = app.nature_pending_counts();
        assert!(added > 0 && planted.planted == added && removed == 0);
        assert!(app.nature_is_dirty() && app.has_unsaved_changes());
        assert!(app
            .nature
            .session
            .additions()
            .iter()
            .all(|a| a.type_id == i32::from(NATURE_TREE) && a.x < 16384.0 && a.z < 16384.0));

        // Remove: the tree/grass under tile (3,3) go; house and its linked child are protected.
        app.nature_set_operation(NatureOperation::Remove);
        app.nature.brush_size = 1;
        let removal = nature_stroke(&mut app, &[(3, 3)]);
        assert_eq!(removal.removed_slots, 2);
        assert_eq!(
            app.nature
                .session
                .removed_slots()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [0, 2]
        );
        assert_eq!(app.nature_pending_counts().0, added);

        // Remove also erases pending additions under the brush.
        let pending_tile = {
            let a = &app.nature.session.additions()[0];
            (
                (a.x / NATURE_TILE_WORLD) as i32,
                (a.z / NATURE_TILE_WORLD) as i32,
            )
        };
        app.nature.brush_size = 2;
        let erased = nature_stroke(&mut app, &[pending_tile]);
        assert!(erased.removed_additions > 0);
        assert!(app.nature_pending_counts().0 < added);

        // Three strokes -> three undo steps, then redo restores them in order.
        assert!(app.nature_undo());
        assert_eq!(app.nature_pending_counts().0, added);
        assert!(app.nature_undo());
        assert_eq!(app.nature_pending_counts(), (added, 0));
        assert!(app.nature_undo());
        assert_eq!(app.nature_pending_counts(), (0, 0));
        assert!(!app.nature_is_dirty() && !app.has_unsaved_changes());
        assert!(!app.nature_undo());
        assert!(app.nature_redo());
        assert_eq!(app.nature_pending_counts(), (added, 0));

        // Esc mid-stroke leaves no step behind.
        app.nature_set_operation(NatureOperation::Plant);
        app.nature_paint_tile(30, 30);
        app.nature_cancel_stroke();
        assert_eq!(app.nature_pending_counts(), (added, 0));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn nature_brush_fails_closed_without_catalog_or_objects() {
        // No DATA session loaded.
        let mut app = EditorApp::initial_state();
        app.nature_set_enabled(true);
        assert!(app.status_is_error, "{}", app.status_message);
        let report = app.nature_paint_tile(3, 3);
        assert!(!report.changed() && !app.nature_is_dirty());
        assert!(
            app.status_message.contains("unavailable"),
            "{}",
            app.status_message
        );

        // DATA loaded but the game objdef catalog is missing.
        let (root, map) = nature_game("nature_no_objdef", false);
        nature_data_fixture(&map, 16);
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.placement.load_session(&map).unwrap();
        app.nature_set_enabled(true);
        assert!(app.status_is_error && app.status_message.contains("catalog unavailable"));
        assert!(!app.nature_paint_tile(3, 3).changed() && !app.nature_is_dirty());

        // Catalog present but no template object of a selected type is on the map: nothing plants.
        let (mut app, root2) = nature_app("nature_unselected");
        app.nature_set_enabled(true);
        app.nature.category = Some(NatureCategory::Rock);
        app.nature_paint_tile(10, 10);
        app.nature_end_stroke();
        assert!(!app.nature_is_dirty());
        assert!(app.status_is_error && app.status_message.contains("pick a landscape type"));

        // Unsaved DATA edits make the snapshot stale, so the brush refuses.
        let (mut app, root3) = nature_app("nature_placement_dirty");
        app.placement.open = true;
        app.placement.select_object(Some((0, 1)));
        let ctx = egui::Context::default();
        pending_panel_click(&mut app, &ctx, 0, "Snap to tile");
        assert!(app.placement.is_dirty());
        app.nature_set_enabled(true);
        assert!(!app.nature_paint_tile(3, 3).changed() && !app.nature_is_dirty());
        assert!(
            app.status_message.contains("unsaved edits"),
            "{}",
            app.status_message
        );
        for dir in [root, root2, root3] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn nature_pending_edits_guard_map_switch_and_discard_cleanly() {
        let (mut app, root) = nature_app("nature_guard");
        app.nature_set_enabled(true);
        app.nature_select_type(i32::from(NATURE_TREE));
        app.nature.brush_size = 3;
        app.nature.density = NatureDensity::Dense;
        nature_stroke(&mut app, &[(10, 10)]);
        assert!(app.nature_is_dirty());

        app.pending_action = Some(PendingAction::Blank(193));
        let err = app
            .set_loaded_map(root.join("elsewhere"), TerrainSession::create_blank(193, 1))
            .unwrap_err();
        assert!(err.contains("unsaved changes"), "{err}");
        assert!(app.nature_is_dirty() && app.pending_action.is_some());

        // The UI path keeps the edit until the designer explicitly discards.
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.handle_pending_action(ctx)
        });
        assert!(app.pending_action.is_some() && app.nature_is_dirty());
        app.perform_action(PendingAction::Blank(193), &ctx);
        assert!(!app.nature_is_dirty() && !app.has_unsaved_changes());
        assert!(app.nature.data.is_none());

        let (mut app, root2) = nature_app("nature_discard");
        app.nature_set_enabled(true);
        nature_stroke(&mut app, &[(10, 10)]);
        assert!(app.nature_is_dirty());
        app.nature_discard();
        assert!(!app.nature_is_dirty() && !app.nature_undo());
        for dir in [root, root2] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn nature_safe_save_persists_additions_and_removals_transactionally() {
        let (mut app, root) = nature_app("nature_save_ok");
        let map = PathBuf::from(&app.map_path_input);
        app.nature_set_enabled(true);
        app.nature_select_type(i32::from(NATURE_TREE));
        app.nature.brush_size = 3;
        app.nature.density = NatureDensity::Dense;
        nature_stroke(&mut app, &[(10, 10)]);
        app.nature_set_operation(NatureOperation::Remove);
        app.nature.brush_size = 1;
        nature_stroke(&mut app, &[(3, 3)]);
        let (added, removed) = app.nature_pending_counts();
        assert!(added > 0 && removed == 2);
        let additions = app.nature.session.additions().to_vec();

        app.safe_save_current();
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(app.status_message.starts_with("Saved safely"));
        assert!(!app.nature_is_dirty() && !app.has_unsaved_changes());
        assert!(!app.placement.is_dirty());

        let objects = nature_active_objects(&map);
        let slots: Vec<_> = objects.iter().map(|o| o.slot).collect();
        // Removed: slot 0 tree, slot 2 grass. Kept: far tree, house and its linked child.
        for removed_slot in [0, 2] {
            assert!(
                !slots.contains(&removed_slot),
                "slot {removed_slot} removed"
            );
        }
        for kept in [1, 3, 4] {
            assert!(slots.contains(&kept), "slot {kept} kept");
        }
        assert_eq!(objects.len(), 3 + added);
        for addition in &additions {
            assert!(
                objects.iter().any(|o| o.type_id == NATURE_TREE
                    && (o.position[0] - addition.x).abs() < 0.01
                    && (o.position[2] - addition.z).abs() < 0.01),
                "addition at ({}, {}) persisted",
                addition.x,
                addition.z
            );
        }
        // The DATA panel was reloaded from the committed files and the brush rebuilds lazily.
        assert_eq!(app.placement.object_count(), 3 + added);
        assert!(app.nature.data.is_none());
        app.nature_paint_tile(20, 20);
        app.nature_end_stroke();
        assert!(app.nature.data.is_some());

        // A second save with nothing pending writes nothing Nature-related.
        let before = save_file_snapshot(&map);
        app.nature_discard();
        app.safe_save_current();
        assert!(!app.status_is_error, "{}", app.status_message);
        assert_eq!(
            save_file_snapshot(&map).get(&PathBuf::from("DATA/objects.dat")),
            before.get(&PathBuf::from("DATA/objects.dat"))
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn nature_safe_save_failure_restores_disk_and_keeps_edits_and_history() {
        use against_rome_maps::TransactionFailurePoint as Fail;
        let (mut app, root) = nature_app("nature_save_fail");
        let map = PathBuf::from(&app.map_path_input);
        app.nature_set_enabled(true);
        app.nature_select_type(i32::from(NATURE_TREE));
        app.nature.brush_size = 3;
        app.nature.density = NatureDensity::Dense;
        nature_stroke(&mut app, &[(10, 10)]);
        let pending = app.nature_pending_counts();
        let before = save_file_snapshot(&map);

        app.safe_save_current_using(|tx| {
            let mut injected =
                std::mem::replace(tx, against_rome_maps::MapSaveTransaction::new("unused"))
                    .with_injected_failure(Fail::BeforeFinalCommit, "simulated disk failure");
            injected.commit().map_err(|e| e.to_string())
        });
        assert!(app.status_is_error, "{}", app.status_message);
        assert_eq!(save_file_snapshot(&map), before);
        assert!(app.nature_is_dirty() && app.has_unsaved_changes());
        assert_eq!(app.nature_pending_counts(), pending);
        assert!(app.nature_undo(), "history survives a failed save");
        assert_eq!(app.nature_pending_counts(), (0, 0));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn nature_safe_save_rejects_unprovable_candidates_without_writing() {
        // Capacity exhausted: every free slot is gone, so a clone cannot be stored.
        let (root, map) = nature_game("nature_save_full", true);
        nature_data_fixture(&map, 5);
        let mut app = EditorApp::initial_state();
        app.session = Some(TerrainSession::create_blank(5, 32));
        app.map_path_input = map.display().to_string();
        app.placement.load_session(&map).unwrap();
        app.nature_set_enabled(true);
        app.nature_select_type(i32::from(NATURE_TREE));
        app.nature.brush_size = 3;
        app.nature.density = NatureDensity::Dense;
        nature_stroke(&mut app, &[(10, 10)]);
        assert!(app.nature_is_dirty());
        let before = save_file_snapshot(&map);
        app.safe_save_current();
        assert!(app.status_is_error, "{}", app.status_message);
        assert!(
            app.status_message.contains("Nature:"),
            "{}",
            app.status_message
        );
        assert_eq!(save_file_snapshot(&map), before);
        assert!(app.nature_is_dirty());

        // The object under a pending removal changed on disk after it was selected.
        let (mut app, root2) = nature_app("nature_save_stale");
        let map2 = PathBuf::from(&app.map_path_input);
        app.nature_set_enabled(true);
        app.nature_set_operation(NatureOperation::Remove);
        nature_stroke(&mut app, &[(3, 3)]);
        assert_eq!(app.nature_pending_counts().1, 2);
        let mut objects = std::fs::read(map2.join("DATA/objects.dat")).unwrap();
        objects[16 + 3..16 + 7].copy_from_slice(&999u32.to_le_bytes());
        std::fs::write(map2.join("DATA/objects.dat"), &objects).unwrap();
        let before = save_file_snapshot(&map2);
        app.safe_save_current();
        assert!(app.status_is_error && app.status_message.contains("changed on disk"));
        assert_eq!(save_file_snapshot(&map2), before);
        assert!(app.nature_is_dirty());

        // Unsaved DATA-panel edits and Nature edits both want objects.dat: refuse, write nothing.
        let (mut app, root3) = nature_app("nature_save_conflict");
        let map3 = PathBuf::from(&app.map_path_input);
        app.nature_set_enabled(true);
        nature_stroke(&mut app, &[(10, 10)]);
        app.placement.open = true;
        app.placement.select_object(Some((0, 1)));
        let ctx = egui::Context::default();
        pending_panel_click(&mut app, &ctx, 0, "Snap to tile");
        let before = save_file_snapshot(&map3);
        app.safe_save_current();
        assert!(app.status_is_error, "{}", app.status_message);
        assert_eq!(save_file_snapshot(&map3), before);
        for dir in [root, root2, root3] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn nature_panel_and_overlay_render_headless() {
        let (mut app, root) = nature_app("nature_render");
        app.nature_set_enabled(true);
        app.nature_select_type(i32::from(NATURE_TREE));
        app.nature.brush_size = 3;
        app.nature.density = NatureDensity::Dense;
        nature_stroke(&mut app, &[(10, 10)]);
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.show_nature_panel(ui);
                let (_, painter) = ui.allocate_painter(Vec2::splat(64.0), Sense::hover());
                app.draw_nature_overlay(
                    &painter,
                    Rect::from_min_size(Pos2::ZERO, Vec2::splat(64.0)),
                );
            });
        });
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn terrain_tool_shortcuts_cover_all_tools_and_preserve_edits() {
        use egui::Key::*;
        let mut app = EditorApp::default();
        for (key, tool) in [
            (Num1, ActiveTool::HeightRaise), (Num2, ActiveTool::HeightLower),
            (Num3, ActiveTool::HeightFlatten), (Num4, ActiveTool::HeightSmooth),
            (Num5, ActiveTool::HeightRoughen), (Num6, ActiveTool::Water),
            (Num7, ActiveTool::CollisionBlock), (Num8, ActiveTool::CollisionClear),
            (Num9, ActiveTool::Material),
        ] {
            assert!(SHORTCUT_KEYS.contains(&key));
            let action = shortcut_action(key, false, false, false, false).expect("工具快捷鍵");
            app.handle_shortcut_action(action);
            assert_eq!(app.active_tool, tool);
            assert_eq!(app.session.as_ref().unwrap().heights(), TerrainSession::create_blank(257, 32).heights());
            assert!(app.pending_action.is_none());
        }
    }

    #[test]
    fn brush_radius_shortcuts_clamp_and_leave_other_brushes_unchanged() {
        let mut app = EditorApp::default();
        app.brush_radius = 1.0;
        let smaller = shortcut_action(egui::Key::OpenBracket, false, false, false, false).unwrap();
        let larger = shortcut_action(egui::Key::CloseBracket, false, false, false, false).unwrap();
        app.handle_shortcut_action(smaller);
        assert_eq!(app.brush_radius, 1.0);
        app.handle_shortcut_action(larger);
        assert_eq!(app.brush_radius, 2.0);
        app.brush_radius = 64.0;
        app.handle_shortcut_action(larger);
        assert_eq!(app.brush_radius, 64.0);
        app.active_tool = ActiveTool::Material;
        app.handle_shortcut_action(smaller);
        assert_eq!(app.brush_radius, 64.0);
        app.active_tool = ActiveTool::HeightRaise;
        app.nature.enabled = true;
        app.handle_shortcut_action(smaller);
        assert_eq!(app.brush_radius, 64.0);
    }

    #[test]
    fn new_shortcuts_ignore_text_focus_and_modifiers() {
        for key in &SHORTCUT_KEYS[..12] {
            for (ctrl, shift, alt, text) in [
                (true, false, false, false), (false, true, false, false),
                (false, false, true, false), (false, false, false, true),
            ] {
                assert_eq!(shortcut_action(*key, ctrl, shift, alt, text), None);
            }
        }
    }

    #[test]
    fn f5_keyboard_event_opens_and_refreshes_design_check_headlessly() {
        let mut app = EditorApp::default();
        let ctx = egui::Context::default();
        gui_frame(&mut app, &ctx, vec![]);
        let key = |pressed| egui::Event::Key {
            key: egui::Key::F5, physical_key: None, pressed,
            repeat: false, modifiers: egui::Modifiers::NONE,
        };
        gui_frame(&mut app, &ctx, vec![key(true)]);
        assert!(app.design_check.open);
        assert!(app.design_check.report.is_some());
        app.design_check.selected = Some(99);
        gui_frame(&mut app, &ctx, vec![key(false)]);
        gui_frame(&mut app, &ctx, vec![key(true)]);
        assert_eq!(app.design_check.selected, None);
        assert!(app.pending_action.is_none());
        assert_eq!(app.session.as_ref().unwrap().heights(), TerrainSession::create_blank(257, 32).heights());
    }
    #[test]
    fn global_shortcuts_do_not_escape_dialogs_or_event_graph() {
        for context in 0..6 {
            let mut app = EditorApp::default();
            app.ambience.mark_dirty();
            app.design_check.open = false;
            match context {
                0 => app.pending_action = Some(PendingAction::Blank(193)),
                1 => app.terrain_confirm = Some(TerrainBulkAction::Reset),
                2 => app.skirmish_wizard.open = true,
                3 => app.event_graph.open = true,
                4 => app.event_graph.pending_close = true,
                _ => app.pending_save_warnings = Some((PathBuf::new(), vec![])),
            }
            let tool = app.active_tool;
            let radius = app.brush_radius;
            let ctx = egui::Context::default();
            gui_frame(&mut app, &ctx, vec![]);
            gui_frame(&mut app, &ctx, [egui::Key::Num9, egui::Key::CloseBracket, egui::Key::F5]
                .into_iter().map(|key| egui::Event::Key {
                    key, physical_key: None, pressed: true, repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }).collect());
            assert_eq!(app.active_tool, tool, "情境 {context}");
            assert_eq!(app.brush_radius, radius, "情境 {context}");
            assert!(!app.design_check.open, "情境 {context}");
        }
    }

    #[test]
    fn scene_delete_shortcut_preserves_original_map_edits_and_history() {
        let (mut app, dir) = scene_app("shortcut_original");
        app.scene.open = true;
        app.scene.move_mode = true;
        app.scene.select(Some(0));
        assert!(app.scene.scene_pick_active());
        app.map_path_input = "C:/Game/MAPS/ENDL_002".into();
        app.loaded_map_path = Some(PathBuf::from(&app.map_path_input));
        let before = app.scene.removals().to_vec();
        app.handle_shortcut_action(ShortcutAction::Delete);
        assert_eq!(app.scene.removals().len(), before.len());
        assert!(!app.scene.is_dirty());
        assert!(app.status_is_error);
        app.map_path_input = dir.display().to_string();
        app.loaded_map_path = Some(dir.clone());
        app.handle_shortcut_action(ShortcutAction::Delete);
        assert!(app.scene.is_dirty());
        assert_eq!(app.scene.removals().len(), before.len() + 1);
        std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap()).unwrap();
    }
    #[test]
    fn shortcut_mapping_is_pure_and_matches_csharp() {
        use egui::Key::*;
        let f = |k, c, s, a, t| shortcut_action(k, c, s, a, t);
        assert_eq!(f(S, true, false, false, false), Some(ShortcutAction::Save));
        assert_eq!(f(S, true, false, false, true), Some(ShortcutAction::Save));
        assert_eq!(f(S, false, false, false, false), None);
        assert_eq!(f(A, true, false, false, false), Some(ShortcutAction::SelectAll));
        assert_eq!(f(A, true, false, false, true), None);
        assert_eq!(f(D, true, false, false, false), Some(ShortcutAction::Duplicate));
        assert_eq!(f(Delete, false, false, false, false), Some(ShortcutAction::Delete));
        assert_eq!(f(Delete, true, false, false, false), None);
        assert_eq!(f(Delete, false, false, false, true), None);
        assert_eq!(
            f(ArrowLeft, true, false, false, false),
            Some(ShortcutAction::Nudge { dx: -256.0, dz: 0.0 })
        );
        assert_eq!(
            f(ArrowRight, true, true, false, false),
            Some(ShortcutAction::Nudge { dx: 64.0, dz: 0.0 })
        );
        assert_eq!(
            f(ArrowUp, true, false, false, false),
            Some(ShortcutAction::Nudge { dx: 0.0, dz: -256.0 })
        );
        assert_eq!(
            f(ArrowDown, true, false, false, false),
            Some(ShortcutAction::Nudge { dx: 0.0, dz: 256.0 })
        );
        // Plain arrows belong to the 3D camera / 2D canvas hover; Shift+Arrows pan the canvas; Alt disables everything.
        assert_eq!(f(ArrowLeft, false, false, false, false), None);
        assert_eq!(
            f(ArrowRight, false, true, false, false),
            Some(ShortcutAction::CanvasPan { dx: -256.0, dy: 0.0 })
        );
        assert_eq!(f(ArrowUp, false, false, false, false), None);
        assert_eq!(
            f(ArrowDown, false, true, false, false),
            Some(ShortcutAction::CanvasPan { dx: 0.0, dy: -256.0 })
        );
        assert_eq!(
            f(PageUp, false, false, false, false),
            Some(ShortcutAction::CanvasZoom(1.15))
        );
        assert_eq!(
            f(PageDown, false, false, false, false),
            Some(ShortcutAction::CanvasZoom(1.0 / 1.15))
        );
        assert_eq!(
            f(Home, false, false, false, false),
            Some(ShortcutAction::CanvasResetView)
        );
        assert_eq!(f(ArrowLeft, false, false, true, false), None);
        assert_eq!(f(S, true, false, true, false), None);
    }

    #[test]
    fn original_map_detection_follows_custom_map_registration() {
        // Unregistered maps inside MAPS (retail ENDL_000-004, tutorials,
        // campaign maps, or ENDL slots without a custom-map marker) are original.
        for original in [
            "MAPS/ENDL_000",
            "MAPS/ENDL_004",
            "D:/g/MAPS/endl_002",
            "MAPS/TUTOR_01",
            "MAPS/ENDL_042",
        ] {
            assert!(is_original_map_dir(Path::new(original)), "{original}");
        }
        for custom in ["work/ENDL_005", "scratch/my_map", ""] {
            assert!(!is_original_map_dir(Path::new(custom)), "{custom}");
        }
    }

    #[test]
    fn save_shortcut_refuses_original_maps_without_saving() {
        let mut app = EditorApp::default();
        app.map_path_input = "C:/Game/MAPS/ENDL_002".into();
        app.handle_shortcut_action(ShortcutAction::Save);
        assert!(app.status_is_error);
        assert!(app.status_message.contains("read-only"));
        assert!(app.pending_save_warnings.is_none());
    }

    #[test]
    fn focus_world_position_centers_the_point_and_box_select_needs_loaded_data() {
        let mut app = EditorApp::default();
        app.toggle_box_select();
        assert!(!app.box_select, "box select needs loaded DATA");
        assert!(app.status_is_error);

        app.last_canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        app.zoom = 2.0;
        app.focus_world_position(4096.0, 12288.0); // normalized (0.25, 0.75) on the 64-tile default
        let side = 600.0 * app.zoom;
        let centre = app.last_canvas_rect.center() + app.pan;
        let point = Pos2::new(centre.x + (0.25 - 0.5) * side, centre.y + (0.75 - 0.5) * side);
        assert!((point - app.last_canvas_rect.center()).length() < 0.01, "{point:?}");

        let pan = app.pan;
        app.focus_world_position(f32::NAN, 1.0);
        assert_eq!(app.pan, pan, "non-finite focus is ignored");
    }

    #[test]
    fn canvas_keyboard_shortcuts_zoom_pan_and_reset() {
        let mut app = EditorApp::default();
        assert_eq!(app.zoom, 1.0);
        assert_eq!(app.pan, Vec2::ZERO);

        // Zoom in
        app.handle_shortcut_action(ShortcutAction::CanvasZoom(1.5));
        assert_eq!(app.zoom, 1.5);

        // Pan
        app.handle_shortcut_action(ShortcutAction::CanvasPan { dx: 100.0, dy: -50.0 });
        assert_eq!(app.pan, Vec2::new(100.0, -50.0));

        // Zoom clamping test
        app.handle_shortcut_action(ShortcutAction::CanvasZoom(100.0));
        assert_eq!(app.zoom, 10.0); // max 10.0
        app.handle_shortcut_action(ShortcutAction::CanvasZoom(0.01));
        assert_eq!(app.zoom, 0.25); // min 0.25

        // Reset view
        app.handle_shortcut_action(ShortcutAction::CanvasResetView);
        assert_eq!(app.zoom, 1.0);
        assert_eq!(app.pan, Vec2::ZERO);

        // In 3D view mode, canvas zoom/pan/reset does not alter 2D zoom/pan
        app.view_mode = ViewMode::View3D;
        app.handle_shortcut_action(ShortcutAction::CanvasZoom(2.0));
        app.handle_shortcut_action(ShortcutAction::CanvasPan { dx: 50.0, dy: 50.0 });
        assert_eq!(app.zoom, 1.0);
        assert_eq!(app.pan, Vec2::ZERO);
    }

    #[test]
    fn status_bar_cursor_readout_and_default_state() {
        let app = EditorApp::default();
        assert_eq!(app.canvas_cursor, None);

        let (wx, wz, tx, tz) = (8192.0, 4096.0, 32, 16);
        let format_readout = |lang| {
            let prefix = tr(lang, "Cursor", "游標");
            let tile_label = tr(lang, "Tile", "圖塊");
            format!("{prefix}: X: {wx:.0}, Z: {wz:.0} | {tile_label}: ({tx}, {tz})")
        };
        assert_eq!(
            format_readout(AppLanguage::English),
            "Cursor: X: 8192, Z: 4096 | Tile: (32, 16)"
        );
        assert_eq!(
            format_readout(AppLanguage::TraditionalChinese),
            "游標: X: 8192, Z: 4096 | 圖塊: (32, 16)"
        );
    }

    #[test]
    fn design_check_locate_request_is_consumed_once() {
        let mut app = EditorApp::default();
        app.last_canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 600.0));
        app.design_check.focus_request = Some((8192.0, 8192.0));
        app.design_check.event_request = Some(2);
        app.consume_design_check_focus();
        assert!(app.design_check.focus_request.is_none() && app.design_check.event_request.is_none());
        assert!(app.scenario.open, "event issues reveal the scenario panel");
        assert!(app.pan.length() < 0.01, "map centre needs no pan");
    }

    #[test]
    fn material_tool_routes_viewport_paint_without_touching_heights() {
        let mut app = EditorApp::initial_state();
        app.active_tool = ActiveTool::Material;
        let heights = app.session.as_ref().unwrap().heights().to_vec();
        // No boden.txt session is loaded: the tool reports it and never falls through to height brushes.
        app.handle_viewport_3d_action(ViewportAction::Paint { grid_x: 10.0, grid_y: 10.0 });
        app.handle_viewport_3d_action(ViewportAction::CommitStroke);
        assert!(app.textures.material_notice().is_some());
        assert_eq!(app.session.as_ref().unwrap().heights(), heights.as_slice());
        assert!(!app.session.as_ref().unwrap().can_undo());
        assert!(!app.textures.is_dirty());
    }

    #[test]
    fn language_switch_changes_text_and_syncs_ambience() {
        let mut app = EditorApp::default();
        assert_eq!(tr(app.lang(), "Save", "儲存"), "Save");
        app.set_app_language(AppLanguage::TraditionalChinese);
        assert_eq!(tr(app.lang(), "Save", "儲存"), "儲存");
        assert_eq!(app.ambience.app_language, AppLanguage::TraditionalChinese);
    }

    #[test]
    fn test_in_game_requires_map_saves_then_spawns_via_injected_spawner() {
        let root = temp_map_dir("test_in_game_root");
        let game = root.join("game");
        let map = game.join("MAPS").join("ENDL_007");
        std::fs::create_dir_all(&map).unwrap();
        let mut calls = 0usize;
        let mut spawn = |_: &Path, _: &Path| {
            calls += 1;
            Ok(())
        };
        let mut app = EditorApp::default();
        app.test_in_game_using(&mut spawn);
        assert!(app.status_is_error, "no map path");
        app.map_path_input = map.to_string_lossy().into_owned();
        app.test_in_game_using(&mut spawn);
        assert!(app.status_is_error && app.status_message.contains("Against_Rome.exe"));
        std::fs::write(game.join("Against_Rome.exe"), b"x").unwrap();
        app.test_in_game_using(&mut spawn);
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(app.status_message.contains("ENDL_007"));
        assert_eq!(calls, 1);
        let _ = std::fs::remove_dir_all(&root);
    }
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ui_frame(ctx);
    }
}

impl EditorApp {
    /// 複製已載入地圖的路徑，不採用尚未載入的輸入文字。
    fn map_folder_clipboard(&self) -> Option<String> {
        self.loaded_map_path.as_ref().map(|p| p.display().to_string())
    }

    /// 對話框及事件圖開啟期間不讓全域快捷鍵穿透至地圖。
    fn shortcuts_blocked(&self) -> bool {
        self.pending_action.is_some() || self.pending_save_warnings.is_some()
            || self.terrain_confirm.is_some() || self.recovery.offer.is_some()
            || self.skirmish_wizard.open || self.event_graph.open || self.event_graph.pending_close
    }

    /// One full editor frame. `eframe::App::update` delegates here; tests drive it headlessly
    /// with an `egui::Context` (no window or `eframe::Frame` needed).
    pub fn ui_frame(&mut self, ctx: &egui::Context) {
        if self.session.is_none() { self.canvas_cursor = None; }
        if !self.recovery_deferred { self.recovery_tick(ctx); }
        self.recovery_dialog(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && !self.has_unsaved_changes() {
            self.reset_journal_epoch();
            if !self.recovery_deferred && self.recovery.offer.is_none() { self.clear_recovery(); }
        }
        let route_objects = if self.placement.is_loaded() {
            self.placement.objects()
        } else {
            self.loaded_map_path.as_ref().and_then(|path|
                against_rome_placement::ObjectStore::load(path).ok()).map(|s| s.objects()).unwrap_or_default()
        };
        self.navigation.update_placement(&route_objects);
        self.navigation.update_buildings(self.placement.navigation_buildings(&route_objects));
        let footprint_game = self.troop_game_root().or_else(|| {
            let path = PathBuf::from(self.maps.game_path_input.trim());
            path.join("SYSTEM/clengine.ini").is_file().then_some(path)
        });
        self.navigation.update_apt(footprint_game, &route_objects, self.placement.type_names());
        self.navigation.refresh(self.session.as_ref(), self.water_surface);
        self.navigation.cancel_pick(ctx);
        let pick_cancel = self.scenario.cancel_canvas_pick(ctx, self.lang() == AppLanguage::English);
        let scenario_cancelled = pick_cancel.is_some();
        if let Some(message) = pick_cancel { self.set_status(message, false); }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.close_allowed
            && self.has_unsaved_changes()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending_action = Some(PendingAction::Close);
        }
        if !scenario_cancelled && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.box_select {
                self.cancel_box_select();
            } else if self.ramp_selecting {
                self.cancel_ramp_selection();
            } else if self.nature.driver.is_active() {
                self.nature_cancel_stroke();
            } else if self.textures.cancel_material_stroke() {
                self.status_message = "Material stroke cancelled.".into();
                self.status_is_error = false;
            } else if self.is_painting {
                if let Some(session) = &mut self.session {
                    session.cancel_stroke();
                }
                self.is_painting = false;
                self.stroke_path.clear();
                self.texture_dirty = true;
            }
        }
        self.collect_pilot_receipts();
        // 快捷鍵處理
        if !ctx.wants_keyboard_input() && !self.shortcuts_blocked() {
            if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Z)) {
                self.navigate_journal(ctx.input(|i| i.modifiers.shift));
            } else if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Y)) {
                self.navigate_journal(true);
            }
        }

        // Editor shortcuts: Ctrl+S, Ctrl+A, Ctrl+D, Delete, Ctrl+Arrow nudge.
        {
            let text_focus = ctx.wants_keyboard_input();
            let actions: Vec<ShortcutAction> = ctx.input(|i| {
                let ctrl = i.modifiers.ctrl || i.modifiers.command;
                SHORTCUT_KEYS
                    .iter()
                    .filter(|k| i.key_pressed(**k))
                    .filter_map(|k| {
                        shortcut_action(*k, ctrl, i.modifiers.shift, i.modifiers.alt, text_focus)
                    })
                    .collect()
            });
            for action in actions {
                self.handle_shortcut_action(action);
            }
        }

        // 頂部導覽列
        let lang = self.lang();
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading(tr(lang, "Against Rome Map Editor (Rust)", "Against Rome 地圖編輯器 (Rust)"));
                ui.separator();
                if ui.button(tr(lang, "Modifier", "修改器")).on_hover_text(tr(lang, "Open game save and memory trainer window", "開啟遊戲存檔與記憶體修改器視窗")).clicked() {
                    self.toolbox.open_modifier();
                }
                if ui.button(tr(lang, "Modifier Studio", "修改器工作室")).on_hover_text(tr(lang, "Open in-editor live value inspection and memory tools", "開啟編輯器內部即時數值檢視與記憶體工具")).clicked() {
                    self.modifier.open = true;
                }
                if ui.button(tr(lang, "Save Manager", "存檔管理")).on_hover_text(tr(lang, "Backup, restore, and organize game save files", "備份、還原與管理遊戲存檔")).clicked() {
                    self.toolbox.open_save_manager();
                }
                if ui.button(tr(lang, "Navigation", "導航")).on_hover_text(tr(lang, "Inspect passability grid, APAT footprints, and pathfinding routes", "檢視通行網格、APAT 建築占地與尋路路徑")).clicked() {
                    self.navigation.open = true;
                }
                if ui.button(tr(lang, "Minimap", "小地圖")).on_hover_text(tr(lang, "Toggle minimap overview window", "切換右下角小地圖總覽視窗")).clicked() {
                    self.minimap.open = true;
                }
                if ui.button(tr(lang, "Objects", "物件")).on_hover_text(tr(lang, "Open placed DATA objects list and placement inspector", "開啟已放置 DATA 物件清單與屬性檢查器")).clicked() {
                    self.placement.open = true;
                }
                if ui.button(tr(lang, "Scenario / Camera", "劇本／鏡頭")).on_hover_text(tr(lang, "Open scenario trigger events, conditions, and camera actions", "開啟劇本觸發事件、判斷條件與鏡頭動作")).clicked() {
                    self.scenario.open = true;
                }
                if ui.button(tr(lang, "Spawns", "放置腳本物件")).on_hover_text(tr(lang, "Place and manage script-spawned units and buildings", "放置與管理腳本生成的部隊與建築")).clicked() {
                    self.spawn_ui.open = true;
                }
                if ui.button(tr(lang, "Materials", "材質")).on_hover_text(tr(lang, "Paint terrain surface textures and blending layers", "繪製地表材質與過渡圖層混合")).clicked() {
                    self.textures.open = true;
                }
                if ui.button(tr(lang, "Scene (SDL)", "場景 (SDL)")).on_hover_text(tr(lang, "Inspect and modify SDL scene markers and entities", "檢視與修改 SDL 場景標記與物件實體")).clicked() {
                    self.scene.open = true;
                }
                if ui.button(tr(lang, "Event Graph", "事件圖")).on_hover_text(tr(lang, "Visual node graph editor for scenario events and triggers", "視覺化劇本事件與觸發器節點圖編輯器")).clicked() {
                    self.open_toolbar_event_graph();
                }
                if ui.button(tr(lang, "Console", "主控台")).on_hover_text(tr(lang, "Interactive developer console and command executor", "互動式開發者主控台與命令執行器")).clicked() {
                    self.console.open = true;
                }
                if ui.button(tr(lang, "Maps", "地圖")).on_hover_text(tr(lang, "Open map file browser and template manager", "開啟地圖檔案瀏覽器與範本管理器")).clicked() {
                    self.maps.open = true;
                }
                if ui.button(tr(lang, "Map properties", "地圖屬性")).on_hover_text(tr(lang, "Edit map title, player briefing, and starting camera position", "編輯地圖標題、玩家任務簡報與起始相機視角")).clicked() {
                    self.map_properties_open = true;
                }
                if ui.button(tr(lang, "New Map", "新地圖")).on_hover_text(tr(lang, "Create a blank map or generate a flat template", "建立空白地圖或產生平坦範本地圖")).clicked() {
                    self.maps.open = true;
                    self.maps.current_tab = crate::maps_ui::MapsTab::NewBlankMap;
                    if self.maps.game_path_input.trim().is_empty()
                        && !self.map_path_input.trim().is_empty()
                    {
                        if let Some(parent) = Path::new(self.map_path_input.trim()).parent() {
                            if let Some(game_root) = parent.parent() {
                                self.maps.game_path_input =
                                    game_root.to_string_lossy().into_owned();
                            }
                        }
                    }
                }
                if ui.button(tr(lang, "New skirmish map…", "新增對戰地圖…")).on_hover_text(tr(lang, "4-step wizard to create a balanced skirmish map with settlement, waves, and objectives", "開啟四步驟精靈建立含聚落、波次與勝負目標的平衡對戰地圖")).clicked() {
                    self.open_skirmish_wizard();
                }
                if ui.button(tr(lang, "Environment", "環境")).on_hover_text(tr(lang, "Configure lighting, sun angles, weather, and day-night presets", "設定光照、太陽角度、天氣與日夜預設")).clicked() {
                    self.environment.open = true;
                }
                if ui.button(tr(lang, "Landscape", "地景")).on_hover_text(tr(lang, "Generate terrain elevations, mountain ridges, and river paths", "產生高低起伏、山脈山脊與河道規劃")).clicked() {
                    self.landscape.open = true;
                }
                if ui.button(tr(lang, "Packaging", "打包")).on_hover_text(tr(lang, "Export map mod as ZIP package for distribution", "將地圖模組匯出為可分發的 ZIP 壓縮包")).clicked() {
                    self.packaging.open = true;
                }
                if ui.button(tr(lang, "Campaign", "戰役")).on_hover_text(tr(lang, "Configure starting camera, intro cutscenes, and AI attack waves", "設定起始視角鏡頭、過場動畫與 AI 進攻波次")).clicked() {
                    self.campaign.open = true;
                }
                if ui.button(tr(lang, "AI Planner", "AI 企劃")).on_hover_text(tr(lang, "Generate terrain, water, and materials with local Ollama AI", "使用本機 Ollama AI 規劃地形、水面與地表材質")).clicked() {
                    self.ai_planner.open = true;
                }
                if ui.button(tr(lang, "Ambience", "氛圍")).on_hover_text(tr(lang, "Configure background music, ambient sound zones, and water level", "設定背景音樂、環境音效區域與地表水位")).clicked() {
                    self.ambience.open = true;
                }
                if ui.button(tr(lang, "Families & Effects", "群組與特效")).on_hover_text(tr(lang, "Inspect DATA object parent-child links, particles, and light attachments", "檢視 DATA 物件父子關聯、粒子系統與附屬光源")).clicked() {
                    self.family_open = true;
                }
                if ui.button(tr(lang, "Tech Doc", "技術文件")).on_hover_text(tr(lang, "Read built-in reverse engineering format documentation and research", "查閱內建逆向工程格式文件與規格研究")).clicked() {
                    self.tech_doc.open = true;
                }
                if ui.button(tr(lang, "Design Check", "設計檢查")).on_hover_text(tr(lang, "Run design check (F5)", "立即執行設計檢查（F5）")).clicked() {
                    self.run_design_check();
                }
                if ui.button(tr(lang, "Map Balance Summary", "地圖平衡摘要")).on_hover_text(tr(lang, "Compare player team resource balance, settlement sizes, and unit counts", "檢視各隊伍起始資源、聚落規模與部隊數量平衡")).clicked() {
                    self.balance_summary.open = true;
                }

                ui.label(tr(lang, "Map Folder:", "地圖資料夾:"));
                ui.text_edit_singleline(&mut self.map_path_input);

                if ui.button(tr(lang, "Load Folder", "載入資料夾")).on_hover_text(tr(lang, "Load map folder entered in the path field", "載入路徑欄位中指定的自製地圖資料夾")).clicked() {
                    self.pending_action = Some(PendingAction::Load(PathBuf::from(
                        self.map_path_input.trim(),
                    )));
                }

                if ui.button(tr(lang, "Safe Save", "安全儲存 (Ctrl+S)")).on_hover_text(tr(lang, "Run design validation and save map files atomically (Ctrl+S)", "執行設計驗證並以不可部分完成交易安全儲存地圖 (Ctrl+S)")).clicked() {
                    self.request_safe_save();
                }

                if ui
                    .button(tr(lang, "Test in Game", "選用：啟動遊戲測試"))
                    .on_hover_text(tr(lang, "Save custom map and launch Against Rome to test in-game", "儲存自製地圖並啟動羅馬帝國遊戲進行實機測試"))
                    .clicked()
                {
                    self.test_in_game();
                }

                ui.separator();
                let mut chosen = lang;
                if ui
                    .selectable_label(lang == AppLanguage::TraditionalChinese, "中")
                    .on_hover_text("繁體中文")
                    .clicked()
                {
                    chosen = AppLanguage::TraditionalChinese;
                }
                if ui
                    .selectable_label(lang == AppLanguage::English, "EN")
                    .on_hover_text("English")
                    .clicked()
                {
                    chosen = AppLanguage::English;
                }
                if chosen != lang {
                    self.set_app_language(chosen);
                }

                ui.separator();

                self.collect_pilot_receipts();
                let can_undo = self.undo_journal.can_undo() || (!self.undo_journal.can_redo() && self.can_undo_any());
                if ui
                    .add_enabled(can_undo, egui::Button::new(tr(lang, "Undo (Ctrl+Z)", "復原 (Ctrl+Z)")))
                    .clicked()
                {
                    self.navigate_journal(false);
                }

                let can_redo = self.undo_journal.can_redo() || (!self.undo_journal.can_undo() && self.can_redo_any());
                if ui
                    .add_enabled(can_redo, egui::Button::new(tr(lang, "Redo (Ctrl+Y)", "重做 (Ctrl+Y)")))
                    .clicked()
                {
                    self.navigate_journal(true);
                }
            });

            ui.horizontal(|ui| {
                if self.view_mode != ViewMode::View3D {
                    ui.label(format!("{}: {:.0}%", tr(lang, "2D zoom", "2D 縮放"), self.zoom * 100.0));
                    if ui.button(tr(lang, "Reset 2D view", "重設 2D 視角")).clicked() {
                        self.handle_shortcut_action(ShortcutAction::CanvasResetView);
                    }
                }
                if ui.add_enabled(self.loaded_map_path.is_some(), egui::Button::new(tr(lang, "Copy map folder", "複製地圖資料夾"))).clicked() {
                    if let Some(path) = self.map_folder_clipboard() {
                        ctx.copy_text(path);
                    }
                }
            });
            // 狀態列
            ui.horizontal(|ui| {
                let color = if self.status_is_error {
                    Color32::from_rgb(240, 80, 80)
                } else {
                    Color32::from_rgb(100, 200, 100)
                };
                ui.colored_label(color, &self.status_message);

                ui.separator();
                ui.label(self.active_tool_status_text(lang));

                if let Some(text) = self.cursor_readout_text(lang) {
                    ui.separator();
                    ui.label(text);
                }

                if let Some(session) = &self.session {
                    if session.is_dirty() || self.reset_auxiliary_layers {
                        ui.colored_label(Color32::YELLOW, "[Unsaved Changes]");
                    }
                }
                if self.nature.session.is_dirty() {
                    ui.colored_label(Color32::YELLOW, "[Unsaved Nature]");
                }
            });
        });

        if let Some((_, issues)) = &self.pending_save_warnings {
            let messages: Vec<_> = issues.iter().map(|i| i.english.clone()).collect();
            let mut open = true;
            let mut decision = None;
            egui::Window::new("Confirm Save (Design Warnings)")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("These design warnings may affect playability. Continue saving?");
                    for message in messages.iter().take(10) {
                        ui.label(message);
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Continue saving").clicked() {
                            decision = Some(true);
                        }
                        if ui.button("Cancel save").clicked() {
                            decision = Some(false);
                        }
                    });
                });
            if !open {
                decision = Some(false);
            }
            if let Some(proceed) = decision {
                self.resolve_safe_save_warnings(proceed);
            }
        }

        self.show_terrain_confirm(ctx);

        // 左側工具面板
        egui::SidePanel::left("left_tools_panel")
            .min_width(220.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading(tr(lang, "Tools & Settings", "工具與設定"));
                    ui.menu_button(tr(lang, "Recent maps", "最近開啟"), |ui| {
                        if let Err(e) = self.recent_maps.prune() {
                            ui.label(format!("Recent maps / 最近地圖：{e}"));
                        }
                        if self.recent_maps.paths.is_empty() {
                            ui.label(tr(lang, "Load a map folder to start this list.", "載入地圖資料夾後會顯示於此。"));
                        }
                        for path in self.recent_maps.paths.clone() {
                            if ui.button(path.display().to_string()).clicked() {
                                self.pending_action = Some(PendingAction::Load(path));
                                ui.close_menu();
                            }
                        }
                    });

                    ui.separator();

                    ui.label(tr(lang, "Display Mode:", "顯示模式:"));
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .selectable_value(&mut self.view_mode, ViewMode::Blended, "Blended")
                            .clicked()
                        {
                            self.texture_dirty = true;
                        }
                        if ui
                            .selectable_value(&mut self.view_mode, ViewMode::Heightmap, "Height")
                            .clicked()
                        {
                            self.texture_dirty = true;
                        }
                        if ui
                            .selectable_value(&mut self.view_mode, ViewMode::Collision, "Collision")
                            .clicked()
                        {
                            self.texture_dirty = true;
                        }
                        if ui
                            .selectable_value(&mut self.view_mode, ViewMode::View3D, "3D View")
                            .clicked()
                        {
                            self.texture_dirty = true;
                        }
                    });

                    ui.separator();
                    let can_box = self.placement.is_loaded() && self.view_mode != ViewMode::View3D;
                    if ui
                        .add_enabled(
                            can_box || self.box_select,
                            egui::Button::new(tr(lang, "Box select objects", "框選物件")).selected(self.box_select),
                        )
                        .on_hover_text(tr(
                            lang,
                            "Drag a rectangle on the 2D map to select placed objects (Shift adds); Esc cancels.",
                            "在 2D 地圖上拖曳矩形選取放置物件（Shift 加選）；Esc 取消。",
                        ))
                        .clicked()
                    {
                        self.toggle_box_select();
                    }
                    ui.label("Brush Mode:");
                    if ui
                        .add(egui::Button::new(if self.ramp_selecting {
                            "Cancel Ramp Selection"
                        } else {
                            "Ramp: choose two points"
                        }))
                        .clicked()
                    {
                        if self.ramp_selecting {
                            self.cancel_ramp_selection();
                        } else {
                            self.ramp_start = None;
                            self.ramp_selecting = true;
                            self.status_message = "Click the ramp start point on the map.".into();
                            self.status_is_error = false;
                        }
                    }
                    ui.add(egui::Slider::new(&mut self.ramp_width, 0.5..=32.0).text("Ramp width"));
                    ui.add(
                        egui::Slider::new(&mut self.ramp_falloff, 0.0..=24.0).text("Ramp falloff"),
                    );
                    if self.ramp_selecting {
                        let cancel_hint = if self.view_mode == ViewMode::View3D {
                            "Use left-click on terrain to choose points; Esc cancels."
                        } else {
                            "Click the endpoint; right-click or Esc cancels."
                        };
                        ui.small(match self.ramp_start {
                            Some((x, y)) => format!("Start: ({x:.0}, {y:.0}); {cancel_hint}"),
                            None => format!("Click a start point, then an endpoint. {cancel_hint}"),
                        });
                    }
                    ui.selectable_value(
                        &mut self.active_tool,
                        ActiveTool::HeightRaise,
                        tr(lang, "Raise (Height) [1]", "升高（高度）[1]"),
                    );
                    ui.selectable_value(
                        &mut self.active_tool,
                        ActiveTool::HeightLower,
                        tr(lang, "Lower (Height) [2]", "降低（高度）[2]"),
                    );
                    ui.selectable_value(
                        &mut self.active_tool,
                        ActiveTool::HeightFlatten,
                        tr(lang, "Flatten (Height) [3]", "整平（高度）[3]"),
                    );
                    ui.selectable_value(
                        &mut self.active_tool,
                        ActiveTool::HeightSmooth,
                        tr(lang, "Smooth (Height) [4]", "平滑（高度）[4]"),
                    );
                    ui.selectable_value(
                        &mut self.active_tool,
                        ActiveTool::HeightRoughen,
                        tr(lang, "Roughen (Noise) [5]", "粗糙（雜訊）[5]"),
                    );
                    ui.selectable_value(
                        &mut self.active_tool,
                        ActiveTool::Water,
                        tr(lang, "Water (Height + Collision) [6]", "水面（高度與通行）[6]"),
                    );
                    ui.selectable_value(
                        &mut self.active_tool,
                        ActiveTool::CollisionBlock,
                        tr(lang, "Block (Collision) [7]", "阻擋（通行）[7]"),
                    );
                    ui.selectable_value(
                        &mut self.active_tool,
                        ActiveTool::CollisionClear,
                        tr(lang, "Clear (Collision) [8]", "清除（通行）[8]"),
                    );
                    ui.selectable_value(
                        &mut self.active_tool,
                        ActiveTool::Material,
                        tr(lang, "Material (terrain paint) [9]", "材質（地表繪製）[9]"),
                    );

                    ui.separator();
                    if self.active_tool == ActiveTool::Material {
                        // The open map's boden.txt loads on demand, like Flat Terrain does; one
                        // attempt per opened map so a failure is not retried every frame.
                        if self.textures.session_textures().is_none()
                            && self.material_autoload_tried != self.loaded_map_path
                        {
                            self.material_autoload_tried = self.loaded_map_path.clone();
                            if let Some(path) = self.loaded_map_path.clone() {
                                if let Err(e) = self.textures.load_session(&path) {
                                    self.status_message = format!("Materials: {e}");
                                    self.status_is_error = true;
                                }
                            }
                        }
                        let regions = self
                            .nature
                            .data
                            .as_ref()
                            .map(|d| d.regions.clone())
                            .unwrap_or_default();
                        self.textures.show_material_palette(
                            ui,
                            lang == AppLanguage::English,
                            &regions,
                        );
                    } else {
                        ui.add(
                            egui::Slider::new(&mut self.brush_radius, 1.0..=64.0)
                                .text(tr(lang, "Radius (vertices) [ / ]", "半徑（頂點）[ / ]")),
                        );
                        ui.add(egui::Slider::new(&mut self.brush_strength, 1..=64).text("Strength"));
                    }

                    if self.active_tool == ActiveTool::HeightFlatten {
                        ui.checkbox(&mut self.sample_flatten_target, "Sample first dab height");
                        if !self.sample_flatten_target {
                            ui.add(
                                egui::Slider::new(&mut self.flatten_target, 0..=255)
                                    .text("Flatten Target"),
                            );
                        }
                    }
                    if self.active_tool == ActiveTool::Water {
                        ui.add(
                            egui::Slider::new(&mut self.water_surface, 0.0..=255.0)
                                .text("Water surface (G units)"),
                        );
                        ui.add(egui::Slider::new(&mut self.water_bed, 0..=254).text("Bed height"));
                        ui.label("Local excavation/blocking; set game water level separately.");
                        ui.label(format!(
                            "World water: {:.2}; Heightmapstep: {}",
                            self.water_surface * self.height_step,
                            self.height_step
                        ));
                        if self.water_surface < 4.0 {
                            ui.label("Water surface too low for the water brush (minimum 4).");
                        }
                    }
                    if self.active_tool == ActiveTool::HeightRoughen {
                        ui.add(
                            egui::Slider::new(&mut self.roughness_seed, 0..=1000)
                                .text("Noise Seed"),
                        );
                    }

                    ui.separator();
                    ui.label(tr(lang, "Terrain:", "地表："));
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(self.can_reset_terrain(), egui::Button::new(tr(lang, "Reset Terrain", "還原地表")))
                            .on_hover_text(tr(
                                lang,
                                "Discard unsaved terrain changes (textures, heights, passability).",
                                "放棄尚未儲存的地表變更（材質、高度、通行區域）。",
                            ))
                            .clicked()
                        {
                            self.request_terrain_action(TerrainBulkAction::Reset);
                        }
                        if ui
                            .add_enabled(self.can_apply_flat_terrain(), egui::Button::new(tr(lang, "Reset Flat Terrain…", "重設平坦地形…")))
                            .on_hover_text(tr(
                                lang,
                                "Flatten the whole map just above the water level, paint one base material, clear blocked ground, and reset vertex colors / smoothing / lighting on Save. Settlements are kept.",
                                "整平整張地圖到水面上方、鋪單一基礎材質、清除阻擋，並在儲存時重設頂點色／平滑／光照。聚落保留。",
                            ))
                            .clicked()
                        {
                            self.request_terrain_action(TerrainBulkAction::Flat);
                        }
                    });
                    ui.checkbox(&mut self.show_grid, tr(lang, "Show grid", "顯示格線"));
                    ui.checkbox(&mut self.scenario.canvas.show,
                        tr(lang, "Show scenario overlay", "顯示劇本標記"));
                    ui.checkbox(
                        &mut self.show_objects,
                        tr(lang, "Show buildings and objects", "顯示建築與場景物件"),
                    );

                    ui.separator();
                    self.show_nature_panel(ui);

                    ui.separator();
                    ui.heading("Map Diagnostics");
                    if let Some(session) = &self.session {
                        let v_size = session.vertex_size;
                        let is_standard_257 = v_size == 257;
                        if is_standard_257 {
                            ui.label(format!(
                                "Height Dimensions: {}x{} (Standard)",
                                v_size, v_size
                            ));
                        } else {
                            ui.colored_label(
                                Color32::from_rgb(255, 180, 50),
                                format!(
                                    "Height Dimensions: {}x{} (Non-257 Compatible)",
                                    v_size, v_size
                                ),
                            );
                        }
                        ui.label(format!(
                            "Collision Grid: {}x{}",
                            session.collision_size, session.collision_size
                        ));
                        ui.label("Channel: Green channel height decoded");
                        if session.raw_boden_bmp.is_some() {
                            ui.colored_label(Color32::LIGHT_GREEN, "Original R/B pixels preserved");
                        } else {
                            ui.label("Synthesized grayscale heightmap");
                        }
                    }

                    ui.separator();
                    ui.heading("Quick Presets");
                    if ui.button("New Blank 257x257 (Retail)").clicked() {
                        self.pending_action = Some(PendingAction::Blank(257));
                    }
                    if ui.button("New Blank 193x193 (TUTOR_01)").clicked() {
                        self.pending_action = Some(PendingAction::Blank(193));
                    }
                });
            });

        // 導航取點僅使用 2D 畫布，避免 3D 筆刷接收相同手勢。
        if self.navigation.owns_canvas() && self.view_mode == ViewMode::View3D {
            self.view_mode = ViewMode::Blended;
            self.texture_dirty = true;
        }
        // 中央畫布檢視區
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.view_mode == ViewMode::View3D {
                self.refresh_troop_objects(ctx);
                if let Some(session) = &self.session {
                    let placement_objects = self.placement.objects();
                    let selected_obj = self.placement.selected_object();
                    self.viewport_3d.terrain_pick_mode = self.ramp_selecting;
                    self.viewport_3d.show_grid = self.show_grid;
                    self.viewport_3d.show_objects = self.show_objects;
                    if let Some(grid) = self.textures.take_view_sync() {
                        self.viewport_3d.set_tile_textures(grid);
                    }
                    self.viewport_3d.scene_pick = self.scene.scene_pick_active();
                    self.viewport_3d.scene_markers =
                        if self.scene.open { self.scene.markers() } else { Vec::new() };
                    let action = self.viewport_3d.show(
                        ui,
                        ctx,
                        session,
                        self.height_step,
                        self.water_surface,
                        self.brush_radius,
                        &placement_objects,
                        selected_obj,
                    );
                    for event in std::mem::take(&mut self.viewport_3d.scene_events) {
                        self.scene.scene_pointer(event);
                    }
                    self.handle_viewport_3d_action(action);
                    if let Some(hit) = self.viewport_3d.hover_world {
                        let wx = hit.x * 256.0;
                        let wz = hit.z * 256.0;
                        let (tx, tz) = cursor_tile(wx, wz);
                        self.canvas_cursor = Some((wx, wz, tx, tz));
                    } else {
                        self.canvas_cursor = None;
                    }
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.heading("No terrain session loaded. Create or load a map first.");
                    });
                }
                return;
            }

            if self.texture_dirty || self.texture.is_none() {
                self.update_texture(ctx);
            }

            let Some(texture) = &self.texture else { return };
            let Some(session) = &self.session else { return };

            let (response, painter) =
                ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
            let canvas_rect = response.rect;

            // 計算畫布置中顯示
            let map_aspect = 1.0;
            let display_size = if canvas_rect.width() / canvas_rect.height() > map_aspect {
                Vec2::new(canvas_rect.height() * map_aspect, canvas_rect.height())
            } else {
                Vec2::new(canvas_rect.width(), canvas_rect.width() / map_aspect)
            } * self.zoom;

            let top_left = canvas_rect.min + (canvas_rect.size() - display_size) * 0.5 + self.pan;
            let map_rect = Rect::from_min_size(top_left, display_size);
            self.last_canvas_rect = canvas_rect;
            self.last_map_rect = map_rect;

            // 最小可用 pan state：支援滑鼠中鍵拖曳平移
            if response.dragged_by(egui::PointerButton::Middle) {
                self.pan += response.drag_delta();
            }

            // 無修飾鍵由畫布處理；Shift 方向鍵已由全域路徑處理一次。
            if response.hovered() && !ctx.wants_keyboard_input() && !self.shortcuts_blocked() {
                let (ctrl, alt) = ctx.input(|i| (i.modifiers.ctrl || i.modifiers.command, i.modifiers.alt));
                if !ctrl && !alt && !ctx.input(|i| i.modifiers.shift) {
                    let step = 64.0;
                    if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                        self.pan.x += step;
                    }
                    if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                        self.pan.x -= step;
                    }
                    if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                        self.pan.y += step;
                    }
                    if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                        self.pan.y -= step;
                    }
                }
            }

            // 支援滑鼠滾輪縮放（以游標為錨點保持世界座標不變）
            let scroll_delta = ctx.input(|i| i.raw_scroll_delta.y);
            if response.hovered() && scroll_delta.abs() > 0.0 {
                let factor = if scroll_delta > 0.0 {
                    1.15_f32
                } else {
                    1.0_f32 / 1.15_f32
                };
                let new_zoom = (self.zoom * factor).clamp(0.25, 10.0);
                if let Some(mouse_pos) = response.hover_pos() {
                    let center_offset = mouse_pos - canvas_rect.center();
                    self.pan = (self.pan - center_offset) * (new_zoom / self.zoom) + center_offset;
                }
                self.zoom = new_zoom;
            }

            painter.image(
                texture.id(),
                map_rect,
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            if self.active_tool == ActiveTool::Material {
                self.textures.draw_material_overlay(ctx, &painter, map_rect);
            }
            if self.show_grid {
                let dimension = self.textures.tile_dimension().unwrap_or(NATURE_DEFAULT_DIMENSION as usize);
                draw_canvas_grid(&painter, map_rect, dimension);
            }
            if self.show_objects {
                self.draw_nature_overlay(&painter, map_rect);
                self.spawn_ui.draw_overlay(&painter, map_rect);
            }

            self.navigation.paint_canvas(&painter, map_rect, session.collision_size);
            if self.navigation.canvas_input(ctx, &response, map_rect, session.collision_size) {
                self.spawn_ui.cancel_drag();
                self.scene.end_drag();
                return;
            }
            self.scenario.paint_canvas(&painter, map_rect, lang == AppLanguage::English);
            let (pick_owned, pick_status) = self.scenario.canvas_pick_input(
                ctx, &response, map_rect, lang == AppLanguage::English);
            if let Some((message, error)) = pick_status {
                self.status_message = message;
                self.status_is_error = error;
            }
            if pick_owned || scenario_cancelled {
                self.spawn_ui.cancel_drag();
                self.scene.end_drag();
                return;
            }

            // 放置物件面板：點選／拖曳 2D 標記（命中時筆刷不作用）
            let tool_busy = self.pending_action.is_some()
                || self.box_select
                || self.ramp_selecting
                || self.nature.enabled
                || self.landscape.region_pick_active();
            let spawn_cx = SpawnContext {
                lang: self.loc.current_language(),
                heights: Some(session.heights()),
                vertex_size: session.vertex_size,
                height_step: self.height_step,
                read_only: is_original_map_dir(Path::new(self.map_path_input.trim())),
            };
            Self::handle_spawn_canvas(
                ctx,
                &response,
                map_rect,
                tool_busy,
                &spawn_cx,
                &mut self.spawn_ui,
                &mut self.scenario,
            );

            // SDL 場景物件：疊圖＋移動模式的點選／拖曳（命中時筆刷不作用）
            {
                let world_from_screen = |p: Pos2| crate::scenario_canvas::world_from_screen(map_rect, p);
                let screen_from_world = |w: (f32, f32)| crate::scenario_canvas::screen_from_world(map_rect, w.0, w.1);
                if self.show_objects {
                    self.scene.paint_scene_overlay(&painter, &screen_from_world);
                }
                if !tool_busy {
                    self.scene
                        .scene_canvas_input(&response, &world_from_screen, &screen_from_world);
                } else if self.scene.is_capturing() {
                    self.scene.end_drag();
                }
            }

            if let Some(mouse_pos) = response.hover_pos() {
                if map_rect.contains(mouse_pos) {
                    let (wx, wz) = crate::scenario_canvas::world_from_screen(map_rect, mouse_pos);
                    let (tile_x, tile_y) = cursor_tile(wx, wz);
                    self.canvas_cursor = Some((wx, wz, tile_x, tile_y));
                } else {
                    self.canvas_cursor = None;
                }
            } else {
                self.canvas_cursor = None;
            }

            // 處理滑鼠操作繪製
            if let Some(mouse_pos) = response.hover_pos().filter(|_| {
                !self.box_select
                    && !self.landscape.region_pick_active()
                    && !self.spawn_ui.is_capturing()
                    && !self.scene.scene_pick_active()
            }) {
                if map_rect.contains(mouse_pos) {
                    let norm_x = (mouse_pos.x - map_rect.min.x) / map_rect.width();
                    let norm_y = (mouse_pos.y - map_rect.min.y) / map_rect.height();

                    let grid_x = norm_x * (session.vertex_size - 1) as f32;
                    let grid_y = norm_y * (session.vertex_size - 1) as f32;

                    // 繪製筆刷圓圈指示器
                    let brush_vertices = if self.nature.enabled {
                        self.nature.brush_size as f32 / 2.0 * (session.vertex_size - 1) as f32
                            / NATURE_DEFAULT_DIMENSION as f32
                    } else if self.active_tool == ActiveTool::Material {
                        self.textures.material_brush_tiles() / 2.0 * (session.vertex_size - 1) as f32
                            / self.textures.tile_dimension().unwrap_or(64) as f32
                    } else {
                        self.brush_radius
                    };
                    let pixel_radius =
                        brush_vertices * (map_rect.width() / (session.vertex_size - 1) as f32);
                    painter.circle_stroke(
                        mouse_pos,
                        pixel_radius,
                        Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 180)),
                    );

                    // 筆刷塗畫
                    if self.pending_action.is_none() && self.ramp_selecting {
                        if response.clicked_by(egui::PointerButton::Primary) {
                            self.handle_ramp_point((grid_x, grid_y));
                        } else if response.clicked_by(egui::PointerButton::Secondary) {
                            self.cancel_ramp_selection();
                        }
                    } else if self.spawn_ui.place_mode() {
                        self.spawn_ui.draw_ghost(&painter, mouse_pos);
                        if self.pending_action.is_none() && response.clicked_by(egui::PointerButton::Primary) {
                            let tiles = 64.0_f32;
                            let tile_x = ((norm_x * tiles).floor() as i32).clamp(0, 63);
                            let tile_y = ((norm_y * tiles).floor() as i32).clamp(0, 63);
                            let cx = SpawnContext {
                                lang: self.loc.current_language(),
                                heights: Some(session.heights()),
                                vertex_size: session.vertex_size,
                                height_step: self.height_step,
                                read_only: is_original_map_dir(Path::new(self.map_path_input.trim())),
                            };
                            self.spawn_ui.set_active(true);
                            self.spawn_ui.place_at_tile(&mut self.scenario, &cx, tile_x, tile_y);
                        }
                    } else if self.nature.enabled {
                        if self.pending_action.is_none()
                            && (response.is_pointer_button_down_on()
                                && ctx.input(|i| i.pointer.primary_down())
                                || response.clicked_by(egui::PointerButton::Primary))
                        {
                            if let Some((tx, ty)) = self.nature_tile_from_grid(grid_x, grid_y) {
                                self.nature_paint_tile(tx, ty);
                            }
                        }
                    } else if self.active_tool == ActiveTool::Material {
                        // Material tool (C# texture tool): drag paints, right-click eyedrops.
                        if self.pending_action.is_none() {
                            let english = self.lang() == AppLanguage::English;
                            let span = (session.vertex_size - 1) as f32;
                            if response.clicked_by(egui::PointerButton::Secondary) {
                                self.textures
                                    .sample_material_at_grid(grid_x, grid_y, span, english);
                            } else if response.is_pointer_button_down_on()
                                && ctx.input(|i| i.pointer.primary_down())
                                || response.clicked_by(egui::PointerButton::Primary)
                            {
                                self.textures
                                    .paint_material_at_grid(grid_x, grid_y, span, english);
                            }
                        }
                    } else if self.pending_action.is_none()
                        && (response.is_pointer_button_down_on()
                            && ctx.input(|i| i.pointer.primary_down())
                            || response.clicked_by(egui::PointerButton::Primary))
                    {
                        if !self.is_painting {
                            self.stroke_path.clear();
                        }
                        self.is_painting = true;
                        self.nature.last_action = false;
                        let tool = self.active_tool;
                        let radius = self.brush_radius;
                        let strength = self.brush_strength;
                        let seed = self.roughness_seed;

                        let points = self.stroke_path.advance((
                            grid_x.round_ties_even() as i32,
                            grid_y.round_ties_even() as i32,
                        ));

                        if let Some(session_mut) = &mut self.session {
                            for (x, y) in points {
                                let grid_x = x as f32;
                                let grid_y = y as f32;
                                if tool == ActiveTool::HeightFlatten
                                    && self.stroke_path.flatten_target.is_none()
                                {
                                    self.stroke_path.flatten_target =
                                        Some(if self.sample_flatten_target {
                                            session_mut.heights()
                                                [y as usize * session_mut.vertex_size + x as usize]
                                        } else {
                                            self.flatten_target
                                        });
                                }
                                let flat_target = self.stroke_path.flatten_target;
                                let result = match tool {
                            // Material is handled by its own branch before this brush loop.
                            ActiveTool::Material => continue,
                                    ActiveTool::HeightRaise => session_mut.paint_height(
                                        grid_x,
                                        grid_y,
                                        radius,
                                        HeightBrushOp::Raise,
                                        strength,
                                        None,
                                        seed,
                                    ),
                                    ActiveTool::HeightLower => session_mut.paint_height(
                                        grid_x,
                                        grid_y,
                                        radius,
                                        HeightBrushOp::Lower,
                                        strength,
                                        None,
                                        seed,
                                    ),
                                    ActiveTool::HeightFlatten => session_mut.paint_height(
                                        grid_x,
                                        grid_y,
                                        radius,
                                        HeightBrushOp::Flatten,
                                        strength,
                                        flat_target,
                                        seed,
                                    ),
                                    ActiveTool::HeightSmooth => session_mut.paint_height(
                                        grid_x,
                                        grid_y,
                                        radius,
                                        HeightBrushOp::Smooth,
                                        strength,
                                        None,
                                        seed,
                                    ),
                                    ActiveTool::HeightRoughen => session_mut.paint_height(
                                        grid_x,
                                        grid_y,
                                        radius,
                                        HeightBrushOp::Roughen,
                                        strength,
                                        None,
                                        seed,
                                    ),
                                    ActiveTool::Water if self.water_surface < 4.0 => {
                                        Err(against_rome_editor::terrain::Error::InvalidParameters)
                                    }
                                    ActiveTool::Water => session_mut
                                        .paint_water(
                                            grid_x,
                                            grid_y,
                                            radius,
                                            strength,
                                            self.water_bed,
                                            self.water_surface,
                                        )
                                        .map(|s| s.heights),
                                    ActiveTool::CollisionBlock => {
                                        let col_scale = session_mut.collision_size as f32
                                            / (session_mut.vertex_size - 1) as f32;
                                        session_mut.paint_collision(
                                            grid_x * col_scale,
                                            grid_y * col_scale,
                                            radius * col_scale,
                                            CollisionBrushOp::Block,
                                        )
                                    }
                                    ActiveTool::CollisionClear => {
                                        let col_scale = session_mut.collision_size as f32
                                            / (session_mut.vertex_size - 1) as f32;
                                        session_mut.paint_collision(
                                            grid_x * col_scale,
                                            grid_y * col_scale,
                                            radius * col_scale,
                                            CollisionBrushOp::Clear,
                                        )
                                    }
                                };
                                if let Err(error) = result {
                                    self.status_message = format!("Brush rejected: {error}");
                                    self.status_is_error = true;
                                }
                            }
                            self.texture_dirty = true;
                        }
                    }
                }
            }

            // Region Tools：拖曳矩形並繪製選取覆蓋
            self.handle_region_canvas(&response, &painter, map_rect);

            // 框選工具：拖曳矩形選取放置物件
            if self.box_select {
                self.handle_box_select_input(ctx, &response, &painter, map_rect);
            }

            // 放開拖曳時提交自然物件筆劃（一筆一個復原步驟）
            if !ctx.input(|i| i.pointer.primary_down()) && self.nature.driver.is_active() {
                self.nature_end_stroke();
            }
            // 放開拖曳時提交材質筆劃（一筆一個復原步驟）
            if !ctx.input(|i| i.pointer.primary_down()) {
                self.textures.end_material_stroke();
            }
            // 放開拖曳時提交筆劃
            if !ctx.input(|i| i.pointer.primary_down()) && self.is_painting {
                self.is_painting = false;
                if let Some(session_mut) = &mut self.session {
                    session_mut.commit_stroke();
                }
                self.stroke_path.clear();
            }
        });
        self.handle_pending_action(ctx);
        self.toolbox.show(ctx);
        self.navigation
            .show(ctx, self.session.as_ref(), self.water_surface);
        if let Some(new_pan) = self.minimap.show(
            ctx,
            self.session.as_ref(),
            self.texture.as_ref(),
            self.last_canvas_rect,
            self.last_map_rect,
            self.zoom,
        ) {
            self.pan = new_pan;
        }
        self.refresh_placement_type_names();
        self.placement.settlement_history_routing = !self.settlement_undo.is_empty() || !self.settlement_redo.is_empty();
        self.placement.settlement_history_available = (!self.settlement_undo.is_empty(), !self.settlement_redo.is_empty());
        self.placement.show(ctx);
        // Opening the Scenario, Spawns, or Balance Summary panel loads the open map's script on demand (once per
        // map, never over pending edits); otherwise added events or objects could not be
        // checked or saved, and Spawns would send the designer to the Scenario panel.
        if self.scenario.open || self.spawn_ui.open || self.balance_summary.open {
            self.autoload_scenario_if_needed();
        }
        // 初次開啟與按下主屋按鈕才重算，避免每幀讀取原生 DATA。
        if self.scenario.open && self.scenario.wave_target_requested {
            self.scenario.wave_target_requested = false;
            self.scenario.wave_target = self.loaded_map_path.as_deref()
                .and_then(|path| self.wave_player_target(path));
            if let Some(target) = &self.scenario.wave_target {
                self.scenario.wave_generator.destination = target.clone();
            }
        }
        self.scenario.english = self.lang() == AppLanguage::English;
        self.scenario.show(ctx);
        if let Some(prompt) = self.scenario.pick_prompt(self.scenario.english) {
            self.status_message = prompt.into();
            self.status_is_error = false;
        }
        if let Some(result) = self.scenario.objective_notice.take() {
            match result {
                Ok(added) => {
                    self.status_message = match (self.scenario.english, added) {
                        (true, true) => "Objective added; Save to write it.",
                        (false, true) => "已新增勝負條件；請存檔以寫入變更。",
                        (true, false) => "Equivalent objective already present.",
                        (false, false) => "已存在等價勝負條件。",
                    }.into();
                    self.status_is_error = false;
                    if added { self.run_design_check(); }
                }
                Err(e) => { self.status_message = e; self.status_is_error = true; }
            }
        }
        if let Some(result) = self.scenario.wave_notice.take() {
            match result {
                Ok(n) => {
                    self.status_message = if self.scenario.english {
                        format!("Generated {n} attack waves; Save to write them.")
                    } else { format!("已產生 {n} 個進攻波次；請存檔以寫入變更。") };
                    self.status_is_error = false;
                    self.run_design_check();
                }
                Err(e) => { self.status_message = e; self.status_is_error = true; }
            }
        }
        self.show_spawn_panel(ctx);
        self.textures.show(ctx);
        self.scene.english = self.lang() == AppLanguage::English;
        self.scene.show(ctx);

        self.console.show(ctx);
        self.tech_doc.show(ctx);
        self.show_design_check(ctx);
        self.show_balance_summary(ctx);
        self.show_map_properties(ctx);
        self.show_save_blockers(ctx);
        self.consume_design_check_focus();
        self.show_studio_panels(ctx);
        self.consume_panel_history_requests();
        self.consume_panel_save_requests();
        self.collect_pilot_receipts();
    }
}

impl EditorApp {
    /// Autoloads the scenario on demand (once per map, never over pending edits)
    /// when panels like Scenario, Spawns, or Balance Summary are opened.
    pub(crate) fn autoload_scenario_if_needed(&mut self) {
        if !self.scenario.is_loaded()
            && !self.scenario.dirty()
            && self.scenario_autoload_tried != self.loaded_map_path
        {
            self.scenario_autoload_tried = self.loaded_map_path.clone();
            if let Some(path) = self.loaded_map_path.clone() {
                if let Err(e) = self.scenario.load_session(&path) {
                    self.status_message = format!("Scenario: {e}");
                    self.status_is_error = true;
                }
            }
        }
    }

    /// 收集已提交的子系統操作，依全域序號排列。
    fn collect_pilot_receipts(&mut self) {
        use crate::placement_ui::PlacementHistoryReceipt;
        use crate::undo_journal::{SubsystemId, SubsystemStep};

        let mut receipts = self
            .session
            .as_mut()
            .map_or_else(Vec::new, |s| s.take_history_receipts());
        receipts.extend(self.textures.take_history_receipts());
        receipts.extend(self.spawn_ui.take_history_receipts());
        receipts.extend(self.scenario.take_history_receipts());
        receipts.extend(self.scene.take_history_receipts());
        receipts.extend(self.nature.session.take_history_receipts().into_iter().map(
            |(before_token, after_token)| SubsystemStep {
                subsystem: SubsystemId::Nature,
                before_token,
                after_token,
            },
        ));
        let mut notifications: Vec<_> = receipts
            .into_iter()
            .map(PlacementHistoryReceipt::Commit)
            .collect();
        notifications.extend(self.placement.take_history_receipts());
        // 外部 DATA 修改也佔用同一序號軸，僅清除它之前的旁錄。
        notifications.sort_by_key(PlacementHistoryReceipt::after_token);
        for notification in notifications {
            match notification {
                PlacementHistoryReceipt::Commit(step) => {
                    self.undo_journal.record_compound(vec![step]);
                }
                PlacementHistoryReceipt::Barrier { .. } => self.undo_journal.clear(),
            }
        }
    }
}
#[cfg(test)]
impl EditorApp {
    fn pilot_history_consistent(&self) -> bool {
        self.undo_journal.consistent_tokens(|id| match id {
            crate::undo_journal::SubsystemId::Terrain => {
                self.session.as_ref().map_or(0, |s| s.history_token())
            }
            crate::undo_journal::SubsystemId::Materials => self.textures.history_token(),
            crate::undo_journal::SubsystemId::Placement => self.placement.history_token(),
            crate::undo_journal::SubsystemId::Spawns => self.spawn_ui.history_token(),
            crate::undo_journal::SubsystemId::Scenario => self.scenario.history_token(),
            crate::undo_journal::SubsystemId::Nature => self.nature.session.history_token(),
            crate::undo_journal::SubsystemId::Scene => self.scene.history_token(),
            crate::undo_journal::SubsystemId::Auxiliary => self.auxiliary_undo.len() as u64,
            _ => 0,
        })
    }
}

#[cfg(test)]
mod pilot_history_tests {
    use super::*;
    #[test]
    fn step3b_scene_receipts_are_collected_in_commit_order_shadow_only() {
        use crate::undo_journal::{SubsystemId, UndoTarget};

        // 僅供測試委派順序，不接入任何正式 UI 路由。
        struct Target<'a>(&'a mut EditorApp, Vec<SubsystemId>);
        impl UndoTarget for Target<'_> {
            fn token(&self, id: SubsystemId) -> u64 {
                match id {
                    SubsystemId::Scene => self.0.scene.history_token(),
                    SubsystemId::Terrain => self.0.session.as_ref().unwrap().history_token(),
                    _ => panic!("非預期子系統：{id:?}"),
                }
            }
            fn undo(&mut self, id: SubsystemId) -> bool {
                self.1.push(id);
                match id {
                    SubsystemId::Scene => self.0.scene.undo(),
                    SubsystemId::Terrain => self.0.session.as_mut().unwrap().undo(),
                    _ => panic!("非預期子系統：{id:?}"),
                }
            }
            fn redo(&mut self, id: SubsystemId) -> bool {
                self.1.push(id);
                match id {
                    SubsystemId::Scene => self.0.scene.redo(),
                    SubsystemId::Terrain => self.0.session.as_mut().unwrap().redo(),
                    _ => panic!("非預期子系統：{id:?}"),
                }
            }
        }
        let dir = std::env::temp_dir().join(format!("arm_step3b_shadow_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("scene.sdl"),
            "[settlement]\nrefpos=1000,0,2000\n[object0000]\ndef=1700\nnamedef=BauRomHau00_Haupthaus\npos=100,0,100\nteam=0\n").unwrap();
        let mut app = EditorApp::initial_state();
        app.scene.reset_session(Some(&dir));
        assert_eq!(app.scene.objects().len(), 1);
        let original = app.scene.history_token();
        app.scene.set_team(0, 3).unwrap();
        let scene_token = app.scene.history_token();
        let terrain = app.session.as_mut().unwrap();
        terrain.apply_height_changes(&[crate::terrain::SampleChange {
            index: 0, before: 32, after: 40,
        }]);
        assert!(terrain.commit_stroke());
        app.collect_pilot_receipts();
        assert!(app.pilot_history_consistent());
        let journal = app.undo_journal.clone();
        app.collect_pilot_receipts();
        assert_eq!(app.undo_journal, journal);
        // 原面板仍只復原 Scene，旁錄不介入路由。
        assert!(app.scene.undo());
        assert_eq!(app.scene.history_token(), original);
        assert_eq!(app.session.as_ref().unwrap().heights()[0], 40);
        assert!(!app.pilot_history_consistent());
        assert!(app.scene.redo());
        assert_eq!(app.scene.history_token(), scene_token);
        assert!(app.pilot_history_consistent());
        app.collect_pilot_receipts();
        assert_eq!(app.undo_journal, journal);
        let mut replay = journal.clone();
        let mut target = Target(&mut app, Vec::new());
        assert!(replay.undo(&mut target));
        assert!(replay.undo(&mut target));
        assert!(!replay.undo(&mut target));
        assert_eq!(target.0.scene.history_token(), original);
        assert!(replay.redo(&mut target));
        assert!(replay.redo(&mut target));
        assert!(!replay.redo(&mut target));
        assert_eq!(target.1, vec![SubsystemId::Terrain, SubsystemId::Scene,
            SubsystemId::Scene, SubsystemId::Terrain]);
        app.scene.set_team(0, 3).unwrap();
        app.collect_pilot_receipts();
        assert_eq!(app.undo_journal, journal);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn pilot_records_without_switching_old_navigation() {
        let mut app = EditorApp::initial_state();
        let terrain = app.session.as_mut().unwrap();
        terrain.apply_height_changes(&[crate::terrain::SampleChange {
            index: 0,
            before: 32,
            after: 40,
        }]);
        assert!(terrain.commit_stroke());
        app.collect_pilot_receipts();
        assert!(app.undo_journal.can_undo());
        assert!(app.pilot_history_consistent());
        assert!(app.session.as_mut().unwrap().undo());
        assert!(!app.pilot_history_consistent());
    }
}

#[cfg(test)]
mod step3a_history_tests {
    use super::tests::spawn_save_tests::{cx, fixture, place_house};
    use super::*;
    use crate::undo_journal::{SubsystemId, UndoJournal, UndoTarget};

    /// 僅供交錯測試使用，不接管工具列或面板路由。
    struct RealTarget<'a> {
        app: &'a mut EditorApp,
        calls: Vec<(bool, SubsystemId)>,
    }

    impl UndoTarget for RealTarget<'_> {
        fn token(&self, id: SubsystemId) -> u64 {
            match id {
                SubsystemId::Terrain => self.app.session.as_ref().unwrap().history_token(),
                SubsystemId::Spawns => self.app.spawn_ui.history_token(),
                SubsystemId::Scenario => self.app.scenario.history_token(),
                SubsystemId::Nature => self.app.nature.session.history_token(),
                _ => panic!("測試未支援的子系統：{id:?}"),
            }
        }

        fn undo(&mut self, id: SubsystemId) -> bool {
            self.calls.push((false, id));
            match id {
                SubsystemId::Terrain => self.app.session.as_mut().unwrap().undo(),
                SubsystemId::Spawns => self.app.spawn_ui.undo(&mut self.app.scenario, &cx()),
                SubsystemId::Scenario => self.app.scenario.undo_events(),
                SubsystemId::Nature => self.app.nature.session.undo(),
                _ => panic!("測試未支援的子系統：{id:?}"),
            }
        }

        fn redo(&mut self, id: SubsystemId) -> bool {
            self.calls.push((true, id));
            match id {
                SubsystemId::Terrain => self.app.session.as_mut().unwrap().redo(),
                SubsystemId::Spawns => self.app.spawn_ui.redo(&mut self.app.scenario, &cx()),
                SubsystemId::Scenario => self.app.scenario.redo_events(),
                SubsystemId::Nature => self.app.nature.session.redo(),
                _ => panic!("測試未支援的子系統：{id:?}"),
            }
        }
    }

    fn terrain_edit(app: &mut EditorApp, after: u8) {
        let terrain = app.session.as_mut().unwrap();
        let before = terrain.heights()[0];
        terrain.apply_height_changes(&[crate::terrain::SampleChange {
            index: 0,
            before,
            after,
        }]);
        assert!(terrain.commit_stroke());
    }

    fn event_edit(app: &mut EditorApp) {
        use against_rome_scenario::{Action, Event};
        let first = app.scenario.document().events.len();
        assert_eq!(
            app.scenario
                .add_events(&[Event {
                    name: "交錯事件".into(),
                    actions: vec![Action {
                        text: "step3a".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                }])
                .unwrap(),
            first
        );
    }

    fn nature_edit(app: &mut EditorApp) {
        app.nature
            .session
            .plant(against_rome_environment::nature::NatureAddition::new(
                1, "Tree", 10.0, 0.0, 10.0, 0.0,
            ));
        assert!(app.nature.session.commit_stroke());
    }

    fn snapshot(
        app: &EditorApp,
    ) -> (
        Vec<u8>,
        Vec<u8>,
        against_rome_environment::nature::NatureSnapshot,
    ) {
        (
            app.session.as_ref().unwrap().heights().to_vec(),
            serde_json::to_vec(app.scenario.document()).unwrap(),
            app.nature.session.capture(),
        )
    }

    #[test]
    fn batch9_bulk_events_interleave_with_terrain_in_unified_journal() {
        let (mut app, root, _) = fixture("batch9_bulk_journal", &[]);
        event_edit(&mut app);
        event_edit(&mut app);
        app.collect_pilot_receipts();
        app.undo_journal.clear();
        let before = snapshot(&app);
        assert_eq!(app.scenario.set_events_enabled(&[0, 1, 1, 999], false).unwrap(), 2);
        let bulk = snapshot(&app);
        let token = app.scenario.history_token();
        assert_eq!(app.scenario.set_events_enabled(&[0, 1], false).unwrap(), 0);
        assert_eq!(app.scenario.history_token(), token);
        terrain_edit(&mut app, 41);
        let after = snapshot(&app);
        app.collect_pilot_receipts();
        let mut journal = std::mem::take(&mut app.undo_journal);
        let mut target = RealTarget { app: &mut app, calls: vec![] };
        assert!(journal.undo(&mut target));
        assert_eq!(snapshot(target.app), bulk);
        assert!(journal.undo(&mut target));
        assert_eq!(snapshot(target.app), before);
        assert!(!journal.undo(&mut target));
        assert!(journal.consistent_tokens(|id| target.token(id)));
        assert!(journal.redo(&mut target));
        assert_eq!(snapshot(target.app), bulk);
        assert!(journal.redo(&mut target));
        assert_eq!(snapshot(target.app), after);
        assert!(!journal.redo(&mut target));
        assert!(journal.consistent_tokens(|id| target.token(id)));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn terrain_spawn_event_nature_chronological_undo_redo() {
        use SubsystemId::{Nature, Scenario, Spawns, Terrain};
        let (mut app, root, _) = fixture("step3a_chronological", &[]);
        app.collect_pilot_receipts();
        app.undo_journal.clear();
        let mut states = vec![snapshot(&app)];
        terrain_edit(&mut app, 40);
        states.push(snapshot(&app));
        place_house(&mut app, (30, 30), 0);
        states.push(snapshot(&app));
        event_edit(&mut app);
        states.push(snapshot(&app));
        nature_edit(&mut app);
        states.push(snapshot(&app));

        // 故意整批收集，驗證不同子系統的掃描順序不會取代提交順序。
        app.collect_pilot_receipts();
        assert!(app.pilot_history_consistent());
        let mut journal = std::mem::take(&mut app.undo_journal);
        let mut target = RealTarget {
            app: &mut app,
            calls: vec![],
        };
        for expected in states[..4].iter().rev() {
            assert!(journal.undo(&mut target));
            assert_eq!(&snapshot(target.app), expected);
            assert!(journal.consistent_tokens(|id| target.token(id)));
        }
        assert!(!journal.undo(&mut target));
        for expected in &states[1..] {
            assert!(journal.redo(&mut target));
            assert_eq!(&snapshot(target.app), expected);
            assert!(journal.consistent_tokens(|id| target.token(id)));
        }
        assert!(!journal.redo(&mut target));
        assert_eq!(
            target.calls,
            vec![
                (false, Nature),
                (false, Scenario),
                (false, Spawns),
                (false, Terrain),
                (true, Terrain),
                (true, Spawns),
                (true, Scenario),
                (true, Nature),
            ]
        );
        app.undo_journal = journal;
        let recorded = app.undo_journal.clone();
        app.collect_pilot_receipts();
        assert_eq!(app.undo_journal, recorded, "重收與導航不得新增 receipt");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn collector_orders_multiple_external_barriers_with_commits() {
        use crate::undo_journal::SubsystemStep;
        let (mut app, root, map) = fixture("step3a_barriers", &[(232, None)]);
        app.placement.load_session(&map).unwrap();
        app.collect_pilot_receipts();
        app.undo_journal.clear();
        terrain_edit(&mut app, 40);
        place_house(&mut app, (30, 30), 0);
        app.placement.select_object(Some((0, 1)));
        app.placement.nudge_selection(1.0, 0.0).unwrap();
        app.placement.note_external_store_edit();
        event_edit(&mut app);
        app.placement.note_external_store_edit();

        // 最後一道 barrier 之後的提交必須留下，且仍依跨系統時間排列。
        let nature_before = app.nature.session.history_token();
        nature_edit(&mut app);
        let nature_after = app.nature.session.history_token();
        let terrain_before = app.session.as_ref().unwrap().history_token();
        terrain_edit(&mut app, 48);
        let terrain_after = app.session.as_ref().unwrap().history_token();
        let placement_before = app.placement.history_token();
        app.placement.nudge_selection(1.0, 0.0).unwrap();
        let placement_after = app.placement.history_token();
        let content = snapshot(&app);
        app.collect_pilot_receipts();

        let mut expected = UndoJournal::default();
        for _ in 0..app.undo_journal.epoch() { expected.new_epoch(); }
        for (subsystem, before_token, after_token) in [
            (SubsystemId::Nature, nature_before, nature_after),
            (SubsystemId::Terrain, terrain_before, terrain_after),
            (SubsystemId::Placement, placement_before, placement_after),
        ] {
            assert!(expected.record_compound(vec![SubsystemStep {
                subsystem,
                before_token,
                after_token
            }]));
        }
        assert_eq!(app.undo_journal, expected);
        assert!(app.pilot_history_consistent());
        assert_eq!(snapshot(&app), content, "collector 只能旁錄，不得導航");
        app.collect_pilot_receipts();
        assert_eq!(app.undo_journal, expected, "所有通知都必須只消耗一次");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn collector_external_barrier_clears_existing_redo_and_pending_commits() {
        let mut app = EditorApp::initial_state();
        terrain_edit(&mut app, 40);
        app.collect_pilot_receipts();
        let mut journal = std::mem::take(&mut app.undo_journal);
        assert!(journal.undo(&mut RealTarget {
            app: &mut app,
            calls: vec![]
        }));
        assert!(journal.can_redo());
        app.undo_journal = journal;
        // 未收集的提交也在 barrier 之前，不能在清除之後被重新加回。
        nature_edit(&mut app);
        app.placement.note_external_store_edit();
        let content = snapshot(&app);
        app.collect_pilot_receipts();
        assert!(!app.undo_journal.can_undo());
        assert!(!app.undo_journal.can_redo());
        assert_eq!(snapshot(&app), content);
    }
}

impl EditorApp {
    /// 生命週期界線排空通知並建立新的統一歷程。
    fn reset_journal_epoch(&mut self) {
        // 排空尚未收集的通知，避免重設後於下一個 frame 復活。
        self.collect_pilot_receipts();
        self.undo_journal.new_epoch();
        self.auxiliary_undo.clear();
        self.auxiliary_redo.clear();
    }

    fn navigate_auxiliary(&mut self, forward: bool) -> bool {
        let source = if forward { &mut self.auxiliary_redo } else { &mut self.auxiliary_undo };
        let Some((reset, emboss)) = source.pop() else { return false; };
        let previous = (self.reset_auxiliary_layers, self.emboss.take());
        self.reset_auxiliary_layers = reset;
        self.emboss = emboss;
        if forward { self.auxiliary_undo.push(previous); } else { self.auxiliary_redo.push(previous); }
        true
    }

    /// 先結束手勢，再選擇統一紀錄或相容路由；委派失敗不得轉往其他子系統。
    fn finish_history_gestures(&mut self) {
        if let Some(session) = self.session.as_mut() { session.commit_stroke(); }
        self.is_painting = false;
        self.stroke_path.clear();
        self.textures.end_material_stroke();
        self.nature_end_stroke();
        self.scene.end_drag();
        let cx = Self::spawn_cx(&self.session, self.loc.current_language(), self.height_step, self.current_map_is_original());
        self.spawn_ui.canvas_release(&mut self.scenario, &cx);
        self.collect_pilot_receipts();
    }
    fn navigate_journal(&mut self, forward: bool) {
        self.finish_history_gestures();
        let mut journal = std::mem::take(&mut self.undo_journal);
        let available = if forward { journal.can_redo() } else { journal.can_undo() };
        let desync = !journal.matches_target(self, forward);
        if desync { journal.clear(); }
        if desync || (!journal.can_undo() && !journal.can_redo()) {
            self.undo_journal = journal;
            if forward { self.perform_redo(); } else { self.perform_undo(); }
            if desync {
                self.status_message = tr(self.loc.current_language(), "Undo history out of sync; cleared and used current tool history.", "復原紀錄已失同步，已清除並改用目前工具的復原路由。").into();
                self.status_is_error = false;
            }
            return;
        }
        if !available {
            self.undo_journal = journal;
            return;
        }
        let success = if forward { journal.redo(self) } else { journal.undo(self) };
        self.undo_journal = journal;
        self.texture_dirty = true;
        if success { self.spawn_landscape = None; }
        self.status_message = if success {
            tr(self.loc.current_language(), "Unified undo history applied.", "已套用統一復原紀錄。")
        } else {
            tr(self.loc.current_language(), "Undo operation failed; stopped without switching tools.", "復原操作失敗，已停止，未改用其他工具。")
        }.into();
        self.status_is_error = !success;
    }
}

impl crate::undo_journal::UndoTarget for EditorApp {
    fn token(&self, id: crate::undo_journal::SubsystemId) -> u64 {
        use crate::undo_journal::SubsystemId::*;
        match id {
            Terrain => self.session.as_ref().map_or(0, |s| s.history_token()),
            Materials => self.textures.history_token(),
            Placement => self.placement.history_token(),
            Nature => self.nature.session.history_token(),
            Spawns => self.spawn_ui.history_token(),
            Scenario => self.scenario.history_token(),
            Scene => self.scene.history_token(),
            Auxiliary => self.auxiliary_undo.len() as u64,
            Other => {
                // 局部操作若改動任一側，不得讓 journal 跨過複合操作邊界。
                let matches = self.settlement_undo.last().is_some_and(|edit| {
                    self.placement.matches_undo_boundary(&edit.after)
                        && self.nature.session.history_token() == edit.nature_after.history_token()
                        && self.nature.session.capture() == edit.nature_after.capture()
                }) || self.settlement_redo.last().is_some_and(|edit| {
                    self.placement.matches_state(&edit.before)
                        && self.nature.session.history_token() == edit.nature_before.history_token()
                        && self.nature.session.capture() == edit.nature_before.capture()
                });
                if matches { self.settlement_undo.len() as u64 } else { u64::MAX }
            },
        }
    }
    fn undo(&mut self, id: crate::undo_journal::SubsystemId) -> bool {
        use crate::undo_journal::SubsystemId::*;
        match id {
            Terrain => self.session.as_mut().is_some_and(|s| s.undo()),
            Materials => self.textures.material_undo(),
            Placement => self.placement.undo().is_ok(),
            Nature => {
                let success = self.nature.session.undo();
                if success { self.nature.last_action = true; }
                success
            },
            Spawns => {
                let cx = Self::spawn_cx(&self.session, self.loc.current_language(), self.height_step, self.current_map_is_original());
                self.spawn_ui.undo(&mut self.scenario, &cx)
            }
            Scenario => {
                let success = self.scenario.undo_events();
                if success { self.spawn_ui.sync_with_document(&self.scenario, self.loaded_map_path.as_deref()); }
                success
            },
            Scene => self.scene.undo(),
            Auxiliary => self.navigate_auxiliary(false),
            Other => {
                let success = self.settlement_history(false);
                // 此快照交換已由 journal 導航，不再送出旁錄 barrier。
                if success { self.placement.take_history_receipts(); }
                success
            },
        }
    }
    fn redo(&mut self, id: crate::undo_journal::SubsystemId) -> bool {
        use crate::undo_journal::SubsystemId::*;
        match id {
            Terrain => self.session.as_mut().is_some_and(|s| s.redo()),
            Materials => self.textures.material_redo(),
            Placement => self.placement.redo().is_ok(),
            Nature => {
                let success = self.nature.session.redo();
                if success { self.nature.last_action = true; }
                success
            },
            Spawns => {
                let cx = Self::spawn_cx(&self.session, self.loc.current_language(), self.height_step, self.current_map_is_original());
                self.spawn_ui.redo(&mut self.scenario, &cx)
            }
            Scenario => {
                let success = self.scenario.redo_events();
                if success { self.spawn_ui.sync_with_document(&self.scenario, self.loaded_map_path.as_deref()); }
                success
            },
            Scene => self.scene.redo(),
            Auxiliary => self.navigate_auxiliary(true),
            Other => {
                let success = self.settlement_history(true);
                // 此快照交換已由 journal 導航，不再送出旁錄 barrier。
                if success { self.placement.take_history_receipts(); }
                success
            },
        }
    }
}
impl EditorApp {
    fn consume_panel_history_requests(&mut self) {
        use crate::undo_journal::SubsystemId::*;
        let requests = [
            (Placement, self.placement.history_request.take()),
            (Scenario, self.scenario.history_request.take()),
            (Spawns, self.spawn_ui.history_request.take()),
            (Materials, self.textures.history_request.take()),
            (Scene, self.scene.history_request.take()),
            (Placement, self.family.history_request.take()),
        ];
        for (id, forward) in requests {
            if let Some(forward) = forward { self.navigate_panel(id, forward); }
        }
    }

    fn navigate_panel(&mut self, id: crate::undo_journal::SubsystemId, forward: bool) -> bool {
        use crate::undo_journal::UndoTarget;
        self.finish_history_gestures();
        let mut journal = std::mem::take(&mut self.undo_journal);
        if matches!(id, crate::undo_journal::SubsystemId::Placement | crate::undo_journal::SubsystemId::Nature) {
            let before = self.token(crate::undo_journal::SubsystemId::Other);
            if self.settlement_history(forward) {
                self.placement.take_history_receipts();
                if !journal.reconcile_settlement(self, forward, before) {
                    journal.clear();
                    self.status_message = tr(self.loc.current_language(),
                        "Panel history could not be reconciled; unified history cleared.",
                        "面板歷程無法安全整合，已清除統一復原紀錄。").into();
                }
                self.undo_journal = journal;
                self.texture_dirty = true;
                self.spawn_landscape = None;
                self.family.invalidate();
                self.navigation.invalidate();
                return true;
            }
        }
        let result = journal.navigate_subsystem(self, id, forward);
        let cleared = result.is_none();
        let success = result.unwrap_or_else(|| {
            // 無法安全拆解複合操作或 token 不符時，清除 journal，再只操作指定子系統。
            journal.clear();
            if forward { UndoTarget::redo(self, id) } else { UndoTarget::undo(self, id) }
        });
        self.undo_journal = journal;
        self.texture_dirty = true;
        if success {
            self.spawn_landscape = None;
            if id == crate::undo_journal::SubsystemId::Placement {
                self.family.invalidate();
                self.navigation.invalidate();
            }
        }
        self.status_message = if cleared {
            tr(self.loc.current_language(),
                "Panel history could not be reconciled; unified history cleared.",
                "面板歷程無法安全整合，已清除統一復原紀錄。")
        } else if success {
            tr(self.loc.current_language(), "Panel undo history applied.", "已套用面板復原紀錄。")
        } else {
            tr(self.loc.current_language(), "Panel undo operation failed; stopped.", "面板復原操作失敗，已停止。")
        }.into();
        self.status_is_error = !success;
        success
    }

    /// Renders the small Map Properties window displaying title, briefing, and start camera settings.
    pub fn show_map_properties(&mut self, ctx: &egui::Context) {
        if !self.map_properties_open {
            return;
        }

        self.autoload_scenario_if_needed();

        let lang = self.lang();
        let mut open = self.map_properties_open;

        egui::Window::new(tr(lang, "Map properties", "地圖屬性"))
            .open(&mut open)
            .default_width(460.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading(tr(lang, "Map Title & Briefing", "地圖標題與任務簡報"));
                    ui.label(tr(
                        lang,
                        "Edits native briefing metadata (TEXT/US/briefing.put).",
                        "編輯原生簡報資料（TEXT/US/briefing.put）。",
                    ));
                    ui.add_space(4.0);

                    ui.label(tr(lang, "Map Title:", "地圖標題:"));
                    if ui.text_edit_singleline(&mut self.ambience.briefing_title).changed() {
                        self.ambience.dirty = true;
                    }

                    ui.label(tr(lang, "Briefing Subtitle:", "任務副標題:"));
                    if ui.text_edit_singleline(&mut self.ambience.briefing_subtitle).changed() {
                        self.ambience.dirty = true;
                    }

                    ui.label(tr(lang, "Briefing Description / Objectives:", "任務簡報說明 / 目標:"));
                    if ui
                        .add(egui::TextEdit::multiline(&mut self.ambience.briefing_body).desired_rows(5))
                        .changed()
                    {
                        self.ambience.dirty = true;
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.heading(tr(lang, "Start Camera", "起始鏡頭"));
                    ui.label(tr(
                        lang,
                        "Initial camera coordinates when the map loads.",
                        "地圖載入時的初始鏡頭座標。",
                    ));
                    ui.add_space(4.0);

                    let camera_before = self.campaign.start_camera.clone();

                    ui.horizontal(|ui| {
                        ui.label(tr(lang, "Start Camera Enabled:", "啟用起始鏡頭:"));
                        ui.checkbox(&mut self.campaign.start_camera.enabled, "");
                    });

                    ui.horizontal(|ui| {
                        ui.label(tr(lang, "Start Position (X, Z):", "起始位置 (X, Z):"));
                        ui.add(
                            egui::DragValue::new(&mut self.campaign.start_camera.x)
                                .prefix("X: ")
                                .clamp_range(0.0..=16383.0),
                        );
                        ui.add(
                            egui::DragValue::new(&mut self.campaign.start_camera.z)
                                .prefix("Z: ")
                                .clamp_range(0.0..=16383.0),
                        );
                    });

                    if self.campaign.start_camera != camera_before {
                        self.campaign.camera_dirty = true;
                    }

                    ui.add_space(4.0);
                    let main_house = self.player_main_house_position();
                    let has_main_house = main_house.is_some();
                    ui.horizontal(|ui| {
                        let center_btn = ui.add_enabled(
                            has_main_house,
                            egui::Button::new(tr(
                                lang,
                                "Center on player main house",
                                "置中玩家主屋",
                            )),
                        );
                        if has_main_house {
                            if center_btn.clicked() {
                                self.center_start_camera_on_player_main_house();
                            }
                        } else {
                            center_btn.on_disabled_hover_text(tr(
                                lang,
                                "No player main house (Team 0) found on the map",
                                "未在地圖上找到玩家主屋（陣營 0）",
                            ));
                        }

                        if ui.button(tr(lang, "Apply", "套用")).clicked() {
                            let _ = self.apply_start_camera_candidate_from_ui();
                        }
                    });

                    if self.campaign.camera_dirty {
                        if self.campaign.camera_candidate_ready() {
                            ui.colored_label(
                                egui::Color32::from_rgb(120, 200, 120),
                                tr(
                                    lang,
                                    "Candidate ready; use Safe Save to write.",
                                    "候選已備妥；使用安全儲存寫入。",
                                ),
                            );
                        } else {
                            ui.colored_label(
                                egui::Color32::from_rgb(220, 180, 80),
                                tr(
                                    lang,
                                    "Camera edited; click Apply before Safe Save.",
                                    "鏡頭已變更；安全儲存前請點擊「套用」。",
                                ),
                            );
                        }
                    }
                });
            });

        self.map_properties_open = open;
    }
}
