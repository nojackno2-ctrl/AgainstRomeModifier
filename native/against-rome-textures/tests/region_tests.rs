use against_rome_textures::{
    NativeTerrainMaterialResolver, TerrainBlendAuthoringMap, TerrainBlendEditSession,
    TerrainRegionPlanner,
};
use std::sync::Arc;

struct RegionResolver;
impl NativeTerrainMaterialResolver for RegionResolver {
    fn try_resolve_native_corners(&self, texture: &str) -> Option<[String; 4]> {
        let parts: Vec<&str> = texture.split('/').collect();
        if parts.len() == 4 {
            Some([
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
                parts[3].to_string(),
            ])
        } else {
            None
        }
    }

    fn resolve_native_tile(&self, corners: &[&str; 4], _x: usize, _y: usize) -> Option<String> {
        if corners.iter().any(|&c| c.eq_ignore_ascii_case("unsupported")) {
            None
        } else {
            Some(corners.join("/"))
        }
    }
}

#[test]
fn rectangle_reverses_corners_and_connected_fill_excludes_diagonal_and_other_materials() {
    let rect = TerrainRegionPlanner::rectangle(3, (2, 2), (1, 1)).unwrap();
    assert_eq!(rect, vec![(1, 1), (2, 1), (1, 2), (2, 2)]);

    let materials = vec![
        "grass".to_string(),
        "rock".to_string(),
        "grass".to_string(),
        "GRASS".to_string(),
        "rock".to_string(),
        "grass".to_string(),
        "rock".to_string(),
        "grass".to_string(),
        "grass".to_string(),
    ];
    let connected = TerrainRegionPlanner::connected(3, &materials, (0, 0)).unwrap();
    assert_eq!(connected, vec![(0, 0), (0, 1)]);

    assert!(TerrainRegionPlanner::rectangle(3, (0, 0), (3, 2)).is_err());
}

#[test]
fn polyline_has_connected_diagonal_steps_turns_and_clipped_width() {
    let cells = TerrainRegionPlanner::polyline(8, &[(0, 0), (4, 4), (4, 7)], 3).unwrap();
    assert!(cells.contains(&(0, 0)));
    assert!(cells.contains(&(4, 7)));

    let mut dedup = cells.clone();
    dedup.sort();
    dedup.dedup();
    assert_eq!(cells.len(), dedup.len());

    for &(x, y) in &cells {
        assert!(x < 8 && y < 8);
    }

    let mask: Vec<String> = (0..64)
        .map(|i| {
            if cells.contains(&(i % 8, i / 8)) {
                "road".to_string()
            } else {
                "grass".to_string()
            }
        })
        .collect();

    let conn = TerrainRegionPlanner::connected(8, &mask, (0, 0)).unwrap();
    assert_eq!(cells.len(), conn.len());
}

#[test]
fn region_bakes_as_one_stroke_and_failed_region_preserves_redo_and_pending_edits() {
    let resolver = Arc::new(RegionResolver);
    let original: Vec<String> = vec!["grass/grass/grass/grass".to_string(); 16];
    let import = TerrainBlendAuthoringMap::import(4, &original, resolver.as_ref(), "grass").unwrap();
    let mut session = TerrainBlendEditSession::new(import, &original, resolver).unwrap();

    let rect = TerrainRegionPlanner::rectangle(4, (1, 1), (2, 2)).unwrap();
    let res = session.paint_tiles(&rect, "road", false).unwrap();
    assert!(res.succeeded);

    assert!(session.commit_stroke());
    let painted = session.current_textures().to_vec();
    assert_eq!(painted[5], "road/road/road/road");

    session.undo().unwrap();
    assert_eq!(session.current_textures(), original.as_slice());
    assert!(!session.can_undo());

    assert!(!session.paint_tiles(&[(0, 0)], "unsupported", false).unwrap().succeeded);
    assert_eq!(session.current_textures(), original.as_slice());
    assert!(session.can_redo());

    session.redo().unwrap();
    assert_eq!(session.current_textures(), painted.as_slice());

    assert!(session.paint_circle(0.0, 0.0, 0.0, "sand", true, false, None).unwrap().succeeded);
    let pending = session.current_textures().to_vec();

    assert!(!session
        .paint_tiles(&[(0, 0), (3, 3)], "unsupported", false)
        .unwrap()
        .succeeded);
    assert_eq!(session.current_textures(), pending.as_slice());

    assert!(session.paint_tiles(&[(0, 0), (4, 0)], "road", false).is_err());
    assert_eq!(session.current_textures(), pending.as_slice());
}
