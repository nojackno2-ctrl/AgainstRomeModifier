use against_rome_landscape::cliff::{CliffFacing, CliffTileCatalog, CliffTileEntry};
use against_rome_maps::*;
use against_rome_scenario::model::{Action, Condition, Event, ScenarioDocument, Spawn};
use std::fs;
use std::path::Path;
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
fn test_map_design_validator_event_only_wave_team_needs_no_main_house() {
    let wave = Event {
        name: "Wave_01".into(),
        actions: vec![Action { kind: 2, team: 1, alias: "HUN_KAVINF00".into(), count: 3, ..Default::default() }],
        ..Default::default()
    };
    let base = |team| Spawn { alias: "GER_INF01".into(), x: 8000.0, z: 8000.0, team, count: 10, id: Uuid::new_v4(), ..Default::default() };
    let no_house = |objects: Vec<Spawn>| -> Vec<i32> {
        let req = MapDesignValidationRequest { objects, events: vec![wave.clone()], ..Default::default() };
        MapDesignValidator::validate(&req).into_iter().filter(|i| i.code == "team-no-main-house").map(|i| i.team).collect()
    };
    // Team 1 exists only as event-spawned attack waves: no base is expected.
    assert_eq!(no_house(vec![base(0)]), vec![0]);
    // Once team 1 has placed objects it is a faction and must have a main house.
    assert_eq!(no_house(vec![base(0), base(1)]), vec![0, 1]);
}

