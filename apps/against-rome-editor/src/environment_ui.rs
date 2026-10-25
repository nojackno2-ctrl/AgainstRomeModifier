//! Environment Studio UI: Flora Scatter, Neutral Lairs, Settlement Generator.
//! Replicates C# `SettlementGeneratorDialog.cs`, `WildLairDialog.cs` and integrates `native/against-rome-environment`.

use against_rome_environment::{
    nature::{
        BiomeEcologyProfile, BiomeType, FloraScatterContext, FloraScatterEngine,
        FloraScatterParameters, NatureAddition,
    },
    settlement::{
        MapLayoutPreset, MultiplayerDistributionResult, MultiplayerFairnessBalancer,
        ResourceClusterPlanner, SettlementGeneratorEngine, SettlementSiteEvaluator,
        SettlementTribe, SymmetryMode,
    },
    wild_lair::{
        NeutralLairCatalog, PlacedNeutralLair, ScenarioActionKind, ScenarioConditionKind, ScenarioEvent,
        WildLairScenarioEventBinder,
    },
};
use crate::layout_io::{self, LayoutDialogKind, LayoutFilePicker};
use crate::placement_ui::TileRect;
use crate::scenario_ui::ScenarioUi;
use against_rome_scenario::{Action, Condition, Event};
use crate::design_apply::{parse_templates, resolver, PlacementRequest};
use std::collections::HashMap;
use eframe::egui::{self, Color32, Rect, Stroke, Vec2};

/// World units per tile for lair spawn offsets (1 tile = 256 world units).
const LAIR_TILE_WORLD_SIZE: f32 = 256.0;
/// Scenario event cap enforced by `ScenarioUi::add_events`.
const SCENARIO_EVENT_LIMIT: usize = 256;

/// Settlement tiles are height-grid cells (four cells per 256-world-unit texture tile).
const SETTLEMENT_TILE_WORLD_SIZE: f32 = 64.0;

struct SettlementTerrain<'a> {
    dimension: usize,
    heights: &'a [u8],
    collision: &'a [u8],
    height_step: f32,
    water_world: f32,
}

