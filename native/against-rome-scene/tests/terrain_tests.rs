use against_rome_scene::{
    build_terrain_mesh, TerrainHeightField, TerrainMeshData, Vec2,
};

#[test]
fn terrain_height_field_sampling_and_bilinear_interpolation() {
    let samples = vec![10, 70, 150, 250];
    let field = TerrainHeightField::new(2, 2, samples, 6.0, 2.0, 2.0, Some(4.0)).unwrap();

    assert_eq!(2, field.width());
    assert_eq!(2, field.height());

    // Corner samples
    assert_eq!(10.0, field.sample_raw(0.0, 0.0));
    assert_eq!(70.0, field.sample_raw(2.0, 0.0));
    assert_eq!(150.0, field.sample_raw(0.0, 2.0));
    assert_eq!(250.0, field.sample_raw(2.0, 2.0));

    // Midpoint: (10 + 70 + 150 + 250) / 4 = 120
    assert!((field.sample_raw(1.0, 1.0) - 120.0).abs() < 1e-4);

    // Height scaled sample: 120 / 255 * 6.0
    let expected_height = 120.0 / 255.0 * 6.0;
    assert!((field.sample_height(1.0, 1.0) - expected_height).abs() < 1e-4);
}

#[test]
fn terrain_height_field_native_queries() {
    let samples = vec![10, 70, 150, 250];
    let field = TerrainHeightField::new(2, 2, samples, 6.0, 2.0, 2.0, Some(4.0)).unwrap();

    assert!(field.has_native_query());
    assert_eq!(Some(40.0), field.minimum_native_height()); // min sample 10 * step 4.0

    let texels = field.build_native_height_texels().unwrap();
    assert_eq!(vec![40.0, 280.0, 600.0, 1000.0], texels);

    // Native query height at (0, 0) is 10 * 4 = 40.0
    let qh = field.native_query_height(0.0, 0.0).unwrap();
    assert_eq!(40.0, qh);

    // Native height scaled for preview: world_y / step / 255 * scale
    // e.g. 510.25 / 4.0 / 255.0 * 6.0 = 3.00147
    let nh = field.native_height(510.25).unwrap();
    let expected_nh = 510.25 / 4.0 / 255.0 * 6.0;
    assert!((nh - expected_nh).abs() < 1e-4);
}

#[test]
fn terrain_height_field_dimension_validation() {
    // Under 2x2
    assert!(TerrainHeightField::new(1, 1, vec![0], 6.0, 1.0, 1.0, None).is_err());
    // Mismatching length
    assert!(TerrainHeightField::new(2, 2, vec![0, 1, 2], 6.0, 2.0, 2.0, None).is_err());
    // Nonpositive tile width
    assert!(TerrainHeightField::new(2, 2, vec![0, 1, 2, 3], 6.0, 0.0, 2.0, None).is_err());
    // Nonfinite tile height
    assert!(TerrainHeightField::new(2, 2, vec![0, 1, 2, 3], 6.0, 2.0, f32::NAN, None).is_err());
}

#[test]
fn terrain_mesh_builder_geometry_and_ccw_indices() {
    let samples = vec![0; 9];
    let field = TerrainHeightField::new(3, 3, samples, 6.0, 2.0, 2.0, None).unwrap();

    let dimension = 2;
    let mesh: TerrainMeshData = build_terrain_mesh(&field, dimension, |_x, _y| {
        [
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ]
    })
    .unwrap();

    // 2x2 tiles = 4 tiles * 4 vertices = 16 vertices
    assert_eq!(16, mesh.vertices.len());
    // 4 tiles * 6 indices = 24 indices
    assert_eq!(24, mesh.indices.len());

    // Check CCW winding order for first tile:
    // [start, start+2, start+1, start, start+3, start+2] -> [0, 2, 1, 0, 3, 2]
    assert_eq!(&mesh.indices[0..6], &[0, 2, 1, 0, 3, 2]);

    // Flat terrain normals should point straight up (+Y)
    for vertex in &mesh.vertices {
        assert!((vertex.normal.x).abs() < 1e-4);
        assert!((vertex.normal.y - 1.0).abs() < 1e-4);
        assert!((vertex.normal.z).abs() < 1e-4);
    }
}

#[test]
fn terrain_mesh_builder_dimension_validation() {
    let field = TerrainHeightField::new(2, 2, vec![0; 4], 6.0, 1.0, 1.0, None).unwrap();
    assert!(build_terrain_mesh(&field, 0, |_, _| [Vec2::ZERO; 4]).is_err());
}
