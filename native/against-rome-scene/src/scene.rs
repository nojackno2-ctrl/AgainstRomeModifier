use crate::{atlas::AtlasUv, terrain::TerrainHeightField, Mat4, Vec3};

pub const WORLD_UNITS_PER_TILE: f32 = 256.0;
pub const MAP_WORLD_SIZE: f32 = 16384.0;
pub const TILES_PER_SPRITE_PIXEL: f32 = 0.25 * 1.41421356 / 64.0;
pub const FLOATS_PER_SPRITE_VERTEX: usize = 8;
pub const QUAD_FLOATS: usize = 48;
pub const UNKNOWN_NATIVE_QUERY_Y: f32 = f32::MAX;

/// Minimal positioning projection. The caller retains object metadata and applies these fields.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SceneObjectPosition {
    pub world: Vec3,
    pub local_x: f32,
    pub local_z: f32,
    pub has_native_saved_position: bool,
}

pub fn move_to_world_position(
    source: SceneObjectPosition,
    x: f32,
    z: f32,
) -> Result<SceneObjectPosition, &'static str> {
    if !x.is_finite() || !z.is_finite() {
        return Err("scene-object coordinates must be finite");
    }
    let clamped_x = x.clamp(0.0, MAP_WORLD_SIZE);
    let clamped_z = z.clamp(0.0, MAP_WORLD_SIZE);
    Ok(SceneObjectPosition {
        world: Vec3::new(clamped_x, source.world.y, clamped_z),
        local_x: source.local_x + clamped_x - source.world.x,
        local_z: source.local_z + clamped_z - source.world.z,
        ..source
    })
}

pub fn ground_point(item: SceneObjectPosition, h: &TerrainHeightField, lift: f32) -> Vec3 {
    let x = item.world.x / WORLD_UNITS_PER_TILE;
    let z = item.world.z / WORLD_UNITS_PER_TILE;
    let y = if item.has_native_saved_position {
        h.native_height(item.world.y)
    } else {
        None
    }
    .unwrap_or_else(|| h.sample_height(x, z));
    Vec3::new(x, y + lift, z)
}

pub fn native_query_y(item: SceneObjectPosition, h: &TerrainHeightField) -> f32 {
    if item.has_native_saved_position {
        if item.world.y.is_finite() {
            item.world.y
        } else {
            UNKNOWN_NATIVE_QUERY_Y
        }
    } else {
        h.native_query_height(
            item.world.x / WORLD_UNITS_PER_TILE,
            item.world.z / WORLD_UNITS_PER_TILE,
        )
        .unwrap_or(UNKNOWN_NATIVE_QUERY_Y)
    }
}

pub fn build_marker_points(objects: &[SceneObjectPosition], heights: &TerrainHeightField) -> Vec<Vec3> {
    objects
        .iter()
        .map(|&item| ground_point(item, heights, 0.25))
        .collect()
}

/// Resolved sprite metadata: None in an input slice means missing sprite or atlas UV.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sprite {
    pub width: f32,
    pub height: f32,
    pub anchor_x: f32,
    pub anchor_y: f32,
    pub uv: AtlasUv,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct VisibleSprite {
    depth: f32,
    index: usize,
    anchor: Vec3,
    native_y: f32,
    sprite: Sprite,
}

/// Reuses allocations across animation, rotation, movement and terrain edits.
/// Value-based recomputation avoids C# reference-identity stale-data hazards.
#[derive(Default, Debug)]
pub struct SceneSpriteGeometryBuffer {
    vertices: Vec<f32>,
    visible: Vec<VisibleSprite>,
    object_offsets: Vec<Option<usize>>,
}

impl SceneSpriteGeometryBuffer {
    pub fn vertices(&self) -> &[f32] {
        &self.vertices
    }

    pub fn object_offsets(&self) -> &[Option<usize>] {
        &self.object_offsets
    }

