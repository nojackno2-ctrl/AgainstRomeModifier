use against_rome_environment::{
    BiomeEcologyProfile, FloraScatterContext, FloraScatterEngine, FloraScatterParameters,
};
use std::collections::HashMap;

fn create_test_catalog() -> (Vec<i32>, HashMap<i32, String>) {
    let ids = vec![1, 2, 3, 4, 5];
    let mut names = HashMap::new();
    names.insert(1, "LanGerNad00_Tanne_gross".to_string());
    names.insert(2, "LanGerBus01_Busch".to_string());
    names.insert(3, "LanGerGra01_Gras".to_string());
    names.insert(4, "LanGerSch01_Schilf".to_string());
    names.insert(5, "LanGerSte01_Stein".to_string());
    (ids, names)
}

#[test]
fn deterministic_generation_produces_identical_results_for_same_seed() {
    let (templates, names) = create_test_catalog();
    let context = FloraScatterContext {
        dimension: 64,
        tile_world_size: 64.0,
        profile: BiomeEcologyProfile::germanic_forest(),
        available_templates: templates,
        objdef_names: names,
        ..Default::default()
    };

    let parameters = FloraScatterParameters {
        seed: 4242,
        min_tile_x: 10,
        min_tile_y: 10,
        max_tile_x: 30,
        max_tile_y: 30,
        density_multiplier: 1.0,
        ..Default::default()
    };

    let result1 = FloraScatterEngine::generate(&context, &parameters);
    let result2 = FloraScatterEngine::generate(&context, &parameters);

    assert!(!result1.additions.is_empty());
    assert_eq!(result1.additions.len(), result2.additions.len());
    assert_eq!(result1.report.total_planted, result2.report.total_planted);

    for (a, b) in result1.additions.iter().zip(result2.additions.iter()) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.x, b.x);
        assert_eq!(a.y, b.y);
        assert_eq!(a.z, b.z);
        assert_eq!(a.rotation, b.rotation);
    }
}

#[test]
fn different_seeds_produce_different_distributions() {
    let (templates, names) = create_test_catalog();
    let context = FloraScatterContext {
        dimension: 64,
        tile_world_size: 64.0,
        profile: BiomeEcologyProfile::germanic_forest(),
        available_templates: templates,
        objdef_names: names,
        ..Default::default()
    };

    let p1 = FloraScatterParameters {
        seed: 1001,
        min_tile_x: 5,
        min_tile_y: 5,
        max_tile_x: 25,
        max_tile_y: 25,
        ..Default::default()
    };
    let p2 = FloraScatterParameters {
        seed: 2002,
        min_tile_x: 5,
        min_tile_y: 5,
        max_tile_x: 25,
        max_tile_y: 25,
        ..Default::default()
    };

    let result1 = FloraScatterEngine::generate(&context, &p1);
    let result2 = FloraScatterEngine::generate(&context, &p2);

    assert!(!result1.additions.is_empty());
    assert!(!result2.additions.is_empty());
    assert_ne!(result1.additions, result2.additions);
}

#[test]
fn riparian_adaptation_generates_water_plants_along_shoreline_and_avoids_deep_water() {
    let (templates, names) = create_test_catalog();
    let dimension = 32;
    let vertex_size = 33;
    let mut heights = vec![0u8; (vertex_size * vertex_size) as usize];

    // 左側 (tx < 10) 為深水 (H = 110, waterLevel = 120, depth = 10 * 4 = 40)
    // 中間 (tx = 10 ~ 12) 為水岸 (H = 120, waterLevel = 120)
    // 右側 (tx > 12) 為陸地 (H = 135)
    for z in 0..vertex_size {
        for x in 0..vertex_size {
            let idx = (z * vertex_size + x) as usize;
            if x < 10 {
                heights[idx] = 110;
            } else if x <= 12 {
                heights[idx] = 120;
            } else {
                heights[idx] = 135;
            }
        }
    }

    let context = FloraScatterContext {
        dimension,
        vertex_size,
        heights: Some(heights),
        height_map_step: 4.0,
        water_level: 120,
        profile: BiomeEcologyProfile::germanic_forest(),
        available_templates: templates,
        objdef_names: names,
        ..Default::default()
    };

    let parameters = FloraScatterParameters {
        seed: 5566,
        min_tile_x: 0,
        min_tile_y: 0,
        max_tile_x: 31,
        max_tile_y: 31,
        enable_riparian_flora: true,
        density_multiplier: 1.5,
        ..Default::default()
    };

    let result = FloraScatterEngine::generate(&context, &parameters);

    // 深水區（X < 9.5 * 64.0 = 608）嚴格禁止有任何植物
    let deep_water_additions: Vec<_> = result
        .additions
        .iter()
        .filter(|a| a.x < 9.5 * 64.0)
        .collect();
    assert!(deep_water_additions.is_empty());

    // 水岸帶（X 約 10 ~ 13 格）應生成水岸植物（Schilf / 蘆葦）
    let riparian_additions: Vec<_> = result
        .additions
        .iter()
        .filter(|a| a.name.contains("Schilf"))
        .collect();
    assert!(!riparian_additions.is_empty());
    assert!(result.report.riparian_count > 0);
}

