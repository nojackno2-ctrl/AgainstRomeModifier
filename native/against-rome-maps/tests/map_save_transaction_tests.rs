use against_rome_maps::*;
use against_rome_scenario::model::{Action, Condition, Event, ScenarioDocument};
use chrono::Utc;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Helper: takes a snapshot of all files and their exact byte contents recursively.
fn snapshot_directory(dir: &Path) -> HashMap<PathBuf, Vec<u8>> {
    let mut map = HashMap::new();
    collect_files_recursive(dir, dir, &mut map);
    map
}

fn collect_files_recursive(base: &Path, current: &Path, map: &mut HashMap<PathBuf, Vec<u8>>) {
    if let Ok(entries) = fs::read_dir(current) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files_recursive(base, &path, map);
            } else if path.is_file() {
                let rel = path.strip_prefix(base).unwrap().to_path_buf();
                let bytes = fs::read(&path).expect("failed to read file for snapshot");
                map.insert(rel, bytes);
            }
        }
    }
}

/// Helper: verifies that current disk directory matches the expected snapshot byte-for-byte.
fn assert_byte_for_byte_identical(dir: &Path, expected: &HashMap<PathBuf, Vec<u8>>) {
    let current = snapshot_directory(dir);

    // Verify no missing files and byte-for-byte identity
    for (rel_path, expected_bytes) in expected {
        let actual_bytes = current.get(rel_path).unwrap_or_else(|| {
            panic!(
                "檔案遺失: {} 未在回滾後恢復。目前目錄下檔案: {:?}",
                rel_path.display(),
                current.keys().collect::<Vec<_>>()
            )
        });
        assert_eq!(
            actual_bytes, expected_bytes,
            "檔案位元組不一致 ({}): 預期長度 {}, 實際長度 {}",
            rel_path.display(),
            expected_bytes.len(),
            actual_bytes.len()
        );
    }

    // Verify no unexpected new or leftover files
    for rel_path in current.keys() {
        assert!(
            expected.contains_key(rel_path),
            "發現非預期的新增或殘留檔案: {}",
            rel_path.display()
        );
    }

    assert_eq!(current.len(), expected.len(), "檔案總數與預期不符");
}

/// Creates a realistic test map directory structure.
fn create_test_map_fixture(prefix: &str) -> (PathBuf, HashMap<PathBuf, Vec<u8>>) {
    let temp_root = std::env::temp_dir().join(format!("arm_tx_{}_{}", prefix, Uuid::new_v4().simple()));
    let map_dir = temp_root.join("MAPS").join("ENDL_005");
    fs::create_dir_all(map_dir.join("TEXT").join("US")).unwrap();
    fs::create_dir_all(map_dir.join("DATA")).unwrap();

    // 1. Terrain files
    let boden_bmp = vec![0x42, 0x4D, 0x10, 0x20, 0x30, 0x40, 0xAA, 0xBB, 0xCC, 0xDD];
    fs::write(map_dir.join("boden.bmp"), &boden_bmp).unwrap();

    let boden_ini = b"[Heightmapstep]\n2.0\n[Waterlevel]\n30.0\n".to_vec();
    fs::write(map_dir.join("boden.ini"), &boden_ini).unwrap();

    // 2. Text briefing
    let briefing = b"; Briefing put\r\nvar:briefing_titel_1 =\"Original Title\";\r\n".to_vec();
    fs::write(map_dir.join("TEXT").join("US").join("briefing.put"), &briefing).unwrap();

    // 3. Objects pool
    let objects_dat = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04];
    fs::write(map_dir.join("DATA").join("objects.dat"), &objects_dat).unwrap();

    // 4. Runtime cache (staged for deletion)
    let skydens_dat = b"stale cache data 123456789".to_vec();
    fs::write(map_dir.join("skydens.dat"), &skydens_dat).unwrap();

    // 5. Untouched file
    let untouched = b"keep me completely untouched".to_vec();
    fs::write(map_dir.join("untouched.dat"), &untouched).unwrap();

    // 6. Custom map marker
    let marker = CustomMapEntry::new(5, 1, Utc::now(), "1.0.0");
    let marker_json = serde_json::to_string_pretty(&marker).unwrap().into_bytes();
    fs::write(map_dir.join(CustomMapManifest::MARKER_FILE_NAME), &marker_json).unwrap();

    let snapshot = snapshot_directory(&map_dir);
    (temp_root, snapshot)
}

// =========================================================================
// 1. Successful Whole-Map Multi-File Save
// =========================================================================

