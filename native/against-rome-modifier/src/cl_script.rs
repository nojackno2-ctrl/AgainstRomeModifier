//! cl_script.ini patcher ported faithfully from src.Core/Core/Patches/ClScriptPatcher.cs.

use crate::patch_text::TextDocument;
use std::collections::HashMap;
use std::io;

#[derive(Debug, Clone)]
pub struct ClScriptOptions {
    pub fast_civilian_production: bool,
    pub infinite_morale: bool,
    pub spell_damage_5x: bool,
    pub spell_healing_10x: bool,
    pub spell_resurrection: bool,
    pub general_skills_5x: bool,
    pub german_spell_radius_multiplier: f64,
    pub celt_spell_radius_multiplier: f64,
    pub hun_spell_radius_multiplier: f64,
    pub spell_range_3x: bool,
    pub original_bytes: Option<Vec<u8>>,
}

impl Default for ClScriptOptions {
    fn default() -> Self {
        Self {
            fast_civilian_production: false,
            infinite_morale: false,
            spell_damage_5x: false,
            spell_healing_10x: false,
            spell_resurrection: false,
            general_skills_5x: false,
            german_spell_radius_multiplier: 1.0,
            celt_spell_radius_multiplier: 1.0,
            hun_spell_radius_multiplier: 1.0,
            spell_range_3x: false,
            original_bytes: None,
        }
    }
}

pub const DAMAGE_SPELL_KEYS: &[&str] = &[
    "GER_Spell2",
    "HUN_Spell0",
    "HUN_Spell1",
    "HUN_Spell2",
    "KEL_Spell0",
    "KEL_Spell2",
];

pub struct ClScriptPatcher;

impl ClScriptPatcher {
    pub fn get_patched_bytes(original: &[u8], options: &ClScriptOptions) -> io::Result<Vec<u8>> {
        let canonical_bytes = options.original_bytes.as_deref().unwrap_or(original);
        let orig_doc = TextDocument::read(canonical_bytes)?;
        let doc = TextDocument::read(original)?;

        let original_radius = Self::read_radius_values(&orig_doc.lines);
        let original_spells = Self::read_spell_values(&orig_doc.lines);
        let original_abilities = Self::read_ability_values(&orig_doc.lines);

        let mut output_lines = Vec::with_capacity(doc.lines.len());

        for line in &doc.lines {
            let processed = Self::patch_line(
                line,
                options,
                &orig_doc.lines,
                &original_radius,
                &original_spells,
                &original_abilities,
            );
            output_lines.push(processed);
        }

        doc.write_with_lines(&output_lines)
    }

    fn split_comment(line: &str) -> (&str, &str) {
        if let Some(pos) = line.find(';') {
            (&line[..pos], &line[pos..])
        } else {
            (line, "")
        }
    }

    fn read_radius_values(lines: &[String]) -> HashMap<String, f64> {
        let mut map = HashMap::new();
        for line in lines {
            let (body, _) = Self::split_comment(line);
            if let Some((key_part, rest)) = body.split_once('=') {
                if key_part.trim().eq_ignore_ascii_case("Radius") {
                    let parts: Vec<&str> = rest.split(',').collect();
                    if parts.len() >= 3 {
                        let faction = parts[0].trim();
                        let spell = parts[1].trim();
                        if let Ok(val) = parts[2].trim().parse::<f64>() {
                            map.insert(format!("{}_{}", faction, spell), val);
                        }
                    }
                }
            }
        }
        map
    }

    fn read_spell_values(lines: &[String]) -> HashMap<String, f64> {
        let mut map = HashMap::new();
        for line in lines {
            let (body, _) = Self::split_comment(line);
            if let Some((key_part, rest)) = body.split_once('=') {
                let key = key_part.trim();
                if key.starts_with("Value") {
                    let parts: Vec<&str> = rest.split(',').collect();
                    if parts.len() >= 3 && parts[1].trim().starts_with("Spell") {
                        let faction = parts[0].trim();
                        let spell = parts[1].trim();
                        if let Ok(val) = parts[2].trim().parse::<f64>() {
                            map.insert(format!("{}_{}_{}", key, faction, spell), val);
                        }
                    }
                }
            }
        }
        map
    }

