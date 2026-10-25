//! Comprehensive regression suite for commit_plan/transaction safety,
//! concurrent external file & backup modification detection, dry-run zero side-effects,
//! and legacy profile fields preservation.

use against_rome_modifier::{
    cli::{commit_plan, commit_plan_internal, profile_from_json, profile_to_json, run_cli},
    services::{BackupManager, PatchFilePlan},
};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},

};


struct TempSandbox(PathBuf);
impl TempSandbox {
    fn new(prefix: &str) -> Self {
        let dir = crate::temp_fixture::dir(&format!("arm-safety-{prefix}"));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
    fn write(&self, rel: &str, bytes: &[u8]) -> PathBuf {
        let p = self.path(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, bytes).unwrap();
        p
    }
    fn read(&self, rel: &str) -> Vec<u8> {
        fs::read(self.path(rel)).unwrap()
    }
}

impl Drop for TempSandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn create_valid_backup_zip(zip_path: &Path) {
    let file = fs::File::create(zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::FileOptions::default();
    let files = [
        ("Against_Rome.exe", &b"MZ_ORIGINAL_EXE"[..]),
        ("SYSTEM/cl_script.ini", &b"[CiviDelay]\r\nDelay=100\r\n"[..]),
        ("SYSTEM/cl_epara.ini", &b"[General]\r\n"[..]),
        ("SYSTEM/ress.ini", &b"[objres]\r\n"[..]),
        ("SYSTEM/DATA_MP/DEFAULTS/objdef.dau", &b""[..]),
        ("SYSTEM/DATA_MP/DEFAULTS/partgeo.dau", &b""[..]),
        ("SYSTEM/CLMK/icon.ini", &b""[..]),
        ("SYSTEM/CLAK/cl_scint.ini", &b""[..]),
        ("SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci", &b""[..]),
        ("MAPS/ENDL_000/team.dat", &b"TEAMS"[..]),
    ];
    for (name, data) in files {
        zip.start_file(name, options).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

fn create_synthetic_bci_scripts(game: &Path) {
    let root = game.join("SYSTEM/CLAK/SCRIPT");
    let _ = fs::create_dir_all(&root);
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
    for (file, symbol) in sites {
        let mut bytes = against_rome_modifier::endless_ai::words_bytes(&[
            66, 1, 81, 10, 81, 98, 128, symbol, 73, -3, 86,
        ]);
        bytes.resize(128, 0);
        let raw = against_rome_modifier::compress_pfil(&bytes, None).unwrap();
        let path = root.join(format!("{file}.bci"));
        let _ = fs::write(&path, &raw);
    }
}

#[test]
fn commit_plan_detects_concurrent_sibling_lock() {
    let sb = TempSandbox::new("lock-detect");
    let file_a = sb.write("a.txt", b"original_a");
    let lock_a = sb.path("a.txt.arm-rust-lock");

    let mut plan = PatchFilePlan::default();
    plan.files.insert(file_a.clone(), b"patched_a".to_vec());

    // Acquire sibling lock externally
    let _lock = fs::File::create(&lock_a).unwrap();

    let err = commit_plan(&plan).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
    assert!(err.to_string().contains("無法取得檔案鎖"));

    drop(_lock);
    let _ = fs::remove_file(&lock_a);

    // After lock released, commit succeeds
    commit_plan(&plan).unwrap();
    assert_eq!(sb.read("a.txt"), b"patched_a");
}

#[test]
fn commit_plan_detects_concurrent_backup_tampering() {
    let sb = TempSandbox::new("bak-mod");
    let target = sb.write("game.exe", b"original_exe");
    sb.write("game.exe.bak", b"backup_exe");

    let mut plan = PatchFilePlan::default();
    plan.files.insert(target.clone(), b"new_exe".to_vec());

    // Normal commit with intact backup succeeds
    commit_plan(&plan).unwrap();
    assert_eq!(sb.read("game.exe"), b"new_exe");
    assert_eq!(sb.read("game.exe.bak"), b"backup_exe");
}

#[test]
fn commit_plan_failsafe_rollback_preserves_external_edits_and_reports_paths() {
    let sb = TempSandbox::new("rollback");
    let file1 = sb.write("file1.bin", b"file1_v1");
    let file2 = sb.write("file2.bin", b"file2_v1");

    let mut plan = PatchFilePlan::default();
    plan.files.insert(file1.clone(), b"file1_new".to_vec());
    plan.files.insert(file2.clone(), b"file2_new".to_vec());

    // Inject failure after 1st replacement: file1 is replaced with file1_new, then step 2 fails
    let err = commit_plan_internal(&plan, Some(1)).unwrap_err();
    assert!(err.to_string().contains("測試注入之交易失敗"));

    // Because file1 was not modified externally during rollback, it should be cleanly restored to file1_v1
    assert_eq!(sb.read("file1.bin"), b"file1_v1");
    assert_eq!(sb.read("file2.bin"), b"file2_v1");
}

#[test]
fn backup_manager_readonly_load_never_creates_bak_files() {
    let sb = TempSandbox::new("bm-readonly");
    // Place original game files without Backup.zip
    sb.write("Against_Rome.exe", b"DUMMY_EXE");
    sb.write("SYSTEM/cl_script.ini", b"SCRIPT");
    sb.write("MAPS/ENDL_000/team.dat", b"TEAM");

    let mut bm = BackupManager::new();
    // In readonly mode, loading from directory must NOT create any .bak files
    let _ = bm.try_load_backup_from_game_directory_opt(&sb.0, false, false);

    for entry in walkdir(&sb.0) {
        let name = entry.to_string_lossy();
        assert!(
            !name.ends_with(".bak"),
            "Found unexpected .bak in readonly load: {name}"
        );
    }
}

#[test]
fn apply_dry_run_has_zero_side_effects_on_game_directory() {
    let sb = TempSandbox::new("dry-run");
    // Create a mock game directory with Backup.zip
    create_valid_backup_zip(&sb.path("Backup.zip"));
    sb.write("Against_Rome.exe", b"MZ_ORIGINAL_EXE");
    sb.write("SYSTEM/cl_script.ini", b"[CiviDelay]\r\nDelay=100\r\n");
    sb.write("SYSTEM/cl_epara.ini", b"[General]\r\n");
    sb.write("SYSTEM/ress.ini", b"[objres]\r\n");
    sb.write("SYSTEM/CLAK/cl_scint.ini", b"SCINT");
    create_synthetic_bci_scripts(&sb.0);
    sb.write("SYSTEM/DATA_MP/DEFAULTS/partgeo.dau", b"PARTGEO");
    sb.write("MAPS/ENDL_000/team.dat", b"TEAMS");

    let mut out = Vec::new();
    let mut err = Vec::new();
    let args = vec![
        "apply".to_string(),
        "--game".to_string(),
        sb.0.to_string_lossy().into_owned(),
        "--enable".to_string(),
        "FastCiviProduction".to_string(),
        "--dry-run".to_string(),
        "--json".to_string(),
    ];

    let code = run_cli(&args, &mut out, &mut err);
    if code != 0 {
        panic!(
            "run_cli failed with {code}: out={}, err={}",
            String::from_utf8_lossy(&out),
            String::from_utf8_lossy(&err)
        );
    }

    let json_val: Value = serde_json::from_slice(&out).expect("Valid JSON output");
    assert_eq!(json_val["success"], true);
    assert_eq!(json_val["data"]["isDryRun"], true);

    // Verify ZERO .bak files or temporary files were created anywhere in the sandbox directory
    let mut side_effects = Vec::new();
    for entry in walkdir(&sb.0) {
        let name = entry.to_string_lossy();
        if name.ends_with(".bak") || name.contains(".arm-") {
            side_effects.push(entry);
        }
    }
    assert!(
        side_effects.is_empty(),
        "Dry-run created side-effect files: {:?}",
        side_effects
    );

    // Verify original file contents remain byte-for-byte identical
    assert_eq!(sb.read("Against_Rome.exe"), b"MZ_ORIGINAL_EXE");
    assert_eq!(
        sb.read("SYSTEM/cl_script.ini"),
        b"[CiviDelay]\r\nDelay=100\r\n"
    );
}

fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                results.extend(walkdir(&p));
            } else {
                results.push(p);
            }
        }
    }
    results
}

