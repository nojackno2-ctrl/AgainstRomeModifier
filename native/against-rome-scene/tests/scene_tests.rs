use against_rome_scene::{
    build_marker_points, build_shadow_vertices, build_sprite_vertices, combined_sprite_plan,
    ground_point, move_to_world_position, native_query_y, order_scene_draws, AtlasUv, Mat4,
    ParticleSpriteDraw, SceneCombinedSpritePlan, SceneObjectPosition, SceneShadowGeometryCache,
    SceneSpriteDrawKind, SceneSpriteGeometryBuffer, Shadow, Sprite, TerrainHeightField, Vec3,
    MAP_WORLD_SIZE, QUAD_FLOATS, UNKNOWN_NATIVE_QUERY_Y,
};

#[test]
fn scene_object_positioning_movement_and_clamping() {
    let source = SceneObjectPosition {
        world: Vec3::new(100.0, 50.0, 200.0),
        local_x: 10.0,
        local_z: 20.0,
        has_native_saved_position: false,
    };

    let moved = move_to_world_position(source, 150.0, 250.0).unwrap();
    assert_eq!(150.0, moved.world.x);
    assert_eq!(50.0, moved.world.y);
    assert_eq!(250.0, moved.world.z);
    assert_eq!(60.0, moved.local_x); // 10 + 150 - 100 = 60
    assert_eq!(70.0, moved.local_z); // 20 + 250 - 200 = 70

    // Clamping to [0, MAP_WORLD_SIZE]
    let clamped = move_to_world_position(source, -100.0, MAP_WORLD_SIZE + 500.0).unwrap();
    assert_eq!(0.0, clamped.world.x);
    assert_eq!(MAP_WORLD_SIZE, clamped.world.z);

    // Nonfinite rejects
    assert!(move_to_world_position(source, f32::NAN, 100.0).is_err());
    assert!(move_to_world_position(source, 100.0, f32::INFINITY).is_err());
}

#[test]
fn ground_point_and_native_elevation() {
    let samples = vec![10, 70, 150, 250];
    let heights = TerrainHeightField::new(2, 2, samples, 6.0, 2.0, 2.0, Some(4.0)).unwrap();

    // Standard object: samples terrain height
    let standard = SceneObjectPosition {
        world: Vec3::new(256.0, 0.0, 256.0), // (1, 1) in tile units
        local_x: 0.0,
        local_z: 0.0,
        has_native_saved_position: false,
    };
    let pt = ground_point(standard, &heights, 0.25);
    assert_eq!(1.0, pt.x);
    assert_eq!(1.0, pt.z);
    let expected_y = heights.sample_height(1.0, 1.0) + 0.25;
    assert!((pt.y - expected_y).abs() < 1e-4);

    // Native object with saved position: uses native height calculation
    let native = SceneObjectPosition {
        world: Vec3::new(256.0, 510.25, 256.0),
        local_x: 0.0,
        local_z: 0.0,
        has_native_saved_position: true,
    };
    let native_pt = ground_point(native, &heights, 0.0);
    let expected_native_y = heights.native_height(510.25).unwrap();
    assert!((native_pt.y - expected_native_y).abs() < 1e-4);
}

#[test]
fn native_query_y_sentinel() {
    let samples = vec![10, 70, 150, 250];
    let heights = TerrainHeightField::new(2, 2, samples, 6.0, 2.0, 2.0, Some(4.0)).unwrap();

    let native = SceneObjectPosition {
        world: Vec3::new(256.0, 123.0, 256.0),
        local_x: 0.0,
        local_z: 0.0,
        has_native_saved_position: true,
    };
    assert_eq!(123.0, native_query_y(native, &heights));

    let standard = SceneObjectPosition {
        world: Vec3::new(0.0, 0.0, 0.0),
        local_x: 0.0,
        local_z: 0.0,
        has_native_saved_position: false,
    };
    assert_eq!(40.0, native_query_y(standard, &heights)); // 10 * 4.0 = 40.0

    // Without native step, returns sentinel
    let no_step = TerrainHeightField::new(2, 2, vec![0; 4], 6.0, 2.0, 2.0, None).unwrap();
    assert_eq!(UNKNOWN_NATIVE_QUERY_Y, native_query_y(standard, &no_step));
}

