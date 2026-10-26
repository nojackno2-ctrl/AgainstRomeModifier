use against_rome_scene::{
    picking::{pick_object, project_eye_to_screen, PickableSprite},
    scene::SceneObjectPosition,
    terrain::TerrainHeightField,
    Mat4, Vec2, Vec3,
};

#[test]
fn project_eye_to_screen_transforms_correctly() {
    let proj = Mat4::orthographic(100.0, 100.0, 1.0, 1000.0);
    let viewport = Vec2::new(800.0, 600.0);

    // Eye origin (0, 0, -10) should project to screen center (400, 300)
    let screen = project_eye_to_screen([0.0, 0.0, -10.0, 1.0], proj, viewport);
    assert!((screen.x - 400.0).abs() < 1e-3);
    assert!((screen.y - 300.0).abs() < 1e-3);
}

#[test]
fn pick_object_hits_opaque_pixel_and_ignores_transparent() {
    let samples = vec![0u8; 16];
    let heights = TerrainHeightField::new(4, 4, samples, 6.0, 4.0, 4.0, Some(4.0)).unwrap();

    let view = Mat4::look_at(Vec3::new(2.0, 20.0, 10.0), Vec3::new(2.0, 0.0, 2.0));
    let proj = Mat4::orthographic(20.0, 20.0, 1.0, 100.0);
    let viewport = Vec2::new(800.0, 600.0);

    let obj = SceneObjectPosition {
        world: Vec3::new(2.0 * 256.0, 0.0, 2.0 * 256.0),
        local_x: 0.0,
        local_z: 0.0,
        has_native_saved_position: false,
    };

    // 2x2 sprite: top-left opaque (0xFFFFFFFF), bottom-right transparent (0x00000000)
    let pixels = [
        0xFFFFFFFF, 0x00000000,
        0x00000000, 0xFFFFFFFF,
    ];
    let sprite = PickableSprite::new(2, 2, 1.0, 1.0, Some(&pixels));

    // Screen position of the anchor
    let anchor_screen = project_eye_to_screen(
        view.transform4([2.0, heights.sample_height(2.0, 2.0), 2.0, 1.0]),
        proj,
        viewport,
    );

    // Click near anchor should hit the sprite
    let hit = pick_object(
        &[obj],
        &[Some(sprite.clone())],
        |_, _| true,
        &heights,
        view,
        proj,
        viewport,
        anchor_screen,
        8.0,
        |_| true,
    );
    assert_eq!(hit, Some(0));

    // Click far away should miss
    let miss = pick_object(
        &[obj],
        &[Some(sprite)],
        |_, _| true,
        &heights,
        view,
        proj,
        viewport,
        Vec2::new(10.0, 10.0),
        8.0,
        |_| true,
    );
    assert_eq!(miss, None);
}

#[test]
fn pick_object_prefers_nearer_object_on_overlap() {
    let samples = vec![0u8; 16];
    let heights = TerrainHeightField::new(4, 4, samples, 6.0, 4.0, 4.0, Some(4.0)).unwrap();

    let eye = Vec3::new(2.0, 20.0, 20.0);
    let target = Vec3::new(2.0, 0.0, 2.0);
    let view = Mat4::look_at(eye, target);
    let proj = Mat4::orthographic(20.0, 20.0, 1.0, 100.0);
    let viewport = Vec2::new(800.0, 600.0);

    // Far object at (2.0, 0, 1.0)
    let far_obj = SceneObjectPosition {
        world: Vec3::new(2.0 * 256.0, 0.0, 1.0 * 256.0),
        local_x: 0.0,
        local_z: 0.0,
        has_native_saved_position: false,
    };
    // Near object at (2.0, 0, 3.0)
    let near_obj = SceneObjectPosition {
        world: Vec3::new(2.0 * 256.0, 0.0, 3.0 * 256.0),
        local_x: 0.0,
        local_z: 0.0,
        has_native_saved_position: false,
    };

    let pixels = [0xFFFFFFFF; 16];
    let sprite = PickableSprite::new(4, 4, 2.0, 2.0, Some(&pixels));

    // Click on the near object's anchor
    let near_anchor = Vec3::new(2.0, heights.sample_height(2.0, 3.0), 3.0);
    let near_screen = project_eye_to_screen(
        view.transform4([near_anchor.x, near_anchor.y, near_anchor.z, 1.0]),
        proj,
        viewport,
    );

    let picked = pick_object(
        &[far_obj, near_obj],
        &[Some(sprite.clone()), Some(sprite)],
        |_, _| true,
        &heights,
        view,
        proj,
        viewport,
        near_screen,
        8.0,
        |_| true,
    );
    assert_eq!(picked, Some(1));
}

#[test]
fn pick_object_falls_back_to_marker_when_no_sprite() {
    let samples = vec![0u8; 16];
    let heights = TerrainHeightField::new(4, 4, samples, 6.0, 4.0, 4.0, Some(4.0)).unwrap();

    let view = Mat4::look_at(Vec3::new(2.0, 20.0, 10.0), Vec3::new(2.0, 0.0, 2.0));
    let proj = Mat4::orthographic(20.0, 20.0, 1.0, 100.0);
    let viewport = Vec2::new(800.0, 600.0);

    let obj = SceneObjectPosition {
        world: Vec3::new(2.0 * 256.0, 0.0, 2.0 * 256.0),
        local_x: 0.0,
        local_z: 0.0,
        has_native_saved_position: false,
    };

    let anchor = Vec3::new(2.0, heights.sample_height(2.0, 2.0) + 0.25, 2.0);
    let screen = project_eye_to_screen(
        view.transform4([anchor.x, anchor.y, anchor.z, 1.0]),
        proj,
        viewport,
    );

    // Within marker radius 8.0
    let hit = pick_object(
        &[obj],
        &[None],
        |_, _| false,
        &heights,
        view,
        proj,
        viewport,
        screen + Vec2::new(4.0, 4.0),
        8.0,
        |_| true,
    );
    assert_eq!(hit, Some(0));

    // Beyond marker radius
    let miss = pick_object(
        &[obj],
        &[None],
        |_, _| false,
        &heights,
        view,
        proj,
        viewport,
        screen + Vec2::new(15.0, 15.0),
        8.0,
        |_| true,
    );
    assert_eq!(miss, None);
}
