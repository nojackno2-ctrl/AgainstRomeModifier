//! Read-only checks against the installed game (`cargo test -- --ignored`).
//! Expected values were compared byte-for-byte (modulo CRLF / instruction spelling) with the Python originals.
use ar_re_tools::cmd::{engine_config, verify_24_pools};
use std::path::Path;

const GAME: &str = r"C:\Program Files (x86)\Against Rome";

#[test]
#[ignore = "needs the installed game"]
fn engine_table_has_36_sections_and_40_options() {
    let exe = std::fs::read(Path::new(GAME).join("Against_Rome.exe")).unwrap();
    let report = engine_config::inspect(&exe).unwrap();
    assert_eq!(report.get("sections").unwrap().len(), 36);
    assert_eq!(report.get("game_options").unwrap().len(), 40);
    assert_eq!(report.get("embedded_default_headers").unwrap().len(), 10);
}

#[test]
#[ignore = "needs the installed game"]
fn all_74_maps_have_24_well_formed_pools() {
    let report = verify_24_pools::validate(&Path::new(GAME).join("MAPS")).unwrap();
    assert_eq!(report.get("total_files_checked").and_then(|v| v.as_i128()), Some(1776));
    assert_eq!(report.get("anomaly_count").and_then(|v| v.as_i128()), Some(0));
}
