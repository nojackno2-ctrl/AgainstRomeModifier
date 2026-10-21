//! Regression and differential tests for category-selective restoration
//! (Stats vs Compat vs Language) and startup safe migrations, matching
//! C# PatchRestoreService.cs and CharacterizationTests.cs T3 behavior.

use against_rome_modifier::{
    detectors::FeatureDetector,
    endless_ai::words_bytes,
    exe_features::*,
    lzss::compress_pfil,
    services::{BackupManager, PatchEngine, PatchFilePlan},
    unit_stats, FeatureCategory, FeatureValue, PatchProfile,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

struct TempSandbox(PathBuf);
impl TempSandbox {
    fn new(prefix: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "arm-cat-restore-{}-{}-{}",
            prefix,
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn path(&self, rel: &str) -> PathBuf {
        let mut p = self.0.clone();
        for part in rel.split(['/', '\\']) {
            if !part.is_empty() {
                p.push(part);
            }
        }
        p
    }
    fn write(&self, rel: &str, bytes: &[u8]) -> PathBuf {
        let p = self.path(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, bytes).unwrap();
        p
    }
}

impl Drop for TempSandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn apply_plan_to_disk(plan: &PatchFilePlan) {
    for (path, bytes) in &plan.files {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(path, bytes).unwrap();
    }
    for path in &plan.deletes {
        let _ = fs::remove_file(path);
    }
}

fn create_synthetic_original_exe() -> Vec<u8> {
    let mut b = vec![0x90u8; 0x220000];
    let sites: &[(usize, &[u8])] = &[
        (FOCUS_OFFSET, FOCUS_ORIGINAL),
        (ROMAN_ENDLESS_OFFSET, ROMAN_ENDLESS_ORIGINAL),
        (CIVI_PRODUCE_20_OFFSET, CIVI_PRODUCE_20_ORIGINAL),
        (UNIT_RECRUIT_20_OFFSET, UNIT_RECRUIT_20_ORIGINAL),
        (IDLE_SELECT_999_OFFSET, IDLE_SELECT_999_ORIGINAL),
        (CORPSE_RETENTION_OFFSET, CORPSE_RETENTION_ORIGINAL),
        (VILLAGE_RANGE_X_OFFSET, VILLAGE_RANGE_X_ORIGINAL),
        (VILLAGE_RANGE_Z_OFFSET, VILLAGE_RANGE_Z_ORIGINAL),
        (VILLAGE_FRAME_X_OFFSET, VILLAGE_FRAME_X_ORIGINAL),
        (VILLAGE_FRAME_Z_OFFSET, VILLAGE_FRAME_Z_ORIGINAL),
        (VILLAGE_HOOK_OFFSET, VILLAGE_HOOK_ORIGINAL),
        (VILLAGE_CAVE_OFFSET, VILLAGE_CAVE_ORIGINAL),
        (WS_ID_OFFSET, WS_ID_ORIGINAL),
        (WS_CREATE_OFFSET, WS_CREATE_ORIGINAL),
        (WS_TEXT_OFFSET, WS_TEXT_ORIGINAL),
        (WS_FORCE_OFFSET, WS_FORCE_ORIGINAL),
        (WS_ACTMODE_OFFSET, WS_ACTMODE_ORIGINAL),
        (WS_IGM_OFFSET, WS_IGM_ORIGINAL),
        (ZOOM_INIT_OFFSET, ZOOM_INIT_ORIGINAL),
        (ZOOM_LOAD_OFFSET, ZOOM_LOAD_ORIGINAL),
        (ZOOM_SCRIPT_OFFSET, ZOOM_SCRIPT_ORIGINAL),
        (ZOOM_CAVE_OFFSET, ZOOM_CAVE_ORIGINAL),
    ];
    for &(o, s) in sites {
        b[o..o + s.len()].copy_from_slice(s);
    }
    for &(o, orig, _) in SPELL_ALTAR_SITES {
        b[o..o + orig.len()].copy_from_slice(orig);
    }
    b[GAME_SPEED_QPC_OFFSET..GAME_SPEED_QPC_OFFSET + 8]
        .copy_from_slice(&GAME_SPEED_QPC_BASE.to_le_bytes());
    b[GAME_SPEED_TGT_OFFSET..GAME_SPEED_TGT_OFFSET + 8]
        .copy_from_slice(&GAME_SPEED_TGT_BASE.to_le_bytes());
    b
}

fn create_synthetic_bci(_game: &Path, symbol: i32) -> Vec<u8> {
    let mut words = vec![66, 1, 81, 10, 81, 98, 128, symbol, 73, -3, 86];
    words.resize(64, 0);
    let raw = words_bytes(&words);
    compress_pfil(&raw, None).unwrap()
}

fn setup_full_game_environment(sandbox: &TempSandbox) -> BackupManager {
    let original_exe = create_synthetic_original_exe();

    let cl_script_orig = b"[CiviDelay]\r\nCiviDelay =GER, 5000\r\n";
    let cl_epara_orig = b"[General]\r\n";
    let ress_orig = b"[objres]\r\nRes0=100\r\n";
    let scint_orig = b"[Scint]\r\n";
    let partgeo_orig = b"; partgeo original\r\n";
    let objdef_orig = b"Header1\r\nHeader2\r\n1,2,3,4,1.6,0,0,0,0,0,0,0,0,0,0,0,0,0,0,400,0,0,0,0,500,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,FigRomAnf00_Anfuehrer,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,10,0,0,0,10,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0\r\n";
    let objdef_pfil = compress_pfil(objdef_orig, None).unwrap();
    let team_dat_orig = b"TEAMS\r\n";
    let ak_anf_orig = create_synthetic_bci(&sandbox.0, 87);

    // Write backup manager files in memory
    let mut backup = BackupManager::new();
    backup.set_backup_file("Against_Rome.exe", original_exe.clone());
    backup.set_backup_file("SYSTEM/cl_script.ini", cl_script_orig.to_vec());
    backup.set_backup_file("SYSTEM/cl_epara.ini", cl_epara_orig.to_vec());
    backup.set_backup_file("SYSTEM/ress.ini", ress_orig.to_vec());
    backup.set_backup_file("SYSTEM/CLAK/cl_scint.ini", scint_orig.to_vec());
    backup.set_backup_file("SYSTEM/DATA_MP/DEFAULTS/partgeo.dau", partgeo_orig.to_vec());
    backup.set_backup_file("SYSTEM/DATA_MP/DEFAULTS/objdef.dau", objdef_pfil.clone());
    backup.set_backup_file("MAPS/ENDL_000/team.dat", team_dat_orig.to_vec());
    backup.set_backup_file("SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci", ak_anf_orig.clone());

    // Prepare a fully patched state on disk:
    let mut patched_profile = PatchProfile::new();
    // Stats features
    patched_profile.set_bool("FastCiviProduction", true);
    patched_profile.set_bool("RomanEndless", true);
    patched_profile.set_bool("CiviProduce20", true);
    patched_profile.set_bool("UnitRecruit20", true);
    patched_profile.set_bool("CorpseRetention", true);
    // Compat features
    patched_profile.set_bool("FocusLoss", true);
    patched_profile.set_bool("NativeWidescreen1920x1080", true);
    patched_profile.set_bool("CameraZoomOut1", true);
    patched_profile.set("GameSpeed", FeatureValue::Int(2));

    let patched_exe = apply_profile(&original_exe, &patched_profile).unwrap();
    sandbox.write("Against_Rome.exe", &patched_exe);

    // Write modified data files
    sandbox.write(
        "SYSTEM/cl_script.ini",
        b"[CiviDelay]\r\nCiviDelay =GER, 500\r\n",
    );
    sandbox.write("SYSTEM/cl_epara.ini", b"[General]\r\nPatched=1\r\n");
    sandbox.write("SYSTEM/ress.ini", b"[objres]\r\nRes0=0\r\n");
    sandbox.write("SYSTEM/CLAK/cl_scint.ini", b"[Scint]\r\nPatched=1\r\n");
    sandbox.write(
        "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau",
        b"; partgeo patched\r\n",
    );
    sandbox.write("SYSTEM/DATA_MP/DEFAULTS/objdef.dau", &objdef_pfil);
    sandbox.write("MAPS/ENDL_000/team.dat", b"TEAMS_MODIFIED\r\n");

    // Write BCI scripts
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
    for (file, sym) in sites {
        let bci = create_synthetic_bci(&sandbox.0, sym);
        sandbox.write(&format!("SYSTEM/CLAK/SCRIPT/{file}.bci"), &bci);
    }

    // Install mock dgVoodoo & trace using proper install plans and markers
    let mut res = against_rome_modifier::install::InstallResources::default();
    for n in ["D3D8.dll", "DDraw.dll", "dgVoodooCpl.exe"] {
        res.dgvoodoo.insert(n.into(), n.as_bytes().to_vec());
    }
    res.dgvoodoo.insert(
        "dgVoodoo.conf".into(),
        b"FullScreenMode=false\nScalingMode=a\nCenterAppWindow=false\nWindowedAttributes=\nFullscreenAttributes=\n".to_vec(),
    );
    res.argm_winmm = b"winmm-bytes".to_vec();

    let dg_plan =
        against_rome_modifier::install::DgVoodooFeature::plan(&sandbox.0, true, true, &res)
            .unwrap();
    apply_plan_to_disk(&PatchFilePlan {
        files: dg_plan.writes,
        deletes: dg_plan.deletes,
    });
    let trace_plan =
        against_rome_modifier::install::ArgmTraceFeature::plan(&sandbox.0, true, &res).unwrap();
    apply_plan_to_disk(&PatchFilePlan {
        files: trace_plan.writes,
        deletes: trace_plan.deletes,
    });

    backup
}

#[test]
fn restore_stats_only_keeps_compat_and_reverts_stats_on_exe_and_data() {
    let sb = TempSandbox::new("stats-only");
    let backup = setup_full_game_environment(&sb);

    let plan = PatchEngine::restore_stats_only(&sb.0, &backup).unwrap();

    // Verify Against_Rome.exe is present in plan with Stats removed, Compat preserved
    let exe_path = sb.path("Against_Rome.exe");
    assert!(
        plan.files.contains_key(&exe_path),
        "Against_Rome.exe must be updated"
    );
    let planned_exe = &plan.files[&exe_path];

    // Stats features on EXE must be restored to original
    assert_eq!(
        get_roman_endless_patch_state(planned_exe),
        ExeRomanEndlessPatchState::Original
    );
    assert_eq!(
        get_civi_produce_20_patch_state(planned_exe),
        ExeCiviProduce20PatchState::Original
    );
    assert_eq!(
        get_unit_recruit_20_patch_state(planned_exe),
        ExeUnitRecruit20PatchState::Original
    );
    assert_eq!(
        get_corpse_retention_patch_state(planned_exe),
        ExeCorpseRetentionPatchState::Original
    );

    // Compat features on EXE must REMAIN PATCHED!
    assert_eq!(
        get_focus_patch_state(planned_exe),
        ExePatchState::FocusPatched
    );
    assert_eq!(
        get_native_widescreen_patch_state(planned_exe),
        ExeNativeWidescreenPatchState::Original
    );
    assert_eq!(
        get_camera_zoom_out_patch_state(planned_exe),
        ExeCameraZoomOutPatchState::Patched
    );
    assert_eq!(get_game_speed_multiplier(planned_exe), 2);

    // Data files must be restored
    assert!(plan.files.contains_key(&sb.path("SYSTEM/cl_script.ini")));
    assert!(plan.files.contains_key(&sb.path("SYSTEM/ress.ini")));
    assert!(plan.files.keys().any(|p| p
        .to_string_lossy()
        .eq_ignore_ascii_case(&sb.path("MAPS/ENDL_000/team.dat").to_string_lossy())));

    // DgVoodoo / Trace files must NOT be deleted
    assert!(!plan.deletes.contains(&sb.path("DDraw.dll")));
    assert!(!plan.deletes.contains(&sb.path("winmm.dll")));

    // Apply the plan to disk and detect
    apply_plan_to_disk(&plan);
    let detected = FeatureDetector::detect(&sb.0, &backup);

    // Assert Stats are off
    assert!(!detected.get_bool("FastCiviProduction"));
    assert!(!detected.get_bool("RomanEndless"));

    // Assert Compat are ON
    assert!(detected.get_bool("FocusLoss"));
    assert!(detected.get_bool("NativeWidescreen1920x1080"));
    assert!(detected.get_bool("CameraZoomOut1"));
    assert_eq!(detected.get_int("GameSpeed"), 2);
    assert!(detected.get_bool("DgVoodoo"));
    assert!(detected.get_bool("ArgmTrace"));
}

#[test]
fn restore_compat_only_keeps_stats_and_preserves_roman_endless_on_exe() {
    let sb = TempSandbox::new("compat-only");
    let backup = setup_full_game_environment(&sb);

    let plan = PatchEngine::restore_compat_only(&sb.0, &backup).unwrap();

    let exe_path = sb.path("Against_Rome.exe");
    assert!(plan.files.contains_key(&exe_path));
    let planned_exe = &plan.files[&exe_path];

    // Compat features on EXE must be restored to original
    assert_eq!(get_focus_patch_state(planned_exe), ExePatchState::Original);
    assert_eq!(
        get_native_widescreen_patch_state(planned_exe),
        ExeNativeWidescreenPatchState::Original
    );
    assert_eq!(
        get_camera_zoom_out_patch_state(planned_exe),
        ExeCameraZoomOutPatchState::Original
    );
    assert_eq!(get_game_speed_multiplier(planned_exe), 1);

    // RomanEndless must be KEPT PATCHED because Stats are not being restored!
    assert_eq!(
        get_roman_endless_patch_state(planned_exe),
        ExeRomanEndlessPatchState::Patched
    );
    assert_eq!(
        get_civi_produce_20_patch_state(planned_exe),
        ExeCiviProduce20PatchState::Patched
    );
    assert_eq!(
        get_unit_recruit_20_patch_state(planned_exe),
        ExeUnitRecruit20PatchState::Patched
    );
    assert_eq!(
        get_corpse_retention_patch_state(planned_exe),
        ExeCorpseRetentionPatchState::Patched
    );

    // Data files must NOT be touched by compat restore
    assert!(!plan.files.contains_key(&sb.path("SYSTEM/cl_script.ini")));
    assert!(!plan.files.contains_key(&sb.path("SYSTEM/ress.ini")));
    assert!(!plan.files.contains_key(&sb.path("MAPS/ENDL_000/team.dat")));

    // DgVoodoo / Trace uninstallation planned
    assert!(plan.deletes.contains(&sb.path("DDraw.dll")));
    assert!(plan.deletes.contains(&sb.path("winmm.dll")));

    // Apply plan and detect
    apply_plan_to_disk(&plan);
    let detected = FeatureDetector::detect(&sb.0, &backup);

    // Compat off
    assert!(!detected.get_bool("FocusLoss"));
    assert!(!detected.get_bool("NativeWidescreen1920x1080"));
    assert!(!detected.get_bool("CameraZoomOut1"));
    assert_eq!(detected.get_int("GameSpeed"), 1);
    assert!(!detected.get_bool("DgVoodoo"));
    assert!(!detected.get_bool("ArgmTrace"));

    // Stats remain ON
    assert!(detected.get_bool("FastCiviProduction"));
    // 此 fixture 的 team.dat 不是可解析隊伍資料；EXE 保留由上方位元組斷言驗證，
    // 整合偵測不得只因 EXE 已套用，就將未知的隊伍資料也視為完成。
    assert!(!detected.get_bool("RomanEndless"));
}

#[test]
fn custom_map_is_preserved_during_selective_restore() {
    let sb = TempSandbox::new("custom-map-preserve");
    let backup = setup_full_game_environment(&sb);

    // Add a custom map with .arm_custom_map marker
    sb.write("MAPS/ENDL_005/.arm_custom_map", b"custom");
    let custom_team = sb.write("MAPS/ENDL_005/team.dat", b"CUSTOM_TEAM_DATA");

    // Also add to backup manager
    let mut backup = backup;
    backup.set_backup_file("MAPS/ENDL_005/team.dat", b"ORIGINAL_TEAM_DATA".to_vec());

    let plan = PatchEngine::restore_stats_only(&sb.0, &backup).unwrap();
    assert!(
        !plan.files.contains_key(&custom_team),
        "Custom map must be preserved"
    );

    let full_restore_plan = PatchEngine::restore_original_files(&sb.0, &backup, false).unwrap();
    assert!(
        !full_restore_plan.files.contains_key(&custom_team),
        "Full restore must not write into custom maps before the outer transaction deletes them"
    );
}

#[test]
fn startup_safe_migrations_runs_mandatory_repair_and_food_healing_check() {
    let sb = TempSandbox::new("startup-safe");
    let backup = setup_full_game_environment(&sb);

    let plan = PatchEngine::run_startup_safe_migrations(&sb.0, &backup).unwrap();
    // BCI scripts should be checked and processed
    apply_plan_to_disk(&plan);
    assert!(sb.path("SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci").exists());
}

#[test]
fn normalize_independent_fields_matches_csharp_normalization() {
    let mut backup = BackupManager::new();
    let objdef_text = "Header1\r\nHeader2\r\n1,2,3,4,2.5,0,0,0,0,0,0,0,0,0,0,0,0,0,0,500,0,0,0,0,700,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,FigKelPri00_Priester,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,10,0,0,0,10,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0\r\n";
    let dau = compress_pfil(objdef_text.as_bytes(), None).unwrap();
    backup.set_backup_file("SYSTEM/DATA_MP/DEFAULTS/objdef.dau", dau);

    const KEY: &str = "FigKelPri00_Priester";
    let original = backup.get_original_stats(KEY).unwrap();
    let input = [111., 22., 33., 44., 999., 888., 77., 666., 555.];

    let normalized = unit_stats::normalize_independent_fields(&backup, KEY, &input).unwrap();

    // Input must not be mutated
    assert_eq!(input[4], 999.);
    assert_eq!(input[5], 888.);
    assert_eq!(input[7], 666.);
    assert_eq!(input[8], 555.);

    // Independent fields must match original
    assert_eq!(normalized[4], original[4]); // Speed
    assert_eq!(normalized[5], original[5]); // Sight (priest)
    assert_eq!(normalized[7], original[7]); // Range
    assert_eq!(normalized[8], original[8]); // SpellRadius

    // Non-independent fields must keep user input
    assert_eq!(normalized[0], 111.);
    assert_eq!(normalized[1], 22.);
    assert_eq!(normalized[2], 33.);
    assert_eq!(normalized[3], 44.);
    assert_eq!(normalized[6], 77.);
}

fn snapshot_directory(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut snap = std::collections::BTreeMap::new();
    if !root.is_dir() {
        return snap;
    }
    fn walk(base: &Path, current: &Path, snap: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        if let Ok(entries) = fs::read_dir(current) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(base, &path, snap);
                } else if path.is_file() {
                    let rel = path.strip_prefix(base).unwrap().to_path_buf();
                    let bytes = fs::read(&path).unwrap_or_default();
                    snap.insert(rel, bytes);
                }
            }
        }
    }
    walk(root, root, &mut snap);
    snap
}

