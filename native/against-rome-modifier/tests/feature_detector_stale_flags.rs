//! Detectors must overwrite every flag the C# detectors assign, so a pre-populated
//! PatchProfile never keeps a stale `true`/non-zero value.
use against_rome_modifier::{
    bci_features::BciFeatureDetector,
    detectors::{ExeFeatureDetector, ObjdefFeatureDetector},
    endless_ai::EndlessAiOrchestrator,
    lzss::compress_pfil,
    services::BackupManager,
    PatchProfile,
};
use std::{fs, path::PathBuf};

struct Tmp(PathBuf);
impl Tmp {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("arm-stale-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        Tmp(p)
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn exe_detector_resets_unknown_game_speed() {
    let t = Tmp::new("exe");
    fs::write(t.0.join("Against_Rome.exe"), vec![0u8; 0x400000]).unwrap();
    let mut profile = PatchProfile::new();
    profile.set_int("GameSpeed", 3);
    profile.set_bool("CiviProduce20", true);
    ExeFeatureDetector::detect(&t.0, &mut profile);
    assert_eq!(profile.get_int("GameSpeed"), 0);
    assert!(!profile.get_bool("CiviProduce20"));
}

#[test]
fn objdef_detector_without_backup_clears_backup_independent_flags() {
    let t = Tmp::new("objdef");
    let line = format!("{}\r\n", ",x".repeat(60));
    let raw = compress_pfil(line.repeat(3).as_bytes(), None).unwrap();
    let path = t.0.join("SYSTEM/DATA_MP/DEFAULTS/objdef.dau");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, raw).unwrap();

    let ids = [
        "AllUnitsEntireMapVision",
        "Balance",
        "LeaderGlory",
        "UnitMovementSpeed2x",
        "RangedRange3x",
        "VillagerMovementSpeed5x",
        "SpellEntireMap",
    ];
    let mut profile = PatchProfile::new();
    for id in ids {
        profile.set_bool(id, true);
    }
    // Backup-relative flags stay untouched when no backup exists (C# skips them).
    profile.set_bool("HousingCapacity20x", true);

    ObjdefFeatureDetector::detect(&t.0, &BackupManager::new(), &mut profile);
    for id in ids {
        assert!(!profile.get_bool(id), "{id} stayed stale");
    }
    assert!(profile.get_bool("HousingCapacity20x"));
}

#[test]
fn bci_detector_resets_food_healing_when_try_detect_reports_false() {
    // No SYSTEM/CLAK/SCRIPT/*.bci: C# TryDetect returns false with enabled=false,
    // and BciFeatureDetector still writes that false into the profile.
    let t = Tmp::new("bci-missing");
    let mut profile = PatchProfile::new();
    profile.set_bool("FoodHealing10x", true);
    let mut orchestrator = EndlessAiOrchestrator::new(false);
    BciFeatureDetector::detect(&t.0, &mut profile, &mut orchestrator);
    assert!(
        !profile.get_bool("FoodHealing10x"),
        "FoodHealing10x stayed stale"
    );
}

#[test]
fn bci_detector_resets_food_healing_on_ambiguous_site_count() {
    // Every script present but with no matching signature (orig + ult == 0): TryDetect is false.
    let t = Tmp::new("bci-nomatch");
    let dir = t.0.join("SYSTEM/CLAK/SCRIPT");
    fs::create_dir_all(&dir).unwrap();
    let raw = compress_pfil(&[0u8; 64], None).unwrap();
    for f in [
        "ak_anfuehrer",
        "ak_artillerie",
        "ak_geisterreiter",
        "ak_kampfverband",
        "ak_krieger",
        "ak_kundschafterwolf",
        "ak_landtier",
        "ak_packpferd",
        "ak_priester",
        "ak_verbandswolf",
        "ak_zivilist",
        "ak_zivilverband",
    ] {
        fs::write(dir.join(format!("{f}.bci")), &raw).unwrap();
    }
    let mut profile = PatchProfile::new();
    profile.set_bool("FoodHealing10x", true);
    let mut orchestrator = EndlessAiOrchestrator::new(false);
    BciFeatureDetector::detect(&t.0, &mut profile, &mut orchestrator);
    assert!(
        !profile.get_bool("FoodHealing10x"),
        "FoodHealing10x stayed stale"
    );
}
