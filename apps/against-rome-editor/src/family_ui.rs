//! Object family / saved light / saved particle / particle definition panel.
//!
//! Port of the C# `MapEditorForm.SavedLights`, `SavedParticles`, `ParticleDefinitions`,
//! `FamilyPreview`, `FamilyParticle*` and `NativeFamilies` partials, reduced to what a map designer
//! needs: list and inspect the saved light/particle pools, list the particle base definitions,
//! preview a closed object family (objects, lights, particle children as tables and simple 2D
//! overlay data) and copy a native family with undo.
//!
//! Everything that is not egui is a pure function over `against-rome-data` /
//! `against-rome-placement` types so it can be tested without a window. Pixel-exact particle
//! rendering is out of scope.
//!
//! Data flow: the panel reads the *current* decoded `light`/`particle` payloads from the
//! `ObjectStore` handed in through [`FamilyCtx`] (falling back to `DATA/*.dat` on disk when no store
//! is loaded), so after a copy / undo / redo the tables show the edited pools. Parsing the 2.3 MB
//! particle pool every frame would be wasteful, so rows are cached; call
//! [`FamilyUi::invalidate`] when something else edited the store, or press "Refresh".

use against_rome_data::{
    particle::{ParticlePool, CHILDREN},
    particle_definition_sources::{self, EffectiveDefinitions},
    LightPool,
};
use against_rome_placement::{fixed_displacement, ObjectStore, WorldObject};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

/// World units per terrain tile (matches C# `WorldX / 256f`).
pub const WORLD_UNITS_PER_TILE: f32 = 256.0;
/// 16.16 fixed point scale used by saved particle children.
const FIXED_ONE: f32 = 65536.0;

// ------------------------------------------------------------------------------------------------
// Pure models
// ------------------------------------------------------------------------------------------------

/// One active slot of the saved light pool (`light.dat`).
#[derive(Clone, Debug, PartialEq)]
pub struct LightRow {
    pub slot: u32,
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub effective_radius: f32,
    pub baseline_radius: f32,
    pub type_raw: u16,
    pub owner: i16,
    pub parent: i16,
    pub definition: i16,
}

impl LightRow {
    fn from_snapshot(s: &against_rome_data::LightSnapshot) -> Self {
        Self {
            slot: s.slot(),
            position: s.position(),
            color: s.color(),
            effective_radius: s.effective_radius(),
            baseline_radius: s.baseline_radius(),
            type_raw: s.type_raw(),
            owner: s.owner(),
            parent: s.parent(),
            definition: s.definition(),
        }
    }
    /// Display color, clamped to 0..=1 (saved values may exceed 1 for over-bright lights).
    pub fn display_color(&self) -> Color32 {
        let c = |v: f32| (if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 } * 255.0).round() as u8;
        Color32::from_rgb(c(self.color[0]), c(self.color[1]), c(self.color[2]))
    }
}

/// All active lights of a decoded `light.dat` payload (or PFIL file image).
pub fn light_rows_from_bytes(bytes: &[u8]) -> Result<Vec<LightRow>, String> {
    let pool = LightPool::from_file_bytes(bytes).map_err(|e| e.to_string())?;
    Ok(light_rows(&pool))
}

pub fn light_rows(pool: &LightPool) -> Vec<LightRow> {
    pool.active_snapshots().map(|s| LightRow::from_snapshot(&s)).collect()
}

/// One saved child of a particle emitter, position converted from 16.16 fixed point.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleChildRow {
    pub ordinal: u32,
    pub position: [f32; 3],
    pub velocity_raw: [i32; 3],
    pub lifetime_raw: i32,
    /// Light slot this child drives, if any (`-1` in the file means none).
    pub light_link: Option<u16>,
}

/// One active emitter of the saved particle pool (`particle.dat`).
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleRow {
    pub slot: u32,
    pub owner: i16,
    pub active_raw: u16,
    pub count_raw: i32,
    pub age_raw: i32,
    pub children: Vec<ParticleChildRow>,
}

