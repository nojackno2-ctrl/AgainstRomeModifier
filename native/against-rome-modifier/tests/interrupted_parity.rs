use against_rome_modifier::{
    endless_ai::{self, EndlessAiOrchestrator},
    services::{BackupManager, PatchEngine},
    unit_stats, PatchProfile, TroopConfig,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Tmp(PathBuf);
impl Tmp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "arm-interrupted-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn legacy_core_migration_is_atomic_case_insensitive_and_idempotent() {
    // C# PatchProfile.NormalizeCompositeValues migrates all legacy lifecycle selections.
    for (id, enabled) in [("Core", true), ("m3", true), ("M2", false)] {
        let mut p = PatchProfile::new();
        p.endless_ai_modules.insert(id.into(), enabled);
        p.normalize_composite_values();
        for name in ["Core", "M2", "M3", "M4"] {
            assert_eq!(p.get_endless_ai_module(name), enabled, "{id}/{name}");
        }
        let once = p.endless_ai_modules.clone();
        p.normalize_composite_values();
        assert_eq!(p.endless_ai_modules, once);
    }
    let mut p = PatchProfile::new();
    p.endless_ai_modules.insert("M2".into(), false);
    p.endless_ai_modules.insert("M3".into(), true);
    p.normalize_composite_values();
    for name in ["Core", "M2", "M3", "M4"] {
        assert!(p.get_endless_ai_module(name));
    }
    let mut p = PatchProfile::new();
    p.set_bool("EndlessAi.Core", true);
    assert!(p.get_endless_ai_module("M2"));
}

#[test]
fn legacy_quota_and_roman_migrate_without_replacing_explicit_multiplier() {
    let mut p = PatchProfile::new();
    p.endless_ai_modules.insert("m6".into(), true);
    p.endless_ai_modules
        .insert("VillageGarrisonQuota3x".into(), true);
    p.normalize_composite_values();
    assert!(p.get_bool("RomanReinforcementGarrison"));
    assert!(p.get_endless_ai_module("M6"));
    assert_eq!(p.get_int("VillageGarrisonQuotaMultiplier"), 3);
    p.set_int("VillageGarrisonQuotaMultiplier", 7);
    p.normalize_composite_values();
    assert_eq!(p.get_int("VillageGarrisonQuotaMultiplier"), 7);
}

#[test]
fn projected_custom_priest_stats_preserve_independent_fields_then_apply_modifiers() {
    // Mirrors UnitStatsProjectionServiceTests: baseline merge before display modifiers.
    let key = "FigKelPri00_Priester";
    let b = BackupManager::new();
    let baseline = *TroopConfig::get_balanced_stats(key).unwrap();
    let mut p = PatchProfile::new();
    p.set_bool("Balance", true);
    p.custom_unit_stats = Some(
        [(
            key.to_lowercase(),
            vec![111., 22., 33., 44., 999., 888., 77., 666., 555.],
        )]
        .into(),
    );
    for id in ["UnitMovementSpeed2x", "SpellEntireMap", "SpellRange3x"] {
        p.set_bool(id, true);
    }
    let before = p.custom_unit_stats.clone();
    let result = unit_stats::project(&b, key, &p).unwrap();
    assert_eq!(&result[..4], &[111., 22., 33., 44.]);
    assert_eq!(result[6], 77.);
    assert_eq!(result[4], baseline[4] * 2.);
    assert_eq!(result[5], baseline[5]);
    assert_eq!(result[7], 30000.);
    assert_eq!(result[8], baseline[8] * 3.);
    assert_eq!(p.custom_unit_stats, before);
    p.set_bool("Balance", false);
    // 自訂屬性可直接套用在原版基準上，不再依賴平衡預設。
    assert_eq!(unit_stats::project(&b, key, &p).unwrap()[0], 111.);
}

#[test]
fn projected_ranged_range_vision_and_nonspell_radius_match_csharp_rules() {
    let key = TroopConfig::sorted_units()
        .into_iter()
        .find(|k| TroopConfig::get_metadata(k).unwrap().unit_type == "ranged_inf")
        .unwrap();
    let b = BackupManager::new();
    let baseline = *TroopConfig::get_balanced_stats(key).unwrap();
    let mut p = PatchProfile::new();
    for id in [
        "Balance",
        "RangedRange3x",
        "UnitMovementSpeed2x",
        "AllUnitsEntireMapVision",
    ] {
        p.set_bool(id, true);
    }
    let result = unit_stats::project(&b, key, &p).unwrap();
    assert_eq!(result[4], baseline[4] * 2.);
    assert_eq!(result[5], 30000.);
    assert_eq!(result[7], baseline[7] * 3.);
    assert_eq!(result[8], 0.);
}

