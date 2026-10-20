//! Tests for the pure models of `family_ui` (saved lights / particles / definitions / family copy).
use against_rome_data::particle_definition_sources::EffectiveDefinitions;
use against_rome_editor_app::family_ui::*;
use against_rome_placement::{
    ObjectStore, BIG_LAGER_CAPACITY, BIG_LAGER_MEMBERS, BIG_LAGER_PAYLOAD_LEN, BIG_LAGER_RECORD_SIZE, COLUMNS,
    LAGER_CAPACITY, LAGER_PAYLOAD_LEN, LAGER_RECORD_SIZE, PARTICLE_CAPACITY, PARTICLE_CHILDREN,
    PARTICLE_RECORD_SIZE, SEGMENTS,
};
use std::{collections::BTreeSet, fs, path::{Path, PathBuf}};

fn create_synthetic_fixture_map(dir: &Path) {
    let n = 10;
    fs::create_dir_all(dir.join("DATA")).unwrap();

    // 1. objects.dat
    let mut o = vec![0; 16 + n * (79 + COLUMNS.iter().sum::<usize>())];
    o[0..4].copy_from_slice(&1u32.to_le_bytes());
    o[4..8].copy_from_slice(&(n as u32).to_le_bytes());
    o[8..12].copy_from_slice(&30u32.to_le_bytes());
    o[12..16].copy_from_slice(&30u32.to_le_bytes());
    for s in 0..4 {
        let r = 16 + s * 79;
        o[r] = 1;
        o[r + 3..r + 7].copy_from_slice(&(s as u32 + 1).to_le_bytes());
        for offset in [67, 69] {
            o[r + offset..r + offset + 2].copy_from_slice(&(s as u16).to_le_bytes());
        }
        for offset in [71, 73, 77] {
            o[r + offset..r + offset + 2].copy_from_slice(&(s as u16).to_le_bytes());
        }
        for c in 0..18 {
            let p = 16 + n * 79 + n * COLUMNS[..c].iter().sum::<usize>() + s * COLUMNS[c];
            if COLUMNS[c] == 2 {
                o[p..p + 2].fill(255);
            }
            if c == 1 {
                o[p..p + 2].copy_from_slice(&(s as u16).to_le_bytes());
            }
        }
    }
    // Child link: slot 1 parent is slot 0 (column 13)
    let p_parent = 16 + n * 79 + n * COLUMNS[..13].iter().sum::<usize>() + 1 * COLUMNS[13];
    o[p_parent..p_parent + 2].copy_from_slice(&0u16.to_le_bytes());
    // Direct light link on slot 0 (column 12)
    let p_light = 16 + n * 79 + n * COLUMNS[..12].iter().sum::<usize>() + 0 * COLUMNS[12];
    o[p_light..p_light + 2].copy_from_slice(&0u16.to_le_bytes());
    // Particle emitter link on slot 0 (column 11)
    let p_part = 16 + n * 79 + n * COLUMNS[..11].iter().sum::<usize>() + 0 * COLUMNS[11];
    o[p_part..p_part + 2].copy_from_slice(&0u16.to_le_bytes());
    // Lager camp links (column 5): slot 0 -> camp 0, slot 1 -> camp 1
    let p_lager0 = 16 + n * 79 + n * COLUMNS[..5].iter().sum::<usize>() + 0 * COLUMNS[5];
    o[p_lager0..p_lager0 + 2].copy_from_slice(&0u16.to_le_bytes());
    let p_lager1 = 16 + n * 79 + n * COLUMNS[..5].iter().sum::<usize>() + 1 * COLUMNS[5];
    o[p_lager1..p_lager1 + 2].copy_from_slice(&1u16.to_le_bytes());
    fs::write(dir.join("DATA").join("objects.dat"), &o).unwrap();

    // 2. position.dat
    let mut p = vec![0; 8 + n * 17];
    p[0..4].copy_from_slice(&1u32.to_le_bytes());
    p[4..8].copy_from_slice(&(n as u32).to_le_bytes());
    for s in 0..4 {
        p[8 + s * 17] = 1;
        p[9 + s * 17..13 + s * 17].copy_from_slice(&(s as f32 * 20.).to_le_bytes());
        p[21 + s * 17..25 + s * 17].copy_from_slice(&90f32.to_le_bytes());
    }
    fs::write(dir.join("DATA").join("position.dat"), &p).unwrap();

    // 3. objdata.dat
    let mut d = vec![0; 8 + n * SEGMENTS.iter().sum::<usize>()];
    d[0..4].copy_from_slice(&1u32.to_le_bytes());
    d[4..8].copy_from_slice(&(n as u32).to_le_bytes());
    for s in 0..4 {
        d[8 + s * 9] = 1;
    }
    fs::write(dir.join("DATA").join("objdata.dat"), &d).unwrap();

    // 4. light.dat
    let mut lb = vec![0; against_rome_data::pool::PAYLOAD_LEN];
    lb[0..4].copy_from_slice(&1u32.to_le_bytes());
    lb[4..8].copy_from_slice(&1024u32.to_le_bytes());
    lb[8..12].copy_from_slice(&1u32.to_le_bytes());
    let widths = against_rome_data::pool::WIDTHS;
    let mut base = 8;
    for (c, w) in widths.iter().enumerate() {
        if c >= 13 {
            lb[base..base + 1024 * w].fill(255);
        }
        base += 1024 * w;
    }
    // Light 0 is direct light owned by object 0
    let owner_offset = 8 + 1024 * widths[..13].iter().sum::<usize>();
    lb[owner_offset..owner_offset + 2].copy_from_slice(&0u16.to_le_bytes());
    // Light 1 is particle child light
    lb[8 + 1 * 4..8 + 1 * 4 + 4].copy_from_slice(&1u32.to_le_bytes());
    for a in 0..3 {
        let off = 8 + 1024 * widths[..10 + a].iter().sum::<usize>() + 1 * 4;
        lb[off..off + 4].copy_from_slice(&((a + 1) as f32).to_le_bytes());
    }
    fs::write(dir.join("DATA").join("light.dat"), &lb).unwrap();

    // 5. particle.dat
    let mut part = vec![0; against_rome_data::particle::PAYLOAD_LEN];
    part[0..4].copy_from_slice(&1u32.to_le_bytes());
    part[4..8].copy_from_slice(&(PARTICLE_CAPACITY as u32).to_le_bytes());
    part[8..12].copy_from_slice(&(PARTICLE_CHILDREN as u32).to_le_bytes());
    for slot in 0..PARTICLE_CAPACITY {
        let rec = 12 + slot * PARTICLE_RECORD_SIZE;
        part[rec + 6..rec + 8].copy_from_slice(&(-1i16).to_le_bytes());
        for i in 0..PARTICLE_CHILDREN {
            part[rec + 12 + i * 35 + 33..rec + 12 + i * 35 + 35].copy_from_slice(&(-1i16).to_le_bytes());
        }
        for col in 0..4 {
            let off = 12 + PARTICLE_CAPACITY * PARTICLE_RECORD_SIZE + col * PARTICLE_CAPACITY * 4 + slot * 4;
            let val = if col == 3 { 30i32 } else { 0i32 };
            part[off..off + 4].copy_from_slice(&val.to_le_bytes());
        }
    }
    // Emitter 0 owned by object 0, child with light link to light 1
    let rec = 12 + 0 * PARTICLE_RECORD_SIZE;
    part[rec..rec + 2].copy_from_slice(&1u16.to_le_bytes());
    part[rec + 2..rec + 6].copy_from_slice(&1u32.to_le_bytes());
    part[rec + 6..rec + 8].copy_from_slice(&0u16.to_le_bytes());
    part[rec + 12] = 1;
    part[rec + 13..rec + 17].copy_from_slice(&65536i32.to_le_bytes());
    part[rec + 17..rec + 21].copy_from_slice(&131072i32.to_le_bytes());
    part[rec + 21..rec + 25].copy_from_slice(&196608i32.to_le_bytes());
    part[rec + 45..rec + 47].copy_from_slice(&1i16.to_le_bytes());
    fs::write(dir.join("DATA").join("particle.dat"), &part).unwrap();

    // 6. lager.dat
    let mut l = vec![0; LAGER_PAYLOAD_LEN];
    l[0..4].copy_from_slice(&1u32.to_le_bytes());
    l[4..8].copy_from_slice(&(LAGER_CAPACITY as u32).to_le_bytes());
    l[8..12].copy_from_slice(&6u32.to_le_bytes());
    l[12..16].copy_from_slice(&10u32.to_le_bytes());
    let big_base = 16 + LAGER_CAPACITY * LAGER_RECORD_SIZE;
    for i in 0..LAGER_CAPACITY {
        l[big_base + i * 2..big_base + i * 2 + 2].copy_from_slice(&(-1i16).to_le_bytes());
        let rec = 16 + i * LAGER_RECORD_SIZE;
        for m in 0..10 {
            l[rec + 23 + m * 2..rec + 23 + m * 2 + 2].copy_from_slice(&(-1i16).to_le_bytes());
        }
    }
    // Camp 0: owner object 0, small member [2]
    let rec0 = 16 + 0 * LAGER_RECORD_SIZE;
    l[rec0] = 1;
    l[rec0 + 9..rec0 + 11].copy_from_slice(&1i16.to_le_bytes());
    l[rec0 + 23..rec0 + 25].copy_from_slice(&2i16.to_le_bytes());
    // Camp 1: owner object 1, small member [3], big lager link 0
    let rec1 = 16 + 1 * LAGER_RECORD_SIZE;
    l[rec1] = 1;
    l[rec1 + 9..rec1 + 11].copy_from_slice(&1i16.to_le_bytes());
    l[big_base + 1 * 2..big_base + 1 * 2 + 2].copy_from_slice(&0i16.to_le_bytes());
    fs::write(dir.join("DATA").join("lager.dat"), &l).unwrap();

    // 7. biglager.dat
    let mut bl = vec![0; BIG_LAGER_PAYLOAD_LEN];
    bl[0..4].copy_from_slice(&1u32.to_le_bytes());
    bl[4..8].copy_from_slice(&(BIG_LAGER_CAPACITY as u32).to_le_bytes());
    bl[8..12].copy_from_slice(&(BIG_LAGER_MEMBERS as u32).to_le_bytes());
    for i in 0..BIG_LAGER_CAPACITY {
        let rec = 12 + i * BIG_LAGER_RECORD_SIZE;
        for m in 0..BIG_LAGER_MEMBERS {
            bl[rec + 1 + m * 2..rec + 1 + m * 2 + 2].copy_from_slice(&(-1i16).to_le_bytes());
        }
    }
    // Big lager 0: member [3]
    let rec_bl = 12 + 0 * BIG_LAGER_RECORD_SIZE;
    bl[rec_bl] = 1;
    bl[rec_bl + 1..rec_bl + 3].copy_from_slice(&3i16.to_le_bytes());
    fs::write(dir.join("DATA").join("biglager.dat"), &bl).unwrap();

    // 8, 9, 10. anim, gfxtype, action
    for (name, h, stride, tails) in [
        ("anim", 8, 21, 2),
        ("gfxtype", 8, 15, 1),
        ("action", 12, 25, 0),
    ] {
        let mut b = vec![0; h + 10 * (stride + tails * 2)];
        b[..4].copy_from_slice(&1u32.to_le_bytes());
        b[4..8].copy_from_slice(&10u32.to_le_bytes());
        if h == 12 {
            b[8..12].copy_from_slice(&6u32.to_le_bytes());
        }
        for i in 0..10 {
            for j in 1..stride {
                b[h + i * stride + j] = (i * 31 + j) as u8;
            }
            b[h + i * stride] = if i < 4 { 1 } else { 0 };
            for t in 0..tails {
                let o = h + 10 * stride + t * 20 + i * 2;
                b[o..o + 2].copy_from_slice(&(0x1100u16 + t as u16 * 256 + i as u16).to_le_bytes());
            }
        }
        fs::write(dir.join("DATA").join(format!("{name}.dat")), &b).unwrap();
    }

    // 11. engine.dat
    let mut engine = vec![0xA3; 94];
    engine[..4].copy_from_slice(&1u32.to_le_bytes());
    engine[34..38].copy_from_slice(&100u32.to_le_bytes());
    fs::write(dir.join("DATA").join("engine.dat"), &engine).unwrap();
}
fn temp_fixture_dir(label: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "ar-placement-test-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&p);
    p
}

