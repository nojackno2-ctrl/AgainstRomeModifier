use against_rome_landscape::cliff::{CliffFacing, CliffTileCatalog, CliffTileEntry};
use against_rome_maps::*;
use against_rome_scenario::model::{Action, Condition, Event, ScenarioDocument, Spawn};
use std::fs;
use uuid::Uuid;

// ==========================================
// 1. Text Encoding & LZSS Compression Tests
// ==========================================

#[test]
fn test_can_encode_game_text_and_byte_count() {
    assert!(can_encode_game_text("Hello World! 123"));
    assert!(can_encode_game_text("Римляне и Варвары")); // Cyrillic
    assert!(!can_encode_game_text("中文測試")); // Chinese
    assert!(!can_encode_game_text("😀")); // Emoji

    let s = "Hello";
    assert_eq!(game_text_byte_count(s), 5);
}

#[test]
fn test_lzss_and_pfil_compression_roundtrip() {
    let original = b"The quick brown fox jumps over the lazy dog. The quick brown fox jumps over the lazy dog. 1234567890 1234567890";
    let compressed = text_encoding::compress_lzss(original);
    let decoded = against_rome_data::lzss::decode(&compressed, original.len()).unwrap();
    assert_eq!(original.to_vec(), decoded);

    let mut template_header = vec![0u8; 64];
    template_header[0..4].copy_from_slice(b"PFIL");
    let pfil_bytes = text_encoding::compress_pfil(original, &template_header).unwrap();
    assert!(text_encoding::is_pfil(&pfil_bytes));

    let decompressed = text_encoding::decompress_pfil_if_needed(&pfil_bytes).unwrap();
    assert_eq!(original.to_vec(), decompressed);
}

// ==========================================
// 2. Custom Map Manifest & Access Tests
// ==========================================

#[test]
fn test_parse_endl_slot() {
    assert_eq!(parse_endl_slot("ENDL_001"), Some(1));
    assert_eq!(parse_endl_slot("endl_005"), Some(5));
    assert_eq!(parse_endl_slot("MP_001"), None);
    assert_eq!(parse_endl_slot("ENDL_abc"), None);
}

#[test]
fn test_custom_map_manifest_and_access() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_manifest_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&temp_dir).unwrap();

    let map_dir = temp_dir.join("MAPS").join("ENDL_005");
    fs::create_dir_all(&map_dir).unwrap();
    fs::write(map_dir.join(CustomMapManifest::MARKER_FILE_NAME), "marker").unwrap();

    let mut manifest = CustomMapManifest::new();
    let entry = CustomMapEntry::new(5, 1, chrono::Utc::now(), "1.0.0");
    manifest.register(entry.clone()).unwrap();
    manifest.save(&temp_dir).unwrap();

    let loaded = CustomMapManifest::load(&temp_dir).unwrap();
    assert_eq!(loaded.entries().len(), 1);
    assert_eq!(loaded.entries()[0].slot, 5);

    assert!(CustomMapAccess::is_editable_directory(&map_dir));

    fs::remove_dir_all(&temp_dir).unwrap();
}

// ==========================================
// 3. Text Documents (Put, BodenIni, Sdl) Tests
// ==========================================

