use against_rome_scenario::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "arm_test_scenario_cli_{}_{}",
            prefix,
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn bin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_against-rome-scenario"))
}

fn assert_no_temp_files(dir: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(
                !name.contains(".tmp."),
                "found leftover temporary file in {}: {}",
                dir.display(),
                name
            );
        }
    }
}

fn make_pfil_bci() -> Vec<u8> {
    let raw = BciImage::idle().serialize();
    let mut pfil = vec![0u8; 64];
    pfil[0..4].copy_from_slice(b"PFIL");
    pfil[16..20].copy_from_slice(&(raw.len() as u32).to_le_bytes());
    for chunk in raw.chunks(8) {
        pfil.push(((1u16 << chunk.len()) - 1) as u8);
        pfil.extend(chunk);
    }
    pfil
}

fn minimal_scenario_json() -> Vec<u8> {
    serde_json::to_vec(&ScenarioDocument::default()).unwrap()
}

fn rich_scenario_json() -> Vec<u8> {
    let doc = ScenarioDocument {
        version: 6,
        spawns: vec![Spawn {
            alias: "unit".into(),
            x: 100.5,
            z: 120.5,
            team: 1,
            count: 5,
            angle: 45,
            y: 3.25,
            prebuilt: false,
            id: uuid::Uuid::new_v4(),
            raw_rotation: None,
        }],
        events: vec![Event {
            name: "init".into(),
            enabled: true,
            delay_seconds: 1,
            repeat: false,
            actions: vec![Action {
                kind: 0,
                text: "Mission Start".into(),
                ..Default::default()
            }],
            conditions: vec![],
        }],
        ..Default::default()
    };
    serde_json::to_vec(&doc).unwrap()
}

