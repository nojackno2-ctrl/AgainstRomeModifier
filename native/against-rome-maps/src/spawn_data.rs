//! Spawn -> DATA save planning: Rust port of the prebuilt block of C#
//! `MapEditorForm.Persistence.cs` (`TrySaveMap`).
//!
//! New building spawns on team 0..=8 become prebuilt native DATA objects (complete at game start,
//! built from an official/current-map singleton template), moved/edited bound spawns keep or
//! rebuild their DATA, and duplicates of bound (or native-sourced) spawns copy their DATA family.
//! Everything is computed on detached `ObjectStore` clones: this module never writes a file, it
//! only returns the pool bytes that the caller stages into the whole-map save transaction, so any
//! failure leaves nothing written.

use crate::custom_map::CustomMapManifest;
use crate::native_objdef::NativeObjectDefinitions;
use crate::placement_edit::{build_family_copy_requests, ScenarioFamilyCopyRequest};
use crate::script_aliases::{ScriptObjectAlias, ScriptObjectAliases};
use crate::sdl_objects::{SdlObjectCategory, SdlPlacedObject};
use crate::starter_kit::native::ObjectStore;
use crate::starter_kit::BlankMapStarterKit;
use crate::starter_kit::transaction::{apply_to_candidate_with_copies, ScenarioCandidateResult, Template};
use against_rome_scenario::model::{ScenarioDocument, Spawn};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// DATA pools in the order the editor stages them.
pub const DATA_POOLS: [&str; 11] = [
    "objects", "objdata", "position", "anim", "gfxtype", "action", "engine", "light", "particle",
    "lager", "biglager",
];

/// C# `(Building && team 0..=8) || RawRotation.HasValue`: which placements become DATA objects.
pub fn intended_prebuilt(item: &SdlPlacedObject) -> bool {
    (item.obj_type.category == SdlObjectCategory::Building && (0..=8).contains(&item.team))
        || item.raw_rotation.is_some()
}

/// Applies [`intended_prebuilt`] to the spawns that belong to `placed` (by persistent id).
/// Spawns that are not represented by a placement (foreign aliases) are left untouched.
pub fn mark_prebuilt(spawns: &mut [Spawn], placed: &[SdlPlacedObject]) {
    let by_id: HashMap<Uuid, &SdlPlacedObject> = placed.iter().map(|p| (p.scenario_id, p)).collect();
    for spawn in spawns {
        if let Some(item) = by_id.get(&spawn.id) {
            spawn.prebuilt = intended_prebuilt(item);
        }
    }
}

/// Game catalogs the DATA transaction needs: script aliases (alias -> object definition name),
/// the objdef type-id -> name table, and the official `MAPS` folder providing building templates.
#[derive(Clone, Debug, Default)]
pub struct SpawnDataCatalog {
    pub aliases: Vec<ScriptObjectAlias>,
    pub type_names: HashMap<i32, String>,
    pub game_maps: Option<PathBuf>,
}

impl SpawnDataCatalog {
    pub fn from_game(game: &Path) -> Result<Self, String> {
        let aliases = ScriptObjectAliases::load(game)?;
        let type_names = NativeObjectDefinitions::load(game)?
            .into_iter()
            .filter_map(|(id, d)| d.name.filter(|n| !n.is_empty()).map(|n| (id, n)))
            .collect();
        Ok(Self { aliases, type_names, game_maps: Some(game.join("MAPS")) })
    }

    /// C# `ResolveNativeAlias`: type id -> object name -> catalog aliases of that name; the
    /// canonical alias is the case-insensitive smallest one.
    pub fn resolve_native_alias(&self, type_id: i32) -> Option<String> {
        let name = self.type_names.get(&type_id).filter(|n| !n.trim().is_empty())?;
        self.aliases
            .iter()
            .filter(|a| a.name_def.eq_ignore_ascii_case(name))
            .map(|a| a.alias.clone())
            .min_by_key(|a| a.to_uppercase())
    }

