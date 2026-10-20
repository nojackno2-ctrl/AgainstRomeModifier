//! `.arm-layout.json` compatibility with the C# `MapLayoutPresets` serializer (public API only).

use against_rome_editor_app::layout_io::{
    deserialize, plan_requests, serialize, LayoutApplyOptions, MapLayoutKind,
};

/// Verbatim shape of a C# `MapLayoutPresets.Serialize` result (System.Text.Json, indented, CRLF).
const CSHARP_FILE: &str = "{\r\n  \"version\": 1,\r\n  \"kind\": \"Nature\",\r\n  \"entries\": [\r\n    {\r\n      \"type\": \"LanGerNad00_Tanne_gross\",\r\n      \"x\": 12.5,\r\n      \"z\": 300,\r\n      \"heightOffset\": 0.25,\r\n      \"angle\": 359.9,\r\n      \"team\": 0,\r\n      \"count\": 0\r\n    }\r\n  ]\r\n}";

#[test]
fn csharp_file_loads_and_reserializes_identically() {
    let preset = deserialize(CSHARP_FILE).expect("C# layout must load");
    assert_eq!(preset.kind, MapLayoutKind::Nature);
    assert_eq!(preset.entries[0].type_name, "LanGerNad00_Tanne_gross");
    assert_eq!(serialize(&preset).unwrap(), CSHARP_FILE);
}

#[test]
fn camel_case_names_are_required_and_extra_members_rejected() {
    let pascal = CSHARP_FILE.replace("heightOffset", "HeightOffset");
    assert!(deserialize(&pascal).is_err());
    let extra = CSHARP_FILE.replace("\"count\": 0", "\"count\": 0, \"color\": 1");
    assert!(deserialize(&extra).is_err());
}

#[test]
fn imported_nature_layout_plans_without_team() {
    let preset = deserialize(CSHARP_FILE).unwrap();
    let plan = plan_requests(
        &preset,
        &|name| (name == "LanGerNad00_Tanne_gross").then_some(77),
        &LayoutApplyOptions { anchor_tile_x: 1.0, anchor_tile_z: 2.0, rotation: 0.0, team: Some(3) },
        &|_, _| 100.0,
    )
    .unwrap();
    assert_eq!(plan.len(), 1);
    assert_eq!((plan[0].type_id, plan[0].team), (77, None), "nature layouts ignore team override");
    assert_eq!((plan[0].x, plan[0].z, plan[0].y), (268.5, 812.0, 100.25));
}
