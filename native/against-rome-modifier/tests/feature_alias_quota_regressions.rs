use against_rome_modifier::{
    bci_features::BciFeatureDetector,
    cli::{profile_from_json, profile_to_json},
    endless_ai::{
        read_word, EndlessAiOrchestrator, EndlessPatch, P20VillageGarrisonQuotaPatch, PatchState,
    },
    PatchProfile,
};
use serde_json::json;
use std::{fs, path::PathBuf};

#[test]
fn direct_and_legacy_core_aliases_survive_normalization_and_json_roundtrip() {
    for alias in ["Core", "M2", "M3", "M4"] {
        for legacy in [false, true] {
            let input = if legacy {
                json!({"EndlessAiModules": {alias: true}})
            } else {
                json!({"Features": {format!("EndlessAi.{alias}"): true}})
            };
            let mut p = profile_from_json(&input).unwrap();
            for _ in 0..2 {
                p.normalize_composite_values();
                for id in ["Core", "M2", "M3", "M4"] {
                    assert!(p.get_bool(&format!("EndlessAi.{id}")), "{input}/{id}");
                    assert!(p.get_endless_ai_module(id));
                }
                p = profile_from_json(&profile_to_json(&p)).unwrap();
            }
        }
    }
    let p = profile_from_json(&json!({
        "Features": {"EndlessAi.M2": true},
        "EndlessAiModules": {"M2": false, "Core": false}
    }))
    .unwrap();
    assert!(p.get_bool("EndlessAi.Core"));
}

#[test]
fn quota_registry_alias_bool_migration_precedence_and_roundtrip() {
    for (input, expected) in [
        (json!({"Features":{"VillageGarrisonQuota3x":5}}), 5),
        (json!({"Features":{"villagegarrisonquota3x":true}}), 3),
        (json!({"VillageGarrisonQuota3x":true}), 3),
        (
            json!({"EndlessAiModules":{"VillageGarrisonQuota3x":true}}),
            3,
        ),
        (
            json!({"Features":{"VillageGarrisonQuota3x":5},"VillageGarrisonQuota3x":true}),
            5,
        ),
        (
            json!({"Features":{"VillageGarrisonQuota3x":5},"VillageGarrisonQuotaMultiplier":2}),
            2,
        ),
        (
            json!({"Features":{"villagegarrisonquota3x":5,"VILLAGEGARRISONQUOTAMULTIPLIER":10}}),
            10,
        ),
        (
            json!({"VillageGarrisonQuotaMultiplier":1,"VillageGarrisonQuota3x":true,
            "EndlessAiModules":{"VillageGarrisonQuota3x":true}}),
            1,
        ),
        (
            json!({"Features":{"VillageGarrisonQuota3x":false},
            "EndlessAiModules":{"VillageGarrisonQuota3x":true}}),
            1,
        ),
    ] {
        let mut p = profile_from_json(&input).unwrap();
        for _ in 0..2 {
            p.normalize_composite_values();
            for id in ["VillageGarrisonQuota3x", "VillageGarrisonQuotaMultiplier"] {
                assert_eq!(p.get_int(id), expected, "{input}/{id}");
            }
            assert_eq!(
                p.get_endless_ai_module("VillageGarrisonQuota3x"),
                expected > 1
            );
            p = profile_from_json(&profile_to_json(&p)).unwrap();
        }
    }
    let mut p = PatchProfile::new();
    assert_eq!(p.get_int("VillageGarrisonQuotaMultiplier"), 1);
    for id in ["VillageGarrisonQuota3x", "VillageGarrisonQuotaMultiplier"] {
        p.set_int(id, 10);
        assert_eq!(p.get_int("VillageGarrisonQuota3x"), 10);
        assert_eq!(p.get_int("VillageGarrisonQuotaMultiplier"), 10);
        p.set_bool(id, false);
        assert_eq!(p.get_int("VillageGarrisonQuota3x"), 1);
    }
    for value in [json!(2147483648_i64), json!(2.5), json!("5"), json!(null)] {
        assert!(profile_from_json(&json!({"Features":{"VillageGarrisonQuota3x":value}})).is_err());
    }
}

#[test]
fn canonical_composite_toggle_off_clears_detected_or_normalized_aliases() {
    for (canonical, aliases) in [
        ("EndlessAi.Core", vec!["Core", "M2", "M3", "M4"]),
        ("RomanReinforcementGarrison", vec!["M6"]),
    ] {
        let mut profile = PatchProfile::new();
        profile.set_bool(canonical, true);
        profile.normalize_composite_values();
        profile.set_bool(canonical, false);
        profile.normalize_composite_values();
        assert!(!profile.get_bool(canonical));
        for alias in aliases {
            assert!(!profile.get_endless_ai_module(alias));
            assert!(!profile.get_bool(&format!("EndlessAi.{alias}")));
        }
        let restored = profile_from_json(&profile_to_json(&profile)).unwrap();
        assert!(!restored.get_bool(canonical));
    }
}

