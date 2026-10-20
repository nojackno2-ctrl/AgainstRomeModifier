//! Starter kit on an existing blank map (`against_rome_maps::spawn_data::stage_starter_kit`,
//! `plan_spawn_data_with_kit`; C# `ApplyStarterKit` / `ImportPendingStarterKit`) on synthetic
//! temp-dir maps: gate refusals, terrain validation, DATA family import, translation of staged
//! kit spawns and the all-or-nothing behavior (planning never writes).

use against_rome_maps::sdl_objects::{SdlObjectCategory, SdlObjectType, SdlPlacedObject};
use against_rome_maps::spawn_data::{
    mark_prebuilt, plan_spawn_data, plan_spawn_data_with_kit, stage_starter_kit, PendingStarterKit, SpawnDataCatalog,
    StarterKitStage, StarterKitTerrain,
};
use against_rome_maps::starter_kit::native::{ObjectStore, COLUMNS, SEGMENTS};
use against_rome_scenario::model::{ScenarioDocument, Spawn};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const HOUSE: &str = "BauGerHau00_Haupthaus";
const UNIT: &str = "FigGerInf02_Zweihandaxt";

fn write_map(path: &Path, n: usize, live: &[(u16, Option<u16>)], cursor: u32) {
    fs::create_dir_all(path.join("DATA")).unwrap();
    let mut o = vec![0u8; 16 + n * (79 + COLUMNS.iter().sum::<usize>())];
    for (i, v) in [1u32, n as u32, 30, 30].iter().enumerate() {
        o[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    let mut p = vec![0u8; 8 + n * 3 * 17];
    p[..4].copy_from_slice(&1u32.to_le_bytes());
    p[4..8].copy_from_slice(&((n * 3) as u32).to_le_bytes());
    let mut d = vec![0u8; 8 + n * SEGMENTS.iter().sum::<usize>()];
    d[..4].copy_from_slice(&1u32.to_le_bytes());
    d[4..8].copy_from_slice(&(n as u32).to_le_bytes());
    for s in 0..n {
        let r = 16 + s * 79;
        for off in [67, 69, 71, 73, 77] {
            o[r + off..r + off + 2].fill(255);
        }
        for c in 0..18 {
            if COLUMNS[c] == 2 {
                let off = 16 + n * 79 + n * COLUMNS[..c].iter().sum::<usize>() + s * COLUMNS[c];
                o[off..off + 2].fill(255);
            }
        }
        if let Some(&(typ, parent)) = live.get(s) {
            o[r] = 1;
            o[r + 3..r + 7].copy_from_slice(&(s as u32 + 1).to_le_bytes());
            o[r + 75..r + 77].copy_from_slice(&typ.to_le_bytes());
            for off in [67, 69] {
                o[r + off..r + off + 2].copy_from_slice(&(s as u16).to_le_bytes());
            }
            for (c, v) in [(1, s as u16), (13, parent.unwrap_or(65535))] {
                let off = 16 + n * 79 + n * COLUMNS[..c].iter().sum::<usize>() + s * COLUMNS[c];
                o[off..off + 2].copy_from_slice(&v.to_le_bytes());
            }
            let off = 8 + s * 17;
            p[off] = 1;
            for (a, v) in [4000. + s as f32 * 20., 20., 4000.].iter().enumerate() {
                p[off + 1 + a * 4..off + 5 + a * 4].copy_from_slice(&v.to_le_bytes());
            }
            p[off + 13..off + 17].copy_from_slice(&90f32.to_le_bytes());
            d[8 + s * 9] = 1;
        }
    }
    let mut l = vec![0u8; against_rome_data::pool::PAYLOAD_LEN];
    l[..4].copy_from_slice(&1u32.to_le_bytes());
    l[4..8].copy_from_slice(&1024u32.to_le_bytes());
    let mut e = vec![0u8; 94];
    e[..4].copy_from_slice(&1u32.to_le_bytes());
    e[34..38].copy_from_slice(&cursor.to_le_bytes());
    for (name, b) in [("objects", o), ("position", p), ("objdata", d), ("light", l), ("engine", e)] {
        fs::write(path.join(format!("DATA/{name}.dat")), b).unwrap();
    }
}

struct Fx {
    root: PathBuf,
    map: PathBuf,
}
impl Drop for Fx {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn idle() -> Vec<u8> {
    against_rome_scenario::BciImage::idle().serialize()
}

/// Game root with the starter aliases/objdefs, an official `TUTOR_01` holding a linked house
/// family (232 + child 234) and an axeman (218), and a blank custom map `ENDL_005`.
fn fixture(work_live: &[(u16, Option<u16>)]) -> Fx {
    let root = std::env::temp_dir().join(format!("arm-starter-import-{}", Uuid::new_v4()));
    fs::create_dir_all(root.join("SYSTEM/CLAK")).unwrap();
    fs::create_dir_all(root.join("SYSTEM/DATA_MP/DEFAULTS")).unwrap();
    fs::write(
        root.join("SYSTEM/CLAK/cl_scint.ini"),
        format!("[ObjDefName]\nGER_HAU00={HOUSE}\nGER_INF02={UNIT}\n"),
    )
    .unwrap();
    let lines = [(232, HOUSE), (218, UNIT)].map(|(id, name)| {
        let mut fields = vec!["0".to_string(); 53];
        fields[0] = id.to_string();
        fields[52] = name.into();
        fields.join(",")
    });
    fs::write(root.join("SYSTEM/DATA_MP/DEFAULTS/objdef.dau"), lines.join("\n")).unwrap();
    write_map(&root.join("MAPS/TUTOR_01"), 6, &[(232, None), (234, Some(0)), (218, None)], 50);
    let map = root.join("MAPS/ENDL_005");
    write_map(&map, 12, work_live, 100);
    fs::create_dir_all(map.join("SCRIPT")).unwrap();
    fs::write(map.join(".arm_custom_map"), "{}").unwrap();
    for name in ["ak_level.bci", "ak_level.arm_original"] {
        fs::write(map.join("SCRIPT").join(name), idle()).unwrap();
    }
    Fx { root, map }
}

/// Flat dry open terrain: 65x65 vertices at 32 * 4 = 128 world units, 256x256 free collision.
struct Terrain {
    heights: Vec<u8>,
    collision: Vec<u8>,
}
impl Terrain {
    fn flat() -> Self {
        Self { heights: vec![32; 65 * 65], collision: vec![0; 256 * 256] }
    }
    fn view(&self, water_world: f32) -> StarterKitTerrain<'_> {
        StarterKitTerrain {
            heights: &self.heights,
            vertex_size: 65,
            height_step: 4.0,
            collision: &self.collision,
            collision_size: 256,
            water_world,
        }
    }
}

fn disk(map: &Path) -> Vec<Vec<u8>> {
    ["objects", "objdata", "position", "light", "engine"]
        .iter()
        .map(|n| fs::read(map.join("DATA").join(format!("{n}.dat"))).unwrap())
        .collect()
}

fn stage(f: &Fx, team: i32, x: f32, z: f32, terrain: &StarterKitTerrain) -> Result<StarterKitStage, String> {
    stage_starter_kit(&f.root, &f.map, &ScenarioDocument::default(), team, x, z, terrain)
}

fn placed_of(spawn: &Spawn) -> SdlPlacedObject {
    let figure = spawn.alias == "GER_INF02";
    let (name, category) = if figure { (UNIT, SdlObjectCategory::Figure) } else { (HOUSE, SdlObjectCategory::Building) };
    let kind = SdlObjectType::new(
        name,
        -1,
        category,
        "Ger",
        0,
        HashMap::from([("alias".to_string(), spawn.alias.clone())]),
    );
    let mut item = SdlPlacedObject::new(kind, spawn.x, spawn.y, spawn.z, spawn.team, spawn.angle as f32, 0);
    item.scenario_id = spawn.id;
    item.raw_rotation = spawn.raw_rotation;
    item
}

/// The editor document after the kit was staged: spawns as planned (prebuilt flags from the
/// placement rule, bindings still empty because they are created by the save transaction).
fn staged_doc(stage: &StarterKitStage) -> (ScenarioDocument, Vec<SdlPlacedObject>) {
    let placed: Vec<_> = stage.spawns.iter().map(placed_of).collect();
    let mut doc = ScenarioDocument { spawns: stage.spawns.clone(), ..Default::default() };
    for spawn in &mut doc.spawns {
        spawn.prebuilt = false;
    }
    mark_prebuilt(&mut doc.spawns, &placed);
    (doc, placed)
}

fn ids(stage: &StarterKitStage) -> HashSet<Uuid> {
    stage.kit.spawns.iter().map(|s| s.id).collect()
}

fn copy_with_plan(files: &[(PathBuf, Vec<u8>)], map: &Path, dir: &Path) -> ObjectStore {
    let copy = dir.join(format!("applied-{}", Uuid::new_v4()));
    fs::create_dir_all(copy.join("DATA")).unwrap();
    for entry in fs::read_dir(map.join("DATA")).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), copy.join("DATA").join(entry.file_name())).unwrap();
    }
    for (rel, bytes) in files {
        fs::write(copy.join(rel), bytes).unwrap();
    }
    ObjectStore::load(&copy).unwrap()
}

