//! Differential against tests/fixtures/csharp-cl-script-detector.json, frozen from the original
//! internal ClScriptFeatureDetector.Detect. The C# oracle source was retired with the .NET projects.
//! Every case is compared flag-by-flag from both an all-false and an all-true starting profile
//! (the latter proves reset behaviour, including the C# no-backup early return that keeps spell flags).
use against_rome_modifier::{detectors::ClScriptFeatureDetector, PatchProfile};
use serde_json::Value;

fn run(text: &str, original: Option<&str>, flags: &[&str], initial: bool) -> PatchProfile {
    let mut profile = PatchProfile::new();
    for flag in flags {
        profile.set_bool(*flag, initial);
    }
    ClScriptFeatureDetector::detect_text(text, original, &mut profile);
    profile
}

#[test]
fn csharp_cl_script_detector_matches_rust_flag_by_flag() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/csharp-cl-script-detector.json")).unwrap();
    let flags: Vec<&str> = fixture["Flags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let cases = fixture["Cases"].as_array().unwrap();
    assert!(cases.len() >= 50, "oracle fixture unexpectedly small");
    let mut mismatches = Vec::new();
    for case in cases {
        let name = case["Name"].as_str().unwrap();
        let text = case["Text"].as_str().unwrap();
        let original = case["Original"].as_str();
        for (key, initial) in [("FromFalse", false), ("FromTrue", true)] {
            let profile = run(text, original, &flags, initial);
            for flag in &flags {
                let expected = case[key][*flag].as_bool().unwrap();
                if profile.get_bool(flag) != expected {
                    mismatches.push(format!(
                        "{name} [{key}] {flag}: C#={expected} Rust={}",
                        !expected
                    ));
                }
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} Rust/C# mismatches:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
