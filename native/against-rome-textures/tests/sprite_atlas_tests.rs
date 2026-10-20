use against_rome_textures::{
    apt_finished_frame, decode_animation_frames, direction_for_angle, BoundedSpriteCatalog,
    NativeSprite, NativeSpriteAnimation, NativeSpriteAtlas,
};
use std::sync::Arc;

fn make_sprite(width: i32, height: i32, base_color: u32, name: &str) -> Arc<NativeSprite> {
    let count = (width * height) as usize;
    let mut pixels = Vec::with_capacity(count);
    for i in 0..count {
        pixels.push(base_color.wrapping_add((i as u32).wrapping_mul(0x100)));
    }
    Arc::new(NativeSprite::new(width, height, pixels, 0, 0, name).unwrap())
}

#[test]
fn packs_distinct_sprites_with_gutters_and_exact_uv_rectangles() {
    let tall = make_sprite(3, 5, 0xFF000001, "tall");
    let wide = make_sprite(6, 2, 0xFF000002, "wide");
    let same = make_sprite(3, 5, 0xFF000001, "same");

    let atlas = NativeSpriteAtlas::pack(
        &[wide.clone(), tall.clone(), tall.clone(), same.clone()],
        16,
    )
    .expect("Packing atlas");

    // Reference identity: value-equal records stay separate entries
    assert_eq!(3, atlas.count());
    assert_eq!(16, atlas.width());
    assert_eq!(16, atlas.height());

    for sprite in [&tall, &wide, &same] {
        let uv = atlas.try_get_uv(sprite).expect("UV must exist for sprite");
        let left = (uv.u0 * atlas.width() as f32).round() as i32;
        let top = (uv.v0 * atlas.height() as f32).round() as i32;

        assert_eq!(
            sprite.width,
            (uv.u1 * atlas.width() as f32).round() as i32 - left
        );
        assert_eq!(
            sprite.height,
            (uv.v1 * atlas.height() as f32).round() as i32 - top
        );

        let w = atlas.width() as usize;
        for y in 0..sprite.height {
            for x in 0..sprite.width {
                let expected = sprite.argb_pixels[(y * sprite.width + x) as usize];
                let actual = atlas.argb_pixels()[((top + y) as usize) * w + ((left + x) as usize)];
                assert_eq!(expected, actual);
            }
        }
        // Transparent gutter above
        assert_eq!(0, atlas.argb_pixels()[((top - 1) as usize) * w + (left as usize)]);
        // Transparent gutter to the left
        assert_eq!(0, atlas.argb_pixels()[(top as usize) * w + ((left - 1) as usize)]);
    }
}

#[test]
fn oversized_and_overflowing_sprites_are_omitted_instead_of_failing() {
    let huge = make_sprite(20, 2, 1, "huge");
    let narrow = make_sprite(2, 2, 9, "narrow");
    let blocks: Vec<Arc<NativeSprite>> = (0..5)
        .map(|i| make_sprite(6, 6, i as u32 + 2, &format!("block_{i}")))
        .collect();

    let mut all = vec![huge.clone()];
    all.extend(blocks.clone());
    all.push(narrow.clone());

    let atlas = NativeSpriteAtlas::pack(&all, 16).expect("Pack atlas");

    assert!(atlas.try_get_uv(&huge).is_none());
    let placed_blocks = blocks
        .iter()
        .filter(|b| atlas.try_get_uv(b).is_some())
        .count();
    assert_eq!(4, placed_blocks); // two 8px cells per shelf, two shelves fit 4
    assert!(atlas.try_get_uv(&narrow).is_none());
    assert_eq!(16, atlas.width());
    assert_eq!(16, atlas.height());

    assert!(NativeSpriteAtlas::pack(&[], 0).is_err());
    assert_eq!(0, NativeSpriteAtlas::pack(&[], 16).unwrap().count());
}

