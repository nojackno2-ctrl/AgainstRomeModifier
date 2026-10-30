use against_rome_editor_app::landscape_ui::*;
use against_rome_textures::{NativeTerrainMaterialResolver, TerrainBlendAuthoringMap, TerrainBlendEditSession};
use std::sync::Arc;

/// Corner-joining resolver (as in the textures crate's region tests); "unsupported" has no tile.
struct JoinResolver;
impl NativeTerrainMaterialResolver for JoinResolver {
    fn try_resolve_native_corners(&self, texture: &str) -> Option<[String; 4]> {
        let p: Vec<&str> = texture.split('/').collect();
        (p.len() == 4).then(|| [p[0].into(), p[1].into(), p[2].into(), p[3].into()])
    }
    fn resolve_native_tile(&self, corners: &[&str; 4], _x: usize, _y: usize) -> Option<String> {
        if corners.iter().any(|c| c.eq_ignore_ascii_case("unsupported")) {
            None
        } else {
            Some(corners.join("/"))
        }
    }
}

fn session(dim: usize) -> TerrainBlendEditSession {
    let resolver = Arc::new(JoinResolver);
    let textures = vec!["G/G/G/G".to_string(); dim * dim];
    let import = TerrainBlendAuthoringMap::import(dim, &textures, resolver.as_ref(), "G").unwrap();
    TerrainBlendEditSession::new(import, &textures, resolver).unwrap()
}

#[test]
fn vertices_parse_and_bounds_check() {
    assert_eq!(parse_region_vertices("1,2\n 3 , 4 \n\n", 8).unwrap(), vec![(1, 2), (3, 4)]);
    assert!(parse_region_vertices("1;2", 8).is_err());
    assert!(parse_region_vertices("a,2", 8).is_err());
    assert!(parse_region_vertices("-1,2", 8).is_err());
    assert!(parse_region_vertices("8,0", 8).is_err());
}

#[test]
fn planning_matches_c_sharp_vertex_rules() {
    let rect = plan_region_cells(RegionOperation::RectangleFill, 8, &[(3, 3), (1, 2)], 3, None).unwrap();
    assert_eq!(rect.len(), 3 * 2);
    // Fill takes exactly two vertices; polyline takes any number >= 2.
    assert!(plan_region_cells(RegionOperation::RectangleFill, 8, &[(0, 0), (1, 1), (2, 2)], 3, None).is_err());
    assert!(plan_region_cells(RegionOperation::RectangleFill, 8, &[(0, 0)], 3, None).is_err());
    let road = plan_region_cells(RegionOperation::RoadPolyline, 8, &[(0, 0), (4, 0), (4, 4)], 1, None).unwrap();
    assert_eq!(road.len(), 9);
    // Connected fill needs the texture grid and a single seed.
    let mut grid = vec!["A".to_string(); 16];
    grid[5] = "B".into();
    let region = plan_region_cells(RegionOperation::ConnectedFill, 4, &[(0, 0)], 3, Some(&grid)).unwrap();
    assert_eq!(region.len(), 15);
    assert!(plan_region_cells(RegionOperation::ConnectedFill, 4, &[(0, 0)], 3, None).is_err());
}

#[test]
fn objects_in_rect_uses_half_open_tile_bounds() {
    let rect = TileRect::from_corners((2, 2), (3, 3));
    let objs = [(1, 512.0, 512.0), (2, 1023.9, 1023.9), (3, 1024.0, 600.0), (4, 100.0, 100.0)];
    assert_eq!(objects_in_rect(&rect, &objs), vec![1, 2]);
    assert_eq!(rect.tile_count(), 4);
    assert_eq!(TileRect::from_corners((5, 1), (2, 9)), TileRect { min_x: 2, min_y: 1, max_x: 5, max_y: 9 });
    assert_eq!(TileRect { min_x: -3, min_y: 0, max_x: 99, max_y: 4 }.clamped(8).unwrap().max_x, 7);
}

#[test]
fn drag_tracks_rectangle_and_cancels() {
    let mut drag = RegionDrag::default();
    assert!(!drag.is_active());
    drag.update((1, 1));
    assert!(drag.rect().is_none(), "update without begin is ignored");
    drag.begin((4, 6));
    drag.update((2, 9));
    assert_eq!(drag.rect(), Some(TileRect { min_x: 2, min_y: 6, max_x: 4, max_y: 9 }));
    let done = drag.finish();
    assert!(done.is_some() && !drag.is_active());
    drag.begin((0, 0));
    drag.cancel();
    assert!(drag.rect().is_none());
}

