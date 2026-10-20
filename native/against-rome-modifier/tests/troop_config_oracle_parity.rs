//! TroopConfig parity: the Rust tables must match the tables frozen from the former C# TroopConfig.cs
//! (fixtures/csharp-frozen-tables.json). It defines 43 unique units
//! (not 44) in each of UnitNames, UnitOrder, UnitMeta and BalancedUnitStats.
use against_rome_modifier::TroopConfig;
use std::collections::HashSet;

fn frozen() -> serde_json::Value {
    let f: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/csharp-frozen-tables.json")).unwrap();
    f["troop"].clone()
}

fn strs(v: &serde_json::Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()
}

fn cs_names() -> Vec<(String, String)> {
    frozen()["names"].as_array().unwrap().iter().map(|p| { let q = strs(p); (q[0].clone(), q[1].clone()) }).collect()
}

fn cs_order() -> Vec<String> {
    strs(&frozen()["order"])
}

fn cs_meta() -> Vec<(String, [String; 4])> {
    frozen()["meta"].as_array().unwrap().iter().map(|p| {
        let q = strs(&p[1]);
        (p[0].as_str().unwrap().to_string(), [q[0].clone(), q[1].clone(), q[2].clone(), q[3].clone()])
    }).collect()
}

fn cs_stats() -> Vec<(String, Vec<f64>)> {
    frozen()["stats"].as_array().unwrap().iter().map(|p| {
        (p[0].as_str().unwrap().to_string(), p[1].as_array().unwrap().iter().map(|n| n.as_f64().unwrap()).collect())
    }).collect()
}

fn unique(keys: impl Iterator<Item = String>) -> usize {
    keys.collect::<HashSet<_>>().len()
}

#[test]
fn csharp_defines_43_unique_units_in_every_table() {
    assert_eq!(cs_names().len(), 43);
    assert_eq!(unique(cs_names().into_iter().map(|x| x.0)), 43);
    assert_eq!(cs_order().len(), 43);
    assert_eq!(unique(cs_order().into_iter()), 43);
    assert_eq!(cs_meta().len(), 43);
    assert_eq!(unique(cs_meta().into_iter().map(|x| x.0)), 43);
    assert_eq!(cs_stats().len(), 43);
    assert_eq!(unique(cs_stats().into_iter().map(|x| x.0)), 43);
}

#[test]
fn rust_names_match_csharp_exactly() {
    let expected = cs_names();
    let actual: Vec<_> = TroopConfig::UNIT_NAME_ENTRIES
        .iter()
        .map(|(k, n)| (k.to_string(), n.to_string()))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn rust_metadata_matches_csharp_exactly() {
    let expected = cs_meta();
    let actual: Vec<_> = TroopConfig::UNIT_METADATA_ENTRIES
        .iter()
        .map(|(k, m)| {
            (
                k.to_string(),
                [
                    m.faction.into(),
                    m.tier.into(),
                    m.unit_type.into(),
                    m.style.into(),
                ],
            )
        })
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn rust_balanced_stats_match_csharp_exactly() {
    let expected = cs_stats();
    for (key, nums) in &expected {
        assert_eq!(nums.len(), 9, "{key}");
    }
    let mut actual: Vec<_> = TroopConfig::BALANCED_STATS_ENTRIES
        .iter()
        .map(|(k, s)| (k.to_string(), s.to_vec()))
        .collect();
    let mut expected = expected;
    actual.sort_by(|a, b| a.0.cmp(&b.0));
    expected.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(&expected) {
        assert_eq!(a.0, e.0);
        for (x, y) in a.1.iter().zip(&e.1) {
            assert_eq!(x.to_bits(), y.to_bits(), "{}", a.0);
        }
    }
    // Case-insensitive lookup, like StringComparer.OrdinalIgnoreCase.
    assert!(TroopConfig::get_balanced_stats("figromanf00_anfuehrer").is_some());
}

#[test]
fn sorted_units_match_csharp_static_ctor_stable_tier_sort() {
    let meta = cs_meta();
    let prio = |t: &str| match t {
        "low" => 1,
        "mid" => 2,
        "high" => 3,
        "ace" => 4,
        "leader" => 5,
        "siege" => 6,
        _ => 99,
    };
    let tier_of = |k: &str| {
        meta.iter()
            .find(|m| m.0 == k)
            .map(|m| m.1[1].clone())
            .unwrap_or("low".into())
    };
    let mut indexed: Vec<(usize, String)> = cs_order().into_iter().enumerate().collect();
    indexed.sort_by_key(|(i, k)| (prio(&tier_of(k)), *i));
    let expected: Vec<String> = indexed.into_iter().map(|x| x.1).collect();
    let actual: Vec<String> = TroopConfig::sorted_units()
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 43);
}

#[test]
fn all_names_metadata_and_stats_key_sets_agree() {
    let n: HashSet<_> = cs_names().into_iter().map(|x| x.0).collect();
    let m: HashSet<_> = cs_meta().into_iter().map(|x| x.0).collect();
    let s: HashSet<_> = cs_stats().into_iter().map(|x| x.0).collect();
    let o: HashSet<_> = cs_order().into_iter().collect();
    assert_eq!(n, m);
    assert_eq!(m, s);
    assert_eq!(s, o);
}
