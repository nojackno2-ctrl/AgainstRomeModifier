//! UnitStatsProjectionService and TroopPresetCodec.Write, ported from src.Core/Core/Services.
use crate::{
    services::BackupManager, troop_config::unit_meta_exact, troops::TroopConfig, PatchProfile,
};
use std::{fmt::Write as _, io};

/// C# UnitStatsProjectionService.Project: baseline stats with the profile's global multipliers applied.
pub fn project(backup: &BackupManager, key: &str, profile: &PatchProfile) -> io::Result<[f64; 9]> {
    let result = backup.get_base_stats_for_unit(key, profile)?;
    Ok(apply_projection(result, key, profile))
}

/// C# `UnitMeta` is an ordinal Dictionary, so the metadata lookup is case-sensitive
/// (unlike the ignore-case balanced-stats table); a miscased key returns the baseline untouched.
fn apply_projection(mut result: [f64; 9], key: &str, profile: &PatchProfile) -> [f64; 9] {
    let Some(meta) = unit_meta_exact(key) else {
        return result;
    };
    if profile.get_bool("RangedRange3x") && TroopConfig::supports_ranged_range_3x(meta.unit_type) {
        result[7] *= 3.0;
    }
    if profile.get_bool("UnitMovementSpeed2x") {
        result[4] *= 2.0;
    }
    if profile.get_bool("AllUnitsEntireMapVision") {
        result[5] = crate::objdef::ENTIRE_MAP_SIGHT as f64;
    }
    if meta.unit_type == "priest" {
        if profile.get_bool("AllUnitsEntireMapVision") || profile.get_bool("SpellEntireMap") {
            result[7] = crate::objdef::ENTIRE_MAP_SIGHT as f64;
        }
        if profile.get_bool("SpellRange3x") && TroopConfig::supports_configurable_spell_radius(key)
        {
            result[8] *= 3.0;
        }
    }
    result
}

/// C# TroopPresetCodec.Write; `generated_at` must already be formatted `yyyy-MM-dd HH:mm:ss` (invariant culture).
pub fn write_preset(stats: &[(String, [f64; 9])], generated_at: &str) -> String {
    let mut out = String::new();
    out.push_str(
        "# Against Rome Modifier - Custom Troop Preset File (independent modifiers removed)\r\n",
    );
    let _ = write!(out, "# Generated on: {generated_at}\r\n");
    out.push_str("# Format: UnitKey=HP,Dmg,VW,AW,Sight,Relt\r\n\r\n");
    for (key, v) in stats {
        let _ = write!(
            out,
            "{key}={},{},{},{},{},{}\r\n",
            v[0], v[1], v[2], v[3], v[5], v[6]
        );
    }
    out
}

/// C# UnitStatsEditorService.NormalizeIndependentFields:
/// Normalizes independent fields without mutating input.
/// Speed (4), Range (7), and SpellRadius (8) are reset to original values;
/// Sight (5) is also reset to original value if the unit is a priest.
pub fn normalize_independent_fields(
    backup: &BackupManager,
    key: &str,
    stats: &[f64; 9],
) -> io::Result<[f64; 9]> {
    let original = backup.get_original_stats(key)?;
    Ok(normalize_against(stats, &original, key))
}

fn normalize_against(stats: &[f64; 9], original: &[f64; 9], key: &str) -> [f64; 9] {
    let mut normalized = *stats;
    for &index in &[4, 7, 8] {
        normalized[index] = original[index];
    }
    if let Some(meta) = unit_meta_exact(key) {
        if meta.unit_type == "priest" {
            normalized[5] = original[5];
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: [f64; 9] = [100., 10., 1., 2., 3., 4., 5., 6., 7.];

    fn profile(flags: &[&str]) -> PatchProfile {
        let mut p = PatchProfile::default();
        for f in flags {
            p.set_bool(*f, true);
        }
        p
    }

    #[test]
    fn projection_applies_multipliers_for_exact_key() {
        let p = profile(&["RangedRange3x", "UnitMovementSpeed2x"]);
        let r = apply_projection(BASE, "FigRomSch01_Bogen", &p);
        assert_eq!(r[7], 18.0);
        assert_eq!(r[4], 6.0);
    }

    #[test]
    fn projection_metadata_lookup_is_case_sensitive() {
        let p = profile(&[
            "RangedRange3x",
            "UnitMovementSpeed2x",
            "AllUnitsEntireMapVision",
        ]);
        for key in [
            "figromsch01_bogen",
            "FIGROMSCH01_BOGEN",
            "FigRomSch01_bogen",
        ] {
            assert!(
                TroopConfig::get_metadata(key).is_none(),
                "exact-key metadata lookup rejects {key}"
            );
            assert!(unit_meta_exact(key).is_none());
            assert_eq!(apply_projection(BASE, key, &p), BASE, "{key}");
        }
    }

    #[test]
    fn projection_spell_radius_requires_exact_priest_key() {
        let p = profile(&["SpellRange3x"]);
        let exact = apply_projection(BASE, "FigKelPri00_Priester", &p);
        assert_eq!(exact[8], 21.0);
        // Metadata miss short-circuits even though SupportsConfigurableSpellRadius is ignore-case.
        assert_eq!(apply_projection(BASE, "figkelpri00_priester", &p), BASE);
    }

    #[test]
    fn normalize_priest_sight_reset_is_case_sensitive() {
        let stats = [1.; 9];
        let orig = [9.; 9];
        let exact = normalize_against(&stats, &orig, "FigGerPri00_Priester");
        assert_eq!(exact[5], 9.0);
        let miscased = normalize_against(&stats, &orig, "figgerpri00_priester");
        assert_eq!(miscased[5], 1.0);
        for i in [4, 7, 8] {
            assert_eq!(miscased[i], 9.0);
        }
    }

    #[test]
    fn exact_lookup_agrees_with_every_builtin_key() {
        for (k, _) in TroopConfig::UNIT_METADATA_ENTRIES {
            assert!(unit_meta_exact(k).is_some(), "{k}");
        }
    }

    #[test]
    fn preset_layout_matches_csharp_columns() {
        let text = write_preset(
            &[(
                "A".into(),
                [100., 12.5, 40., 20., 3.2, 1500., 500., 400., 0.],
            )],
            "2026-01-02 03:04:05",
        );
        assert_eq!(
            text.lines().nth(1),
            Some("# Generated on: 2026-01-02 03:04:05")
        );
        assert!(text.ends_with("A=100,12.5,40,20,1500,500\r\n"));
    }
}