    fn read_ability_values(lines: &[String]) -> HashMap<String, f64> {
        let mut map = HashMap::new();
        for line in lines {
            let (body, _) = Self::split_comment(line);
            if let Some((key_part, rest)) = body.split_once('=') {
                let key = key_part.trim();
                if key.starts_with("Value") {
                    let parts: Vec<&str> = rest.split(',').collect();
                    if parts.len() >= 3 && parts[1].trim().starts_with("SAbility") {
                        let faction = parts[0].trim();
                        let ability = parts[1].trim();
                        if let Ok(val) = parts[2].trim().parse::<f64>() {
                            map.insert(format!("{}_{}_{}", key, faction, ability), val);
                        }
                    }
                }
            }
        }
        map
    }

    fn find_faction_value(lines: &[String], key_name: &str, faction: &str, fallback: f64) -> f64 {
        for line in lines {
            let (body, _) = Self::split_comment(line);
            if let Some((k, rest)) = body.split_once('=') {
                if k.trim().eq_ignore_ascii_case(key_name) {
                    let parts: Vec<&str> = rest.split(',').collect();
                    if parts.len() >= 2 && parts[0].trim().eq_ignore_ascii_case(faction) {
                        if let Ok(val) = parts[1].trim().parse::<f64>() {
                            return val;
                        }
                    }
                }
            }
        }
        fallback
    }

    fn patch_line(
        line: &str,
        options: &ClScriptOptions,
        orig_lines: &[String],
        original_radius: &HashMap<String, f64>,
        original_spells: &HashMap<String, f64>,
        original_abilities: &HashMap<String, f64>,
    ) -> String {
        let (body, comment) = Self::split_comment(line);
        let Some((key_part, rest)) = body.split_once('=') else {
            return line.to_string();
        };

        let key = key_part.trim();

        // 1. Radius
        if key.eq_ignore_ascii_case("Radius") {
            let parts: Vec<&str> = rest.split(',').collect();
            if parts.len() >= 3 {
                let faction = parts[0].trim();
                let spell = parts[1].trim();
                let original_val = original_radius
                    .get(&format!("{}_{}", faction, spell))
                    .copied()
                    .or_else(|| parts[2].trim().parse::<f64>().ok())
                    .unwrap_or(0.0);

                let mut multiplier = match faction {
                    "GER" => options.german_spell_radius_multiplier,
                    "KEL" => options.celt_spell_radius_multiplier,
                    "HUN" => options.hun_spell_radius_multiplier,
                    _ => 1.0,
                };
                if options.spell_range_3x {
                    multiplier *= 3.0;
                }
                let new_val = (original_val * multiplier) as i64;
                return format!(
                    "Radius     ={0}, {1}, {2:<10}{3}",
                    faction, spell, new_val, comment
                );
            }
        }

        // 2. Spell / Ability Value
        if key.starts_with("Value") {
            let parts: Vec<&str> = rest.split(',').collect();
            if parts.len() >= 3 {
                let faction = parts[0].trim();
                let second = parts[1].trim();
                if second.starts_with("Spell") {
                    let spell = second;
                    let orig_val = original_spells
                        .get(&format!("{}_{}_{}", key, faction, spell))
                        .copied()
                        .or_else(|| parts[2].trim().parse::<f64>().ok())
                        .unwrap_or(0.0);

                    let mut value = orig_val as i64;
                    if options.spell_damage_5x
                        && key == "Value"
                        && DAMAGE_SPELL_KEYS.contains(&format!("{}_{}", faction, spell).as_str())
                    {
                        value = (orig_val * 5.0) as i64;
                    }
                    if options.spell_healing_10x
                        && faction == "KEL"
                        && spell == "Spell1"
                        && key == "Value"
                    {
                        value = (orig_val * 10.0) as i64;
                    }
                    if options.spell_resurrection
                        && faction == "KEL"
                        && spell == "Spell3"
                        && (key == "Value" || key == "Value2")
                    {
                        value = 100;
                    }
                    return format!(
                        "{0:<10} ={1}, {2}, {3:<10}{4}",
                        key, faction, spell, value, comment
                    );
                } else if second.starts_with("SAbility") {
                    let ability = second;
                    let mut val = original_abilities
                        .get(&format!("{}_{}_{}", key, faction, ability))
                        .copied()
                        .or_else(|| parts[2].trim().parse::<f64>().ok())
                        .unwrap_or(0.0);

                    if options.general_skills_5x {
                        val *= 5.0;
                    }
                    return format!(
                        "{0:<10} ={1}, {2}, {3:<10}{4}",
                        key, faction, ability, val as i64, comment
                    );
                }
            }
        }

        // 3. CiviDelay
        if key.eq_ignore_ascii_case("CiviDelay") {
            let parts: Vec<&str> = rest.split(',').collect();
            if parts.len() >= 2 {
                let faction = parts[0].trim();
                let value = if options.fast_civilian_production {
                    500.0
                } else {
                    Self::find_faction_value(orig_lines, "CiviDelay", faction, 5000.0)
                };
                return format!(
                    "CiviDelay  ={0}, {1:<10}{2}",
                    faction, value as i64, comment
                );
            }
        }

        // 4. LPIncIdle
        if key.eq_ignore_ascii_case("LPIncIdle") {
            let parts: Vec<&str> = rest.split(',').collect();
            if parts.len() >= 2 {
                let faction = parts[0].trim();
                let value = Self::find_faction_value(orig_lines, "LPIncIdle", faction, 15000.0);
                return format!(
                    "LPIncIdle       ={0}, {1:<10}{2}",
                    faction, value as i64, comment
                );
            }
        }

        // 5. Morale
        if options.infinite_morale {
            if key.eq_ignore_ascii_case("MoralsDecLostMem") {
                return Self::replace_morale_val(line, "0");
            }
            if key.eq_ignore_ascii_case("MoralsDecFlee") {
                return Self::replace_morale_val(line, "0");
            }
            if key.eq_ignore_ascii_case("MoralsDecOverPop") {
                return Self::replace_morale_val(line, "99999999");
            }
            if key.eq_ignore_ascii_case("MoralsIncIdle") {
                return Self::replace_morale_val(line, "500");
            }
        } else if key.starts_with("Morals") {
            let parts: Vec<&str> = rest.split(',').collect();
            if !parts.is_empty() {
                let faction = parts[0].trim();
                for orig in orig_lines {
                    let (obody, _) = Self::split_comment(orig);
                    if let Some((ok, orest)) = obody.split_once('=') {
                        if ok.trim().eq_ignore_ascii_case(key) {
                            let oparts: Vec<&str> = orest.split(',').collect();
                            if !oparts.is_empty() && oparts[0].trim().eq_ignore_ascii_case(faction)
                            {
                                return orig.clone();
                            }
                        }
                    }
                }
            }
        }

        line.to_string()
    }

