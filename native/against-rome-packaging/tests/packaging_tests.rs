use against_rome_packaging::*;
use against_rome_scenario::{ScenarioDocument, Spawn};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use zip::write::FileOptions;
use zip::{ZipArchive, ZipWriter};

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Self {
        let dir = env::temp_dir().join(format!("{}_{}", prefix, Uuid::new_v4().simple()));
        fs::create_dir_all(&dir).unwrap();
        Self { path: dir }
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn write_solid_bmp(path: &Path, width: i32, height: i32, fill: u8) {
    let stride = ((width * 3 + 3) / 4) * 4;
    let image_size = stride * height;
    let file_size = 54 + image_size;
    let mut bytes = vec![0u8; file_size as usize];

    bytes[0] = b'B';
    bytes[1] = b'M';
    bytes[2..6].copy_from_slice(&file_size.to_le_bytes());
    bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
    bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&width.to_le_bytes());
    bytes[22..26].copy_from_slice(&height.to_le_bytes());
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
    bytes[34..38].copy_from_slice(&image_size.to_le_bytes());

    for y in 0..height {
        let row = 54 + (y * stride) as usize;
        for x in 0..width {
            let offset = row + (x * 3) as usize;
            bytes[offset] = fill;
            bytes[offset + 1] = fill;
            bytes[offset + 2] = fill;
        }
    }

    fs::write(path, bytes).unwrap();
}

fn create_valid_mock_map(dir: &Path, slot: i32, title: &str) {
    fs::create_dir_all(dir.join("TEXT").join("US")).unwrap();
    fs::create_dir_all(dir.join("DATA")).unwrap();
    fs::create_dir_all(dir.join("SCRIPT")).unwrap();

    write_solid_bmp(&dir.join("boden.bmp"), 257, 257, 128);
    write_solid_bmp(&dir.join("collision.bmp"), 256, 256, 0);

    fs::write(
        dir.join("boden.ini"),
        "Waterlevel = 50.0\nWaterColor = 30 100 180\n",
    )
    .unwrap();

    let put_content = format!(
        "var:briefing_titel_1 = \"{}\";\nvar:briefing_titel_2 = \"Subtitle\";\nvar:briefing_text = \"Test briefing\";\n",
        title
    );
    fs::write(dir.join("TEXT").join("US").join("briefing.put"), put_content).unwrap();

    fs::write(dir.join("DATA").join("objects.dat"), vec![0u8; 64]).unwrap();
    fs::write(dir.join("DATA").join("objdata.dat"), vec![0u8; 64]).unwrap();
    fs::write(dir.join("DATA").join("pos.dat"), vec![0u8; 64]).unwrap();

    fs::write(
        dir.join("SCRIPT").join("ak_level.bci"),
        b"PFIL\0\0\0\0",
    )
    .unwrap();

    let sdl_text = format!(
        "[settlement]\nname = MAPS/ENDL_{:03}/Endlos_005_Siedlung1.sdl\nrefpos = 8192, 100, 8192\n\n[object0]\nnamedef = BauGerHau00\npos = 0, 0, 0\nteam = 0\n",
        slot
    );
    fs::write(dir.join("Endlos_005_Siedlung1.sdl"), sdl_text).unwrap();

    let marker_json = format!(
        "{{\"Slot\":{},\"SourceSlot\":{},\"CreatedAt\":\"2026-10-08T00:00:00Z\",\"ToolVersion\":\"1.0\"}}",
        slot, slot
    );
    fs::write(dir.join(".arm_custom_map"), marker_json).unwrap();
}

fn create_install_fixture(temp_dir: &Path, standalone: bool) -> (PathBuf, PathBuf) {
    let source = temp_dir.join("source").join("MAPS").join("ENDL_005");
    create_valid_mock_map(&source, 5, "Test Forest");
    if standalone {
        let marker = CustomMapEntry {
            slot: 5,
            source_slot: 1,
            created_at: "2026-10-08T00:00:00Z".to_string(),
            tool_version: "test".to_string(),
            standalone_level: true,
        };
        fs::write(
            source.join(CUSTOM_MAP_MARKER_FILE),
            serde_json::to_string(&marker).unwrap(),
        )
        .unwrap();
    }
    let zip_path = temp_dir.join("fixture.zip");
    ModBundleExporter::export_to_zip(
        &source,
        &zip_path,
        Some(&ModExportOptions {
            generate_thumbnail: false,
            ..Default::default()
        }),
    )
    .unwrap();

    let game_path = temp_dir.join("game");
    fs::create_dir_all(game_path.join("MAPS")).unwrap();
    (zip_path, game_path)
}

