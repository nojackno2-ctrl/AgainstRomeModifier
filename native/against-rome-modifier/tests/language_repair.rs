//! Synthetic temp-dir tests for the Repair-LanguageBackup.ps1 port.
use against_rome_modifier::{
    install::LANGUAGE_BACKUP,
    language_repair::{discover_original_in, repair_language_backup},
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Env {
    root: PathBuf,
    game: PathBuf,
    orig: PathBuf,
}
impl Env {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "arm-langrepair-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let (game, orig) = (root.join("game"), root.join("orig"));
        for (d, exe) in [(&game, b"G".as_slice()), (&orig, b"O".as_slice())] {
            fs::create_dir_all(d).unwrap();
            fs::write(d.join("Against_Rome.exe"), exe).unwrap();
        }
        fs::create_dir_all(game.join("ToEng/DATA")).unwrap();
        fs::create_dir_all(game.join("DATA")).unwrap();
        for (rel, ov, og) in [("a.txt", "EN-a", "CN-a"), ("DATA/b.txt", "EN-b", "CN-b")] {
            fs::write(game.join("ToEng").join(rel), ov).unwrap();
            fs::write(game.join(rel), ov).unwrap();
            if let Some(p) = orig.join(rel).parent() {
                fs::create_dir_all(p).unwrap();
            }
            fs::write(orig.join(rel), og).unwrap();
        }
        Self { root, game, orig }
    }
}
impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn err(e: Result<impl std::fmt::Debug, std::io::Error>) -> String {
    e.unwrap_err().to_string()
}
fn no_backup(game: &Path) {
    assert!(!game.join(LANGUAGE_BACKUP).exists());
    let leftovers = fs::read_dir(game)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp-"))
        .count();
    assert_eq!(leftovers, 0);
}

#[test]
fn creates_backup_with_original_bytes_and_manifest() {
    let e = Env::new();
    let r = repair_language_backup(&e.game, &e.orig, false).unwrap();
    assert_eq!(r.existing_files, ["DATA/b.txt", "a.txt"]);
    let b = e.game.join(LANGUAGE_BACKUP);
    assert_eq!(fs::read(b.join("files/a.txt")).unwrap(), b"CN-a");
    assert_eq!(fs::read(b.join("files/DATA/b.txt")).unwrap(), b"CN-b");
    let m: serde_json::Value = serde_json::from_slice(&fs::read(b.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(m["ExistingFiles"], serde_json::json!(["DATA/b.txt", "a.txt"]));
    assert_eq!(m["MissingFiles"], serde_json::json!([]));
    // Game files untouched; no temp leftovers.
    assert_eq!(fs::read(e.game.join("a.txt")).unwrap(), b"EN-a");
    assert_eq!(fs::read(e.orig.join("a.txt")).unwrap(), b"CN-a");
    // Compatible with LanguagePackFeature restore planning.
    let plan = against_rome_modifier::install::LanguagePackFeature::plan(&e.game, false).unwrap();
    assert_eq!(plan.writes.get(&e.game.join("a.txt")).unwrap(), b"CN-a");
}

#[test]
fn dry_run_validates_without_writing() {
    let e = Env::new();
    let r = repair_language_backup(&e.game, &e.orig, true).unwrap();
    assert!(r.dry_run && r.existing_files.len() == 2);
    no_backup(&e.game);
    fs::write(e.orig.join("a.txt"), "EN-a").unwrap();
    assert!(err(repair_language_backup(&e.game, &e.orig, true)).contains("identical to the overlay"));
}

#[test]
fn safety_checks_fail_closed() {
    let e = Env::new();
    // missing original file
    fs::remove_file(e.orig.join("DATA/b.txt")).unwrap();
    assert!(err(repair_language_backup(&e.game, &e.orig, false)).contains("original source is incomplete"));
    no_backup(&e.game);
    fs::write(e.orig.join("DATA/b.txt"), "CN-b").unwrap();
    // missing active file
    fs::remove_file(e.game.join("a.txt")).unwrap();
    assert!(err(repair_language_backup(&e.game, &e.orig, false)).contains("active language overlay is incomplete"));
    // active differs from overlay
    fs::write(e.game.join("a.txt"), "other").unwrap();
    assert!(err(repair_language_backup(&e.game, &e.orig, false)).contains("does not match the ToEng overlay"));
    fs::write(e.game.join("a.txt"), "EN-a").unwrap();
    // invalid directories
    fs::remove_file(e.orig.join("Against_Rome.exe")).unwrap();
    assert!(err(repair_language_backup(&e.game, &e.orig, false)).contains("Invalid original game directory"));
    assert!(err(repair_language_backup(&e.root, &e.orig, false)).contains("Invalid game directory"));
    no_backup(&e.game);
}

#[test]
fn existing_or_incomplete_backup_is_never_touched() {
    let e = Env::new();
    let b = e.game.join(LANGUAGE_BACKUP);
    fs::create_dir(&b).unwrap();
    fs::write(b.join("keep"), "x").unwrap();
    assert!(err(repair_language_backup(&e.game, &e.orig, false)).contains("incomplete managed language backup"));
    fs::write(b.join("manifest.json"), "{}").unwrap();
    assert!(err(repair_language_backup(&e.game, &e.orig, false)).contains("managed language backup already exists"));
    assert_eq!(fs::read(b.join("keep")).unwrap(), b"x");
}

#[test]
fn empty_or_missing_overlay_rejected() {
    let e = Env::new();
    fs::remove_dir_all(e.game.join("ToEng")).unwrap();
    assert!(repair_language_backup(&e.game, &e.orig, false).is_err());
    fs::create_dir(e.game.join("ToEng")).unwrap();
    assert!(err(repair_language_backup(&e.game, &e.orig, false)).contains("No language overlay files"));
    no_backup(&e.game);
}

#[test]
fn discovery_requires_exactly_one_candidate() {
    let e = Env::new();
    // Only the game directory has exe + ToEng here.
    assert_eq!(discover_original_in(&e.root).unwrap(), e.game);
    fs::create_dir(e.orig.join("ToEng")).unwrap();
    assert!(err(discover_original_in(&e.root)).contains("uniquely"));
    fs::remove_dir_all(e.game.join("ToEng")).unwrap();
    assert_eq!(discover_original_in(&e.root).unwrap(), e.orig);
    fs::remove_dir_all(e.orig.join("ToEng")).unwrap();
    assert!(err(discover_original_in(&e.root)).contains("uniquely"));
}

#[test]
fn cli_dry_run_then_real_run() {
    let e = Env::new();
    let run = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_against-rome-modifier"))
            .args(["repair-language-backup", "--game"])
            .arg(&e.game)
            .arg("--original")
            .arg(&e.orig)
            .args(extra)
            .output()
            .unwrap()
    };
    let o = run(&["--dry-run", "--json"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    no_backup(&e.game);
    let o = run(&["--json"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["data"]["existingCount"], 2);
    assert!(e.game.join(LANGUAGE_BACKUP).join("manifest.json").is_file());
    assert_eq!(run(&[]).status.code(), Some(1));
}