#[test]
fn test_untouched_native_map_reports_one_native_only_warning() {
    let codes = |req: &MapDesignValidationRequest| -> Vec<String> {
        MapDesignValidator::validate(req).into_iter().map(|i| i.code).collect()
    };
    // Untouched template: one explanation instead of team/start/win-loss guesses.
    assert_eq!(codes(&MapDesignValidationRequest::default()), vec!["scenario-native-only".to_string()]);
    let empty = MapCheckSnapshot::new(vec![], vec![], vec!["U".into()], 8, Some(vec![0u8; 64]));
    assert!(MapDiagnostics::check(&empty).iter().all(|i| i.code != "no-start"));
    // Events alone (a template plus Victory/Defeat) keep the native base and troops unguessed.
    let events_only = MapDesignValidationRequest {
        events: vec![Event {
            name: "win".into(),
            actions: vec![Action { kind: 3, ..Default::default() }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let codes_events_only = codes(&events_only);
    assert!(!codes_events_only.iter().any(|c| c.starts_with("team-") || c == "scenario-native-only"), "{codes_events_only:?}");
    // One authored object brings back the full checks.
    let req = MapDesignValidationRequest {
        objects: vec![Spawn { alias: "GER_INF01".into(), x: 8000.0, z: 8000.0, team: 0, count: 10, id: Uuid::new_v4(), ..Default::default() }],
        ..Default::default()
    };
    let authored = codes(&req);
    assert!(!authored.contains(&"scenario-native-only".to_string()));
    assert!(authored.contains(&"scenario-no-win-loss".to_string()));
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

// ==========================================
// 15. FileRollbackScope & SafeFileWriter Tests
// ==========================================

#[test]
fn test_rollback_file_mutation_and_deletion() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_rb_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&temp_dir).unwrap();

    let file_to_modify = temp_dir.join("modify.txt");
    fs::write(&file_to_modify, b"initial content").unwrap();

    let file_to_delete = temp_dir.join("delete.txt");
    fs::write(&file_to_delete, b"do not permanently delete").unwrap();

    let file_to_create = temp_dir.join("create.txt");

    {
        let mut scope = FileRollbackScope::new().unwrap();
        // Modify file
        SafeFileWriter::write_all_bytes(&file_to_modify, b"changed content", Some(&mut scope)).unwrap();
        assert_eq!(fs::read(&file_to_modify).unwrap(), b"changed content");

        // Delete file
        SafeFileWriter::delete_file(&file_to_delete, Some(&mut scope)).unwrap();
        assert!(!file_to_delete.exists());

        // Create file
        SafeFileWriter::write_all_bytes(&file_to_create, b"new content", Some(&mut scope)).unwrap();
        assert!(file_to_create.exists());

        // Drop scope WITHOUT calling commit()
    }

    // After rollback:
    assert_eq!(fs::read(&file_to_modify).unwrap(), b"initial content");
    assert_eq!(fs::read(&file_to_delete).unwrap(), b"do not permanently delete");
    assert!(!file_to_create.exists());

    // Test committing retains changes
    {
        let mut scope = FileRollbackScope::new().unwrap();
        SafeFileWriter::write_all_bytes(&file_to_modify, b"committed content", Some(&mut scope)).unwrap();
        scope.commit();
    }
    assert_eq!(fs::read(&file_to_modify).unwrap(), b"committed content");

    fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_rollback_directory_snapshot() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_dir_rb_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&temp_dir).unwrap();

    let sub_dir = temp_dir.join("sub");
    fs::create_dir_all(&sub_dir).unwrap();
    fs::write(sub_dir.join("a.txt"), b"file a").unwrap();
    fs::write(sub_dir.join("b.txt"), b"file b").unwrap();

    {
        let mut scope = FileRollbackScope::new().unwrap();
        scope.track_directory(&sub_dir).unwrap();

        // Mutate directory contents
        fs::write(sub_dir.join("a.txt"), b"corrupted a").unwrap();
        fs::remove_file(sub_dir.join("b.txt")).unwrap();
        fs::write(sub_dir.join("c.txt"), b"extra c").unwrap();

        // Drop without commit -> should restore directory
    }

    assert_eq!(fs::read(sub_dir.join("a.txt")).unwrap(), b"file a");
    assert_eq!(fs::read(sub_dir.join("b.txt")).unwrap(), b"file b");
    assert!(!sub_dir.join("c.txt").exists());

    fs::remove_dir_all(&temp_dir).unwrap();
}

// ==========================================
// 16. Blank Map Content Staging Reset Tests
// ==========================================

fn setup_synthetic_pools(data_dir: &Path, count: usize) {
    fs::create_dir_all(data_dir).unwrap();

    // 1. objects.dat (8 + count * 56)
    let obj_len = 8 + count * 56;
    let mut obj_bytes = vec![0u8; obj_len];
    obj_bytes[0..4].copy_from_slice(&1i32.to_le_bytes());
    obj_bytes[4..8].copy_from_slice(&(count as i32).to_le_bytes());
    // slot 1 has active name
    obj_bytes[8 + 30] = b'X';
    fs::write(data_dir.join("objects.dat"), obj_bytes).unwrap();

    // 2. objdata.dat (8 + count * 9)
    let objdata_len = 8 + count * 9;
    let mut objdata_bytes = vec![0u8; objdata_len];
    objdata_bytes[0..4].copy_from_slice(&1i32.to_le_bytes());
    objdata_bytes[4..8].copy_from_slice(&(count as i32).to_le_bytes());
    objdata_bytes[8 + 1] = 1; // active slot 1
    fs::write(data_dir.join("objdata.dat"), objdata_bytes).unwrap();

    // 3. position.dat (8 + count * 20)
    let pos_len = 8 + count * 20;
    let mut pos_bytes = vec![0u8; pos_len];
    pos_bytes[0..4].copy_from_slice(&1i32.to_le_bytes());
    pos_bytes[4..8].copy_from_slice(&(count as i32).to_le_bytes());
    pos_bytes[8 + 2] = 99; // some active position byte
    fs::write(data_dir.join("position.dat"), pos_bytes).unwrap();

    // 4. light.dat (8 + 1024 * 56 = 57352)
    let light_len = 8 + 1024 * 56;
    let mut light_bytes = vec![0u8; light_len];
    light_bytes[0..4].copy_from_slice(&1i32.to_le_bytes());
    light_bytes[4..8].copy_from_slice(&1024i32.to_le_bytes());
    light_bytes[8] = 1; // active light in slot 0
    fs::write(data_dir.join("light.dat"), light_bytes).unwrap();

    // 5. particle.dat (12 + 1024 * 432 + 4 * 1024 * 4 = 458764)
    let particle_len = 12 + 1024 * 432 + 16384;
    let mut particle_bytes = vec![0u8; particle_len];
    particle_bytes[0..4].copy_from_slice(&1i32.to_le_bytes());
    particle_bytes[4..8].copy_from_slice(&1024i32.to_le_bytes());
    particle_bytes[8..12].copy_from_slice(&4i32.to_le_bytes());
    fs::write(data_dir.join("particle.dat"), particle_bytes).unwrap();

    // 6. lager.dat (16 + 3200 * (43 + 2) = 144016)
    let lager_len = 16 + 3200 * 45;
    let mut lager_bytes = vec![0u8; lager_len];
    lager_bytes[0..4].copy_from_slice(&1i32.to_le_bytes());
    lager_bytes[4..8].copy_from_slice(&3200i32.to_le_bytes());
    lager_bytes[8..12].copy_from_slice(&6i32.to_le_bytes());
    lager_bytes[12..16].copy_from_slice(&10i32.to_le_bytes());
    fs::write(data_dir.join("lager.dat"), lager_bytes).unwrap();

    // 7. biglager.dat (12 + 800 * 1601 = 1280812)
    let biglager_len = 12 + 800 * 1601;
    let mut biglager_bytes = vec![0u8; biglager_len];
    biglager_bytes[0..4].copy_from_slice(&1i32.to_le_bytes());
    biglager_bytes[4..8].copy_from_slice(&800i32.to_le_bytes());
    biglager_bytes[8..12].copy_from_slice(&800i32.to_le_bytes());
    fs::write(data_dir.join("biglager.dat"), biglager_bytes).unwrap();

    // 8. way.dat (12 + count * 1030)
    let way_len = 12 + count * 1030;
    let mut way_bytes = vec![0u8; way_len];
    way_bytes[0..4].copy_from_slice(&1i32.to_le_bytes());
    way_bytes[4..8].copy_from_slice(&(count as i32).to_le_bytes());
    way_bytes[8..12].copy_from_slice(&256i32.to_le_bytes());
    way_bytes[14] = 42; // non-zero way byte
    fs::write(data_dir.join("way.dat"), way_bytes).unwrap();

    // 9. Runtime pools
    let runtime_specs: &[(&str, usize, usize, usize, &[i32])] = &[
        ("anim.dat", 8, 21, 4, &[]),
        ("gfxtype.dat", 8, 15, 2, &[]),
        ("action.dat", 12, 25, 0, &[6]),
        ("hirarchy.dat", 12, 103, 2, &[50]),
        ("formatio.dat", 8, 15, 10, &[]),
        ("explos.dat", 8, 46, 8, &[]),
        ("hitex.dat", 8, 28, 12, &[]),
        ("flash.dat", 12, 201, 32, &[16]),
    ];

    for &(name, header, record, col_sum, extra) in runtime_specs {
        let len = header + count * (record + col_sum);
        let mut bytes = vec![0u8; len];
        bytes[0..4].copy_from_slice(&1i32.to_le_bytes());
        bytes[4..8].copy_from_slice(&(count as i32).to_le_bytes());
        for (i, &ex) in extra.iter().enumerate() {
            bytes[8 + i * 4..8 + i * 4 + 4].copy_from_slice(&ex.to_le_bytes());
        }
        // Slot 0 is empty (0), slot 1 is active (1)
        bytes[header + record] = 1;
        fs::write(data_dir.join(name), bytes).unwrap();
    }
}

fn create_full_synthetic_map(map_dir: &Path) {
    fs::create_dir_all(map_dir).unwrap();
    setup_synthetic_pools(&map_dir.join("DATA"), 10);

    // boden.ini
    fs::write(
        map_dir.join("boden.ini"),
        "[Dimension]\n64\n\n[Waterlevel]\n30.0\n\n[Heightmapstep]\n2.0\n",
    )
    .unwrap();

    // boden.txt
    let mut boden_txt = String::from("[Dimension]\n64\n\n[Texturen]\n");
    for _ in 0..4096 {
        boden_txt.push_str("GRAS_1\n");
    }
    fs::write(map_dir.join("boden.txt"), boden_txt).unwrap();

    // Terrain BMPs
    fs::write(map_dir.join("boden.bmp"), create_solid_bmp(257, 257, 50)).unwrap();
    fs::write(map_dir.join("vertex.bmp"), create_solid_bmp(257, 257, 255)).unwrap();
    fs::write(map_dir.join("smooth.bmp"), create_solid_bmp(257, 257, 0)).unwrap();
    fs::write(map_dir.join("emboss.bmp"), create_solid_bmp(257, 257, 0)).unwrap();
    fs::write(map_dir.join("collision.bmp"), create_solid_bmp(256, 256, 0)).unwrap();
    fs::write(map_dir.join("minimap.bmp"), create_solid_bmp(256, 256, 128)).unwrap();

    // SCRIPT
    let scripts = map_dir.join("SCRIPT");
    fs::create_dir_all(&scripts).unwrap();
    fs::write(scripts.join("ak_level.bci"), against_rome_scenario::bci::BciImage::idle().serialize()).unwrap();
    fs::write(scripts.join("old_dummy.bci"), b"dummy").unwrap();

    // TEXT
    let us_dir = map_dir.join("TEXT").join("US");
    fs::create_dir_all(&us_dir).unwrap();
    fs::write(us_dir.join("briefing.put"), "var:briefing_titel_1 =\"Original Title\";\r\n").unwrap();

    // SDL settlement
    fs::write(
        map_dir.join("Endlos_01_Siedlung01.sdl"),
        "[settlement]\r\nrefpos=100.0,20.0,100.0\r\n\r\n[object0000]\r\ndef=100\r\npos=10.0,0.0,10.0\r\n",
    )
    .unwrap();
}

#[test]
fn test_blank_map_content_staging_reset_and_rejections() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_blank_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&temp_dir).unwrap();

    // Non-staging directory rejection (not ending with .tmp_arm)
    let non_staging = temp_dir.join("MAPS").join("ENDL_005");
    fs::create_dir_all(&non_staging).unwrap();
    assert!(BlankMapContent::reset_staging_directory(&non_staging).is_err());

    // Proper staging directory
    let staging = temp_dir.join("MAPS").join("ENDL_005.tmp_arm");
    create_full_synthetic_map(&staging);

    // Call reset
    BlankMapContent::reset_staging_directory(&staging).unwrap();

    // 1. Verify positions cleared from byte 8
    let pos = fs::read(staging.join("DATA").join("position.dat")).unwrap();
    assert!(pos[8..].iter().all(|&b| b == 0));

    // 2. Verify ways cleared from byte 12
    let way = fs::read(staging.join("DATA").join("way.dat")).unwrap();
    assert!(way[12..].iter().all(|&b| b == 0));

    // 3. Verify lights cleared (active=0, owner/parent/def = -1)
    let light = fs::read(staging.join("DATA").join("light.dat")).unwrap();
    assert_eq!(light[8], 0);

    // 4. Verify scripts cleaned up
    let scripts_dir = staging.join("SCRIPT");
    assert!(!scripts_dir.join("old_dummy.bci").exists());
    assert!(scripts_dir.join("ak_level.bci").is_file());
    assert!(scripts_dir.join("ak_level.arm_original").is_file());
    assert_eq!(
        fs::read(scripts_dir.join("ak_level.bci")).unwrap(),
        fs::read(scripts_dir.join("ak_level.arm_original")).unwrap()
    );

    // 5. Verify scenario initialized
    assert!(staging.join("arm_scenario.json").is_file());

    // 6. Verify SDL settlement kept intact
    assert!(staging.join("Endlos_01_Siedlung01.sdl").is_file());

    fs::remove_dir_all(&temp_dir).unwrap();
}

