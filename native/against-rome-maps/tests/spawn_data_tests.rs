//! Spawn -> DATA save planning (`against_rome_maps::spawn_data`) on synthetic maps in temp dirs:
//! prebuilt conversion, template fallback, bound-spawn move/delete, family copies (ordinary and
//! native-source) and failure paths that must publish nothing.

use against_rome_maps::sdl_objects::{NativeObjectIdentity, SdlObjectCategory, SdlObjectType, SdlPlacedObject};
use against_rome_maps::script_aliases::ScriptObjectAlias;
use against_rome_maps::spawn_data::{intended_prebuilt, mark_prebuilt, plan_spawn_data, SpawnDataCatalog, SpawnDataPlan};
use against_rome_maps::starter_kit::native::{ObjectStore, COLUMNS, SEGMENTS};
use against_rome_scenario::model::{DataSlot, ScenarioDocument, Spawn};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const HOUSE: &str = "BauGerHau00_Haupthaus";

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

struct Fixture {
    root: PathBuf,
    map: PathBuf,
    catalog: SpawnDataCatalog,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// `live` describes the working map; the official map holds a singleton house (type 232).
fn fixture(live: &[(u16, Option<u16>)]) -> Fixture {
    let root = std::env::temp_dir().join(format!("arm-spawn-data-{}", Uuid::new_v4()));
    let map = root.join("work").join("ENDL_005");
    write_map(&map, 12, live, 100);
    write_map(&root.join("game").join("MAPS").join("OFFICIAL_A"), 6, &[(232, None)], 50);
    let aliases = [("HOUSE", HOUSE), ("OTHER", "BauRomOther"), ("TROOP", "FigGerInf02_Zweihandaxt")]
        .map(|(a, n)| ScriptObjectAlias { alias: a.into(), name_def: n.into() })
        .to_vec();
    let type_names = HashMap::from([(232, HOUSE.to_string()), (233, "BauRomOther".to_string())]);
    let catalog = SpawnDataCatalog { aliases, type_names, game_maps: Some(root.join("game").join("MAPS")) };
    Fixture { root, map, catalog }
}

fn disk_bytes(map: &Path) -> Vec<Vec<u8>> {
    ["objects", "objdata", "position", "light", "engine"]
        .iter()
        .map(|n| fs::read(map.join("DATA").join(format!("{n}.dat"))).unwrap())
        .collect()
}

fn house_type(alias: &str) -> SdlObjectType {
    SdlObjectType::new(
        HOUSE,
        -1,
        SdlObjectCategory::Building,
        "Ger",
        0,
        HashMap::from([("alias".to_string(), alias.to_string())]),
    )
}

fn spawn(id: u128, alias: &str, x: f32, z: f32, team: i32, angle: i32, prebuilt: bool) -> Spawn {
    Spawn { id: Uuid::from_u128(id), alias: alias.into(), x, y: 20.0, z, team, angle, prebuilt, ..Default::default() }
}

fn placed(id: u128, alias: &str, x: f32, z: f32, team: i32, angle: f32) -> SdlPlacedObject {
    let mut item = SdlPlacedObject::new(house_type(alias), x, 20.0, z, team, angle, 0);
    item.scenario_id = Uuid::from_u128(id);
    item
}

fn doc(spawns: Vec<Spawn>, slots: Vec<DataSlot>) -> ScenarioDocument {
    ScenarioDocument { spawns, data_slots: slots, ..Default::default() }
}

/// Writes the staged pools onto a copy of the map and reloads it.
fn apply(plan: &SpawnDataPlan, map: &Path, dir: &Path) -> ObjectStore {
    let copy = dir.join(format!("applied-{}", Uuid::new_v4()));
    fs::create_dir_all(copy.join("DATA")).unwrap();
    for entry in fs::read_dir(map.join("DATA")).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), copy.join("DATA").join(entry.file_name())).unwrap();
    }
    for (rel, bytes) in &plan.files {
        fs::write(copy.join(rel), bytes).unwrap();
    }
    ObjectStore::load(&copy).unwrap()
}

struct View {
    type_id: u16,
    team: u16,
    position: [f32; 3],
}

fn object_at(store: &ObjectStore, slot: i32, uid: u32) -> View {
    let o = store.objects().into_iter().find(|o| o.slot as i32 == slot && o.uid == uid).expect("bound object exists");
    View { type_id: o.type_id, team: o.team, position: o.position }
}

