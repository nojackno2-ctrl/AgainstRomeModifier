use against_rome_scene::{
    camera::EditorCamera,
    draw_list::{
        prepare_scene_frame, team_color, BrushCursorOptions,
        PlacementPreviewOptions, SceneFrameBuilder,
        SceneFrameOptions, SelectionRect, BRUSH_CURSOR_COLOR, SCRIPT_MARK_COLOR,
    },
    lighting::{
        NativeLightDefinition, NativeLightInstance, NativeObjectLightingDefinition,
        SavedSceneLightSet, SceneLightingContext,
    },
    math::{Vec2, Vec3},
    scene::{Shadow, Sprite},
    scene_object::SceneObject,
    terrain::TerrainHeightField,
    AtlasUv,
};

fn create_test_terrain() -> TerrainHeightField {
    let samples = vec![
        10, 20, 30, 40,
        50, 60, 70, 80,
        90, 100, 110, 120,
        130, 140, 150, 160,
    ];
    TerrainHeightField::new(4, 4, samples, 6.0, 64.0, 64.0, Some(4.0)).unwrap()
}

fn create_test_sprite(width: f32, height: f32) -> Sprite {
    Sprite {
        width,
        height,
        anchor_x: width * 0.5,
        anchor_y: height,
        uv: AtlasUv {
            u0: 0.0,
            v0: 0.0,
            u1: 1.0,
            v1: 1.0,
        },
    }
}

#[test]
fn frame_preparation_viewport_validation_and_camera_matrices() {
    let heights = create_test_terrain();
    let camera = EditorCamera::default();

    // Positive finite viewport succeeds
    let viewport = Vec2::new(1280.0, 720.0);
    let frame = prepare_scene_frame(
        &camera,
        viewport,
        &heights,
        &[],
        &[],
        &[],
        None,
        None,
        &SceneFrameOptions::default(),
    )
    .expect("frame preparation succeeds for valid viewport");

    assert_eq!(frame.viewport, viewport);
    assert_eq!(frame.view_matrix, camera.view_matrix());
    let aspect = 1280.0 / 720.0;
    assert_eq!(frame.projection_matrix, camera.projection_matrix(aspect));
    assert_eq!(frame.view_projection_matrix, frame.view_matrix * frame.projection_matrix);
    assert_eq!(frame.camera_target, camera.target);
    assert_eq!(frame.camera_position, camera.position());

    // Center ray points towards the target
    assert!(frame.camera_ray_center.direction.length() > 0.99);

    // Invalid non-positive or non-finite viewports are rejected
    assert!(prepare_scene_frame(
        &camera,
        Vec2::new(0.0, 720.0),
        &heights,
        &[],
        &[],
        &[],
        None,
        None,
        &SceneFrameOptions::default(),
    )
    .is_err());

    assert!(prepare_scene_frame(
        &camera,
        Vec2::new(-100.0, 720.0),
        &heights,
        &[],
        &[],
        &[],
        None,
        None,
        &SceneFrameOptions::default(),
    )
    .is_err());

    assert!(prepare_scene_frame(
        &camera,
        Vec2::new(f32::NAN, 720.0),
        &heights,
        &[],
        &[],
        &[],
        None,
        None,
        &SceneFrameOptions::default(),
    )
    .is_err());
}