#[test]
fn staging_builds_five_native_spawns_and_writes_nothing() {
    let f = fixture(&[]);
    let before = disk(&f.map);
    let t = Terrain::flat();
    let staged = stage(&f, 2, 8192.0, 8192.0, &t.view(0.0)).unwrap();
    assert_eq!(disk(&f.map), before, "staging is detached");
    assert_eq!(staged.spawns.len(), 5);
    assert_eq!(staged.spawns[0].alias, "GER_HAU00");
    assert!(staged.spawns[1..].iter().all(|s| s.alias == "GER_INF02"));
    assert!(staged.spawns.iter().all(|s| s.team == 2 && s.count == 0 && s.raw_rotation.is_some()));
    assert_eq!((staged.spawns[0].x, staged.spawns[0].z), (8192.0, 8192.0));
    assert_eq!((staged.spawns[1].x, staged.spawns[1].z), (8192.0 - 480.0, 8192.0 + 800.0));
    assert_eq!(staged.spawns[0].y, 128.0, "Y from the terrain");
    assert_eq!(ids(&staged).len(), 5);
}

#[test]
fn gate_refuses_every_non_blank_map_without_writing() {
    let t = Terrain::flat();
    // Native DATA is not empty.
    let f = fixture(&[(232, None)]);
    let before = disk(&f.map);
    assert!(stage(&f, 0, 8192.0, 8192.0, &t.view(0.0)).is_err());
    assert_eq!(disk(&f.map), before);
    // Scenario already has a placement.
    let f = fixture(&[]);
    let mut previous = ScenarioDocument::default();
    previous.spawns.push(Spawn { id: Uuid::from_u128(5), alias: "GER_INF02".into(), ..Default::default() });
    assert!(stage_starter_kit(&f.root, &f.map, &previous, 0, 8192.0, 8192.0, &t.view(0.0)).is_err());
    // Level script (or its backup) is not the exact idle bootstrap.
    for name in ["ak_level.bci", "ak_level.arm_original"] {
        let f = fixture(&[]);
        fs::write(f.map.join("SCRIPT").join(name), b"not an idle level").unwrap();
        let error = stage(&f, 0, 8192.0, 8192.0, &t.view(0.0)).unwrap_err();
        assert!(error.contains("閒置開局腳本"), "{error}");
    }
    let f = fixture(&[]);
    fs::remove_file(f.map.join("SCRIPT/ak_level.arm_original")).unwrap();
    assert!(stage(&f, 0, 8192.0, 8192.0, &t.view(0.0)).is_err(), "a missing backup is not an idle bootstrap");
    // Not an editable custom map (retail name).
    let f = fixture(&[]);
    let retail = f.root.join("MAPS/TUTOR_02");
    fs::create_dir_all(retail.join("DATA")).unwrap();
    for entry in fs::read_dir(f.map.join("DATA")).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), retail.join("DATA").join(entry.file_name())).unwrap();
    }
    assert!(stage_starter_kit(&f.root, &retail, &ScenarioDocument::default(), 0, 8192.0, 8192.0, &t.view(0.0)).is_err());
}

