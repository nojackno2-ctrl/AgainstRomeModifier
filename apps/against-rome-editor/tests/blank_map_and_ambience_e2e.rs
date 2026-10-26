//! End-to-end integration tests verifying:
//! 1. BlankMapBuilder entry point in the Rust EditorApp new map creation flow.
//! 2. AmbienceUi native ambience & scene models connection.
//! 3. Safe disk persistence of boden.ini, soundscape.sndz, and briefing PUT scripts.
//! 4. Full roundtrip reload into a clean editor session.

use against_rome_ambience::{
    atmosphere::WeatherPresets,
    soundscape::{AudioCategory, SoundscapeShapeType},
};
use against_rome_editor_app::{
    app::{EditorApp, PendingAction},
    bmp::BmpImage,
    storage::load_map,
    terrain::SampleChange,
};
use against_rome_maps::{
    blank_builder::BlankMapSpec,
    custom_map::CustomMapManifest,
};
use eframe::egui::{self, Rect};
use std::{
    fs,
    path::Path,
};
use uuid::Uuid;

fn setup_synthetic_pools(data_dir: &Path, count: usize) {
    fs::create_dir_all(data_dir).unwrap();

    // 1. objects.dat (8 + count * 56)
    let obj_len = 8 + count * 56;
    let mut obj_bytes = vec![0u8; obj_len];
    obj_bytes[0..4].copy_from_slice(&1i32.to_le_bytes());
    obj_bytes[4..8].copy_from_slice(&(count as i32).to_le_bytes());
    obj_bytes[8 + 30] = b'X';
    fs::write(data_dir.join("objects.dat"), obj_bytes).unwrap();

    // 2. objdata.dat (8 + count * 9)
    let objdata_len = 8 + count * 9;
    let mut objdata_bytes = vec![0u8; objdata_len];
    objdata_bytes[0..4].copy_from_slice(&1i32.to_le_bytes());
    objdata_bytes[4..8].copy_from_slice(&(count as i32).to_le_bytes());
    objdata_bytes[8 + 1] = 1;
    fs::write(data_dir.join("objdata.dat"), objdata_bytes).unwrap();

    // 3. position.dat：8 位元組標頭與每槽 17 位元組。
    let pos_len = 8 + count * 17;
    let mut pos_bytes = vec![0u8; pos_len];
    pos_bytes[0..4].copy_from_slice(&1i32.to_le_bytes());
    pos_bytes[4..8].copy_from_slice(&(count as i32).to_le_bytes());
    pos_bytes[8 + 2] = 99;
    fs::write(data_dir.join("position.dat"), pos_bytes).unwrap();

    // 4. light.dat (8 + 1024 * 56 = 57352)
    let light_len = 8 + 1024 * 56;
    let mut light_bytes = vec![0u8; light_len];
    light_bytes[0..4].copy_from_slice(&1i32.to_le_bytes());
    light_bytes[4..8].copy_from_slice(&1024i32.to_le_bytes());
    light_bytes[8] = 1;
    fs::write(data_dir.join("light.dat"), light_bytes).unwrap();

    // 5. particle.dat (12 + 1024 * 432 + 16384)
    let particle_len = 12 + 1024 * 432 + 16384;
    let mut particle_bytes = vec![0u8; particle_len];
    particle_bytes[0..4].copy_from_slice(&1i32.to_le_bytes());
    particle_bytes[4..8].copy_from_slice(&1024i32.to_le_bytes());
    particle_bytes[8..12].copy_from_slice(&4i32.to_le_bytes());
    fs::write(data_dir.join("particle.dat"), particle_bytes).unwrap();

    // 6. lager.dat (16 + 3200 * 45)
    let lager_len = 16 + 3200 * 45;
    let mut lager_bytes = vec![0u8; lager_len];
    lager_bytes[0..4].copy_from_slice(&1i32.to_le_bytes());
    lager_bytes[4..8].copy_from_slice(&3200i32.to_le_bytes());
    lager_bytes[8..12].copy_from_slice(&6i32.to_le_bytes());
    lager_bytes[12..16].copy_from_slice(&10i32.to_le_bytes());
    fs::write(data_dir.join("lager.dat"), lager_bytes).unwrap();

    // 7. biglager.dat (12 + 800 * 1601)
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
    way_bytes[14] = 42;
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
        bytes[header + record] = 1;
        fs::write(data_dir.join(name), bytes).unwrap();
    }
}

