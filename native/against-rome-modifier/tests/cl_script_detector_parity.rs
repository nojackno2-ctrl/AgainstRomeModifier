//! ClScriptFeatureDetector must follow the C# regex semantics: first-match morale lines,
//! Int32 digit-only parsing, and a missing resurrection key means "not detected".
use against_rome_modifier::{detectors::ClScriptFeatureDetector, PatchProfile};

fn detect(text: &str, original: Option<&str>) -> PatchProfile {
    let mut profile = PatchProfile::new();
    ClScriptFeatureDetector::detect_text(text, original, &mut profile);
    profile
}

fn morale(lost: &str, flee: &str, over: &str, idle: &str) -> String {
    format!(
        "MoralsDecLostMem=GER,{lost}\r\nMoralsDecFlee=GER,{flee}\r\nMoralsDecOverPop=GER,{over}\r\nMoralsIncIdle=GER,{idle}\r\n"
    )
}

#[test]
fn infinite_morale_uses_first_match_only() {
    assert!(detect(&morale("0", "0", "99999999", "500"), None).get_bool("InfiniteMorale"));
    // A later patched duplicate must not rescue an unpatched first line (C# Regex.Match).
    let text = format!(
        "{}{}",
        morale("5", "0", "99999999", "500"),
        morale("0", "0", "99999999", "500")
    );
    assert!(!detect(&text, None).get_bool("InfiniteMorale"));
    // ...and a later unpatched duplicate must not break a patched first line.
    let text = format!(
        "{}{}",
        morale("0", "0", "99999999", "500"),
        morale("5", "5", "1", "1")
    );
    assert!(detect(&text, None).get_bool("InfiniteMorale"));
}

#[test]
fn morale_integer_parsing_matches_dotnet() {
    // Regex \d+ has no sign: "-0" does not match, so the whole feature is false.
    assert!(!detect(&morale("-0", "0", "99999999", "500"), None).get_bool("InfiniteMorale"));
    assert!(!detect(&morale("+0", "0", "99999999", "500"), None).get_bool("InfiniteMorale"));
    // Int32 overflow makes int.TryParse fail (i64 would have accepted it).
    assert!(!detect(&morale("0", "0", "99999999999", "500"), None).get_bool("InfiniteMorale"));
    // Leading zeros and trailing fractions: regex captures only the digit run.
    assert!(detect(&morale("000", "0", "0099999999", "500.7"), None).get_bool("InfiniteMorale"));
    // Threshold is >= 99999999.
    assert!(!detect(&morale("0", "0", "99999998", "500"), None).get_bool("InfiniteMorale"));
    // Missing line, wrong faction.
    assert!(!detect("MoralsDecLostMem=GER,0\r\n", None).get_bool("InfiniteMorale"));
    assert!(!detect(
        &morale("0", "0", "99999999", "500").replace("MoralsDecFlee=GER", "MoralsDecFlee=ROM"),
        None
    )
    .get_bool("InfiniteMorale"));
}

#[test]
fn infinite_morale_resets_to_false() {
    let mut profile = PatchProfile::new();
    profile.set_bool("InfiniteMorale", true);
    profile.set_bool("FastCiviProduction", true);
    ClScriptFeatureDetector::detect_text("", None, &mut profile);
    assert!(!profile.get_bool("InfiniteMorale"));
    assert!(!profile.get_bool("FastCiviProduction"));
}

#[test]
fn civi_delay_all_matches_digits_only() {
    assert!(
        detect("CiviDelay=GER,100\r\nCiviDelay=ROM,500\r\n", None).get_bool("FastCiviProduction")
    );
    assert!(
        !detect("CiviDelay=GER,100\r\nCiviDelay=ROM,501\r\n", None).get_bool("FastCiviProduction")
    );
    assert!(!detect("CiviDelay=GER,0\r\n", None).get_bool("FastCiviProduction"));
    assert!(!detect("CiviDelay=GER,-5\r\n", None).get_bool("FastCiviProduction"));
    assert!(!detect("CiviDelay=ger,5\r\n", None).get_bool("FastCiviProduction"));
    assert!(detect("CiviDelay = GER , 250\r\n", None).get_bool("FastCiviProduction"));
}

const ORIGINAL: &str = "Value=KEL,Spell3,50\r\nValue2=KEL,Spell3,60\r\nValue=GER,Spell3,10\r\n";

#[test]
fn resurrection_missing_current_key_is_not_detected() {
    // Both keys patched.
    assert!(detect(
        "Value=KEL,Spell3,100\r\nValue2=KEL,Spell3,100\r\n",
        Some(ORIGINAL)
    )
    .get_bool("SpellResurrection"));
    // Value2 missing in current: C# returns false (Rust used to skip and return true).
    assert!(!detect("Value=KEL,Spell3,100\r\n", Some(ORIGINAL)).get_bool("SpellResurrection"));
    // Value patched, Value2 left at original.
    assert!(!detect(
        "Value=KEL,Spell3,100\r\nValue2=KEL,Spell3,60\r\n",
        Some(ORIGINAL)
    )
    .get_bool("SpellResurrection"));
    // Key already 100 in the original is skipped; nothing else => false.
    assert!(
        !detect("Value=KEL,Spell3,100\r\n", Some("Value=KEL,Spell3,100\r\n"))
            .get_bool("SpellResurrection")
    );
    // Without a backup nothing is evaluated.
    assert!(
        !detect("Value=KEL,Spell3,100\r\nValue2=KEL,Spell3,100\r\n", None)
            .get_bool("SpellResurrection")
    );
}

#[test]
fn keyed_values_are_line_anchored_and_case_sensitive() {
    let orig = "Value=GER,Spell2,10\r\n";
    assert!(detect("Value=GER,Spell2,50\r\n", Some(orig)).get_bool("SpellDamage5x"));
    // Leading whitespace breaks the ^ anchor in C#, so the value is missing => false.
    assert!(!detect("  Value=GER,Spell2,50\r\n", Some(orig)).get_bool("SpellDamage5x"));
    // Lower-case faction does not match [A-Z]{3}.
    assert!(!detect("Value=ger,Spell2,50\r\n", Some(orig)).get_bool("SpellDamage5x"));
    // Trailing comment is stripped.
    assert!(detect("Value=GER,Spell2,50 ; x5\r\n", Some(orig)).get_bool("SpellDamage5x"));
}

#[test]
fn radius_and_abilities() {
    let orig = "Radius=GER,Spell1,10\r\nValue=GER,SAbility1,4\r\n";
    assert!(detect(
        "Radius=GER,Spell1,30\r\nValue=GER,SAbility1,20\r\n",
        Some(orig)
    )
    .get_bool("SpellRange3x"));
    assert!(detect(
        "Radius=GER,Spell1,30\r\nValue=GER,SAbility1,20\r\n",
        Some(orig)
    )
    .get_bool("GeneralSkills"));
    assert!(!detect(
        "Radius=GER,Spell1,31\r\nValue=GER,SAbility1,4\r\n",
        Some(orig)
    )
    .get_bool("SpellRange3x"));
    assert!(!detect(
        "Radius=GER,Spell1,30\r\nValue=GER,SAbility1,4\r\n",
        Some(orig)
    )
    .get_bool("GeneralSkills"));
}
