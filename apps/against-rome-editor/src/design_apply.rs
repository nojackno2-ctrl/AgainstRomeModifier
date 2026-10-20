//! Applies designer-panel plans (environment / landscape) to the editor's edit sessions:
//! DATA objects (`ObjectStore`), terrain heights/collision (`TerrainSession`) and tile textures
//! (`TerrainBlendEditSession`). Every function validates first and mutates a fork or commits a
//! single undoable stroke, so a rejected plan leaves the sessions untouched.
use against_rome_landscape::{
    cliff::{
        apply_cliff_plan, plan_cliffs, CliffBlendSession, CliffDetectionResult, CliffHeightSession,
        CliffPlannerOptions, CliffTileCatalog,
    },
    fortification::{synchronize_passability, CollisionPixelChange, WallStrokePlan},
    river::RiverStrokePlan,
    road::RoadStrokePlan,
};
use against_rome_placement::ObjectStore;
use against_rome_textures::TerrainBlendEditSession;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::terrain::{CollisionBrushOp, SampleChange, TerrainSession};

/// Each object edit snapshots every DATA pool, so one apply call is bounded.
pub const MAX_OBJECTS_PER_BATCH: usize = 96;
const WORLD_LIMIT: f32 = 16384.0;

/// An object the DATA session should receive: an existing family of `type_id` is cloned to the
/// target (the core cannot create new object definitions).
#[derive(Clone, Debug, PartialEq)]
pub struct PlacementRequest {
    pub type_id: i32,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rotation_deg: f32,
    pub team: Option<u16>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ApplyReport {
    pub textures: usize,
    pub heights: usize,
    pub collision: usize,
    pub objects: usize,
    pub skipped: Vec<String>,
}

impl ApplyReport {
    pub fn summary(&self) -> String {
        let mut s = format!(
            "applied {} tile textures, {} height samples, {} collision pixels, {} objects",
            self.textures, self.heights, self.collision, self.objects
        );
        if !self.skipped.is_empty() {
            s.push_str("; skipped: ");
            s.push_str(&self.skipped.join("; "));
        }
        s
    }
}

/// Case-insensitive set of texture names a plan may write.
#[derive(Clone, Debug, Default)]
pub struct TextureNames(HashSet<String>);
impl TextureNames {
    pub fn new<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self(names.into_iter().map(|n| n.as_ref().to_ascii_uppercase()).collect())
    }
    pub fn contains(&self, name: &str) -> bool {
        self.0.contains(&name.to_ascii_uppercase())
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

// ---------------------------------------------------------------- DATA objects

fn fixed(delta: f32) -> Result<i32, String> {
    let scaled = (delta as f64 * 65536.0).round();
    if !scaled.is_finite() || scaled < i32::MIN as f64 || scaled > i32::MAX as f64 {
        return Err("object displacement outside signed 16.16 range".into());
    }
    Ok(scaled as i32)
}

/// Outcome of one bounded batch against an `ObjectStore`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlacementBatch {
    /// Requests consumed from the front of the slice (placed or skipped with a reason).
    pub consumed: usize,
    /// History entries added to the store (for grouped undo).
    pub ops: usize,
    pub placed: usize,
    pub skipped: Vec<String>,
}

/// Clones template families to each request. All-or-nothing for core errors (the store is only
/// replaced when every request in the batch succeeded); requests with no template instance,
/// invalid coordinates or an out-of-map position are skipped and reported.
pub fn apply_placements(store: &mut ObjectStore, requests: &[PlacementRequest]) -> Result<PlacementBatch, String> {
    let mut templates: HashMap<u16, (usize, u32, [f32; 3], f32)> = HashMap::new();
    for o in store.objects() {
        if o.parent.is_none() && o.position.iter().all(|v| v.is_finite()) {
            templates
                .entry(o.type_id)
                .or_insert((o.slot, o.uid, o.position, o.rotation_degrees));
        }
    }
    let mut candidate = store.clone();
    let mut batch = PlacementBatch::default();
    let mut missing: BTreeMap<i32, usize> = BTreeMap::new();
    let (mut invalid, mut outside) = (0usize, 0usize);
    for r in requests.iter().take(MAX_OBJECTS_PER_BATCH) {
        batch.consumed += 1;
        if ![r.x, r.y, r.z, r.rotation_deg].iter().all(|v| v.is_finite()) {
            invalid += 1;
            continue;
        }
        if r.x < 0.0 || r.z < 0.0 || r.x >= WORLD_LIMIT || r.z >= WORLD_LIMIT {
            outside += 1;
            continue;
        }
        let Some(&(slot, uid, pos, rot)) = u16::try_from(r.type_id).ok().and_then(|t| templates.get(&t)) else {
            *missing.entry(r.type_id).or_default() += 1;
            continue;
        };
        let delta = [fixed(r.x - pos[0])?, fixed(r.y - pos[1])?, fixed(r.z - pos[2])?];
        let (root, new_uid) = candidate.clone_family_fixed(slot, uid, delta)?;
        batch.ops += 1;
        if (r.rotation_deg - rot).abs() > 0.01 {
            candidate.set_family_rotation(root, new_uid, r.rotation_deg)?;
            batch.ops += 1;
        }
        if let Some(team) = r.team {
            candidate.set_family_team(root, new_uid, team)?;
            batch.ops += 1;
        }
        batch.placed += 1;
    }
    if !missing.is_empty() {
        let list: Vec<_> = missing.iter().map(|(t, n)| format!("type {t} ×{n}")).collect();
        batch.skipped.push(format!("no template object on the map for {}", list.join(", ")));
    }
    if invalid > 0 {
        batch.skipped.push(format!("{invalid} non-finite requests"));
    }
    if outside > 0 {
        batch.skipped.push(format!("{outside} requests outside the 16384×16384 world"));
    }
    if batch.placed > 0 {
        *store = candidate;
    }
    Ok(batch)
}

// ---------------------------------------------------------------- tile textures

#[derive(Clone, Debug, Default)]
struct StampTally {
    changed: usize,
    unknown: BTreeSet<String>,
    out_of_range: usize,
}

/// Stamps on a fork; the caller decides whether to adopt it.
fn stamp_on_fork<'a>(
    session: &TerrainBlendEditSession,
    known: &TextureNames,
    stamps: impl IntoIterator<Item = (i32, i32, &'a str)>,
) -> (TerrainBlendEditSession, StampTally) {
    let dim = session.tile_dimension();
    let mut fork = session.fork();
    let mut tally = StampTally::default();
    for (x, y, texture) in stamps {
        if x < 0 || y < 0 || x as usize >= dim || y as usize >= dim {
            tally.out_of_range += 1;
        } else if !known.contains(texture) {
            tally.unknown.insert(texture.to_string());
        } else if fork.stamp_texture(x as usize, y as usize, texture).is_some() {
            tally.changed += 1;
        }
    }
    (fork, tally)
}

fn tally_skips(tally: &StampTally, skipped: &mut Vec<String>) {
    if !tally.unknown.is_empty() {
        let names: Vec<_> = tally.unknown.iter().cloned().collect();
        skipped.push(format!("textures not in this map/floortex library: {}", names.join(", ")));
    }
    if tally.out_of_range > 0 {
        skipped.push(format!("{} tiles outside the texture grid", tally.out_of_range));
    }
}

/// Road tiles → one undoable texture stroke.
pub fn apply_road(
    textures: &mut TerrainBlendEditSession,
    known: &TextureNames,
    plan: &RoadStrokePlan,
) -> Result<ApplyReport, String> {
    if !plan.succeeded {
        return Err(format!("road plan did not succeed ({} unsupported tiles)", plan.unsupported.len()));
    }
    let (mut fork, tally) = stamp_on_fork(textures, known, plan.tiles.iter().map(|t| (t.x, t.y, t.texture.as_str())));
    let mut report = ApplyReport::default();
    tally_skips(&tally, &mut report.skipped);
    if tally.changed == 0 {
        return Err(format!("nothing to apply ({})", report.summary()));
    }
    fork.commit_stroke();
    *textures = fork;
    report.textures = tally.changed;
    Ok(report)
}

/// River water/bank tiles (texture session) plus riverbed height adjustments (terrain session).
pub fn apply_river(
    terrain: &mut TerrainSession,
    textures: &mut TerrainBlendEditSession,
    known: &TextureNames,
    plan: &RiverStrokePlan,
) -> Result<ApplyReport, String> {
    if !plan.succeeded {
        return Err(format!("river plan did not succeed ({} unsupported tiles)", plan.unsupported.len()));
    }
    let heights = terrain.heights();
    let mut changes = Vec::new();
    for a in &plan.height_adjustments {
        match heights.get(a.index) {
            Some(&current) if current == a.before => changes.push(SampleChange {
                index: a.index,
                before: a.before,
                after: a.after,
            }),
            _ => return Err("river plan is stale: terrain changed since planning; plan again".into()),
        }
    }
    let stamps = plan
        .water_tiles
        .iter()
        .map(|t| (t.x, t.y, t.texture.as_str()))
        .chain(plan.bank_tiles.iter().map(|t| (t.x, t.y, t.texture.as_str())));
    let (fork, tally) = stamp_on_fork(textures, known, stamps);
    let mut report = ApplyReport::default();
    tally_skips(&tally, &mut report.skipped);
    if tally.changed == 0 && changes.is_empty() {
        return Err(format!("nothing to apply ({})", report.summary()));
    }
    if tally.changed > 0 {
        let mut fork = fork;
        fork.commit_stroke();
        *textures = fork;
        report.textures = tally.changed;
    }
    if !changes.is_empty() {
        report.heights = terrain.apply_height_changes(&changes).len();
        terrain.commit_stroke();
    }
    Ok(report)
}

// ---------------------------------------------------------------- cliffs

struct BlendAdapter<'a> {
    fork: TerrainBlendEditSession,
    known: &'a TextureNames,
    tally: StampTally,
}
impl CliffBlendSession for BlendAdapter<'_> {
    fn stamp_texture(&mut self, x: i32, y: i32, texture: &str) -> bool {
        let dim = self.fork.tile_dimension();
        if x < 0 || y < 0 || x as usize >= dim || y as usize >= dim {
            self.tally.out_of_range += 1;
            return false;
        }
        if !self.known.contains(texture) {
            self.tally.unknown.insert(texture.to_string());
            return false;
        }
        let changed = self.fork.stamp_texture(x as usize, y as usize, texture).is_some();
        self.tally.changed += changed as usize;
        changed
    }
    fn paint_tiles(&mut self, tiles: &[(i32, i32)], material: &str, auto_bridge: bool) -> bool {
        let dim = self.fork.tile_dimension();
        let tiles: Vec<_> = tiles
            .iter()
            .filter(|&&(x, y)| x >= 0 && y >= 0 && (x as usize) < dim && (y as usize) < dim)
            .map(|&(x, y)| (x as usize, y as usize))
            .collect();
        match self.fork.paint_tiles(&tiles, material, auto_bridge) {
            Ok(result) => {
                self.tally.changed += result.texture_changes.len();
                result.succeeded
            }
            Err(_) => false,
        }
    }
    fn commit_stroke(&mut self) {
        self.fork.commit_stroke();
    }
}

