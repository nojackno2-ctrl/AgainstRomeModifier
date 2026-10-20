use against_rome_editor_app::modifier_ui::{default_state_dir, ModifierUi};
use eframe::egui;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("{}_{}", prefix, Uuid::new_v4().simple()));
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
    PathBuf::from(env!("CARGO_BIN_EXE_against-rome-modifier-gui"))
}

#[test]
fn test_modifier_binary_help_and_version() {
    let bin = bin_path();
    assert!(bin.is_file(), "binary must exist at {}", bin.display());

    // --help
    let out_help = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("run --help");
    assert!(out_help.status.success());
    let stdout_help = String::from_utf8_lossy(&out_help.stdout);
    assert!(stdout_help.contains("against-rome-modifier-gui"));
    assert!(stdout_help.contains("--smoke-test"));

    // -h
    let out_h = Command::new(&bin).arg("-h").output().expect("run -h");
    assert!(out_h.status.success());
    let stdout_h = String::from_utf8_lossy(&out_h.stdout);
    assert_eq!(stdout_help, stdout_h);

    // --version
    let out_ver = Command::new(&bin)
        .arg("--version")
        .output()
        .expect("run --version");
    assert!(out_ver.status.success());
    let stdout_ver = String::from_utf8_lossy(&out_ver.stdout);
    assert_eq!(stdout_ver.trim(), "against-rome-modifier-gui 0.1.0");

    // -v
    let out_v = Command::new(&bin).arg("-v").output().expect("run -v");
    assert!(out_v.status.success());
    let stdout_v = String::from_utf8_lossy(&out_v.stdout);
    assert_eq!(stdout_v.trim(), "against-rome-modifier-gui 0.1.0");
}

#[test]
fn test_modifier_binary_smoke_test_headless() {
    let bin = bin_path();
    let temp = TempDirGuard::new("arm_modifier_bin_smoke");

    let out = Command::new(&bin)
        .arg("--state-dir")
        .arg(&temp.path)
        .arg("--smoke-test")
        .output()
        .expect("run --smoke-test");

    assert!(
        out.status.success(),
        "smoke test must succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Native desktop smoke test completed: three UI updates executed."),
        "stdout was: {}",
        stdout
    );
}

#[test]
fn test_modifier_binary_invalid_and_duplicate_args() {
    let bin = bin_path();

    // Unknown argument
    let out_unknown = Command::new(&bin)
        .arg("--invalid-flag")
        .output()
        .expect("run invalid");
    assert!(!out_unknown.status.success());
    let stderr = String::from_utf8_lossy(&out_unknown.stderr);
    assert!(stderr.contains("unknown or duplicate argument: --invalid-flag"));

    // Duplicate --smoke-test
    let out_dup = Command::new(&bin)
        .arg("--smoke-test")
        .arg("--smoke-test")
        .output()
        .expect("run duplicate");
    assert!(!out_dup.status.success());
    let stderr_dup = String::from_utf8_lossy(&out_dup.stderr);
    assert!(stderr_dup.contains("unknown or duplicate argument: --smoke-test"));

    // Missing parameter for --state-dir
    let out_missing = Command::new(&bin)
        .arg("--state-dir")
        .output()
        .expect("run missing state dir");
    assert!(!out_missing.status.success());
    let stderr_missing = String::from_utf8_lossy(&out_missing.stderr);
    assert!(stderr_missing.contains("--state-dir requires a directory"));
}

#[test]
fn test_modifier_standalone_state_persisted_helper() {
    let temp = TempDirGuard::new("arm_modifier_state_helper");
    let state_dir = temp.path.join("modifier_state");

    // Test that default_state_dir is valid and non-empty
    let def = default_state_dir();
    assert!(
        def.to_string_lossy().contains("modifier"),
        "default_state_dir should contain 'modifier': {}",
        def.display()
    );

    // Initial load from custom isolated state directory
    let mut modifier = ModifierUi::persisted(state_dir.clone());
    modifier.open = true;

    let ctx = egui::Context::default();
    for _ in 0..3 {
        let _ = ctx.run(Default::default(), |ctx| {
            modifier.show(ctx);
        });
    }

    assert!(modifier.open);
    assert_eq!(modifier.model.state_dir, Some(state_dir));
}

#[test]
fn test_modifier_gui_binary_rejects_cli_subcommands_without_opening_window() {
    let bin = bin_path();

    // CLI subcommands belong to the native modifier CLI; the GUI binary must
    // fail fast with the shared error instead of falling through to a window.
    for sub in ["features", "apply", "inspect", "troops"] {
        let out = Command::new(&bin).arg(sub).output().expect("run subcommand");
        assert!(!out.status.success(), "{sub} must not start the GUI");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains(&format!("unknown or duplicate argument: {sub}")),
            "stderr for {sub} was: {stderr}"
        );
    }

    // --help stays authoritative even when followed by an unknown argument.
    let out = Command::new(&bin)
        .args(["--help", "features"])
        .output()
        .expect("run help with trailing arg");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("against-rome-modifier-gui"));
}
