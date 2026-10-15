use against_rome_navigation::*;
fn grid(size: usize, cells: &[u8]) -> PassabilityGrid {
    PassabilityGrid::new(size, Some(cells), None, 16384.0).unwrap()
}
#[test]
fn independent_layers_and_owned_snapshots() {
    let mut collision = vec![0; 4];
    collision[1] = 7;
    let heights = vec![2; 9];
    let g =
        PassabilityGrid::new(2, Some(&collision), Some((3, &heights, 4.0, 9.0)), 16384.0).unwrap();
    collision[1] = 0;
    assert_eq!(g.collision_byte(1, 0), 7);
    assert!(g.submerged(0, 0));
    assert!(!g.passable(0, 0));
    assert!(!g.passable(-1, 0));
    let dry = PassabilityGrid::new(2, Some(&[1]), Some((3, &heights, 4.0, 8.0)), 16384.0).unwrap();
    assert!(dry.passable(0, 0));
    assert_eq!(dry.world_to_tile(-4.0, 20000.0), (0, 1));
    assert_eq!(dry.coordinate(0, 0).world_x, 4096.0);
}
#[test]
fn collision_footprint_checks_every_covered_cell() {
    let mut v = vec![0; 16];
    v[1] = 3;
    let g = grid(4, &v);
    assert!(g.tile_collision_blocked(0, 0, 2));
    assert!(!g.tile_collision_blocked(1, 1, 2));
    assert!(g.tile_collision_blocked(0, 0, 0));
}
#[test]
fn separate_islands_bridge_distance_and_real_path_midpoint() {
    let mut v = vec![0; 25];
    for z in 0..5 {
        v[z * 5 + 2] = 1;
    }
    let g = grid(5, &v);
    let r = analyze_connectivity(&g, &[(0, 0)], &[(4, 3)], 1);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].tile_count, 10);
    assert!(r[0].contains_interest);
    assert_eq!(r[0].bounds, (3, 0, 4, 4));
    assert_eq!(r[0].bridge_distance, 1);
    assert_eq!(r[0].bridge.unwrap().x, 2);
    let repair = bridge_repair(&r[0], "H_WEG1", 5).unwrap();
    assert_eq!(repair.x, 2);
    assert_eq!(repair.collision, Some(0));
    assert!(analyze_connectivity(&g, &[(0, 0), (4, 0)], &[], 1).is_empty());
    assert!(analyze_connectivity(&g, &[(0, 0)], &[], 11).is_empty());
}
#[test]
fn diagonals_do_not_connect_and_missing_primary_has_no_bridge() {
    let g = grid(2, &[0, 1, 1, 0]);
    let r = analyze_connectivity(&g, &[], &[], 1);
    assert_eq!(r.len(), 2);
    assert!(r.iter().all(|r| r.bridge.is_none()));
    assert!(find_path(&g, (0, 0), (1, 1)).is_none());
}
#[test]
fn bridge_search_stops_at_twelve_blocked_cells() {
    let size = 16;
    let mut v = vec![1; size * size];
    for z in 0..size {
        v[z * size] = 0;
        v[z * size + 15] = 0;
    }
    let r = analyze_connectivity(&grid(size, &v), &[(0, 0)], &[], 1);
    assert!(r[0].bridge.is_none());
}
#[test]
fn clearance_preserves_manhattan_and_open_map_sentinel() {
    let g = grid(3, &[1, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(clearance_field(&g), vec![0, 1, 2, 1, 2, 3, 2, 3, 4]);
    assert_eq!(clearance_field(&grid(2, &[0; 4])), vec![-1; 4]);
}
#[test]
fn corridor_chokes_are_deduplicated_and_single_troops_ignored() {
    let g = grid(
        5,
        &[
            1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1,
        ],
    );
    let path = find_path(&g, (2, 0), (2, 4)).unwrap();
    assert_eq!(path.len(), 5);
    let chokes = evaluate_corridors(
        &g,
        &[
            ((2, 0), (2, 4), 10),
            ((2, 0), (2, 4), 20),
            ((2, 0), (2, 4), 1),
        ],
    );
    assert_eq!(chokes.len(), 1);
    assert_eq!(chokes[0].clearance, 1);
    assert_eq!(chokes[0].safe_units, 1);
    assert_eq!(topological_chokes(&g, 10).len(), 3);
}
#[test]
fn formation_contract_thresholds() {
    assert_eq!(required_clearance(0), (0, 0.0));
    assert_eq!(required_clearance(1), (1, 64.0));
    assert_eq!(required_clearance(5), (2, 160.0));
    assert_eq!(required_clearance(10), (4, 280.0));
    assert_eq!(required_clearance(15), (5, 360.0));
    assert_eq!(required_clearance(20), (6, 440.0));
    assert_eq!(max_safe_unit_count(4), 12);
}
fn road() -> Vec<String> {
    let mut t = vec!["grass".into(); 25];
    t[10] = "H_WEG1".into();
    t[14] = "H_WEG1".into();
    t
}
#[test]
fn road_gap_dimensions_direction_style_and_collision_prescriptions() {
    let mut t = road();
    t[14] = "WEG_H1ROM".into();
    assert!(detect_road_gaps(5, &t, None, 2).unwrap().is_empty());
    t[13] = t[14].clone();
    t[14] = "grass".into();
    let mut v = vec![0; 25];
    v[11] = 7;
    let g = grid(5, &v);
    let gaps = detect_road_gaps(5, &t, Some(&g), 2).unwrap();
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0].tiles.len(), 2);
    assert_eq!(gaps[0].style, "Roman");
    assert_eq!(gaps[0].texture, "WEG_H1ROM");
    assert!(gaps[0].clear_collision);
    let repairs = road_repairs(&gaps);
    assert_eq!(repairs[0][0].collision, Some(0));
    assert_eq!(repairs[0][1].x, 2);
    t[13] = "V_WEG1".into();
    assert!(detect_road_gaps(5, &t, None, 2).unwrap().is_empty());
    assert!(detect_road_gaps(2, &t, None, 2).is_err());
}
#[test]
fn submerged_gaps_never_offer_road_fill() {
    let mut t = road();
    t[12] = "H_WEG1".into();
    t[14] = "grass".into();
    let h = vec![0; 36];
    let g = PassabilityGrid::new(5, None, Some((6, &h, 4.0, 1.0)), 16384.0).unwrap();
    assert!(detect_road_gaps(5, &t, Some(&g), 2).unwrap().is_empty());
}
#[test]
fn road_names_are_case_insensitive() {
    assert!(is_road("h_weg1"));
    assert!(is_road("Pflaster_braun1"));
    assert!(!is_road("grass"));
    assert_eq!(road_style("pfad2"), "Dirt");
}
#[test]
fn invalid_world_and_zero_size_are_errors() {
    assert!(PassabilityGrid::new(0, None, None, 16384.0).is_err());
    assert!(PassabilityGrid::new(1, None, None, f32::NAN).is_err());
}