    /// Object type id of a spawn alias (smallest id when several definitions share the name).
    pub fn type_id_for_alias(&self, alias: &str) -> Option<i32> {
        let name = &self.aliases.iter().find(|a| a.alias.eq_ignore_ascii_case(alias))?.name_def;
        self.type_names.iter().filter(|(_, n)| n.eq_ignore_ascii_case(name)).map(|(id, _)| *id).min()
    }
}

/// C# `BuildingTemplateFor`: official building template first (first official map, in ordinal
/// path order, that has a safe singleton of the type), then a singleton of the current map.
struct Templates<'a> {
    catalog: &'a SpawnDataCatalog,
    current: &'a ObjectStore,
    cache: RefCell<HashMap<i32, Option<Template>>>,
}

impl Templates<'_> {
    fn singleton_in(store: &ObjectStore, type_id: i32) -> Option<Template> {
        store
            .objects()
            .into_iter()
            .filter(|o| i32::from(o.type_id) == type_id)
            .find(|o| store.singleton_template(o.slot, o.uid).is_ok())
            .map(|o| Template { store: store.clone(), slot: o.slot, uid: o.uid })
    }

    fn official(&self, type_id: i32) -> Option<Template> {
        let is_building = self.catalog.type_names.get(&type_id).is_some_and(|n| {
            n.len() >= 3 && n.as_bytes()[..3].eq_ignore_ascii_case(b"Bau")
        });
        if !is_building {
            return None;
        }
        let mut maps: Vec<PathBuf> = std::fs::read_dir(self.catalog.game_maps.as_ref()?)
            .ok()?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        maps.sort_by_key(|p| p.to_string_lossy().to_uppercase());
        for map in maps {
            if CustomMapManifest::is_custom_map_directory(&map) || !map.join("DATA").join("objects.dat").is_file() {
                continue;
            }
            let Ok(store) = ObjectStore::load(&map) else { continue };
            if let Some(template) = Self::singleton_in(&store, type_id) {
                return Some(template);
            }
        }
        None
    }

    fn for_spawn(&self, spawn: &Spawn) -> Option<Template> {
        let type_id = self.catalog.type_id_for_alias(&spawn.alias)?;
        if let Some(hit) = self.cache.borrow().get(&type_id) {
            return hit.clone();
        }
        let found = self.official(type_id).or_else(|| Self::singleton_in(self.current, type_id));
        self.cache.borrow_mut().insert(type_id, found.clone());
        found
    }
}

/// Result of a spawn DATA transaction plan.
#[derive(Clone, Debug)]
pub struct SpawnDataPlan {
    /// Scenario to save: final spawns (script fallback for template-less buildings), the new DATA
    /// bindings and the original events.
    pub scenario: ScenarioDocument,
    /// `(slot, uid)` of every active object of the candidate DATA (binding verification input).
    pub active: Vec<(i32, u32)>,
    /// Changed DATA pools as `DATA/<pool>.dat` bytes (PFIL container kept), ready to stage.
    pub files: Vec<(PathBuf, Vec<u8>)>,
    /// Prebuilt spawns without an official/current template: they stay script-created.
    pub skipped: Vec<Uuid>,
}

/// PFIL container with literal-only chunks; keeps the 64-byte header (length patched).
fn encode_pfil(header: &[u8; 64], payload: &[u8]) -> Vec<u8> {
    let mut header = *header;
    header[16..20].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    let mut out = header.to_vec();
    for part in payload.chunks(8) {
        out.push(((1u16 << part.len()) - 1) as u8);
        out.extend_from_slice(part);
    }
    out
}

fn stage_changed_pools(map: &Path, original: &ObjectStore, candidate: &ObjectStore) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    let mut files = Vec::new();
    for name in DATA_POOLS {
        let (Some(new), old) = (candidate.payload(name), original.payload(name)) else { continue };
        if old == Some(new) {
            continue;
        }
        let rel = PathBuf::from(format!("DATA/{name}.dat"));
        let disk = std::fs::read(map.join(&rel)).ok();
        let bytes = match disk {
            Some(raw) if raw.starts_with(b"PFIL") => {
                let header: [u8; 64] = raw
                    .get(..64)
                    .and_then(|h| h.try_into().ok())
                    .ok_or_else(|| format!("{} 的 PFIL 標頭已毀損。", rel.display()))?;
                encode_pfil(&header, new)
            }
            _ => new.to_vec(),
        };
        files.push((rel, bytes));
    }
    Ok(files)
}

