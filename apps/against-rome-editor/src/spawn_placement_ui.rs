//! Scenario spawn placement panel (port of C# `MapEditorForm.Placement.cs`,
//! `MapEditorForm.PlacementWorkflow.cs` and `PlacementBatchEditDialog.cs`).
//!
//! The panel edits scenario spawns (`ScenarioDocument.spawns`, compiled into the level BCI on
//! Safe Save) through a [`SpawnPlacementSession`]. `ScenarioUi` stays the single source of truth:
//! after every session command the spawn list is validated and written through
//! `ScenarioUi::apply_spawns`; if the document rejects it the command is rolled back.
//!
//! Prebuilt rules (C# `MapEditorForm.Persistence.cs`): while editing, every placement is an
//! ordinary spawn in the scenario document. On Safe Save, buildings on team 0..=8 (and native
//! raw-rotation objects) become prebuilt DATA objects through the save-time DATA transaction
//! (`against_rome_maps::spawn_data`), duplicates of bound spawns copy their DATA family, and a
//! building without a template falls back to a script-created construction site. Bound spawns stay
//! fully editable: ordinary ones are rebuilt from their template; native families (raw rotation)
//! only translate (the session rejects team/direction changes).

use crate::app::{tr, ShortcutAction};
use crate::scenario_ui::ScenarioUi;
use crate::spawn_placement::{
    category_text, landscape_move_warning, landscape_nearby, landscape_place_warning, load_spawns, spawn_to_placed,
    matches_search, nearest_placement, new_placement, normalize_angle, object_display_name, session_to_spawns, takes_unit_count, team_range, tile_center, tile_ground_height, tribe_text,
    type_for_alias, world_ground_height, PlacementError, SpawnPlacementSession,
};
use against_rome_ambience::Language as AppLanguage;
use against_rome_maps::sdl_objects::{SdlObjectCategory, SdlObjectType, SdlPlacedObject};
use against_rome_maps::script_aliases::ScriptObjectAliases;
use against_rome_maps::spawn_data::{stage_starter_kit, StarterKitTerrain};
use against_rome_maps::starter_kit::BlankMapStarterKit;
use against_rome_scenario::model::Spawn;
use against_rome_textures::{NativeSprite, NativeSpriteCatalog, DEFAULT_DIRECTION};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, TextureHandle};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const WORLD_SIZE: f32 = 16384.0;

fn sprite_color_image(sprite: &NativeSprite) -> egui::ColorImage {
    let pixels: Vec<Color32> = sprite
        .argb_pixels
        .iter()
        .map(|argb| {
            Color32::from_rgba_unmultiplied(
                (argb >> 16) as u8,
                (argb >> 8) as u8,
                *argb as u8,
                (argb >> 24) as u8,
            )
        })
        .collect();
    egui::ColorImage {
        size: [sprite.width as usize, sprite.height as usize],
        pixels,
    }
}

/// 面板每影格所需的編輯器資料，避免直接借用 `EditorApp`。
pub struct SpawnContext<'a> {
    pub lang: AppLanguage,
    pub heights: Option<&'a [u8]>,
    pub vertex_size: usize,
    pub height_step: f32,
    /// Retail maps are read-only (C# `IsCustom == false`).
    pub read_only: bool,
}

impl SpawnContext<'_> {
    fn english(&self) -> bool {
        self.lang == AppLanguage::English
    }
    fn ground_height(&self, x: f32, z: f32) -> f32 {
        self.heights
            .map_or(0.0, |h| world_ground_height(h, self.vertex_size, x, z, self.height_step))
    }
}

#[derive(Clone, Debug, PartialEq)]
struct EditDialog {
    index: usize,
    team: i32,
    x: f32,
    y: f32,
    z: f32,
    angle: i32,
    count: i32,
    figure: bool,
    animal: bool,
    native: bool,
    min_team: i32,
    max_team: i32,
}

/// Starter kit dialog inputs: team and anchor in map tiles (C# `StarterKitDialog`).
#[derive(Clone, Debug, PartialEq)]
struct StarterDialog {
    team: i32,
    x: f32,
    z: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct BatchDialog {
    count: usize,
    includes_units: bool,
    apply_team: bool,
    team: i32,
    apply_angle: bool,
    angle: i32,
}

/// A canvas gesture on the selected placements: world-space press point and current offset.
#[derive(Clone, Copy, Debug)]
struct CanvasDrag {
    start: (f32, f32),
    offset: (f32, f32),
    moved: bool,
}

/// Result of one row click, applied after the list closure ends.
enum RowClick {
    Single(usize),
    Toggle(usize),
    Range(usize),
}

pub struct SpawnPlacementUi {
    layout_picker: crate::layout_io::LayoutFilePicker,
    pending_layout: Option<crate::spawn_layout::SpawnLayout>,
    layout_remap: bool,
    layout_from: i32,
    layout_to: i32,
    pub open: bool,
    session: SpawnPlacementSession,
    catalog: Vec<SdlObjectType>,
    catalog_root: Option<PathBuf>,
    catalog_fixed: bool,
    sprite_catalog: Option<NativeSpriteCatalog>,
    sprite_catalog_root: Option<PathBuf>,
    palette_textures: std::collections::HashMap<(String, i32), TextureHandle>,
    foreign: Vec<Spawn>,
    /// Document spawns the session currently reflects (change detection + `previous` lookup).
    synced: Vec<Spawn>,
    /// Spawns as loaded/saved: lets undo restore a deleted spawn with its original flags.
    loaded: Vec<Spawn>,
    bound: HashSet<Uuid>,
    map_key: Option<PathBuf>,
    selection: BTreeSet<usize>,
    anchor: Option<usize>,
    category_filter: Option<SdlObjectCategory>,
    tribe_filter: Option<String>,
    search: String,
    selected_type: Option<String>,
    team: i32,
    count: i32,
    angle: i32,
    place_mode: bool,
    active: bool,
    edit_dialog: Option<EditDialog>,
    batch_dialog: Option<BatchDialog>,
    /// Landscape (tree/bush) object positions `(x, z)` for the building clearance warning.
    landscape: Vec<(f32, f32)>,
    drag: Option<CanvasDrag>,
    /// Detached starter kit staged on an existing blank map; imported into DATA on Safe Save
    /// (C# `_pendingStarterKit`). Retained across Undo/Redo; live ids decide whether it applies.
    pending_kit: Option<BlankMapStarterKit>,
    starter_dialog: Option<StarterDialog>,
    starter_request: Option<(i32, f32, f32)>,
    /// A canvas press hit a placement: brushes must ignore the gesture until it is released.
    captured: bool,
    status: String,
    status_error: bool,
}

impl Default for SpawnPlacementUi {
    fn default() -> Self {
        Self {
            layout_picker: crate::layout_io::native_layout_picker(),
            pending_layout: None,
            layout_remap: false,
            layout_from: 0,
            layout_to: 1,
            open: false,
            session: SpawnPlacementSession::new(),
            catalog: Vec::new(),
            catalog_root: None,
            catalog_fixed: false,
            sprite_catalog: None,
            sprite_catalog_root: None,
            palette_textures: std::collections::HashMap::new(),
            foreign: Vec::new(),
            synced: Vec::new(),
            loaded: Vec::new(),
            bound: HashSet::new(),
            map_key: None,
            selection: BTreeSet::new(),
            anchor: None,
            category_filter: None,
            tribe_filter: None,
            search: String::new(),
            selected_type: None,
            team: 0,
            count: 10,
            angle: 0,
            place_mode: false,
            active: false,
            edit_dialog: None,
            batch_dialog: None,
            landscape: Vec::new(),
            drag: None,
            pending_kit: None,
            starter_dialog: None,
            starter_request: None,
            captured: false,
            status: String::new(),
            status_error: false,
        }
    }
}

pub(crate) fn team_color(team: i32) -> Color32 {
    const PALETTE: [(u8, u8, u8); 9] = [
        (80, 160, 255),
        (255, 90, 90),
        (90, 220, 110),
        (255, 210, 70),
        (200, 110, 255),
        (80, 230, 230),
        (255, 150, 60),
        (230, 130, 190),
        (200, 200, 200),
    ];
    let (r, g, b) = PALETTE.get(team.max(0) as usize).copied().unwrap_or((170, 170, 170));
    Color32::from_rgb(r, g, b)
}

fn error_text(lang: AppLanguage, error: &PlacementError, out_of_range: (&'static str, &'static str)) -> String {
    match error {
        PlacementError::OutOfRange(_) => tr(lang, out_of_range.0, out_of_range.1).to_string(),
        other => other.to_string(),
    }
}

impl SpawnPlacementUi {
    // ---- state access (tests / app) ----------------------------------------------------