impl<'a> SettlementTerrain<'a> {
    fn new(
        dimension: usize,
        heights: Option<&'a [u8]>,
        collision: Option<(usize, &'a [u8])>,
        water_level: f32,
        height_step: f32,
    ) -> Result<Self, String> {
        let heights = heights.ok_or("Load a terrain session before generating settlements.")?;
        let (collision_size, collision) =
            collision.ok_or("Current-session collision is unavailable.")?;
        if dimension < 2
            || dimension - 1 > i32::MAX as usize
            || dimension.checked_mul(dimension) != Some(heights.len())
            || collision_size != dimension - 1
            || collision_size.checked_mul(collision_size) != Some(collision.len())
        {
            return Err(
                "Inconsistent current-session height/collision dimensions or lengths.".into(),
            );
        }
        let water_world = water_level * height_step;
        if !height_step.is_finite()
            || height_step <= 0.0
            || !(255.0 * height_step).is_finite()
            || !water_world.is_finite()
        {
            return Err("Invalid current-session height step or water level.".into());
        }
        Ok(Self {
            dimension,
            heights,
            collision,
            height_step,
            water_world,
        })
    }

    fn sample_height(&self, x: f32, z: f32) -> f32 {
        // Match C# MapEditorForm.LayoutGroundHeight: map world coordinates to the
        // nearest height vertex, then multiply the raw sample by the active step.
        let max = (self.dimension - 1) as f32;
        let vx = (x / SETTLEMENT_TILE_WORLD_SIZE).clamp(0.0, max);
        let vz = (z / SETTLEMENT_TILE_WORLD_SIZE).clamp(0.0, max);
        let ix = vx.round_ties_even() as usize;
        let iz = vz.round_ties_even() as usize;
        self.heights[iz * self.dimension + ix] as f32 * self.height_step
    }

    fn is_blocked(&self, x: i32, z: i32) -> bool {
        let size = self.dimension - 1;
        x < 0
            || z < 0
            || x as usize >= size
            || z as usize >= size
            || against_rome_navigation::placement_collision_blocked(
                self.collision[z as usize * size + x as usize],
            )
    }
}

const DEFAULT_FLORA_TEMPLATES: &str = "1,LanGerNad00_Tanne_gross\n2,LanGerBus01_Busch\n3,LanGerGra01_Gras\n4,LanGerSch01_Schilf\n5,LanGerSte01_Stein";

/// Maps a binder event onto the scenario model (Message=0, SpawnUnit=2; ObjectExists=0, ObjectDeadOrRemoved=1).
fn to_scenario_event(e: &ScenarioEvent) -> Event {
    Event {
        name: e.name.clone(),
        delay_seconds: e.delay_seconds,
        repeat: e.repeat,
        enabled: e.active,
        actions: e
            .actions
            .iter()
            .map(|a| Action {
                kind: match a.kind {
                    ScenarioActionKind::Message => 0,
                    ScenarioActionKind::SpawnUnit => 2,
                },
                team: a.team,
                alias: a.alias.clone(),
                x: a.x,
                z: a.z,
                count: a.count,
                text: a.text.clone(),
                ..Default::default()
            })
            .collect(),
        conditions: e
            .conditions
            .iter()
            .map(|c| Condition {
                kind: match c.kind {
                    ScenarioConditionKind::ObjectExists => 0,
                    ScenarioConditionKind::ObjectDeadOrRemoved => 1,
                },
                target_id: c.target_id,
                ..Default::default()
            })
            .collect(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvApply {
    Flora,
    Lairs,
    Settlement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvironmentTab {
    FloraScatter,
    NeutralLairs,
    SettlementGenerator,
    Layouts,
}

/// Detailed resource allotment summary for an individual player base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseResourceSummary {
    pub base_index: usize,
    pub team: i32,
    pub tribe: SettlementTribe,
    pub world_x: i32,
    pub world_z: i32,
    pub building_count: usize,
    pub wood_count: usize,
    pub stone_count: usize,
    pub food_count: usize,
}

pub struct EnvironmentUi {
    pub open: bool,
    pub current_tab: EnvironmentTab,

    // --- Flora Scatter state ---
    pub selected_biome: BiomeType,
    pub flora_seed: i32,
    pub flora_density: f32,
    pub forest_coverage: f32,
    pub forest_patch_scale: f32,
    pub flora_jitter: f32,
    pub avoid_roads: bool,
    /// One `id,name` object template per line; replace with the loaded game's catalog.
    pub flora_templates_text: String,
    pub generated_flora: Vec<NatureAddition>,

    // --- Neutral Lairs state ---
    pub lair_catalog: NeutralLairCatalog,
    pub selected_lair_id: String,
    pub lair_world_x: f32,
    pub lair_world_z: f32,
    pub lair_rotation_deg: f32,
    pub generated_lairs: Vec<PlacedNeutralLair>,
    pub generated_lair_events: Vec<ScenarioEvent>,
    /// Scenario events produced per lair (parallel to `generated_lairs`), so removal drops its events.
    pub lair_event_counts: Vec<usize>,

    // --- Settlement Generator state ---
    pub settlement_players: usize,
    pub settlement_tribe: SettlementTribe,
    pub settlement_symmetry: SymmetryMode,
    pub settlement_seed: i32,
    pub settlement_include_nature: bool,
    pub settlement_spawn_radius: f32,
    pub last_distribution: Option<MultiplayerDistributionResult>,
    pub last_placement_preset: Option<MapLayoutPreset>,
    pub last_nature_preset: Option<MapLayoutPreset>,
    pub show_canvas_preview: bool,
    pub regeneration_blocked: bool,
    pub focus_base_request: Option<(f32, f32)>,

    // Dirty / guards
    /// Set by the "Apply to Objects session" buttons; the app turns it into placement requests.
    pub apply_requested: Option<EnvApply>,
    /// Set by the "Apply Lair Events" button; the app merges the pending lair events into the scenario.
    pub lair_events_apply_requested: bool,

    // --- Layout import/export (.arm-layout.json) ---
    /// Nature capture rectangle in tiles: left, top, right, bottom (half-open).
    pub layout_rect: [String; 4],
    /// Injectable layout file picker; `None` uses the native `rfd` dialogs.
    pub layout_picker: Option<LayoutFilePicker>,
    layout_export_request: Option<TileRect>,
    layout_import_requested: bool,

    pub dirty: bool,
    pub pending_discard: bool,
    pub english: bool,
    pending_clear_lairs: bool,
    pub status_message: String,
    pub status_is_error: bool,
}

impl Default for EnvironmentUi {
    fn default() -> Self {
        let catalog = NeutralLairCatalog::default();
        let default_lair_id = catalog
            .all_definitions()
            .first()
            .map(|d| d.id.clone())
            .unwrap_or_else(|| "LAIR_WOLF".to_string());

        Self {
            open: false,
            current_tab: EnvironmentTab::FloraScatter,
            selected_biome: BiomeType::GermanicForest,
            flora_seed: 1337,
            flora_density: 1.0,
            forest_coverage: 1.0,
            forest_patch_scale: 0.035,
            flora_jitter: 0.65,
            avoid_roads: true,
            flora_templates_text: DEFAULT_FLORA_TEMPLATES.to_string(),
            generated_flora: Vec::new(),
            lair_catalog: catalog,
            selected_lair_id: default_lair_id,
            lair_world_x: 4000.0,
            lair_world_z: 6000.0,
            lair_rotation_deg: 0.0,
            generated_lairs: Vec::new(),
            generated_lair_events: Vec::new(),
            lair_event_counts: Vec::new(),
            settlement_players: 2,
            settlement_tribe: SettlementTribe::Germanic,
            settlement_symmetry: SymmetryMode::RotationalSymmetry,
            settlement_seed: 42,
            settlement_include_nature: true,
            settlement_spawn_radius: 0.55,
            last_distribution: None,
            last_placement_preset: None,
            last_nature_preset: None,
            show_canvas_preview: true,
            regeneration_blocked: false,
            focus_base_request: None,
            apply_requested: None,
            lair_events_apply_requested: false,
            layout_rect: ["0".into(), "0".into(), "8".into(), "8".into()],
            layout_picker: None,
            layout_export_request: None,
            layout_import_requested: false,
            dirty: false,
            pending_discard: false,
            english: false,
            pending_clear_lairs: false,
            status_message: "Ready.".to_string(),
            status_is_error: false,
        }
    }
}

impl EnvironmentUi {
    pub fn is_dirty(&self) -> bool {
        self.dirty
            || !self.generated_flora.is_empty()
            || !self.generated_lairs.is_empty()
            || !self.generated_lair_events.is_empty()
            || self.last_distribution.is_some()
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn clear_dirty(&mut self) {
        self.reset_session();
    }

    /// Resets all map-bound generated flora, lairs, lair events, settlement layouts, and pending apply requests.
    /// Preserves user preferences (open state, current tab, biome, flora parameters/templates,
    /// lair selections, settlement options).
    pub fn reset_session(&mut self) {
        self.pending_clear_lairs = false;
        self.dirty = false;
        self.pending_discard = false;
        self.generated_flora.clear();
        self.generated_lairs.clear();
        self.generated_lair_events.clear();
        self.lair_event_counts.clear();
        self.last_distribution = None;
        self.last_placement_preset = None;
        self.last_nature_preset = None;
        self.focus_base_request = None;
        self.apply_requested = None;
        self.lair_events_apply_requested = false;
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

    fn parse_templates(text: &str) -> Result<(Vec<i32>, HashMap<i32, String>), String> {
        let mut ids = Vec::new();
        let mut names = HashMap::new();
        for (i, line) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
            let (id, name) = line
                .split_once(',')
                .ok_or_else(|| format!("Template line {} must be 'id,name'.", i + 1))?;
            let id: i32 = id
                .trim()
                .parse()
                .map_err(|_| format!("Template line {} has a non-numeric id.", i + 1))?;
            ids.push(id);
            names.insert(id, name.trim().to_string());
        }
        if ids.is_empty() {
            return Err("No object templates configured.".into());
        }
        Ok((ids, names))
    }

    /// Flora plan as DATA-session requests (engine rotation is radians; the store uses degrees).
    pub fn flora_requests(&self) -> Vec<PlacementRequest> {
        self.generated_flora
            .iter()
            .map(|f| PlacementRequest {
                type_id: f.type_id,
                name: f.name.clone(),
                x: f.x,
                y: f.y,
                z: f.z,
                rotation_deg: f.rotation.to_degrees(),
                team: None,
            })
            .collect()
    }

    /// Resolve observed neutral wildlife to existing map templates for the DATA session.
    /// The editor can clone a known object family; it cannot invent a missing game object type.
    pub fn lair_requests(
        &self,
        dimension: usize,
        heights: Option<&[u8]>,
        height_step: f32,
    ) -> Result<(Vec<PlacementRequest>, Vec<String>), String> {
        if self.generated_lairs.is_empty() {
            return Err("Add a neutral animal first.".into());
        }
        if self.generated_lairs.len() > crate::design_apply::MAX_OBJECTS_PER_BATCH {
            return Err(format!(
                "Apply at most {} neutral animals per undoable batch.",
                crate::design_apply::MAX_OBJECTS_PER_BATCH
            ));
        }
        let templates = crate::design_apply::parse_templates(&self.flora_templates_text)?;
        let resolve = resolver(&templates);
        let mut requests = Vec::new();
        let mut unresolved = std::collections::BTreeSet::new();
        for lair in &self.generated_lairs {
            if !lair.world_x.is_finite()
                || !lair.world_z.is_finite()
                || !lair.rotation_deg.is_finite()
                || !(0.0..16384.0).contains(&lair.world_x)
                || !(0.0..16384.0).contains(&lair.world_z)
            {
                return Err(format!(
                    "Neutral animal '{}' must have finite coordinates inside the map.",
                    lair.definition_id
                ));
            }
            let def = self
                .lair_catalog
                .get_by_id(&lair.definition_id)
                .ok_or_else(|| format!("Unknown lair definition '{}'.", lair.definition_id))?;
            match resolve(&def.native_building_or_landscape_type) {
                Some(type_id) => requests.push(PlacementRequest {
                    type_id,
                    name: def.native_building_or_landscape_type.clone(),
                    x: lair.world_x,
                    y: ground_height(heights, dimension, lair.world_x, lair.world_z, height_step),
                    z: lair.world_z,
                    rotation_deg: lair.rotation_deg,
                    team: Some(8),
                }),
                None => {
                    unresolved.insert(def.native_building_or_landscape_type.clone());
                }
            }
        }
        Ok((requests, unresolved.into_iter().collect()))
    }

    /// Settlement plan as requests; entries whose `type_name` is not in the template list are
    /// returned as unresolved (the editor has no ObjDef catalog to map names to type ids).
    /// `height_step` converts raw height bytes to world height.
    pub fn settlement_requests(
        &self,
        dimension: usize,
        heights: Option<&[u8]>,
        height_step: f32,
    ) -> Result<(Vec<PlacementRequest>, Vec<String>), String> {
        let dist = self.last_distribution.as_ref().ok_or("Generate a settlement first.")?;
        let templates = parse_templates(&self.flora_templates_text)?;
        let resolve = resolver(&templates);
        let ground = |x: f32, z: f32| ground_height(heights, dimension, x, z, height_step);
        let mut requests = Vec::new();
        let mut unresolved = std::collections::BTreeSet::new();
        let mut push = |name: &str, x: f32, z: f32, offset: f32, angle: f32, team: Option<i32>| {
            match resolve(name) {
                Some(type_id) => requests.push(PlacementRequest {
                    type_id,
                    name: name.to_string(),
                    x,
                    y: ground(x, z) + offset,
                    z,
                    rotation_deg: angle,
                    team: team.and_then(|t| u16::try_from(t).ok()),
                }),
                None => {
                    unresolved.insert(name.to_string());
                }
            }
        };
        for player in &dist.players {
            for b in &player.buildings {
                push(&b.type_name, b.world_x, b.world_z, b.height_offset, b.angle, Some(b.team));
            }
            for t in player.forest_trees.iter().chain(&player.stone_quarries) {
                push(&t.type_name, t.world_x, t.world_z, t.height_offset, t.angle_deg, None);
            }
            for w in &player.wildlife_and_food {
                push(&w.type_name, w.world_x, w.world_z, w.height_offset, w.angle_deg, Some(w.team));
            }
        }
        Ok((requests, unresolved.into_iter().collect()))
    }

    /// Resolve the entire generated plan against current-map cloneable definitions. No partial
    /// application: missing or ambiguous names are collected across both destinations.
    pub(crate) fn settlement_plan(
        &self,
        catalog: &[(i32, String)],
        dimension: usize,
        heights: Option<&[u8]>,
        collision: Option<(usize, &[u8])>,
        water: f32,
        step: f32,
    ) -> Result<(Vec<PlacementRequest>, Vec<NatureAddition>), String> {
        let terrain = SettlementTerrain::new(dimension, heights, collision, water, step)?;
        let dist = self
            .last_distribution
            .as_ref()
            .ok_or("Generate a settlement first")?;
        let mut placement = Vec::new();
        let mut nature = Vec::new();
        let mut unresolved = std::collections::BTreeSet::new();
        let mut push = |name: &str,
                        x: f32,
                        z: f32,
                        offset: f32,
                        angle: f32,
                        team: Option<i32>,
                        natural: bool| {
            let ids: std::collections::BTreeSet<_> = catalog
                .iter()
                .filter(|(_, n)| n.eq_ignore_ascii_case(name))
                .map(|(id, _)| *id)
                .collect();
            if ids.len() != 1 {
                unresolved.insert(name.to_owned());
                return;
            }
            let type_id = *ids.first().unwrap();
            let y = terrain.sample_height(x, z) + offset;
            if natural {
                nature.push(NatureAddition::new(
                    type_id,
                    name,
                    x,
                    y,
                    z,
                    angle.to_radians(),
                ));
            } else {
                placement.push(PlacementRequest {
                    type_id,
                    name: name.into(),
                    x,
                    y,
                    z,
                    rotation_deg: angle,
                    team: team.and_then(|t| u16::try_from(t).ok()),
                });
            }
        };
        for player in &dist.players {
            for b in &player.buildings {
                push(
                    &b.type_name,
                    b.world_x,
                    b.world_z,
                    b.height_offset,
                    b.angle,
                    Some(b.team),
                    false,
                );
            }
            for w in &player.wildlife_and_food {
                push(
                    &w.type_name,
                    w.world_x,
                    w.world_z,
                    w.height_offset,
                    w.angle_deg,
                    Some(w.team),
                    false,
                );
            }
            if self.settlement_include_nature {
                for t in player.forest_trees.iter().chain(&player.stone_quarries) {
                    push(
                        &t.type_name,
                        t.world_x,
                        t.world_z,
                        t.height_offset,
                        t.angle_deg,
                        None,
                        true,
                    );
                }
            }
        }
        if !unresolved.is_empty() {
            return Err(format!(
                "Settlement unresolved definitions/templates: {}",
                unresolved.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
        Ok((placement, nature))
    }

    /// Parses the nature-capture rectangle (C# `TerrainRegionPlanner.Rectangle` input).
    pub fn parse_layout_rect(&self) -> Result<TileRect, String> {
        let mut v = [0i32; 4];
        for (i, text) in self.layout_rect.iter().enumerate() {
            v[i] = text
                .trim()
                .parse::<i32>()
                .map_err(|_| format!("{} must be an integer tile index.", ["left", "top", "right", "bottom"][i]))?;
        }
        let rect = TileRect { left: v[0], top: v[1], right: v[2], bottom: v[3] };
        if rect.left < 0 || rect.top < 0 || rect.left >= rect.right || rect.top >= rect.bottom {
            return Err("Rectangle must satisfy 0 <= left < right and 0 <= top < bottom.".into());
        }
        Ok(rect)
    }

    /// Queues a forest/nature layout export of the current rectangle (executed by the app, which
    /// owns the object session and terrain).
    pub fn request_nature_layout_export(&mut self) {
        match self.parse_layout_rect() {
            Ok(rect) => {
                self.layout_export_request = Some(rect);
                self.status_message = "Nature layout export queued.".into();
                self.status_is_error = false;
            }
            Err(e) => {
                self.status_message = e;
                self.status_is_error = true;
            }
        }
    }
    pub fn take_layout_export_request(&mut self) -> Option<TileRect> {
        self.layout_export_request.take()
    }
    pub fn request_layout_import(&mut self) {
        self.layout_import_requested = true;
    }
    pub fn take_layout_import_request(&mut self) -> bool {
        std::mem::take(&mut self.layout_import_requested)
    }

    /// Saves the last generated settlement preset (placement or nature) as `.arm-layout.json`.
    pub fn export_generated_preset(&mut self, nature: bool) -> Result<Option<std::path::PathBuf>, String> {
        let preset = if nature { &self.last_nature_preset } else { &self.last_placement_preset }
            .clone()
            .ok_or("Generate a settlement first.")?;
        let kind = LayoutDialogKind::Save { suggested_name: layout_io::suggested_file_name(preset.kind) };
        let picked = match &self.layout_picker {
            Some(picker) => picker(&kind),
            None => layout_io::native_layout_picker()(&kind),
        };
        let Some(path) = picked else { return Ok(None) };
        let written = layout_io::write_layout_file(&path, &preset)?;
        Ok(Some(written))
    }

    pub fn request_lair_events_apply(&mut self) {
        self.lair_events_apply_requested = true;
    }

    pub fn take_lair_events_apply_request(&mut self) -> bool {
        std::mem::take(&mut self.lair_events_apply_requested)
    }

    /// Merges the pending lair events into the live scenario through `ScenarioUi::add_events`
    /// (validation, 256-event cap, one undo step). Requires a loaded scenario with a clean graph.
    /// Only on success are the pending events cleared; every failure keeps all pending state.
    pub fn apply_lair_events(&mut self, scenario: &mut ScenarioUi) -> Result<usize, String> {
        let result = self.try_apply_lair_events(scenario);
        match &result {
            Ok(n) => {
                self.status_message = format!(
                    "Applied {n} lair events to the scenario (total: {}).",
                    scenario.document().events.len()
                );
                self.status_is_error = false;
            }
            Err(e) => {
                self.status_message = format!("Lair events not applied: {e}");
                self.status_is_error = true;
            }
        }
        result
    }

    fn try_apply_lair_events(&mut self, scenario: &mut ScenarioUi) -> Result<usize, String> {
        if self.generated_lair_events.is_empty() {
            return Err("No lair events pending.".to_string());
        }
        if !scenario.is_loaded() {
            return Err("Load the scenario (map script) first.".to_string());
        }
        if scenario.graph_ui().is_dirty() {
            return Err("Apply or discard the pending event graph edits first.".to_string());
        }
        let pending = self.generated_lair_events.len();
        let existing = scenario.document().events.len();
        if existing + pending > SCENARIO_EVENT_LIMIT {
            return Err(format!(
                "Scenario event limit exceeded ({existing} existing + {pending} pending > {SCENARIO_EVENT_LIMIT})."
            ));
        }
        let events: Vec<_> = self.generated_lair_events.iter().map(to_scenario_event).collect();
        scenario.add_events(&events)?;
        // Counts stay parallel to `generated_lairs`; the lairs themselves remain pending.
        self.generated_lair_events.clear();
        self.lair_event_counts.iter_mut().for_each(|c| *c = 0);
        Ok(events.len())
    }

    pub fn take_apply_request(&mut self) -> Option<EnvApply> {
        self.apply_requested.take()
    }

    /// The app queued the plan into the DATA session: the plan now lives in that session's
    /// dirty/undo state, so the panel's own copy is cleared.
    pub fn plan_queued(&mut self, kind: EnvApply, count: usize, note: &str) {
        let discarded_lair_events =
            kind == EnvApply::Lairs && !self.generated_lair_events.is_empty();
        match kind {
            EnvApply::Flora => self.generated_flora.clear(),
            EnvApply::Lairs => {
                self.generated_lairs.clear();
                self.generated_lair_events.clear();
                self.lair_event_counts.clear();
            }
            EnvApply::Settlement => {
                self.last_distribution = None;
                self.last_placement_preset = None;
                self.last_nature_preset = None;
            }
        }
        self.pending_clear_lairs = false;
        self.dirty = false;
        self.status_message = format!(
            "Queued {count} objects for {}.{note}{}",
            if kind == EnvApply::Settlement { "Placement / Nature" } else { "the Objects session" },
            if discarded_lair_events {
                " Lair event drafts were cleared with their placement records."
            } else {
                ""
            }
        );
        self.status_is_error = false;
    }

    pub fn apply_failed(&mut self, message: impl Into<String>) {
        self.status_message = message.into();
        self.status_is_error = true;
    }

    /// `dimension` is the height-grid vertex count; `heights` is the matching raw u8 grid.
    pub fn generate_flora_scatter(&mut self, dimension: usize, heights: Option<&[u8]>, water_level: f32) {
        let (ids, names) = match Self::parse_templates(&self.flora_templates_text) {
            Ok(t) => t,
            Err(e) => {
                self.status_message = e;
                self.status_is_error = true;
                return;
            }
        };
        let tiles = dimension.saturating_sub(1) as i32;
        let heights = heights.filter(|h| h.len() == dimension * dimension);
        let context = FloraScatterContext {
            dimension: tiles,
            vertex_size: dimension as i32,
            heights: heights.map(|h| h.to_vec()),
            water_level: water_level.clamp(0.0, 255.0) as u8,
            profile: BiomeEcologyProfile::get_preset(self.selected_biome),
            available_templates: ids,
            objdef_names: names,
            ..Default::default()
        };
        let params = FloraScatterParameters {
            seed: self.flora_seed,
            density_multiplier: self.flora_density,
            forest_coverage: self.forest_coverage,
            forest_patch_scale: self.forest_patch_scale,
            min_tile_x: 0,
            min_tile_y: 0,
            max_tile_x: (tiles - 1).max(0),
            max_tile_y: (tiles - 1).max(0),
            jitter_amount: self.flora_jitter,
            avoid_roads_and_buildings: self.avoid_roads,
            ..Default::default()
        };

        let result = FloraScatterEngine::generate(&context, &params);
        self.generated_flora = result.additions;
        self.dirty = true;
        self.status_message = format!(
            "Generated {} flora objects for {:?} ecology (canopy {}, understory {}, ground {}, rocks {}).",
            result.report.total_planted,
            self.selected_biome,
            result.report.canopy_count,
            result.report.understory_count,
            result.report.ground_flora_count,
            result.report.rock_count,
        );
        self.status_is_error = false;
    }

    pub fn add_neutral_lair(&mut self) -> Result<(), String> {
        let def = self
            .lair_catalog
            .get_by_id(&self.selected_lair_id)
            .ok_or_else(|| format!("Unknown lair definition '{}'", self.selected_lair_id))?
            .clone();

        let mut placed = PlacedNeutralLair::new(
            uuid::Uuid::new_v4(),
            def.id.clone(),
            self.lair_world_x,
            0.0,
            self.lair_world_z,
        );
        placed.rotation_deg = self.lair_rotation_deg;

        // The binder is deliberately strict (e.g. FigTie animals cannot be spawned by events);
        // a rejection keeps the placed lair but reports why no scenario events were produced.
        let aliases = vec![(
            def.script_alias.clone().unwrap_or_else(|| def.id.clone()),
            def.native_building_or_landscape_type.clone(),
        )];
        let (events, note) = match WildLairScenarioEventBinder::generate_events_for_lair(
            &placed, &def, 0, LAIR_TILE_WORLD_SIZE, &aliases,
        ) {
            Ok(events) => (events, String::new()),
            Err(e) => (Vec::new(), format!(" No scenario events: {e}")),
        };

        self.generated_lairs.push(placed);
        self.lair_event_counts.push(events.len());
        self.generated_lair_events.extend(events);
        self.dirty = true;
        self.status_message = format!(
            "Added neutral lair '{}'; {} scenario events total.{}",
            def.display_name_en,
            self.generated_lair_events.len(),
            note
        );
        self.status_is_error = false;
        Ok(())
    }

    /// Removes one placed lair together with the scenario events it generated.
    pub fn remove_lair(&mut self, index: usize) -> bool {
        if index >= self.generated_lairs.len() || self.lair_event_counts.len() != self.generated_lairs.len() {
            return false;
        }
        let start: usize = self.lair_event_counts[..index].iter().sum();
        let end = (start + self.lair_event_counts[index]).min(self.generated_lair_events.len());
        self.generated_lair_events.drain(start.min(end)..end);
        self.lair_event_counts.remove(index);
        self.generated_lairs.remove(index);
        self.status_message = format!("Removed lair; {} remaining.", self.generated_lairs.len());
        self.status_is_error = false;
        true
    }

    fn request_clear_lairs(&mut self) {
        self.pending_clear_lairs = !self.generated_lairs.is_empty()
            || !self.generated_lair_events.is_empty() || !self.lair_event_counts.is_empty();
    }

    fn resolve_clear_lairs(&mut self, confirm: bool) {
        if self.pending_clear_lairs && confirm { self.clear_lairs(); }
        self.pending_clear_lairs = false;
    }

    pub fn clear_lairs(&mut self) {
        self.generated_lairs.clear();
        self.generated_lair_events.clear();
        self.lair_event_counts.clear();
        self.status_message = "Cleared neutral lairs.".to_string();
        self.status_is_error = false;
    }

    /// `dimension` is the height-grid vertex count. Water is in raw height units;
    /// collision must be the current session's cell grid. Missing sources fail closed.
    pub fn generate_settlement(
        &mut self,
        dimension: usize,
        heights: Option<&[u8]>,
        collision: Option<(usize, &[u8])>,
        water_level: f32,
        height_step: f32,
    ) -> Result<(), String> {
        let terrain =
            SettlementTerrain::new(dimension, heights, collision, water_level, height_step)?;
        let tiles = (dimension - 1) as i32;
        let sample_height = |x, z| terrain.sample_height(x, z);
        let is_blocked = |x, z| terrain.is_blocked(x, z);

        let evaluator = SettlementSiteEvaluator::new(
            tiles,
            &sample_height,
            &is_blocked,
            terrain.water_world,
            SETTLEMENT_TILE_WORLD_SIZE,
        );
        let planner = ResourceClusterPlanner::new(
            tiles,
            &sample_height,
            &is_blocked,
            terrain.water_world,
            SETTLEMENT_TILE_WORLD_SIZE,
        );
        let balancer = MultiplayerFairnessBalancer::new(
            &evaluator,
            &planner,
            tiles,
            SETTLEMENT_TILE_WORLD_SIZE,
        );
        let tribes = vec![self.settlement_tribe; self.settlement_players];

        let result = balancer.generate(
            self.settlement_players,
            self.settlement_symmetry,
            Some(&tribes),
            self.settlement_seed,
            self.settlement_spawn_radius,
        )?;

        let placement_preset = SettlementGeneratorEngine::to_placement_layout_preset(&result);
        let nature_preset = self
            .settlement_include_nature
            .then(|| SettlementGeneratorEngine::to_nature_layout_preset(&result));

        self.status_message = format!(
            "Settlement generation complete: {} player bases, {}.",
            result.players.len(),
            if result.fairness_report.is_balanced {
                "balanced"
            } else {
                "NOT balanced"
            }
        );
        self.last_distribution = Some(result);
        self.last_placement_preset = Some(placement_preset);
        self.last_nature_preset = nature_preset;
        self.dirty = true;
        self.status_is_error = false;
        Ok(())
    }

    /// Returns per-base breakdown of buildings, wood (trees), stone quarries, and wildlife/food.
    pub fn base_resource_summaries(&self) -> Vec<BaseResourceSummary> {
        let Some(dist) = &self.last_distribution else {
            return Vec::new();
        };
        dist.players
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let wx = p.site_evaluation.world_x.round() as i32;
                let wz = p.site_evaluation.world_z.round() as i32;
                let food_count = p.wildlife_and_food.len();
                BaseResourceSummary {
                    base_index: i + 1,
                    team: p.team_index,
                    tribe: p.tribe,
                    world_x: wx,
                    world_z: wz,
                    building_count: p.buildings.len(),
                    wood_count: if self.settlement_include_nature { p.forest_trees.len() } else { 0 },
                    stone_count: if self.settlement_include_nature { p.stone_quarries.len() } else { 0 },
                    food_count,
                }
            })
            .collect()
    }

    /// Consumes any pending request to center the camera on a specific base position.
    pub fn take_focus_base_request(&mut self) -> Option<(f32, f32)> {
        self.focus_base_request.take()
    }

    pub fn has_modal(&self) -> bool {
        self.pending_discard || self.pending_clear_lairs
    }

    /// Advances the settlement seed randomly and immediately generates a new settlement layout.
    pub fn regenerate_settlement_with_new_seed(
        &mut self,
        dimension: usize,
        heights: Option<&[u8]>,
        collision: Option<(usize, &[u8])>,
        water_level: f32,
        height_step: f32,
    ) -> Result<i32, String> {
        if self.regeneration_blocked || self.has_modal() {
            return Err(tl(self.english, "請先關閉對話框或結束文字輸入。", "Close dialogs or finish text input first.").into());
        }
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.subsec_nanos());
        let new_seed = ((self.settlement_seed as u32).wrapping_mul(1103515245).wrapping_add(12345 ^ nanos) % 99_999) as i32 + 1;
        let new_seed = if new_seed == self.settlement_seed { new_seed % 99_999 + 1 } else { new_seed };
        let previous_seed = self.settlement_seed;
        self.settlement_seed = new_seed;
        if let Err(error) = self.generate_settlement(dimension, heights, collision, water_level, height_step) {
            self.settlement_seed = previous_seed;
            return Err(error);
        }
        Ok(new_seed)
    }

    /// Renders a non-destructive 2D canvas overlay of the pending settlement distribution
    /// (town centers, buildings, wood/stone resources, and wildlife) before "Apply" commits them.
    pub fn paint_canvas_preview(&self, painter: &egui::Painter, rect: Rect, english: bool) {
        if !self.show_canvas_preview || !painter.clip_rect().intersects(rect) {
            return;
        }
        let Some(dist) = &self.last_distribution else {
            return;
        };

        for (base_idx, player) in dist.players.iter().enumerate() {
            let color = crate::spawn_placement_ui::team_color(player.team_index);
            let base_x = player.site_evaluation.world_x;
            let base_z = player.site_evaluation.world_z;
            if !base_x.is_finite() || !base_z.is_finite() {
                continue;
            }
            let base_pos = crate::scenario_canvas::screen_from_world(rect, base_x, base_z);

            // Base Town Center marker: double circle with team color
            let base_ring_color = color.linear_multiply(0.8);
            painter.circle_filled(base_pos, 8.0, color);
            painter.circle_stroke(base_pos, 13.0, Stroke::new(2.0_f32, Color32::WHITE));
            painter.circle_stroke(base_pos, 15.0, Stroke::new(1.5_f32, base_ring_color));

            // Base label
            let base_label = if english {
                format!("Base {} (T{})", base_idx + 1, player.team_index)
            } else {
                format!("基地 {} (隊伍{})", base_idx + 1, player.team_index)
            };
            painter.text(
                base_pos + egui::vec2(16.0, -8.0),
                egui::Align2::LEFT_CENTER,
                base_label,
                egui::FontId::proportional(12.0),
                Color32::WHITE,
            );

            // Buildings: small colored squares
            for b in &player.buildings {
                if !b.world_x.is_finite() || !b.world_z.is_finite() {
                    continue;
                }
                let bp = crate::scenario_canvas::screen_from_world(rect, b.world_x, b.world_z);
                let b_rect = Rect::from_center_size(bp, egui::vec2(6.0, 6.0));
                painter.rect_filled(b_rect, 1.0, color);
                painter.rect_stroke(b_rect, 1.0, Stroke::new(1.0_f32, Color32::WHITE));
            }

            // Wildlife & Food: small amber dots
            for w in &player.wildlife_and_food {
                if !w.world_x.is_finite() || !w.world_z.is_finite() {
                    continue;
                }
                let wp = crate::scenario_canvas::screen_from_world(rect, w.world_x, w.world_z);
                painter.circle_filled(wp, 3.0, Color32::from_rgb(255, 200, 50));
            }

            // Nature trees and stone (if included)
            if self.settlement_include_nature {
                for t in &player.forest_trees {
                    if !t.world_x.is_finite() || !t.world_z.is_finite() {
                        continue;
                    }
                    let tp = crate::scenario_canvas::screen_from_world(rect, t.world_x, t.world_z);
                    painter.circle_filled(tp, 2.5, Color32::from_rgb(80, 200, 100));
                }
                for s in &player.stone_quarries {
                    if !s.world_x.is_finite() || !s.world_z.is_finite() {
                        continue;
                    }
                    let sp = crate::scenario_canvas::screen_from_world(rect, s.world_x, s.world_z);
                    painter.circle_filled(sp, 3.0, Color32::from_rgb(180, 180, 195));
                }
            }
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        dimension: usize,
        heights: Option<&[u8]>,
        collision: Option<(usize, &[u8])>,
        water_level: f32,
        height_step: f32,
    ) {
        if !self.open {
            return;
        }

        let mut is_open = self.open;
        egui::Window::new(tl(self.english, "環境與聚落工作室", "Environment & Settlement Studio"))
            .open(&mut is_open)
            .min_size(Vec2::new(740.0, 540.0))
            .show(ctx, |ui| {
                ui.set_enabled(!self.has_modal());
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.current_tab, EnvironmentTab::FloraScatter, tl(self.english, "植物散布", "Flora Scatter"));
                    ui.selectable_value(&mut self.current_tab, EnvironmentTab::NeutralLairs, tl(self.english, "中立巢穴", "Neutral Lairs"));
                    ui.selectable_value(
                        &mut self.current_tab,
                        EnvironmentTab::SettlementGenerator,
                        tl(self.english, "聚落生成器", "Settlement Generator"),
                    );
                    ui.selectable_value(&mut self.current_tab, EnvironmentTab::Layouts, tl(self.english, "配置範本 (.arm-layout.json)", "Layouts (.arm-layout.json)"));
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
                    EnvironmentTab::FloraScatter => self.render_flora_tab(ui, dimension, heights, water_level),
                    EnvironmentTab::NeutralLairs => self.render_lairs_tab(ui),
                    EnvironmentTab::SettlementGenerator => {
                        self.render_settlement_tab(ui, dimension, heights, collision, water_level, height_step)
                    }
                    EnvironmentTab::Layouts => self.render_layouts_tab(ui),
                }
            });

        if !is_open {
            self.request_close();
        }

        if self.pending_clear_lairs {
            let mut answer = None;
            egui::Window::new(tl(self.english, "清除所有巢穴？", "Clear All Lairs?"))
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false).resizable(false)
                .show(ctx, |ui| {
                    ui.label(tl(self.english, "將清除所有巢穴與事件草稿。", "Clear all lairs and event drafts."));
                    ui.horizontal(|ui| {
                        if ui.button(tl(self.english, "清除", "Clear")).clicked() { answer = Some(true); }
                        if ui.button(tl(self.english, "取消", "Cancel")).clicked() { answer = Some(false); }
                    });
                });
            if let Some(confirm) = answer { self.resolve_clear_lairs(confirm); }
        }
        if self.pending_discard {
            let mut close_modal = false;
            let mut confirm = false;

            egui::Window::new(discard_labels(self.english)[0])
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(discard_labels(self.english)[1]);
                    ui.horizontal(|ui| {
                        if ui.button(discard_labels(self.english)[2]).clicked() {
                            confirm = true;
                        }
                        if ui.button(discard_labels(self.english)[3]).clicked() {
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

    fn render_flora_tab(&mut self, ui: &mut egui::Ui, dimension: usize, heights: Option<&[u8]>, water_level: f32) {
        ui.heading(tl(self.english, "植物與生態自動散布", "Flora & Ecology Auto-Scatter"));
        ui.label(tl(
            self.english,
            "依據 Perlin 水文與生物群落設定檔自動生成有機植被與岩石結構。",
            "Generates organic nature foliage and rock formations based on Perlin hydrology and biome profiles.",
        ));

        ui.add_space(6.0);
        egui::Grid::new("flora_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(tl(self.english, "生物群落生態：", "Biome Ecology:"));
            let biome_label = match (self.selected_biome, self.english) {
                (BiomeType::GermanicForest, false) => "日耳曼森林",
                (BiomeType::GermanicForest, true) => "Germanic Forest",
                (BiomeType::DanubeBroadleaf, false) => "多瑙河闊葉林",
                (BiomeType::DanubeBroadleaf, true) => "Danube Broadleaf",
                (BiomeType::ItalianHills, false) => "義大利丘陵",
                (BiomeType::ItalianHills, true) => "Italian Hills",
                (BiomeType::BritishMarsh, false) => "不列顛沼澤",
                (BiomeType::BritishMarsh, true) => "British Marsh",
            };
            egui::ComboBox::from_id_source("biome_select")
                .selected_text(biome_label)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.selected_biome, BiomeType::GermanicForest, tl(self.english, "日耳曼森林", "Germanic Forest"));
                    ui.selectable_value(&mut self.selected_biome, BiomeType::DanubeBroadleaf, tl(self.english, "多瑙河闊葉林", "Danube Broadleaf"));
                    ui.selectable_value(&mut self.selected_biome, BiomeType::ItalianHills, tl(self.english, "義大利丘陵", "Italian Hills"));
                    ui.selectable_value(&mut self.selected_biome, BiomeType::BritishMarsh, tl(self.english, "不列顛沼澤", "British Marsh"));
                });
            ui.end_row();

            ui.label(tl(self.english, "隨機種子碼：", "Random Seed:"));
            ui.add(egui::DragValue::new(&mut self.flora_seed));
            ui.end_row();

            ui.label(tl(self.english, "密度倍率：", "Density Multiplier:"));
            ui.add(egui::Slider::new(&mut self.flora_density, 0.1..=3.0).text("x"));
            ui.end_row();

            ui.label(tl(self.english, "森林覆蓋率：", "Forest Coverage:"));
            ui.add(egui::Slider::new(&mut self.forest_coverage, 0.1..=2.0).text("ratio"));
            ui.end_row();

            ui.label(tl(self.english, "林區區塊尺度：", "Forest Patch Scale:"));
            ui.add(egui::Slider::new(&mut self.forest_patch_scale, 0.01..=0.1));
            ui.end_row();

            ui.label(tl(self.english, "抖動散布量：", "Jitter Amount:"));
            ui.add(egui::Slider::new(&mut self.flora_jitter, 0.0..=1.0));
            ui.end_row();

            ui.label(tl(self.english, "避開道路與建築：", "Avoid Roads & Buildings:"));
            ui.checkbox(&mut self.avoid_roads, "");
            ui.end_row();

            ui.label(tl(self.english, "物件範本 (id,name)：", "Object templates (id,name):"));
            ui.add(egui::TextEdit::multiline(&mut self.flora_templates_text).desired_rows(4));
            ui.end_row();
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button(tl(self.english, "產生植物散布", "Generate Flora Scatter")).clicked() {
                self.generate_flora_scatter(dimension, heights, water_level);
            }
            if ui.button(tl(self.english, "清除已產生的植物", "Clear Generated Flora")).clicked() {
                self.generated_flora.clear();
                self.status_message = tl(self.english, "已清除產生的植物。", "Cleared generated flora.").to_string();
                self.status_is_error = false;
            }
            if ui
                .add_enabled(!self.generated_flora.is_empty(), egui::Button::new(tl(self.english, "套用至物件工作階段", "Apply to Objects session")))
                .on_hover_text(tl(
                    self.english,
                    "以各範本類型複製地圖既有物件；請先在物件面板載入 DATA。",
                    "Clones existing map objects of each template type; load DATA in the Objects panel first.",
                ))
                .clicked()
            {
                self.apply_requested = Some(EnvApply::Flora);
            }
        });

        ui.separator();
        ui.label(format!("{}: {}", tl(self.english, "已產生植物數量", "Generated Flora Count"), self.generated_flora.len()));
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            for (i, item) in self.generated_flora.iter().take(50).enumerate() {
                ui.label(format!("{}. {} ({} {:.1}, {:.1}, {:.1})", i + 1, item.name, tl(self.english, "位置：", "Pos:"), item.x, item.y, item.z));
            }
            if self.generated_flora.len() > 50 {
                ui.label(format!(
                    "{} {} {}",
                    tl(self.english, "... 以及另外", "... and"),
                    self.generated_flora.len() - 50,
                    tl(self.english, "個植物物件。", "more flora objects.")
                ));
            }
        });
    }

    fn render_lairs_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading(tl(self.english, "中立野生動物與巢穴配置", "Neutral Wildlife & Lair Setup"));
        ui.label(tl(
            self.english,
            "目前目錄包含已觀察到的中立動物類型。請在植物分頁新增相應的 id,name 範本，然後複製至物件工作階段。未驗證的巢穴建築與生成規則不作推測。",
            "The current catalog contains observed neutral animal types. Add matching id,name templates in the Flora tab, then clone them into the Objects session. Unverified lair buildings and spawn rules are not inferred.",
        ));

        ui.add_space(6.0);
        egui::Grid::new("lair_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(tl(self.english, "巢穴定義：", "Lair Definition:"));
            egui::ComboBox::from_id_source("lair_def_select")
                .selected_text(&self.selected_lair_id)
                .show_ui(ui, |ui| {
                    for def in self.lair_catalog.all_definitions() {
                        ui.selectable_value(
                            &mut self.selected_lair_id,
                            def.id.clone(),
                            format!("{} ({})", def.display_name_en, def.id),
                        );
                    }
                });
            ui.end_row();

            ui.label(tl(self.english, "世界座標 X / Z：", "World Location X / Z:"));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.lair_world_x).clamp_range(0.0..=16383.0).prefix("X: "));
                ui.add(egui::DragValue::new(&mut self.lair_world_z).clamp_range(0.0..=16383.0).prefix("Z: "));
            });
            ui.end_row();

            ui.label(tl(self.english, "旋轉角度（度）：", "Rotation (deg):"));
            ui.add(egui::DragValue::new(&mut self.lair_rotation_deg).clamp_range(0.0..=359.0));
            ui.end_row();
        });

        ui.add_space(8.0);
        if ui.button(tl(self.english, "新增中立巢穴", "Add Neutral Lair")).clicked() {
            if let Err(e) = self.add_neutral_lair() {
                self.status_message = format!("Failed to add neutral lair: {e}");
                self.status_is_error = true;
            }
        }

        ui.separator();
        if ui
            .add_enabled(
                !self.generated_lairs.is_empty(),
                egui::Button::new(tl(self.english, "套用動物至物件工作階段", "Apply Animals to Objects session")),
            )
            .on_hover_text(tl(
                self.english,
                "需要每種動物類型在載入的地圖中以可複製範本形式存在。",
                "Requires each animal type to exist in the loaded map as a cloneable template.",
            ))
            .clicked()
        {
            self.apply_requested = Some(EnvApply::Lairs);
        }
        if ui
            .add_enabled(
                !self.generated_lair_events.is_empty(),
                egui::Button::new(tl(self.english, "套用巢穴事件", "Apply Lair Events")),
            )
            .clicked()
        {
            self.request_lair_events_apply();
        }
        ui.label(format!(
            "{}: {} ({}: {})",
            tl(self.english, "已放置巢穴", "Placed Lairs"),
            self.generated_lairs.len(),
            tl(self.english, "劇本事件", "Scenario Events"),
            self.generated_lair_events.len()
        ));
        if ui
            .add_enabled(!self.generated_lairs.is_empty() || !self.generated_lair_events.is_empty() || !self.lair_event_counts.is_empty(), egui::Button::new(tl(self.english, "清除所有巢穴", "Clear Lairs")))
            .clicked()
        {
            self.request_clear_lairs();
        }
        let mut remove = None;
        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
            for (i, lair) in self.generated_lairs.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{}. {} {} ({:.1}, {:.1}) {} {:.0}, {} {}",
                        i + 1,
                        lair.definition_id,
                        tl(self.english, "位於", "at"),
                        lair.world_x,
                        lair.world_z,
                        tl(self.english, "旋轉", "rot"),
                        lair.rotation_deg,
                        self.lair_event_counts.get(i).copied().unwrap_or(0),
                        tl(self.english, "個事件", "events")
                    ));
                    if ui.small_button(tl(self.english, "移除", "Remove")).clicked() {
                        remove = Some(i);
                    }
                });
            }
        });
        if let Some(i) = remove {
            self.remove_lair(i);
        }
        if !self.generated_lair_events.is_empty() {
            ui.separator();
            ui.label(tl(self.english, "劇本事件預覽：", "Scenario event preview:"));
            egui::ScrollArea::vertical().id_source("lair_event_preview").max_height(120.0).show(ui, |ui| {
                for e in &self.generated_lair_events {
                    ui.label(format!(
                        "{} ({} {} {}, {} {}, {} {}, {} {})",
                        e.name, tl(self.english, "延遲", "delay"), e.delay_seconds, tl(self.english, "秒", "s"),
                        tl(self.english, "重複", "repeat"), e.repeat, e.conditions.len(), tl(self.english, "個條件", "conditions"),
                        e.actions.len(), tl(self.english, "個動作", "actions")
                    ));
                }
            });
        }
    }

    fn render_settlement_tab(
        &mut self,
        ui: &mut egui::Ui,
        dimension: usize,
        heights: Option<&[u8]>,
        collision: Option<(usize, &[u8])>,
        water_level: f32,
        height_step: f32,
    ) {
        ui.heading(tl(self.english, "一鍵聚落 / 基地生成器", "One-Click Settlement / Base Generator"));
        ui.label(tl(
            self.english,
            "即時平衡起始市中心、兵營、資源堆與周圍森林／採石場。",
            "Instantly balances starting town centers, barracks, resource depots, and surrounding forests/stone.",
        ));

        ui.add_space(6.0);
        egui::Grid::new("settlement_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(tl(self.english, "玩家人數 (2..8)：", "Player Count (2..8):"));
            ui.add(egui::Slider::new(&mut self.settlement_players, 2..=8));
            ui.end_row();

            ui.label(tl(self.english, "部族建築風格：", "Tribe Architecture:"));
            egui::ComboBox::from_id_source("settlement_tribe_select")
                .selected_text(settlement_tribe_label(self.english, self.settlement_tribe))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.settlement_tribe, SettlementTribe::Germanic, tl(self.english, "日耳曼", "Germanic"));
                    ui.selectable_value(&mut self.settlement_tribe, SettlementTribe::Roman, tl(self.english, "羅馬", "Roman"));
                    ui.selectable_value(&mut self.settlement_tribe, SettlementTribe::Celtic, tl(self.english, "塞爾特", "Celtic"));
                    ui.selectable_value(&mut self.settlement_tribe, SettlementTribe::Hun, tl(self.english, "匈人", "Hun"));
                });
            ui.end_row();

            ui.label(tl(self.english, "配置對稱性：", "Layout Symmetry:"));
            egui::ComboBox::from_id_source("settlement_symmetry_select")
                .selected_text(settlement_symmetry_label(self.english, self.settlement_symmetry))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.settlement_symmetry, SymmetryMode::RotationalSymmetry, settlement_symmetry_label(self.english, SymmetryMode::RotationalSymmetry));
                    ui.selectable_value(&mut self.settlement_symmetry, SymmetryMode::CentralSymmetry, tl(self.english, "中心點對稱", "Central"));
                    ui.selectable_value(&mut self.settlement_symmetry, SymmetryMode::TopologicalEquidistant, tl(self.english, "拓撲等距", "Topological equidistant"));
                });
            ui.end_row();

            ui.label(tl(self.english, "種子碼：", "Seed:"));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.settlement_seed));
                if ui.button(tl(self.english, "隨機抽樣", "Randomize")).clicked() {
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(1, |d| d.subsec_nanos());
                    self.settlement_seed = (nanos % 99_999) as i32 + 1;
                }
            });
            ui.end_row();

            ui.label(tl(self.english, "包含自然物件：", "Include Nature Objects:"));
            ui.checkbox(&mut self.settlement_include_nature, tl(self.english, "產生森林與採石場露頭", "Generate Forests & Stone Quarries"));
            ui.end_row();

            ui.label(tl(self.english, "生成半徑比例：", "Spawn Radius Ratio:"));
            ui.add(egui::Slider::new(&mut self.settlement_spawn_radius, 0.4..=0.75));
            ui.end_row();
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button(tl(self.english, "產生聚落基地", "Generate Settlement Bases")).clicked() {
                if let Err(e) = self.generate_settlement(dimension, heights, collision, water_level, height_step) {
                    self.status_message = format!("Generation error: {e}");
                    self.status_is_error = true;
                }
            }
            if ui
                .button(tl(
                    self.english,
                    "🎲 使用新種子碼重新產生 (Ctrl+R / Alt+R)",
                    "🎲 Regenerate with New Seed (Ctrl+R / Alt+R)",
                ))
                .on_hover_text(tl(
                    self.english,
                    "隨機更新種子碼並即時重新生成聚落配置",
                    "Advance seed randomly and regenerate settlement layout immediately",
                ))
                .clicked()
            {
                match self.regenerate_settlement_with_new_seed(
                    dimension,
                    heights,
                    collision,
                    water_level,
                    height_step,
                ) {
                    Ok(seed) => {
                        self.status_message = format!(
                            "{}: {seed}",
                            tl(self.english, "已使用新種子碼重新產生聚落", "Regenerated settlement with new seed")
                        );
                        self.status_is_error = false;
                    }
                    Err(e) => {
                        self.status_message = format!("Generation error: {e}");
                        self.status_is_error = true;
                    }
                }
            }
        });

        if self.last_distribution.is_some()
            && ui
                .button(tl(self.english, "套用至物件工作階段", "Apply to Objects session"))
                .on_hover_text(tl(
                    self.english,
                    "範本名稱取自植物分頁之 id,name 清單",
                    "Template names come from the Flora tab's id,name list.",
                ))
                .clicked()
        {
            self.apply_requested = Some(EnvApply::Settlement);
        }

        if let Some(dist) = &self.last_distribution {
            ui.separator();
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut self.show_canvas_preview,
                    tl(self.english, "在 2D 畫布預覽配置", "Preview on 2D canvas"),
                );
                ui.label(tl(
                    self.english,
                    "(顯示基地核心、建築與資源露頭)",
                    "(displays bases, buildings & resource clusters)",
                ));
            });

            let f = &dist.fairness_report;
            ui.colored_label(
                if f.is_balanced { Color32::LIGHT_GREEN } else { Color32::from_rgb(250, 180, 80) },
                format!(
                    "{}: {} {}, {}",
                    tl(self.english, "生成結果", "Generation Result"),
                    dist.players.len(),
                    tl(self.english, "個基地", "bases"),
                    if f.is_balanced {
                        tl(self.english, "平衡", "balanced")
                    } else {
                        tl(self.english, "不平衡", "NOT balanced")
                    }
                ),
            );
            ui.label(format!(
                "{}: {:.1}/{:.1}/{:.1}; {} {:.2} {} {:.2} {} {:.2}",
                tl(self.english, "玩家間距 平均/最小/最大", "Inter-player distance avg/min/max"),
                f.average_inter_player_distance,
                f.min_inter_player_distance,
                f.max_inter_player_distance,
                tl(self.english, "木材變異", "resource variance wood"),
                f.wood_resource_variance,
                tl(self.english, "石材", "stone"),
                f.stone_resource_variance,
                tl(self.english, "食物", "food"),
                f.food_resource_variance,
            ));

            ui.add_space(4.0);
            ui.label(tl(
                self.english,
                "各勢力基地資源分配明細：",
                "Per-Base Resource Allotment Breakdown:",
            ));

            let summaries = self.base_resource_summaries();
            egui::Grid::new("base_resource_breakdown_grid")
                .striped(true)
                .min_col_width(45.0)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.strong(tl(self.english, "基地", "Base"));
                    ui.strong(tl(self.english, "隊伍", "Team"));
                    ui.strong(tl(self.english, "部族", "Tribe"));
                    ui.strong(tl(self.english, "座標 (X, Z)", "Pos (X, Z)"));
                    ui.strong(tl(self.english, "建築", "Buildings"));
                    ui.strong(tl(self.english, "木材", "Wood"));
                    ui.strong(tl(self.english, "石材", "Stone"));
                    ui.strong(tl(self.english, "食物", "Food"));
                    ui.strong(tl(self.english, "操作", "Action"));
                    ui.end_row();

                    for s in summaries {
                        let tribe_str = match s.tribe {
                            SettlementTribe::Germanic => tl(self.english, "日耳曼", "Germanic"),
                            SettlementTribe::Roman => tl(self.english, "羅馬", "Roman"),
                            SettlementTribe::Celtic => tl(self.english, "塞爾特", "Celtic"),
                            SettlementTribe::Hun => tl(self.english, "匈人", "Hun"),
                        };
                        ui.label(format!("#{}", s.base_index));
                        ui.label(format!("T{}", s.team));
                        ui.label(tribe_str);
                        ui.label(format!("{}, {}", s.world_x, s.world_z));
                        ui.label(format!("{}", s.building_count));
                        ui.label(format!("{}", s.wood_count));
                        ui.label(format!("{}", s.stone_count));
                        ui.label(format!("{}", s.food_count));
                        if ui
                            .small_button(tl(self.english, "聚焦", "Focus"))
                            .on_hover_text(tl(
                                self.english,
                                "將視角中心對齊此基地",
                                "Center view on this base",
                            ))
                            .clicked()
                        {
                            self.focus_base_request =
                                Some((s.world_x as f32, s.world_z as f32));
                        }
                        ui.end_row();
                    }
                });

            if let Some(placement) = &self.last_placement_preset {
                ui.label(format!("{}: {}", tl(self.english, "放置預設項目", "Placement Preset Entries"), placement.entries.len()));
            }
            if let Some(nature) = &self.last_nature_preset {
                ui.label(format!("{}: {}", tl(self.english, "自然預設項目", "Nature Preset Entries"), nature.entries.len()));
            }
            ui.horizontal(|ui| {
                if self.last_placement_preset.is_some() && ui.button(tl(self.english, "儲存放置預設為配置範本...", "Save placement preset as layout...")).clicked() {
                    self.report_preset_export(false);
                }
                if self.last_nature_preset.is_some() && ui.button(tl(self.english, "儲存自然預設為配置範本...", "Save nature preset as layout...")).clicked() {
                    self.report_preset_export(true);
                }
            });
        }
    }

    fn report_preset_export(&mut self, nature: bool) {
        match self.export_generated_preset(nature) {
            Ok(Some(path)) => {
                self.status_message = format!("Saved layout: {}", path.display());
                self.status_is_error = false;
            }
            Ok(None) => {}
            Err(e) => {
                self.status_message = format!("Layout failed: {e}");
                self.status_is_error = true;
            }
        }
    }

    fn render_layouts_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading(tl(self.english, "配置", "Layouts"));
        ui.label(tl(
            self.english,
            "森林（自然）配置會捕捉圖塊矩形內的景觀物件；聚落配置則可由物件面板（選取）或聚落生成器儲存。",
            "Forest (nature) layouts capture landscape objects in a tile rectangle; settlement layouts are saved from the Objects panel (selection) or the Settlement Generator.",
        ));
        ui.horizontal(|ui| {
            let labels = [
                tl(self.english, "左", "left"),
                tl(self.english, "上", "top"),
                tl(self.english, "右", "right"),
                tl(self.english, "下", "bottom"),
            ];
            for (index, name) in labels.iter().enumerate() {
                ui.label(*name);
                ui.add(egui::TextEdit::singleline(&mut self.layout_rect[index]).desired_width(48.0));
            }
        });
        ui.horizontal(|ui| {
            if ui.button(tl(self.english, "保存森林區域...", "Save forest region...")).clicked() {
                self.request_nature_layout_export();
            }
            if ui.button(tl(self.english, "載入並套用配置...", "Load and apply layout...")).clicked() {
                self.request_layout_import();
            }
        });
        ui.small(tl(
            self.english,
            "「套用」會開啟物件面板的套用配置對話框（錨點圖塊、旋轉、隊伍覆蓋）；佔一步復原步驟。",
            "Apply opens the Objects panel's Apply layout dialog (anchor tile, rotation, team override); one Undo step.",
        ));
    }
}

