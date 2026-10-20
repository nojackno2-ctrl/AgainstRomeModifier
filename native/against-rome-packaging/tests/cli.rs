use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Self {
        let dir = env::temp_dir().join(format!("{}_{}", prefix, Uuid::new_v4().simple()));
        fs::create_dir_all(&dir).unwrap();
        Self { path: dir }
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn bin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ArmMapPackage"))
}

fn write_solid_bmp(path: &Path, width: i32, height: i32, fill: u8) {
    let stride = ((width * 3 + 3) / 4) * 4;
    let image_size = stride * height;
    let file_size = 54 + image_size;
    let mut bytes = vec![0u8; file_size as usize];

    bytes[0] = b'B';
    bytes[1] = b'M';
    bytes[2..6].copy_from_slice(&file_size.to_le_bytes());
    bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
    bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&width.to_le_bytes());
    bytes[22..26].copy_from_slice(&height.to_le_bytes());
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
    bytes[34..38].copy_from_slice(&image_size.to_le_bytes());

    for y in 0..height {
        let row = 54 + (y * stride) as usize;
        for x in 0..width {
            let offset = row + (x * 3) as usize;
            bytes[offset] = fill;
            bytes[offset + 1] = fill;
            bytes[offset + 2] = fill;
        }
    }

    fs::write(path, bytes).unwrap();
}

fn create_valid_mock_map(dir: &Path, slot: i32, title: &str, include_boden_ini: bool) {
    fs::create_dir_all(dir.join("TEXT").join("US")).unwrap();
    fs::create_dir_all(dir.join("DATA")).unwrap();
    fs::create_dir_all(dir.join("SCRIPT")).unwrap();

    write_solid_bmp(&dir.join("boden.bmp"), 257, 257, 128);
    write_solid_bmp(&dir.join("collision.bmp"), 256, 256, 0);

    if include_boden_ini {
        fs::write(
            dir.join("boden.ini"),
            "Waterlevel = 50.0\nWaterColor = 30 100 180\n",
        )
        .unwrap();
    }

    let put_content = format!(
        "var:briefing_titel_1 = \"{}\";\nvar:briefing_titel_2 = \"Subtitle\";\nvar:briefing_text = \"Test briefing\";\n",
        title
    );
    fs::write(dir.join("TEXT").join("US").join("briefing.put"), put_content).unwrap();

    fs::write(dir.join("DATA").join("objects.dat"), vec![0u8; 64]).unwrap();
    fs::write(dir.join("DATA").join("objdata.dat"), vec![0u8; 64]).unwrap();
    fs::write(dir.join("DATA").join("pos.dat"), vec![0u8; 64]).unwrap();

    fs::write(
        dir.join("SCRIPT").join("ak_level.bci"),
        b"PFIL\0\0\0\0",
    )
    .unwrap();

    let sdl_text = format!(
        "[settlement]\nname = MAPS/ENDL_{:03}/Endlos_005_Siedlung1.sdl\nrefpos = 8192, 100, 8192\n\n[object0]\nnamedef = BauGerHau00\npos = 0, 0, 0\nteam = 0\n",
        slot
    );
    fs::write(dir.join("Endlos_005_Siedlung1.sdl"), sdl_text).unwrap();

    let marker_json = format!(
        "{{\"Slot\":{},\"SourceSlot\":{},\"CreatedAt\":\"2026-10-08T00:00:00Z\",\"ToolVersion\":\"1.0\"}}",
        slot, slot
    );
    fs::write(dir.join(".arm_custom_map"), marker_json).unwrap();
}

