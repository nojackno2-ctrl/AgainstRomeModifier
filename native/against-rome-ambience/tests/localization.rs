use against_rome_ambience::localization::*;
use std::collections::BTreeMap;

#[test]
fn escape_unescape_round_trip() {
    let s = "a\"b\\c\nd";
    assert_eq!(PutScriptFormatting::unescape(&PutScriptFormatting::escape(s)), s);
}

#[test]
fn catalog_keys() {
    assert_eq!(BriefingTextTableCatalog::get_team_key(3), "briefing_text_teamname3");
    assert_eq!(BriefingTextTableCatalog::try_get_team_index("briefing_text_teamname5"), Some(5));
    assert_eq!(BriefingTextTableCatalog::try_get_team_index("briefing_titel_1"), None);
    assert_eq!(BriefingTextTableCatalog::get_all_standard_descriptors().len(), 19);
    assert!(BriefingTextTableCatalog::try_get_descriptor("briefing_titel_1").unwrap().is_required);
}

#[test]
fn table_round_trip() {
    let mut t = BriefingTextTable::create_empty();
    t.set_title1("Hello");
    t.set_video_number(7);
    t.set_team_name(2, "Rome");
    let back = BriefingTextTable::parse(&t.to_script_text()).unwrap();
    assert_eq!(back.title1(), Some("Hello"));
    assert_eq!(back.video_number(), Some(7));
    assert_eq!(back.get_team_name(2), Some("Rome"));
}

#[test]
fn validator_flags_unencodable_text() {
    let mut t = BriefingTextTable::create_empty();
    t.set_title1("\u{1F600}");
    assert!(!PutScriptSyntaxValidator::validate_table(&t, "windows-1252").is_valid());
    let mut ok = BriefingTextTable::create_empty();
    ok.set_title1("Fine");
    assert!(PutScriptSyntaxValidator::validate_table(&ok, "windows-1252").is_valid());
}

#[test]
fn studio_parity_tracks_missing_and_stale() {
    let mut tables = BTreeMap::new();
    let mut zh = BriefingTextTable::create_empty();
    zh.set_title1("title");
    tables.insert(SupportedLanguage::TraditionalChinese, zh);
    let mut s = MultiLanguageTextStudio::new();
    s.load_from_tables(&tables);
    s.refresh_parity_status();
    assert!(s.calculate_coverage(SupportedLanguage::TraditionalChinese) > 0.0);
    assert!(s.export_to_tables().contains_key(&SupportedLanguage::TraditionalChinese));
}

#[test]
fn save_transaction_writes_per_language_and_preflight_rejects() {
    let dir = crate::temp_fixture::dir("ar_amb");
    let mut tables = BTreeMap::new();
    let mut en = BriefingTextTable::create_empty();
    en.set_title1("Map");
    tables.insert(SupportedLanguage::English, en);
    BriefingSaveTransaction::save_multi_language_briefings(&dir, &tables).unwrap();
    let p = dir.join("TEXT").join(SupportedLanguage::English.to_directory_code()).join("briefing.put");
    assert!(p.exists());
    let mut bad = BriefingTextTable::create_empty();
    bad.set_title1("\u{1F600}");
    let mut btables = BTreeMap::new();
    btables.insert(SupportedLanguage::English, bad);
    assert!(BriefingSaveTransaction::preflight_validate(&btables).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
