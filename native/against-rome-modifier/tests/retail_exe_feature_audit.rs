use against_rome_modifier::{exe_features::*, PatchProfile};
use std::{fs, path::PathBuf};

// 僅讀取明確指定的零售 EXE，所有補丁與還原均在記憶體內完成。
#[test]
#[ignore = "需要以 ARM_AUDIT_EXE 指定未修改的零售 EXE"]
fn retail_exe_features_apply_idempotently_and_restore_exactly(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(std::env::var_os("ARM_AUDIT_EXE").ok_or("缺少 ARM_AUDIT_EXE")?);
    let original = fs::read(&path)?;
    assert!(is_exe_original(&original), "來源必須符合原版補丁簽章");
    assert_eq!(get_game_speed_multiplier(&original), 1);
    assert_eq!(apply_profile(&original, &PatchProfile::new())?, original);

    let ids = [
        "FocusLoss",
        "VillageBuildRange",
        "NoSpellAltar",
        "RomanEndless",
        "CiviProduce20",
        "UnitRecruit20",
        "IdleSelect999",
        "CorpseRetention",
        "CameraZoomOut1",
        // 此項只證明位元組往返，畫面與既有退役政策另由稽核判定。
        "NativeWidescreen1920x1080",
    ];
    for id in ids {
        let mut profile = PatchProfile::new();
        profile.set_bool(id, true);
        let patched = apply_profile(&original, &profile)?;
        assert_ne!(patched, original, "{id} 必須實際改變位元組");
        assert_eq!(patched.len(), original.len(), "{id} 必須維持 EXE 長度");
        assert_eq!(apply_profile(&patched, &profile)?, patched, "{id} 重複套用");
        assert_eq!(
            apply_profile(&patched, &PatchProfile::new())?,
            original,
            "{id} 關閉還原"
        );
        let restored = restore_stats_exe(&restore_compat_exe(&patched, false)?)?;
        assert_eq!(restored, original, "{id} 分類還原");
        eprintln!("零售 EXE 記憶體往返通過：{id}");
    }
    for speed in [2, 5, 10] {
        let mut profile = PatchProfile::new();
        profile.set_int("GameSpeed", speed);
        let patched = apply_profile(&original, &profile)?;
        assert_eq!(get_game_speed_multiplier(&patched), speed);
        assert_eq!(apply_profile(&patched, &profile)?, patched);
        assert_eq!(apply_profile(&patched, &PatchProfile::new())?, original);
        eprintln!("零售 EXE 記憶體往返通過：GameSpeed={speed}");
    }
    let mut combined = PatchProfile::new();
    for id in ids {
        combined.set_bool(id, true);
    }
    combined.set_int("GameSpeed", 5);
    let patched = apply_profile(&original, &combined)?;
    assert_eq!(apply_profile(&patched, &combined)?, patched);
    assert_eq!(apply_profile(&patched, &PatchProfile::new())?, original);
    assert_eq!(
        restore_stats_exe(&restore_compat_exe(&patched, false)?)?,
        original
    );
    assert_eq!(fs::read(&path)?, original, "安裝來源必須完全不變");
    Ok(())
}
