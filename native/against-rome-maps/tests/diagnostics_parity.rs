use against_rome_landscape::cliff::CliffTileCatalog;
use against_rome_maps::{
    CliffDiagnostics, MapCheckObject, MapCheckSnapshot, MapDiagnostics, MapIssueSeverity,
};
use against_rome_scenario::model::{Action, Condition, Event, Spawn};
use uuid::Uuid;

fn obj(id: Uuid, x: f32, z: f32) -> MapCheckObject {
    MapCheckObject::new(
        Spawn {
            alias: "UNIT".into(),
            x,
            z,
            team: 0,
            count: 1,
            id,
            ..Default::default()
        },
        false,
        true,
    )
}

fn unit(x: f32, z: f32) -> MapCheckObject {
    MapCheckObject::new(
        Spawn {
            alias: "UNIT".into(),
            x,
            z,
            team: 0,
            count: 10,
            id: Uuid::new_v4(),
            ..Default::default()
        },
        false,
        false,
    )
}

fn house(x: f32, z: f32, template: bool) -> MapCheckObject {
    MapCheckObject::new(
        Spawn {
            alias: "HOUSE".into(),
            x,
            z,
            team: 0,
            count: 0,
            prebuilt: true,
            id: Uuid::new_v4(),
            ..Default::default()
        },
        true,
        template,
    )
}

fn map_snap(
    objects: Vec<MapCheckObject>,
    collision: Option<Vec<u8>>,
    events: Option<Vec<Event>>,
) -> MapCheckSnapshot {
    let size = if collision.is_some() { 8 } else { 0 };
    MapCheckSnapshot::new(
        objects,
        events.unwrap_or_default(),
        vec!["UNIT".into(), "HOUSE".into()],
        size,
        collision,
    )
}

#[test]
fn duplicate_id_keeps_first_object_for_event_lookup() {
    // First object is in a blocked cell; the duplicate sits in open ground.
    let id = Uuid::new_v4();
    let (size, mut collision) = (4usize, vec![0u8; 16]);
    collision[0] = 1; // cell (0,0) blocked
    let event = Event {
        name: "e".into(),
        conditions: vec![Condition {
            kind: 2,
            target_id: id,
            min_x: 0,
            min_z: 0,
            max_x: 16383,
            max_z: 16383,
        }],
        actions: vec![Action {
            kind: 3, // Victory
            ..Default::default()
        }],
        ..Default::default()
    };
    let snap = MapCheckSnapshot::new(
        vec![obj(id, 10.0, 10.0), obj(id, 12000.0, 12000.0)],
        vec![event],
        vec!["UNIT".into()],
        size,
        Some(collision),
    );
    let issues = MapDiagnostics::check(&snap);
    assert_eq!(issues.iter().filter(|i| i.code == "identity").count(), 1);
    // C# TryAdd keeps the first (blocked) target, so the area check is skipped.
    assert!(!issues.iter().any(|i| i.code == "isolated-area"));
}

#[test]
fn overlap_issues_follow_first_seen_group_order() {
    let ids: Vec<Uuid> = (0..6).map(|_| Uuid::new_v4()).collect();
    let pts = [
        (5000.0, 5000.0),
        (100.0, 100.0),
        (5010.0, 5010.0),
        (9000.0, 9000.0),
        (110.0, 110.0),
        (9010.0, 9010.0),
    ];
    let objects = ids
        .iter()
        .zip(pts)
        .map(|(i, (x, z))| obj(*i, x, z))
        .collect();
    let snap = MapCheckSnapshot::new(objects, vec![], vec!["UNIT".into()], 0, None);
    for _ in 0..20 {
        let got: Vec<Uuid> = MapDiagnostics::check(&snap)
            .iter()
            .filter(|i| i.code == "overlap")
            .map(|i| i.object_id)
            .collect();
        assert_eq!(got, vec![ids[0], ids[2], ids[1], ids[4], ids[3], ids[5]]);
    }
}

