use against_rome_data::{particle_definitions::*, particle_frames::lifetime_frame};

fn geometry() -> Vec<u8> {
    b"[ParticleGeometrieDefault]\n;idx,activ,name,exsiz,ezsiz,heigh,exadd,eyadd,ezadd,exvar,eyvar,ezvar,ysub,xadd,yadd,zadd\n23,2,GeometryName,3,4,983040,6,7,8,9,10,11,-983040,13,14,15\n".to_vec()
}
fn parameters() -> Vec<u8> {
    b"[ParticleParameterDefault]\n;idx,activ,name,maxam,timer,reflec,birth,fadet,windf,floorc,swarm,timdie,swarad,swarmtim,heisen,refint\n22,2,ParameterName,64,100,0,0,0,0,0,0,0,0,0,0,0\n".to_vec()
}
fn grafiks() -> Vec<u8> {
    b"[ParticleGrafikDefault]\n;idx,activ,name,lolig,obdef,expde,objpa,objpv,objpp\n24,2,GrafikName,-1,227,0,-1,0,0\n".to_vec()
}
fn objects(active: i32, alr: i32, parameter: i32) -> Vec<u8> {
    let mut header: Vec<String> = (0..209).map(|i| format!("field{i}")).collect();
    for (i, name) in [
        (0, "idx"),
        (1, "activ"),
        (2, "alen"),
        (5, "alrid"),
        (52, "name"),
        (63, "papar"),
        (64, "pageo"),
        (65, "pagfx"),
    ] {
        header[i] = name.into();
    }
    let mut text = format!("[ObjectDefaults]\n;{}\n", header.join(","));
    for (id, length, asset, par, geo, gfx) in [
        (830, 999, 777, parameter, 23, 24),
        (227, 100, alr, -1, -1, -1),
    ] {
        let mut row = vec!["0".to_string(); 209];
        for (i, value) in [
            (0, id),
            (1, active),
            (2, length),
            (5, asset),
            (63, par),
            (64, geo),
            (65, gfx),
        ] {
            row[i] = value.to_string();
        }
        row[4] = "3.25".into();
        row[52] = format!("Object{id}");
        text.push_str(&row.join(","));
        text.push('\n');
    }
    text.into_bytes()
}
#[test]
fn raw_geometry_and_native_object_narrowing_are_preserved() {
    let geo = geometry_definitions(&geometry()).unwrap();
    assert_eq!(
        geo[&23].numeric_raw,
        [2, 3, 4, 983040, 6, 7, 8, 9, 10, 11, -983040, 13, 14, 15]
    );
    let obj = object_particle_definitions(&objects(256, 65535, 65558)).unwrap();
    assert_eq!(obj[&830].active_native(), 0);
    assert_eq!(obj[&830].parameter_id(), 22);
    assert_eq!(obj[&227].alr_id(), -1);
    assert_eq!(obj[&227].numeric_raw[2], 65535);
}
#[test]
fn explicit_ids_resolve_different_names_and_use_grafik_object_divisor_and_asset() {
    let obj = object_particle_definitions(&objects(2, 258, 22)).unwrap();
    let par = parameter_definitions(&parameters()).unwrap();
    let geo = geometry_definitions(&geometry()).unwrap();
    let gfx = grafik_definitions(&grafiks()).unwrap();
    let chain = definition_chain(830, &obj, &par, &geo, &gfx).unwrap();
    assert_eq!(chain.grafik_object.id, 227);
    assert_eq!(chain.grafik_object.alr_id(), 258);
    assert_ne!(
        chain.owner.animation_length(),
        chain.grafik_object.animation_length()
    );
    assert_eq!(
        lifetime_frame(50, &chain.frame_context(8, 1, 8).unwrap()),
        Ok(4)
    );
}
#[test]
fn inactive_or_missing_chain_members_and_sentinels_do_not_fallback() {
    let mut obj = object_particle_definitions(&objects(2, 258, 22)).unwrap();
    let par = parameter_definitions(&parameters()).unwrap();
    let mut geo = geometry_definitions(&geometry()).unwrap();
    let gfx = grafik_definitions(&grafiks()).unwrap();
    assert!(matches!(
        definition_chain(999, &obj, &par, &geo, &gfx),
        Err(DefinitionChainError::MissingOwner)
    ));
    geo.get_mut(&23).unwrap().numeric_raw[0] = 0;
    assert!(matches!(
        definition_chain(830, &obj, &par, &geo, &gfx),
        Err(DefinitionChainError::InactiveGeometry)
    ));
    geo.get_mut(&23).unwrap().numeric_raw[0] = 2;
    obj.remove(&227);
    assert!(matches!(
        definition_chain(830, &obj, &par, &geo, &gfx),
        Err(DefinitionChainError::MissingGrafikObject(227))
    ));
    obj.get_mut(&830).unwrap().numeric_raw[3] = -1;
    assert!(matches!(
        definition_chain(830, &obj, &par, &geo, &gfx),
        Err(DefinitionChainError::MissingParameter(-1))
    ));
}
#[test]
fn invalid_object_header_selected_number_count_and_duplicate_id_reject_catalog() {
    let valid = objects(2, 258, 22);
    let bad = String::from_utf8(valid.clone())
        .unwrap()
        .replace("alen", "wrong");
    assert!(object_particle_definitions(bad.as_bytes()).is_err());
    let bad = String::from_utf8(valid.clone())
        .unwrap()
        .replace("830,2,999", "830,2,2147483648");
    assert!(object_particle_definitions(bad.as_bytes()).is_err());
    let bad = String::from_utf8(valid)
        .unwrap()
        .replace("227,2,100", "830,2,100");
    assert!(object_particle_definitions(bad.as_bytes()).is_err());
    assert!(object_particle_definitions(b"[ObjectDefaults]\n0,1").is_err());
}