fn setup_synthetic_game_root(root: &Path) {
    let maps_dir = root.join("MAPS");
    let template_dir = maps_dir.join("ENDL_000");
    fs::create_dir_all(&template_dir).unwrap();

    setup_synthetic_pools(&template_dir.join("DATA"), 10);

    // boden.ini
    fs::write(
        template_dir.join("boden.ini"),
        "[Dimension]\n64\n\n[Waterlevel]\n30.0\n\n[Heightmapstep]\n2.0\n\n[WaterBumpAmplitude]\n256\n\n[WaterColor]\n0xffdfbf\n",
    )
    .unwrap();

    // boden.txt
    let mut boden_txt = String::from("[Dimension]\n64\n\n[Texturen]\n");
    for _ in 0..4096 {
        boden_txt.push_str("GRAS_1\n");
    }
    fs::write(template_dir.join("boden.txt"), boden_txt).unwrap();

    // Terrain BMPs
    fs::write(
        template_dir.join("boden.bmp"),
        BmpImage::new_solid(257, 257, 0, 50, 0).encode_bmp(),
    )
    .unwrap();
    fs::write(
        template_dir.join("vertex.bmp"),
        BmpImage::new_solid(257, 257, 255, 255, 255).encode_bmp(),
    )
    .unwrap();
    fs::write(
        template_dir.join("smooth.bmp"),
        BmpImage::new_solid(257, 257, 0, 0, 0).encode_bmp(),
    )
    .unwrap();
    fs::write(
        template_dir.join("emboss.bmp"),
        BmpImage::new_solid(257, 257, 0, 0, 0).encode_bmp(),
    )
    .unwrap();
    fs::write(
        template_dir.join("collision.bmp"),
        BmpImage::new_solid(256, 256, 0, 0, 0).encode_bmp(),
    )
    .unwrap();
    fs::write(
        template_dir.join("minimap.bmp"),
        BmpImage::new_solid(256, 256, 128, 128, 128).encode_bmp(),
    )
    .unwrap();

    // 24x6 day/night lighting BMP
    fs::write(
        template_dir.join("daynight.bmp"),
        BmpImage::new_solid(24, 6, 180, 180, 200).encode_bmp(),
    )
    .unwrap();

    // SCRIPT ak_level.bci
    let scripts = template_dir.join("SCRIPT");
    fs::create_dir_all(&scripts).unwrap();
    fs::write(
        scripts.join("ak_level.bci"),
        against_rome_scenario::bci::BciImage::idle().serialize(),
    )
    .unwrap();

    // TEXT
    let us_dir = template_dir.join("TEXT").join("US");
    fs::create_dir_all(&us_dir).unwrap();
    fs::write(
        us_dir.join("briefing.put"),
        "var:briefing_titel_1 =\"Original Title\";\r\n",
    )
    .unwrap();

    // SDL settlement
    fs::write(
        template_dir.join("Endlos_01_Siedlung01.sdl"),
        "[settlement]\r\nrefpos=100.0,20.0,100.0\r\n\r\n[object0000]\r\ndef=100\r\npos=10.0,0.0,10.0\r\n",
    )
    .unwrap();
}

fn disk_snapshot(root: &Path) -> std::collections::BTreeMap<std::path::PathBuf, Option<Vec<u8>>> {
    fn visit(root: &Path, dir: &Path,
        entries: &mut std::collections::BTreeMap<std::path::PathBuf, Option<Vec<u8>>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let is_dir = path.is_dir();
            entries.insert(path.strip_prefix(root).unwrap().to_path_buf(),
                if is_dir { None } else { Some(fs::read(&path).unwrap()) });
            if is_dir { visit(root, &path, entries); }
        }
    }
    let mut entries = std::collections::BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