fn particle_row(s: &against_rome_data::particle::ParticleSnapshot) -> ParticleRow {
    let children = (0..CHILDREN as u32)
        .filter_map(|ordinal| s.child(ordinal).ok().map(|c| (ordinal, c)))
        .filter(|(_, c)| c.active_raw() != 0)
        .map(|(ordinal, c)| {
            let p = c.position_fixed();
            ParticleChildRow {
                ordinal,
                position: [p[0] as f32 / FIXED_ONE, p[1] as f32 / FIXED_ONE, p[2] as f32 / FIXED_ONE],
                velocity_raw: c.velocity_raw(),
                lifetime_raw: c.lifetime_raw(),
                light_link: u16::try_from(c.light_link()).ok(),
            }
        })
        .collect();
    ParticleRow {
        slot: s.slot(),
        owner: s.owner(),
        active_raw: s.active_raw(),
        count_raw: s.count_raw(),
        age_raw: s.age_raw(),
        children,
    }
}

pub fn particle_rows(pool: &ParticlePool) -> Vec<ParticleRow> {
    pool.active_slots().filter_map(|s| pool.capture(s)).map(|s| particle_row(&s)).collect()
}

pub fn particle_rows_from_bytes(bytes: &[u8]) -> Result<Vec<ParticleRow>, String> {
    let pool = ParticlePool::from_file_bytes(bytes).map_err(|e| e.to_string())?;
    Ok(particle_rows(&pool))
}

/// One base particle definition row (objdef particle columns).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefinitionRow {
    pub id: i32,
    pub name: String,
    pub active: u8,
    pub animation_length: i32,
    pub alr_id: i16,
    pub parameter_id: i16,
    pub geometry_id: i16,
    pub grafik_id: i16,
    pub mode: Option<i16>,
}

/// Definition rows that have particles (`active != 0`) matching `filter` (case-insensitive substring
/// of the name or the numeric id). An empty filter keeps all active rows; `include_inactive` also
/// lists the placeholder rows.
pub fn definition_rows(defs: &EffectiveDefinitions, filter: &str, include_inactive: bool) -> Vec<DefinitionRow> {
    let needle = filter.trim().to_ascii_lowercase();
    defs.objects
        .values()
        .filter(|d| include_inactive || d.active_native() != 0)
        .filter(|d| {
            needle.is_empty() || d.name.to_ascii_lowercase().contains(&needle) || d.id.to_string() == needle
        })
        .map(|d| DefinitionRow {
            id: d.id,
            name: d.name.clone(),
            active: d.active_native(),
            animation_length: d.animation_length(),
            alr_id: d.alr_id(),
            parameter_id: d.parameter_id(),
            geometry_id: d.geometry_id(),
            grafik_id: d.grafik_id(),
            mode: d.mode_native(),
        })
        .collect()
}

/// A family root candidate: parentless active object with a valid, closed family.
#[derive(Clone, Debug, PartialEq)]
pub struct RootRow {
    pub slot: usize,
    pub uid: u32,
    pub type_id: u16,
    pub team: u16,
    pub position: [f32; 3],
    pub object_count: usize,
    pub light_count: usize,
    pub particle_count: usize,
    pub lager_count: usize,
}

/// Lists every parentless object whose family closure is valid. `only_effects` keeps only roots
/// whose family owns lights or particles.
pub fn family_roots(store: &ObjectStore, only_effects: bool) -> Vec<RootRow> {
    store
        .objects()
        .into_iter()
        .filter(|o| o.parent.is_none() && o.uid != 0)
        .filter_map(|o| {
            let d = store.family_details(o.slot, o.uid).ok()?;
            Some(RootRow {
                slot: o.slot,
                uid: o.uid,
                type_id: o.type_id,
                team: o.team,
                position: o.position,
                object_count: d.object_slots.len(),
                light_count: d.light_slots.len(),
                particle_count: d.particle_slots.len(),
                lager_count: d.lager_slots.len() + d.big_lager_slots.len(),
            })
        })
        .filter(|r| !only_effects || r.light_count > 0 || r.particle_count > 0)
        .collect()
}

/// Closed-family preview: member objects, lights and particle children, all in world units.
#[derive(Clone, Debug, PartialEq)]
pub struct FamilyPreview {
    pub root_slot: usize,
    pub root_uid: u32,
    pub objects: Vec<WorldObject>,
    pub lights: Vec<LightRow>,
    pub particles: Vec<ParticleRow>,
    pub lager_slots: Vec<usize>,
    pub big_lager_slots: Vec<usize>,
}

