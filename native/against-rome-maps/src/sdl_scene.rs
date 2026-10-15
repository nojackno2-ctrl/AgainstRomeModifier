//! Faithful port of `SdlSceneCatalog`, `MapSceneObject`, and `SdlSceneEditService`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use crate::custom_map::CustomMapAccess;
use crate::sdl_objects::SdlPlacedObjectsFile;
use crate::text_docs::{SdlDocument, SdlVector3};

/// A placed object parsed from a settlement SDL blueprint (`MapSceneObject`).
#[derive(Clone, Debug, PartialEq)]
pub struct MapSceneObject {
    pub name: String,
    pub world_x: f32,
    pub world_y: f32,
    pub world_z: f32,
    pub team: i32,
    pub source_file: String,
    pub object_index: i32,
    pub local_x: f32,
    pub local_y: f32,
    pub local_z: f32,
    pub angle: Option<f32>,
    pub has_native_saved_position: bool,
}

impl MapSceneObject {
    pub fn kind(&self) -> &'static str {
        let lower = self.name.to_ascii_lowercase();
        if lower.starts_with("bau") {
            "建築"
        } else if lower.starts_with("fig") {
            "單位"
        } else {
            "物件"
        }
    }
}

/// Catalog scanning settlement SDL files within a map (`SdlSceneCatalog`).
pub struct SdlSceneCatalog;

impl SdlSceneCatalog {
    pub const WORLD_UNITS_PER_MAP_PIXEL: f32 = 64.0;
    pub const MAP_PIXEL_SIZE: f32 = 256.0;

    pub fn load_directory(map_directory: &Path) -> Result<Vec<MapSceneObject>, String> {
        let mut objects = Vec::new();
        for path in Self::settlement_files(map_directory)? {
            if let Ok(objs) = Self::load(&path) {
                objects.extend(objs);
            }
        }
        Ok(objects)
    }

    pub fn load(path: &Path) -> Result<Vec<MapSceneObject>, String> {
        let doc = SdlDocument::load(path)?;
        let settlement = doc.settlement();
        let refpos_str = settlement.get("refpos").ok_or("SDL 缺少 settlement 區段或 refpos。")?;
        let refpos = SdlVector3::parse(refpos_str)?;

        let source_file = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let mut result = Vec::new();

        for sec in doc.objects() {
            let pos_str = sec.get_value("pos").unwrap_or("0,0,0");
            let local_pos = SdlVector3::parse(pos_str).unwrap_or_default();
            let name = sec.get_value("namedef").unwrap_or("未命名物件").trim().to_string();
            let team = sec.get_value("team").and_then(|v| v.parse().ok()).unwrap_or(0);
            let angle = sec.get_value("angle").and_then(|v| v.parse().ok()).filter(|v: &f32| v.is_finite());

            let world_x = refpos.x + local_pos.x;
            let world_y = refpos.y + local_pos.y;
            let world_z = refpos.z + local_pos.z;

            let max_bound = Self::WORLD_UNITS_PER_MAP_PIXEL * Self::MAP_PIXEL_SIZE;
            if (0.0..=max_bound).contains(&world_x) && (0.0..=max_bound).contains(&world_z) {
                result.push(MapSceneObject {
                    name,
                    world_x,
                    world_y,
                    world_z,
                    team,
                    source_file: source_file.clone(),
                    object_index: sec.index,
                    local_x: local_pos.x,
                    local_y: local_pos.y,
                    local_z: local_pos.z,
                    angle,
                    has_native_saved_position: false,
                });
            }
        }

        Ok(result)
    }

    pub fn load_settlement_origins(map_directory: &Path) -> Result<HashMap<String, SdlVector3>, String> {
        let mut origins = HashMap::new();
        for path in Self::settlement_files(map_directory)? {
            if let Ok(doc) = SdlDocument::load(&path) {
                if let Some(refpos_str) = doc.settlement().get("refpos") {
                    if let Ok(vec) = SdlVector3::parse(refpos_str) {
                        if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                            origins.insert(file_name.to_string(), vec);
                        }
                    }
                }
            }
        }
        Ok(origins)
    }

    fn settlement_files(map_directory: &Path) -> Result<Vec<std::path::PathBuf>, String> {
        let mut files = Vec::new();
        let entries = fs::read_dir(map_directory).map_err(|e| format!("無法讀取地圖目錄: {e}"))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("sdl") {
                        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                            if !name.eq_ignore_ascii_case(SdlPlacedObjectsFile::FILE_NAME) {
                                files.push(path);
                            }
                        }
                    }
                }
            }
        }
        Ok(files)
    }
}