    pub fn session(&self) -> &SpawnPlacementSession {
        &self.session
    }
    pub fn catalog(&self) -> &[SdlObjectType] {
        &self.catalog
    }
    /// Injects a catalog (tests / callers that already hold the alias list).
    pub fn set_catalog(&mut self, catalog: Vec<SdlObjectType>) {
        self.catalog = catalog;
        self.catalog_fixed = true;
        self.catalog_root = None;
        self.synced.clear();
    }
    pub fn place_mode(&self) -> bool {
        self.open && self.place_mode
    }
    pub fn set_place_mode(&mut self, on: bool) {
        self.place_mode = on;
    }
    /// Keyboard shortcuts target this panel while it is open and was last interacted with.
    pub fn is_active(&self) -> bool {
        self.open && self.active
    }
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }
    pub fn selection(&self) -> Vec<usize> {
        self.selection.iter().copied().collect()
    }
    pub fn has_selection(&self) -> bool {
        !self.selection.is_empty()
    }
    pub fn status(&self) -> (&str, bool) {
        (&self.status, self.status_error)
    }
    /// True when the spawn at `index` is a prebuilt native DATA object (bound to a DATA family).
    pub fn is_data_bound_index(&self, index: usize) -> bool {
        self.session.get(index).is_some_and(|o| self.bound.contains(&o.scenario_id))
    }
    pub fn select_type(&mut self, name_def: &str) {
        self.selected_type = Some(name_def.to_string());
    }
    pub fn set_placement_values(&mut self, team: i32, count: i32, angle: i32) {
        self.team = team;
        self.count = count;
        self.angle = normalize_angle(angle);
    }
    pub fn placement_angle(&self) -> i32 {
        self.angle
    }

    pub(crate) fn set_status(&mut self, message: impl Into<String>, error: bool) {
        self.status = message.into();
        self.status_error = error;
    }

    fn selected_catalog_type(&self) -> Option<&SdlObjectType> {
        let name = self.selected_type.as_deref()?;
        self.catalog.iter().find(|t| t.name_def == name)
    }

    // ---- document sync -----------------------------------------------------------------

    /// Loads the alias catalog for the game root of `map` once per root.
    fn ensure_catalog(&mut self, map: Option<&Path>) {
        if self.catalog_fixed {
            return;
        }
        let root = map.and_then(Path::parent).and_then(Path::parent).map(Path::to_path_buf);
        if root == self.catalog_root {
            return;
        }
        self.catalog = match root.as_deref().map(ScriptObjectAliases::load) {
            Some(Ok(aliases)) => crate::spawn_placement::build_spawn_catalog(&aliases),
            _ => Vec::new(),
        };
        self.catalog_root = root;
        self.synced.clear();
        self.selected_type = None;
    }

    /// 唯讀開啟遊戲精靈資料，並只快取目前顯示的縮圖。
    fn ensure_sprite_catalog(&mut self, map: Option<&Path>) {
        let root = map.and_then(Path::parent).and_then(Path::parent).map(Path::to_path_buf);
        if root == self.sprite_catalog_root {
            return;
        }
        self.sprite_catalog = root.as_deref().and_then(NativeSpriteCatalog::open);
        self.sprite_catalog_root = root;
        self.palette_textures.clear();
    }

    fn palette_texture(
        &mut self,
        ctx: &egui::Context,
        object_type: &SdlObjectType,
        team: i32,
    ) -> Option<TextureHandle> {
        let key = (object_type.name_def.clone(), team);
        if let Some(texture) = self.palette_textures.get(&key) {
            return Some(texture.clone());
        }
        let sprite = self.sprite_catalog.as_ref()?.get_sprite(
            &object_type.name_def,
            team,
            DEFAULT_DIRECTION,
            None,
            None,
        )?;
        let image = sprite_color_image(&sprite);
        let texture = ctx.load_texture(
            format!("spawn-palette:{}:team-{team}", object_type.name_def),
            image,
            egui::TextureOptions::LINEAR,
        );
        self.palette_textures.insert(key, texture.clone());
        Some(texture)
    }

    fn rebuild_bound(&mut self, scenario: &ScenarioUi) {
        let document = scenario.document();
        self.bound = document
            .spawns
            .iter()
            .filter(|s| s.prebuilt || document.data_slots.iter().any(|b| b.spawn_id == s.id))
            .map(|s| s.id)
            .collect();
    }

    /// Re-reads the session from the scenario document when it changed behind our back (map
    /// load/switch, Safe Save reload, reset) and accepts a saved state (C# `AcceptSavedChanges`).
    pub fn sync_with_document(&mut self, scenario: &ScenarioUi, map: Option<&Path>) {
        self.ensure_catalog(map);
        let key = map.map(Path::to_path_buf);
        let spawns = &scenario.document().spawns;
        if key != self.map_key || *spawns != self.synced {
            self.pending_layout = None;
            self.map_key = key;
            let (placed, foreign) = load_spawns(spawns, &self.catalog);
            self.session.load(&placed);
            self.foreign = foreign;
            self.synced = spawns.clone();
            self.loaded = spawns.clone();
            self.selection.clear();
            self.anchor = None;
            self.edit_dialog = None;
            self.batch_dialog = None;
            self.pending_kit = None;
            self.rebuild_bound(scenario);
        } else if self.session.is_dirty() && !scenario.dirty() {
            // Safe Save committed the spawns (scenario baseline advanced): re-baseline.
            self.session.accept_saved_changes();
            self.pending_kit = None;
            self.rebuild_bound(scenario);
        }
    }

    /// Writes the session content into the scenario document; the document validates it.
    fn push_to_scenario(&mut self, scenario: &mut ScenarioUi) -> Result<(), String> {
        let mut original = self.synced.clone();
        original.extend(self.loaded.iter().filter(|s| !self.synced.iter().any(|k| k.id == s.id)).cloned());
        let spawns = session_to_spawns(self.session.objects(), &original, &self.foreign, &self.catalog);
        scenario.apply_spawns(spawns)?;
        self.synced = scenario.document().spawns.clone();
        self.selection.retain(|&i| i < self.session.count());
        Ok(())
    }

    /// 全部別名與位置先驗證，整份配置只建立一個復原命令。
    fn import_layout(&mut self, scenario: &mut ScenarioUi, layout: &crate::spawn_layout::SpawnLayout,
        anchor: (f32,f32), remap: Option<(i32,i32)>, lang: AppLanguage) -> Result<(usize,usize),String> {
        let (spawns, clamped) = layout.plan(scenario.known_aliases(), anchor, remap)?;
        let placed = spawns.iter().map(|s| spawn_to_placed(s, &self.catalog)
            .ok_or_else(|| format!("Unknown alias / 未知別名：{}", s.alias))).collect::<Result<Vec<_>,_>>()?;
        let old_len = self.loaded.len();
        self.loaded.extend(spawns.iter().cloned());
        if let Err(e) = self.mutate(scenario, lang, ("Invalid layout values", "配置數值無效"),
            |session| session.add_many(&placed).map(|_| ())) {
            self.loaded.truncate(old_len);
            return Err(format!("Import rejected / 匯入遭拒：{e}"));
        }
        Ok((spawns.len(),clamped))
    }

    /// Runs one session command, then syncs the document; a rejected sync rolls the command back.
    fn mutate(
        &mut self,
        scenario: &mut ScenarioUi,
        lang: AppLanguage,
        out_of_range: (&'static str, &'static str),
        command: impl FnOnce(&mut SpawnPlacementSession) -> Result<(), PlacementError>,
    ) -> Result<(), String> {
        let before = self.session.undo_depth();
        command(&mut self.session).map_err(|e| error_text(lang, &e, out_of_range))?;
        if self.session.undo_depth() == before {
            return Ok(()); // no-op command (nothing changed)
        }
        if let Err(e) = self.push_to_scenario(scenario) {
            self.session.abort_last();
            return Err(e);
        }
        Ok(())
    }

