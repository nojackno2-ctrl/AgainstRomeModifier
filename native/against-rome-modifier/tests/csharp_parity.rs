//! Exe patch tables frozen from the former C# ExePatchModel.cs (fixtures/csharp-frozen-tables.json).
use against_rome_modifier::{FOCUS, ROMAN_ENDLESS, UNIT_RECRUIT_20};
fn frozen() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/csharp-frozen-tables.json")).unwrap()
}
fn u8s(v: &serde_json::Value) -> Vec<u8> {
    v.as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as u8).collect()
}
#[test]
fn exported_rust_definitions_match_current_csharp_source() {
    let f = frozen();
    for (p, prefix) in [
        (FOCUS, "Focus"),
        (ROMAN_ENDLESS, "RomanEndless"),
        (UNIT_RECRUIT_20, "UnitRecruit20"),
    ] {
        let e = &f["exe_patches"][prefix];
        assert_eq!(p.offset as u64, e["offset"].as_u64().unwrap(), "{prefix} offset");
        assert_eq!(p.original, u8s(&e["original"]), "{prefix} original");
        assert_eq!(p.replacement, u8s(&e["patched"]), "{prefix} patched");
    }
}
