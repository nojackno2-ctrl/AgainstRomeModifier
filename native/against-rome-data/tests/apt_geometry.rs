use against_rome_data::apt::*;
use against_rome_data::apt_geometry::*;

#[test]
fn round_half_to_even_handles_integers_half_integers_and_signs() {
    // Exact integers
    assert_eq!(round_half_to_even(0.0), 0.0);
    assert_eq!(round_half_to_even(1.0), 1.0);
    assert_eq!(round_half_to_even(-1.0), -1.0);
    assert_eq!(round_half_to_even(42.0), 42.0);
    assert_eq!(round_half_to_even(-42.0), -42.0);

    // Non-half values round to nearest
    assert_eq!(round_half_to_even(0.4), 0.0);
    assert_eq!(round_half_to_even(0.6), 1.0);
    assert_eq!(round_half_to_even(-0.4), 0.0);
    assert_eq!(round_half_to_even(-0.6), -1.0);

    // Ties round to even integer: positive
    assert_eq!(round_half_to_even(0.5), 0.0);
    assert_eq!(round_half_to_even(1.5), 2.0);
    assert_eq!(round_half_to_even(2.5), 2.0);
    assert_eq!(round_half_to_even(3.5), 4.0);
    assert_eq!(round_half_to_even(4.5), 4.0);

    // Ties round to even integer: negative
    assert_eq!(round_half_to_even(-0.5), 0.0);
    assert_eq!(round_half_to_even(-1.5), -2.0);
    assert_eq!(round_half_to_even(-2.5), -2.0);
    assert_eq!(round_half_to_even(-3.5), -4.0);
    assert_eq!(round_half_to_even(-4.5), -4.0);
}

#[test]
fn world_grid_rejects_non_finite_and_out_of_bounds_values() {
    assert_eq!(
        world_grid(f32::NAN),
        Err(AptError::ParentCoordinateNotFinite)
    );
    assert_eq!(
        world_grid(f32::INFINITY),
        Err(AptError::ParentCoordinateNotFinite)
    );
    assert_eq!(
        world_grid(f32::NEG_INFINITY),
        Err(AptError::ParentCoordinateNotFinite)
    );

    // Extremely large float exceeding signed i32 bounds
    assert_eq!(
        world_grid(1.0e15),
        Err(AptError::ParentCoordinateOutOfBounds)
    );
    assert_eq!(
        world_grid(-1.0e15),
        Err(AptError::ParentCoordinateOutOfBounds)
    );
}

#[test]
fn palette_to_argb_swaps_red_and_blue_and_sets_opaque_alpha() {
    // 0x00BBGGRR -> 0xFFAARRGGBB
    // Red = 0x11, Green = 0x22, Blue = 0x33
    let stored = 0x00332211;
    let expected = 0xFF112233;
    assert_eq!(palette_to_argb(stored), expected);

    // Black
    assert_eq!(palette_to_argb(0x00000000), 0xFF000000);
    // White
    assert_eq!(palette_to_argb(0x00FFFFFF), 0xFFFFFFFF);
    // Pure Red: stored low byte 0xFF
    assert_eq!(palette_to_argb(0x000000FF), 0xFFFF0000);
    // Pure Blue: stored high byte 0xFF0000
    assert_eq!(palette_to_argb(0x00FF0000), 0xFF0000FF);
    // Pure Green: stored mid byte 0x00FF00
    assert_eq!(palette_to_argb(0x0000FF00), 0xFF00FF00);
}

#[test]
fn apt_tile_info_correctly_unpacks_packed_coordinates() {
    // X is low 11 bits (0..=2047), Y is bits 11..21 (0..=2047)
    let tile = AptTileInfo::new(128, 0x0000001E, 0); // X = 30, Y = 0
    assert_eq!(tile.x(), 30);
    assert_eq!(tile.y(), 0);

    let tile2 = AptTileInfo::new(256, (15 << 11) | 30, 0); // X = 30, Y = 15
    assert_eq!(tile2.x(), 30);
    assert_eq!(tile2.y(), 15);

    let tile_max = AptTileInfo::new(0, 0x003FFFFF, 0); // X = 2047, Y = 2047
    assert_eq!(tile_max.x(), 2047);
    assert_eq!(tile_max.y(), 2047);
}
