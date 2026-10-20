//! Blank map creator with flat terrain, representative texture, and reset pools (`BlankMapBuilder`).
//! Replicates and unifies C# `BlankMapBuilder.cs`, `TerrainLayerFiles.cs`, and `BlankMapStarterKit.cs`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use against_rome_scenario::model::ScenarioDocument;
use crate::blank_content::BlankMapContent;
use crate::catalog::EndlessMapInfo;
use crate::cloner::EndlessMapCloner;
use crate::custom_map::CustomMapManifest;
use crate::rollback::{FileRollbackScope, SafeFileWriter};
use crate::starter_kit::BlankMapStarterKit;
use crate::text_docs::{BodenIniDocument, BodenTexturesDocument, PutTextDocument};
use crate::text_encoding::{can_encode_game_text, compress_pfil, encode_cp1251, is_pfil};

pub const HEIGHT_DEPENDENT_CACHES: &[&str] = &[
    "skydens.dat",
    "visible.dat",
    "cliprect.dat",
    "shadows.dat",
];

/// Specification for creating a blank endless map.
#[derive(Clone, Debug, PartialEq)]
pub struct BlankMapSpec {
    pub source_map_id: String,
    pub target_slot: i32,
    pub target_name: String,
    pub starter_kit: Option<StarterKitSpec>,
    pub height_override: Option<u8>,
    pub briefing_text: Option<String>,
}

/// Placement and team configuration for the blank map starter kit.
#[derive(Clone, Debug, PartialEq)]
pub struct StarterKitSpec {
    pub team: i32,
    pub center_x: f32,
    pub center_z: f32,
}

impl StarterKitSpec {
    pub fn new(team: i32, center_x: f32, center_z: f32) -> Self {
        Self {
            team,
            center_x,
            center_z,
        }
    }
}

impl BlankMapSpec {
    pub fn new(
        source_map_id: impl Into<String>,
        target_slot: i32,
        target_name: impl Into<String>,
    ) -> Self {
        Self {
            source_map_id: source_map_id.into(),
            target_slot,
            target_name: target_name.into(),
            starter_kit: None,
            height_override: None,
            briefing_text: None,
        }
    }

    pub fn with_starter_kit(mut self, team: i32, center_x: f32, center_z: f32) -> Self {
        self.starter_kit = Some(StarterKitSpec::new(team, center_x, center_z));
        self
    }

    pub fn with_height_override(mut self, height: u8) -> Self {
        self.height_override = Some(height);
        self
    }

    pub fn with_briefing(mut self, text: impl Into<String>) -> Self {
        self.briefing_text = Some(text.into());
        self
    }

    /// Pre-validates the specification against the target game installation without mutating disk state.
    pub fn validate(&self, game_path: &Path) -> Result<(), String> {
        if !(5..=999).contains(&self.target_slot) {
            return Err("自製地圖槽位必須在 ENDL_005 至 ENDL_999。".into());
        }

        let trimmed_name = self.target_name.trim();
        if trimmed_name.is_empty() {
            return Err("請輸入地圖名稱。".into());
        }
        if self.target_name.chars().any(|c| "/\\:*?\"<>|".contains(c)) {
            return Err("地圖名稱含不允許的字元。".into());
        }
        if !can_encode_game_text(&self.target_name) {
            return Err("地圖名稱包含遊戲無法儲存的字元。請改用英文、數字或原版地圖使用的字元。".into());
        }

        let maps_path = game_path.join("MAPS");
        let source_dir = maps_path.join(&self.source_map_id);
        if !source_dir.is_dir() {
            return Err(format!("來源地圖不存在: {}", self.source_map_id));
        }

        let target_id = format!("ENDL_{:03}", self.target_slot);
        let dest_dir = maps_path.join(&target_id);
        let temp_dir = maps_path.join(format!("{}.tmp_arm", target_id));
        if dest_dir.exists() || temp_dir.exists() {
            return Err(format!("目標地圖槽位已存在或有待清理暫存資料夾: {target_id}"));
        }

        if let Some(kit) = &self.starter_kit {
            if !(0..=7).contains(&kit.team) {
                return Err("Starter kit team must be 0–7.".into());
            }
            if !kit.center_x.is_finite()
                || !kit.center_z.is_finite()
                || !(0.0..=16383.0).contains(&kit.center_x)
                || !(0.0..=16383.0).contains(&kit.center_z)
            {
                return Err("Starter kit positions must be finite with X/Z between 0 and 16383.".into());
            }

            // Verify offsets stay within map bounds
            let offsets = [
                (0.0f32, 0.0f32),
                (-480.0, 800.0),
                (-160.0, 800.0),
                (160.0, 800.0),
                (480.0, 800.0),
            ];
            for (dx, dz) in offsets {
                let px = kit.center_x + dx;
                let pz = kit.center_z + dz;
                if !(0.0..=16383.0).contains(&px) || !(0.0..=16383.0).contains(&pz) {
                    return Err("Starter kit placement extends outside valid map boundaries (0..16383).".into());
                }
            }
        }

        if let Some(briefing) = &self.briefing_text {
            if briefing.contains('\0') || briefing.contains('\n') || briefing.contains('\r') {
                return Err("Briefing text must not contain NUL or line break characters.".into());
            }
            if !can_encode_game_text(briefing) {
                return Err("Briefing text contains characters incompatible with game encoding.".into());
            }
        }

        Ok(())
    }
}