#[test]
fn missing_height_data_does_not_imply_deep_water_or_shoreline() {
    let (templates, names) = create_test_catalog();
    for empty_heights in [false, true] {
        for water_level in [0u8, 120u8] {
            let heights = if empty_heights { Some(Vec::new()) } else { None };
            let context = FloraScatterContext {
                dimension: 32,
                vertex_size: 33,
                heights,
                water_level,
                profile: BiomeEcologyProfile::germanic_forest(),
                available_templates: templates.clone(),
                objdef_names: names.clone(),
                ..Default::default()
            };

            let result = FloraScatterEngine::generate(
                &context,
                &FloraScatterParameters {
                    seed: 4242,
                    ..Default::default()
                },
            );

            assert!(!result.additions.is_empty());
            assert_eq!(result.report.riparian_count, 0);
            for a in &result.additions {
                assert_eq!(a.y, 0.0);
            }
        }
    }
}

#[test]
fn present_zero_height_data_is_still_treated_as_deep_water() {
    let (templates, names) = create_test_catalog();
    let context = FloraScatterContext {
        dimension: 32,
        vertex_size: 33,
        heights: Some(vec![0u8; 33 * 33]),
        water_level: 120,
        profile: BiomeEcologyProfile::germanic_forest(),
        available_templates: templates,
        objdef_names: names,
        ..Default::default()
    };

    let result = FloraScatterEngine::generate(&context, &FloraScatterParameters::default());
    assert!(result.additions.is_empty());
    assert_eq!(result.report.total_planted, 0);
}

#[test]
fn cliff_steep_slope_excludes_canopy_trees_in_favor_of_rocks() {
    let (templates, names) = create_test_catalog();
    let dimension = 32;
    let vertex_size = 33;
    let mut heights = vec![0u8; (vertex_size * vertex_size) as usize];

    // 建立斷崖地形：X = 15 ~ 16 處有垂直懸崖高差 (100 -> 160)
    for z in 0..vertex_size {
        for x in 0..vertex_size {
            let idx = (z * vertex_size + x) as usize;
            if x <= 15 {
                heights[idx] = 100;
            } else {
                heights[idx] = 160;
            }
        }
    }

    let context = FloraScatterContext {
        dimension,
        vertex_size,
        heights: Some(heights),
        height_map_step: 4.0,
        water_level: 50,
        profile: BiomeEcologyProfile::germanic_forest(),
        available_templates: templates,
        objdef_names: names,
        ..Default::default()
    };

    let parameters = FloraScatterParameters {
        seed: 7788,
        min_tile_x: 14,
        min_tile_y: 0,
        max_tile_x: 17,
        max_tile_y: 31,
        enable_cliff_rocks: true,
        density_multiplier: 1.5,
        ..Default::default()
    };

    let result = FloraScatterEngine::generate(&context, &parameters);

    // 在懸崖格上（tx=15~16），不可生成高大喬木 (LanGerNad)
    let cliff_trees: Vec<_> = result
        .additions
        .iter()
        .filter(|a| a.x >= 15.0 * 64.0 && a.x <= 17.0 * 64.0 && a.name.contains("LanGerNad"))
        .collect();
    assert!(cliff_trees.is_empty());
}