// ==========================================
// 17. BlankMapBuilder End-to-End Creation Tests
// ==========================================

#[test]
fn test_blank_map_builder_create_and_describe() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_builder_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&temp_dir).unwrap();

    let maps_dir = temp_dir.join("MAPS");
    fs::create_dir_all(&maps_dir).unwrap();

    // Create source template map ENDL_000
    let source_map = maps_dir.join("ENDL_000");
    create_full_synthetic_map(&source_map);

    // Add a height-dependent cache file to source
    fs::write(source_map.join("skydens.dat"), b"cache bytes").unwrap();

    // Describe
    let desc_en = BlankMapBuilder::describe("ENDL_000", true);
    assert!(desc_en.contains("Source: ENDL_000"));
    assert!(desc_en.contains("Cleared: all native DATA objects"));

    let desc_zh = BlankMapBuilder::describe("ENDL_000", false);
    assert!(desc_zh.contains("來源：ENDL_000"));
    assert!(desc_zh.contains("清除：所有原生 DATA 物件"));

    // Create blank map in slot 5
    let created = BlankMapBuilder::create(&temp_dir, "ENDL_000", 5, "My Blank Scene").unwrap();
    assert_eq!(created.slot, 5);
    assert_eq!(created.id(), "ENDL_005");
    assert!(created.is_custom);

    let map_dir = maps_dir.join("ENDL_005");
    assert!(map_dir.is_dir());

    // Verify cache files deleted
    assert!(!map_dir.join("skydens.dat").exists());

    // Verify terrain layers are flat and match expected uniform fill
    // Water level = 30.0, step = 2.0 -> height = round(30.0 / 2.0) + 20 = 15 + 20 = 35
    let boden_bmp = fs::read(map_dir.join("boden.bmp")).unwrap();
    assert_eq!(boden_bmp[54], 35); // pixel value

    let vertex_bmp = fs::read(map_dir.join("vertex.bmp")).unwrap();
    assert_eq!(vertex_bmp[54], 255);

    let smooth_bmp = fs::read(map_dir.join("smooth.bmp")).unwrap();
    assert_eq!(smooth_bmp[54], 0);

    let emboss_bmp = fs::read(map_dir.join("emboss.bmp")).unwrap();
    assert_eq!(emboss_bmp[54], 0);

    let collision_bmp = fs::read(map_dir.join("collision.bmp")).unwrap();
    assert_eq!(collision_bmp[54], 0);

    let minimap_bmp = fs::read(map_dir.join("minimap.bmp")).unwrap();
    assert_eq!(minimap_bmp.len(), 54 + 256 * 256 * 3);

    // Verify briefing title updated
    let put_doc = PutTextDocument::load(&map_dir.join("TEXT").join("US").join("briefing.put")).unwrap();
    assert_eq!(put_doc.get_value("briefing_titel_1").unwrap(), "My Blank Scene");

    // Verify manifest registered with standalone_level = true
    let manifest = CustomMapManifest::load(&temp_dir).unwrap();
    let entry = manifest.entries().iter().find(|e| e.slot == 5).unwrap();
    assert!(entry.standalone_level);

    // Verify source map is untouched!
    assert!(source_map.join("skydens.dat").exists());
    let src_put = PutTextDocument::load(&source_map.join("TEXT").join("US").join("briefing.put")).unwrap();
    assert_eq!(src_put.get_value("briefing_titel_1").unwrap(), "Original Title");

    fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_blank_map_builder_water_too_high_rejected() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_high_water_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&temp_dir).unwrap();

    let maps_dir = temp_dir.join("MAPS");
    let staging = maps_dir.join("ENDL_005.tmp_arm");
    create_full_synthetic_map(&staging);

    // Set waterlevel ridiculously high so height exceeds 255
    fs::write(
        staging.join("boden.ini"),
        "[Dimension]\n64\n\n[Waterlevel]\n1000.0\n\n[Heightmapstep]\n1.0\n",
    )
    .unwrap();

    let err = BlankMapBuilder::prepare(&temp_dir, &staging).unwrap_err();
    assert!(err.contains("水面太高"));

    fs::remove_dir_all(&temp_dir).unwrap();
}

