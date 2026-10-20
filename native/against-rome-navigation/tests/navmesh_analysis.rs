use against_rome_navigation::*;

fn make_grid(size: usize, collision: Option<&[u8]>, heights: Option<(usize, &[u8], f32, f32)>) -> PassabilityGrid {
    PassabilityGrid::new(size, collision, heights, 16384.0).unwrap()
}

#[test]
fn coordinate_from_world_clamps_and_preserves_floats_matching_csharp() {
    const SIZE: usize = 256;
    const WORLD: f32 = 16384.0;

    // Out of bounds negative clamps to 0
    let c_neg = Coordinate::from_world(-1000.0, -500.0, SIZE, WORLD);
    assert_eq!((c_neg.x, c_neg.z), (0, 0));
    assert_eq!((c_neg.world_x, c_neg.world_z), (-1000.0, -500.0));

    // Out of bounds high clamps to size - 1
    let c_high = Coordinate::from_world(99999.0, 88888.0, SIZE, WORLD);
    assert_eq!((c_high.x, c_high.z), (255, 255));
    assert_eq!((c_high.world_x, c_high.world_z), (99999.0, 88888.0));

    // Mid point (scale is 16384 / 256 = 64)
    let c_mid = Coordinate::from_world(640.0, 1280.0, SIZE, WORLD);
    assert_eq!((c_mid.x, c_mid.z), (10, 20));
    assert_eq!((c_mid.world_x, c_mid.world_z), (640.0, 1280.0));

    // PassabilityGrid convenience method matches
    let grid = make_grid(SIZE, None, None);
    let from_grid = grid.world_to_coordinate(640.0, 1280.0);
    assert_eq!(c_mid, from_grid);
}

#[test]
fn road_gap_distance_matches_tile_count() {
    let start = Coordinate::from_tile(0, 0, 16, 16384.0);
    let end = Coordinate::from_tile(3, 0, 16, 16384.0);
    let t1 = Coordinate::from_tile(1, 0, 16, 16384.0);
    let t2 = Coordinate::from_tile(2, 0, 16, 16384.0);
    let gap = RoadGap {
        start,
        end,
        tiles: vec![t1, t2],
        texture: "H_WEG1",
        clear_collision: true,
        style: "Standard",
    };
    assert_eq!(gap.gap_distance(), 2);
}

#[test]
fn recommended_repairs_combines_road_heals_and_bridge_actions() {
    let dim = 16;
    let start = Coordinate::from_tile(3, 5, dim, 16384.0);
    let end = Coordinate::from_tile(5, 5, dim, 16384.0);
    let gap_tile = Coordinate::from_tile(4, 5, dim, 16384.0);
    let gap = RoadGap {
        start,
        end,
        tiles: vec![gap_tile],
        texture: "H_WEG1",
        clear_collision: true,
        style: "Standard",
    };

    let isolated = IsolatedRegion {
        component_id: 1,
        tile_count: 50,
        centroid: Coordinate::from_tile(12, 12, dim, 16384.0),
        bounds: (10, 10, 15, 15),
        contains_interest: true,
        bridge: Some(Coordinate::from_tile(8, 8, dim, 16384.0)),
        bridge_distance: 2,
    };

    let isolated_without_bridge = IsolatedRegion {
        component_id: 2,
        tile_count: 10,
        centroid: Coordinate::from_tile(14, 14, dim, 16384.0),
        bounds: (13, 13, 15, 15),
        contains_interest: false,
        bridge: None,
        bridge_distance: 0,
    };

    let repairs = recommended_repairs(
        &[gap],
        &[isolated, isolated_without_bridge],
        "H_WEG1",
        dim,
    );

    assert_eq!(repairs.len(), 2);
    // First is road heal action
    assert_eq!(repairs[0].id, "road-heal-1-4-5");
    assert_eq!(repairs[0].kind, IssueKind::RoadGap);
    assert_eq!(repairs[0].changes.len(), 1);
    assert_eq!(repairs[0].changes[0].collision, Some(0));

    // Second is bridge action
    assert_eq!(repairs[1].id, "bridge-1-8-8");
    assert_eq!(repairs[1].kind, IssueKind::IsolatedLand);
    assert_eq!(repairs[1].changes.len(), 1);
    assert_eq!(repairs[1].changes[0].collision, Some(0));
}