#[test]
fn test_manifest_roundtrip_serialization_and_checksum() {
    let now = chrono::Utc::now();
    let mut manifest = MapPackageManifest {
        schema_version: "1.1".to_string(),
        package_id: "mod_teutoburg_forest".to_string(),
        source_map_id: "ENDL_008".to_string(),
        payload_layout: "map-relative".to_string(),
        title: "條頓堡之圍".to_string(),
        subtitle: Some("羅馬第十九軍團的殞落".to_string()),
        author: "TeutonModder".to_string(),
        version: "1.2.0".to_string(),
        description: "在深邃的條頓堡日耳曼密林中伏擊瓦盧斯的大軍。".to_string(),
        created_at: now,
        modified_at: now,
        dimensions: MapPackageDimensions {
            grid_width: 256,
            grid_height: 256,
            water_level: 75.5,
            ..Default::default()
        },
        players: MapRecommendedPlayers {
            min_players: 1,
            max_players: 4,
            optimal_players: 2,
            ..Default::default()
        },
        game_modes: vec!["Endless".to_string(), "Survival".to_string()],
        tags: vec!["Forest".to_string(), "Ambush".to_string(), "Historical".to_string()],
        compatibility: MapCompatibilityPolicy {
            preferred_slot: 8,
            allow_dynamic_slot_remapping: true,
            standalone_level: true,
            min_game_version: "1.0.0".to_string(),
            ..Default::default()
        },
        localizations: BTreeMap::new(),
        files: vec![
            MapPackageFileEntry {
                relative_path: "boden.bmp".to_string(),
                size_bytes: 198458,
                sha256: "ABCDEF123456".to_string(),
                category: MapFileCategory::TerrainHeight,
                is_required: true,
            },
            MapPackageFileEntry {
                relative_path: "collision.bmp".to_string(),
                size_bytes: 196662,
                sha256: "7890ABCDEF12".to_string(),
                category: MapFileCategory::Collision,
                is_required: true,
            },
        ],
        package_checksum: String::new(),
    };

    manifest.localizations.insert(
        "US".to_string(),
        MapPackageLocalization {
            title: "Battle of Teutoburg".to_string(),
            subtitle: Some("Fall of Varus".to_string()),
            briefing_text: None,
            team_names: None,
        },
    );
    manifest.localizations.insert(
        "DE".to_string(),
        MapPackageLocalization {
            title: "Varusschlacht".to_string(),
            subtitle: Some("Teutoburger Wald".to_string()),
            briefing_text: None,
            team_names: None,
        },
    );

    let json = manifest.to_json().unwrap();
    assert!(!manifest.package_checksum.is_empty());

    let loaded = MapPackageManifest::from_json(&json).unwrap();
    assert_eq!(manifest.package_id, loaded.package_id);
    assert_eq!(manifest.title, loaded.title);
    assert_eq!(manifest.subtitle, loaded.subtitle);
    assert_eq!(manifest.author, loaded.author);
    assert_eq!(manifest.version, loaded.version);
    assert_eq!(75.5, loaded.dimensions.water_level);
    assert!(loaded.compatibility.standalone_level);
    assert_eq!(8, loaded.compatibility.preferred_slot);
    assert_eq!(2, loaded.files.len());
    assert_eq!(manifest.package_checksum, loaded.package_checksum);
    assert!(loaded.validate().is_ok());
}

#[test]
fn test_manifest_validation_rejects_forbidden_native_slots() {
    let manifest = MapPackageManifest {
        package_id: "illegal_mod".to_string(),
        title: "覆蓋官方地圖".to_string(),
        compatibility: MapCompatibilityPolicy {
            preferred_slot: 2,
            ..Default::default()
        },
        ..Default::default()
    };

    let err = manifest.validate().unwrap_err();
    assert!(err.iter().any(|e| e.contains("PreferredSlot (2)")));
}

