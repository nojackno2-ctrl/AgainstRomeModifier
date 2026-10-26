//! Spawn placement session (port of C# `PlacementEditSession`) and the spawn placement panel logic.

use against_rome_ambience::Language;
use against_rome_editor_app::scenario_ui::ScenarioUi;
use against_rome_editor_app::spawn_placement::*;
use against_rome_editor_app::spawn_placement_ui::{SpawnContext, SpawnPlacementUi};
use against_rome_maps::sdl_objects::{NativeObjectIdentity, SdlObjectCategory, SdlObjectType, SdlPlacedObject};
use against_rome_scenario::model::{Condition, Event, Spawn};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

fn object_type(name: &str, category: SdlObjectCategory, alias: &str) -> SdlObjectType {
    let mut fields = HashMap::new();
    fields.insert("alias".to_string(), alias.to_string());
    SdlObjectType::new(name, -1, category, "Rom", 0, fields)
}

fn building() -> SdlObjectType {
    object_type("BauRomHaus", SdlObjectCategory::Building, "house")
}
fn squad() -> SdlObjectType {
    object_type("FigRomLeg", SdlObjectCategory::Figure, "legion")
}
fn animal() -> SdlObjectType {
    object_type("FigTieWolf", SdlObjectCategory::Figure, "wolf")
}
fn catalog() -> Vec<SdlObjectType> {
    vec![building(), squad(), animal()]
}

fn placed(t: SdlObjectType, x: f32, z: f32, team: i32, angle: f32, count: i32) -> SdlPlacedObject {
    let mut item = SdlPlacedObject::new(t, x, 5.0, z, team, angle, count);
    item.scenario_id = Uuid::new_v4();
    item
}

fn session_with(items: &[SdlPlacedObject]) -> SpawnPlacementSession {
    let mut session = SpawnPlacementSession::new();
    session.load(items);
    session
}

// ---------------------------------------------------------------- session: basics

#[test]
fn add_undo_redo_and_dirty_baseline() {
    let mut s = SpawnPlacementSession::new();
    assert!(!s.is_dirty() && !s.can_undo());
    let mut item = placed(building(), 1000.0, 1000.0, 0, 0.0, 0);
    item.scenario_id = Uuid::nil();
    s.add(item).unwrap();
    assert_eq!(s.count(), 1);
    assert!(s.is_dirty() && s.can_undo() && !s.can_redo());
    assert!(!s.get(0).unwrap().scenario_id.is_nil(), "missing id gets a fresh persistent id");
    assert!(s.undo());
    assert_eq!(s.count(), 0);
    assert!(!s.is_dirty() && s.can_redo());
    assert!(s.redo());
    assert_eq!(s.count(), 1);
    s.accept_changes();
    assert!(!s.is_dirty() && !s.can_undo() && !s.can_redo());
    assert!(!s.undo() && !s.redo());
}

#[test]
fn new_command_clears_redo_and_reset_restores_baseline() {
    let mut s = session_with(&[placed(building(), 500.0, 500.0, 0, 0.0, 0)]);
    s.add(placed(building(), 600.0, 600.0, 0, 0.0, 0)).unwrap();
    assert!(s.undo());
    assert!(s.can_redo());
    s.add(placed(building(), 700.0, 700.0, 0, 0.0, 0)).unwrap();
    assert!(!s.can_redo());
    s.reset();
    assert_eq!(s.count(), 1);
    assert!(!s.is_dirty() && !s.can_undo());
}

#[test]
fn abort_last_reverts_without_redo_entry() {
    let mut s = session_with(&[]);
    s.add(placed(building(), 500.0, 500.0, 0, 0.0, 0)).unwrap();
    assert!(s.abort_last());
    assert_eq!(s.count(), 0);
    assert!(!s.can_redo() && !s.can_undo());
}

#[test]
fn add_validates_bounds_team_and_counts() {
    let mut s = SpawnPlacementSession::new();
    assert!(s.add(placed(building(), 16384.0, 10.0, 0, 0.0, 0)).is_err());
    assert!(s.add(placed(building(), -1.0, 10.0, 0, 0.0, 0)).is_err());
    assert!(s.add(placed(building(), f32::NAN, 10.0, 0, 0.0, 0)).is_err());
    assert!(s.add(placed(building(), 10.0, 10.0, 16, 0.0, 0)).is_err());
    assert!(s.add(placed(building(), 10.0, 10.0, -1, 0.0, 0)).is_ok());
    assert!(s.add(placed(squad(), 10.0, 10.0, 8, 0.0, 5)).is_err(), "squad team 0..=7");
    assert!(s.add(placed(squad(), 10.0, 10.0, 7, 0.0, 21)).is_err(), "squad count 0..=20");
    assert!(s.add(placed(squad(), 10.0, 10.0, 7, 0.0, 0)).is_ok(), "add is non-strict about count 0");
    assert!(s.add(placed(animal(), 10.0, 10.0, 3, 0.0, 2)).is_err(), "animals must have count 0");
    assert!(s.add(placed(animal(), 10.0, 10.0, 3, 0.0, 0)).is_ok());
    assert_eq!(s.count(), 3);
}