#[test]
fn frame_terrain_water_and_grid_commands() {
    let heights = create_test_terrain();
    let camera = EditorCamera::default();
    let viewport = Vec2::new(800.0, 600.0);

    let mut options = SceneFrameOptions::default();
    options.show_grid = true;
    options.show_water = true;
    options.water_level = 100.0;
    options.height_map_step = 4.0;
    options.water_color = [0.1, 0.4, 0.9, 0.55];

    let frame = prepare_scene_frame(
        &camera,
        viewport,
        &heights,
        &[],
        &[],
        &[],
        None,
        None,
        &options,
    )
    .unwrap();

    // Terrain command validation
    assert_eq!(frame.terrain.mvp, frame.view_projection_matrix);
    assert_eq!(frame.terrain.sun_light, Vec3::new(0.4, 0.85, 0.3));
    assert!(frame.terrain.native_height_known);
    assert_eq!(frame.terrain.native_height_grid, [4.0, 4.0, 64.0, 64.0]);
    assert_eq!(frame.terrain.dimension, 64.0);

    // Grid command enabled
    assert!(frame.grid.is_some());
    assert_eq!(frame.grid.as_ref().unwrap().mvp, frame.view_projection_matrix);

    // Water command validation
    let water = frame.water.as_ref().expect("water command present");
    let expected_water_y = 100.0 / 4.0 / 255.0 * heights.height_scale;
    assert!((water.water_y - expected_water_y).abs() < 1e-4);
    assert_eq!(water.color, [0.1, 0.4, 0.9, 0.55]);
    assert_eq!(water.vertices[0], Vec3::new(0.0, expected_water_y, 0.0));
    assert_eq!(water.vertices[1], Vec3::new(0.0, expected_water_y, 64.0));
    assert_eq!(water.vertices[2], Vec3::new(64.0, expected_water_y, 64.0));
    assert_eq!(water.vertices[3], Vec3::new(64.0, expected_water_y, 0.0));

    // When water is below minimum native height, water command is omitted
    let mut low_water_opts = options.clone();
    // minimum native height for sample 10 with step 4 is 40.0
    low_water_opts.water_level = 20.0;
    let low_water_frame = prepare_scene_frame(
        &camera,
        viewport,
        &heights,
        &[],
        &[],
        &[],
        None,
        None,
        &low_water_opts,
    )
    .unwrap();
    assert!(low_water_frame.water.is_none());

    // When show_water is false, water command is omitted
    let mut no_water_opts = options.clone();
    no_water_opts.show_water = false;
    let no_water_frame = prepare_scene_frame(
        &camera,
        viewport,
        &heights,
        &[],
        &[],
        &[],
        None,
        None,
        &no_water_opts,
    )
    .unwrap();
    assert!(no_water_frame.water.is_none());
}

