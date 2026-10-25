//! Layered `lightdef.dau` + `lightdef.d00..d09` loader, port of C# `NativeLightDefinitions`
//! and `NativeLightTypeDefinition` (strict native-faithful variant of `NativeLightCatalog`).

use crate::math::Vec3;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Light creation request, mirrors C# `NativeLightCreation`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeLightCreation {
    pub position: Vec3,
    pub color: Vec3,
    pub radius: f32,
    pub flicker_type: i16,
    pub flicker_parameter: f32,
    pub special_fx: u32,
    pub definition: i16,
    pub owner_object: i32,
    pub parent_object: i32,
}

/// One `[LightDefault]` row, mirrors C# `NativeLightTypeDefinition`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeLightTypeDefinition {
    pub index: i32,
    pub active: i32,
    pub color: Vec3,
    pub radius: f32,
    pub flicker_type: i16,
    pub flicker_parameter: f32,
    pub special_fx: u32,
    pub name: String,
}

impl NativeLightTypeDefinition {
    pub fn initial(index: i32) -> Self {
        Self {
            index,
            active: 0,
            color: Vec3::new(1.0, 1.0, 1.0),
            radius: 500.0,
            flicker_type: 0,
            flicker_parameter: 0.0,
            special_fx: 0,
            name: "LD_NoNameEntered".into(),
        }
    }

    /// C# `Create(position, ownerObject = -1, parentObject = -1)`.
    pub fn create(
        &self,
        position: Vec3,
        owner_object: i32,
        parent_object: i32,
    ) -> Result<NativeLightCreation, String> {
        if !(0..256).contains(&self.index) || self.active == 0 {
            return Err("光源 definition 未啟用或超出原生索引範圍。".into());
        }
        Ok(NativeLightCreation {
            position,
            color: self.color,
            radius: self.radius,
            flicker_type: self.flicker_type,
            flicker_parameter: self.flicker_parameter,
            special_fx: self.special_fx,
            definition: self.index as i16,
            owner_object,
            parent_object,
        })
    }
}

/// Layered light definition table keyed by native index.
pub type NativeLightTypeTable = HashMap<i32, NativeLightTypeDefinition>;

/// C# `NativeLightDefinitions.Parse`: a single text layer.
pub fn parse_light_type_definitions(text: &str) -> Result<NativeLightTypeTable, String> {
    let mut result = HashMap::new();
    apply_layer(text, &mut result)?;
    Ok(result)
}

fn read_game_text(path: &Path) -> Result<String, String> {
    let raw = std::fs::read(path).map_err(|e| format!("讀取 {} 失敗: {e}", path.display()))?;
    let bytes = if raw.len() >= 4 && &raw[0..4] == b"PFIL" {
        against_rome_data::lzss::decompress_pfil(&raw, None).map_err(|e| e.to_string())?
    } else {
        raw
    };
    let (text, _) = encoding_rs::WINDOWS_1251.decode_without_bom_handling(&bytes);
    Ok(text.into_owned())
}

/// Loads layers from an explicit DefaultPath directory.
pub fn load_light_type_definitions_from_prefix(prefix: &Path) -> Result<NativeLightTypeTable, String> {
    let mut result = HashMap::new();
    let files = std::iter::once("lightdef.dau".to_string())
        .chain((0..10).map(|i| format!("lightdef.d{i:02}")));
    for file in files {
        let path = prefix.join(&file);
        if path.is_file() {
            apply_layer(&read_game_text(&path)?, &mut result)?;
        }
    }
    Ok(result)
}

/// C# `NativeLightDefinitions.Load(gamePath)`.
pub fn load_light_type_definitions(game_path: &Path) -> Result<NativeLightTypeTable, String> {
    load_light_type_definitions_from_prefix(&default_prefix(game_path)?)
}

fn default_prefix(game_path: &Path) -> Result<PathBuf, String> {
    let ini = game_path.join("SYSTEM").join("clengine.ini");
    if !ini.is_file() {
        return Ok(game_path.join("SYSTEM").join("DATA_MP").join("DEFAULTS"));
    }
    let text = read_game_text(&ini)?;
    let mut in_section = false;
    for line in text.split('\n') {
        let row = line.split(';').next().unwrap_or("").trim();
        if row.starts_with('[') {
            in_section = row.eq_ignore_ascii_case("[DefaultPath]");
            continue;
        }
        if !in_section || row.is_empty() {
            continue;
        }
        let normalized = row.replace('\\', "/");
        if Path::new(&normalized).is_absolute() || normalized.starts_with('/') {
            return Err("clengine.ini DefaultPath 必須是遊戲目錄內的相對路徑。".into());
        }
        let root = game_path.canonicalize().map_err(|e| e.to_string())?;
        let candidate = root.join(&normalized);
        let target = if candidate.exists() {
            candidate.canonicalize().map_err(|e| e.to_string())?
        } else {
            candidate
        };
        if !target.starts_with(&root) {
            return Err("clengine.ini DefaultPath 超出遊戲目錄，無法驗證物件定義來源。".into());
        }
        return Ok(target);
    }
    Err("clengine.ini 缺少有效 DefaultPath，無法驗證物件定義來源。".into())
}

