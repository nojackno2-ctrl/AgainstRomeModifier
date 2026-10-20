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
use crate::starter_kit::transaction::{apply_to_candidate_with_copies, Template};
use against_rome_scenario::model::{ScenarioDocument, Spawn};
use std::cell::RefCell;
use std::collections::HashMap;
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

/// Plans the DATA transaction of one save. `previous` is the saved scenario (bindings),
/// `next` the intended one (spawn flags already set with [`mark_prebuilt`]), `placed` the editor
/// placements (family-copy provenance). Returns `None` when no spawn is, or was, prebuilt.
pub fn plan_spawn_data(
    map: &Path,
    catalog: &SpawnDataCatalog,
    previous: &ScenarioDocument,
    next: &ScenarioDocument,
    placed: &[SdlPlacedObject],
) -> Result<Option<SpawnDataPlan>, String> {
    if !next.spawns.iter().any(|s| s.prebuilt) && previous.data_slots.is_empty() {
        return Ok(None);
    }
    let original = ObjectStore::load(map).map_err(|e| format!("無法讀取地圖 DATA：{e}"))?;
    let resolver = |type_id: i32| catalog.resolve_native_alias(type_id);
    let requests: HashMap<Uuid, ScenarioFamilyCopyRequest> =
        build_family_copy_requests(placed, previous, &original, Some(&resolver))?;
    let templates = Templates { catalog, current: &original, cache: RefCell::default() };
    let template_for = |spawn: &Spawn| templates.for_spawn(spawn);
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
    let files = stage_changed_pools(map, &original, &result.store)?;
    Ok(Some(SpawnDataPlan { scenario, active, files, skipped }))
}
