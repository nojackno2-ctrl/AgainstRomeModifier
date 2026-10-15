//! Faithful port of `SdlObjectCatalog`, `SdlObjectType`, `SdlPlacedObject`, and `SdlPlacedObjectsFile`.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use uuid::Uuid;
use crate::custom_map::CustomMapManifest;
use crate::text_docs::{SdlDocument, SdlVector3};
use crate::text_encoding::{compress_pfil, encode_cp1251, is_pfil};

/// Major object category based on namedef prefix (`SdlObjectCategory`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SdlObjectCategory {
    Building = 0,
    UnitGroup = 1,
    Figure = 2,
    Effect = 3,
    Other = 4,
}

/// A placeable SDL object type definition with a full template (`SdlObjectType`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdlObjectType {
    pub name_def: String,
    pub definition: i32,
    pub category: SdlObjectCategory,
    pub tribe: String,
    pub occurrences: usize,
    pub template_fields: HashMap<String, String>,
}

impl SdlObjectType {
    pub fn new(
        name_def: impl Into<String>,
        definition: i32,
        category: SdlObjectCategory,
        tribe: impl Into<String>,
        occurrences: usize,
        template_fields: HashMap<String, String>,
    ) -> Self {
        Self {
            name_def: name_def.into(),
            definition,
            category,
            tribe: tribe.into(),
            occurrences,
            template_fields,
        }
    }

    pub fn is_animal(&self) -> bool {
        self.category == SdlObjectCategory::Figure && self.name_def.to_ascii_lowercase().starts_with("figtie")
    }

    pub fn has_unit_count(&self) -> bool {
        self.category == SdlObjectCategory::UnitGroup
    }
}

/// Identity of a native DATA object (`NativeObjectIdentity`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NativeObjectIdentity {
    pub slot: i32,
    pub uid: u32,
}

impl std::fmt::Display for NativeObjectIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Slot {}, Uid {}", self.slot, self.uid)
    }
}

/// A placed object positioned in absolute world coordinates (`SdlPlacedObject`).
#[derive(Clone, Debug, PartialEq)]
pub struct SdlPlacedObject {
    pub obj_type: SdlObjectType,
    pub world_x: f32,
    pub world_y: f32,
    pub world_z: f32,
    pub team: i32,
    pub angle: f32,
    pub unit_count: i32,
    pub scenario_id: Uuid,
    pub copy_source_scenario_id: Uuid,
    pub native_source_identity: Option<NativeObjectIdentity>,
    pub raw_rotation: Option<f32>,
}

impl SdlPlacedObject {
    pub fn new(
        obj_type: SdlObjectType,
        world_x: f32,
        world_y: f32,
        world_z: f32,
        team: i32,
        angle: f32,
        unit_count: i32,
    ) -> Self {
        Self {
            obj_type,
            world_x,
            world_y,
            world_z,
            team,
            angle,
            unit_count,
            scenario_id: Uuid::nil(),
            copy_source_scenario_id: Uuid::nil(),
            native_source_identity: None,
            raw_rotation: None,
        }
    }
}

/// Catalog scanning retail game map SDLs for placeable object definitions (`SdlObjectCatalog`).
pub struct SdlObjectCatalog;