#[test]
fn terrain_validation_rejects_water_blocked_steep_and_out_of_range() {
    let f = fixture(&[]);
    let t = Terrain::flat();
    assert!(stage(&f, 0, 8192.0, 8192.0, &t.view(127.0)).unwrap_err().contains("水面"));
    assert!(stage(&f, 0, 8192.0, 8192.0, &t.view(300.0)).unwrap_err().contains("水面"));
    // A blocked collision tile under the house.
    let mut blocked = Terrain::flat();
    blocked.collision[(8192 / 64) * 256 + 8192 / 64] = 255;
    assert!(stage(&f, 0, 8192.0, 8192.0, &blocked.view(0.0)).unwrap_err().contains("阻擋"));
    // Slope above 32 world units across the family extent.
    let mut steep = Terrain::flat();
    for vz in 0..65 {
        for vx in 0..65 {
            steep.heights[vz * 65 + vx] = (vz * 8).min(255) as u8;
        }
    }
    assert!(stage(&f, 0, 8192.0, 8192.0, &steep.view(0.0)).unwrap_err().contains("高差"));
    // Outside the map / invalid team.
    assert!(stage(&f, 0, 100.0, 8192.0, &t.view(0.0)).is_err());
    assert!(stage(&f, 9, 8192.0, 8192.0, &t.view(0.0)).is_err());
    // Untrusted terrain.
    let empty = StarterKitTerrain { heights: &[], vertex_size: 0, height_step: 4.0, collision: &[], collision_size: 0, water_world: 0.0 };
    assert!(stage(&f, 0, 8192.0, 8192.0, &empty).is_err());
}