/// Builds the preview of the family rooted at `slot`/`uid` from the store's current pools.
pub fn build_family_preview(store: &ObjectStore, slot: usize, uid: u32) -> Result<FamilyPreview, String> {
    let d = store.family_details(slot, uid)?;
    let members: BTreeSet<usize> = d.object_slots.iter().copied().collect();
    let objects = store.objects().into_iter().filter(|o| members.contains(&o.slot)).collect();
    let lights = match store.payload("light") {
        Some(bytes) => {
            let pool = LightPool::from_payload(bytes).map_err(|e| e.to_string())?;
            d.light_slots
                .iter()
                .filter_map(|&s| pool.capture(s as u32))
                .map(|s| LightRow::from_snapshot(&s))
                .collect()
        }
        None => Vec::new(),
    };
    let particles = match store.payload("particle") {
        Some(bytes) => {
            let pool = ParticlePool::from_payload(bytes).map_err(|e| e.to_string())?;
            d.particle_slots.iter().filter_map(|&s| pool.capture(s as u32)).map(|s| particle_row(&s)).collect()
        }
        None => Vec::new(),
    };
    Ok(FamilyPreview {
        root_slot: slot,
        root_uid: uid,
        objects,
        lights,
        particles,
        lager_slots: d.lager_slots,
        big_lager_slots: d.big_lager_slots,
    })
}

/// What an overlay marker represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayKind {
    Object,
    Light,
    ParticleChild,
}

/// A 2D marker on the terrain plane (`x`, `z` world units).
#[derive(Clone, Debug, PartialEq)]
pub struct OverlayPoint {
    pub kind: OverlayKind,
    pub slot: usize,
    pub x: f32,
    pub z: f32,
    /// World radius for lights, 0 otherwise.
    pub radius: f32,
    pub color: Color32,
}

/// Flattens a preview into 2D overlay markers, skipping non-finite positions.
pub fn overlay_points(preview: &FamilyPreview) -> Vec<OverlayPoint> {
    let mut out = Vec::new();
    for o in &preview.objects {
        if o.position[0].is_finite() && o.position[2].is_finite() {
            out.push(OverlayPoint {
                kind: OverlayKind::Object,
                slot: o.slot,
                x: o.position[0],
                z: o.position[2],
                radius: 0.0,
                color: Color32::from_rgb(200, 200, 200),
            });
        }
    }
    for l in &preview.lights {
        if l.position[0].is_finite() && l.position[2].is_finite() {
            out.push(OverlayPoint {
                kind: OverlayKind::Light,
                slot: l.slot as usize,
                x: l.position[0],
                z: l.position[2],
                radius: if l.effective_radius.is_finite() { l.effective_radius.max(0.0) } else { 0.0 },
                color: l.display_color(),
            });
        }
    }
    for p in &preview.particles {
        for c in &p.children {
            out.push(OverlayPoint {
                kind: OverlayKind::ParticleChild,
                slot: p.slot as usize,
                x: c.position[0],
                z: c.position[2],
                radius: 0.0,
                color: Color32::from_rgb(255, 160, 60),
            });
        }
    }
    out
}

/// Bounding box `(min_x, min_z, max_x, max_z)` of the markers including light radii, or `None`.
pub fn overlay_bounds(points: &[OverlayPoint]) -> Option<(f32, f32, f32, f32)> {
    points.iter().fold(None, |acc, p| {
        let (x0, z0, x1, z1) = (p.x - p.radius, p.z - p.radius, p.x + p.radius, p.z + p.radius);
        Some(match acc {
            None => (x0, z0, x1, z1),
            Some((a, b, c, d)) => (a.min(x0), b.min(z0), c.max(x1), d.max(z1)),
        })
    })
}

/// A validated native copy source (C# `NativeFamilySourceInfo`).
#[derive(Clone, Debug, PartialEq)]
pub struct NativeSource {
    pub slot: usize,
    pub uid: u32,
    pub type_id: u16,
    pub team: u16,
    pub position: [f32; 3],
    pub rotation_degrees: f32,
    pub object_count: usize,
    pub light_count: usize,
    pub particle_count: usize,
}