struct CollisionAdapter<'a> {
    terrain: &'a mut TerrainSession,
    pixels: usize,
}
impl CliffHeightSession for CollisionAdapter<'_> {
    fn block_collision(&mut self, x: i32, y: i32, _radius: f32) {
        // The plan lists every pixel explicitly: paint exactly that pixel.
        if let Ok(changes) = self.terrain.paint_collision(x as f32 + 0.5, y as f32 + 0.5, 0.5, CollisionBrushOp::Block) {
            self.pixels += changes.len();
        }
    }
    fn commit_stroke(&mut self) {
        // Deferred: the caller commits only if the whole cliff apply succeeded.
    }
}

/// Plans cliff faces with the textures this map really has, then stamps/blends them and marks
/// collision. All-or-nothing: any blend error discards the texture fork and cancels the stroke.
pub fn apply_cliffs(
    terrain: &mut TerrainSession,
    textures: &mut TerrainBlendEditSession,
    known: &TextureNames,
    texture_names: &[String],
    detection: &CliffDetectionResult,
    options: &CliffPlannerOptions,
) -> Result<ApplyReport, String> {
    let catalog = CliffTileCatalog::build_available(Some(texture_names), None).filtered_by(|t| known.contains(t));
    let plan = plan_cliffs(textures.tile_dimension(), detection, &catalog, options)?;
    if !plan.succeeded {
        let why = plan.issues.first().cloned().unwrap_or_else(|| "no usable cliff tiles".into());
        return Err(format!("cliff plan failed: {why} (this map's textures lack cliff tiles?)"));
    }
    let mut blend = BlendAdapter {
        fork: textures.fork(),
        known,
        tally: StampTally::default(),
    };
    let mut collision = CollisionAdapter { terrain, pixels: 0 };
    let result = apply_cliff_plan(&mut blend, Some(&mut collision), &plan);
    let pixels = collision.pixels;
    if !result.succeeded || !result.errors.is_empty() {
        terrain.cancel_stroke();
        return Err(format!("cliff apply rejected: {}", result.errors.join("; ")));
    }
    terrain.commit_stroke();
    *textures = blend.fork;
    let mut report = ApplyReport {
        textures: result.stamped_cliff_tiles + result.painted_scree_tiles,
        collision: pixels,
        ..Default::default()
    };
    tally_skips(&blend.tally, &mut report.skipped);
    report.skipped.extend(plan.issues.iter().take(3).cloned());
    Ok(report)
}