#[test]
fn frame_selection_wraps_the_exact_cycle_without_rounding_drift() {
    let frames: Vec<Arc<NativeSprite>> = (0..24)
        .map(|i| make_sprite(10, 10, 0xFF000000 + i, &format!("f_{i}")))
        .collect();
    let anim = NativeSpriteAnimation::new(frames, 1000).expect("Valid animation");

    let oracle_cases: &[(f64, usize)] = &[
        (0.0, 0),
        (41.0, 0),
        (42.0, 1),
        (500.0, 12),
        (999.0, 23),
        (1000.0, 0),
        (1500.0, 12),
        (-1.0, 0),
        (f64::NAN, 0),
        (f64::INFINITY, 0),
        (f64::NEG_INFINITY, 0),
    ];

    for &(elapsed, expected) in oracle_cases {
        assert_eq!(
            expected,
            anim.get_frame_index(elapsed),
            "Failed at elapsed_ms = {elapsed}"
        );
    }
}

#[test]
fn atlas_preserves_stills_and_rolls_back_whole_sequences_that_do_not_fit() {
    let sprite = |name: &str| -> Arc<NativeSprite> {
        let pixels = vec![0xFF00FF00; 900];
        Arc::new(NativeSprite::new(30, 30, pixels, 15, 30, name).unwrap())
    };

    let a = sprite("a");
    let b = sprite("b");
    let c = sprite("c");
    let d = sprite("d");
    let e = sprite("e");

    let too_large = NativeSpriteAnimation::new(
        vec![a.clone(), c.clone(), d.clone(), e.clone()],
        1000,
    )
    .unwrap();
    let small = NativeSpriteAnimation::new(vec![b.clone(), c.clone()], 1000).unwrap();

    let atlas = NativeSpriteAtlas::pack_animations(
        &[a.clone(), b.clone()],
        &[too_large, small],
        64,
    )
    .expect("Pack animations");

    assert!(atlas.try_get_uv(&a).is_some());
    assert!(atlas.try_get_uv(&b).is_some());
    assert!(atlas.try_get_uv(&c).is_some());
    assert!(atlas.try_get_uv(&d).is_none());
    assert_eq!(3, atlas.count());
    assert!(atlas.width() >= 1 && atlas.width() <= 64);
    assert!(atlas.height() >= 1 && atlas.height() <= 64);
}

#[test]
fn crop_trims_transparent_margins_and_adjusts_anchors() {
    // 8x8 image with 3x4 opaque rectangle at (2, 2)
    let mut pixels = vec![0u32; 64];
    for y in 2..6 {
        for x in 2..5 {
            pixels[y * 8 + x] = 0xFFFFFFFF;
        }
    }

    let cropped = NativeSprite::crop("test_crop", 8, 8, &pixels, 4, 4)
        .expect("Should crop non-empty image");
    assert_eq!(3, cropped.width);
    assert_eq!(4, cropped.height);
    assert_eq!(4 - 2, cropped.anchor_x);
    assert_eq!(4 - 2, cropped.anchor_y);
    assert_eq!(12, cropped.argb_pixels.len());

    // Completely transparent image returns None
    let empty = vec![0u32; 64];
    assert!(NativeSprite::crop("empty", 8, 8, &empty, 0, 0).is_none());
}

#[test]
fn direction_for_angle_and_apt_finished_frame_oracles() {
    assert_eq!(14, direction_for_angle(0.0, 16));
    assert_eq!(12, direction_for_angle(45.0, 16));
    assert_eq!(10, direction_for_angle(90.0, 16));
    assert_eq!(8, direction_for_angle(135.0, 16));
    assert_eq!(6, direction_for_angle(180.0, 16));
    assert_eq!(0, direction_for_angle(f32::NAN, 16));
    assert_eq!(0, direction_for_angle(45.0, 1));

    // APT finished frame
    // 5 * 2 * 5 * 24 = 1200 frames total. Finished frame = 4 * 240 = 960.
    assert_eq!(960, apt_finished_frame(&[5, 2, 5, 24], 1200));
    assert_eq!(750, apt_finished_frame(&[4, 2, 5, 25], 1000));
    assert_eq!(2, apt_finished_frame(&[3], 30));
    assert_eq!(20, apt_finished_frame(&[3, 10], 30));
    assert_eq!(0, apt_finished_frame(&[], 10));
}