#[test]
fn test_put_text_document_escaping_and_composite() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_put_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&temp_dir).unwrap();
    let file_path = temp_dir.join("briefing.put");

    let initial = "; comment\r\nvar:briefing_titel_1 =\"Original\";\r\nvar:other =\"test\";\r\n";
    fs::write(&file_path, initial).unwrap();

    let mut doc = PutTextDocument::load(&file_path).unwrap();
    let val = "План \"Alpha\" C:\\new\\route\\ \t end";
    doc.set_value("briefing_titel_1", val).unwrap();
    doc.save().unwrap();

    let loaded = PutTextDocument::load(&file_path).unwrap();
    assert_eq!(loaded.get_value("briefing_titel_1").unwrap(), val);

    // Test rejection of invalid text (e.g. NUL or newlines or non-CP1251)
    assert!(doc.set_value("briefing_titel_1", "bad\0text").is_err());
    assert!(doc.set_value("briefing_titel_1", "bad\ntext").is_err());
    assert!(doc.set_value("briefing_titel_1", "中文").is_err());

    fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_boden_ini_document() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_ini_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&temp_dir).unwrap();
    let file_path = temp_dir.join("boden.ini");

    let initial = "[Dimension]\n48\n\n[Waterlevel]\n110.0\n\n[Heightmapstep]\n4.0\n";
    fs::write(&file_path, initial).unwrap();

    let mut doc = BodenIniDocument::load(&file_path).unwrap();
    assert_eq!(doc.dimension(), Some(48));
    assert_eq!(doc.water_level(), Some(110.0));
    assert_eq!(doc.height_step(), Some(4.0));

    doc.set_value("Dimension", "64").unwrap();
    doc.set_value("Waterlevel", "125.5").unwrap();
    doc.save().unwrap();

    let loaded = BodenIniDocument::load(&file_path).unwrap();
    assert_eq!(loaded.dimension(), Some(64));
    assert_eq!(loaded.water_level(), Some(125.5));

    fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_sdl_document_and_settlement_offset() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_sdl_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&temp_dir).unwrap();
    let file_path = temp_dir.join("map.sdl");

    let initial = "[settlement]\r\nrefpos=1000.0,50.0,2000.0\r\n\r\n[object0000]\r\ndef=1700\r\nnamedef=BauRomHau00_Haupthaus\r\npos=100.0,10.0,100.0\r\nteam=0\r\nangle=0.0\r\n";
    fs::write(&file_path, initial).unwrap();

    let mut doc = SdlDocument::load(&file_path).unwrap();
    let refpos = doc.get_settlement_refpos().unwrap();
    assert_eq!(refpos, SdlVector3::new(1000.0, 50.0, 2000.0));

    doc.translate_settlement(100.0, 10.0, 200.0).unwrap();
    assert_eq!(doc.get_settlement_refpos().unwrap(), SdlVector3::new(1100.0, 60.0, 2200.0));

    fs::remove_dir_all(&temp_dir).unwrap();
}

// ==========================================
// 4. Native Marker Type Table Tests
// ==========================================

fn mock_definitions() -> std::collections::HashMap<i32, NativeObjectDefinition> {
    let mut defs = std::collections::HashMap::new();
    let mut d = NativeObjectDefinition::new(123, true, 0);
    d.name = Some("Banner".into());
    defs.insert(123, d);
    defs
}

fn mock_engine(saved_type: i32) -> Vec<u8> {
    let mut eng = vec![0u8; 94];
    eng[0..4].copy_from_slice(&1i32.to_le_bytes());
    for i in 0..9 {
        eng[54 + i * 4..54 + i * 4 + 4].copy_from_slice(&saved_type.to_le_bytes());
    }
    eng
}

#[test]
fn test_native_marker_team_tribe_selection() {
    let mut defs = mock_definitions();
    let mut other = NativeObjectDefinition::new(456, true, 0);
    other.name = Some("OtherObject".into());
    defs.insert(456, other);

    let banner = "[volk00_obdef_bver07]\nBanner\nOtherObject\n";
    let team_dat = "[teamdata]\n3,GER,35,35,200,7";
    let eng = mock_engine(99);

    let result = NativeMarkerTypeTable::parse(banner, team_dat, &eng, &defs).unwrap();
    let sel = result.selections().get(3).unwrap();
    assert_eq!(sel.tribe, 0);
    assert_eq!(sel.banner_version, 7);
    assert_eq!(sel.definition_name.as_deref(), Some("Banner"));
    assert_eq!(sel.candidate_type, 123);
    assert_eq!(sel.effective_type, 123);

    assert_eq!(result.get_effective_type(8), 99); // team 8 is no-op
    assert_eq!(result.get_effective_type(-1), -1);
    assert_eq!(result.get_effective_type(9), -1);
}

#[test]
fn test_native_marker_invalid_candidate_preserves_saved_type() {
    let defs = mock_definitions();
    let result = NativeMarkerTypeTable::parse(
        "[volk00_obdef_bver00]\nMissingName",
        "[teamdata]\n0,GER",
        &mock_engine(812),
        &defs,
    )
    .unwrap();
    assert_eq!(result.get_effective_type(0), 812);
    assert_eq!(result.selections().get(0).unwrap().candidate_type, -1);
}

#[test]
fn test_native_marker_engine_shape_and_duplicates_rejected() {
    let defs = mock_definitions();
    assert!(NativeMarkerTypeTable::parse("", "", &[0u8; 93], &defs).is_err());

    let mut bad_eng = mock_engine(0);
    bad_eng[0] = 2; // bad version
    assert!(NativeMarkerTypeTable::parse("", "", &bad_eng, &defs).is_err());

    // Duplicate section in banner
    let dup_banner = "[volk00_obdef_bver00]\nBanner\n[VOLK00_OBDEF_BVER00]\nnone";
    assert!(NativeMarkerTypeTable::parse(dup_banner, "", &mock_engine(0), &defs).is_err());
}