/// Mirrors C# `SelectNativeSceneObject`: the slot must be active, not already bound by the
/// scenario, and be an independent closed family root.
pub fn validate_native_source(
    store: &ObjectStore,
    slot: usize,
    bound_root_slots: &BTreeSet<usize>,
) -> Result<NativeSource, String> {
    let obj = store
        .objects()
        .into_iter()
        .find(|o| o.slot == slot)
        .ok_or_else(|| format!("Slot {slot} is inactive or does not exist."))?;
    if bound_root_slots.contains(&slot) {
        return Err(format!("Slot {slot} already has a scenario binding; duplicate it from the Placed list instead."));
    }
    let d = store.family_details(slot, obj.uid).map_err(|_| {
        format!("Slot {slot} is not an independent root (child, shared data, or invalid closure).")
    })?;
    Ok(NativeSource {
        slot,
        uid: obj.uid,
        type_id: obj.type_id,
        team: obj.team,
        position: obj.position,
        rotation_degrees: obj.rotation_degrees,
        object_count: d.object_slots.len(),
        light_count: d.light_slots.len(),
        particle_count: d.particle_slots.len(),
    })
}

/// Signed 16.16 displacement for a copy offset given in world units (default C# offset: 256 in X and Z).
pub fn copy_delta(offset_x: f32, offset_y: f32, offset_z: f32) -> Result<[i32; 3], String> {
    fixed_displacement([0.0; 3], [offset_x, offset_y, offset_z])
}

/// Copies the family through the store's transactional `clone_family_fixed` (one undo step).
/// Re-validates the source against the live store first so a stale selection cannot copy a
/// different object. Returns the new root `(slot, uid)`.
pub fn copy_family(
    store: &mut ObjectStore,
    source: &NativeSource,
    bound_root_slots: &BTreeSet<usize>,
    delta: [i32; 3],
) -> Result<(usize, u32), String> {
    let live = validate_native_source(store, source.slot, bound_root_slots)?;
    if live.uid != source.uid {
        return Err("Source object UID changed or object was removed.".into());
    }
    store.clone_family_fixed(source.slot, source.uid, delta)
}

// ------------------------------------------------------------------------------------------------
// UI
// ------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FamilyTab {
    #[default]
    Families,
    Lights,
    Particles,
    Definitions,
}

/// Everything the panel needs from the host for one frame.
pub struct FamilyCtx<'a> {
    /// The editable DATA store (placement session). Copy/undo/redo act on it.
    pub store: Option<&'a mut ObjectStore>,
    /// Map directory, used to read `DATA/light.dat` / `particle.dat` when no store is loaded.
    pub map_dir: Option<&'a Path>,
    /// Game directory for particle base definitions (`objdef`/`partpar`/... sources).
    pub game_dir: Option<&'a Path>,
    /// Copy / undo are only allowed on custom maps (never on original campaign maps).
    pub can_edit: bool,
    pub english: bool,
    /// Root slots already bound to scenario objects (cannot be copied as native families).
    pub bound_root_slots: Option<&'a BTreeSet<usize>>,
}

pub struct FamilyUi {
    pub(crate) history_request: Option<bool>,
    pub tab: FamilyTab,
    /// Selected family root `(slot, uid)`.
    pub selected: Option<(usize, u32)>,
    pub only_effects: bool,
    pub filter: String,
    pub include_inactive_definitions: bool,
    pub copy_offset_x: f32,
    pub copy_offset_z: f32,
    pub status: String,
    /// Set after a successful copy so the parent can refresh its placement list / 3D scene.
    pub store_changed: bool,
    lights: Option<Result<Vec<LightRow>, String>>,
    particles: Option<Result<Vec<ParticleRow>, String>>,
    roots: Option<Vec<RootRow>>,
    preview: Option<Result<FamilyPreview, String>>,
    definitions: Option<Result<EffectiveDefinitions, String>>,
    definitions_dir: Option<PathBuf>,
    map_key: Option<PathBuf>,
}

impl Default for FamilyUi {
    fn default() -> Self {
        Self {
            history_request: None,
            tab: FamilyTab::Families,
            selected: None,
            only_effects: false,
            filter: String::new(),
            include_inactive_definitions: false,
            copy_offset_x: WORLD_UNITS_PER_TILE,
            copy_offset_z: WORLD_UNITS_PER_TILE,
            status: String::new(),
            store_changed: false,
            lights: None,
            particles: None,
            roots: None,
            preview: None,
            definitions: None,
            definitions_dir: None,
            map_key: None,
        }
    }
}

