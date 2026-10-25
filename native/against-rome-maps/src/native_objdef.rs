//! Faithful port of `NativeObjectDefinitions` and `NativeObjectDefinition`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use crate::text_encoding::{decode_cp1251, decompress_pfil_if_needed};

/// Verified objdef fields used by native object creation (`NativeObjectDefinition`).
#[derive(Clone, Debug, PartialEq)]
pub struct NativeObjectDefinition {
    pub type_id: i32,
    pub active: bool,
    pub aptix: Option<i16>,
    pub afstr: i16,
    pub initial_hp: Option<i32>,
    pub initial_morale: Option<i32>,
    pub initial_moves: Option<f32>,
    pub initial_max_ruhm: Option<i32>,
    pub typus: Option<i16>,
    pub initial_field1c: Option<u16>,
    pub name: Option<String>,
    pub direct_light_definition: i16,
    pub direct_light_height: i16,
    pub direct_light_distance: f32,
    pub direct_light_angle: f32,
}

impl NativeObjectDefinition {
    pub fn new(type_id: i32, active: bool, afstr: i16) -> Self {
        Self {
            type_id,
            active,
            aptix: None,
            afstr,
            initial_hp: Some(0),
            initial_morale: Some(0),
            initial_moves: Some(0.0),
            initial_max_ruhm: Some(0),
            typus: Some(-1),
            initial_field1c: None,
            name: None,
            direct_light_definition: -1,
            direct_light_height: 0,
            direct_light_distance: 0.0,
            direct_light_angle: 0.0,
        }
    }

    pub fn direct_light_position(&self, position: [f32; 3], rotation_degrees: f32) -> Result<[f32; 3], String> {
        if !position[0].is_finite()
            || !position[1].is_finite()
            || !position[2].is_finite()
            || !rotation_degrees.is_finite()
            || !self.direct_light_distance.is_finite()
            || !self.direct_light_angle.is_finite()
        {
            return Err("光源座標與方向必須是有限數值。".into());
        }

        let angle = (rotation_degrees as f64 - 90.0 - self.direct_light_angle as f64)
            * (1.0 / 360.0)
            * 2.0
            * std::f64::consts::PI;
        let res_x = position[0] as f64 + angle.sin() * self.direct_light_distance as f64;
        let res_y = position[1] as f64 + self.direct_light_height as f64;
        let res_z = position[2] as f64 + angle.cos() * self.direct_light_distance as f64;

        if !res_x.is_finite() || !res_y.is_finite() || !res_z.is_finite() {
            return Err("光源計算結果超出 float 座標範圍。".into());
        }

        Ok([res_x as f32, res_y as f32, res_z as f32])
    }

    pub fn requires_third_position(&self) -> bool {
        self.active && self.afstr != 0
    }
}

pub struct NativeObjectDefinitions;

impl NativeObjectDefinitions {
    pub fn load(game_path: &Path) -> Result<HashMap<i32, NativeObjectDefinition>, String> {
        let prefix = Self::default_prefix(game_path)?;
        let mut result = HashMap::new();

        let objdef_files = std::iter::once("objdef.dau".to_string())
            .chain((0..10).map(|i| format!("objdef.d{:02}", i)));

        for file in objdef_files {
            let path = prefix.join(&file);
            if path.exists() {
                let text = read_text(&path)?;
                Self::apply_layer(&text, &mut result)?;
            }
        }

        let mut typus: HashMap<i32, struct_types::TypusEntryInternal> = HashMap::new();
        let typus_files = std::iter::once("typus.dau".to_string())
            .chain((0..10).map(|i| format!("typus.d{:02}", i)));

        for file in typus_files {
            let path = prefix.join(&file);
            if path.exists() {
                let text = read_text(&path)?;
                Self::apply_typus_layer(&text, &mut typus)?;
            }
        }

        for definition in result.values_mut() {
            let enabled = definition.active
                && definition.typus.map(|t| (0..2500).contains(&t)).unwrap_or(false)
                && typus
                    .get(&(definition.typus.unwrap() as i32))
                    .map(|entry| entry.active && (entry.flags & 0x400) != 0)
                    .unwrap_or(false);
            definition.initial_field1c = Some(if enabled { 100 } else { 0 });
        }

        Ok(result)
    }

    pub fn parse(text: &str) -> Result<HashMap<i32, NativeObjectDefinition>, String> {
        let mut result = HashMap::new();
        Self::apply_layer(text, &mut result)?;
        Ok(result)
    }

