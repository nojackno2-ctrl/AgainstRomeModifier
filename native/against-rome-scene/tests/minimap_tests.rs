use against_rome_scene::{
    canvas_visible_rect, point_to_tile, tile_to_point, zoomed_image_bounds, Rect, Vec2,
};

#[test]
fn minimap_zoom_bounds_letterbox_horizontal_and_vertical() {
    // Client tall: 200x400, image: 100x100 -> letterboxed top and bottom
    let bounds_tall = zoomed_image_bounds((200, 400), (100, 100));
    assert_eq!(Rect::new(0, 100, 200, 200), bounds_tall);

    // Client wide: 400x200, image: 100x100 -> letterboxed left and right
    let bounds_wide = zoomed_image_bounds((400, 200), (100, 100));
    assert_eq!(Rect::new(100, 0, 200, 200), bounds_wide);

    // Client: 300x300, image: 200x100 (2:1 aspect)
    let bounds_rect = zoomed_image_bounds((300, 300), (200, 100));
    assert_eq!(Rect::new(0, 75, 300, 150), bounds_rect);

    // Invalid / zero sizes
    assert_eq!(Rect::EMPTY, zoomed_image_bounds((0, 0), (100, 100)));
    assert_eq!(Rect::EMPTY, zoomed_image_bounds((200, 400), (0, 0)));
    assert_eq!(Rect::EMPTY, zoomed_image_bounds((-10, 400), (100, 100)));
}

#[test]
fn minimap_point_to_tile_outside_clicks_are_ignored() {
    let image_bounds = Rect::new(50, 50, 200, 200);
    const DIMENSION: i32 = 64;

    // Left gutter
    assert!(point_to_tile((20, 100), image_bounds, DIMENSION).is_none());
    // Top gutter
    assert!(point_to_tile((100, 10), image_bounds, DIMENSION).is_none());
    // Right gutter
    assert!(point_to_tile((260, 100), image_bounds, DIMENSION).is_none());
    // Bottom gutter
    assert!(point_to_tile((100, 260), image_bounds, DIMENSION).is_none());
    // Negative
    assert!(point_to_tile((-5, -5), image_bounds, DIMENSION).is_none());
}

#[test]
fn minimap_coordinates_round_trip_accurately() {
    let image_bounds = Rect::new(20, 30, 200, 200);
    const DIMENSION: i32 = 64;

    let test_tiles = [
        Vec2::new(0.0, 0.0),
        Vec2::new(32.0, 32.0),
        Vec2::new(16.5, 48.25),
        Vec2::new(63.5, 63.5),
    ];

    for tile in test_tiles {
        let pt = tile_to_point(tile, image_bounds, DIMENSION);
        let int_pt = (pt.x.round() as i32, pt.y.round() as i32);
        let back_tile = point_to_tile(int_pt, image_bounds, DIMENSION);
        assert!(back_tile.is_some(), "Point should map back inside bounds");
        let back = back_tile.unwrap();
        assert!((tile.x - back.x).abs() <= 0.5);
        assert!((tile.y - back.y).abs() <= 0.5);
    }
}

#[test]
fn minimap_canvas_visible_rect_computation_handles_zoom_and_pan() {
    let minimap_image = Rect::new(0, 0, 200, 200);
    const DIMENSION: i32 = 64;
    let canvas_client = (400, 400);

    // Scene centered and 1:1 with canvas
    let scene_centered = Rect::new(0, 0, 400, 400);
    let visible_all = canvas_visible_rect(scene_centered, canvas_client, minimap_image, DIMENSION);
    assert!((visible_all.x - 0.0).abs() < 0.01);
    assert!((visible_all.y - 0.0).abs() < 0.01);
    assert!((visible_all.width - 200.0).abs() < 0.01);
    assert!((visible_all.height - 200.0).abs() < 0.01);

    // Zoomed 2x
    let scene_zoomed = Rect::new(0, 0, 800, 800);
    let visible_zoomed = canvas_visible_rect(scene_zoomed, canvas_client, minimap_image, DIMENSION);
    assert!((visible_zoomed.width - 100.0).abs() < 0.01);
    assert!((visible_zoomed.height - 100.0).abs() < 0.01);
}
