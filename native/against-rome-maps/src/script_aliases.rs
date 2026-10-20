//! Script object alias catalog from `SYSTEM/CLAK/cl_scint.ini [ObjDefName]`
//! (port of C# `ScriptObjectAlias` / `ScriptObjectAliases`).

use crate::sdl_objects::{SdlObjectCatalog, SdlObjectCategory};
use crate::text_encoding::{decode_cp1251, decompress_pfil_if_needed};
use std::path::Path;

/// One `[ObjDefName]` entry: script APIs (`s_createObj`, ...) address object definitions by alias.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptObjectAlias {
    pub alias: String,
    pub name_def: String,
}

impl ScriptObjectAlias {
    pub fn category(&self) -> SdlObjectCategory {
        SdlObjectCatalog::categorize(&self.name_def)
    }

    pub fn tribe(&self) -> &'static str {
        SdlObjectCatalog::tribe(&self.name_def)
    }
}

pub struct ScriptObjectAliases;

impl ScriptObjectAliases {
    /// Reads `SYSTEM/CLAK/cl_scint.ini` (PFIL); a missing file yields an empty list.
    pub fn load(game_path: &Path) -> Result<Vec<ScriptObjectAlias>, String> {
        let path = game_path.join("SYSTEM").join("CLAK").join("cl_scint.ini");
        if !path.is_file() {
            return Ok(Vec::new());
        }
        Ok(Self::parse(&Self::read_text(&path)?))
    }

    /// Decodes a PFIL (or plain) game text file using the game encoding.
    pub fn read_text(path: &Path) -> Result<String, String> {
        let raw = std::fs::read(path).map_err(|e| format!("讀取 {} 失敗: {e}", path.display()))?;
        decode_cp1251(&decompress_pfil_if_needed(&raw)?)
    }

    pub fn parse(text: &str) -> Vec<ScriptObjectAlias> {
        let mut result = Vec::new();
        let mut in_section = false;
        for raw in text.split('\n') {
            let line = raw.trim();
            if line.starts_with('[') {
                in_section = line.eq_ignore_ascii_case("[ObjDefName]");
                continue;
            }
            if !in_section || line.is_empty() || line.starts_with(';') {
                continue;
            }
            let Some(eq) = line.find('=') else { continue };
            if eq == 0 {
                continue;
            }
            let alias = line[..eq].trim();
            let name_def = line[eq + 1..].trim();
            if !alias.is_empty() && !name_def.is_empty() {
                result.push(ScriptObjectAlias {
                    alias: alias.to_string(),
                    name_def: name_def.to_string(),
                });
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_objdefname_section() {
        let text = "[Other]\nx=y\n[ObjDefName]\n; c\nHouse = BauRoemHaus1 \r\n=bad\nempty=\nnoequals\nTroop=FigRoemLegionaer\n[Z]\nq=r\n";
        let list = ScriptObjectAliases::parse(text);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].alias, "House");
        assert_eq!(list[0].name_def, "BauRoemHaus1");
        assert_eq!(list[0].category(), SdlObjectCategory::Building);
        assert_eq!(list[1].category(), SdlObjectCategory::Figure);
    }

    #[test]
    fn missing_file_is_empty() {
        let dir = std::env::temp_dir().join("ar_script_alias_missing_dir");
        assert!(ScriptObjectAliases::load(&dir).unwrap().is_empty());
    }

    #[test]
    fn loads_plain_file() {
        let dir = std::env::temp_dir().join(format!("ar_script_alias_{}", std::process::id()));
        let clak = dir.join("SYSTEM").join("CLAK");
        std::fs::create_dir_all(&clak).unwrap();
        std::fs::write(clak.join("cl_scint.ini"), "[ObjDefName]\nA=BauX\n").unwrap();
        let list = ScriptObjectAliases::load(&dir).unwrap();
        assert_eq!(list, vec![ScriptObjectAlias { alias: "A".into(), name_def: "BauX".into() }]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