/// Plans the DATA transaction of one save without a pending starter kit.
pub fn plan_spawn_data(
    map: &Path,
    catalog: &SpawnDataCatalog,
    previous: &ScenarioDocument,
    next: &ScenarioDocument,
    placed: &[SdlPlacedObject],
) -> Result<Option<SpawnDataPlan>, String> {
    plan_spawn_data_with_kit(map, catalog, previous, next, placed, None)
}

/// Plans the DATA transaction of one save. `previous` is the saved scenario (bindings),
/// `next` the intended one (spawn flags already set with [`mark_prebuilt`]), `placed` the editor
/// placements (family-copy provenance). A pending starter kit with live ids is first imported into
/// the saved DATA exactly like C# `ImportPendingStarterKit` (blank-map gate, main-house linked
/// family from the official source, native metadata binding) and the final candidate is validated
/// against the terrain. Returns `None` when no spawn is, or was, prebuilt.
pub fn plan_spawn_data_with_kit(
    map: &Path,
    catalog: &SpawnDataCatalog,
    previous: &ScenarioDocument,
    next: &ScenarioDocument,
    placed: &[SdlPlacedObject],
    kit: Option<&PendingStarterKit>,
) -> Result<Option<SpawnDataPlan>, String> {
    if !next.spawns.iter().any(|s| s.prebuilt) && previous.data_slots.is_empty() {
        return Ok(None);
    }
    let disk = ObjectStore::load(map).map_err(|e| format!("無法讀取地圖 DATA：{e}"))?;
    let kit = kit.filter(|k| !k.ids.is_empty());
    let (original, previous) = match kit {
        Some(k) => {
            let game = catalog.game_maps.as_deref().and_then(Path::parent);
            require_starter_kit_blank_map(map, game, &disk, previous)?;
            let templates = Templates { catalog, current: &disk, cache: RefCell::default() };
            let mut imported = k.kit.apply_to_candidate(&disk, previous, Some(&|s| templates.for_spawn(s)))?;
            let all: HashSet<Uuid> = k.kit.spawns.iter().map(|s| s.id).collect();
            bind_native_metadata(&mut imported, &all)?;
            (imported.store, imported.scenario)
        }
        None => (disk.clone(), previous.clone()),
    };
    let previous = &previous;
    let resolver = |type_id: i32| catalog.resolve_native_alias(type_id);
    let requests: HashMap<Uuid, ScenarioFamilyCopyRequest> =
        build_family_copy_requests(placed, previous, &original, Some(&resolver))?;
    let templates = Templates { catalog, current: &original, cache: RefCell::default() };
    let template_for = |spawn: &Spawn| {
        kit.and_then(|k| k.kit.template_for(spawn)).or_else(|| templates.for_spawn(spawn))
    };
    // Events do not take part in the DATA transaction (C# validates spawns only); a condition
    // that targets a new prebuilt object is validated by the final document below.
    let (mut data_previous, mut data_next) = (previous.clone(), next.clone());
    data_next.data_slots.clear(); // bindings are rebuilt by the transaction
    data_previous.events.clear();
    data_next.events.clear();
    let result = apply_to_candidate_with_copies(
        &original,
        &data_previous,
        &data_next,
        &template_for,
        Some(&requests),
        Some(&resolver),
    )?;
    let mut scenario = result.scenario;
    scenario.events = next.events.clone();
    scenario.version = next.version;
    // No template: the original game has none for this building, script creation (a
    // construction site at game start) is the fallback, exactly like C#.
    let skipped: Vec<Uuid> = result.skipped.iter().map(|s| s.id).collect();
    for spawn in &mut scenario.spawns {
        if skipped.contains(&spawn.id) {
            spawn.prebuilt = false;
        }
    }
    let active: Vec<(i32, u32)> =
        result.store.objects().iter().map(|o| (o.slot as i32, o.uid)).collect();
    scenario.validate(None)?;
    against_rome_scenario::verify_binding_snapshot(&scenario, &active)?;
    if let Some(k) = kit {
        validate_starter_kit_candidate(&result.store, &scenario, k.ids, k.terrain)?;
    }
    let files = stage_changed_pools(map, &disk, &result.store)?;
    Ok(Some(SpawnDataPlan { scenario, active, files, skipped }))
}