fn make_binding_fixture(dir: &Path, uid: u32, active: bool) {
    fs::create_dir_all(dir.join("DATA")).unwrap();
    let mut bytes = vec![0u8; 16 + 10 * 121];
    for (offset, value) in [(0, 1u32), (4, 10), (8, 30), (12, 30)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[16 + 9 * 79] = u8::from(active);
    bytes[19 + 9 * 79..23 + 9 * 79].copy_from_slice(&uid.to_le_bytes());
    fs::write(dir.join("DATA").join("objects.dat"), bytes).unwrap();
}

#[test]
fn cli_compile_success_preserves_pfil_format_and_cleans_temporary() {
    let temp = TempDirGuard::new("success_pfil");
    let input = temp.path.join("original.bci");
    let json = temp.path.join("scenario.json");
    let output = temp.path.join("export.bci");

    let pfil_data = make_pfil_bci();
    fs::write(&input, &pfil_data).unwrap();
    fs::write(&json, minimal_scenario_json()).unwrap();

    let status = Command::new(bin_path())
        .arg("compile")
        .arg(&input)
        .arg(&json)
        .arg(&output)
        .output()
        .expect("failed to execute scenario CLI");

    assert!(
        status.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let stdout = String::from_utf8_lossy(&status.stdout);
    assert!(stdout.contains("compiled"));
    assert!(stdout.contains("export.bci"));

    assert!(output.is_file(), "output file should exist");
    let out_bytes = fs::read(&output).unwrap();
    assert!(
        out_bytes.starts_with(b"PFIL"),
        "output must preserve PFIL format when input is PFIL"
    );

    // Verify output decompressibility
    let decompressed = against_rome_data::lzss::decompress_pfil(&out_bytes, None)
        .expect("output PFIL must decompress cleanly");
    BciImage::parse(&decompressed).expect("decompressed BCI must parse as valid BciImage");

    // Verify no temporary files remain in output directory
    assert_no_temp_files(&temp.path);
}

#[test]
fn cli_compile_raw_bci_success_and_cleans_temporary() {
    let temp = TempDirGuard::new("success_raw");
    let input = temp.path.join("raw.bci");
    let json = temp.path.join("scenario.json");
    let output = temp.path.join("out_raw.bci");

    let raw = BciImage::idle().serialize();
    fs::write(&input, &raw).unwrap();
    fs::write(&json, minimal_scenario_json()).unwrap();

    let status = Command::new(bin_path())
        .arg("compile")
        .arg(&input)
        .arg(&json)
        .arg(&output)
        .output()
        .expect("failed to execute scenario CLI");

    assert!(status.status.success());
    assert!(output.is_file());
    let out_bytes = fs::read(&output).unwrap();
    assert!(!out_bytes.starts_with(b"PFIL"));
    BciImage::parse(&out_bytes).expect("output must parse as valid raw BCI");

    assert_no_temp_files(&temp.path);
}

#[test]
fn cli_compile_rich_scenario_with_camera_and_aliases_succeeds() {
    let temp = TempDirGuard::new("rich_camera_aliases");
    let input = temp.path.join("original.bci");
    let json = temp.path.join("rich_scenario.json");
    let camera_file = temp.path.join("camera.json");
    let output = temp.path.join("rich_export.bci");
    let game_root = temp.path.join("game_catalog");

    let catalog_dir = game_root.join("SYSTEM").join("CLAK");
    fs::create_dir_all(&catalog_dir).unwrap();
    fs::write(
        catalog_dir.join("cl_scint.ini"),
        b"[ObjDefName]\nunit=r_infantry_01\nhouse=r_building_01\n",
    )
    .unwrap();

    let pfil_data = make_pfil_bci();
    fs::write(&input, &pfil_data).unwrap();
    fs::write(&json, rich_scenario_json()).unwrap();

    let camera = StartCamera {
        enabled: true,
        x: 100.0,
        y: 0.0,
        z: 200.0,
        zoom: 1.0,
        primitive: 0,
    };
    fs::write(&camera_file, serde_json::to_vec(&camera).unwrap()).unwrap();

    let status = Command::new(bin_path())
        .arg("compile")
        .arg(&input)
        .arg(&json)
        .arg(&output)
        .arg(&camera_file)
        .arg(&game_root)
        .output()
        .expect("failed to execute scenario CLI");

    assert!(
        status.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert!(output.is_file());
    let out_bytes = fs::read(&output).unwrap();
    assert!(out_bytes.starts_with(b"PFIL"));
    assert_no_temp_files(&temp.path);
}

#[test]
fn cli_compile_existing_output_fails_and_preserves_original_content() {
    let temp = TempDirGuard::new("existing_output");
    let input = temp.path.join("input.bci");
    let json = temp.path.join("scenario.json");
    let output = temp.path.join("already_exists.bci");

    let raw = BciImage::idle().serialize();
    fs::write(&input, &raw).unwrap();
    fs::write(&json, minimal_scenario_json()).unwrap();

    let original_bytes = b"PREEXISTING_ORIGINAL_BCI_CONTENT_DO_NOT_OVERWRITE";
    fs::write(&output, original_bytes).unwrap();

    let status = Command::new(bin_path())
        .arg("compile")
        .arg(&input)
        .arg(&json)
        .arg(&output)
        .output()
        .expect("failed to execute scenario CLI");

    assert!(
        !status.status.success(),
        "CLI must fail when output file already exists"
    );
    let stderr = String::from_utf8_lossy(&status.stderr);
    assert!(
        stderr.contains("already exists"),
        "stderr should mention already exists: {stderr}"
    );

    // Verify existing file is completely untouched
    let current_bytes = fs::read(&output).unwrap();
    assert_eq!(
        current_bytes, original_bytes,
        "existing output file must never be modified or overwritten"
    );

    // Verify no temporary files remain
    assert_no_temp_files(&temp.path);
}

#[test]
fn cli_compile_missing_input_or_bad_json_fails_without_creating_output() {
    let temp = TempDirGuard::new("bad_preconditions");
    let non_existent_input = temp.path.join("does_not_exist.bci");
    let json = temp.path.join("scenario.json");
    let output1 = temp.path.join("out1.bci");

    fs::write(&json, minimal_scenario_json()).unwrap();

    // 1. Missing input file
    let status1 = Command::new(bin_path())
        .arg("compile")
        .arg(&non_existent_input)
        .arg(&json)
        .arg(&output1)
        .output()
        .expect("failed to execute scenario CLI");

    assert!(!status1.status.success());
    assert!(!output1.exists(), "no output should be created on missing input");
    assert_no_temp_files(&temp.path);

    // 2. Corrupted scenario JSON
    let input = temp.path.join("input.bci");
    let bad_json = temp.path.join("corrupted.json");
    let output2 = temp.path.join("out2.bci");

    fs::write(&input, BciImage::idle().serialize()).unwrap();
    fs::write(&bad_json, b"NOT_A_VALID_JSON_OBJECT{{{").unwrap();

    let status2 = Command::new(bin_path())
        .arg("compile")
        .arg(&input)
        .arg(&bad_json)
        .arg(&output2)
        .output()
        .expect("failed to execute scenario CLI");

    assert!(!status2.status.success());
    assert!(!output2.exists(), "no output should be created on corrupt json");
    assert_no_temp_files(&temp.path);
}

#[test]
fn cli_compile_forbidden_game_path_rejected_without_creating_output() {
    let temp = TempDirGuard::new("forbidden_path");
    // Directory structure simulating installed game path containing \Against Rome\
    let game_dir = temp.path.join("Against Rome").join("MAPS").join("TEST_001");
    fs::create_dir_all(&game_dir).unwrap();

    let input = temp.path.join("input.bci");
    let json = temp.path.join("scenario.json");
    let forbidden_output = game_dir.join("forbidden.bci");

    fs::write(&input, BciImage::idle().serialize()).unwrap();
    fs::write(&json, minimal_scenario_json()).unwrap();

    let status = Command::new(bin_path())
        .arg("compile")
        .arg(&input)
        .arg(&json)
        .arg(&forbidden_output)
        .output()
        .expect("failed to execute scenario CLI");

    assert!(!status.status.success());
    let stderr = String::from_utf8_lossy(&status.stderr);
    assert!(
        stderr.contains("installed game directory is forbidden"),
        "stderr should explain game directory prohibition: {stderr}"
    );
    assert!(
        !forbidden_output.exists(),
        "forbidden output must not be created"
    );
    assert_no_temp_files(&game_dir);
}

#[test]
fn cli_compile_bindings_validation_failure_cleans_temporary() {
    let temp = TempDirGuard::new("bindings_fail");
    let input = temp.path.join("input.bci");
    let json = temp.path.join("scenario.json");
    let output = temp.path.join("output.bci");
    let source_dir = temp.path.join("game_root");

    make_binding_fixture(&source_dir, u32::MAX, true);

    let doc = ScenarioDocument {
        version: 6,
        spawns: vec![Spawn {
            alias: "house".into(),
            x: 50.0,
            z: 70.0,
            team: 0,
            prebuilt: true,
            id: uuid::Uuid::parse_str("22345678-1122-3344-5566-778899001122").unwrap(),
            ..Default::default()
        }],
        data_slots: vec![DataSlot {
            slot: 9,
            uid: u32::MAX,
            spawn_id: uuid::Uuid::parse_str("22345678-1122-3344-5566-778899001122").unwrap(),
        }],
        ..Default::default()
    };

    fs::write(&input, BciImage::idle().serialize()).unwrap();
    fs::write(&json, serde_json::to_vec(&doc).unwrap()).unwrap();

    // 1. First: valid compilation with binding source succeeds
    let status_valid = Command::new(bin_path())
        .arg("compile")
        .arg(&input)
        .arg(&json)
        .arg(&output)
        .arg("-")
        .arg("-")
        .arg(&source_dir)
        .output()
        .expect("failed to execute scenario CLI");

    assert!(status_valid.status.success());
    assert!(output.is_file());
    assert_no_temp_files(&temp.path);

    // Remove output to prepare for mismatched UID test
    fs::remove_file(&output).unwrap();

    // 2. Mismatched UID in source objects.dat (uid 42 vs required u32::MAX)
    make_binding_fixture(&source_dir, 42, true);

    let status_mismatch = Command::new(bin_path())
        .arg("compile")
        .arg(&input)
        .arg(&json)
        .arg(&output)
        .arg("-")
        .arg("-")
        .arg(&source_dir)
        .output()
        .expect("failed to execute scenario CLI");

    assert!(
        !status_mismatch.status.success(),
        "compilation must fail when UID in objects.dat does not match document"
    );
    assert!(
        !output.exists(),
        "output must not be created when binding validation fails"
    );
    assert_no_temp_files(&temp.path);

    // 3. Missing binding source completely when prebuilt slot is present
    let output3 = temp.path.join("output3.bci");
    let status_no_source = Command::new(bin_path())
        .arg("compile")
        .arg(&input)
        .arg(&json)
        .arg(&output3)
        .output()
        .expect("failed to execute scenario CLI");

    assert!(
        !status_no_source.status.success(),
        "compilation must fail when prebuilt slots exist but no binding source is supplied"
    );
    assert!(
        !output3.exists(),
        "output must not be created when binding source is missing"
    );
    assert_no_temp_files(&temp.path);

    // 4. Corrupted objects.dat in binding source
    let output4 = temp.path.join("output4.bci");
    fs::write(source_dir.join("DATA").join("objects.dat"), b"CORRUPTED_OBJECTS_DATA").unwrap();
    let status_corrupt = Command::new(bin_path())
        .arg("compile")
        .arg(&input)
        .arg(&json)
        .arg(&output4)
        .arg("-")
        .arg("-")
        .arg(&source_dir)
        .output()
        .expect("failed to execute scenario CLI");

    assert!(
        !status_corrupt.status.success(),
        "compilation must fail when binding source objects.dat is corrupt"
    );
    assert!(
        !output4.exists(),
        "output must not be created when binding source is corrupt"
    );
    assert_no_temp_files(&temp.path);
}

#[test]
fn cli_compile_directory_as_output_fails_without_residual_temp() {
    let temp = TempDirGuard::new("directory_collision");
    let input = temp.path.join("input.bci");
    let json = temp.path.join("scenario.json");
    let dir_output = temp.path.join("existing_directory");
    fs::create_dir_all(&dir_output).unwrap();

    fs::write(&input, BciImage::idle().serialize()).unwrap();
    fs::write(&json, minimal_scenario_json()).unwrap();

    let status = Command::new(bin_path())
        .arg("compile")
        .arg(&input)
        .arg(&json)
        .arg(&dir_output)
        .output()
        .expect("failed to execute scenario CLI");

    assert!(!status.status.success());
    assert!(dir_output.is_dir(), "directory must still exist");
    assert_no_temp_files(&temp.path);
    assert_no_temp_files(&dir_output);
}

#[test]
fn cli_usage_and_inspect_command_parity() {
    // 1. No arguments -> usage error
    let status_empty = Command::new(bin_path()).output().unwrap();
    assert!(!status_empty.status.success());
    let stderr = String::from_utf8_lossy(&status_empty.stderr);
    assert!(stderr.contains("commands: inspect, compile"));

    // 2. Inspect command with PFIL input
    let temp = TempDirGuard::new("inspect");
    let pfil_path = temp.path.join("test.bci");
    fs::write(&pfil_path, make_pfil_bci()).unwrap();

    let status_inspect = Command::new(bin_path())
        .arg("inspect")
        .arg(&pfil_path)
        .output()
        .unwrap();

    assert!(status_inspect.status.success());
    let stdout = String::from_utf8_lossy(&status_inspect.stdout);
    assert!(stdout.contains("CODE="));
    assert!(stdout.contains("constants="));
    assert!(stdout.contains("main="));
    assert!(stdout.contains("dtor="));
}