#[test]
fn save_plan_imports_the_linked_main_house_family_and_binds_all_five() {
    let f = fixture(&[]);
    let t = Terrain::flat();
    let terrain = t.view(0.0);
    let staged = stage(&f, 2, 8192.0, 8192.0, &terrain).unwrap();
    let catalog = SpawnDataCatalog::from_game(&f.root).unwrap();
    let (next, placed) = staged_doc(&staged);
    assert!(next.spawns.iter().all(|s| s.prebuilt), "raw-rotation placements are prebuilt");
    let before = disk(&f.map);

    // Without the pending kit the bindings of the new native-rotation spawns cannot be created.
    assert!(plan_spawn_data(&f.map, &catalog, &ScenarioDocument::default(), &next, &placed).is_err());

    let live = ids(&staged);
    let pending = PendingStarterKit { kit: &staged.kit, ids: &live, terrain: &terrain };
    let plan = plan_spawn_data_with_kit(&f.map, &catalog, &ScenarioDocument::default(), &next, &placed, Some(&pending))
        .unwrap()
        .expect("kit import is a DATA transaction");
    assert_eq!(disk(&f.map), before, "planning must not write");
    assert!(plan.skipped.is_empty());
    assert_eq!(plan.scenario.data_slots.len(), 5);
    assert!(plan.scenario.spawns.iter().all(|s| s.prebuilt && s.raw_rotation.is_some() && s.count == 0));
    against_rome_scenario::verify_binding_snapshot(&plan.scenario, &plan.active).unwrap();
    let store = copy_with_plan(&plan.files, &f.map, &f.root);
    let objects = store.objects();
    assert_eq!(objects.len(), 6, "house family (2) + 4 axemen");
    assert!(objects.iter().any(|o| o.type_id == 234), "the linked house child came with the family");
    assert!(objects.iter().all(|o| o.team == 2));
    let house = plan.scenario.spawns.iter().find(|s| s.alias == "GER_HAU00").unwrap();
    let binding = plan.scenario.data_slots.iter().find(|b| b.spawn_id == house.id).unwrap();
    assert_eq!(store.family(binding.slot as usize, binding.uid).unwrap().len(), 2);
    let root = objects.iter().find(|o| o.slot as i32 == binding.slot).unwrap();
    assert_eq!(root.position, [8192.0, 128.0, 8192.0]);
}