impl SdlObjectCatalog {
    pub fn build(game_path: &Path) -> Result<Vec<SdlObjectType>, String> {
        let maps = game_path.join("MAPS");
        if !maps.is_dir() {
            return Ok(Vec::new());
        }

        struct TemplateRecord {
            fields: HashMap<String, String>,
            count: usize,
            preferred: bool,
        }

        let mut templates: HashMap<String, TemplateRecord> = HashMap::new();

        let map_dirs = fs::read_dir(&maps).map_err(|e| format!("無法讀取 MAPS: {e}"))?;
        for map_entry in map_dirs.flatten() {
            let map_dir = map_entry.path();
            if !map_dir.is_dir() || CustomMapManifest::is_custom_map_directory(&map_dir) {
                continue;
            }

            let sdl_files = fs::read_dir(&map_dir);
            let Ok(sdl_entries) = sdl_files else { continue; };

            for sdl_entry in sdl_entries.flatten() {
                let sdl_path = sdl_entry.path();
                if sdl_path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("sdl")) {
                    let Ok(doc) = SdlDocument::load(&sdl_path) else { continue; };
                    for section in doc.objects() {
                        let Some(name) = section.get_value("namedef").map(str::trim) else { continue; };
                        if name.is_empty() {
                            continue;
                        }
                        let Some(def) = section.get_value("def").and_then(|d| d.trim().parse::<i32>().ok()) else { continue; };
                        if def < 0 {
                            continue;
                        }

                        let preferred = section.get_value("onload").map(str::trim) == Some("1");
                        let name_key = name.to_string();

                        if let Some(existing) = templates.get_mut(&name_key) {
                            if preferred && !existing.preferred {
                                existing.fields = section.fields;
                                existing.preferred = true;
                            }
                            existing.count += 1;
                        } else {
                            templates.insert(
                                name_key,
                                TemplateRecord {
                                    fields: section.fields,
                                    count: 1,
                                    preferred,
                                },
                            );
                        }
                    }
                }
            }
        }

        let mut result = Vec::new();
        for (name, rec) in templates {
            let def: i32 = rec
                .fields
                .get("def")
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
            let cat = Self::categorize(&name);
            let tr = Self::tribe(&name).to_string();
            result.push(SdlObjectType::new(name, def, cat, tr, rec.count, rec.fields));
        }

        result.sort_by(|a, b| {
            a.category
                .cmp(&b.category)
                .then_with(|| a.tribe.cmp(&b.tribe))
                .then_with(|| a.name_def.to_ascii_lowercase().cmp(&b.name_def.to_ascii_lowercase()))
        });

        Ok(result)
    }

    pub fn categorize(name_def: &str) -> SdlObjectCategory {
        let lower = name_def.to_ascii_lowercase();
        if lower.starts_with("bau") {
            SdlObjectCategory::Building
        } else if lower.starts_with("ver") && lower.contains("ico") {
            SdlObjectCategory::UnitGroup
        } else if lower.starts_with("fig") {
            SdlObjectCategory::Figure
        } else if lower.starts_with("fx") {
            SdlObjectCategory::Effect
        } else {
            SdlObjectCategory::Other
        }
    }

    pub fn tribe(name_def: &str) -> &'static str {
        if name_def.len() < 6 {
            return "";
        }
        match &name_def[3..6] {
            s if s.eq_ignore_ascii_case("ger") => "Ger",
            s if s.eq_ignore_ascii_case("hun") => "Hun",
            s if s.eq_ignore_ascii_case("kel") => "Kel",
            s if s.eq_ignore_ascii_case("rom") => "Rom",
            _ => "",
        }
    }
}

/// Renderer and serializer for editor-placed objects stored in `ARM_Placed.sdl` (`SdlPlacedObjectsFile`).
pub struct SdlPlacedObjectsFile;

impl SdlPlacedObjectsFile {
    pub const FILE_NAME: &'static str = "ARM_Placed.sdl";
    pub const SETTLEMENT_NAME: &'static str = "ARM_Placed";