#[test]
fn prebuilt_rule_matches_csharp() {
    let mut item = placed(1, "HOUSE", 100.0, 100.0, 0, 0.0);
    assert!(intended_prebuilt(&item));
    item.team = 8;
    assert!(intended_prebuilt(&item));
    item.team = 9;
    assert!(!intended_prebuilt(&item));
    item.team = -1;
    assert!(!intended_prebuilt(&item));
    let mut unit = placed(2, "TROOP", 1.0, 1.0, 0, 0.0);
    unit.obj_type = SdlObjectType::new("FigGerInf02_Zweihandaxt", -1, SdlObjectCategory::Figure, "Ger", 0, HashMap::new());
    assert!(!intended_prebuilt(&unit));
    unit.raw_rotation = Some(1.0);
    assert!(intended_prebuilt(&unit));

    let mut spawns = vec![spawn(1, "HOUSE", 1.0, 1.0, 0, 0, false), spawn(99, "FOREIGN", 1.0, 1.0, 0, 0, false)];
    mark_prebuilt(&mut spawns, &[placed(1, "HOUSE", 1.0, 1.0, 0, 0.0)]);
    assert!(spawns[0].prebuilt);
    assert!(!spawns[1].prebuilt, "spawns without a placement are untouched");
}

#[test]
fn new_building_becomes_a_prebuilt_data_object_and_nothing_is_written() {
    let f = fixture(&[]);
    let before = disk_bytes(&f.map);
    let next = doc(vec![spawn(1, "HOUSE", 9000.0, 9000.0, 3, 180, true)], vec![]);
    let plan = plan_spawn_data(&f.map, &f.catalog, &ScenarioDocument::default(), &next, &[placed(1, "HOUSE", 9000.0, 9000.0, 3, 180.0)])
        .unwrap()
        .expect("prebuilt spawn needs a DATA transaction");
    assert_eq!(disk_bytes(&f.map), before, "planning must not write");
    assert!(plan.skipped.is_empty());
    assert_eq!(plan.scenario.data_slots.len(), 1);
    assert!(plan.scenario.spawns[0].prebuilt);
    let names: Vec<_> = plan.files.iter().map(|(p, _)| p.to_string_lossy().replace('\\', "/")).collect();
    assert!(names.contains(&"DATA/objects.dat".to_string()) && names.contains(&"DATA/position.dat".to_string()), "{names:?}");
    let store = apply(&plan, &f.map, &f.root);
    let binding = &plan.scenario.data_slots[0];
    assert_eq!(binding.spawn_id, Uuid::from_u128(1));
    assert!(plan.active.contains(&(binding.slot, binding.uid)));
    let o = object_at(&store, binding.slot, binding.uid);
    assert_eq!((o.type_id, o.team), (232, 3));
    assert_eq!(o.position, [9000.0, 20.0, 9000.0]);
    against_rome_scenario::verify_binding_snapshot(&plan.scenario, &plan.active).unwrap();
}

#[test]
fn building_without_a_template_falls_back_to_a_script_spawn() {
    let f = fixture(&[]);
    let before = disk_bytes(&f.map);
    // "OTHER" (type 233) has no template in any official map nor in the working map.
    let next = doc(vec![spawn(2, "OTHER", 5000.0, 5000.0, 0, 0, true)], vec![]);
    let plan = plan_spawn_data(&f.map, &f.catalog, &ScenarioDocument::default(), &next, &[placed(2, "OTHER", 5000.0, 5000.0, 0, 0.0)])
        .unwrap()
        .unwrap();
    assert_eq!(plan.skipped, vec![Uuid::from_u128(2)]);
    assert!(!plan.scenario.spawns[0].prebuilt);
    assert!(plan.scenario.data_slots.is_empty());
    assert!(plan.files.is_empty());
    assert_eq!(disk_bytes(&f.map), before);
}

#[test]
fn no_prebuilt_involved_means_no_data_transaction() {
    let f = fixture(&[]);
    let next = doc(vec![spawn(3, "HOUSE", 1000.0, 1000.0, 12, 0, false)], vec![]);
    assert!(plan_spawn_data(&f.map, &f.catalog, &ScenarioDocument::default(), &next, &[]).unwrap().is_none());
}

