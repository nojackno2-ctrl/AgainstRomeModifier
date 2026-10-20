use crate::{
    atlas::AtlasUv,
    camera::{EditorCamera, TerrainRay},
    lighting::{
        PreparedSceneLighting, SavedSceneLightSet, SceneLightDiagnostics, SceneLightingContext,
    },
    math::{Mat4, Vec2, Vec3},
    picking::{pick_object, PickableSprite},
    ray_picker::TerrainRayPicker,
    rendering::{is_visible, IsometricFootprint, IsometricRenderLayer, IsometricSortItem},
    scene::{
        build_shadow_vertices, build_sprite_vertices, ground_point, native_query_y,
        SceneObjectPosition, SceneSpriteGeometryBuffer, Shadow, Sprite,
        FLOATS_PER_SPRITE_VERTEX, QUAD_FLOATS, TILES_PER_SPRITE_PIXEL, WORLD_UNITS_PER_TILE,
    },
    scene_object::{SceneObject, SceneObjectKind},
    terrain::TerrainHeightField,
};

pub const SCRIPT_MARK_COLOR: [f32; 4] = [0.2, 0.85, 0.95, 1.0];
pub const BRUSH_CURSOR_COLOR: [f32; 4] = [1.0, 0.94, 0.47, 1.0];
pub const WATER_DEFAULT_COLOR: [f32; 4] = [0.25, 0.5, 0.8, 0.55];

/// Maps a team index (0..15) to its distinct RGBA color matching C# Map3DViewControl.TeamColor.
pub fn team_color(team: i32) -> [f32; 4] {
    match team {
        1 => [0.25, 0.45, 0.95, 1.0], // Blue
        2 => [0.9, 0.15, 0.2, 1.0],   // Red
        3 => [0.9, 0.7, 0.1, 1.0],    // Yellow
        4 => [0.2, 0.7, 0.4, 1.0],    // Green
        5 => [0.7, 0.35, 0.85, 1.0],  // Purple
        6 => [0.95, 0.4, 0.1, 1.0],   // Orange
        7 => [0.1, 0.85, 0.9, 1.0],   // Cyan
        _ => [0.8, 0.8, 0.8, 1.0],    // Grey
    }
}

/// 2D tile-grid selection bounds for terrain editing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl SelectionRect {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
}

/// Options controlling brush cursor outline generation.
#[derive(Clone, Debug, PartialEq)]
pub struct BrushCursorOptions {
    pub hover_tile_x: i32,
    pub hover_tile_y: i32,
    pub brush_size: i32,
    pub selection: Option<SelectionRect>,
    pub color: [f32; 4],
    pub line_width: f32,
}

impl Default for BrushCursorOptions {
    fn default() -> Self {
        Self {
            hover_tile_x: -1,
            hover_tile_y: -1,
            brush_size: 1,
            selection: None,
            color: BRUSH_CURSOR_COLOR,
            line_width: 2.0,
        }
    }
}

/// Ghost preview of an object to be placed at the hovered tile.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacementPreviewOptions {
    pub name: String,
    pub team: i32,
    pub angle_degrees: f32,
    pub hover_tile_x: i32,
    pub hover_tile_y: i32,
    pub sprite: Sprite,
    pub alpha: f32,
}

impl PlacementPreviewOptions {
    pub fn new(
        name: impl Into<String>,
        team: i32,
        angle_degrees: f32,
        hover_tile_x: i32,
        hover_tile_y: i32,
        sprite: Sprite,
    ) -> Self {
        Self {
            name: name.into(),
            team,
            angle_degrees,
            hover_tile_x,
            hover_tile_y,
            sprite,
            alpha: 0.6,
        }
    }
}

/// Configuration options for preparing a scene rendering frame.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneFrameOptions {
    pub show_grid: bool,
    pub show_objects: bool,
    pub show_water: bool,
    pub show_shadows: bool,
    pub show_markers: bool,
    pub show_particles: bool,
    pub game_lighting_enabled: bool,
    pub game_hour: f32,
    pub culling_enabled: bool,
    pub culling_padding: f32,
    pub water_level: f32,
    pub water_color: [f32; 4],
    pub height_map_step: f32,
    pub brush_cursor: Option<BrushCursorOptions>,
    pub placement_preview: Option<PlacementPreviewOptions>,
    pub particles: Vec<ParticleInstance>,
}

impl Default for SceneFrameOptions {
    fn default() -> Self {
        Self {
            show_grid: false,
            show_objects: true,
            show_water: true,
            show_shadows: true,
            show_markers: true,
            show_particles: true,
            game_lighting_enabled: false,
            game_hour: 12.0,
            culling_enabled: false,
            culling_padding: 32.0,
            water_level: 0.0,
            water_color: WATER_DEFAULT_COLOR,
            height_map_step: 4.0,
            brush_cursor: None,
            placement_preview: None,
            particles: Vec::new(),
        }
    }
}

/// CPU-prepared draw command for the terrain mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainDrawCommand {
    pub mvp: Mat4,
    pub sun_light: Vec3,
    pub ambient_color: Vec3,
    pub game_lighting_enabled: bool,
    pub lighting: PreparedSceneLighting,
    pub native_height_known: bool,
    pub native_height_grid: [f32; 4],
    pub dimension: f32,
}