// ==========================================
// 5. Native Object Definitions Tests
// ==========================================

#[test]
fn test_native_objdef_direct_light_position() {
    let mut def = NativeObjectDefinition::new(1, true, 0);
    def.direct_light_height = 50;
    def.direct_light_distance = 100.0;
    def.direct_light_angle = 0.0;

    let pos = [1000.0, 100.0, 2000.0];
    let [lx, ly, lz] = def.direct_light_position(pos, 0.0).unwrap();

    assert!((ly - 150.0).abs() < 1e-3);
    assert!((lx - 900.0).abs() < 1e-1 || (lx - 1100.0).abs() < 1e-1 || (lz - 2000.0).abs() < 1e-1);
}

// ==========================================
// 6. Native Pool Diagnostics Tests
// ==========================================

#[test]
fn test_native_pool_diagnostics_empty_or_valid() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_pool_{}", Uuid::new_v4().simple()));
    let data_dir = temp_dir.join("DATA");
    fs::create_dir_all(&data_dir).unwrap();

    // Diagnose empty directory
    let report = NativePoolDiagnostics::diagnose_directory(&data_dir, None);
    assert_eq!(report.active_objects, 0);

    fs::remove_dir_all(&temp_dir).unwrap();
}

// ==========================================
// 7. Map Design Validator Tests
// ==========================================

#[test]
fn test_map_design_validator_bounds() {
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let id3 = Uuid::new_v4();

    let spawns = vec![
        Spawn { alias: "GER_HAU00".into(), x: -10.0, z: 5000.0, team: 0, count: 0, id: id1, ..Default::default() },
        Spawn { alias: "GER_INF00".into(), x: 8000.0, z: 16385.0, team: 0, count: 10, id: id2, ..Default::default() },
        Spawn { alias: "ROM_HAU00".into(), x: f32::NAN, z: 5000.0, team: 1, count: 0, id: id3, ..Default::default() },
    ];

    let req = MapDesignValidationRequest {
        objects: spawns,
        ..Default::default()
    };

    let issues = MapDesignValidator::validate(&req);
    let bounds_issues: Vec<_> = issues.iter().filter(|i| i.code == "bounds").collect();
    assert_eq!(bounds_issues.len(), 3);
    assert!(bounds_issues.iter().any(|i| i.object_alias == "GER_HAU00" && i.object_id == id1));
    assert!(bounds_issues.iter().any(|i| i.object_alias == "GER_INF00" && i.object_id == id2));
    assert!(bounds_issues.iter().any(|i| i.object_alias == "ROM_HAU00" && i.object_id == id3));
}

#[test]
fn test_map_design_validator_water_and_collision() {
    let height_size = 257;
    let heights = vec![10u8; height_size * height_size]; // 10 * 4 = 40.0 < water_level 50.0

    let col_size = 256;
    let mut collision = vec![0u8; col_size * col_size];
    let target_idx = (8000.0 / 16384.0 * col_size as f32) as usize;
    collision[target_idx * col_size + target_idx] = 255; // impassable

    let obj_id = Uuid::new_v4();
    let spawns = vec![
        Spawn { alias: "GER_HAU00".into(), x: 8000.0, z: 8000.0, team: 0, count: 0, id: obj_id, ..Default::default() },
    ];

    let events = vec![
        Event {
            name: "WinLoss".into(),
            actions: vec![
                Action { kind: 3, ..Default::default() }, // Victory
                Action { kind: 4, ..Default::default() }, // Defeat
            ],
            ..Default::default()
        }
    ];

    let req = MapDesignValidationRequest {
        objects: spawns,
        events,
        collision: Some(collision),
        collision_size: col_size,
        heights: Some(heights),
        height_size,
        height_step: 4.0,
        water_level: 50.0,
        ..Default::default()
    };

    let issues = MapDesignValidator::validate(&req);
    assert!(issues.iter().any(|i| i.code == "water" && i.object_id == obj_id));
    assert!(issues.iter().any(|i| i.code == "impassable" && i.object_id == obj_id));
}