// ==========================================
// 18. BlankMapStarterKit Layout & Guards Tests
// ==========================================

#[test]
fn test_blank_map_starter_kit_layout_and_guards() {
    // Valid creation at center (8000.0, 8000.0)
    let kit = BlankMapStarterKit::create(2, 8000.0, 8000.0, None, None).unwrap();
    assert_eq!(kit.spawns.len(), 5);
    assert_eq!(kit.native_object_count, 5);

    // Check main house
    let house = &kit.spawns[0];
    assert_eq!(house.alias, BlankMapStarterKit::MAIN_HOUSE_ALIAS);
    assert_eq!(house.x, 8000.0);
    assert_eq!(house.z, 8000.0);
    assert_eq!(house.team, 2);
    assert!(house.prebuilt);

    // Check 4 units relative offsets: (-480, 800), (-160, 800), (160, 800), (480, 800)
    assert_eq!(kit.spawns[1].x, 8000.0 - 480.0);
    assert_eq!(kit.spawns[1].z, 8000.0 + 800.0);
    assert_eq!(kit.spawns[2].x, 8000.0 - 160.0);
    assert_eq!(kit.spawns[3].x, 8000.0 + 160.0);
    assert_eq!(kit.spawns[4].x, 8000.0 + 480.0);

    // Apply to scenario document
    let mut scenario = ScenarioDocument::default();
    kit.apply_to_scenario(&mut scenario).unwrap();
    assert_eq!(scenario.spawns.len(), 5);

    // Applying twice should be rejected
    assert!(kit.apply_to_scenario(&mut scenario).is_err());

    // Test guard: invalid team (< 0 or > 7)
    assert!(BlankMapStarterKit::create(-1, 8000.0, 8000.0, None, None).is_err());
    assert!(BlankMapStarterKit::create(8, 8000.0, 8000.0, None, None).is_err());

    // Test guard: out of bounds coordinates
    assert!(BlankMapStarterKit::create(0, -1.0, 8000.0, None, None).is_err());
    assert!(BlankMapStarterKit::create(0, 16384.0, 8000.0, None, None).is_err());
    assert!(BlankMapStarterKit::create(0, f32::NAN, 8000.0, None, None).is_err());
}