/// Request to add a new object to an SDL file using an existing object as template (`SdlSceneObjectAddition`).
#[derive(Clone, Debug, PartialEq)]
pub struct SdlSceneObjectAddition {
    pub source_file: String,
    pub template_index: i32,
    pub team: i32,
    pub local_x: f32,
    pub local_y: f32,
    pub local_z: f32,
    pub angle: Option<f32>,
    pub target_file: Option<String>,
}

impl SdlSceneObjectAddition {
    pub fn effective_target_file(&self) -> &str {
        self.target_file.as_deref().unwrap_or(&self.source_file)
    }
}

/// Request to remove an SDL object by index (`SdlSceneObjectRemoval`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SdlSceneObjectRemoval {
    pub source_file: String,
    pub object_index: i32,
}

/// Request to translate a settlement's `refpos` (`SdlSettlementTranslation`).
#[derive(Clone, Debug, PartialEq)]
pub struct SdlSettlementTranslation {
    pub source_file: String,
    pub delta_x: f32,
    pub delta_y: f32,
    pub delta_z: f32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SceneKey {
    source_file: String,
    object_index: i32,
}

impl SceneKey {
    fn new(file: &str, index: i32) -> Self {
        Self {
            source_file: file.to_ascii_uppercase(),
            object_index: index,
        }
    }
}

/// Transactional edit service applying object additions, removals, and translations (`SdlSceneEditService`).
pub struct SdlSceneEditService;

impl SdlSceneEditService {
    pub fn save_changes(
        map_directory: &Path,
        saved_objects: &[MapSceneObject],
        current_objects: &[MapSceneObject],
        removals: Option<&[SdlSceneObjectRemoval]>,
        additions: Option<&[SdlSceneObjectAddition]>,
        translations: Option<&[SdlSettlementTranslation]>,
    ) -> Result<(), String> {
        let map_path = CustomMapAccess::require_editable_directory(map_directory, None)?;
        let saved = Self::index(saved_objects)?;
        let current = Self::index(current_objects)?;

        if saved.len() != current.len() || saved.keys().any(|k| !current.contains_key(k)) {
            return Err("SDL 場景物件集合已改變，請重新開啟地圖後再試。".into());
        }

        let pending_removals = Self::validate_removals(removals, &current)?;
        let pending_additions = Self::validate_additions(additions, &current)?;
        let pending_translations = Self::validate_translations(translations)?;

        // Pre-read template fields before modifying any file
        let mut template_fields: HashMap<(String, i32), HashMap<String, String>> = HashMap::new();
        for addition in &pending_additions {
            let key = (addition.source_file.to_ascii_uppercase(), addition.template_index);
            if !template_fields.contains_key(&key) {
                let template_doc = SdlDocument::load(&Self::resolve_sdl_path(&map_path, &addition.source_file)?)?;
                let template = template_doc
                    .objects()
                    .into_iter()
                    .find(|s| s.index == addition.template_index)
                    .ok_or_else(|| {
                        format!(
                            "SDL 複製模板不存在：{} / {}。",
                            addition.source_file, addition.template_index
                        )
                    })?;

                let mut fields = template.fields;
                fields.insert("team".to_string(), addition.team.to_string());
                fields.insert(
                    "pos".to_string(),
                    SdlVector3::new(addition.local_x, addition.local_y, addition.local_z).to_string_2dp(),
                );
                if let Some(angle) = addition.angle {
                    fields.insert("angle".to_string(), format!("{:.2}", angle));
                }
                template_fields.insert(key, fields);
            }
        }

        // Determine all affected files
        let mut affected_files = HashSet::new();
        for item in current.values() {
            if let Some(saved_item) = saved.get(&SceneKey::new(&item.source_file, item.object_index)) {
                if Self::has_changed(saved_item, item) {
                    affected_files.insert(item.source_file.to_ascii_uppercase());
                }
            }
        }
        for r in &pending_removals {
            affected_files.insert(r.source_file.to_ascii_uppercase());
        }
        for a in &pending_additions {
            affected_files.insert(a.effective_target_file().to_ascii_uppercase());
        }
        for t in &pending_translations {
            affected_files.insert(t.source_file.to_ascii_uppercase());
        }

        for file_key in affected_files {
            let path = Self::resolve_sdl_path(&map_path, &file_key)?;
            let mut document = SdlDocument::load(&path)?;

            // 1. Update modified existing objects
            for item in current.values() {
                if item.source_file.eq_ignore_ascii_case(&file_key) {
                    if let Some(saved_item) = saved.get(&SceneKey::new(&item.source_file, item.object_index)) {
                        if Self::has_changed(saved_item, item) {
                            document.set_object_team(item.object_index, item.team)?;
                            document.set_object_position(
                                item.object_index,
                                SdlVector3::new(item.local_x, item.local_y, item.local_z),
                            )?;
                            if let Some(angle) = item.angle {
                                if saved_item.angle != Some(angle) {
                                    document.set_object_angle(item.object_index, angle)?;
                                }
                            }
                        }
                    }
                }
            }

            // 2. Settlement translations
            for translation in &pending_translations {
                if translation.source_file.eq_ignore_ascii_case(&file_key) {
                    document.translate_settlement(translation.delta_x, translation.delta_y, translation.delta_z)?;
                }
            }

            // 3. Additions
            for addition in &pending_additions {
                if addition.effective_target_file().eq_ignore_ascii_case(&file_key) {
                    let key = (addition.source_file.to_ascii_uppercase(), addition.template_index);
                    if let Some(fields) = template_fields.get(&key) {
                        document.add_object(fields)?;
                    }
                }
            }

            // 4. Removals in descending index order
            let mut file_removals: Vec<_> = pending_removals
                .iter()
                .filter(|r| r.source_file.eq_ignore_ascii_case(&file_key))
                .collect();
            file_removals.sort_by_key(|r| std::cmp::Reverse(r.object_index));
            for removal in file_removals {
                document.remove_object(removal.object_index)?;
            }

            document.save()?;
        }

        Ok(())
    }