/// Bilinear world height (raw byte × step) of the vertex grid; 64 world units per vertex cell.
pub fn ground_height(heights: Option<&[u8]>, dimension: usize, x: f32, z: f32, step: f32) -> f32 {
    let Some(h) = heights.filter(|h| dimension >= 2 && h.len() == dimension * dimension) else {
        return 0.0;
    };
    let max = (dimension - 1) as f32;
    let (vx, vz) = ((x / 64.0).clamp(0.0, max), (z / 64.0).clamp(0.0, max));
    let (x0, z0) = (vx.floor() as usize, vz.floor() as usize);
    let (x1, z1) = ((x0 + 1).min(dimension - 1), (z0 + 1).min(dimension - 1));
    let (fx, fz) = (vx - x0 as f32, vz - z0 as f32);
    let at = |x: usize, z: usize| h[z * dimension + x] as f32;
    let top = at(x0, z0) + (at(x1, z0) - at(x0, z0)) * fx;
    let bottom = at(x0, z1) + (at(x1, z1) - at(x0, z1)) * fx;
    (top + (bottom - top) * fz) * step
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_ui_initial_state_and_dirty_guards() {
        let mut ui = EnvironmentUi::default();
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
    fn test_environment_ui_flora_generation() {
        let mut ui = EnvironmentUi::default();
        ui.flora_density = 0.5;
        ui.generate_flora_scatter(65, None, 0.0);
        assert!(!ui.generated_flora.is_empty());
        assert!(ui.is_dirty());
    }

    #[test]
    fn test_environment_ui_neutral_lair_addition() {
        let mut ui = EnvironmentUi::default();
        assert!(ui.add_neutral_lair().is_ok());
        assert_eq!(ui.generated_lairs.len(), 1);
        // Canonical definitions are FigTie animals; the binder rejects them and the panel must say so.
        assert!(ui.generated_lair_events.is_empty());
        assert!(ui.status_message.contains("No scenario events"));
        assert!(ui.is_dirty());
    }

    #[test]
    fn test_environment_ui_lair_remove_and_clear_keep_events_in_sync() {
        let mut ui = EnvironmentUi::default();
        ui.add_neutral_lair().unwrap();
        ui.add_neutral_lair().unwrap();
        // Simulate event output per lair (canonical animal lairs yield none).
        ui.generated_lair_events = vec![
            ScenarioEvent::new("a1", 0, false, true),
            ScenarioEvent::new("a2", 0, false, true),
            ScenarioEvent::new("b1", 0, false, true),
        ];
        ui.lair_event_counts = vec![2, 1];
        assert!(!ui.remove_lair(5));
        assert!(ui.remove_lair(0));
        assert_eq!(ui.generated_lairs.len(), 1);
        assert_eq!(ui.generated_lair_events.len(), 1);
        assert_eq!(ui.generated_lair_events[0].name, "b1");
        ui.clear_lairs();
        assert!(ui.generated_lairs.is_empty() && ui.generated_lair_events.is_empty() && ui.lair_event_counts.is_empty());
    }

    #[test]
    fn neutral_animal_requests_resolve_template_and_sample_ground_height() {
        let mut ui = EnvironmentUi::default();
        ui.lair_world_x = 64.0;
        ui.lair_world_z = 64.0;
        ui.lair_rotation_deg = 90.0;
        ui.add_neutral_lair().unwrap();
        ui.flora_templates_text = "77,FigTieWol00_Wilder_Wolf".into();
        let (requests, unresolved) = ui.lair_requests(2, Some(&[1, 2, 3, 4]), 2.0).unwrap();
        assert!(unresolved.is_empty());
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].type_id, 77);
        assert_eq!(requests[0].name, "FigTieWol00_Wilder_Wolf");
        assert_eq!(
            (requests[0].x, requests[0].y, requests[0].z),
            (64.0, 8.0, 64.0)
        );
        assert_eq!(requests[0].rotation_deg, 90.0);
        assert_eq!(requests[0].team, Some(8));
    }

    #[test]
    fn neutral_animal_requests_report_unresolved_template_names() {
        let mut ui = EnvironmentUi::default();
        ui.add_neutral_lair().unwrap();
        ui.flora_templates_text = "77,OtherAnimal".into();
        let (requests, unresolved) = ui.lair_requests(65, None, 1.0).unwrap();
        assert!(requests.is_empty());
        assert_eq!(unresolved, vec!["FigTieWol00_Wilder_Wolf"]);
    }

    #[test]
    fn neutral_animal_requests_refuse_more_than_one_undoable_batch() {
        let mut ui = EnvironmentUi::default();
        for _ in 0..=crate::design_apply::MAX_OBJECTS_PER_BATCH {
            ui.add_neutral_lair().unwrap();
        }
        assert!(ui
            .lair_requests(65, None, 1.0)
            .unwrap_err()
            .contains("undoable batch"));
        assert_eq!(
            ui.generated_lairs.len(),
            crate::design_apply::MAX_OBJECTS_PER_BATCH + 1
        );
    }

    #[test]
    fn queued_neutral_animals_clear_their_associated_event_drafts() {
        let mut ui = EnvironmentUi::default();
        ui.add_neutral_lair().unwrap();
        ui.generated_lair_events
            .push(ScenarioEvent::new("lair-wave", 60, true, true));
        ui.lair_event_counts = vec![1];
        ui.plan_queued(EnvApply::Lairs, 1, "");
        assert!(ui.generated_lairs.is_empty());
        assert!(ui.generated_lair_events.is_empty() && ui.lair_event_counts.is_empty());
        assert!(ui.status_message.contains("Lair event drafts were cleared"));
    }

    #[test]
    fn test_environment_ui_lairs_tab_renders_with_entries() {
        let mut ui = EnvironmentUi::default();
        ui.open = true;
        ui.current_tab = EnvironmentTab::NeutralLairs;
        ui.add_neutral_lair().unwrap();
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| ui.show(ctx, 65, None, None, 0.0, 4.0));
        assert_eq!(ui.generated_lairs.len(), 1);
    }

    #[test]
    fn test_environment_ui_settlement_generation() {
        let mut ui = EnvironmentUi::default();
        ui.settlement_players = 2;
        assert!(ui
            .generate_settlement(
                129,
                Some(&vec![20; 129 * 129]),
                Some((128, &vec![0; 128 * 128])),
                0.0,
                4.0
            )
            .is_ok());
        assert!(ui.last_distribution.is_some());
        assert!(ui.last_placement_preset.is_some());
        assert!(ui.is_dirty());
    }

    #[test]
    fn settlement_terrain_converts_world_heights_water_and_blocked_cells() {
        let heights = [10, 20, 30, 20, 30, 40, 30, 40, 50];
        let collision = [0, 128, 129, 255];
        let terrain =
            SettlementTerrain::new(3, Some(&heights), Some((2, &collision)), 12.5, 4.0).unwrap();
        assert_eq!(terrain.sample_height(64.0, 0.0), 80.0);
        assert_eq!(
            terrain.sample_height(32.0, 32.0),
            40.0,
            "half-vertex coordinates use MathF.Round ties-to-even"
        );
        assert_eq!(terrain.sample_height(128.0, 128.0), 200.0);
        assert_eq!(terrain.water_world, 50.0);
        assert!(!terrain.is_blocked(0, 0));
        assert!(terrain.is_blocked(1, 0));
        assert!(terrain.is_blocked(0, 1));
        assert!(terrain.is_blocked(1, 1));
        for (x, z) in [(-1, 0), (0, -1), (2, 0), (0, 2), (i32::MAX, 0)] {
            assert!(terrain.is_blocked(x, z));
        }
    }

    #[test]
    fn settlement_collision_matches_native_free_cell_boundary() {
        let heights = [20; 4];
        for (value, blocked) in [
            (0, false), (1, false), (5, false), (6, true),
            (127, true), (128, true), (129, true), (255, true),
        ] {
            let collision = [value];
            let terrain = SettlementTerrain::new(
                2, Some(&heights), Some((1, &collision)), 0.0, 4.0,
            ).unwrap();
            assert_eq!(terrain.is_blocked(0, 0), blocked, "collision={value}");
        }
    }

    #[test]
    fn settlement_terrain_rejects_missing_inconsistent_and_invalid_sources() {
        let h = [20; 9];
        let c = [0; 4];
        assert!(SettlementTerrain::new(0, None, None, 0.0, 4.0).is_err());
        assert!(SettlementTerrain::new(3, Some(&h), None, 0.0, 4.0).is_err());
        for (dimension, heights, size, collision) in [
            (1, &h[..1], 0, &c[..0]),
            (3, &h[..8], 2, &c[..]),
            (3, &h[..], 1, &c[..1]),
            (3, &h[..], 2, &c[..3]),
            (usize::MAX, &h[..], 2, &c[..]),
        ] {
            assert!(SettlementTerrain::new(
                dimension,
                Some(heights),
                Some((size, collision)),
                0.0,
                4.0
            )
            .is_err());
        }
        for step in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::MAX] {
            assert!(SettlementTerrain::new(3, Some(&h), Some((2, &c)), 0.0, step).is_err());
        }
        assert!(SettlementTerrain::new(3, Some(&h), Some((2, &c)), f32::NAN, 4.0).is_err());
    }

    #[test]
    fn settlement_generation_no_session_preserves_existing_ui_plan_and_settings() {
        let mut ui = EnvironmentUi::default();
        let heights = vec![20; 129 * 129];
        let collision = vec![0; 128 * 128];
        ui.generate_settlement(129, Some(&heights), Some((128, &collision)), 0.0, 4.0)
            .unwrap();
        let before = ui.last_distribution.clone();
        let preset = ui.last_placement_preset.clone();
        let nature = ui.last_nature_preset.clone();
        ui.apply_requested = Some(EnvApply::Settlement);
        ui.settlement_seed = 777;
        ui.open = true;
        assert!(ui
            .generate_settlement(0, None, None, 0.0, 4.0)
            .unwrap_err()
            .contains("session"));
        assert!(ui
            .generate_settlement(129, Some(&heights), Some((127, &collision)), 0.0, 4.0)
            .is_err());
        assert_eq!(ui.last_distribution, before);
        assert_eq!(ui.last_placement_preset, preset);
        assert_eq!(ui.last_nature_preset, nature);
        assert_eq!(ui.apply_requested, Some(EnvApply::Settlement));
        assert_eq!(ui.settlement_seed, 777);
        assert!(ui.open && ui.is_dirty());
    }

    #[test]
    fn settlement_live_collision_changes_sites_and_resources_deterministically() {
        let mut ui = EnvironmentUi::default();
        let heights = vec![20; 129 * 129];
        let mut collision = vec![0; 128 * 128];
        ui.generate_settlement(129, Some(&heights), Some((128, &collision)), 0.0, 4.0)
            .unwrap();
        let clear = ui.last_distribution.clone().unwrap();
        assert!(clear.players.iter().all(|p| p.site_evaluation.is_valid));
        let site = &clear.players[0].site_evaluation;
        let (cx, cz) = (site.anchor_tile_x as usize, site.anchor_tile_z as usize);
        for z in cz.saturating_sub(4)..=(cz + 4).min(127) {
            for x in cx.saturating_sub(4)..=(cx + 4).min(127) {
                collision[z * 128 + x] = 255;
            }
        }
        assert!(!clear.players[0].forest_trees.is_empty());
        for tree in &clear.players[0].forest_trees {
            let x = (tree.world_x / SETTLEMENT_TILE_WORLD_SIZE) as usize;
            let z = (tree.world_z / SETTLEMENT_TILE_WORLD_SIZE) as usize;
            collision[z * 128 + x] = 255;
        }
        ui.generate_settlement(129, Some(&heights), Some((128, &collision)), 0.0, 4.0)
            .unwrap();
        let blocked = ui.last_distribution.clone().unwrap();
        assert_ne!(clear.players[0].buildings, blocked.players[0].buildings);
        assert_ne!(
            clear.players[0].forest_trees,
            blocked.players[0].forest_trees
        );
        let terrain =
            SettlementTerrain::new(129, Some(&heights), Some((128, &collision)), 0.0, 4.0).unwrap();
        for player in &blocked.players {
            for tree in player.forest_trees.iter().chain(&player.stone_quarries) {
                assert!(!terrain.is_blocked(
                    (tree.world_x / SETTLEMENT_TILE_WORLD_SIZE) as i32,
                    (tree.world_z / SETTLEMENT_TILE_WORLD_SIZE) as i32
                ));
            }
        }
        ui.generate_settlement(129, Some(&heights), Some((128, &collision)), 0.0, 4.0)
            .unwrap();
        assert_eq!(ui.last_distribution.as_ref(), Some(&blocked));
    }

    #[test]
    fn settlement_live_height_step_and_water_change_site_validity() {
        let mut heights = vec![20; 129 * 129];
        for z in 0..129 {
            for x in 0..129 {
                heights[z * 129 + x] += ((x + z) % 2) as u8 * 2;
            }
        }
        let collision = vec![0; 128 * 128];
        let valid = |step, water| {
            let terrain =
                SettlementTerrain::new(129, Some(&heights), Some((128, &collision)), water, step)
                    .unwrap();
            let height = |x, z| terrain.sample_height(x, z);
            let blocked = |x, z| terrain.is_blocked(x, z);
            SettlementSiteEvaluator::new(
                128,
                &height,
                &blocked,
                terrain.water_world,
                SETTLEMENT_TILE_WORLD_SIZE,
            )
            .evaluate(64.0, 64.0, SettlementTribe::Germanic, 0, 0.0)
            .is_valid
        };
        assert!(valid(1.0, 0.0));
        assert!(
            !valid(4.0, 0.0),
            "world height delta 8 exceeds the slope limit"
        );
        assert!(!valid(1.0, 22.0), "current water level submerges the site");
    }

    #[test]
    fn settlement_ground_height_matches_nearest_csharp_vertex_sampling() {
        let heights = [0, 100, 200, 10, 110, 210, 20, 120, 220];
        let collision = [0; 4];
        let terrain =
            SettlementTerrain::new(3, Some(&heights), Some((2, &collision)), 0.0, 2.0)
                .unwrap();
        assert_eq!(terrain.sample_height(31.0, 0.0), 0.0);
        assert_eq!(terrain.sample_height(32.0, 0.0), 0.0, "0.5 ties to even");
        assert_eq!(terrain.sample_height(96.0, 0.0), 400.0, "1.5 ties to even");
        assert_eq!(terrain.sample_height(10_000.0, 0.0), 400.0, "clamped at map edge");
        assert_eq!(terrain.sample_height(0.0, 64.0), 20.0);
    }

    #[test]
    fn layout_rect_parsing_and_export_requests() {
        let mut ui = EnvironmentUi::default();
        assert_eq!(ui.parse_layout_rect().unwrap(), TileRect { left: 0, top: 0, right: 8, bottom: 8 });
        ui.layout_rect = ["2".into(), "3".into(), "2".into(), "9".into()];
        assert!(ui.parse_layout_rect().is_err(), "empty width");
        ui.request_nature_layout_export();
        assert!(ui.status_is_error && ui.take_layout_export_request().is_none());
        ui.layout_rect = ["-1".into(), "0".into(), "4".into(), "4".into()];
        assert!(ui.parse_layout_rect().is_err());
        ui.layout_rect = [" 2 ".into(), "3".into(), "6".into(), "9".into()];
        ui.request_nature_layout_export();
        assert_eq!(ui.take_layout_export_request(), Some(TileRect { left: 2, top: 3, right: 6, bottom: 9 }));
        assert!(ui.take_layout_export_request().is_none(), "take-once");
        ui.request_layout_import();
        assert!(ui.take_layout_import_request() && !ui.take_layout_import_request());
    }

    #[test]
    fn generated_settlement_presets_export_through_injected_picker() {
        use against_rome_environment::settlement::{MapLayoutEntry, MapLayoutKind, MapLayoutPreset};
        let dir = std::env::temp_dir().join(format!("arm_env_layout_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("base");
        let mut ui = EnvironmentUi::default();
        assert!(ui.export_generated_preset(false).is_err(), "nothing generated yet");
        ui.last_placement_preset = Some(MapLayoutPreset::new(
            MapLayoutKind::Placement,
            vec![MapLayoutEntry::with_team_and_count("RomHaus01", 10.0, 20.0, 0.0, 90.0, 1, 0)],
        ));
        let t = target.clone();
        ui.layout_picker = Some(Box::new(move |_| Some(t.clone())));
        let written = ui.export_generated_preset(false).unwrap().unwrap();
        assert!(written.to_string_lossy().ends_with("base.arm-layout.json"));
        let back = crate::layout_io::read_layout_file(&written).unwrap();
        assert_eq!(back.entries[0].type_name, "RomHaus01");
        ui.layout_picker = Some(Box::new(|_| None));
        assert_eq!(ui.export_generated_preset(false).unwrap(), None, "cancelled picker");
        let _ = std::fs::remove_dir_all(dir);
    }
}

fn settlement_tribe_label(english: bool, value: SettlementTribe) -> &'static str {
    match value {
        SettlementTribe::Germanic => tl(english, "日耳曼", "Germanic"),
        SettlementTribe::Roman => tl(english, "羅馬", "Roman"),
        SettlementTribe::Celtic => tl(english, "塞爾特", "Celtic"),
        SettlementTribe::Hun => tl(english, "匈人", "Hun"),
    }
}