// ==========================================
// 19. Injected Failure Rollback Tests
// ==========================================

#[test]
fn test_cloner_prepare_failure_rolls_back_cleanly() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_fail_rb_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&temp_dir).unwrap();

    let maps_dir = temp_dir.join("MAPS");
    fs::create_dir_all(&maps_dir).unwrap();

    let source_map = maps_dir.join("ENDL_000");
    create_full_synthetic_map(&source_map);

    let cloner = EndlessMapCloner::new();
    let res = cloner.clone_prepared(
        &temp_dir,
        "ENDL_000",
        6,
        "Will Fail",
        |_staging| Err("Injected preparation error".to_string()),
        Some(true),
    );

    assert!(res.is_err());
    assert!(!maps_dir.join("ENDL_006").exists());
    assert!(!maps_dir.join("ENDL_006.tmp_arm").exists());

    // Manifest should have no entry for slot 6
    let manifest = CustomMapManifest::load(&temp_dir).unwrap();
    assert!(!manifest.entries().iter().any(|e| e.slot == 6));

    fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_parse_endl_slot_rejects_non_ascii_and_non_digits() {
    assert_eq!(parse_endl_slot("ENDL_005"), Some(5));
    assert_eq!(parse_endl_slot("endl_012"), Some(12));
    assert_eq!(parse_endl_slot("ENDLé12"), None); // byte 5 is inside a multi-byte char
    assert_eq!(parse_endl_slot("ENDL_+12"), None);
    assert_eq!(parse_endl_slot("ENDL_-12"), None);
}

#[test]
fn test_cloner_rejects_control_characters_in_name() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_ctrl_name_{}", Uuid::new_v4().simple()));
    let maps_dir = temp_dir.join("MAPS");
    fs::create_dir_all(&maps_dir).unwrap();
    create_full_synthetic_map(&maps_dir.join("ENDL_000"));

    let cloner = EndlessMapCloner::new();
    for name in ["Bad\nName", "Bad\rName", "Bad\tName", "Bad\0Name", "Bad\u{7f}Name"] {
        let res = cloner.clone_prepared(&temp_dir, "ENDL_000", 6, name, |_| Ok(()), Some(true));
        assert!(res.is_err(), "{name:?} should be rejected");
        assert!(!maps_dir.join("ENDL_006").exists());
    }

    fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_deleter_refuses_retail_slots_and_rolls_back_on_error() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_del_rb_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&temp_dir).unwrap();

    let maps_dir = temp_dir.join("MAPS");
    fs::create_dir_all(&maps_dir).unwrap();

    // Retail slots 0..5 must be refused
    let deleter = EndlessMapDeleter::new();
    for slot in 0..5 {
        assert!(deleter.delete(&temp_dir, slot).is_err());
    }

    fs::remove_dir_all(&temp_dir).unwrap();
}

// ==========================================
// 20. BlankMapSpec Validation Guards Tests
// ==========================================

