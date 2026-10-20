use against_rome_scene::{camera::TerrainRay, ray_picker::TerrainRayPicker, terrain::TerrainHeightField, Vec3};

#[test]
fn ray_picker_clips_to_map_bounds() {
    let samples = vec![10u8; 16];
    let field = TerrainHeightField::new(4, 4, samples, 6.0, 4.0, 4.0, Some(4.0)).unwrap();

    let ray_origin = Vec3::new(2.0, 20.0, -10.0);
    let ray_direction = Vec3::new(0.0, -1.0, 1.0).normalized();
    let clip = TerrainRayPicker::clip_to_map(&field, ray_origin, ray_direction);
    assert!(clip.is_some());
    let (start, end) = clip.unwrap();
    assert!(start >= 0.0);
    assert!(end >= start);
}

#[test]
fn ray_picker_hits_flat_terrain_with_bisection_accuracy() {
    // 4x4 height samples, raw byte = 10, scale = 6.0. Flat height = 10 / 255 * 6.0
    let samples = vec![10u8; 16];
    let field = TerrainHeightField::new(4, 4, samples, 6.0, 4.0, 4.0, Some(4.0)).unwrap();
    let expected_height = field.sample_height(2.0, 2.0);

    let ray = TerrainRay {
        origin: Vec3::new(2.0, 50.0, 2.0),
        direction: Vec3::new(0.0, -1.0, 0.0),
    };

    let hit = TerrainRayPicker::try_pick_point(&field, ray);
    assert!(hit.is_some());
    let pt = hit.unwrap();
    assert!((pt.x - 2.0).abs() < 1e-3);
    assert!((pt.z - 2.0).abs() < 1e-3);
    assert!((pt.y - expected_height).abs() < 1e-3);

    let tile = TerrainRayPicker::try_pick(&field, ray);
    assert_eq!(tile, Some((2, 2)));
}

#[test]
fn ray_picker_hits_sloped_terrain_and_returns_correct_tile() {
    // Sloping terrain: samples increasing along X
    let mut samples = vec![0u8; 16];
    for y in 0..4 {
        for x in 0..4 {
            samples[y * 4 + x] = (x * 60) as u8;
        }
    }
    let field = TerrainHeightField::new(4, 4, samples, 6.0, 4.0, 4.0, Some(4.0)).unwrap();

    // Ray angled down towards (1.5, 0, 1.5)
    let origin = Vec3::new(1.5, 30.0, -5.0);
    let target = Vec3::new(1.5, 0.0, 1.5);
    let ray = TerrainRay {
        origin,
        direction: (target - origin).normalized(),
    };

    let hit = TerrainRayPicker::try_pick_point(&field, ray);
    assert!(hit.is_some());
    let pt = hit.unwrap();
    let expected_y = field.sample_height(pt.x, pt.z);
    assert!((pt.y - expected_y).abs() < 0.01);

    let tile = TerrainRayPicker::try_pick(&field, ray);
    assert!(tile.is_some());
    let (tx, ty) = tile.unwrap();
    assert!(tx >= 0 && tx < 4);
    assert!(ty >= 0 && ty < 4);
}

#[test]
fn ray_picker_misses_when_aimed_away() {
    let samples = vec![10u8; 16];
    let field = TerrainHeightField::new(4, 4, samples, 6.0, 4.0, 4.0, Some(4.0)).unwrap();

    // Ray shooting upwards away from map
    let ray = TerrainRay {
        origin: Vec3::new(2.0, 50.0, 2.0),
        direction: Vec3::new(0.0, 1.0, 0.0),
    };

    assert!(TerrainRayPicker::try_pick_point(&field, ray).is_none());
    assert!(TerrainRayPicker::try_pick(&field, ray).is_none());
}

#[test]
fn ray_picker_handles_degenerate_direction() {
    let samples = vec![10u8; 16];
    let field = TerrainHeightField::new(4, 4, samples, 6.0, 4.0, 4.0, Some(4.0)).unwrap();

    let ray = TerrainRay {
        origin: Vec3::new(2.0, 50.0, 2.0),
        direction: Vec3::ZERO,
    };

    assert!(TerrainRayPicker::try_pick_point(&field, ray).is_none());
    assert!(TerrainRayPicker::try_pick(&field, ray).is_none());
}