    fn replace_morale_val(line: &str, new_val: &str) -> String {
        let (body, comment) = Self::split_comment(line);
        if let Some((prefix, _val_part)) = body.split_once(',') {
            format!("{0}, {1}{2}", prefix, new_val, comment)
        } else {
            line.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cl_script_fast_civi_and_morale_roundtrip() {
        let sample = "\
; cl_script
CiviDelay  = GER, 5000       ; civilian delay
MoralsDecLostMem = GER, 10   ; lost memory
MoralsDecFlee    = GER, 20   ; flee
MoralsDecOverPop = GER, 30   ; overpop
MoralsIncIdle    = GER, 40   ; idle
Radius     = GER, Spell0, 100        ; radius
Value      = GER, Spell2, 50         ; damage spell
";
        let raw = sample.as_bytes();
        let options = ClScriptOptions {
            fast_civilian_production: true,
            infinite_morale: true,
            spell_damage_5x: true,
            ..Default::default()
        };

        let patched = ClScriptPatcher::get_patched_bytes(raw, &options).unwrap();
        let text = String::from_utf8(patched).unwrap();

        assert!(text.contains("CiviDelay  =GER, 500       ; civilian delay"));
        assert!(
            text.contains("MoralsDecLostMem = GER, 0; lost memory")
                || text.contains("MoralsDecLostMem = GER, 0   ; lost memory")
        );
        assert!(
            text.contains("MoralsDecFlee    = GER, 0; flee")
                || text.contains("MoralsDecFlee    = GER, 0   ; flee")
        );
        assert!(
            text.contains("MoralsDecOverPop = GER, 99999999; overpop")
                || text.contains("MoralsDecOverPop = GER, 99999999   ; overpop")
        );
        assert!(
            text.contains("MoralsIncIdle    = GER, 500; idle")
                || text.contains("MoralsIncIdle    = GER, 500   ; idle")
        );
        assert!(text.contains("Value      =GER, Spell2, 250       ; damage spell"));

        // Disable options and restore
        let restored = ClScriptPatcher::get_patched_bytes(
            &text.into_bytes(),
            &ClScriptOptions {
                original_bytes: Some(raw.to_vec()),
                ..Default::default()
            },
        )
        .unwrap();
        let restored_text = String::from_utf8(restored).unwrap();
        assert!(
            restored_text.contains("CiviDelay  =GER, 5000")
                || restored_text.contains("CiviDelay  = GER, 5000")
        );
        assert!(restored_text.contains("MoralsDecLostMem = GER, 10"));
    }
}
