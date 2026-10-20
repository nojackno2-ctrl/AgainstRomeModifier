//! Regenerate with tests/oracle/ModifierServicesOracle.csproj; compares actual C# output,
//! and rejects stale fixtures when either linked C# implementation changes.
use against_rome_modifier::{tga, unit_stats, TroopConfig};
use serde_json::Value;
#[test]
fn csharp_tga_and_preset_runtime_fixtures_match_exactly() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/csharp-services.json")).unwrap();
    let order: Vec<_> = fixture["UnitOrder"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(TroopConfig::sorted_units(), order);
    for key in order {
        let expected: Vec<_> = fixture["BalancedStats"][key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        assert_eq!(
            TroopConfig::get_balanced_stats(key).unwrap().as_slice(),
            expected
        );
    }
    for case in fixture["Images"].as_array().unwrap() {
        let input: Vec<u8> = case["Input"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_u64().unwrap() as u8)
            .collect();
        match tga::decode(&input) {
            None => assert!(case["Image"].is_null()),
            Some(image) => {
                assert_eq!(image.width as u64, case["Image"]["Width"].as_u64().unwrap());
                assert_eq!(
                    image.height as u64,
                    case["Image"]["Height"].as_u64().unwrap()
                );
                let expected: Vec<u8> = case["Image"]["Bgra"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|n| n.as_u64().unwrap() as u8)
                    .collect();
                assert_eq!(image.bgra, expected);
            }
        }
    }
    let mut stats = [0.; 9];
    for (i, n) in fixture["PresetValues"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        stats[i] = n.as_f64().unwrap();
    }
    assert_eq!(
        unit_stats::write_preset(&[("A".into(), stats)], "2026-01-02 03:04:05"),
        fixture["Preset"].as_str().unwrap()
    );
}