fn settlement_symmetry_label(english: bool, value: SymmetryMode) -> &'static str {
    match value {
        SymmetryMode::RotationalSymmetry => tl(english, "旋轉對稱", "Rotational"),
        SymmetryMode::CentralSymmetry => tl(english, "中心點對稱", "Central"),
        SymmetryMode::TopologicalEquidistant => tl(english, "拓撲等距", "Topological equidistant"),
    }
}

fn tl(english: bool, zh: &'static str, en: &'static str) -> &'static str {
    if english { en } else { zh }
}
fn discard_labels(english: bool) -> [&'static str; 4] {
    [
        tl(english, "捨棄未儲存的環境變更？", "Discard Unsaved Environment Changes?"),
        tl(english, "關閉將捨棄目前產生的植物、巢穴與聚落預設。", "Closing will discard currently generated flora, lairs, and settlement presets."),
        tl(english, "捨棄並關閉", "Discard & Close"),
        tl(english, "繼續編輯", "Keep Editing"),
    ]
}
#[cfg(test)]
mod small_ux_tests {
    use super::*;
    #[test]
    fn clear_lairs_confirmation_cancel_confirm_empty_and_drafts() {
        let mut ui = EnvironmentUi::default();
        ui.request_clear_lairs();
        assert!(!ui.pending_clear_lairs);
        ui.add_neutral_lair().unwrap();
        ui.generated_lair_events.push(ScenarioEvent::new("draft", 0, false, true));
        ui.lair_event_counts = vec![1];
        ui.request_clear_lairs();
        assert!(ui.pending_clear_lairs);
        ui.resolve_clear_lairs(false);
        assert_eq!(ui.generated_lairs.len(), 1);
        assert_eq!(ui.generated_lair_events.len(), 1);
        assert_eq!(ui.lair_event_counts, vec![1]);
        ui.request_clear_lairs();
        ui.resolve_clear_lairs(true);
        assert!(ui.generated_lairs.is_empty() && ui.generated_lair_events.is_empty() && ui.lair_event_counts.is_empty());
        ui.generated_lair_events.push(ScenarioEvent::new("draft", 0, false, true));
        ui.request_clear_lairs();
        assert!(ui.pending_clear_lairs);
    }
    #[test]
    fn discard_dialog_is_bilingual() {
        assert_eq!(discard_labels(true), ["Discard Unsaved Environment Changes?", "Closing will discard currently generated flora, lairs, and settlement presets.", "Discard & Close", "Keep Editing"]);
        assert_eq!(discard_labels(false), ["捨棄未儲存的環境變更？", "關閉將捨棄目前產生的植物、巢穴與聚落預設。", "捨棄並關閉", "繼續編輯"]);
    }