#[test]
fn replace_keeps_id_when_missing_and_validates() {
    let mut s = session_with(&[placed(building(), 500.0, 500.0, 0, 0.0, 0)]);
    let id = s.get(0).unwrap().scenario_id;
    let mut item = placed(building(), 900.0, 900.0, 1, 0.0, 0);
    item.scenario_id = Uuid::nil();
    s.replace(0, item).unwrap();
    assert_eq!(s.get(0).unwrap().scenario_id, id);
    assert!(matches!(s.replace(3, placed(building(), 1.0, 1.0, 0, 0.0, 0)), Err(PlacementError::OutOfRange(_))));
    assert!(s.undo());
    assert_eq!(s.get(0).unwrap().world_x, 500.0);
}

#[test]
fn remove_at_and_out_of_range() {
    let mut s = session_with(&[placed(building(), 1.0, 1.0, 0, 0.0, 0), placed(building(), 2.0, 2.0, 0, 0.0, 0)]);
    let first = s.get(0).unwrap();
    s.remove_at(0).unwrap();
    assert_eq!(s.count(), 1);
    assert!(s.remove_at(5).is_err());
    assert!(s.undo());
    assert_eq!(s.get(0).unwrap(), first);
}

// ---------------------------------------------------------------- session: edit

#[test]
fn edit_values_preserves_identity_and_zeroes_count_for_non_figures() {
    let mut s = session_with(&[placed(building(), 500.0, 500.0, 0, 0.0, 1), placed(squad(), 700.0, 700.0, 0, 90.0, 5)]);
    let building_id = s.get(0).unwrap().scenario_id;
    s.edit_values(0, 3, 600.0, 7.0, 650.0, 45.0, 9).unwrap();
    let b = s.get(0).unwrap();
    assert_eq!((b.team, b.world_x, b.world_y, b.world_z, b.angle, b.unit_count), (3, 600.0, 7.0, 650.0, 45.0, 0));
    assert_eq!(b.scenario_id, building_id);
    s.edit_values(1, 2, 700.0, 7.0, 700.0, 90.0, 12).unwrap();
    assert_eq!(s.get(1).unwrap().unit_count, 12);
    // strict: a squad needs 1..=20
    assert!(s.edit_values(1, 2, 700.0, 7.0, 700.0, 90.0, 0).is_err());
    assert!(s.edit_values(1, 9, 700.0, 7.0, 700.0, 90.0, 5).is_err(), "squad team 0..=7");
    assert_eq!(s.undo_depth(), 2, "failed edits add no history");
}

#[test]
fn raw_rotation_objects_only_translate() {
    let mut item = placed(building(), 500.0, 500.0, 2, 90.0, 0);
    item.raw_rotation = Some(std::f32::consts::FRAC_PI_2);
    let mut s = session_with(&[item]);
    assert!(matches!(
        s.edit_values(0, 3, 500.0, 5.0, 500.0, 90.0, 0),
        Err(PlacementError::InvalidOperation(_))
    ));
    assert!(matches!(
        s.edit_values(0, 2, 500.0, 5.0, 500.0, 180.0, 0),
        Err(PlacementError::InvalidOperation(_))
    ));
    // same team and direction, only the position moves; raw rotation survives
    s.edit_values(0, 2, 800.0, 5.0, 900.0, 90.0, 0).unwrap();
    let moved = s.get(0).unwrap();
    assert_eq!((moved.world_x, moved.world_z), (800.0, 900.0));
    assert_eq!(moved.raw_rotation.unwrap().to_bits(), std::f32::consts::FRAC_PI_2.to_bits());
    // same direction after rounding/normalisation is still accepted (90 vs 450)
    s.edit_values(0, 2, 800.0, 5.0, 900.0, 450.0, 0).unwrap();
}

#[test]
fn edit_preserves_provenance_even_if_updated_value_drops_it() {
    let mut s = session_with(&[placed(building(), 500.0, 500.0, 0, 0.0, 0)]);
    s.duplicate(0, 256.0, 256.0).unwrap();
    let mut updated = s.get(1).unwrap();
    updated.copy_source_scenario_id = Uuid::nil();
    updated.scenario_id = Uuid::new_v4();
    updated.world_x += 10.0;
    s.edit(1, &updated).unwrap();
    let after = s.get(1).unwrap();
    assert_eq!(after.copy_source_scenario_id, s.get(0).unwrap().scenario_id);
    assert_ne!(after.scenario_id, updated.scenario_id);
}

#[test]
fn edit_many_is_atomic_and_one_undo_step() {
    let mut s = session_with(&[
        placed(squad(), 100.0, 100.0, 0, 0.0, 5),
        placed(building(), 200.0, 200.0, 0, 0.0, 0),
    ]);
    s.edit_many(&[0, 1], Some(4), Some(90.0)).unwrap();
    assert_eq!((s.get(0).unwrap().team, s.get(0).unwrap().angle), (4, 90.0));
    assert_eq!((s.get(1).unwrap().team, s.get(1).unwrap().angle), (4, 90.0));
    assert_eq!(s.undo_depth(), 1);
    // team 9 is illegal for the squad -> nothing changes for the building either
    assert!(s.edit_many(&[0, 1], Some(9), None).is_err());
    assert_eq!(s.get(1).unwrap().team, 4);
    assert_eq!(s.undo_depth(), 1);
    // no-op leaves no history
    s.edit_many(&[0, 1], None, None).unwrap();
    s.edit_many(&[0, 1], Some(4), Some(90.0)).unwrap();
    assert_eq!(s.undo_depth(), 1);
    assert!(s.undo());
    assert_eq!(s.get(0).unwrap().team, 0);
}

