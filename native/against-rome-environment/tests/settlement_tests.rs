use against_rome_environment::{
    MapLayoutKind, MapLayoutPresets, MultiplayerFairnessBalancer, NatureEditSession,
    ResourceClusterPlanner, SettlementGeneratorEngine, SettlementSiteEvaluator, SettlementTribe,
    SymmetryMode,
};

const DIMENSION: i32 = 256;
const TILE_SIZE: f32 = 64.0;

#[test]
fn evaluator_rejects_steep_terrain_and_water_hazards() {
    let steep_evaluator = SettlementSiteEvaluator::new(
        DIMENSION,
        &(move |x, _z| if x > 100.0 * TILE_SIZE { 50.0 } else { 0.0 }),
        &(|_, _| false),
        0.0,
        TILE_SIZE,
    );

    let steep_result = steep_evaluator.evaluate(100.0, 100.0, SettlementTribe::Germanic, 0, 0.0);
    assert!(!steep_result.is_valid);
    assert!(steep_result.rejection_reason.is_some());

    let water_evaluator = SettlementSiteEvaluator::new(
        DIMENSION,
        &(|_, _| 5.0),
        &(|_, _| false),
        8.0, // 水位高於地面
        TILE_SIZE,
    );

    let flood_result = water_evaluator.evaluate(128.0, 128.0, SettlementTribe::Germanic, 0, 0.0);
    assert!(!flood_result.is_valid);
    let reason = flood_result.rejection_reason.expect("has rejection reason");
    assert!(reason.to_ascii_lowercase().contains("water"));
}

#[test]
fn evaluator_places_main_house_and_six_core_buildings_without_collisions() {
    let evaluator = SettlementSiteEvaluator::new(
        DIMENSION,
        &(|_, _| 20.0), // 平坦高地
        &(|_, _| false),
        0.0,
        TILE_SIZE,
    );

    let result = evaluator.evaluate(128.0, 128.0, SettlementTribe::Germanic, 0, 0.0);
    assert!(result.is_valid);
    assert!(result.total_score > 75.0);
    assert_eq!(result.planned_buildings.len(), 7); // 主屋 + 6 棟核心建築

    let main_house = &result.planned_buildings[0];
    assert_eq!(main_house.type_name, "GER_HAU00");

    for i in 0..result.planned_buildings.len() {
        for j in (i + 1)..result.planned_buildings.len() {
            let b1 = &result.planned_buildings[i];
            let b2 = &result.planned_buildings[j];
            let dx = (b1.world_x - b2.world_x) / TILE_SIZE;
            let dz = (b1.world_z - b2.world_z) / TILE_SIZE;
            let dist = (dx * dx + dz * dz).sqrt();
            let min_dist = b1.footprint_radius + b2.footprint_radius;
            assert!(
                dist >= min_dist,
                "Buildings {} and {} overlap! dist={}, min_dist={}",
                b1.type_name,
                b2.type_name,
                dist,
                min_dist
            );
        }
    }
}

#[test]
fn resource_planner_generates_forest_quarry_and_wildlife_with_proper_constraints() {
    let planner = ResourceClusterPlanner::new(
        DIMENSION,
        &(|_, _| 15.0),
        &(|_, _| false),
        0.0,
        TILE_SIZE,
    );

    let (forest, quarry, wildlife) = planner.plan_clusters(128.0, 128.0, SettlementTribe::Germanic, 12345, 0.0);

    // 森林驗證
    assert!(!forest.is_empty());
    for tree in &forest {
        let dx = (tree.world_x / TILE_SIZE) - 128.0;
        let dz = (tree.world_z / TILE_SIZE) - 128.0;
        let dist = (dx * dx + dz * dz).sqrt();
        assert!(dist >= 13.5 && dist <= 33.0, "tree dist {} out of range", dist);
    }

    // 採石場驗證
    assert!(!quarry.is_empty());
    for rock in &quarry {
        assert!(rock.type_name.to_ascii_lowercase().starts_with("langerste"));
    }

    // 野生動物群驗證
    assert!(!wildlife.is_empty());
    for animal in &wildlife {
        assert_eq!(animal.team, -1); // 中立
    }
}