    #[test]
    fn settlement_resource_summaries_and_focus_request_work() {
        let mut ui = EnvironmentUi::default();
        assert!(ui.base_resource_summaries().is_empty());
        assert!(ui.take_focus_base_request().is_none());

        let heights = vec![20; 129 * 129];
        let collision = vec![0; 128 * 128];
        ui.generate_settlement(129, Some(&heights), Some((128, &collision)), 0.0, 4.0).unwrap();

        let summaries = ui.base_resource_summaries();
        assert_eq!(summaries.len(), ui.settlement_players);
        for s in &summaries {
            assert!(s.building_count > 0, "must have buildings");
            assert!(s.wood_count > 0, "must have wood/forest");
            assert!(s.stone_count > 0, "must have stone quarries");
            assert!(s.world_x > 0 && s.world_z > 0);
        }

        ui.focus_base_request = Some((summaries[0].world_x as f32, summaries[0].world_z as f32));
        assert_eq!(
            ui.take_focus_base_request(),
            Some((summaries[0].world_x as f32, summaries[0].world_z as f32))
        );
        assert!(ui.take_focus_base_request().is_none(), "take-once");
    }

    #[test]
    fn settlement_preview_and_failed_regeneration_preserve_plan() {
        let mut ui = EnvironmentUi::default();
        let heights = vec![20; 129 * 129];
        let collision = vec![0; 128 * 128];
        ui.generate_settlement(129, Some(&heights), Some((128, &collision)), 0.0, 4.0).unwrap();
        let plan = ui.last_distribution.clone();
        let placement = ui.last_placement_preset.clone();
        let nature = ui.last_nature_preset.clone();
        let seed = ui.settlement_seed;
        let dirty = ui.is_dirty();
        let ctx = egui::Context::default();
        for english in [false, true] {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |panel| {
                    ui.paint_canvas_preview(panel.painter(), panel.max_rect(), english);
                });
            });
        }
        assert!(ui.regenerate_settlement_with_new_seed(129, None, None, 0.0, 4.0).is_err());
        assert_eq!(ui.settlement_seed, seed);
        assert_eq!(ui.last_distribution, plan);
        assert_eq!(ui.last_placement_preset, placement);
        assert_eq!(ui.last_nature_preset, nature);
        assert_eq!(ui.is_dirty(), dirty);
        for modal in [false, true] {
            ui.pending_discard = modal;
            ui.regeneration_blocked = !modal;
            assert!(ui.regenerate_settlement_with_new_seed(129, Some(&heights), Some((128, &collision)), 0.0, 4.0).is_err());
            assert_eq!(ui.last_distribution, plan);
            assert_eq!(ui.settlement_seed, seed);
        }
    }

    #[test]
    fn settlement_counts_match_actual_apply_entries() {
        let mut ui = EnvironmentUi::default();
        let heights = vec![20; 129 * 129];
        let collision = vec![0; 128 * 128];
        ui.generate_settlement(129, Some(&heights), Some((128, &collision)), 0.0, 4.0).unwrap();
        let dist = ui.last_distribution.as_mut().unwrap();
        for player in &mut dist.players {
            for food in &mut player.wildlife_and_food { food.count = 9; }
        }
        let expected = SettlementGeneratorEngine::to_placement_layout_preset(ui.last_distribution.as_ref().unwrap());
        let summaries = ui.base_resource_summaries();
        assert_eq!(summaries.iter().map(|s| s.building_count + s.food_count).sum::<usize>(), expected.entries.len());
        ui.settlement_include_nature = false;
        assert!(ui.base_resource_summaries().iter().all(|s| s.wood_count == 0 && s.stone_count == 0));
    }

    #[test]
    fn regenerate_settlement_with_new_seed_advances_seed_and_updates_distribution() {
        let mut ui = EnvironmentUi::default();
        let initial_seed = ui.settlement_seed;
        let heights = vec![20; 129 * 129];
        let collision = vec![0; 128 * 128];

        let new_seed = ui
            .regenerate_settlement_with_new_seed(129, Some(&heights), Some((128, &collision)), 0.0, 4.0)
            .unwrap();
        assert_ne!(new_seed, initial_seed);
        assert_eq!(ui.settlement_seed, new_seed);
        assert!(ui.last_distribution.is_some());
        assert_eq!(ui.base_resource_summaries().len(), ui.settlement_players);
    }

    #[test]
    fn settlement_canvas_preview_toggle_and_reset() {
        let mut ui = EnvironmentUi::default();
        assert!(ui.show_canvas_preview);
        ui.show_canvas_preview = false;
        assert!(!ui.show_canvas_preview);

        let heights = vec![20; 129 * 129];
        let collision = vec![0; 128 * 128];
        ui.generate_settlement(129, Some(&heights), Some((128, &collision)), 0.0, 4.0).unwrap();
        assert!(ui.last_distribution.is_some());

        ui.reset_session();
        assert!(ui.last_distribution.is_none());
        assert!(ui.focus_base_request.is_none());
    }

    fn collect_shape_texts(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
        match shape {
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::epaint::Shape::Vec(shapes) => {
                for s in shapes {
                    collect_shape_texts(s, out);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn test_environment_ui_chinese_localization_renders_no_untranslated_literals() {
        let tabs = [
            EnvironmentTab::FloraScatter,
            EnvironmentTab::NeutralLairs,
            EnvironmentTab::SettlementGenerator,
            EnvironmentTab::Layouts,
        ];
        let english_literals_to_forbid = [
            "Flora & Ecology Auto-Scatter",
            "Neutral Wildlife & Lair Setup",
            "One-Click Settlement / Base Generator",
            "Environment & Settlement Studio",
            "Generates organic nature foliage",
            "Biome Ecology:",
            "Random Seed:",
            "Density Multiplier:",
            "Forest Coverage:",
            "Forest Patch Scale:",
            "Jitter Amount:",
            "Avoid Roads & Buildings:",
            "Object templates (id,name):",
            "Generate Flora Scatter",
            "Clear Generated Flora",
            "Lair Definition:",
            "World Location X / Z:",
            "Rotation (deg):",
            "Add Neutral Lair",
            "Apply Animals to Objects session",
            "Apply Lair Events",
            "Player Count (2..8):",
            "Tribe Architecture:",
            "Layout Symmetry:",
            "Generate Forests & Stone Quarries",
            "Spawn Radius Ratio:",
            "Save forest region... / 保存森林區域",
            "Load and apply layout... / 載入並套用配置",
        ];

        let ctx = egui::Context::default();
        let heights = vec![20u8; 129 * 129];
        let collision = vec![0u8; 128 * 128];

        for tab in tabs {
            let mut ui = EnvironmentUi::default();
            ui.open = true;
            ui.english = false;
            ui.current_tab = tab;

            // egui windows measure on first frame and paint on subsequent frames
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))),
                    ..Default::default()
                },
                |ctx| {
                    ui.show(
                        ctx,
                        129,
                        Some(&heights),
                        Some((128, &collision)),
                        0.0,
                        4.0,
                    );
                },
            );

            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))),
                    ..Default::default()
                },
                |ctx| {
                    ui.show(
                        ctx,
                        129,
                        Some(&heights),
                        Some((128, &collision)),
                        0.0,
                        4.0,
                    );
                },
            );

            let mut texts = Vec::new();
            for shape in &output.shapes {
                collect_shape_texts(&shape.shape, &mut texts);
            }
            let all_text = texts.join(" ");

            // Assert Chinese header is present
            assert!(all_text.contains("環境與聚落工作室"), "tab {:?} missing Chinese window title", tab);

            // Assert no untranslated literals remain in rendered UI
            for &forbidden in &english_literals_to_forbid {
                assert!(
                    !all_text.contains(forbidden),
                    "Tab {:?} rendered untranslated literal: '{}'",
                    tab,
                    forbidden
                );
            }
        }
    }
}