// ---- Starter kit on an existing blank map (C# MapEditorForm.StarterKit.cs) --------------------

/// A starter kit staged in the editor plus the ids of its placements that are still alive
/// (kit originals and their copies; C# `PendingStarterKitIds`).
pub struct PendingStarterKit<'a> {
    pub kit: &'a BlankMapStarterKit,
    pub ids: &'a HashSet<Uuid>,
    pub terrain: &'a StarterKitTerrain<'a>,
}

/// Trusted terrain data used to place and validate the kit (C# `LayoutGroundHeight`, collision
/// layer, water level). Heights are bytes scaled by `height_step`; `water_world` is in world units.
#[derive(Clone, Copy, Debug)]
pub struct StarterKitTerrain<'a> {
    pub heights: &'a [u8],
    pub vertex_size: usize,
    pub height_step: f32,
    pub collision: &'a [u8],
    pub collision_size: usize,
    pub water_world: f32,
}

impl StarterKitTerrain<'_> {
    pub fn ground(&self, x: f32, z: f32) -> f32 {
        let step = (self.vertex_size - 1) as f32 / (64.0 * 256.0);
        let max = self.vertex_size as i32 - 1;
        let vx = ((x * step).round_ties_even() as i32).clamp(0, max) as usize;
        let vz = ((z * step).round_ties_even() as i32).clamp(0, max) as usize;
        f32::from(self.heights[vz * self.vertex_size + vx]) * self.height_step
    }

    fn check_trusted(&self) -> Result<(), String> {
        if self.vertex_size < 2
            || self.heights.len() < self.vertex_size * self.vertex_size
            || !self.height_step.is_finite()
            || self.height_step <= 0.0
            || !self.water_world.is_finite()
        {
            return Err("地圖缺少可信地形高度資料，無法驗證起始配置。".into());
        }
        if self.collision_size == 0 || self.collision.len() != self.collision_size * self.collision_size {
            return Err("起始配置缺少可信阻擋地形資料。".into());
        }
        Ok(())
    }
}

fn script_directory(map: &Path) -> PathBuf {
    std::fs::read_dir(map)
        .ok()
        .and_then(|entries| {
            entries.flatten().find(|e| {
                e.path().is_dir() && e.file_name().to_string_lossy().eq_ignore_ascii_case("SCRIPT")
            })
        })
        .map_or_else(|| map.join("SCRIPT"), |e| e.path())
}

/// C# `RequireStarterKitBlankMap`: only an editable custom map whose native DATA and Scenario
/// placements are all empty and whose level scripts are the exact idle bootstrap qualifies.
pub fn require_starter_kit_blank_map(
    map: &Path,
    game: Option<&Path>,
    store: &ObjectStore,
    previous: &ScenarioDocument,
) -> Result<PathBuf, String> {
    let map = crate::custom_map::CustomMapAccess::require_editable_directory(map, game)?;
    if !(1..=BlankMapStarterKit::MAXIMUM_OBJECTS).contains(&store.capacity())
        || !store.objects().is_empty()
        || !previous.spawns.is_empty()
        || !previous.data_slots.is_empty()
    {
        return Err("起始配置只允許原生 DATA 與 Scenario 放置皆為空的自製地圖。".into());
    }
    // Empty objects alone does not prove the settlement script is disabled: both the live script
    // and its backup must be the exact idle image BlankMapContent writes.
    let idle = against_rome_scenario::BciImage::idle().serialize();
    let scripts = script_directory(&map);
    for name in ["ak_level.bci", "ak_level.arm_original"] {
        let not_idle = || "此地圖不是具有閒置開局腳本的空白場景，已拒絕起始配置。".to_string();
        let bytes = std::fs::read(scripts.join(name)).map_err(|_| not_idle())?;
        let image = crate::text_encoding::decompress_pfil_if_needed(&bytes).map_err(|_| not_idle())?;
        if image != idle {
            return Err(not_idle());
        }
    }
    Ok(map)
}