/// CPU-prepared draw command for the water plane quad.
#[derive(Clone, Debug, PartialEq)]
pub struct WaterDrawCommand {
    pub water_y: f32,
    pub vertices: [Vec3; 4],
    pub color: [f32; 4],
}

/// CPU-prepared draw command for the terrain tile grid wireframe overlay.
#[derive(Clone, Debug, PartialEq)]
pub struct GridDrawCommand {
    pub mvp: Mat4,
}

/// Packed ground shadow geometry vertices for a single draw call.
/// Vertex layout: 5 floats per vertex (world X, ground Y, world Z, U, V).
#[derive(Clone, Debug, PartialEq)]
pub struct ShadowDrawBatch {
    pub vertices: Vec<f32>,
    pub vertex_count: usize,
    pub quad_count: usize,
    pub mvp: Mat4,
}

/// Packed screen-aligned sprite geometry vertices for a single draw call.
/// Vertex layout: 8 floats per vertex (anchor xyz, offset xy, uv, native query Y).
#[derive(Clone, Debug, PartialEq)]
pub struct SpriteDrawBatch {
    pub vertices: Vec<f32>,
    pub vertex_count: usize,
    pub quad_count: usize,
    pub view: Mat4,
    pub projection: Mat4,
    pub alpha: f32,
    pub is_preview: bool,
}

/// An individual sorted sprite item within the frame draw list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrderedSpriteDraw {
    pub object_index: usize,
    pub depth: f32,
    pub anchor: Vec3,
    pub native_y: f32,
    pub sprite: Sprite,
    pub first_vertex: usize,
    pub vertex_count: usize,
}

/// Marker point draw data for objects without active sprites or script marks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneMarkerDraw {
    pub object_index: usize,
    pub point: Vec3,
    pub color: [f32; 4],
    pub point_size: f32,
    pub is_script_mark: bool,
}

/// Brush cursor perimeter line loop draw data.
#[derive(Clone, Debug, PartialEq)]
pub struct BrushCursorDraw {
    pub perimeter_points: Vec<Vec3>,
    pub color: [f32; 4],
    pub line_width: f32,
}

/// Signed 2D billboard rectangle in sprite pixels relative to the anchor point,
/// matching C# NativeParticleSpriteRect and the editor ALR anchor convention.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl ParticleRect {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn from_edges(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left: left as f32,
            top: top as f32,
            right: right as f32,
            bottom: bottom as f32,
        }
    }

    pub fn from_dimensions(width: f32, height: f32) -> Self {
        let hw = width * 0.5;
        let hh = height * 0.5;
        Self {
            left: -hw,
            top: -hh,
            right: hw,
            bottom: hh,
        }
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }
}

/// Billboard blending mode and native dispatch metadata.
///
/// Matches native Against Rome 0x4B53C0 mode dispatch and C# particle rendering states:
/// - Cutout: Mode 0 (alpha-tested/cutout, GL blend disabled).
/// - Alpha: Standard alpha blending (GL blend enabled, src_alpha / one_minus_src_alpha).
/// - Additive: Additive blending (GL blend enabled, one / one).
/// - Native: Unimplemented or native-specific leaf modes (modes 1..=9, 49, etc.)
///   with raw mode number and optional native leaf dispatch address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParticleBlendMode {
    /// Mode 0: Cutout replacement with alpha discard; GL blend disabled.
    Cutout,
    /// Alpha blending; GL blend enabled.
    Alpha,
    /// Additive blending; GL blend enabled.
    Additive,
    /// Native game mode with raw mode index and optional native leaf function address (e.g., 0x4ca010, 0x4ca340, 0x4caf80).
    Native {
        mode: i32,
        leaf_address: Option<u32>,
    },
}

impl ParticleBlendMode {
    pub fn from_resolved_mode(mode: i32, leaf_address: Option<u32>) -> Self {
        match mode {
            0 => Self::Cutout,
            _ => Self::Native {
                mode,
                leaf_address,
            },
        }
    }

    pub fn resolved_mode(&self) -> Option<i32> {
        match self {
            Self::Cutout => Some(0),
            Self::Native { mode, .. } => Some(*mode),
            _ => None,
        }
    }

    pub fn leaf_address(&self) -> Option<u32> {
        match self {
            Self::Native { leaf_address, .. } => *leaf_address,
            _ => None,
        }
    }

    pub fn is_cutout(&self) -> bool {
        matches!(self, Self::Cutout)
    }

    pub fn gl_blend_enabled(&self) -> bool {
        match self {
            Self::Cutout => false,
            Self::Alpha | Self::Additive => true,
            Self::Native { mode, .. } => *mode != 0,
        }
    }

    pub fn is_implemented_for_gl(&self) -> bool {
        matches!(self, Self::Cutout | Self::Alpha | Self::Additive)
    }
}

/// High-level particle instance specification for scene rendering.
///
/// Holds the world position anchor, native query Y, pixel bounds, UV coordinates,
/// rotation, size scale, blend mode, and optional diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleInstance {
    pub anchor: Vec3,
    pub native_y: f32,
    pub rect: ParticleRect,
    pub uv: AtlasUv,
    pub rotation: f32,
    pub size: Vec2,
    pub blend_mode: ParticleBlendMode,
    pub alpha: f32,
    pub diagnostic: Option<String>,
}