#[test]
fn orchestrator_preserves_template_header_and_planning_never_writes_source() {
    let t = Tmp::new();
    let path = t.0.join("script.bci");
    let mut header = vec![0xA7; 64];
    header[..4].copy_from_slice(b"PFIL");
    let raw = against_rome_modifier::compress_pfil(&[1; 128], Some(&header)).unwrap();
    fs::write(&path, &raw).unwrap();
    let mut o = EndlessAiOrchestrator::new(false);
    let f = o.get_or_load_file(&path).unwrap();
    f.0[5] = 42;
    f.1 = true;
    let planned = o.save_all().unwrap();
    assert_eq!(planned.len(), 1);
    assert_eq!(
        against_rome_modifier::decompress_pfil(&planned[0].1).unwrap()[5],
        42
    );
    assert_eq!(&planned[0].1[16..64], &raw[16..64]);
    assert_eq!(fs::read(&path).unwrap(), raw);
    assert!(o.save_all().unwrap().is_empty());
    o.clear_cache();
    assert!(o.file_cache.is_empty() && o.raw_headers.is_empty());
}

#[test]
fn malformed_pfil_never_enters_script_cache() {
    let t = Tmp::new();
    let path = t.0.join("script.bci");
    fs::write(&path, b"PFILshort").unwrap();
    let mut o = EndlessAiOrchestrator::new(false);
    assert!(o.get_or_load_file(&path).is_err());
    assert!(o.file_cache.is_empty() && o.raw_headers.is_empty());
    assert!(o.replace_raw_bytes(&path, b"PFILshort").is_err());
    assert!(o.file_cache.is_empty());
}

#[test]
fn serialization_failure_preserves_all_dirty_scripts_for_retry() {
    let mut o = EndlessAiOrchestrator::new(false);
    let a = PathBuf::from("a.bci");
    let z = PathBuf::from("z.bci");
    let header = against_rome_modifier::compress_pfil(&[1; 128], None).unwrap();
    for path in [&a, &z] {
        o.file_cache.insert(path.clone(), (vec![2; 128], true));
    }
    o.raw_headers.insert(a.clone(), header.clone());
    assert!(o.save_all().is_err());
    assert!(o.file_cache[&a].1 && o.file_cache[&z].1);
    o.raw_headers.insert(z.clone(), header);
    let result = o.save_all().unwrap();
    assert_eq!(
        result.iter().map(|(p, _)| p).collect::<Vec<_>>(),
        vec![&a, &z]
    );
}

#[test]
fn standalone_marker_filters_only_valid_slots_and_never_panics_on_unicode_names() {
    let t = Tmp::new();
    for (name, standalone, expected) in [
        ("ENDL_005", true, true),
        ("ENDL_004", true, false),
        ("ENDL_006", false, false),
        ("ab中def", true, false),
    ] {
        let dir = t.0.join("MAPS").join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(".arm_custom_map"),
            format!("{{\"StandaloneLevel\":{standalone}}}"),
        )
        .unwrap();
        assert_eq!(endless_ai::has_standalone_level(&dir), expected);
    }
    assert!(PatchEngine::compose_bci(&t.0, &PatchProfile::new(), &BackupManager::new()).is_err());
    // C# requires the food-healing scripts even when disabled.
}

#[test]
fn compose_bci_food_healing_enable_disable_is_exact_and_read_only() {
    let t = Tmp::new();
    let root = t.0.join("SYSTEM/CLAK/SCRIPT");
    fs::create_dir_all(&root).unwrap();
    let sites = [
        ("ak_anfuehrer", 87),
        ("ak_artillerie", 69),
        ("ak_geisterreiter", 68),
        ("ak_kampfverband", 89),
        ("ak_krieger", 73),
        ("ak_kundschafterwolf", 77),
        ("ak_landtier", 72),
        ("ak_packpferd", 57),
        ("ak_priester", 92),
        ("ak_verbandswolf", 68),
        ("ak_zivilist", 83),
        ("ak_zivilverband", 87),
    ];
    let mut originals = std::collections::BTreeMap::new();
    for (file, symbol) in sites {
        let mut bytes = endless_ai::words_bytes(&[66, 1, 81, 10, 81, 98, 128, symbol, 73, -3, 86]);
        bytes.resize(128, 0);
        let raw = against_rome_modifier::compress_pfil(&bytes, None).unwrap();
        let path = root.join(format!("{file}.bci"));
        fs::write(&path, &raw).unwrap();
        originals.insert(path, raw);
    }
    let mut p = PatchProfile::new();
    p.set_bool("FoodHealing10x", true);
    let enabled = PatchEngine::compose_bci(&t.0, &p, &BackupManager::new()).unwrap();
    assert_eq!(enabled.len(), 12);
    for (path, raw) in &enabled {
        assert_eq!(fs::read(path).unwrap(), originals[path]);
        let data = against_rome_modifier::decompress_pfil(raw).unwrap();
        assert_eq!(endless_ai::read_word(&data, 4), Some(10));
        // Commit only into this synthetic fixture, then verify disable restores all decompressed bytes.
        fs::write(path, raw).unwrap();
    }
    p.set_bool("FoodHealing10x", false);
    let disabled = PatchEngine::compose_bci(&t.0, &p, &BackupManager::new()).unwrap();
    assert_eq!(disabled.len(), 12);
    for (path, raw) in disabled {
        assert_eq!(
            against_rome_modifier::decompress_pfil(&raw).unwrap(),
            against_rome_modifier::decompress_pfil(&originals[&path]).unwrap()
        );
    }
}