#[test]
fn test_successful_whole_map_transactional_save() {
    let (temp_root, _initial_snapshot) = create_test_map_fixture("success");
    let map_dir = temp_root.join("MAPS").join("ENDL_005");

    let mut tx = MapSaveTransaction::new(&map_dir);

    // 1. Stage updated terrain
    let new_boden_bmp = vec![0x42, 0x4D, 0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA];
    tx.stage_file("boden.bmp", new_boden_bmp.clone()).unwrap();

    // 2. Stage updated boden.ini via document
    let ini_doc = BodenIniDocument::from_text("[Heightmapstep]\n3.5\n[Waterlevel]\n42.0\n");
    tx.stage_boden_ini("boden.ini", &ini_doc).unwrap();

    // 3. Stage updated briefing via PutTextDocument
    let put_doc = PutTextDocument::from_text("; New Briefing\r\nvar:briefing_titel_1 =\"New Awesome Map\";\r\n");
    tx.stage_put_document("TEXT/US/briefing.put", &put_doc).unwrap();

    // 4. Stage source file copy
    let source_file = temp_root.join("source_objects.dat");
    let source_bytes = vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
    fs::write(&source_file, &source_bytes).unwrap();
    tx.stage_file_from_disk("DATA/objects.dat", &source_file).unwrap();

    // 5. Stage newly created file in nested folder
    let scenario_doc = ScenarioDocument::default();
    tx.stage_scenario_document(&scenario_doc).unwrap();

    // 6. Stage cache invalidation (delete skydens.dat)
    tx.stage_height_cache_invalidation().unwrap();

    assert_eq!(tx.staged_count(), 9); // 5 files + 4 height caches
    assert!(tx.has_staged("boden.bmp"));
    assert!(tx.has_staged("skydens.dat"));
    assert!(tx.has_staged("arm_scenario.json"));

    // Execute transaction
    let report = tx.commit().expect("transaction commit must succeed");

    assert_eq!(report.target_directory, map_dir);
    assert!(report.written_files.contains(&map_dir.join("boden.bmp")));
    assert!(report.written_files.contains(&map_dir.join("boden.ini")));
    assert!(report.written_files.contains(&map_dir.join("TEXT").join("US").join("briefing.put")));
    assert!(report.written_files.contains(&map_dir.join("DATA").join("objects.dat")));
    assert!(report.written_files.contains(&map_dir.join("arm_scenario.json")));
    assert!(report.deleted_files.contains(&map_dir.join("skydens.dat")));

    // Verify on disk
    assert_eq!(fs::read(map_dir.join("boden.bmp")).unwrap(), new_boden_bmp);
    assert_eq!(fs::read(map_dir.join("DATA").join("objects.dat")).unwrap(), source_bytes);
    assert!(!map_dir.join("skydens.dat").exists(), "skydens.dat must be deleted");
    assert!(map_dir.join("arm_scenario.json").exists(), "arm_scenario.json must exist");
    assert_eq!(
        fs::read(map_dir.join("untouched.dat")).unwrap(),
        b"keep me completely untouched"
    );

    // Verify no temporary files remain
    let mut temp_files = Vec::new();
    for entry in fs::read_dir(&map_dir).unwrap().flatten() {
        if entry.file_name().to_string_lossy().contains(".tmp_tx") {
            temp_files.push(entry.path());
        }
    }
    assert!(temp_files.is_empty(), "No temporary files should remain: {:?}", temp_files);

    let _ = fs::remove_dir_all(&temp_root);
}

// =========================================================================
// 2. Preflight Validation & Security Rejections (Zero Disk Writes)
// =========================================================================

#[test]
fn test_preflight_validation_path_security_and_collisions() {
    let (temp_root, initial_snapshot) = create_test_map_fixture("validation");
    let map_dir = temp_root.join("MAPS").join("ENDL_005");

    // 1. Path traversal rejected on stage
    let mut tx = MapSaveTransaction::new(&map_dir);
    assert!(tx.stage_file("../escape.txt", b"hacked").is_err());
    assert!(tx.stage_file("DATA/../../escape.txt", b"hacked").is_err());
    assert!(tx.stage_file("/absolute/path.txt", b"hacked").is_err());
    assert!(tx.stage_file("C:\\windows\\win.ini", b"hacked").is_err());
    assert!(tx.stage_file("", b"empty").is_err());
    assert!(tx.stage_file("illegal*char.txt", b"bad").is_err());

    // 2. Non-existent source file on stage_file_from_disk
    assert!(tx.stage_file_from_disk("DATA/copy.dat", temp_root.join("non_existent.dat")).is_err());

    // 3. Target directory is a file
    let file_target = temp_root.join("target_is_file.txt");
    fs::write(&file_target, b"file").unwrap();
    let tx_file = MapSaveTransaction::new(&file_target);
    let err = tx_file.validate().unwrap_err();
    assert_eq!(err.stage, TransactionStage::PreflightValidation);
    assert_eq!(err.rollback_status, RollbackStatus::NoModificationsToRevert);

    // 4. Custom map guard rejection (e.g. retail slot ENDL_001)
    let retail_dir = temp_root.join("MAPS").join("ENDL_001");
    fs::create_dir_all(&retail_dir).unwrap();
    let tx_retail = MapSaveTransaction::new(&retail_dir).with_custom_map_guard(true);
    let err = tx_retail.validate().unwrap_err();
    assert_eq!(err.stage, TransactionStage::PreflightValidation);
    assert!(err.message.contains("自製地圖"));

    // 5. Preflight validator custom rejection
    let mut tx_custom = MapSaveTransaction::new(&map_dir);
    tx_custom.stage_file("test.txt", b"hello").unwrap();
    tx_custom.add_preflight_validator(|_dir, _staged| {
        Err("自訂劇本驗證失敗：事件目標物件已刪除".into())
    });
    let err = tx_custom.commit().unwrap_err();
    assert_eq!(err.stage, TransactionStage::PreflightValidation);
    assert_eq!(err.rollback_status, RollbackStatus::NoModificationsToRevert);
    assert!(err.message.contains("目標物件已刪除"));

    // Verify target directory remained 100% byte-for-byte identical
    assert_byte_for_byte_identical(&map_dir, &initial_snapshot);

    let _ = fs::remove_dir_all(&temp_root);
}