#[test]
fn scene_sprite_geometry_buffer_generation_and_sorting() {
    let heights = TerrainHeightField::new(2, 2, vec![50; 4], 6.0, 2.0, 2.0, None).unwrap();
    let view = Mat4::look_at(Vec3::new(0.0, 10.0, 0.0), Vec3::new(32.0, 0.0, 32.0));

    let obj1 = SceneObjectPosition {
        world: Vec3::new(2560.0, 0.0, 2560.0), // Near
        local_x: 0.0,
        local_z: 0.0,
        has_native_saved_position: false,
    };
    let obj2 = SceneObjectPosition {
        world: Vec3::new(12800.0, 0.0, 12800.0), // Far
        local_x: 0.0,
        local_z: 0.0,
        has_native_saved_position: false,
    };
    let objects = [obj1, obj2];

    let sprite1 = Sprite {
        width: 32.0,
        height: 32.0,
        anchor_x: 16.0,
        anchor_y: 32.0,
        uv: AtlasUv {
            u0: 0.0,
            v0: 0.0,
            u1: 0.5,
            v1: 0.5,
        },
    };
    let sprite2 = Sprite {
        width: 48.0,
        height: 48.0,
        anchor_x: 24.0,
        anchor_y: 48.0,
        uv: AtlasUv {
            u0: 0.5,
            v0: 0.5,
            u1: 1.0,
            v1: 1.0,
        },
    };
    let sprites = [Some(sprite1), Some(sprite2)];

    let mut buffer = SceneSpriteGeometryBuffer::default();
    let vertices = buffer.build(&objects, &sprites, &heights, view);

    // 2 visible sprites * 6 vertices * 8 floats = 96 floats
    assert_eq!(96, vertices.len());

    // Both object offsets are set
    let offsets = buffer.object_offsets();
    assert_eq!(2, offsets.len());
    assert!(offsets[0].is_some());
    assert!(offsets[1].is_some());

    // Farthest sprite (obj2) is sorted first (offset 0)
    assert_eq!(Some(0), offsets[1]);
    assert_eq!(Some(QUAD_FLOATS), offsets[0]);

    // Marker points helper
    let markers = build_marker_points(&objects, &heights);
    assert_eq!(2, markers.len());
    assert!((markers[0].y - (heights.sample_height(10.0, 10.0) + 0.25)).abs() < 1e-4);
}

#[test]
fn scene_shadow_geometry_cache_build() {
    let heights = TerrainHeightField::new(2, 2, vec![80; 4], 6.0, 2.0, 2.0, None).unwrap();
    let obj = SceneObjectPosition {
        world: Vec3::new(8192.0, 0.0, 8192.0),
        local_x: 0.0,
        local_z: 0.0,
        has_native_saved_position: false,
    };
    let shadow = Shadow {
        correction_x: 10.0,
        correction_z: 10.0,
        half_extent: 64.0,
        uv: AtlasUv {
            u0: 0.0,
            v0: 0.0,
            u1: 1.0,
            v1: 1.0,
        },
    };

    let mut cache = SceneShadowGeometryCache::default();
    let vertices = cache.build(&[obj], &[Some(shadow)], &heights);

    // 1 shadow * 6 vertices * 5 floats = 30 floats
    assert_eq!(30, vertices.len());

    // Vertices should include sample height + 0.03 for elevation
    let expected_y = heights.sample_height(32.0, 32.0) + 0.03;
    assert!((vertices[1] - expected_y).abs() < 1e-4);

    // Rebuild with same data reuses buffer
    let ptr1 = vertices.as_ptr();
    let vertices2 = cache.build(&[obj], &[Some(shadow)], &heights);
    assert_eq!(ptr1, vertices2.as_ptr());
}

#[test]
fn order_scene_draws_interleaving_and_stability() {
    // Port of RustSceneDrawOrderTests:
    // Shared order interleaves heterogeneous depths and keeps ties stable.
    let depths = [10.0, 3.0, 8.0, 3.0, 12.0];
    let order = order_scene_draws(&depths).unwrap();
    assert_eq!(vec![4, 0, 2, 1, 3], order);

    // Negative and zero floats: -0.0 and 0.0 tie break stably
    let tie_depths = [-0.0, 0.0, 1.0, -1.0];
    let tie_order = order_scene_draws(&tie_depths).unwrap();
    assert_eq!(vec![2, 0, 1, 3], tie_order);

    // Empty depths
    let empty_order = order_scene_draws(&[]).unwrap();
    assert!(empty_order.is_empty());
}

