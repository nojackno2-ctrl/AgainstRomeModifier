//! Fails when the C# `FeatureKeys` canonical ids and `FeatureRegistry::ALL` drift apart.
use against_rome_modifier::features::FeatureRegistry;
use std::collections::BTreeSet;

const SOURCE: &str = include_str!("../../../src.Core/Core/Features/FeatureKey.cs");

/// Returns the first string literal in `text` (no escape handling; ids are plain).
fn first_string_literal(text: &str) -> Option<&str> {
    let start = text.find('"')? + 1;
    let len = text[start..].find('"')?;
    Some(&text[start..start + len])
}

/// Extracts ids from `public static readonly FeatureKey<..> Name = FeatureKey<..>.Bool|Int|Reference<..>("id"...)`
/// declarations inside `class FeatureKeys`. Any declaration in another shape panics rather than being skipped.
fn csharp_ids() -> Vec<String> {
    let body = SOURCE
        .split("class FeatureKeys")
        .nth(1)
        .expect("class FeatureKeys not found");
    let mut ids = Vec::new();
    for line in body.lines().map(str::trim) {
        if !line.starts_with("public static readonly FeatureKey<") {
            continue;
        }
        let rhs = line
            .split_once(" = ")
            .unwrap_or_else(|| panic!("unsupported FeatureKeys declaration: {line}"))
            .1;
        let factory = rhs
            .split_once(">.")
            .unwrap_or_else(|| panic!("unsupported FeatureKeys factory: {line}"))
            .1;
        // Reference<..> may nest generics, so only the leading factory name is inspected.
        let args = if let Some(rest) = factory.strip_prefix("Bool(") {
            rest
        } else if let Some(rest) = factory.strip_prefix("Int(") {
            rest
        } else if factory.starts_with("Reference<") {
            // Skip the (possibly nested) generic argument list.
            let mut depth = 0usize;
            let mut end = None;
            for (i, c) in factory.char_indices() {
                match c {
                    '<' => depth += 1,
                    '>' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let end = end.unwrap_or_else(|| panic!("unbalanced Reference<> in: {line}"));
            factory[end + 1..]
                .strip_prefix('(')
                .unwrap_or_else(|| panic!("unsupported Reference declaration: {line}"))
        } else {
            panic!("unsupported FeatureKeys factory (expected Bool/Int/Reference): {line}");
        };
        let id = first_string_literal(args).unwrap_or_else(|| panic!("no id literal in: {line}"));
        ids.push(id.to_string());
    }
    ids
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
