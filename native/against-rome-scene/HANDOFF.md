# Scene port handoff

2026-10-18. Owned path: native/against-rome-scene/** only. No commits made.
Status: ADVANCED & COMPLETED for editor render preparation and camera integration. Dependency-free crate, no GPU.
Ported modules:
- math.rs: row-vector Vec2/Vec3/Mat4 (System.Numerics convention), Vec3 min/max/clamp
- terrain.rs: TerrainHeightField + TerrainMeshBuilder (vertex/index buffers)
- scene.rs: SceneObjectPositioning, SceneGeometryBuffers (sprite/shadow), SceneCombinedSpritePlan
- scene_object.rs: SceneObject (MapSceneObject equivalent with Kind, marker visibility, suppression key, safe bounds move)
- camera.rs: EditorCamera (zoom, pan, position, view/proj matrices, create_ray, focus_tile, zoom_step)
- ray_picker.rs: TerrainRayPicker (bounding-box slab clipping, 0.25-interval marching, 12-iteration bisection refinement)
- picking.rs: PickableSprite + pick_object (exact screen quad unproject/project, far-to-near depth test, alpha mask hit test, marker distance fallback)
- lighting.rs: MapLightingBitmap (pure 24-bit BI_RGB decoder), MapLightingDayNight (16.16 floor weight ambient table), MapLightingModel (0x49FFE0 falloff + 0x49A490 sprite gain), NativeLightCatalog, NativeObjectLightingCatalog, NativeLightInstance (saved vs synthetic), SceneLightSuppressionKey, SavedSceneLightSet, SceneLightingContext, PreparedSceneLighting (packs up to 64 nearest lights into GPU uniform arrays)
- thumbnail.rs: SpriteThumbnailCache logic; minimap.rs: MinimapNavigator
- atlas.rs: DynamicAtlasPacker; rendering.rs: IsometricDepthSorter + IsometricFrustumCuller
- draw_list.rs / frame.rs: CPU-testable scene draw list and frame preparation from SceneObject and camera/light APIs (PreparedSceneFrame / SceneDrawList, TerrainDrawCommand, WaterDrawCommand, GridDrawCommand, ShadowDrawBatch, SpriteDrawBatch, OrderedSpriteDraw, SceneMarkerDraw, BrushCursorDraw, SceneFrameOptions, SceneFrameBuilder, SelectionRect, team_color, evaluate_lighting_at, pick_object, pick_terrain, pick_tile, project_world_to_screen, and diagnostics)

Tests (tests/*.rs): 89 passing, 0 failing, 0 warnings (14 test suites).
Run:
  cargo test --release --locked --offline --manifest-path native/against-rome-scene/Cargo.toml --target-dir re_workspace/target-against-rome-scene