/// 對照 C# `MapDiagnosticsTests.Invalid_targets_and_positions_are_errors_and_disabled_targets_do_not_block`
#[test]
fn invalid_targets_and_positions_are_errors_and_disabled_targets_do_not_block() {
    let item = Event {
        name: "Deleted".into(),
        enabled: true,
        conditions: vec![Condition {
            kind: 0, // ObjectExists
            target_id: Uuid::new_v4(),
            ..Default::default()
        }],
        actions: vec![Action {
            kind: 3, // Victory
            ..Default::default()
        }],
        ..Default::default()
    };

    let issues = MapDiagnostics::check(&map_snap(
        vec![unit(f32::NAN, 1000.0)],
        None,
        Some(vec![item.clone()]),
    ));

    assert!(issues
        .iter()
        .any(|i| i.code == "coordinates" && i.severity == MapIssueSeverity::Error));
    assert!(issues
        .iter()
        .any(|i| i.code == "event-target" && i.event_index == 0));

    let disabled_item = Event {
        enabled: false,
        ..item
    };
    let issues_disabled = MapDiagnostics::check(&map_snap(
        vec![unit(1000.0, 1000.0)],
        None,
        Some(vec![disabled_item]),
    ));
    assert!(!issues_disabled
        .iter()
        .any(|i| i.code == "event-target"));
}

/// 對照 C# `MapDiagnosticsTests.Collision_wall_disconnects_house_and_event_area_without_turning_warnings_into_errors`
#[test]
fn collision_wall_disconnects_house_and_event_area_without_turning_warnings_into_errors() {
    let u = unit(1000.0, 1000.0);
    let h = house(14000.0, 1000.0, true);
    let mut collision = vec![0u8; 64];
    for y in 0..8 {
        collision[y * 8 + 4] = 255; // Column 4 blocked
    }
    let area_event = Event {
        name: "Cross wall".into(),
        enabled: true,
        conditions: vec![Condition {
            kind: 2, // ObjectInArea
            target_id: u.spawn.id,
            min_x: 13000,
            min_z: 0,
            max_x: 15000,
            max_z: 2000,
        }],
        actions: vec![Action {
            kind: 3, // Victory
            ..Default::default()
        }],
        ..Default::default()
    };

    let issues = MapDiagnostics::check(&map_snap(
        vec![u.clone(), h.clone()],
        Some(collision.clone()),
        Some(vec![area_event.clone()]),
    ));

    assert!(issues
        .iter()
        .any(|i| i.code == "isolated-building" && i.object_id == h.spawn.id));
    assert!(issues.iter().any(|i| {
        i.code == "isolated-area"
            && i.event_index == 0
            && i.world_x.map(|x| (x - 14000.0).abs() < 1e-4).unwrap_or(false)
    }));
    assert!(!issues.iter().any(|i| i.severity == MapIssueSeverity::Error));

    // Remove the wall opening at row 0, col 4
    collision[4] = 0;
    let issues_open = MapDiagnostics::check(&map_snap(
        vec![u, h],
        Some(collision),
        Some(vec![area_event]),
    ));
    assert!(!issues_open
        .iter()
        .any(|i| i.code.starts_with("isolated-")));
}

/// 對照 C# `MapDiagnosticsTests.Submerged_start_and_missing_templates_are_warnings_and_input_is_unchanged`
#[test]
fn submerged_start_and_missing_templates_are_warnings_and_input_is_unchanged() {
    let u = unit(1000.0, 1000.0);
    let h = house(14000.0, 14000.0, false);
    let collision = vec![0u8; 64];
    let heights = vec![10u8; 81];
    let mut snap = map_snap(vec![u, h], Some(collision.clone()), None);
    snap.height_size = 9;
    snap.heights = Some(heights.clone());
    snap.height_step = 4.0;
    snap.water_level = 120.0;

    let issues = MapDiagnostics::check(&snap);
    assert!(issues.iter().any(|i| i.code == "blocked-start"));
    assert!(issues.iter().any(|i| i.code == "construction-site"));
    assert!(issues.iter().all(|i| i.severity == MapIssueSeverity::Warning));

    // Input vectors must remain unchanged
    assert_eq!(snap.collision.as_ref().unwrap(), &collision);
    assert_eq!(snap.heights.as_ref().unwrap(), &heights);
}

/// 對照 C# `MapDiagnosticsTests.Missing_terrain_and_start_overlap_are_reported_without_guessing_script_spawns`
#[test]
fn missing_terrain_and_start_overlap_are_reported_without_guessing_script_spawns() {
    let a = house(1000.0, 1000.0, true);
    let b = house(1010.0, 1010.0, true);
    let issues = MapDiagnostics::check(&map_snap(vec![a, b], None, None));

    assert!(issues.iter().any(|i| i.code == "no-start"));
    assert!(issues.iter().any(|i| i.code == "no-collision"));
    assert_eq!(issues.iter().filter(|i| i.code == "overlap").count(), 2);
}

