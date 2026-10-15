use against_rome_scene::{
    compute_continuous_depth, compute_key, should_draw_before, sort_indices, sort_items,
    IsometricFootprint, IsometricRenderLayer, IsometricSortItem,
};

#[test]
fn layer_ordering_always_dominates_regardless_of_coordinates() {
    let shadow = IsometricSortItem::with_coords(
        1,
        100.0,
        0.0,
        100.0,
        IsometricFootprint::POINT,
        IsometricRenderLayer::GroundShadow,
        0,
    );
    let unit = IsometricSortItem::with_coords(
        2,
        0.0,
        0.0,
        0.0,
        IsometricFootprint::UNIT,
        IsometricRenderLayer::StandardObject,
        0,
    );
    let canopy = IsometricSortItem::with_coords(
        3,
        0.0,
        10.0,
        0.0,
        IsometricFootprint::POINT,
        IsometricRenderLayer::OverheadCanopy,
        0,
    );

    assert!(compute_key(&shadow) < compute_key(&unit));
    assert!(compute_key(&unit) < compute_key(&canopy));

    let mut items = [canopy, shadow, unit];
    sort_items(&mut items);

    assert_eq!(IsometricRenderLayer::GroundShadow, items[0].layer);
    assert_eq!(IsometricRenderLayer::StandardObject, items[1].layer);
    assert_eq!(IsometricRenderLayer::OverheadCanopy, items[2].layer);
}

#[test]
fn high_altitude_unit_behind_tree_is_drawn_before_foreground_tree() {
    let mountain_soldier = IsometricSortItem::with_coords(
        101,
        5.0,
        40.0,
        5.0,
        IsometricFootprint::UNIT,
        IsometricRenderLayer::StandardObject,
        0,
    );
    let foreground_tree = IsometricSortItem::with_coords(
        102,
        12.0,
        0.0,
        12.0,
        IsometricFootprint::new(1.5, 1.5, 8.0),
        IsometricRenderLayer::StandardObject,
        0,
    );

    assert!(should_draw_before(&mountain_soldier, &foreground_tree));
    assert!(compute_key(&mountain_soldier) < compute_key(&foreground_tree));

    let mut scene = [foreground_tree, mountain_soldier];
    sort_items(&mut scene);

    assert_eq!(101, scene[0].item_id);
    assert_eq!(102, scene[1].item_id);
}

#[test]
fn multi_tile_large_building_and_surrounding_units_sort_correctly() {
    let tower = IsometricSortItem::with_coords(
        200,
        10.0,
        0.0,
        10.0,
        IsometricFootprint::ROMAN_TOWER,
        IsometricRenderLayer::StandardObject,
        0,
    );

    let soldier_rear = IsometricSortItem::with_coords(
        201,
        9.0,
        0.0,
        9.0,
        IsometricFootprint::UNIT,
        IsometricRenderLayer::StandardObject,
        0,
    );

    let soldier_front = IsometricSortItem::with_coords(
        202,
        13.0,
        0.0,
        13.0,
        IsometricFootprint::UNIT,
        IsometricRenderLayer::StandardObject,
        0,
    );

    assert!(compute_key(&soldier_rear) < compute_key(&tower));
    assert!(compute_key(&tower) < compute_key(&soldier_front));

    let mut scene = [soldier_front, tower, soldier_rear];
    sort_items(&mut scene);

    assert_eq!(201, scene[0].item_id); // Rear soldier drawn first
    assert_eq!(200, scene[1].item_id); // Tower
    assert_eq!(202, scene[2].item_id); // Front soldier drawn last
}

#[test]
fn deterministic_tie_breaker_prevents_flickering_between_coplanar_items() {
    let item_a = IsometricSortItem::with_coords(
        10,
        5.0,
        0.0,
        5.0,
        IsometricFootprint::POINT,
        IsometricRenderLayer::StandardObject,
        0,
    );
    let item_b = IsometricSortItem::with_coords(
        20,
        5.0,
        0.0,
        5.0,
        IsometricFootprint::POINT,
        IsometricRenderLayer::StandardObject,
        0,
    );

    let key_a = compute_key(&item_a);
    let key_b = compute_key(&item_b);

    assert_ne!(key_a.0, key_b.0);

    let mut list1 = [item_a, item_b];
    let mut list2 = [item_b, item_a];

    sort_items(&mut list1);
    sort_items(&mut list2);

    assert_eq!(list1[0].item_id, list2[0].item_id);
    assert_eq!(list1[1].item_id, list2[1].item_id);
}

#[test]
fn sort_indices_produces_identical_sequence_without_modifying_source() {
    let original = [
        IsometricSortItem::with_coords(
            1,
            20.0,
            0.0,
            20.0,
            IsometricFootprint::UNIT,
            IsometricRenderLayer::StandardObject,
            0,
        ),
        IsometricSortItem::with_coords(
            2,
            5.0,
            0.0,
            5.0,
            IsometricFootprint::UNIT,
            IsometricRenderLayer::StandardObject,
            0,
        ),
        IsometricSortItem::with_coords(
            3,
            15.0,
            0.0,
            15.0,
            IsometricFootprint::UNIT,
            IsometricRenderLayer::StandardObject,
            0,
        ),
    ];

    let sorted_indices = sort_indices(&original);
    assert_eq!(vec![1, 2, 0], sorted_indices);

    let mut copied = original;
    sort_items(&mut copied);
    assert_eq!(2, copied[0].item_id);
    assert_eq!(3, copied[1].item_id);
    assert_eq!(1, copied[2].item_id);
}

#[test]
fn continuous_depth_reflects_layer_and_ground_projection() {
    let item1 = IsometricSortItem::with_coords(
        1,
        10.0,
        2.0,
        10.0,
        IsometricFootprint::UNIT,
        IsometricRenderLayer::StandardObject,
        10,
    );
    let item2 = IsometricSortItem::with_coords(
        2,
        10.0,
        2.0,
        10.0,
        IsometricFootprint::UNIT,
        IsometricRenderLayer::AirborneEffect,
        10,
    );

    let d1 = compute_continuous_depth(&item1);
    let d2 = compute_continuous_depth(&item2);

    assert!(d1 < d2);
    // StandardObject is layer 3 -> ~30000; AirborneEffect is layer 5 -> ~50000
    assert!(d1 >= 30000.0 && d1 < 40000.0);
    assert!(d2 >= 50000.0);
}

#[test]
fn southmost_ground_offset_calculation() {
    let longhouse = IsometricFootprint::LONGHOUSE;
    assert_eq!((4.0 + 3.0) * 0.5, longhouse.southmost_ground_offset());
}