#[test]
fn public_map_switch_rejection_preserves_valid_game_and_build_outputs() {

    let temp_root = std::env::temp_dir().join(format!("arm_e2e_switch_guard_{}", Uuid::new_v4()));
    setup_synthetic_game_root(&temp_root);
    fs::write(temp_root.join("existing-output.txt"), b"preserve existing output").unwrap();
    let source = temp_root.join("MAPS/ENDL_000");
    let spec = BlankMapSpec::new("ENDL_000", 7, "Guard Regression")
        .with_height_override(42).with_briefing("Preserve existing maps.");
    spec.validate(&temp_root).expect("valid builder inputs");

    for dirty_kind in 0..3 {
        let mut app = EditorApp::initial_state();
        app.set_loaded_map(source.clone(), load_map(&source).unwrap()).unwrap();
        match dirty_kind {
            0 => { app.session.as_mut().unwrap().apply_height_changes(&[SampleChange {
                index: 100, before: 50, after: 95,
            }]); }
            1 => app.ambience.load_preset(WeatherPresets::STORM_ID),
            2 => app.maps.mark_dirty(),
            _ => unreachable!(),
        }
        let before_disk = disk_snapshot(&temp_root);
        let session = app.session.as_ref().unwrap();
        let before_session = (session as *const _, session.heights().to_vec(),
            session.collision().to_vec(), session.is_dirty(), session.can_undo(), session.can_redo());
        let before_app = (app.map_path_input.clone(), app.status_message.clone(), app.status_is_error,
            app.water_surface, app.water_bed, app.ambience.is_dirty(), app.maps.is_dirty(),
            serde_json::to_vec(&app.ambience.profile).unwrap());

        for build in [false, true] {
            let err = if build {
                app.create_blank_map(&temp_root, &spec).err().expect("dirty build rejected")
            } else {
                app.set_loaded_map(temp_root.join("MAPS/ENDL_007"),
                    load_map(&source).unwrap()).unwrap_err()
            };
            assert!(err.contains("unsaved changes"), "{dirty_kind}: {err}");
            let session = app.session.as_ref().unwrap();
            assert_eq!((session as *const _, session.heights().to_vec(),
                session.collision().to_vec(), session.is_dirty(), session.can_undo(), session.can_redo()),
                before_session);
            assert_eq!((app.map_path_input.clone(), app.status_message.clone(), app.status_is_error,
                app.water_surface, app.water_bed, app.ambience.is_dirty(), app.maps.is_dirty(),
                serde_json::to_vec(&app.ambience.profile).unwrap()), before_app);
            assert_eq!(disk_snapshot(&temp_root), before_disk, "dirty kind {dirty_kind}, build={build}");
        }
    }

    // Prove these exact inputs really can build, rather than only testing an invalid builder.
    let mut clean = EditorApp::initial_state();
    let result = clean.create_blank_map(&temp_root, &spec).expect("clean build succeeds");
    result.verify_disk_artifacts(&temp_root).unwrap();
    assert_eq!(clean.map_path_input, result.map_path.to_string_lossy());
    assert_eq!(clean.session.as_ref().unwrap().heights()[100], 42);
    assert!(!clean.session.as_ref().unwrap().is_dirty());
    fs::remove_dir_all(&temp_root).unwrap();
}