#[test]
fn edit_many_refuses_native_group_objects() {
    let mut native = placed(building(), 100.0, 100.0, 2, 0.0, 0);
    native.raw_rotation = Some(0.0);
    let mut s = session_with(&[placed(building(), 50.0, 50.0, 0, 0.0, 0), native]);
    assert!(matches!(s.edit_many(&[0, 1], Some(5), None), Err(PlacementError::InvalidOperation(_))));
    assert_eq!(s.get(0).unwrap().team, 0, "atomic");
    // unchanged team passes for the native object
    s.edit_many(&[1], Some(2), None).unwrap();
}

#[test]
fn copied_native_group_allows_team_and_direction_edits() {
    let mut copy = placed(building(), 100.0, 100.0, 2, 90.0, 0);
    copy.raw_rotation = Some((90f32 as f64 * (std::f64::consts::PI / 180.0)) as f32);
    copy.native_source_identity = Some(NativeObjectIdentity { slot: 3, uid: 9 });
    let mut s = session_with(&[copy]);
    s.edit_many(&[0], Some(5), Some(180.0)).unwrap();
    assert_eq!((s.get(0).unwrap().team, s.get(0).unwrap().angle), (5, 180.0));
    assert_eq!(s.get(0).unwrap().raw_rotation.unwrap().to_bits(), std::f32::consts::PI.to_bits());
    s.rotate_many(&[0], 45).unwrap();
    assert_eq!(s.get(0).unwrap().angle, 225.0);
    assert_eq!(s.get(0).unwrap().raw_rotation.unwrap().to_bits(), ((225f32 as f64 * (std::f64::consts::PI / 180.0)) as f32).to_bits());
}

#[test]
fn rotate_many_normalizes_and_is_relative() {
    let mut s = session_with(&[
        placed(building(), 100.0, 100.0, 0, 350.0, 0),
        placed(building(), 200.0, 200.0, 0, 10.0, 0),
    ]);
    s.rotate_many(&[0, 1], 45).unwrap();
    assert_eq!(s.get(0).unwrap().angle, 35.0);
    assert_eq!(s.get(1).unwrap().angle, 55.0);
    s.rotate_many(&[0, 1], -45).unwrap();
    assert_eq!(s.get(0).unwrap().angle, 350.0);
    assert_eq!(s.get(1).unwrap().angle, 10.0);
    assert_eq!(s.undo_depth(), 2);
    s.rotate_many(&[0], 360).unwrap();
    assert_eq!(s.undo_depth(), 2, "full turn changes nothing");
    s.rotate_many(&[0], -400).unwrap();
    assert_eq!(s.get(0).unwrap().angle, 310.0);
}

#[test]
fn rotate_many_rejects_raw_rotation_atomically() {
    let mut raw = placed(building(), 100.0, 100.0, 0, 0.0, 0);
    raw.raw_rotation = Some(0.0);
    let mut s = session_with(&[placed(building(), 50.0, 50.0, 0, 0.0, 0), raw]);
    assert!(matches!(s.rotate_many(&[0, 1], 45), Err(PlacementError::InvalidOperation(_))));
    assert_eq!(s.get(0).unwrap().angle, 0.0);
    assert!(!s.can_undo());
}

// ---------------------------------------------------------------- session: duplicate / remove / move

#[test]
fn duplicate_many_inserts_after_each_source_as_one_undo_step() {
    let mut s = session_with(&[
        placed(building(), 100.0, 100.0, 0, 0.0, 0),
        placed(squad(), 200.0, 200.0, 1, 90.0, 6),
        placed(building(), 300.0, 300.0, 0, 0.0, 0),
    ]);
    let ids: Vec<Uuid> = (0..3).map(|i| s.get(i).unwrap().scenario_id).collect();
    let copies = s.duplicate_many(&[2, 0], 256.0, 256.0).unwrap();
    assert_eq!(copies, vec![1, 4], "copy indices follow the shifted layout");
    assert_eq!(s.count(), 5);
    let c0 = s.get(1).unwrap();
    assert_eq!((c0.world_x, c0.world_z), (356.0, 356.0));
    assert_eq!(c0.copy_source_scenario_id, ids[0]);
    assert_ne!(c0.scenario_id, ids[0]);
    assert_eq!(s.get(2).unwrap().scenario_id, ids[1]);
    assert_eq!(s.get(4).unwrap().copy_source_scenario_id, ids[2]);
    assert_eq!(s.undo_depth(), 1);
    assert!(s.undo());
    assert_eq!(s.count(), 3);
    assert_eq!((0..3).map(|i| s.get(i).unwrap().scenario_id).collect::<Vec<_>>(), ids);
    assert!(s.redo());
    assert_eq!(s.count(), 5);
}

#[test]
fn duplicate_of_duplicate_keeps_original_provenance_and_native_identity() {
    let mut s = session_with(&[placed(building(), 100.0, 100.0, 0, 0.0, 0)]);
    let original = s.get(0).unwrap().scenario_id;
    s.duplicate(0, 256.0, 0.0).unwrap();
    s.duplicate(1, 256.0, 0.0).unwrap();
    assert_eq!(s.get(2).unwrap().copy_source_scenario_id, original);
    let mut native = placed(building(), 100.0, 100.0, 2, 0.0, 0);
    native.native_source_identity = Some(NativeObjectIdentity { slot: 4, uid: 77 });
    native.raw_rotation = Some(1.0);
    let mut n = session_with(&[native]);
    n.duplicate(0, 256.0, 0.0).unwrap();
    let copy = n.get(1).unwrap();
    assert!(copy.copy_source_scenario_id.is_nil());
    assert_eq!(copy.native_source_identity, Some(NativeObjectIdentity { slot: 4, uid: 77 }));
    assert_eq!(copy.raw_rotation, Some(1.0));
}