#[test]
fn direction_for_angle_ties_to_even_and_boundary_cases() {
    // 2 rows: 90 degrees / 360 * 2 = 0.5 (tie).
    // Ties-to-even: 0.5 rounds to 0.0, step = 0.
    // row = 14 * 2 / 16 - 0 = 1 - 0 = 1.
    // Away-from-zero would round to 1.0, step = 1, row = 0 (the bug reported by C# oracle).
    assert_eq!(1, direction_for_angle(90.0, 2));
    assert_eq!(1, direction_for_angle(270.0, 2)); // 1.5 -> rounds to 2, row = 1 - 2 = -1 -> 1
    assert_eq!(1, direction_for_angle(450.0, 2)); // 2.5 -> rounds to 2, row = 1 - 2 = -1 -> 1
    assert_eq!(1, direction_for_angle(630.0, 2)); // 3.5 -> rounds to 4, row = 1 - 4 = -3 -> 1

    // 2 rows negative angle ties:
    // -90 / 360 * 2 = -0.5 -> rounds to 0.0, step = 0 -> row 1
    assert_eq!(1, direction_for_angle(-90.0, 2));
    // -270 / 360 * 2 = -1.5 -> rounds to -2.0, step = -2 -> row = 1 - (-2) = 3 -> 1
    assert_eq!(1, direction_for_angle(-270.0, 2));
    // -450 / 360 * 2 = -2.5 -> rounds to -2.0, step = -2 -> row = 1 - (-2) = 3 -> 1
    assert_eq!(1, direction_for_angle(-450.0, 2));
    // -630 / 360 * 2 = -3.5 -> rounds to -4.0, step = -4 -> row = 1 - (-4) = 5 -> 1
    assert_eq!(1, direction_for_angle(-630.0, 2));

    // 4 rows ties:
    // 45 / 360 * 4 = 0.5 -> 0, row = 3 - 0 = 3
    assert_eq!(3, direction_for_angle(45.0, 4));
    // 135 / 360 * 4 = 1.5 -> 2, row = 3 - 2 = 1
    assert_eq!(1, direction_for_angle(135.0, 4));
    // 225 / 360 * 4 = 2.5 -> 2, row = 3 - 2 = 1
    assert_eq!(1, direction_for_angle(225.0, 4));
    // 315 / 360 * 4 = 3.5 -> 4, row = 3 - 4 = -1 -> 3
    assert_eq!(3, direction_for_angle(315.0, 4));

    // 4 rows negative ties:
    // -45 / 360 * 4 = -0.5 -> 0, row = 3 - 0 = 3
    assert_eq!(3, direction_for_angle(-45.0, 4));
    // -135 / 360 * 4 = -1.5 -> -2, row = 3 - (-2) = 5 -> 1
    assert_eq!(1, direction_for_angle(-135.0, 4));
    // -225 / 360 * 4 = -2.5 -> -2, row = 3 - (-2) = 5 -> 1
    assert_eq!(1, direction_for_angle(-225.0, 4));
    // -315 / 360 * 4 = -3.5 -> -4, row = 3 - (-4) = 7 -> 3
    assert_eq!(3, direction_for_angle(-315.0, 4));

    // 16 rows ties:
    // 11.25 / 360 * 16 = 0.5 -> 0, row = 14
    assert_eq!(14, direction_for_angle(11.25, 16));
    // 33.75 / 360 * 16 = 1.5 -> 2, row = 12
    assert_eq!(12, direction_for_angle(33.75, 16));
    // 56.25 / 360 * 16 = 2.5 -> 2, row = 12
    assert_eq!(12, direction_for_angle(56.25, 16));
    // 78.75 / 360 * 16 = 3.5 -> 4, row = 10
    assert_eq!(10, direction_for_angle(78.75, 16));
    // Negative 16 rows ties:
    assert_eq!(14, direction_for_angle(-11.25, 16));
    assert_eq!(0, direction_for_angle(-33.75, 16)); // row 14 - (-2) = 16 -> 0
    assert_eq!(0, direction_for_angle(-56.25, 16)); // row 14 - (-2) = 16 -> 0
    assert_eq!(2, direction_for_angle(-78.75, 16)); // row 14 - (-4) = 18 -> 2

    // Positive / negative angle wrapping and boundary cases:
    assert_eq!(14, direction_for_angle(0.0, 16));
    assert_eq!(14, direction_for_angle(360.0, 16));
    assert_eq!(14, direction_for_angle(-360.0, 16));
    assert_eq!(14, direction_for_angle(720.0, 16));
    assert_eq!(14, direction_for_angle(-720.0, 16));
    assert_eq!(14, direction_for_angle(3600.0, 16));
    assert_eq!(14, direction_for_angle(-3600.0, 16));
    assert_eq!(0, direction_for_angle(-45.0, 16));
    assert_eq!(2, direction_for_angle(-90.0, 16));
    assert_eq!(6, direction_for_angle(-180.0, 16));
    assert_eq!(10, direction_for_angle(-270.0, 16));

    // Non-finite and invalid rows:
    assert_eq!(0, direction_for_angle(f32::NAN, 16));
    assert_eq!(0, direction_for_angle(f32::INFINITY, 16));
    assert_eq!(0, direction_for_angle(f32::NEG_INFINITY, 16));
    assert_eq!(0, direction_for_angle(90.0, 1));
    assert_eq!(0, direction_for_angle(90.0, 0));
    assert_eq!(0, direction_for_angle(90.0, -1));
    assert_eq!(0, direction_for_angle(90.0, -16));
}