#[test]
fn frame_lighting_preparation_and_cpu_evaluation() {
    let heights = create_test_terrain();
    let camera = EditorCamera::default();
    let viewport = Vec2::new(800.0, 600.0);

    // Build lighting definitions
    let light_def = NativeLightDefinition {
        index: 10,
        is_active: true,
        color: Vec3::new(1.0, 0.8, 0.6),
        radius: 300.0,
        flicker_type: 0,
        flicker_param: 0.0,
        special_fx: 0,
        name: "CampfireLight".into(),
    };
    let light_catalog = against_rome_scene::lighting::NativeLightCatalog::new([light_def]);

    let obj_light_def = NativeObjectLightingDefinition {
        type_id: 1,
        name: "BauCampfire".into(),
        apt_index: -1,
        light_def_index: 10,
        light_height_offset: 20,
        apt_light_def_index: -1,
        apt_light_height_offset: 0,
    };
    let obj_catalog = against_rome_scene::lighting::NativeObjectLightingCatalog::new([obj_light_def]);

    let mut context = SceneLightingContext::new(
        None,
        Some(light_catalog),
        Some(obj_catalog),
        Default::default(),
    );

    // Saved native light
    let saved_light = NativeLightInstance::create_saved(
        0,
        Vec3::new(5120.0, 100.0, 5120.0),
        Vec3::new(0.2, 0.5, 1.0),
        400.0,
        400.0,
        true,
        Some(10),
    );
    let saved_set = SavedSceneLightSet::new(vec![saved_light], Default::default()).unwrap();

    let obj = SceneObject::new(
        "BauCampfire",
        Vec3::new(8192.0, 0.0, 8192.0),
        Vec3::ZERO,
        0,
        "test.sdl",
        1,
        None,
    );

    let mut options = SceneFrameOptions::default();
    options.game_lighting_enabled = true;
    options.game_hour = 12.0;

    let frame = prepare_scene_frame(
        &camera,
        viewport,
        &heights,
        &[obj],
        &[None],
        &[None],
        Some(&mut context),
        Some(&saved_set),
        &options,
    )
    .unwrap();

    // 2 total lights: 1 synthesized + 1 saved
    assert_eq!(frame.lighting.selected_count, 2);
    assert_eq!(frame.diagnostics.selected_lights, 2);
    assert_eq!(frame.diagnostics.light_diagnostics.total_saved, 1);
    assert_eq!(frame.diagnostics.light_diagnostics.synthesized_lights, 1);

    // CPU lighting evaluation:
    // 1. When game lighting disabled, returns pure white (1, 1, 1)
    let disabled_lit = frame.lighting.evaluate_lighting(
        Vec3::new(0.2, 0.2, 0.2),
        false,
        Vec3::new(32.0, 0.0, 32.0),
        100.0,
    );
    assert_eq!(disabled_lit, Vec3::new(1.0, 1.0, 1.0));

    // 2. Dim ambient lighting evaluation:
    let dim_ambient = Vec3::new(0.1, 0.1, 0.1);
    // Far from all lights: returns dim ambient color
    let far_tile = Vec3::new(0.0, 0.0, 0.0);
    let far_lit = frame.lighting.evaluate_lighting(dim_ambient, true, far_tile, 100.0);
    assert_eq!(far_lit, dim_ambient);

    // 3. Right at the saved light position (5120 / 256 = 20): gets illuminated
    let near_saved_tile = Vec3::new(20.0, 0.0, 20.0);
    let near_saved_lit = frame.lighting.evaluate_lighting(dim_ambient, true, near_saved_tile, 100.0);
    assert!(near_saved_lit.z > dim_ambient.z);

    // 4. Saved light with unknown native Y (>= 1e37) is ignored
    let unknown_y_lit = frame.lighting.evaluate_lighting(dim_ambient, true, near_saved_tile, 1.1e38);
    assert_eq!(unknown_y_lit, dim_ambient);
}

#[test]
fn frame_sprites_geometry_batches_and_far_to_near_ordering() {
    let heights = create_test_terrain();
    let camera = EditorCamera::default();
    let viewport = Vec2::new(1280.0, 720.0);

    // From default camera (yaw 45, pitch 30, pos > 32):
    // Object at tile 10 (world 2560) is farther from camera than object at tile 50 (world 12800)
    let obj_farther = SceneObject::new(
        "FigLegion",
        Vec3::new(2560.0, 0.0, 2560.0), // tile 10
        Vec3::ZERO,
        1,
        "test.sdl",
        1,
        None,
    );
    let obj_nearer = SceneObject::new(
        "BauRomanTower",
        Vec3::new(12800.0, 0.0, 12800.0), // tile 50
        Vec3::ZERO,
        2,
        "test.sdl",
        2,
        None,
    );

    let sprite_farther = create_test_sprite(32.0, 48.0);
    let sprite_nearer = create_test_sprite(64.0, 128.0);

    let objects = [obj_farther, obj_nearer];
    let sprites = [Some(sprite_farther), Some(sprite_nearer)];

    let frame = prepare_scene_frame(
        &camera,
        viewport,
        &heights,
        &objects,
        &sprites,
        &[None, None],
        None,
        None,
        &SceneFrameOptions::default(),
    )
    .unwrap();

    // 1 sprite batch with 2 quads (12 vertices, 96 floats)
    assert_eq!(frame.sprites.len(), 1);
    let batch = &frame.sprites[0];
    assert_eq!(batch.quad_count, 2);
    assert_eq!(batch.vertex_count, 12);
    assert_eq!(batch.vertices.len(), 96);
    assert_eq!(batch.alpha, 1.0);
    assert!(!batch.is_preview);

    // Ordered sprites must be sorted descending by depth (far-to-near)
    assert_eq!(frame.ordered_sprites.len(), 2);
    assert!(frame.ordered_sprites[0].depth > frame.ordered_sprites[1].depth);
    assert_eq!(frame.ordered_sprites[0].object_index, 0); // obj_farther is farthest
    assert_eq!(frame.ordered_sprites[1].object_index, 1); // obj_nearer is nearest
}

