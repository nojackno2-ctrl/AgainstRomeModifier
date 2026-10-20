//! Differential and parity tests for LevelObjectStore family operations
//! (parent/child, particle emitter, garrison camp, big lager, child lights, direct light, auxiliary pools).
//!
//! Validates:
//! 1. Family closure discovery (family_details: object, particle, lager, biglager, light slots).
//! 2. Exact signed 16.16 translation of objects, particles, and lights.
//! 3. Cross-pool cloning with disjoint allocation, UID remap, and pointer rewiring.
//! 4. Complete family removal and clean pool resetting.
//! 5. Bit-for-bit parity against sha256 digests frozen from the original C# LevelObjectStore oracle
//!    (verified identical to a live oracle run before the C# sources were removed).

use against_rome_data::sha256::sha256_hex;
use against_rome_placement::{
    ObjectStore, COLUMNS, LAGER_CAPACITY, LAGER_RECORD_SIZE, LAGER_PAYLOAD_LEN,
    BIG_LAGER_CAPACITY, BIG_LAGER_MEMBERS, BIG_LAGER_RECORD_SIZE, BIG_LAGER_PAYLOAD_LEN,
    PARTICLE_CAPACITY, PARTICLE_CHILDREN, PARTICLE_RECORD_SIZE, SEGMENTS,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

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

#[test]
fn family_graph_particle_garrison_closure_and_pool_updates_match_oracle_fixture() {
    let dir = temp_fixture_dir("family-golden");
    create_synthetic_fixture_map(&dir);

    let mut store = ObjectStore::load(&dir).unwrap();

    // 1. Verify family closure graph
    let d = store.family_details(0, 1).unwrap();
    assert_eq!(d.object_slots, vec![0, 1, 2, 3]);
    assert_eq!(d.particle_slots, vec![0]);
    assert_eq!(d.lager_slots, vec![0, 1]);
    assert_eq!(d.big_lager_slots, vec![0]);
    assert_eq!(d.light_slots, vec![1, 0]);

    // 2. Exact signed 16.16 translation
    store
        .translate_family_fixed(0, 1, [65536, 32768, -123])
        .unwrap();

    let expected_move: [(&str, &str); 11] = [
        ("action", "C03AF0DE675DC8248D53EB7808C3001782BCCD8E1AFB4CFD614D593CF66669C2"),
        ("anim", "12642FBD6C36B92539CBECA3A43B4FB796FBCDEC35BE2800787F8F908468C2AD"),
        ("biglager", "C224E11FABCA4A5C5023CE29A3B2E47EBBDB67760098FAAE2833F63FE55482FE"),
        ("engine", "F3D4931422025448114CA2405E7709933099FBA13B95EC1F32099B4B5AE64039"),
        ("gfxtype", "AD93ED8599C30E98CB3986AC84CB15BD8C309A717498CD0759E9C64805F69691"),
        ("lager", "02DD00852DBA82F3C2C35904BA5CF3BA6D7FCB1DCB2F0367B50381B18581F097"),
        ("light", "460EB31203CD49FF1E416FF6A4C8D080663572136C36F011E838B4E57AE6A521"),
        ("objdata", "069A455AB53314F44CDA1FB620EC132B825E5EC92EE8159220711C3A61032865"),
        ("objects", "A2D39DA58EF4B482F6B3AE545B13B62D2282949758B9E04EE05D65F087643290"),
        ("particle", "B66BEFFA69DD729ECFEAEB9D6BFE51F870910A5817077691C00AB1EE74730BCE"),
        ("position", "59EBF0D1741F88AC1D1FF59CD8D9B78EB1FFA5D03370D020F8F03EDCD4C86F68"),
    ];
    for (pool, expected_hash) in expected_move {
        let actual_hash = sha256_hex(store.payload(pool).unwrap()).to_uppercase();
        assert_eq!(actual_hash, expected_hash, "move/{pool} hash mismatch");
    }

    // 3. Independent cross-pool cloning with UID allocation
    let (copy, uid) = store
        .clone_family_fixed(0, 1, [131072, -65536, 4321])
        .unwrap();
    assert_eq!((copy, uid), (4, 100));

    let cloned_details = store.family_details(copy, uid).unwrap();
    assert_eq!(cloned_details.object_slots, vec![4, 5, 6, 7]);
    assert_eq!(cloned_details.particle_slots, vec![1]);
    assert_eq!(cloned_details.lager_slots, vec![2, 3]);
    assert_eq!(cloned_details.big_lager_slots, vec![1]);
    assert_eq!(cloned_details.light_slots, vec![2, 3]);

    let expected_clone: [(&str, &str); 11] = [
        ("action", "197E7F46A45D521829602F89BFF006E506ED4D01AB9B38F2F4B99255D5968D81"),
        ("anim", "903A47C71564961C34286FDF29BBC3A33ED7190D38FA3FA4C4871BD89E190A33"),
        ("biglager", "F0ECB3D820DAAC1F88F9506D672626F6B9FABCA7B62ABF1D28289FBE833506EE"),
        ("engine", "73277040E010090C93CF4C7A577EF73E277DC2E18AC294268A2C14C253F73E29"),
        ("gfxtype", "EF701BA4D5CAEAA9C5CE3EBF4A94D889507089C85ACB48524D7199CB0A3E7094"),
        ("lager", "52477780576627941788290D6420117D74189DB1232BE08C4FE353DF89589B91"),
        ("light", "9D536F17C6679535703D4FA55A80DFA23EA853AB7099CC75EAD93AA6FB4482B2"),
        ("objdata", "96397A6C2438772BFE93CDB935015CBAE378F5F2702C346D5B35433BE7F460E5"),
        ("objects", "E0B0C89E2E8FF09833F480A4E92522BA663A0DD4479D0B01867323D1FD58FD7E"),
        ("particle", "1BBE4D67442795D5FED33B4E392D7C63E8B5F2783EC6C8ED23688C3FE0FA4566"),
        ("position", "3B74F58A6E86562AEECD694257946726C21B29DCE8011D3E370A9A72D7D248B3"),
    ];
    for (pool, expected_hash) in expected_clone {
        let actual_hash = sha256_hex(store.payload(pool).unwrap()).to_uppercase();
        assert_eq!(actual_hash, expected_hash, "clone/{pool} hash mismatch");
    }

    // 4. Whole-family removal and clean resetting
    store.remove_family(copy, uid).unwrap();

    let expected_delete: [(&str, &str); 11] = [
        ("action", "AB7E3591B5B4EFEBC493EC9C3C1E4A0EB52324A15E1570C00CD128020D05B7DD"),
        ("anim", "F1136D9F964C7D6743EF6C37DA90ACCF5796EE82740B3D9965F3F7072F10260A"),
        ("biglager", "C224E11FABCA4A5C5023CE29A3B2E47EBBDB67760098FAAE2833F63FE55482FE"),
        ("engine", "73277040E010090C93CF4C7A577EF73E277DC2E18AC294268A2C14C253F73E29"),
        ("gfxtype", "A8276B2A0B595488B3F5169334C876AC3174D72A2F9872A850D8ACE882CDE475"),
        ("lager", "02DD00852DBA82F3C2C35904BA5CF3BA6D7FCB1DCB2F0367B50381B18581F097"),
        ("light", "460EB31203CD49FF1E416FF6A4C8D080663572136C36F011E838B4E57AE6A521"),
        ("objdata", "069A455AB53314F44CDA1FB620EC132B825E5EC92EE8159220711C3A61032865"),
        ("objects", "BE239F661DDC5E8C3B1150B8BBC6D23F211BAB710E323CBA3F195D3C169C7C7C"),
        ("particle", "B66BEFFA69DD729ECFEAEB9D6BFE51F870910A5817077691C00AB1EE74730BCE"),
        ("position", "59EBF0D1741F88AC1D1FF59CD8D9B78EB1FFA5D03370D020F8F03EDCD4C86F68"),
    ];
    for (pool, expected_hash) in expected_delete {
        let actual_hash = sha256_hex(store.payload(pool).unwrap()).to_uppercase();
        assert_eq!(actual_hash, expected_hash, "delete/{pool} hash mismatch");
    }

    // Original family remains intact and valid
    assert_eq!(store.family_details(0, 1).unwrap(), d);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn family_closure_fail_closed_validation() {
    let dir = temp_fixture_dir("family-fail-closed");
    create_synthetic_fixture_map(&dir);

    let store = ObjectStore::load(&dir).unwrap();

    // Stale or nonexistent UID rejects
    assert!(store.family_details(0, 999).is_err());
    assert!(store.family_details(99, 1).is_err());

    // Selecting non-root child rejects
    let err_child = store.family_details(1, 2).unwrap_err();
    assert!(err_child.contains("select family root"));

    let _ = fs::remove_dir_all(dir);
}