impl ParticleInstance {
    pub fn new(anchor: Vec3, native_y: f32, rect: ParticleRect, uv: AtlasUv) -> Self {
        Self {
            anchor,
            native_y,
            rect,
            uv,
            rotation: 0.0,
            size: Vec2::new(1.0, 1.0),
            blend_mode: ParticleBlendMode::Cutout,
            alpha: 1.0,
            diagnostic: None,
        }
    }

    pub fn from_child_coords(
        child_x: f32,
        child_y: f32,
        child_z: f32,
        rect: ParticleRect,
        uv: AtlasUv,
        heights: &TerrainHeightField,
    ) -> Result<Self, &'static str> {
        let ground_y = heights
            .native_height(child_y)
            .ok_or("Particle native height conversion is unavailable.")?;
        let anchor = Vec3::new(
            child_x / WORLD_UNITS_PER_TILE,
            ground_y,
            child_z / WORLD_UNITS_PER_TILE,
        );
        Ok(Self::new(anchor, child_y, rect, uv))
    }

    pub fn with_rotation(mut self, radians: f32) -> Self {
        self.rotation = radians;
        self
    }

    pub fn with_rotation_degrees(mut self, degrees: f32) -> Self {
        self.rotation = degrees.to_radians();
        self
    }

    pub fn with_size(mut self, scale_x: f32, scale_y: f32) -> Self {
        self.size = Vec2::new(scale_x, scale_y);
        self
    }

    pub fn with_blend_mode(mut self, mode: ParticleBlendMode) -> Self {
        self.blend_mode = mode;
        self
    }

    pub fn with_alpha(mut self, alpha: f32) -> Self {
        self.alpha = alpha;
        self
    }

    pub fn with_diagnostic(mut self, diag: impl Into<String>) -> Self {
        self.diagnostic = Some(diag.into());
        self
    }
}

/// Writes a screen-aligned (billboard) particle quad with rotation and size scaling.
///
/// Layout matches FLOATS_PER_SPRITE_VERTEX (8 floats per vertex, 6 vertices = 48 floats):
/// [anchor.x, anchor.y, anchor.z, offset_x, offset_y, uv.u, uv.v, native_y]
///
/// Offset calculations match C# SceneParticleRenderer:
/// scale = TILES_PER_SPRITE_PIXEL (0.25 * sqrt(2) / 64)
/// offset_x = x_rot * scale
/// offset_y = -y_rot * scale
pub fn write_particle_quad(
    out: &mut Vec<f32>,
    anchor: Vec3,
    native_y: f32,
    rect: ParticleRect,
    uv: AtlasUv,
    rotation: f32,
    size: Vec2,
) {
    let has_rot = rotation.abs() > 1e-6;
    let cos_t = if has_rot { rotation.cos() } else { 1.0 };
    let sin_t = if has_rot { rotation.sin() } else { 0.0 };

    let transform_corner = |x_raw: f32, y_raw: f32| -> (f32, f32) {
        let xs = x_raw * size.x;
        let ys = y_raw * size.y;
        let (xr, yr) = if has_rot {
            (xs * cos_t - ys * sin_t, xs * sin_t + ys * cos_t)
        } else {
            (xs, ys)
        };
        (xr * TILES_PER_SPRITE_PIXEL, -yr * TILES_PER_SPRITE_PIXEL)
    };

    let corners = [
        (rect.left, rect.bottom, uv.u0, uv.v1),
        (rect.right, rect.bottom, uv.u1, uv.v1),
        (rect.right, rect.top, uv.u1, uv.v0),
        (rect.left, rect.bottom, uv.u0, uv.v1),
        (rect.right, rect.top, uv.u1, uv.v0),
        (rect.left, rect.top, uv.u0, uv.v0),
    ];

    for (x_raw, y_raw, u, v) in corners {
        let (ox, oy) = transform_corner(x_raw, y_raw);
        out.extend([anchor.x, anchor.y, anchor.z, ox, oy, u, v, native_y]);
    }
}

/// An individual sorted particle draw item within the frame draw list.
#[derive(Clone, Debug, PartialEq)]
pub struct OrderedParticleDraw {
    pub source_index: usize,
    pub depth: f32,
    pub anchor: Vec3,
    pub native_y: f32,
    pub rect: ParticleRect,
    pub uv: AtlasUv,
    pub rotation: f32,
    pub size: Vec2,
    pub blend_mode: ParticleBlendMode,
    pub first_vertex: usize,
    pub vertex_count: usize,
}

/// CPU-prepared draw command and geometry batch for particle sprites.
///
/// Contains packed 8-float vertices ordered far-to-near, transformation matrices,
/// blend mode, per-item draw descriptors, and draw diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleDrawBatch {
    pub vertices: Vec<f32>,
    pub vertex_count: usize,
    pub quad_count: usize,
    pub view: Mat4,
    pub projection: Mat4,
    pub blend_mode: ParticleBlendMode,
    pub alpha: f32,
    pub draws: Vec<OrderedParticleDraw>,
    pub diagnostics: Vec<String>,
}

impl ParticleDrawBatch {
    pub fn is_empty(&self) -> bool {
        self.quad_count == 0
    }