#[test]
fn moved_kit_spawns_translate_their_families_at_save() {
    let f = fixture(&[]);
    let t = Terrain::flat();
    let terrain = t.view(0.0);
    let staged = stage(&f, 0, 8192.0, 8192.0, &terrain).unwrap();
    let catalog = SpawnDataCatalog::from_game(&f.root).unwrap();
    let (mut next, mut placed) = staged_doc(&staged);
    for spawn in &mut next.spawns {
        spawn.x += 512.0;
    }
    for item in &mut placed {
        item.world_x += 512.0;
    }
    let live = ids(&staged);
    let pending = PendingStarterKit { kit: &staged.kit, ids: &live, terrain: &terrain };
    let plan = plan_spawn_data_with_kit(&f.map, &catalog, &ScenarioDocument::default(), &next, &placed, Some(&pending))
        .unwrap()
        .unwrap();
    let store = copy_with_plan(&plan.files, &f.map, &f.root);
    let house = plan.scenario.spawns.iter().find(|s| s.alias == "GER_HAU00").unwrap();
    let binding = plan.scenario.data_slots.iter().find(|b| b.spawn_id == house.id).unwrap();
    let root = store.objects().into_iter().find(|o| o.slot as i32 == binding.slot).unwrap();
    assert_eq!(root.position[0], 8192.0 + 512.0);
}

#[test]
fn save_time_failures_publish_nothing() {
    let f = fixture(&[]);
    let t = Terrain::flat();
    let terrain = t.view(0.0);
    let staged = stage(&f, 0, 8192.0, 8192.0, &terrain).unwrap();
    let catalog = SpawnDataCatalog::from_game(&f.root).unwrap();
    let (next, placed) = staged_doc(&staged);
    let live = ids(&staged);
    let before = disk(&f.map);

    // The terrain turned wet after staging (validated again on the final candidate).
    let wet = t.view(500.0);
    let pending = PendingStarterKit { kit: &staged.kit, ids: &live, terrain: &wet };
    let error = plan_spawn_data_with_kit(&f.map, &catalog, &ScenarioDocument::default(), &next, &placed, Some(&pending)).unwrap_err();
    assert!(error.contains("水面"), "{error}");
    assert_eq!(disk(&f.map), before);

    // The map was filled behind our back: the blank gate refuses at save.
    write_map(&f.map, 12, &[(232, None)], 100);
    let filled = disk(&f.map);
    let pending = PendingStarterKit { kit: &staged.kit, ids: &live, terrain: &terrain };
    assert!(plan_spawn_data_with_kit(&f.map, &catalog, &ScenarioDocument::default(), &next, &placed, Some(&pending)).is_err());
    assert_eq!(disk(&f.map), filled);

    // A kit whose placements are all gone is ignored (empty id set, nothing prebuilt).
    write_map(&f.map, 12, &[], 100);
    let none = HashSet::new();
    let pending = PendingStarterKit { kit: &staged.kit, ids: &none, terrain: &terrain };
    let empty_doc = ScenarioDocument::default();
    assert!(plan_spawn_data_with_kit(&f.map, &catalog, &empty_doc, &empty_doc, &[], Some(&pending)).unwrap().is_none());
}
