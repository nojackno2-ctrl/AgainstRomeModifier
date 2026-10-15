use against_rome_modifier::{FOCUS, ROMAN_ENDLESS, UNIT_RECRUIT_20};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
    exe: PathBuf,
    original: Vec<u8>,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "arm-modifier-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let exe = root.join("synthetic game.exe");
        let mut original = vec![0x55; FOCUS.offset + FOCUS.original.len() + 1];
        for p in [FOCUS, ROMAN_ENDLESS, UNIT_RECRUIT_20] {
            original[p.offset..p.offset + p.original.len()].copy_from_slice(p.original);
        }
        fs::write(&exe, &original).unwrap();
        Self {
            root,
            exe,
            original,
        }
    }
    fn command(&self) -> Command {
        Command::new(env!("CARGO_BIN_EXE_against-rome-modifier"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn cli_inspect_apply_reinspect_disable_roundtrip() {
    let f = Fixture::new();
    let inspect = f.command().arg(&f.exe).output().unwrap();
    assert!(inspect.status.success());
    assert_eq!(
        String::from_utf8(inspect.stdout)
            .unwrap()
            .matches("Original")
            .count(),
        3
    );
    assert_eq!(fs::read(&f.exe).unwrap(), f.original);
    let backup = f.root.join("original.bin");
    assert!(f
        .command()
        .arg("apply")
        .arg(&f.exe)
        .arg("--backup")
        .arg(&backup)
        .args(["--enable", "focus", "--enable", "unit-recruit-20"])
        .status()
        .unwrap()
        .success());
    assert_eq!(fs::read(&backup).unwrap(), f.original);
    let inspect = f.command().arg("inspect").arg(&f.exe).output().unwrap();
    assert!(inspect.status.success());
    assert_eq!(
        String::from_utf8(inspect.stdout)
            .unwrap()
            .matches("Patched")
            .count(),
        2
    );
    assert!(f
        .command()
        .arg("apply")
        .arg(&f.exe)
        .arg("--backup")
        .arg(f.root.join("patched.bin"))
        .args(["--disable", "focus", "--disable", "unit-recruit-20"])
        .status()
        .unwrap()
        .success());
    assert_eq!(fs::read(&f.exe).unwrap(), f.original);
}

#[test]
fn cli_rejects_invalid_arguments_without_writing() {
    let f = Fixture::new();
    for tail in [
        vec!["--enable", "bogus"],
        vec!["--enable", "focus"],
        vec!["--backup", "backup", "--typo"],
        vec![
            "--backup",
            "backup",
            "--enable",
            "focus",
            "--disable",
            "focus",
        ],
    ] {
        assert!(!f
            .command()
            .arg("apply")
            .arg(&f.exe)
            .args(tail)
            .output()
            .unwrap()
            .status
            .success());
        assert_eq!(fs::read(&f.exe).unwrap(), f.original);
    }
    assert!(!f
        .command()
        .args(["--unknown"])
        .output()
        .unwrap()
        .status
        .success());
    assert!(!f
        .command()
        .arg("inspect")
        .arg(&f.exe)
        .arg("--enable")
        .arg("focus")
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 1);
}