// ---------------------------------------------------------------- walls

/// Pre-validated wall edits: pure data so the caller can check every session before mutating.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WallEdits {
    pub collision: Vec<CollisionPixelChange>,
    pub requests: Vec<PlacementRequest>,
    pub unresolved: BTreeSet<String>,
}

pub fn plan_wall_edits(
    terrain: &TerrainSession,
    tile_dimension: usize,
    plan: &WallStrokePlan,
    resolve: &dyn Fn(&str) -> Option<i32>,
) -> Result<WallEdits, String> {
    if !plan.succeeded {
        return Err(format!(
            "wall plan did not succeed{}",
            plan.failure_reason.as_ref().map(|r| format!(": {r}")).unwrap_or_default()
        ));
    }
    let mut grid = terrain.collision().to_vec();
    let collision = synchronize_passability(&mut grid, terrain.collision_size, tile_dimension, plan)
        .map_err(|e| e.to_string())?;
    let mut edits = WallEdits {
        collision,
        ..Default::default()
    };
    for p in &plan.placements {
        match resolve(&p.name_def) {
            Some(type_id) => edits.requests.push(PlacementRequest {
                type_id,
                name: p.name_def.clone(),
                x: p.world_x,
                y: p.world_y,
                z: p.world_z,
                rotation_deg: p.angle_deg,
                team: u16::try_from(p.team).ok(),
            }),
            None => {
                edits.unresolved.insert(p.name_def.clone());
            }
        }
    }
    Ok(edits)
}