#[test]
fn frame_marker_fallback_and_team_colors() {
    let heights = create_test_terrain();
    let camera = EditorCamera::default();
    let viewport = Vec2::new(800.0, 600.0);

    // Object with team 1
    let obj_team1 = SceneObject::new(
        "FigTroop",
        Vec3::new(1000.0, 0.0, 1000.0),
        Vec3::ZERO,
        1,
        "custom.sdl",
        1,
        None,
    );
    // Object with team 2
    let obj_team2 = SceneObject::new(
        "BauHut",
        Vec3::new(2000.0, 0.0, 2000.0),
        Vec3::ZERO,
        2,
        "custom.sdl",
        2,
        None,
    );
    // DATA/objects.dat normal object (invisible marker)
    let obj_invisible = SceneObject::new(
        "Tree",
        Vec3::new(3000.0, 0.0, 3000.0),
        Vec3::ZERO,
        0,
        "DATA/objects.dat",
        3,
        None,
    );
    // DATA/objects.dat script mark (visible marker)
    let obj_script = SceneObject::new(
        "SkriptmarkSpawn",
        Vec3::new(4000.0, 0.0, 4000.0),
        Vec3::ZERO,
        0,
        "DATA/objects.dat",
        4,
        None,
    );

    let objects = [obj_team1, obj_team2, obj_invisible, obj_script];
    let sprites = [None, None, None, None];

    let frame = prepare_scene_frame(
        &camera,
        viewport,
        &heights,
        &objects,
        &sprites,
        &[None, None, None, None],
        None,
        None,
        &SceneFrameOptions::default(),
    )
    .unwrap();

    // Exactly 3 markers: team 1, team 2, and script mark (invisible DATA tree is excluded)
    assert_eq!(frame.markers.len(), 3);

    // Team 1 marker
    assert_eq!(frame.markers[0].object_index, 0);
    assert_eq!(frame.markers[0].color, team_color(1));
    assert_eq!(frame.markers[0].point_size, 10.0);
    assert!(!frame.markers[0].is_script_mark);

    // Team 2 marker
    assert_eq!(frame.markers[1].object_index, 1);
    assert_eq!(frame.markers[1].color, team_color(2));
    assert_eq!(frame.markers[1].point_size, 10.0);
    assert!(!frame.markers[1].is_script_mark);

    // Script mark
    assert_eq!(frame.markers[2].object_index, 3);
    assert_eq!(frame.markers[2].color, SCRIPT_MARK_COLOR);
    assert_eq!(frame.markers[2].point_size, 5.0);
    assert!(frame.markers[2].is_script_mark);
}

#[test]
fn frame_ground_shadows_and_suppression() {
    let heights = create_test_terrain();
    let camera = EditorCamera::default();
    let viewport = Vec2::new(800.0, 600.0);

    let obj = SceneObject::new(
        "BauHut",
        Vec3::new(2560.0, 0.0, 2560.0),
        Vec3::ZERO,
        0,
        "test.sdl",
        1,
        None,
    );

    let shadow = Shadow {
        correction_x: 0.0,
        correction_z: 0.0,
        half_extent: 128.0,
        uv: AtlasUv {
            u0: 0.0,
            v0: 0.0,
            u1: 1.0,
            v1: 1.0,
        },
    };

    // Shadow enabled
    let frame = prepare_scene_frame(
        &camera,
        viewport,
        &heights,
        &[obj.clone()],
        &[None],
        &[Some(shadow)],
        None,
        None,
        &SceneFrameOptions::default(),
    )
    .unwrap();

    let shadow_batch = frame.shadows.as_ref().expect("shadow batch present");
    assert_eq!(shadow_batch.quad_count, 1);
    assert_eq!(shadow_batch.vertex_count, 6);
    assert_eq!(shadow_batch.vertices.len(), 30); // 6 vertices * 5 floats

    // Shadow disabled in options
    let mut no_shadow_opts = SceneFrameOptions::default();
    no_shadow_opts.show_shadows = false;
    let no_shadow_frame = prepare_scene_frame(
        &camera,
        viewport,
        &heights,
        &[obj],
        &[None],
        &[Some(shadow)],
        None,
        None,
        &no_shadow_opts,
    )
    .unwrap();
    assert!(no_shadow_frame.shadows.is_none());
}

