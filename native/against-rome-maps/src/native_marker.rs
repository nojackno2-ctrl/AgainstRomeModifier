//! Faithful port of `NativeMarkerTypeTable` and `NativeMarkerTypeSelection`.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use crate::native_objdef::{NativeObjectDefinition, NativeObjectDefinitions};
use crate::text_encoding::{decode_cp1251, decompress_pfil_if_needed};

/// Result of banner resolution for a team marker (`NativeMarkerTypeSelection`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeMarkerTypeSelection {
    pub team: i32,
    pub tribe: i32,
    pub banner_version: i32,
    pub definition_name: Option<String>,
    pub candidate_type: i32,
    pub saved_type: i32,
    pub effective_type: i32,
}

/// Native engine team banner/marker type table (`NativeMarkerTypeTable`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeMarkerTypeTable {
    selections: Vec<NativeMarkerTypeSelection>,
}

impl NativeMarkerTypeTable {
    pub const SERIALIZED_OFFSET: usize = 54;
    pub const TEAM_COUNT: usize = 9;

    pub fn selections(&self) -> &[NativeMarkerTypeSelection] {
        &self.selections
    }

    pub fn get_effective_type(&self, team: i32) -> i32 {
        if team >= 0 && (team as usize) < Self::TEAM_COUNT {
            self.selections[team as usize].effective_type
        } else {
            -1
        }
    }

    pub fn load(
        game_path: &Path,
        data_directory: &Path,
        definitions: Option<&HashMap<i32, NativeObjectDefinition>>,
    ) -> Result<Self, String> {
        let loaded_defs;
        let defs = match definitions {
            Some(d) => d,
            None => {
                loaded_defs = NativeObjectDefinitions::load(game_path)?;
                &loaded_defs
            }
        };

        let banner_path = game_path.join("SYSTEM").join("banner.ini");
        let team_path = data_directory.join("team.dat");
        let engine_path = data_directory.join("engine.dat");

        let banner_text = read_text(&banner_path)?;
        let team_text = read_text(&team_path)?;

        let engine_raw = fs::read(&engine_path).map_err(|e| format!("讀取 engine.dat 失敗: {e}"))?;
        let engine = decompress_pfil_if_needed(&engine_raw)?;

        Self::parse(&banner_text, &team_text, &engine, defs)
    }

    pub fn parse(
        banner_text: &str,
        team_text: &str,
        engine: &[u8],
        definitions: &HashMap<i32, NativeObjectDefinition>,
    ) -> Result<Self, String> {
        if engine.len() != 94 || i32::from_le_bytes(engine[0..4].try_into().unwrap()) != 1 {
            return Err("engine.dat marker type table 的版本或長度無效。".into());
        }

        let banners = parse_sections(banner_text)?;
        let team_sections = parse_sections(team_text)?;

        let mut teams = [(0i32, 0i32); 8];
        if let Some(rows) = team_sections.get("[teamdata]") {
            for row in rows {
                let columns: Vec<&str> = row
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .collect();
                if columns.is_empty() {
                    continue;
                }
                let team: i32 = columns[0]
                    .trim()
                    .parse()
                    .map_err(|_| "team.dat 隊伍編號必須介於 0 和 7。".to_string())?;
                if !(0..8).contains(&team) {
                    return Err("team.dat 隊伍編號必須介於 0 和 7。".into());
                }

                let mut current = teams[team as usize];
                if columns.len() > 1 {
                    current.0 = match columns[1].trim().to_ascii_uppercase().as_str() {
                        "KEL" => 1,
                        "HUN" => 2,
                        "ROM" => 3,
                        _ => 0,
                    };
                }
                if columns.len() > 5 {
                    let version: i32 = columns[5]
                        .trim()
                        .parse()
                        .map_err(|_| "team.dat 旗幟版本必須介於 0 和 9。".to_string())?;
                    if !(0..10).contains(&version) {
                        return Err("team.dat 旗幟版本必須介於 0 和 9。".into());
                    }
                    current.1 = version;
                }
                teams[team as usize] = current;
            }
        }

        let mut names: HashMap<String, Vec<&NativeObjectDefinition>> = HashMap::new();
        for def in definitions.values() {
            if let Some(name) = &def.name {
                if !name.is_empty() {
                    names
                        .entry(name.to_ascii_lowercase())
                        .or_default()
                        .push(def);
                }
            }
        }

        let mut selections = Vec::with_capacity(Self::TEAM_COUNT);
        for team in 0..Self::TEAM_COUNT {
            let offset = Self::SERIALIZED_OFFSET + team * 4;
            let saved = i32::from_le_bytes(engine[offset..offset + 4].try_into().unwrap());
            let mut candidate = -1;
            let mut tribe = -1;
            let mut version = -1;
            let mut alias: Option<String> = None;

            if team < 8 {
                let (t, v) = teams[team];
                tribe = t;
                version = v;
                let section_key = format!("[volk{:02}_obdef_bver{:02}]", tribe, version);
                if let Some(list) = banners.get(&section_key.to_ascii_lowercase()) {
                    if let Some(first) = list.first() {
                        alias = Some(first.clone());
                        if !first.eq_ignore_ascii_case("none") {
                            if let Some(matches) = names.get(&first.to_ascii_lowercase()) {
                                if matches.len() != 1 {
                                    return Err(format!("旗幟定義名稱 {first} 對應多個物件，無法確定類型。"));
                                }
                                candidate = matches[0].type_id;
                            }
                        }
                    }
                }
            }

            let effective = if (0..2500).contains(&candidate)
                && definitions.get(&candidate).map(|d| d.active).unwrap_or(false)
            {
                candidate
            } else {
                saved
            };

            selections.push(NativeMarkerTypeSelection {
                team: team as i32,
                tribe,
                banner_version: version,
                definition_name: alias,
                candidate_type: candidate,
                saved_type: saved,
                effective_type: effective,
            });
        }

        Ok(Self { selections })
    }
}

fn read_text(path: &Path) -> Result<String, String> {
    let raw = fs::read(path).map_err(|e| format!("讀取檔案失敗 ({}): {e}", path.display()))?;
    let decompressed = decompress_pfil_if_needed(&raw)?;
    decode_cp1251(&decompressed)
}

fn parse_sections(text: &str) -> Result<HashMap<String, Vec<String>>, String> {
    let mut result: HashMap<String, Vec<String>> = HashMap::new();
    let mut current_section: Option<String> = None;

    for line in text.lines() {
        let row = line.split(';').next().unwrap_or("").trim();
        if row.starts_with('[') {
            let key = row.to_ascii_lowercase();
            if result.contains_key(&key) {
                return Err(format!("重複設定區段 {row}，無法驗證旗幟來源。"));
            }
            result.insert(key.clone(), Vec::new());
            current_section = Some(key);
        } else if !row.is_empty() {
            if let Some(sec) = &current_section {
                result.get_mut(sec).unwrap().push(row.to_string());
            }
        }
    }

    Ok(result)
}