#[test]
fn duplicate_outside_the_map_rejects_the_whole_batch() {
    let mut s = session_with(&[
        placed(building(), 100.0, 100.0, 0, 0.0, 0),
        placed(building(), 16300.0, 100.0, 0, 0.0, 0),
    ]);
    assert!(matches!(s.duplicate_many(&[0, 1], 256.0, 0.0), Err(PlacementError::OutOfRange(_))));
    assert_eq!(s.count(), 2);
    assert!(!s.can_undo());
    assert_eq!(s.duplicate_many(&[], 256.0, 0.0).unwrap(), Vec::<usize>::new());
}

#[test]
fn remove_many_restores_order_and_identity_on_undo() {
    let items: Vec<_> = (0..5).map(|i| placed(building(), 100.0 * (i + 1) as f32, 100.0, 0, 0.0, 0)).collect();
    let mut s = session_with(&items);
    s.remove_many(&[3, 1, 1]).unwrap();
    assert_eq!(s.count(), 3);
    assert_eq!(s.get(0).unwrap(), items[0]);
    assert_eq!(s.get(1).unwrap(), items[2]);
    assert_eq!(s.get(2).unwrap(), items[4]);
    assert!(s.remove_many(&[0, 9]).is_err());
    assert_eq!(s.count(), 3);
    assert!(s.undo());
    assert_eq!(s.capture(), items);
}

#[test]
fn move_many_validates_whole_batch_and_updates_height() {
    let mut s = session_with(&[
        placed(building(), 100.0, 100.0, 0, 0.0, 0),
        placed(building(), 16300.0, 100.0, 0, 0.0, 0),
    ]);
    assert!(s.move_many(&[0, 1], 256.0, 0.0, None).is_err());
    assert_eq!(s.get(0).unwrap().world_x, 100.0);
    assert!(!s.can_undo());
    let height = |x: f32, z: f32| x / 100.0 + z / 1000.0;
    s.move_many(&[0], 256.0, 0.0, Some(&height)).unwrap();
    let moved = s.get(0).unwrap();
    assert_eq!(moved.world_x, 356.0);
    assert!((moved.world_y - (3.56 + 0.1)).abs() < 1e-4);
    assert!(s.move_many(&[0], f32::NAN, 0.0, None).is_err());
    s.move_many(&[0], 0.0, 0.0, None).unwrap();
    assert_eq!(s.undo_depth(), 1);
    // raw rotation and identity survive a move
    let mut raw = placed(building(), 100.0, 100.0, 2, 90.0, 0);
    raw.raw_rotation = Some(1.5);
    let mut r = session_with(&[raw.clone()]);
    r.move_many(&[0], 10.0, 10.0, None).unwrap();
    assert_eq!(r.get(0).unwrap().raw_rotation, Some(1.5));
    assert_eq!(r.get(0).unwrap().scenario_id, raw.scenario_id);
}

#[test]
fn snap_many_preserves_spacing_around_first_anchor() {
    let mut s = session_with(&[
        placed(building(), 300.0, 300.0, 0, 0.0, 0),
        placed(building(), 400.0, 450.0, 0, 0.0, 0),
    ]);
    s.snap_many_to_grid(&[0, 1], None).unwrap();
    let a = s.get(0).unwrap();
    let b = s.get(1).unwrap();
    assert_eq!((a.world_x, a.world_z), (384.0, 384.0), "tile centre of tile (1,1)");
    assert_eq!((b.world_x - a.world_x, b.world_z - a.world_z), (100.0, 150.0));
    assert_eq!(s.undo_depth(), 1);
}

#[test]
fn add_many_requires_distinct_ids_and_validates_strictly() {
    let mut s = session_with(&[placed(building(), 1.0, 1.0, 0, 0.0, 0)]);
    let existing = s.get(0).unwrap().scenario_id;
    let mut clash = placed(building(), 5.0, 5.0, 0, 0.0, 0);
    clash.scenario_id = existing;
    assert!(matches!(s.add_many(&[clash]), Err(PlacementError::InvalidArgument(_))));
    assert!(s.add_many(&[placed(squad(), 5.0, 5.0, 0, 0.0, 0)]).is_err(), "strict count");
    assert_eq!(s.count(), 1);
    let indices = s.add_many(&[placed(building(), 5.0, 5.0, 0, 0.0, 0), placed(building(), 6.0, 6.0, 0, 0.0, 0)]).unwrap();
    assert_eq!(indices, vec![1, 2]);
    assert_eq!(s.undo_depth(), 1);
    assert!(s.undo());
    assert_eq!(s.count(), 1);
}

#[test]
fn accept_saved_changes_clears_provenance_and_history() {
    let mut s = session_with(&[placed(building(), 100.0, 100.0, 0, 0.0, 0)]);
    s.duplicate(0, 256.0, 0.0).unwrap();
    assert!(!s.get(1).unwrap().copy_source_scenario_id.is_nil());
    s.accept_saved_changes();
    assert!(s.get(1).unwrap().copy_source_scenario_id.is_nil());
    assert!(!s.is_dirty() && !s.can_undo());
}