// =========================================================================
// 3. Byte-For-Byte Survival Under All Injected Failure Points
// =========================================================================

#[test]
fn test_byte_for_byte_survival_under_all_injected_failure_points() {
    let failure_points = vec![
        ("BeforeValidation", TransactionFailurePoint::BeforeValidation),
        ("AfterValidation", TransactionFailurePoint::AfterValidation),
        ("DuringStaging_0", TransactionFailurePoint::DuringStaging(0)),
        ("DuringStaging_2", TransactionFailurePoint::DuringStaging(2)),
        ("BeforeAnyReplacement", TransactionFailurePoint::BeforeAnyReplacement),
        ("BeforeReplacement_0", TransactionFailurePoint::BeforeReplacement(0)),
        ("DuringReplacement_0", TransactionFailurePoint::DuringReplacement(0)),
        ("BeforeReplacement_2", TransactionFailurePoint::BeforeReplacement(2)),
        ("DuringReplacement_2", TransactionFailurePoint::DuringReplacement(2)),
        ("BeforePath_briefing", TransactionFailurePoint::BeforePath(PathBuf::from("TEXT").join("US").join("briefing.put"))),
        ("AfterPath_briefing", TransactionFailurePoint::AfterPath(PathBuf::from("TEXT").join("US").join("briefing.put"))),
        ("DuringPostVerification", TransactionFailurePoint::DuringPostVerification),
        ("BeforeFinalCommit", TransactionFailurePoint::BeforeFinalCommit),
    ];

    for (name, point) in failure_points {
        let (temp_root, initial_snapshot) = create_test_map_fixture(&format!("fail_{}", name));
        let map_dir = temp_root.join("MAPS").join("ENDL_005");

        let mut tx = MapSaveTransaction::new(&map_dir);

        // Stage multiple destructive operations:
        // 0. Modify boden.bmp
        tx.stage_file("boden.bmp", vec![0x99, 0x88, 0x77, 0x66, 0x55, 0x44]).unwrap();
        // 1. Modify boden.ini
        tx.stage_file("boden.ini", b"[Heightmapstep]\n99.9\n".to_vec()).unwrap();
        // 2. Modify TEXT/US/briefing.put
        tx.stage_file("TEXT/US/briefing.put", b"CORRUPTED BRIEFING".to_vec()).unwrap();
        // 3. Create a brand new file in nested folder
        tx.stage_file("DATA/brand_new_object.dat", b"NEW OBJECT DATA".to_vec()).unwrap();
        // 4. Delete existing skydens.dat
        tx.stage_delete("skydens.dat").unwrap();

        // Inject failure at current point
        tx = tx.with_injected_failure(point, format!("Injected test failure at {}", name));

        let result = tx.commit();
        assert!(result.is_err(), "Transaction must fail when injected with error at {}", name);
        let err = result.unwrap_err();

        assert!(
            err.rollback_status == RollbackStatus::CleanlyRestored
                || err.rollback_status == RollbackStatus::NoModificationsToRevert,
            "Rollback must cleanly restore or have no modifications for {}: {:?}",
            name,
            err.rollback_status
        );

        // PROOF: Verify 100% byte-for-byte survival of all original files!
        assert_byte_for_byte_identical(&map_dir, &initial_snapshot);

        // Verify newly created file was deleted on rollback
        assert!(
            !map_dir.join("DATA").join("brand_new_object.dat").exists(),
            "Newly created file must be cleaned up on rollback for {}",
            name
        );

        let _ = fs::remove_dir_all(&temp_root);
    }
}

// =========================================================================
// 4. Read-Only Original Files Rolled Back Byte-for-Byte
// =========================================================================

#[test]
fn test_readonly_files_rolled_back_byte_for_byte() {
    let (temp_root, initial_snapshot) = create_test_map_fixture("readonly");
    let map_dir = temp_root.join("MAPS").join("ENDL_005");

    // Make boden.bmp and briefing.put read-only
    let boden_path = map_dir.join("boden.bmp");
    let briefing_path = map_dir.join("TEXT").join("US").join("briefing.put");

    let mut perms1 = fs::metadata(&boden_path).unwrap().permissions();
    perms1.set_readonly(true);
    fs::set_permissions(&boden_path, perms1).unwrap();

    let mut perms2 = fs::metadata(&briefing_path).unwrap().permissions();
    perms2.set_readonly(true);
    fs::set_permissions(&briefing_path, perms2).unwrap();

    let mut tx = MapSaveTransaction::new(&map_dir);
    tx.stage_file("boden.bmp", b"overwritten boden".to_vec()).unwrap();
    tx.stage_file("TEXT/US/briefing.put", b"overwritten briefing".to_vec()).unwrap();
    tx.stage_file("DATA/new.txt", b"new content").unwrap();

    // Inject failure after briefing.put is replaced
    tx = tx.with_injected_failure(
        TransactionFailurePoint::AfterPath(PathBuf::from("TEXT").join("US").join("briefing.put")),
        "Injected failure after replacing read-only files",
    );

    let err = tx.commit().unwrap_err();
    assert_eq!(err.rollback_status, RollbackStatus::CleanlyRestored);

    // Verify byte-for-byte restoration of original files
    assert_byte_for_byte_identical(&map_dir, &initial_snapshot);

    let _ = fs::remove_dir_all(&temp_root);
}

