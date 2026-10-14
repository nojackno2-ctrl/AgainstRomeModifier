use against_rome_data::particle_definitions::*;

const HEADER:&str="[ParticleParameterDefault]\r\n;idx,activ,name,maxam,timer,reflec,birth,fadet,windf,floorc,swarm,timdie,swarad,swarmtim,heisen,refint\r\n";
fn fixture(row: &str) -> Vec<u8> {
    format!("{HEADER}{row}\r\n").into_bytes()
}
#[test]
fn parameter_callback_positions_keep_all_signed_words_and_nonboolean_flags() {
    let source = fixture("511,2,Pfeil00,3,15000,5,6,7,8,9,-1,11,12,13,-7,15");
    let parsed = parameter_definitions(&source).unwrap();
    let row = &parsed[&511];
    assert_eq!(row.active_raw(), 2);
    assert_eq!(row.lifetime(), 15000);
    assert_eq!(row.skip_first_raw(), -1);
    assert_eq!(row.velocity_branch_raw(), -7);
    assert_eq!(
        row.numeric_raw,
        [2, 3, 15000, 5, 6, 7, 8, 9, -1, 11, 12, 13, -7, 15]
    );
    assert_eq!(row.name, "Pfeil00");
    let context = row.frame_context(15000, 8, 1, 8).unwrap();
    assert_eq!(context.definition_lifetime, 15000);
    assert_eq!(context.skip_first_raw, -1);
    assert_eq!(context.velocity_branch_raw, -7);
}
#[test]
fn gfx_object_is_column_four_and_inactive_rows_are_preserved() {
    let source=b"[ParticleGrafikDefault]\n;idx,activ,name,lolig,obdef,expde,objpa,objpv,objpp\n24,0,Pfeil00,17,227,19,-1,-2,23\n";
    let parsed = grafik_definitions(source).unwrap();
    let row = &parsed[&24];
    assert_eq!(row.active_raw(), 0);
    assert_eq!(row.object_definition(), 227);
    assert_eq!(row.numeric_raw, [0, 17, 227, 19, -1, -2, 23]);
}
#[test]
fn malformed_tables_are_rejected_without_partial_catalog() {
    for row in [
        "512,1,X,3,4,5,6,7,8,9,10,11,12,13,14,15",
        "-1,1,X,3,4,5,6,7,8,9,10,11,12,13,14,15",
        "0,1,X,3,2147483648,5,6,7,8,9,10,11,12,13,14,15",
        "0,1,X,3,4",
        "0,1,two words,3,4,5,6,7,8,9,10,11,12,13,14,15",
    ] {
        assert!(parameter_definitions(&fixture(row)).is_err());
    }
    let valid = "0,1,X,3,4,5,6,7,8,9,10,11,12,13,14,15";
    assert!(parameter_definitions(&fixture(&format!("{valid}\n{valid}"))).is_err());
    assert!(parameter_definitions(b"[ParticleParameterDefault]\n0,1,X").is_err());
    let mut bad = fixture(valid);
    bad.push(255);
    assert!(parameter_definitions(&bad).is_err());
    let repeated = format!("{HEADER}{valid}\n{HEADER}{valid}");
    assert!(parameter_definitions(repeated.as_bytes()).is_err());
}

#[test]
fn parsed_parameter_connects_to_lifetime_resolver_and_inactive_rows_do_not_resolve() {
    use against_rome_data::particle_frames::lifetime_frame;
    let parsed = parameter_definitions(&fixture("0,2,Smoke,64,100,0,0,0,0,0,0,0,0,0,0,0")).unwrap();
    let context = parsed[&0].frame_context(100, 8, 1, 8).unwrap();
    assert_eq!(lifetime_frame(50, &context), Ok(4));
    let inactive =
        parameter_definitions(&fixture("0,0,Smoke,64,100,0,0,0,0,0,0,0,0,0,0,0")).unwrap();
    assert!(inactive[&0].frame_context(100, 8, 1, 8).is_err());
}