    fn guard(&mut self, scenario: &ScenarioUi, cx: &SpawnContext) -> bool {
        let lang = cx.lang;
        if cx.read_only {
            self.set_status(
                tr(lang, "Original maps are read-only; copy to a custom map first.", "原廠地圖為唯讀；請先複製為自製地圖。"),
                true,
            );
            return false;
        }
        if !scenario.is_loaded() {
            self.set_status(
                tr(
                    lang,
                    "Load the scenario first (the map needs SCRIPT/ak_level.bci).",
                    "請先載入劇本（地圖需有 SCRIPT/ak_level.bci）。",
                ),
                true,
            );
            return false;
        }
        true
    }

    // ---- operations (C# MapEditorForm.Placement*.cs) ------------------------------------

    /// C# `PlaceObjectAt`: one object at the centre of the clicked tile; Y from the terrain.
    pub fn place_at_tile(&mut self, scenario: &mut ScenarioUi, cx: &SpawnContext, tile_x: i32, tile_y: i32) {
        if !self.guard(scenario, cx) {
            return;
        }
        let lang = cx.lang;
        let Some(object_type) = self.selected_catalog_type().cloned() else {
            self.set_status(
                tr(lang, "Pick an object type in the palette first.", "請先在清單中選擇物件類型。"),
                true,
            );
            return;
        };
        let (world_x, world_z) = tile_center(tile_x, tile_y);
        let world_y = cx
            .heights
            .map_or(0.0, |h| tile_ground_height(h, cx.vertex_size, tile_x, tile_y, cx.height_step));
        let item = match new_placement(&object_type, world_x, world_y, world_z, self.team, self.angle as f32, self.count) {
            Ok(item) => item,
            Err(e) => {
                let message = error_text(lang, &e, ("Units need a team between 0 and 7.", "人物與部隊的隊伍必須介於 0 與 7。"));
                self.set_status(message, true);
                return;
            }
        };
        let name = object_display_name(&object_type, cx.english());
        match self.mutate(scenario, lang, ("Cannot place here.", "無法在此放置。"), |s| s.add(item)) {
            Ok(()) => {
                self.selection.clear();
                self.selection.insert(self.session.count() - 1);
                self.anchor = Some(self.session.count() - 1);
                let mut message = if cx.english() {
                    format!("Placed {name} at tile ({tile_x},{tile_y}); Save to write it.")
                } else {
                    format!("已在格子 ({tile_x},{tile_y}) 放置 {name}；按「儲存」才會寫入。")
                };
                // Buildings over trees: script creation is rejected by the game, DATA clips through.
                if self.warns_landscape(&[self.session.count() - 1]) {
                    message.push_str(landscape_place_warning(cx.english()));
                }
                self.set_status(message, false);
            }
            Err(e) => self.set_status(e, true),
        }
    }

    pub fn delete_selected(&mut self, scenario: &mut ScenarioUi, cx: &SpawnContext) {
        let indices = self.selection();
        if indices.is_empty() || !self.guard(scenario, cx) {
            return;
        }
        match self.mutate(scenario, cx.lang, ("Cannot delete.", "無法刪除。"), |s| s.remove_many(&indices)) {
            Ok(()) => {
                self.selection.clear();
                self.anchor = None;
                self.set_status(
                    if cx.english() {
                        format!("Deleted {} object(s); Save to write.", indices.len())
                    } else {
                        format!("已刪除 {} 個物件；按「儲存」才會寫入。", indices.len())
                    },
                    false,
                );
            }
            Err(e) => self.set_status(e, true),
        }
    }

    pub fn duplicate_selected(&mut self, scenario: &mut ScenarioUi, cx: &SpawnContext) {
        let indices = self.selection();
        if indices.is_empty() || !self.guard(scenario, cx) {
            return;
        }
        let mut copies = Vec::new();
        let result = self.mutate(
            scenario,
            cx.lang,
            (
                "Cannot duplicate: a shifted object would be outside the map or has invalid values. No objects were changed.",
                "無法複製：偏移後有物件超出地圖或數值無效；所有物件均未變更。",
            ),
            |s| {
                copies = s.duplicate_many(&indices, 256.0, 256.0)?;
                Ok(())
            },
        );
        match result {
            Ok(()) => {
                self.selection = copies.iter().copied().collect();
                self.anchor = copies.first().copied();
                self.set_status(
                    if cx.english() {
                        format!("Duplicated {} placed object(s); Save to write.", indices.len())
                    } else {
                        format!("已複製 {} 個放置物件；按「儲存」才會寫入。", indices.len())
                    },
                    false,
                );
            }
            Err(e) => self.set_status(e, true),
        }
    }

    /// C# `HandlePlacementRotationKey` for the placed list: relative rotation of the selection.
    pub fn rotate_selected(&mut self, scenario: &mut ScenarioUi, cx: &SpawnContext, degrees: i32) {
        let indices = self.selection();
        if indices.is_empty() || !self.guard(scenario, cx) {
            return;
        }
        if let Err(e) = self.mutate(scenario, cx.lang, ("Cannot rotate.", "無法旋轉。"), |s| {
            s.rotate_many(&indices, degrees)
        }) {
            self.set_status(e, true);
        }
    }

    /// Next placement direction (type list `R` / `Shift+R`).
    pub fn rotate_next_placement(&mut self, degrees: i32) {
        self.angle = normalize_angle(self.angle + degrees);
    }

    /// C# `MoveSelectedPlacements` (Ctrl+arrows / align to tile centre).
    pub fn move_selected(&mut self, scenario: &mut ScenarioUi, cx: &SpawnContext, dx: f32, dz: f32, snap: bool) {
        let indices = self.selection();
        if indices.is_empty() || !self.guard(scenario, cx) {
            return;
        }
        let height = |x: f32, z: f32| cx.ground_height(x, z);
        let height_at: Option<&dyn Fn(f32, f32) -> f32> = cx.heights.map(|_| &height as &dyn Fn(f32, f32) -> f32);
        let result = self.mutate(
            scenario,
            cx.lang,
            (
                "Cannot move: selection would leave the map. No objects changed.",
                "無法移動：選取物件會超出地圖；所有物件均未變更。",
            ),
            |s| {
                if snap {
                    s.snap_many_to_grid(&indices, height_at)
                } else {
                    s.move_many(&indices, dx, dz, height_at)
                }
            },
        );
        match result {
            Ok(()) => {
                let mut message = if cx.english() {
                    format!("Moved {} objects; Save to write.", indices.len())
                } else {
                    format!("已移動 {} 個物件；按「儲存」才會寫入。", indices.len())
                };
                if self.warns_landscape(&indices) {
                    message.push_str(landscape_move_warning(cx.english()));
                }
                self.set_status(message, false);
            }
            Err(e) => self.set_status(e, true),
        }
    }

    // ---- landscape clearance + canvas picking --------------------------------------------

    /// Replaces the landscape (tree/bush) positions used by the building clearance warning.
    pub fn set_landscape(&mut self, points: Vec<(f32, f32)>) {
        self.landscape = points;
    }

    pub fn landscape_count(&self) -> usize {
        self.landscape.len()
    }

    fn warns_landscape(&self, indices: &[usize]) -> bool {
        indices.iter().filter_map(|&i| self.session.get(i)).any(|o| {
            o.obj_type.category == SdlObjectCategory::Building && landscape_nearby(&self.landscape, o.world_x, o.world_z)
        })
    }

    /// True between a canvas press on a placement and its release.
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Whether a drag moved far enough to count as a move (not a click).
    pub fn drag_offset(&self) -> Option<(f32, f32)> {
        self.drag.filter(|d| d.moved).map(|d| d.offset)
    }

    /// Canvas press at a world point. A hit selects the nearest placement (Ctrl toggles, Shift adds
    /// without dragging) and, for a plain press, arms a drag of the whole selection. Returns true
    /// when the press hit a placement, i.e. the gesture belongs to this panel and not to a brush.
    pub fn canvas_press(&mut self, world: (f32, f32), radius: f32, toggle: bool, add: bool) -> bool {
        let Some(hit) = nearest_placement(self.session.objects(), world.0, world.1, radius) else {
            return false;
        };
        self.active = true;
        self.captured = true;
        if toggle {
            if !self.selection.remove(&hit) {
                self.selection.insert(hit);
            }
            self.anchor = Some(hit);
            return true;
        }
        if add {
            self.selection.insert(hit);
            self.anchor = Some(hit);
            return true;
        }
        if !self.selection.contains(&hit) {
            self.selection.clear();
            self.selection.insert(hit);
        }
        self.anchor = Some(hit);
        self.drag = Some(CanvasDrag { start: world, offset: (0.0, 0.0), moved: false });
        true
    }