fn fixture(label: &str) -> (PathBuf, ObjectStore) {
    let dir = temp_fixture_dir(label);
    create_synthetic_fixture_map(&dir);
    let store = ObjectStore::load(&dir).unwrap();
    (dir, store)
}

fn store_lights(store: &ObjectStore) -> Vec<LightRow> {
    light_rows(&against_rome_data::LightPool::from_payload(store.payload("light").unwrap()).unwrap())
}

#[test]
fn light_and_particle_rows_list_saved_pools() {
    let (dir, store) = fixture("rows");
    let lights = light_rows_from_bytes(&fs::read(dir.join("DATA/light.dat")).unwrap()).unwrap();
    assert_eq!(lights.iter().map(|l| l.slot).collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(lights[1].position, [0.0, 0.0, 0.0]);
    assert_eq!(lights[0].owner, 0);
    let particles = particle_rows_from_bytes(&fs::read(dir.join("DATA/particle.dat")).unwrap()).unwrap();
    assert_eq!(particles.len(), 1);
    assert_eq!(particles[0].owner, 0);
    let child = &particles[0].children[0];
    assert_eq!(child.position, [1.0, 2.0, 3.0]);
    assert_eq!(child.light_link, Some(1));
    assert_eq!(store_lights(&store), lights);
    assert!(light_rows_from_bytes(&[1, 2, 3]).is_err());
}

#[test]
fn roots_preview_and_overlay_follow_the_family_closure() {
    let (_dir, store) = fixture("preview");
    let roots = family_roots(&store, false);
    let root = roots.iter().find(|r| r.slot == 0).expect("slot 0 is a root");
    assert!(root.light_count >= 1 && root.particle_count == 1);
    assert!(family_roots(&store, true).iter().all(|r| r.light_count > 0 || r.particle_count > 0));
    assert!(roots.iter().all(|r| r.slot != 1), "children are not roots");

    let preview = build_family_preview(&store, root.slot, root.uid).unwrap();
    assert_eq!(preview.objects.len(), root.object_count);
    assert_eq!(preview.lights.len(), root.light_count);
    let points = overlay_points(&preview);
    assert!(points.iter().any(|p| p.kind == OverlayKind::Light));
    assert!(points.iter().any(|p| p.kind == OverlayKind::ParticleChild));
    assert!(overlay_bounds(&points).is_some());
    assert!(build_family_preview(&store, 0, 999).is_err(), "stale uid rejected");
}

#[test]
fn native_source_validation_rejects_children_bound_and_missing_slots() {
    let (_dir, store) = fixture("validate");
    let none = BTreeSet::new();
    let uid = store.objects().iter().find(|o| o.slot == 0).unwrap().uid;
    let src = validate_native_source(&store, 0, &none).unwrap();
    assert_eq!((src.slot, src.uid), (0, uid));
    assert!(validate_native_source(&store, 1, &none).is_err(), "child is not a root");
    assert!(validate_native_source(&store, 0, &BTreeSet::from([0])).unwrap_err().contains("binding"));
    assert!(validate_native_source(&store, 9, &none).is_err());
}

#[test]
fn copy_family_is_one_undo_step_and_new_family_is_independent() {
    let (_dir, mut store) = fixture("copy");
    let none = BTreeSet::new();
    let src = validate_native_source(&store, 0, &none).unwrap();
    let before = store.objects().len();
    let lights_before = store_lights(&store).len();
    let delta = copy_delta(256.0, 0.0, 256.0).unwrap();
    assert_eq!(delta, [256 * 65536, 0, 256 * 65536]);
    let (new_slot, new_uid) = copy_family(&mut store, &src, &none, delta).unwrap();
    assert_ne!((new_slot, new_uid), (src.slot, src.uid));
    assert_eq!(store.objects().len(), before + src.object_count);
    let new_obj = store.objects().into_iter().find(|o| o.slot == new_slot).unwrap();
    assert_eq!(new_obj.position[0], src.position[0] + 256.0);
    assert_eq!(new_obj.position[2], src.position[2] + 256.0);
    assert_eq!(store_lights(&store).len(), lights_before + src.light_count);
    assert!(build_family_preview(&store, new_slot, new_uid).is_ok());

    assert!(store.can_undo());
    assert!(store.undo());
    assert_eq!(store.objects().len(), before);
    assert!(store.redo());
    assert_eq!(store.objects().len(), before + src.object_count);

    // A stale uid is refused without touching the store.
    let stale = NativeSource { uid: src.uid + 1000, ..src.clone() };
    assert!(copy_family(&mut store, &stale, &none, delta).is_err());
    assert_eq!(store.objects().len(), before + src.object_count);
}

#[test]
fn copy_delta_rejects_unrepresentable_offsets() {
    assert!(copy_delta(f32::NAN, 0.0, 0.0).is_err());
    assert!(copy_delta(1.0e9, 0.0, 0.0).is_err());
}

#[test]
fn definition_rows_filter_by_name_and_activity() {
    let defs = EffectiveDefinitions::initialized();
    assert!(definition_rows(&defs, "", false).is_empty(), "placeholders are inactive");
    let all = definition_rows(&defs, "", true);
    assert_eq!(all.len(), defs.objects.len());
    assert_eq!(definition_rows(&defs, "7", true).iter().filter(|r| r.id == 7).count(), 1);
    assert!(definition_rows(&defs, "no-such-name", true).is_empty());
}

#[test]
fn family_ui_select_root_tracks_validation() {
    let (_dir, store) = fixture("select");
    let mut ui = FamilyUi::default();
    assert!(ui.select_root(&store, 1, None).is_err());
    assert!(ui.selected.is_none() && !ui.status.is_empty());
    let src = ui.select_root(&store, 0, None).unwrap();
    assert_eq!(ui.selected, Some((src.slot, src.uid)));
    assert!(!ui.take_store_changed());
}