fn tr(en: bool, english: &'static str, chinese: &'static str) -> &'static str {
    if en {
        english
    } else {
        chinese
    }
}

impl FamilyUi {
    /// Drops all cached tables (call after the placement store was edited elsewhere, or on map load).
    pub fn invalidate(&mut self) {
        self.lights = None;
        self.particles = None;
        self.roots = None;
        self.preview = None;
    }

    /// Full reset for a different map.
    pub fn reset_session(&mut self) {
        self.invalidate();
        self.selected = None;
        self.status.clear();
        self.store_changed = false;
        self.map_key = None;
    }

    /// Host hook for 3D/placement picking of a DATA object slot (C# `SelectNativeSceneObject`).
    /// Returns the validation result; on success the root becomes the selected family.
    pub fn select_root(
        &mut self,
        store: &ObjectStore,
        slot: usize,
        bound: Option<&BTreeSet<usize>>,
    ) -> Result<NativeSource, String> {
        let empty = BTreeSet::new();
        match validate_native_source(store, slot, bound.unwrap_or(&empty)) {
            Ok(src) => {
                self.selected = Some((src.slot, src.uid));
                self.preview = None;
                Ok(src)
            }
            Err(e) => {
                self.selected = None;
                self.preview = None;
                self.status = e.clone();
                Err(e)
            }
        }
    }

    /// Takes the "store changed" flag so the host can refresh dependent views once.
    pub fn take_store_changed(&mut self) -> bool {
        std::mem::take(&mut self.store_changed)
    }

    fn refresh(&mut self, ctx: &FamilyCtx) {
        let key = ctx.map_dir.map(Path::to_path_buf);
        if key != self.map_key {
            self.reset_session();
            self.map_key = key;
        }
        let store = ctx.store.as_deref();
        if self.lights.is_none() {
            self.lights = Some(match (store.and_then(|s| s.payload("light")), ctx.map_dir) {
                (Some(bytes), _) => LightPool::from_payload(bytes).map(|p| light_rows(&p)).map_err(|e| e.to_string()),
                (None, Some(dir)) => std::fs::read(dir.join("DATA").join("light.dat"))
                    .map_err(|e| e.to_string())
                    .and_then(|b| light_rows_from_bytes(&b)),
                (None, None) => Err("No map loaded.".into()),
            });
        }
        if self.particles.is_none() {
            self.particles = Some(match (store.and_then(|s| s.payload("particle")), ctx.map_dir) {
                (Some(bytes), _) => ParticlePool::from_payload(bytes)
                    .map(|p| particle_rows(&p))
                    .map_err(|e| e.to_string()),
                (None, Some(dir)) => std::fs::read(dir.join("DATA").join("particle.dat"))
                    .map_err(|e| e.to_string())
                    .and_then(|b| particle_rows_from_bytes(&b)),
                (None, None) => Err("No map loaded.".into()),
            });
        }
        if self.roots.is_none() {
            self.roots = Some(store.map(|s| family_roots(s, false)).unwrap_or_default());
        }
        if self.preview.is_none() {
            if let (Some((slot, uid)), Some(s)) = (self.selected, store) {
                self.preview = Some(build_family_preview(s, slot, uid));
            }
        }
    }

    fn load_definitions(&mut self, game_dir: Option<&Path>) {
        let Some(dir) = game_dir else {
            self.definitions = Some(Err("Game directory is not set.".into()));
            return;
        };
        self.definitions_dir = Some(dir.to_path_buf());
        self.definitions = Some(particle_definition_sources::load(dir));
    }