/// Differential parity test matching C# `MapDiagnostics_AnalyzeNavMesh_With64TileTexturesAnd256Collision_DetectsRoadGaps`.
#[test]
fn parity_with_csharp_analyze_navmesh_64textures_and_256collision() {
    const TEX_DIM: usize = 64;
    const COLL_DIM: usize = 256;

    let mut textures = vec!["Gras1".to_string(); TEX_DIM * TEX_DIM];
    // Create road gap at tile (30, 20): road at (28, 20), (29, 20) and (31, 20), (32, 20)
    textures[20 * TEX_DIM + 28] = "H_WEG1".into();
    textures[20 * TEX_DIM + 29] = "H_WEG1".into();
    // gap at (30, 20)
    textures[20 * TEX_DIM + 31] = "H_WEG1".into();
    textures[20 * TEX_DIM + 32] = "H_WEG1".into();

    let mut collision = vec![0u8; COLL_DIM * COLL_DIM];
    // Block collision at the gap tile (30, 20) -> collision pixels [120..124, 80..84]
    for pz in 80..84 {
        for px in 120..124 {
            collision[pz * COLL_DIM + px] = 255;
        }
    }

    let grid = make_grid(COLL_DIM, Some(&collision), None);

    let req = NavMeshAnalysisRequest {
        textures: Some((&textures, TEX_DIM)),
        max_gap_distance: 2,
        ..Default::default()
    };

    let result = analyze_navmesh(&grid, &req);

    assert_eq!(result.road_gaps.len(), 1);
    let gap = &result.road_gaps[0];
    assert_eq!(gap.gap_distance(), 1);
    assert_eq!(gap.tiles.len(), 1);
    assert_eq!((gap.tiles[0].x, gap.tiles[0].z), (30, 20));
    assert!(gap.clear_collision);
    assert_eq!(gap.texture, "H_WEG1");
    assert_eq!(gap.style, "Standard");

    assert_eq!(result.recommended_repairs.len(), 1);
    let repair = &result.recommended_repairs[0];
    assert_eq!(repair.id, "road-heal-1-30-20");
    assert_eq!(repair.changes.len(), 1);
    assert_eq!((repair.changes[0].x, repair.changes[0].z), (30, 20));
    assert_eq!(repair.changes[0].collision, Some(0));
}