    /// Borrows the contiguous quad slices as ParticleSpriteDraw items
    /// suitable for SceneCombinedSpritePlan::build.
    pub fn as_sprite_plan_draws<'a>(&'a self) -> Vec<crate::scene::ParticleSpriteDraw<'a>> {
        self.draws
            .iter()
            .filter_map(|d| {
                let start = d.first_vertex * FLOATS_PER_SPRITE_VERTEX;
                let end = start + d.vertex_count * FLOATS_PER_SPRITE_VERTEX;
                self.vertices.get(start..end).map(|v| crate::scene::ParticleSpriteDraw {
                    resolved_mode: d.blend_mode.resolved_mode(),
                    vertices: v,
                })
            })
            .collect()
    }
}

/// Builds a particle draw batch from particle instances, camera view, and projection matrices.
///
/// Replicates C# SceneParticleRenderer::Build far-to-near sorting, depth filtering,
/// vertex geometry generation, and mode diagnostic tracking.
pub fn build_particle_batch(
    instances: &[ParticleInstance],
    view: Mat4,
    projection: Mat4,
) -> ParticleDrawBatch {
    let mut diagnostics = Vec::new();
    let mut accepted = Vec::new();

    for (i, p) in instances.iter().enumerate() {
        if let Some(ref diag) = p.diagnostic {
            diagnostics.push(diag.clone());
            continue;
        }

        let depth = -view.transform_point(p.anchor).z;
        if !depth.is_finite() {
            diagnostics.push("Particle view depth is invalid.".to_string());
            continue;
        }
        if depth <= 0.0 {
            // Particle is behind the camera plane
            continue;
        }

        accepted.push((i, p, depth));
    }

    if accepted.is_empty() {
        return ParticleDrawBatch {
            vertices: Vec::new(),
            vertex_count: 0,
            quad_count: 0,
            view,
            projection,
            blend_mode: ParticleBlendMode::Cutout,
            alpha: 1.0,
            draws: Vec::new(),
            diagnostics,
        };
    }

    // Sort descending by depth (far-to-near). Stable sort preserves tie order.
    accepted.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    let mut vertices = Vec::with_capacity(accepted.len() * QUAD_FLOATS);
    let mut draws = Vec::with_capacity(accepted.len());

    for (src_idx, p, depth) in accepted {
        if let ParticleBlendMode::Native { mode, .. } = p.blend_mode {
            if mode != 0 {
                diagnostics.push(format!("Particle GL mode {} is not implemented.", mode));
            }
        }

        let first_vertex = vertices.len() / FLOATS_PER_SPRITE_VERTEX;
        write_particle_quad(
            &mut vertices,
            p.anchor,
            p.native_y,
            p.rect,
            p.uv,
            p.rotation,
            p.size,
        );

        draws.push(OrderedParticleDraw {
            source_index: src_idx,
            depth,
            anchor: p.anchor,
            native_y: p.native_y,
            rect: p.rect,
            uv: p.uv,
            rotation: p.rotation,
            size: p.size,
            blend_mode: p.blend_mode,
            first_vertex,
            vertex_count: 6,
        });
    }

    let vertex_count = vertices.len() / FLOATS_PER_SPRITE_VERTEX;
    ParticleDrawBatch {
        quad_count: vertex_count / 6,
        vertex_count,
        vertices,
        view,
        projection,
        blend_mode: ParticleBlendMode::Cutout,
        alpha: 1.0,
        draws,
        diagnostics,
    }
}

/// Detached particle renderer matching C# SceneParticleRenderer static class.
pub struct SceneParticleRenderer;

impl SceneParticleRenderer {
    pub fn build(
        instances: &[ParticleInstance],
        view: Mat4,
        projection: Mat4,
    ) -> ParticleDrawBatch {
        build_particle_batch(instances, view, projection)
    }
}

/// Summary counts and diagnostics for a prepared scene frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameDiagnostics {
    pub total_objects: usize,
    pub visible_objects: usize,
    pub culled_objects: usize,
    pub sprite_count: usize,
    pub shadow_count: usize,
    pub marker_count: usize,
    pub particle_count: usize,
    pub selected_lights: usize,
    pub light_diagnostics: SceneLightDiagnostics,
    pub particle_diagnostics: Vec<String>,
}

/// Complete CPU-testable prepared scene frame and draw list.
///
/// Contains all transformed matrices, lighting uniforms, and geometry batches
/// needed to execute a 3D scene rendering pass without any GPU or native dependencies.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedSceneFrame {
    pub viewport: Vec2,
    pub view_matrix: Mat4,
    pub projection_matrix: Mat4,
    pub view_projection_matrix: Mat4,
    pub camera_position: Vec3,
    pub camera_target: Vec3,
    pub camera_ray_center: TerrainRay,
    pub ambient_color: Vec3,
    pub lighting: PreparedSceneLighting,
    pub terrain: TerrainDrawCommand,
    pub water: Option<WaterDrawCommand>,
    pub grid: Option<GridDrawCommand>,
    pub shadows: Option<ShadowDrawBatch>,
    pub sprites: Vec<SpriteDrawBatch>,
    pub ordered_sprites: Vec<OrderedSpriteDraw>,
    pub particles: Option<ParticleDrawBatch>,
    pub markers: Vec<SceneMarkerDraw>,
    pub cursor: Option<BrushCursorDraw>,
    pub diagnostics: FrameDiagnostics,
}

/// Type alias for PreparedSceneFrame, matching standard scene draw list terminology.
pub type SceneDrawList = PreparedSceneFrame;