#[cfg(test)]
mod batch20_tests {
    use super::*;
    #[test]
    fn settlement_selected_labels_follow_language_without_changing_values() {
        let mut panel = EnvironmentUi::default();
        for tribe in [SettlementTribe::Germanic, SettlementTribe::Roman, SettlementTribe::Celtic, SettlementTribe::Hun] {
            for symmetry in [SymmetryMode::RotationalSymmetry, SymmetryMode::CentralSymmetry, SymmetryMode::TopologicalEquidistant] {
                panel.settlement_tribe = tribe;
                panel.settlement_symmetry = symmetry;
                for english in [true, false, true, false] {
                    panel.english = english;
                    let text = crate::batch20_localization_tests::render(|ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| panel.render_settlement_tab(ui, 33, None, None, 0.0, 4.0));
                    });
                    assert!(text.contains(settlement_tribe_label(english, tribe)), "{text}");
                    assert!(text.contains(settlement_symmetry_label(english, symmetry)), "{text}");
                    if !english {
                        assert!(!text.contains("RotationalSymmetry"));
                        assert!(!text.contains("Germanic"));
                        assert!(!text.contains("環形對稱"));
                        if symmetry == SymmetryMode::RotationalSymmetry { assert!(text.contains("旋轉對稱")); }
                    }
                    assert_eq!(panel.settlement_tribe, tribe);
                    assert_eq!(panel.settlement_symmetry, symmetry);
                }
            }
        }
    }
}
