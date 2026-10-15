use crate::{Mat4, Vec2, Vec3};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum IsometricRenderLayer {
    TerrainDecal = 0,
    GroundShadow = 1,
    TerrainAttachment = 2,
    StandardObject = 3,
    OverheadCanopy = 4,
    AirborneEffect = 5,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IsometricFootprint {
    pub width_x: f32,
    pub depth_z: f32,
    pub height_y: f32,
}

impl IsometricFootprint {
    pub const POINT: Self = Self {
        width_x: 0.0,
        depth_z: 0.0,
        height_y: 0.0,
    };
    pub const UNIT: Self = Self {
        width_x: 0.5,
        depth_z: 0.5,
        height_y: 1.8,
    };
    pub const LONGHOUSE: Self = Self {
        width_x: 4.0,
        depth_z: 3.0,
        height_y: 6.0,
    };
    pub const ROMAN_TOWER: Self = Self {
        width_x: 2.0,
        depth_z: 2.0,
        height_y: 14.0,
    };

    pub const fn new(width_x: f32, depth_z: f32, height_y: f32) -> Self {
        Self {
            width_x,
            depth_z,
            height_y,
        }
    }

    pub fn southmost_ground_offset(self) -> f32 {
        (self.width_x + self.depth_z) * 0.5
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IsometricSortItem {
    pub item_id: i32,
    pub position: Vec3,
    pub footprint: IsometricFootprint,
    pub layer: IsometricRenderLayer,
    sub_priority: i32,
}

impl IsometricSortItem {
    pub fn new(
        item_id: i32,
        position: Vec3,
        footprint: IsometricFootprint,
        layer: IsometricRenderLayer,
        sub_priority: i32,
    ) -> Self {
        Self {
            item_id,
            position,
            footprint,
            layer,
            sub_priority: sub_priority.clamp(-128, 127),
        }
    }

    pub fn with_coords(
        item_id: i32,
        world_x: f32,
        world_y: f32,
        world_z: f32,
        footprint: IsometricFootprint,
        layer: IsometricRenderLayer,
        sub_priority: i32,
    ) -> Self {
        Self::new(
            item_id,
            Vec3::new(world_x, world_y, world_z),
            footprint,
            layer,
            sub_priority,
        )
    }

    pub fn sub_priority(self) -> i32 {
        self.sub_priority
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct IsometricDepthKey(pub u64);

pub fn compute_key(item: &IsometricSortItem) -> IsometricDepthKey {
    let ground = item.position.x + item.position.z + item.footprint.southmost_ground_offset();
    let fixed = (ground.clamp(0.0, 65535.0) * 65536.0) as u64;
    let elev = ((item.position.y.clamp(0.0, 255.0) * 16.0) as u32) & 0xfff;
    let tie = (((item.sub_priority + 128) & 0xff) as u32) << 4
        | ((item.item_id.wrapping_mul(73856093) as u32) & 15);
    IsometricDepthKey(
        ((item.layer as u64) << 56)
            | ((fixed & 0xffffffff) << 24)
            | ((elev as u64) << 12)
            | (tie as u64 & 0xfff),
    )
}

pub fn compute_continuous_depth(item: &IsometricSortItem) -> f32 {
    item.layer as u8 as f32 * 10000.0
        + (item.position.x + item.position.z + item.footprint.southmost_ground_offset())
        + item.position.y * 0.001
        + item.sub_priority as f32 * 0.0001
}

pub fn should_draw_before(a: &IsometricSortItem, b: &IsometricSortItem) -> bool {
    if a.layer != b.layer {
        return a.layer < b.layer;
    }
    let ax = a.position.x + a.footprint.width_x;
    let az = a.position.z + a.footprint.depth_z;
    let bx = b.position.x + b.footprint.width_x;
    let bz = b.position.z + b.footprint.depth_z;
    if ax <= b.position.x && az <= b.position.z {
        return true;
    }
    if bx <= a.position.x && bz <= a.position.z {
        return false;
    }
    if ax + az <= b.position.x + b.position.z {
        return true;
    }
    if bx + bz <= a.position.x + a.position.z {
        return false;
    }
    compute_key(a) < compute_key(b)
}

/// Stable input order for equal keys.
pub fn sort_indices(items: &[IsometricSortItem]) -> Vec<usize> {
    let mut out: Vec<usize> = (0..items.len()).collect();
    out.sort_by_key(|&i| compute_key(&items[i]));
    out
}

pub fn sort_items(items: &mut [IsometricSortItem]) {
    items.sort_by_key(compute_key);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenAabb {
    pub min: Vec2,
    pub max: Vec2,
}

impl ScreenAabb {
    pub const fn new(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        Self {
            min: Vec2::new(min_x, min_y),
            max: Vec2::new(max_x, max_y),
        }
    }

    pub fn intersects_viewport(self, viewport: Vec2) -> bool {
        self.max.x >= 0.0 && self.min.x <= viewport.x && self.max.y >= 0.0 && self.min.y <= viewport.y
    }

    pub fn intersects(self, b: Self) -> bool {
        self.max.x >= b.min.x && self.min.x <= b.max.x && self.max.y >= b.min.y && self.min.y <= b.max.y
    }

    pub fn padded(self, v: Vec2, p: f32) -> bool {
        self.max.x >= -p && self.min.x <= v.x + p && self.max.y >= -p && self.min.y <= v.y + p
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldAabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl WorldAabb {
    pub const fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn from_sort_item(i: &IsometricSortItem) -> Self {
        Self {
            min: i.position,
            max: i.position
                + Vec3::new(
                    i.footprint.width_x.max(0.5),
                    i.footprint.height_y.max(1.0),
                    i.footprint.depth_z.max(0.5),
                ),
        }
    }

    pub fn expand(&mut self, b: Self) {
        self.min = Vec3::new(
            self.min.x.min(b.min.x),
            self.min.y.min(b.min.y),
            self.min.z.min(b.min.z),
        );
        self.max = Vec3::new(
            self.max.x.max(b.max.x),
            self.max.y.max(b.max.y),
            self.max.z.max(b.max.z),
        );
    }
}

/// Projects all eight corners, retaining C#'s near-zero-W fallback (not a 3D clip test).
pub fn project_aabb_to_screen(a: WorldAabb, m: Mat4, viewport: Vec2) -> ScreenAabb {
    let mut out = ScreenAabb {
        min: Vec2::new(f32::MAX, f32::MAX),
        max: Vec2::new(f32::MIN, f32::MIN),
    };
    for x in [a.min.x, a.max.x] {
        for y in [a.min.y, a.max.y] {
            for z in [a.min.z, a.max.z] {
                let c = m.transform4([x, y, z, 1.0]);
                let w = if c[3].abs() < 1e-6 { 1.0 } else { c[3] };
                let sx = (c[0] / w * 0.5 + 0.5) * viewport.x;
                let sy = (0.5 - c[1] / w * 0.5) * viewport.y;
                out.min.x = out.min.x.min(sx);
                out.min.y = out.min.y.min(sy);
                out.max.x = out.max.x.max(sx);
                out.max.y = out.max.y.max(sy);
            }
        }
    }
    out
}

pub fn is_visible(item: &IsometricSortItem, m: Mat4, v: Vec2, padding: f32) -> bool {
    project_aabb_to_screen(WorldAabb::from_sort_item(item), m, v).padded(v, padding)
}

pub fn cull_items(
    items: &[IsometricSortItem],
    m: Mat4,
    v: Vec2,
    out: &mut [usize],
    padding: f32,
) -> Result<usize, &'static str> {
    if out.len() < items.len() {
        return Err("visible buffer shorter than items");
    }
    let mut n = 0;
    for (i, item) in items.iter().enumerate() {
        if is_visible(item, m, v, padding) {
            out[n] = i;
            n += 1;
        }
    }
    Ok(n)
}

#[derive(Clone, Debug)]
pub struct SpatialChunk {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub object_indices: Vec<usize>,
    pub bounding_box: WorldAabb,
}

impl SpatialChunk {
    pub const DEFAULT_CHUNK_SIZE: f32 = 32.0;

    pub fn new(x: i32, z: i32) -> Self {
        Self {
            chunk_x: x,
            chunk_z: z,
            object_indices: Vec::new(),
            bounding_box: WorldAabb {
                min: Vec3::new(x as f32 * Self::DEFAULT_CHUNK_SIZE, 0.0, z as f32 * Self::DEFAULT_CHUNK_SIZE),
                max: Vec3::new((x as f32 + 1.0) * Self::DEFAULT_CHUNK_SIZE, 0.0, (z as f32 + 1.0) * Self::DEFAULT_CHUNK_SIZE),
            },
        }
    }

    pub fn expand_bounds(&mut self, b: WorldAabb) {
        self.bounding_box.expand(b);
    }
}

pub fn build_chunk_grid(items: &[IsometricSortItem]) -> BTreeMap<(i32, i32), SpatialChunk> {
    let mut out = BTreeMap::new();
    for (i, item) in items.iter().enumerate() {
        let x = (item.position.x / SpatialChunk::DEFAULT_CHUNK_SIZE).floor() as i32;
        let z = (item.position.z / SpatialChunk::DEFAULT_CHUNK_SIZE).floor() as i32;
        let chunk = out.entry((x, z)).or_insert_with(|| SpatialChunk::new(x, z));
        chunk.object_indices.push(i);
        chunk.expand_bounds(WorldAabb::from_sort_item(item));
    }
    out
}

pub fn cull_with_chunks<'a>(
    items: &[IsometricSortItem],
    chunks: impl IntoIterator<Item = &'a SpatialChunk>,
    m: Mat4,
    v: Vec2,
    out: &mut [usize],
    padding: f32,
) -> Result<usize, &'static str> {
    if out.len() < items.len() {
        return Err("visible buffer shorter than items");
    }
    let mut n = 0;
    for chunk in chunks {
        if !project_aabb_to_screen(chunk.bounding_box, m, v).padded(v, padding) {
            continue;
        }
        for &i in &chunk.object_indices {
            let item = items.get(i).ok_or("stale chunk index")?;
            if is_visible(item, m, v, padding) {
                if n == out.len() {
                    return Err("duplicate chunk indices overflow output");
                }
                out[n] = i;
                n += 1;
            }
        }
    }
    Ok(n)
}
