//! Read-only settings for water tools and design diagnostics. Layer sizes come
//! from BMPs, never from Heightmapstep (which scales vertical heights).
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct MapSettings {
    pub height_step: f32,
    pub water_world: f32,
}
impl Default for MapSettings {
    fn default() -> Self { Self { height_step: 4.0, water_world: 0.0 } }
}
impl MapSettings {
    pub fn parse(input: &[u8]) -> Result<Self, String> {
        let decoded = if input.starts_with(b"PFIL") {
            against_rome_data::lzss::decompress_pfil(input, None).map_err(|e| e.to_string())?
        } else { input.to_vec() };
        let text = String::from_utf8_lossy(&decoded);
        let mut key = String::new();
        let mut result = Self::default();
        for raw in text.lines() {
            // German inline comments follow the closing section bracket too.
            let line = raw.split(';').next().unwrap_or("").trim();
            if line.is_empty() { continue; }
            if let Some(after) = line.strip_prefix('[') {
                if let Some(end) = after.find(']') { key = after[..end].trim().to_ascii_lowercase(); }
                else { key.clear(); }
                continue;
            }
            if key == "heightmapstep" || key == "waterlevel" {
                let value: f32 = line.parse().map_err(|_| format!("Invalid {key} in boden.ini"))?;
                if !value.is_finite() || (key == "heightmapstep" && value <= 0.0) {
                    return Err(format!("Invalid {key} in boden.ini"));
                }
                if key == "heightmapstep" { result.height_step = value; }
                else { result.water_world = value; }
                key.clear();
            }
        }
        if !(result.water_world / result.height_step).is_finite() {
            return Err("Water height conversion overflow".into());
        }
        Ok(result)
    }
    pub fn load(directory: &Path) -> Result<Self, String> {
        match std::fs::read(directory.join("boden.ini")) {
            Ok(bytes) => Self::parse(&bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn water_surface(&self) -> f32 { self.water_world / self.height_step }
    pub fn is_water_above_max_height(&self) -> bool { self.water_surface() > 255.0 }
    pub fn validate_bilingual(&self, english: bool) -> Result<(), String> {
        if !self.height_step.is_finite() || self.height_step <= 0.0 {
            return Err(if english {
                "Heightmapstep must be a finite number greater than 0.".into()
            } else {
                "高度步階（Heightmapstep）必須為大於 0 的有限數值。".into()
            });
        }
        if !self.water_world.is_finite() {
            return Err(if english {
                "Waterlevel must be a finite number.".into()
            } else {
                "水位高度（Waterlevel）必須為有限數值。".into()
            });
        }
        if !self.water_surface().is_finite() {
            return Err(if english {
                "Water height conversion overflow (Waterlevel / Heightmapstep).".into()
            } else {
                "水位高度換算溢位（Waterlevel / Heightmapstep）。".into()
            });
        }
        Ok(())
    }
    pub fn suggested_bed(&self) -> u8 {
        (self.water_surface().floor() - 16.0).clamp(0.0, 255.0) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comments_defaults_and_vertical_scale_are_independent_of_dimensions() {
        let s = MapSettings::parse(b"[Waterlevel] ; world units\r\n110\r\n[Heightmapstep]; vertical only\r\n4\r\n").unwrap();
        assert_eq!(s.water_surface(), 27.5);
        assert_eq!(s.suggested_bed(), 11);
        assert_eq!(MapSettings::parse(b"[Waterlevel]\n20\n").unwrap().water_surface(),5.0);
        for invalid in ["0", "NaN", "inf", "-1", "bad"] {
            assert!(MapSettings::parse(format!("[Heightmapstep]\n{invalid}\n").as_bytes()).is_err());
        }
        let raw=b"[Waterlevel]\n110\n[Heightmapstep]\n4\n";
        let mut pfil=vec![0;64];pfil[..4].copy_from_slice(b"PFIL");pfil[16..20].copy_from_slice(&(raw.len() as u32).to_le_bytes());
        for chunk in raw.chunks(8) {pfil.push(0xff);pfil.extend_from_slice(chunk);}
        assert_eq!(MapSettings::parse(&pfil).unwrap(),s);
    }

    #[test]
    fn validate_bilingual_and_water_above_max_height() {
        let valid = MapSettings { height_step: 4.0, water_world: 100.0 };
        assert!(valid.validate_bilingual(false).is_ok());
        assert!(valid.validate_bilingual(true).is_ok());
        assert!(!valid.is_water_above_max_height());

        let submerged = MapSettings { height_step: 1.0, water_world: 300.0 };
        assert!(submerged.validate_bilingual(false).is_ok());
        assert!(submerged.is_water_above_max_height());

        let bad_step = MapSettings { height_step: 0.0, water_world: 0.0 };
        assert!(bad_step.validate_bilingual(false).unwrap_err().contains("Heightmapstep"));
        assert!(bad_step.validate_bilingual(true).unwrap_err().contains("Heightmapstep"));

        let bad_water = MapSettings { height_step: 4.0, water_world: f32::NAN };
        assert!(bad_water.validate_bilingual(false).unwrap_err().contains("Waterlevel"));
        assert!(bad_water.validate_bilingual(true).unwrap_err().contains("Waterlevel"));
    }
}
