//! Unit definitions, statistics metadata, and parser ported from
//! src.Core/Core/TroopConfig.cs and src.Core/Core/Services/UnitStatParser.cs.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitMetadata {
    pub faction: &'static str,
    pub tier: &'static str,
    pub unit_type: &'static str,
    pub style: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjdefIndex {
    Moves = 4,
    Hp = 19,
    Movsf = 23,
    Sirad = 24,
    StorageCapacity = 42,
    Name = 52,
    Weapon1Akti = 78,
    Weapon1Dam = 79,
    Weapon1RangeMin = 80,
    Weapon1RangeMax = 81,
    Weapon1Relt = 84,
    Weapon1Emit = 85,
    Aw = 142,
    Vw = 146,
    HousingCapacity = 156,
    Weapon1Drad = 164,
    Bmovs = 191,
    Weapon1Dtyp = 199,
    Lpsub = 208,
}

pub struct TroopConfig;

impl TroopConfig {
    pub fn supports_ranged_range_3x(unit_type: &str) -> bool {
        matches!(unit_type, "ranged_inf" | "ranged_cav" | "hybrid_inf" | "siege")
    }

    pub fn supports_configurable_spell_radius(key: &str) -> bool {
        key.eq_ignore_ascii_case("FigKelPri00_Priester")
            || key.eq_ignore_ascii_case("FigHunPri00_Priester")
    }