/// Detailed metrics and results of a completed blank map build.
#[derive(Clone, Debug)]
pub struct BlankMapBuildResult {
    pub info: EndlessMapInfo,
    pub map_path: PathBuf,
    pub applied_height: u8,
    pub representative_texture: String,
    pub starter_kit_applied: bool,
    pub starter_spawns_count: usize,
    pub files_created: Vec<String>,
}

/// Summary of disk artifacts verified after building a blank map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiskArtifactSummary {
    pub slot: i32,
    pub map_id: String,
    pub has_manifest_entry: bool,
    pub boden_bmp_dim: (i32, i32),
    pub boden_height: u8,
    pub collision_bmp_dim: (i32, i32),
    pub minimap_bmp_dim: (i32, i32),
    pub representative_texture: String,
    pub scenario_spawns_count: usize,
    pub has_idle_level_script: bool,
    pub has_custom_marker: bool,
}

impl BlankMapBuildResult {
    /// Directly verifies all physical disk artifacts generated by the builder.
    pub fn verify_disk_artifacts(&self, game_path: &Path) -> Result<DiskArtifactSummary, String> {
        let manifest = CustomMapManifest::load(game_path)?;
        let has_manifest_entry = manifest.entries().iter().any(|e| e.slot == self.info.slot);
        if !has_manifest_entry {
            return Err(format!("Manifest missing registered entry for slot {}", self.info.slot));
        }

        let map_dir = &self.map_path;
        if !map_dir.is_dir() {
            return Err(format!("Map directory does not exist: {}", map_dir.display()));
        }

        // Verify height-dependent cache files are deleted
        for cache_name in HEIGHT_DEPENDENT_CACHES {
            let p = map_dir.join(cache_name);
            if p.exists() {
                return Err(format!("Height dependent cache was not invalidated: {cache_name}"));
            }
        }

        let boden_bmp_path = map_dir.join("boden.bmp");
        let boden_bytes = fs::read(&boden_bmp_path)
            .map_err(|e| format!("Failed to read boden.bmp: {e}"))?;
        let boden_bmp_dim = read_bmp_dimensions(&boden_bytes)?;
        if boden_bmp_dim != (257, 257) {
            return Err(format!("boden.bmp dimensions mismatch: {:?}", boden_bmp_dim));
        }
        let boden_height = if boden_bytes.len() > 54 { boden_bytes[54] } else { 0 };
        if boden_height != self.applied_height {
            return Err(format!(
                "boden.bmp height byte mismatch: expected {}, got {}",
                self.applied_height, boden_height
            ));
        }

        let collision_bmp_path = map_dir.join("collision.bmp");
        let collision_bytes = fs::read(&collision_bmp_path)
            .map_err(|e| format!("Failed to read collision.bmp: {e}"))?;
        let collision_bmp_dim = read_bmp_dimensions(&collision_bytes)?;
        if collision_bmp_dim != (256, 256) {
            return Err(format!("collision.bmp dimensions mismatch: {:?}", collision_bmp_dim));
        }

        let minimap_bmp_path = map_dir.join("minimap.bmp");
        let minimap_bytes = fs::read(&minimap_bmp_path)
            .map_err(|e| format!("Failed to read minimap.bmp: {e}"))?;
        let minimap_bmp_dim = read_bmp_dimensions(&minimap_bytes)?;
        if minimap_bmp_dim != (256, 256) {
            return Err(format!("minimap.bmp dimensions mismatch: {:?}", minimap_bmp_dim));
        }

        let boden_txt_path = map_dir.join("boden.txt");
        let textures_doc = BodenTexturesDocument::load(&boden_txt_path)?;
        let textures = textures_doc.textures()?;
        let representative_texture = textures.first().cloned().unwrap_or_default();
        if representative_texture != self.representative_texture {
            return Err(format!(
                "Representative texture mismatch: expected {}, got {}",
                self.representative_texture, representative_texture
            ));
        }

        let scenario_path = map_dir.join("arm_scenario.json");
        let scenario_bytes = fs::read_to_string(&scenario_path)
            .map_err(|e| format!("Failed to read arm_scenario.json: {e}"))?;
        let scenario: ScenarioDocument = serde_json::from_str(&scenario_bytes)
            .map_err(|e| format!("Failed to parse arm_scenario.json: {e}"))?;

        if scenario.spawns.len() != self.starter_spawns_count {
            return Err(format!(
                "Scenario spawn count mismatch: expected {}, got {}",
                self.starter_spawns_count,
                scenario.spawns.len()
            ));
        }

        if self.starter_kit_applied {
            if scenario.spawns.len() != 5 {
                return Err(format!("Starter kit expected 5 spawns, got {}", scenario.spawns.len()));
            }
            if scenario.spawns[0].alias != BlankMapStarterKit::MAIN_HOUSE_ALIAS {
                return Err(format!(
                    "First spawn should be main house {}, got {}",
                    BlankMapStarterKit::MAIN_HOUSE_ALIAS,
                    scenario.spawns[0].alias
                ));
            }
            for spawn in &scenario.spawns[1..] {
                if spawn.alias != BlankMapStarterKit::UNIT_ALIAS {
                    return Err(format!(
                        "Unit spawn should be {}, got {}",
                        BlankMapStarterKit::UNIT_ALIAS,
                        spawn.alias
                    ));
                }
            }
        }

        let level_script_path = map_dir.join("SCRIPT").join("ak_level.bci");
        let has_idle_level_script = level_script_path.is_file();
        if !has_idle_level_script {
            return Err("Missing idle ak_level.bci script on disk.".into());
        }

        let marker_path = map_dir.join(CustomMapManifest::MARKER_FILE_NAME);
        let has_custom_marker = marker_path.is_file();
        if !has_custom_marker {
            return Err("Missing custom map marker file on disk.".into());
        }

        Ok(DiskArtifactSummary {
            slot: self.info.slot,
            map_id: self.info.id(),
            has_manifest_entry,
            boden_bmp_dim,
            boden_height,
            collision_bmp_dim,
            minimap_bmp_dim,
            representative_texture,
            scenario_spawns_count: scenario.spawns.len(),
            has_idle_level_script,
            has_custom_marker,
        })
    }
}

