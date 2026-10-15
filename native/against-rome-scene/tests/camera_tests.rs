use against_rome_scene::{EditorCamera, Vec3};

#[test]
fn camera_defaults_and_initial_matrices() {
    let cam = EditorCamera::default();
    assert_eq!(Vec3::new(32.0, 0.0, 32.0), cam.target);
    assert_eq!(45.0, cam.yaw_degrees());
    assert_eq!(30.0, cam.pitch_degrees());
    assert_eq!(82.0, cam.distance());
    assert!(cam.orthographic);

    let view = cam.view_matrix();
    let proj = cam.projection_matrix(800.0 / 600.0);

    // View matrix should not be identity and must have valid finite values
    for row in view.0 {
        for val in row {
            assert!(val.is_finite());
        }
    }
    for row in proj.0 {
        for val in row {
            assert!(val.is_finite());
        }
    }
}

#[test]
fn camera_rotation_and_pitch_clamping() {
    let mut cam = EditorCamera::default();
    cam.rotate(15.0, 20.0);
    assert_eq!(60.0, cam.yaw_degrees());
    assert_eq!(50.0, cam.pitch_degrees());

    // Pitch should clamp to 80 max
    cam.rotate(0.0, 100.0);
    assert_eq!(80.0, cam.pitch_degrees());

    // Pitch should clamp to 20 min
    cam.rotate(0.0, -100.0);
    assert_eq!(20.0, cam.pitch_degrees());
}

#[test]
fn camera_zoom_and_distance_clamping() {
    let mut cam = EditorCamera::default();
    cam.zoom(2.0);
    assert_eq!(164.0, cam.distance());

    // Should clamp to max_distance (180)
    cam.zoom(2.0);
    assert_eq!(180.0, cam.distance());

    // Should clamp to min_distance (2)
    cam.zoom(0.001);
    assert_eq!(2.0, cam.distance());
}

#[test]
fn camera_zoom_to_game_scale() {
    let mut cam = EditorCamera::default();
    cam.zoom_to_game_scale(768);

    // Verify distance is clamped and reasonable
    assert!(cam.distance() >= cam.min_distance);
    assert!(cam.distance() <= cam.max_distance);
}

#[test]
fn camera_pan_moves_target_in_camera_plane() {
    let mut cam = EditorCamera::default();
    let old_target = cam.target;
    cam.pan(10.0, 15.0);

    assert_ne!(old_target, cam.target);
    assert_eq!(0.0, cam.target.y); // Horizontal plane panning keeps Y unchanged
}

#[test]
fn camera_projection_orthographic_and_perspective() {
    let mut cam = EditorCamera::default();
    let ortho = cam.projection_matrix(1.5);
    // Orthographic: M44 is 1.0, row 3 has no perspective divide
    assert_eq!(1.0, ortho.0[3][3]);

    cam.orthographic = false;
    let persp = cam.projection_matrix(1.5);
    // Perspective: M34 is -1.0, M44 is 0.0
    assert_eq!(-1.0, persp.0[2][3]);
    assert_eq!(0.0, persp.0[3][3]);
}

#[test]
fn camera_create_ray_unprojects_screen_point() {
    let cam = EditorCamera::default();
    let ray = cam.create_ray(400.0, 300.0, 800.0, 600.0);

    assert!(ray.origin.x.is_finite());
    assert!(ray.origin.y.is_finite());
    assert!(ray.origin.z.is_finite());

    assert!(ray.direction.x.is_finite());
    assert!(ray.direction.y.is_finite());
    assert!(ray.direction.z.is_finite());

    // Direction should be normalized
    assert!((ray.direction.length() - 1.0).abs() < 1e-4);
}

#[test]
fn camera_degenerate_viewport_handling() {
    let cam = EditorCamera::default();
    let ray = cam.create_ray(0.0, 0.0, 0.0, 0.0);
    assert_eq!(cam.target, ray.origin);
}
