//! Independent extraction from the current C# source, never from a game binary.
use against_rome_modifier::{FOCUS, ROMAN_ENDLESS, UNIT_RECRUIT_20};
const SOURCE: &str = include_str!("../../../src.Core/Core/Patches/ExePatchModel.cs");

fn bytes(name: &str) -> Vec<u8> {
    let declaration = SOURCE.split(&format!("byte[] {name} =")).nth(1).unwrap();
    let body = declaration
        .split('{')
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    body.split(',')
        .filter_map(|s| {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                Some(u8::from_str_radix(s.strip_prefix("0x").unwrap(), 16).unwrap())
            }
        })
        .collect()
}
fn offset(name: &str) -> usize {
    let value = SOURCE
        .split(&format!("long {name} ="))
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .trim();
    usize::from_str_radix(value.strip_prefix("0x").unwrap(), 16).unwrap()
}
#[test]
fn exported_rust_definitions_match_current_csharp_source() {
    for (p, prefix) in [
        (FOCUS, "Focus"),
        (ROMAN_ENDLESS, "RomanEndless"),
        (UNIT_RECRUIT_20, "UnitRecruit20"),
    ] {
        assert_eq!(
            p.offset,
            offset(&format!("{prefix}PatchOffset")),
            "{prefix} offset"
        );
        assert_eq!(
            p.original,
            bytes(&format!("{prefix}OriginalBytes")),
            "{prefix} original"
        );
        assert_eq!(
            p.replacement,
            bytes(&format!("{prefix}PatchedBytes")),
            "{prefix} patched"
        );
    }
}
