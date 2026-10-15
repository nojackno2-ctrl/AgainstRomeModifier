use crate::contract::{is_payload_path, reject_links};
use crate::manifest::{
    CustomMapEntry, MapPackageFileEntry, MapPackageManifest,
    CURRENT_SCHEMA_VERSION, CUSTOM_MAPS_REGISTRY_FILE, CUSTOM_MAP_MARKER_FILE, MANIFEST_FILE_NAME,
};
use crate::preflight::{MapExportPreflightChecker, MapPreflightReport, PreflightCategory, PreflightSeverity};
use crate::thumbnail::{MinimapThumbnailRenderer, ThumbnailRenderOptions};
use chrono::Utc;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Error, ErrorKind, Read, Result, Write};
use std::path::Path;
use uuid::Uuid;
use zip::write::FileOptions;
use zip::{ZipArchive, ZipWriter};

pub const MIN_CUSTOM_SLOT: i32 = 5;
pub const MAX_CUSTOM_SLOT: i32 = 999;
pub const MAP_SUBDIRECTORY_IN_ZIP: &str = "map/";

#[derive(Clone, Debug)]
pub struct ModExportOptions {
    pub package_id: String,
    pub author: Option<String>,
    pub version: Option<String>,
    pub clean_caches: bool,
    pub force_export_on_errors: bool,
    pub generate_thumbnail: bool,
    pub thumbnail_resolution: usize,
}