#[test]
fn test_map_design_validator_is_main_house() {
    assert!(MapDesignValidator::is_main_house("GER_HAU00"));
    assert!(MapDesignValidator::is_main_house("ROM_HAU00"));
    assert!(MapDesignValidator::is_main_house("KEL_HAU00"));
    assert!(MapDesignValidator::is_main_house("HUN_HAU00"));
    assert!(MapDesignValidator::is_main_house("BauGerHau00_Haupthaus"));
    assert!(MapDesignValidator::is_main_house("BauRomHau00_Hauptzelt"));
    assert!(MapDesignValidator::is_main_house("BauKelHau00_Haupthaus"));
    assert!(MapDesignValidator::is_main_house("BauHunHau00_Haupthaus"));

    assert!(!MapDesignValidator::is_main_house("GER_INF00"));
    assert!(!MapDesignValidator::is_main_house("BauGerTur00_Turm"));
    assert!(!MapDesignValidator::is_main_house(""));
}

#[test]
fn test_map_design_validator_win_loss() {
    let spawns = vec![
        Spawn { alias: "GER_HAU00".into(), x: 5000.0, z: 5000.0, team: 0, count: 0, id: Uuid::new_v4(), ..Default::default() },
        Spawn { alias: "GER_INF00".into(), x: 5100.0, z: 5000.0, team: 0, count: 10, id: Uuid::new_v4(), ..Default::default() },
    ];

    // 1. No events -> scenario-no-win-loss
    let req1 = MapDesignValidationRequest { objects: spawns.clone(), events: vec![], ..Default::default() };
    let issues1 = MapDesignValidator::validate(&req1);
    assert!(issues1.iter().any(|i| i.code == "scenario-no-win-loss"));

    // 2. Victory only -> scenario-no-defeat
    let victory_only = Event {
        name: "E".into(),
        actions: vec![Action { kind: 3, ..Default::default() }],
        ..Default::default()
    };
    let req2 = MapDesignValidationRequest { objects: spawns.clone(), events: vec![victory_only], ..Default::default() };
    let issues2 = MapDesignValidator::validate(&req2);
    assert!(issues2.iter().any(|i| i.code == "scenario-no-defeat"));
    assert!(!issues2.iter().any(|i| i.code == "scenario-no-victory"));
}

// ==========================================
// 8. Cliff Diagnostics Tests
// ==========================================

#[test]
fn test_cliff_diagnostics_unblocked_steep_slope() {
    let height_size = 9;
    let collision_size = 8;
    let mut heights = vec![10u8; height_size * height_size];
    for vx in 0..height_size {
        heights[vx] = 100; // y=0 is 100, delta = 90 >= 15
    }
    let collision = vec![0u8; collision_size * collision_size]; // unblocked

    let snapshot = MapCheckSnapshot {
        objects: vec![],
        events: vec![],
        aliases: vec![],
        collision_size,
        collision: Some(collision),
        height_size,
        heights: Some(heights),
        height_step: 4.0,
        water_level: 0.0,
    };

    let issues = CliffDiagnostics::check(&snapshot, None, None, 2);
    assert!(issues.iter().any(|i| i.code == "cliff-steep-unblocked" && i.severity == MapIssueSeverity::Warning));
}

#[test]
fn test_cliff_diagnostics_texture_missing_collision() {
    let tile_dim = 2;
    let collision_size = 8;
    let collision = vec![0u8; collision_size * collision_size]; // unblocked

    let mut catalog = CliffTileCatalog {
        entries: vec![],
        families: vec![],
        requires_exact_facing: false,
    };
    catalog.register(CliffTileEntry::new("FELS_N1", CliffFacing::North, "Rock"));

    let textures = vec!["FELS_N1".into(), "GRAS_1".into(), "GRAS_1".into(), "GRAS_1".into()];

    let snapshot = MapCheckSnapshot {
        objects: vec![],
        events: vec![],
        aliases: vec![],
        collision_size,
        collision: Some(collision),
        height_size: 0,
        heights: None,
        height_step: 4.0,
        water_level: 0.0,
    };

    let issues = CliffDiagnostics::check(&snapshot, Some(&catalog), Some(&textures), tile_dim);
    assert!(issues.iter().any(|i| i.code == "cliff-missing-collision" && i.severity == MapIssueSeverity::Warning));
}

// ==========================================
// 9. Map Diagnostics Tests
// ==========================================