#[test]
fn biome_profiles_preset_coverage_and_classification() {
    use against_rome_environment::{BiomeType, VegetationLayer};

    for b in [
        BiomeType::GermanicForest,
        BiomeType::DanubeBroadleaf,
        BiomeType::ItalianHills,
        BiomeType::BritishMarsh,
    ] {
        let preset = BiomeEcologyProfile::get_preset(b);
        assert_eq!(preset.biome, b);
        assert!(!preset.species.is_empty());
        assert!(!preset.region_code.is_empty());
        assert!(preset.canopy_threshold > preset.understory_threshold);
        assert!(preset.understory_threshold > preset.ground_flora_threshold);
    }

    assert!(BiomeEcologyProfile::is_landscape("LanGerNad00"));
    assert!(!BiomeEcologyProfile::is_landscape("BauGerHau00"));

    assert_eq!(BiomeEcologyProfile::classify_category("LanGerNad00"), "tree");
    assert_eq!(BiomeEcologyProfile::classify_category("LanGerGra01"), "grass");
    assert_eq!(BiomeEcologyProfile::classify_category("LanGerBus01"), "bush");
    assert_eq!(BiomeEcologyProfile::classify_category("LanGerSch01"), "water");
    assert_eq!(BiomeEcologyProfile::classify_category("LanGerSte01"), "rock");

    assert_eq!(BiomeEcologyProfile::layer_to_category_string(VegetationLayer::Canopy), "tree");
    assert_eq!(BiomeEcologyProfile::layer_to_category_string(VegetationLayer::Understory), "bush");
    assert_eq!(BiomeEcologyProfile::layer_to_category_string(VegetationLayer::GroundFlora), "grass");
    assert_eq!(BiomeEcologyProfile::layer_to_category_string(VegetationLayer::Riparian), "water");
    assert_eq!(BiomeEcologyProfile::layer_to_category_string(VegetationLayer::Rock), "rock");
}

#[test]
fn nature_edit_session_transactions_and_rejections() {
    use against_rome_environment::{NatureAddition, NatureEditSession, NatureSnapshot};

    let mut session = NatureEditSession::new();
    assert_eq!(session.module_id(), "nature");
    assert!(!session.is_dirty());
    assert!(!session.can_undo());
    assert!(!session.can_redo());

    let a1 = NatureAddition::new(1, "LanGerNad00_Tanne_gross", 100.0, 10.0, 200.0, 0.5);
    let a2 = NatureAddition::new(2, "LanGerBus01_Busch", 150.0, 10.0, 250.0, 1.2);

    // Invalid plants rejected
    let bad1 = NatureAddition::new(1, "", 100.0, 10.0, 200.0, 0.0);
    assert!(session.plant_many(&[bad1]).is_err());

    let bad2 = NatureAddition::new(1, "Tree", f32::NAN, 10.0, 200.0, 0.0);
    assert!(session.plant_many(&[bad2]).is_err());

    let bad3 = NatureAddition::new(1, "Tree", 20000.0, 10.0, 200.0, 0.0);
    assert!(session.plant_many(&[bad3]).is_err());

    // Valid plant_many
    assert!(session.plant_many(&[a1.clone(), a2.clone()]).expect("plant ok"));
    assert_eq!(session.additions().len(), 2);
    assert!(session.is_dirty());
    assert!(session.can_undo());

    // Remove operation
    assert!(session.remove(&[10, 11], &[a1.clone()]));
    assert_eq!(session.additions().len(), 1);
    assert_eq!(session.removed_slots().len(), 2);

    // Undo remove
    assert!(session.undo());
    assert_eq!(session.additions().len(), 2);
    assert_eq!(session.removed_slots().len(), 0);

    // Redo remove
    assert!(session.redo());
    assert_eq!(session.additions().len(), 1);
    assert_eq!(session.removed_slots().len(), 2);

    // Accept changes resets dirtiness
    session.accept_changes();
    assert!(!session.is_dirty());

    // Snapshot load and capture
    let snap = session.capture();
    let mut session2 = NatureEditSession::new();
    session2.load(snap.clone());
    assert_eq!(session2.capture(), snap);

    session2.clear();
    assert_eq!(session2.capture(), NatureSnapshot::default());
}

#[test]
fn spatial_clearance_grid_boundary_and_radius() {
    use against_rome_environment::SpatialClearanceGrid;

    let mut grid = SpatialClearanceGrid::new(1024.0, 64.0);
    grid.add(100.0, 100.0, 10.0);

    assert!(!grid.is_free(105.0, 100.0, 10.0));
    assert!(grid.is_free(150.0, 100.0, 10.0));
    assert!(grid.is_free(500.0, 500.0, 10.0));
}