    pub fn render(objects: &[SdlPlacedObject]) -> Result<String, String> {
        let mut text = String::new();
        text.push_str("[settlement]\r\n");
        text.push_str(&format!("name    ={}\r\n", Self::SETTLEMENT_NAME));
        let now = chrono::Local::now();
        text.push_str(&format!("createdt={}\r\n", now.format("%Y/%m/%d")));
        text.push_str(&format!("createtm={}\r\n", now.format("%H:%M:%S")));
        text.push_str("teamsett=-1\r\n");
        text.push_str("refpos  =0,0,0\r\n");

        for (index, item) in objects.iter().enumerate() {
            if !item.world_x.is_finite() || !item.world_y.is_finite() || !item.world_z.is_finite() {
                return Err("預放物件座標必須是有限數值。".into());
            }
            if !(-1..=15).contains(&item.team) {
                return Err("預放物件隊伍必須介於 -1 與 15。".into());
            }

            let mut fields = item.obj_type.template_fields.clone();
            fields.insert("name".to_string(), String::new());
            fields.insert(
                "pos".to_string(),
                SdlVector3::new(item.world_x, item.world_y, item.world_z).to_string_2dp(),
            );
            fields.insert("team".to_string(), item.team.to_string());
            fields.insert("angle".to_string(), format!("{:.2}", item.angle));
            fields.insert("onload".to_string(), "1".to_string());

            if item.obj_type.has_unit_count() && item.unit_count > 0 {
                let existing_anzv = fields.get("anzv").cloned().unwrap_or_else(|| "0,0,0".into());
                let mut counts: Vec<String> = existing_anzv.split(',').map(|s| s.to_string()).collect();
                if counts.is_empty() {
                    counts.push("0".into());
                }
                counts[0] = item.unit_count.clamp(1, 50).to_string();
                fields.insert("anzv".to_string(), counts.join(","));
            }

            text.push_str(&format!("\r\n[object{:04}]\r\n", index));

            let mut sorted_keys: Vec<_> = fields.keys().cloned().collect();
            sorted_keys.sort();
            for key in sorted_keys {
                let value = &fields[&key];
                if key.contains('=') || key.contains('\r') || key.contains('\n') || value.contains('\r') || value.contains('\n') {
                    return Err("SDL 範本欄位含有無效字元。".into());
                }
                let padded_key = format!("{:width$}", key, width = 8);
                text.push_str(&format!("{}={}\r\n", padded_key, value));
            }
        }

        Ok(text)
    }

    pub fn load(map_directory: &Path, catalog: &[SdlObjectType]) -> Result<Vec<SdlPlacedObject>, String> {
        let path = map_directory.join(Self::FILE_NAME);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let doc = SdlDocument::load(&path)?;
        let by_name: HashMap<String, SdlObjectType> = catalog
            .iter()
            .map(|t| (t.name_def.to_ascii_lowercase(), t.clone()))
            .collect();

        let mut result = Vec::new();
        for section in doc.objects() {
            let name = section.get_value("namedef").unwrap_or("").trim();
            let obj_type = if let Some(known) = by_name.get(&name.to_ascii_lowercase()) {
                known.clone()
            } else {
                let def = section.get_value("def").and_then(|v| v.parse().ok()).unwrap_or(-1);
                SdlObjectType::new(name, def, SdlObjectCategory::Other, "", 0, section.fields.clone())
            };

            let pos_str = section.get_value("pos").unwrap_or("0,0,0");
            let pos = SdlVector3::parse(pos_str).unwrap_or_default();
            let team = section.get_value("team").and_then(|v| v.parse().ok()).unwrap_or(0);
            let angle = section.get_value("angle").and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let anzv = section.get_value("anzv").unwrap_or("0");
            let unit_count = anzv.split(',').next().and_then(|v| v.trim().parse().ok()).unwrap_or(0);

            result.push(SdlPlacedObject::new(
                obj_type, pos.x, pos.y, pos.z, team, angle, unit_count,
            ));
        }

        Ok(result)
    }

    pub fn save(map_directory: &Path, objects: &[SdlPlacedObject]) -> Result<(), String> {
        let path = map_directory.join(Self::FILE_NAME);
        if objects.is_empty() {
            if path.exists() {
                let _ = fs::remove_file(&path);
            }
            return Ok(());
        }

        let rendered = Self::render(objects)?;
        let plain = encode_cp1251(&rendered)?;

        // Find header source template
        let mut header_template: Option<Vec<u8>> = None;
        if let Ok(entries) = fs::read_dir(map_directory) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file()
                    && p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("sdl"))
                    && p.file_name().and_then(|s| s.to_str()) != Some(Self::FILE_NAME)
                {
                    if let Ok(bytes) = fs::read(&p) {
                        if is_pfil(&bytes) {
                            header_template = Some(bytes);
                            break;
                        }
                    }
                }
            }
        }

        let output_bytes = if let Some(template) = header_template {
            compress_pfil(&plain, &template)?
        } else {
            plain
        };

        let tmp_path = map_directory.join(format!("{}.tmp_save", Self::FILE_NAME));
        fs::write(&tmp_path, &output_bytes).map_err(|e| format!("寫入預放物件暫存檔失敗: {e}"))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("替換預放物件檔案失敗: {e}"))?;

        Ok(())
    }
}