    /// Updates the drag with the current world point; `threshold` (world units) separates a click
    /// from a move.
    pub fn canvas_drag(&mut self, world: (f32, f32), threshold: f32) {
        if let Some(drag) = &mut self.drag {
            drag.offset = (world.0 - drag.start.0, world.1 - drag.start.1);
            drag.moved |= drag.offset.0.hypot(drag.offset.1) > threshold;
        }
    }

    pub fn cancel_drag(&mut self) {
        self.drag = None;
        self.captured = false;
    }

    /// True while a canvas gesture that started on a placement is held down.
    pub fn is_capturing(&self) -> bool {
        self.captured
    }

    /// Ends a drag: a real move translates the selection as one undoable command (Y from the
    /// terrain, same rules and refusal messages as the keyboard move); a click only selected.
    pub fn canvas_release(&mut self, scenario: &mut ScenarioUi, cx: &SpawnContext) -> bool {
        self.captured = false;
        let Some(drag) = self.drag.take() else { return false };
        if !drag.moved {
            return false;
        }
        self.move_selected(scenario, cx, drag.offset.0, drag.offset.1, false);
        true
    }

    pub fn select_all(&mut self) {
        self.selection = (0..self.session.count()).collect();
        self.anchor = self.selection.iter().next().copied();
    }

    pub fn apply_batch(&mut self, scenario: &mut ScenarioUi, cx: &SpawnContext, team: Option<i32>, angle: Option<f32>) {
        let indices = self.selection();
        if indices.is_empty() || !self.guard(scenario, cx) {
            return;
        }
        match self.mutate(scenario, cx.lang, ("Invalid team or direction for the selection.", "所選物件的隊伍或方向無效。"), |s| {
            s.edit_many(&indices, team, angle)
        }) {
            Ok(()) => self.set_status(
                tr(cx.lang, "Updated team / direction of the selection.", "已更新選取物件的隊伍／方向。"),
                false,
            ),
            Err(e) => self.set_status(e, true),
        }
    }

    fn open_edit_dialog(&mut self, cx: &SpawnContext) {
        let selected = self.selection();
        if selected.len() != 1 {
            return;
        }
        let index = selected[0];
        let Some(item) = self.session.get(index) else { return };
        let (min_team, max_team) = team_range(&item.obj_type);
        self.edit_dialog = Some(EditDialog {
            index,
            team: item.team.clamp(min_team, max_team),
            x: item.world_x,
            y: item.world_y,
            z: item.world_z,
            angle: normalize_angle(item.angle.round_ties_even() as i32),
            count: if takes_unit_count(&item.obj_type) { item.unit_count.clamp(1, 20) } else { 1 },
            figure: item.obj_type.category == SdlObjectCategory::Figure,
            animal: item.obj_type.is_animal(),
            native: item.raw_rotation.is_some() || item.native_source_identity.is_some(),
            min_team,
            max_team,
        });
        let _ = cx;
    }

    fn open_batch_dialog(&mut self) {
        let indices = self.selection();
        if indices.is_empty() {
            return;
        }
        let includes_units = indices
            .iter()
            .filter_map(|&i| self.session.get(i))
            .any(|o| o.obj_type.category == SdlObjectCategory::Figure && !o.obj_type.is_animal());
        self.batch_dialog = Some(BatchDialog {
            count: indices.len(),
            includes_units,
            apply_team: false,
            team: 0,
            apply_angle: false,
            angle: 0,
        });
    }

    /// Applies the edit dialog (C# `PlacedObjectEditDialog.OnOk` + `Edit`).
    fn commit_edit_dialog(&mut self, scenario: &mut ScenarioUi, cx: &SpawnContext, dialog: &EditDialog) -> bool {
        if !self.guard(scenario, cx) {
            return false;
        }
        let lang = cx.lang;
        let Some(before) = self.session.get(dialog.index) else { return false };
        let squad = takes_unit_count(&before.obj_type);
        if squad {
            if !(0..=7).contains(&dialog.team) {
                self.set_status(tr(lang, "Units need a team between 0 and 7.", "人物與部隊的隊伍必須介於 0 與 7。"), true);
                return false;
            }
            if !(1..=20).contains(&dialog.count) {
                self.set_status(tr(lang, "Unit count must be between 1 and 20.", "部隊人數必須介於 1 與 20。"), true);
                return false;
            }
        } else if !(-1..=15).contains(&dialog.team) {
            self.set_status(tr(lang, "Team must be between -1 and 15.", "隊伍必須介於 -1 與 15。"), true);
            return false;
        }
        // Native group objects only translate: keep their own team / direction.
        let (team, angle) = if dialog.native { (before.team, before.angle) } else { (dialog.team, dialog.angle as f32) };
        let count = if squad { dialog.count } else { 0 };
        let name = object_display_name(&before.obj_type, cx.english());
        let result = self.mutate(scenario, lang, ("Invalid values for this object.", "此物件的數值無效。"), |s| {
            s.edit_values(dialog.index, team, dialog.x, dialog.y, dialog.z, angle, count)
        });
        match result {
            Ok(()) => {
                self.set_status(if cx.english() { format!("Updated {name}.") } else { format!("已更新 {name}。") }, false);
                true
            }
            Err(e) => {
                self.set_status(e, true);
                false
            }
        }
    }

    pub fn undo(&mut self, scenario: &mut ScenarioUi, cx: &SpawnContext) -> bool {
        if !self.guard(scenario, cx) || !self.session.undo() {
            return false;
        }
        if let Err(e) = self.push_to_scenario(scenario) {
            self.session.redo();
            self.set_status(e, true);
            return false;
        }
        self.set_status(tr(cx.lang, "Undo performed.", "已復原。"), false);
        true
    }

    pub fn redo(&mut self, scenario: &mut ScenarioUi, cx: &SpawnContext) -> bool {
        if !self.guard(scenario, cx) || !self.session.redo() {
            return false;
        }
        if let Err(e) = self.push_to_scenario(scenario) {
            self.session.undo();
            self.set_status(e, true);
            return false;
        }
        self.set_status(tr(cx.lang, "Redo performed.", "已重做。"), false);
        true
    }

    /// C# `HandlePlacementWorkflowKey`: shortcuts act on this panel while it is active.
    pub fn handle_shortcut(&mut self, action: ShortcutAction, scenario: &mut ScenarioUi, cx: &SpawnContext) {
        match action {
            ShortcutAction::SelectAll => self.select_all(),
            ShortcutAction::Delete => self.delete_selected(scenario, cx),
            ShortcutAction::Duplicate => self.duplicate_selected(scenario, cx),
            ShortcutAction::Nudge { dx, dz } => self.move_selected(scenario, cx, dx, dz, false),
            ShortcutAction::Save => {}
        }
    }

    // ---- canvas overlay ------------------------------------------------------------------

    /// Draws placed spawns over the 2D map: team coloured dots, squares for DATA-bound ones,
    /// white ring on selected, direction tick.
    pub fn draw_overlay(&self, painter: &egui::Painter, map_rect: Rect) {
        if !self.open && self.session.is_empty() {
            return;
        }
        let to_screen = |x: f32, z: f32| {
            Pos2::new(
                map_rect.min.x + x / WORLD_SIZE * map_rect.width(),
                map_rect.min.y + z / WORLD_SIZE * map_rect.height(),
            )
        };
        let (ox, oz) = self.drag_offset().unwrap_or((0.0, 0.0));
        for (index, item) in self.session.objects().iter().enumerate() {
            let selected = self.selection.contains(&index);
            let (dx, dz) = if selected { (ox, oz) } else { (0.0, 0.0) };
            let pos = to_screen(item.world_x + dx, item.world_z + dz);
            let color = team_color(item.team);
            if self.bound.contains(&item.scenario_id) {
                let half = 3.5;
                painter.rect_filled(Rect::from_center_size(pos, egui::vec2(half * 2.0, half * 2.0)), 0.0, color);
            } else {
                painter.circle_filled(pos, 3.5, color);
            }
            let radians = (item.angle as f64).to_radians() as f32;
            // Compass angle 0 points to -Z (map up), like the C# tile preview.
            let tip = pos + egui::vec2(radians.sin(), -radians.cos()) * 8.0;
            painter.line_segment([pos, tip], Stroke::new(1.0_f32, color));
            if selected {
                painter.circle_stroke(pos, 7.0, Stroke::new(1.5_f32, Color32::WHITE));
            }
        }
    }