/// Internal metrics emitted during preparation.
#[derive(Clone, Debug)]
pub struct BuildMetrics {
    pub applied_height: u8,
    pub representative_texture: String,
    pub starter_spawns_count: usize,
    pub files_created: Vec<String>,
}

pub struct BlankMapBuilder;

impl BlankMapBuilder {
    /// Creates a new custom endless map slot initialized according to `spec`.
    /// Performs pre-validation, preparation in an uncommitted staging directory,
    /// atomic publish, manifest registration, and returns full build results.
    /// If any error occurs, all changes are rolled back cleanly.
    pub fn build_with_spec(
        game_path: &Path,
        spec: &BlankMapSpec,
    ) -> Result<BlankMapBuildResult, String> {
        spec.validate(game_path)?;

        let cloner = EndlessMapCloner::new();
        let game_buf = game_path.to_path_buf();
        let spec_clone = spec.clone();
        let metrics_store: Arc<Mutex<Option<BuildMetrics>>> = Arc::new(Mutex::new(None));
        let metrics_capture = Arc::clone(&metrics_store);

        let info = cloner.clone_prepared(
            game_path,
            &spec.source_map_id,
            spec.target_slot,
            &spec.target_name,
            move |staging_directory| {
                let metrics = Self::prepare_with_spec(&game_buf, staging_directory, &spec_clone)?;
                if let Ok(mut lock) = metrics_capture.lock() {
                    *lock = Some(metrics);
                }
                Ok(())
            },
            Some(true),
        )?;

        let metrics = metrics_store
            .lock()
            .map_err(|_| "Failed to lock metrics store.".to_string())?
            .take()
            .ok_or_else(|| "Build metrics were not captured during preparation.".to_string())?;

        let map_path = game_path.join("MAPS").join(info.id());

        Ok(BlankMapBuildResult {
            info,
            map_path,
            applied_height: metrics.applied_height,
            representative_texture: metrics.representative_texture,
            starter_kit_applied: spec.starter_kit.is_some(),
            starter_spawns_count: metrics.starter_spawns_count,
            files_created: metrics.files_created,
        })
    }