#[test]
fn bound_singleton_is_rebuilt_with_new_position_team_and_angle() {
    let f = fixture(&[(232, None)]);
    let prev = doc(vec![spawn(1, "HOUSE", 4000.0, 4000.0, 0, 90, true)], vec![DataSlot { slot: 0, uid: 1, spawn_id: Uuid::from_u128(1) }]);
    let next = doc(vec![spawn(1, "HOUSE", 5120.0, 6000.0, 3, 180, true)], vec![]);
    let plan = plan_spawn_data(&f.map, &f.catalog, &prev, &next, &[placed(1, "HOUSE", 5120.0, 6000.0, 3, 180.0)])
        .unwrap()
        .unwrap();
    let store = apply(&plan, &f.map, &f.root);
    assert_eq!(store.objects().len(), 1, "old object replaced, not duplicated");
    let b = &plan.scenario.data_slots[0];
    let o = object_at(&store, b.slot, b.uid);
    assert_eq!((o.type_id, o.team, o.position), (232, 3, [5120.0, 20.0, 6000.0]));
}

#[test]
fn deleting_a_bound_spawn_removes_its_data_object() {
    let f = fixture(&[(232, None)]);
    let prev = doc(vec![spawn(1, "HOUSE", 4000.0, 4000.0, 0, 90, true)], vec![DataSlot { slot: 0, uid: 1, spawn_id: Uuid::from_u128(1) }]);
    let plan = plan_spawn_data(&f.map, &f.catalog, &prev, &doc(vec![], vec![]), &[]).unwrap().unwrap();
    assert!(plan.scenario.data_slots.is_empty() && plan.scenario.spawns.is_empty());
    assert!(apply(&plan, &f.map, &f.root).objects().is_empty());
}

#[test]
fn duplicate_of_a_bound_family_copies_the_whole_data_family() {
    // Root (slot 0) with a child (slot 1): not a singleton, so the copy must clone the family.
    let f = fixture(&[(232, None), (233, Some(0))]);
    let prev = doc(vec![spawn(1, "HOUSE", 4000.0, 4000.0, 0, 90, true)], vec![DataSlot { slot: 0, uid: 1, spawn_id: Uuid::from_u128(1) }]);
    let next = doc(
        vec![spawn(1, "HOUSE", 4000.0, 4000.0, 0, 90, true), spawn(2, "HOUSE", 4256.0, 4256.0, 0, 90, true)],
        vec![],
    );
    let mut copy = placed(2, "HOUSE", 4256.0, 4256.0, 0, 90.0);
    copy.copy_source_scenario_id = Uuid::from_u128(1);
    let plan = plan_spawn_data(&f.map, &f.catalog, &prev, &next, &[placed(1, "HOUSE", 4000.0, 4000.0, 0, 90.0), copy])
        .unwrap()
        .unwrap();
    let store = apply(&plan, &f.map, &f.root);
    assert_eq!(store.objects().len(), 4, "family of two cloned next to the original");
    assert_eq!(plan.scenario.data_slots.len(), 2);
    let original = plan.scenario.data_slots.iter().find(|b| b.spawn_id == Uuid::from_u128(1)).unwrap();
    let copied = plan.scenario.data_slots.iter().find(|b| b.spawn_id == Uuid::from_u128(2)).unwrap();
    assert_eq!((original.slot, original.uid), (0, 1), "original family is retained in place");
    assert_eq!(object_at(&store, copied.slot, copied.uid).position, [4256.0, 20.0, 4256.0]);
    assert_eq!(store.family(copied.slot as usize, copied.uid).unwrap().len(), 2);
    assert_eq!(store.family(0, 1).unwrap().len(), 2);
}

#[test]
fn native_family_copy_requires_exact_native_rotation_and_alias() {
    let f = fixture(&[(232, None), (233, Some(0))]);
    let radians = (90f32 as f64 * (std::f64::consts::PI / 180.0)) as f32;
    let mut s = spawn(5, "HOUSE", 4500.0, 4500.0, 0, 90, true);
    s.raw_rotation = Some(radians);
    let mut item = placed(5, "HOUSE", 4500.0, 4500.0, 0, 90.0);
    item.raw_rotation = Some(radians);
    item.native_source_identity = Some(NativeObjectIdentity { slot: 0, uid: 1 });
    let next = doc(vec![s.clone()], vec![]);
    let plan = plan_spawn_data(&f.map, &f.catalog, &ScenarioDocument::default(), &next, &[item.clone()]).unwrap().unwrap();
    let store = apply(&plan, &f.map, &f.root);
    assert_eq!(store.objects().len(), 4);
    assert_eq!(plan.scenario.data_slots.len(), 1);
    let b = &plan.scenario.data_slots[0];
    assert_ne!((b.slot, b.uid), (0, 1), "the native source stays untouched and unbound");
    assert_eq!(object_at(&store, b.slot, b.uid).position[0], 4500.0);

    // A rotation that is not bit-identical to the source is refused.
    let mut off = item;
    off.raw_rotation = Some(radians + 0.01);
    s.raw_rotation = off.raw_rotation;
    let before = disk_bytes(&f.map);
    assert!(plan_spawn_data(&f.map, &f.catalog, &ScenarioDocument::default(), &doc(vec![s], vec![]), &[off]).is_err());
    assert_eq!(disk_bytes(&f.map), before);
}

