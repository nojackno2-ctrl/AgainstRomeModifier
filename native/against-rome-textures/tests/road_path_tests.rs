use against_rome_textures::{
    build_available_road_tiles, orthogonal_between, plan_road_stroke, RoadConnections as C,
    RoadTile, RoadTilePlacement, SimpleRuleResolver, TerrainBlendAuthoringMap,
    TerrainBlendEditSession,
};
use std::sync::Arc;

fn complete_tiles() -> Vec<RoadTile> {
    (1..=15u8)
        .map(|m| RoadTile::new(format!("road{m}"), C(m)))
        .collect()
}
fn blank(dim: usize) -> Vec<String> {
    vec!["grass".to_string(); dim * dim]
}
fn placement(x: i32, y: i32, t: &str) -> RoadTilePlacement {
    RoadTilePlacement { x, y, texture: t.into() }
}

#[test]
fn sparse_pointer_path_chooses_straights_turn_and_endpoints() {
    let plan = plan_road_stroke(6, &blank(6), &[(1, 1), (4, 1), (4, 4)], &complete_tiles(), "road10").unwrap();
    assert!(plan.succeeded);
    assert_eq!(plan.tiles.len(), 7);
    for expected in [
        placement(2, 1, "road10"),
        placement(4, 1, "road12"),
        placement(4, 2, "road5"),
        placement(1, 1, "road2"),
        placement(4, 4, "road1"),
    ] {
        assert!(plan.tiles.contains(&expected), "{expected:?}");
    }
}

#[test]
fn crossing_existing_road_preserves_old_links_and_chooses_junction() {
    let mut textures = blank(6);
    for x in 0..6 {
        textures[2 * 6 + x] = "road10".into();
    }
    let before = textures.clone();
    let plan = plan_road_stroke(6, &textures, &[(3, 0), (3, 5)], &complete_tiles(), "road5").unwrap();
    assert!(plan.succeeded);
    assert!(plan.tiles.contains(&placement(3, 2, "road15")));
    assert_eq!(textures, before, "planner never mutates input");
}

#[test]
fn missing_turn_rejects_entire_plan() {
    let straights = [
        RoadTile::new("horizontal", C::EAST | C::WEST),
        RoadTile::new("vertical", C::NORTH | C::SOUTH),
    ];
    let plan = plan_road_stroke(6, &blank(6), &[(1, 1), (4, 1), (4, 4)], &straights, "horizontal").unwrap();
    assert!(!plan.succeeded);
    assert!(plan.tiles.is_empty());
    assert_eq!(plan.unsupported, [(4, 1)]);
    assert!(plan_road_stroke(6, &blank(6), &[(1, 1), (4, 1)], &straights, "horizontal").unwrap().succeeded);
}

#[test]
fn diagonal_paths_are_edge_connected() {
    for (a, b) in [((1, 1), (4, 4)), ((4, 4), (1, 1)), ((1, 4), (4, 1)), ((4, 1), (1, 4))] {
        let mut path = vec![a];
        path.extend(orthogonal_between(a, b));
        assert_eq!(*path.last().unwrap(), b);
        for w in path.windows(2) {
            assert_eq!((w[0].0 - w[1].0).abs() + (w[0].1 - w[1].1).abs(), 1);
        }
        assert!(plan_road_stroke(6, &blank(6), &[a, b], &complete_tiles(), "road10").unwrap().succeeded);
    }
}

#[test]
fn revisited_tiles_combine_links_without_duplicates() {
    let plan = plan_road_stroke(6, &blank(6), &[(1, 2), (4, 2), (2, 2), (2, 4)], &complete_tiles(), "road10").unwrap();
    assert!(plan.succeeded);
    assert!(plan.tiles.contains(&placement(2, 2, "road14")));
    let mut cells: Vec<_> = plan.tiles.iter().map(|t| (t.x, t.y)).collect();
    cells.sort();
    cells.dedup();
    assert_eq!(cells.len(), plan.tiles.len());
}