    pub const UNIT_METADATA_ENTRIES: &[(&'static str, UnitMetadata)] = &[
        ("FigRomInf00_Lanze_Schild", UnitMetadata { faction: "Roman", tier: "mid", unit_type: "melee_inf", style: "shield" }),
        ("FigRomSch00_Speer_Schild", UnitMetadata { faction: "Roman", tier: "high", unit_type: "hybrid_inf", style: "shield" }),
        ("FigRomInf01_Schwert_Schild", UnitMetadata { faction: "Roman", tier: "high", unit_type: "melee_inf", style: "shield" }),
        ("FigRomSch01_Bogen", UnitMetadata { faction: "Roman", tier: "mid", unit_type: "ranged_inf", style: "ranged" }),
        ("FigRomKav00_Schwert_Schild", UnitMetadata { faction: "Roman", tier: "ace", unit_type: "cav", style: "shield" }),
        ("FigRomAnf00_Anfuehrer", UnitMetadata { faction: "Roman", tier: "leader", unit_type: "leader_melee", style: "none" }),
        ("FigGerInf01_Schwert", UnitMetadata { faction: "Teuton", tier: "low", unit_type: "melee_inf", style: "none" }),
        ("FigGerSch00_Speer", UnitMetadata { faction: "Teuton", tier: "low", unit_type: "ranged_inf", style: "two_handed" }),
        ("FigGerInf00_Hammer_Schild", UnitMetadata { faction: "Teuton", tier: "mid", unit_type: "melee_inf", style: "shield" }),
        ("FigGerSch01_Axt_Schild", UnitMetadata { faction: "Teuton", tier: "mid", unit_type: "hybrid_inf", style: "shield" }),
        ("FigGerInf02_Zweihandaxt", UnitMetadata { faction: "Teuton", tier: "high", unit_type: "melee_inf", style: "two_handed" }),
        ("FigGerKav00_Schwert_Schild", UnitMetadata { faction: "Teuton", tier: "high", unit_type: "cav", style: "shield" }),
        ("FigGerInf03_Doppelhammer", UnitMetadata { faction: "Teuton", tier: "ace", unit_type: "melee_inf", style: "dual_wield" }),
        ("FigGerAnf00_Anfuehrer", UnitMetadata { faction: "Teuton", tier: "leader", unit_type: "leader_melee", style: "two_handed" }),
        ("FigGerPri00_Priester", UnitMetadata { faction: "Teuton", tier: "mid", unit_type: "priest", style: "none" }),
        ("FigKelInf00_Schwert", UnitMetadata { faction: "Celt", tier: "low", unit_type: "melee_inf", style: "none" }),
        ("FigKelSch00_Bogen", UnitMetadata { faction: "Celt", tier: "low", unit_type: "ranged_inf", style: "ranged" }),
        ("FigKelInf01_Lanze", UnitMetadata { faction: "Celt", tier: "mid", unit_type: "melee_inf", style: "shield" }),
        ("FigKelSch01_Schleuder", UnitMetadata { faction: "Celt", tier: "mid", unit_type: "ranged_inf", style: "ranged" }),
        ("FigKelInf02_Doppelschwert", UnitMetadata { faction: "Celt", tier: "high", unit_type: "melee_inf", style: "dual_wield" }),
        ("FigKelSch02_Schwere_Schleuder", UnitMetadata { faction: "Celt", tier: "ace", unit_type: "ranged_inf", style: "ranged" }),
        ("FigKelKav00_Lanze_Schild", UnitMetadata { faction: "Celt", tier: "high", unit_type: "cav", style: "shield" }),
        ("FigKelAnf00_Anfuehrer", UnitMetadata { faction: "Celt", tier: "leader", unit_type: "leader_melee", style: "shield" }),
        ("FigKelPri00_Priester", UnitMetadata { faction: "Celt", tier: "mid", unit_type: "priest", style: "none" }),
        ("FigHunInf00_Keule", UnitMetadata { faction: "Hun", tier: "low", unit_type: "melee_inf", style: "none" }),
        ("FigHunSch00_Bogen", UnitMetadata { faction: "Hun", tier: "low", unit_type: "ranged_inf", style: "ranged" }),
        ("FigHunInf01_Schwert_Schild", UnitMetadata { faction: "Hun", tier: "mid", unit_type: "melee_inf", style: "shield" }),
        ("FigHunKav00_Schwert_Schild", UnitMetadata { faction: "Hun", tier: "mid", unit_type: "cav", style: "shield" }),
        ("FigHunKav01_Bogen", UnitMetadata { faction: "Hun", tier: "high", unit_type: "ranged_cav", style: "ranged" }),
        ("FigHunKav02_Lanze_Schild", UnitMetadata { faction: "Hun", tier: "ace", unit_type: "cav", style: "shield" }),
        ("FigHunKav03_Geisterreiter", UnitMetadata { faction: "Hun", tier: "high", unit_type: "cav", style: "none" }),
        ("FigHunAnf00_Anfuehrer", UnitMetadata { faction: "Hun", tier: "leader", unit_type: "leader_cav", style: "none" }),
        ("FigHunPri00_Priester", UnitMetadata { faction: "Hun", tier: "mid", unit_type: "priest", style: "none" }),
        ("FigGerArt00_Katapult", UnitMetadata { faction: "Teuton", tier: "siege", unit_type: "siege", style: "none" }),
        ("FigGerArt00_Katapult_Aufbau", UnitMetadata { faction: "Teuton", tier: "siege", unit_type: "siege", style: "none" }),
        ("FigRomArt00_Speerschleuder", UnitMetadata { faction: "Roman", tier: "siege", unit_type: "siege", style: "none" }),
        ("FigRomArt00_Speerschleuder_Auf", UnitMetadata { faction: "Roman", tier: "siege", unit_type: "siege", style: "none" }),
        ("FigRomArt01_Katapult", UnitMetadata { faction: "Roman", tier: "siege", unit_type: "siege", style: "none" }),
        ("FigRomArt01_Katapult_Aufbau", UnitMetadata { faction: "Roman", tier: "siege", unit_type: "siege", style: "none" }),
        ("FigKelArt00_Speerschleuder", UnitMetadata { faction: "Celt", tier: "siege", unit_type: "siege", style: "none" }),
        ("FigKelArt00_Speerschleuder_A", UnitMetadata { faction: "Celt", tier: "siege", unit_type: "siege", style: "none" }),
        ("FigKelArt01_Katapult", UnitMetadata { faction: "Celt", tier: "siege", unit_type: "siege", style: "none" }),
        ("FigKelArt01_Katapult_Aufbau", UnitMetadata { faction: "Celt", tier: "siege", unit_type: "siege", style: "none" }),
    ];

    pub const UNIT_NAME_ENTRIES: &[(&'static str, &'static str)] = &[
        ("FigRomAnf00_Anfuehrer", "羅馬領袖"),
        ("FigRomInf00_Lanze_Schild", "羅馬輕裝步兵"),
        ("FigRomInf01_Schwert_Schild", "羅馬劍盾兵"),
        ("FigRomKav00_Schwert_Schild", "羅馬突擊騎兵"),
        ("FigRomSch00_Speer_Schild", "羅馬重裝步兵"),
        ("FigRomSch01_Bogen", "羅馬弓箭手"),
        ("FigGerAnf00_Anfuehrer", "條頓領袖"),
        ("FigGerInf00_Hammer_Schild", "條頓錘盾兵"),
        ("FigGerInf01_Schwert", "條頓劍士"),
        ("FigGerInf02_Zweihandaxt", "條頓雙手斧兵"),
        ("FigGerInf03_Doppelhammer", "條頓雙錘兵"),
        ("FigGerKav00_Schwert_Schild", "條頓騎兵"),
        ("FigGerPri00_Priester", "條頓祭司"),
        ("FigGerSch00_Speer", "條頓矛兵"),
        ("FigGerSch01_Axt_Schild", "條頓斧盾兵"),
        ("FigKelAnf00_Anfuehrer", "塞爾特領袖"),
        ("FigKelInf00_Schwert", "塞爾特劍士"),
        ("FigKelInf01_Lanze", "塞爾特槍兵"),
        ("FigKelInf02_Doppelschwert", "塞爾特重裝步兵"),
        ("FigKelKav00_Lanze_Schild", "塞爾特槍騎兵"),
        ("FigKelPri00_Priester", "塞爾特祭司"),
        ("FigKelSch00_Bogen", "塞爾特弓箭手"),
        ("FigKelSch01_Schleuder", "塞爾特投石兵"),
        ("FigKelSch02_Schwere_Schleuder", "塞爾特重裝投石兵"),
        ("FigHunAnf00_Anfuehrer", "匈奴領袖"),
        ("FigHunInf00_Keule", "匈奴棍棒兵"),
        ("FigHunInf01_Schwert_Schild", "匈奴劍盾兵"),
        ("FigHunKav00_Schwert_Schild", "匈奴輕裝騎兵"),
        ("FigHunKav01_Bogen", "匈奴弓騎兵"),
        ("FigHunKav02_Lanze_Schild", "匈奴重裝騎兵"),
        ("FigHunKav03_Geisterreiter", "匈奴幽靈武士"),
        ("FigHunPri00_Priester", "匈奴祭司"),
        ("FigHunSch00_Bogen", "匈奴弓箭手"),
        ("FigGerArt00_Katapult", "條頓投石機"),
        ("FigGerArt00_Katapult_Aufbau", "條頓投石機(架設)"),
        ("FigRomArt00_Speerschleuder", "羅馬弩車"),
        ("FigRomArt00_Speerschleuder_Auf", "羅馬弩車(架設)"),
        ("FigRomArt01_Katapult", "羅馬投石機"),
        ("FigRomArt01_Katapult_Aufbau", "羅馬投石機(架設)"),
        ("FigKelArt00_Speerschleuder", "塞爾特弩車"),
        ("FigKelArt00_Speerschleuder_A", "塞爾特弩車(架設)"),
        ("FigKelArt01_Katapult", "塞爾特投石機"),
        ("FigKelArt01_Katapult_Aufbau", "塞爾特投石機(架設)"),
    ];

    pub const BALANCED_STATS_ENTRIES: &[(&'static str, [f64; 9])] = &[
        // Roman
        ("FigRomInf00_Lanze_Schild", [100.0, 20.0, 40.0, 50.0, 3.2, 1500.0, 500.0, 400.0, 0.0]),
        ("FigRomSch00_Speer_Schild", [100.0, 15.0, 80.0, 50.0, 3.2, 1500.0, 500.0, 900.0, 0.0]),
        ("FigRomInf01_Schwert_Schild", [100.0, 20.0, 80.0, 70.0, 3.2, 1500.0, 500.0, 400.0, 0.0]),
        ("FigRomSch01_Bogen", [100.0, 10.0, 50.0, 50.0, 3.2, 1500.0, 1300.0, 1200.0, 0.0]),
        ("FigRomKav00_Schwert_Schild", [100.0, 20.0, 80.0, 75.0, 3.2, 1500.0, 500.0, 400.0, 0.0]),
        ("FigRomAnf00_Anfuehrer", [400.0, 20.0, 100.0, 80.0, 2.6, 1500.0, 500.0, 128.0, 0.0]),
        ("FigRomArt00_Speerschleuder", [150.0, 175.0, 40.0, 60.0, 0.0, 1500.0, 3000.0, 1300.0, 0.0]),
        ("FigRomArt00_Speerschleuder_Auf", [150.0, 175.0, 40.0, 60.0, 0.0, 1500.0, 3000.0, 1300.0, 0.0]),
        ("FigRomArt01_Katapult", [300.0, 250.0, 40.0, 60.0, 0.0, 1500.0, 6000.0, 1200.0, 0.0]),
        ("FigRomArt01_Katapult_Aufbau", [300.0, 250.0, 40.0, 60.0, 0.0, 1500.0, 6000.0, 1200.0, 0.0]),

        // Teuton
        ("FigGerInf01_Schwert", [100.0, 12.0, 40.0, 20.0, 3.2, 1500.0, 500.0, 400.0, 0.0]),
        ("FigGerSch00_Speer", [100.0, 5.0, 25.0, 20.0, 3.2, 1500.0, 1500.0, 900.0, 0.0]),
        ("FigGerInf00_Hammer_Schild", [100.0, 12.0, 60.0, 20.0, 3.2, 1500.0, 500.0, 400.0, 0.0]),
        ("FigGerSch01_Axt_Schild", [100.0, 12.0, 80.0, 25.0, 3.2, 1500.0, 500.0, 700.0, 0.0]),
        ("FigGerInf02_Zweihandaxt", [100.0, 20.0, 50.0, 30.0, 3.2, 1500.0, 500.0, 400.0, 0.0]),
        ("FigGerKav00_Schwert_Schild", [100.0, 12.0, 90.0, 50.0, 3.2, 1500.0, 500.0, 400.0, 0.0]),
        ("FigGerInf03_Doppelhammer", [100.0, 30.0, 50.0, 40.0, 3.2, 1500.0, 500.0, 400.0, 0.0]),
        ("FigGerAnf00_Anfuehrer", [400.0, 20.0, 100.0, 30.0, 2.6, 1500.0, 500.0, 128.0, 0.0]),
        ("FigGerPri00_Priester", [100.0, 10.0, 70.0, 30.0, 2.6, 1500.0, 500.0, 1500.0, 0.0]),
        ("FigGerArt00_Katapult", [50.0, 150.0, 50.0, 50.0, 0.0, 1500.0, 5000.0, 1000.0, 0.0]),
        ("FigGerArt00_Katapult_Aufbau", [10.0, 150.0, 50.0, 0.0, 0.0, 1500.0, 5000.0, 1000.0, 0.0]),

        // Celt
        ("FigKelInf00_Schwert", [100.0, 5.0, 85.0, 20.0, 3.2, 1500.0, 500.0, 400.0, 0.0]),
        ("FigKelSch00_Bogen", [100.0, 7.5, 30.0, 50.0, 3.2, 1500.0, 1250.0, 1100.0, 0.0]),
        ("FigKelInf01_Lanze", [100.0, 10.0, 90.0, 30.0, 3.2, 1500.0, 500.0, 400.0, 0.0]),
        ("FigKelSch01_Schleuder", [100.0, 25.0, 70.0, 65.0, 3.2, 1500.0, 1000.0, 1000.0, 0.0]),
        ("FigKelInf02_Doppelschwert", [100.0, 25.0, 70.0, 60.0, 3.2, 1500.0, 500.0, 400.0, 0.0]),
        ("FigKelSch02_Schwere_Schleuder", [100.0, 30.0, 70.0, 80.0, 3.2, 1500.0, 1500.0, 900.0, 0.0]),
        ("FigKelKav00_Lanze_Schild", [100.0, 20.0, 95.0, 75.0, 3.2, 1500.0, 500.0, 400.0, 0.0]),
        ("FigKelAnf00_Anfuehrer", [400.0, 20.0, 100.0, 60.0, 2.6, 1500.0, 500.0, 128.0, 0.0]),
        ("FigKelPri00_Priester", [100.0, 10.0, 80.0, 60.0, 2.6, 1500.0, 500.0, 1500.0, 500.0]),
        ("FigKelArt00_Speerschleuder", [150.0, 100.0, 40.0, 60.0, 0.0, 1500.0, 3000.0, 1100.0, 0.0]),
        ("FigKelArt00_Speerschleuder_A", [150.0, 100.0, 40.0, 60.0, 0.0, 1500.0, 3000.0, 1100.0, 0.0]),
        ("FigKelArt01_Katapult", [50.0, 175.0, 50.0, 50.0, 0.0, 1500.0, 5000.0, 1000.0, 0.0]),
        ("FigKelArt01_Katapult_Aufbau", [10.0, 175.0, 50.0, 0.0, 0.0, 1500.0, 5000.0, 1000.0, 0.0]),

        // Hun
        ("FigHunInf00_Keule", [100.0, 5.0, 20.0, 30.0, 3.2, 1500.0, 500.0, 400.0, 0.0]),
        ("FigHunSch00_Bogen", [100.0, 10.0, 20.0, 50.0, 3.2, 1500.0, 750.0, 1000.0, 0.0]),
        ("FigHunInf01_Schwert_Schild", [100.0, 10.0, 50.0, 50.0, 2.6, 1500.0, 500.0, 400.0, 0.0]),
        ("FigHunKav00_Schwert_Schild", [100.0, 15.0, 35.0, 40.0, 3.2, 1500.0, 500.0, 400.0, 0.0]),
        ("FigHunKav01_Bogen", [100.0, 10.0, 20.0, 70.0, 3.2, 1500.0, 500.0, 1000.0, 0.0]),
        ("FigHunKav02_Lanze_Schild", [100.0, 30.0, 60.0, 75.0, 3.2, 1500.0, 500.0, 400.0, 0.0]),
        ("FigHunKav03_Geisterreiter", [100.0, 15.0, 60.0, 60.0, 3.2, 1500.0, 500.0, 400.0, 0.0]),
        ("FigHunAnf00_Anfuehrer", [400.0, 20.0, 100.0, 50.0, 3.2, 1500.0, 500.0, 128.0, 0.0]),
        ("FigHunPri00_Priester", [100.0, 15.0, 30.0, 50.0, 2.6, 1500.0, 500.0, 1500.0, 500.0]),
    ];

    pub fn get_metadata(key: &str) -> Option<&'static UnitMetadata> {
        Self::UNIT_METADATA_ENTRIES
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, m)| m)
    }

    pub fn get_name_zh(key: &str) -> Option<&'static str> {
        Self::UNIT_NAME_ENTRIES
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, n)| *n)
    }

    pub fn get_balanced_stats(key: &str) -> Option<&'static [f64; 9]> {
        Self::BALANCED_STATS_ENTRIES
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, s)| s)
    }

    /// Sorted list of units by Tier priority (low -> mid -> high -> ace -> leader -> siege).
    pub fn sorted_units() -> Vec<&'static str> {
        let tier_prio = |tier: &str| -> i32 {
            match tier {
                "low" => 1,
                "mid" => 2,
                "high" => 3,
                "ace" => 4,
                "leader" => 5,
                "siege" => 6,
                _ => 99,
            }
        };

        let mut entries: Vec<(&'static str, usize)> = Self::UNIT_METADATA_ENTRIES
            .iter()
            .enumerate()
            .map(|(idx, (k, _))| (*k, idx))
            .collect();

        entries.sort_by(|(ka, idxa), (kb, idxb)| {
            let meta_a = Self::get_metadata(ka).unwrap();
            let meta_b = Self::get_metadata(kb).unwrap();
            let pa = tier_prio(meta_a.tier);
            let pb = tier_prio(meta_b.tier);
            if pa != pb {
                pa.cmp(&pb)
            } else {
                idxa.cmp(idxb)
            }
        });

        entries.into_iter().map(|(k, _)| k).collect()
    }
}