#[test]
fn test_blank_map_spec_validation_guards() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_spec_val_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&temp_dir).unwrap();

    let maps_dir = temp_dir.join("MAPS");
    fs::create_dir_all(&maps_dir).unwrap();

    let source_map = maps_dir.join("ENDL_000");
    create_full_synthetic_map(&source_map);

    // 1. Invalid target slot (< 5 or > 999)
    let spec_low = BlankMapSpec::new("ENDL_000", 4, "Too Low");
    assert!(spec_low.validate(&temp_dir).is_err());
    let spec_high = BlankMapSpec::new("ENDL_000", 1000, "Too High");
    assert!(spec_high.validate(&temp_dir).is_err());

    // 2. Empty or invalid name
    let spec_empty = BlankMapSpec::new("ENDL_000", 5, "   ");
    assert!(spec_empty.validate(&temp_dir).is_err());
    let spec_bad_char = BlankMapSpec::new("ENDL_000", 5, "Bad/Name");
    assert!(spec_bad_char.validate(&temp_dir).is_err());
    let spec_chinese = BlankMapSpec::new("ENDL_000", 5, "自製地圖");
    assert!(spec_chinese.validate(&temp_dir).is_err());

    // 3. Non-existent source map
    let spec_no_src = BlankMapSpec::new("ENDL_NONEXISTENT", 5, "Valid Name");
    assert!(spec_no_src.validate(&temp_dir).is_err());

    // 4. Target slot collision or temp folder collision
    let dest_exists = maps_dir.join("ENDL_005");
    fs::create_dir_all(&dest_exists).unwrap();
    let spec_dest = BlankMapSpec::new("ENDL_000", 5, "Valid Name");
    assert!(spec_dest.validate(&temp_dir).is_err());
    fs::remove_dir_all(&dest_exists).unwrap();

    let temp_exists = maps_dir.join("ENDL_005.tmp_arm");
    fs::create_dir_all(&temp_exists).unwrap();
    assert!(spec_dest.validate(&temp_dir).is_err());
    fs::remove_dir_all(&temp_exists).unwrap();

    // 5. Starter kit team out of range (0..=7)
    let spec_bad_team = BlankMapSpec::new("ENDL_000", 5, "Valid Name")
        .with_starter_kit(8, 8000.0, 8000.0);
    assert!(spec_bad_team.validate(&temp_dir).is_err());

    // 6. Starter kit center out of bounds or boundary overflow
    let spec_out_bound = BlankMapSpec::new("ENDL_000", 5, "Valid Name")
        .with_starter_kit(0, 16300.0, 16000.0); // +800 on Z will exceed 16383
    assert!(spec_out_bound.validate(&temp_dir).is_err());

    // 7. Briefing text with invalid characters (NUL or newline)
    let spec_nul_brief = BlankMapSpec::new("ENDL_000", 5, "Valid Name")
        .with_briefing("Bad\0Text");
    assert!(spec_nul_brief.validate(&temp_dir).is_err());
    let spec_newline_brief = BlankMapSpec::new("ENDL_000", 5, "Valid Name")
        .with_briefing("Bad\nText");
    assert!(spec_newline_brief.validate(&temp_dir).is_err());

    // 8. Fully valid spec passes validation
    let valid_spec = BlankMapSpec::new("ENDL_000", 5, "Valid Blank Map")
        .with_starter_kit(1, 8000.0, 8000.0)
        .with_height_override(45)
        .with_briefing("Custom Briefing Text");
    assert!(valid_spec.validate(&temp_dir).is_ok());

    fs::remove_dir_all(&temp_dir).unwrap();
}

// ==========================================
// 21. Build With Spec and Disk Verification Loop Tests
// ==========================================

#[test]
fn test_blank_map_builder_build_with_spec_and_verify_disk_artifacts() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_build_spec_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&temp_dir).unwrap();

    let maps_dir = temp_dir.join("MAPS");
    fs::create_dir_all(&maps_dir).unwrap();

    let source_map = maps_dir.join("ENDL_000");
    create_full_synthetic_map(&source_map);
    fs::write(source_map.join("skydens.dat"), b"cache bytes").unwrap();

    // Build with Starter Kit and custom briefing
    let spec = BlankMapSpec::new("ENDL_000", 7, "Builder Loop Scene")
        .with_starter_kit(3, 7000.0, 7000.0)
        .with_height_override(42)
        .with_briefing("Battle of the Frontier");

    let result = BlankMapBuilder::build_with_spec(&temp_dir, &spec).unwrap();
    assert_eq!(result.info.slot, 7);
    assert_eq!(result.info.id(), "ENDL_007");
    assert_eq!(result.applied_height, 42);
    assert!(result.starter_kit_applied);
    assert_eq!(result.starter_spawns_count, 5);
    assert_eq!(result.map_path, maps_dir.join("ENDL_007"));

    // Verify disk artifacts via dedicated verification method
    let summary = result.verify_disk_artifacts(&temp_dir).unwrap();
    assert_eq!(summary.slot, 7);
    assert_eq!(summary.map_id, "ENDL_007");
    assert!(summary.has_manifest_entry);
    assert_eq!(summary.boden_bmp_dim, (257, 257));
    assert_eq!(summary.boden_height, 42);
    assert_eq!(summary.collision_bmp_dim, (256, 256));
    assert_eq!(summary.minimap_bmp_dim, (256, 256));
    assert_eq!(summary.scenario_spawns_count, 5);
    assert!(summary.has_idle_level_script);
    assert!(summary.has_custom_marker);

    // Direct assertions on disk contents
    let target_dir = &result.map_path;

    // 1. Verify height-dependent cache was deleted on target
    assert!(!target_dir.join("skydens.dat").exists());
    // 2. Source map was not touched
    assert!(source_map.join("skydens.dat").exists());

    // 3. Verify scenario placements
    let scenario_raw = fs::read_to_string(target_dir.join("arm_scenario.json")).unwrap();
    let scenario: ScenarioDocument = serde_json::from_str(&scenario_raw).unwrap();
    assert_eq!(scenario.spawns.len(), 5);
    assert_eq!(scenario.spawns[0].alias, BlankMapStarterKit::MAIN_HOUSE_ALIAS);
    assert_eq!(scenario.spawns[0].team, 3);
    assert_eq!(scenario.spawns[0].x, 7000.0);
    assert_eq!(scenario.spawns[0].z, 7000.0);
    // Height step in boden.ini is 2.0, so y = 42.0 * 2.0 = 84.0
    assert_eq!(scenario.spawns[0].y, 84.0);

    for s in &scenario.spawns[1..] {
        assert_eq!(s.alias, BlankMapStarterKit::UNIT_ALIAS);
        assert_eq!(s.team, 3);
        assert_eq!(s.y, 84.0);
    }

    // 4. Verify briefing title updated
    let put_doc = PutTextDocument::load(&target_dir.join("TEXT").join("US").join("briefing.put")).unwrap();
    assert_eq!(put_doc.get_value("briefing_titel_1").unwrap(), "Battle of the Frontier");

    fs::remove_dir_all(&temp_dir).unwrap();
}