#[test]
fn test_minimap_thumbnail_renderer_generates_valid_24bit_bmp() {
    let size = 257;
    let mut heights = vec![0u8; size * size];

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - 128.0;
            let dy = y as f32 - 128.0;
            let dist = (dx * dx + dy * dy).sqrt();
            let h = (150.0 - dist).clamp(10.0, 220.0);
            heights[y * size + x] = h as u8;
        }
    }

    let options = ThumbnailRenderOptions {
        width: 256,
        height: 256,
        water_level: 100.0,
        height_step: 4.0,
        show_water: true,
        show_shorelines: true,
        show_spawn_banners: true,
        ..Default::default()
    };

    let spawns = vec![
        MapThumbnailSpawnPoint {
            world_x: 8192.0,
            world_z: 8192.0,
            team: 0,
            label: "P1".to_string(),
            is_player: true,
        },
        MapThumbnailSpawnPoint {
            world_x: 3000.0,
            world_z: 3000.0,
            team: 1,
            label: "P2".to_string(),
            is_player: true,
        },
    ];

    let bmp = MinimapThumbnailRenderer::render_to_bmp(
        Some(&heights),
        size,
        &options,
        None,
        0,
        Some(&spawns),
        None,
    );
    assert!(bmp.len() > 54);
    assert_eq!(bmp[0], b'B');
    assert_eq!(bmp[1], b'M');

    let width = i32::from_le_bytes(bmp[18..22].try_into().unwrap());
    let height = i32::from_le_bytes(bmp[22..26].try_into().unwrap());
    let bpp = i16::from_le_bytes(bmp[28..30].try_into().unwrap());

    assert_eq!(256, width);
    assert_eq!(256, height);
    assert_eq!(24, bpp);

    let stride = ((256 * 3 + 3) / 4) * 4;
    assert_eq!(54 + stride * 256, bmp.len());
}

#[test]
fn test_preflight_checker_detects_missing_files_and_sanitizes_height_caches() {
    let temp = TempDirGuard::new("ArmPreflightTest");
    let map_dir = temp.path.join("ENDL_005");
    fs::create_dir_all(&map_dir).unwrap();

    fs::write(map_dir.join("boden.ini"), "Waterlevel = 0\n").unwrap();

    let report = MapExportPreflightChecker::check(&map_dir, None, None).unwrap();
    assert!(!report.can_export());
    assert!(report.error_count() >= 4);
    assert!(report.issues.iter().any(|i| i.code == "missing-boden"));
    assert!(report.issues.iter().any(|i| i.code == "missing-collision"));

    fs::write(map_dir.join("skydens.dat"), "cache").unwrap();
    fs::write(map_dir.join("shadows.dat"), "cache").unwrap();

    let report2 = MapExportPreflightChecker::check(&map_dir, None, None).unwrap();
    assert!(report2.files_to_exclude.contains(&"skydens.dat".to_string()));
    assert!(report2.files_to_exclude.contains(&"shadows.dat".to_string()));
    assert!(report2.issues.iter().any(|i| i.code == "height-cache-excluded"));
}

#[test]
fn test_mod_bundle_exporter_full_export_and_slot_remapped_install() {
    let temp = TempDirGuard::new("ArmFullExportTest");
    let source_map = temp.path.join("SourceMap_ENDL_005");
    create_valid_mock_map(&source_map, 5, "Teuton Forest");

    let zip_output = temp.path.join("TeutonForest_1.0.armpack");

    let export_options = ModExportOptions {
        package_id: "teuton_forest".to_string(),
        author: Some("ArmDev".to_string()),
        version: Some("1.0.0".to_string()),
        generate_thumbnail: true,
        thumbnail_resolution: 256,
        ..Default::default()
    };

    let export_result = ModBundleExporter::export_to_zip(&source_map, &zip_output, Some(&export_options)).unwrap();
    assert!(export_result.success);
    assert!(zip_output.exists());
    assert!(export_result.file_count > 0);

    {
        let zip_file = File::open(&zip_output).unwrap();
        let mut archive = ZipArchive::new(zip_file).unwrap();
        assert!(archive.by_name("manifest.json").is_ok());
        assert!(archive.by_name("thumbnail.bmp").is_ok());
        assert!(archive.by_name("README.txt").is_ok());
        assert!(archive.by_name("map/boden.bmp").is_ok());
        assert!(archive.by_name("map/boden.ini").is_ok());
        assert!(archive.by_name("map/TEXT/US/briefing.put").is_ok());
    }

    let mock_game = temp.path.join("MockGame");
    let mock_maps = mock_game.join("MAPS");
    fs::create_dir_all(&mock_maps).unwrap();

    let occupied = mock_maps.join("ENDL_005");
    fs::create_dir_all(&occupied).unwrap();
    fs::write(occupied.join(".arm_custom_map"), "{\"Slot\":5}").unwrap();

    let install_policy = ModInstallPolicy {
        target_slot: Some(5),
        collision_strategy: SlotCollisionStrategy::AutoAllocateNextFree,
        ..Default::default()
    };

    let install_result = ModBundleExporter::install_from_zip(&zip_output, &mock_game, Some(&install_policy)).unwrap();
    assert_eq!(6, install_result.installed_slot);
    assert_eq!(Some(5), install_result.remapped_from_slot);
    assert!(mock_maps.join("ENDL_006").exists());

    let installed_sdl = mock_maps.join("ENDL_006").join("Endlos_005_Siedlung1.sdl");
    assert!(installed_sdl.exists());
    let sdl_content = fs::read_to_string(installed_sdl).unwrap();
    assert!(sdl_content.contains("MAPS/ENDL_006/"));
    assert!(!sdl_content.contains("MAPS/ENDL_005/"));

    let reg_path = mock_maps.join(CUSTOM_MAPS_REGISTRY_FILE);
    assert!(reg_path.exists());
    let reg_entries: Vec<CustomMapEntry> = serde_json::from_str(&fs::read_to_string(reg_path).unwrap()).unwrap();
    assert!(reg_entries.iter().any(|e| e.slot == 6));
}