    pub fn has_changes(baseline: &[MapSceneObject], current: &[MapSceneObject]) -> bool {
        let Ok(before) = Self::index(baseline) else { return true; };
        let Ok(after) = Self::index(current) else { return true; };
        if before.len() != after.len() {
            return true;
        }
        before.iter().any(|(k, v)| {
            match after.get(k) {
                Some(item) => Self::has_changed(v, item),
                None => true,
            }
        })
    }

    fn index(objects: &[MapSceneObject]) -> Result<HashMap<SceneKey, MapSceneObject>, String> {
        let mut map = HashMap::new();
        for item in objects {
            if item.object_index < 0 {
                return Err("SDL 場景物件缺少有效的 object 索引。".into());
            }
            Self::validate_scene_object(item)?;
            let key = SceneKey::new(&item.source_file, item.object_index);
            if map.contains_key(&key) {
                return Err(format!(
                    "SDL 場景物件識別重複：{} / {}。",
                    item.source_file, item.object_index
                ));
            }
            map.insert(key, item.clone());
        }
        Ok(map)
    }

    fn has_changed(before: &MapSceneObject, after: &MapSceneObject) -> bool {
        before.team != after.team
            || before.local_x != after.local_x
            || before.local_y != after.local_y
            || before.local_z != after.local_z
            || before.angle != after.angle
    }

