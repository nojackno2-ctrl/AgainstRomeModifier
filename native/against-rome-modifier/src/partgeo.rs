//! partgeo.dau patcher ported faithfully from src.Core/Core/Patches/PartgeoPatcher.cs.

use crate::patch_text::{check_length, parse_csv_line, to_csv_string, TextDocument};
use std::io::{self, Error, ErrorKind};

pub const PROJECTILE_GEO_NAMES: &[&str] = &[
    "Wurfspeer00",
    "Wurfaxt00",
    "Katapultstein00",
    "Katapultstein01",
    "Pfeil00",
    "Spiess00",
    "Fackel",
];

pub const ARC_YSUB_MULTIPLIER: f64 = 2.0;
pub const YSUB_COLUMN: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PartgeoOptions {
    pub projectile_arc_height: bool,
}

pub struct PartgeoPatcher;

impl PartgeoPatcher {
    pub fn get_patched_bytes(original: &[u8], options: &PartgeoOptions) -> io::Result<Vec<u8>> {
        if !options.projectile_arc_height {
            return Ok(original.to_vec());
        }

        let doc = TextDocument::read(original)?;
        let mut lines = doc.lines.clone();
        let mut patched_count = 0;

        for line in &mut lines {
            let mut cols = parse_csv_line(line);
            if cols.len() <= YSUB_COLUMN {
                continue;
            }

            let name = cols[2].trim();
            if !PROJECTILE_GEO_NAMES.contains(&name) {
                continue;
            }

            let ysub: i64 = cols[YSUB_COLUMN].trim().parse().map_err(|_| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("partgeo.dau entry {} ysub value invalid or unparseable", name),
                )
            })?;
            if ysub <= 0 {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("partgeo.dau entry {} ysub is not positive", name),
                ));
            }

            let new_value = (ysub as f64 * ARC_YSUB_MULTIPLIER).round() as i64;
            let new_val_str = new_value.to_string();
            let width = cols[YSUB_COLUMN].len();

            if let Some(fitted) = check_length(&new_val_str, width) {
                let padded = format!("{:>width$}", fitted, width = width);
                cols[YSUB_COLUMN] = padded;
                *line = to_csv_string(cols.iter().map(|s| s.as_str()));
                patched_count += 1;
            } else {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "partgeo.dau entry {} ysub {} exceeds field width {}",
                        name, new_val_str, width
                    ),
                ));
            }
        }

        if patched_count != PROJECTILE_GEO_NAMES.len() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "partgeo.dau projectile count mismatch: expected {}, found {}",
                    PROJECTILE_GEO_NAMES.len(),
                    patched_count
                ),
            ));
        }

        doc.write_with_lines(&lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partgeo_scales_all_7_projectiles() {
        let mut rows = vec!["header1".to_string(), "header2".to_string()];
        for name in PROJECTILE_GEO_NAMES {
            let mut cols = vec!["     0".to_string(); 20];
            cols[2] = format!("{:>20}", name);
            cols[12] = "   100".to_string();
            rows.push(cols.join(","));
        }
        let raw = rows.join("\r\n") + "\r\n";

        let options = PartgeoOptions { projectile_arc_height: true };
        let patched = PartgeoPatcher::get_patched_bytes(raw.as_bytes(), &options).unwrap();
        let s = String::from_utf8(patched).unwrap();

        for line in s.lines().skip(2) {
            let cols = parse_csv_line(line);
            assert_eq!(cols[12].trim(), "200");
        }

        let untouched = PartgeoPatcher::get_patched_bytes(raw.as_bytes(), &PartgeoOptions::default()).unwrap();
        assert_eq!(untouched, raw.as_bytes());
    }
}