    /// Placement ghost under the cursor while in place mode.
    pub fn draw_ghost(&self, painter: &egui::Painter, pointer: Pos2) {
        if self.place_mode() && self.selected_type.is_some() {
            let color = team_color(self.team);
            painter.circle_stroke(pointer, 6.0, Stroke::new(1.5_f32, color));
            let radians = (self.angle as f32).to_radians();
            painter.line_segment(
                [pointer, pointer + egui::vec2(radians.sin(), -radians.cos()) * 12.0],
                Stroke::new(1.5_f32, color),
            );
        }
    }

    // ---- window ---------------------------------------------------------------------------

    fn visible_types(&self) -> Vec<&SdlObjectType> {
        self.catalog
            .iter()
            .filter(|t| self.category_filter.map_or(true, |c| t.category == c))
            .filter(|t| self.tribe_filter.as_ref().map_or(true, |tr| &t.tribe == tr))
            .filter(|t| matches_search(t, &self.search))
            .collect()
    }

    fn placed_row_text(&self, item: &SdlPlacedObject, english: bool, locked: bool) -> String {
        let mut name = object_display_name(&item.obj_type, english);
        if item.unit_count > 1 {
            name.push_str(&format!(" ×{}", item.unit_count));
        }
        format!(
            "{}{}   T{}   ({},{})   {}°",
            if locked { "[DATA] " } else { "" },
            name,
            item.team,
            (item.world_x / 256.0) as i32,
            (item.world_z / 256.0) as i32,
            item.angle as i32
        )
    }