#[test]
fn bound_copy_with_changed_team_fails_closed_without_writing() {
    let f = fixture(&[(232, None), (233, Some(0))]);
    let before = disk_bytes(&f.map);
    let prev = doc(vec![spawn(1, "HOUSE", 4000.0, 4000.0, 0, 90, true)], vec![DataSlot { slot: 0, uid: 1, spawn_id: Uuid::from_u128(1) }]);
    let next = doc(
        vec![spawn(1, "HOUSE", 4000.0, 4000.0, 0, 90, true), spawn(2, "HOUSE", 4256.0, 4256.0, 3, 90, true)],
        vec![],
    );
    let mut copy = placed(2, "HOUSE", 4256.0, 4256.0, 3, 90.0);
    copy.copy_source_scenario_id = Uuid::from_u128(1);
    let err = plan_spawn_data(&f.map, &f.catalog, &prev, &next, &[placed(1, "HOUSE", 4000.0, 4000.0, 0, 90.0), copy]).unwrap_err();
    assert!(err.contains("平移"), "{err}");
    assert_eq!(disk_bytes(&f.map), before);
}

#[test]
fn bound_family_cannot_be_rotated_or_changed_team_only_translated() {
    let f = fixture(&[(232, None), (233, Some(0))]);
    let prev = doc(vec![spawn(1, "HOUSE", 4000.0, 4000.0, 0, 90, true)], vec![DataSlot { slot: 0, uid: 1, spawn_id: Uuid::from_u128(1) }]);
    let moved = doc(vec![spawn(1, "HOUSE", 5000.0, 4000.0, 0, 90, true)], vec![]);
    let plan = plan_spawn_data(&f.map, &f.catalog, &prev, &moved, &[placed(1, "HOUSE", 5000.0, 4000.0, 0, 90.0)]).unwrap().unwrap();
    let store = apply(&plan, &f.map, &f.root);
    assert_eq!(plan.scenario.data_slots[0].slot, 0, "translation keeps the family bound to its slot");
    assert_eq!(object_at(&store, 0, 1).position[0], 5000.0);
    assert_eq!(store.family(0, 1).unwrap().len(), 2);
    let turned = doc(vec![spawn(1, "HOUSE", 5000.0, 4000.0, 0, 180, true)], vec![]);
    assert!(plan_spawn_data(&f.map, &f.catalog, &prev, &turned, &[placed(1, "HOUSE", 5000.0, 4000.0, 0, 180.0)]).is_err());
    let recolored = doc(vec![spawn(1, "HOUSE", 5000.0, 4000.0, 2, 90, true)], vec![]);
    assert!(plan_spawn_data(&f.map, &f.catalog, &prev, &recolored, &[placed(1, "HOUSE", 5000.0, 4000.0, 2, 90.0)]).is_err());
}

#[test]
fn capacity_exhaustion_is_atomic() {
    // 12 slots, all live: no room for the new prebuilt object.
    let live: Vec<(u16, Option<u16>)> = (0..12).map(|_| (232u16, None)).collect();
    let f = fixture(&live);
    let before = disk_bytes(&f.map);
    let next = doc(vec![spawn(7, "HOUSE", 9000.0, 9000.0, 0, 0, true)], vec![]);
    assert!(plan_spawn_data(&f.map, &f.catalog, &ScenarioDocument::default(), &next, &[placed(7, "HOUSE", 9000.0, 9000.0, 0, 0.0)]).is_err());
    assert_eq!(disk_bytes(&f.map), before);
}

#[test]
fn events_do_not_block_the_data_transaction_and_survive_into_the_plan() {
    use against_rome_scenario::model::{Action, Event};
    let f = fixture(&[]);
    let mut next = doc(vec![spawn(1, "HOUSE", 9000.0, 9000.0, 0, 0, true)], vec![]);
    next.events.push(Event { name: "msg".into(), enabled: true, actions: vec![Action { kind: 0, text: "hi".into(), ..Default::default() }], ..Default::default() });
    let plan = plan_spawn_data(&f.map, &f.catalog, &ScenarioDocument::default(), &next, &[placed(1, "HOUSE", 9000.0, 9000.0, 0, 0.0)])
        .unwrap()
        .unwrap();
    assert_eq!(plan.scenario.events, next.events);
}