#[test]
fn test_blank_map_builder_to_editor_app_closed_loop() {
    let temp_root = std::env::temp_dir().join(format!("arm_e2e_game_{}", Uuid::new_v4()));
    setup_synthetic_game_root(&temp_root);

    let mut app = EditorApp::initial_state();

    // 1. Create a blank map using BlankMapSpec through EditorApp
    let spec = BlankMapSpec::new("ENDL_000", 7, "Frontier Stronghold")
        .with_starter_kit(0, 8192.0, 8192.0)
        .with_height_override(42)
        .with_briefing("Establish frontier stronghold.");

    let result = app
        .create_blank_map(&temp_root, &spec)
        .expect("create_blank_map should succeed");

    assert_eq!(result.info.slot, 7);
    assert_eq!(result.info.id(), "ENDL_007");
    assert_eq!(result.applied_height, 42);
    assert!(result.starter_kit_applied);
    assert_eq!(result.starter_spawns_count, 5);

    // Verify physical disk artifacts
    let summary = result
        .verify_disk_artifacts(&temp_root)
        .expect("disk artifacts valid");
    assert_eq!(summary.slot, 7);
    assert_eq!(summary.boden_height, 42);
    assert!(summary.has_manifest_entry);

    // 2. Verify EditorApp state is seamlessly bound to the new map
    assert!(app.session.is_some());
    let session = app.session.as_ref().unwrap();
    assert_eq!(session.vertex_size, 257);
    assert_eq!(session.heights()[0], 42);
    assert_eq!(session.heights()[128 * 257 + 128], 42);
    assert_eq!(app.map_path_input, result.map_path.to_string_lossy());
    assert_eq!(app.water_surface, 15.0);

    // 3. Edit terrain inside the session
    app.session.as_mut().unwrap().apply_height_changes(&[SampleChange {
        index: 100,
        before: 42,
        after: 95,
    }]);
    assert!(app.session.as_ref().unwrap().is_dirty());

    // 4. Configure Ambience UI (native models: AtmosphereProfile, SceneLightingContext, SoundscapeZone)
    app.ambience.load_preset(WeatherPresets::STORM_ID);
    app.ambience.profile.boden.water_bump_amplitude = 340;
    app.ambience.profile.boden.water_bump_frequency = 6;
    app.ambience.profile.boden.water_warp_shift = 14;
    app.ambience.add_sound_zone(
        "Waterfall Pass",
        "SND_WATERFALL",
        AudioCategory::Hydrology,
        SoundscapeShapeType::Circle,
    );
    app.ambience.briefing_title = "Defend Against Barbarians".to_string();
    app.ambience.briefing_body = "Stand firm at the river crossing.".to_string();

    // Verify scene lighting bridge calculates uniforms
    app.ambience.game_hour = 17.0;
    let uniforms = app.ambience.evaluate_scene_lighting();
    assert_eq!(uniforms.water_dynamics[0], 340.0);
    assert_eq!(uniforms.water_dynamics[1], 6.0);
    assert!(uniforms.effective_ambient[0] > 0.0);

    // 5. Persist ambience & soundscape to map folder
    let save_rep = app
        .ambience
        .save_to_map(&result.map_path)
        .expect("ambience persistence should succeed");
    assert!(save_rep.boden_ini_saved);
    assert!(save_rep.soundscape_saved);
    assert!(save_rep.briefing_saved);

    // Also safe-save terrain map
    let terrain_rep = against_rome_editor_app::storage::safe_save_map(
        &result.map_path,
        app.session.as_mut().unwrap(),
    )
    .expect("safe save terrain");
    assert_eq!(terrain_rep.height_vertex_size, 257);

    // 6. Roundtrip Verification: Launch a fresh editor instance and load the created map
    let mut app2 = EditorApp::initial_state();
    let loaded_session = load_map(&result.map_path).expect("load_map on created map");
    app2.set_loaded_map(result.map_path.clone(), loaded_session)
        .expect("clean editor accepts a loaded map");

    assert_eq!(app2.session.as_ref().unwrap().heights()[100], 95);
    assert_eq!(app2.ambience.profile.boden.water_bump_amplitude, 340);
    assert_eq!(app2.ambience.profile.boden.water_bump_frequency, 6);
    assert_eq!(app2.ambience.zones.len(), 1);
    assert_eq!(app2.ambience.zones[0].name, "Waterfall Pass");
    assert_eq!(
        app2.ambience.briefing_title,
        "Defend Against Barbarians"
    );
    assert_eq!(
        app2.ambience.briefing_body,
        "Stand firm at the river crossing."
    );
    assert!(app2.ambience.lighting_context.day_night.is_some());

    // Clean up temporary files
    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn test_maps_ui_interactive_blank_map_builder_flow() {
    let temp_root = std::env::temp_dir().join(format!("arm_e2e_maps_ui_{}", Uuid::new_v4()));
    setup_synthetic_game_root(&temp_root);

    let mut maps_ui = against_rome_editor_app::maps_ui::MapsUi::default();
    maps_ui.current_tab = against_rome_editor_app::maps_ui::MapsTab::NewBlankMap;
    maps_ui.game_path_input = temp_root.to_string_lossy().to_string();
    maps_ui.blank_source_slot = 0;
    maps_ui.blank_target_slot = 9;
    maps_ui.blank_target_name = "Alpine Valley".to_string();
    maps_ui.blank_height_override = 64;
    maps_ui.blank_include_starter_kit = true;
    maps_ui.blank_starter_team = 1;
    maps_ui.blank_starter_x = 4000.0;
    maps_ui.blank_starter_z = 4000.0;
    maps_ui.blank_briefing = "Alpine conquest mission.".to_string();

    let build_res = maps_ui.build_blank_map().expect("build_blank_map via UI");
    assert_eq!(build_res.info.slot, 9);
    assert_eq!(build_res.info.id(), "ENDL_009");
    assert_eq!(build_res.applied_height, 64);
    assert!(maps_ui.status_message.contains("ENDL_009 created and verified"));

    // Verify manifest entry exists
    let manifest = CustomMapManifest::load(&temp_root).expect("manifest load");
    assert!(manifest.entries().iter().any(|e| e.slot == 9));

    // Verify map can be directly loaded into TerrainSession
    let session = load_map(&build_res.map_path).expect("load newly built map");
    assert_eq!(session.vertex_size, 257);
    assert_eq!(session.heights()[0], 64);

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn test_build_and_open_ui_flow_dirty_session_safety_and_explicit_discard() {
    let temp_root = std::env::temp_dir().join(format!("arm_e2e_maps_ui_safety_{}", Uuid::new_v4()));
    setup_synthetic_game_root(&temp_root);
    let source = temp_root.join("MAPS/ENDL_000");

    let mut app = EditorApp::initial_state();
    app.set_loaded_map(source.clone(), load_map(&source).unwrap()).unwrap();

    // 1. Dirty the session
    app.session.as_mut().unwrap().apply_height_changes(&[SampleChange {
        index: 50,
        before: 50,
        after: 120,
    }]);
    assert!(app.has_unsaved_changes());
    let before_disk = disk_snapshot(&temp_root);

    // 2. Configure MapsUi NewBlankMap parameters for target slot 9
    app.maps.open = true;
    app.maps.current_tab = against_rome_editor_app::maps_ui::MapsTab::NewBlankMap;
    app.maps.game_path_input = temp_root.to_string_lossy().to_string();
    app.maps.blank_target_slot = 9;
    app.maps.blank_target_name = "Safety Valley".to_string();
    app.maps.blank_height_override = 45;
    app.maps.blank_include_starter_kit = false;

    // 3. User clicks "Build Blank Map & Open" via UI (or calls request_build_blank_map)
    let intent = app.maps.request_build_blank_map();
    assert_eq!(app.maps.pending_build_intent, Some(intent.clone()));

    // Running studio panels picks up the intent into app.pending_action
    let ctx = egui::Context::default();
    let _ = ctx.run(Default::default(), |ctx| app.show_studio_panels(ctx));
    assert!(app.maps.pending_build_intent.is_none());
    assert_eq!(app.pending_action(), Some(&PendingAction::BuildBlank(intent)));

    // CRITICAL SAFETY CHECK: Zero disk mutations on intent queuing
    assert_eq!(disk_snapshot(&temp_root), before_disk, "no disk mutation on pending build intent");
    assert!(!temp_root.join("MAPS/ENDL_009").exists(), "target directory must NOT exist");

    // 4. Run handle_pending_action: dialog prompts for unsaved changes
    let _ = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
    let output = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
    assert_eq!(disk_snapshot(&temp_root), before_disk, "no disk mutation while dialog is active");

    // 5. User clicks "Keep editing" (Cancel)
    let cancel_pos = output.shapes.iter().find_map(|s| match &s.shape {
        egui::epaint::Shape::Text(t) if t.galley.text() == "Keep editing" =>
            Some(Rect::from_min_size(t.pos, t.galley.size()).center()),
        _ => None,
    }).expect("Keep editing button");
    for pressed in [true, false] {
        let _ = ctx.run(egui::RawInput {
            events: vec![egui::Event::PointerMoved(cancel_pos), egui::Event::PointerButton {
                pos: cancel_pos, button: egui::PointerButton::Primary, pressed,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        }, |ctx| app.handle_pending_action(ctx));
    }

    assert!(app.pending_action().is_none());
    assert_eq!(disk_snapshot(&temp_root), before_disk, "zero disk mutations when cancelled");
    assert!(!temp_root.join("MAPS/ENDL_009").exists());
    assert!(app.has_unsaved_changes(), "session remains dirty with edits intact");
    assert_eq!(app.session.as_ref().unwrap().heights()[50], 120);

    // 6. Request build again, now confirm "Discard unsaved changes"
    app.maps.request_build_blank_map();
    let _ = ctx.run(Default::default(), |ctx| app.show_studio_panels(ctx));
    assert!(app.pending_action().is_some());

    let _ = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
    let output = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
    let discard_pos = output.shapes.iter().find_map(|s| match &s.shape {
        egui::epaint::Shape::Text(t) if t.galley.text() == "Discard unsaved changes" =>
            Some(Rect::from_min_size(t.pos, t.galley.size()).center()),
        _ => None,
    }).expect("Discard unsaved changes button");
    for pressed in [true, false] {
        let _ = ctx.run(egui::RawInput {
            events: vec![egui::Event::PointerMoved(discard_pos), egui::Event::PointerButton {
                pos: discard_pos, button: egui::PointerButton::Primary, pressed,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        }, |ctx| app.handle_pending_action(ctx));
    }

    // Now, after explicit confirmation, build completes and map is loaded!
    assert!(app.pending_action().is_none());
    let map9_path = temp_root.join("MAPS").join("ENDL_009");
    assert!(map9_path.exists(), "target map created after explicit discard");
    assert_eq!(Path::new(&app.map_path_input), map9_path);
    assert_eq!(app.session.as_ref().unwrap().heights()[0], 45);
    assert!(!app.has_unsaved_changes(), "new session is clean");

    let manifest = CustomMapManifest::load(&temp_root).unwrap();
    assert!(manifest.entries().iter().any(|e| e.slot == 9));

    // 7. Test clean flow: building from a clean session executes immediately without discard prompt
    app.maps.blank_target_slot = 10;
    app.maps.blank_target_name = "Clean Valley".to_string();
    app.maps.blank_height_override = 55;
    app.maps.request_build_blank_map();
    let _ = ctx.run(Default::default(), |ctx| app.show_studio_panels(ctx));
    assert!(app.pending_action().is_some());

    let _ = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
    assert!(app.pending_action().is_none());
    let map10_path = temp_root.join("MAPS").join("ENDL_010");
    assert!(map10_path.exists(), "clean build executes immediately");
    assert_eq!(Path::new(&app.map_path_input), map10_path);
    assert_eq!(app.session.as_ref().unwrap().heights()[0], 55);
    assert!(!app.has_unsaved_changes());

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn flat_template_clones_opens_and_flattens_terrain_unsaved() {
    let temp_root = std::env::temp_dir().join(format!("arm_e2e_flat_template_{}", Uuid::new_v4()));
    setup_synthetic_game_root(&temp_root);
    let mut app = EditorApp::initial_state();
    app.maps.game_path_input = temp_root.to_string_lossy().to_string();
    app.maps.blank_source_slot = 0;
    app.maps.blank_target_name = "Flat Probe".to_string();

    let intent = app.maps.request_build_flat_template().expect("flat template intent");
    assert!(intent.flat_template);
    assert_eq!(intent.spec.target_slot, 5, "next free custom slot");
    assert_eq!(intent.spec.source_map_id, "ENDL_000");
    // Requesting never touches the disk.
    assert!(!temp_root.join("MAPS/ENDL_005").exists());

    app.queue_build_intent(intent);
    let ctx = egui::Context::default();
    app.handle_pending_action(&ctx);
    assert!(!app.status_is_error, "{}", app.status_message);
    let map = temp_root.join("MAPS/ENDL_005");
    assert!(map.join("Endlos_01_Siedlung01.sdl").is_file(), "settlements are kept");
    assert!(app.map_path_input.ends_with("ENDL_005"), "{}", app.map_path_input);

    // Water 30 / step 2 = 15 G units; C# flat terrain = water + 20.
    let session = app.session.as_ref().unwrap();
    assert!(session.heights().iter().all(|h| *h == 35));
    assert!(session.collision().iter().all(|c| *c == 0));
    assert!(app.has_unsaved_changes(), "flattening is pending until the designer saves");
    let on_disk = load_map(&map).unwrap();
    assert_eq!(on_disk.heights()[0], 50, "disk keeps the template terrain until Save");

    // Reset Terrain brings the template heights back; the designer can still choose not to flatten.
    assert!(app.reset_terrain_now());
    assert_eq!(app.session.as_ref().unwrap().heights()[0], 50);
    assert!(!app.has_unsaved_changes());
    let _ = fs::remove_dir_all(&temp_root);
}