// ---------------------------------------------------------------- conversions

fn spawn(alias: &str, x: f32, z: f32, team: i32, count: i32, angle: i32) -> Spawn {
    Spawn { alias: alias.into(), x, z, team, count, angle, y: 3.0, id: Uuid::new_v4(), ..Default::default() }
}

#[test]
fn untouched_spawns_round_trip_exactly_and_unknown_aliases_are_preserved() {
    let spawns = vec![
        spawn("house", 1000.0, 1000.0, 0, 0, 725), // unnormalised angle must survive untouched
        spawn("legion", 2000.0, 2000.0, 1, 0, 0),  // count-0 figure (single object)
        spawn("mystery", 3000.0, 3000.0, 2, 0, 0), // not in the catalog
        spawn("wolf", 4000.0, 4000.0, 3, 0, 90),
    ];
    let cat = catalog();
    let (placed_items, foreign) = load_spawns(&spawns, &cat);
    assert_eq!(placed_items.len(), 3);
    assert_eq!(foreign.len(), 1);
    let rebuilt = session_to_spawns(&placed_items, &spawns, &foreign, &cat);
    let expected: Vec<Spawn> = vec![spawns[0].clone(), spawns[1].clone(), spawns[3].clone(), spawns[2].clone()];
    assert_eq!(rebuilt, expected);
}

#[test]
fn new_objects_become_script_spawns_with_csharp_count_rules() {
    let cat = catalog();
    let b = placed_to_spawn(&placed(building(), 10.0, 20.0, 8, 370.0, 1), None);
    assert_eq!((b.alias.as_str(), b.team, b.count, b.angle, b.prebuilt), ("house", 8, 0, 10, false));
    let q = placed_to_spawn(&placed(squad(), 10.0, 20.0, 2, 90.0, 0), None);
    assert_eq!(q.count, 1, "a squad always keeps at least one soldier");
    let w = placed_to_spawn(&placed(animal(), 10.0, 20.0, 2, 90.0, 0), None);
    assert_eq!(w.count, 0);
    let mut raw = placed(building(), 10.0, 20.0, 2, 0.0, 0);
    raw.raw_rotation = Some(std::f32::consts::PI);
    let r = placed_to_spawn(&raw, None);
    assert_eq!((r.angle, r.raw_rotation), (180, Some(std::f32::consts::PI)));
    // spawn -> placed uses the raw rotation for the displayed angle
    let mut back = spawn("house", 1.0, 1.0, 2, 0, 0);
    back.raw_rotation = Some(std::f32::consts::FRAC_PI_2);
    back.prebuilt = true;
    let item = spawn_to_placed(&back, &cat).unwrap();
    assert!((item.angle - 90.0).abs() < 1e-3);
    assert_eq!(item.unit_count, 0);
}

#[test]
fn catalog_helpers_search_names_and_limits() {
    let b = building();
    assert!(matches_search(&b, "rom haus"));
    assert!(matches_search(&b, "HOUSE"));
    assert!(matches_search(&b, "建築 羅馬"));
    assert!(!matches_search(&b, "legion"));
    assert_eq!(team_range(&squad()), (0, 7));
    assert_eq!(team_range(&animal()), (-1, 15));
    assert_eq!(team_range(&b), (-1, 15));
    assert!(takes_unit_count(&squad()) && !takes_unit_count(&animal()) && !takes_unit_count(&b));
    assert_eq!(object_display_name(&b, true), "Romans BauRomHaus");
    assert!(new_placement(&squad(), 1.0, 1.0, 1.0, 9, 0.0, 5).is_err());
    assert_eq!(new_placement(&building(), 1.0, 1.0, 1.0, 9, 0.0, 5).unwrap().unit_count, 0);
    assert_eq!(normalize_angle(-45), 315);
    assert_eq!(radians_to_display_angle(std::f32::consts::PI), 180);
}

#[test]
fn ground_height_follows_csharp_vertex_lookup() {
    let size = 65usize;
    let mut heights = vec![0u8; size * size];
    heights[(1 * size) + 1] = 10; // tile (0,0): step 1, centre vertex (0,0)+0 => index 0 ... see below
    heights[0] = 7;
    assert_eq!(tile_ground_height(&heights, size, 0, 0, 4.0), 28.0);
    assert_eq!(tile_ground_height(&heights, size, 63, 63, 4.0), 0.0);
    assert_eq!(tile_ground_height(&[], size, 0, 0, 4.0), 0.0);
    assert_eq!(tile_center(2, 3), (640.0, 896.0));
    // world lookup rounds to the nearest vertex: 1 vertex per 256 units here
    assert_eq!(world_ground_height(&heights, size, 256.0, 256.0, 2.0), 20.0);
    assert_eq!(world_ground_height(&heights, size, 100.0, 100.0, 2.0), 14.0);
}

// ---------------------------------------------------------------- panel logic over ScenarioUi

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("arm_spawn_ui_{tag}_{}", Uuid::new_v4().simple()));
    std::fs::create_dir_all(dir.join("SCRIPT")).unwrap();
    std::fs::write(dir.join("SCRIPT/ak_level.bci"), against_rome_scenario::BciImage::idle().serialize()).unwrap();
    dir
}