#[test]
fn test_map_diagnostics_invalid_coords_and_isolated_building() {
    let unit_id = Uuid::new_v4();
    let house_id = Uuid::new_v4();

    let unit = MapCheckObject::new(
        Spawn { alias: "UNIT".into(), x: 1000.0, z: 1000.0, team: 0, count: 10, id: unit_id, ..Default::default() },
        false,
        false,
    );
    let house = MapCheckObject::new(
        Spawn { alias: "HOUSE".into(), x: 14000.0, z: 1000.0, team: 0, count: 0, prebuilt: true, id: house_id, ..Default::default() },
        true,
        true,
    );

    // Wall at x = 4 separating unit (x=1000) from house (x=14000)
    let size = 8;
    let mut collision = vec![0u8; size * size];
    for y in 0..size {
        collision[y * size + 4] = 255;
    }

    let snapshot = MapCheckSnapshot {
        objects: vec![unit, house],
        events: vec![],
        aliases: vec!["UNIT".into(), "HOUSE".into()],
        collision_size: size,
        collision: Some(collision),
        height_size: 0,
        heights: None,
        height_step: 4.0,
        water_level: 0.0,
    };

    let issues = MapDiagnostics::check(&snapshot);
    assert!(issues.iter().any(|i| i.code == "isolated-building" && i.object_id == house_id));
}

// ==========================================
// 10. Scenario Save Preflight Tests
// ==========================================

#[test]
fn test_scenario_save_preflight_deleted_target_rejected() {
    let item = Event {
        name: "Target".into(),
        actions: vec![Action { kind: 0, text: "Ready".into(), ..Default::default() }],
        conditions: vec![Condition { kind: 0, target_id: Uuid::new_v4(), ..Default::default() }],
        enabled: true,
        ..Default::default()
    };
    let mut doc = ScenarioDocument {
        events: vec![item],
        ..Default::default()
    };

    // Enabled event referencing deleted object -> Err
    assert!(ScenarioSavePreflight::validate(&doc, &[]).is_err());

    // Disabled event -> Ok
    doc.events[0].enabled = false;
    assert!(ScenarioSavePreflight::validate(&doc, &[]).is_ok());
}

#[test]
fn test_scenario_save_preflight_invalid_spawn_positions_and_counts() {
    let mut doc = ScenarioDocument::default();
    doc.spawns.push(Spawn {
        alias: "UNIT".into(),
        x: f32::NAN,
        z: 10.0,
        team: 0,
        count: 0,
        id: Uuid::new_v4(),
        ..Default::default()
    });
    assert!(ScenarioSavePreflight::validate(&doc, &["UNIT".into()]).is_err());

    // Unit count > 20
    doc.spawns[0].x = 10.0;
    doc.spawns[0].count = 21;
    assert!(ScenarioSavePreflight::validate(&doc, &["UNIT".into()]).is_err());

    // Team 8 for unit
    doc.spawns[0].count = 10;
    doc.spawns[0].team = 8;
    assert!(ScenarioSavePreflight::validate(&doc, &["UNIT".into()]).is_err());
}

// ==========================================
// 11. Cloner & Deleter Rollback Tests
// ==========================================

#[test]
fn test_cloner_and_deleter_workflow() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_cloner_{}", Uuid::new_v4().simple()));
    let maps_dir = temp_dir.join("MAPS");
    let source_dir = maps_dir.join("ENDL_001");
    fs::create_dir_all(&source_dir).unwrap();

    // Create minimal mock source files
    fs::write(source_dir.join("level.put"), "var:map_name =\"Template\";\r\n").unwrap();
    fs::write(source_dir.join("briefing.put"), "var:briefing_titel_1 =\"Brief\";\r\n").unwrap();
    fs::write(source_dir.join("map.sdl"), "[settlement]\r\nrefpos=100.0,0.0,100.0\r\n").unwrap();
    fs::write(source_dir.join("boden.txt"), "[Dimension]\n64\n\n[Texturen]\n").unwrap();
    fs::write(source_dir.join("minimap.bmp"), "BM").unwrap();

    let cloner = EndlessMapCloner::new();
    let cloned = cloner
        .clone_prepared(
            &temp_dir,
            "ENDL_001",
            5,
            "My Cloned Map",
            |target_dir: &std::path::Path| {
                // Prepare hook modifies file
                let path = target_dir.join("level.put");
                let mut doc = PutTextDocument::load(&path)?;
                doc.set_value("map_name", "Hook Name")?;
                doc.save()
            },
            None,
        )
        .unwrap();

    assert_eq!(cloned.slot, 5);
    assert_eq!(cloned.id(), "ENDL_005");
    let target_dir = maps_dir.join("ENDL_005");
    assert!(target_dir.exists());

    // Verify deleter safely removes it
    let deleter = EndlessMapDeleter::new();
    deleter.delete(&temp_dir, 5).unwrap();
    assert!(!target_dir.exists());

    fs::remove_dir_all(&temp_dir).unwrap();
}