#[test]
fn absent_primary_object_is_a_declared_sentinel_without_owner_asset_fallback() {
    let obj = object_particle_definitions(&objects(2, 258, 22)).unwrap();
    let par = parameter_definitions(&parameters()).unwrap();
    let geo = geometry_definitions(&geometry()).unwrap();
    let mut gfx = grafik_definitions(&grafiks()).unwrap();
    gfx.get_mut(&24).unwrap().numeric_raw[2] = -1;
    assert!(matches!(
        definition_chain(830, &obj, &par, &geo, &gfx),
        Err(DefinitionChainError::NoPrimaryGrafikObject)
    ));
}

#[test]
fn explicit_object_disp_header_preserves_raw_mode_and_native_word_narrowing() {
    let mut source = String::from_utf8(objects(2, 258, 22)).unwrap();
    assert_eq!(
        object_particle_definitions(source.as_bytes()).unwrap()[&830].mode_native(),
        None
    );
    source = source.replace(",field7,", ",disps,");
    let mut lines: Vec<String> = source.lines().map(str::to_owned).collect();
    for line in &mut lines {
        if line.starts_with("830,") {
            let mut fields: Vec<String> = line.split(',').map(str::to_owned).collect();
            fields[7] = "65539".into();
            *line = fields.join(",");
        }
    }
    let parsed = object_particle_definitions(lines.join("\n").as_bytes()).unwrap();
    assert_eq!(parsed[&830].mode_raw, Some(65539));
    assert_eq!(parsed[&830].mode_native(), Some(3));
}

#[test]
fn installed_objdef_disp_modes_are_captured_without_source_changes() {
    let Ok(root) = std::env::var("ARM_GAME_PATH") else {
        return;
    };
    let path = std::path::Path::new(&root).join("SYSTEM/DATA_MP/DEFAULTS/objdef.dau");
    let input = std::fs::read(&path).unwrap();
    let decoded = if input.starts_with(b"PFIL") {
        against_rome_data::lzss::decompress_pfil(&input, None).unwrap()
    } else {
        input.clone()
    };
    let definitions = object_particle_definitions(&input).unwrap();
    let text = std::str::from_utf8(&decoded).unwrap();
    println!(
        "installed header {:?}",
        text.lines()
            .find(|line| line.trim_start().starts_with(";idx"))
            .map(|line| line.split(',').nth(7))
    );
    let row = text
        .lines()
        .find(|line| line.trim_start().starts_with("227,"))
        .unwrap();
    let expected = row
        .split(',')
        .nth(7)
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    assert_eq!(definitions[&227].mode_raw, Some(expected));
    assert_eq!(definitions[&227].mode_native(), Some(expected as i16));
    assert!(definitions
        .values()
        .all(|definition| definition.mode_raw.is_some()));
    assert_eq!(input, std::fs::read(&path).unwrap());
    println!(
        "installed objdef mode rows{}, primary227 raw{}, source unchanged",
        definitions.len(),
        expected
    );
}
