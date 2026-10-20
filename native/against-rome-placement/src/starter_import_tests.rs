// Synthetic contracts deliberately reuse fixture builders, never modify the existing oracle.
#[test]
fn starter_import_cross_capacity_preserves_aliases_and_inputs() {
    let mut source = fixture_count(4);
    let mut light = vec![0; against_rome_data::pool::PAYLOAD_LEN];
    light[..4].copy_from_slice(&1u32.to_le_bytes());
    light[4..8].copy_from_slice(&1024u32.to_le_bytes());
    source
        .pools
        .insert("light".into(), Blob::decode(light.clone()).unwrap());
    let parent = source.column_offset(1, 13);
    source.pools.get_mut("objects").unwrap().bytes[parent..parent + 2]
        .copy_from_slice(&0u16.to_le_bytes());
    let third = source.column_offset(0, 10);
    source.pools.get_mut("objects").unwrap().bytes[third..third + 2]
        .copy_from_slice(&0u16.to_le_bytes());
    let mut target = fixture_count(9);
    for slot in 0..9 {
        target.remove_family(slot, slot as u32 + 1).unwrap();
    }
    target
        .pools
        .insert("light".into(), Blob::decode(light).unwrap());
    // Occupy a target slot and retain unrelated native bytes.
    let (target, _, _) = target
        .import_family_candidate(&source, 2, 3, [0; 3], 1)
        .unwrap();
    let before = target.snapshot();
    let original = source.snapshot();
    let (candidate, root, uid) = target
        .import_family_candidate(&source, 0, 1, [65536, 32768, 131072], 7)
        .unwrap();
    assert_eq!(before, target.snapshot());
    assert_eq!(original, source.snapshot());
    let family = candidate.family(root, uid).unwrap();
    assert_eq!(family.len(), 2);
    assert_eq!(candidate.link(root, 67), candidate.link(root, 69));
    assert_eq!(candidate.link(root, 67), candidate.col(root, 10));
    assert!(family.iter().all(|s| candidate
        .objects()
        .iter()
        .find(|o| o.slot == *s)
        .unwrap()
        .team
        == 7));
    let pos = candidate
        .objects()
        .into_iter()
        .find(|o| o.slot == root)
        .unwrap()
        .position;
    assert_eq!(pos, [1., 0.5, 2.]);
}
#[test]
fn starter_import_rich_family_matches_existing_clone_payloads() {
    let source = rich_fixture_n(16);
    let before = source.snapshot();
    let mut cloned = source.clone();
    let expected = cloned
        .clone_family_fixed(0, 1, [65536, 32768, 65536])
        .unwrap();
    let team = source
        .objects()
        .into_iter()
        .find(|o| o.slot == 0)
        .unwrap()
        .team;
    let (imported, slot, uid) = source
        .import_family_candidate(&source, 0, 1, [65536, 32768, 65536], team)
        .unwrap();
    assert_eq!(expected, (slot, uid));
    assert_eq!(cloned.snapshot(), imported.snapshot());
    assert_eq!(before, source.snapshot());
}
#[test]
fn starter_import_exhaustion_and_missing_pool_are_atomic() {
    let source = rich_fixture_n(16);
    let target = fixture_count(2);
    let before = target.snapshot();
    assert!(target
        .import_family_candidate(&source, 0, 1, [0; 3], 0)
        .is_err());
    assert_eq!(before, target.snapshot());
    let mut target = rich_fixture_n(16);
    target.pools.remove("light");
    let before = target.snapshot();
    assert!(target
        .import_family_candidate(&source, 0, 1, [0; 3], 0)
        .is_err());
    assert_eq!(before, target.snapshot());
    assert!(fixed_displacement([0.; 3], [f32::NAN, 0., 0.]).is_err());
    assert!(fixed_displacement([0.; 3], [0.0000001, 0., 0.]).is_err());
    assert!(fixed_displacement([0.; 3], [32768., 0., 0.]).is_err());
}
