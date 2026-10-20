use formation_probe::safety::{
    validate_asset_name, validate_explicit_path, validate_new_output_dir,
};
use formation_probe::{cli, pfil};
use std::ffi::OsString;
use std::fs;
use std::path::Path;

#[test]
fn test_safety_explicit_path() {
    assert!(validate_explicit_path(Path::new("")).is_err());
    assert!(validate_explicit_path(Path::new("foo\0bar")).is_err());
    assert!(validate_explicit_path(Path::new("../escape")).is_err());
    assert!(validate_explicit_path(Path::new("foo/../bar")).is_err());
    assert!(validate_explicit_path(Path::new("safe/path.dat")).is_ok());
}

#[test]
fn test_safety_new_output_dir() {
    let temp_dir = std::env::temp_dir().join(format!("safety_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let existing = temp_dir.join("existing_folder");
    fs::create_dir_all(&existing).unwrap();
    assert!(validate_new_output_dir(&existing).is_err());

    let non_existent = temp_dir.join("brand_new_folder");
    assert!(validate_new_output_dir(&non_existent).is_ok());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_safety_asset_name() {
    assert!(validate_asset_name("gerinf01.alr").is_ok());
    assert!(validate_asset_name("SYSTEM/DATA/ALR/gerinf01.alr").is_ok());
    assert!(validate_asset_name("").is_err());
    assert!(validate_asset_name("..").is_err());
    assert!(validate_asset_name("foo/../bar.alr").is_err());
    assert!(validate_asset_name("foo\0bar").is_err());
}

#[test]
fn test_cli_help() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = cli(&[OsString::from("--help")], &mut stdout, &mut stderr);
    assert_eq!(code, 2);
    let err_str = String::from_utf8_lossy(&stderr);
    assert!(err_str.contains("Usage:"));
}

#[test]
fn test_cli_disassembly_boundary() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = cli(
        &[
            OsString::from("probe.exe"),
            OsString::from("--range"),
            OsString::from("0x401000:0x401020"),
        ],
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(code, 2);
    let err_str = String::from_utf8_lossy(&stderr);
    assert!(err_str.contains("不支援 EXE 反組譯"));
}

#[test]
fn test_cli_layout_command() {
    let temp_dir = std::env::temp_dir().join(format!("cli_layout_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let formdef_text = "\
[FormationDefault]\r\n\
0, 1, 9, 0.05,-0.05, 0.20, 0.05,-0.20, 0.15,-0.05, 0.05, 0.20,-0.20, 0.05,-0.15,-0.10,-0.05,-0.20,-0.20, 0.30,-0.15, 0.40, 0.10,-0.25, 0.25,-0.40, 0.05, 0.05, 0.30, 0.30, 0.20, 0.34,-0.25, 0.10,-0.34,-0.20,-0.34,-0.40,-0.15, 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0, 0.0, 180.0, All_Haufen\r\n";
    let pfil_data = pfil::create_synthetic_pfil(formdef_text.as_bytes());
    let path = temp_dir.join("formdef.dau");
    fs::write(&path, pfil_data).unwrap();

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = cli(
        &[OsString::from("--layout"), path.as_os_str().to_os_string()],
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(code, 0);

    let output_str = String::from_utf8_lossy(&stdout);
    assert!(output_str.contains("\"matchesEmbedded\":true"));
    assert!(output_str.contains("\"segmentCount\":9"));
    assert!(output_str.contains("\"layouts\":["));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_cli_dump_command() {
    let temp_dir = std::env::temp_dir().join(format!("cli_dump_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let path = temp_dir.join("data.bin");
    fs::write(&path, b"AgainstRomeFormationData").unwrap();

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = cli(
        &[
            OsString::from("dump"),
            path.as_os_str().to_os_string(),
            OsString::from("--dump"),
            OsString::from("0:16"),
        ],
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(code, 0);

    let output_str = String::from_utf8_lossy(&stdout);
    assert!(output_str.starts_with("SHA256 "));
    assert!(output_str.contains("payload length 24"));
    assert!(output_str
        .contains("000000 41 67 61 69 6e 73 74 52 6f 6d 65 46 6f 72 6d 61 b'AgainstRomeForma'"));

    let _ = fs::remove_dir_all(&temp_dir);
}