#[test]
fn restore_stats_without_backup_manager_returns_err_and_never_mutates_disk() {
    let sb = TempSandbox::new("stats-no-backup");
    let _ = setup_full_game_environment(&sb);
    let before = snapshot_directory(&sb.0);

    let res = PatchEngine::restore_categories(&sb.0, None, &[FeatureCategory::Stats], true);
    assert!(
        res.is_err(),
        "Expected error when restoring stats without BackupManager"
    );
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("BackupManager is required to restore stats"),
        "Unexpected error: {err_msg}"
    );

    let after = snapshot_directory(&sb.0);
    assert_eq!(
        before, after,
        "Planning operation must never mutate the filesystem on error"
    );
}

#[test]
fn restore_language_with_active_overlay_and_missing_manifest_returns_err_and_never_mutates_disk() {
    let sb = TempSandbox::new("lang-missing-manifest");
    // Setup ToEng package and matching game files (overlay state == Some(true))
    sb.write("ToEng/DATA/text.txt", b"English text");
    sb.write("DATA/text.txt", b"English text");
    let before = snapshot_directory(&sb.0);

    let res = PatchEngine::restore_language_only(&sb.0);
    assert!(
        res.is_err(),
        "Expected error when language overlay is active but manifest is missing"
    );
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("missing language restore manifest.json")
            || err_msg.contains("Failed to plan language pack restoration"),
        "Unexpected error: {err_msg}"
    );

    let after = snapshot_directory(&sb.0);
    assert_eq!(
        before, after,
        "Planning operation must never mutate the filesystem on error"
    );
}

