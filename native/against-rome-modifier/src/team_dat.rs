//! team.dat patcher ported faithfully from src.Core/Core/Patches/TeamDatPatcher.cs.

use crate::patch_text::{parse_csv_line, to_csv_string, TextDocument};
use std::io;

pub const DEFAULT_POPULATION_LIMIT: i32 = 1600;
pub const ROMAN_FACTION_TOKEN: &str = "ROM";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeamDatOptions {
    pub max_population: bool,
    pub population_limit: i32,
    pub roman_player: bool,
}

impl Default for TeamDatOptions {
    fn default() -> Self {
        Self {
            max_population: false,
            population_limit: DEFAULT_POPULATION_LIMIT,
            roman_player: false,
        }
    }
}

pub struct TeamDatPatcher;

impl TeamDatPatcher {
    pub fn get_patched_bytes(original: &[u8], options: &TeamDatOptions) -> io::Result<Vec<u8>> {
        if !options.max_population && !options.roman_player {
            return Ok(original.to_vec());
        }

        let doc = TextDocument::read(original)?;
        let mut output_lines = doc.lines.clone();
        let mut in_teamdata = false;

        for line in &mut output_lines {
            let stripped = line.trim();
            if stripped.starts_with('[') {
                in_teamdata = stripped == "[teamdata]";
                continue;
            }
            if !in_teamdata || !line.contains(',') {
                continue;
            }

            let mut cols = parse_csv_line(line);
            let mut changed = false;

            if options.roman_player && cols.len() >= 2 && cols[0].trim() == "0" {
                cols[1] = ROMAN_FACTION_TOKEN.to_string();
                changed = true;
            }

            if options.max_population && cols.len() >= 5 {
                if let Ok(value) = cols[4].trim().parse::<i32>() {
                    if value > 0 {
                        cols[4] = options.population_limit.to_string();
                        changed = true;
                    }
                }
            }

            if changed {
                *line = to_csv_string(cols.iter().map(|s| s.as_str()));
            }
        }

        doc.write_with_lines(&output_lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_dat_patch_sets_positive_population_and_restores() {
        let sample = "[teamdata]\nA,B,C,D,80,F\nA,B,C,D,0,F\n";
        let options = TeamDatOptions {
            max_population: true,
            population_limit: 1600,
            roman_player: false,
        };

        let patched_bytes = TeamDatPatcher::get_patched_bytes(sample.as_bytes(), &options).unwrap();
        let patched_str = String::from_utf8(patched_bytes).unwrap();

        assert!(patched_str.contains("A,B,C,D,1600,F"));
        assert!(patched_str.contains("A,B,C,D,0,F")); // Zero pop unaffected

        // No options: untouched
        let untouched = TeamDatPatcher::get_patched_bytes(sample.as_bytes(), &TeamDatOptions::default()).unwrap();
        assert_eq!(untouched, sample.as_bytes());
    }

    #[test]
    fn team_dat_sets_roman_player() {
        let sample = "[teamdata]\n0,GER,C,D,80,F\n";
        let options = TeamDatOptions {
            max_population: false,
            population_limit: 1600,
            roman_player: true,
        };
        let patched = TeamDatPatcher::get_patched_bytes(sample.as_bytes(), &options).unwrap();
        let s = String::from_utf8(patched).unwrap();
        assert!(s.contains("0,ROM,C,D,80,F"));
    }
}
