//! UnitBaselineCatalog.cs uses exact UnitMeta keys, but case-insensitive row,
//! balanced-stat and custom-stat lookups.
use against_rome_modifier::{
    compress_pfil,
    services::{BackupManager, OBJDEF_KEY},
    ObjdefIndex, PatchProfile, TroopConfig,
};

fn row(key: &str) -> Vec<String> {
    let mut columns = vec!["0".into(); 256];
    columns[ObjdefIndex::Name as usize] = key.into();
    columns[ObjdefIndex::Hp as usize] = "123".into();
    columns[ObjdefIndex::Sirad as usize] = "777".into();
    columns
}

fn backup(rows: &[Vec<String>]) -> BackupManager {
    let mut text = String::from("header\nheader\n");
    for columns in rows {
        text.push_str(&columns.join(","));
        text.push('\n');
    }
    let mut backup = BackupManager::new();
    backup.set_backup_file(OBJDEF_KEY, compress_pfil(text.as_bytes(), None).unwrap());
    backup
}

#[test]
fn backup_rows_require_exact_metadata_or_civilian_key() {
    // UnitBaselineCatalog.GetUnitRows: UnitMeta.ContainsKey(name), plus an exact
    // civilian exception. Unknown and case-variant rows must not be admitted.
    let canonical = "FigRomSch01_Bogen";
    let civilian = "FigZivMan00_Zivilist";
    let backup = backup(&[
        row(canonical),
        row(&canonical.to_ascii_lowercase()),
        row(civilian),
        row(&civilian.to_ascii_lowercase()),
        row("UnknownUnit"),
    ]);
    let rows = backup.get_backup_unit_rows().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.contains_key(canonical));
    assert!(rows.contains_key(civilian));

    let rejected = backup.get_original_stats("UnknownUnit").unwrap();
    assert_eq!(rejected, [0.; 9]);
}

#[test]
fn original_stats_keep_row_lookup_insensitive_but_metadata_exact() {
    // UnitBaselineCatalog.GetOriginalStats: rows.TryGetValue ignores case;
    // UnitMeta.TryGetValue(key) falls back to melee_inf on a case variant.
    for key in ["FigRomSch01_Bogen", "FigGerArt00_Katapult"] {
        let mut columns = row(key);
        let active = ObjdefIndex::Weapon1Akti as usize;
        columns[active] = "1".into();
        columns[active + 1] = "41".into();
        columns[active + 6] = "61".into();
        columns[ObjdefIndex::Weapon1Dtyp as usize] = "1".into();
        let backup = backup(&[columns]);
        let exact = backup.get_original_stats(key).unwrap();
        let variant = backup
            .get_original_stats(&key.to_ascii_lowercase())
            .unwrap();
        assert_eq!((exact[0], exact[1], exact[6]), (123., 41., 61.), "{key}");
        assert_eq!(
            (variant[0], variant[1], variant[6]),
            (123., 0., 0.),
            "{key}"
        );
    }
}

#[test]
fn original_priest_range_requires_exact_metadata() {
    let key = "FigKelPri00_Priester";
    let backup = backup(&[row(key)]);
    let exact = backup.get_original_stats(key).unwrap();
    let variant = backup
        .get_original_stats(&key.to_ascii_lowercase())
        .unwrap();
    assert_eq!((exact[0], exact[7], exact[8]), (123., 777., 500.));
    assert_eq!((variant[0], variant[7], variant[8]), (123., 0., 500.));
}

#[test]
fn custom_priest_sight_requires_exact_metadata() {
    // GetBaseStatsForUnit passes metadata?.UnitType == "priest" to MergeLayers.
    // Balance, custom key matching and spell-radius support still ignore case.
    let key = "FigKelPri00_Priester";
    let mut profile = PatchProfile::default();
    profile.set_bool("Balance", true);
    profile.custom_unit_stats = Some([(key.into(), vec![999.; 9])].into());
    let backup = BackupManager::new();
    let balanced = *TroopConfig::get_balanced_stats(key).unwrap();
    let exact = backup.get_base_stats_for_unit(key, &profile).unwrap();
    let variant = backup
        .get_base_stats_for_unit(&key.to_ascii_lowercase(), &profile)
        .unwrap();
    let mut expected = [999.; 9];
    for index in [4, 5, 7, 8] {
        expected[index] = balanced[index];
    }
    assert_eq!(exact, expected);
    expected[5] = 999.;
    assert_eq!(variant, expected);
}

#[test]
fn custom_stats_apply_without_balance_and_preserve_independent_fields() {
    let key = "FigKelPri00_Priester";
    let backup = backup(&[row(key)]);
    let original = backup.get_original_stats(key).unwrap();
    let mut profile = PatchProfile::new();
    profile.custom_unit_stats = Some([(key.into(), vec![999.; 9])].into());
    let actual = backup.get_base_stats_for_unit(key, &profile).unwrap();
    for index in [0, 1, 2, 3, 6] {
        assert_eq!(actual[index], 999., "可編輯欄位 {index}");
    }
    for index in [4, 5, 7, 8] {
        assert_eq!(actual[index], original[index], "獨立欄位 {index}");
    }
    assert_eq!(backup.get_base_stats_for_unit(key, &PatchProfile::new()).unwrap(), original);
}
