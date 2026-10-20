//! RessFeatureDetector must detect exactly what RessPatcher writes, using the same
//! RessIndex/VolkresIndex ranges and first-match line semantics as the C# detector.
use against_rome_modifier::{
    detectors::RessFeatureDetector, PatchProfile, RessOptions, RessPatcher,
};

fn row(name: &str, cols: usize) -> String {
    format!("{name},{}", vec!["7"; cols - 1].join(","))
}

fn ress_text(extra_unit_row: &str) -> String {
    [
        "[objres]".to_string(),
        row("FigRomInf00_Lanze_Schild", 29),
        row("FigGerPri00_Priester", 29),
        "[volkres]".to_string(),
        row("Ger_Kampf", 300),
        extra_unit_row.to_string(),
        String::new(),
    ]
    .join("\r\n")
}

fn detect(text: &str) -> PatchProfile {
    let mut profile = PatchProfile::new();
    RessFeatureDetector::detect_text(text, &mut profile);
    profile
}

fn patched(text: &str, options: RessOptions) -> String {
    let bytes = RessPatcher::get_patched_bytes(text.as_bytes(), &options).unwrap();
    String::from_utf8(bytes).unwrap()
}

#[test]
fn unpatched_ress_reports_no_free_features() {
    let profile = detect(&ress_text(""));
    for id in ["FreeProduction", "FreeUpgrade", "NoSpellCost"] {
        assert!(!profile.get_bool(id), "{id}");
    }
}

#[test]
fn each_patch_option_is_detected_independently() {
    let original = ress_text("");
    let production = detect(&patched(
        &original,
        RessOptions {
            free_production: true,
            ..Default::default()
        },
    ));
    assert!(production.get_bool("FreeProduction"));
    assert!(!production.get_bool("FreeUpgrade") && !production.get_bool("NoSpellCost"));

    let upgrades = detect(&patched(
        &original,
        RessOptions {
            free_upgrades: true,
            ..Default::default()
        },
    ));
    assert!(upgrades.get_bool("FreeUpgrade"));
    assert!(!upgrades.get_bool("FreeProduction") && !upgrades.get_bool("NoSpellCost"));

    let spells = detect(&patched(
        &original,
        RessOptions {
            no_spell_cost: true,
            ..Default::default()
        },
    ));
    assert!(spells.get_bool("NoSpellCost"));
    assert!(!spells.get_bool("FreeProduction") && !spells.get_bool("FreeUpgrade"));
}

#[test]
fn only_the_first_matching_line_decides() {
    // A later, differently priced duplicate must not flip the result (C# uses Regex.Match).
    let original = ress_text(&row("Ger_Kampf2", 300));
    let all = RessOptions {
        free_production: true,
        free_upgrades: true,
        no_spell_cost: true,
    };
    let mut text = patched(&original, all);
    text.push_str(&row("FigRomInf00_Lanze_Schild", 29));
    let profile = detect(&text);
    assert!(
        profile.get_bool("FreeProduction")
            && profile.get_bool("FreeUpgrade")
            && profile.get_bool("NoSpellCost")
    );
}

#[test]
fn partially_zeroed_ranges_are_not_free() {
    let text = ress_text("");
    let patched_text = patched(
        &text,
        RessOptions {
            free_production: true,
            ..Default::default()
        },
    );
    let mut cols: Vec<String> = patched_text
        .lines()
        .find(|l| l.starts_with("FigRomInf00_Lanze_Schild"))
        .unwrap()
        .split(',')
        .map(String::from)
        .collect();
    cols[18] = "5".into();
    let tampered = patched_text.replace(
        patched_text
            .lines()
            .find(|l| l.starts_with("FigRomInf00_Lanze_Schild"))
            .unwrap(),
        &cols.join(","),
    );
    assert!(!detect(&tampered).get_bool("FreeProduction"));
}