impl PreparedSceneFrame {
    /// Replicates the native 0x49FFE0 / GLSL GameLightingShader light calculation on the CPU.
    pub fn evaluate_lighting_at(&self, tile_position: Vec3, native_y: f32) -> Vec3 {
        self.lighting.evaluate_lighting(
            self.ambient_color,
            self.terrain.game_lighting_enabled,
            tile_position,
            native_y,
        )
    }

    /// Performs pixel-accurate scene object picking at the specified viewport screen coordinate.
    pub fn pick_object(
        &self,
        screen_point: Vec2,
        objects: &[SceneObject],
        sprites: &[Option<PickableSprite>],
        heights: &TerrainHeightField,
        marker_radius: f32,
    ) -> Option<usize> {
        let positions: Vec<SceneObjectPosition> = objects.iter().map(|o| o.position()).collect();
        pick_object(
            &positions,
            sprites,
            |_idx, _sprite| true,
            heights,
            self.view_matrix,
            self.projection_matrix,
            self.viewport,
            screen_point,
            marker_radius,
            |idx| objects.get(idx).map(|o| o.marker_visible()).unwrap_or(false),
        )
    }

    /// Performs terrain raycast picking at the specified viewport screen coordinate, returning the 3D hit point.
    pub fn pick_terrain(
        &self,
        screen_point: Vec2,
        heights: &TerrainHeightField,
    ) -> Option<Vec3> {
        let ray = self.create_ray(screen_point.x, screen_point.y);
        TerrainRayPicker::try_pick_point(heights, ray)
    }

    /// Performs terrain raycast picking at the specified viewport screen coordinate, returning discrete (tile_x, tile_y).
    pub fn pick_tile(
        &self,
        screen_point: Vec2,
        heights: &TerrainHeightField,
    ) -> Option<(i32, i32)> {
        let ray = self.create_ray(screen_point.x, screen_point.y);
        TerrainRayPicker::try_pick(heights, ray)
    }

    /// Creates a camera ray from a 2D screen coordinate.
    pub fn create_ray(&self, screen_x: f32, screen_y: f32) -> TerrainRay {
        if self.viewport.x <= 0.0 || self.viewport.y <= 0.0 {
            return TerrainRay {
                origin: self.camera_target,
                direction: Vec3::new(0.0, -1.0, 0.0),
            };
        }
        let inv = (self.view_matrix * self.projection_matrix).inverse();
        let unproject = |z: f32| -> Vec3 {
            if let Some(m) = inv {
                let w = m.transform4([
                    screen_x / self.viewport.x * 2.0 - 1.0,
                    1.0 - screen_y / self.viewport.y * 2.0,
                    z * 2.0 - 1.0,
                    1.0,
                ]);
                if w[3].abs() > 1e-12 {
                    Vec3::new(w[0] / w[3], w[1] / w[3], w[2] / w[3])
                } else {
                    self.camera_target
                }
            } else {
                self.camera_target
            }
        };
        let near = unproject(0.0);
        let far = unproject(1.0);
        TerrainRay {
            origin: near,
            direction: (far - near).normalized(),
        }
    }

    /// Projects a 3D world space coordinate to 2D screen viewport pixel coordinates (top-left origin).
    /// Returns None if the point is behind the camera or singular.
    pub fn project_world_to_screen(&self, world_point: Vec3) -> Option<Vec2> {
        let clip = self.view_projection_matrix.transform4([world_point.x, world_point.y, world_point.z, 1.0]);
        if clip[3] <= 1e-6 {
            return None;
        }
        let sx = (clip[0] / clip[3] * 0.5 + 0.5) * self.viewport.x;
        let sy = (0.5 - clip[1] / clip[3] * 0.5) * self.viewport.y;
        Some(Vec2::new(sx, sy))
    }

    /// Returns the total number of quads across all geometry batches (water, shadows, sprites, particles).
    pub fn total_quads(&self) -> usize {
        let mut count = 0;
        if self.water.is_some() {
            count += 1;
        }
        if let Some(ref sh) = self.shadows {
            count += sh.quad_count;
        }
        for sp in &self.sprites {
            count += sp.quad_count;
        }
        if let Some(ref pt) = self.particles {
            count += pt.quad_count;
        }
        count
    }

    /// Returns the total number of vertices across all geometry batches, markers, and overlays.
    pub fn total_vertices(&self) -> usize {
        let mut count = 0;
        if self.water.is_some() {
            count += 4;
        }
        if let Some(ref sh) = self.shadows {
            count += sh.vertex_count;
        }
        for sp in &self.sprites {
            count += sp.vertex_count;
        }
        if let Some(ref pt) = self.particles {
            count += pt.vertex_count;
        }
        count += self.markers.len();
        if let Some(ref c) = self.cursor {
            count += c.perimeter_points.len();
        }
        count
    }

    /// Returns the total number of distinct render passes/draw calls in this frame.
    pub fn total_draw_calls(&self) -> usize {
        let mut calls = 1; // Terrain is always present
        if self.grid.is_some() {
            calls += 1;
        }
        if self.water.is_some() {
            calls += 1;
        }
        if self.shadows.is_some() {
            calls += 1;
        }
        calls += self.sprites.len();
        if let Some(ref pt) = self.particles {
            if !pt.is_empty() {
                calls += 1;
            }
        }
        if !self.markers.is_empty() {
            calls += 1;
        }
        if self.cursor.is_some() {
            calls += 1;
        }
        calls
    }
}