    /// Creates a new custom endless map slot initialized as a blank scene (legacy convenience method).
    pub fn create(
        game_path: &Path,
        source_id: &str,
        slot: i32,
        name: &str,
    ) -> Result<EndlessMapInfo, String> {
        let spec = BlankMapSpec::new(source_id, slot, name);
        let result = Self::build_with_spec(game_path, &spec)?;
        Ok(result.info)
    }

    /// Prepares flat terrain, uniform texture, cleared pools, and an idle level script
    /// inside an uncommitted staging directory using default blank specifications.
    pub fn prepare(game_path: &Path, directory: &Path) -> Result<(), String> {
        let default_spec = BlankMapSpec::new("TEMPLATE", 5, "Blank");
        Self::prepare_with_spec(game_path, directory, &default_spec).map(|_| ())
    }

    /// Prepares flat terrain, uniform texture, cleared pools, starter kit (if requested),
    /// and idle level script according to `spec` inside an uncommitted staging directory.
    pub fn prepare_with_spec(
        game_path: &Path,
        directory: &Path,
        spec: &BlankMapSpec,
    ) -> Result<BuildMetrics, String> {
        let floortex_path = game_path.join("floortex.dat");
        if floortex_path.exists() && !floortex_path.is_file() {
            return Err("建立空白場景需要有效的 floortex.dat。".into());
        }

        let ini_path = directory.join("boden.ini");
        if !ini_path.is_file() {
            return Err("空白場景缺少 boden.ini。".into());
        }
        let ini = BodenIniDocument::load(&ini_path)?;
        let step = ini
            .height_step()
            .ok_or("空白場景的高度比例或水面資料不符。")?;
        let water = ini
            .water_level()
            .ok_or("空白場景的高度比例或水面資料不符。")?;

        if !step.is_finite() || step <= 0.0 || !water.is_finite() {
            return Err("空白場景的高度比例或水面資料不符。".into());
        }

        let height = if let Some(h) = spec.height_override {
            let h_val = h as i32;
            if (h_val as f32) * step <= water {
                return Err("指定的高度不高於水面，無法建立高於水面的空白場景。".into());
            }
            h_val
        } else {
            let calculated = (water / step).round() as i32 + 20;
            if !(0..=255).contains(&calculated) || (calculated as f32) * step <= water {
                return Err("水面太高，無法建立高於水面的空白場景。".into());
            }
            calculated
        };

        let boden_txt_path = directory.join("boden.txt");
        if !boden_txt_path.is_file() {
            return Err("空白場景缺少 boden.txt。".into());
        }
        let mut textures = BodenTexturesDocument::load(&boden_txt_path)?;
        let dim = textures.dimension()?;
        if dim != 64 {
            return Err("boden.bmp 尺寸不符。".into());
        }

        // Determine representative texture
        let existing_textures = textures.textures()?;
        if existing_textures.is_empty() {
            return Err("找不到可用的原生地表材質。".into());
        }

        let mut frequency: HashMap<String, usize> = HashMap::new();
        for t in &existing_textures {
            *frequency.entry(t.clone()).or_insert(0) += 1;
        }
        let mut sorted: Vec<_> = frequency.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let representative_texture = sorted[0].0.clone();

        let mut rollback = FileRollbackScope::new()?;
        let mut files_created = Vec::new();

        // Write uniform texture table (64x64)
        let uniform_table = vec![representative_texture.clone(); 64 * 64];
        textures.set_textures(&uniform_table)?;
        let plain_bytes = encode_cp1251(textures.text())?;
        let output_bytes = if is_pfil(&fs::read(&boden_txt_path).unwrap_or_default()) {
            compress_pfil(&plain_bytes, &fs::read(&boden_txt_path).unwrap_or_default()[..64])?
        } else {
            plain_bytes
        };
        SafeFileWriter::write_all_bytes(&boden_txt_path, &output_bytes, Some(&mut rollback))?;
        files_created.push("boden.txt".to_string());

        // Prepare terrain layer BMPs
        let layers: [(&str, u8, i32); 5] = [
            ("boden.bmp", height as u8, 257),
            ("vertex.bmp", 255, 257),
            ("smooth.bmp", 0, 257),
            ("emboss.bmp", 0, 257),
            ("collision.bmp", 0, 256),
        ];

        for (file_name, fill_value, expected_dim) in layers {
            let layer_path = directory.join(file_name);
            if !layer_path.is_file() {
                return Err(format!("空白場景缺少 {file_name}。"));
            }
            let existing_bytes = fs::read(&layer_path)
                .map_err(|e| format!("無法讀取 {file_name}: {e}"))?;
            let (w, h) = read_bmp_dimensions(&existing_bytes)
                .map_err(|_| format!("{file_name} 尺寸不符。"))?;
            if w != expected_dim || h != expected_dim {
                return Err(format!("{file_name} 尺寸不符。"));
            }

            let new_bmp = create_solid_bmp(expected_dim, expected_dim, fill_value);
            SafeFileWriter::write_all_bytes(&layer_path, &new_bmp, Some(&mut rollback))?;
            files_created.push(file_name.to_string());
        }

        // Create 256x256 minimap.bmp
        let minimap_path = directory.join("minimap.bmp");
        let minimap_bmp = create_solid_bmp(256, 256, 128);
        SafeFileWriter::write_all_bytes(&minimap_path, &minimap_bmp, Some(&mut rollback))?;
        files_created.push("minimap.bmp".to_string());

        // Invalidate height-dependent caches
        let removed_caches = invalidate_height_caches(directory, Some(&mut rollback))?;
        for c in removed_caches {
            files_created.push(format!("removed:{c}"));
        }

        // Reset all native binary pools, scenarios, and scripts
        BlankMapContent::reset_staging_directory(directory)?;

        // If starter kit was requested, apply it to arm_scenario.json
        let mut starter_spawns_count = 0;
        if let Some(kit_spec) = &spec.starter_kit {
            let world_y = (height as f32) * step;
            let height_fn = |_x: f32, _z: f32| world_y;
            let kit = BlankMapStarterKit::create(
                kit_spec.team,
                kit_spec.center_x,
                kit_spec.center_z,
                Some(&height_fn),
                None,
            )?;

            let scenario_path = directory.join("arm_scenario.json");
            let mut scenario: ScenarioDocument = serde_json::from_str(
                &fs::read_to_string(&scenario_path).map_err(|e| format!("讀取場景檔案失敗: {e}"))?,
            )
            .map_err(|e| format!("解析場景失敗: {e}"))?;

            kit.apply_to_scenario(&mut scenario)?;
            starter_spawns_count = scenario.spawns.len();

            let updated_json = serde_json::to_string_pretty(&scenario)
                .map_err(|e| format!("序列化更新後場景失敗: {e}"))?;
            SafeFileWriter::write_all_bytes(
                &scenario_path,
                updated_json.as_bytes(),
                Some(&mut rollback),
            )?;
            files_created.push("arm_scenario.json (starter kit)".to_string());
        }

        // Apply custom briefing text if provided
        if let Some(briefing_text) = &spec.briefing_text {
            let briefing_path = directory.join("TEXT").join("US").join("briefing.put");
            if briefing_path.exists() {
                if let Ok(mut put) = PutTextDocument::load(&briefing_path) {
                    put.set_value("briefing_titel_1", briefing_text)?;
                    let plain = encode_cp1251(put.text())?;
                    SafeFileWriter::write_all_bytes(&briefing_path, &plain, Some(&mut rollback))?;
                    files_created.push("briefing.put (custom title)".to_string());
                }
            }
        }

        rollback.commit();

        Ok(BuildMetrics {
            applied_height: height as u8,
            representative_texture,
            starter_spawns_count,
            files_created,
        })
    }

