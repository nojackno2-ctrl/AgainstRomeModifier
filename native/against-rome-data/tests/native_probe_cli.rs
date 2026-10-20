//! Subprocess checks: failure must be nonzero and must never print PASS.
use std::{
    path::Path,
    process::{Command, Output},
};
fn run(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ar-native-probe"))
        .args(args)
        .output()
        .unwrap()
}
fn rejected(output: Output, diagnostic: &str) {
    assert!(!output.status.success(), "unexpected success: {output:?}");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("PASS"));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(diagnostic),
        "{output:?}"
    );
}
#[test]
fn rejects_missing_relative_and_extra_arguments() {
    rejected(run(&[]), "usage:");
    rejected(run(&["against_rome_data.dll".as_ref()]), "must be absolute");
    rejected(run(&["one".as_ref(), "two".as_ref()]), "exactly one");
}
#[cfg(windows)]
#[test]
fn rejects_missing_file_and_non_dll_pe_without_fallback() {
    let missing =
        std::env::temp_dir().join(format!("absent-native-probe-{}.dll", std::process::id()));
    assert!(!missing.exists());
    rejected(run(&[missing.as_os_str()]), "FAIL:");
    // The probe executable is a valid PE, but does not have IMAGE_FILE_DLL.
    rejected(
        run(&[Path::new(env!("CARGO_BIN_EXE_ar-native-probe")).as_os_str()]),
        "x64 PE DLL",
    );
}
#[cfg(all(windows, target_arch = "x86_64"))]
#[test]
fn loaded_unrelated_library_is_rejected_for_missing_exports() {
    let dll = std::env::var_os("SystemRoot").expect("SystemRoot");
    let dll = Path::new(&dll).join("System32/kernel32.dll");
    rejected(
        run(&[dll.as_os_str()]),
        "missing export ar_light_abi_version",
    );
}
#[cfg(all(windows, target_arch = "x86_64"))]
#[test]
#[ignore = "explicit DLL gate: set AR_NATIVE_PROBE_TEST_DLL to an absolute freshly built DLL path"]
fn explicit_dll_passes_actual_load_and_pixel_calls() {
    let dll = std::env::var_os("AR_NATIVE_PROBE_TEST_DLL")
        .expect("AR_NATIVE_PROBE_TEST_DLL is required; no fallback");
    assert!(Path::new(&dll).is_absolute());
    // Exercise UTF-16 argument handling and an explicitly selected, renamed DLL.
    let dir = std::env::temp_dir().join(format!(
        "Rust ABI probe 測試 {}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    let selected = dir.join("selected library.dll");
    std::fs::copy(&dll, &selected).unwrap();
    let output = run(&[selected.as_os_str()]);
    // The child exited; its RAII library guard must have released the file.
    std::fs::remove_file(&selected).unwrap();
    std::fs::remove_dir(&dir).unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("PASS:"));
    assert!(output.stderr.is_empty(), "{output:?}");
}