/// Prepares a complete scene frame and draw list from SceneObjects and camera/light APIs.
pub fn prepare_scene_frame(
    camera: &EditorCamera,
    viewport: Vec2,
    heights: &TerrainHeightField,
    objects: &[SceneObject],
    sprites: &[Option<Sprite>],
    shadows: &[Option<Shadow>],
    mut lighting_context: Option<&mut SceneLightingContext>,
    saved_lights: Option<&SavedSceneLightSet>,
    options: &SceneFrameOptions,
) -> Result<PreparedSceneFrame, &'static str> {
    if !viewport.x.is_finite() || !viewport.y.is_finite() || viewport.x <= 0.0 || viewport.y <= 0.0 {
        return Err("viewport width and height must be finite and positive");
    }

    let aspect = viewport.x / viewport.y.max(1.0);
    let view = camera.view_matrix();
    let projection = camera.projection_matrix(aspect);
    let view_proj = view * projection;
    let camera_position = camera.position();
    let camera_target = camera.target;
    let camera_ray_center = camera.create_ray(viewport.x * 0.5, viewport.y * 0.5, viewport.x, viewport.y);

    // 1. Lighting preparation
    let ambient_color = if options.game_lighting_enabled {
        lighting_context
            .as_ref()
            .map(|ctx| ctx.get_ambient_color(options.game_hour))
            .unwrap_or(Vec3::new(1.0, 1.0, 1.0))
    } else {
        Vec3::new(1.0, 1.0, 1.0)
    };

    let mut all_lights = Vec::new();
    let mut synthesized_count = 0;
    if options.game_lighting_enabled {
        if let Some(ref mut ctx) = lighting_context {
            let light_items: Vec<_> = objects.iter().map(|o| o.as_light_source_item()).collect();
            let synth = ctx.collect_scene_lights(
                &light_items,
                |x, z| heights.sample_height(x / 256.0, z / 256.0) * 256.0,
                saved_lights.map(|s| &s.suppressed_objects),
            );
            synthesized_count = synth.len();
            all_lights.extend(synth);
        }
    }

    let mut suppressed_count = 0;
    let total_saved = saved_lights.map(|s| s.instances.len()).unwrap_or(0);
    let renderable_saved = saved_lights
        .map(|s| s.instances.iter().filter(|l| l.is_renderable()).count())
        .unwrap_or(0);

    if let Some(saved) = saved_lights {
        if !saved.suppressed_objects.is_empty() {
            for obj in objects {
                if let Some(k) = obj.suppression_key() {
                    if saved.suppressed_objects.contains(&k) {
                        suppressed_count += 1;
                    }
                }
            }
        }
        all_lights.extend(saved.instances.clone());
    }

    let target_world = camera_target * 256.0;
    let light_diag = SceneLightDiagnostics {
        total_saved,
        renderable_saved,
        suppressed_objects: suppressed_count,
        synthesized_lights: synthesized_count,
        native_height_known: heights.has_native_query(),
    };
    let prepared_lighting = PreparedSceneLighting::prepare(&all_lights, target_world, light_diag);

    // 2. Terrain draw command
    let dimension = heights.tile_width.max(heights.tile_height);
    let terrain_cmd = TerrainDrawCommand {
        mvp: view_proj,
        sun_light: Vec3::new(0.4, 0.85, 0.3),
        ambient_color,
        game_lighting_enabled: options.game_lighting_enabled,
        lighting: prepared_lighting.clone(),
        native_height_known: heights.has_native_query(),
        native_height_grid: [
            heights.width() as f32,
            heights.height() as f32,
            heights.tile_width,
            heights.tile_height,
        ],
        dimension,
    };

    // 3. Grid draw command
    let grid_cmd = if options.show_grid {
        Some(GridDrawCommand { mvp: view_proj })
    } else {
        None
    };

    // 4. Water draw command
    let water_cmd = if options.show_water {
        let is_below_minimum = heights.has_native_query()
            && heights
                .minimum_native_height()
                .map(|min| options.water_level < min)
                .unwrap_or(false);
        if is_below_minimum {
            None
        } else {
            let water_y = if options.height_map_step <= 0.0 {
                0.0
            } else {
                options.water_level / options.height_map_step / 255.0 * heights.height_scale
            };
            Some(WaterDrawCommand {
                water_y,
                vertices: [
                    Vec3::new(0.0, water_y, 0.0),
                    Vec3::new(0.0, water_y, dimension),
                    Vec3::new(dimension, water_y, dimension),
                    Vec3::new(dimension, water_y, 0.0),
                ],
                color: options.water_color,
            })
        }
    } else {
        None
    };

    // 5. Culling & Visible object filtering
    let mut visible_indices = Vec::with_capacity(objects.len());
    let mut culled_count = 0;
    for (i, obj) in objects.iter().enumerate() {
        if options.culling_enabled {
            let footprint = match obj.kind() {
                SceneObjectKind::Building => IsometricFootprint::LONGHOUSE,
                SceneObjectKind::Unit => IsometricFootprint::UNIT,
                SceneObjectKind::Object => IsometricFootprint::POINT,
            };
            let anchor = ground_point(obj.position(), heights, 0.0);
            let sort_item = IsometricSortItem::new(
                obj.object_index,
                anchor,
                footprint,
                IsometricRenderLayer::StandardObject,
                0,
            );
            if is_visible(&sort_item, view_proj, viewport, options.culling_padding) {
                visible_indices.push(i);
            } else {
                culled_count += 1;
            }
        } else {
            visible_indices.push(i);
        }
    }

    // 6. Markers
    let mut markers = Vec::new();
    if options.show_markers {
        for &idx in &visible_indices {
            let obj = &objects[idx];
            let has_sprite = sprites.get(idx).and_then(|s| *s).is_some();
            if !has_sprite && obj.marker_visible() {
                let is_script = obj.source_file.eq_ignore_ascii_case("DATA/objects.dat")
                    || obj.name.to_ascii_lowercase().starts_with("skriptmark");
                let (color, point_size) = if is_script {
                    (SCRIPT_MARK_COLOR, 5.0)
                } else {
                    (team_color(obj.team), 10.0)
                };
                markers.push(SceneMarkerDraw {
                    object_index: idx,
                    point: ground_point(obj.position(), heights, 0.25),
                    color,
                    point_size,
                    is_script_mark: is_script,
                });
            }
        }
    }

    // 7. Ground Shadows
    let shadow_batch = if options.show_shadows {
        let shadow_positions: Vec<_> = visible_indices.iter().map(|&i| objects[i].position()).collect();
        let shadow_refs: Vec<_> = visible_indices
            .iter()
            .map(|&i| shadows.get(i).copied().flatten())
            .collect();
        let shadow_verts = build_shadow_vertices(&shadow_positions, &shadow_refs, heights);
        if shadow_verts.is_empty() {
            None
        } else {
            let vertex_count = shadow_verts.len() / 5;
            Some(ShadowDrawBatch {
                quad_count: vertex_count / 6,
                vertex_count,
                vertices: shadow_verts,
                mvp: view_proj,
            })
        }
    } else {
        None
    };

    // 8. Sprites
    let mut sprite_batches = Vec::new();
    let mut ordered_sprites = Vec::new();
    if options.show_objects {
        let sprite_positions: Vec<_> = visible_indices.iter().map(|&i| objects[i].position()).collect();
        let sprite_refs: Vec<_> = visible_indices
            .iter()
            .map(|&i| sprites.get(i).copied().flatten())
            .collect();
        let mut sprite_buf = SceneSpriteGeometryBuffer::default();
        let sprite_vertices = sprite_buf.build(&sprite_positions, &sprite_refs, heights, view).to_vec();

        // Extract ordered sprites with their view depth
        for (local_idx, &global_idx) in visible_indices.iter().enumerate() {
            if let Some(Some(sprite)) = sprite_refs.get(local_idx) {
                if let Some(Some(offset)) = sprite_buf.object_offsets().get(local_idx) {
                    let anchor = ground_point(objects[global_idx].position(), heights, 0.0);
                    let depth = -view.transform_point(anchor).z;
                    ordered_sprites.push(OrderedSpriteDraw {
                        object_index: global_idx,
                        depth,
                        anchor,
                        native_y: native_query_y(objects[global_idx].position(), heights),
                        sprite: *sprite,
                        first_vertex: *offset / FLOATS_PER_SPRITE_VERTEX,
                        vertex_count: 6,
                    });
                }
            }
        }

        // Sort descending by depth (far-to-near)
        ordered_sprites.sort_by(|a, b| b.depth.partial_cmp(&a.depth).unwrap_or(std::cmp::Ordering::Equal));

        if !sprite_vertices.is_empty() {
            let vertex_count = sprite_vertices.len() / FLOATS_PER_SPRITE_VERTEX;
            sprite_batches.push(SpriteDrawBatch {
                vertices: sprite_vertices,
                vertex_count,
                quad_count: vertex_count / 6,
                view,
                projection,
                alpha: 1.0,
                is_preview: false,
            });
        }

        // Ghost placement preview
        if let Some(ref prev) = options.placement_preview {
            if prev.hover_tile_x >= 0 && prev.hover_tile_y >= 0 {
                let ghost_x = (prev.hover_tile_x as f32 + 0.5) * 256.0;
                let ghost_z = (prev.hover_tile_y as f32 + 0.5) * 256.0;
                let ghost_pos = SceneObjectPosition {
                    world: Vec3::new(ghost_x, 0.0, ghost_z),
                    local_x: 0.0,
                    local_z: 0.0,
                    has_native_saved_position: false,
                };
                let ghost_verts = build_sprite_vertices(&[ghost_pos], &[Some(prev.sprite)], heights, view);
                if !ghost_verts.is_empty() {
                    let vertex_count = ghost_verts.len() / FLOATS_PER_SPRITE_VERTEX;
                    sprite_batches.push(SpriteDrawBatch {
                        vertices: ghost_verts,
                        vertex_count,
                        quad_count: vertex_count / 6,
                        view,
                        projection,
                        alpha: prev.alpha,
                        is_preview: true,
                    });
                }
            }
        }
    }

    // 9. Brush Cursor
    let cursor_cmd = if let Some(ref c) = options.brush_cursor {
        if c.hover_tile_x >= 0 && c.hover_tile_y >= 0 {
            let (x0, y0, x1, y1) = if let Some(sel) = c.selection {
                (sel.left, sel.top, sel.right, sel.bottom)
            } else {
                let radius = c.brush_size.max(0) / 2;
                let x0 = (c.hover_tile_x - radius).max(0);
                let y0 = (c.hover_tile_y - radius).max(0);
                let x1 = (c.hover_tile_x + radius).min(heights.tile_width as i32 - 1) + 1;
                let y1 = (c.hover_tile_y + radius).min(heights.tile_height as i32 - 1) + 1;
                (x0, y0, x1, y1)
            };

            let cursor_point = |tx: i32, ty: i32| -> Vec3 {
                Vec3::new(
                    tx as f32,
                    heights.sample_height(tx as f32, ty as f32) + 0.15,
                    ty as f32,
                )
            };

            let mut perimeter = Vec::new();
            for x in x0..=x1 {
                perimeter.push(cursor_point(x, y0));
            }
            for y in (y0 + 1)..=y1 {
                perimeter.push(cursor_point(x1, y));
            }
            for x in (x0..=(x1 - 1)).rev() {
                perimeter.push(cursor_point(x, y1));
            }
            for y in ((y0 + 1)..=(y1 - 1)).rev() {
                perimeter.push(cursor_point(x0, y));
            }

            Some(BrushCursorDraw {
                perimeter_points: perimeter,
                color: c.color,
                line_width: c.line_width,
            })
        } else {
            None
        }
    } else {
        None
    };

    // 10. Particles
    let particle_batch = if options.show_particles && !options.particles.is_empty() {
        let batch = build_particle_batch(&options.particles, view, projection);
        Some(batch)
    } else {
        None
    };

    let particle_count = particle_batch.as_ref().map(|b| b.quad_count).unwrap_or(0);
    let particle_diagnostics = particle_batch
        .as_ref()
        .map(|b| b.diagnostics.clone())
        .unwrap_or_default();

    let total_sprites: usize = sprite_batches.iter().map(|b| b.quad_count).sum();
    let total_shadows = shadow_batch.as_ref().map(|b| b.quad_count).unwrap_or(0);
    let diagnostics = FrameDiagnostics {
        total_objects: objects.len(),
        visible_objects: visible_indices.len(),
        culled_objects: culled_count,
        sprite_count: total_sprites,
        shadow_count: total_shadows,
        marker_count: markers.len(),
        particle_count,
        selected_lights: prepared_lighting.selected_count,
        light_diagnostics: light_diag,
        particle_diagnostics,
    };

    Ok(PreparedSceneFrame {
        viewport,
        view_matrix: view,
        projection_matrix: projection,
        view_projection_matrix: view_proj,
        camera_position,
        camera_target,
        camera_ray_center,
        ambient_color,
        lighting: prepared_lighting,
        terrain: terrain_cmd,
        water: water_cmd,
        grid: grid_cmd,
        shadows: shadow_batch,
        sprites: sprite_batches,
        ordered_sprites,
        particles: particle_batch,
        markers,
        cursor: cursor_cmd,
        diagnostics,
    })
}

