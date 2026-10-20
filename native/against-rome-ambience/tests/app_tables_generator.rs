#[path = "../tools/app_tables.rs"]
mod generator;

use std::{ffi::OsString, fs, path::PathBuf};

const CANONICAL: &str = include_str!("../tools/app_tables.json");
const FROZEN: &str = include_str!("fixtures/app_tables_expected.json");

#[test]
fn canonical_matches_every_frozen_entry_in_order() {
    // A typed schema also rejects missing/extra/duplicate table names and bad pairs.
    let canonical: generator::Tables = serde_json::from_str(CANONICAL).unwrap();
    let oracle: generator::Tables = serde_json::from_str(FROZEN).unwrap();
    assert_eq!(canonical, oracle);
}

#[test]
fn generated_bytes_match_checked_in_file() {
    assert_eq!(
        generator::render(CANONICAL).unwrap().as_bytes(),
        include_bytes!("../src/app_tables.rs")
    );
}

#[test]
fn escapes_rust_literals_without_changing_unicode_or_placeholders() {
    assert_eq!(
        generator::rust_literal("\"\\\r\n\t\0\u{8}\u{c}\u{1f}\u{7f}\u{85}繁體🙂{0}"),
        r#""\"\\\r\n\t\0\u{8}\u{c}\u{1f}\u{7f}\u{85}繁體🙂{0}""#
    );
    assert_eq!(generator::rust_literal(""), "\"\"");
    assert_eq!(generator::rust_literal("'\\n"), r#""'\\n""#);
}

#[test]
fn renderer_preserves_pair_order_duplicates_and_json_escapes() {
    let json = r#"{"ZH":[["z","\"\\\r\n\t\u0000"],["a","繁體🙂"],["z",""]],"EN":[],"EN_UNIT_NAMES":[],"ZH_UNIT_NAMES":[]}"#;
    assert_eq!(
        generator::render(json).unwrap(),
        format!(
            "{}{}",
            generator::HEADER,
            concat!(
                "pub const ZH: &[(&str, &str)] = &[\n",
                "    (\"z\", \"\\\"\\\\\\r\\n\\t\\0\"),\n",
                "    (\"a\", \"繁體🙂\"),\n",
                "    (\"z\", \"\"),\n",
                "];\n",
                "pub const EN: &[(&str, &str)] = &[\n];\n",
                "pub const EN_UNIT_NAMES: &[(&str, &str)] = &[\n];\n",
                "pub const ZH_UNIT_NAMES: &[(&str, &str)] = &[\n];\n"
            )
        )
    );
}

#[test]
fn rejects_invalid_schema_instead_of_silently_dropping_data() {
    for json in [
        "{}",
        r#"{"ZH":[],"EN":[],"EN_UNIT_NAMES":[],"ZH_UNIT_NAMES":[],"extra":[]}"#,
        r#"{"ZH":[],"ZH":[],"EN":[],"EN_UNIT_NAMES":[],"ZH_UNIT_NAMES":[]}"#,
        r#"{"ZH":[["key"]],"EN":[],"EN_UNIT_NAMES":[],"ZH_UNIT_NAMES":[]}"#,
        r#"{"ZH":[["key",42]],"EN":[],"EN_UNIT_NAMES":[],"ZH_UNIT_NAMES":[]}"#,
    ] {
        assert!(generator::render(json).is_err(), "{json}");
    }
}

struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn cli_check_detects_staleness_and_never_writes() {
    let dir = TempDir(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("test-output")
            .join(uuid::Uuid::new_v4().to_string()),
    );
    fs::create_dir_all(&dir.0).unwrap();
    let output = dir.0.join("generated tables.rs");
    let args = || {
        vec![
            OsString::from("--check"),
            OsString::from("--output"),
            output.clone().into_os_string(),
        ]
    };
    assert!(generator::run(args()).is_err());
    assert!(!output.exists());

    fs::write(&output, b"stale\r\n").unwrap();
    let modified = fs::metadata(&output).unwrap().modified().unwrap();
    assert!(generator::run(args())
        .unwrap_err()
        .contains("stale generated tables"));
    assert_eq!(fs::read(&output).unwrap(), b"stale\r\n");
    assert_eq!(fs::metadata(&output).unwrap().modified().unwrap(), modified);

    generator::run([OsString::from("--output"), output.clone().into_os_string()]).unwrap();
    let bytes = fs::read(&output).unwrap();
    let modified = fs::metadata(&output).unwrap().modified().unwrap();
    generator::run(args()).unwrap();
    assert_eq!(fs::metadata(&output).unwrap().modified().unwrap(), modified);
    generator::run([OsString::from("--output"), output.clone().into_os_string()]).unwrap();
    assert_eq!(fs::read(&output).unwrap(), bytes);
    assert_eq!(bytes, generator::render(CANONICAL).unwrap().as_bytes());

    for invalid in [
        vec!["--unknown"],
        vec!["--output"],
        vec!["--check", "--check"],
    ] {
        assert!(generator::run(invalid.into_iter().map(OsString::from)).is_err());
    }
    assert_eq!(fs::read(&output).unwrap(), bytes);
}
