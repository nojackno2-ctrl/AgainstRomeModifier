use against_rome_scene::{AtlasRect, DynamicAtlasPacker, MaxRectsHeuristic};

#[test]
fn max_rects_packs_heterogeneous_rectangles_with_high_efficiency() {
    let mut packer = DynamicAtlasPacker::new(512, 1).unwrap();

    let sprites: [(i32, i32); 12] = [
        (64, 64),
        (128, 64),
        (32, 96),
        (200, 150),
        (48, 48),
        (64, 32),
        (80, 80),
        (128, 128),
        (16, 32),
        (32, 32),
        (64, 128),
        (96, 64),
    ];

    let mut success_count = 0;
    for &(w, h) in &sprites {
        if let Some((placed_rect, uv)) =
            packer.try_insert(w, h, MaxRectsHeuristic::BestShortSideFit)
        {
            success_count += 1;
            assert_eq!(w, placed_rect.width);
            assert_eq!(h, placed_rect.height);
            assert!(placed_rect.x >= 1);
            assert!(placed_rect.y >= 1);
            assert!(uv.u1 > uv.u0);
            assert!(uv.v1 > uv.v0);
        }
    }

    assert_eq!(sprites.len(), success_count);
    let efficiency = packer.calculate_efficiency(packer.used_width(), packer.used_height());
    assert!(
        efficiency > 60.0,
        "Efficiency should exceed 60%, got {efficiency:.2}%"
    );
}

#[test]
fn gutter_margins_are_strictly_respected_and_isolated() {
    let mut packer = DynamicAtlasPacker::new(64, 2).unwrap();

    let (rect_a, _) = packer
        .try_insert(10, 10, MaxRectsHeuristic::BestShortSideFit)
        .expect("rect A should fit");
    let (rect_b, _) = packer
        .try_insert(10, 10, MaxRectsHeuristic::BestShortSideFit)
        .expect("rect B should fit");

    assert_eq!(2, rect_a.x);
    assert_eq!(2, rect_a.y);
    assert!(
        rect_b.x >= rect_a.right() + 4 || rect_b.y >= rect_a.bottom() + 4,
        "rects must be separated by at least 2*gutter"
    );
}

#[test]
fn transaction_rollback_reverts_state_completely_on_failure() {
    let mut packer = DynamicAtlasPacker::new(64, 1).unwrap();

    assert!(packer
        .try_insert(20, 20, MaxRectsHeuristic::BestShortSideFit)
        .is_some());
    let initial_placed = packer.placed_count();
    let initial_free = packer.free_rectangles_count();
    let initial_width = packer.used_width();
    let initial_height = packer.used_height();
    let initial_efficiency = packer.calculate_efficiency(64, 64);

    packer.begin_transaction();
    assert!(packer
        .try_insert(20, 20, MaxRectsHeuristic::BestShortSideFit)
        .is_some());
    assert!(packer
        .try_insert(20, 20, MaxRectsHeuristic::BestShortSideFit)
        .is_some());
    // 43x43 cannot fit in remaining space
    assert!(packer
        .try_insert(43, 43, MaxRectsHeuristic::BestShortSideFit)
        .is_none());

    packer.rollback_transaction();

    assert_eq!(initial_placed, packer.placed_count());
    assert_eq!(initial_free, packer.free_rectangles_count());
    assert_eq!(initial_width, packer.used_width());
    assert_eq!(initial_height, packer.used_height());
    assert_eq!(initial_efficiency, packer.calculate_efficiency(64, 64));

    let mut control = DynamicAtlasPacker::new(64, 1).unwrap();
    assert!(control
        .try_insert(20, 20, MaxRectsHeuristic::BestShortSideFit)
        .is_some());
    for &(width, height) in &[(40, 40), (20, 20), (20, 20), (1, 1)] {
        let expected = control.try_insert(width, height, MaxRectsHeuristic::BestShortSideFit);
        let actual = packer.try_insert(width, height, MaxRectsHeuristic::BestShortSideFit);
        assert_eq!(expected.is_some(), actual.is_some());
        if let (Some((er, euv)), Some((ar, auv))) = (expected, actual) {
            assert_eq!(er, ar);
            assert_eq!(euv, auv);
        }
    }
}

