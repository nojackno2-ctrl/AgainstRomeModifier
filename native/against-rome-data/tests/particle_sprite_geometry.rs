use against_rome_data::particle_frames::{particle_sprite_rect, ParticleSpriteRect};

#[test]
fn geometry_ffi_writes_four_edges_atomically() {
    use against_rome_data::particle_frames::ar_particle_sprite_rect;
    let mut edges = [123; 4];
    unsafe {
        assert_eq!(ar_particle_sprite_rect(11, 9, -3, 7, 4, 2, 8, edges.as_mut_ptr()), 0);
        assert_eq!(edges, [-8, 3, -4, 5]);
        assert_eq!(ar_particle_sprite_rect(11, 9, i32::MIN, 7, 4, 2, 8, edges.as_mut_ptr()), 253);
        assert_eq!(edges, [-8, 3, -4, 5]);
        assert_eq!(ar_particle_sprite_rect(11, 9, -3, 7, 4, 2, 8, std::ptr::null_mut()), 1);
    }
}

#[test]
fn uncropped_offset_frame_preserves_centre_and_extent() {
    assert_eq!(particle_sprite_rect(11, 9, -3, 7, 4, 2, 8), Some(ParticleSpriteRect {
        left: -8, top: 3, right: -4, bottom: 5,
    }));
    // The frame extends beyond its canvas; it must not be clipped to it.
    assert_eq!(particle_sprite_rect(2, 2, 3, -5, 4, 2, 8), Some(ParticleSpriteRect {
        left: 2, top: -6, right: 6, bottom: -4,
    }));
}

#[test]
fn invalid_images_and_overflow_do_not_create_geometry() {
    for (aw, ah, x, y, w, h, pixels) in [
        (0, 4, 0, 0, 2, 2, 4), (4, 0, 0, 0, 2, 2, 4),
        (u32::MAX, 4, 0, 0, 2, 2, 4), (4, u32::MAX, 0, 0, 2, 2, 4),
        (4, 4, 0, 0, 0, 2, 0), (4, 4, 0, 0, 2, -1, 0),
        (4, 4, 0, 0, 2, 2, 3), (4, 4, 0, 0, 2, 2, 5),
        (4, 4, i32::MIN, 0, 2, 2, 4), (4, 4, 0, i32::MIN, 2, 2, 4),
        (2, 2, i32::MAX, 0, 3, 2, 6), (2, 2, 0, i32::MAX, 2, 3, 6),
    ] {
        assert_eq!(particle_sprite_rect(aw, ah, x, y, w, h, pixels), None);
    }
}

#[test]
fn translating_stored_offset_translates_all_edges_without_resizing() {
    for x in -8..8 {
        for y in -8..8 {
            let a = particle_sprite_rect(17, 13, x, y, 7, 3, 21).unwrap();
            let b = particle_sprite_rect(17, 13, x + 5, y - 2, 7, 3, 21).unwrap();
            assert_eq!((b.left - a.left, b.right - a.right), (5, 5));
            assert_eq!((b.top - a.top, b.bottom - a.bottom), (-2, -2));
            assert_eq!((b.right - b.left, b.bottom - b.top), (7, 3));
        }
    }
}