// ==========================================
// 12. Boden Textures Document Tests
// ==========================================

#[test]
fn test_boden_textures_document() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_tex_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&temp_dir).unwrap();
    let file_path = temp_dir.join("boden.txt");

    let mut initial = String::from("[Dimension]\n64\n\n[Texturen]\n");
    for _ in 0..4096 {
        initial.push_str("GRAS_1\n");
    }
    fs::write(&file_path, initial).unwrap();

    let mut doc = BodenTexturesDocument::load(&file_path).unwrap();
    assert_eq!(doc.dimension().unwrap(), 64);
    assert_eq!(doc.get_texture(0, 0).unwrap(), "GRAS_1".to_string());

    doc.set_texture(0, 0, "FELS_N1").unwrap();
    doc.save().unwrap();

    let loaded = BodenTexturesDocument::load(&file_path).unwrap();
    assert_eq!(loaded.get_texture(0, 0).unwrap(), "FELS_N1".to_string());

    fs::remove_dir_all(&temp_dir).unwrap();
}

// ==========================================
// 13. NavMesh Analysis & Issue Conversion Tests
// ==========================================

#[test]
fn test_map_diagnostics_nav_mesh_analysis_and_conversion() {
    let size = 8;
    let mut collision = vec![0u8; size * size];
    // Block row 3
    for x in 0..size {
        collision[3 * size + x] = 255;
    }

    let unit = MapCheckObject::new(
        Spawn { alias: "UNIT".into(), x: 1000.0, z: 1000.0, team: 0, count: 10, id: Uuid::new_v4(), ..Default::default() },
        false,
        false,
    );
    let target = MapCheckObject::new(
        Spawn { alias: "ENEMY".into(), x: 14000.0, z: 14000.0, team: 1, count: 5, id: Uuid::new_v4(), ..Default::default() },
        false,
        false,
    );

    let snapshot = MapCheckSnapshot {
        objects: vec![unit, target],
        events: vec![],
        aliases: vec!["UNIT".into(), "ENEMY".into()],
        collision_size: size,
        collision: Some(collision),
        height_size: 0,
        heights: None,
        height_step: 4.0,
        water_level: 0.0,
    };

    let nav_result = MapDiagnostics::analyze_nav_mesh(&snapshot, None, 1);
    let issues = MapDiagnostics::convert_to_issues(&nav_result);
    // Should detect isolated regions across the wall
    assert!(nav_result.isolated_regions.len() >= 1);
    assert!(issues.iter().any(|i| i.code == "isolated-land"));
}

// ==========================================
// 14. Bilinear Height Sampling Parity Tests
// ==========================================

#[test]
fn test_sample_bilinear_raw_height_matches_formula() {
    // 3x3 grid:
    // (0,0)=10, (1,0)=20, (2,0)=30
    // (0,1)=40, (1,1)=50, (2,1)=60
    // (0,2)=70, (1,2)=80, (2,2)=90
    let heights = vec![
        10, 20, 30,
        40, 50, 60,
        70, 80, 90,
    ];
    let vertex_size = 3;

    // Center point (normX = 0.5, normZ = 0.5) -> gx = 1.0, gz = 1.0 -> exactly vertex (1,1) = 50.0
    let center = MapDesignValidator::sample_bilinear_raw_height(&heights, vertex_size, 0.5, 0.5);
    assert!((center - 50.0).abs() < 1e-4);

    // Corner (0,0) -> 10.0
    let corner = MapDesignValidator::sample_bilinear_raw_height(&heights, vertex_size, 0.0, 0.0);
    assert!((corner - 10.0).abs() < 1e-4);

    // Midpoint between (0,0) and (1,0): normX = 0.25 (gx = 0.5), normZ = 0.0 (gz = 0.0) -> 15.0
    let mid_x = MapDesignValidator::sample_bilinear_raw_height(&heights, vertex_size, 0.25, 0.0);
    assert!((mid_x - 15.0).abs() < 1e-4);
}