#[test]
fn order_scene_draws_rejects_nonfinite_depth() {
    assert!(order_scene_draws(&[1.0, f32::NAN]).is_err());
    assert!(order_scene_draws(&[1.0, f32::INFINITY]).is_err());
    assert!(order_scene_draws(&[1.0, f32::NEG_INFINITY]).is_err());
}

#[test]
fn combined_sprite_plan_interleaves_objects_and_particles() {
    let view = Mat4::look_at(Vec3::new(0.0, 0.0, 100.0), Vec3::new(0.0, 0.0, 0.0));

    // Two object quads: one at z=10, one at z=30
    let mut object_verts = vec![0.0; QUAD_FLOATS * 2];
    // Object 0 anchor at (0, 0, 10)
    for i in 0..6 {
        object_verts[i * 8 + 0] = 0.0;
        object_verts[i * 8 + 1] = 0.0;
        object_verts[i * 8 + 2] = 10.0;
    }
    // Object 1 anchor at (0, 0, 30)
    for i in 0..6 {
        object_verts[QUAD_FLOATS + i * 8 + 0] = 0.0;
        object_verts[QUAD_FLOATS + i * 8 + 1] = 0.0;
        object_verts[QUAD_FLOATS + i * 8 + 2] = 30.0;
    }

    // Particle quad at z=20
    let mut particle_verts = vec![0.0; QUAD_FLOATS];
    for i in 0..6 {
        particle_verts[i * 8 + 0] = 0.0;
        particle_verts[i * 8 + 1] = 0.0;
        particle_verts[i * 8 + 2] = 20.0;
    }

    let particle_draw = ParticleSpriteDraw {
        resolved_mode: Some(0),
        vertices: &particle_verts,
    };

    let plan = SceneCombinedSpritePlan::build(&object_verts, &[particle_draw], view).unwrap();
    assert_eq!(3, plan.len());

    // View is at (0, 0, 100) looking at (0, 0, 0).
    // In camera view space:
    // Anchor (0, 0, 10): eye-space Z is -90 -> depth = 90
    // Anchor (0, 0, 20): eye-space Z is -80 -> depth = 80
    // Anchor (0, 0, 30): eye-space Z is -70 -> depth = 70
    // Sorted descending depth (far to near): 90 (object 0), 80 (particle 0), 70 (object 1)
    assert_eq!(SceneSpriteDrawKind::Object, plan[0].kind);
    assert_eq!(0, plan[0].source_index);

    assert_eq!(SceneSpriteDrawKind::ParticleReplacement, plan[1].kind);
    assert_eq!(0, plan[1].source_index);

    assert_eq!(SceneSpriteDrawKind::Object, plan[2].kind);
    assert_eq!(1, plan[2].source_index);
}

#[test]
fn free_function_wrappers_match_stateful_builders() {
    let heights =
        TerrainHeightField::new(2, 2, vec![10, 70, 150, 250], 6.0, 2.0, 2.0, None).unwrap();
    let objects = [SceneObjectPosition {
        world: Vec3::new(128.0, 0.0, 128.0),
        local_x: 0.0,
        local_z: 0.0,
        has_native_saved_position: false,
    }];
    let uv = AtlasUv { u0: 0.0, v0: 0.0, u1: 1.0, v1: 1.0 };
    let sprite = Sprite { width: 16.0, height: 16.0, anchor_x: 8.0, anchor_y: 16.0, uv };
    let view = Mat4::IDENTITY;

    let v = build_sprite_vertices(&objects, &[Some(sprite)], &heights, view);
    assert_eq!(QUAD_FLOATS, v.len());
    let mut buf = SceneSpriteGeometryBuffer::default();
    assert_eq!(v.as_slice(), buf.build(&objects, &[Some(sprite)], &heights, view));
    assert!(build_sprite_vertices(&objects, &[None], &heights, view).is_empty());

    let shadow = Shadow { correction_x: 0.0, correction_z: 0.0, half_extent: 32.0, uv };
    let s = build_shadow_vertices(&objects, &[Some(shadow)], &heights);
    assert_eq!(30, s.len()); // 6 vertices * (x, y, z, u, v)
    assert_eq!(s.as_slice(), SceneShadowGeometryCache::default().build(&objects, &[Some(shadow)], &heights));

    let plan = combined_sprite_plan(&v, &[], view).unwrap();
    assert_eq!(1, plan.len());
    assert_eq!(SceneSpriteDrawKind::Object, plan[0].kind);
    assert_eq!(6, plan[0].vertex_count);
    assert!(combined_sprite_plan(&v[..QUAD_FLOATS - 1], &[], view).is_err());
}