#[test]
fn balancer_generates_perfect_2p_central_symmetry() {
    let evaluator = SettlementSiteEvaluator::new(DIMENSION, &(|_, _| 20.0), &(|_, _| false), 0.0, TILE_SIZE);
    let planner = ResourceClusterPlanner::new(DIMENSION, &(|_, _| 20.0), &(|_, _| false), 0.0, TILE_SIZE);
    let balancer = MultiplayerFairnessBalancer::new(&evaluator, &planner, DIMENSION, TILE_SIZE);

    let result = balancer
        .generate(
            2,
            SymmetryMode::CentralSymmetry,
            Some(&[SettlementTribe::Germanic, SettlementTribe::Roman]),
            999,
            0.60,
        )
        .expect("generation succeeded");

    assert_eq!(result.player_count, 2);
    let p1 = &result.players[0];
    let p2 = &result.players[1];

    // 驗證點對稱中心 (128, 128)
    let mid_x = (p1.site_evaluation.anchor_tile_x + p2.site_evaluation.anchor_tile_x) * 0.5;
    let mid_z = (p1.site_evaluation.anchor_tile_z + p2.site_evaluation.anchor_tile_z) * 0.5;
    assert!((mid_x - 128.0).abs() < 1.0);
    assert!((mid_z - 128.0).abs() < 1.0);

    // 驗證建築數與隊伍
    assert_eq!(p1.buildings[0].team, 0);
    assert_eq!(p2.buildings[0].team, 1);
    assert_eq!(p1.buildings[0].type_name, "GER_HAU00");
    assert_eq!(p2.buildings[0].type_name, "ROM_HAU00");

    assert!(result.fairness_report.is_balanced);
}

#[test]
fn balancer_generates_fair_4p_rotational_symmetry() {
    let evaluator = SettlementSiteEvaluator::new(DIMENSION, &(|_, _| 20.0), &(|_, _| false), 0.0, TILE_SIZE);
    let planner = ResourceClusterPlanner::new(DIMENSION, &(|_, _| 20.0), &(|_, _| false), 0.0, TILE_SIZE);
    let balancer = MultiplayerFairnessBalancer::new(&evaluator, &planner, DIMENSION, TILE_SIZE);

    let tribes = [
        SettlementTribe::Germanic,
        SettlementTribe::Roman,
        SettlementTribe::Celtic,
        SettlementTribe::Hun,
    ];
    let result = balancer
        .generate(4, SymmetryMode::RotationalSymmetry, Some(&tribes), 777, 0.60)
        .expect("generation succeeded");

    assert_eq!(result.player_count, 4);
    for p in &result.players {
        assert!(p.site_evaluation.is_valid);
    }
    assert!(result.fairness_report.min_inter_player_distance > 50.0 * TILE_SIZE);
}

#[test]
fn engine_converts_to_presets_and_integrates_with_edit_sessions_and_json() {
    let evaluator = SettlementSiteEvaluator::new(DIMENSION, &(|_, _| 20.0), &(|_, _| false), 0.0, TILE_SIZE);
    let planner = ResourceClusterPlanner::new(DIMENSION, &(|_, _| 20.0), &(|_, _| false), 0.0, TILE_SIZE);
    let balancer = MultiplayerFairnessBalancer::new(&evaluator, &planner, DIMENSION, TILE_SIZE);

    let result = balancer
        .generate(2, SymmetryMode::CentralSymmetry, None, 42, 0.60)
        .expect("generation succeeded");

    // 1. 轉為 Placement Preset
    let placement_preset = SettlementGeneratorEngine::to_placement_layout_preset(&result);
    assert_eq!(placement_preset.kind, MapLayoutKind::Placement);
    assert!(placement_preset.entries.len() >= 14); // 至少 2 * 7 棟建築

    // 2. 轉為 Nature Preset
    let nature_preset = SettlementGeneratorEngine::to_nature_layout_preset(&result);
    assert_eq!(nature_preset.kind, MapLayoutKind::Nature);
    assert!(!nature_preset.entries.is_empty());

    // 3. JSON 序列化與反序列化測試
    let (placement_json, nature_json) =
        SettlementGeneratorEngine::export_layout_jsons(&result).expect("json export ok");
    let read_placement = MapLayoutPresets::deserialize(&placement_json).expect("placement deserialized");
    let read_nature = MapLayoutPresets::deserialize(&nature_json).expect("nature deserialized");
    assert_eq!(placement_preset.entries.len(), read_placement.entries.len());
    assert_eq!(nature_preset.entries.len(), read_nature.entries.len());

    // 4. 驗證與 NatureEditSession 整合（PlantMany 與 Undo/Redo）
    let additions = MapLayoutPresets::plan_nature(
        &read_nature,
        |_name| Some(42),
        0.0,
        0.0,
        0.0,
        |_x, _z| 20.0,
    )
    .expect("nature planned");

    let mut nature_session = NatureEditSession::new();
    assert!(nature_session.plant_many(&additions).expect("planted"));
    assert_eq!(additions.len(), nature_session.additions().len());

    assert!(nature_session.undo());
    assert!(nature_session.additions().is_empty());
    assert!(nature_session.redo());
    assert_eq!(additions.len(), nature_session.additions().len());
}