    fn validate_removals(
        removals: Option<&[SdlSceneObjectRemoval]>,
        current: &HashMap<SceneKey, MapSceneObject>,
    ) -> Result<Vec<SdlSceneObjectRemoval>, String> {
        let Some(removals) = removals else { return Ok(Vec::new()); };
        let mut seen = HashSet::new();
        for r in removals {
            let key = SceneKey::new(&r.source_file, r.object_index);
            if !current.contains_key(&key) {
                return Err(format!("待刪除的 SDL 物件不存在：{} / {}。", r.source_file, r.object_index));
            }
            if !seen.insert(key) {
                return Err(format!("重複的 SDL 刪除項目：{} / {}。", r.source_file, r.object_index));
            }
        }
        Ok(removals.to_vec())
    }

    fn validate_additions(
        additions: Option<&[SdlSceneObjectAddition]>,
        current: &HashMap<SceneKey, MapSceneObject>,
    ) -> Result<Vec<SdlSceneObjectAddition>, String> {
        let Some(additions) = additions else { return Ok(Vec::new()); };
        for a in additions {
            let key = SceneKey::new(&a.source_file, a.template_index);
            if !current.contains_key(&key) {
                return Err(format!("SDL 複製模板不存在：{} / {}。", a.source_file, a.template_index));
            }
            if !(-1..=15).contains(&a.team) {
                return Err("SDL 場景物件隊伍必須介於 -1 與 15。".into());
            }
            if let Some(target) = &a.target_file {
                if !is_sdl_file_name(target) {
                    return Err("SDL 新增物件的目標檔名無效。".into());
                }
            }
            if let Some(angle) = a.angle {
                if !is_valid_angle(angle) {
                    return Err("SDL 場景物件角度必須介於 -360 與 360。".into());
                }
            }
            if !a.local_x.is_finite() || !a.local_y.is_finite() || !a.local_z.is_finite() {
                return Err("SDL 場景物件座標必須是有限數值。".into());
            }
        }
        Ok(additions.to_vec())
    }

    fn validate_translations(
        translations: Option<&[SdlSettlementTranslation]>,
    ) -> Result<Vec<SdlSettlementTranslation>, String> {
        let Some(translations) = translations else { return Ok(Vec::new()); };
        let mut seen = HashSet::new();
        for t in translations {
            if !is_sdl_file_name(&t.source_file) {
                return Err("SDL 聚落平移的來源檔名無效。".into());
            }
            if !seen.insert(t.source_file.to_ascii_uppercase()) {
                return Err(format!("重複的聚落平移項目：{}。", t.source_file));
            }
            if !t.delta_x.is_finite() || !t.delta_y.is_finite() || !t.delta_z.is_finite() {
                return Err("聚落平移量必須是有限數值。".into());
            }
        }
        Ok(translations.to_vec())
    }

    fn validate_scene_object(item: &MapSceneObject) -> Result<(), String> {
        if !is_sdl_file_name(&item.source_file) {
            return Err("SDL 場景來源檔名無效。".into());
        }
        if !(-1..=15).contains(&item.team) {
            return Err("SDL 場景物件隊伍必須介於 -1 與 15。".into());
        }
        if !item.local_x.is_finite() || !item.local_y.is_finite() || !item.local_z.is_finite() {
            return Err("SDL 場景物件座標必須是有限數值。".into());
        }
        if let Some(angle) = item.angle {
            if !is_valid_angle(angle) {
                return Err("SDL 場景物件角度必須介於 -360 與 360。".into());
            }
        }
        Ok(())
    }

    fn resolve_sdl_path(map_path: &Path, source_file: &str) -> Result<std::path::PathBuf, String> {
        let target = map_path.join(source_file);
        let canonical_map = map_path.canonicalize().map_err(|e| e.to_string())?;
        let canonical_target = target.canonicalize().map_err(|_| format!("找不到自製地圖的 SDL 場景檔: {}", target.display()))?;
        if canonical_target.parent() != Some(&canonical_map) {
            return Err(format!("找不到自製地圖的 SDL 場景檔: {}", target.display()));
        }
        Ok(canonical_target)
    }
}

fn is_sdl_file_name(file: &str) -> bool {
    let p = Path::new(file);
    p.file_name() == Some(std::ffi::OsStr::new(file))
        && file.to_ascii_lowercase().ends_with(".sdl")
}

fn is_valid_angle(angle: f32) -> bool {
    angle.is_finite() && (-360.0..=360.0).contains(&angle)
}