/// C# `BindStarterKitNativeMetadata`: the kit's spawns take the exact native rotation of their
/// bound root (raw rotation, display angle, no unit count).
fn bind_native_metadata(candidate: &mut ScenarioCandidateResult, ids: &HashSet<Uuid>) -> Result<(), String> {
    let invalid = || "起始配置 DATA binding 或旋轉資料無效。".to_string();
    for i in 0..candidate.scenario.spawns.len() {
        let id = candidate.scenario.spawns[i].id;
        if !ids.contains(&id) {
            continue;
        }
        let bindings: Vec<_> = candidate.scenario.data_slots.iter().filter(|b| b.spawn_id == id).collect();
        let [binding] = bindings.as_slice() else { return Err(invalid()) };
        let root = candidate
            .store
            .objects()
            .into_iter()
            .find(|o| o.slot as i32 == binding.slot && o.uid == binding.uid)
            .ok_or_else(invalid)?;
        let rotation = candidate.store.native_rotation(root.slot, root.uid).map_err(|_| invalid())?;
        if !rotation.is_finite() {
            return Err(invalid());
        }
        let spawn = &mut candidate.scenario.spawns[i];
        spawn.raw_rotation = Some(rotation);
        spawn.angle = crate::starter_kit::transaction::display_angle(rotation);
        spawn.count = 0;
    }
    Ok(())
}

/// C# `ValidateStarterKitCandidate`: every kit family must stay inside the map, not overlap
/// another kit family, and sit on dry, unblocked, nearly flat ground.
/// (Differs from C#: the obstacle scan of SDL scene objects is not available here; the blank-map
/// gate already guarantees an empty native DATA store.)
pub fn validate_starter_kit_candidate(
    store: &ObjectStore,
    scenario: &ScenarioDocument,
    ids: &HashSet<Uuid>,
    terrain: &StarterKitTerrain,
) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    terrain.check_trusted().map_err(|_| "地圖缺少可信地形高度資料。".to_string())?;
    let all = store.objects();
    if store.capacity() > BlankMapStarterKit::MAXIMUM_OBJECTS || all.len() > store.capacity() {
        return Err("起始配置超出原生物件容量。".into());
    }
    let mut extents: Vec<(f32, f32, f32, f32)> = Vec::new();
    for spawn in scenario.spawns.iter().filter(|s| ids.contains(&s.id)) {
        let bindings: Vec<_> = scenario.data_slots.iter().filter(|b| b.spawn_id == spawn.id).collect();
        let [binding] = bindings.as_slice() else {
            return Err("起始配置原生 family 無法安全驗證。".into());
        };
        let family = store
            .family(binding.slot as usize, binding.uid)
            .map_err(|_| "起始配置原生 family 無法安全驗證。".to_string())?;
        let members: Vec<_> = all.iter().filter(|o| family.contains(&o.slot)).collect();
        if members.is_empty() {
            return Err("起始配置原生 family 無法安全驗證。".into());
        }
        let margin = if spawn.alias.eq_ignore_ascii_case(BlankMapStarterKit::MAIN_HOUSE_ALIAS) { 384.0 } else { 64.0 };
        let fold = |f: fn(f32, f32) -> f32, axis: usize, init: f32| {
            members.iter().fold(init, |acc, o| f(acc, o.position[axis]))
        };
        let left = fold(f32::min, 0, f32::INFINITY) - margin;
        let right = fold(f32::max, 0, f32::NEG_INFINITY) + margin;
        let top = fold(f32::min, 2, f32::INFINITY) - margin;
        let bottom = fold(f32::max, 2, f32::NEG_INFINITY) + margin;
        if ![left, right, top, bottom].iter().all(|v| v.is_finite())
            || left < 0.0
            || top < 0.0
            || right > 16383.0
            || bottom > 16383.0
            || members.iter().any(|o| !o.position[1].is_finite())
        {
            return Err("起始配置的完整 family 範圍超出地圖邊界。".into());
        }
        if extents.iter().any(|&(l, r, t, b)| left <= r && right >= l && top <= b && bottom >= t) {
            return Err("起始配置物件彼此重疊，請分開放置。".into());
        }
        extents.push((left, right, top, bottom));
        let size = terrain.collision_size;
        let sample_step = 64f32.min((16384.0 / size as f32).min(16384.0 / (terrain.vertex_size - 1) as f32));
        if !sample_step.is_finite() || sample_step <= 0.0 {
            return Err("起始配置地形取樣間距無效。".into());
        }
        let (mut minimum, mut maximum) = (f32::INFINITY, f32::NEG_INFINITY);
        // Includes every family member and the whole extent, not just the root.
        let mut z = top;
        loop {
            let mut x = left;
            loop {
                let ground = terrain.ground(x, z);
                if !ground.is_finite() || ground <= terrain.water_world + 1.0 {
                    return Err("起始配置範圍含水面或無效地形。".into());
                }
                let tx = ((x / 16384.0 * size as f32) as usize).min(size - 1);
                let tz = ((z / 16384.0 * size as f32) as usize).min(size - 1);
                if terrain.collision[tz * size + tx] > 128 {
                    return Err("起始配置範圍含阻擋地形。".into());
                }
                minimum = minimum.min(ground);
                maximum = maximum.max(ground);
                if x == right {
                    break;
                }
                x = right.min(x + sample_step);
            }
            if z == bottom {
                break;
            }
            z = bottom.min(z + sample_step);
        }
        if maximum - minimum > 32.0 {
            return Err("起始配置範圍地形高差過大（超過 32 世界單位）。".into());
        }
    }
    Ok(())
}