#[test]
fn tribal_blueprints_and_presets_all_tribes() {
    use against_rome_environment::SettlementTribalPresets;

    for &tribe in &[
        SettlementTribe::Germanic,
        SettlementTribe::Roman,
        SettlementTribe::Celtic,
        SettlementTribe::Hun,
    ] {
        let (main_house, core_buildings) = SettlementTribalPresets::get_blueprints(tribe);
        assert!(!main_house.type_name.is_empty());
        assert_eq!(core_buildings.len(), 6);

        let trees = SettlementTribalPresets::get_forest_tree_palette(tribe);
        assert!(!trees.is_empty());

        let stones = SettlementTribalPresets::get_stone_quarry_palette(tribe);
        assert!(!stones.is_empty());
    }

    let wildlife = SettlementTribalPresets::get_wildlife_palette();
    assert_eq!(wildlife, &["ALL_EBE00"]);
}

#[test]
fn map_layout_preset_validation_rejections() {
    use against_rome_environment::{MapLayoutEntry, MapLayoutKind, MapLayoutPreset, MapLayoutPresets};

    // Empty entries rejected
    let empty_preset = MapLayoutPreset::new(MapLayoutKind::Placement, Vec::new());
    assert!(MapLayoutPresets::validate(&empty_preset).is_err());

    // Bad version rejected
    let bad_version = MapLayoutPreset {
        version: 2,
        kind: MapLayoutKind::Placement,
        entries: vec![MapLayoutEntry::new("Type", 0.0, 0.0, 0.0, 0.0)],
    };
    assert!(MapLayoutPresets::validate(&bad_version).is_err());

    // Bad coordinates
    let bad_coords = MapLayoutPreset::new(
        MapLayoutKind::Placement,
        vec![MapLayoutEntry::new("Type", 20000.0, 0.0, 0.0, 0.0)],
    );
    assert!(MapLayoutPresets::validate(&bad_coords).is_err());

    // Nature preset with non-zero team/count
    let bad_nature = MapLayoutPreset::new(
        MapLayoutKind::Nature,
        vec![MapLayoutEntry::with_team_and_count("Tree", 10.0, 10.0, 0.0, 0.0, 1, 0)],
    );
    assert!(MapLayoutPresets::validate(&bad_nature).is_err());

    // Deserialization of invalid JSON
    assert!(MapLayoutPresets::deserialize("not json").is_err());
}

#[test]
fn balancer_topological_equidistant_mode_3p_5p_8p() {
    let evaluator = SettlementSiteEvaluator::new(DIMENSION, &(|_, _| 20.0), &(|_, _| false), 0.0, TILE_SIZE);
    let planner = ResourceClusterPlanner::new(DIMENSION, &(|_, _| 20.0), &(|_, _| false), 0.0, TILE_SIZE);
    let balancer = MultiplayerFairnessBalancer::new(&evaluator, &planner, DIMENSION, TILE_SIZE);

    assert_eq!(balancer.dimension(), DIMENSION);
    assert_eq!(balancer.tile_world_size(), TILE_SIZE);

    for count in [3, 5, 8] {
        let res = balancer
            .generate(count, SymmetryMode::TopologicalEquidistant, None, 123 + count as i32, 0.60)
            .expect("topological generation succeeded");
        assert_eq!(res.player_count, count);
        assert_eq!(res.players.len(), count);
        for p in &res.players {
            assert!(p.site_evaluation.is_valid);
            assert_eq!(p.buildings.len(), 7);
        }
    }

    // Player count out of bounds [2, 8]
    assert!(balancer.generate(1, SymmetryMode::CentralSymmetry, None, 42, 0.60).is_err());
    assert!(balancer.generate(9, SymmetryMode::CentralSymmetry, None, 42, 0.60).is_err());
}

