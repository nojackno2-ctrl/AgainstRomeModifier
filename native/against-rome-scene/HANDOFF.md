# Scene port handoff

2026-10-15. Owned path: native/against-rome-scene/** only. No commits made.
Status: COMPLETE. Dependency-free crate, no GPU. Ported (src -> C#):
- math.rs: row-vector Vec2/Vec3/Mat4 (System.Numerics convention)
- terrain.rs: TerrainHeightField + TerrainMeshBuilder (vertex/index buffers)
- scene.rs: SceneObjectPositioning, SceneGeometryBuffers (sprite/shadow), SceneCombinedSpritePlan
- thumbnail.rs: SpriteThumbnailCache logic; camera.rs: EditorCamera; minimap.rs: MinimapNavigator
- atlas.rs: DynamicAtlasPacker; rendering.rs: IsometricDepthSorter + IsometricFrustumCuller
Tests (tests/*.rs): 57 passing, 0 failing, no warnings.
Run (from native/against-rome-scene, with CARGO_HOME/RUSTUP_HOME set and cargo bin on PATH):
  cargo test --offline --target-dir ../../re_workspace/target-against-rome-scene
Not done / non-goals: pixel-exact rendering, GPU upload, image decoding.