#[test]
fn frame_frustum_culling_filters_offscreen_objects() {
    let heights = create_test_terrain();
    let mut camera = EditorCamera::default();
    camera.target = Vec3::new(32.0, 0.0, 32.0); // centered around tile 32
    camera.set_distance(40.0); // zoom in tight
    let viewport = Vec2::new(400.0, 300.0);

    // Visible object near camera target (tile 32 = world 8192)
    let visible_obj = SceneObject::new(
        "FigNear",
        Vec3::new(8192.0, 0.0, 8192.0),
        Vec3::ZERO,
        1,
        "test.sdl",
        1,
        None,
    );

    // Distant object far outside camera view (world 100)
    let distant_obj = SceneObject::new(
        "FigFar",
        Vec3::new(100.0, 0.0, 100.0),
        Vec3::ZERO,
        2,
        "test.sdl",
        2,
        None,
    );

    let objects = [visible_obj, distant_obj];
    let sprites = [
        Some(create_test_sprite(32.0, 32.0)),
        Some(create_test_sprite(32.0, 32.0)),
    ];

    // Culling enabled
    let mut cull_opts = SceneFrameOptions::default();
    cull_opts.culling_enabled = true;
    cull_opts.culling_padding = 10.0;

    let frame = prepare_scene_frame(
        &camera,
        viewport,
        &heights,
        &objects,
        &sprites,
        &[None, None],
        None,
        None,
        &cull_opts,
    )
    .unwrap();

    assert_eq!(frame.diagnostics.total_objects, 2);
    assert_eq!(frame.diagnostics.visible_objects, 1);
    assert_eq!(frame.diagnostics.culled_objects, 1);
    assert_eq!(frame.ordered_sprites.len(), 1);
    assert_eq!(frame.ordered_sprites[0].object_index, 0);

    // Culling disabled preserves all objects
    let mut no_cull_opts = cull_opts.clone();
    no_cull_opts.culling_enabled = false;
    let no_cull_frame = prepare_scene_frame(
        &camera,
        viewport,
        &heights,
        &objects,
        &sprites,
        &[None, None],
        None,
        None,
        &no_cull_opts,
    )
    .unwrap();

    assert_eq!(no_cull_frame.diagnostics.total_objects, 2);
    assert_eq!(no_cull_frame.diagnostics.visible_objects, 2);
    assert_eq!(no_cull_frame.diagnostics.culled_objects, 0);
    assert_eq!(no_cull_frame.ordered_sprites.len(), 2);
}

