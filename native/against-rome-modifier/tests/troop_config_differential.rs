//! Differential test against the real C# TroopConfig compiled by tests/oracle_troop_config.
//! Expected output is a frozen C# oracle fixture; the oracle source was retired with the .NET projects.
use against_rome_modifier::{
    troop_config::*,
    troops::{ObjdefIndex, TroopConfig},
};
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/csharp-troop-config.json")).unwrap()
}

fn expected(v: &Value, name: &str) -> Vec<(String, i64)> {
    let mut out: Vec<_> = v[name]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, n)| (k.clone(), n.as_i64().unwrap()))
        .collect();
    out.sort();
    out
}

fn sorted(v: Vec<(&str, i64)>) -> Vec<(String, i64)> {
    let mut o: Vec<_> = v.into_iter().map(|(k, n)| (k.to_string(), n)).collect();
    o.sort();
    o
}

#[test]
fn index_enums_match_csharp() {
    let f = fixture();
    use ObjdefIndex as O;
    assert_eq!(
        expected(&f, "ObjdefIndex"),
        sorted(vec![
            ("Moves", O::Moves as i64),
            ("Hp", O::Hp as i64),
            ("Movsf", O::Movsf as i64),
            ("Sirad", O::Sirad as i64),
            ("Name", O::Name as i64),
            ("Weapon1Akti", O::Weapon1Akti as i64),
            ("Weapon1Dam", O::Weapon1Dam as i64),
            ("Weapon1RangeMin", O::Weapon1RangeMin as i64),
            ("Weapon1RangeMax", O::Weapon1RangeMax as i64),
            ("Weapon1Relt", O::Weapon1Relt as i64),
            ("Weapon1Emit", O::Weapon1Emit as i64),
            ("Weapon1Drad", O::Weapon1Drad as i64),
            ("Aw", O::Aw as i64),
            ("Vw", O::Vw as i64),
            ("HousingCapacity", O::HousingCapacity as i64),
            ("Bmovs", O::Bmovs as i64),
            ("Weapon1Dtyp", O::Weapon1Dtyp as i64),
            ("StorageCapacity", O::StorageCapacity as i64),
            ("Lpsub", O::Lpsub as i64),
        ])
    );
    use RessIndex as R;
    assert_eq!(
        expected(&f, "RessIndex"),
        sorted(vec![
            ("BauBuildCostStart", R::BauBuildCostStart as i64),
            ("BauBuildCostEnd", R::BauBuildCostEnd as i64),
            ("BauUpgradeCostStart", R::BauUpgradeCostStart as i64),
            ("BauUpgradeCostEnd", R::BauUpgradeCostEnd as i64),
            ("FigProdCostStart", R::FigProdCostStart as i64),
            ("FigProdCostEnd", R::FigProdCostEnd as i64),
            ("FigPriestSpellCostStart", R::FigPriestSpellCostStart as i64),
            ("FigPriestSpellCostEnd", R::FigPriestSpellCostEnd as i64),
            (
                "FigSiegeBuildCostStart",
                R::FIG_SIEGE_BUILD_COST_START as i64
            ),
            ("FigSiegeBuildCostEnd", R::FIG_SIEGE_BUILD_COST_END as i64),
        ])
    );
    use VolkresIndex as V;
    assert_eq!(
        expected(&f, "VolkresIndex"),
        sorted(vec![
            ("ResearchUpgradeWood1", V::ResearchUpgradeWood1 as i64),
            ("ResearchUpgradeGold1", V::ResearchUpgradeGold1 as i64),
            ("ResearchUpgradeWood2", V::ResearchUpgradeWood2 as i64),
            ("ResearchUpgradeGold2", V::ResearchUpgradeGold2 as i64),
            ("TechCostStart", V::TechCostStart as i64),
            ("TechCostEnd", V::TechCostEnd as i64),
            ("UnitUpgradeStart", V::UnitUpgradeStart as i64),
            ("UnitUpgradeEnd", V::UnitUpgradeEnd as i64),
        ])
    );
}

#[test]
fn tables_and_lookups_match_csharp() {
    let f = fixture();
    let names = f["UnitNames"].as_array().unwrap();
    assert_eq!(names.len(), TroopConfig::UNIT_NAME_ENTRIES.len());
    for e in names {
        assert_eq!(unit_name_exact(e[0].as_str().unwrap()), e[1].as_str());
    }
    let meta = f["UnitMeta"].as_array().unwrap();
    assert_eq!(meta.len(), TroopConfig::UNIT_METADATA_ENTRIES.len());
    for e in meta {
        let m = unit_meta_exact(e[0].as_str().unwrap()).unwrap();
        assert_eq!(
            [m.faction, m.tier, m.unit_type, m.style],
            [1, 2, 3, 4].map(|i| e[i].as_str().unwrap())
        );
    }
    assert_eq!(
        f["BalancedStats"].as_object().unwrap().len(),
        TroopConfig::BALANCED_STATS_ENTRIES.len()
    );
    let lower = f["LowerKey"].as_str().unwrap();
    assert_eq!(
        unit_name_exact(lower).is_some(),
        f["LowerInNames"].as_bool().unwrap()
    );
    assert_eq!(
        unit_meta_exact(lower).is_some(),
        f["LowerInMeta"].as_bool().unwrap()
    );
    assert_eq!(
        balanced_stats_ignore_case(lower).is_some(),
        f["LowerInBalanced"].as_bool().unwrap()
    );
}

#[test]
fn ranged_range_3x_matches_csharp_including_case() {
    for (t, want) in fixture()["SupportsRangedRange3x"].as_object().unwrap() {
        assert_eq!(
            TroopConfig::supports_ranged_range_3x(t),
            want.as_bool().unwrap(),
            "{t:?}"
        );
    }
}

#[test]
fn static_constructor_invariants_hold() {
    assert_eq!(validate_tables(), Ok(()));
}
