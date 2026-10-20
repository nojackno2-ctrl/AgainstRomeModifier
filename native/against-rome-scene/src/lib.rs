//! Pure scene data and renderer-independent scene logic.
//! Matrices use System.Numerics row vectors (translation in row 4, vector * matrix).
//! No GPU, image decoder, native runtime, or platform dependencies.

pub mod atlas;
pub mod camera;
pub mod draw_list;
pub mod frame;
pub mod light_definitions;
pub mod lighting;
pub mod math;
pub mod minimap;
pub mod picking;
pub mod ray_picker;
pub mod rendering;
pub mod scene;
pub mod scene_object;
pub mod terrain;
pub mod thumbnail;

pub use atlas::{AtlasRect, AtlasUv, DynamicAtlasPacker, MaxRectsHeuristic};
pub use camera::{EditorCamera, TerrainRay};
pub use draw_list::{
    build_particle_batch, prepare_scene_frame, team_color, write_particle_quad, BrushCursorDraw,
    BrushCursorOptions, FrameDiagnostics, GridDrawCommand, OrderedParticleDraw, OrderedSpriteDraw,
    ParticleBlendMode, ParticleDrawBatch, ParticleInstance, ParticleRect, PlacementPreviewOptions,
    PreparedSceneFrame, SceneDrawList, SceneFrameBuilder, SceneFrameOptions, SceneMarkerDraw,
    SceneParticleRenderer, SelectionRect, ShadowDrawBatch, SpriteDrawBatch, TerrainDrawCommand,
    WaterDrawCommand, BRUSH_CURSOR_COLOR, SCRIPT_MARK_COLOR, WATER_DEFAULT_COLOR,
};
pub use lighting::{
    MapLightingBitmap, MapLightingDayNight, MapLightingModel, NativeLightCatalog,
    NativeLightDefinition, NativeLightInstance, NativeObjectLightingCatalog,
    NativeObjectLightingDefinition, PreparedSceneLighting, SavedSceneLightSet,
    SceneLightDiagnostics, SceneLightSourceItem, SceneLightSuppressionKey,
    SceneLightingContext, MAXIMUM_GAME_LIGHTS,
};
pub use math::{Mat4, Vec2, Vec3};
pub use minimap::{
    canvas_visible_rect, point_to_tile, tile_to_point, zoomed_image_bounds, Rect, RectF,
};
pub use picking::{pick_object, project_eye_to_screen, PickableSprite};
pub use ray_picker::TerrainRayPicker;
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
pub use scene_object::{SceneObject, SceneObjectKind};
pub use terrain::{
    build_terrain_mesh, TerrainHeightField, TerrainMeshData, TerrainVertex,
};
pub use thumbnail::{SpriteThumbnailCache, ThumbnailPlan, ThumbnailSource};

