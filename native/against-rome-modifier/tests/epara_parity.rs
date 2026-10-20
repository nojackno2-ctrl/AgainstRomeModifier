//! Parity tests comparing Rust clean and modified epara with C# implementations and baseline files.

use against_rome_modifier::{decompress_pfil, is_pfil, CleanEparaBaseline, EparaPatcher};
use std::fs;
use std::path::Path;

const RETAIL_EPARA_PATH: &str =
    "../../re_workspace/cd-modifier-baseline-20261010/files/SYSTEM/cl_epara.ini";

#[test]
fn clean_epara_baseline_text_matches_frozen_csharp_text() {
    let frozen: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/csharp-frozen-tables.json")).unwrap();
    let cs_text = frozen["epara_text"].as_str().unwrap();

    // Normalize newlines for comparison
    let cs_normalized = cs_text.replace("\r\n", "\n");
    let rust_normalized = CleanEparaBaseline::TEXT.replace("\r\n", "\n");

    assert_eq!(
        rust_normalized, cs_normalized,
        "Rust CleanEparaBaseline text must match C# verbatim"
    );
}

#[test]
fn clean_epara_baseline_compressed_roundtrips_and_patches_identically() {
    let clean_bytes = CleanEparaBaseline::create_bytes().expect("create compressed clean epara");
    assert!(is_pfil(&clean_bytes));

    let decompressed = decompress_pfil(&clean_bytes).expect("decompress clean epara");
    assert_eq!(
        String::from_utf8_lossy(&decompressed).replace("\r\n", "\n"),
        CleanEparaBaseline::TEXT.replace("\r\n", "\n")
    );

    // Apply Range3x patch to clean epara
    let patched_bytes =
        EparaPatcher::get_patched_bytes(&clean_bytes, true).expect("apply range 3x patch");
    assert!(is_pfil(&patched_bytes));

    let patched_decomp = decompress_pfil(&patched_bytes).expect("decompress patched");
    let patched_str = String::from_utf8_lossy(&patched_decomp);

    // Verify ProjectileVarianceOnMove became 0.0
    assert!(
        patched_str.contains("[ProjectileVarianceOnMove]\r\n0.0\r\n")
            || patched_str.contains("[ProjectileVarianceOnMove]\n0.0\n"),
        "variance must be patched to 0.0"
    );

    // Verify ProjectileInitSpeedFactor 1.5 * 1.08 = 1.62
    assert!(
        patched_str.contains("[ProjectileInitSpeedFactor]\r\n1.62\r\n")
            || patched_str.contains("[ProjectileInitSpeedFactor]\n1.62\n"),
        "init speed factor must be scaled by 1.08 to 1.62"
    );

    // Verify unpatched range3x=false returns identical bytes
    let unpatched_bytes =
        EparaPatcher::get_patched_bytes(&clean_bytes, false).expect("unpatched call");
    assert_eq!(unpatched_bytes, clean_bytes);
}

#[test]
fn retail_cd_baseline_epara_patches_and_preserves_roundtrip() {
    let retail_path = Path::new(RETAIL_EPARA_PATH);
    if !retail_path.exists() {
        eprintln!("Retail baseline path not found; skipping retail check");
        return;
    }

    let retail_bytes = fs::read(retail_path).expect("read retail cl_epara.ini");
    assert!(is_pfil(&retail_bytes), "retail cl_epara.ini must be PFIL");

    let decompressed = decompress_pfil(&retail_bytes).expect("decompress retail");
    let orig_text = String::from_utf8_lossy(&decompressed);
    assert!(orig_text.contains("[ProjectileVarianceOnMove]"));
    assert!(orig_text.contains("[ProjectileInitSpeedFactor]"));

    // Patch retail file with range3x
    let patched_retail =
        EparaPatcher::get_patched_bytes(&retail_bytes, true).expect("patch retail epara");
    assert!(is_pfil(&patched_retail));

    let patched_decomp = decompress_pfil(&patched_retail).expect("decompress patched retail");
    let patched_text = String::from_utf8_lossy(&patched_decomp);
    assert!(patched_text.contains("0.0"));
    assert!(patched_text.contains("1.62"));

    // Restore / no-op
    let restored_retail =
        EparaPatcher::get_patched_bytes(&retail_bytes, false).expect("restore retail epara");
    assert_eq!(
        restored_retail, retail_bytes,
        "false option must leave retail bytes verbatim"
    );
}