/// Fake game root with an alias catalog: `<root>/MAPS/CUSTOM` (map) + `SYSTEM/CLAK/cl_scint.ini`.
fn scratch_game(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("arm_spawn_game_{tag}_{}", Uuid::new_v4().simple()));
    let map = root.join("MAPS").join("CUSTOM");
    std::fs::create_dir_all(map.join("SCRIPT")).unwrap();
    std::fs::write(map.join("SCRIPT/ak_level.bci"), against_rome_scenario::BciImage::idle().serialize()).unwrap();
    std::fs::create_dir_all(root.join("SYSTEM/CLAK")).unwrap();
    std::fs::write(
        root.join("SYSTEM/CLAK/cl_scint.ini"),
        "[ObjDefName]
house=BauRomHaus
legion=FigRomLeg
wolf=FigTieWolf
",
    )
    .unwrap();
    map
}

struct Fixture {
    map: PathBuf,
    scenario: ScenarioUi,
    ui: SpawnPlacementUi,
    heights: Vec<u8>,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        Self::at(scratch(tag))
    }
    fn at(map: PathBuf) -> Self {
        let mut scenario = ScenarioUi::default();
        scenario.load_session(&map).unwrap();
        let mut ui = SpawnPlacementUi::default();
        ui.set_catalog(catalog());
        let mut f = Self { map, scenario, ui, heights: vec![10u8; 65 * 65] };
        f.sync();
        f
    }
    fn sync(&mut self) {
        let map = self.map.clone();
        self.ui.sync_with_document(&self.scenario, Some(&map));
    }
    fn place(&mut self, name: &str, tile: (i32, i32)) {
        self.ui.select_type(name);
        let cx = SpawnContext {
            lang: Language::English,
            heights: Some(&self.heights),
            vertex_size: 65,
            height_step: 4.0,
            read_only: false,
        };
        self.ui.place_at_tile(&mut self.scenario, &cx, tile.0, tile.1);
    }
}

#[test]
fn clicking_a_tile_places_a_script_spawn_with_terrain_height() {
    let mut f = Fixture::new("place");
    f.ui.set_placement_values(2, 8, 90);
    f.place("FigRomLeg", (3, 4));
    let spawns = &f.scenario.document().spawns;
    assert_eq!(spawns.len(), 1);
    let s = &spawns[0];
    assert_eq!((s.alias.as_str(), s.x, s.z, s.team, s.count, s.angle, s.y), ("legion", 896.0, 1152.0, 2, 8, 90, 40.0));
    assert!(!s.prebuilt && !s.id.is_nil());
    assert!(f.scenario.dirty(), "scenario document is the source of truth and became dirty");
    assert_eq!(f.ui.selection(), vec![0]);
    assert!(f.ui.status().0.contains("tile (3,4)") && !f.ui.status().1);

    // undo / redo flow through the document; undoing everything returns to a clean document
    let cx = SpawnContext { lang: Language::English, heights: None, vertex_size: 0, height_step: 4.0, read_only: false };
    assert!(f.ui.undo(&mut f.scenario, &cx));
    assert!(f.scenario.document().spawns.is_empty());
    assert!(!f.scenario.dirty());
    assert!(f.ui.redo(&mut f.scenario, &cx));
    assert_eq!(f.scenario.document().spawns.len(), 1);
}

#[test]
fn invalid_team_and_read_only_and_missing_type_refuse_without_changes() {
    let mut f = Fixture::new("refuse");
    f.ui.set_placement_values(9, 5, 0);
    f.place("FigRomLeg", (1, 1));
    assert!(f.scenario.document().spawns.is_empty());
    assert!(f.ui.status().1 && f.ui.status().0.contains("team between 0 and 7"));

    let heights = f.heights.clone();
    let cx = SpawnContext { lang: Language::English, heights: Some(&heights), vertex_size: 65, height_step: 4.0, read_only: true };
    let mut ui = std::mem::take(&mut f.ui);
    ui.select_type("BauRomHaus");
    ui.set_placement_values(0, 1, 0);
    ui.place_at_tile(&mut f.scenario, &cx, 1, 1);
    assert!(f.scenario.document().spawns.is_empty());
    assert!(ui.status().0.contains("read-only"));
    f.ui = ui;

    let mut empty = Fixture::new("notype");
    let mut ui = SpawnPlacementUi::default();
    ui.set_catalog(catalog());
    let cx = SpawnContext { lang: Language::English, heights: None, vertex_size: 0, height_step: 4.0, read_only: false };
    ui.place_at_tile(&mut empty.scenario, &cx, 1, 1);
    assert!(ui.status().0.contains("Pick an object type"));
}

#[test]
fn unloaded_scenario_refuses_placement() {
    let mut scenario = ScenarioUi::default();
    let mut ui = SpawnPlacementUi::default();
    ui.set_catalog(catalog());
    ui.select_type("BauRomHaus");
    let cx = SpawnContext { lang: Language::English, heights: None, vertex_size: 0, height_step: 4.0, read_only: false };
    ui.place_at_tile(&mut scenario, &cx, 1, 1);
    assert!(ui.status().1 && ui.status().0.contains("Load the scenario first"));
    assert!(scenario.document().spawns.is_empty());
}