#[test]
fn out_of_bounds_rejects_and_empty_path_is_noop() {
    let plan = plan_road_stroke(6, &blank(6), &[(1, 1), (6, 1)], &complete_tiles(), "road10").unwrap();
    assert!(!plan.succeeded && plan.tiles.is_empty());
    assert_eq!(plan.unsupported, [(6, 1)]);
    assert!(plan_road_stroke(6, &blank(6), &[], &complete_tiles(), "road10").unwrap().tiles.is_empty());
    assert!(plan_road_stroke(6, &blank(5), &[], &complete_tiles(), "road10").is_err());
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn catalog_plans_standard_roads() {
    let available = build_available_road_tiles(Some(&names(&["H_WEG1", "H_WEG2", "V_WEG1", "weg1"])), None);
    assert!(available.contains(&RoadTile::new("H_WEG1", C::EAST | C::WEST)));
    assert!(available.contains(&RoadTile::new("V_WEG1", C::NORTH | C::SOUTH)));
    assert!(available.contains(&RoadTile::new("weg1", C::NORTH | C::EAST)));
    let plan = plan_road_stroke(6, &blank(6), &[(1, 1), (4, 1), (4, 4)], &available, "H_WEG1").unwrap();
    assert!(plan.succeeded);
    assert_eq!(plan.tiles.len(), 7);
    let at = |x, y| plan.tiles.iter().find(|t| t.x == x && t.y == y).unwrap().texture.clone();
    assert!(at(2, 1).starts_with("H_WEG"));
    assert_eq!(at(4, 1), "weg1");
    assert!(at(4, 2).starts_with("V_WEG"));
}

#[test]
fn catalog_prefers_roman_and_falls_back() {
    let all = names(&["H_WEG1", "V_WEG1", "weg1", "WEG_H1ROM", "WEG_V1ROM", "Pflaster_braun1"]);
    let available = build_available_road_tiles(Some(&all), Some("WEG_H1ROM"));
    let plan = plan_road_stroke(6, &blank(6), &[(1, 1), (3, 1), (3, 3)], &available, "WEG_H1ROM").unwrap();
    assert!(plan.succeeded);
    assert!(plan.tiles.contains(&placement(2, 1, "WEG_H1ROM")));
    assert!(plan.tiles.contains(&placement(3, 2, "WEG_V1ROM")));
    assert!(plan.tiles.contains(&placement(3, 1, "Pflaster_braun1")));

    let only = build_available_road_tiles(Some(&names(&["weg1"])), None);
    let plan = plan_road_stroke(6, &blank(6), &[(1, 1), (3, 1), (3, 3)], &only, "weg1").unwrap();
    assert!(plan.succeeded);
    assert!(plan.tiles.iter().all(|t| t.texture == "weg1"));
    assert!(build_available_road_tiles(Some(&names(&["unrelated"])), None).is_empty());
}

fn session(dim: usize) -> TerrainBlendEditSession {
    let resolver = Arc::new(SimpleRuleResolver::new().with_materials(&["grass"]));
    let source: Vec<String> = vec!["grass/grass/grass/grass".to_string(); dim * dim];
    let import = TerrainBlendAuthoringMap::import(dim, &source, resolver.as_ref(), "grass").unwrap();
    TerrainBlendEditSession::new(import, &source, resolver).unwrap()
}

#[test]
fn road_session_rejection_restores_pending_stroke_and_keeps_history() {
    let mut s = session(6);
    let source = s.current_textures().to_vec();
    let straights = [
        RoadTile::new("horizontal", C::EAST | C::WEST),
        RoadTile::new("vertical", C::NORTH | C::SOUTH),
    ];
    assert!(s.paint_road_path(&[(0, 0), (3, 0)], &straights, "horizontal").unwrap().succeeded);
    assert!(s.commit_stroke());
    let baseline = s.current_textures().to_vec();
    assert!(s.paint_road_path(&[(1, 3), (4, 3)], &straights, "horizontal").unwrap().succeeded);
    let rejected = s.paint_road_path(&[(1, 3), (4, 3), (4, 5)], &straights, "horizontal").unwrap();
    assert!(!rejected.succeeded);
    assert_eq!(rejected.texture_changes.len(), 4);
    assert_eq!(rejected.unsupported, [(4, 3)]);
    assert_eq!(s.current_textures(), baseline);
    assert!(!s.commit_stroke());
    s.undo();
    assert_eq!(s.current_textures(), source);
    s.redo();
    assert_eq!(s.current_textures(), baseline);
    assert!(s.paint_road_path(&[(1, 3), (4, 3), (4, 5)], &complete_tiles(), "road10").unwrap().succeeded);
    assert!(s.commit_stroke());
    let after = s.current_textures().to_vec();
    s.undo();
    assert_eq!(s.current_textures(), baseline);
    s.redo();
    assert_eq!(s.current_textures(), after);
}