    pub fn build(
        &mut self,
        objects: &[SceneObjectPosition],
        sprites: &[Option<Sprite>],
        h: &TerrainHeightField,
        view: Mat4,
    ) -> &[f32] {
        self.visible.clear();
        self.object_offsets.clear();
        self.object_offsets.resize(objects.len(), None);

        for (i, (&object, sprite)) in objects.iter().zip(sprites).enumerate() {
            if let Some(sprite) = sprite {
                let anchor = ground_point(object, h, 0.0);
                self.visible.push(VisibleSprite {
                    depth: -view.transform_point(anchor).z,
                    index: i,
                    anchor,
                    native_y: native_query_y(object, h),
                    sprite: *sprite,
                });
            }
        }

        // Far-to-near sorting (descending depth). Stable sort on equal depths.
        self.visible.sort_by(|a, b| float_compare_descending(a.depth, b.depth));

        self.vertices.clear();
        for item in &self.visible {
            self.object_offsets[item.index] = Some(self.vertices.len());
            write_quad(&mut self.vertices, item.anchor, item.native_y, item.sprite);
        }
        &self.vertices
    }
}

fn float_compare_descending(a: f32, b: f32) -> std::cmp::Ordering {
    match (a.is_nan(), b.is_nan()) {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        (false, false) => b.partial_cmp(&a).unwrap_or(std::cmp::Ordering::Equal),
    }
}

pub fn write_quad(out: &mut Vec<f32>, anchor: Vec3, native_y: f32, s: Sprite) {
    let left = -s.anchor_x * TILES_PER_SPRITE_PIXEL;
    let right = (s.width - s.anchor_x) * TILES_PER_SPRITE_PIXEL;
    let top = s.anchor_y * TILES_PER_SPRITE_PIXEL;
    let bottom = (s.anchor_y - s.height) * TILES_PER_SPRITE_PIXEL;

    for (x, y, u, v) in [
        (left, bottom, s.uv.u0, s.uv.v1),
        (right, bottom, s.uv.u1, s.uv.v1),
        (right, top, s.uv.u1, s.uv.v0),
        (left, bottom, s.uv.u0, s.uv.v1),
        (right, top, s.uv.u1, s.uv.v0),
        (left, top, s.uv.u0, s.uv.v0),
    ] {
        out.extend([anchor.x, anchor.y, anchor.z, x, y, u, v, native_y]);
    }
}

pub fn build_sprite_vertices(
    objects: &[SceneObjectPosition],
    sprites: &[Option<Sprite>],
    heights: &TerrainHeightField,
    view: Mat4,
) -> Vec<f32> {
    let mut buffer = SceneSpriteGeometryBuffer::default();
    buffer.build(objects, sprites, heights, view).to_vec()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub correction_x: f64,
    pub correction_z: f64,
    pub half_extent: f64,
    pub uv: AtlasUv,
}

#[derive(Default, Debug)]
pub struct SceneShadowGeometryCache {
    vertices: Vec<f32>,
    objects: Vec<SceneObjectPosition>,
    shadows: Vec<Option<Shadow>>,
    heights: Option<TerrainHeightField>,
}

impl SceneShadowGeometryCache {
    pub fn build(
        &mut self,
        objects: &[SceneObjectPosition],
        shadows: &[Option<Shadow>],
        h: &TerrainHeightField,
    ) -> &[f32] {
        if self.objects == objects && self.shadows == shadows && self.heights.as_ref() == Some(h) {
            return &self.vertices;
        }
        self.vertices.clear();
        for (&item, shadow) in objects.iter().zip(shadows) {
            let Some(s) = shadow else { continue };
            let cx = item.world.x as f64 + s.correction_x;
            let cz = item.world.z as f64 + s.correction_z;
            let e = s.half_extent;
            for (wx, wz, u, v) in [
                (cx - e, cz - e, s.uv.u0, s.uv.v0),
                (cx + e, cz - e, s.uv.u1, s.uv.v0),
                (cx + e, cz + e, s.uv.u1, s.uv.v1),
                (cx - e, cz - e, s.uv.u0, s.uv.v0),
                (cx + e, cz + e, s.uv.u1, s.uv.v1),
                (cx - e, cz + e, s.uv.u0, s.uv.v1),
            ] {
                let x = (wx / 256.0) as f32;
                let z = (wz / 256.0) as f32;
                self.vertices.extend([
                    x,
                    h.sample_height(x.clamp(0.0, h.tile_width), z.clamp(0.0, h.tile_height)) + 0.03,
                    z,
                    u,
                    v,
                ]);
            }
        }
        self.objects.clear();
        self.objects.extend_from_slice(objects);
        self.shadows.clear();
        self.shadows.extend_from_slice(shadows);
        self.heights = Some(h.clone());
        &self.vertices
    }
}