// ==========================================
// 22. Plain Spec Auto-Height Build Tests
// ==========================================

#[test]
fn test_blank_map_builder_plain_spec_auto_height_and_disk_verification() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_plain_spec_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&temp_dir).unwrap();

    let maps_dir = temp_dir.join("MAPS");
    fs::create_dir_all(&maps_dir).unwrap();

    let source_map = maps_dir.join("ENDL_000");
    create_full_synthetic_map(&source_map);

    // Spec without starter kit or height override
    let spec = BlankMapSpec::new("ENDL_000", 9, "Plain Blank Map");
    let result = BlankMapBuilder::build_with_spec(&temp_dir, &spec).unwrap();

    assert_eq!(result.info.slot, 9);
    // Water level = 30.0, step = 2.0 -> (30.0 / 2.0).round() + 20 = 15 + 20 = 35
    assert_eq!(result.applied_height, 35);
    assert!(!result.starter_kit_applied);
    assert_eq!(result.starter_spawns_count, 0);

    let summary = result.verify_disk_artifacts(&temp_dir).unwrap();
    assert_eq!(summary.boden_height, 35);
    assert_eq!(summary.scenario_spawns_count, 0);

    fs::remove_dir_all(&temp_dir).unwrap();
}

// ==========================================
// 23. Failure Rollback and Disk Cleanliness Tests
// ==========================================

#[test]
fn test_blank_map_builder_failure_rollback_preserves_disk_cleanliness() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_build_fail_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&temp_dir).unwrap();

    let maps_dir = temp_dir.join("MAPS");
    fs::create_dir_all(&maps_dir).unwrap();

    let source_map = maps_dir.join("ENDL_000");
    create_full_synthetic_map(&source_map);

    // 1. Validation failure: target slot out of range -> zero disk activity
    let invalid_spec = BlankMapSpec::new("ENDL_000", 2, "Retail Slot");
    let res = BlankMapBuilder::build_with_spec(&temp_dir, &invalid_spec);
    assert!(res.is_err());
    assert!(!maps_dir.join("ENDL_002").exists());
    assert!(!maps_dir.join("ENDL_002.tmp_arm").exists());

    // 2. Preparation failure: Corrupt boden.ini in staging causes rollback
    // To trigger preparation failure through spec, set height override lower than waterlevel
    // Waterlevel = 30.0, step = 2.0, so height 10 has y = 20.0 <= 30.0 (underwater!)
    let underwater_spec = BlankMapSpec::new("ENDL_000", 11, "Underwater Map")
        .with_height_override(10);

    let err = BlankMapBuilder::build_with_spec(&temp_dir, &underwater_spec).unwrap_err();
    assert!(err.contains("不高於水面"));

    // Staging and target must be cleanly removed
    assert!(!maps_dir.join("ENDL_011").exists());
    assert!(!maps_dir.join("ENDL_011.tmp_arm").exists());

    // Manifest must remain untouched
    let manifest = CustomMapManifest::load(&temp_dir).unwrap();
    assert!(!manifest.entries().iter().any(|e| e.slot == 11));

    // Source map must be completely untouched
    let src_put = PutTextDocument::load(&source_map.join("TEXT").join("US").join("briefing.put")).unwrap();
    assert_eq!(src_put.get_value("briefing_titel_1").unwrap(), "Original Title");

    fs::remove_dir_all(&temp_dir).unwrap();
}

// ==========================================
// 24. FileRollbackScope Enhanced Testability Tests
// ==========================================