#[test]
fn selection_workflow_duplicate_rotate_move_batch_delete() {
    let mut f = Fixture::new("workflow");
    f.place("BauRomHaus", (2, 2));
    f.place("FigRomLeg", (5, 5));
    f.ui.select_all();
    assert_eq!(f.ui.selection(), vec![0, 1]);
    let cx = SpawnContext {
        lang: Language::English,
        heights: Some(&f.heights),
        vertex_size: 65,
        height_step: 4.0,
        read_only: false,
    };
    // rotate +45 then batch team (squad forces 0..=7)
    f.ui.rotate_selected(&mut f.scenario, &cx, 45);
    assert!(f.scenario.document().spawns.iter().all(|s| s.angle == 45));
    f.ui.apply_batch(&mut f.scenario, &cx, Some(3), None);
    assert!(f.scenario.document().spawns.iter().all(|s| s.team == 3));
    f.ui.apply_batch(&mut f.scenario, &cx, Some(9), None);
    assert!(f.ui.status().1, "team 9 illegal for the squad");
    assert!(f.scenario.document().spawns.iter().all(|s| s.team == 3), "rejected batch changes nothing");

    // nudge one tile right; out-of-map move is rejected atomically
    f.ui.move_selected(&mut f.scenario, &cx, 256.0, 0.0, false);
    assert_eq!(f.scenario.document().spawns[0].x, 640.0 + 256.0);
    f.ui.move_selected(&mut f.scenario, &cx, 20000.0, 0.0, false);
    assert!(f.ui.status().1);
    assert_eq!(f.scenario.document().spawns[0].x, 896.0);

    // duplicate both (+256,+256), selection moves to the copies
    f.ui.duplicate_selected(&mut f.scenario, &cx);
    assert_eq!(f.scenario.document().spawns.len(), 4);
    assert_eq!(f.ui.selection().len(), 2);
    let ids: std::collections::HashSet<_> = f.scenario.document().spawns.iter().map(|s| s.id).collect();
    assert_eq!(ids.len(), 4, "copies have fresh persistent ids");

    // delete the copies, undo brings them back in place
    f.ui.delete_selected(&mut f.scenario, &cx);
    assert_eq!(f.scenario.document().spawns.len(), 2);
    assert!(f.ui.undo(&mut f.scenario, &cx));
    assert_eq!(f.scenario.document().spawns.len(), 4);
}

#[test]
fn document_validation_rolls_back_deleting_a_spawn_used_by_an_event() {
    let mut f = Fixture::new("event");
    f.place("BauRomHaus", (2, 2));
    let id = f.scenario.document().spawns[0].id;
    let event = Event {
        name: "Watch".into(),
        conditions: vec![Condition { kind: 0, target_id: id, ..Default::default() }],
        actions: vec![against_rome_scenario::model::Action { text: "hello".into(), ..Default::default() }],
        ..Default::default()
    };
    f.scenario.add_events(&[event]).unwrap();
    f.sync();
    f.ui.select_all();
    let cx = SpawnContext { lang: Language::English, heights: None, vertex_size: 0, height_step: 4.0, read_only: false };
    let depth = f.ui.session().undo_depth();
    f.ui.delete_selected(&mut f.scenario, &cx);
    assert!(f.ui.status().1, "{}", f.ui.status().0);
    assert_eq!(f.scenario.document().spawns.len(), 1, "document keeps the referenced spawn");
    assert_eq!(f.ui.session().count(), 1, "session command was rolled back");
    assert_eq!(f.ui.session().undo_depth(), depth);
    assert!(!f.ui.session().can_redo());
}

#[test]
fn prebuilt_data_spawns_are_editable_and_unknown_aliases_survive_edits() {
    let mut f = Fixture::new("bound");
    let mut prebuilt = spawn("house", 1000.0, 1000.0, 0, 0, 0);
    prebuilt.prebuilt = true;
    let unknown = spawn("mystery", 3000.0, 3000.0, 2, 0, 0);
    f.scenario.apply_spawns(vec![prebuilt.clone(), unknown.clone()]).unwrap();
    f.sync();
    assert_eq!(f.ui.session().count(), 1, "unknown alias is not a placed object");
    assert!(f.ui.is_data_bound_index(0), "prebuilt spawns are marked as DATA objects in the list");
    f.ui.select_all();
    let cx = SpawnContext { lang: Language::English, heights: None, vertex_size: 0, height_step: 4.0, read_only: false };
    // C# parity: prebuilt spawns are ordinary placements; Save decides how DATA follows.
    f.ui.move_selected(&mut f.scenario, &cx, 256.0, 0.0, false);
    assert_eq!(f.scenario.document().spawns[0].x, 1256.0);
    assert!(f.scenario.document().spawns[0].prebuilt, "an edit keeps the prebuilt flag");
    f.ui.rotate_selected(&mut f.scenario, &cx, 45);
    assert_eq!(f.scenario.document().spawns[0].angle, 45);
    f.ui.undo(&mut f.scenario, &cx);
    f.ui.undo(&mut f.scenario, &cx);
    assert_eq!(f.scenario.document().spawns, vec![prebuilt.clone(), unknown.clone()]);
    f.ui.select_all();
    f.ui.delete_selected(&mut f.scenario, &cx);
    assert_eq!(f.scenario.document().spawns, vec![unknown.clone()], "the foreign spawn survives a delete");
    f.ui.undo(&mut f.scenario, &cx);
    assert_eq!(f.scenario.document().spawns.len(), 2);

    // a normal placement keeps both original spawns and appends the new one before the foreign tail
    f.place("BauRomHaus", (9, 9));
    let spawns = &f.scenario.document().spawns;
    assert_eq!(spawns.len(), 3);
    assert_eq!(spawns[0], prebuilt);
    assert_eq!(spawns[2], unknown, "non-representable spawns are preserved, not dropped");
    assert!(!spawns[1].prebuilt);
}