    pub fn show(&mut self, ui: &mut egui::Ui, mut ctx: FamilyCtx) {
        let en = ctx.english;
        self.refresh(&ctx);

        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, FamilyTab::Families, tr(en, "Families", "物件群組"));
            ui.selectable_value(&mut self.tab, FamilyTab::Lights, tr(en, "Saved lights", "已存光源"));
            ui.selectable_value(&mut self.tab, FamilyTab::Particles, tr(en, "Saved particles", "已存粒子"));
            ui.selectable_value(&mut self.tab, FamilyTab::Definitions, tr(en, "Particle definitions", "粒子定義"));
            if ui.button(tr(en, "Refresh", "重新整理")).clicked() {
                self.invalidate();
                self.refresh(&ctx);
            }
        });
        if !self.status.is_empty() {
            ui.label(&self.status);
        }
        ui.separator();

        match self.tab {
            FamilyTab::Families => self.show_families(ui, &mut ctx),
            FamilyTab::Lights => self.show_lights(ui, en),
            FamilyTab::Particles => self.show_particles(ui, en),
            FamilyTab::Definitions => self.show_definitions(ui, &ctx),
        }
    }

    fn show_lights(&mut self, ui: &mut egui::Ui, en: bool) {
        match &self.lights {
            Some(Ok(rows)) => {
                ui.label(format!("{} {}", rows.len(), tr(en, "active lights", "個啟用光源")));
                egui::ScrollArea::both().id_source("family_lights").show(ui, |ui| {
                    egui::Grid::new("family_lights_grid").striped(true).show(ui, |ui| {
                        for h in ["Slot", "X", "Y", "Z", "Color", "Radius", "Base", "Type", "Owner", "Parent", "Def"] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for r in rows {
                            ui.label(r.slot.to_string());
                            for v in r.position {
                                ui.label(format!("{v:.1}"));
                            }
                            let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                            ui.painter().rect_filled(rect, 2.0, r.display_color());
                            ui.label(format!("{:.1}", r.effective_radius));
                            ui.label(format!("{:.1}", r.baseline_radius));
                            ui.label(r.type_raw.to_string());
                            ui.label(r.owner.to_string());
                            ui.label(r.parent.to_string());
                            ui.label(r.definition.to_string());
                            ui.end_row();
                        }
                    });
                });
            }
            Some(Err(e)) => {
                ui.colored_label(Color32::from_rgb(250, 120, 120), format!("{}: {e}", tr(en, "Saved lights unavailable", "無法讀取已存光源")));
            }
            None => {}
        }
    }

    fn show_particles(&mut self, ui: &mut egui::Ui, en: bool) {
        match &self.particles {
            Some(Ok(rows)) => {
                ui.label(format!("{} {}", rows.len(), tr(en, "active emitters", "個啟用發射器")));
                egui::ScrollArea::vertical().id_source("family_particles").show(ui, |ui| {
                    for r in rows {
                        egui::CollapsingHeader::new(format!(
                            "#{}  owner {}  count {}  age {}  children {}",
                            r.slot,
                            r.owner,
                            r.count_raw,
                            r.age_raw,
                            r.children.len()
                        ))
                        .id_source(("family_particle", r.slot))
                        .show(ui, |ui| {
                            egui::Grid::new(("family_particle_grid", r.slot)).striped(true).show(ui, |ui| {
                                for h in ["#", "X", "Y", "Z", "Life", "Light"] {
                                    ui.strong(h);
                                }
                                ui.end_row();
                                for c in &r.children {
                                    ui.label(c.ordinal.to_string());
                                    for v in c.position {
                                        ui.label(format!("{v:.2}"));
                                    }
                                    ui.label(c.lifetime_raw.to_string());
                                    ui.label(c.light_link.map(|l| l.to_string()).unwrap_or_else(|| "-".into()));
                                    ui.end_row();
                                }
                            });
                        });
                    }
                });
            }
            Some(Err(e)) => {
                ui.colored_label(Color32::from_rgb(250, 120, 120), format!("{}: {e}", tr(en, "Saved particles unavailable", "無法讀取已存粒子")));
            }
            None => {}
        }
    }

    fn show_definitions(&mut self, ui: &mut egui::Ui, ctx: &FamilyCtx) {
        let en = ctx.english;
        ui.horizontal(|ui| {
            if ui.button(tr(en, "Load from game directory", "從遊戲目錄載入")).clicked() {
                self.load_definitions(ctx.game_dir);
            }
            ui.label(tr(en, "Filter", "篩選"));
            ui.text_edit_singleline(&mut self.filter);
            ui.checkbox(&mut self.include_inactive_definitions, tr(en, "Show inactive", "顯示未啟用"));
        });
        if self.definitions_dir.as_deref() != ctx.game_dir && self.definitions.is_some() {
            ui.small(tr(en, "Game directory changed; reload to refresh.", "遊戲目錄已變更，請重新載入。"));
        }
        match &self.definitions {
            None => {
                ui.label(tr(en, "Definitions are not loaded.", "尚未載入粒子定義。"));
            }
            Some(Err(e)) => {
                ui.colored_label(Color32::from_rgb(250, 120, 120), format!("{}: {e}", tr(en, "Particle definitions unavailable", "粒子定義無法使用")));
            }
            Some(Ok(defs)) => {
                let rows = definition_rows(defs, &self.filter, self.include_inactive_definitions);
                ui.label(format!("{} {}", rows.len(), tr(en, "rows", "列")));
                egui::ScrollArea::both().id_source("family_defs").show(ui, |ui| {
                    egui::Grid::new("family_defs_grid").striped(true).show(ui, |ui| {
                        for h in ["Id", "Name", "Active", "Anim", "ALR", "Param", "Geo", "Gfx", "Mode"] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for r in rows.iter().take(2000) {
                            ui.label(r.id.to_string());
                            ui.label(&r.name);
                            ui.label(r.active.to_string());
                            ui.label(r.animation_length.to_string());
                            ui.label(r.alr_id.to_string());
                            ui.label(r.parameter_id.to_string());
                            ui.label(r.geometry_id.to_string());
                            ui.label(r.grafik_id.to_string());
                            ui.label(r.mode.map(|m| m.to_string()).unwrap_or_else(|| "-".into()));
                            ui.end_row();
                        }
                    });
                });
            }
        }
    }

    fn show_families(&mut self, ui: &mut egui::Ui, ctx: &mut FamilyCtx) {
        let en = ctx.english;
        if ctx.store.is_none() {
            ui.label(tr(en, "No DATA object store is loaded.", "尚未載入 DATA 物件資料。"));
            return;
        }
        let mut changed_filter = false;
        ui.horizontal(|ui| {
            changed_filter |= ui.checkbox(&mut self.only_effects, tr(en, "Only families with lights/particles", "只顯示含光源／粒子的群組")).changed();
        });
        if changed_filter {
            self.roots = None;
            self.refresh(ctx);
        }

        let mut pick: Option<(usize, u32)> = None;
        egui::ScrollArea::vertical().id_source("family_roots").max_height(220.0).show(ui, |ui| {
            egui::Grid::new("family_roots_grid").striped(true).show(ui, |ui| {
                for h in ["", "Slot", "UID", "Type", "Team", "Objs", "Lights", "Particles", "Camps"] {
                    ui.strong(h);
                }
                ui.end_row();
                let roots = self.roots.as_deref().unwrap_or(&[]);
                for r in roots.iter().filter(|r| !self.only_effects || r.light_count > 0 || r.particle_count > 0) {
                    let sel = self.selected == Some((r.slot, r.uid));
                    if ui.selectable_label(sel, "▶").clicked() {
                        pick = Some((r.slot, r.uid));
                    }
                    ui.label(r.slot.to_string());
                    ui.label(r.uid.to_string());
                    ui.label(r.type_id.to_string());
                    ui.label(r.team.to_string());
                    ui.label(r.object_count.to_string());
                    ui.label(r.light_count.to_string());
                    ui.label(r.particle_count.to_string());
                    ui.label(r.lager_count.to_string());
                    ui.end_row();
                }
            });
        });
        if let Some(p) = pick {
            self.selected = Some(p);
            self.preview = None;
            self.refresh(ctx);
        }

        ui.separator();
        ui.heading(tr(en, "Family preview", "群組預覽"));
        match &self.preview {
            None => {
                ui.label(tr(en, "Select a family root above.", "請在上方選擇群組根物件。"));
            }
            Some(Err(e)) => {
                ui.colored_label(Color32::from_rgb(250, 120, 120), e);
            }
            Some(Ok(p)) => {
                ui.label(format!(
                    "root {} (uid {}): {} objects, {} lights, {} emitters, {} camps",
                    p.root_slot,
                    p.root_uid,
                    p.objects.len(),
                    p.lights.len(),
                    p.particles.len(),
                    p.lager_slots.len() + p.big_lager_slots.len()
                ));
                show_overlay(ui, &overlay_points(p));
            }
        }

        // Native copy (C# `_nativeCopyButton`) + undo/redo of the store transaction.
        ui.separator();
        let empty = BTreeSet::new();
        let bound = ctx.bound_root_slots.unwrap_or(&empty);
        ui.horizontal(|ui| {
            ui.label(tr(en, "Offset X", "位移 X"));
            ui.add(egui::DragValue::new(&mut self.copy_offset_x).speed(16.0));
            ui.label(tr(en, "Offset Z", "位移 Z"));
            ui.add(egui::DragValue::new(&mut self.copy_offset_z).speed(16.0));
        });
        let mut do_copy = false;
        let (mut do_undo, mut do_redo) = (false, false);
        let (can_undo, can_redo) = ctx
            .store
            .as_deref()
            .map(|s| (s.can_undo(), s.can_redo()))
            .unwrap_or((false, false));
        ui.horizontal(|ui| {
            let can_copy = ctx.can_edit && self.selected.is_some();
            do_copy = ui
                .add_enabled(can_copy, egui::Button::new(tr(en, "Copy native family", "複製原生群組")))
                .on_disabled_hover_text(tr(
                    en,
                    "Select a family root on a custom map.",
                    "請在自製地圖上選擇群組根物件。",
                ))
                .clicked();
            do_undo = ui.add_enabled(ctx.can_edit && can_undo, egui::Button::new(tr(en, "Undo", "復原"))).clicked();
            do_redo = ui.add_enabled(ctx.can_edit && can_redo, egui::Button::new(tr(en, "Redo", "重做"))).clicked();
        });
        if do_copy {
            self.run_copy(ctx, bound);
        }
        if do_undo { self.history_request = Some(false); }
        if do_redo { self.history_request = Some(true); }
    }

    fn after_edit(&mut self, message: &str) {
        self.status = message.to_string();
        self.store_changed = true;
        self.invalidate();
    }

    fn run_copy(&mut self, ctx: &mut FamilyCtx, bound: &BTreeSet<usize>) {
        let en = ctx.english;
        let Some((slot, _)) = self.selected else { return };
        let Some(store) = ctx.store.as_deref_mut() else { return };
        let result = validate_native_source(store, slot, bound).and_then(|src| {
            let delta = copy_delta(self.copy_offset_x, 0.0, self.copy_offset_z)?;
            copy_family(store, &src, bound, delta).map(|new| (src, new))
        });
        match result {
            Ok((src, (new_slot, new_uid))) => {
                self.selected = Some((new_slot, new_uid));
                self.after_edit(&format!(
                    "{} {} → {} (uid {new_uid})",
                    tr(en, "Copied family", "已複製群組"),
                    src.slot,
                    new_slot
                ));
            }
            Err(e) => self.status = e,
        }
    }
}