#[test]
fn test_mod_bundle_exporter_install_rejects_native_slots_strictly() {
    let temp = TempDirGuard::new("ArmNativeRejectTest");
    let source_map = temp.path.join("SourceMap_ENDL_005");
    create_valid_mock_map(&source_map, 5, "Native Slot Test");

    let zip_output = temp.path.join("NativeAttack.zip");
    ModBundleExporter::export_to_zip(
        &source_map,
        &zip_output,
        Some(&ModExportOptions {
            force_export_on_errors: true,
            generate_thumbnail: false,
            ..Default::default()
        }),
    )
    .unwrap();

    let mock_game = temp.path.join("MockGameNative");
    fs::create_dir_all(mock_game.join("MAPS")).unwrap();

    for illegal_slot in 0..5 {
        let policy = ModInstallPolicy {
            target_slot: Some(illegal_slot),
            collision_strategy: SlotCollisionStrategy::FailIfOccupied,
            ..Default::default()
        };
        let err = ModBundleExporter::install_from_zip(&zip_output, &mock_game, Some(&policy));
        assert!(err.is_err());
        let msg = err.err().unwrap().to_string();
        assert!(msg.contains("原廠官方地圖") || msg.contains("5"));
    }
}

#[test]
fn test_export_install_fills_first_gap() {
    for &occupy_five in &[false, true] {
        let temp = TempDirGuard::new("ArmGapTest");
        let source = temp.path.join("ENDL_008");
        create_valid_mock_map(&source, 8, "Portable Forest");
        fs::write(source.join("shadows.dat"), "cache").unwrap();
        fs::create_dir_all(source.join("TEXTURES")).unwrap();
        fs::write(source.join("TEXTURES").join("game.apt"), "global asset").unwrap();
        fs::write(source.join("game.exe"), "global executable").unwrap();

        let zip_path = temp.path.join("portable.zip");
        let export = ModBundleExporter::export_to_zip(
            &source,
            &zip_path,
            Some(&ModExportOptions {
                generate_thumbnail: false,
                clean_caches: false,
                ..Default::default()
            }),
        )
        .unwrap();

        {
            let zip_file = File::open(&zip_path).unwrap();
            let mut archive = ZipArchive::new(zip_file).unwrap();
            assert!(archive.by_name("map/.arm_custom_map").is_err());
            assert!(archive.by_name("map/TEXTURES/game.apt").is_err());
            assert!(archive.by_name("map/game.exe").is_err());
        }

        assert_eq!("ENDL_008", export.manifest.source_map_id);
        assert_eq!("map-relative", export.manifest.payload_layout);

        let game = temp.path.join("target");
        fs::create_dir_all(game.join("MAPS").join("ENDL_008")).unwrap();
        if occupy_five {
            fs::create_dir_all(game.join("MAPS").join("ENDL_005")).unwrap();
        }

        let result = ModBundleExporter::install_from_zip(&zip_path, &game, None).unwrap();
        let expected = if occupy_five { 6 } else { 5 };
        assert_eq!(expected, result.installed_slot);

        let sdl_path = Path::new(&result.directory_path).join("Endlos_005_Siedlung1.sdl");
        let sdl = fs::read_to_string(sdl_path).unwrap();
        assert!(sdl.contains(&format!("MAPS/ENDL_{:03}/", expected)));
        assert!(!sdl.contains("MAPS/ENDL_008/"));

        let marker_file = Path::new(&result.directory_path).join(CUSTOM_MAP_MARKER_FILE);
        let marker: CustomMapEntry = serde_json::from_str(&fs::read_to_string(marker_file).unwrap()).unwrap();
        assert_eq!(expected, marker.slot);
        assert_eq!(8, marker.source_slot);
        assert!(!marker.standalone_level);
    }
}

