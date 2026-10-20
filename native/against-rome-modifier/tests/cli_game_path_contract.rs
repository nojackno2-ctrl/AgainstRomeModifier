//! Game-path resolution contract (messages frozen from the former C# CliCommands).
use against_rome_modifier::cli::{resolve_game_path, run_cli, strip_verbatim_prefix};
use std::{fs, path::PathBuf};

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("arm-gp-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn csharp_messages_match_rust_messages() {
    let missing = std::env::temp_dir().join("arm-gp-definitely-missing-dir");
    let err = resolve_game_path(Some(missing.to_str().unwrap())).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(
        err.to_string(),
        format!("指定的遊戲目錄不存在: {}", missing.display())
    );
}

#[test]
fn explicit_missing_or_file_game_path_never_falls_back_to_detection() {
    let d = temp("file");
    let f = d.join("Against_Rome.exe");
    fs::write(&f, b"MZ").unwrap();
    // A file (not a directory) is ambiguous: reject instead of using its parent.
    assert!(resolve_game_path(Some(f.to_str().unwrap())).is_err());
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let code = run_cli(
        &args(&["maps", "--game", f.to_str().unwrap(), "--json"]),
        &mut o,
        &mut e,
    );
    assert_eq!(code, 1);
    let v: serde_json::Value = serde_json::from_slice(&o).unwrap();
    assert_eq!(v["success"], false);
    assert_eq!(v["command"], "maps");
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn resolved_path_has_no_verbatim_prefix_and_detect_game_matches() {
    let d = temp("ok");
    let r = resolve_game_path(Some(d.to_str().unwrap())).unwrap();
    assert!(!r.to_string_lossy().starts_with(r"\\?\"), "{r:?}");
    assert!(r.is_dir());
    let (mut o, mut e) = (Vec::new(), Vec::new());
    assert_eq!(
        run_cli(
            &args(&["detect-game", "--game", d.to_str().unwrap(), "--json"]),
            &mut o,
            &mut e
        ),
        0
    );
    let v: serde_json::Value = serde_json::from_slice(&o).unwrap();
    let p = v["data"]["detectedPath"].as_str().unwrap();
    assert!(!p.starts_with(r"\\?\"), "{p}");
    assert_eq!(v["data"]["hasExe"], false);
    assert_eq!(PathBuf::from(p), r);
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn verbatim_prefix_stripping_is_conservative() {
    let s = |x: &str| strip_verbatim_prefix(PathBuf::from(x));
    assert_eq!(s(r"\\?\C:\Games\AR"), PathBuf::from(r"C:\Games\AR"));
    assert_eq!(s(r"\\?\UNC\srv\share\AR"), PathBuf::from(r"\\srv\share\AR"));
    assert_eq!(s(r"C:\Games\AR"), PathBuf::from(r"C:\Games\AR"));
    // Device/volume-GUID paths have no plain equivalent and stay untouched.
    assert_eq!(
        s(r"\\?\Volume{1234}\AR"),
        PathBuf::from(r"\\?\Volume{1234}\AR")
    );
    assert_eq!(s(r"\\?\C:"), PathBuf::from(r"\\?\C:"));
}