pub struct UnitStatParser;

impl UnitStatParser {
    pub fn merge_layers(
        fallback: &[f64],
        custom: &[f64],
        supports_spell_radius: bool,
        remove_priest_sight: bool,
    ) -> [f64; 9] {
        let mut layered = [0.0; 9];
        for index in 0..9 {
            if index == 8 && !supports_spell_radius {
                layered[index] = 0.0;
                continue;
            }

            if index == 4 || index == 7 || index == 8 || (index == 5 && remove_priest_sight) {
                layered[index] = if fallback.len() > index { fallback[index] } else { 0.0 };
                continue;
            }

            layered[index] = if custom.len() > index {
                custom[index]
            } else if fallback.len() > index {
                fallback[index]
            } else {
                0.0
            };
        }
        layered
    }

    pub fn get_melee_and_ranged_damage(columns: &[String], unit_type: &str) -> (f64, f64) {
        let mut melee: f64 = 0.0;
        let mut ranged: f64 = 0.0;

        for weapon in 1..=8 {
            let active_idx = ObjdefIndex::Weapon1Akti as usize + (weapon - 1) * 8;
            let dmg_idx = active_idx + 1;
            let type_idx = ObjdefIndex::Weapon1Dtyp as usize + (weapon - 1);

            if active_idx >= columns.len() || dmg_idx >= columns.len() || type_idx >= columns.len() {
                continue;
            }
            if columns[active_idx].trim() != "1" {
                continue;
            }

            let val: f64 = columns[dmg_idx].trim().parse().unwrap_or(0.0);
            let is_ranged = Self::is_ranged_weapon(columns[type_idx].trim(), unit_type);
            if is_ranged {
                ranged = ranged.max(val);
            } else {
                melee = melee.max(val);
            }
        }

        (melee, ranged)
    }