#[test]
fn frame_brush_cursor_perimeter_generation_and_selection() {
    let heights = create_test_terrain();
    let camera = EditorCamera::default();
    let viewport = Vec2::new(800.0, 600.0);

    let mut options = SceneFrameOptions::default();
    options.brush_cursor = Some(BrushCursorOptions {
        hover_tile_x: 20,
        hover_tile_y: 20,
        hover_point: None,
        hover_radius: None,
        brush_size: 4,
        selection: None,
        color: BRUSH_CURSOR_COLOR,
        line_width: 3.0,
    });

    let frame = prepare_scene_frame(
        &camera,
        viewport,
        &heights,
        &[],
        &[],
        &[],
        None,
        None,
        &options,
    )
    .unwrap();

    let cursor = frame.cursor.as_ref().expect("cursor draw present");
    assert_eq!(cursor.color, BRUSH_CURSOR_COLOR);
    assert_eq!(cursor.line_width, 3.0);
    // Radius = 2 -> bounds: x0 = 18, x1 = 23, y0 = 18, y1 = 23.
    // Perimeter length: (23 - 18 + 1) * 2 + (23 - 18 - 1) * 2 = 6 * 2 + 4 * 2 = 20 points
    assert_eq!(cursor.perimeter_points.len(), 20);

    // Explicit SelectionRect override
    let mut sel_options = options.clone();
    sel_options.brush_cursor = Some(BrushCursorOptions {
        hover_tile_x: 10,
        hover_tile_y: 10,
        hover_point: None,
        hover_radius: None,
        brush_size: 1,
        selection: Some(SelectionRect::new(5, 5, 10, 10)),
        color: [1.0, 0.0, 0.0, 1.0],
        line_width: 2.0,
    });

    let sel_frame = prepare_scene_frame(
        &camera,
        viewport,
        &heights,
        &[],
        &[],
        &[],
        None,
        None,
        &sel_options,
    )
    .unwrap();

    let sel_cursor = sel_frame.cursor.as_ref().expect("selection cursor present");
    assert_eq!(sel_cursor.color, [1.0, 0.0, 0.0, 1.0]);
    // Rect from (5, 5) to (10, 10):
    // top: 5..=10 (6), right: 6..=10 (5), bottom: 9..=5 (5), left: 9..=6 (4) = 20 points
    assert_eq!(sel_cursor.perimeter_points.len(), 20);
}

#[test]
fn frame_continuous_brush_cursor_tracks_raycast_point_and_fractional_radius() {
    let heights = create_test_terrain();
    let mut options = SceneFrameOptions::default();
    options.brush_cursor = Some(BrushCursorOptions {
        hover_tile_x: 0,
        hover_tile_y: 0,
        hover_point: Some((1.25, 2.5)),
        hover_radius: Some(0.5),
        brush_size: 1,
        selection: None,
        color: BRUSH_CURSOR_COLOR,
        line_width: 2.0,
    });

    let frame = prepare_scene_frame(
        &EditorCamera::default(),
        Vec2::new(800.0, 600.0),
        &heights,
        &[],
        &[],
        &[],
        None,
        None,
        &options,
    )
    .unwrap();

    let perimeter = &frame.cursor.as_ref().expect("continuous cursor").perimeter_points;
    assert_eq!(perimeter.len(), 64);
    assert!((perimeter[0].x - 1.75).abs() < 1e-5);
    assert!((perimeter[0].z - 2.5).abs() < 1e-5);
    let opposite = perimeter[32];
    assert!((opposite.x - 0.75).abs() < 1e-5);
    assert!((opposite.z - 2.5).abs() < 1e-5);
}

#[test]
fn frame_ghost_placement_preview_generation() {
    let heights = create_test_terrain();
    let camera = EditorCamera::default();
    let viewport = Vec2::new(800.0, 600.0);

    let preview_sprite = create_test_sprite(48.0, 48.0);
    let mut options = SceneFrameOptions::default();
    options.placement_preview = Some(PlacementPreviewOptions::new(
        "BauTemple",
        3,
        90.0,
        15,
        25,
        preview_sprite,
    ));

    let frame = prepare_scene_frame(
        &camera,
        viewport,
        &heights,
        &[],
        &[],
        &[],
        None,
        None,
        &options,
    )
    .unwrap();

    // 1 preview sprite batch with 1 quad (6 vertices, 48 floats)
    assert_eq!(frame.sprites.len(), 1);
    let preview_batch = &frame.sprites[0];
    assert!(preview_batch.is_preview);
    assert_eq!(preview_batch.alpha, 0.6);
    assert_eq!(preview_batch.quad_count, 1);
    assert_eq!(preview_batch.vertex_count, 6);
}