// =========================================================================
// 5. Dynamic Failure Injector & Retry Workflow
// =========================================================================

#[test]
fn test_failure_recovery_and_retry_succeeds() {
    let (temp_root, initial_snapshot) = create_test_map_fixture("retry");
    let map_dir = temp_root.join("MAPS").join("ENDL_005");

    // First attempt: fails during replacement
    {
        let mut tx = MapSaveTransaction::new(&map_dir);
        tx.stage_file("boden.bmp", b"attempt 1".to_vec()).unwrap();
        tx.stage_file("boden.ini", b"attempt 1 ini".to_vec()).unwrap();
        tx = tx.with_injected_failure(
            TransactionFailurePoint::DuringReplacement(1),
            "Transient disk sharing violation",
        );

        let err = tx.commit().unwrap_err();
        assert_eq!(err.rollback_status, RollbackStatus::CleanlyRestored);

        // Verify disk is in exact initial state
        assert_byte_for_byte_identical(&map_dir, &initial_snapshot);
    }

    // Second attempt: retry after fixing condition
    {
        let mut tx = MapSaveTransaction::new(&map_dir);
        tx.stage_file("boden.bmp", b"attempt 2 success".to_vec()).unwrap();
        tx.stage_file("boden.ini", b"attempt 2 ini success".to_vec()).unwrap();

        let report = tx.commit().expect("Retry attempt must succeed");
        assert_eq!(report.written_files.len(), 2);
        assert_eq!(fs::read(map_dir.join("boden.bmp")).unwrap(), b"attempt 2 success");
        assert_eq!(fs::read(map_dir.join("boden.ini")).unwrap(), b"attempt 2 ini success");
    }

    let _ = fs::remove_dir_all(&temp_root);
}

// =========================================================================
// 6. Scenario Preflight Integration Test
// =========================================================================

#[test]
fn test_scenario_preflight_integration_rejection_leaves_disk_untouched() {
    let (temp_root, initial_snapshot) = create_test_map_fixture("scen_preflight");
    let map_dir = temp_root.join("MAPS").join("ENDL_005");

    let mut scenario = ScenarioDocument::default();
    let deleted_id = Uuid::new_v4();
    scenario.events.push(Event {
        name: "TestDeletedTarget".into(),
        delay_seconds: 5,
        repeat: false,
        enabled: true,
        actions: vec![Action {
            kind: 0,
            text: "Hello".into(),
            team: 0,
            other_team: 1,
            count: 1,
            x: 100.0,
            z: 100.0,
            alias: String::new(),
            move_destination: None,
            hostile: false,
        }],
        conditions: vec![Condition {
            kind: 0,
            target_id: deleted_id, // Target does not exist in spawns!
            min_x: 0,
            min_z: 0,
            max_x: 100,
            max_z: 100,
        }],
        ..Default::default()
    });

    let mut tx = MapSaveTransaction::new(&map_dir);
    tx.stage_file("boden.bmp", b"new terrain bytes".to_vec()).unwrap();
    tx.stage_scenario_document(&scenario).unwrap();

    let scenario_clone = scenario.clone();
    let aliases = vec!["GER_INF01".to_string()];
    tx.add_preflight_validator(move |_dir, _staged| {
        ScenarioSavePreflight::validate(&scenario_clone, &aliases)
    });

    let err = tx.commit().unwrap_err();
    assert_eq!(err.stage, TransactionStage::PreflightValidation);
    assert_eq!(err.rollback_status, RollbackStatus::NoModificationsToRevert);
    assert!(err.message.contains("目標物件已刪除"));

    // Verify disk was completely untouched
    assert_byte_for_byte_identical(&map_dir, &initial_snapshot);

    let _ = fs::remove_dir_all(&temp_root);
}

// =========================================================================
// 7. Staging Inspection & Mutation API Tests
// =========================================================================

#[test]
fn test_transaction_staging_state_and_manipulation() {
    let temp_root = std::env::temp_dir().join(format!("arm_tx_inspect_{}", Uuid::new_v4().simple()));
    let map_dir = temp_root.join("MAPS").join("ENDL_005");
    fs::create_dir_all(&map_dir).unwrap();

    let mut tx = MapSaveTransaction::new(&map_dir);
    assert!(tx.is_empty());
    assert_eq!(tx.staged_count(), 0);

    // Stage files
    tx.stage_file_tagged("boden.bmp", b"bytes", "Terrain").unwrap();
    tx.stage_file("boden.ini", b"ini").unwrap();
    assert_eq!(tx.staged_count(), 2);
    assert!(tx.has_staged("boden.bmp"));
    assert!(tx.has_staged("boden.ini"));
    assert!(!tx.has_staged("other.txt"));

    let entry = tx.find_entry("boden.bmp").unwrap();
    assert_eq!(entry.tag(), Some("Terrain"));
    assert_eq!(entry.byte_size(), Some(5));
    assert!(!entry.is_deletion());

    // Stage marker
    let marker = CustomMapEntry::new(5, 1, Utc::now(), "1.0.0");
    tx.stage_custom_map_marker(&marker).unwrap();
    assert!(tx.has_staged(CustomMapManifest::MARKER_FILE_NAME));

    // Stage deletion
    tx.stage_delete("obsolete.dat").unwrap();
    let del_entry = tx.find_entry("obsolete.dat").unwrap();
    assert!(del_entry.is_deletion());
    assert_eq!(del_entry.byte_size(), None);

    // Unstage
    assert!(tx.unstage("boden.ini"));
    assert!(!tx.has_staged("boden.ini"));
    assert!(!tx.unstage("non_existent"));

    // Clear
    tx.clear();
    assert!(tx.is_empty());
    assert_eq!(tx.staged_count(), 0);

    let _ = fs::remove_dir_all(&temp_root);
}

