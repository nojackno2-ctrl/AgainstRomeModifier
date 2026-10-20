use std::f32::consts::FRAC_PI_2;

use against_rome_scene::{
    camera::EditorCamera,
    draw_list::{
        build_particle_batch, write_particle_quad, ParticleBlendMode,
        ParticleInstance, ParticleRect, SceneFrameBuilder,
        SceneFrameOptions, SceneParticleRenderer,
    },
    math::{Vec2, Vec3},
    scene::{SceneCombinedSpritePlan, SceneSpriteDrawKind, FLOATS_PER_SPRITE_VERTEX, QUAD_FLOATS, TILES_PER_SPRITE_PIXEL},
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

fn sample_uv() -> AtlasUv {
    AtlasUv {
        u0: 0.1,
        v0: 0.2,
        u1: 0.9,
        v1: 0.8,
    }
}

#[test]
fn particle_rect_constructors_and_dimensions() {
    let r1 = ParticleRect::new(-10.0, -5.0, 10.0, 5.0);
    assert_eq!(r1.width(), 20.0);
    assert_eq!(r1.height(), 10.0);

    let r2 = ParticleRect::from_edges(-16, -8, 16, 8);
    assert_eq!(r2.left, -16.0);
    assert_eq!(r2.top, -8.0);
    assert_eq!(r2.right, 16.0);
    assert_eq!(r2.bottom, 8.0);
    assert_eq!(r2.width(), 32.0);
    assert_eq!(r2.height(), 16.0);

    let r3 = ParticleRect::from_dimensions(64.0, 32.0);
    assert_eq!(r3.left, -32.0);
    assert_eq!(r3.right, 32.0);
    assert_eq!(r3.top, -16.0);
    assert_eq!(r3.bottom, 16.0);
    assert_eq!(r3.width(), 64.0);
    assert_eq!(r3.height(), 32.0);
}

#[test]
fn particle_blend_mode_contracts_and_leaf_addresses() {
    // Mode 0: Cutout replacement
    let cutout = ParticleBlendMode::Cutout;
    assert_eq!(cutout.resolved_mode(), Some(0));
    assert!(cutout.is_cutout());
    assert!(!cutout.gl_blend_enabled());
    assert!(cutout.is_implemented_for_gl());
    assert_eq!(cutout.leaf_address(), None);

    // Alpha blend
    let alpha = ParticleBlendMode::Alpha;
    assert!(alpha.gl_blend_enabled());
    assert!(alpha.is_implemented_for_gl());
    assert!(!alpha.is_cutout());

    // Additive blend
    let additive = ParticleBlendMode::Additive;
    assert!(additive.gl_blend_enabled());
    assert!(additive.is_implemented_for_gl());

    // Native mode 3 (0x4caf80 leaf address in Against_Rome.exe / against-rome-data)
    let native_mode3 = ParticleBlendMode::Native {
        mode: 3,
        leaf_address: Some(0x4caf80),
    };
    assert_eq!(native_mode3.resolved_mode(), Some(3));
    assert_eq!(native_mode3.leaf_address(), Some(0x4caf80));
    assert!(native_mode3.gl_blend_enabled());
    assert!(!native_mode3.is_implemented_for_gl());

    // from_resolved_mode constructor
    assert_eq!(ParticleBlendMode::from_resolved_mode(0, None), ParticleBlendMode::Cutout);
    assert_eq!(
        ParticleBlendMode::from_resolved_mode(3, Some(0x4caf80)),
        ParticleBlendMode::Native {
            mode: 3,
            leaf_address: Some(0x4caf80)
        }
    );
}

#[test]
fn write_particle_quad_generates_exact_48_floats_matching_csharp() {
    let mut vertices = Vec::new();
    let anchor = Vec3::new(10.0, 5.0, 20.0);
    let native_y = 123.5;
    let rect = ParticleRect::new(-16.0, -8.0, 16.0, 8.0);
    let uv = sample_uv();

    write_particle_quad(
        &mut vertices,
        anchor,
        native_y,
        rect,
        uv,
        0.0, // unrotated
        Vec2::new(1.0, 1.0), // unit scale
    );

    assert_eq!(vertices.len(), QUAD_FLOATS);

    // Each vertex has 8 floats: [anchor.x, anchor.y, anchor.z, ox, oy, u, v, native_y]
    for i in 0..6 {
        let base = i * FLOATS_PER_SPRITE_VERTEX;
        assert_eq!(vertices[base], anchor.x);
        assert_eq!(vertices[base + 1], anchor.y);
        assert_eq!(vertices[base + 2], anchor.z);
        assert_eq!(vertices[base + 7], native_y);
    }

    // Verify 6 vertices match C# quad:
    // V0: (rect.left, rect.bottom, uv.u0, uv.v1)
    let v0 = &vertices[0..8];
    assert!((v0[3] - rect.left * TILES_PER_SPRITE_PIXEL).abs() < 1e-6);
    assert!((v0[4] - (-rect.bottom * TILES_PER_SPRITE_PIXEL)).abs() < 1e-6);
    assert_eq!(v0[5], uv.u0);
    assert_eq!(v0[6], uv.v1);

    // V1: (rect.right, rect.bottom, uv.u1, uv.v1)
    let v1 = &vertices[8..16];
    assert!((v1[3] - rect.right * TILES_PER_SPRITE_PIXEL).abs() < 1e-6);
    assert!((v1[4] - (-rect.bottom * TILES_PER_SPRITE_PIXEL)).abs() < 1e-6);
    assert_eq!(v1[5], uv.u1);
    assert_eq!(v1[6], uv.v1);

    // V2: (rect.right, rect.top, uv.u1, uv.v0)
    let v2 = &vertices[16..24];
    assert!((v2[3] - rect.right * TILES_PER_SPRITE_PIXEL).abs() < 1e-6);
    assert!((v2[4] - (-rect.top * TILES_PER_SPRITE_PIXEL)).abs() < 1e-6);
    assert_eq!(v2[5], uv.u1);
    assert_eq!(v2[6], uv.v0);
}

#[test]
fn write_particle_quad_handles_rotation_and_scaling() {
    let anchor = Vec3::new(0.0, 0.0, 0.0);
    let native_y = 0.0;
    let rect = ParticleRect::new(-10.0, -10.0, 10.0, 10.0);
    let uv = sample_uv();

    // Size scaling 2x on X and 3x on Y
    let mut scaled_vertices = Vec::new();
    write_particle_quad(
        &mut scaled_vertices,
        anchor,
        native_y,
        rect,
        uv,
        0.0,
        Vec2::new(2.0, 3.0),
    );
    // V0 corner (x=-10 * 2, y=10 * 3) -> offset (-20 * scale, -30 * scale)
    let v0_scaled = &scaled_vertices[0..8];
    assert!((v0_scaled[3] - (-20.0 * TILES_PER_SPRITE_PIXEL)).abs() < 1e-6);
    assert!((v0_scaled[4] - (-30.0 * TILES_PER_SPRITE_PIXEL)).abs() < 1e-6);

    // 90 degree counter-clockwise rotation (PI / 2)
    let mut rotated_vertices = Vec::new();
    write_particle_quad(
        &mut rotated_vertices,
        anchor,
        native_y,
        rect,
        uv,
        FRAC_PI_2,
        Vec2::new(1.0, 1.0),
    );
    // V0 has x_raw = -10, y_raw = 10 (bottom).
    // Under 90 deg rotation:
    // x_rot = -10 * cos(90) - 10 * sin(90) = -10
    // y_rot = -10 * sin(90) + 10 * cos(90) = -10
    // ox = x_rot * scale = -10 * scale
    // oy = -y_rot * scale = 10 * scale
    let v0_rot = &rotated_vertices[0..8];
    assert!((v0_rot[3] - (-10.0 * TILES_PER_SPRITE_PIXEL)).abs() < 1e-4);
    assert!((v0_rot[4] - (10.0 * TILES_PER_SPRITE_PIXEL)).abs() < 1e-4);
}

#[test]
fn particle_instance_creation_and_child_coordinates_resolution() {
    let heights = create_test_terrain();
    let rect = ParticleRect::from_dimensions(32.0, 32.0);
    let uv = sample_uv();

    // Valid child coords
    let valid = ParticleInstance::from_child_coords(
        512.0, // 512 / 256 = tile X 2.0
        60.0,  // native Y 60.0 maps via heights.native_height
        768.0, // 768 / 256 = tile Z 3.0
        rect,
        uv,
        &heights,
    )
    .expect("resolves valid child coords to anchor");

    assert_eq!(valid.anchor.x, 2.0);
    assert_eq!(valid.anchor.z, 3.0);
    assert_eq!(valid.native_y, 60.0);
    assert!(valid.anchor.y.is_finite());

    // Invalid native height fails with exact diagnostic string
    let invalid = ParticleInstance::from_child_coords(
        512.0,
        f32::NAN,
        768.0,
        rect,
        uv,
        &heights,
    );
    assert_eq!(
        invalid.unwrap_err(),
        "Particle native height conversion is unavailable."
    );
}

#[test]
fn build_particle_batch_sorting_diagnostics_and_filtering() {
    let camera = EditorCamera::default();
    let view = camera.view_matrix();
    let proj = camera.projection_matrix(1.6);
    let rect = ParticleRect::from_dimensions(20.0, 20.0);
    let uv = sample_uv();

    // 1. Diagnostics skip particle and record reason
    let p_diag = ParticleInstance::new(Vec3::new(10.0, 0.0, 10.0), 0.0, rect, uv)
        .with_diagnostic("Particle frame missing");

    // 2. Particle behind camera (behind eye in opposite view direction)
    let eye = camera.position();
    let view_dir = (camera.target - eye).normalized();
    let p_behind = ParticleInstance::new(eye - view_dir * 50.0, 0.0, rect, uv);

    // 3. Particle with invalid non-finite coordinates
    let p_nan = ParticleInstance::new(Vec3::new(f32::NAN, 0.0, 10.0), 0.0, rect, uv);

    // 4. Valid mode 0 particle at target - view_dir * 30.0 (nearer to eye, smaller depth)
    let p_near = ParticleInstance::new(camera.target - view_dir * 30.0, 10.0, rect, uv)
        .with_blend_mode(ParticleBlendMode::Cutout);

    // 5. Valid mode 0 particle at target + view_dir * 30.0 (further from eye, larger depth)
    let p_far = ParticleInstance::new(camera.target + view_dir * 30.0, 15.0, rect, uv)
        .with_blend_mode(ParticleBlendMode::Cutout);

    // 6. Valid particle with unsupported native mode 3 at target (intermediate depth)
    let p_mode3 = ParticleInstance::new(camera.target, 12.0, rect, uv)
        .with_blend_mode(ParticleBlendMode::Native {
            mode: 3,
            leaf_address: Some(0x4caf80),
        });

    let instances = vec![p_diag, p_behind, p_nan, p_near, p_far, p_mode3];
    let batch = build_particle_batch(&instances, view, proj);

    // Diagnostics should record the pre-existing diagnostic, the invalid depth, and mode 3 warning
    assert!(batch.diagnostics.iter().any(|d| d.contains("Particle frame missing")));
    assert!(batch.diagnostics.iter().any(|d| d.contains("Particle view depth is invalid.")));
    assert!(batch.diagnostics.iter().any(|d| d.contains("Particle GL mode 3 is not implemented.")));

    // Accepted particles: p_near, p_far, p_mode3 = 3 particles
    assert_eq!(batch.quad_count, 3);
    assert_eq!(batch.vertex_count, 18);
    assert_eq!(batch.vertices.len(), 3 * QUAD_FLOATS);

    // Verify far-to-near order (descending depth):
    // p_far must precede p_mode3 which must precede p_near
    assert!(batch.draws[0].depth >= batch.draws[1].depth);
    assert!(batch.draws[1].depth >= batch.draws[2].depth);

    // Verify SceneParticleRenderer wrapper produces identical output
    let static_batch = SceneParticleRenderer::build(&instances, view, proj);
    assert_eq!(batch, static_batch);
}

#[test]
fn particle_batch_interleaves_with_scene_combined_sprite_plan() {
    let camera = EditorCamera::default();
    let view = camera.view_matrix();
    let proj = camera.projection_matrix(1.6);
    let rect = ParticleRect::from_dimensions(20.0, 20.0);
    let uv = sample_uv();

    let eye = camera.position();
    let view_dir = (camera.target - eye).normalized();

    // Mode 0 particle at intermediate distance (target)
    let particle = ParticleInstance::new(camera.target, 0.0, rect, uv)
        .with_blend_mode(ParticleBlendMode::Cutout);

    let batch = build_particle_batch(&[particle], view, proj);
    assert_eq!(batch.quad_count, 1);

    // Two object quads:
    // Object 0 further (target + view_dir * 20.0) -> larger depth
    let obj0_pos = camera.target + view_dir * 20.0;
    // Object 1 nearer (target - view_dir * 20.0) -> smaller depth
    let obj1_pos = camera.target - view_dir * 20.0;

    let mut object_verts = vec![0.0; 2 * QUAD_FLOATS];
    for i in 0..6 {
        object_verts[i * 8 + 0] = obj0_pos.x;
        object_verts[i * 8 + 1] = obj0_pos.y;
        object_verts[i * 8 + 2] = obj0_pos.z;
    }
    for i in 0..6 {
        object_verts[QUAD_FLOATS + i * 8 + 0] = obj1_pos.x;
        object_verts[QUAD_FLOATS + i * 8 + 1] = obj1_pos.y;
        object_verts[QUAD_FLOATS + i * 8 + 2] = obj1_pos.z;
    }

    let plan_draws = batch.as_sprite_plan_draws();
    assert_eq!(plan_draws.len(), 1);
    assert_eq!(plan_draws[0].resolved_mode, Some(0));

    let plan = SceneCombinedSpritePlan::build(&object_verts, &plan_draws, view)
        .expect("combined plan succeeds");

    assert_eq!(plan.len(), 3);
    // Depth descending order:
    // 1st: Object 0 (further)
    assert_eq!(plan[0].kind, SceneSpriteDrawKind::Object);
    assert_eq!(plan[0].source_index, 0);

    // 2nd: Particle (middle)
    assert_eq!(plan[1].kind, SceneSpriteDrawKind::ParticleReplacement);
    assert_eq!(plan[1].source_index, 0);

    // 3rd: Object 1 (nearer)
    assert_eq!(plan[2].kind, SceneSpriteDrawKind::Object);
    assert_eq!(plan[2].source_index, 1);
}

#[test]
fn prepare_scene_frame_integrates_particle_batch_and_aggregates() {
    let heights = create_test_terrain();
    let camera = EditorCamera::default();
    let viewport = Vec2::new(1280.0, 720.0);
    let rect = ParticleRect::from_dimensions(16.0, 16.0);
    let uv = sample_uv();

    let p1 = ParticleInstance::new(camera.target + Vec3::new(2.0, 0.0, 5.0), 10.0, rect, uv);
    let p2 = ParticleInstance::new(camera.target + Vec3::new(-2.0, 0.0, 10.0), 15.0, rect, uv);

    let frame = SceneFrameBuilder::new(&camera, viewport, &heights, &[])
        .particles(vec![p1.clone(), p2.clone()])
        .options(SceneFrameOptions {
            show_grid: false,
            show_water: false,
            show_shadows: false,
            show_objects: false,
            show_markers: false,
            show_particles: true,
            ..Default::default()
        })
        .build()
        .expect("frame preparation with particles succeeds");

    let particles = frame.particles.as_ref().expect("particle batch present");
    assert_eq!(particles.quad_count, 2);
    assert_eq!(particles.vertex_count, 12);
    assert_eq!(frame.diagnostics.particle_count, 2);

    // Aggregate counts: 2 particle quads, 12 vertices, 2 draw calls (terrain + particles)
    assert_eq!(frame.total_quads(), 2);
    assert_eq!(frame.total_vertices(), 12);
    assert_eq!(frame.total_draw_calls(), 2);

    // When show_particles is false, particles are excluded
    let no_particle_frame = SceneFrameBuilder::new(&camera, viewport, &heights, &[])
        .particles(vec![p1, p2])
        .options(SceneFrameOptions {
            show_particles: false,
            ..Default::default()
        })
        .build()
        .expect("frame preparation without particles succeeds");

    assert!(no_particle_frame.particles.is_none());
    assert_eq!(no_particle_frame.diagnostics.particle_count, 0);
}