impl Default for ModExportOptions {
    fn default() -> Self {
        Self {
            package_id: String::new(),
            author: None,
            version: None,
            clean_caches: true,
            force_export_on_errors: false,
            generate_thumbnail: true,
            thumbnail_resolution: 512,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ModExportResult {
    pub success: bool,
    pub output_zip_path: String,
    pub file_count: usize,
    pub total_size_bytes: u64,
    pub manifest: MapPackageManifest,
    pub preflight_report: MapPreflightReport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotCollisionStrategy {
    AutoAllocateNextFree,
    FailIfOccupied,
    OverwriteCustom,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModInstallPolicy {
    pub target_slot: Option<i32>,
    pub collision_strategy: SlotCollisionStrategy,
}

impl Default for ModInstallPolicy {
    fn default() -> Self {
        Self {
            target_slot: None,
            collision_strategy: SlotCollisionStrategy::AutoAllocateNextFree,
        }
    }
}

impl ModInstallPolicy {
    pub fn new(target_slot: Option<i32>, collision_strategy: SlotCollisionStrategy) -> Self {
        Self {
            target_slot,
            collision_strategy,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ModInstallResult {
    pub installed_slot: i32,
    pub directory_path: String,
    pub remapped_from_slot: Option<i32>,
    pub manifest: MapPackageManifest,
    pub extracted_file_count: usize,
    pub preflight_report: Option<MapPreflightReport>,
}

pub struct ModBundleExporter;

impl ModBundleExporter {
    pub fn export_to_zip(
        map_directory: &Path,
        output_zip_path: &Path,
        options: Option<&ModExportOptions>,
    ) -> Result<ModExportResult> {
        let default_opt = ModExportOptions::default();
        let options = options.unwrap_or(&default_opt);

        reject_links(map_directory)?;

        // 步驟 1：執行發布前預檢驗收
        let preflight = MapExportPreflightChecker::check(map_directory, None, None)?;
        if !preflight.can_export() && !options.force_export_on_errors {
            let first_error = preflight
                .issues
                .iter()
                .find(|i| i.severity == PreflightSeverity::Error)
                .unwrap();
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("地圖未通過發布預檢: [{}] {}", first_error.code, first_error.chinese),
            ));
        }

        // 步驟 2：建立並填寫 Manifest
        let pkg_id = if options.package_id.trim().is_empty() {
            map_directory
                .file_name()
                .map(|s| s.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_else(|| "custom_map".to_string())
        } else {
            options.package_id.clone()
        };

        let mut manifest = MapPackageManifest::create_from_directory(
            map_directory,
            Some(&pkg_id),
            options.author.as_deref(),
            options.version.as_deref(),
            None,
        )?;

        // Filter files
        let package_files: HashSet<String> = preflight.files_to_package.iter().cloned().collect();
        manifest.files.retain(|f| {
            is_payload_path(&f.relative_path)
                && (!options.clean_caches || package_files.contains(&f.relative_path))
        });

        if let Err(errs) = manifest.validate() {
            return Err(Error::new(ErrorKind::InvalidData, errs.join("; ")));
        }

        // 步驟 3：產生發布縮圖
        let thumbnail_bmp = if options.generate_thumbnail {
            Self::try_generate_thumbnail(map_directory, options.thumbnail_resolution, &manifest)
        } else {
            None
        };

        // 步驟 4：建立暫存檔案並執行 ZIP 打包
        if let Some(parent) = output_zip_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let temp_zip_path = output_zip_path.with_extension(format!("tmp_{}.zip", Uuid::new_v4().simple()));

        let mut file_count = 0;
        let mut total_bytes = 0u64;

        let result = (|| -> Result<()> {
            let file = File::create(&temp_zip_path)?;
            let mut zip = ZipWriter::new(file);
            let zip_options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

            // 寫入 thumbnail.bmp
            if let Some(thumb_bytes) = thumbnail_bmp {
                zip.start_file("thumbnail.bmp", zip_options)?;
                zip.write_all(&thumb_bytes)?;
            }

            // 寫入 README.txt
            zip.start_file("README.txt", zip_options)?;
            let readme = format!(
                "=======================================================\r\n\
                 Against Rome Custom Map: {}\r\n\
                 Author: {} | Version: {}\r\n\
                 Preferred Slot: ENDL_{:03}\r\n\
                 =======================================================\r\n\r\n\
                 {}\r\n\r\n\
                 [安裝方式]\r\n\
                 1. 先關閉遊戲，使用 tools/ArmMapPackage 安裝 CLI（詳見 docs/map-package-install.md）。\r\n\
                 2. map/ 是相對於槽位的內容；不可直接解壓到任意 ENDL_XXX。安裝器會選連續空位、重寫 SDL 並建立 marker。\r\n",
                manifest.title,
                manifest.author,
                manifest.version,
                manifest.compatibility.preferred_slot,
                manifest.description
            );
            zip.write_all(readme.as_bytes())?;

            // 寫入 map/ 檔案
            let files_to_include: Vec<MapPackageFileEntry> = manifest.files.clone();
            for file_entry in files_to_include {
                let source_path = map_directory.join(&file_entry.relative_path);
                if !source_path.exists() {
                    continue;
                }

                let zip_entry_name = format!("{}{}", MAP_SUBDIRECTORY_IN_ZIP, file_entry.relative_path);
                zip.start_file(zip_entry_name, zip_options)?;

                let mut src = File::open(&source_path)?;
                let mut buf = Vec::new();
                src.read_to_end(&mut buf)?;
                zip.write_all(&buf)?;

                file_count += 1;
                total_bytes += buf.len() as u64;
            }

            // 寫入 manifest.json
            let manifest_json = manifest.to_json().map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
            zip.start_file(MANIFEST_FILE_NAME, zip_options)?;
            zip.write_all(manifest_json.as_bytes())?;

            zip.finish()?;
            Ok(())
        })();

        if let Err(e) = result {
            let _ = fs::remove_file(&temp_zip_path);
            return Err(e);
        }

        if output_zip_path.exists() {
            let _ = fs::remove_file(output_zip_path);
        }
        fs::rename(&temp_zip_path, output_zip_path)?;

        Ok(ModExportResult {
            success: true,
            output_zip_path: output_zip_path.to_string_lossy().to_string(),
            file_count,
            total_size_bytes: total_bytes,
            manifest,
            preflight_report: preflight,
        })
    }

    pub fn install_from_zip(
        zip_path: &Path,
        game_path: &Path,
        policy: Option<&ModInstallPolicy>,
    ) -> Result<ModInstallResult> {
        if !zip_path.exists() || !zip_path.is_file() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("找不到模組 ZIP 檔案: {}", zip_path.display()),
            ));
        }

        let maps_root = game_path.join("MAPS");
        fs::create_dir_all(&maps_root)?;

        let default_policy = ModInstallPolicy::default();
        let policy = policy.unwrap_or(&default_policy);

        let zip_file = File::open(zip_path)?;
        let mut archive = ZipArchive::new(zip_file)?;

        let mut entry_names = HashSet::new();

        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            let name = file.name().to_string();
            let lower_name = name.to_ascii_lowercase();

            if !entry_names.insert(lower_name.clone()) {
                return Err(Error::new(ErrorKind::InvalidData, "Duplicate ZIP entry."));
            }

            if name == MANIFEST_FILE_NAME {
                if file.size() > 1024 * 1024 {
                    return Err(Error::new(ErrorKind::InvalidData, "Missing or oversized manifest.json."));
                }
            } else if name == "README.txt" || name == "thumbnail.bmp" {
                continue;
            } else if let Some(stripped) = name.strip_prefix(MAP_SUBDIRECTORY_IN_ZIP) {
                if !is_payload_path(stripped) {
                    return Err(Error::new(ErrorKind::InvalidData, format!("Unsupported ZIP path: {}", name)));
                }
            } else {
                return Err(Error::new(ErrorKind::InvalidData, format!("Unsupported ZIP path: {}", name)));
            }
        }

        let mut manifest_file = archive
            .by_name(MANIFEST_FILE_NAME)
            .map_err(|_| Error::new(ErrorKind::InvalidData, "Missing or oversized manifest.json."))?;
        let mut m_str = String::new();
        manifest_file.read_to_string(&mut m_str)?;
        drop(manifest_file);

        let manifest = MapPackageManifest::from_json(&m_str)?;
        if manifest.schema_version != CURRENT_SCHEMA_VERSION
            || manifest.payload_layout != "map-relative"
            || manifest.source_map_id != format!("ENDL_{:03}", manifest.compatibility.preferred_slot)
            || manifest.package_checksum != manifest.compute_package_checksum()
            || manifest.validate().is_err()
        {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Invalid or unsupported package manifest; re-export with schema 1.1.",
            ));
        }

        let mut payload_files = HashSet::new();
        let mut total_size: i64 = 0;

        for file in &manifest.files {
            if !is_payload_path(&file.relative_path) || !payload_files.insert(file.relative_path.to_ascii_lowercase()) {
                return Err(Error::new(ErrorKind::InvalidData, "Invalid payload inventory."));
            }
            let zip_entry_name = format!("{}{}", MAP_SUBDIRECTORY_IN_ZIP, file.relative_path);
            let entry = archive.by_name(&zip_entry_name).map_err(|_| {
                Error::new(ErrorKind::InvalidData, "Invalid payload inventory.")
            })?;
            if file.size_bytes != entry.size() as i64 || file.size_bytes < 0 || file.size_bytes > 128 * 1024 * 1024 {
                return Err(Error::new(ErrorKind::InvalidData, "Invalid payload inventory."));
            }
            total_size += file.size_bytes;
        }

        if total_size > 512 * 1024 * 1024 {
            return Err(Error::new(ErrorKind::InvalidData, "Payload inventory mismatch or size limit exceeded."));
        }

        let map_entry_count = entry_names
            .iter()
            .filter(|k| k.starts_with(MAP_SUBDIRECTORY_IN_ZIP))
            .count();
        if map_entry_count != payload_files.len() {
            return Err(Error::new(ErrorKind::InvalidData, "Payload inventory mismatch or size limit exceeded."));
        }

        let source_slot = manifest.compatibility.preferred_slot;
        if let Some(target) = policy.target_slot {
            if target < MIN_CUSTOM_SLOT {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "目標槽位屬於原廠官方地圖，禁止覆蓋！",
                ));
            }
            if target > MAX_CUSTOM_SLOT {
                return Err(Error::new(ErrorKind::InvalidInput, "Target slot out of range."));
            }
        }

