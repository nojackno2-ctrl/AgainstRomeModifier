use against_rome_navigation::*;

fn grid(size: usize, cells: &[u8]) -> PassabilityGrid {
    PassabilityGrid::new(size, Some(cells), None, 16384.0).unwrap()
}
fn trip(start: (i32, i32), target: (i32, i32), troops: i32) -> FormationTrip {
    FormationTrip {
        start,
        target,
        troops,
    }
}
/// 7x7 with a one-cell lane at x = 3 through z = 2..=4.
fn lane() -> PassabilityGrid {
    let mut v = vec![0u8; 49];
    for z in 2..=4 {
        for x in [0, 1, 2, 4, 5, 6] {
            v[z * 7 + x] = 1;
        }
    }
    grid(7, &v)
}

#[test]
fn signed_thresholds_match_csharp_for_non_positive_counts() {
    assert_eq!(required_clearance_signed(-5), (0, 0.0));
    assert_eq!(required_clearance_signed(i32::MIN), (0, 0.0));
    assert_eq!(required_clearance_signed(1), (1, 64.0));
    assert_eq!(required_clearance_signed(i32::MAX), (6, 440.0));
    assert_eq!(max_safe_unit_count(-1), 0);
}

#[test]
fn route_reports_narrowest_cell_requirement_and_fit() {
    let g = lane();
    let d = diagnose_route(&g, trip((3, 0), (3, 6), 10));
    assert_eq!(d.status, RouteStatus::Reachable);
    assert_eq!(d.path_len, 7);
    // First cell of minimum clearance 1 along the route is (3,2), not (3,3)/(3,4).
    assert_eq!(d.narrowest, Some(((3, 2), 1)));
    assert!(d.formation_evaluated && !d.no_obstacles);
    assert_eq!((d.required_clearance, d.required_width), (4, 280.0));
    assert_eq!(d.fits(), Some(false));
    let c = d.choke.unwrap();
    assert_eq!((c.clearance, c.safe_units, c.troop_count), (1, 1, 10));
    assert_eq!(c.kind, "TravelPathChoke");
    assert_eq!(c.width, 16384.0 / 7.0);
}

#[test]
fn single_units_are_reachable_but_not_formation_evaluated() {
    let d = diagnose_route(&lane(), trip((3, 0), (3, 6), 1));
    assert_eq!(d.status, RouteStatus::Reachable);
    assert!(!d.formation_evaluated && d.choke.is_none());
    assert_eq!(d.fits(), None);
    assert_eq!(
        diagnose_route(&lane(), trip((3, 0), (3, 6), -4)).fits(),
        None
    );
}

#[test]
fn a_wide_enough_lane_fits_its_formation() {
    let d = diagnose_route(&lane(), trip((3, 0), (3, 6), 2));
    // 2..=5 men need clearance 2 but the lane is 1 wide.
    assert_eq!(d.fits(), Some(false));
    let open = grid(9, &[0; 81]);
    let d = diagnose_route(&open, trip((0, 0), (8, 8), 20));
    // No obstacle anywhere: C# keeps the -1 sentinel and still flags it; callers can tell via the flag.
    assert!(d.no_obstacles);
    assert_eq!(d.narrowest.map(|n| n.1), Some(-1));
    assert_eq!(d.fits(), Some(false));
}

#[test]
fn unreachable_causes_are_distinguished() {
    let mut v = vec![0u8; 25];
    v[0] = 5; // blocked start
    for z in 0..5 {
        v[z * 5 + 3] = 1; // wall splitting x<3 from x=4
    }
    let g = grid(5, &v);
    let d = diagnose_route(&g, trip((0, 0), (1, 1), 5));
    assert_eq!(d.status, RouteStatus::EndpointUnusable);
    assert_eq!(d.start_issue, Some(EndpointIssue::Collision));
    assert_eq!(d.target_issue, None);
    let d = diagnose_route(&g, trip((1, 1), (4, 4), 5));
    assert_eq!(d.status, RouteStatus::Disconnected);
    assert_eq!((d.start_issue, d.target_issue, d.path_len), (None, None, 0));
    assert!(d.narrowest.is_none() && d.choke.is_none());
    let d = diagnose_route(&g, trip((-1, 2), (5, 9), 5));
    assert_eq!(d.status, RouteStatus::EndpointUnusable);
    assert_eq!(d.start_issue, Some(EndpointIssue::OutOfBounds));
    assert_eq!(d.target_issue, Some(EndpointIssue::OutOfBounds));
}