/// Writes the wall footprint into the collision layer as one undoable stroke.
pub fn commit_wall_collision(terrain: &mut TerrainSession, edits: &WallEdits) -> usize {
    let size = terrain.collision_size;
    let mut count = 0;
    for c in &edits.collision {
        let op = if c.after == 0 { CollisionBrushOp::Clear } else { CollisionBrushOp::Block };
        let (x, y) = ((c.index % size) as f32 + 0.5, (c.index / size) as f32 + 0.5);
        if let Ok(changes) = terrain.paint_collision(x, y, 0.5, op) {
            count += changes.len();
        }
    }
    terrain.commit_stroke();
    count
}

// ---------------------------------------------------------------- name → type id

/// Parses `id,name` lines (blank lines ignored) into a case-insensitive name → id map.
pub fn parse_templates(text: &str) -> Result<Vec<(i32, String)>, String> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let (id, name) = line.split_once(',').ok_or_else(|| format!("template line {} must be 'id,name'", i + 1))?;
        let id: i32 = id.trim().parse().map_err(|_| format!("template line {} has a non-numeric id", i + 1))?;
        out.push((id, name.trim().to_string()));
    }
    Ok(out)
}

pub fn resolver(templates: &[(i32, String)]) -> impl Fn(&str) -> Option<i32> + '_ {
    move |name| {
        templates
            .iter()
            .find(|(_, n)| n.eq_ignore_ascii_case(name.trim()))
            .map(|(id, _)| *id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use against_rome_landscape::{
        fortification::{plan_wall_stroke, FortificationStyle, WallStrokeOptions, WallTileCatalog},
        river::{plan as plan_river, RiverElevationMode, RiverPlannerOptions, RiverTileCatalog},
        road::{build_available, plan as plan_road},
    };
    use against_rome_textures::{StandardFloorMaterialCatalog, TerrainBlendAuthoringMap};
    use std::sync::Arc;

    fn texture_session(dim: usize, names: &[&str]) -> TerrainBlendEditSession {
        let textures = vec![names[0].to_string(); dim * dim];
        let catalog = Arc::new(StandardFloorMaterialCatalog::from_texture_names(names));
        let import = TerrainBlendAuthoringMap::import(dim, &textures, catalog.as_ref(), "BA").unwrap();
        TerrainBlendEditSession::new(import, &textures, catalog).unwrap()
    }

    #[test]
    fn road_plan_applies_as_one_undoable_stroke_and_rejects_unknown_textures() {
        let names = ["4BA___51", "4BB___51", "H_WEG1"];
        let mut session = texture_session(8, &names);
        let grid = session.current_textures().to_vec();
        let name_strings = names.map(String::from);
        let road = build_available(Some(&name_strings), Some("4BB___51"));
        let plan = plan_road(8, &grid, &[(1, 1), (5, 1)], &road, "4BB___51").unwrap();
        assert!(plan.succeeded && !plan.tiles.is_empty());
        let known = TextureNames::new(names);
        let report = apply_road(&mut session, &known, &plan).unwrap();
        assert_eq!(report.textures, plan.tiles.len());
        assert!(session.is_dirty());
        assert!(session.can_undo());
        assert!(session.get_texture(2, 1).is_ok());
        session.undo().unwrap();
        assert!(!session.is_dirty());
        assert_eq!(session.get_texture(2, 1).unwrap(), "4BA___51");
        // The same plan against a library lacking the texture changes nothing.
        let none = TextureNames::new(["4BA___51"]);
        let before = session.current_textures().to_vec();
        assert!(apply_road(&mut session, &none, &plan).is_err());
        assert_eq!(session.current_textures(), before);
        assert!(!session.is_dirty());
    }

    #[test]
    fn river_apply_stamps_carves_and_is_stale_checked() {
        let names = ["4BA___51", "4BB___51"];
        let mut textures = texture_session(8, &names);
        let mut terrain = TerrainSession::create_blank(33, 100);
        let grid = textures.current_textures().to_vec();
        let options = RiverPlannerOptions {
            preferred_river_texture: Some("4BB___51".into()),
            elevation_mode: RiverElevationMode::AutoCarveTrench,
            trench_depth: 6,
            generate_bank_transitions: false,
            ..Default::default()
        };
        let catalog = RiverTileCatalog::build_available(Some(&names.map(String::from)), Some("4BB___51"), None);
        let plan = plan_river(8, &grid, Some(terrain.heights()), 33, &[(1, 3), (6, 3)], &catalog, &options).unwrap();
        assert!(plan.succeeded);
        let known = TextureNames::new(names);
        let heights_before = terrain.heights().to_vec();
        let report = apply_river(&mut terrain, &mut textures, &known, &plan).unwrap();
        assert!(report.textures > 0 || report.heights > 0, "{report:?}");
        assert_eq!(report.heights, plan.height_adjustments.len());
        if report.heights > 0 {
            assert!(terrain.is_dirty());
            assert!(terrain.undo());
            assert_eq!(terrain.heights(), heights_before);
            // Plan is now consistent again; mutate the terrain and the plan must be refused.
            terrain.redo();
            assert!(apply_river(&mut terrain, &mut textures, &known, &plan).is_err());
        }
    }

    #[test]
    fn river_with_unknown_textures_leaves_everything_untouched() {
        let mut textures = texture_session(8, &["4BA___51"]);
        let mut terrain = TerrainSession::create_blank(33, 100);
        let grid = textures.current_textures().to_vec();
        let catalog = RiverTileCatalog::build_available(None, None, None);
        let options = RiverPlannerOptions::default();
        let plan = plan_river(8, &grid, None, 33, &[(1, 3), (6, 3)], &catalog, &options).unwrap();
        let known = TextureNames::new(["4BA___51"]);
        assert!(apply_river(&mut terrain, &mut textures, &known, &plan).is_err());
        assert!(!textures.is_dirty() && !terrain.is_dirty());
    }

    #[test]
    fn wall_collision_is_one_undoable_stroke_and_objects_are_resolved_by_name() {
        let mut terrain = TerrainSession::create_blank(33, 100);
        let size = terrain.collision_size;
        let tile_dim = 8;
        let options = WallStrokeOptions {
            style: FortificationStyle::GermanicPalisade,
            ..Default::default()
        };
        let plan = plan_wall_stroke(tile_dim as i32, &[(1, 1), (5, 1)], &options, &WallTileCatalog::default(), None, None, None);
        assert!(plan.succeeded, "{:?}", plan.failure_reason);
        let first = plan.placements[0].name_def.clone();
        let templates = vec![(7, first.clone())];
        let resolve = resolver(&templates);
        let edits = plan_wall_edits(&terrain, tile_dim, &plan, &resolve).unwrap();
        assert!(!edits.collision.is_empty());
        assert_eq!(edits.requests.iter().filter(|r| r.type_id == 7).count(), plan.placements.iter().filter(|p| p.name_def == first).count());
        assert_eq!(edits.requests.len() + plan.placements.iter().filter(|p| p.name_def != first).count(), plan.placements.len());
        assert!(!terrain.is_dirty(), "planning must not mutate");
        let blocked = commit_wall_collision(&mut terrain, &edits);
        assert_eq!(blocked, edits.collision.len());
        assert!(terrain.is_dirty());
        assert!(terrain.collision().iter().filter(|&&v| v != 0).count() > 0);
        assert!(terrain.undo());
        assert!(!terrain.is_dirty());
        assert!(terrain.collision().iter().all(|&v| v == 0));
        let _ = size;
    }

    #[test]
    fn cliffs_without_cliff_textures_are_rejected_without_changes() {
        let mut terrain = TerrainSession::create_blank(33, 0);
        let mut raised = vec![0u8; 33 * 33];
        for y in 0..33 {
            for x in 16..33 {
                raised[y * 33 + x] = 200;
            }
        }
        let changes: Vec<_> = raised
            .iter()
            .enumerate()
            .map(|(index, &after)| SampleChange { index, before: 0, after })
            .collect();
        terrain.apply_height_changes(&changes);
        terrain.commit_stroke();
        let mut textures = texture_session(8, &["4BA___51", "4BB___51"]);
        let detection = against_rome_landscape::cliff::detect_from_vertex_heights(33, terrain.heights(), 8, 4, 30.0, 8.0).unwrap();
        assert!(!detection.cliff_cells.is_empty());
        let names: Vec<String> = textures.current_textures().to_vec();
        let known = TextureNames::new(&names);
        let collision_before = terrain.collision().to_vec();
        let err = apply_cliffs(&mut terrain, &mut textures, &known, &names, &detection, &CliffPlannerOptions::default());
        assert!(err.is_err());
        assert_eq!(terrain.collision(), collision_before);
        assert!(!textures.is_dirty());
    }

    #[test]
    fn cliffs_with_real_cliff_textures_stamp_and_block_collision() {
        let mut terrain = TerrainSession::create_blank(33, 0);
        let changes: Vec<_> = (0..33 * 33)
            .filter(|i| i % 33 >= 16)
            .map(|index| SampleChange { index, before: 0, after: 200 })
            .collect();
        terrain.apply_height_changes(&changes);
        terrain.commit_stroke();
        let names = ["4BA___51", "FELS_AA_008", "FELS_AA_004", "FELS_AA_002", "FELS_AA_006"];
        let mut textures = texture_session(8, &names);
        let detection = against_rome_landscape::cliff::detect_from_vertex_heights(33, terrain.heights(), 8, 4, 30.0, 8.0).unwrap();
        let name_list: Vec<String> = names.iter().map(|s| s.to_string()).collect();
        let known = TextureNames::new(names);
        let options = CliffPlannerOptions {
            generate_scree: false,
            ..Default::default()
        };
        let report = apply_cliffs(&mut terrain, &mut textures, &known, &name_list, &detection, &options).unwrap();
        assert!(report.textures > 0, "{report:?}");
        assert!(report.collision > 0);
        assert!(textures.is_dirty() && terrain.is_dirty());
        assert!(textures.undo().is_some());
        assert!(!textures.is_dirty());
    }

    #[test]
    fn template_parsing_and_resolution() {
        let t = parse_templates("1, Tanne \n\n2,Busch").unwrap();
        assert_eq!(t, vec![(1, "Tanne".to_string()), (2, "Busch".to_string())]);
        assert_eq!(resolver(&t)("busch"), Some(2));
        assert_eq!(resolver(&t)("Fels"), None);
        assert!(parse_templates("x").is_err());
        assert!(parse_templates("a,b").is_err());
    }
}