#[test]
fn fill_applies_as_one_undoable_stroke() {
        let mut s = session(8);
    let request = RegionPaintRequest {
        cells: plan_region_cells(RegionOperation::RectangleFill, 8, &[(1, 1), (3, 2)], 3, None).unwrap(),
        material: "R".into(),
        auto_bridge: false,
    };
    let changed = apply_region_paint(&mut s, &request).unwrap();
    assert!(changed > 0);
    assert!(s.can_undo());
    s.undo().unwrap();
    assert!(!s.is_dirty());
    assert!(apply_region_paint(&mut s, &RegionPaintRequest { cells: vec![(99, 99)], ..request.clone() }).is_err());
    assert!(!s.is_dirty());
}

#[test]
fn fill_with_unsupported_transition_is_rejected_and_leaves_session_unchanged() {
        let mut s = session(8);
    let before = s.current_textures().to_vec();
    let request = RegionPaintRequest {
        cells: vec![(1, 1)],
        material: "unsupported".into(),
        auto_bridge: false,
    };
    let err = apply_region_paint(&mut s, &request).unwrap_err();
    assert!(err.contains("region unchanged"), "{err}");
    assert_eq!(s.current_textures(), before);
    assert!(!s.is_dirty());
}

#[test]
fn ui_plans_requests_and_applies() {
        let mut s = session(8);
    let grid = s.current_textures().to_vec();
    let mut ui = LandscapeUi::default();
    ui.open = true;
    let cx = LandscapeContext { vertex_size: 33, heights: None, textures: Some(&grid), texture_names: &[] };
    ui.region.vertices_text = "1,1\n2,2".into();
    ui.plan_region(&cx).unwrap();
    assert!(ui.is_dirty());
    // No material: refused, plan kept.
    assert!(ui.request_region_apply().is_err());
    ui.region.material = "R".into();
    ui.region.auto_bridge = false;
    ui.request_region_apply().unwrap();
    let outcome = ui.apply_pending_region(Some(&mut s), Vec::new);
    assert!(outcome.textures_changed > 0 && s.is_dirty());
    assert!(ui.region.plan.is_none());

    // Select objects.
    ui.region.op = RegionOperation::SelectObjects;
    ui.region.vertices_text = "1,1\n2,2".into();
    ui.plan_region(&cx).unwrap();
    ui.request_region_apply().unwrap();
    let outcome = ui.apply_pending_region(None, || vec![(7, 300.0, 300.0), (8, 5000.0, 5000.0)]);
    assert_eq!(outcome.selected_objects, Some(vec![7]));

    // Selection from a drag feeds the dialog and the erosion tab.
    ui.set_region_selection(TileRect::from_corners((3, 4), (1, 2)));
    assert_eq!(ui.region.vertices_text, "1,2\n3,4");
    assert_eq!((ui.erosion_start_x, ui.erosion_end_y), (1, 4));
    ui.reset_session();
    assert!(ui.region.selection.is_none() && !ui.is_dirty());
}

#[test]
fn handle_region_pointer_completes_selection_and_cancels() {
    let mut ui = LandscapeUi::default();
    ui.open = true;
    ui.current_tab = LandscapeTab::RegionTools;
    assert!(!ui.region_pick_active());

    // When pick_armed is false, events are ignored
    assert_eq!(ui.handle_region_pointer(LandscapeRegionPointer::Press((2, 3)), 16), None);

    ui.region.pick_armed = true;
    assert!(ui.region_pick_active());

    // Sequence: Press -> Drag -> Release
    assert_eq!(ui.handle_region_pointer(LandscapeRegionPointer::Press((5, 10)), 16), None);
    assert_eq!(ui.handle_region_pointer(LandscapeRegionPointer::Drag((12, 14)), 16), None);
    let completed = ui.handle_region_pointer(LandscapeRegionPointer::Release(Some((12, 14))), 16);
    assert_eq!(completed, Some(TileRect { min_x: 5, min_y: 10, max_x: 12, max_y: 14 }));
    assert_eq!(ui.region.selection, completed);
    assert_eq!(ui.region.vertices_text, "5,10\n12,14");
    assert_eq!((ui.erosion_start_x, ui.erosion_start_y, ui.erosion_end_x, ui.erosion_end_y), (5, 10, 12, 14));

    // Cancel sequence
    assert_eq!(ui.handle_region_pointer(LandscapeRegionPointer::Press((1, 1)), 16), None);
    assert!(ui.region.drag.is_active());
    assert_eq!(ui.handle_region_pointer(LandscapeRegionPointer::Cancel, 16), None);
    assert!(!ui.region.drag.is_active());
}