    pub fn show(&mut self, ctx: &egui::Context, scenario: &mut ScenarioUi, cx: &SpawnContext, map: Option<&Path>) {
        self.sync_with_document(scenario, map);
        if let Some(anchor) = scenario.canvas.layout_anchor.take() {
            if let Some(layout) = self.pending_layout.take() {
                if self.guard(scenario, cx) {
                    let remap = self.layout_remap.then_some((self.layout_from, self.layout_to));
                    match self.import_layout(scenario, &layout, anchor, remap, cx.lang) {
                        Ok((count, clamped)) => self.set_status(if cx.english() {
                            format!("Imported {count} spawns; {clamped} spawns clamped. Save to write changes.")
                        } else { format!("已匯入 {count} 個生成物件；{clamped} 個座標已限制於地圖範圍。請存檔以寫入變更。") }, false),
                        Err(e) => self.set_status(e, true),
                    }
                }
            }
        } else if !matches!(scenario.canvas.pick, Some(crate::scenario_canvas::PickTarget::SpawnLayout)) {
            self.pending_layout = None;
        }
        if self.pending_layout.is_none() && matches!(scenario.canvas.pick, Some(crate::scenario_canvas::PickTarget::SpawnLayout)) {
            scenario.canvas.pick = None;
        }
        self.ensure_sprite_catalog(map);
        if !self.open {
            self.place_mode = false;
            return;
        }
        let en = cx.english();
        let lang = cx.lang;
        let mut open = true;
        let mut clicked: Option<RowClick> = None;
        let mut action_edit = false;
        let mut action_batch = false;
        let mut action_duplicate = false;
        let mut action_delete = false;
        let mut action_snap = false;
        let mut action_undo = false;
        let mut action_redo = false;
        let mut rotate: Option<i32> = None;
        let mut hovered_palette = false;
        let mut hovered_list = false;
        let text_focus_before = ctx.wants_keyboard_input();
        let title = tr(lang, "Place Script Objects (Spawns)", "放置腳本物件 (Spawns)");
        let response = egui::Window::new(title)
            .open(&mut open)
            .default_width(380.0)
            .default_height(640.0)
            .show(ctx, |ui| {
                if !scenario.is_loaded() {
                    ui.colored_label(
                        Color32::from_rgb(255, 180, 50),
                        tr(
                            lang,
                            "No scenario loaded: the map needs SCRIPT/ak_level.bci (load it in the Scenario panel).",
                            "尚未載入劇本：地圖需有 SCRIPT/ak_level.bci（請在劇本面板載入）。",
                        ),
                    );
                }
                ui.small(tr(
                    lang,
                    "Pick a type, enable \"Place on map\", then click the 2D map. Buildings on teams 0-8 become prebuilt DATA objects on Save (complete at game start; no template = construction site); other objects are script-created. Click or drag a marker on the 2D map to select / move it. Units: team 0-7, 1-20 soldiers; team 0 is the player. R / Shift+R turns the next placement or the selection by 45 degrees. List: Ctrl+A select all, Ctrl+arrows move one tile (Shift: 64 units), Ctrl+D duplicate, Delete remove.",
                    "選類型後啟用「在地圖上放置」並點擊 2D 地圖。隊伍 0-8 的建築於儲存時寫成預建 DATA 物件（開局即完工；找不到範本則為工地），其餘為腳本生成物件。可在 2D 地圖上點選或拖曳標記來選取／移動。人物以部隊建立，隊伍 0-7、人數 1-20；隊伍 0 為玩家。R／Shift+R 調整下次放置方向或旋轉選取物件 ±45°。清單：Ctrl+A 全選、Ctrl+方向鍵移動一格（Shift 為 64 單位）、Ctrl+D 複製、Delete 刪除。",
                ));
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui.button(tr(lang, "Export script layout", "匯出腳本配置")).clicked() {
                        let ids = self.selection.iter().filter_map(|&i| self.session.get(i).map(|p| p.scenario_id)).collect();
                        let (layout, skipped) = crate::spawn_layout::SpawnLayout::capture(scenario.document(), &ids);
                        let result = layout.serialize().and_then(|json| {
                            let path = (self.layout_picker)(&crate::layout_io::LayoutDialogKind::Save {
                                suggested_name: "spawns.arm-spawns.json".into() });
                            if let Some(path) = path {
                                crate::layout_io::write_json_atomic(&path, &json)?;
                                Ok(true)
                            } else { Ok(false) }
                        });
                        match result {
                            Ok(true) => self.set_status(if en { format!("Export complete; skipped {skipped} bound/prebuilt spawns (DATA dependencies).") }
                                else { format!("匯出完成；略過 {skipped} 個綁定／預建物件（依賴 DATA）。") }, false),
                            Ok(false) => self.set_status(tr(lang, "Export cancelled.", "已取消匯出。"), false),
                            Err(e) => self.set_status(format!("{e}; skipped / 略過 {skipped}: bound/prebuilt DATA / 綁定或預建 DATA"), true),
                        }
                    }
                    if ui.add_enabled(!cx.read_only && scenario.is_loaded(), egui::Button::new(tr(lang, "Import script layout", "匯入腳本配置"))).clicked() {
                        if let Some(path) = (self.layout_picker)(&crate::layout_io::LayoutDialogKind::Open) {
                            let result: Result<crate::spawn_layout::SpawnLayout, String> = (|| {
                                let text = crate::layout_io::read_json_file(&path).map_err(|e| format!("Read failed / 讀取失敗：{e}"))?;
                                let layout = crate::spawn_layout::SpawnLayout::deserialize(&text)?;
                                layout.plan(scenario.known_aliases(), (8192.,8192.), None)?;
                                Ok(layout)
                            })();
                            match result {
                                Ok(layout) => {
                                    self.pending_layout = Some(layout);
                                    self.place_mode = false;
                                    scenario.canvas.layout_anchor = None;
                                    scenario.canvas.pick = Some(crate::scenario_canvas::PickTarget::SpawnLayout);
                                    self.set_status(tr(lang, "Click the 2D map for the layout anchor; Esc/right-click cancels.", "點擊 2D 地圖選取配置錨點；Esc／右鍵取消。"), false);
                                }
                                Err(e) => self.set_status(e, true),
                            }
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.layout_remap, tr(lang, "Remap team", "重新對應隊伍"));
                    ui.label(tr(lang, "Source", "來源"));
                    ui.add(egui::DragValue::new(&mut self.layout_from).clamp_range(-1..=15));
                    ui.label(tr(lang, "Target", "目標"));
                    ui.add(egui::DragValue::new(&mut self.layout_to).clamp_range(-1..=15));
                });
                // Palette filters
                ui.horizontal_wrapped(|ui| {
                    ui.label(tr(lang, "Category", "類別"));
                    let cat_label = self
                        .category_filter
                        .map_or(tr(lang, "All", "全部").to_string(), |c| category_text(c, en).to_string());
                    egui::ComboBox::from_id_source("spawn_category").selected_text(cat_label).show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.category_filter, None, tr(lang, "All", "全部"));
                        for category in [
                            SdlObjectCategory::Building,
                            SdlObjectCategory::UnitGroup,
                            SdlObjectCategory::Figure,
                            SdlObjectCategory::Effect,
                            SdlObjectCategory::Other,
                        ] {
                            if self.catalog.iter().any(|t| t.category == category) {
                                ui.selectable_value(&mut self.category_filter, Some(category), category_text(category, en));
                            }
                        }
                    });
                    ui.label(tr(lang, "Tribe", "部族"));
                    let tribe_label = self
                        .tribe_filter
                        .as_deref()
                        .map_or(tr(lang, "All", "全部").to_string(), |t| tribe_text(t, en).to_string());
                    egui::ComboBox::from_id_source("spawn_tribe").selected_text(tribe_label).show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.tribe_filter, None, tr(lang, "All", "全部"));
                        for code in ["Ger", "Hun", "Kel", "Rom", ""] {
                            if self.catalog.iter().any(|t| t.tribe == code) {
                                ui.selectable_value(&mut self.tribe_filter, Some(code.to_string()), tribe_text(code, en));
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Search", "搜尋"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search).hint_text(tr(
                            lang,
                            "Name or alias (space = AND)",
                            "名稱／別名（空白分隔多條件）",
                        )),
                    );
                });
                if self.catalog.is_empty() {
                    ui.colored_label(
                        Color32::from_rgb(255, 180, 50),
                        tr(
                            lang,
                            "No object alias catalog (SYSTEM/CLAK/cl_scint.ini) found for this map's game folder.",
                            "此地圖的遊戲資料夾找不到物件別名目錄 (SYSTEM/CLAK/cl_scint.ini)。",
                        ),
                    );
                }
                let types: Vec<SdlObjectType> = self.visible_types().into_iter().cloned().collect();
                let palette = egui::ScrollArea::vertical().id_source("spawn_palette").max_height(170.0).show_rows(ui, 32.0, types.len(), |ui, rows| {
                    for index in rows {
                        let object_type = &types[index];
                        let selected = self.selected_type.as_deref() == Some(object_type.name_def.as_str());
                        let label = format!(
                            "{}  ({})",
                            object_display_name(object_type, en),
                            crate::spawn_placement::alias_of(object_type)
                        );
                        ui.horizontal(|ui| {
                            if let Some(texture) = self.palette_texture(ui.ctx(), object_type, self.team) {
                                let image = egui::Image::new((texture.id(), egui::vec2(28.0, 28.0)));
                                ui.add(image);
                            } else {
                                ui.add_space(28.0);
                            }
                            if ui.selectable_label(selected, label).clicked() {
                                self.selected_type = Some(object_type.name_def.clone());
                                let (min, max) = team_range(object_type);
                                self.team = self.team.clamp(min, max);
                            }
                        });
                    }
                });
                hovered_palette = ui.rect_contains_pointer(palette.inner_rect);
                if self.selected_type.is_none() {
                    if let Some(first) = types.first() {
                        self.selected_type = Some(first.name_def.clone());
                    }
                }
                // Placement values with per-category limits
                let (min_team, max_team, takes_count) = self
                    .selected_catalog_type()
                    .map_or((-1, 15, false), |t| {
                        let (a, b) = team_range(t);
                        (a, b, takes_unit_count(t))
                    });
                self.team = self.team.clamp(min_team, max_team);
                egui::Grid::new("spawn_values").num_columns(2).show(ui, |ui| {
                    ui.label(tr(lang, "Team", "隊伍"));
                    ui.add(egui::DragValue::new(&mut self.team).clamp_range(min_team..=max_team));
                    ui.end_row();
                    ui.label(tr(lang, "Soldiers", "人數"));
                    ui.add_enabled(takes_count, egui::DragValue::new(&mut self.count).clamp_range(1..=20));
                    ui.end_row();
                    ui.label(tr(lang, "Angle", "角度"));
                    ui.add(egui::DragValue::new(&mut self.angle).clamp_range(0..=359).speed(1.0));
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.place_mode, tr(lang, "Place on map", "在地圖上放置"));
                    if ui.small_button("-45").clicked() {
                        self.rotate_next_placement(-45);
                    }
                    if ui.small_button("+45").clicked() {
                        self.rotate_next_placement(45);
                    }
                });
                if ui
                    .button(tr(lang, "Starter kit: main house + 4 axemen…", "起始配置：主屋＋4 名斧兵…"))
                    .on_hover_text(tr(
                        lang,
                        "For a custom blank map. Choose team and anchor; staged until Save, with one Undo.",
                        "用於自製空白地圖。指定隊伍與錨點；儲存前不寫檔，可單步復原。",
                    ))
                    .clicked()
                {
                    self.open_starter_dialog();
                }
                ui.separator();
                // Placed list
                ui.horizontal(|ui| {
                    ui.strong(format!("{} ({})", tr(lang, "Placed objects", "已放置物件"), self.session.count()));
                    if self.session.is_dirty() {
                        ui.colored_label(Color32::YELLOW, tr(lang, "[unsaved]", "[未儲存]"));
                    }
                });
                let list = egui::ScrollArea::vertical().id_source("spawn_placed").max_height(200.0).show(ui, |ui| {
                    let modifiers = ui.input(|i| i.modifiers);
                    for (index, item) in self.session.objects().iter().enumerate() {
                        let locked = self.bound.contains(&item.scenario_id);
                        let selected = self.selection.contains(&index);
                        let row = ui
                            .selectable_label(selected, self.placed_row_text(item, en, locked))
                            .on_hover_text(format!("ID {}", item.scenario_id));
                        if row.clicked() {
                            clicked = Some(if modifiers.shift {
                                RowClick::Range(index)
                            } else if modifiers.ctrl || modifiers.command {
                                RowClick::Toggle(index)
                            } else {
                                RowClick::Single(index)
                            });
                        }
                    }
                });
                hovered_list = ui.rect_contains_pointer(list.inner_rect);
                let count = self.selection.len();
                ui.horizontal_wrapped(|ui| {
                    action_edit = ui.add_enabled(count == 1, egui::Button::new(tr(lang, "Edit...", "編輯..."))).clicked();
                    action_duplicate = ui.add_enabled(count > 0, egui::Button::new(tr(lang, "Duplicate", "複製選取"))).clicked();
                    action_batch = ui
                        .add_enabled(count > 0, egui::Button::new(tr(lang, "Team / Direction...", "隊伍／方向…")))
                        .clicked();
                    action_delete = ui
                        .add_enabled(count > 0, egui::Button::new(tr(lang, "Delete Selected", "刪除選取的物件")))
                        .clicked();
                    action_snap = ui
                        .add_enabled(count > 0, egui::Button::new(tr(lang, "Align selection to tile center", "整批對齊格心")))
                        .clicked();
                });
                ui.horizontal(|ui| {
                    if ui.add_enabled(count > 0, egui::Button::new("-45")).clicked() {
                        rotate = Some(-45);
                    }
                    if ui.add_enabled(count > 0, egui::Button::new("+45")).clicked() {
                        rotate = Some(45);
                    }
                    action_undo = ui
                        .add_enabled(self.session.can_undo(), egui::Button::new(tr(lang, "Undo", "復原")))
                        .clicked();
                    action_redo = ui
                        .add_enabled(self.session.can_redo(), egui::Button::new(tr(lang, "Redo", "重做")))
                        .clicked();
                });
                if !self.status.is_empty() {
                    let color = if self.status_error { Color32::from_rgb(240, 80, 80) } else { Color32::from_rgb(100, 200, 100) };
                    ui.colored_label(color, &self.status);
                }
            });
        // Which part of the editor owns keyboard shortcuts.
        if let Some(window) = &response {
            if ctx.input(|i| i.pointer.any_pressed()) {
                let over = ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| window.response.rect.contains(p));
                if over {
                    self.active = true;
                } else if !self.place_mode {
                    self.active = false;
                }
            }
        }
        if !open {
            self.open = false;
            self.place_mode = false;
            self.active = false;
        }

        // R / Shift+R (type list: next direction; placed list: rotate selection)
        if self.active && !text_focus_before {
            let rotation = ctx.input(|i| {
                if i.key_pressed(egui::Key::R) && !i.modifiers.ctrl && !i.modifiers.alt && !i.modifiers.command {
                    Some(if i.modifiers.shift { -45 } else { 45 })
                } else {
                    None
                }
            });
            if let Some(degrees) = rotation {
                if hovered_list || (!hovered_palette && self.has_selection()) {
                    rotate = Some(degrees);
                } else {
                    self.rotate_next_placement(degrees);
                }
            }
        }

        // Apply row selection
        if let Some(click) = clicked {
            match click {
                RowClick::Single(i) => {
                    self.selection.clear();
                    self.selection.insert(i);
                    self.anchor = Some(i);
                }
                RowClick::Toggle(i) => {
                    if !self.selection.remove(&i) {
                        self.selection.insert(i);
                    }
                    self.anchor = Some(i);
                }
                RowClick::Range(i) => {
                    let from = self.anchor.unwrap_or(i);
                    self.selection = (from.min(i)..=from.max(i)).collect();
                }
            }
            self.active = true;
        }

        // Actions
        if action_edit {
            self.open_edit_dialog(cx);
        }
        if action_batch {
            self.open_batch_dialog();
        }
        if action_duplicate {
            self.duplicate_selected(scenario, cx);
        }
        if action_delete {
            self.delete_selected(scenario, cx);
        }
        if action_snap {
            self.move_selected(scenario, cx, 0.0, 0.0, true);
        }
        if let Some(degrees) = rotate {
            self.rotate_selected(scenario, cx, degrees);
        }
        if action_undo {
            self.undo(scenario, cx);
        }
        if action_redo {
            self.redo(scenario, cx);
        }
        self.show_dialogs(ctx, scenario, cx);
    }

    fn show_dialogs(&mut self, ctx: &egui::Context, scenario: &mut ScenarioUi, cx: &SpawnContext) {
        let lang = cx.lang;
        if let Some(mut dialog) = self.edit_dialog.take() {
            let mut keep = true;
            let mut apply = false;
            let name = self.session.get(dialog.index).map(|o| {
                format!("{} ({})", object_display_name(&o.obj_type, cx.english()), o.obj_type.name_def)
            });
            let id = self.session.get(dialog.index).map(|o| o.scenario_id);
            egui::Window::new(tr(lang, "Edit Placed Object", "編輯放置物件"))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(name.unwrap_or_default());
                    ui.small(format!("{}: {}", tr(lang, "Scenario ID", "識別碼"), id.map(|i| i.to_string()).unwrap_or_default()));
                    egui::Grid::new("spawn_edit_grid").num_columns(2).show(ui, |ui| {
                        ui.label(tr(lang, "Team", "隊伍"));
                        ui.add_enabled(
                            !dialog.native,
                            egui::DragValue::new(&mut dialog.team).clamp_range(dialog.min_team..=dialog.max_team),
                        );
                        ui.end_row();
                        ui.label(tr(lang, "Position X", "世界 X"));
                        ui.add(egui::DragValue::new(&mut dialog.x).clamp_range(0.0..=16383.0));
                        ui.end_row();
                        ui.label(tr(lang, "Position Z", "世界 Z"));
                        ui.add(egui::DragValue::new(&mut dialog.z).clamp_range(0.0..=16383.0));
                        ui.end_row();
                        ui.label(tr(lang, "Height Y", "高度 Y"));
                        ui.add(egui::DragValue::new(&mut dialog.y).clamp_range(-1000.0..=10000.0));
                        ui.end_row();
                        ui.label(tr(lang, "Angle", "角度"));
                        ui.add_enabled(!dialog.native, egui::DragValue::new(&mut dialog.angle).clamp_range(0..=359));
                        ui.end_row();
                        ui.label(tr(lang, "Soldiers", "人數"));
                        ui.add_enabled(
                            dialog.figure && !dialog.animal && !dialog.native,
                            egui::DragValue::new(&mut dialog.count).clamp_range(1..=20),
                        );
                        ui.end_row();
                    });
                    ui.small(if dialog.figure && !dialog.animal {
                        tr(lang, "Units: team 0-7, count 1-20 (team 0 = player).", "人物／部隊：隊伍 0-7，人數 1-20（隊伍 0 為玩家）。")
                    } else {
                        tr(lang, "Team -1 to 15 (8 = neutral); count does not apply.", "隊伍 -1 至 15（8 為中立）；人數不適用。")
                    });
                    ui.horizontal(|ui| {
                        if ui.button(tr(lang, "OK", "確定")).clicked() {
                            apply = true;
                        }
                        if ui.button(tr(lang, "Cancel", "取消")).clicked() {
                            keep = false;
                        }
                    });
                });
            if apply {
                if self.commit_edit_dialog(scenario, cx, &dialog) {
                    keep = false;
                }
            }
            if keep {
                self.edit_dialog = Some(dialog);
            }
        }
        if let Some(mut dialog) = self.starter_dialog.take() {
            let mut keep = true;
            let mut place = false;
            egui::Window::new(tr(lang, "Blank map starter kit", "空白地圖起始配置"))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(tr(
                        lang,
                        "1 Germanic main house + 4 individual axemen. Anchor uses map tiles. Team 0 is the player. Save writes the kit; Undo removes it as one action.",
                        "1 棟日耳曼主屋＋4 名獨立斧兵。錨點使用圖格座標，隊伍 0 為玩家。儲存才寫入，可整批單步復原。",
                    ));
                    egui::Grid::new("spawn_starter_grid").num_columns(2).show(ui, |ui| {
                        ui.label(tr(lang, "Team", "隊伍"));
                        ui.add(egui::DragValue::new(&mut dialog.team).clamp_range(0..=7));
                        ui.end_row();
                        ui.label(tr(lang, "Anchor X", "錨點 X"));
                        ui.add(egui::DragValue::new(&mut dialog.x).clamp_range(0.0..=63.99).speed(0.1).fixed_decimals(2));
                        ui.end_row();
                        ui.label(tr(lang, "Anchor Z", "錨點 Z"));
                        ui.add(egui::DragValue::new(&mut dialog.z).clamp_range(0.0..=63.99).speed(0.1).fixed_decimals(2));
                        ui.end_row();
                    });
                    ui.horizontal(|ui| {
                        if ui.button(tr(lang, "Place", "放置")).clicked() {
                            place = true;
                        }
                        if ui.button(tr(lang, "Cancel", "取消")).clicked() {
                            keep = false;
                        }
                    });
                });
            if place {
                self.starter_request = Some((dialog.team, dialog.x * 256.0, dialog.z * 256.0));
                keep = false;
            }
            if keep {
                self.starter_dialog = Some(dialog);
            }
        }
        if let Some(mut dialog) = self.batch_dialog.take() {
            let mut keep = true;
            let mut apply = false;
            let (min_team, max_team) = if dialog.includes_units { (0, 7) } else { (-1, 15) };
            egui::Window::new(tr(lang, "Set selected teams / directions", "批次設定隊伍／方向"))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(if cx.english() {
                        format!(
                            "{} selected objects. Only checked settings change; position, count and event identity are preserved.",
                            dialog.count
                        )
                    } else {
                        format!("已選取 {} 個物件。只修改勾選的設定；位置、人數及事件目標身份保留。", dialog.count)
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut dialog.apply_team, tr(lang, "Change team", "設定隊伍"));
                        ui.add_enabled(
                            dialog.apply_team,
                            egui::DragValue::new(&mut dialog.team).clamp_range(min_team..=max_team),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut dialog.apply_angle, tr(lang, "Change direction", "設定方向"));
                        ui.add_enabled(dialog.apply_angle, egui::DragValue::new(&mut dialog.angle).clamp_range(0..=359));
                    });
                    ui.horizontal(|ui| {
                        let any = dialog.apply_team || dialog.apply_angle;
                        if ui.add_enabled(any, egui::Button::new(tr(lang, "Apply", "套用"))).clicked() {
                            apply = true;
                        }
                        if ui.button(tr(lang, "Cancel", "取消")).clicked() {
                            keep = false;
                        }
                    });
                });
            if apply {
                let team = dialog.apply_team.then_some(dialog.team);
                let angle = dialog.apply_angle.then_some(dialog.angle as f32);
                self.apply_batch(scenario, cx, team, angle);
                keep = false;
            }
            if keep {
                self.batch_dialog = Some(dialog);
            }
        }
    }

    /// Test/automation access to the pending dialogs.
    pub fn has_open_dialog(&self) -> bool {
        self.edit_dialog.is_some() || self.batch_dialog.is_some() || self.starter_dialog.is_some()
    }

    // ---- starter kit on an existing blank map (C# MapEditorForm.StarterKit.cs) -----------------

    /// Opens the starter kit dialog; the team defaults to the current placement team (0..=7).
    pub fn open_starter_dialog(&mut self) {
        self.starter_dialog = Some(StarterDialog { team: self.team.clamp(0, 7), x: 32.0, z: 32.0 });
    }

    /// The dialog's confirmed `(team, anchor X, anchor Z)` in world units, once; the editor applies
    /// it because only it owns the terrain data the kit is validated against.
    pub fn take_starter_request(&mut self) -> Option<(i32, f32, f32)> {
        self.starter_request.take()
    }

    /// Staged kit and the ids of its placements that are still alive (kit originals and their
    /// copies; C# `PendingStarterKitIds`). `None` once every kit placement was removed.
    pub fn pending_starter_kit(&self) -> Option<(&BlankMapStarterKit, HashSet<Uuid>)> {
        let kit = self.pending_kit.as_ref()?;
        let originals: HashSet<Uuid> = kit.spawns.iter().map(|s| s.id).collect();
        let ids: HashSet<Uuid> = self
            .session
            .objects()
            .iter()
            .filter(|o| originals.contains(&o.scenario_id) || originals.contains(&o.copy_source_scenario_id))
            .map(|o| o.scenario_id)
            .collect();
        (!ids.is_empty()).then_some((kit, ids))
    }

    /// C# `ApplyStarterKit`: adds 1 main house + 4 individual axemen to a blank custom map as ONE
    /// undo step. Everything is planned on detached copies first; any failure changes nothing.
    /// The DATA family import happens at Safe Save ([`Self::pending_starter_kit`]).
    pub fn apply_starter_kit(
        &mut self,
        scenario: &mut ScenarioUi,
        cx: &SpawnContext,
        map: &Path,
        terrain: &StarterKitTerrain,
        team: i32,
        x: f32,
        z: f32,
    ) -> bool {
        if !self.guard(scenario, cx) {
            return false;
        }
        match self.try_starter_kit(scenario, cx, map, terrain, team, x, z) {
            Ok(()) => {
                self.set_status(
                    tr(
                        cx.lang,
                        "Staged 1 main house + 4 individual axemen; Save to write. Single Undo/Redo available.",
                        "已暫存 1 棟主屋＋4 名獨立斧兵；按儲存才寫入，可單步復原／重做。",
                    ),
                    false,
                );
                true
            }
            Err(e) => {
                self.set_status(e, true);
                false
            }
        }
    }

    fn try_starter_kit(
        &mut self,
        scenario: &mut ScenarioUi,
        cx: &SpawnContext,
        map: &Path,
        terrain: &StarterKitTerrain,
        team: i32,
        x: f32,
        z: f32,
    ) -> Result<(), String> {
        let lang = cx.lang;
        self.sync_with_document(scenario, Some(map));
        if !self.session.is_empty() || !scenario.document().spawns.is_empty() {
            return Err("請先復原其他待存放置，再加入起始配置。".into());
        }
        let game = map
            .parent()
            .and_then(Path::parent)
            .ok_or("地圖資料夾不在遊戲 MAPS 目錄內。")?;
        let previous = scenario.saved_document()?;
        let stage = stage_starter_kit(game, map, &previous, team, x, z, terrain)?;
        let planned = stage
            .spawns
            .iter()
            .map(|spawn| {
                spawn_to_placed(spawn, &self.catalog)
                    .ok_or_else(|| format!("起始配置類型 {} 不在放置目錄內。", spawn.alias))
            })
            .collect::<Result<Vec<_>, _>>()?;
        // add_many prevalidates identities and bounds before its single history command; a
        // document rejection rolls that command back (nothing staged).
        self.mutate(scenario, lang, ("The starter kit does not fit on the map.", "起始配置超出地圖範圍。"), |s| {
            s.add_many(&planned).map(|_| ())
        })?;
        self.pending_kit = Some(stage.kit);
        self.selection = (0..self.session.count()).collect();
        self.anchor = Some(0);
        Ok(())
    }

    /// Alias of the catalog type used by a spawn (tests).
    pub fn catalog_type_for(&self, alias: &str) -> Option<&SdlObjectType> {
        type_for_alias(&self.catalog, alias)
    }
}