/// 對照 C# `MapDiagnosticsTests.Separate_starting_regions_both_count_as_reachable_and_diagonal_contact_is_not_a_path`
#[test]
fn separate_starting_regions_both_count_as_reachable_and_diagonal_contact_is_not_a_path() {
    let mut collision = vec![255u8; 64];
    collision[0] = 0; // tile (0, 0)
    collision[9] = 0; // tile (1, 1) - diagonal only!
    let u = unit(1000.0, 1000.0);
    let h = house(3000.0, 3000.0, true);

    let issues_single_start = MapDiagnostics::check(&map_snap(
        vec![u.clone(), h.clone()],
        Some(collision.clone()),
        None,
    ));
    assert!(issues_single_start
        .iter()
        .any(|i| i.code == "isolated-building"));

    // Add a second start in the other diagonal region
    let u2 = unit(3000.0, 3000.0);
    let issues_two_starts = MapDiagnostics::check(&map_snap(
        vec![u, u2, h],
        Some(collision),
        None,
    ));
    assert!(!issues_two_starts
        .iter()
        .any(|i| i.code == "isolated-building"));
}

/// 驗證團隊與部隊人數合法性範圍邊界 (team-count)
#[test]
fn team_and_count_boundaries_parity() {
    // Count out of bounds (< 0 or > 20)
    let bad_count = MapCheckObject::new(
        Spawn {
            alias: "UNIT".into(),
            x: 1000.0,
            z: 1000.0,
            team: 0,
            count: 21,
            id: Uuid::new_v4(),
            ..Default::default()
        },
        false,
        true,
    );
    let snap1 = map_snap(vec![bad_count], None, None);
    assert!(MapDiagnostics::check(&snap1)
        .iter()
        .any(|i| i.code == "team-count" && i.severity == MapIssueSeverity::Error));

    // Count > 0 requires Team in 0..=7. Team 8 is invalid.
    let bad_team = MapCheckObject::new(
        Spawn {
            alias: "UNIT".into(),
            x: 1000.0,
            z: 1000.0,
            team: 8,
            count: 10,
            id: Uuid::new_v4(),
            ..Default::default()
        },
        false,
        true,
    );
    let snap2 = map_snap(vec![bad_team], None, None);
    assert!(MapDiagnostics::check(&snap2)
        .iter()
        .any(|i| i.code == "team-count" && i.severity == MapIssueSeverity::Error));

    // Count == 0 allows Team in -1..=15. Team -1 with count 0 is valid.
    let neutral_building = MapCheckObject::new(
        Spawn {
            alias: "HOUSE".into(),
            x: 1000.0,
            z: 1000.0,
            team: -1,
            count: 0,
            prebuilt: true,
            id: Uuid::new_v4(),
            ..Default::default()
        },
        true,
        true,
    );
    let snap3 = map_snap(vec![neutral_building], None, None);
    assert!(!MapDiagnostics::check(&snap3)
        .iter()
        .any(|i| i.code == "team-count"));
}

/// 驗證別名與 prebuilt 邏輯 (alias parity)
#[test]
fn alias_and_prebuilt_rules_parity() {
    // Non-prebuilt with unknown alias -> Error
    let unknown_spawn = MapCheckObject::new(
        Spawn {
            alias: "UNKNOWN_ALIEN".into(),
            x: 1000.0,
            z: 1000.0,
            team: 0,
            count: 1,
            prebuilt: false,
            id: Uuid::new_v4(),
            ..Default::default()
        },
        false,
        true,
    );
    let snap1 = map_snap(vec![unknown_spawn], None, None);
    assert!(MapDiagnostics::check(&snap1)
        .iter()
        .any(|i| i.code == "alias" && i.severity == MapIssueSeverity::Error));

    // Prebuilt with unknown alias -> allowed (template script / native object)
    let prebuilt_unknown = MapCheckObject::new(
        Spawn {
            alias: "UNKNOWN_ALIEN".into(),
            x: 1000.0,
            z: 1000.0,
            team: 0,
            count: 0,
            prebuilt: true,
            id: Uuid::new_v4(),
            ..Default::default()
        },
        true,
        true,
    );
    let snap2 = map_snap(vec![prebuilt_unknown], None, None);
    assert!(!MapDiagnostics::check(&snap2)
        .iter()
        .any(|i| i.code == "alias"));
}