#[test]
fn bounded_catalog_enforces_capacity_and_evicts_lru() {
    let mut catalog = BoundedSpriteCatalog::new(3, 2);

    let s1 = make_sprite(4, 4, 1, "s1");
    let s2 = make_sprite(4, 4, 2, "s2");
    let s3 = make_sprite(4, 4, 3, "s3");
    let s4 = make_sprite(4, 4, 4, "s4");

    catalog.insert_sprite("s1", s1.clone());
    catalog.insert_sprite("s2", s2.clone());
    catalog.insert_sprite("s3", s3.clone());
    assert_eq!(3, catalog.cached_sprite_count());

    // Inserting s4 should evict s1 (oldest)
    catalog.insert_sprite("s4", s4.clone());
    assert_eq!(3, catalog.cached_sprite_count());
    assert!(catalog.get_sprite("s1").is_none());
    assert!(catalog.get_sprite("s2").is_some());
    assert!(catalog.get_sprite("s3").is_some());
    assert!(catalog.get_sprite("s4").is_some());

    // Test animation bounds
    let a1 = Arc::new(NativeSpriteAnimation::new(vec![s2.clone(), s3.clone()], 500).unwrap());
    let a2 = Arc::new(NativeSpriteAnimation::new(vec![s3.clone(), s4.clone()], 500).unwrap());
    let a3 = Arc::new(NativeSpriteAnimation::new(vec![s2.clone(), s4.clone()], 500).unwrap());

    catalog.insert_animation("a1", a1.clone());
    catalog.insert_animation("a2", a2.clone());
    assert_eq!(2, catalog.cached_animation_count());

    // Inserting a3 should evict a1
    catalog.insert_animation("a3", a3.clone());
    assert_eq!(2, catalog.cached_animation_count());
    assert!(catalog.get_animation("a1").is_none());
    assert!(catalog.get_animation("a2").is_some());
    assert!(catalog.get_animation("a3").is_some());

    let atlas = catalog.build_atlas(64).expect("Build atlas from catalog");
    assert!(atlas.count() >= 3);
}

#[test]
fn decode_animation_frames_requires_valid_still_and_all_frames() {
    let s1 = make_sprite(4, 4, 1, "s1");
    let s2 = make_sprite(4, 4, 2, "s2");

    // Missing still -> None
    assert!(decode_animation_frames(2, 500, None, |_| Some(s2.clone())).is_none());

    // Count < 2 -> None
    assert!(decode_animation_frames(1, 500, Some(s1.clone()), |_| Some(s2.clone())).is_none());

    // Cycle <= 1 -> None
    assert!(decode_animation_frames(2, 1, Some(s1.clone()), |_| Some(s2.clone())).is_none());

    // Failure in decode closure -> None
    assert!(decode_animation_frames(3, 500, Some(s1.clone()), |i| {
        if i == 1 {
            Some(s2.clone())
        } else {
            None
        }
    })
    .is_none());

    // Success
    let anim = decode_animation_frames(2, 500, Some(s1.clone()), |_| Some(s2.clone()))
        .expect("Decoded animation");
    assert_eq!(2, anim.frames().len());
    assert_eq!(500, anim.cycle_duration_ms());
}