pub fn build_shadow_vertices(
    objects: &[SceneObjectPosition],
    shadows: &[Option<Shadow>],
    heights: &TerrainHeightField,
) -> Vec<f32> {
    let mut cache = SceneShadowGeometryCache::default();
    cache.build(objects, shadows, heights).to_vec()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneSpriteDrawKind {
    Object,
    ParticleReplacement,
}

#[derive(Clone, Copy, Debug)]
pub struct ParticleSpriteDraw<'a> {
    pub resolved_mode: Option<i32>,
    pub vertices: &'a [f32],
}

#[derive(Clone, Copy, Debug)]
pub struct SceneSpriteDraw<'a> {
    pub kind: SceneSpriteDrawKind,
    pub source_index: usize,
    pub vertices: &'a [f32],
    pub first_vertex: usize,
    pub vertex_count: usize,
    pub depth: f32,
}

/// Orders scene draw items by depth descending (far to near).
/// Preserves input stability on ties (matching RustAlrFrameDecoder.OrderSceneDraws in C#).
/// Rejects any nonfinite depth with an error before returning.
pub fn order_scene_draws(depths: &[f32]) -> Result<Vec<usize>, &'static str> {
    for &d in depths {
        if !d.is_finite() {
            return Err("nonfinite scene depth");
        }
    }
    let mut indices: Vec<usize> = (0..depths.len()).collect();
    indices.sort_by(|&a, &b| depths[b].partial_cmp(&depths[a]).unwrap());
    Ok(indices)
}

/// Borrows source buffers, recomputes particle depths with the current view, stable on ties.
pub fn combined_sprite_plan<'a>(
    objects: &'a [f32],
    particles: &[ParticleSpriteDraw<'a>],
    view: Mat4,
) -> Result<Vec<SceneSpriteDraw<'a>>, &'static str> {
    if objects.len() % QUAD_FLOATS != 0 {
        return Err("object sprite buffer does not contain complete quads");
    }
    let mut pending = Vec::new();
    let depth = |v: &[f32]| -view.transform_point(Vec3::new(v[0], v[1], v[2])).z;
    for (i, v) in objects.chunks_exact(QUAD_FLOATS).enumerate() {
        let d = depth(v);
        pending.push(SceneSpriteDraw {
            kind: SceneSpriteDrawKind::Object,
            source_index: i,
            vertices: objects,
            first_vertex: i * 6,
            vertex_count: 6,
            depth: d,
        });
    }
    for (i, p) in particles.iter().enumerate() {
        if p.resolved_mode != Some(0) {
            continue;
        }
        if p.vertices.len() != QUAD_FLOATS {
            return Err("particle sprite buffer does not contain one complete quad");
        }
        let d = depth(p.vertices);
        pending.push(SceneSpriteDraw {
            kind: SceneSpriteDrawKind::ParticleReplacement,
            source_index: i,
            vertices: p.vertices,
            first_vertex: 0,
            vertex_count: 6,
            depth: d,
        });
    }
    if pending.is_empty() {
        return Ok(Vec::new());
    }
    let depths: Vec<f32> = pending.iter().map(|d| d.depth).collect();
    let order = order_scene_draws(&depths)?;
    Ok(order.into_iter().map(|idx| pending[idx]).collect())
}

/// Struct-based wrapper matching C# SceneCombinedSpritePlan static class.
pub struct SceneCombinedSpritePlan;

impl SceneCombinedSpritePlan {
    pub fn build<'a>(
        objects: &'a [f32],
        particles: &[ParticleSpriteDraw<'a>],
        view: Mat4,
    ) -> Result<Vec<SceneSpriteDraw<'a>>, &'static str> {
        combined_sprite_plan(objects, particles, view)
    }
}