    pub fn default_prefix(game_path: &Path) -> Result<PathBuf, String> {
        let path = game_path.join("SYSTEM").join("clengine.ini");
        if !path.exists() {
            return Ok(game_path.join("SYSTEM").join("DATA_MP").join("DEFAULTS"));
        }

        let text = read_text(&path)?;
        let mut in_section = false;

        for line in text.lines() {
            let row = line.split(';').next().unwrap_or("").trim();
            if row.starts_with('[') {
                in_section = row.eq_ignore_ascii_case("[DefaultPath]");
                continue;
            }
            if !in_section || row.is_empty() {
                continue;
            }

            let normalized = row.replace('\\', "/");
            let rel_path = Path::new(&normalized);
            if rel_path.is_absolute() {
                return Err("clengine.ini DefaultPath 必須是遊戲目錄內的相對路徑。".into());
            }

            let root = game_path.canonicalize().map_err(|e| e.to_string())?;
            let full_cand = root.join(rel_path);
            let target_dir = if full_cand.exists() {
                full_cand.canonicalize().map_err(|e| e.to_string())?
            } else {
                full_cand
            };

            if !target_dir.starts_with(&root) {
                return Err("clengine.ini DefaultPath 超出遊戲目錄，無法驗證物件定義來源。".into());
            }

            return Ok(target_dir);
        }

        Err("clengine.ini 缺少有效 DefaultPath，無法驗證物件定義來源。".into())
    }

    fn apply_layer(text: &str, definitions: &mut HashMap<i32, NativeObjectDefinition>) -> Result<(), String> {
        let mut in_section = false;
        for line in text.lines() {
            let row = line.split(';').next().unwrap_or("").trim();
            if row.starts_with('[') {
                if in_section {
                    break;
                }
                in_section = row.eq_ignore_ascii_case("[ObjectDefaults]");
                continue;
            }
            if !in_section || row.is_empty() {
                continue;
            }

            // In native engine (0x567d50), comma runs are skipped; empty tokens are removed.
            let columns: Vec<&str> = row
                .split(',')
                .filter(|s| !s.is_empty())
                .collect();
            if columns.is_empty() {
                continue;
            }

            let id = columns[0]
                .trim()
                .parse::<i32>()
                .map_err(|_| "objdef.dau 物件編號無效。".to_string())?;
            if !(0..2500).contains(&id) {
                return Err("objdef.dau 物件編號無效。".into());
            }

            let mut prev = definitions.remove(&id).unwrap_or_else(|| NativeObjectDefinition::new(id, false, 0));

            if columns.len() > 1 {
                let col1 = columns[1].trim();
                let flag = col1
                    .parse::<u8>()
                    .map_err(|_| format!("objdef.dau 物件 {id} 的 activ 欄位無效。"))?;
                prev.active = flag != 0;
            }

            if columns.len() > 60 {
                let afstr = columns[60]
                    .trim()
                    .parse::<i16>()
                    .map_err(|_| format!("objdef.dau 物件 {id} 的 afstr 欄位無效。"))?;
                prev.afstr = afstr;
            }

            if columns.len() > 14 {
                if let Ok(aptix) = columns[14].trim().parse::<i16>() {
                    prev.aptix = Some(aptix);
                }
            }

            if columns.len() > 19 {
                prev.initial_hp = Some(read_vital(columns[19], id, "lpmax")?);
            }
            if columns.len() > 21 {
                prev.initial_morale = Some(read_vital(columns[21], id, "mpmax")?);
            }
            if columns.len() > 4 {
                let val = columns[4]
                    .trim()
                    .parse::<f32>()
                    .map_err(|_| format!("objdef 物件 {id} 的 moves float 欄位無效。"))?;
                if !val.is_finite() {
                    return Err(format!("objdef 物件 {id} 的 moves float 欄位無效。"));
                }
                prev.initial_moves = Some(val);
            }
            if columns.len() > 152 {
                prev.initial_max_ruhm = Some(read_vital(columns[152], id, "maxruhm")?);
            }
            if columns.len() > 54 {
                let val = columns[54]
                    .trim()
                    .parse::<i16>()
                    .map_err(|_| format!("objdef 物件 {id} 的 typus signed word 欄位無效。"))?;
                prev.typus = Some(val);
            }
            if columns.len() > 66 {
                let val = columns[66]
                    .trim()
                    .parse::<i16>()
                    .map_err(|_| format!("objdef 物件 {id} 的 lidef signed16 欄位無效。"))?;
                prev.direct_light_definition = val;
            }
            if columns.len() > 67 {
                prev.direct_light_height = read_vital(columns[67], id, "lihei")? as i16;
            }
            if columns.len() > 70 {
                prev.direct_light_distance = read_finite_float(columns[70], id, "lightrad")?;
            }
            if columns.len() > 71 {
                prev.direct_light_angle = read_finite_float(columns[71], id, "lightang")?;
            }
            if columns.len() > 52 {
                prev.name = Some(columns[52].trim().to_string());
            }

            definitions.insert(id, prev);
        }
        Ok(())
    }