#[test]
fn external_document_changes_reload_the_session_and_save_accepts_it() {
    let mut f = Fixture::at(scratch_game("save"));
    f.place("BauRomHaus", (2, 2));
    f.ui.duplicate_selected(
        &mut f.scenario,
        &SpawnContext { lang: Language::English, heights: None, vertex_size: 0, height_step: 4.0, read_only: false },
    );
    assert!(f.ui.session().is_dirty());
    let copy_id = f.ui.session().get(1).unwrap().scenario_id;
    assert!(!f.ui.session().get(1).unwrap().copy_source_scenario_id.is_nil());

    // Safe Save path: stage -> write -> commit baseline; the panel re-baselines afterwards.
    let staged = f.scenario.stage_transaction_files(&f.map).expect("spawns compile into the BCI");
    assert!(staged.iter().any(|(p, _)| p.ends_with("arm_scenario.json")));
    for (relative, bytes) in &staged {
        let target = f.map.join(relative);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, bytes).unwrap();
    }
    f.scenario.commit_baseline(&f.map).unwrap();
    assert!(!f.scenario.dirty());
    f.sync();
    assert!(!f.ui.session().is_dirty());
    assert!(!f.ui.session().can_undo());
    assert_eq!(f.scenario.document().spawns.len(), 2);
    assert!(f.ui.session().objects().iter().any(|o| o.scenario_id == copy_id));
    assert!(f.ui.session().objects().iter().all(|o| o.copy_source_scenario_id.is_nil()));

    // Reset of the scenario (map switch) empties the panel
    f.scenario.reset_session(None);
    f.ui.sync_with_document(&f.scenario, None);
    assert_eq!(f.ui.session().count(), 0);
    assert!(f.ui.selection().is_empty());
}

#[test]
fn catalog_loads_from_the_game_root_alias_file() {
    let map = scratch_game("catalog");
    let mut scenario = ScenarioUi::default();
    scenario.load_session(&map).unwrap();
    assert_eq!(scenario.known_aliases().len(), 3);
    let mut ui = SpawnPlacementUi::default();
    ui.sync_with_document(&scenario, Some(&map));
    let names: Vec<&str> = ui.catalog().iter().map(|t| t.name_def.as_str()).collect();
    assert_eq!(names, vec!["BauRomHaus", "FigTieWolf", "FigRomLeg"], "sorted by category, tribe (common first) then name");
    assert_eq!(ui.catalog_type_for("LEGION").unwrap().name_def, "FigRomLeg");
    // a spawn whose alias is in the catalog loads as a placed object
    let spawn = Spawn { alias: "wolf".into(), x: 500.0, z: 500.0, team: 1, id: Uuid::new_v4(), ..Default::default() };
    scenario.apply_spawns(vec![spawn]).unwrap();
    ui.sync_with_document(&scenario, Some(&map));
    assert_eq!(ui.session().count(), 1);
    // unknown alias is refused by the document (catalog-checked)
    let bad = Spawn { alias: "nonsense".into(), x: 1.0, z: 1.0, id: Uuid::new_v4(), ..Default::default() };
    assert!(scenario.apply_spawns(vec![bad]).is_err());
}

#[test]
fn panel_and_overlay_render_headless_and_shortcuts_act_on_the_active_panel() {
    use against_rome_editor_app::app::ShortcutAction;
    let mut f = Fixture::new("render");
    f.place("BauRomHaus", (2, 2));
    f.ui.open = true;
    f.ui.set_active(true);
    let ctx = eframe::egui::Context::default();
    let map = f.map.clone();
    let heights = f.heights.clone();
    for _ in 0..2 {
        let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
            let cx = SpawnContext { lang: Language::TraditionalChinese, heights: Some(&heights), vertex_size: 65, height_step: 4.0, read_only: false };
            f.ui.show(ctx, &mut f.scenario, &cx, Some(&map));
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                let painter = ui.painter().clone();
                let rect = eframe::egui::Rect::from_min_size(eframe::egui::pos2(0.0, 0.0), eframe::egui::vec2(512.0, 512.0));
                f.ui.draw_overlay(&painter, rect);
                f.ui.draw_ghost(&painter, eframe::egui::pos2(10.0, 10.0));
            });
        });
    }
    assert!(f.ui.is_active());
    let cx = SpawnContext { lang: Language::English, heights: None, vertex_size: 0, height_step: 4.0, read_only: false };
    f.ui.handle_shortcut(ShortcutAction::SelectAll, &mut f.scenario, &cx);
    assert_eq!(f.ui.selection(), vec![0]);
    f.ui.handle_shortcut(ShortcutAction::Duplicate, &mut f.scenario, &cx);
    assert_eq!(f.scenario.document().spawns.len(), 2);
    f.ui.handle_shortcut(ShortcutAction::Delete, &mut f.scenario, &cx);
    assert_eq!(f.scenario.document().spawns.len(), 1);
    f.ui.rotate_next_placement(-45);
    assert_eq!(f.ui.placement_angle(), 315);
}
