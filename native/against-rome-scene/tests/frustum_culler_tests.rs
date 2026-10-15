use against_rome_scene::{
    build_chunk_grid, cull_items, cull_with_chunks, is_visible, project_aabb_to_screen,
    IsometricFootprint, IsometricRenderLayer, IsometricSortItem, Mat4, ScreenAabb, Vec2, Vec3,
    WorldAabb,
};

#[test]
fn tall_tower_with_ground_anchor_offscreen_remains_visible_due_to_height() {
    let view = Mat4::look_at(Vec3::new(10.0, 20.0, 30.0), Vec3::new(10.0, 0.0, 10.0));
    let proj = Mat4::orthographic(40.0, 30.0, 0.1, 100.0);
    let view_proj = view * proj;
    let viewport = Vec2::new(800.0, 600.0);

    let tall_tower = IsometricSortItem::with_coords(
        1,
        10.0,
        0.0,
        26.0,
        IsometricFootprint::ROMAN_TOWER,
        IsometricRenderLayer::StandardObject,
        0,
    );
    let flat_unit = IsometricSortItem::with_coords(
        2,
        10.0,
        0.0,
        26.0,
        IsometricFootprint::POINT,
        IsometricRenderLayer::StandardObject,
        0,
    );

    let tower_visible = is_visible(&tall_tower, view_proj, viewport, 0.0);
    assert!(tower_visible, "Tall tower top should reach into viewport");

    let tower_box = project_aabb_to_screen(WorldAabb::from_sort_item(&tall_tower), view_proj, viewport);
    let unit_box = project_aabb_to_screen(WorldAabb::from_sort_item(&flat_unit), view_proj, viewport);
    assert!(
        tower_box.min.y < unit_box.min.y,
        "Tall tower top should reach higher up in screen coordinates (lower Y)"
    );
}

#[test]
fn completely_distant_objects_are_properly_culled() {
    let view = Mat4::look_at(Vec3::new(10.0, 20.0, 30.0), Vec3::new(10.0, 0.0, 10.0));
    let proj = Mat4::orthographic(40.0, 30.0, 0.1, 100.0);
    let view_proj = view * proj;
    let viewport = Vec2::new(800.0, 600.0);

    let distant = IsometricSortItem::with_coords(
        999,
        500.0,
        0.0,
        500.0,
        IsometricFootprint::UNIT,
        IsometricRenderLayer::StandardObject,
        0,
    );
    assert!(!is_visible(&distant, view_proj, viewport, 32.0));
}

#[test]
fn cull_items_zero_allocation_writes_exact_visible_indices() {
    let view = Mat4::look_at(Vec3::new(10.0, 20.0, 30.0), Vec3::new(10.0, 0.0, 10.0));
    let proj = Mat4::orthographic(40.0, 30.0, 0.1, 100.0);
    let view_proj = view * proj;
    let viewport = Vec2::new(800.0, 600.0);

    let items = [
        IsometricSortItem::with_coords(
            1,
            10.0,
            0.0,
            10.0,
            IsometricFootprint::UNIT,
            IsometricRenderLayer::StandardObject,
            0,
        ),
        IsometricSortItem::with_coords(
            2,
            500.0,
            0.0,
            500.0,
            IsometricFootprint::UNIT,
            IsometricRenderLayer::StandardObject,
            0,
        ),
        IsometricSortItem::with_coords(
            3,
            11.0,
            0.0,
            11.0,
            IsometricFootprint::UNIT,
            IsometricRenderLayer::StandardObject,
            0,
        ),
        IsometricSortItem::with_coords(
            4,
            -500.0,
            0.0,
            -500.0,
            IsometricFootprint::UNIT,
            IsometricRenderLayer::StandardObject,
            0,
        ),
    ];

    let mut visible_indices = vec![0; items.len()];
    let count = cull_items(&items, view_proj, viewport, &mut visible_indices, 32.0).unwrap();

    assert_eq!(2, count);
    assert_eq!(0, visible_indices[0]);
    assert_eq!(2, visible_indices[1]);
}

#[test]
fn chunk_grid_hierarchical_culling_matches_direct_cull() {
    let view = Mat4::look_at(Vec3::new(10.0, 20.0, 30.0), Vec3::new(10.0, 0.0, 10.0));
    let proj = Mat4::orthographic(40.0, 30.0, 0.1, 100.0);
    let view_proj = view * proj;
    let viewport = Vec2::new(800.0, 600.0);

    let items = [
        IsometricSortItem::with_coords(
            1,
            10.0,
            0.0,
            10.0,
            IsometricFootprint::UNIT,
            IsometricRenderLayer::StandardObject,
            0,
        ),
        IsometricSortItem::with_coords(
            2,
            12.0,
            0.0,
            12.0,
            IsometricFootprint::UNIT,
            IsometricRenderLayer::StandardObject,
            0,
        ),
        IsometricSortItem::with_coords(
            3,
            200.0,
            0.0,
            200.0,
            IsometricFootprint::UNIT,
            IsometricRenderLayer::StandardObject,
            0,
        ),
        IsometricSortItem::with_coords(
            4,
            250.0,
            0.0,
            250.0,
            IsometricFootprint::UNIT,
            IsometricRenderLayer::StandardObject,
            0,
        ),
    ];

    let chunks = build_chunk_grid(&items);
    let mut chunk_results = vec![0; items.len()];
    let count = cull_with_chunks(
        &items,
        chunks.values(),
        view_proj,
        viewport,
        &mut chunk_results,
        32.0,
    )
    .unwrap();

    assert_eq!(2, count);
    let visible_set: Vec<usize> = chunk_results[..count].to_vec();
    assert!(visible_set.contains(&0));
    assert!(visible_set.contains(&1));
}

#[test]
fn screen_aabb_intersects_and_padded() {
    let aabb1 = ScreenAabb::new(10.0, 10.0, 100.0, 100.0);
    let aabb2 = ScreenAabb::new(50.0, 50.0, 150.0, 150.0);
    let aabb3 = ScreenAabb::new(200.0, 200.0, 300.0, 300.0);

    assert!(aabb1.intersects(aabb2));
    assert!(!aabb1.intersects(aabb3));

    let viewport = Vec2::new(800.0, 600.0);
    assert!(aabb1.intersects_viewport(viewport));

    let offscreen = ScreenAabb::new(-20.0, -20.0, -10.0, -10.0);
    assert!(!offscreen.intersects_viewport(viewport));
    assert!(offscreen.padded(viewport, 30.0));
    assert!(!offscreen.padded(viewport, 5.0));
}