/// 對照 C# `CliffDiagnosticsTests`
#[test]
fn cliff_diagnostics_steep_and_missing_collision_parity() {
    let height_size = 9;
    let collision_size = 8;
    let mut heights = vec![10u8; height_size * height_size];
    // vy = 0 is 100, vy = 1 is 10, delta = 90 >= 15
    for vx in 0..height_size {
        heights[vx] = 100;
    }

    let collision_unblocked = vec![0u8; collision_size * collision_size];
    let mut snap = MapCheckSnapshot::new(
        vec![],
        vec![],
        vec![],
        collision_size,
        Some(collision_unblocked),
    );
    snap.height_size = height_size;
    snap.heights = Some(heights.clone());
    snap.height_step = 4.0;

    let issues = CliffDiagnostics::check(&snap, None, None, 2);
    assert!(issues
        .iter()
        .any(|i| i.code == "cliff-steep-unblocked" && i.severity == MapIssueSeverity::Warning));

    // Blocked steep slope clears warning
    snap.collision = Some(vec![255u8; collision_size * collision_size]);
    let issues_blocked = CliffDiagnostics::check(&snap, None, None, 2);
    assert!(!issues_blocked
        .iter()
        .any(|i| i.code == "cliff-steep-unblocked"));

    // Cliff texture without collision triggers cliff-missing-collision
    let catalog = CliffTileCatalog::create_default();
    let textures = vec![
        "FELS_N1".to_string(),
        "4BB___50".to_string(),
        "4BB___50".to_string(),
        "4BB___50".to_string(),
    ];
    let snap_cliff = MapCheckSnapshot::new(
        vec![],
        vec![],
        vec![],
        collision_size,
        Some(vec![0u8; collision_size * collision_size]),
    );
    let issues_cliff = CliffDiagnostics::check(&snap_cliff, Some(&catalog), Some(&textures), 2);
    assert!(issues_cliff
        .iter()
        .any(|i| i.code == "cliff-missing-collision" && i.severity == MapIssueSeverity::Warning));
}

/// 對照 C# `MapDiagnostics.AnalyzeNavMesh` 與 `ConvertToIssues`
#[test]
fn navmesh_analysis_and_convert_to_issues_parity() {
    // Empty collision returns empty analysis
    let empty_snap = MapCheckSnapshot::new(vec![], vec![], vec![], 0, None);
    let empty_result = MapDiagnostics::analyze_nav_mesh(&empty_snap, None, 1);
    assert!(empty_result.isolated_regions.is_empty());
    assert!(empty_result.road_gaps.is_empty());
    assert!(empty_result.formation_choke_points.is_empty());
    assert!(empty_result.repairs.is_empty());
    let empty_issues = MapDiagnostics::convert_to_issues(&empty_result);
    assert!(empty_issues.is_empty());

    // When isolated regions exist, convert_to_issues produces "isolated-land" warnings
    let collision_size = 8;
    let mut collision = vec![255u8; collision_size * collision_size];
    // Create an isolated passable pocket at tile (6, 6)
    collision[6 * collision_size + 6] = 0;
    // Primary seed at tile (1, 1)
    collision[1 * collision_size + 1] = 0;
    let u = unit(2048.0, 2048.0); // Player squad at tile (1, 1)
    let enemy_unit = MapCheckObject::new(
        Spawn {
            alias: "UNIT".into(),
            x: 13312.0,
            z: 13312.0,
            team: 1, // Non-player (interest point, not primary seed)
            count: 10,
            id: Uuid::new_v4(),
            ..Default::default()
        },
        false,
        false,
    );

    let snap = MapCheckSnapshot::new(
        vec![u, enemy_unit],
        vec![],
        vec!["UNIT".into()],
        collision_size,
        Some(collision),
    );

    let nav_result = MapDiagnostics::analyze_nav_mesh(&snap, None, 1);
    assert!(!nav_result.isolated_regions.is_empty());

    let issues = MapDiagnostics::convert_to_issues(&nav_result);
    assert!(issues.iter().any(|i| {
        i.code == "isolated-land"
            && i.severity == MapIssueSeverity::Warning
            && i.chinese.contains("發現孤立無法通達的陸地區域")
            && i.chinese.contains("區域內含有物件或建築！")
    }));
}
