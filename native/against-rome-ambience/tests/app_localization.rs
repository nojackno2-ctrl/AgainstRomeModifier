use against_rome_ambience::app_localization::*;
use std::{fs, path::PathBuf};
fn owned_temp() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-output").join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&path).unwrap(); path
}
#[test]
fn all_application_tables_match_csharp_source() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (file, name, actual) in [
        ("src.Core/Core/Localization.cs", "Zh", ZH),
        ("src.Core/Core/Localization.cs", "En", EN),
        ("src.Core/Core/Localization.cs", "EnUnitNames", EN_UNIT_NAMES),
        ("src.Core/Core/TroopConfig.cs", "UnitNames", ZH_UNIT_NAMES),
    ] {
        let source = fs::read_to_string(root.join(file)).unwrap();
        let start = source.find(&format!("Dictionary<string, string> {name} =")).unwrap();
        let section = source[start..].split("};").next().unwrap();
        let mut expected = Vec::new();
        for line in section.lines().skip(1) {
            let line = line.trim();
            if line.starts_with('{') {
                let pair = line.trim_end_matches(',');
                let pair = format!("[{}]", &pair[1..pair.len()-1]);
                let values: Vec<String> = serde_json::from_str(&pair).unwrap();
                expected.push((values[0].clone(), values[1].clone()));
            }
        }
        assert_eq!(expected.len(), actual.len(), "{name}");
        for ((ek,ev),(ak,av)) in expected.iter().zip(actual) { assert_eq!((ek.as_str(),ev.as_str()),(*ak,*av), "{name}"); }
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