/// Fluent builder for constructing a prepared scene frame.
pub struct SceneFrameBuilder<'a> {
    pub camera: &'a EditorCamera,
    pub viewport: Vec2,
    pub heights: &'a TerrainHeightField,
    pub objects: &'a [SceneObject],
    pub sprites: &'a [Option<Sprite>],
    pub shadows: &'a [Option<Shadow>],
    pub lighting_context: Option<&'a mut SceneLightingContext>,
    pub saved_lights: Option<&'a SavedSceneLightSet>,
    pub options: SceneFrameOptions,
}

impl<'a> SceneFrameBuilder<'a> {
    pub fn new(
        camera: &'a EditorCamera,
        viewport: Vec2,
        heights: &'a TerrainHeightField,
        objects: &'a [SceneObject],
    ) -> Self {
        Self {
            camera,
            viewport,
            heights,
            objects,
            sprites: &[],
            shadows: &[],
            lighting_context: None,
            saved_lights: None,
            options: SceneFrameOptions::default(),
        }
    }

    pub fn sprites(mut self, sprites: &'a [Option<Sprite>]) -> Self {
        self.sprites = sprites;
        self
    }

    pub fn shadows(mut self, shadows: &'a [Option<Shadow>]) -> Self {
        self.shadows = shadows;
        self
    }

    pub fn particles(mut self, particles: Vec<ParticleInstance>) -> Self {
        self.options.particles = particles;
        self
    }

    pub fn lighting_context(mut self, ctx: &'a mut SceneLightingContext) -> Self {
        self.lighting_context = Some(ctx);
        self
    }

    pub fn saved_lights(mut self, saved: &'a SavedSceneLightSet) -> Self {
        self.saved_lights = Some(saved);
        self
    }

    pub fn options(mut self, mut options: SceneFrameOptions) -> Self {
        if options.particles.is_empty() && !self.options.particles.is_empty() {
            options.particles = std::mem::take(&mut self.options.particles);
        }
        self.options = options;
        self
    }

    pub fn build(self) -> Result<PreparedSceneFrame, &'static str> {
        prepare_scene_frame(
            self.camera,
            self.viewport,
            self.heights,
            self.objects,
            self.sprites,
            self.shadows,
            self.lighting_context,
            self.saved_lights,
            &self.options,
        )
    }
}