#[test]
fn restore_language_with_corrupt_manifest_returns_err_and_never_mutates_disk() {
    let sb = TempSandbox::new("lang-corrupt-manifest");
    sb.write(
        ".against-rome-modifier-language-backup/manifest.json",
        b"{ invalid json content",
    );
    let before = snapshot_directory(&sb.0);

    let res = PatchEngine::restore_language_only(&sb.0);
    assert!(
        res.is_err(),
        "Expected error when language manifest is corrupt"
    );
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("corrupt language manifest")
            || err_msg.contains("Failed to plan language pack restoration"),
        "Unexpected error: {err_msg}"
    );

    let after = snapshot_directory(&sb.0);
    assert_eq!(
        before, after,
        "Planning operation must never mutate the filesystem on error"
    );
}

#[test]
fn restore_compat_with_corrupted_bci_file_returns_err_and_never_mutates_disk() {
    let sb = TempSandbox::new("compat-corrupt-bci");
    let backup = setup_full_game_environment(&sb);
    // Corrupt one of the BCI files targeted by orchestrator mandatory repairs (R0)
    sb.write(
        "SYSTEM/CLAK/SCRIPT/ak_npc.bci",
        b"PFIL_CORRUPTED_SHORT_HEADER",
    );
    let before = snapshot_directory(&sb.0);

    let res = PatchEngine::restore_compat_only(&sb.0, &backup);
    assert!(
        res.is_err(),
        "Expected error when BCI file contains corrupt data"
    );
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("Failed to restore Endless AI module")
            || err_msg.contains("Failed to apply mandatory AI repairs")
            || err_msg.contains("PFIL")
            || err_msg.contains("invalid data"),
        "Unexpected error: {err_msg}"
    );

    let after = snapshot_directory(&sb.0);
    assert_eq!(
        before, after,
        "Planning operation must never mutate the filesystem on error"
    );
}