#[cfg(test)]
mod palette_thumbnail_tests {
    use super::*;

    #[test]
    fn spawn_layout_import_is_one_undo_and_unknown_alias_leaves_document_unchanged() {
        let root = std::env::temp_dir().join(format!("arm_spawn_layout_{}",Uuid::new_v4()));
        let map = root.join("MAPS/layout");
        std::fs::create_dir_all(map.join("SCRIPT")).unwrap();
        std::fs::create_dir_all(root.join("SYSTEM/CLAK")).unwrap();
        std::fs::write(root.join("SYSTEM/CLAK/cl_scint.ini"),"[ObjDefName]\nhouse=BauRomHaus\n").unwrap();
        std::fs::write(map.join("SCRIPT/ak_level.bci"),against_rome_scenario::BciImage::idle().serialize()).unwrap();
        let mut scenario = ScenarioUi::default(); scenario.load_session(&map).unwrap();
        let mut panel = SpawnPlacementUi::default(); panel.sync_with_document(&scenario,Some(&map));
        let before = scenario.document().clone();
        let spawn = Spawn {alias:"house".into(),x:0.,z:0.,y:3.5,team:2,count:0,angle:405,id:Uuid::new_v4(),..Default::default()};
        let layout = crate::spawn_layout::SpawnLayout(vec![spawn.clone(),Spawn {x:100.,..spawn.clone()}]);
        assert_eq!(panel.import_layout(&mut scenario,&layout,(1000.,1000.),Some((2,4)),AppLanguage::English).unwrap(),(2,0));
        assert_eq!(panel.session.undo_depth(),1);
        assert_eq!(scenario.document().spawns[0].angle,405);
        assert_eq!(scenario.document().spawns[0].y,3.5);
        assert_eq!(scenario.document().spawns[0].team,4);
        assert_ne!(scenario.document().spawns[0].id,scenario.document().spawns[1].id);
        let cx = SpawnContext {lang:AppLanguage::English,heights:None,vertex_size:0,height_step:4.,read_only:false};
        assert!(panel.undo(&mut scenario,&cx)); assert_eq!(*scenario.document(),before);
        assert!(panel.redo(&mut scenario,&cx));
        let before = scenario.document().clone(); let depth = panel.session.undo_depth();
        let invalid = crate::spawn_layout::SpawnLayout(vec![spawn.clone(),Spawn {alias:"missing_a".into(),..spawn.clone()},Spawn {alias:"missing_b".into(),..spawn}]);
        let error = panel.import_layout(&mut scenario,&invalid,(1000.,1000.),None,AppLanguage::English).unwrap_err();
        assert!(error.contains("missing_a") && error.contains("missing_b"));
        assert_eq!(*scenario.document(),before); assert_eq!(panel.session.undo_depth(),depth);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_argb_sprite_converts_to_rgba_color_image() {
        let sprite = NativeSprite::new(1, 1, vec![0x8011_2233], 0, 0, "fixture").unwrap();
        let image = sprite_color_image(&sprite);
        assert_eq!(image.size, [1, 1]);
        assert_eq!(image.pixels[0].to_srgba_unmultiplied(), [0x11, 0x22, 0x33, 0x80]);
    }
}
