//! Pure scene data and renderer-independent scene logic.
//! Matrices use System.Numerics row vectors (translation in row 4, vector * matrix).
//! No GPU, image decoder, native runtime, or platform dependencies.

pub mod atlas;
pub mod camera;
pub mod math;
pub mod minimap;
pub mod rendering;
pub mod scene;
pub mod terrain;
pub mod thumbnail;

pub use atlas::{AtlasRect, AtlasUv, DynamicAtlasPacker, MaxRectsHeuristic};
pub use camera::{EditorCamera, TerrainRay};
pub use math::{Mat4, Vec2, Vec3};
pub use minimap::{
    canvas_visible_rect, point_to_tile, tile_to_point, zoomed_image_bounds, Rect, RectF,
};
pub use rendering::{
    build_chunk_grid, compute_continuous_depth, compute_key, cull_items, cull_with_chunks,
    is_visible, project_aabb_to_screen, should_draw_before, sort_indices, sort_items,
    IsometricDepthKey, IsometricFootprint, IsometricRenderLayer, IsometricSortItem, ScreenAabb,
    SpatialChunk, WorldAabb,
};
pub use scene::{
    build_marker_points, build_shadow_vertices, build_sprite_vertices, combined_sprite_plan,
    ground_point, move_to_world_position, native_query_y, order_scene_draws, write_quad,
    ParticleSpriteDraw, SceneCombinedSpritePlan, SceneObjectPosition, SceneShadowGeometryCache,
    SceneSpriteDraw, SceneSpriteDrawKind, SceneSpriteGeometryBuffer, Shadow, Sprite,
    FLOATS_PER_SPRITE_VERTEX, MAP_WORLD_SIZE, QUAD_FLOATS, TILES_PER_SPRITE_PIXEL,
    UNKNOWN_NATIVE_QUERY_Y, WORLD_UNITS_PER_TILE,
};
pub use terrain::{
    build_terrain_mesh, TerrainHeightField, TerrainMeshData, TerrainVertex,
};
pub use thumbnail::{SpriteThumbnailCache, ThumbnailPlan, ThumbnailSource};
