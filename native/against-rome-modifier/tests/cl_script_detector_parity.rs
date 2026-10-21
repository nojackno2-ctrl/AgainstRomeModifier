//! 舊 C# 的第一筆抽樣、整數截斷與寬鬆倍率均不是完整功能的判準。
use against_rome_modifier::{detectors::ClScriptFeatureDetector, PatchProfile};
fn detect(text: &str, original: Option<&str>) -> PatchProfile {
    let mut p = PatchProfile::new();
    ClScriptFeatureDetector::detect_text(text, original, &mut p);
    p
}
fn morale(faction: &str, lost: &str, flee: &str, over: &str, idle: &str) -> String {
    format!("MoralsDecLostMem={faction},{lost}\nMoralsDecFlee={faction},{flee}\nMoralsDecOverPop={faction},{over}\nMoralsIncIdle={faction},{idle}\n")
}
#[test]
fn every_morale_faction_and_duplicate_must_be_valid() {
    let good = morale("GER", "0", "0", "99999999", "500");
    assert!(detect(&good, None).get_bool("InfiniteMorale"));
    for text in [
        format!("{good}{}", morale("HUN", "0", "1", "99999999", "500")),
        format!("{good}{good}"),
        morale("KEL", "0", "0", "99999999", "500.7"),
        morale("ROM", "0", "0", "99999998", "500"),
        morale("GER", "NaN", "0", "99999999", "500"),
    ] {
        assert!(!detect(&text, None).get_bool("InfiniteMorale"));
    }
}
#[test]
fn civi_delay_requires_exact_value_on_every_row() {
    assert!(detect("CiviDelay=GER,500\nCiviDelay=ROM,500", None).get_bool("FastCiviProduction"));
    for text in [
        "CiviDelay=GER,100",
        "CiviDelay=GER,500.7",
        "CiviDelay=GER,500\nCiviDelay=ROM,bad",
        "CiviDelay=GER,500\nCiviDelay=GER,500",
    ] {
        assert!(!detect(text, None).get_bool("FastCiviProduction"));
    }
}
#[test]
fn no_backup_or_partial_spell_input_clears_stale_flags() {
    let mut p = PatchProfile::new();
    for id in [
        "SpellDamage5x",
        "SpellHealing10x",
        "SpellResurrection",
        "SpellRange3x",
        "GeneralSkills",
    ] {
        p.set_bool(id, true);
    }
    ClScriptFeatureDetector::detect_text("", None, &mut p);
    for id in [
        "SpellDamage5x",
        "SpellHealing10x",
        "SpellResurrection",
        "SpellRange3x",
        "GeneralSkills",
    ] {
        assert!(!p.get_bool(id));
    }
}
#[test]
fn resurrection_checks_both_fields_exactly() {
    let orig = "Value=KEL,Spell3,50\nValue2=KEL,Spell3,60";
    assert!(
        detect("Value=KEL,Spell3,100\nValue2=KEL,Spell3,100", Some(orig))
            .get_bool("SpellResurrection")
    );
    for text in [
        "Value=KEL,Spell3,100",
        "Value=KEL,Spell3,100\nValue2=KEL,Spell3,100.7",
        "Value=KEL,Spell3,100\nValue2=KEL,Spell3,60",
    ] {
        assert!(!detect(text, Some(orig)).get_bool("SpellResurrection"));
    }
    assert!(
        !detect("Value=KEL,Spell3,100", Some("Value=KEL,Spell3,100")).get_bool("SpellResurrection")
    );
}
#[test]
fn legal_whitespace_comments_and_unrelated_fields_are_tolerated() {
    let orig = "Value=GER,Spell2,10";
    assert!(
        detect("  Value = GER, Spell2, 50 ; 傷害\nOther=3", Some(orig)).get_bool("SpellDamage5x")
    );
    assert!(!detect("Value=GER,Spell2,50.7", Some(orig)).get_bool("SpellDamage5x"));
}
#[test]
fn all_ability_slots_and_radius_entries_must_match() {
    let orig =
        "Radius=GER,Spell1,10\nRadius=HUN,Spell2,20\nValue=GER,SAbility1,4\nValue2=HUN,SAbility3,6";
    let good="Radius=GER,Spell1,30\nRadius=HUN,Spell2,60\nValue=GER,SAbility1,20\nValue2=HUN,SAbility3,30";
    let p = detect(good, Some(orig));
    assert!(p.get_bool("SpellRange3x"));
    assert!(p.get_bool("GeneralSkills"));
    assert!(!detect(&good.replace("Spell2,60", "Spell2,150"), Some(orig)).get_bool("SpellRange3x"));
    assert!(
        !detect(&good.replace("SAbility3,30", "SAbility3,6"), Some(orig)).get_bool("GeneralSkills")
    );
    assert!(
        !detect(&good.replace("Value2=HUN,SAbility3,30", ""), Some(orig)).get_bool("GeneralSkills")
    );
}
