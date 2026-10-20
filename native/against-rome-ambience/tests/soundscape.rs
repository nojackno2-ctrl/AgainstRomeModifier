use against_rome_ambience::soundscape::*;

fn zone(x: f32, z: f32) -> SoundscapeZone {
    SoundscapeZone { center_world_x: x, center_world_z: z, name: "z".into(), ..Default::default() }
}

#[test]
fn binary_round_trip() {
    let snap = SoundscapeSnapshot { zones: vec![zone(1.0, 2.0), zone(30.0, 40.0)], ..Default::default() };
    let bytes = SoundscapeBinaryStorage::serialize_to_binary(&snap).unwrap();
    let back = SoundscapeBinaryStorage::deserialize_from_binary(&bytes).unwrap();
    assert_eq!(back.zones.len(), 2);
    assert_eq!(back.zones[1].center_world_x, 30.0);
    assert_eq!(back.global_ambience_id, snap.global_ambience_id);
}

#[test]
fn binary_rejects_garbage_and_truncation() {
    assert!(SoundscapeBinaryStorage::deserialize_from_binary(b"xx").is_err());
    let snap = SoundscapeSnapshot { zones: vec![zone(1.0, 2.0)], ..Default::default() };
    let bytes = SoundscapeBinaryStorage::serialize_to_binary(&snap).unwrap();
    assert!(SoundscapeBinaryStorage::deserialize_from_binary(&bytes[..bytes.len() - 3]).is_err());
}

#[test]
fn json_round_trip_and_native_export_unsupported() {
    let snap = SoundscapeSnapshot { zones: vec![zone(5.0, 6.0)], ..Default::default() };
    let j = SoundscapeBinaryStorage::export_to_json(&snap).unwrap();
    let back = SoundscapeBinaryStorage::import_from_json(&j).unwrap();
    assert_eq!(back.zones[0].id, snap.zones[0].id);
    assert!(SoundscapeBinaryStorage::export_to_script(&snap).is_err());
    assert!(SoundscapeBinaryStorage::save_to_sound_dat("x.dat", &snap, false, None).is_err());
}

#[test]
fn convex_hull_drops_interior_points() {
    let pts: Vec<_> = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (5.0, 5.0)]
        .iter()
        .map(|&(x, z)| SoundscapeVertex { x, z })
        .collect();
    assert_eq!(SoundscapeZonePlanner::compute_convex_hull(&pts).len(), 4);
}

#[test]
fn editor_undo_redo_and_dirty() {
    let mut m = SoundscapeEditorModule::default();
    assert!(!m.is_dirty());
    let z = zone(1.0, 1.0);
    m.add_zone(&z);
    assert!(m.is_dirty() && m.can_undo());
    assert_eq!(m.zones().len(), 1);
    assert!(m.undo());
    assert_eq!(m.zones().len(), 0);
    assert!(m.redo());
    assert!(m.remove_zone(z.id));
    m.accept_changes();
    assert!(!m.is_dirty());
}

#[test]
fn default_catalog_resolves_known_and_unknown() {
    let c = SoundscapeZoneCatalog::create_default_catalog();
    assert!(!c.get_all_definitions().is_empty());
    let first = c.get_all_definitions()[0].sound_id.clone();
    assert_eq!(c.resolve(&first).sound_id, first);
    let _ = c.resolve("no_such_sound");
}

#[test]
fn editor_height_only_edit_is_dirty() {
    let mut m = SoundscapeEditorModule::default();
    let mut z = zone(5.0, 6.0);
    m.add_zone(&z);
    m.accept_changes();
    assert!(!m.is_dirty());
    z.center_world_y = 42.0;
    assert!(m.update_zone(&z));
    assert!(m.is_dirty(), "changing only CenterWorldY must mark the session dirty");
    assert!(m.undo());
    assert!(!m.is_dirty());
}
