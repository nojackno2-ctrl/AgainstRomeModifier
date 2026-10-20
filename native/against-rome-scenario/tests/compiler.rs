use against_rome_scenario::*;
fn fixture() -> ScenarioDocument {
    ScenarioDocument::from_json(br#"{"Version":6,"Spawns":[{"Alias":"unit","X":100.5,"Z":120.5,"Team":1,"Count":5,"Id":"12345678-1122-3344-5566-778899001122"},{"Alias":"house","X":50,"Z":70,"Team":0,"Id":"22345678-1122-3344-5566-778899001122","Prebuilt":true},{"Alias":"house","X":200.5,"Z":300.5,"Team":-1,"Id":"32345678-1122-3344-5566-778899001122"}],"DataSlots":[{"Slot":9,"Uid":4294967295,"SpawnId":"22345678-1122-3344-5566-778899001122"}],"Events":[{"Name":"mission","DelaySeconds":3,"Actions":[{"Kind":0,"Text":"Hello"},{"Kind":1,"Team":0,"OtherTeam":2},{"Kind":2,"Team":1,"Alias":"unit","Count":20,"X":200.5,"Z":300.5,"MoveDestination":{"X":600,"Z":700}},{"Kind":3}],"Conditions":[{"Kind":0,"TargetId":"12345678-1122-3344-5566-778899001122"},{"Kind":1,"TargetId":"22345678-1122-3344-5566-778899001122"},{"Kind":2,"TargetId":"22345678-1122-3344-5566-778899001122","MinX":50,"MinZ":60,"MaxX":200,"MaxZ":300}]},{"Name":"repeat","Repeat":true,"DelaySeconds":10,"Actions":[{"Kind":0,"Text":"Repeat"}]},{"Name":"defeat","DelaySeconds":600,"Actions":[{"Kind":4}]}]}"#).unwrap()
}
#[test]
fn image_roundtrip_and_truncation() {
    let b = BciImage::idle().serialize();
    assert_eq!(BciImage::parse(&b).unwrap().serialize(), b);
    for n in 0..b.len() {
        assert!(BciImage::parse(&b[..n]).is_err(), "{n}")
    }
    let mut trailing = b;
    trailing.push(0);
    assert!(BciImage::parse(&trailing).is_err());
}
#[test]
fn compiled_preserves_original_except_hook() {
    let source = BciImage::idle();
    let mut i = source.clone();
    let h = find_wait_hook(&i.code, i.main).unwrap();
    inject_events(&mut i, &fixture(), source.main).unwrap();
    assert_eq!(&i.code[..h], &source.code[..h]);
    assert_eq!(&i.code[h + 2..source.code.len()], &source.code[h + 2..]);
    assert_eq!(i.dtor, source.dtor);
    assert_eq!(BciImage::parse(&i.serialize()).unwrap(), i);
}
#[test]
fn failure_transaction() {
    let mut i = BciImage::idle();
    let old = i.clone();
    let mut d = fixture();
    d.events[0].actions.push(Action::default());
    assert!(inject_events(&mut i, &d, old.main).is_err());
    assert_eq!(i, old);
    let mut bad = StartCamera::default();
    bad.zoom = f64::NAN;
    assert!(inject_camera(&mut i, &bad).is_err());
    assert_eq!(i, old);
}
#[test]
fn rejects_unknown_opcode_and_ambiguous_loop() {
    let mut i = BciImage::idle();
    i.code[10] = 999;
    assert!(find_wait_hook(&i.code, i.main).is_err());
    let i = BciImage {
        code: vec![66, 10, 131, 112, -20, 66, 10, 131, 112, -20],
        main: 0,
        ..BciImage::idle()
    };
    assert_eq!(find_wait_hook(&i.code, 0).unwrap(), 0);
}
#[test]
fn legacy_identity_stable() {
    let d = ScenarioDocument::from_json(br#"{"Version":3,"Spawns":[{"Alias":"x"}]}"#).unwrap();
    assert!(!d.spawns[0].id.is_nil());
    assert_eq!(
        d.spawns[0].id,
        ScenarioDocument::from_json(br#"{"Version":3,"Spawns":[{"Alias":"x"}]}"#)
            .unwrap()
            .spawns[0]
            .id
    );
}
#[test]
fn invalid_model_refused() {
    let mut d = fixture();
    d.spawns[1].id = d.spawns[0].id;
    assert!(d.validate(None).is_err());
    let mut d = fixture();
    d.data_slots.clear();
    assert!(d.validate(None).is_err());
    let mut d = fixture();
    d.events[0].actions[2].move_destination.as_mut().unwrap().x = 20000;
    assert!(d.validate(None).is_err());
}
#[test]
fn camera_abi_and_native_constant_reuse() {
    let mut i = BciImage::idle();
    let native = i.add("s_centerToPos").unwrap();
    let before = i.code.len();
    inject_camera(
        &mut i,
        &StartCamera {
            x: 100.5,
            z: 101.5,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        &i.code[before..before + 8],
        &[66, 102, 66, 100, 128, native, 73, -2]
    );
    assert_eq!(i.main, before as i32 * 4);
    assert_eq!(i.main + i.code[before + 16] + 68, 24);
}
#[test]
#[ignore = "requires the installed retail game; compares against SHA-256 values frozen from the removed C# oracle"]
fn csharp_and_retail_differential() {
    differential(true);
}
#[test]
fn csharp_synthetic_differential() {
    differential(false);
}
/// Compiles with the same inputs the C# oracle saw and compares with outputs frozen from it
/// (tests/fixtures/compiler_idle_primitive{0,1}.bci; retail hashes in compiler_retail_hashes.json).
fn differential(retail: bool) {
    use std::fs;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let document = fixture();
    let idle = BciImage::idle().serialize();
    let mut sources: Vec<(String, Vec<u8>)> = vec![("idle".into(), idle)];
    if retail {
        for name in ["ENDL_000", "TUTOR_01"] {
            let p = format!(r"C:\Program Files (x86)\Against Rome\MAPS\{name}\SCRIPT\ak_level.bci");
            sources.push((name.into(), fs::read(p).unwrap()));
        }
    }
    let retail_hashes: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("compiler_retail_hashes.json")).unwrap()).unwrap();
    let mut checked = 0;
    for (name, original) in &sources {
        for primitive in [0, 1] {
            let camera = StartCamera {
                x: 125.5,
                y: -32.25,
                z: 200.5,
                zoom: 4.25,
                primitive,
                ..Default::default()
            };
            let actual = compile(
                original,
                &document,
                Some(&camera),
                Some(&["unit".into(), "house".into()]),
            )
            .unwrap();
            if name == "idle" {
                assert_eq!(
                    actual,
                    fs::read(root.join(format!("compiler_idle_primitive{primitive}.bci"))).unwrap(),
                    "idle primitive={primitive}"
                );
            } else {
                let frozen = &retail_hashes[name.as_str()][primitive.to_string()];
                assert_eq!(
                    against_rome_data::sha256::sha256_hex(original),
                    frozen["source_sha256"].as_str().unwrap(),
                    "{name}: installed script differs from the one the oracle compiled"
                );
                assert_eq!(
                    against_rome_data::sha256::sha256_hex(&actual),
                    frozen["compiled_sha256"].as_str().unwrap(),
                    "{name} primitive={primitive}"
                );
            }
            assert_eq!(
                compile(
                    original,
                    &document,
                    Some(&camera),
                    Some(&["unit".into(), "house".into()])
                )
                .unwrap(),
                actual
            );
            checked += 1;
        }
    }
    assert_eq!(checked, if retail { 6 } else { 2 });
}

#[test]
fn pfil_script_preserves_header_and_restores_empty() {
    let raw = BciImage::idle().serialize();
    let mut original = vec![0x37; 64];
    original[..4].copy_from_slice(b"PFIL");
    original[16..20].copy_from_slice(&(raw.len() as u32).to_le_bytes());
    for c in raw.chunks(8) {
        original.push(((1u16 << c.len()) - 1) as u8);
        original.extend(c)
    }
    let d = fixture();
    let saved =
        compile_script(&original, &d, None, Some(&["unit".into(), "house".into()])).unwrap();
    let decoded = against_rome_data::lzss::decompress_pfil(&saved, None).unwrap();
    assert_eq!(
        decoded,
        compile(&original, &d, None, Some(&["unit".into(), "house".into()])).unwrap()
    );
    assert_eq!(&saved[..16], &original[..16]);
    assert_eq!(&saved[20..64], &original[20..64]);
    assert_eq!(
        compile_script(&original, &ScenarioDocument::default(), None, None).unwrap(),
        original
    );
}
#[test]
fn json_rejects_null_fields_and_unknown_enum() {
    for json in [
        br#"{"Events":null}"#.as_slice(),
        br#"{"Spawns":null}"#.as_slice(),
        br#"{"Events":[{"Name":"x","Actions":[{"Kind":99}]}]}"#.as_slice(),
    ] {
        assert!(ScenarioDocument::from_json(json).is_err());
    }
}
#[test]
fn original_constants_and_offsets_remain_stable() {
    let mut i = BciImage::idle();
    i.add("pre-existing").unwrap();
    let raw = i.serialize();
    let out = compile(
        &raw,
        &fixture(),
        Some(&StartCamera::default()),
        Some(&["unit".into(), "house".into()]),
    )
    .unwrap();
    let n = BciImage::parse(&out).unwrap();
    assert_eq!(&n.constants[..i.constants.len()], &i.constants);
    assert_eq!(
        &n.constant_offsets[..i.constant_offsets.len()],
        &i.constant_offsets
    );
    assert_eq!(n.dtor, i.dtor);
}

#[test]
fn alias_catalog_and_spawn_validation() {
    assert_eq!(parse_aliases("[Other]\nx=bad\n[objdefname]\n ; comment\n Spear = WikingSpear \ninvalid\n=empty\n[Other]\ny=no"),vec!["Spear"]);
    let d = fixture();
    assert!(d.validate(Some(&["unit".into()])).is_err());
    assert!(d.validate(Some(&["UNIT".into(), "HOUSE".into()])).is_ok());
}

#[test]
fn empty_compile_rejects_bad_bci_before_fastpath() {
    let d = ScenarioDocument::default();
    for input in [b"BCI".as_slice(), b"PFIL".as_slice()] {
        assert!(compile_script(input, &d, None, None).is_err());
        assert!(compile_script_verified(input, &d, None, None, None).is_err());
    }
    let good = BciImage::idle().serialize();
    assert_eq!(compile_script(&good, &d, None, None).unwrap(), good);
}

#[test]
fn unterminated_constant_rejected_before_append_or_empty_restore() {
    let mut image = BciImage::idle();
    image.constants = b"abc".to_vec();
    image.constant_offsets = vec![0];
    assert!(image.constant(0).is_err());
    let raw = image.serialize();
    assert!(BciImage::parse(&raw).is_err());
    for camera in [None, Some(StartCamera::default())] {
        assert!(compile_script(&raw, &ScenarioDocument::default(), camera.as_ref(), None).is_err());
    }
    image.constants.push(0);
    let output = compile_script(
        &image.serialize(),
        &ScenarioDocument::default(),
        Some(&StartCamera::default()),
        None,
    )
    .unwrap();
    assert_eq!(
        BciImage::parse(&output).unwrap().constant(0).unwrap(),
        b"abc"
    );
}

#[test]
fn spawn_and_wave_reject_empty_alias_positions_and_overcount_without_clamping() {
    let good = fixture().spawns[0].clone();
    let aliases = ["unit".to_owned()];
    let mut invalid = vec![];
    for alias in ["", " ", "unit\0"] {
        let mut s = good.clone();
        s.alias = alias.into();
        invalid.push(s);
    }
    for value in [-1., 16384., f32::NAN, f32::INFINITY] {
        let mut s = good.clone();
        s.x = value;
        invalid.push(s);
        let mut s = good.clone();
        s.z = value;
        invalid.push(s);
    }
    for count in [-1, 21, i32::MAX] {
        let mut s = good.clone();
        s.count = count;
        invalid.push(s);
    }
    for spawn in invalid {
        let mut image = BciImage::idle();
        let before = image.clone();
        assert!(
            inject_spawns(&mut image, &[spawn.clone()]).is_err(),
            "{spawn:?}"
        );
        assert_eq!(image, before);
        let d = ScenarioDocument {
            spawns: vec![spawn],
            ..Default::default()
        };
        assert!(compile_script(&before.serialize(), &d, None, Some(&aliases)).is_err());
    }
    for alias in ["", " ", "unit\0"] {
        let mut d = fixture();
        d.events[0].actions[2].alias = alias.into();
        assert!(d.validate(None).is_err());
    }
    for value in [-1., 16384., f32::NAN] {
        let mut d = fixture();
        d.events[0].actions[2].z = value;
        assert!(d.validate(None).is_err());
    }
    let mut d = fixture();
    d.events[0].actions[2].count = 21;
    assert!(d.validate(None).is_err());
    // Inclusive valid boundaries still compile without altering their meaning.
    let mut s = good;
    s.x = 0.;
    s.z = 16383.;
    s.count = 20;
    inject_spawns(&mut BciImage::idle(), &[s]).unwrap();
}

#[test]
fn catalog_failure_never_allows_object_creation() {
    let raw = BciImage::idle().serialize();
    let missing = std::env::temp_dir().join(format!("arm-missing-catalog-{}", new_spawn_id()));
    let failed = load_aliases(&missing).ok();
    for aliases in [failed.as_deref(), Some([].as_slice())] {
        assert!(compile(&raw, &fixture(), None, aliases).is_err());
        assert!(compile_script(&raw, &fixture(), None, aliases).is_err());
        let mut wave_only = fixture();
        wave_only.spawns.clear();
        wave_only.data_slots.clear();
        for e in &mut wave_only.events {
            e.conditions.clear();
        }
        assert!(compile_script(&raw, &wave_only, None, aliases).is_err());
    }
    let d = ScenarioDocument::default();
    assert!(compile_script(&raw, &d, Some(&StartCamera::default()), None).is_ok());
}

fn prebuilt_fixture() -> ScenarioDocument {
    let mut d = fixture();
    let prebuilt = d.spawns.remove(1);
    d.spawns = vec![prebuilt];
    d.events.clear();
    d
}
fn object_file(uid: u32, active: bool) -> Vec<u8> {
    let mut bytes = vec![0; 16 + 10 * 121];
    for (offset, value) in [(0, 1u32), (4, 10), (8, 30), (12, 30)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[16 + 9 * 79] = u8::from(active);
    bytes[19 + 9 * 79..23 + 9 * 79].copy_from_slice(&uid.to_le_bytes());
    bytes
}
#[test]
fn prebuilt_uid_checked_against_source_and_source_changes_are_watched() {
    let temp = std::env::temp_dir().join(format!("arm-scenario-bindings-{}", new_spawn_id()));
    std::fs::create_dir_all(temp.join("DATA")).unwrap();
    let path = temp.join("DATA/objects.dat");
    let d = prebuilt_fixture();
    let raw = BciImage::idle().serialize();
    assert!(compile_script_verified(&raw, &d, None, None, None).is_err());
    for (uid, active) in [(1, true), (u32::MAX, false)] {
        std::fs::write(&path, object_file(uid, active)).unwrap();
        let source = DataBindingSource::load(&temp).unwrap();
        assert!(source.compile_script(&raw, &d, None, None).is_err());
    }
    let good = object_file(u32::MAX, true);
    std::fs::write(&path, &good).unwrap();
    let source = DataBindingSource::load(&temp).unwrap();
    assert_eq!(source.compile_script(&raw, &d, None, None).unwrap(), raw);
    // The user-facing CLI must use the verified boundary too.
    let input = temp.join("original.bci");
    let json = temp.join("arm_scenario.json");
    let output = temp.join("export.bci");
    std::fs::write(&input, &raw).unwrap();
    std::fs::write(&json, serde_json::to_vec(&d).unwrap()).unwrap();
    let cli = |with_source: bool| {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_against-rome-scenario"));
        command.arg("compile").arg(&input).arg(&json).arg(&output);
        if with_source {
            command.arg("-").arg("-").arg(&temp);
        }
        command.output().unwrap()
    };
    assert!(!cli(false).status.success());
    assert!(!output.exists());
    std::fs::write(&path, object_file(42, true)).unwrap();
    assert!(!cli(true).status.success());
    assert!(!output.exists());
    std::fs::write(&path, &good).unwrap();
    let success = cli(true);
    assert!(
        success.status.success(),
        "{}",
        String::from_utf8_lossy(&success.stderr)
    );
    assert_eq!(std::fs::read(&output).unwrap(), raw);
    assert!(
        !cli(true).status.success(),
        "existing output must never be overwritten"
    );
    let mut changed = good.clone();
    *changed.last_mut().unwrap() = 1;
    std::fs::write(&path, &changed).unwrap();
    assert!(source.verify_unchanged().is_err());
    assert!(source.compile_script(&raw, &d, None, None).is_err());
    std::fs::remove_file(&path).unwrap();
    assert!(source.verify_unchanged().is_err());
    std::fs::write(&path, b"bad").unwrap();
    assert!(DataBindingSource::load(&temp).is_err());
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn deleting_prebuilt_removes_its_data_slots_and_allows_export() {
    let mut d = prebuilt_fixture();
    let id = d.spawns[0].id;
    d.remove_spawn(id).unwrap();
    assert!(d.spawns.is_empty());
    assert!(d.data_slots.is_empty());
    let restored = ScenarioDocument::from_json(&serde_json::to_vec(&d).unwrap()).unwrap();
    let raw = BciImage::idle().serialize();
    assert_eq!(
        compile_script_verified(&raw, &restored, None, None, None).unwrap(),
        raw
    );
    assert!(d.remove_spawn(id).is_err());
    let mut referenced = fixture();
    let before = serde_json::to_vec(&referenced).unwrap();
    assert!(referenced.remove_spawn(id).is_err());
    assert_eq!(serde_json::to_vec(&referenced).unwrap(), before);
}

#[test]
fn vidx_bounds_checked_and_negative_rejected() {
    let mut image = BciImage::idle();
    image.variables = b"var1\0".to_vec();
    image.variable_offsets = vec![0];
    let raw = image.serialize();
    assert!(BciImage::parse(&raw).is_ok());

    // Case 1: Negative VIDX offset
    image.variable_offsets = vec![-1];
    let bad_negative = image.serialize();
    assert!(BciImage::parse(&bad_negative).is_err());

    // Case 2: Out of bounds VIDX offset (> variables.len())
    image.variable_offsets = vec![100];
    let bad_oob = image.serialize();
    assert!(BciImage::parse(&bad_oob).is_err());

    // Case 3: Empty variables but non-empty VIDX
    image.variables.clear();
    image.variable_offsets = vec![0];
    let bad_empty_var = image.serialize();
    assert!(BciImage::parse(&bad_empty_var).is_err());
}

#[test]
fn prebuilt_data_binding_management_and_preflight_validation() {
    let mut doc = ScenarioDocument::default();
    let spawn_id = new_spawn_id();
    doc.spawns.push(Spawn {
        alias: "house".into(),
        x: 100.0,
        z: 200.0,
        team: 0,
        count: 0,
        angle: 0,
        y: 0.0,
        prebuilt: true,
        id: spawn_id,
        raw_rotation: None,
    });

    // Test data_binding query when unbound
    assert!(doc.data_binding(spawn_id).is_none());

    // Test binding valid slot
    assert!(doc.bind_prebuilt(spawn_id, 10, 500).is_ok());
    let binding = doc.data_binding(spawn_id).unwrap();
    assert_eq!(binding.slot, 10);
    assert_eq!(binding.uid, 500);

    // Test re-binding updates in place
    assert!(doc.bind_prebuilt(spawn_id, 12, 501).is_ok());
    assert_eq!(doc.data_binding(spawn_id).unwrap().slot, 12);
    assert_eq!(doc.data_slots.len(), 1);

    // Test binding duplicate slot to different spawn
    let spawn_id2 = new_spawn_id();
    doc.spawns.push(Spawn {
        alias: "house".into(),
        x: 150.0,
        z: 250.0,
        team: 0,
        count: 0,
        angle: 0,
        y: 0.0,
        prebuilt: true,
        id: spawn_id2,
        raw_rotation: None,
    });
    assert!(doc.bind_prebuilt(spawn_id2, 12, 999).is_err());

    // Test unbind_prebuilt
    doc.unbind_prebuilt(spawn_id);
    assert!(doc.data_binding(spawn_id).is_none());
    assert_eq!(doc.data_slots.len(), 0);

    // Test prepare_identities on doc with nil IDs
    let mut blank_doc = ScenarioDocument::default();
    blank_doc.spawns.push(Spawn {
        alias: "house".into(),
        x: 100.0,
        z: 200.0,
        team: 0,
        count: 0,
        angle: 0,
        y: 0.0,
        prebuilt: true,
        id: uuid::Uuid::nil(),
        raw_rotation: None,
    });
    assert!(blank_doc.prepare_identities(true).is_ok());
    assert!(!blank_doc.spawns[0].id.is_nil());
    assert_eq!(blank_doc.spawns[0].id, legacy_spawn_id(0));

    // Test preflight_validate with DataBindingSource
    let temp = std::env::temp_dir().join(format!("arm-preflight-{}", new_spawn_id()));
    std::fs::create_dir_all(temp.join("DATA")).unwrap();
    let path = temp.join("DATA/objects.dat");
    std::fs::write(&path, object_file(777, true)).unwrap();
    let source = DataBindingSource::load(&temp).unwrap();
    assert!(source.is_slot_active(9, 777));
    assert_eq!(source.uid_for_slot(9), Some(777));
    assert!(!source.is_slot_active(9, 888));

    // Bind slot 9 to spawn
    blank_doc.bind_prebuilt(blank_doc.spawns[0].id, 9, 777).unwrap();
    assert!(blank_doc.preflight_validate(Some(&["house".into()]), Some(&source)).is_ok());
    assert_eq!(blank_doc.sync_prebuilt_bindings(&source).unwrap(), 1);

    std::fs::remove_dir_all(temp).unwrap();
}