#[test]
fn test_cli_usage_errors_exit_code_2() {
    let bin = bin_path();

    // 1. No arguments
    let output = Command::new(&bin).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Usage: ArmMapPackage install <bundle.zip> <game-root> | export <map-directory> <bundle.zip> [--force]")
    );

    // 2. Unknown command
    let output = Command::new(&bin)
        .args(["foo", "bar", "baz"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage: ArmMapPackage"));

    // 3. Missing arguments
    let output = Command::new(&bin)
        .args(["install", "bundle.zip"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));

    // 4. Invalid export flag
    let output = Command::new(&bin)
        .args(["export", "mapdir", "bundle.zip", "--other"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));

    // 5. Too many arguments
    let output = Command::new(&bin)
        .args(["export", "mapdir", "bundle.zip", "--force", "extra"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn test_cli_processing_errors_exit_code_1() {
    let bin = bin_path();
    let temp = TempDirGuard::new("arm_cli_proc_err");

    // 1. Install missing zip
    let fake_zip = temp.path.join("nonexistent.zip");
    let game_dir = temp.path.join("game");
    fs::create_dir_all(&game_dir).unwrap();

    let output = Command::new(&bin)
        .args(["install", fake_zip.to_str().unwrap(), game_dir.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("找不到模組 ZIP 檔案") || stderr.contains("not found"));

    // 2. Install missing game root
    let fake_game = temp.path.join("nonexistent_game");
    let existing_zip = temp.path.join("dummy.zip");
    fs::write(&existing_zip, b"not a real zip").unwrap();

    let output = Command::new(&bin)
        .args(["install", existing_zip.to_str().unwrap(), fake_game.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("找不到遊戲路徑") || stderr.contains("not found"));

    // 3. Export missing map directory
    let fake_map_dir = temp.path.join("nonexistent_map");
    let out_zip = temp.path.join("out.zip");

    let output = Command::new(&bin)
        .args(["export", fake_map_dir.to_str().unwrap(), out_zip.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("地圖目錄不存在") || stderr.contains("not found"));
}

#[test]
fn test_cli_export_and_install_success_exit_code_0() {
    let bin = bin_path();
    let temp = TempDirGuard::new("arm_cli_success");

    let map_dir = temp.path.join("source_map");
    create_valid_mock_map(&map_dir, 5, "Synthetic Test Map", true);

    let bundle_zip = temp.path.join("output_bundle.zip");

    // 1. Export valid map
    let output = Command::new(&bin)
        .args(["export", map_dir.to_str().unwrap(), bundle_zip.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Exported Synthetic Test Map:"));
    assert!(stdout.contains("payload files"));
    assert!(stdout.contains(bundle_zip.to_str().unwrap()) || stdout.contains("output_bundle.zip"));
    assert!(stdout.contains("Preflight: 0 errors, 0 warnings."));
    assert!(bundle_zip.is_file());

    // 2. Install into synthetic game root
    let game_root = temp.path.join("game_root");
    fs::create_dir_all(game_root.join("MAPS")).unwrap();

    let output = Command::new(&bin)
        .args(["install", bundle_zip.to_str().unwrap(), game_root.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Installed Synthetic Test Map: ENDL_005"));
    assert!(stdout.contains("payload files"));
    assert!(stdout.contains("Playability remains unverified: preflight 0 errors, 0 warnings."));

    let installed_dir = game_root.join("MAPS").join("ENDL_005");
    assert!(installed_dir.is_dir());
    assert!(installed_dir.join(".arm_custom_map").is_file());
    assert!(installed_dir.join("TEXT").join("US").join("briefing.put").is_file());
}

#[test]
fn test_cli_force_preflight_behavior() {
    let bin = bin_path();
    let temp = TempDirGuard::new("arm_cli_force");

    let map_dir = temp.path.join("faulty_map");
    // Omitting boden.ini triggers a preflight Error
    create_valid_mock_map(&map_dir, 5, "Faulty Map", false);

    let out_zip = temp.path.join("faulty.zip");

    // Without --force: fails with exit code 1
    let output = Command::new(&bin)
        .args(["export", map_dir.to_str().unwrap(), out_zip.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("地圖未通過發布預檢"));

    // With --force: succeeds with exit code 0
    let output = Command::new(&bin)
        .args(["export", map_dir.to_str().unwrap(), out_zip.to_str().unwrap(), "--force"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Exported Faulty Map:"));
    assert!(stdout.contains("Preflight: 1 errors, 0 warnings."));
    assert!(out_zip.is_file());
}

#[test]
fn test_cli_usage_extended_edge_cases() {
    let bin = bin_path();

    // 1. install does not take --force
    let output = Command::new(&bin)
        .args(["install", "bundle.zip", "game-root", "--force"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage: ArmMapPackage"));

    // 2. Single argument
    let output = Command::new(&bin).args(["export"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage: ArmMapPackage"));

    let output = Command::new(&bin).args(["install"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage: ArmMapPackage"));

    // 3. export with misplaced --force
    let output = Command::new(&bin)
        .args(["export", "--force", "mapdir", "bundle.zip"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage: ArmMapPackage"));

    // 4. Command with 2 arguments
    let output = Command::new(&bin)
        .args(["export", "mapdir"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage: ArmMapPackage"));
}

#[test]
fn test_cli_stream_isolation_and_multislot_install() {
    let bin = bin_path();
    let temp = TempDirGuard::new("arm_cli_stream_iso");

    let map_dir = temp.path.join("source_map");
    create_valid_mock_map(&map_dir, 5, "MultiSlot Map", true);

    let bundle_zip = temp.path.join("bundle.zip");

    // 1. Export: test stdout line structure and stderr emptiness
    let export_output = Command::new(&bin)
        .args(["export", map_dir.to_str().unwrap(), bundle_zip.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(export_output.status.code(), Some(0));
    assert!(export_output.stderr.is_empty(), "stderr should be empty on successful export");

    let stdout_str = String::from_utf8_lossy(&export_output.stdout);
    let lines: Vec<&str> = stdout_str.lines().collect();
    assert_eq!(lines.len(), 3, "Export output should have exactly 3 lines: {}", stdout_str);
    assert!(lines[0].starts_with("Exported MultiSlot Map:"));
    assert_eq!(lines[1], bundle_zip.to_str().unwrap());
    assert_eq!(lines[2], "Preflight: 0 errors, 0 warnings.");

    // 2. Install 1st time into empty game root -> should take ENDL_005
    let game_root = temp.path.join("game_root");
    fs::create_dir_all(game_root.join("MAPS")).unwrap();

    let install_output1 = Command::new(&bin)
        .args(["install", bundle_zip.to_str().unwrap(), game_root.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(install_output1.status.code(), Some(0));
    assert!(install_output1.stderr.is_empty(), "stderr should be empty on successful install");

    let install_stdout1 = String::from_utf8_lossy(&install_output1.stdout);
    let install_lines1: Vec<&str> = install_stdout1.lines().collect();
    assert_eq!(install_lines1.len(), 3, "Install output should have exactly 3 lines");
    assert!(install_lines1[0].contains("Installed MultiSlot Map: ENDL_005"));
    assert_eq!(install_lines1[2], "Playability remains unverified: preflight 0 errors, 0 warnings.");

    // 3. Install 2nd time -> ENDL_005 occupied, should auto allocate ENDL_006
    let install_output2 = Command::new(&bin)
        .args(["install", bundle_zip.to_str().unwrap(), game_root.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(install_output2.status.code(), Some(0));
    assert!(install_output2.stderr.is_empty());

    let install_stdout2 = String::from_utf8_lossy(&install_output2.stdout);
    let install_lines2: Vec<&str> = install_stdout2.lines().collect();
    assert_eq!(install_lines2.len(), 3);
    assert!(install_lines2[0].contains("Installed MultiSlot Map: ENDL_006"), "Second install should allocate ENDL_006: {}", install_lines2[0]);
    assert!(game_root.join("MAPS").join("ENDL_006").is_dir());

    // 4. Corrupt zip install error handling
    let corrupt_zip = temp.path.join("corrupt.zip");
    fs::write(&corrupt_zip, b"PK\x03\x04corrupted zip data").unwrap();
    let corrupt_output = Command::new(&bin)
        .args(["install", corrupt_zip.to_str().unwrap(), game_root.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(corrupt_output.status.code(), Some(1));
    assert!(corrupt_output.stdout.is_empty(), "stdout should be empty on error");
    assert!(!corrupt_output.stderr.is_empty(), "stderr should contain error details");
}

