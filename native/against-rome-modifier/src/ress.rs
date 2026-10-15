//! ress.ini patcher ported faithfully from src.Core/Core/Patches/RessPatcher.cs.

use crate::patch_text::{parse_csv_line, to_csv_string, TextDocument};
use std::io;

pub const BAU_BUILD_COST_START: usize = 1;
pub const BAU_BUILD_COST_END: usize = 6;
pub const BAU_UPGRADE_COST_START: usize = 7;
pub const BAU_UPGRADE_COST_END: usize = 12;

pub const FIG_PROD_COST_START: usize = 13;
pub const FIG_PROD_COST_END: usize = 18;
pub const FIG_PRIEST_SPELL_COST_START: usize = 25;
pub const FIG_PRIEST_SPELL_COST_END: usize = 28;
pub const FIG_SIEGE_BUILD_COST_START: usize = 1;
pub const FIG_SIEGE_BUILD_COST_END: usize = 6;

pub const RESEARCH_UPGRADE_WOOD1: usize = 8;
pub const RESEARCH_UPGRADE_GOLD1: usize = 10;
pub const RESEARCH_UPGRADE_WOOD2: usize = 12;
pub const RESEARCH_UPGRADE_GOLD2: usize = 14;
pub const TECH_COST_START: usize = 24;
pub const TECH_COST_END: usize = 263;
pub const UNIT_UPGRADE_START: usize = 264;
pub const UNIT_UPGRADE_END: usize = 295;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RessOptions {
    pub free_production: bool,
    pub free_upgrades: bool,
    pub no_spell_cost: bool,
}

pub struct RessPatcher;

impl RessPatcher {
    pub fn get_patched_bytes(original: &[u8], options: &RessOptions) -> io::Result<Vec<u8>> {
        let doc = TextDocument::read(original)?;
        let mut output_lines = Vec::with_capacity(doc.lines.len());
        let mut in_objres = false;
        let mut in_volkres = false;

        for line in &doc.lines {
            let stripped = line.trim();
            if stripped.starts_with('[') {
                in_objres = stripped == "[objres]";
                in_volkres = stripped == "[volkres]";
                output_lines.push(line.clone());
                continue;
            }

            if in_objres && line.contains(',') {
                let mut cols = parse_csv_line(line);
                if cols.is_empty() {
                    output_lines.push(line.clone());
                    continue;
                }
                let name = cols[0].trim();
                if name.starts_with("Bau") {
                    if cols.len() < 10 {
                        output_lines.push(line.clone());
                        continue;
                    }
                    Self::rewrite(&mut cols, |i, _| {
                        (options.free_upgrades
                            && i >= BAU_UPGRADE_COST_START
                            && i <= BAU_UPGRADE_COST_END)
                            || (options.free_production
                                && i >= BAU_BUILD_COST_START
                                && i <= BAU_BUILD_COST_END)
                    });
                    output_lines.push(to_csv_string(cols.iter().map(|s| s.as_str())));
                } else if name.starts_with("Fig") {
                    if cols.len() < 29 || name.eq_ignore_ascii_case("FigTiePac00_Packpferd") {
                        output_lines.push(line.clone());
                        continue;
                    }
                    let siege = name.contains("Art") || name.contains("Bar") || name.contains("Fal");
                    let priest = name.contains("Pri") || name.contains("Dru");
                    Self::rewrite(&mut cols, |i, _| {
                        (options.free_production && i >= FIG_PROD_COST_START && i <= FIG_PROD_COST_END)
                            || (options.free_production
                                && siege
                                && i >= FIG_SIEGE_BUILD_COST_START
                                && i <= FIG_SIEGE_BUILD_COST_END)
                            || (options.no_spell_cost
                                && priest
                                && i >= FIG_PRIEST_SPELL_COST_START
                                && i <= FIG_PRIEST_SPELL_COST_END)
                    });
                    output_lines.push(to_csv_string(cols.iter().map(|s| s.as_str())));
                } else {
                    output_lines.push(line.clone());
                }
            } else if in_volkres && line.contains(',') {
                let mut cols = parse_csv_line(line);
                if cols.len() < 3 {
                    output_lines.push(line.clone());
                    continue;
                }
                Self::rewrite(&mut cols, |i, _| {
                    options.free_upgrades
                        && (i == RESEARCH_UPGRADE_WOOD1
                            || i == RESEARCH_UPGRADE_GOLD1
                            || i == RESEARCH_UPGRADE_WOOD2
                            || i == RESEARCH_UPGRADE_GOLD2
                            || (i >= TECH_COST_START && i <= TECH_COST_END && i % 2 == 0)
                            || (i >= UNIT_UPGRADE_START && i <= UNIT_UPGRADE_END))
                });
                output_lines.push(to_csv_string(cols.iter().map(|s| s.as_str())));
            } else {
                output_lines.push(line.clone());
            }
        }

        doc.write_with_lines(&output_lines)
    }

    fn rewrite(columns: &mut [String], zero: impl Fn(usize, &str) -> bool) {
        for i in 1..columns.len() {
            let val = columns[i].trim();
            if !val.is_empty() && zero(i, val) {
                columns[i] = "0".to_string();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ress_cost_patch_zeros_selected_fields() {
        let building = format!("BauHau00,{}", vec!["10"; 12].join(","));
        let unit = format!("FigGerPri00_Priester,{}", vec!["5"; 28].join(","));
        let raw = format!("[objres]\n{}\n{}\n", building, unit);

        let options = RessOptions {
            free_production: true,
            free_upgrades: true,
            no_spell_cost: true,
        };

        let patched_bytes = RessPatcher::get_patched_bytes(raw.as_bytes(), &options).unwrap();
        let patched_text = String::from_utf8(patched_bytes).unwrap();
        let lines: Vec<&str> = patched_text.split('\n').collect();

        // Building columns 1..=6 and 7..=12 should be "0"
        let b_cols: Vec<&str> = lines[1].split(',').collect();
        for &c in &b_cols[1..13] {
            assert_eq!(c, "0");
        }

        // Unit columns 13..=18 and 25..=28 should be "0"
        let u_cols: Vec<&str> = lines[2].split(',').collect();
        for &c in &u_cols[13..19] {
            assert_eq!(c, "0");
        }
        for &c in &u_cols[25..29] {
            assert_eq!(c, "0");
        }
    }
}