    fn apply_typus_layer(
        text: &str,
        definitions: &mut HashMap<i32, struct_types::TypusEntryInternal>,
    ) -> Result<(), String> {
        let mut in_section = false;
        for line in text.lines() {
            let row = line.split(';').next().unwrap_or("").trim();
            if row.starts_with('[') {
                if in_section {
                    break;
                }
                in_section = row.eq_ignore_ascii_case("[TypusDefaults]");
                continue;
            }
            if !in_section || row.is_empty() {
                continue;
            }

            let columns: Vec<&str> = row
                .split(',')
                .filter(|s| !s.is_empty())
                .collect();
            if columns.is_empty() {
                continue;
            }

            let id = columns[0]
                .trim()
                .parse::<i32>()
                .map_err(|_| "typus 物件編號無效。".to_string())?;
            if !(0..2500).contains(&id) {
                return Err("typus 物件編號無效。".into());
            }

            let mut prev = definitions
                .remove(&id)
                .unwrap_or(struct_types::TypusEntryInternal {
                    active: false,
                    flags: 0,
                });

            if columns.len() > 1 {
                prev.active = read_vital(columns[1], id, "typus activ")? != 0;
            }
            if columns.len() > 3 {
                let mut val_str = columns[3].trim();
                if val_str.starts_with("0x") || val_str.starts_with("0X") {
                    val_str = &val_str[2..];
                }
                let flags = u32::from_str_radix(val_str, 16)
                    .map_err(|_| format!("typus {id} 的低 flags hex 欄位無效。"))?;
                prev.flags = flags;
            }

            definitions.insert(id, prev);
        }
        Ok(())
    }
}

mod struct_types {
    pub struct TypusEntryInternal {
        pub active: bool,
        pub flags: u32,
    }
}

fn read_text(path: &Path) -> Result<String, String> {
    let raw = fs::read(path).map_err(|e| format!("讀取檔案失敗 ({}): {e}", path.display()))?;
    let decompressed = decompress_pfil_if_needed(&raw)?;
    decode_cp1251(&decompressed)
}

fn read_vital(val: &str, id: i32, field: &str) -> Result<i32, String> {
    val.trim()
        .parse::<i32>()
        .map_err(|_| format!("objdef 物件 {id} 的 {field} signed integer 欄位無效。"))
}

fn read_finite_float(val: &str, id: i32, field: &str) -> Result<f32, String> {
    let f = val
        .trim()
        .parse::<f32>()
        .map_err(|_| format!("objdef 物件 {id} 的 {field} float 欄位無效。"))?;
    if !f.is_finite() {
        return Err(format!("objdef 物件 {id} 的 {field} float 欄位無效。"));
    }
    Ok(f)
}

#[cfg(test)]
mod footprint_tests {
    use super::*;
    #[test]
    fn aptix_inherits_and_later_layers_override() {
        let mut definitions = HashMap::new();
        let row = |aptix: &str| format!("[ObjectDefaults]\n34,1,0,0,0,0,0,0,0,0,0,0,0,0,{aptix}\n");
        NativeObjectDefinitions::apply_layer(&row("9"), &mut definitions).unwrap();
        NativeObjectDefinitions::apply_layer("[ObjectDefaults]\n34,1\n", &mut definitions).unwrap();
        assert_eq!(definitions[&34].aptix, Some(9));
        NativeObjectDefinitions::apply_layer(&row("-1"), &mut definitions).unwrap();
        assert_eq!(definitions[&34].aptix, Some(-1));
    }
    #[test]
    fn unparsable_aptix_preserves_inherited_or_unknown_value() {
        let mut definitions = HashMap::new();
        let row = "[ObjectDefaults]\n34,1,0,0,0,0,0,0,0,0,0,0,0,0,modded\n";
        NativeObjectDefinitions::apply_layer(row, &mut definitions).unwrap();
        assert_eq!(definitions[&34].aptix, None);
        definitions.get_mut(&34).unwrap().aptix = Some(9);
        NativeObjectDefinitions::apply_layer(row, &mut definitions).unwrap();
        assert_eq!(definitions[&34].aptix, Some(9));
    }
}