#[test]
fn test_install_rejects_unsafe_or_unlisted_zip_entries() {
    let temp = TempDirGuard::new("ArmUnsafeTest");
    let (zip, game) = create_install_fixture(&temp.path, false);

    let bad_paths = [
        "map/../escape.dat",
        "map/DATA/../../escape.dat",
        "map/C:/escape.dat",
        "map/DATA/CON.dat",
        "map/TEXTURES/game.apt",
        "map/.arm_custom_map",
        "map/BODEN.BMP",
    ];

    for path in bad_paths {
        let zip_temp = temp.path.join(format!("bad_{}.zip", Uuid::new_v4().simple()));
        fs::copy(&zip, &zip_temp).unwrap();

        {
            let file = fs::OpenOptions::new().read(true).write(true).open(&zip_temp).unwrap();
            let mut writer = ZipWriter::new_append(file).unwrap();
            writer.start_file(path, FileOptions::default()).unwrap();
            writer.write_all(b"attack").unwrap();
            writer.finish().unwrap();
        }

        let res = ModBundleExporter::install_from_zip(&zip_temp, &game, None);
        assert!(res.is_err(), "Path {} should have been rejected", path);
    }
}

#[test]
fn test_install_rejects_corrupt_payload_without_changing_registry() {
    let temp = TempDirGuard::new("ArmCorruptTest");
    let (zip, game) = create_install_fixture(&temp.path, false);

    let reg_path = game.join("MAPS").join(CUSTOM_MAPS_REGISTRY_FILE);
    fs::write(&reg_path, "[]").unwrap();

    let zip_mod = temp.path.join("corrupt.zip");

    {
        let src_file = File::open(&zip).unwrap();
        let mut src_archive = ZipArchive::new(src_file).unwrap();
        let dst_file = File::create(&zip_mod).unwrap();
        let mut dst_writer = ZipWriter::new(dst_file);

        for i in 0..src_archive.len() {
            let mut file = src_archive.by_index(i).unwrap();
            let name = file.name().to_string();
            dst_writer.start_file(name.clone(), FileOptions::default()).unwrap();
            if name == "map/DATA/objects.dat" {
                dst_writer.write_all(&vec![255u8; 64]).unwrap();
            } else {
                let mut buf = Vec::new();
                file.read_to_end(&mut buf).unwrap();
                dst_writer.write_all(&buf).unwrap();
            }
        }
        dst_writer.finish().unwrap();
    }

    let res = ModBundleExporter::install_from_zip(&zip_mod, &game, None);
    assert!(res.is_err());
    assert_eq!("[]", fs::read_to_string(&reg_path).unwrap());
}

#[test]
fn test_preflight_unknown_alias_handling() {
    let temp = TempDirGuard::new("ArmAliasTest");
    let source = temp.path.join("source").join("MAPS").join("ENDL_005");
    create_valid_mock_map(&source, 5, "Alias Test");

    let scenario = ScenarioDocument {
        spawns: vec![Spawn {
            id: Uuid::new_v4(),
            alias: "UNIT".to_string(),
            x: 4000.0,
            z: 5000.0,
            team: 0,
            ..Default::default()
        }],
        ..Default::default()
    };

    let unverified = MapExportPreflightChecker::check(&source, Some(&scenario), None).unwrap();
    assert!(unverified.issues.iter().any(|i| i.code == "alias" && i.severity == PreflightSeverity::Warning));

    let empty_catalog: Vec<String> = Vec::new();
    let verified = MapExportPreflightChecker::check(&source, Some(&scenario), Some(&empty_catalog)).unwrap();
    assert!(verified.issues.iter().any(|i| i.code == "alias" && i.severity == PreflightSeverity::Error));
}