#[test]
fn p20_unknown_bytes_are_rejected_without_mutation() {
    let patch = P20VillageGarrisonQuotaPatch::default();
    for original in [vec![], vec![0; 48], b"BCI0invalid".to_vec()] {
        for enabled in [false, true] {
            let mut data = original.clone();
            assert_eq!(patch.detect(&data), PatchState::Unknown);
            assert!(patch.apply(&mut data, enabled).is_err());
            assert_eq!(data, original);
        }
    }
}

#[test]
#[ignore = "需 ARM_AUDIT_GAME 指向唯讀零售遊戲目錄；僅改記憶體"]
fn retail_bci_orchestrator_quota_literals_switch_restore_and_detector_mirrors() {
    let game = PathBuf::from(std::env::var_os("ARM_AUDIT_GAME").expect("ARM_AUDIT_GAME"));
    let path = game.join("SYSTEM/CLAK/SCRIPT/Dorfverteidigung.bci");
    let raw = fs::read(&path).unwrap();
    let original = against_rome_modifier::decompress_if_pfil(&raw).unwrap();
    let patch = P20VillageGarrisonQuotaPatch::default();
    assert_eq!(patch.detect(&original), PatchState::Original);
    let mut orch = EndlessAiOrchestrator::default();
    orch.file_cache
        .insert(path.clone(), (original.clone(), false));
    let code_end = 0x24 + read_word(&original, 8).unwrap() as usize;
    let mut installed_len = 0;
    for multiplier in [2, 3, 5, 10, 2] {
        orch.set_village_garrison_quota_multiplier(multiplier);
        assert!(orch
            .apply_module_by_id(&game, "VillageGarrisonQuota3x", true)
            .unwrap()
            .unwrap());
        let data = &orch.file_cache[&path].0;
        assert_eq!(read_word(data, code_end + 14 * 4), Some(multiplier));
        assert_eq!(&data[code_end + 22 * 4..], &original[code_end..]);
        assert_eq!(
            P20VillageGarrisonQuotaPatch::try_read_installed_multiplier(data),
            Some(multiplier)
        );
        if installed_len == 0 {
            installed_len = data.len();
        }
        assert_eq!(data.len(), installed_len);
        let once = data.clone();
        assert!(!orch
            .apply_module_by_id(&game, "VillageGarrisonQuota3x", true)
            .unwrap()
            .unwrap());
        assert_eq!(orch.file_cache[&path].0, once);
        let mut p = PatchProfile::new();
        BciFeatureDetector::detect(&game, &mut p, &mut orch);
        assert_eq!(p.get_int("VillageGarrisonQuota3x"), multiplier);
        assert_eq!(p.get_int("VillageGarrisonQuotaMultiplier"), multiplier);
    }
    assert!(orch
        .apply_module_by_id(&game, "VillageGarrisonQuota3x", false)
        .unwrap()
        .unwrap());
    assert_eq!(orch.file_cache[&path].0, original);
    assert!(!orch
        .apply_module_by_id(&game, "VillageGarrisonQuota3x", false)
        .unwrap()
        .unwrap());

    // 真正 Core patch 套用至唯讀載入的快取，檢查偵測器的正反向別名鏡射。
    for enabled in [true, false] {
        orch.apply_module_by_id(&game, "Core", enabled)
            .unwrap()
            .unwrap();
        let mut p = PatchProfile::new();
        for alias in ["core", "m2", "m3", "m4"] {
            p.endless_ai_modules.insert(alias.into(), !enabled);
        }
        BciFeatureDetector::detect(&game, &mut p, &mut orch);
        for alias in ["Core", "M2", "M3", "M4"] {
            assert_eq!(p.get_bool(&format!("EndlessAi.{alias}")), enabled);
            assert_eq!(p.get_endless_ai_module(alias), enabled);
        }
    }

    // 破壞 helper 常數後，啟用與還原都必須拒絕，保留未知資料。
    orch.apply_module_by_id(&game, "VillageGarrisonQuota3x", true)
        .unwrap()
        .unwrap();
    orch.file_cache.get_mut(&path).unwrap().0[code_end] ^= 1;
    let unknown = orch.file_cache[&path].0.clone();
    for enabled in [true, false] {
        assert!(orch
            .apply_module_by_id(&game, "VillageGarrisonQuota3x", enabled)
            .unwrap()
            .is_err());
        assert_eq!(orch.file_cache[&path].0, unknown);
    }
    assert_eq!(fs::read(&path).unwrap(), raw);
}