/// Differential parity test matching C# `ConnectivityAnalyzer_Detects_Isolated_Land_And_Finds_Bridge`
/// integrated through `analyze_navmesh`.
#[test]
fn parity_with_csharp_analyze_navmesh_isolated_land_and_bridge() {
    const SIZE: usize = 16;
    let mut collision = vec![0u8; SIZE * SIZE];
    // Vertical wall at x = 8
    for z in 0..SIZE {
        collision[z * SIZE + 8] = 255;
    }

    let grid = make_grid(SIZE, Some(&collision), None);

    let primary_seeds = [(2, 2)];
    let interest_points = [(12, 12)];

    let req = NavMeshAnalysisRequest {
        primary_seeds: &primary_seeds,
        interest_points: &interest_points,
        min_isolated_size: 1,
        bridge_texture: "H_WEG1",
        ..Default::default()
    };

    let result = analyze_navmesh(&grid, &req);

    assert_eq!(result.isolated_regions.len(), 1);
    let region = &result.isolated_regions[0];
    assert!(region.contains_interest);
    assert_eq!(region.tile_count, 16 * 7); // 7 columns
    assert!(region.centroid.x >= 9);
    assert_eq!(region.bridge_distance, 1);
    assert_eq!(region.component_id, 1);
    let bridge = region.bridge.expect("bridge point should exist");
    assert_eq!((bridge.x, bridge.z), (8, 0));

    // When textures is None, dimension defaults to 64 matching C# MapDiagnostics.AnalyzeNavMesh
    assert_eq!(result.recommended_repairs.len(), 1);
    let repair = &result.recommended_repairs[0];
    assert_eq!(repair.kind, IssueKind::IsolatedLand);
    assert_eq!(repair.id, "bridge-1-34-2");
    assert_eq!(repair.changes[0].x, 34);
    assert_eq!(repair.changes[0].z, 2);
    assert_eq!(repair.changes[0].collision, Some(0));

    // When textures dimension matches grid size 16, repair action targets tile (8, 0)
    let textures = vec!["Gras1".to_string(); SIZE * SIZE];
    let req_with_tex = NavMeshAnalysisRequest {
        primary_seeds: &primary_seeds,
        interest_points: &interest_points,
        min_isolated_size: 1,
        textures: Some((&textures, SIZE)),
        bridge_texture: "H_WEG1",
        ..Default::default()
    };
    let result_tex = analyze_navmesh(&grid, &req_with_tex);
    assert_eq!(result_tex.recommended_repairs.len(), 1);
    let repair_tex = &result_tex.recommended_repairs[0];
    assert_eq!(repair_tex.id, "bridge-1-8-0");
    assert_eq!(repair_tex.changes[0].x, 8);
    assert_eq!(repair_tex.changes[0].z, 0);
    assert_eq!(repair_tex.changes[0].collision, Some(0));
}

/// Differential parity test matching C# `FormationWidthValidator_Identifies_Choke_Points_For_Squads`
/// integrated through `analyze_navmesh`.
#[test]
fn parity_with_csharp_analyze_navmesh_formation_choke_points() {
    const SIZE: usize = 16;
    let mut collision = vec![0u8; SIZE * SIZE];
    // Single-tile corridor at x = 8, z in 4..=8
    for z in 4..=8 {
        collision[z * SIZE + 7] = 255;
        collision[z * SIZE + 9] = 255;
    }

    let grid = make_grid(SIZE, Some(&collision), None);

    let trip = FormationTrip {
        start: (8, 2),
        target: (8, 12),
        troops: 10,
    };

    let req = NavMeshAnalysisRequest {
        trips: &[trip],
        ..Default::default()
    };

    let result = analyze_navmesh(&grid, &req);

    assert!(!result.formation_choke_points.is_empty());
    let choke = &result.formation_choke_points[0];
    assert_eq!(choke.clearance, 1);
    assert_eq!(choke.troop_count, 10);
    assert_eq!(choke.required, 4);
    assert_eq!(choke.safe_units, 1);
    assert_eq!(choke.kind, "TravelPathChoke");
}

#[test]
fn analyze_navmesh_topological_fallback_when_corridor_chokes_empty() {
    const SIZE: usize = 16;
    let mut collision = vec![0u8; SIZE * SIZE];
    // Create pinch point at (5, 5) with horizontal chasm
    collision[5 * SIZE + 4] = 255;
    collision[5 * SIZE + 6] = 255;

    let grid = make_grid(SIZE, Some(&collision), None);

    let req = NavMeshAnalysisRequest {
        trips: &[], // no trips
        squad_max_troops: 10,
        ..Default::default()
    };

    let result = analyze_navmesh(&grid, &req);

    assert!(!result.formation_choke_points.is_empty());
    let choke = &result.formation_choke_points[0];
    assert_eq!(choke.kind, "HorizontalChasm");
    assert_eq!(choke.troop_count, 10);
    assert_eq!(choke.required, 4);
}

#[test]
fn empty_result_has_all_collections_empty() {
    let empty = NavMeshAnalysisResult::empty();
    assert!(empty.isolated_regions.is_empty());
    assert!(empty.road_gaps.is_empty());
    assert!(empty.formation_choke_points.is_empty());
    assert!(empty.recommended_repairs.is_empty());
}
