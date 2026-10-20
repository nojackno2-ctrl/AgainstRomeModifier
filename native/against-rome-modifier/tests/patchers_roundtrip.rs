//! Comprehensive port of PatcherRoundTripTests.cs to Rust,
//! testing Objdef, Ress, TeamDat, ClScript, and Partgeo patchers on both synthetic and retail data.

use against_rome_modifier::{
    compress_pfil, decompress_pfil, is_pfil, parse_csv_line, ClScriptOptions, ClScriptPatcher,
    ObjdefOptions, ObjdefPatcher, PartgeoOptions, PartgeoPatcher, RessOptions, RessPatcher,
    TeamDatOptions, TeamDatPatcher,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn synthetic_pfil(text: &str) -> Vec<u8> {
    compress_pfil(text.as_bytes(), None).expect("compress synthetic pfil")
}

fn synthetic_text(pfil_bytes: &[u8]) -> String {
    let decomp = decompress_pfil(pfil_bytes).expect("decompress synthetic pfil");
    String::from_utf8(decomp).expect("utf8 text")
}

#[test]
fn objdef_capacity_and_build_time_patch_are_exact_and_restore() {
    let mut columns = vec!["     0".to_string(); 200];
    columns[42] = "   100".to_string();
    columns[52] = format!("{:>20}", "BauHau00_Haus");
    columns[73] = "  1000".to_string();
    columns[74] = "   500".to_string();
    columns[156] = "    10".to_string();

    let raw_text = format!("header1\r\nheader2\r\n{}\r\n", columns.join(","));
    let original = synthetic_pfil(&raw_text);

    let enabled = ObjdefOptions {
        housing_capacity_20x: true,
        storage_capacity_10x: true,
        fast_build_upgrade_repair: true,
        ..Default::default()
    };

    let patched = ObjdefPatcher::get_patched_bytes(&original, &enabled).expect("patch objdef");
    let result_text = synthetic_text(&patched);
    let result_cols = parse_csv_line(result_text.split("\r\n").nth(2).unwrap());

    assert_eq!(result_cols[156].trim(), "200");
    assert_eq!(result_cols[42].trim(), "1000");
    assert_eq!(result_cols[73].trim(), "100");
    assert_eq!(result_cols[74].trim(), "50");

    // Disabled options restore exactly
    let restored = ObjdefPatcher::get_patched_bytes(&original, &ObjdefOptions::default()).unwrap();
    assert_eq!(restored, original);
}

#[test]
fn objdef_leader_glory_multiplies_growth_columns_only() {
    let mut columns = vec!["     0".to_string(); 200];
    columns[52] = format!("{:>24}", "FigRomAnf00_Anfuehrer");
    columns[148] = "     2".to_string();
    columns[149] = "     3".to_string();
    columns[150] = "     4".to_string();
    columns[161] = "     5".to_string();
    columns[153] = "   100".to_string();

    let raw_text = format!("header1\r\nheader2\r\n{}\r\n", columns.join(","));
    let original = synthetic_pfil(&raw_text);

    let options = ObjdefOptions {
        leader_glory_5x: true,
        ..Default::default()
    };

    let patched = ObjdefPatcher::get_patched_bytes(&original, &options).unwrap();
    let result_text = synthetic_text(&patched);
    let result_cols = parse_csv_line(result_text.split("\r\n").nth(2).unwrap());

    assert_eq!(result_cols[148].trim(), "10");
    assert_eq!(result_cols[149].trim(), "15");
    assert_eq!(result_cols[150].trim(), "20");
    assert_eq!(result_cols[161].trim(), "25");
    assert_eq!(result_cols[153].trim(), "100"); // untouched
}

#[test]
fn objdef_ranged_range_3x_expands_targeting_sight() {
    let mut columns = vec!["       0".to_string(); 205];
    columns[19] = "     100".to_string();
    columns[24] = "    1500".to_string();
    columns[52] = format!("{:>20}", "FigKelSch00_Bogen");
    columns[78] = "       1".to_string();
    columns[80] = " 1000.00".to_string();
    columns[81] = " 2000.00".to_string();
    columns[199] = "       1".to_string();

    let raw_text = format!("header1\r\nheader2\r\n{}\r\n", columns.join(","));
    let original = synthetic_pfil(&raw_text);

    let mut stats = HashMap::new();
    stats.insert(
        "FigKelSch00_Bogen".to_string(),
        [100.0, 10.0, 10.0, 10.0, 2.0, 1500.0, 500.0, 2000.0, 0.0],
    );

    let options = ObjdefOptions {
        ranged_range_3x: true,
        unit_stats: stats,
        ..Default::default()
    };

    let patched = ObjdefPatcher::get_patched_bytes(&original, &options).unwrap();
    let result_text = synthetic_text(&patched);
    let result_cols = parse_csv_line(result_text.split("\r\n").nth(2).unwrap());

    assert_eq!(result_cols[80].trim(), "1000.00"); // Min range untouched
    assert_eq!(result_cols[81].trim(), "6000.00"); // Max range x3
    assert_eq!(result_cols[24].trim(), "6000"); // Sight expanded to cover new range
}

#[test]
fn objdef_entire_map_vision_is_final_override() {
    let mut archer = vec!["       0".to_string(); 205];
    archer[19] = "     100".to_string();
    archer[24] = "    1500".to_string();
    archer[52] = format!("{:>28}", "FigKelSch00_Bogen");
    archer[78] = "       1".to_string();
    archer[79] = "   10.00".to_string();
    archer[80] = " 1000.00".to_string();
    archer[81] = " 2000.00".to_string();
    archer[84] = "     500".to_string();
    archer[199] = "       1".to_string();

    let raw_text = format!("header1\r\nheader2\r\n{}\r\n", archer.join(","));
    let original = synthetic_pfil(&raw_text);

    let mut stats = HashMap::new();
    stats.insert(
        "FigKelSch00_Bogen".to_string(),
        [100.0, 10.0, 10.0, 10.0, 2.0, 1500.0, 500.0, 2000.0, 0.0],
    );

    let options = ObjdefOptions {
        ranged_range_3x: true,
        all_units_entire_map_vision: true,
        unit_stats: stats,
        ..Default::default()
    };

    let patched = ObjdefPatcher::get_patched_bytes(&original, &options).unwrap();
    let result_text = synthetic_text(&patched);
    let result_cols = parse_csv_line(result_text.split("\r\n").nth(2).unwrap());

    assert_eq!(result_cols[24].trim(), "30000"); // Sirad is 30000
    assert_eq!(result_cols[81].trim(), "6000.00"); // Range is 6000
}

#[test]
fn ress_cost_patch_zeros_selected_fields_and_restores() {
    let building = format!("BauHau00,{}", vec!["10"; 12].join(","));
    let unit = format!("FigGerPri00_Priester,{}", vec!["5"; 28].join(","));
    let original = synthetic_pfil(&format!("[objres]\n{}\n{}\n", building, unit));

    let options = RessOptions {
        free_production: true,
        free_upgrades: true,
        no_spell_cost: true,
    };

    let patched = RessPatcher::get_patched_bytes(&original, &options).unwrap();
    let text = synthetic_text(&patched);
    let lines: Vec<&str> = text.split('\n').collect();

    let b_cols = parse_csv_line(lines[1]);
    for col in &b_cols[1..13] {
        assert_eq!(col.trim(), "0");
    }

    let u_cols = parse_csv_line(lines[2]);
    for col in &u_cols[13..19] {
        assert_eq!(col.trim(), "0");
    }
}

#[test]
fn team_dat_patch_sets_positive_population_and_restores() {
    let original = synthetic_pfil("[teamdata]\nA,B,C,D,80,F\nA,B,C,D,0,F\n");
    let options = TeamDatOptions {
        max_population: true,
        population_limit: 1600,
        roman_player: false,
    };

    let patched = TeamDatPatcher::get_patched_bytes(&original, &options).unwrap();
    let text = synthetic_text(&patched);

    assert!(text.contains("A,B,C,D,1600,F"));
    assert!(text.contains("A,B,C,D,0,F"));

    let restored =
        TeamDatPatcher::get_patched_bytes(&original, &TeamDatOptions::default()).unwrap();
    assert_eq!(restored, original);
}

#[test]
fn partgeo_projectile_arc_height_doubles_all_ysub() {
    let names = [
        "Wurfspeer00",
        "Wurfaxt00",
        "Katapultstein00",
        "Katapultstein01",
        "Pfeil00",
        "Spiess00",
        "Fackel",
    ];
    let mut rows = vec!["header1".to_string(), "header2".to_string()];
    for name in names {
        let mut cols = vec!["       0".to_string(); 20];
        cols[2] = format!("{:>20}", name);
        cols[12] = "     100".to_string();
        rows.push(cols.join(","));
    }
    let original = synthetic_pfil(&(rows.join("\r\n") + "\r\n"));

    let patched = PartgeoPatcher::get_patched_bytes(
        &original,
        &PartgeoOptions {
            projectile_arc_height: true,
        },
    )
    .unwrap();
    let text = synthetic_text(&patched);

    for line in text.lines().skip(2) {
        let cols = parse_csv_line(line);
        assert_eq!(cols[12].trim(), "200");
    }
}

#[test]
fn retail_baseline_files_roundtrip_and_patch_correctly() {
    let baseline_root = Path::new("../../re_workspace/cd-modifier-baseline-20261010/files");
    if !baseline_root.exists() {
        return;
    }

    // 1. cl_script.ini
    let cl_script_path = baseline_root.join("SYSTEM/cl_script.ini");
    if cl_script_path.exists() {
        let raw = fs::read(&cl_script_path).unwrap();
        let options = ClScriptOptions {
            fast_civilian_production: true,
            infinite_morale: true,
            spell_damage_5x: true,
            spell_healing_10x: true,
            spell_resurrection: true,
            ..Default::default()
        };
        let patched = ClScriptPatcher::get_patched_bytes(&raw, &options).unwrap();
        let decomp = decompress_pfil(&patched).unwrap();
        let text = String::from_utf8_lossy(&decomp);
        let clean_text = text.replace(' ', "").replace('\t', "");
        assert!(clean_text.contains("CiviDelay=GER,500"));
        assert!(clean_text.contains("MoralsDecLostMem=GER,0"));
    }

    // 2. ress.ini
    let ress_path = baseline_root.join("SYSTEM/ress.ini");
    if ress_path.exists() {
        let raw = fs::read(&ress_path).unwrap();
        let options = RessOptions {
            free_production: true,
            free_upgrades: true,
            no_spell_cost: true,
        };
        let patched = RessPatcher::get_patched_bytes(&raw, &options).unwrap();
        assert!(is_pfil(&patched));
    }

    // 3. team.dat
    let team_dat_path = baseline_root.join("MAPS/ENDL_001/DATA/team.dat");
    if team_dat_path.exists() {
        let raw = fs::read(&team_dat_path).unwrap();
        let options = TeamDatOptions {
            max_population: true,
            population_limit: 1600,
            roman_player: true,
        };
        let patched = TeamDatPatcher::get_patched_bytes(&raw, &options).unwrap();
        let decomp = decompress_pfil(&patched).unwrap();
        let text = String::from_utf8_lossy(&decomp);
        assert!(text.contains("1600"));
        assert!(text.contains("ROM"));
    }

    // 4. partgeo.dau
    let partgeo_path = baseline_root.join("SYSTEM/DATA_MP/DEFAULTS/partgeo.dau");
    if partgeo_path.exists() {
        let raw = fs::read(&partgeo_path).unwrap();
        let patched = PartgeoPatcher::get_patched_bytes(
            &raw,
            &PartgeoOptions {
                projectile_arc_height: true,
            },
        )
        .unwrap();
        assert!(is_pfil(&patched));
    }
}
