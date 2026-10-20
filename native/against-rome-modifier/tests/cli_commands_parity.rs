use against_rome_modifier::cli::{apply_options, run_cli, CliOptions};
use against_rome_modifier::PatchProfile;
use serde_json::Value;
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
            "arm-commands-{}-{}",
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
fn run(t: &Tmp, args: &[&str]) -> (i32, Value) {
    let mut args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    args.extend([
        "--game".into(),
        t.0.to_string_lossy().into_owned(),
        "--json".into(),
    ]);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_cli(&args, &mut out, &mut err);
    assert!(err.is_empty());
    (code, serde_json::from_slice(&out).unwrap())
}

#[test]
fn stats_get_list_and_exports_have_csharp_shapes_and_bom() {
    let t = Tmp::new();
    let (code, v) = run(&t, &["stats", "get", "figkelpri00_priester", "--balance"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["key"], "FigKelPri00_Priester");
    assert!(v["data"]["hp"].as_f64().unwrap() > 0.);
    let (code, v) = run(&t, &["stats", "list"]);
    assert_eq!(code, 0);
    assert_eq!(v["data"].as_array().unwrap().len(), 43);
    for format in ["ini", "json"] {
        let path = t.0.join(format!("stats.{format}"));
        let (code, v) = run(
            &t,
            &[
                "stats",
                "export",
                "--format",
                format,
                "--output",
                path.to_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{v}");
        assert_eq!(v["data"]["exportedUnitsCount"], 43);
        let raw = fs::read(path).unwrap();
        assert!(raw.starts_with(&[239, 187, 191]));
        if format == "json" {
            let list: Value = serde_json::from_slice(&raw[3..]).unwrap();
            assert!(list[0]["Key"].is_string());
            assert!(list[0]["Hp"].is_number());
        } else {
            assert!(String::from_utf8(raw[3..].to_vec())
                .unwrap()
                .contains("# Format: UnitKey=HP,Dmg,VW,AW,Sight,Relt\r\n"));
        }
    }
}

#[test]
fn exported_bom_profile_can_be_imported_without_game_writes() {
    let t = Tmp::new();
    let path = t.0.join("profile.json");
    let (code, v) = run(
        &t,
        &["profile", "export", "--output", path.to_str().unwrap()],
    );
    assert_eq!(code, 0, "{v}");
    let raw = fs::read(&path).unwrap();
    assert!(raw.starts_with(&[239, 187, 191]));
    let options = CliOptions {
        profile: Some(path.to_string_lossy().into_owned()),
        ..Default::default()
    };
    let p = apply_options(&options, PatchProfile::new()).unwrap();
    assert_eq!(p.get_int("GameSpeed"), 1);
    assert_eq!(p.get_int("VillageGarrisonQuotaMultiplier"), 1);
    assert!(p.custom_unit_stats.is_none());
    assert!(!t.0.join("Against_Rome.exe.bak").exists());
}

#[test]
fn help_schema_backup_and_text_renderers_are_available() {
    let t = Tmp::new();
    let (code, v) = run(&t, &["profile", "schema"]);
    assert_eq!(code, 0);
    assert_eq!(v["data"]["type"], "AgainstRome.PatchProfile");
    assert_eq!(v["data"]["availableFeatures"].as_array().unwrap().len(), 49);
    let (code, v) = run(&t, &["backup", "status"]);
    assert_eq!(code, 0);
    assert!(v["data"]["missingCount"].as_u64().unwrap() > 0);
    let (code, v) = run(&t, &["help"]);
    assert_eq!(code, 0);
    assert!(v["data"]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["name"] == "saves"));
    let args = vec!["profile".into(), "schema".into()];
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(run_cli(&args, &mut out, &mut err), 0);
    assert!(String::from_utf8(out)
        .unwrap()
        .contains("AgainstRome.PatchProfile"));
    assert!(err.is_empty());
}

fn run_raw(args: &[&str]) -> (i32, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_cli(&args, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn version_flags_report_cargo_version_without_game_path() {
    let expected = format!("{} {}\n", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    for flags in [
        &["--version"][..],
        &["-v"],
        &["--VERSION"],
        &["--cli", "-v"],
    ] {
        let (code, out, err) = run_raw(flags);
        assert_eq!(
            (code, out.as_str(), err.as_str()),
            (0, expected.as_str(), "")
        );
    }
    for flags in [&["--version", "--json"][..], &["-v", "--json"]] {
        let (code, out, err) = run_raw(flags);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(v["success"], true);
        assert_eq!(v["command"], "version");
        assert_eq!(v["data"]["name"], env!("CARGO_PKG_NAME"));
        assert_eq!(v["data"]["version"], env!("CARGO_PKG_VERSION"));
    }
}

#[test]
fn version_does_not_touch_game_directory() {
    let t = Tmp::new();
    let missing = t.0.join("does-not-exist");
    let (code, out, err) = run_raw(&["--version", "--game", missing.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(out.contains(env!("CARGO_PKG_VERSION")) && err.is_empty());
    assert!(!missing.exists() && fs::read_dir(&t.0).unwrap().next().is_none());
    // `-v` next to a command is still --verbose, not version.
    let (_, out, _) = run_raw(&["help", "-v"]);
    assert!(!out.starts_with(env!("CARGO_PKG_NAME")));
}
