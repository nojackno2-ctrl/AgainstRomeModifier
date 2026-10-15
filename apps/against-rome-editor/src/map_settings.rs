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
}