        if policy.collision_strategy == SlotCollisionStrategy::OverwriteCustom {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "Install never overwrites maps. Install into the first free slot instead.",
            ));
        }

        reject_links(&maps_root)?;

        let next_free = Self::allocate_next_free_slot(game_path);
        let target_slot = match policy.collision_strategy {
            SlotCollisionStrategy::AutoAllocateNextFree => next_free,
            _ => policy.target_slot.unwrap_or(next_free),
        };

        if target_slot != next_free {
            return Err(Error::new(
                ErrorKind::AlreadyExists,
                format!("Target must be the first contiguous free slot: {}", next_free),
            ));
        }

        if target_slot != source_slot && !manifest.compatibility.allow_dynamic_slot_remapping {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Package disallows slot remapping.",
            ));
        }

        let target_dir = maps_root.join(format!("ENDL_{:03}", target_slot));
        if target_dir.exists() {
            return Err(Error::new(ErrorKind::AlreadyExists, "Slot is occupied."));
        }

        // Parse registry beforehand
        let registry_path = maps_root.join(CUSTOM_MAPS_REGISTRY_FILE);
        let mut custom_entries = if registry_path.exists() {
            let content = fs::read_to_string(&registry_path)?;
            serde_json::from_str::<Vec<CustomMapEntry>>(&content)
                .map_err(|e| Error::new(ErrorKind::InvalidData, format!("Malformed registry: {e}")))?
        } else {
            Vec::new()
        };

        let temp_staging = maps_root.join(format!(".arm_install_{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&temp_staging)?;

        let mut moved = false;

        let execute = (|| -> Result<ModInstallResult> {
            // Extract files
            for file_entry in &manifest.files {
                let zip_entry_name = format!("{}{}", MAP_SUBDIRECTORY_IN_ZIP, file_entry.relative_path);
                let mut zip_entry = archive.by_name(&zip_entry_name)?;

                let dest_file = temp_staging.join(&file_entry.relative_path);
                if let Some(p) = dest_file.parent() {
                    fs::create_dir_all(p)?;
                }

                let mut out_file = File::create(&dest_file)?;
                let mut buf = Vec::new();
                zip_entry.read_to_end(&mut buf)?;
                out_file.write_all(&buf)?;
                drop(out_file);

                let hash = against_rome_data::sha256::sha256(&buf);
                let hex_hash: String = hash.iter().map(|b| format!("{:02X}", b)).collect();
                if !hex_hash.eq_ignore_ascii_case(&file_entry.sha256) {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        format!("Payload checksum mismatch: {}", file_entry.relative_path),
                    ));
                }
            }

            let installed_preflight = MapExportPreflightChecker::check(&temp_staging, None, None)?;
            if installed_preflight.issues.iter().any(|i| {
                i.severity == PreflightSeverity::Error && i.category == PreflightCategory::StructuralBinary
            }) {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "Installed payload failed map preflight.",
                ));
            }

            // Rewrite known files
            Self::rewrite_known_files(&temp_staging, &manifest.source_map_id, &format!("ENDL_{:03}", target_slot), &manifest.title)?;

            // Write marker
            let marker = CustomMapEntry {
                slot: target_slot,
                source_slot,
                created_at: Utc::now().to_rfc3339(),
                tool_version: "AgainstRomeModifier.Packaging".to_string(),
                standalone_level: manifest.compatibility.standalone_level,
            };
            let marker_json = serde_json::to_string_pretty(&marker)
                .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
            fs::write(temp_staging.join(CUSTOM_MAP_MARKER_FILE), marker_json)?;

            // Move temp_staging to target_dir
            fs::rename(&temp_staging, &target_dir)?;
            moved = true;

            // Update registry
            custom_entries.retain(|e| e.slot != target_slot);
            custom_entries.push(marker);
            custom_entries.sort_by_key(|e| e.slot);

            let reg_json = serde_json::to_string_pretty(&custom_entries)
                .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
            fs::write(&registry_path, reg_json)?;

            Ok(ModInstallResult {
                installed_slot: target_slot,
                directory_path: target_dir.to_string_lossy().to_string(),
                remapped_from_slot: if target_slot == source_slot { None } else { Some(source_slot) },
                manifest: manifest.clone(),
                extracted_file_count: manifest.files.len(),
                preflight_report: Some(installed_preflight),
            })
        })();

        match execute {
            Ok(res) => {
                if temp_staging.exists() {
                    let _ = fs::remove_dir_all(&temp_staging);
                }
                Ok(res)
            }
            Err(e) => {
                if moved && target_dir.exists() {
                    let _ = fs::remove_dir_all(&target_dir);
                }
                if temp_staging.exists() {
                    let _ = fs::remove_dir_all(&temp_staging);
                }
                Err(e)
            }
        }
    }

    pub fn allocate_next_free_slot(game_path: &Path) -> i32 {
        let maps_root = game_path.join("MAPS");
        for slot in MIN_CUSTOM_SLOT..=MAX_CUSTOM_SLOT {
            let slot_dir = maps_root.join(format!("ENDL_{:03}", slot));
            if !slot_dir.exists() {
                return slot;
            }
        }
        MAX_CUSTOM_SLOT
    }

    pub fn rewrite_known_files(
        directory: &Path,
        old_map_id: &str,
        new_map_id: &str,
        new_name: &str,
    ) -> Result<()> {
        let briefing = directory.join("TEXT").join("US").join("briefing.put");
        if briefing.exists() {
            if let Ok(content) = fs::read_to_string(&briefing) {
                let mut lines = Vec::new();
                let mut replaced = false;
                for line in content.lines() {
                    if let Some((k, _)) = line.split_once('=') {
                        let key = k.trim().strip_prefix("var:").unwrap_or(k.trim()).trim();
                        if key.eq_ignore_ascii_case("briefing_titel_1") {
                            lines.push(format!("var:briefing_titel_1 = \"{}\";", new_name));
                            replaced = true;
                            continue;
                        }
                    }
                    lines.push(line.to_string());
                }
                if !replaced {
                    lines.push(format!("var:briefing_titel_1 = \"{}\";", new_name));
                }
                let mut out = lines.join("\r\n");
                out.push_str("\r\n");
                fs::write(&briefing, out)?;
            }
        }

        // SDL files
        if let Ok(entries) = fs::read_dir(directory) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    let lower = name.to_ascii_lowercase();
                    if lower.starts_with("endlos_") && lower.contains("_siedlung") && lower.ends_with(".sdl") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            let old_pattern = format!("MAPS/{}/", old_map_id);
                            let new_pattern = format!("MAPS/{}/", new_map_id);

                            let mut new_lines = Vec::new();
                            for line in content.lines() {
                                if let Some(idx) = line.to_ascii_lowercase().find(&old_pattern.to_ascii_lowercase()) {
                                    let mut line_replaced = line[..idx].to_string();
                                    line_replaced.push_str(&new_pattern);
                                    line_replaced.push_str(&line[idx + old_pattern.len()..]);
                                    new_lines.push(line_replaced);
                                } else {
                                    new_lines.push(line.to_string());
                                }
                            }
                            let mut out = new_lines.join("\r\n");
                            out.push_str("\r\n");
                            fs::write(&path, out)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn try_generate_thumbnail(
        map_directory: &Path,
        resolution: usize,
        manifest: &MapPackageManifest,
    ) -> Option<Vec<u8>> {
        let boden_path = map_directory.join("boden.bmp");
        if !boden_path.exists() {
            return None;
        }

        let heights = MapExportPreflightChecker::read_bmp_green_channel(&boden_path, 257, 257)?;

        let options = ThumbnailRenderOptions {
            width: resolution,
            height: resolution,
            water_level: manifest.dimensions.water_level,
            height_step: manifest.dimensions.height_step,
            show_water: true,
            show_spawn_banners: true,
            ..Default::default()
        };

        let mut spawns = Vec::new();
        let scenario_path = map_directory.join("arm_scenario.json");
        if scenario_path.exists() {
            if let Ok(content) = fs::read_to_string(&scenario_path) {
                if let Ok(scenario) = serde_json::from_str::<against_rome_scenario::model::ScenarioDocument>(&content) {
                    for s in scenario.spawns.into_iter().filter(|sp| sp.team >= 0) {
                        spawns.push(crate::thumbnail::MapThumbnailSpawnPoint::new(
                            s.x,
                            s.z,
                            s.team,
                            s.alias,
                            s.team == 0,
                        ));
                    }
                }
            }
        }

        Some(MinimapThumbnailRenderer::render_to_bmp(
            Some(&heights),
            257,
            &options,
            None,
            256,
            Some(&spawns),
            None,
        ))
    }
}
