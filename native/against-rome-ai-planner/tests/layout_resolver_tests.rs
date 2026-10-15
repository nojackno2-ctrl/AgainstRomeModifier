use std::collections::HashMap;
use against_rome_ai_planner::layout_resolver::{LayoutTypeResolver, SdlObjectType};

#[test]
fn test_resolve_alias_matches_exact_alias() {
    let catalog = vec![
        SdlObjectType::new("BauGerHau00_alt", Some("GER_HAU00")),
        SdlObjectType::new("FigGerInf00", Some("GER_INF01")),
    ];

    assert_eq!(
        Some("GER_HAU00".to_string()),
        LayoutTypeResolver::resolve_alias(&catalog, "GER_HAU00")
    );
    assert_eq!(
        Some("GER_INF01".to_string()),
        LayoutTypeResolver::resolve_alias(&catalog, "ger_inf01")
    );
}

#[test]
fn test_resolve_alias_matches_name_def_fallback() {
    let catalog = vec![
        SdlObjectType::new("BauRomTurm01", None::<String>),
        SdlObjectType::new("FigGerUnit", Some("UNIT")),
    ];

    assert_eq!(
        Some("BauRomTurm01".to_string()),
        LayoutTypeResolver::resolve_alias(&catalog, "bauromturm01")
    );
}

#[test]
fn test_resolve_alias_matches_prefix_with_underscore_alphabetically() {
    let catalog = vec![
        SdlObjectType::new("BauGerHau00_b", Some("GER_HAU00_B")),
        SdlObjectType::new("BauGerHau00_a", Some("GER_HAU00_A")),
        SdlObjectType::new("BauGerHau00_c", Some("GER_HAU00_C")),
    ];

    // Should pick the alphabetically earliest name_def: BauGerHau00_a -> GER_HAU00_A
    assert_eq!(
        Some("GER_HAU00_A".to_string()),
        LayoutTypeResolver::resolve_alias(&catalog, "BauGerHau00")
    );
}

#[test]
fn test_resolve_alias_returns_none_when_missing() {
    let catalog = vec![SdlObjectType::new("BauGerHau00", Some("GER_HAU00"))];
    assert_eq!(None, LayoutTypeResolver::resolve_alias(&catalog, "BauGerKas00"));
}

#[test]
fn test_resolve_key_matches_exact_and_prefix() {
    let mut templates = HashMap::new();
    templates.insert("LanGerNad00_Tanne_gross".to_string(), 1);
    templates.insert("LanGerNad00_Tanne_klein".to_string(), 2);
    templates.insert("ExactMatch".to_string(), 3);

    assert_eq!(
        Some("ExactMatch".to_string()),
        LayoutTypeResolver::resolve_key(&templates, "exactmatch")
    );

    // Prefix match: LanGerNad00 -> LanGerNad00_Tanne_gross (alphabetically before klein)
    assert_eq!(
        Some("LanGerNad00_Tanne_gross".to_string()),
        LayoutTypeResolver::resolve_key(&templates, "LanGerNad00")
    );

    assert_eq!(None, LayoutTypeResolver::resolve_key(&templates, "UnknownType"));
}