#[test]
fn try_insert_batch_operates_atomically_for_animation_frames() {
    let mut packer = DynamicAtlasPacker::new(128, 1).unwrap();

    let valid_animation: Vec<(i32, i32)> = vec![(24, 24); 8];
    let result = packer.try_insert_batch(&valid_animation);
    assert!(result.is_some());
    let placed = result.unwrap();
    assert_eq!(8, placed.len());
    assert_eq!(8, packer.placed_count());

    let bad_animation = [(16, 16), (200, 200), (16, 16)];
    let bad_result = packer.try_insert_batch(&bad_animation);
    assert!(bad_result.is_none());
    assert_eq!(8, packer.placed_count()); // Remained intact
}

#[test]
fn next_power_of_two_computes_correct_texture_boundaries() {
    assert_eq!(1, DynamicAtlasPacker::next_power_of_two(0));
    assert_eq!(1, DynamicAtlasPacker::next_power_of_two(1));
    assert_eq!(2, DynamicAtlasPacker::next_power_of_two(2));
    assert_eq!(4, DynamicAtlasPacker::next_power_of_two(3));
    assert_eq!(256, DynamicAtlasPacker::next_power_of_two(150));
    assert_eq!(4096, DynamicAtlasPacker::next_power_of_two(3000));
    assert_eq!(4096, DynamicAtlasPacker::next_power_of_two(4096));
}

#[test]
fn heuristics_comparison_bssf_baf_blsf() {
    let mut packer_bssf = DynamicAtlasPacker::new(256, 1).unwrap();
    let mut packer_baf = DynamicAtlasPacker::new(256, 1).unwrap();
    let mut packer_blsf = DynamicAtlasPacker::new(256, 1).unwrap();

    let items = [(32, 64), (64, 32), (16, 128), (48, 48)];
    for &(w, h) in &items {
        assert!(packer_bssf
            .try_insert(w, h, MaxRectsHeuristic::BestShortSideFit)
            .is_some());
        assert!(packer_baf
            .try_insert(w, h, MaxRectsHeuristic::BestAreaFit)
            .is_some());
        assert!(packer_blsf
            .try_insert(w, h, MaxRectsHeuristic::BestLongSideFit)
            .is_some());
    }

    assert_eq!(4, packer_bssf.placed_count());
    assert_eq!(4, packer_baf.placed_count());
    assert_eq!(4, packer_blsf.placed_count());
}

#[test]
fn invalid_atlas_construction_and_nonpositive_sizes_handled() {
    assert!(DynamicAtlasPacker::new(15, 1).is_err());
    assert!(DynamicAtlasPacker::new(64, -1).is_err());

    let mut packer = DynamicAtlasPacker::new(64, 1).unwrap();
    assert!(packer
        .try_insert(0, 10, MaxRectsHeuristic::BestShortSideFit)
        .is_none());
    assert!(packer
        .try_insert(10, 0, MaxRectsHeuristic::BestShortSideFit)
        .is_none());
    assert!(packer
        .try_insert(-5, 10, MaxRectsHeuristic::BestShortSideFit)
        .is_none());
    assert!(packer
        .try_insert(100, 10, MaxRectsHeuristic::BestShortSideFit)
        .is_none());
}

#[test]
fn atlas_rect_methods_work_as_expected() {
    let r1 = AtlasRect::new(10, 10, 20, 20);
    assert_eq!(30, r1.right());
    assert_eq!(30, r1.bottom());
    assert_eq!(400, r1.area());

    let r2 = AtlasRect::new(12, 12, 5, 5);
    assert!(r1.contains(r2));
    assert!(!r2.contains(r1));

    let r3 = AtlasRect::new(25, 25, 20, 20);
    assert!(r1.intersects(r3));
    assert!(r3.intersects(r1));

    let r4 = AtlasRect::new(35, 35, 10, 10);
    assert!(!r1.intersects(r4));
}