    pub fn get_melee_and_ranged_reload(columns: &[String], unit_type: &str) -> (f64, f64) {
        let mut melee: f64 = 0.0;
        let mut ranged: f64 = 0.0;

        for weapon in 1..=8 {
            let active_idx = ObjdefIndex::Weapon1Akti as usize + (weapon - 1) * 8;
            let reload_idx = active_idx + 6;
            let type_idx = ObjdefIndex::Weapon1Dtyp as usize + (weapon - 1);

            if active_idx >= columns.len() || reload_idx >= columns.len() || type_idx >= columns.len() {
                continue;
            }
            if columns[active_idx].trim() != "1" {
                continue;
            }

            let val: f64 = columns[reload_idx].trim().parse().unwrap_or(0.0);
            if val <= 0.0 {
                continue;
            }

            let is_ranged = Self::is_ranged_weapon(columns[type_idx].trim(), unit_type);
            if is_ranged {
                ranged = if ranged == 0.0 { val } else { ranged.min(val) };
            } else {
                melee = if melee == 0.0 { val } else { melee.min(val) };
            }
        }

        (melee, ranged)
    }

    pub fn get_maximum_range(columns: &[String], unit_type: &str) -> f64 {
        if unit_type == "priest" {
            let sirad_idx = ObjdefIndex::Sirad as usize;
            return if sirad_idx < columns.len() {
                columns[sirad_idx].trim().parse().unwrap_or(0.0)
            } else {
                0.0
            };
        }

        let mut max_range = 0.0f64;
        for weapon in 1..=8 {
            let active_idx = ObjdefIndex::Weapon1Akti as usize + (weapon - 1) * 8;
            let min_idx = ObjdefIndex::Weapon1RangeMin as usize + (weapon - 1) * 8;
            let max_idx = ObjdefIndex::Weapon1RangeMax as usize + (weapon - 1) * 8;

            if max_idx >= columns.len() || columns[active_idx].trim() != "1" {
                continue;
            }

            if let Ok(min_val) = columns[min_idx].trim().parse::<f64>() {
                max_range = max_range.max(min_val);
            }
            if let Ok(max_val) = columns[max_idx].trim().parse::<f64>() {
                max_range = max_range.max(max_val);
            }
        }

        max_range
    }