#[test]
fn frame_screen_projection_and_picking_queries() {
    let heights = create_test_terrain();
    let mut camera = EditorCamera::default();
    camera.focus_tile(32.0, 32.0, &heights);
    let viewport = Vec2::new(1000.0, 800.0);

    let obj = SceneObject::new(
        "BauHut",
        Vec3::new(32.0 * 256.0, 0.0, 32.0 * 256.0), // Tile (32, 32)
        Vec3::ZERO,
        1,
        "test.sdl",
        1,
        None,
    );

    let frame = prepare_scene_frame(
        &camera,
        viewport,
        &heights,
        &[obj.clone()],
        &[None],
        &[None],
        None,
        None,
        &SceneFrameOptions::default(),
    )
    .unwrap();

    // 1. World to screen projection: camera target projects to screen center (500, 400)
    let screen_pos = frame
        .project_world_to_screen(camera.target)
        .expect("target projects to screen");
    assert!((screen_pos.x - 500.0).abs() < 1.0);
    assert!((screen_pos.y - 400.0).abs() < 1.0);

    // 2. Terrain raycast picking at screen center hits tile (32, 32)
    let hit_tile = frame
        .pick_tile(Vec2::new(500.0, 400.0), &heights)
        .expect("center ray hits terrain");
    assert_eq!(hit_tile, (32, 32));

    let hit_point = frame
        .pick_terrain(Vec2::new(500.0, 400.0), &heights)
        .expect("center ray hits terrain point");
    assert!((hit_point.x - 32.0).abs() < 0.5);
    assert!((hit_point.z - 32.0).abs() < 0.5);

    // 3. Object marker picking at screen center
    let picked = frame.pick_object(
        Vec2::new(500.0, 400.0),
        &[obj.clone()],
        &[None],
        &heights,
        15.0,
    );
    assert_eq!(picked, Some(0));

    // Clicking far away misses
    let miss = frame.pick_object(
        Vec2::new(10.0, 10.0),
        &[obj],
        &[None],
        &heights,
        5.0,
    );
    assert_eq!(miss, None);
}

#[test]
fn scene_frame_builder_pattern_and_aggregates() {
    let heights = create_test_terrain();
    let camera = EditorCamera::default();
    let viewport = Vec2::new(1280.0, 720.0);

    let obj1 = SceneObject::new("Fig1", Vec3::new(2560.0, 0.0, 2560.0), Vec3::ZERO, 1, "test.sdl", 1, None);
    let obj2 = SceneObject::new("Bau1", Vec3::new(5120.0, 0.0, 5120.0), Vec3::ZERO, 2, "test.sdl", 2, None);
    let objects = [obj1, obj2];

    let sprites = [Some(create_test_sprite(32.0, 32.0)), None];
    let shadows = [None, Some(Shadow {
        correction_x: 0.0,
        correction_z: 0.0,
        half_extent: 64.0,
        uv: AtlasUv { u0: 0.0, v0: 0.0, u1: 1.0, v1: 1.0 },
    })];

    let frame = SceneFrameBuilder::new(&camera, viewport, &heights, &objects)
        .sprites(&sprites)
        .shadows(&shadows)
        .options(SceneFrameOptions {
            show_grid: true,
            show_water: true,
            show_shadows: true,
            show_objects: true,
            show_markers: true,
            water_level: 80.0,
            ..Default::default()
        })
        .build()
        .unwrap();

    // Verification of frame aggregate calculations
    // Water quad (1) + Shadow quad (1) + Sprite quad (1) = 3 quads
    assert_eq!(frame.total_quads(), 3);

    // Water (4) + Shadow (6) + Sprite (6) + Marker (1) = 17 vertices
    assert_eq!(frame.total_vertices(), 17);

    // Terrain (1) + Grid (1) + Water (1) + Shadow (1) + Sprite (1) + Marker (1) = 6 draw calls
    assert_eq!(frame.total_draw_calls(), 6);
}
