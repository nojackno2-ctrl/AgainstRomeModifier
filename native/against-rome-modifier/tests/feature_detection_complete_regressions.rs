//! 完整偵測回歸：獨立破壞必要欄位、列、陣營與相依檔案，不能靠單一旗標證明全套。
use against_rome_modifier::{
    detectors::{
        ClScriptFeatureDetector, FeatureDetector, ObjdefFeatureDetector, RessFeatureDetector,
        TeamDatFeatureDetector,
    },
    epara::EparaPatcher,
    objdef::{ObjdefOptions, ObjdefPatcher},
    partgeo::{PartgeoOptions, PartgeoPatcher, PROJECTILE_GEO_NAMES},
    patch_text::{parse_csv_line, TextDocument},
    services::BackupManager,
    troops::TroopConfig,
    PatchProfile, RessOptions, RessPatcher,
};
use std::{
    fs,
    path::PathBuf,

};
const OBJ: &str = "SYSTEM/DATA_MP/DEFAULTS/objdef.dau";
const GEO: &str = "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau";
const EPA: &str = "SYSTEM/cl_epara.ini";

struct Fixture {
    dir: PathBuf,
    backup: BackupManager,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
impl Fixture {
    fn new() -> Self {
        let dir = crate::temp_fixture::dir("arm-complete");
        fs::create_dir_all(&dir).unwrap();
        let mut f = Self {
            dir,
            backup: BackupManager::new(),
        };
        let mut names: Vec<_> = TroopConfig::sorted_units()
            .into_iter()
            .map(str::to_owned)
            .collect();
        names.extend(
            [
                "FigZivMan00_Zivilist",
                "FigZivWei00_Zivilistin",
                "FigTiePac00_Packpferd",
                "BauRomHau00",
                "BauGerLag00",
            ]
            .map(str::to_owned),
        );
        let mut lines = vec!["header1".to_owned(), "header2".to_owned()];
        for name in names {
            let mut c = vec![format!("{:>12}", 0); 220];
            c[52] = format!("{name:>40}");
            for (i, v) in [
                (4, 10),
                (23, 20),
                (191, 30),
                (19, 100),
                (24, 1000),
                (42, 3),
                (73, 100),
                (74, 200),
                (142, 10),
                (146, 20),
                (156, 2),
                (148, 10),
                (149, 20),
                (150, 30),
                (161, 40),
                (208, 1),
            ] {
                c[i] = format!("{v:>12}");
            }
            for w in 0..8 {
                for (i, v) in [
                    (78 + w * 8, 1),
                    (79 + w * 8, 10),
                    (81 + w * 8, 100),
                    (84 + w * 8, 100),
                    (85 + w * 8, 10),
                    (164 + w * 2, 10),
                    (199 + w, 1),
                ] {
                    c[i] = format!("{v:>12}");
                }
            }
            lines.push(c.join(","));
        }
        f.baseline(OBJ, lines.join("\r\n").as_bytes());
        let mut geo = vec!["header1".to_owned(), "header2".to_owned()];
        for name in PROJECTILE_GEO_NAMES {
            let mut c = vec![format!("{:>12}", 0); 20];
            c[2] = format!("{name:>30}");
            c[12] = format!("{:>12}", 100);
            geo.push(c.join(","));
        }
        f.baseline(GEO, geo.join("\r\n").as_bytes());
        f.baseline(EPA, b"[ProjectileVarianceOnMove]\r\n0.5\r\n[ProjectileInitSpeedFactor]\r\n1.5\r\n[Unrelated]\r\n42\r\n");
        f
    }
    fn write(&self, key: &str, bytes: &[u8]) {
        let path = self.dir.join(key);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    fn baseline(&mut self, key: &str, bytes: &[u8]) {
        self.backup.set_backup_file(key, bytes.to_vec());
        self.write(key, bytes);
    }
    fn options(&self, balance: bool) -> ObjdefOptions {
        let mut o = ObjdefOptions {
            balance,
            ..Default::default()
        };
        for name in TroopConfig::sorted_units() {
            o.unit_stats.insert(
                name.to_owned(),
                if balance {
                    self.backup.get_default_balanced_stats(name)
                } else {
                    self.backup.get_original_stats(name)
                }
                .unwrap(),
            );
        }
        o
    }
    fn apply(&self, o: &ObjdefOptions) {
        self.write(
            OBJ,
            &ObjdefPatcher::get_patched_bytes(self.backup.get_backup_bytes(OBJ).unwrap(), o)
                .unwrap(),
        );
        if o.ranged_range_3x {
            self.write(
                EPA,
                &EparaPatcher::get_patched_bytes(self.backup.get_backup_bytes(EPA).unwrap(), true)
                    .unwrap(),
            );
        }
        if o.projectile_arc_height {
            self.write(
                GEO,
                &PartgeoPatcher::get_patched_bytes(
                    self.backup.get_backup_bytes(GEO).unwrap(),
                    &PartgeoOptions {
                        projectile_arc_height: true,
                    },
                )
                .unwrap(),
            );
        }
    }
    fn detect(&self) -> PatchProfile {
        let mut p = PatchProfile::new();
        ObjdefFeatureDetector::detect(&self.dir, &self.backup, &mut p);
        p
    }
    fn edit(&self, key: &str, name_col: usize, name: &str, col: usize, value: &str) {
        let doc = TextDocument::read(&fs::read(self.dir.join(key)).unwrap()).unwrap();
        let lines: Vec<_> = doc
            .lines
            .iter()
            .map(|l| {
                let mut c = parse_csv_line(l);
                if c.get(name_col).is_some_and(|n| n.trim() == name) {
                    c[col] = format!("{value:>12}");
                    c.join(",")
                } else {
                    l.clone()
                }
            })
            .collect();
        self.write(key, lines.join("\r\n").as_bytes());
    }
    fn remove(&self, key: &str, name_col: usize, name: &str) {
        let doc = TextDocument::read(&fs::read(self.dir.join(key)).unwrap()).unwrap();
        let lines: Vec<_> = doc
            .lines
            .iter()
            .filter(|l| {
                !parse_csv_line(l)
                    .get(name_col)
                    .is_some_and(|n| n.trim() == name)
            })
            .cloned()
            .collect();
        self.write(key, lines.join("\r\n").as_bytes());
    }
}
#[test]
fn speed_checks_every_faction_and_all_three_columns() {
    let f = Fixture::new();
    let mut o = f.options(false);
    o.unit_movement_speed_2x = true;
    for name in TroopConfig::sorted_units() {
        for col in [4, 23, 191] {
            f.apply(&o);
            assert!(f.detect().get_bool("UnitMovementSpeed2x"));
            f.edit(OBJ, 52, name, col, "9");
            assert!(!f.detect().get_bool("UnitMovementSpeed2x"), "{name}:{col}");
        }
    }
}
#[test]
fn villagers_include_woman_and_packhorse_and_missing_rows() {
    let f = Fixture::new();
    let mut o = f.options(false);
    o.villager_movement_speed_5x = true;
    for name in [
        "FigZivMan00_Zivilist",
        "FigZivWei00_Zivilistin",
        "FigTiePac00_Packpferd",
    ] {
        for col in [4, 23, 191] {
            f.apply(&o);
            assert!(f.detect().get_bool("VillagerMovementSpeed5x"));
            f.edit(OBJ, 52, name, col, "9");
            assert!(!f.detect().get_bool("VillagerMovementSpeed5x"));
        }
        f.apply(&o);
        f.remove(OBJ, 52, name);
        assert!(!f.detect().get_bool("VillagerMovementSpeed5x"));
    }
}
#[test]
fn range_checks_every_weapon_sight_and_accuracy_and_both_epara_sections() {
    let f = Fixture::new();
    let mut o = f.options(false);
    o.ranged_range_3x = true;
    for name in ["FigRomSch01_Bogen", "FigGerPri00_Priester"] {
        for col in if name == "FigGerPri00_Priester" {
            vec![164, 178]
        } else {
            vec![81, 137, 164, 178]
        } {
            f.apply(&o);
            assert!(f.detect().get_bool("RangedRange3x"));
            f.edit(OBJ, 52, name, col, "9");
            assert!(!f.detect().get_bool("RangedRange3x"), "{name}:{col}");
        }
    }
    f.apply(&o);
    f.edit(OBJ, 52, "FigRomSch01_Bogen", 24, "9");
    assert!(!f.detect().get_bool("RangedRange3x"));
    for text in [
        "[ProjectileVarianceOnMove]\n0.5\n[ProjectileInitSpeedFactor]\n1.62",
        "[ProjectileVarianceOnMove]\n0\n[ProjectileInitSpeedFactor]\n1.5",
        "[ProjectileVarianceOnMove]\n0",
        "PFIL",
    ] {
        f.apply(&o);
        f.write(EPA, text.as_bytes());
        assert!(!f.detect().get_bool("RangedRange3x"));
    }
    f.apply(&o);
    fs::remove_file(f.dir.join(EPA)).unwrap();
    assert!(!f.detect().get_bool("RangedRange3x"));
}
#[test]
fn arc_requires_both_files_every_geometry_and_every_emit() {
    let f = Fixture::new();
    let mut o = f.options(false);
    o.projectile_arc_height = true;
    for name in PROJECTILE_GEO_NAMES {
        f.apply(&o);
        assert!(f.detect().get_bool("ProjectileArcHeight"));
        f.edit(GEO, 2, name, 12, "100");
        assert!(!f.detect().get_bool("ProjectileArcHeight"));
        f.apply(&o);
        f.remove(GEO, 2, name);
        assert!(!f.detect().get_bool("ProjectileArcHeight"));
    }
    for col in [85, 141] {
        f.apply(&o);
        f.edit(OBJ, 52, "FigRomSch01_Bogen", col, "garbage");
        assert!(!f.detect().get_bool("ProjectileArcHeight"));
    }
    f.apply(&o);
    fs::remove_file(f.dir.join(GEO)).unwrap();
    assert!(!f.detect().get_bool("ProjectileArcHeight"));
}
#[test]
fn balance_requires_complete_preset_and_accepts_known_overlap_combinations() {
    let f = Fixture::new();
    for bits in 0..8 {
        let mut o = f.options(true);
        o.ranged_range_3x = bits & 1 != 0;
        o.spell_entire_map = bits & 2 != 0;
        o.all_units_entire_map_vision = bits & 4 != 0;
        f.apply(&o);
        let p = f.detect();
        assert!(p.get_bool("Balance"), "bits={bits}");
        if o.ranged_range_3x {
            assert!(p.get_bool("RangedRange3x"));
        }
        if o.spell_entire_map && !o.all_units_entire_map_vision {
            assert!(p.get_bool("SpellEntireMap"));
        }
        if o.all_units_entire_map_vision {
            assert!(!p.get_bool("SpellEntireMap"));
        }
        if o.all_units_entire_map_vision {
            assert!(p.get_bool("AllUnitsEntireMapVision"));
        }
    }
    for (name, col) in [
        ("FigRomInf00_Lanze_Schild", 19),
        ("FigHunPri00_Priester", 146),
        ("FigKelPri00_Priester", 142),
        ("FigRomSch01_Bogen", 79),
        ("FigRomSch01_Bogen", 84),
        ("FigRomSch01_Bogen", 81),
        ("FigRomSch01_Bogen", 24),
    ] {
        f.apply(&f.options(true));
        f.edit(OBJ, 52, name, col, "12345");
        assert!(!f.detect().get_bool("Balance"), "{name}:{col}");
    }
    f.apply(&f.options(true));
    f.remove(OBJ, 52, "FigHunPri00_Priester");
    assert!(!f.detect().get_bool("Balance"));
}
#[test]
fn custom_stats_do_not_masquerade_as_balance_or_survive_detection() {
    let f = Fixture::new();
    f.edit(OBJ, 52, "FigRomInf00_Lanze_Schild", 19, "123");
    let mut p = PatchProfile::new();
    p.set_bool("Balance", true);
    p.set_string("CustomUnitStats", "stale");
    p.custom_unit_stats = Some(Default::default());
    ObjdefFeatureDetector::detect(&f.dir, &f.backup, &mut p);
    assert!(!p.get_bool("Balance"));
    assert!(p.custom_unit_stats.is_none());
    assert_eq!(
        p.get("CustomUnitStats"),
        against_rome_modifier::FeatureValue::String(String::new())
    );
    fs::remove_file(f.dir.join(OBJ)).unwrap();
    p.set_bool("Balance", true);
    ObjdefFeatureDetector::detect(&f.dir, &f.backup, &mut p);
    assert!(!p.get_bool("Balance"));
}
#[test]
fn unrelated_fields_are_tolerated_and_independent_speed_survives_custom_hp() {
    let f = Fixture::new();
    let mut o = f.options(true);
    o.unit_movement_speed_2x = true;
    o.ranged_range_3x = true;
    o.projectile_arc_height = true;
    f.apply(&o);
    f.edit(OBJ, 52, "FigRomInf00_Lanze_Schild", 12, "777");
    f.edit(GEO, 2, "Pfeil00", 4, "777");
    let p = f.detect();
    for id in [
        "Balance",
        "UnitMovementSpeed2x",
        "RangedRange3x",
        "ProjectileArcHeight",
    ] {
        assert!(p.get_bool(id), "{id}");
    }
    f.edit(OBJ, 52, "FigRomInf00_Lanze_Schild", 19, "123");
    let p = f.detect();
    assert!(!p.get_bool("Balance"));
    assert!(p.get_bool("UnitMovementSpeed2x"));
}
#[test]
fn other_previous_representative_checks_cover_all_supported_rows() {
    let f = Fixture::new();
    let mut o = f.options(false);
    o.no_run_hp_loss = true;
    o.leader_glory_5x = true;
    o.spell_entire_map = true;
    f.apply(&o);
    for id in ["NoRunHpLoss", "LeaderGlory", "SpellEntireMap"] {
        assert!(f.detect().get_bool(id), "{id}");
    }
    f.edit(OBJ, 52, "FigHunAnf00_Anfuehrer", 161, "1");
    assert!(!f.detect().get_bool("LeaderGlory"));
    f.edit(OBJ, 52, "FigTiePac00_Packpferd", 208, "1");
    assert!(!f.detect().get_bool("NoRunHpLoss"));
    f.edit(OBJ, 52, "FigHunPri00_Priester", 24, "1000");
    assert!(!f.detect().get_bool("SpellEntireMap"));
}
fn resource_text() -> String {
    let mut lines = vec!["[objres]".to_owned()];
    for name in [
        "BauRomHau00",
        "BauGerLag00",
        "FigRomInf00_Lanze_Schild",
        "FigGerArt00",
        "FigKelDru00",
        "FigHunPri00_Priester",
    ] {
        lines.push(format!("{name},{}", vec!["7"; 28].join(",")));
    }
    lines.push("[volkres]".into());
    for faction in ["Rom_Kampf", "Ger_Kampf", "Kel_Kampf", "Hun_Kampf"] {
        lines.push(format!("{faction},{}", vec!["7"; 299].join(",")));
    }
    lines.join("\n")
}
#[test]
fn costs_validate_every_building_unit_siege_priest_tech_and_faction() {
    let f = Fixture::new();
    let orig = resource_text();
    let bytes = RessPatcher::get_patched_bytes(
        orig.as_bytes(),
        &RessOptions {
            free_production: true,
            free_upgrades: true,
            no_spell_cost: true,
        },
    )
    .unwrap();
    for (name, col, id) in [
        ("BauRomHau00", 1, "FreeProduction"),
        ("BauGerLag00", 12, "FreeUpgrade"),
        ("FigGerArt00", 6, "FreeProduction"),
        ("FigRomInf00_Lanze_Schild", 18, "FreeProduction"),
        ("FigKelDru00", 28, "NoSpellCost"),
        ("FigHunPri00_Priester", 25, "NoSpellCost"),
        ("Rom_Kampf", 24, "FreeUpgrade"),
        ("Kel_Kampf", 8, "FreeUpgrade"),
        ("Hun_Kampf", 295, "FreeUpgrade"),
    ] {
        f.write("SYSTEM/ress.ini", &bytes);
        let mut p = PatchProfile::new();
        RessFeatureDetector::detect(&f.dir, &mut p);
        for flag in ["FreeProduction", "FreeUpgrade", "NoSpellCost"] {
            assert!(p.get_bool(flag), "{flag}");
        }
        f.edit("SYSTEM/ress.ini", 0, name, col, "7");
        RessFeatureDetector::detect(&f.dir, &mut p);
        assert!(!p.get_bool(id), "{name}:{col}");
    }
}
#[test]
fn missing_resource_rows_are_rejected_against_backup() {
    let mut f = Fixture::new();
    let orig = resource_text();
    f.baseline("SYSTEM/ress.ini", orig.as_bytes());
    let bytes = RessPatcher::get_patched_bytes(
        orig.as_bytes(),
        &RessOptions {
            free_production: true,
            free_upgrades: true,
            no_spell_cost: true,
        },
    )
    .unwrap();
    f.write("SYSTEM/ress.ini", &bytes);
    f.remove("SYSTEM/ress.ini", 0, "FigHunPri00_Priester");
    let p = FeatureDetector::detect(&f.dir, &f.backup);
    assert!(!p.get_bool("NoSpellCost"));
    assert!(!p.get_bool("FreeProduction"));
}
#[test]
fn population_requires_all_maps_and_teams_but_tolerates_unrelated_changes() {
    let mut f = Fixture::new();
    for name in ["ENDL_001", "ENDL_002", "KAMP_001"] {
        f.baseline(
            &format!("MAPS/{name}/team.dat"),
            b"[teamdata]\n0,GER,A,B,80,flag\n1,HUN,A,B,90,flag\n",
        );
        f.write(
            &format!("MAPS/{name}/team.dat"),
            b"[teamdata]\n0,ROM,Z,B,1600,newflag\n1,HUN,A,B,1600,flag\n",
        );
    }
    assert_eq!(
        TeamDatFeatureDetector::detect(&f.dir, &f.backup),
        (true, true)
    );
    f.write(
        "MAPS/KAMP_001/team.dat",
        b"[teamdata]\n0,ROM,Z,B,1600,newflag\n1,HUN,A,B,90,flag\n",
    );
    assert_eq!(
        TeamDatFeatureDetector::detect(&f.dir, &f.backup),
        (false, true)
    );
    f.write(
        "MAPS/ENDL_002/team.dat",
        b"[teamdata]\n0,GER,A,B,1600,flag\n1,HUN,A,B,1600,flag\n",
    );
    assert_eq!(
        TeamDatFeatureDetector::detect(&f.dir, &f.backup),
        (false, false)
    );
}
#[test]
fn unknown_team_data_missing_maps_and_unbacked_maps_fail_closed() {
    let mut f = Fixture::new();
    f.baseline(
        "MAPS/ENDL_001/team.dat",
        b"[teamdata]\n0,GER,A,B,80\n1,HUN,A,B,90\n",
    );
    for text in [
        "[teamdata]\n0,GER,A,B,80,newflag\n1,HUN,A,B,90\n",
        "[teamdata]\n0,ROM,A,B,1600\n",
        "[teamdata]\n0,ROM,A,B,1600\n1,HUN,A,B,garbage\n",
        "PFIL",
    ] {
        f.write("MAPS/ENDL_001/team.dat", text.as_bytes());
        assert!(!TeamDatFeatureDetector::detect(&f.dir, &f.backup).0);
    }
    fs::remove_file(f.dir.join("MAPS/ENDL_001/team.dat")).unwrap();
    assert_eq!(
        TeamDatFeatureDetector::detect(&f.dir, &f.backup),
        (false, false)
    );
    f.write(
        "MAPS/ENDL_001/team.dat",
        b"[teamdata]\n0,ROM,A,B,1600\n1,HUN,A,B,1600\n",
    );
    f.write("MAPS/ENDL_003/team.dat", b"[teamdata]\n0,ROM,A,B,1600\n");
    assert_eq!(
        TeamDatFeatureDetector::detect(&f.dir, &f.backup),
        (false, false)
    );
}
#[test]
fn script_checks_every_faction_duplicates_missing_and_fractional_values() {
    let mut original = String::new();
    let mut patched = String::new();
    for faction in ["GER", "KEL", "HUN", "ROM"] {
        original += &format!("CiviDelay={faction},5000\n");
        patched += &format!("CiviDelay={faction},500\n");
        for (key, target) in [
            ("MoralsDecLostMem", 0),
            ("MoralsDecFlee", 0),
            ("MoralsDecOverPop", 99999999),
            ("MoralsIncIdle", 500),
        ] {
            original += &format!("{key}={faction},10\n");
            patched += &format!("{key}={faction},{target}\n");
        }
    }
    let mut p = PatchProfile::new();
    ClScriptFeatureDetector::detect_text(&patched, Some(&original), &mut p);
    assert!(p.get_bool("InfiniteMorale"));
    assert!(p.get_bool("FastCiviProduction"));
    for text in [
        patched.replace("MoralsDecFlee=HUN,0", "MoralsDecFlee=HUN,1"),
        patched.replace("MoralsIncIdle=KEL,500\n", ""),
        format!("{patched}MoralsDecFlee=GER,0\n"),
    ] {
        ClScriptFeatureDetector::detect_text(&text, Some(&original), &mut p);
        assert!(!p.get_bool("InfiniteMorale"));
    }
    for text in [
        patched.replace("CiviDelay=HUN,500", "CiviDelay=HUN,500.7"),
        patched.replace("CiviDelay=ROM,500\n", ""),
    ] {
        ClScriptFeatureDetector::detect_text(&text, Some(&original), &mut p);
        assert!(!p.get_bool("FastCiviProduction"));
    }
}

#[test]
fn full_objdef_combination_proves_each_flag_and_unobservable_spell_stays_false() {
    let f = Fixture::new();
    let mut o = f.options(true);
    o.housing_capacity_20x = true;
    o.storage_capacity_10x = true;
    o.hq_hp_10x = true;
    o.fast_build_upgrade_repair = true;
    o.leader_glory_5x = true;
    o.ranged_range_3x = true;
    o.unit_movement_speed_2x = true;
    o.villager_movement_speed_5x = true;
    o.no_run_hp_loss = true;
    o.projectile_arc_height = true;
    o.all_units_entire_map_vision = true;
    o.spell_entire_map = true;
    f.apply(&o);
    let p = f.detect();
    for id in [
        "HousingCapacity20x",
        "StorageCapacity10x",
        "HqHp10x",
        "FastBuildUpgradeRepair",
        "LeaderGlory",
        "RangedRange3x",
        "UnitMovementSpeed2x",
        "VillagerMovementSpeed5x",
        "NoRunHpLoss",
        "ProjectileArcHeight",
        "AllUnitsEntireMapVision",
        "Balance",
    ] {
        assert!(p.get_bool(id), "{id}");
    }
    assert!(!p.get_bool("SpellEntireMap"));
    f.edit(OBJ, 52, "FigHunPri00_Priester", 191, "1");
    let p = f.detect();
    assert!(!p.get_bool("UnitMovementSpeed2x"));
    assert!(p.get_bool("Balance"));
    assert!(p.get_bool("ProjectileArcHeight"));
}

#[test]
fn pfil_multifile_inputs_and_truncated_weapon_rows_fail_closed() {
    let mut f = Fixture::new();
    for key in [OBJ, GEO, EPA] {
        let raw = f.backup.get_backup_bytes(key).unwrap().to_vec();
        let packed = against_rome_modifier::lzss::compress_pfil(&raw, None).unwrap();
        f.baseline(key, &packed);
    }
    let mut o = f.options(true);
    o.projectile_arc_height = true;
    o.ranged_range_3x = true;
    f.apply(&o);
    let p = f.detect();
    for id in ["Balance", "ProjectileArcHeight", "RangedRange3x"] {
        assert!(p.get_bool(id));
    }
    let doc = TextDocument::read(&fs::read(f.dir.join(OBJ)).unwrap()).unwrap();
    let lines: Vec<_> = doc
        .lines
        .iter()
        .map(|l| {
            let mut c = parse_csv_line(l);
            if c.get(52)
                .is_some_and(|n| n.trim() == "FigHunPri00_Priester")
            {
                c.truncate(130);
                c.join(",")
            } else {
                l.clone()
            }
        })
        .collect();
    f.write(OBJ, lines.join("\r\n").as_bytes());
    let p = f.detect();
    assert!(!p.get_bool("ProjectileArcHeight"));
    assert!(!p.get_bool("Balance"));
    assert!(!p.get_bool("RangedRange3x"));
}

#[test]
fn resource_required_columns_and_duplicates_fail_closed() {
    let mut p = PatchProfile::new();
    for text in [
        "[objres]\nBauRomHau00,0,0,0\n",
        "[objres]\nFigHunPri00_Priester,0,0,0\n",
        "[volkres]\nHun_Kampf,0,0,0\n",
    ] {
        RessFeatureDetector::detect_text(text, &mut p);
        for id in ["FreeProduction", "FreeUpgrade", "NoSpellCost"] {
            assert!(!p.get_bool(id));
        }
    }
    let row = format!("FigHunPri00_Priester,{}", vec!["0"; 28].join(","));
    RessFeatureDetector::detect_text(&format!("[objres]\n{row}\n{row}"), &mut p);
    assert!(!p.get_bool("FreeProduction"));
    assert!(!p.get_bool("NoSpellCost"));
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