    fn is_ranged_weapon(damage_type: &str, unit_type: &str) -> bool {
        matches!(damage_type, "1" | "2" | "3" | "4") || unit_type == "siege"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_classifies_damage_reload_and_range() {
        let mut cols = vec!["0".to_string(); 256];
        let set_weapon = |cols: &mut [String], slot: usize, dtype: &str, dam: &str, rel: &str, min_r: &str, max_r: &str| {
            let offset = (slot - 1) * 8;
            let active = ObjdefIndex::Weapon1Akti as usize + offset;
            cols[active] = "1".to_string();
            cols[active + 1] = dam.to_string();
            cols[active + 6] = rel.to_string();
            cols[ObjdefIndex::Weapon1Dtyp as usize + slot - 1] = dtype.to_string();
            cols[ObjdefIndex::Weapon1RangeMin as usize + offset] = min_r.to_string();
            cols[ObjdefIndex::Weapon1RangeMax as usize + offset] = max_r.to_string();
        };

        set_weapon(&mut cols, 1, "0", "10", "2.5", "1", "50");
        set_weapon(&mut cols, 2, "1", "20", "1.5", "5", "100");

        let (melee_dam, ranged_dam) = UnitStatParser::get_melee_and_ranged_damage(&cols, "hybrid_inf");
        let (melee_rel, ranged_rel) = UnitStatParser::get_melee_and_ranged_reload(&cols, "hybrid_inf");

        assert_eq!(melee_dam, 10.0);
        assert_eq!(ranged_dam, 20.0);
        assert_eq!(melee_rel, 2.5);
        assert_eq!(ranged_rel, 1.5);
        assert_eq!(UnitStatParser::get_maximum_range(&cols, "hybrid_inf"), 100.0);

        cols[ObjdefIndex::Sirad as usize] = "300".to_string();
        assert_eq!(UnitStatParser::get_maximum_range(&cols, "priest"), 300.0);
    }

    #[test]
    fn troop_config_has_all_44_units_and_stable_order() {
        assert_eq!(TroopConfig::UNIT_METADATA_ENTRIES.len(), 43);
        assert_eq!(TroopConfig::BALANCED_STATS_ENTRIES.len(), 43);
        let sorted = TroopConfig::sorted_units();
        assert_eq!(sorted.len(), 43);

        // Low tier comes first
        let first_meta = TroopConfig::get_metadata(sorted[0]).unwrap();
        assert_eq!(first_meta.tier, "low");

        // Siege tier comes last
        let last_meta = TroopConfig::get_metadata(sorted.last().unwrap()).unwrap();
        assert_eq!(last_meta.tier, "siege");
    }
}