    /// Localized description of blank map operations.
    pub fn describe(source_id: &str, en: bool) -> String {
        if en {
            format!(
                "Source: {source_id}\r\n\
                 Cleared: all native DATA objects (including markers and linked objects), editor placements/events, source level scripts and saved spawn bindings. Settlement SDL templates are kept (emptying them hangs the game on load); the idle script never calls them.\r\n\
                 Created: flat dry terrain, one native material, passable ground and a new idle level script. No settlements, troops, resources or AI are supplied. Place starting objects and victory events yourself.\r\n\
                 Kept: faction/population defaults, environment and other native DATA pools from the template.\r\n\
                 Experimental: game startup and native DATA compatibility still require in-game validation."
            )
        } else {
            format!(
                "來源：{source_id}\r\n\
                 清除：所有原生 DATA 物件（含標記及連結物件）、編輯器放置／事件、來源關卡腳本與出生綁定。聚落 SDL 範本保留（清空會讓遊戲載入卡死），閒置腳本不會呼叫它們。\r\n\
                 建立：高於水面的平坦地形、單一原生材質、可通行地面與新的等待腳本。不提供聚落、部隊、資源或 AI；起始物件與勝利事件需自行放置。\r\n\
                 保留：範本派系／人口預設、環境及其他原生 DATA 池。\r\n\
                 實驗功能：遊戲開局及原生 DATA 相容性仍須在遊戲內驗證。"
            )
        }
    }
}

