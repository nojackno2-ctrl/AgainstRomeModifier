//! Fails when the frozen C# `FeatureKeys` canonical ids (fixtures/csharp-frozen-tables.json) and `FeatureRegistry::ALL` drift apart.
use against_rome_modifier::features::FeatureRegistry;
use std::collections::BTreeSet;

fn csharp_ids() -> Vec<String> {
    let f: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/csharp-frozen-tables.json")).unwrap();
    f["feature_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn csharp_feature_keys_match_rust_feature_registry() {
    let csharp = csharp_ids();
    assert!(!csharp.is_empty(), "parsed no C# feature keys");

    let csharp_set: BTreeSet<&str> = csharp.iter().map(String::as_str).collect();
    assert_eq!(
        csharp_set.len(),
        csharp.len(),
        "duplicate C# FeatureKey ids: {csharp:?}"
    );

    let rust: Vec<&str> = FeatureRegistry::ALL.iter().map(|f| f.id).collect();
    let rust_set: BTreeSet<&str> = rust.iter().copied().collect();
    assert_eq!(
        rust_set.len(),
        rust.len(),
        "duplicate Rust FeatureRegistry ids: {rust:?}"
    );

    let missing_in_rust: Vec<_> = csharp_set.difference(&rust_set).collect();
    let unexpected_in_rust: Vec<_> = rust_set.difference(&csharp_set).collect();
    assert!(
        missing_in_rust.is_empty() && unexpected_in_rust.is_empty(),
        "feature id drift: missing in Rust = {missing_in_rust:?}; present only in Rust = {unexpected_in_rust:?}"
    );
}