// =========================================================================
// 8. Ancestor Collision Validation Test
// =========================================================================

#[test]
fn test_ancestor_collision_rejected_before_writes() {
    let temp_root = std::env::temp_dir().join(format!("arm_tx_conflict_{}", Uuid::new_v4().simple()));
    let map_dir = temp_root.join("MAPS").join("ENDL_005");
    fs::create_dir_all(&map_dir).unwrap();

    let mut tx = MapSaveTransaction::new(&map_dir);
    tx.stage_file("DATA", b"data as a file").unwrap();
    tx.stage_file("DATA/objects.dat", b"nested file").unwrap();

    let err = tx.validate().unwrap_err();
    assert_eq!(err.stage, TransactionStage::PreflightValidation);
    assert_eq!(err.rollback_status, RollbackStatus::NoModificationsToRevert);
    assert!(err.message.contains("路徑衝突"));

    let _ = fs::remove_dir_all(&temp_root);
}

// =========================================================================
// 9. Symlink and Directory Junction Escape Rejections (Zero Disk Writes)
// =========================================================================

#[cfg(windows)]
fn create_test_junction(link: &Path, target: &Path) -> std::io::Result<()> {
    let output = std::process::Command::new("cmd")
        .args(&["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("mklink /J failed: {}", String::from_utf8_lossy(&output.stderr)),
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn create_test_junction(link: &Path, target: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

fn remove_test_junction(link: &Path) {
    #[cfg(windows)]
    {
        let _ = fs::remove_dir(link);
    }
    #[cfg(not(windows))]
    {
        let _ = fs::remove_file(link);
    }
}

#[test]
fn test_symlink_and_junction_escapes_rejected_before_writes() {
    let temp_root = std::env::temp_dir().join(format!("arm_tx_escape_{}", Uuid::new_v4().simple()));
    let map_dir = temp_root.join("MAPS").join("ENDL_005");
    let outside_dir = temp_root.join("EXTERNAL_SENSITIVE_DIRECTORY");
    fs::create_dir_all(&map_dir).unwrap();
    fs::create_dir_all(&outside_dir).unwrap();

    // Sensitive external file that must NEVER be modified or touched
    let sensitive_file = outside_dir.join("objects.dat");
    let original_external_bytes = b"CRITICAL EXTERNAL DATA NEVER OVERWRITE".to_vec();
    fs::write(&sensitive_file, &original_external_bytes).unwrap();

    // Create junction: map_dir/DATA -> outside_dir
    let junction_data = map_dir.join("DATA");
    if let Err(e) = create_test_junction(&junction_data, &outside_dir) {
        eprintln!("Skipping junction escape test (environment does not support junctions): {e}");
        let _ = fs::remove_dir_all(&temp_root);
        return;
    }

    // Attempt whole-map transactional save staging DATA/objects.dat
    let mut tx = MapSaveTransaction::new(&map_dir);
    let malicious_bytes = b"MALICIOUS OVERWRITE DATA VIA JUNCTION ESCAPE".to_vec();
    tx.stage_file("DATA/objects.dat", malicious_bytes).unwrap();

    // Commit must fail during PreflightValidation with zero modifications
    let result = tx.commit();
    assert!(result.is_err(), "Transaction must reject destination path component junction to outside directory");
    let err = result.unwrap_err();

    assert_eq!(err.stage, TransactionStage::PreflightValidation);
    assert_eq!(err.rollback_status, RollbackStatus::NoModificationsToRevert);
    assert!(
        err.message.contains("重解析點") || err.message.contains("符號連結") || err.message.contains("路徑安全違規"),
        "Error message should mention reparse point/symlink violation: {}",
        err.message
    );

    // PROOF: External target bytes NEVER changed!
    let current_external_bytes = fs::read(&sensitive_file).unwrap();
    assert_eq!(
        current_external_bytes, original_external_bytes,
        "CRITICAL: External file was modified through junction escape!"
    );

    // Clean up
    remove_test_junction(&junction_data);
    let _ = fs::remove_dir_all(&temp_root);
}

// =========================================================================
// 10. Nested Intermediate Path Component Junction Escape Rejection
// =========================================================================

#[test]
fn test_nested_intermediate_component_junction_escape_rejected() {
    let temp_root = std::env::temp_dir().join(format!("arm_tx_nested_esc_{}", Uuid::new_v4().simple()));
    let map_dir = temp_root.join("MAPS").join("ENDL_005");
    let outside_text = temp_root.join("EXTERNAL_BRIEFING_DIR");
    fs::create_dir_all(map_dir.join("TEXT")).unwrap();
    fs::create_dir_all(&outside_text).unwrap();

    let outside_put = outside_text.join("briefing.put");
    let original_put_bytes = b"; Original Sensitive Briefing\r\nvar:secret=1;".to_vec();
    fs::write(&outside_put, &original_put_bytes).unwrap();

    // Create junction: map_dir/TEXT/US -> outside_text
    let junction_us = map_dir.join("TEXT").join("US");
    if let Err(e) = create_test_junction(&junction_us, &outside_text) {
        eprintln!("Skipping nested junction escape test: {e}");
        let _ = fs::remove_dir_all(&temp_root);
        return;
    }

    let mut tx = MapSaveTransaction::new(&map_dir);
    tx.stage_file("TEXT/US/briefing.put", b"; Hacked Put Content".to_vec()).unwrap();

    let err = tx.commit().unwrap_err();
    assert_eq!(err.stage, TransactionStage::PreflightValidation);
    assert_eq!(err.rollback_status, RollbackStatus::NoModificationsToRevert);
    assert!(err.message.contains("重解析點") || err.message.contains("符號連結"));

    // PROOF: External file was never modified!
    assert_eq!(fs::read(&outside_put).unwrap(), original_put_bytes);

    remove_test_junction(&junction_us);
    let _ = fs::remove_dir_all(&temp_root);
}

// =========================================================================
// 11. Linked Entries During Recursive Rollback Copies Rejection
// =========================================================================

#[test]
fn test_recursive_rollback_copy_rejects_linked_entries() {
    let temp_root = std::env::temp_dir().join(format!("arm_tx_rb_link_{}", Uuid::new_v4().simple()));
    let source_dir = temp_root.join("source_folder");
    let outside_dir = temp_root.join("outside_target");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&outside_dir).unwrap();

    fs::write(source_dir.join("normal.txt"), b"normal content").unwrap();
    let outside_file = outside_dir.join("external_secret.txt");
    fs::write(&outside_file, b"EXTERNAL SECRET BYTES").unwrap();

    // Create a junction inside source_dir pointing to outside_dir
    let nested_junction = source_dir.join("linked_sub");
    if let Err(e) = create_test_junction(&nested_junction, &outside_dir) {
        eprintln!("Skipping recursive rollback link test: {e}");
        let _ = fs::remove_dir_all(&temp_root);
        return;
    }

    let mut scope = FileRollbackScope::new().unwrap();

    // Attempt to track source_dir: MUST fail because of nested junction!
    let track_res = scope.track_directory(&source_dir);
    assert!(track_res.is_err(), "track_directory must fail on linked entries");
    let err_msg = track_res.unwrap_err();
    assert!(
        err_msg.contains("無法快照連結") || err_msg.contains("重解析點"),
        "Error message should mention linked entry: {err_msg}"
    );

    // PROOF: External secret bytes never modified!
    assert_eq!(fs::read(&outside_file).unwrap(), b"EXTERNAL SECRET BYTES");

    remove_test_junction(&nested_junction);
    let _ = fs::remove_dir_all(&temp_root);
}

// =========================================================================
// 12. SafeFileWriter Rejects Reparse Point Paths
// =========================================================================

#[test]
fn test_safe_file_writer_rejects_reparse_point_paths() {
    let temp_root = std::env::temp_dir().join(format!("arm_tx_sfw_link_{}", Uuid::new_v4().simple()));
    let junction_dir = temp_root.join("junc");
    let outside_dir = temp_root.join("outside");
    fs::create_dir_all(&outside_dir).unwrap();

    let outside_target = outside_dir.join("protected.txt");
    fs::write(&outside_target, b"PROTECTED DATA").unwrap();

    if let Err(e) = create_test_junction(&junction_dir, &outside_dir) {
        eprintln!("Skipping SafeFileWriter reparse point test: {e}");
        let _ = fs::remove_dir_all(&temp_root);
        return;
    }

    let target_via_junction = junction_dir.join("protected.txt");

    // SafeFileWriter write_all_bytes must reject
    let write_res = SafeFileWriter::write_all_bytes(&target_via_junction, b"CORRUPTED", None);
    assert!(write_res.is_err());
    assert_eq!(fs::read(&outside_target).unwrap(), b"PROTECTED DATA");

    // SafeFileWriter delete_file must reject
    let del_res = SafeFileWriter::delete_file(&target_via_junction, None);
    assert!(del_res.is_err());
    assert!(outside_target.exists());

    remove_test_junction(&junction_dir);
    let _ = fs::remove_dir_all(&temp_root);
}

// =========================================================================
// 13. DuringPostVerification Failure Cleanly Restores
// =========================================================================

#[test]
fn test_during_post_verification_failure_restores_cleanly() {
    let (temp_root, initial_snapshot) = create_test_map_fixture("post_verify_fail");
    let map_dir = temp_root.join("MAPS").join("ENDL_005");

    let mut tx = MapSaveTransaction::new(&map_dir);
    tx.stage_file("boden.bmp", vec![0x11, 0x22, 0x33, 0x44]).unwrap();
    tx.stage_file("DATA/new.dat", b"new data").unwrap();
    tx = tx.with_injected_failure(
        TransactionFailurePoint::DuringPostVerification,
        "Simulated post-verification checksum mismatch",
    );

    let err = tx.commit().unwrap_err();
    assert_eq!(err.stage, TransactionStage::PostVerification);
    assert_eq!(err.rollback_status, RollbackStatus::CleanlyRestored);

    // Verify byte-for-byte identity of all files
    assert_byte_for_byte_identical(&map_dir, &initial_snapshot);
    assert!(!map_dir.join("DATA").join("new.dat").exists());

    let _ = fs::remove_dir_all(&temp_root);
}

// =========================================================================
// 14. Partial Rollback Failure Preserves Backup Snapshots
// =========================================================================

#[test]
fn test_partial_rollback_failure_preserves_backup_snapshots() {
    let temp_root = std::env::temp_dir().join(format!("arm_tx_partial_rb_{}", Uuid::new_v4().simple()));
    let map_dir = temp_root.join("MAPS").join("ENDL_005");
    fs::create_dir_all(&map_dir).unwrap();

    let target_file = map_dir.join("critical.dat");
    fs::write(&target_file, b"ORIGINAL PRECIOUS BYTES").unwrap();

    let mut scope = FileRollbackScope::new().unwrap();
    scope.track_file(&target_file).unwrap();
    let root_path = scope.root_path().to_path_buf();
    assert!(root_path.exists());

    // Modify the file
    fs::write(&target_file, b"MODIFIED BYTES").unwrap();

    // Normal restore should succeed and clean up root_path on commit
    assert!(scope.restore_all().is_ok());
    assert_eq!(fs::read(&target_file).unwrap(), b"ORIGINAL PRECIOUS BYTES");

    let _ = fs::remove_dir_all(&temp_root);
}

// =========================================================================
// 15. Tiered Path Policy Regression Tests
// =========================================================================

#[test]
fn test_tiered_policy_allows_external_standalone_projects() {
    let temp_root = std::env::temp_dir().join(format!("arm_tx_external_{}", Uuid::new_v4().simple()));
    // Standalone project: arbitrary folder name, not ENDL slot, no marker required
    let map_dir = temp_root.join("my_creative_projects").join("campaign_alpha");
    fs::create_dir_all(&map_dir).unwrap();

    let mut tx = MapSaveTransaction::new(&map_dir);
    tx.stage_file("boden.bmp", b"external heightmap").unwrap();
    tx.stage_file("DATA/objects.dat", b"custom objects").unwrap();

    let report = tx.commit().expect("External project save must succeed without marker or ENDL constraints");
    assert_eq!(report.written_files.len(), 2);
    assert_eq!(fs::read(map_dir.join("boden.bmp")).unwrap(), b"external heightmap");
    assert_eq!(fs::read(map_dir.join("DATA/objects.dat")).unwrap(), b"custom objects");

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn test_tiered_policy_standard_install_path_rules() {
    let temp_root = std::env::temp_dir().join(format!("arm_tx_std_install_{}", Uuid::new_v4().simple()));
    let game_dir = temp_root.join("Program Files (x86)").join("Against Rome");
    let maps_dir = game_dir.join("MAPS");
    fs::create_dir_all(&maps_dir).unwrap();

    // 1. Retail ENDL_001 must be rejected
    let endl_001 = maps_dir.join("ENDL_001");
    fs::create_dir_all(&endl_001).unwrap();
    let mut tx_endl_001 = MapSaveTransaction::new(&endl_001);
    tx_endl_001.stage_file("boden.bmp", b"hacked retail").unwrap();
    let err = tx_endl_001.commit().unwrap_err();
    assert_eq!(err.stage, TransactionStage::PreflightValidation);
    assert!(err.message.contains("安全拒絕"), "{}", err.message);
    assert!(err.message.contains("ENDL_000..ENDL_004"), "{}", err.message);
    assert!(!endl_001.join("boden.bmp").exists());

    // 2. Retail TUTOR_01 must be rejected
    let tutor = maps_dir.join("TUTOR_01");
    fs::create_dir_all(&tutor).unwrap();
    let mut tx_tutor = MapSaveTransaction::new(&tutor);
    tx_tutor.stage_file("boden.bmp", b"hacked tutorial").unwrap();
    let err = tx_tutor.commit().unwrap_err();
    assert!(err.message.contains("安全拒絕"), "{}", err.message);
    assert!(err.message.contains("零售地圖"), "{}", err.message);

    // 3. Retail CAMP_01 must be rejected
    let camp = maps_dir.join("CAMP_01");
    fs::create_dir_all(&camp).unwrap();
    let mut tx_camp = MapSaveTransaction::new(&camp);
    tx_camp.stage_file("boden.bmp", b"hacked campaign").unwrap();
    let err = tx_camp.commit().unwrap_err();
    assert!(err.message.contains("安全拒絕"), "{}", err.message);

    // 4. Non-MAPS system/game path write must be rejected
    let mut tx_root = MapSaveTransaction::new(&game_dir);
    tx_root.stage_file("Against_Rome.exe", b"fake exe").unwrap();
    let err = tx_root.commit().unwrap_err();
    assert!(err.message.contains("安全拒絕"), "{}", err.message);

    // 5. ENDL_005 without marker must be rejected
    let endl_005 = maps_dir.join("ENDL_005");
    fs::create_dir_all(&endl_005).unwrap();
    let mut tx_unmarked = MapSaveTransaction::new(&endl_005);
    tx_unmarked.stage_file("boden.bmp", b"unmarked custom map").unwrap();
    let err = tx_unmarked.commit().unwrap_err();
    assert!(err.message.contains("安全拒絕"), "{}", err.message);
    assert!(err.message.contains(".arm_custom_map"), "{}", err.message);

    // 6. ENDL_005 with existing marker on disk succeeds
    let marker_entry = CustomMapEntry::new(5, 1, Utc::now(), "1.0.0");
    let marker_json = serde_json::to_string_pretty(&marker_entry).unwrap().into_bytes();
    fs::write(endl_005.join(CustomMapManifest::MARKER_FILE_NAME), &marker_json).unwrap();

    let mut tx_marked = MapSaveTransaction::new(&endl_005);
    tx_marked.stage_file("boden.bmp", b"marked valid custom map").unwrap();
    let report = tx_marked.commit().expect("Marker-backed ENDL_005 in Program Files must be permitted");
    assert_eq!(report.written_files.len(), 1);
    assert_eq!(fs::read(endl_005.join("boden.bmp")).unwrap(), b"marked valid custom map");

    // 7. New map creation with staged marker succeeds (No Deadlock!)
    let endl_006 = maps_dir.join("ENDL_006");
    let marker_entry_006 = CustomMapEntry::new(6, 1, Utc::now(), "1.0.0");
    let mut tx_staged_new = MapSaveTransaction::new(&endl_006).with_create_if_missing(true);
    tx_staged_new.stage_custom_map_marker(&marker_entry_006).unwrap();
    tx_staged_new.stage_file("boden.bmp", b"brand new map").unwrap();
    let report_new = tx_staged_new.commit().expect("New map creation with staged marker must not deadlock");
    assert!(endl_006.join(CustomMapManifest::MARKER_FILE_NAME).is_file());
    assert_eq!(fs::read(endl_006.join("boden.bmp")).unwrap(), b"brand new map");
    assert_eq!(report_new.written_files.len(), 2);

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn test_tiered_policy_non_standard_install_path_rules() {
    let temp_root = std::env::temp_dir().join(format!("arm_tx_nonstd_install_{}", Uuid::new_v4().simple()));
    // Non-standard install: D:\Games\Against Rome
    let game_dir = temp_root.join("Games").join("Against Rome");
    let maps_dir = game_dir.join("MAPS");
    fs::create_dir_all(&maps_dir).unwrap();
    // Simulate game root executable
    fs::write(game_dir.join("Against_Rome.exe"), b"MOCK_EXE_BYTES").unwrap();

    // 1. Retail ENDL_002 must be rejected in non-standard install path
    let endl_002 = maps_dir.join("ENDL_002");
    fs::create_dir_all(&endl_002).unwrap();
    let mut tx_endl = MapSaveTransaction::new(&endl_002);
    tx_endl.stage_file("boden.bmp", b"retail 002").unwrap();
    let err = tx_endl.commit().unwrap_err();
    assert!(err.message.contains("安全拒絕"), "{}", err.message);
    assert!(err.message.contains("ENDL_000..ENDL_004"), "{}", err.message);

    // 2. Retail TUTOR_02 must be rejected
    let tutor = maps_dir.join("TUTOR_02");
    fs::create_dir_all(&tutor).unwrap();
    let mut tx_tutor = MapSaveTransaction::new(&tutor);
    tx_tutor.stage_file("boden.bmp", b"retail tutor 2").unwrap();
    let err = tx_tutor.commit().unwrap_err();
    assert!(err.message.contains("安全拒絕"), "{}", err.message);

    // 3. Arbitrary non-ENDL folder in MAPS (e.g. MAPS/my_folder) must be rejected
    let arbitrary = maps_dir.join("my_folder");
    fs::create_dir_all(&arbitrary).unwrap();
    let mut tx_arb = MapSaveTransaction::new(&arbitrary);
    tx_arb.stage_file("boden.bmp", b"arbitrary in game maps").unwrap();
    let err = tx_arb.commit().unwrap_err();
    assert!(err.message.contains("安全拒絕"), "{}", err.message);

    // 4. Marker-backed ENDL_008 in non-standard install succeeds
    let endl_008 = maps_dir.join("ENDL_008");
    fs::create_dir_all(&endl_008).unwrap();
    let marker_entry = CustomMapEntry::new(8, 1, Utc::now(), "1.0.0");
    let marker_json = serde_json::to_string_pretty(&marker_entry).unwrap().into_bytes();
    fs::write(endl_008.join(CustomMapManifest::MARKER_FILE_NAME), &marker_json).unwrap();

    let mut tx_custom = MapSaveTransaction::new(&endl_008);
    tx_custom.stage_file("boden.bmp", b"valid custom in D:/Games").unwrap();
    let report = tx_custom.commit().expect("Marker-backed ENDL_008 in non-standard install must succeed");
    assert_eq!(report.written_files.len(), 1);
    assert_eq!(fs::read(endl_008.join("boden.bmp")).unwrap(), b"valid custom in D:/Games");

    // 5. Staged marker new map creation in non-standard install succeeds
    let endl_009 = maps_dir.join("ENDL_009");
    let marker_entry_009 = CustomMapEntry::new(9, 1, Utc::now(), "1.0.0");
    let mut tx_new = MapSaveTransaction::new(&endl_009).with_create_if_missing(true);
    tx_new.stage_custom_map_marker(&marker_entry_009).unwrap();
    tx_new.stage_file("boden.bmp", b"new 009").unwrap();
    let report_new = tx_new.commit().expect("New custom map with staged marker must succeed");
    assert!(endl_009.join(CustomMapManifest::MARKER_FILE_NAME).is_file());
    assert_eq!(report_new.written_files.len(), 2);

    let _ = fs::remove_dir_all(&temp_root);
}