/// Deletes height-dependent caches (skydens.dat, visible.dat, cliprect.dat, shadows.dat).
pub fn invalidate_height_caches(
    map_directory: &Path,
    mut rollback: Option<&mut FileRollbackScope>,
) -> Result<Vec<String>, String> {
    let mut removed = Vec::new();
    for cache_name in HEIGHT_DEPENDENT_CACHES {
        let p = map_directory.join(cache_name);
        if p.is_file() {
            SafeFileWriter::delete_file(&p, rollback.as_deref_mut())?;
            removed.push((*cache_name).to_string());
        }
    }
    Ok(removed)
}

/// Creates a standard 24-bit uncompressed Windows BMP with uniform grayscale fill value.
pub fn create_solid_bmp(width: i32, height: i32, fill: u8) -> Vec<u8> {
    let stride = ((width * 3 + 3) / 4) * 4;
    let image_size = stride * height;
    let file_size = 54 + image_size;
    let mut bytes = vec![0u8; file_size as usize];

    bytes[0] = b'B';
    bytes[1] = b'M';
    bytes[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
    bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
    bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&width.to_le_bytes());
    bytes[22..26].copy_from_slice(&height.to_le_bytes());
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
    bytes[34..38].copy_from_slice(&(image_size as u32).to_le_bytes());

    for y in 0..height {
        let row = 54 + (y * stride) as usize;
        for x in 0..width {
            let offset = row + (x * 3) as usize;
            bytes[offset] = fill;
            bytes[offset + 1] = fill;
            bytes[offset + 2] = fill;
        }
    }

    bytes
}

/// Parses the width and height from a standard 54-byte BMP header.
pub fn read_bmp_dimensions(bytes: &[u8]) -> Result<(i32, i32), String> {
    if bytes.len() < 54 || bytes[0] != b'B' || bytes[1] != b'M' {
        return Err("無效的 BMP 標頭。".into());
    }
    let width = i32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]);
    let height = i32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]);
    Ok((width, height.abs()))
}