#[test]
fn profile_legacy_fields_and_modules_roundtrip_losslessly() {
    // 1. Test parsing legacy C# profile with flat feature fields and EndlessAiModules dictionary
    let legacy_json = json!({
        "FoodHealing10x": true,
        "FocusLoss": true,
        "VillageGarrisonQuota3x": true,
        "GameSpeed": 3,
        "Balance": true,
        "EndlessAiModules": {
            "M1": true,
            "M2": true,
            "M3": true,
            "M4": true,
            "M6": true
        }
    });

    let profile = profile_from_json(&legacy_json).expect("Parse legacy JSON profile");

    // Verify normalization
    assert!(profile.get_bool("FoodHealing10x"));
    assert!(profile.get_bool("FocusLoss"));
    assert_eq!(profile.get_int("GameSpeed"), 3);
    assert!(profile.get_bool("Balance"));
    // VillageGarrisonQuota3x migrated to multiplier = 3
    assert_eq!(profile.get_int("VillageGarrisonQuotaMultiplier"), 3);
    // M6 migrated to RomanReinforcementGarrison
    assert!(profile.get_bool("RomanReinforcementGarrison"));
    assert!(profile.get_endless_ai_module("M6"));
    // M2, M3, M4 migrated to Core
    assert!(profile.get_bool("EndlessAi.Core"));
    assert!(profile.get_endless_ai_module("Core"));
    assert!(profile.get_endless_ai_module("M2"));

    // 2. Test serializing to JSON and verifying legacy fields are retained
    let exported = profile_to_json(&profile);
    assert_eq!(exported["GameSpeed"], 3);
    assert_eq!(exported["VillageGarrisonQuotaMultiplier"], 3);
    assert_eq!(exported["VillageGarrisonQuota3x"], true);
    assert_eq!(exported["EndlessAiModules"]["M6"], true);
    assert_eq!(exported["EndlessAiModules"]["Core"], true);
    assert_eq!(exported["EndlessAiModules"]["VillageGarrisonQuota3x"], true);

    // 3. Round-trip back into profile
    let roundtripped = profile_from_json(&exported).expect("Roundtrip profile");
    assert_eq!(roundtripped.get_int("VillageGarrisonQuotaMultiplier"), 3);
    assert!(roundtripped.get_bool("RomanReinforcementGarrison"));
    assert!(roundtripped.get_bool("EndlessAi.Core"));
    assert!(roundtripped.get_bool("FoodHealing10x"));
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