/// Draws the 2D overlay (objects = grey dots, lights = coloured discs sized by radius, particle
/// children = orange dots) fitted into a square canvas.
pub fn show_overlay(ui: &mut egui::Ui, points: &[OverlayPoint]) {
    let size = Vec2::splat(240.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::from_gray(24));
    let Some((x0, z0, x1, z1)) = overlay_bounds(points) else { return };
    let span = (x1 - x0).max(z1 - z0).max(1.0);
    let scale = (rect.width() - 20.0) / span;
    let to_screen = |x: f32, z: f32| Pos2::new(rect.left() + 10.0 + (x - x0) * scale, rect.top() + 10.0 + (z - z0) * scale);
    for p in points {
        let c = to_screen(p.x, p.z);
        match p.kind {
            OverlayKind::Light => {
                let r = (p.radius * scale).clamp(3.0, rect.width());
                painter.circle_stroke(c, r, Stroke::new(1.0_f32, p.color));
                painter.circle_filled(c, 3.0, p.color);
            }
            OverlayKind::Object => {
                painter.rect_filled(Rect::from_center_size(c, Vec2::splat(6.0)), 1.0, p.color);
            }
            OverlayKind::ParticleChild => {
                painter.circle_filled(c, 2.0, p.color);
            }
        }
    }
    response.on_hover_text("grey = object, ring = light radius, orange = particle child");
}
