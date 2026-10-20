use against_rome_ambience::app_localization::*;
use std::{fs, path::PathBuf};
fn owned_temp() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-output").join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&path).unwrap(); path
}
#[test]
fn all_application_tables_match_frozen_managed_tables() {
    // Tables frozen from the former managed (C#) Localization/TroopConfig sources.
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/app_tables_expected.json");
    let frozen: std::collections::BTreeMap<String, Vec<(String, String)>> =
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    for (name, actual) in [("ZH", ZH), ("EN", EN), ("EN_UNIT_NAMES", EN_UNIT_NAMES), ("ZH_UNIT_NAMES", ZH_UNIT_NAMES)] {
        let expected = &frozen[name];
        assert_eq!(expected.len(), actual.len(), "{name}");
        for ((ek, ev), (ak, av)) in expected.iter().zip(actual) {
            assert_eq!((ek.as_str(), ev.as_str()), (*ak, *av), "{name}");
        }
    }
}
#[test]
fn lookup_case_and_fallback_match_managed_behavior() {
    let en = Loc::new(Language::English); let zh = Loc::default();
    assert_eq!(en.get("BrowseButton"), "Browse...");
    assert_eq!(zh.get("BrowseButton"), "瀏覽...");
    assert_eq!(en.get("browsebutton"), "browsebutton");
    assert_eq!(en.get_unit_name("figromanf00_anfuehrer"), "Roman Leader");
    assert_eq!(zh.get_unit_name("figromanf00_anfuehrer"), "figromanf00_anfuehrer");
    assert_eq!(zh.get_unit_type("ranged_cav"), "遠程部隊");
    assert_eq!(en.get_style_text("shield"), "Shielded");
    assert_eq!(zh.get_tier_text("ace"), "王牌");
    assert_eq!(zh.get_faction_name("Roman"), "羅馬");
    assert_eq!(zh.get_faction_name("Custom"), "Custom");
}
#[test]
fn preferences_legacy_language_and_empty_promotions_are_preserved() {
    let dir = owned_temp(); let path = dir.join("settings.json");
    for value in [r#""English""#, "1", r#""eNgLiSh""#] {
        fs::write(&path, format!(r#"{{"Language":{value},"PromotedFeatures":[]}}"#)).unwrap();
        assert_eq!(Loc::load(&path,"zh-TW").settings.language, Language::English);
    }
    fs::write(&path, r#"{"Language":0,"PromotedFeatures":["A","a","",4,"B"]}"#).unwrap();
    let mut loc = Loc::load(&path,"en-US");
    assert_eq!(loc.settings.language, Language::TraditionalChinese);
    assert_eq!(loc.settings.promoted_features, ["A","B"]);
    loc.promote_feature("a"); loc.demote_feature("b"); loc.set_language(Language::English);
    let loaded = Loc::load(&path,"zh-TW");
    assert_eq!(loaded.settings.language, Language::English);
    assert_eq!(loaded.settings.promoted_features, ["A"]);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn corrupt_or_unrecognized_preferences_use_culture() {
    let dir = owned_temp(); let path = dir.join("settings.json");
    for text in ["{", "[]", r#"{"Language":"other"}"#] {
        fs::write(&path,text).unwrap();
        assert_eq!(Loc::load(&path,"en-GB").settings.language, Language::English);
        assert_eq!(Loc::load(&path,"de-DE").settings.language, Language::TraditionalChinese);
    }
    fs::remove_dir_all(dir).unwrap();
}