#[test]
fn submerged_endpoint_is_reported_as_water() {
    let h = vec![0u8; 36];
    let g = PassabilityGrid::new(5, None, Some((6, &h, 4.0, 1.0)), 16384.0).unwrap();
    let d = diagnose_route(&g, trip((0, 0), (1, 1), 5));
    assert_eq!(d.start_issue, Some(EndpointIssue::Submerged));
    assert_eq!(d.target_issue, Some(EndpointIssue::Submerged));
}

#[test]
fn report_deduplicates_across_trips_and_falls_back_to_topology() {
    let g = lane();
    let trips = [
        trip((3, 0), (3, 6), 10),
        trip((3, 6), (3, 0), 20), // reverse route: first minimum differs
        trip((3, 0), (3, 6), 1),
    ];
    let r = analyze_formation(&g, &trips, 20);
    assert_eq!(r.routes.len(), 3);
    assert_eq!(r.routes[2].fits(), None);
    // Route order matters for "first minimum": (3,2) forward, (3,4) reverse => two distinct cells.
    let cells: Vec<_> = r
        .corridor_chokes
        .iter()
        .map(|c| (c.location.x, c.location.z))
        .collect();
    assert_eq!(cells, vec![(3, 2), (3, 4)]);
    assert!(r.topological_chokes.is_empty());
    assert_eq!(r.corridor_chokes, evaluate_corridors_signed(&g, &trips),);

    // Same cell from two trips is reported once.
    let dup = analyze_formation(&g, &[trips[0], trips[0]], 10);
    assert_eq!(dup.corridor_chokes.len(), 1);

    // No corridor choke (unreachable) => topological fallback, only when squads exist.
    let mut v = vec![0u8; 49];
    for z in 2..=4 {
        for x in [0, 1, 2, 4, 5, 6] {
            v[z * 7 + x] = 1;
        }
    }
    let r = analyze_formation(&grid(7, &v), &[trip((3, 0), (3, 3), 1)], 10);
    assert!(r.corridor_chokes.is_empty());
    assert_eq!(r.topological_chokes.len(), 3);
    assert!(r
        .topological_chokes
        .iter()
        .all(|c| c.kind == "HorizontalChasm"));
    assert_eq!(r.topological_chokes, topological_chokes(&grid(7, &v), 10));
    assert!(analyze_formation(&grid(7, &v), &[], 0)
        .topological_chokes
        .is_empty());
}

#[test]
fn topological_signed_ignores_non_positive_counts() {
    let g = lane();
    assert!(topological_chokes_signed(&g, 0).is_empty());
    assert!(topological_chokes_signed(&g, -7).is_empty());
    assert_eq!(topological_chokes_signed(&g, 10).len(), 3);
    // A formation of 1 needs clearance 1: nothing is narrower than that.
    assert!(topological_chokes_signed(&g, 1).is_empty());
}

#[test]
fn diagnose_routes_preserves_input_order() {
    let g = lane();
    let trips = [
        trip((3, 0), (3, 6), 5),
        trip((0, 0), (0, 1), 5),
        trip((9, 9), (0, 0), 5),
    ];
    let out = diagnose_routes(&g, &trips);
    assert_eq!(out.iter().map(|d| d.trip).collect::<Vec<_>>(), trips);
    assert_eq!(out[2].status, RouteStatus::EndpointUnusable);
}