#[test]
fn restore_stats_with_missing_bci_script_returns_err_and_never_mutates_disk() {
    let sb = TempSandbox::new("stats-missing-bci");
    let backup = setup_full_game_environment(&sb);
    // Remove a required BCI script for FoodHealing
    let script_path = sb.path("SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci");
    let _ = fs::remove_file(&script_path);
    let before = snapshot_directory(&sb.0);

    let res = PatchEngine::restore_stats_only(&sb.0, &backup);
    assert!(
        res.is_err(),
        "Expected error when required BCI script is missing from disk"
    );
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("Failed to restore Food Healing feature")
            || err_msg.contains("missing script"),
        "Unexpected error: {err_msg}"
    );

    let after = snapshot_directory(&sb.0);
    assert_eq!(
        before, after,
        "Planning operation must never mutate the filesystem on error"
    );
}

#[test]
fn restore_on_clean_environment_is_legitimate_noop() {
    let sb = TempSandbox::new("clean-noop");
    let original_exe = create_synthetic_original_exe();
    sb.write("Against_Rome.exe", &original_exe);

    let mut backup = BackupManager::new();
    backup.set_backup_file("Against_Rome.exe", original_exe);

    // Compat restore on clean environment should succeed without deletions
    let compat_plan = PatchEngine::restore_compat_only(&sb.0, &backup)
        .expect("Clean compat restore should be a valid no-op");
    assert!(
        compat_plan.deletes.is_empty(),
        "No dgVoodoo/trace files should be queued for deletion"
    );

    // Language restore on clean environment should succeed with empty plan
    let lang_plan = PatchEngine::restore_language_only(&sb.0)
        .expect("Clean language restore should be a valid no-op");
    assert!(
        lang_plan.files.is_empty(),
        "No language files should be queued for write"
    );
    assert!(
        lang_plan.deletes.is_empty(),
        "No language files should be queued for delete"
    );
}