fn apply_layer(text: &str, result: &mut NativeLightTypeTable) -> Result<(), String> {
    let mut in_section = false;
    for line in text.split('\n') {
        let row = line.split(';').next().unwrap_or("").trim();
        if row.starts_with('[') {
            if in_section {
                break;
            }
            in_section = row.eq_ignore_ascii_case("[LightDefault]");
            continue;
        }
        if !in_section || row.is_empty() || row.starts_with('#') {
            continue;
        }
        // Native tokenizer 567d50 skips comma runs, not whitespace tokens.
        let columns: Vec<&str> = row.split(',').filter(|s| !s.is_empty()).collect();
        if columns.is_empty() {
            return Err("lightdef signed32 欄位無效。".into());
        }
        let id = integer(columns[0])?;
        if !(0..256).contains(&id) {
            return Err("lightdef 索引超出0..255。".into());
        }
        let mut value = result
            .get(&id)
            .cloned()
            .unwrap_or_else(|| NativeLightTypeDefinition::initial(id));
        if columns.len() > 1 {
            value.active = integer(columns[1])?;
        }
        if columns.len() > 2 {
            value.color.x = float(columns[2])?;
        }
        if columns.len() > 3 {
            value.color.y = float(columns[3])?;
        }
        if columns.len() > 4 {
            value.color.z = float(columns[4])?;
        }
        if columns.len() > 5 {
            value.radius = float(columns[5])?;
        }
        if columns.len() > 6 {
            value.flicker_type = integer(columns[6])? as i16;
        }
        if columns.len() > 7 {
            value.flicker_parameter = float(columns[7])?;
        }
        if columns.len() > 8 {
            value.special_fx = integer(columns[8])? as u32;
        }
        if columns.len() > 9 {
            let name = columns[9]
                .trim()
                .split([' ', '\t', '\r'])
                .find(|s| !s.is_empty())
                .unwrap_or("");
            if name.is_empty() {
                return Err("lightdef name 必須含非空白token。".into());
            }
            let (bytes, _, _) = encoding_rs::WINDOWS_1251.encode(name);
            let cut = &bytes[..bytes.len().min(30)];
            let (decoded, _) = encoding_rs::WINDOWS_1251.decode_without_bom_handling(cut);
            value.name = decoded.into_owned();
        }
        result.insert(id, value);
    }
    Ok(())
}

fn integer(text: &str) -> Result<i32, String> {
    text.trim()
        .parse::<i32>()
        .map_err(|_| "lightdef signed32 欄位無效。".to_string())
}

fn float(text: &str) -> Result<f32, String> {
    match text.trim().parse::<f32>() {
        Ok(v) if v.is_finite() => Ok(v),
        _ => Err("lightdef float32 欄位無效或非有限值。".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "[LightDefault]\n; c\n1, 1, 0.5, 0.25, 0.75, 300, 2, 1.5, 7, torch extra\r\n2,1\n[Other]\n3,1\n";

    #[test]
    fn parses_row_and_stops_at_next_section() {
        let t = parse_light_type_definitions(BASE).unwrap();
        assert_eq!(t.len(), 2);
        let d = &t[&1];
        assert_eq!((d.active, d.radius, d.flicker_type, d.special_fx), (1, 300.0, 2, 7));
        assert_eq!(d.color, Vec3::new(0.5, 0.25, 0.75));
        assert_eq!(d.name, "torch");
        assert_eq!(t[&2].radius, 500.0);
        assert_eq!(t[&2].name, "LD_NoNameEntered");
    }

    #[test]
    fn layers_override_fieldwise_and_comma_runs_skipped() {
        let mut t = parse_light_type_definitions(BASE).unwrap();
        apply_layer("[LightDefault]\n1,,,,,,900\n", &mut t).unwrap();
        // empty tokens removed: columns are [1, 900] -> active becomes 900
        assert_eq!(t[&1].active, 900);
        assert_eq!(t[&1].radius, 300.0);
    }

    #[test]
    fn rejects_invalid_rows() {
        assert!(parse_light_type_definitions("[LightDefault]\n256,1\n").is_err());
        assert!(parse_light_type_definitions("[LightDefault]\n1,x\n").is_err());
        assert!(parse_light_type_definitions("[LightDefault]\n1,1,inf\n").is_err());
        assert!(parse_light_type_definitions("[LightDefault]\n1,1,1,1,1,1,0,0,0, ,z\n").is_err());
    }

    #[test]
    fn create_requires_active_in_range() {
        let t = parse_light_type_definitions(BASE).unwrap();
        let c = t[&1].create(Vec3::new(1.0, 2.0, 3.0), 4, -1).unwrap();
        assert_eq!((c.definition, c.owner_object, c.parent_object), (1, 4, -1));
        assert_eq!(c.radius, 300.0);
        let zero = Vec3::new(0.0, 0.0, 0.0);
        assert!(NativeLightTypeDefinition::initial(5).create(zero, -1, -1).is_err());
        let mut bad = t[&1].clone();
        bad.index = 256;
        assert!(bad.create(zero, -1, -1).is_err());
    }

    #[test]
    fn loads_from_game_dir_with_layers() {
        let dir = crate::light_definitions::temp_fixture::dir("ar_lightdef");
        let defaults = dir.join("SYSTEM").join("DATA_MP").join("DEFAULTS");
        std::fs::create_dir_all(&defaults).unwrap();
        std::fs::write(defaults.join("lightdef.dau"), BASE).unwrap();
        std::fs::write(defaults.join("lightdef.d03"), "[LightDefault]\n1,1,1,1,1,42\n").unwrap();
        let t = load_light_type_definitions(&dir).unwrap();
        assert_eq!(t[&1].radius, 42.0);
        assert_eq!(t[&1].name, "torch");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