/// A kit staged for an existing blank map: the detached kit (kept for the save-time import) and
/// the five placements with their exact native metadata.
pub struct StarterKitStage {
    pub kit: BlankMapStarterKit,
    pub spawns: Vec<Spawn>,
}

/// C# `ApplyStarterKit` without UI: gate, create the kit from the official source, apply it to a
/// detached candidate, bind native metadata and validate. Writes nothing.
pub fn stage_starter_kit(
    game: &Path,
    map: &Path,
    previous: &ScenarioDocument,
    team: i32,
    x: f32,
    z: f32,
    terrain: &StarterKitTerrain,
) -> Result<StarterKitStage, String> {
    let store = ObjectStore::load(map).map_err(|e| format!("無法讀取地圖 DATA：{e}"))?;
    let map = require_starter_kit_blank_map(map, Some(game), &store, previous)?;
    terrain.check_trusted()?;
    let height = |x: f32, z: f32| terrain.ground(x, z);
    let kit = BlankMapStarterKit::create_from_game(game, &map, team, x, z, Some(&height), Some(&previous.spawns))?;
    let catalog = SpawnDataCatalog::from_game(game).ok();
    let templates = catalog.as_ref().map(|c| Templates { catalog: c, current: &store, cache: RefCell::default() });
    let fallback = |s: &Spawn| templates.as_ref().and_then(|t| t.for_spawn(s));
    let mut candidate = kit.apply_to_candidate(&store, previous, Some(&fallback))?;
    let ids: HashSet<Uuid> = kit.spawns.iter().map(|s| s.id).collect();
    bind_native_metadata(&mut candidate, &ids)?;
    let spawns: Vec<Spawn> = kit
        .spawns
        .iter()
        .filter_map(|k| candidate.scenario.spawns.iter().find(|s| s.id == k.id).cloned())
        .collect();
    if spawns.len() != 5 || ids.len() != 5 || ids.contains(&Uuid::nil()) {
        return Err("起始配置身份或數量不符。".into());
    }
    validate_starter_kit_candidate(&candidate.store, &candidate.scenario, &ids, terrain)?;
    Ok(StarterKitStage { kit, spawns })
}

impl std::fmt::Debug for StarterKitStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StarterKitStage").field("spawns", &self.spawns).finish_non_exhaustive()
    }
}