#[test]
fn test_file_rollback_scope_inspection_and_uncommitted_drop() {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_rb_inspect_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&temp_dir).unwrap();

    let existing_file = temp_dir.join("keep.txt");
    fs::write(&existing_file, b"Original Content").unwrap();

    let created_file = temp_dir.join("new.txt");

    {
        let mut scope = FileRollbackScope::new().unwrap();
        assert!(!scope.is_committed());
        assert!(!scope.is_restored());
        assert!(scope.root_path().exists());

        // Track existing file before modification
        scope.track_file(&existing_file).unwrap();
        assert_eq!(scope.tracked_file_count(), 1);
        assert!(scope.has_tracked_file(&existing_file));

        // Modify existing file
        fs::write(&existing_file, b"Modified Content").unwrap();

        // Track new file before creation
        scope.track_file(&created_file).unwrap();
        assert_eq!(scope.tracked_file_count(), 2);
        assert!(scope.has_tracked_file(&created_file));
        fs::write(&created_file, b"Newly Created").unwrap();

        // Scope drops here WITHOUT commit -> automatic rollback!
    }

    // Existing file restored
    let restored_bytes = fs::read(&existing_file).unwrap();
    assert_eq!(restored_bytes, b"Original Content");

    // Newly created file removed
    assert!(!created_file.exists());

    fs::remove_dir_all(&temp_dir).unwrap();
}

// ==========================================
// 25. Readonly handling and atomic replace (C# SafeFileWriter parity)
// ==========================================

fn set_readonly(path: &Path, readonly: bool) {
    let mut perms = fs::metadata(path).unwrap().permissions();
    perms.set_readonly(readonly);
    fs::set_permissions(path, perms).unwrap();
}

#[test]
fn test_safe_writer_replaces_and_deletes_readonly_files_and_rolls_back() {
    let dir = std::env::temp_dir().join(format!("arm_test_ro_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&dir).unwrap();
    let target = dir.join("ro.txt");
    let doomed = dir.join("ro_delete.txt");
    fs::write(&target, b"original").unwrap();
    fs::write(&doomed, b"keep me").unwrap();
    set_readonly(&target, true);
    set_readonly(&doomed, true);

    {
        let mut scope = FileRollbackScope::new().unwrap();
        SafeFileWriter::write_all_bytes(&target, b"updated", Some(&mut scope)).unwrap();
        SafeFileWriter::delete_file(&doomed, Some(&mut scope)).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"updated");
        assert!(!doomed.exists());
        // dropped without commit -> restore, even though files were read-only
    }
    assert_eq!(fs::read(&target).unwrap(), b"original");
    assert_eq!(fs::read(&doomed).unwrap(), b"keep me");

    // No temp leftovers
    let leftovers: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty());

    set_readonly(&target, false);
    set_readonly(&doomed, false);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_rollback_restore_directory_with_readonly_file() {
    let dir = std::env::temp_dir().join(format!("arm_test_rodir_{}", Uuid::new_v4().simple()));
    let sub = dir.join("sub");
    fs::create_dir_all(&sub).unwrap();
    let file = sub.join("a.txt");
    fs::write(&file, b"orig").unwrap();

    {
        let mut scope = FileRollbackScope::new().unwrap();
        scope.track_directory(&sub).unwrap();
        fs::write(&file, b"changed").unwrap();
        fs::write(sub.join("extra.txt"), b"x").unwrap();
        set_readonly(&file, true);
        scope.restore_directory(&sub).unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"orig");
        assert!(!sub.join("extra.txt").exists());
        scope.commit();
    }

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_rollback_restore_directory_untracked_errors() {
    let dir = std::env::temp_dir().join(format!("arm_test_untr_{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&dir).unwrap();
    let mut scope = FileRollbackScope::new().unwrap();
    assert!(scope.restore_directory(&dir).is_err());
    assert!(scope.track_directory(&dir.join("missing")).is_err());
    scope.commit();
    fs::remove_dir_all(&dir).unwrap();
}

fn assert_clone_fails_closed_on_corrupt(rel_file: &str) {
    let temp_dir = std::env::temp_dir().join(format!("arm_test_corrupt_clone_{}", Uuid::new_v4().simple()));
    let maps_dir = temp_dir.join("MAPS");
    fs::create_dir_all(&maps_dir).unwrap();
    let source = maps_dir.join("ENDL_000");
    create_full_synthetic_map(&source);

    // PFIL magic with a garbage body: recognised as compressed but cannot be decompressed.
    let mut corrupt = b"PFIL".to_vec();
    corrupt.extend(std::iter::repeat(0xFFu8).take(80));
    let corrupt_path = source.join(rel_file);
    fs::write(&corrupt_path, &corrupt).unwrap();

    let res = EndlessMapCloner::new().clone_prepared(&temp_dir, "ENDL_000", 6, "New Name", |_| Ok(()), Some(true));
    assert!(res.is_err(), "{rel_file}: clone must fail closed");
    assert!(!maps_dir.join("ENDL_006").exists());
    assert!(!maps_dir.join("ENDL_006.tmp_arm").exists());
    assert_eq!(fs::read(&corrupt_path).unwrap(), corrupt, "source must be untouched");
    let manifest = CustomMapManifest::load(&temp_dir).unwrap();
    assert!(!manifest.entries().iter().any(|e| e.slot == 6));

    fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_cloner_corrupt_briefing_fails_closed_without_residue() {
    assert_clone_fails_closed_on_corrupt("TEXT/US/briefing.put");
}

#[test]
fn test_cloner_corrupt_sdl_fails_closed_without_residue() {
    assert_clone_fails_closed_on_corrupt("Endlos_01_Siedlung01.sdl");
}
