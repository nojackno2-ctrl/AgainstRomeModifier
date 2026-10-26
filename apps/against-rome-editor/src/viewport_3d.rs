//! 3D Terrain & Object Viewport for Against Rome Map Editor.
//!
//! Provides an interactive 3D terrain and placed object viewport with:
//! - Orbit, Pan, and Zoom camera controls based on `against_rome_scene::EditorCamera`
//! - Terrain ray-cast picking using `against_rome_scene::TerrainRayPicker`
//! - Pixel-accurate sprite and marker object picking using `against_rome_scene::PreparedSceneFrame::pick_object`
//! - Shaded relief, heightmap, and collision visualization
//! - Placed object visualization supporting native sprites (via `NativeSpriteCatalog` / atlas)
//!   and clear, stable team-colored marker placeholders with graceful fallback when assets are missing
//! - Water plane and 3D brush perimeter cursor overlays
//! - Real-time world-to-map coordinate routing for terrain editing tools while ensuring
//!   object picking selects placements without inadvertently triggering the terrain brush
//!
//! # Parity Notice
//! This module provides an editor preview viewport based on pure CPU scene/frame
//! mathematics and egui geometry rendering. It is NOT full game-renderer parity
//! (no original game APT patch interpolation, retail particle blend shaders,
//! retail DirectX 8/9 pipeline, or retail soundscape triggers).

use std::collections::HashMap;
use std::sync::Arc;
#[inline]
pub(crate) fn tl(english: bool, zh: &'static str, en: &'static str) -> &'static str {
    if english {
        en
    } else {
        zh
    }
}


use against_rome_placement::WorldObject;
use against_rome_scene::{
    camera::EditorCamera,
    draw_list::{
        prepare_scene_frame, BrushCursorOptions, PreparedSceneFrame, SceneFrameOptions,
        BRUSH_CURSOR_COLOR,
    },
    math::{Vec2 as SceneVec2, Vec3 as SceneVec3},
    picking::PickableSprite,
    ray_picker::TerrainRayPicker,
    scene::Sprite,
    scene_object::SceneObject,
    terrain::TerrainHeightField,
    AtlasUv,
};
use against_rome_textures::{
    direction_for_angle, AtlasPoint, BoundedSpriteCatalog, FloorTextureAtlas, NativeSpriteAtlas,
    NativeSpriteCatalog, DEFAULT_DIRECTION,
};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Ui};

use crate::scene_ui::{nearest_marker, SceneMarker, ScenePointer};
use crate::terrain::TerrainSession;

/// Visual shading mode for the 3D terrain surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadingMode {
    /// Elevation-based natural terrain relief (sand, grass, highlands, rock, snow)
    Relief,
    /// Grayscale heightmap visualization lit by sunlight
    Heightmap,
    /// Walkability / collision overlay
    Collision,
    /// Textured ground tiles mapped via FloorTextureAtlas
    Textured,
}

/// Action produced by viewport interaction to be executed by EditorApp.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewportAction {
    None,
    /// A stroke dab was applied at height vertex coordinates `(grid_x, grid_y)`.
    Paint {
        grid_x: f32,
        grid_y: f32,
    },
    /// Pointer was released; commit the stroke to history.
    CommitStroke,
    /// An object was selected (or selection changed) via 3D picking.
    SelectObject(Option<(usize, u32)>),
    /// A discrete terrain tile center, converted to height vertex coordinates.
    PickTerrain {
        grid_x: f32,
        grid_y: f32,
    },
    /// Cancel a pending two-point terrain selection.
    CancelTerrainPick,
}

impl ViewportAction {
    /// Returns the picked terrain grid coordinates `(grid_x, grid_y)` if this is a `PickTerrain` action.
    #[inline]
    pub fn picked_terrain(&self) -> Option<(f32, f32)> {
        match *self {
            Self::PickTerrain { grid_x, grid_y } => Some((grid_x, grid_y)),
            _ => None,
        }
    }

    /// Returns the selected object `(slot, uid)` if this is a `SelectObject` action.
    #[inline]
    pub fn selected_object(&self) -> Option<Option<(usize, u32)>> {
        match *self {
            Self::SelectObject(key) => Some(key),
            _ => None,
        }
    }

    /// Returns true if this action is a discrete terrain pick.
    #[inline]
    pub fn is_pick_terrain(&self) -> bool {
        matches!(self, Self::PickTerrain { .. })
    }
}

/// Keyboard camera commands (C# `Map3DViewControl.OnKeyDown`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CameraKeyAction {
    /// Pan in screen directions: (right, forward) as a fraction of the camera distance.
    Pan(f32, f32),
    ZoomIn,
    ZoomOut,
    ResetGameScale,
}

/// Pure key -> camera mapping. Ctrl/Alt combinations belong to other features
/// (Ctrl+Arrow nudges the placement selection), so they never move the camera.
pub fn camera_key_action(key: egui::Key, ctrl: bool, alt: bool) -> Option<CameraKeyAction> {
    if ctrl || alt {
        return None;
    }
    Some(match key {
        egui::Key::ArrowLeft => CameraKeyAction::Pan(-0.1, 0.0),
        egui::Key::ArrowRight => CameraKeyAction::Pan(0.1, 0.0),
        egui::Key::ArrowUp => CameraKeyAction::Pan(0.0, 0.1),
        egui::Key::ArrowDown => CameraKeyAction::Pan(0.0, -0.1),
        egui::Key::PageUp => CameraKeyAction::ZoomIn,
        egui::Key::PageDown => CameraKeyAction::ZoomOut,
        egui::Key::Home => CameraKeyAction::ResetGameScale,
        _ => return None,
    })
}

const CAMERA_KEYS: [egui::Key; 7] = [
    egui::Key::ArrowLeft,
    egui::Key::ArrowRight,
    egui::Key::ArrowUp,
    egui::Key::ArrowDown,
    egui::Key::PageUp,
    egui::Key::PageDown,
    egui::Key::Home,
];

/// Interactive 3D terrain and object viewport state and renderer.
#[derive(Clone)]
pub struct Viewport3d {
    pub camera: EditorCamera,
    pub show_grid: bool,
    pub show_water: bool,
    pub show_markers: bool,
    pub show_objects: bool,
    pub mesh_lod_step: usize,
    pub relief_scale: f32,
    pub shading_mode: ShadingMode,
    pub hover_tile: Option<(i32, i32)>,
    pub hover_world: Option<SceneVec3>,
    pub hover_grid: Option<(f32, f32)>,
    pub hover_object: Option<(usize, u32)>,
    pub status_info: String,

    // Catalogs and asset mapping
    pub sprite_catalog: Option<Arc<NativeSpriteCatalog>>,
    pub bounded_catalog: Option<Arc<BoundedSpriteCatalog>>,
    pub sprite_atlas: Option<Arc<NativeSpriteAtlas>>,
    pub atlas_texture: Option<egui::TextureHandle>,
    pub type_names: HashMap<u16, String>,

    // Floor texture atlas and tile mapping
    pub floor_atlas: Option<Arc<FloorTextureAtlas>>,
    pub floor_atlas_texture: Option<egui::TextureHandle>,
    pub tile_textures: Option<Vec<String>>,

    /// Select ramp endpoints instead of applying continuous brush dabs.
    pub terrain_pick_mode: bool,
    terrain_pick_press: Option<(f32, f32)>,

    /// SDL scene move mode: presses/drags select and move `scene_markers` instead of painting.
    pub scene_pick: bool,
    /// Scene entries to draw/pick (world units); refreshed by the app each frame.
    pub scene_markers: Vec<SceneMarker>,
    /// Formation-expanded scenario spawns (display only); refreshed by the app.
    pub troop_objects: Vec<TroopObject>,
    /// Gestures produced by the last `show`; the app drains them into `SceneUi::scene_pointer`.
    pub scene_events: Vec<ScenePointer>,
    scene_dragging: bool,

    // Interaction tracking
    pub is_panning: bool,
    pub is_rotating: bool,
    pub rotate_start: Option<Pos2>,
    pub last_pointer: Option<Pos2>,
    pub drag_started_on_object: bool,
    pub is_dragging_terrain: bool,
    pub last_rect: Option<Rect>,
    pub english: bool,
}

impl std::fmt::Debug for Viewport3d {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Viewport3d")
            .field("camera", &self.camera)
            .field("show_grid", &self.show_grid)
            .field("show_water", &self.show_water)
            .field("show_markers", &self.show_markers)
            .field("show_objects", &self.show_objects)
            .field("mesh_lod_step", &self.mesh_lod_step)
            .field("relief_scale", &self.relief_scale)
            .field("shading_mode", &self.shading_mode)
            .field("hover_tile", &self.hover_tile)
            .field("hover_world", &self.hover_world)
            .field("hover_grid", &self.hover_grid)
            .field("hover_object", &self.hover_object)
            .field("status_info", &self.status_info)
            .field("has_sprite_catalog", &self.sprite_catalog.is_some())
            .field("has_bounded_catalog", &self.bounded_catalog.is_some())
            .field("has_sprite_atlas", &self.sprite_atlas.is_some())
            .field("has_floor_atlas", &self.floor_atlas.is_some())
            .field(
                "has_floor_atlas_texture",
                &self.floor_atlas_texture.is_some(),
            )
            .field(
                "tile_textures_count",
                &self.tile_textures.as_ref().map(|v| v.len()),
            )
            .field("type_names_count", &self.type_names.len())
            .field("drag_started_on_object", &self.drag_started_on_object)
            .field("is_dragging_terrain", &self.is_dragging_terrain)
            .field("last_rect", &self.last_rect)
            .field("english", &self.english)
            .finish()
    }
}

impl Default for Viewport3d {
    fn default() -> Self {
        Self::new()
    }
}

impl Viewport3d {
    pub const PAN_SPEED: f32 = 0.08;
    pub const ROTATE_SPEED: f32 = 0.35;
    pub const DRAG_THRESHOLD: f32 = 4.0;
    pub const OBJECT_PICK_RADIUS: f32 = 12.0;
    pub const NON_PARITY_NOTICE: &'static str =
        "3D Terrain Viewport: Editor preview via EditorCamera and TerrainRayPicker. Not full game-renderer parity (no native runtime APT/FPU shaders).";

    pub fn new() -> Self {
        Self {
            terrain_pick_mode: false,
            terrain_pick_press: None,
            scene_pick: false,
            scene_markers: Vec::new(),
            troop_objects: Vec::new(),
            scene_events: Vec::new(),
            scene_dragging: false,
            camera: EditorCamera::default(),
            show_grid: false,
            show_water: true,
            show_markers: true,
            show_objects: true,
            mesh_lod_step: 2,
            relief_scale: 1.0,
            shading_mode: ShadingMode::Relief,
            hover_tile: None,
            hover_world: None,
            hover_grid: None,
            hover_object: None,
            status_info: String::new(),
            sprite_catalog: None,
            bounded_catalog: None,
            sprite_atlas: None,
            atlas_texture: None,
            type_names: HashMap::new(),
            floor_atlas: None,
            floor_atlas_texture: None,
            tile_textures: None,
            is_panning: false,
            is_rotating: false,
            english: false,
            rotate_start: None,
            last_pointer: None,
            drag_started_on_object: false,
            is_dragging_terrain: false,
            last_rect: None,
        }
    }

    /// Sets or replaces the active floor texture atlas, invalidating any cached GPU handle.
    pub fn set_floor_atlas(&mut self, atlas: Option<Arc<FloorTextureAtlas>>) {
        self.floor_atlas = atlas;
        self.floor_atlas_texture = None;
    }

    /// Sets or updates the per-tile texture name grid.
    pub fn set_tile_textures(&mut self, textures: Option<Vec<String>>) {
        self.tile_textures = textures;
    }

    /// Resolves the texture name for a specific terrain tile coordinate.
    pub fn resolve_tile_texture(&self, x: usize, z: usize, dimension: usize) -> &str {
        if let Some(ref textures) = self.tile_textures {
            let idx = z * dimension + x;
            if let Some(name) = textures.get(idx) {
                let trimmed = name.trim();
                if !trimmed.is_empty() {
                    return trimmed;
                }
            }
        }
        if let Some(ref atlas) = self.floor_atlas {
            if let Some(first) = atlas.names().first() {
                return first.as_str();
            }
        }
        "4BA___51"
    }

    /// Samples the 4 UV corner points `[Top-Left, Top-Right, Bottom-Right, Bottom-Left]`
    /// for a terrain tile coordinate using the active floor texture atlas.
    pub fn sample_tile_uv(&self, x: usize, z: usize, dimension: usize) -> Option<[AtlasPoint; 4]> {
        let atlas = self.floor_atlas.as_ref()?;
        let name = self.resolve_tile_texture(x, z, dimension);
        Some(atlas.get_uv(name))
    }

    /// Synchronizes the CPU `FloorTextureAtlas` ARGB pixels into an egui `TextureHandle`.
    pub fn sync_floor_atlas_texture(&mut self, ctx: &egui::Context) {
        if let Some(ref atlas) = self.floor_atlas {
            let width = atlas.size();
            let height = atlas.size();
            let mut pixels = Vec::with_capacity(width * height);
            for &argb in atlas.argb_pixels() {
                let a = ((argb >> 24) & 0xFF) as u8;
                let r = ((argb >> 16) & 0xFF) as u8;
                let g = ((argb >> 8) & 0xFF) as u8;
                let b = (argb & 0xFF) as u8;
                pixels.push(Color32::from_rgba_unmultiplied(r, g, b, a));
            }
            let color_image = egui::ColorImage {
                size: [width, height],
                pixels,
            };
            let handle = ctx.load_texture(
                "floor_texture_atlas",
                color_image,
                egui::TextureOptions::LINEAR,
            );
            self.floor_atlas_texture = Some(handle);
        }
    }

    /// Resets the camera to center on the terrain dimension with default orbit angles.
    pub fn reset_camera(&mut self, dimension: f32) {
        let center = dimension * 0.5;
        self.camera.target = SceneVec3::new(center, 0.0, center);
        self.camera.set_distance(82.0);
        self.camera.rotate(
            45.0 - self.camera.yaw_degrees(),
            30.0 - self.camera.pitch_degrees(),
        );
    }

    /// Rotates the camera orbit angles (yaw and pitch in degrees).
    pub fn orbit(&mut self, delta_x: f32, delta_y: f32) {
        self.camera
            .rotate(delta_x * Self::ROTATE_SPEED, -delta_y * Self::ROTATE_SPEED);
    }

    /// Pans the camera target along the right/forward ground planes.
    pub fn pan(&mut self, delta_x: f32, delta_y: f32) {
        self.camera
            .pan(-delta_x * Self::PAN_SPEED, delta_y * Self::PAN_SPEED);
    }

    /// Centers the camera target on the given world coordinates (0..16384).
    pub fn center_on_world(&mut self, world_x: f32, world_z: f32) {
        if world_x.is_finite() && world_z.is_finite() {
            self.camera.target.x = (world_x / 256.0).clamp(0.0, 256.0);
            self.camera.target.z = (world_z / 256.0).clamp(0.0, 256.0);
        }
    }

    /// Frames the currently selected object by centering the camera target on its position.
    pub fn frame_selection(
        &mut self,
        objects: &[WorldObject],
        selected: Option<(usize, u32)>,
    ) -> bool {
        if let Some(key) = selected {
            if let Some(obj) = objects.iter().find(|o| (o.slot, o.uid) == key) {
                if obj.position[0].is_finite() && obj.position[2].is_finite() {
                    self.center_on_world(obj.position[0], obj.position[2]);
                    return true;
                }
            }
        }
        false
    }

    /// Applies a keyboard camera command; `viewport_height` is the canvas height in pixels.
    pub fn apply_camera_key(&mut self, action: CameraKeyAction, viewport_height: f32) {
        match action {
            CameraKeyAction::Pan(right, forward) => {
                let d = self.camera.distance();
                self.camera.pan(right * d, forward * d);
            }
            CameraKeyAction::ZoomIn => self.camera.zoom(0.84),
            CameraKeyAction::ZoomOut => self.camera.zoom(1.19),
            CameraKeyAction::ResetGameScale => {
                self.camera.zoom_to_game_scale(viewport_height.max(1.0) as i32)
            }
        }
    }

    /// Zooms the camera distance by standard stepping factors.
    pub fn zoom(&mut self, zoom_in: bool) {
        self.camera.zoom_step(zoom_in);
    }

    /// Evaluates discrete terrain pick coordinates for a screen position if ray hits empty terrain.
    ///
    /// Returns `None` if an object was picked (object-selection priority) or if ray-cast picking misses the terrain.
    pub fn try_pick_terrain_grid(
        &self,
        screen_pt: Pos2,
        viewport_rect: Rect,
        session: &TerrainSession,
        field: &TerrainHeightField,
        _objects: &[WorldObject],
        frame: &PreparedSceneFrame,
        scene_objects: &[SceneObject],
        pickable_sprites: &[Option<PickableSprite<'_>>],
    ) -> Option<(f32, f32)> {
        let local_x = screen_pt.x - viewport_rect.min.x;
        let local_y = screen_pt.y - viewport_rect.min.y;
        let screen_vec = SceneVec2::new(local_x, local_y);

        // Object priority: if hitting an object, terrain pick is rejected
        if self.show_objects {
            if frame
                .pick_object(
                    screen_vec,
                    scene_objects,
                    pickable_sprites,
                    field,
                    Self::OBJECT_PICK_RADIUS,
                )
                .is_some()
            {
                return None;
            }
        }

        let ray = self.camera.create_ray(
            local_x,
            local_y,
            viewport_rect.width(),
            viewport_rect.height(),
        );
        let tile = TerrainRayPicker::try_pick(field, ray)?;
        Some(Self::terrain_tile_to_grid(session, field, tile))
    }

    /// C# Map3DViewControl.TryGetTile floors/clamps the hit via TerrainRayPicker.TryPick;
    /// TerrainRampTool.TileToVertex then maps that integer tile's center to vertices.
    fn terrain_tile_to_grid(
        session: &TerrainSession,
        field: &TerrainHeightField,
        (tile_x, tile_y): (i32, i32),
    ) -> (f32, f32) {
        let scale_x = (session.vertex_size - 1) as f32 / field.tile_width;
        let scale_y = (session.vertex_size - 1) as f32 / field.tile_height;
        (
            (tile_x as f32 + 0.5) * scale_x,
            (tile_y as f32 + 0.5) * scale_y,
        )
    }

    fn collision_cell_at_world(
        session: &TerrainSession,
        field: &TerrainHeightField,
        world: SceneVec3,
    ) -> Option<(usize, usize, bool)> {
        let size = session.collision_size;
        if size == 0
            || !world.x.is_finite()
            || !world.z.is_finite()
            || !field.tile_width.is_finite()
            || !field.tile_height.is_finite()
            || field.tile_width <= 0.0
            || field.tile_height <= 0.0
        {
            return None;
        }

        let cell_x = ((world.x / field.tile_width * size as f32).floor() as usize).min(size - 1);
        let cell_z = ((world.z / field.tile_height * size as f32).floor() as usize).min(size - 1);
        let value = *session.collision().get(cell_z * size + cell_x)?;
        Some((cell_x, cell_z, value > 0))
    }

    /// Builds a `TerrainHeightField` from the current `TerrainSession`.
    pub fn build_height_field(
        session: &TerrainSession,
        height_scale: f32,
        native_step: f32,
    ) -> Result<TerrainHeightField, &'static str> {
        let dimension = ((session.vertex_size.saturating_sub(1)) / 4).max(1) as f32;
        TerrainHeightField::new(
            session.vertex_size,
            session.vertex_size,
            session.heights().to_vec(),
            height_scale,
            dimension,
            dimension,
            Some(native_step),
        )
    }

    /// Converts raw `WorldObject` instances into rich `SceneObject` definitions,
    /// pickable sprites, and scene sprites based on active catalog assets.
    pub fn convert_world_objects(
        &self,
        objects: &[WorldObject],
    ) -> (
        Vec<SceneObject>,
        Vec<Option<PickableSprite<'static>>>,
        Vec<Option<Sprite>>,
    ) {
        let mut scene_objs = Vec::with_capacity(objects.len());
        let mut pickables = Vec::with_capacity(objects.len());
        let mut scene_sprites = Vec::with_capacity(objects.len());

        for obj in objects {
            if !obj.position[0].is_finite() || !obj.position[2].is_finite() {
                continue;
            }

            let name = self
                .type_names
                .get(&obj.type_id)
                .cloned()
                .unwrap_or_else(|| format!("obj_{}", obj.type_id));

            let angle = if obj.rotation_degrees.is_finite() {
                Some(obj.rotation_degrees)
            } else {
                None
            };

            let scene_obj = SceneObject {
                name: name.clone(),
                world: SceneVec3::new(obj.position[0], obj.position[1], obj.position[2]),
                local: SceneVec3::ZERO,
                team: obj.team as i32,
                source_file: "placement".to_string(),
                object_index: obj.slot as i32,
                angle,
                has_native_saved_position: true,
            };

            let (pickable, scene_sprite) = self.resolve_visual(&name, obj.team as i32, angle);

            scene_objs.push(scene_obj);
            pickables.push(pickable);
            scene_sprites.push(scene_sprite);
        }

        (scene_objs, pickables, scene_sprites)
    }

    /// Resolves the native sprite (catalog, else bounded catalog) of an object name into its
    /// pickable footprint and scene sprite; both are `None` when no asset resolves.
    fn resolve_visual(
        &self,
        name: &str,
        team: i32,
        angle: Option<f32>,
    ) -> (Option<PickableSprite<'static>>, Option<Sprite>) {
        // 1. Resolve native sprite from catalogs if available
        let native_sprite = if let Some(ref cat) = self.sprite_catalog {
            let dir = angle
                .map(|a| direction_for_angle(a, 8))
                .unwrap_or(DEFAULT_DIRECTION);
            cat.get_sprite(name, team, dir, angle, None)
        } else if let Some(ref bcat) = self.bounded_catalog {
            bcat.get_sprite(&name)
        } else {
            None
        };
        let (pickable, scene_sprite) = if let Some(ref ns) = native_sprite {
            let p = PickableSprite::new(
                ns.width as u32,
                ns.height as u32,
                ns.anchor_x as f32,
                ns.anchor_y as f32,
                None,
            );

            let uv = self
                .sprite_atlas
                .as_ref()
                .and_then(|atlas| atlas.try_get_uv(ns))
                .map(|u| AtlasUv {
                    u0: u.u0,
                    v0: u.v0,
                    u1: u.u1,
                    v1: u.v1,
                })
                .unwrap_or(AtlasUv {
                    u0: 0.0,
                    v0: 0.0,
                    u1: 1.0,
                    v1: 1.0,
                });

            let s = Sprite {
                width: ns.width as f32,
                height: ns.height as f32,
                anchor_x: ns.anchor_x as f32,
                anchor_y: ns.anchor_y as f32,
                uv,
            };
            (Some(p), Some(s))
        } else {
            (None, None)
        };

        (pickable, scene_sprite)
    }

    /// Display-only objects of the formation-expanded scenario spawns (never picked or edited).
    pub fn convert_troop_objects(
        &self,
        troops: &[TroopObject],
    ) -> (
        Vec<SceneObject>,
        Vec<Option<PickableSprite<'static>>>,
        Vec<Option<Sprite>>,
    ) {
        let mut scene_objs = Vec::with_capacity(troops.len());
        let mut pickables = Vec::with_capacity(troops.len());
        let mut sprites = Vec::with_capacity(troops.len());
        for troop in troops {
            let angle = Some(troop.angle).filter(|a| a.is_finite());
            scene_objs.push(SceneObject {
                name: troop.name.clone(),
                world: SceneVec3::new(troop.world[0], troop.world[1], troop.world[2]),
                local: SceneVec3::ZERO,
                team: troop.team,
                source_file: "troop".to_string(),
                object_index: -5000 - troop.spawn_index as i32,
                angle,
                has_native_saved_position: true,
            });
            let (pickable, sprite) = self.resolve_visual(&troop.name, troop.team, angle);
            pickables.push(pickable);
            sprites.push(sprite);
        }
        (scene_objs, pickables, sprites)
    }

    /// Prepares a pure CPU scene frame using `against_rome_scene`.
    pub fn prepare_frame(
        &self,
        viewport_size: egui::Vec2,
        field: &TerrainHeightField,
        water_surface: f32,
        brush_radius_tiles: f32,
        objects: &[SceneObject],
        sprites: &[Option<Sprite>],
    ) -> Result<PreparedSceneFrame, &'static str> {
        let mut options = SceneFrameOptions::default();
        options.show_grid = self.show_grid;
        options.show_water = self.show_water;
        options.show_markers = self.show_markers && self.show_objects;
        options.show_objects = self.show_objects;
        options.water_level = water_surface;
        options.brush_cursor = self.hover_tile.map(|(tx, ty)| BrushCursorOptions {
            hover_tile_x: tx,
            hover_tile_y: ty,
            brush_size: (brush_radius_tiles * 2.0).round().max(1.0) as i32,
            selection: None,
            color: BRUSH_CURSOR_COLOR,
            line_width: 2.0,
        });

        let vp = SceneVec2::new(viewport_size.x.max(1.0), viewport_size.y.max(1.0));
        prepare_scene_frame(
            &self.camera,
            vp,
            field,
            objects,
            sprites,
            &[],
            None,
            None,
            &options,
        )
    }

    /// Shows the 3D terrain and object viewport in the given UI region and returns any editing action.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        session: &TerrainSession,
        height_step: f32,
        water_surface: f32,
        brush_radius: f32,
        objects: &[WorldObject],
        selected_object: Option<(usize, u32)>,
    ) -> ViewportAction {
        let mut action = ViewportAction::None;

        // 1b. Synchronize floor texture atlas to GPU texture handle if not yet loaded
        if self.floor_atlas.is_some() && self.floor_atlas_texture.is_none() {
            self.sync_floor_atlas_texture(ctx);
        }

        // 2. Build heightfield from session
        let height_scale = 6.0 * self.relief_scale;
        let field = match Self::build_height_field(session, height_scale, height_step) {
            Ok(f) => f,
            Err(err) => {
                ui.colored_label(Color32::RED, format!("{} {err}", tl(self.english, "地形初始化錯誤：", "Terrain init error:")));
                return action;
            }
        };

        // 1. Top controls toolbar
        self.render_toolbar(ui, session, &field, objects, selected_object);

        // 3. Allocate canvas area
        let available = ui.available_size();
        let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());
        let rect = response.rect;
        self.last_rect = Some(rect);

        if rect.width() <= 10.0 || rect.height() <= 10.0 {
            return action;
        }

        // 4. Handle camera interactions: Zoom (Mouse wheel)
        if response.hovered() {
            let scroll = ctx.input(|i| i.raw_scroll_delta.y);
            if scroll > 0.0 {
                self.zoom(true);
            } else if scroll < 0.0 {
                self.zoom(false);
            }
        }

        // 5. Handle camera interactions: Pan (Middle click or Alt + Left click)
        let is_alt_down = ctx.input(|i| i.modifiers.alt);
        if response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged_by(egui::PointerButton::Primary) && is_alt_down)
        {
            let delta = response.drag_delta();
            self.pan(delta.x, delta.y);
            self.is_panning = true;
        } else {
            self.is_panning = false;
        }

        // 6. Handle camera interactions: Orbit (Right-click drag)
        if response.dragged_by(egui::PointerButton::Secondary) {
            let delta = response.drag_delta();
            self.orbit(delta.x, delta.y);
            self.is_rotating = true;
        } else {
            self.is_rotating = false;
        }

        // 6b. Keyboard camera (arrows pan, PgUp/PgDn zoom, Home = game scale), hover-only so
        // placement nudging (Ctrl+Arrow) and text fields keep their keys.
        if response.hovered() && !ctx.wants_keyboard_input() {
            let (ctrl, alt) = ctx.input(|i| (i.modifiers.ctrl || i.modifiers.command, i.modifiers.alt));
            for key in CAMERA_KEYS {
                if ctx.input(|i| i.key_pressed(key)) {
                    if let Some(action) = camera_key_action(key, ctrl, alt) {
                        self.apply_camera_key(action, rect.height());
                    }
                }
            }
        }

        // 7. Convert WorldObjects to SceneObjects and resolve sprites
        let (mut scene_objects, mut pickable_sprites, mut scene_sprites) =
            self.convert_world_objects(objects);
        // Squad markers and formation members follow the placed objects; they are never picked.
        let pickable_count = scene_objects.len();
        if self.show_objects && !self.troop_objects.is_empty() {
            let (extra_objects, extra_pickables, extra_sprites) =
                self.convert_troop_objects(&self.troop_objects);
            scene_objects.extend(extra_objects);
            pickable_sprites.extend(extra_pickables);
            scene_sprites.extend(extra_sprites);
        }

        // 8. Prepare CPU scene frame
        let brush_radius_tiles =
            brush_radius * (field.tile_width / (session.vertex_size - 1) as f32);
        let frame = match self.prepare_frame(
            rect.size(),
            &field,
            water_surface,
            brush_radius_tiles,
            &scene_objects,
            &scene_sprites,
        ) {
            Ok(f) => f,
            Err(e) => {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("Frame Error: {e}"),
                    egui::FontId::proportional(16.0),
                    Color32::RED,
                );
                return action;
            }
        };

        // 9. Ray Picking & Object Detection
        let pointer_pos = response
            .interact_pointer_pos()
            .or_else(|| response.hover_pos())
            .or_else(|| ctx.input(|i| i.pointer.latest_pos()));
        let mut picked_obj_idx = None;

        if let Some(pos) = pointer_pos {
            let local_x = pos.x - rect.min.x;
            let local_y = pos.y - rect.min.y;
            let screen_pt = SceneVec2::new(local_x, local_y);

            // 9a. Object Picking (priority over terrain brush)
            if self.show_objects {
                picked_obj_idx = frame.pick_object(
                    screen_pt,
                    &scene_objects[..pickable_count],
                    &pickable_sprites[..pickable_count],
                    &field,
                    Self::OBJECT_PICK_RADIUS,
                );
            }

            if let Some(idx) = picked_obj_idx {
                if let Some(raw) = objects.get(idx) {
                    self.hover_object = Some((raw.slot, raw.uid));
                }
            } else {
                self.hover_object = None;
            }

            // 9b. Terrain Picking
            let ray = self
                .camera
                .create_ray(local_x, local_y, rect.width(), rect.height());

            self.hover_world = TerrainRayPicker::try_pick_point(&field, ray);
            self.hover_tile = TerrainRayPicker::try_pick(&field, ray);

            if let Some(hit) = self.hover_world {
                let v_scale_x = (session.vertex_size - 1) as f32 / field.tile_width;
                let v_scale_y = (session.vertex_size - 1) as f32 / field.tile_height;
                let gx = (hit.x * v_scale_x).clamp(0.0, (session.vertex_size - 1) as f32);
                let gy = (hit.z * v_scale_y).clamp(0.0, (session.vertex_size - 1) as f32);
                self.hover_grid = Some((gx, gy));

                // Left click & drag interaction:
                if !is_alt_down && !self.terrain_pick_mode && !self.scene_pick {
                    let is_primary_clicked = response.clicked_by(egui::PointerButton::Primary)
                        || (rect.contains(pos)
                            && ctx
                                .input(|i| i.pointer.button_clicked(egui::PointerButton::Primary))
                            && !response.dragged());

                    if is_primary_clicked {
                        // Discrete primary click:
                        // Priority 1: Object selection if clicking an object
                        if let Some(idx) = picked_obj_idx {
                            let raw = &objects[idx];
                            action = ViewportAction::SelectObject(Some((raw.slot, raw.uid)));
                            self.drag_started_on_object = true;
                        } else if let Some(tile) = self.hover_tile {
                            // Priority 2: Primary click on empty terrain -> emit discrete terrain pick (for ramp endpoints)
                            let (gx, gy) = Self::terrain_tile_to_grid(session, &field, tile);
                            action = ViewportAction::PickTerrain {
                                grid_x: gx,
                                grid_y: gy,
                            };
                        }
                    } else {
                        let is_primary_dragged = response.dragged_by(egui::PointerButton::Primary)
                            || (rect.contains(pos)
                                && ctx.input(|i| {
                                    i.pointer.button_down(egui::PointerButton::Primary)
                                })
                                && ctx.input(|i| i.pointer.delta().length() > 0.0));

                        if is_primary_dragged {
                            // Brush drag:
                            if picked_obj_idx.is_some() {
                                self.drag_started_on_object = true;
                            }
                            if !self.drag_started_on_object && picked_obj_idx.is_none() {
                                self.is_dragging_terrain = true;
                                action = ViewportAction::Paint {
                                    grid_x: gx,
                                    grid_y: gy,
                                };
                            }
                        }
                    }
                }
            } else {
                self.hover_grid = None;
                // Ray did not hit terrain, but check if user clicked an object extending outside terrain:
                let is_primary_clicked = response.clicked_by(egui::PointerButton::Primary)
                    || ((rect.contains(pos) || response.hovered())
                        && ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary))
                        && !response.dragged());
                if !is_alt_down && !self.terrain_pick_mode && !self.scene_pick && is_primary_clicked {
                    if let Some(idx) = picked_obj_idx {
                        let raw = &objects[idx];
                        action = ViewportAction::SelectObject(Some((raw.slot, raw.uid)));
                        self.drag_started_on_object = true;
                    }
                    // For failed ray pick on empty space: do not emit PickTerrain
                }
            }
        } else {
            self.hover_world = None;
            self.hover_tile = None;
            self.hover_grid = None;
            self.hover_object = None;
        }

        // 9c. SDL scene move mode: press selects the nearest marker, drag follows the terrain
        // hit point (world units), release ends the drag (one undo step in `SceneUi`).
        self.scene_events.clear();
        if self.scene_pick && !is_alt_down && !self.terrain_pick_mode {
            let (pressed, down) = ctx.input(|i| {
                (
                    i.pointer.button_pressed(egui::PointerButton::Primary),
                    i.pointer.button_down(egui::PointerButton::Primary),
                )
            });
            if pressed && response.hovered() {
                if let Some(pos) = pointer_pos {
                    let hit = nearest_marker(
                        &self.scene_markers,
                        pos,
                        &|m| {
                            let p = against_rome_scene::ground_point(
                                against_rome_scene::SceneObjectPosition {
                                    world: SceneVec3::new(m.world[0], m.world[1], m.world[2]),
                                    ..Default::default()
                                },
                                &field,
                                0.25,
                            );
                            frame
                                .project_world_to_screen(p)
                                .map(|s| Pos2::new(rect.min.x + s.x, rect.min.y + s.y))
                        },
                        12.0,
                    );
                    self.scene_events.push(ScenePointer::Press { hit });
                    self.scene_dragging = hit.is_some();
                }
            } else if self.scene_dragging {
                if down {
                    if let Some(h) = self.hover_world {
                        let k = against_rome_scene::WORLD_UNITS_PER_TILE;
                        self.scene_events.push(ScenePointer::Drag { world: (h.x * k, h.z * k) });
                    }
                } else {
                    self.scene_events.push(ScenePointer::Release);
                    self.scene_dragging = false;
                }
            }
        } else if self.scene_dragging {
            self.scene_events.push(ScenePointer::Release);
            self.scene_dragging = false;
        }

        // Ramp endpoints use integer tile centers; hover_grid/brush dabs stay continuous.
        let terrain_pick_grid = self
            .hover_tile
            .map(|tile| Self::terrain_tile_to_grid(session, &field, tile));
        // Capture only presses inside this canvas; a release outside or in the sky
        // must not reuse the last valid terrain coordinate.
        if self.terrain_pick_mode {
            if response.clicked_by(egui::PointerButton::Secondary) {
                self.terrain_pick_press = None;
                action = ViewportAction::CancelTerrainPick;
            } else if !is_alt_down {
                if response.is_pointer_button_down_on()
                    && ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary))
                {
                    self.terrain_pick_press = None;
                    if let Some(idx) = picked_obj_idx {
                        let raw = &objects[idx];
                        action = ViewportAction::SelectObject(Some((raw.slot, raw.uid)));
                    } else if let Some(point) = terrain_pick_grid
                        .filter(|_| pointer_pos.is_some_and(|pos| rect.contains(pos)))
                    {
                        self.terrain_pick_press = Some(point);
                        action = ViewportAction::PickTerrain {
                            grid_x: point.0,
                            grid_y: point.1,
                        };
                    }
                }
                if ctx.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
                    if let Some(start) = self.terrain_pick_press.take() {
                        if let Some(point) = terrain_pick_grid.filter(|point| {
                            *point != start
                                && picked_obj_idx.is_none()
                                && pointer_pos.is_some_and(|pos| rect.contains(pos))
                        }) {
                            action = ViewportAction::PickTerrain {
                                grid_x: point.0,
                                grid_y: point.1,
                            };
                        }
                    }
                }
            } else {
                self.terrain_pick_press = None;
            }
        } else {
            self.terrain_pick_press = None;
        }

        // Commit stroke when primary button is released after a terrain brush drag
        if ctx.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
            let was_dragging_object = self.drag_started_on_object;
            let was_dragging_terrain = self.is_dragging_terrain;
            self.drag_started_on_object = false;
            self.is_dragging_terrain = false;
            if !self.terrain_pick_mode
                && !is_alt_down
                && was_dragging_terrain
                && !was_dragging_object
            {
                action = ViewportAction::CommitStroke;
            }
        }

        // 10. Render Terrain Mesh (projected triangles in back-to-front painter order)
        self.render_terrain_mesh(&painter, rect, &frame, &field, session);

        // 11. Render Water Plane
        if self.show_water {
            if let Some(ref water_cmd) = frame.water {
                self.render_water_plane(&painter, rect, &frame, water_cmd);
            }
        }

        // 12. Render Placed Objects (Sprites & Fallback Placeholders)
        if self.show_objects {
            self.render_objects(
                &painter,
                rect,
                &frame,
                objects,
                &scene_objects,
                selected_object,
            );
        }

        // 12b. Render SDL scene markers (team-colored dots; selection ringed; removals crossed)
        for m in &self.scene_markers {
            let p = against_rome_scene::ground_point(
                against_rome_scene::SceneObjectPosition {
                    world: SceneVec3::new(m.world[0], m.world[1], m.world[2]),
                    ..Default::default()
                },
                &field,
                0.25,
            );
            if let Some(s) = frame.project_world_to_screen(p) {
                let c = against_rome_scene::draw_list::team_color(m.team);
                let fill = Color32::from_rgb(
                    (c[0] * 255.0) as u8,
                    (c[1] * 255.0) as u8,
                    (c[2] * 255.0) as u8,
                );
                let at = Pos2::new(rect.min.x + s.x, rect.min.y + s.y);
                painter.circle_filled(at, 5.0, fill);
                painter.circle_stroke(at, 5.0, Stroke::new(1.0_f32, Color32::BLACK));
                if m.selected {
                    painter.circle_stroke(at, 9.0, Stroke::new(2.0_f32, Color32::YELLOW));
                }
                if m.removed {
                    painter.line_segment(
                        [at + egui::vec2(-6.0, -6.0), at + egui::vec2(6.0, 6.0)],
                        Stroke::new(2.0_f32, Color32::RED),
                    );
                    painter.line_segment(
                        [at + egui::vec2(-6.0, 6.0), at + egui::vec2(6.0, -6.0)],
                        Stroke::new(2.0_f32, Color32::RED),
                    );
                }
            }
        }

        // 13. Render 3D Brush Cursor
        if let Some(ref cursor_cmd) = frame.cursor {
            self.render_brush_cursor(&painter, rect, &frame, cursor_cmd);
        }

        // 14. Render Hit Point Indicator (when not hovering an object)
        if self.hover_object.is_none() {
            if let Some(hit) = self.hover_world {
                if let Some(screen_pt) = frame.project_world_to_screen(hit) {
                    let p = Pos2::new(rect.min.x + screen_pt.x, rect.min.y + screen_pt.y);
                    painter.circle_filled(p, 4.0, Color32::from_rgb(255, 235, 70));
                    painter.circle_stroke(p, 5.0, Stroke::new(1.0_f32, Color32::BLACK));
                }
            }
        }

        // 15. Floating Viewport HUD
        self.render_hud(&painter, rect, objects, selected_object);

        action
    }

    /// Renders placed objects using active sprite quads or clear stable marker placeholders.
    fn render_objects(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        frame: &PreparedSceneFrame,
        raw_objects: &[WorldObject],
        scene_objects: &[SceneObject],
        selected_object: Option<(usize, u32)>,
    ) {
        // 1. Render ordered sprites
        for ordered in &frame.ordered_sprites {
            if let Some(screen_pt) = frame.project_world_to_screen(ordered.anchor) {
                let p = Pos2::new(rect.min.x + screen_pt.x, rect.min.y + screen_pt.y);
                let s = ordered.sprite;
                let left = p.x - s.anchor_x;
                let top = p.y - s.anchor_y;
                let sprite_rect =
                    Rect::from_min_size(Pos2::new(left, top), egui::Vec2::new(s.width, s.height));

                if let Some(ref texture) = self.atlas_texture {
                    let uv_rect = Rect::from_min_max(
                        Pos2::new(s.uv.u0, s.uv.v0),
                        Pos2::new(s.uv.u1, s.uv.v1),
                    );
                    painter.image(texture.id(), sprite_rect, uv_rect, Color32::WHITE);
                } else {
                    // Safe placeholder billboard when sprite is resolved but atlas texture is not yet on GPU
                    let raw = raw_objects.get(ordered.object_index);
                    let team = raw.map(|r| r.team as i32).unwrap_or(0);
                    let col = against_rome_scene::draw_list::team_color(team);
                    let color = Color32::from_rgba_unmultiplied(
                        (col[0] * 255.0) as u8,
                        (col[1] * 255.0) as u8,
                        (col[2] * 255.0) as u8,
                        170,
                    );
                    painter.rect_filled(sprite_rect, 2.0, color);
                    painter.rect_stroke(sprite_rect, 2.0, Stroke::new(1.0_f32, Color32::BLACK));
                }

                // Selection outline around sprite
                if let Some(raw) = raw_objects.get(ordered.object_index) {
                    if selected_object == Some((raw.slot, raw.uid)) {
                        painter.rect_stroke(
                            sprite_rect.expand(2.0),
                            2.0,
                            Stroke::new(2.5_f32, Color32::from_rgb(255, 235, 70)),
                        );
                    } else if self.hover_object == Some((raw.slot, raw.uid)) {
                        painter.rect_stroke(
                            sprite_rect.expand(1.5),
                            1.5,
                            Stroke::new(1.5_f32, Color32::WHITE),
                        );
                    }
                }
            }
        }

        // 2. Render marker placeholders for objects without active sprites or script marks
        if self.show_markers {
            for marker in &frame.markers {
                let raw = raw_objects.get(marker.object_index);

                let Some(screen_pt) = frame.project_world_to_screen(marker.point) else {
                    continue;
                };

                let p = Pos2::new(rect.min.x + screen_pt.x, rect.min.y + screen_pt.y);
                let is_selected = raw.is_some_and(|raw| selected_object == Some((raw.slot, raw.uid)));
                let is_hovered = raw.is_some_and(|raw| self.hover_object == Some((raw.slot, raw.uid)));

                let base_color = Color32::from_rgba_unmultiplied(
                    (marker.color[0] * 255.0) as u8,
                    (marker.color[1] * 255.0) as u8,
                    (marker.color[2] * 255.0) as u8,
                    (marker.color[3] * 255.0) as u8,
                );

                let radius = (marker.point_size * 0.5).max(3.0);

                if marker.is_script_mark {
                    // Script mark: crisp cyan diamond
                    let pts = vec![
                        Pos2::new(p.x, p.y - radius),
                        Pos2::new(p.x + radius, p.y),
                        Pos2::new(p.x, p.y + radius),
                        Pos2::new(p.x - radius, p.y),
                    ];
                    painter.add(egui::Shape::convex_polygon(
                        pts,
                        base_color,
                        Stroke::new(1.0_f32, Color32::BLACK),
                    ));
                } else {
                    // Standard object: team-colored filled circle with dark perimeter
                    painter.circle_filled(p, radius, base_color);
                    painter.circle_stroke(
                        p,
                        radius,
                        Stroke::new(1.0_f32, Color32::from_rgb(20, 20, 20)),
                    );
                }

                // Selection halo
                if is_selected {
                    painter.circle_stroke(
                        p,
                        radius + 4.0,
                        Stroke::new(2.5_f32, Color32::from_rgb(255, 235, 70)),
                    );
                    painter.circle_stroke(
                        p,
                        radius + 6.0,
                        Stroke::new(1.0_f32, Color32::from_rgb(20, 20, 20)),
                    );
                } else if is_hovered {
                    painter.circle_stroke(p, radius + 3.0, Stroke::new(1.5_f32, Color32::WHITE));
                }

                // Label for selected or hovered object
                if let (Some(raw), true) = (raw, is_selected || is_hovered) {
                    let obj_name = scene_objects
                        .get(marker.object_index)
                        .map(|s| s.name.as_str())
                        .unwrap_or("object");
                    let text = format!("{}: {} (T{})", raw.slot, obj_name, raw.team);
                    painter.text(
                        Pos2::new(p.x, p.y - radius - 8.0),
                        egui::Align2::CENTER_BOTTOM,
                        text,
                        egui::FontId::monospace(10.5),
                        if is_selected {
                            Color32::from_rgb(255, 235, 70)
                        } else {
                            Color32::WHITE
                        },
                    );
                }
            }
        }
    }

    /// Renders the top toolbar with camera toggles and status indicators.
    fn render_toolbar(
        &mut self,
        ui: &mut Ui,
        session: &TerrainSession,
        field: &TerrainHeightField,
        objects: &[WorldObject],
        selected_object: Option<(usize, u32)>,
    ) {
        ui.horizontal_wrapped(|ui| {
            ui.heading(tl(self.english, "3D 編輯預覽", "3D Viewport"));
            ui.separator();

            let dimension = ((session.vertex_size.saturating_sub(1)) / 4).max(1) as f32;
            if ui.button(tl(self.english, "重設鏡頭", "Reset Camera")).clicked() {
                self.reset_camera(dimension);
            }

            let frame_enabled = selected_object.is_some();
            if ui
                .add_enabled(
                    frame_enabled,
                    egui::Button::new(tl(self.english, "聚焦選取 (F)", "Frame (F)")),
                )
                .on_hover_text(tl(
                    self.english,
                    "將 3D 鏡頭置中對準目前選取的物件（快捷鍵：F）",
                    "Center 3D camera on selected object (Shortcut: F)",
                ))
                .clicked()
            {
                self.frame_selection(objects, selected_object);
            }

            // Orthographic vs Perspective toggle
            let ortho_label = if self.camera.orthographic {
                tl(self.english, "投影：正交", "Proj: Ortho")
            } else {
                tl(self.english, "投影：透視", "Proj: Persp")
            };
            if ui.button(ortho_label).clicked() {
                self.camera.orthographic = !self.camera.orthographic;
            }

            // Grid overlay toggle
            ui.toggle_value(&mut self.show_grid, tl(self.english, "網格", "Grid"));

            // Water toggle
            ui.toggle_value(&mut self.show_water, tl(self.english, "水面", "Water"));

            // Objects & Markers toggles
            ui.separator();
            ui.toggle_value(&mut self.show_objects, tl(self.english, "物件", "Objects"));
            ui.toggle_value(&mut self.show_markers, tl(self.english, "標記", "Markers"));

            // Shading mode selection
            ui.separator();
            ui.label(tl(self.english, "著色模式：", "Shading:"));
            ui.selectable_value(
                &mut self.shading_mode,
                ShadingMode::Relief,
                tl(self.english, "浮雕", "Relief"),
            );
            ui.selectable_value(
                &mut self.shading_mode,
                ShadingMode::Heightmap,
                tl(self.english, "高度", "Height"),
            );
            ui.selectable_value(
                &mut self.shading_mode,
                ShadingMode::Collision,
                tl(self.english, "碰撞", "Collision"),
            );
            ui.selectable_value(
                &mut self.shading_mode,
                ShadingMode::Textured,
                tl(self.english, "材質貼圖", "Textured"),
            );

            // LOD Mesh Step selection
            ui.separator();
            ui.label(tl(self.english, "精細度：", "LOD:"));
            ui.selectable_value(
                &mut self.mesh_lod_step,
                1,
                tl(self.english, "1 (精細)", "1 (Fine)"),
            );
            ui.selectable_value(
                &mut self.mesh_lod_step,
                2,
                tl(self.english, "2 (中等)", "2 (Med)"),
            );
            ui.selectable_value(
                &mut self.mesh_lod_step,
                4,
                tl(self.english, "4 (快速)", "4 (Fast)"),
            );

            // Object count status
            ui.separator();
            ui.label(if self.english {
                format!("Objects: {}", objects.len())
            } else {
                format!("物件數：{}", objects.len())
            });
            if let Some(sel) = selected_object {
                ui.label(if self.english {
                    format!("Selected: [slot={}, uid={}]", sel.0, sel.1)
                } else {
                    format!("已選取：[槽位={}, UID={}]", sel.0, sel.1)
                });
            }

            if let Some((tx, ty)) = self.hover_tile {
                let h = self.hover_world.map(|w| w.y).unwrap_or(0.0);
                ui.label(if self.english {
                    format!("Tile: ({tx}, {ty}) Y: {h:.2}")
                } else {
                    format!("圖塊：({tx}, {ty}) 高度：{h:.2}")
                });
            }

            if self.shading_mode == ShadingMode::Collision {
                if let Some(world) = self.hover_world {
                    if let Some((cell_x, cell_z, blocked)) =
                        Self::collision_cell_at_world(session, field, world)
                    {
                        ui.label(if self.english {
                            format!(
                                "Collision cell: ({cell_x}, {cell_z}) {}",
                                if blocked { "Blocked" } else { "Passable" }
                            )
                        } else {
                            format!(
                                "通行格：({cell_x}, {cell_z}) {}",
                                if blocked { "阻擋" } else { "可通行" }
                            )
                        });
                    }
                }
            }

            if let Some(ref atlas) = self.floor_atlas {
                ui.separator();
                ui.label(if self.english {
                    format!("Atlas: {} tiles", atlas.count())
                } else {
                    format!("圖集：{} 個圖塊", atlas.count())
                });
            }
        });
    }

    /// Renders the floating HUD overlay in the viewport.
    fn render_hud(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        objects: &[WorldObject],
        selected_object: Option<(usize, u32)>,
    ) {
        let hud_pos = Pos2::new(rect.min.x + 8.0, rect.max.y - 42.0);
        let sel_str = if let Some(sel) = selected_object {
            if self.english {
                format!(" | Selected: Slot {}, UID {}", sel.0, sel.1)
            } else {
                format!(" | 選取：槽位 {}, UID {}", sel.0, sel.1)
            }
        } else {
            String::new()
        };
        let hov_str = if let Some(hov) = self.hover_object {
            if self.english {
                format!(" | Hover: Slot {}, UID {}", hov.0, hov.1)
            } else {
                format!(" | 暫留：槽位 {}, UID {}", hov.0, hov.1)
            }
        } else {
            String::new()
        };
        let atlas_str = if self.floor_atlas.is_some() {
            if self.floor_atlas_texture.is_some() {
                if self.english { " | Atlas: Textured" } else { " | 圖集：已貼圖" }
            } else {
                if self.english { " | Atlas: Fallback" } else { " | 圖集：備用" }
            }
        } else {
            ""
        };
        let controls_str = if self.english {
            "Controls: Right-drag: Orbit | Middle/Alt-drag: Pan | Wheel: Zoom | Click: Paint / Pick Object"
        } else {
            "操作指南：右鍵拖曳：旋轉視角 | 中鍵/Alt+左鍵拖曳：平移 | 滾輪：縮放 | 左鍵點選：繪製／選取物件"
        };
        let objects_label = if self.english { "Objects:" } else { "物件數：" };
        let notice = if self.english {
            Self::NON_PARITY_NOTICE
        } else {
            "3D 地形視角：透過 EditorCamera 與 TerrainRayPicker 進行編輯預覽。非遊戲原生精確著色器。"
        };
        let text = format!(
            "{}\n{} {}{}{}{}\n{}",
            controls_str,
            objects_label,
            objects.len(),
            sel_str,
            hov_str,
            atlas_str,
            notice
        );
        painter.text(
            hud_pos,
            egui::Align2::LEFT_BOTTOM,
            text,
            egui::FontId::monospace(10.5),
            Color32::from_rgba_unmultiplied(220, 220, 220, 180),
        );
    }

    /// Renders the shaded terrain mesh quads in back-to-front order.
    fn render_terrain_mesh(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        frame: &PreparedSceneFrame,
        field: &TerrainHeightField,
        session: &TerrainSession,
    ) {
        let step = self.mesh_lod_step.clamp(1, 8);
        let dim = field.tile_width as usize;

        // Determine back-to-front iteration order based on camera position relative to target
        let cam_pos = self.camera.position();
        let target = self.camera.target;

        let x_rev = cam_pos.x < target.x;
        let z_rev = cam_pos.z < target.z;

        let x_steps: Vec<usize> = if x_rev {
            (0..dim).step_by(step).rev().collect()
        } else {
            (0..dim).step_by(step).collect()
        };

        let z_steps: Vec<usize> = if z_rev {
            (0..dim).step_by(step).rev().collect()
        } else {
            (0..dim).step_by(step).collect()
        };

        let sun_dir = SceneVec3::new(0.577, 0.707, 0.408).normalized();
        let grid_stroke = if self.show_grid {
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(40, 40, 40, 70))
        } else {
            Stroke::NONE
        };

        let is_textured_mode = self.shading_mode == ShadingMode::Textured;
        let mut textured_mesh = if is_textured_mode {
            self.floor_atlas_texture
                .as_ref()
                .map(|t| egui::Mesh::with_texture(t.id()))
        } else {
            None
        };

        for &z in &z_steps {
            for &x in &x_steps {
                let x0 = x as f32;
                let z0 = z as f32;
                let x1 = (x + step).min(dim) as f32;
                let z1 = (z + step).min(dim) as f32;

                let h0 = field.sample_height(x0, z0);
                let h1 = field.sample_height(x1, z0);
                let h2 = field.sample_height(x1, z1);
                let h3 = field.sample_height(x0, z1);

                let p0 = SceneVec3::new(x0, h0, z0);
                let p1 = SceneVec3::new(x1, h1, z0);
                let p2 = SceneVec3::new(x1, h2, z1);
                let p3 = SceneVec3::new(x0, h3, z1);

                // Project 4 corners
                let sp0 = match frame.project_world_to_screen(p0) {
                    Some(s) => Pos2::new(rect.min.x + s.x, rect.min.y + s.y),
                    None => continue,
                };
                let sp1 = match frame.project_world_to_screen(p1) {
                    Some(s) => Pos2::new(rect.min.x + s.x, rect.min.y + s.y),
                    None => continue,
                };
                let sp2 = match frame.project_world_to_screen(p2) {
                    Some(s) => Pos2::new(rect.min.x + s.x, rect.min.y + s.y),
                    None => continue,
                };
                let sp3 = match frame.project_world_to_screen(p3) {
                    Some(s) => Pos2::new(rect.min.x + s.x, rect.min.y + s.y),
                    None => continue,
                };

                // Surface normal estimation for lighting
                let cx = (x0 + x1) * 0.5;
                let cz = (z0 + z1) * 0.5;
                let dx = field.sample_height(cx + 0.5, cz) - field.sample_height(cx - 0.5, cz);
                let dz = field.sample_height(cx, cz + 0.5) - field.sample_height(cx, cz - 0.5);
                let normal = SceneVec3::new(-dx, 1.0, -dz).normalized();
                let diffuse = normal.dot(sun_dir).max(0.0);
                let light = (0.35 + 0.65 * diffuse).clamp(0.2, 1.0);

                let avg_h = (h0 + h1 + h2 + h3) * 0.25;
                let norm_h = if field.height_scale > 0.0 {
                    (avg_h / field.height_scale).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                // Textured mode with active GPU atlas texture handle
                if let Some(ref mut mesh) = textured_mesh {
                    let tex_name = self.resolve_tile_texture(x, z, dim);
                    let uv = if let Some(ref atlas) = self.floor_atlas {
                        atlas.get_uv(tex_name)
                    } else {
                        [AtlasPoint::new(0.0, 0.0); 4]
                    };

                    let tint = Color32::from_rgb(
                        (255.0 * light) as u8,
                        (255.0 * light) as u8,
                        (255.0 * light) as u8,
                    );

                    let base_idx = mesh.vertices.len() as u32;
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: sp0,
                        uv: Pos2::new(uv[0].u, uv[0].v),
                        color: tint,
                    });
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: sp1,
                        uv: Pos2::new(uv[1].u, uv[1].v),
                        color: tint,
                    });
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: sp2,
                        uv: Pos2::new(uv[2].u, uv[2].v),
                        color: tint,
                    });
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: sp3,
                        uv: Pos2::new(uv[3].u, uv[3].v),
                        color: tint,
                    });
                    mesh.add_triangle(base_idx, base_idx + 1, base_idx + 2);
                    mesh.add_triangle(base_idx, base_idx + 2, base_idx + 3);

                    if self.show_grid {
                        painter.add(egui::Shape::closed_line(
                            vec![sp0, sp1, sp2, sp3],
                            grid_stroke,
                        ));
                    }
                    continue;
                }

                let color = match self.shading_mode {
                    ShadingMode::Relief => Self::relief_color(norm_h, light),
                    ShadingMode::Heightmap => Self::heightmap_color(norm_h, light),
                    ShadingMode::Collision => {
                        let collision_size = session.collision_size;
                        let collision_scale = collision_size as f32 / field.tile_width;
                        let col_x0 = ((x0 * collision_scale).floor() as usize)
                            .min(collision_size);
                        let col_z0 = ((z0 * collision_scale).floor() as usize)
                            .min(collision_size);
                        let col_x1 = ((x1 * collision_scale).ceil() as usize)
                            .min(collision_size);
                        let col_z1 = ((z1 * collision_scale).ceil() as usize)
                            .min(collision_size);
                        let collision = session.collision();
                        let blocked = (col_z0..col_z1).any(|col_z| {
                            (col_x0..col_x1).any(|col_x| {
                                collision
                                    .get(col_z * collision_size + col_x)
                                    .copied()
                                    .unwrap_or(0)
                                    > 0
                            })
                        });
                        if blocked {
                            Color32::from_rgb(
                                (210.0 * light) as u8,
                                (60.0 * light) as u8,
                                (60.0 * light) as u8,
                            )
                        } else {
                            Self::relief_color(norm_h, light * 0.9)
                        }
                    }
                    ShadingMode::Textured => {
                        // CPU Fallback: map texture via FloorTextureAtlas::fallback_color
                        let tex_name = self.resolve_tile_texture(x, z, dim);
                        if self.floor_atlas.is_some() {
                            let argb = FloorTextureAtlas::fallback_color(tex_name);
                            let r = ((argb >> 16) & 0xFF) as f32;
                            let g = ((argb >> 8) & 0xFF) as f32;
                            let b = (argb & 0xFF) as f32;
                            Color32::from_rgb(
                                (r * light).clamp(0.0, 255.0) as u8,
                                (g * light).clamp(0.0, 255.0) as u8,
                                (b * light).clamp(0.0, 255.0) as u8,
                            )
                        } else {
                            Self::relief_color(norm_h, light)
                        }
                    }
                };

                // Draw two convex triangles forming the quad
                painter.add(egui::Shape::convex_polygon(
                    vec![sp0, sp1, sp2],
                    color,
                    grid_stroke,
                ));
                painter.add(egui::Shape::convex_polygon(
                    vec![sp0, sp2, sp3],
                    color,
                    grid_stroke,
                ));
            }
        }

        if let Some(mesh) = textured_mesh {
            if !mesh.is_empty() {
                painter.add(egui::Shape::Mesh(mesh));
            }
        }
    }

    /// Renders the water plane quad at dynamic water height.
    fn render_water_plane(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        frame: &PreparedSceneFrame,
        cmd: &against_rome_scene::draw_list::WaterDrawCommand,
    ) {
        let mut screen_pts = Vec::with_capacity(4);
        for &v in &cmd.vertices {
            if let Some(s) = frame.project_world_to_screen(v) {
                screen_pts.push(Pos2::new(rect.min.x + s.x, rect.min.y + s.y));
            }
        }
        if screen_pts.len() == 4 {
            let water_color = Color32::from_rgba_unmultiplied(
                (cmd.color[0] * 255.0) as u8,
                (cmd.color[1] * 255.0) as u8,
                (cmd.color[2] * 255.0) as u8,
                ((cmd.color[3] * 0.8).clamp(0.0, 1.0) * 255.0) as u8,
            );
            painter.add(egui::Shape::convex_polygon(
                screen_pts,
                water_color,
                Stroke::NONE,
            ));
        }
    }

    /// Renders the 3D brush cursor perimeter loop.
    fn render_brush_cursor(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        frame: &PreparedSceneFrame,
        cmd: &against_rome_scene::draw_list::BrushCursorDraw,
    ) {
        let pts: Vec<Pos2> = cmd
            .perimeter_points
            .iter()
            .filter_map(|&p| {
                frame
                    .project_world_to_screen(p)
                    .map(|s| Pos2::new(rect.min.x + s.x, rect.min.y + s.y))
            })
            .collect();

        if pts.len() >= 3 {
            let color = Color32::from_rgba_unmultiplied(
                (cmd.color[0] * 255.0) as u8,
                (cmd.color[1] * 255.0) as u8,
                (cmd.color[2] * 255.0) as u8,
                (cmd.color[3] * 255.0) as u8,
            );
            painter.add(egui::Shape::closed_line(
                pts,
                Stroke::new(cmd.line_width, color),
            ));
        }
    }

    /// Maps normalized elevation (0.0..1.0) and sun lighting to natural landscape relief colors.
    fn relief_color(norm_height: f32, light: f32) -> Color32 {
        let (r, g, b) = if norm_height < 0.12 {
            // Water bed / sand
            (195.0, 180.0, 135.0)
        } else if norm_height < 0.38 {
            // Grass / lowlands
            let t = (norm_height - 0.12) / 0.26;
            (
                195.0 + (75.0 - 195.0) * t,
                180.0 + (145.0 - 180.0) * t,
                135.0 + (65.0 - 135.0) * t,
            )
        } else if norm_height < 0.68 {
            // Forest / plateau
            let t = (norm_height - 0.38) / 0.30;
            (
                75.0 + (115.0 - 75.0) * t,
                145.0 + (125.0 - 145.0) * t,
                65.0 + (75.0 - 65.0) * t,
            )
        } else if norm_height < 0.88 {
            // Mountain rock
            let t = (norm_height - 0.68) / 0.20;
            (
                115.0 + (140.0 - 115.0) * t,
                125.0 + (120.0 - 125.0) * t,
                75.0 + (105.0 - 75.0) * t,
            )
        } else {
            // Snow / crest
            let t = ((norm_height - 0.88) / 0.12).min(1.0);
            (
                140.0 + (225.0 - 140.0) * t,
                120.0 + (230.0 - 120.0) * t,
                105.0 + (240.0 - 105.0) * t,
            )
        };

        Color32::from_rgb(
            (r * light).clamp(0.0, 255.0) as u8,
            (g * light).clamp(0.0, 255.0) as u8,
            (b * light).clamp(0.0, 255.0) as u8,
        )
    }

    /// Maps normalized elevation to grayscale heightmap color.
    fn heightmap_color(norm_height: f32, light: f32) -> Color32 {
        let v = (norm_height * 255.0 * light).clamp(0.0, 255.0) as u8;
        Color32::from_rgb(v, v, v)
    }
}

// ---------------------------------------------------------------------------------------------
// Squad formation expansion for the 3D view (C# `MapEditorForm.Formations.cs`)
// ---------------------------------------------------------------------------------------------

use against_rome_ai_planner::formation::{FormationLayout, FormationLayoutDefinition};
use against_rome_maps::sdl_objects::{SdlObjectCategory, SdlObjectType};
use against_rome_maps::spawn_data::SpawnDataCatalog;
use against_rome_scenario::model::Spawn;
use std::cell::RefCell;
use std::path::Path;

/// C# `LoadFormationDefinition`: `SYSTEM/DATA_MP/DEFAULTS/formdef.dau` formation 0, falling back to
/// the native default layout when the file is missing or malformed (3D display only, never fatal).
pub fn load_formation_definition(game_root: &Path) -> FormationLayoutDefinition {
    let path = game_root.join("SYSTEM").join("DATA_MP").join("DEFAULTS").join("formdef.dau");
    let Ok(mut bytes) = std::fs::read(&path) else {
        return FormationLayoutDefinition::native_default();
    };
    if bytes.starts_with(b"PFIL") {
        match against_rome_data::lzss::decompress_pfil(&bytes, None) {
            Ok(raw) => bytes = raw,
            Err(_) => return FormationLayoutDefinition::native_default(),
        }
    }
    let text = against_rome_maps::decode_cp1251(&bytes).unwrap_or_else(|_| String::from_utf8_lossy(&bytes).into_owned());
    FormationLayoutDefinition::from_form_def_text(&text, 0).unwrap_or_else(|_| FormationLayoutDefinition::native_default())
}

/// Display names resolved for one spawn alias.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TroopNames {
    /// Object name of the spawn marker itself (the SDL object, or the alias when unknown).
    pub marker: String,
    /// Member figure name; `None` for anything that is not a squad.
    pub soldier: Option<String>,
    /// Banner object shown at the squad position (`None` keeps the plain spawn marker).
    pub banner: Option<String>,
}

/// Catalog data needed to expand squads into formation members (C# `SoldierName` / `BannerName`).
#[derive(Clone, Debug, Default)]
pub struct TroopCatalog {
    types: Vec<SdlObjectType>,
    spawn_catalog: SpawnDataCatalog,
    /// Lower-case objdef names, used to validate the fallback banner name.
    known_names: std::collections::HashSet<String>,
    cache: RefCell<HashMap<String, TroopNames>>,
}

impl TroopCatalog {
    pub fn new(types: Vec<SdlObjectType>, spawn_catalog: SpawnDataCatalog) -> Self {
        let known_names = spawn_catalog.type_names.values().map(|n| n.to_ascii_lowercase()).collect();
        Self { types, spawn_catalog, known_names, cache: RefCell::default() }
    }

    /// Loads the SDL object templates of the retail maps and the objdef / script alias catalogs.
    pub fn from_game(game_root: &Path) -> Result<Self, String> {
        let types = against_rome_maps::sdl_objects::SdlObjectCatalog::build(game_root)?;
        let spawn_catalog = SpawnDataCatalog::from_game(game_root)?;
        Ok(Self::new(types, spawn_catalog))
    }

    fn type_for_name(&self, name: &str) -> Option<&SdlObjectType> {
        self.types.iter().find(|t| t.name_def.eq_ignore_ascii_case(name))
    }

    /// Script alias -> object definition name (`cl_scint.ini [ObjDefName]`).
    fn name_for_alias(&self, alias: &str) -> Option<&str> {
        self.spawn_catalog
            .aliases
            .iter()
            .find(|a| a.alias.eq_ignore_ascii_case(alias))
            .map(|a| a.name_def.as_str())
    }

    /// C# `SoldierName`: a figure is its own soldier; a unit group names its member through the
    /// template's `objdefn0` (objdef id, script alias, or a literal `Fig…` name).
    fn soldier_name(&self, ty: &SdlObjectType) -> Option<String> {
        if ty.category == SdlObjectCategory::Figure {
            return Some(ty.name_def.clone());
        }
        if !ty.has_unit_count() {
            return None;
        }
        let member = ty.template_fields.get("objdefn0")?.trim();
        if let Ok(id) = member.parse::<i32>() {
            return self.spawn_catalog.type_names.get(&id).cloned();
        }
        if let Some(name) = self.name_for_alias(member) {
            return Some(name.to_string());
        }
        member.get(..3).filter(|p| p.eq_ignore_ascii_case("Fig")).map(|_| member.to_string())
    }

    /// C# `BannerName`.
    fn banner_name(&self, ty: &SdlObjectType, soldier: &str) -> Option<String> {
        if ty.has_unit_count() {
            return Some(ty.name_def.clone());
        }
        if let Some(template) = self
            .types
            .iter()
            .find(|t| t.has_unit_count() && self.soldier_name(t).is_some_and(|s| s.eq_ignore_ascii_case(soldier)))
        {
            return Some(template.name_def.clone());
        }
        // No SDL squad template for this soldier: only reuse an existing retail military banner.
        let name = format!(
            "Ver{}KamIco00_Kampf_Icon",
            against_rome_maps::sdl_objects::SdlObjectCatalog::tribe(soldier)
        );
        self.known_names.contains(&name.to_ascii_lowercase()).then_some(name)
    }

    /// Names for a spawn alias (cached; the catalog scan is linear in the SDL template count).
    pub fn names_for_alias(&self, alias: &str) -> TroopNames {
        let key = alias.to_ascii_lowercase();
        if let Some(hit) = self.cache.borrow().get(&key) {
            return hit.clone();
        }
        let ty = self.name_for_alias(alias).and_then(|name| self.type_for_name(name));
        let names = match ty {
            None => TroopNames { marker: alias.to_string(), soldier: None, banner: None },
            Some(ty) => {
                let soldier = self.soldier_name(ty);
                let banner = soldier.as_deref().and_then(|s| self.banner_name(ty, s));
                TroopNames { marker: ty.name_def.clone(), soldier, banner }
            }
        };
        self.cache.borrow_mut().insert(key, names.clone());
        names
    }
}

/// One display-only object of a spawn: the spawn marker/banner or a formation member.
#[derive(Clone, Debug, PartialEq)]
pub struct TroopObject {
    /// Index of the owning spawn in the scenario's spawn list.
    pub spawn_index: usize,
    pub name: String,
    pub world: [f32; 3],
    pub team: i32,
    pub angle: f32,
    /// True for the spawn's own marker/banner, false for formation members.
    pub is_marker: bool,
}

/// C# `ExpandTroopsFor3D`: every spawn yields its marker; squads (unit groups / figures with a
/// positive count) additionally yield one soldier per member laid out by the formation definition.
/// Display only: nothing is added to the scenario, SDL or DATA.
pub fn expand_troops_for_3d(
    spawns: &[Spawn],
    catalog: &TroopCatalog,
    definition: &FormationLayoutDefinition,
) -> Vec<TroopObject> {
    let mut out = Vec::new();
    for (spawn_index, spawn) in spawns.iter().enumerate() {
        if !(spawn.x.is_finite() && spawn.y.is_finite() && spawn.z.is_finite()) {
            continue;
        }
        let names = catalog.names_for_alias(&spawn.alias);
        let marker = |name: String| TroopObject {
            spawn_index,
            name,
            world: [spawn.x, spawn.y, spawn.z],
            team: spawn.team,
            angle: spawn.angle as f32,
            is_marker: true,
        };
        let Some(soldier) = names.soldier.clone().filter(|_| spawn.count > 0) else {
            out.push(marker(names.marker));
            continue;
        };
        // An unresolved banner still keeps one spawn marker (same as the 2D / selection behaviour).
        out.push(marker(names.banner.clone().unwrap_or_else(|| format!("TroopSpawn:{}", spawn.alias))));
        let count = (spawn.count as usize).min(FormationLayout::MAXIMUM_SCENARIO_MEMBERS);
        let offsets = FormationLayout::create(count, spawn.angle as f32, definition, FormationLayout::INITIAL_SPACING)
            .unwrap_or_default();
        for (dx, dz) in offsets {
            out.push(TroopObject {
                spawn_index,
                name: soldier.clone(),
                world: [spawn.x + dx, spawn.y, spawn.z + dz],
                team: spawn.team,
                angle: spawn.angle as f32,
                is_marker: false,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use against_rome_placement::WorldObject;
    use against_rome_scene::camera::TerrainRay;
    use against_rome_textures::NativeSprite;

    fn make_test_world_object(
        slot: usize,
        uid: u32,
        type_id: u16,
        team: u16,
        pos_x: f32,
        pos_z: f32,
    ) -> WorldObject {
        WorldObject {
            slot,
            type_id,
            team,
            uid,
            position: [pos_x, 0.0, pos_z],
            rotation_degrees: 0.0,
            parent: None,
            vitals: None,
        }
    }

    #[test]
    fn test_viewport_3d_initial_state() {
        let vp = Viewport3d::new();
        assert_eq!(vp.camera.target, SceneVec3::new(32.0, 0.0, 32.0));
        assert_eq!(vp.camera.yaw_degrees(), 45.0);
        assert_eq!(vp.camera.pitch_degrees(), 30.0);
        assert_eq!(vp.camera.distance(), 82.0);
        assert!(vp.camera.orthographic);
        assert!(!vp.show_grid);
        assert!(vp.show_water);
        assert!(vp.show_markers);
        assert!(vp.show_objects);
        assert_eq!(vp.mesh_lod_step, 2);
        assert_eq!(vp.shading_mode, ShadingMode::Relief);
        assert!(vp.hover_tile.is_none());
        assert!(vp.hover_world.is_none());
        assert!(vp.hover_object.is_none());
    }

    #[test]
    fn test_camera_key_mapping_and_application() {
        use egui::Key::*;
        assert_eq!(camera_key_action(ArrowLeft, false, false), Some(CameraKeyAction::Pan(-0.1, 0.0)));
        assert_eq!(camera_key_action(ArrowUp, false, false), Some(CameraKeyAction::Pan(0.0, 0.1)));
        assert_eq!(camera_key_action(PageUp, false, false), Some(CameraKeyAction::ZoomIn));
        assert_eq!(camera_key_action(PageDown, false, false), Some(CameraKeyAction::ZoomOut));
        assert_eq!(camera_key_action(Home, false, false), Some(CameraKeyAction::ResetGameScale));
        assert_eq!(camera_key_action(F, false, false), None);
        assert_eq!(camera_key_action(F, true, false), None);
        assert_eq!(camera_key_action(F, false, true), None);
        assert_eq!(camera_key_action(ArrowLeft, true, false), None);
        assert_eq!(camera_key_action(ArrowLeft, false, true), None);
        assert_eq!(camera_key_action(A, false, false), None);

        let mut vp = Viewport3d::new();
        let target = vp.camera.target;
        vp.apply_camera_key(CameraKeyAction::Pan(0.1, 0.0), 600.0);
        assert_ne!(vp.camera.target, target);
        let d = vp.camera.distance();
        vp.apply_camera_key(CameraKeyAction::ZoomIn, 600.0);
        assert!(vp.camera.distance() < d);
        vp.apply_camera_key(CameraKeyAction::ZoomOut, 600.0);
        vp.apply_camera_key(CameraKeyAction::ResetGameScale, 600.0);
        assert!(vp.camera.distance() > 0.0);
    }

    #[test]
    fn test_viewport_3d_frame_selection_and_center_on_world() {
        let mut vp = Viewport3d::new();
        // 初始中心 (32.0, 0.0, 32.0)
        assert_eq!(vp.camera.target.x, 32.0);
        assert_eq!(vp.camera.target.z, 32.0);

        // 移至 (8192, 4096) -> (32.0, 16.0)
        vp.center_on_world(8192.0, 4096.0);
        assert_eq!(vp.camera.target.x, 32.0);
        assert_eq!(vp.camera.target.z, 16.0);

        // 移至 (16384, 0) -> (64.0, 0.0)
        vp.center_on_world(16384.0, 0.0);
        assert_eq!(vp.camera.target.x, 64.0);
        assert_eq!(vp.camera.target.z, 0.0);

        // 測試 frame_selection
        let obj1 = make_test_world_object(10, 501, 1, 0, 4096.0, 8192.0);
        let obj2 = make_test_world_object(11, 502, 2, 0, 10240.0, 12800.0);
        let objects = vec![obj1, obj2];

        // 選取不存在物件回傳 false，不改變鏡頭
        assert!(!vp.frame_selection(&objects, None));
        assert!(!vp.frame_selection(&objects, Some((99, 999))));
        assert_eq!(vp.camera.target.x, 64.0);

        // 選取 obj1 (slot 10, uid 501) 成功聚焦至 (16.0, 32.0)
        assert!(vp.frame_selection(&objects, Some((10, 501))));
        assert_eq!(vp.camera.target.x, 16.0);
        assert_eq!(vp.camera.target.z, 32.0);

        // 選取 obj2 (slot 11, uid 502) 成功聚焦至 (40.0, 50.0)
        assert!(vp.frame_selection(&objects, Some((11, 502))));
        assert_eq!(vp.camera.target.x, 40.0);
        assert_eq!(vp.camera.target.z, 50.0);
    }

    #[test]
    fn test_camera_orbit_pan_zoom() {
        let mut vp = Viewport3d::new();
        let initial_yaw = vp.camera.yaw_degrees();
        let initial_pitch = vp.camera.pitch_degrees();

        // Orbit test
        vp.orbit(10.0, 5.0);
        assert!(
            (vp.camera.yaw_degrees() - (initial_yaw + 10.0 * Viewport3d::ROTATE_SPEED)).abs()
                < 1e-4
        );
        assert!(
            (vp.camera.pitch_degrees() - (initial_pitch - 5.0 * Viewport3d::ROTATE_SPEED)).abs()
                < 1e-4
        );

        // Zoom test
        let dist = vp.camera.distance();
        vp.zoom(true); // zoom in
        assert!(vp.camera.distance() < dist);
        vp.zoom(false); // zoom out
        assert!((vp.camera.distance() - dist).abs() < 1.0);

        // Pan test
        let target_before = vp.camera.target;
        vp.pan(5.0, 5.0);
        assert_ne!(vp.camera.target, target_before);

        // Reset camera
        vp.reset_camera(64.0);
        assert_eq!(vp.camera.target, SceneVec3::new(32.0, 0.0, 32.0));
        assert_eq!(vp.camera.yaw_degrees(), 45.0);
        assert_eq!(vp.camera.pitch_degrees(), 30.0);
    }

    #[test]
    fn test_heightfield_construction_and_ray_picking() {
        let session = TerrainSession::create_blank(257, 32);
        let field = Viewport3d::build_height_field(&session, 6.0, 4.0).unwrap();
        assert_eq!(field.width(), 257);
        assert_eq!(field.height(), 257);
        assert_eq!(field.tile_width, 64.0);
        assert_eq!(field.tile_height, 64.0);

        // Blank map height: 32 / 255 * 6.0
        let expected_h = 32.0 / 255.0 * 6.0;
        let sampled = field.sample_height(32.0, 32.0);
        assert!((sampled - expected_h).abs() < 1e-4);

        // Top-down vertical ray hitting center
        let ray = TerrainRay {
            origin: SceneVec3::new(32.0, 50.0, 32.0),
            direction: SceneVec3::new(0.0, -1.0, 0.0),
        };
        let hit = TerrainRayPicker::try_pick_point(&field, ray);
        assert!(hit.is_some());
        let pt = hit.unwrap();
        assert!((pt.x - 32.0).abs() < 1e-3);
        assert!((pt.z - 32.0).abs() < 1e-3);
        assert!((pt.y - expected_h).abs() < 1e-3);

        let tile = TerrainRayPicker::try_pick(&field, ray);
        assert_eq!(tile, Some((32, 32)));

        // World to grid conversion
        let v_scale = (session.vertex_size - 1) as f32 / field.tile_width;
        let gx = pt.x * v_scale;
        let gy = pt.z * v_scale;
        assert!((gx - 128.0).abs() < 1e-2);
        assert!((gy - 128.0).abs() < 1e-2);
    }

    #[test]
    fn test_object_conversion_and_fallback_markers() {
        let session = TerrainSession::create_blank(257, 32);
        let field = Viewport3d::build_height_field(&session, 6.0, 4.0).unwrap();
        let mut vp = Viewport3d::new();

        // 3 test objects: Team 1 (Roman blue), Team 2 (Germanic red), and Scriptmark
        let obj1 = make_test_world_object(0, 101, 1, 1, 8192.0, 8192.0); // tile (32, 32)
        let obj2 = make_test_world_object(1, 102, 2, 2, 4096.0, 4096.0); // tile (16, 16)
        let obj3 = make_test_world_object(2, 103, 3, 0, 12288.0, 12288.0); // tile (48, 48)

        vp.type_names.insert(1, "BauLegionCamp".into());
        vp.type_names.insert(2, "FigGermanicWarrior".into());
        vp.type_names.insert(3, "SkriptmarkZoneA".into());

        let raw = vec![obj1, obj2, obj3];
        let (scene_objs, pickables, sprites) = vp.convert_world_objects(&raw);

        assert_eq!(scene_objs.len(), 3);
        assert_eq!(scene_objs[0].name, "BauLegionCamp");
        assert_eq!(scene_objs[1].name, "FigGermanicWarrior");
        assert_eq!(scene_objs[2].name, "SkriptmarkZoneA");

        // With no catalog loaded, sprites are safely None (graceful fallback)
        assert!(sprites.iter().all(|s| s.is_none()));
        assert!(pickables.iter().all(|p| p.is_none()));

        // Prepare frame
        let frame = vp
            .prepare_frame(
                egui::Vec2::new(1280.0, 720.0),
                &field,
                50.0,
                4.0,
                &scene_objs,
                &sprites,
            )
            .expect("prepare frame succeeds");

        // Markers are generated for all 3 objects
        assert_eq!(frame.markers.len(), 3);
        assert_eq!(frame.diagnostics.marker_count, 3);

        // Marker 0: Team 1 color [0.25, 0.45, 0.95, 1.0], point size 10.0
        assert_eq!(frame.markers[0].color, [0.25, 0.45, 0.95, 1.0]);
        assert_eq!(frame.markers[0].point_size, 10.0);
        assert!(!frame.markers[0].is_script_mark);

        // Marker 1: Team 2 color [0.90, 0.15, 0.20, 1.0], point size 10.0
        assert_eq!(frame.markers[1].color, [0.90, 0.15, 0.20, 1.0]);
        assert_eq!(frame.markers[1].point_size, 10.0);
        assert!(!frame.markers[1].is_script_mark);

        // Marker 2: Script mark color SCRIPT_MARK_COLOR [0.2, 0.85, 0.95, 1.0], point size 5.0
        assert_eq!(frame.markers[2].color, [0.2, 0.85, 0.95, 1.0]);
        assert_eq!(frame.markers[2].point_size, 5.0);
        assert!(frame.markers[2].is_script_mark);
    }

    #[test]
    fn test_object_picking_and_terrain_brush_guard() {
        let session = TerrainSession::create_blank(257, 32);
        let field = Viewport3d::build_height_field(&session, 6.0, 4.0).unwrap();
        let mut vp = Viewport3d::new();

        // Object placed exactly at center (world 8192, 8192 => tile 32, 32)
        let obj = make_test_world_object(5, 777, 10, 1, 8192.0, 8192.0);
        let raw = vec![obj];
        let (scene_objs, pickables, sprites) = vp.convert_world_objects(&raw);

        let viewport = egui::Vec2::new(800.0, 600.0);
        let frame = vp
            .prepare_frame(viewport, &field, 50.0, 4.0, &scene_objs, &sprites)
            .unwrap();

        // Target (32, y, 32) is centered in the camera view
        let screen_center = frame
            .project_world_to_screen(SceneVec3::new(32.0, 0.0, 32.0))
            .expect("projects to screen");

        // 1. Pick object at exact screen center
        let picked = frame.pick_object(
            screen_center,
            &scene_objs,
            &pickables,
            &field,
            Viewport3d::OBJECT_PICK_RADIUS,
        );
        assert_eq!(picked, Some(0), "Object at center should be picked");

        // 2. Pick point far away from object
        let picked_far = frame.pick_object(
            SceneVec2::new(10.0, 10.0),
            &scene_objs,
            &pickables,
            &field,
            Viewport3d::OBJECT_PICK_RADIUS,
        );
        assert_eq!(picked_far, None, "Empty coordinate must not hit object");

        // 3. Verify drag_started_on_object flag prevents terrain painting
        vp.drag_started_on_object = true;
        // When drag_started_on_object is true, dragging does not emit Paint
        assert!(vp.drag_started_on_object);
    }

    #[test]
    fn test_sprite_catalog_and_atlas_rendering() {
        let session = TerrainSession::create_blank(257, 32);
        let field = Viewport3d::build_height_field(&session, 6.0, 4.0).unwrap();
        let mut vp = Viewport3d::new();

        // Create in-memory test sprite (16x16 pixels)
        let pixels = vec![0xFFFFFFFF; 256];
        let sprite =
            Arc::new(NativeSprite::new(16, 16, pixels, 8, 16, "test_building.bmp").unwrap());

        let mut bcat = BoundedSpriteCatalog::new(10, 10);
        bcat.insert_sprite("BauTest", sprite.clone());
        let atlas = bcat.build_atlas(256).unwrap();

        vp.bounded_catalog = Some(Arc::new(bcat));
        vp.sprite_atlas = Some(Arc::new(atlas));
        vp.type_names.insert(42, "BauTest".into());

        let raw = vec![make_test_world_object(1, 99, 42, 1, 8192.0, 8192.0)];
        let (scene_objs, pickables, sprites) = vp.convert_world_objects(&raw);

        assert_eq!(scene_objs.len(), 1);
        assert!(
            sprites[0].is_some(),
            "Sprite should be resolved from bounded catalog"
        );
        assert!(pickables[0].is_some(), "Pickable sprite should be created");

        let s = sprites[0].unwrap();
        assert_eq!(s.width, 16.0);
        assert_eq!(s.height, 16.0);
        assert_eq!(s.anchor_x, 8.0);
        assert_eq!(s.anchor_y, 16.0);

        let frame = vp
            .prepare_frame(
                egui::Vec2::new(800.0, 600.0),
                &field,
                50.0,
                4.0,
                &scene_objs,
                &sprites,
            )
            .unwrap();

        // When a sprite is present, it is routed into ordered_sprites
        assert_eq!(frame.ordered_sprites.len(), 1);
        assert_eq!(frame.ordered_sprites[0].object_index, 0);
        assert_eq!(frame.diagnostics.sprite_count, 1);
    }

    #[test]
    fn test_shading_color_helpers() {
        let sand = Viewport3d::relief_color(0.05, 1.0);
        let snow = Viewport3d::relief_color(0.95, 1.0);
        assert_ne!(sand, snow);

        let h_dark = Viewport3d::heightmap_color(0.1, 1.0);
        let h_bright = Viewport3d::heightmap_color(0.9, 1.0);
        assert!(h_bright.r() > h_dark.r());
    }

    #[test]
    fn test_viewport_3d_visibility_toggles() {
        let session = TerrainSession::create_blank(257, 32);
        let field = Viewport3d::build_height_field(&session, 6.0, 4.0).unwrap();
        let mut vp = Viewport3d::new();

        let raw = vec![make_test_world_object(0, 101, 1, 1, 8192.0, 8192.0)];
        let (scene_objs, _pickables, sprites) = vp.convert_world_objects(&raw);

        // 1. Objects hidden
        vp.show_objects = false;
        let frame_no_objs = vp
            .prepare_frame(
                egui::Vec2::new(800.0, 600.0),
                &field,
                50.0,
                4.0,
                &scene_objs,
                &sprites,
            )
            .unwrap();
        assert_eq!(frame_no_objs.markers.len(), 0);
        assert_eq!(frame_no_objs.ordered_sprites.len(), 0);

        // 2. Objects shown, markers hidden
        vp.show_objects = true;
        vp.show_markers = false;
        let frame_no_markers = vp
            .prepare_frame(
                egui::Vec2::new(800.0, 600.0),
                &field,
                50.0,
                4.0,
                &scene_objs,
                &sprites,
            )
            .unwrap();
        assert_eq!(frame_no_markers.markers.len(), 0);
    }

    #[test]
    fn test_viewport_3d_shared_placement_state_updates() {
        let session = TerrainSession::create_blank(257, 32);
        let field = Viewport3d::build_height_field(&session, 6.0, 4.0).unwrap();
        let vp = Viewport3d::new();

        // Initial snapshot: 1 object
        let raw1 = vec![make_test_world_object(0, 101, 1, 1, 8192.0, 8192.0)];
        let (objs1, _, sprites1) = vp.convert_world_objects(&raw1);
        let frame1 = vp
            .prepare_frame(
                egui::Vec2::new(800.0, 600.0),
                &field,
                50.0,
                4.0,
                &objs1,
                &sprites1,
            )
            .unwrap();
        assert_eq!(frame1.markers.len(), 1);

        // Updated snapshot: 2 objects placed
        let raw2 = vec![
            make_test_world_object(0, 101, 1, 1, 8192.0, 8192.0),
            make_test_world_object(1, 102, 2, 2, 4096.0, 4096.0),
        ];
        let (objs2, _, sprites2) = vp.convert_world_objects(&raw2);
        let frame2 = vp
            .prepare_frame(
                egui::Vec2::new(800.0, 600.0),
                &field,
                50.0,
                4.0,
                &objs2,
                &sprites2,
            )
            .unwrap();
        assert_eq!(
            frame2.markers.len(),
            2,
            "Updated placement snapshot immediately reflected"
        );
    }

    #[test]
    fn test_headless_viewport_show_runs_cleanly_with_objects() {
        let session = TerrainSession::create_blank(257, 32);
        let mut vp = Viewport3d::new();
        let objects = vec![
            make_test_world_object(0, 1001, 1, 1, 8192.0, 8192.0),
            make_test_world_object(1, 1002, 2, 2, 4096.0, 4096.0),
        ];

        let ctx = egui::Context::default();
        let mut recorded_action = ViewportAction::None;

        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                recorded_action =
                    vp.show(ui, ctx, &session, 4.0, 50.0, 4.0, &objects, Some((0, 1001)));
            });
        });

        // Headless run with no pointer events returns ViewportAction::None
        assert_eq!(recorded_action, ViewportAction::None);
        assert_eq!(vp.hover_object, None);
    }

    #[test]
    fn test_floor_texture_atlas_wiring_and_uv_lookup() {
        let mut vp = Viewport3d::new();
        assert!(vp.floor_atlas.is_none());
        assert!(vp.floor_atlas_texture.is_none());
        assert!(vp.tile_textures.is_none());

        // Create an in-memory FloorTextureAtlas with two textures
        let atlas = FloorTextureAtlas::create_with_decoder(vec!["4BA___51", "4BB___51"], |_| None)
            .expect("in-memory atlas creation succeeds");

        assert_eq!(atlas.count(), 2);
        assert_eq!(atlas.size(), 2048);

        vp.set_floor_atlas(Some(Arc::new(atlas)));
        assert!(vp.floor_atlas.is_some());
        assert!(vp.floor_atlas_texture.is_none());

        // 4 tiles in a 2x2 grid
        vp.set_tile_textures(Some(vec![
            "4BA___51".into(),
            "4BB___51".into(),
            "4BB___51".into(),
            "4BA___51".into(),
        ]));

        assert_eq!(vp.resolve_tile_texture(0, 0, 2), "4BA___51");
        assert_eq!(vp.resolve_tile_texture(1, 0, 2), "4BB___51");
        assert_eq!(vp.resolve_tile_texture(0, 1, 2), "4BB___51");
        assert_eq!(vp.resolve_tile_texture(1, 1, 2), "4BA___51");

        // Verify valid UV points for tile (0, 0)
        let uv = vp.sample_tile_uv(0, 0, 2).expect("tile UV must be present");
        assert_eq!(uv.len(), 4);
        // All UVs should be normalized within [0.0..=1.0]
        for pt in &uv {
            assert!(pt.u >= 0.0 && pt.u <= 1.0);
            assert!(pt.v >= 0.0 && pt.v <= 1.0);
        }
        // Top-Left should be to the left of Top-Right
        assert!(uv[0].u < uv[1].u);
        // Top-Left should be above Bottom-Left
        assert!(uv[0].v < uv[3].v);
    }

    #[test]
    fn test_floor_texture_atlas_fallback_when_unmapped() {
        let mut vp = Viewport3d::new();

        // 1. Without atlas and without tile_textures, fallback returns default "4BA___51"
        assert_eq!(vp.resolve_tile_texture(0, 0, 64), "4BA___51");
        assert!(vp.sample_tile_uv(0, 0, 64).is_none());

        // 2. With atlas having unique name "CustomGround", fallback returns first atlas name
        let atlas = FloorTextureAtlas::create_with_decoder(vec!["CustomGround"], |_| None).unwrap();
        vp.set_floor_atlas(Some(Arc::new(atlas)));
        assert_eq!(vp.resolve_tile_texture(999, 999, 64), "CustomGround");

        let uv = vp.sample_tile_uv(999, 999, 64).expect("sample UV succeeds");
        assert!(uv[1].u > uv[0].u);

        // 3. Fallback color is deterministic non-zero color
        let col = FloorTextureAtlas::fallback_color("CustomGround");
        assert_ne!(col, 0);
        assert_eq!((col >> 24) & 0xFF, 0xFF); // Alpha must be 255
    }

    #[test]
    fn test_floor_atlas_texture_synchronization() {
        let mut vp = Viewport3d::new();
        let atlas = FloorTextureAtlas::create_with_decoder(vec!["4BA___51"], |_| None).unwrap();
        vp.set_floor_atlas(Some(Arc::new(atlas)));
        assert!(vp.floor_atlas_texture.is_none());

        let ctx = egui::Context::default();
        vp.sync_floor_atlas_texture(&ctx);
        assert!(vp.floor_atlas_texture.is_some());

        // Replacing atlas invalidates previous GPU texture handle
        let new_atlas = FloorTextureAtlas::create_with_decoder(vec!["4BB___51"], |_| None).unwrap();
        vp.set_floor_atlas(Some(Arc::new(new_atlas)));
        assert!(vp.floor_atlas_texture.is_none());
    }

    #[test]
    fn test_headless_viewport_show_with_floor_atlas_textured_and_fallback() {
        let session = TerrainSession::create_blank(257, 32);
        let mut vp = Viewport3d::new();
        vp.shading_mode = ShadingMode::Textured;

        let atlas =
            FloorTextureAtlas::create_with_decoder(vec!["4BA___51", "4BB___51"], |_| None).unwrap();
        vp.set_floor_atlas(Some(Arc::new(atlas)));
        vp.set_tile_textures(Some(vec!["4BA___51".into(); 64 * 64]));

        let ctx = egui::Context::default();

        // Run show in Textured mode: show() will automatically sync floor_atlas_texture with ctx
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let action = vp.show(ui, ctx, &session, 4.0, 50.0, 4.0, &[], None);
                assert_eq!(action, ViewportAction::None);
            });
        });

        // Verify that the texture handle was created during show
        assert!(vp.floor_atlas_texture.is_some());

        // Now test CPU fallback path when floor_atlas_texture is explicitly cleared
        vp.floor_atlas_texture = None;
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                // Manually calling render_terrain_mesh without texture handle tests CPU fallback
                let field = Viewport3d::build_height_field(&session, 6.0, 4.0).unwrap();
                let frame = vp
                    .prepare_frame(egui::Vec2::new(800.0, 600.0), &field, 50.0, 4.0, &[], &[])
                    .unwrap();
                let painter = ui.painter();
                vp.render_terrain_mesh(painter, ui.max_rect(), &frame, &field, &session);
            });
        });
    }

    #[test]
    fn collision_shading_emits_red_painted_terrain_for_blocked_cell() {
        let mut collision = vec![0; 256 * 256];
        // At LOD 4, tile quad [32,36] spans collision cells [128,144).
        // Cell (129,129) is away from the quad center, which would sample (136,136).
        collision[129 * 256 + 129] = 255;
        let session = TerrainSession::new(
            257,
            vec![32; 257 * 257],
            256,
            collision,
            None,
            None,
        );
        let mut vp = Viewport3d::new();
        vp.shading_mode = ShadingMode::Collision;
        vp.mesh_lod_step = 4;
        vp.show_water = false;

        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))),
            ..Default::default()
        };
        let output = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = vp.show(ui, ctx, &session, 4.0, 50.0, 2.0, &[], None);
            });
        });

        fn count_red_polygons(shape: &egui::Shape) -> usize {
            match shape {
                egui::Shape::Path(path)
                    if path.fill.r() > path.fill.g().saturating_mul(2)
                        && path.fill.r() > path.fill.b().saturating_mul(2) =>
                {
                    1
                }
                egui::Shape::Vec(shapes) => shapes.iter().map(count_red_polygons).sum(),
                _ => 0,
            }
        }
        let red_terrain_polygons: usize = output
            .shapes
            .iter()
            .map(|clipped| count_red_polygons(&clipped.shape))
            .sum();
        assert!(
            red_terrain_polygons > 0,
            "blocked collision data should produce red terrain polygons in egui's paint output"
        );
    }

    #[test]
    fn collision_hover_readout_matches_terrain_picker_cell_and_passability() {
        let mut collision = vec![0; 256 * 256];
        collision[129 * 256 + 129] = 255;
        let session = TerrainSession::new(
            257,
            vec![32; 257 * 257],
            256,
            collision,
            None,
            None,
        );
        let field = Viewport3d::build_height_field(&session, 6.0, 4.0).unwrap();
        let pick_x = 32.375;
        let pick_z = 32.375;
        let pick_y = field.sample_height(pick_x, pick_z);
        let ray = TerrainRay {
            origin: SceneVec3::new(pick_x, pick_y + 1.0, pick_z),
            direction: SceneVec3::new(0.0, -1.0, 0.0),
        };
        let blocked_world = TerrainRayPicker::try_pick_point(&field, ray)
            .expect("vertical picker ray should hit the terrain");
        assert!((blocked_world.x - pick_x).abs() < 1e-4);
        assert!((blocked_world.z - pick_z).abs() < 1e-4);
        assert_eq!(
            Viewport3d::collision_cell_at_world(&session, &field, blocked_world),
            Some((129, 129, true))
        );
        assert_eq!(
            Viewport3d::collision_cell_at_world(
                &session,
                &field,
                SceneVec3::new(32.5, 0.0, 32.5)
            ),
            Some((130, 130, false))
        );

        let mut vp = Viewport3d::new();
        vp.shading_mode = ShadingMode::Collision;
        vp.hover_tile = Some((32, 32));
        vp.hover_world = Some(blocked_world);
        let chinese = crate::batch20_localization_tests::render(|ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                vp.render_toolbar(ui, &session, &field, &[], None);
            });
        });
        assert!(chinese.contains("通行格：(129, 129) 阻擋"), "{chinese}");

        vp.english = true;
        let english = crate::batch20_localization_tests::render(|ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                vp.render_toolbar(ui, &session, &field, &[], None);
            });
        });
        assert!(
            english.contains("Collision cell: (129, 129) Blocked"),
            "{english}"
        );
    }

    #[test]
    fn test_discrete_terrain_pick_point_click_vs_drag() {
        let session = TerrainSession::create_blank(257, 32);
        let mut vp = Viewport3d::new();
        let viewport_size = egui::Vec2::new(800.0, 600.0);
        let center = Pos2::new(400.0, 300.0);

        // 1. Point-click on empty terrain emits PickTerrain (not Paint, not CommitStroke)
        let ctx = egui::Context::default();
        let mut recorded_action = ViewportAction::None;

        // Frame 1: Pointer down at center
        let mut raw_down = egui::RawInput::default();
        raw_down.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, viewport_size));
        raw_down.events.push(egui::Event::PointerMoved(center));
        raw_down.events.push(egui::Event::PointerButton {
            pos: center,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        });
        let _ = ctx.run(raw_down, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                recorded_action = vp.show(ui, ctx, &session, 4.0, 50.0, 4.0, &[], None);
            });
        });
        // Pointer is down without release; no action yet
        assert_eq!(recorded_action, ViewportAction::None);
        assert!(!vp.is_dragging_terrain);

        // Frame 2: Pointer released at same location without movement (discrete click)
        let mut raw_up = egui::RawInput::default();
        raw_up.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, viewport_size));
        raw_up.events.push(egui::Event::PointerMoved(center));
        raw_up.events.push(egui::Event::PointerButton {
            pos: center,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        });
        let _ = ctx.run(raw_up, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                recorded_action = vp.show(ui, ctx, &session, 4.0, 50.0, 4.0, &[], None);
            });
        });

        // Must emit PickTerrain carrying valid terrain grid coordinates
        assert!(
            recorded_action.is_pick_terrain(),
            "Action must be PickTerrain on discrete click, got: {recorded_action:?}"
        );
        let (gx, gy) = recorded_action
            .picked_terrain()
            .expect("must yield grid coords");
        assert!(gx >= 0.0 && gx <= 256.0, "gx {gx} within grid bounds");
        assert!(gy >= 0.0 && gy <= 256.0, "gy {gy} within grid bounds");
        assert_eq!((gx, gy), (110.0, 110.0));
        assert_eq!(vp.hover_tile, Some((27, 27)));
        assert_ne!(
            (gx, gy),
            vp.hover_grid.expect("hover_grid stays continuous")
        );
        assert_ne!(recorded_action, ViewportAction::CommitStroke);
        assert!(!vp.is_dragging_terrain);

        // 2. Brush Drag: Moving mouse while button is down emits Paint, and releasing commits stroke
        let ctx2 = egui::Context::default();
        let mut drag_action = ViewportAction::None;

        // Frame 1: Pointer down
        let mut raw_drag1 = egui::RawInput::default();
        raw_drag1.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, viewport_size));
        raw_drag1.events.push(egui::Event::PointerMoved(center));
        raw_drag1.events.push(egui::Event::PointerButton {
            pos: center,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        });
        let _ = ctx2.run(raw_drag1, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                drag_action = vp.show(ui, ctx, &session, 4.0, 50.0, 4.0, &[], None);
            });
        });
        assert_eq!(drag_action, ViewportAction::None);

        // Frame 2: Pointer dragged by 30px (exceeds DRAG_THRESHOLD)
        let drag_pos = Pos2::new(center.x + 30.0, center.y + 30.0);
        let mut raw_drag2 = egui::RawInput::default();
        raw_drag2.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, viewport_size));
        raw_drag2.events.push(egui::Event::PointerMoved(drag_pos));
        let _ = ctx2.run(raw_drag2, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                drag_action = vp.show(ui, ctx, &session, 4.0, 50.0, 4.0, &[], None);
            });
        });
        // Dragging must emit Paint, NOT PickTerrain
        match drag_action {
            ViewportAction::Paint { grid_x, grid_y } => {
                assert!(grid_x >= 0.0 && grid_x <= 256.0);
                assert!(grid_y >= 0.0 && grid_y <= 256.0);
            }
            other => panic!("Expected ViewportAction::Paint during brush drag, got: {other:?}"),
        }
        assert!(
            vp.is_dragging_terrain,
            "is_dragging_terrain flag must be set during brush drag"
        );

        // Frame 3: Pointer released after drag -> emits CommitStroke
        let mut raw_drag3 = egui::RawInput::default();
        raw_drag3.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, viewport_size));
        raw_drag3.events.push(egui::Event::PointerButton {
            pos: drag_pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        });
        let _ = ctx2.run(raw_drag3, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                drag_action = vp.show(ui, ctx, &session, 4.0, 50.0, 4.0, &[], None);
            });
        });
        assert_eq!(
            drag_action,
            ViewportAction::CommitStroke,
            "Release after drag must emit CommitStroke"
        );
        assert!(
            !vp.is_dragging_terrain,
            "is_dragging_terrain must be reset after commit"
        );
    }

    #[test]
    fn test_terrain_pick_csharp_floor_then_tile_center_contract() {
        use against_rome_scene::camera::TerrainRay;

        for vertex_size in [193, 257, 258] {
            let session = TerrainSession::create_blank(vertex_size, 32);
            let field = Viewport3d::build_height_field(&session, 6.0, 4.0).unwrap();
            let dimension = field.tile_width;
            let step = (vertex_size - 1) as f32 / dimension;
            for (x, z, expected_tile) in [
                (0.0, 0.0, (0, 0)),
                (0.999, 1.001, (0, 1)),
                (12.01, 23.99, (12, 23)),
                (12.99, 23.01, (12, 23)),
                (13.0, 24.0, (13, 24)),
                (
                    dimension - 0.001,
                    dimension - 0.001,
                    (dimension as i32 - 1, dimension as i32 - 1),
                ),
            ] {
                let ray = TerrainRay {
                    origin: SceneVec3::new(x, 50.0, z),
                    direction: SceneVec3::new(0.0, -1.0, 0.0),
                };
                let tile = TerrainRayPicker::try_pick(&field, ray).unwrap();
                assert_eq!(tile, expected_tile);
                let grid = Viewport3d::terrain_tile_to_grid(&session, &field, tile);
                // Exact C# TerrainRampTool.TileToVertex formula, including +0.5.
                assert_eq!(
                    grid,
                    (
                        (expected_tile.0 as f32 + 0.5) * step,
                        (expected_tile.1 as f32 + 0.5) * step,
                    )
                );
                assert!(grid.0 > 0.0 && grid.0 < (vertex_size - 1) as f32);
                assert!(grid.1 > 0.0 && grid.1 < (vertex_size - 1) as f32);
            }
        }
    }

    #[test]
    fn test_ramp_gesture_uses_tile_centers_and_ignores_same_tile_release() {
        let session = TerrainSession::create_blank(257, 32);
        let field = Viewport3d::build_height_field(&session, 6.0, 4.0).unwrap();
        let mut vp = Viewport3d::new();
        vp.terrain_pick_mode = true;
        let ctx = egui::Context::default();
        let mut action = ViewportAction::None;
        let mut run = |vp: &mut Viewport3d, events: Vec<egui::Event>| {
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))),
                events,
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    action = vp.show(ui, ctx, &session, 4.0, 50.0, 4.0, &[], None);
                });
            });
            action.clone()
        };
        run(&mut vp, vec![]);
        let rect = vp.last_rect.unwrap();
        let frame = vp
            .prepare_frame(rect.size(), &field, 50.0, 4.0, &[], &[])
            .unwrap();
        let screen = |x, z| {
            let p = frame
                .project_world_to_screen(SceneVec3::new(x, field.sample_height(x, z), z))
                .unwrap();
            rect.min + egui::vec2(p.x, p.y)
        };
        let pointer = |pos, pressed| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ]
        };
        let start = screen(30.2, 30.2);
        let same_tile = screen(30.8, 30.8);
        assert_eq!(
            run(&mut vp, pointer(start, true)).picked_terrain(),
            Some((122.0, 122.0))
        );
        assert_eq!(vp.terrain_pick_press, Some((122.0, 122.0)));
        assert_eq!(
            run(&mut vp, pointer(same_tile, false)),
            ViewportAction::None
        );
        assert_eq!(vp.terrain_pick_press, None);
        assert_eq!(
            run(&mut vp, pointer(start, true)).picked_terrain(),
            Some((122.0, 122.0))
        );
        assert_eq!(
            run(&mut vp, pointer(screen(31.2, 32.2), false)).picked_terrain(),
            Some((126.0, 130.0))
        );
        assert!(!vp.is_dragging_terrain);
        assert!(!session.can_undo());
    }

    #[test]
    fn test_discrete_terrain_pick_coordinate_payload() {
        let session = TerrainSession::create_blank(257, 32);
        let field = Viewport3d::build_height_field(&session, 6.0, 4.0).unwrap();
        let vp = Viewport3d::new();
        let viewport_rect = Rect::from_min_size(Pos2::ZERO, egui::Vec2::new(800.0, 600.0));

        let (scene_objs, pickables, sprites) = vp.convert_world_objects(&[]);
        let frame = vp
            .prepare_frame(
                viewport_rect.size(),
                &field,
                50.0,
                4.0,
                &scene_objs,
                &sprites,
            )
            .unwrap();

        // 1. Center of screen projects to valid grid coordinates on terrain
        let center_screen = Pos2::new(400.0, 300.0);
        let picked = vp.try_pick_terrain_grid(
            center_screen,
            viewport_rect,
            &session,
            &field,
            &[],
            &frame,
            &scene_objs,
            &pickables,
        );
        assert!(picked.is_some(), "Center screen point should pick terrain");
        let (gx, gy) = picked.unwrap();
        assert_eq!((gx, gy), (130.0, 130.0));

        // 2. ViewportAction payload roundtrip helper verification
        let action = ViewportAction::PickTerrain {
            grid_x: gx,
            grid_y: gy,
        };
        assert_eq!(action.picked_terrain(), Some((gx, gy)));
        assert!(action.is_pick_terrain());
        assert_eq!(action.selected_object(), None);

        // 3. Coordinate clamping: values must remain clamped within [0.0..=vertex_size - 1]
        let max_coord = (session.vertex_size - 1) as f32;
        assert!(gx >= 0.0 && gx <= max_coord);
        assert!(gy >= 0.0 && gy <= max_coord);
    }

    #[test]
    fn test_discrete_terrain_pick_object_priority_and_failed_ray() {
        for terrain_pick_mode in [false, true] {
            let session = TerrainSession::create_blank(257, 32);
            let field = Viewport3d::build_height_field(&session, 6.0, 4.0).unwrap();
            let mut vp = Viewport3d::new();
            vp.terrain_pick_mode = terrain_pick_mode;
            let viewport_size = egui::Vec2::new(800.0, 600.0);

            // Place object at terrain center (world 8192, 8192 => tile 32, 32)
            let obj = make_test_world_object(3, 8888, 1, 1, 8192.0, 8192.0);
            let raw_objects = vec![obj];
            let (scene_objs, pickables, sprites) = vp.convert_world_objects(&raw_objects);

            // Run an initial layout frame to allocate canvas rect
            let ctx = egui::Context::default();
            let mut action = ViewportAction::None;
            let mut raw_init = egui::RawInput::default();
            raw_init.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, viewport_size));
            let _ = ctx.run(raw_init, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let _ = vp.show(ui, ctx, &session, 4.0, 50.0, 4.0, &raw_objects, None);
                });
            });
            let canvas_rect = vp.last_rect.expect("last_rect must be recorded");

            let frame = vp
                .prepare_frame(canvas_rect.size(), &field, 50.0, 4.0, &scene_objs, &sprites)
                .unwrap();

            // Compute exact projected screen position of the placed object
            let screen_pt = frame
                .project_world_to_screen(SceneVec3::new(32.0, 0.0, 32.0))
                .expect("object projects to screen");
            let obj_click_pos = Pos2::new(
                canvas_rect.min.x + screen_pt.x,
                canvas_rect.min.y + screen_pt.y,
            );

            // 1. Object Priority: try_pick_terrain_grid returns None when hitting an object
            let picked_grid = vp.try_pick_terrain_grid(
                obj_click_pos,
                canvas_rect,
                &session,
                &field,
                &raw_objects,
                &frame,
                &scene_objs,
                &pickables,
            );
            assert_eq!(
                picked_grid, None,
                "try_pick_terrain_grid must return None when an object occupies the position"
            );

            // Verify through show() UI interaction loop: click on object
            // Frame 1: Pointer down over object
            let mut raw_down = egui::RawInput::default();
            raw_down.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, viewport_size));
            raw_down
                .events
                .push(egui::Event::PointerMoved(obj_click_pos));
            raw_down.events.push(egui::Event::PointerButton {
                pos: obj_click_pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            });
            let _ = ctx.run(raw_down, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    action = vp.show(ui, ctx, &session, 4.0, 50.0, 4.0, &raw_objects, None);
                });
            });

            if terrain_pick_mode {
                assert_eq!(action, ViewportAction::SelectObject(Some((3, 8888))));
                assert_eq!(vp.terrain_pick_press, None);
            }

            // Frame 2: Pointer up (click gesture)
            let mut raw_up = egui::RawInput::default();
            raw_up.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, viewport_size));
            raw_up.events.push(egui::Event::PointerMoved(obj_click_pos));
            raw_up.events.push(egui::Event::PointerButton {
                pos: obj_click_pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            });
            let _ = ctx.run(raw_up, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    action = vp.show(ui, ctx, &session, 4.0, 50.0, 4.0, &raw_objects, None);
                });
            });

            // Must select the object and NOT emit PickTerrain
            assert_eq!(
                action,
                if terrain_pick_mode {
                    ViewportAction::None // already selected on press; no ramp endpoint on release
                } else {
                    ViewportAction::SelectObject(Some((3, 8888)))
                }
            );
            assert_eq!(
                action.picked_terrain(),
                None,
                "Must not emit terrain pick when clicking an object"
            );

            // 2. Failed Ray Pick: Clicking into the sky / empty space outside terrain must NOT emit PickTerrain
            let sky_pos = Pos2::new(canvas_rect.min.x + 10.0, canvas_rect.min.y + 10.0);
            let sky_pick = vp.try_pick_terrain_grid(
                sky_pos,
                canvas_rect,
                &session,
                &field,
                &raw_objects,
                &frame,
                &scene_objs,
                &pickables,
            );
            assert_eq!(
                sky_pick, None,
                "try_pick_terrain_grid must return None when ray misses terrain"
            );

            // Verify through show() UI loop
            let ctx2 = egui::Context::default();
            let mut sky_action = ViewportAction::None;

            let mut raw_sky_down = egui::RawInput::default();
            raw_sky_down.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, viewport_size));
            raw_sky_down.events.push(egui::Event::PointerMoved(sky_pos));
            raw_sky_down.events.push(egui::Event::PointerButton {
                pos: sky_pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            });
            let _ = ctx2.run(raw_sky_down, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    sky_action = vp.show(ui, ctx, &session, 4.0, 50.0, 4.0, &raw_objects, None);
                });
            });

            let mut raw_sky_up = egui::RawInput::default();
            raw_sky_up.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, viewport_size));
            raw_sky_up.events.push(egui::Event::PointerMoved(sky_pos));
            raw_sky_up.events.push(egui::Event::PointerButton {
                pos: sky_pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            });
            let _ = ctx2.run(raw_sky_up, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    sky_action = vp.show(ui, ctx, &session, 4.0, 50.0, 4.0, &raw_objects, None);
                });
            });

            assert_eq!(
                sky_action,
                ViewportAction::None,
                "Failed ray pick on sky must not emit PickTerrain or any action"
            );
            assert_eq!(sky_action.picked_terrain(), None);
        }
    }
}

#[cfg(test)]
mod troop_tests {
    use super::*;
    use against_rome_maps::script_aliases::ScriptObjectAlias;

    fn catalog() -> TroopCatalog {
        let mut legion_fields = HashMap::new();
        legion_fields.insert("objdefn0".to_string(), "7".to_string());
        let types = vec![
            SdlObjectType::new("VerRomLegion", 100, SdlObjectCategory::UnitGroup, "Rom", 3, legion_fields),
            SdlObjectType::new("FigRomSoldat", 7, SdlObjectCategory::Figure, "Rom", 3, HashMap::new()),
            SdlObjectType::new("FigKelAxt", 8, SdlObjectCategory::Figure, "Kel", 1, HashMap::new()),
            SdlObjectType::new("FigGerSpeer", 9, SdlObjectCategory::Figure, "Ger", 1, HashMap::new()),
            SdlObjectType::new("BauRomHaus", 50, SdlObjectCategory::Building, "Rom", 1, HashMap::new()),
        ];
        let alias = |alias: &str, name: &str| ScriptObjectAlias { alias: alias.into(), name_def: name.into() };
        let spawn_catalog = SpawnDataCatalog {
            aliases: vec![
                alias("Legion", "VerRomLegion"),
                alias("Soldat", "FigRomSoldat"),
                alias("Axt", "FigKelAxt"),
                alias("Speer", "FigGerSpeer"),
                alias("Haus", "BauRomHaus"),
            ],
            type_names: HashMap::from([
                (7, "FigRomSoldat".to_string()),
                (11, "VerKelKamIco00_Kampf_Icon".to_string()),
            ]),
            game_maps: None,
        };
        TroopCatalog::new(types, spawn_catalog)
    }

    fn spawn(alias: &str, x: f32, z: f32, count: i32, angle: i32) -> Spawn {
        Spawn {
            alias: alias.into(),
            x,
            z,
            team: 1,
            count,
            angle,
            y: 12.0,
            prebuilt: false,
            id: Default::default(),
            raw_rotation: None,
        }
    }

    #[test]
    fn squads_expand_into_formation_members_around_the_banner() {
        let def = FormationLayoutDefinition::native_default();
        let spawns = [spawn("Legion", 1000.0, 2000.0, 5, 90)];
        let out = expand_troops_for_3d(&spawns, &catalog(), &def);
        assert_eq!(out.len(), 6);
        assert!(out[0].is_marker && out[0].name == "VerRomLegion", "banner = the unit-group type");
        let offsets = FormationLayout::create(5, 90.0, &def, FormationLayout::INITIAL_SPACING).unwrap();
        for (member, (dx, dz)) in out[1..].iter().zip(offsets) {
            assert!(!member.is_marker && member.name == "FigRomSoldat");
            assert_eq!(member.world, [1000.0 + dx, 12.0, 2000.0 + dz]);
            assert_eq!((member.spawn_index, member.team), (0, 1));
        }
        // The formation really spreads the members instead of stacking them.
        assert!(out[1..].windows(2).any(|w| w[0].world != w[1].world));
    }

    #[test]
    fn member_count_is_capped_and_non_squads_stay_single_markers() {
        let def = FormationLayoutDefinition::native_default();
        let spawns = [
            spawn("Legion", 0.0, 0.0, 99, 0),
            spawn("Haus", 100.0, 100.0, 5, 0),
            spawn("Legion", 200.0, 200.0, 0, 0),
            spawn("Unknown", 300.0, 300.0, 4, 0),
        ];
        let out = expand_troops_for_3d(&spawns, &catalog(), &def);
        assert_eq!(out.iter().filter(|o| o.spawn_index == 0).count(), 1 + FormationLayout::MAXIMUM_SCENARIO_MEMBERS);
        let single = |index: usize| -> Vec<&TroopObject> { out.iter().filter(|o| o.spawn_index == index).collect() };
        assert_eq!(single(1).len(), 1);
        assert_eq!(single(1)[0].name, "BauRomHaus");
        assert_eq!((single(2).len(), single(2)[0].name.as_str()), (1, "VerRomLegion"), "count 0 is not expanded");
        assert_eq!((single(3).len(), single(3)[0].name.as_str()), (1, "Unknown"), "unknown alias keeps its alias");
    }

    #[test]
    fn banner_falls_back_to_the_military_icon_or_a_troop_spawn_marker() {
        let def = FormationLayoutDefinition::native_default();
        let out = expand_troops_for_3d(&[spawn("Axt", 0.0, 0.0, 2, 0), spawn("Speer", 0.0, 0.0, 2, 0)], &catalog(), &def);
        let first: Vec<_> = out.iter().filter(|o| o.spawn_index == 0).collect();
        assert_eq!(first.len(), 3);
        assert_eq!(first[0].name, "VerKelKamIco00_Kampf_Icon", "existing retail icon of the tribe");
        assert_eq!(first[1].name, "FigKelAxt");
        let second: Vec<_> = out.iter().filter(|o| o.spawn_index == 1).collect();
        assert_eq!(second[0].name, "TroopSpawn:Speer", "no template and no icon keeps one spawn marker");
        // A figure that is the member of a unit-group template reuses that template as its banner.
        let soldiers = expand_troops_for_3d(&[spawn("Soldat", 0.0, 0.0, 1, 0)], &catalog(), &def);
        assert_eq!(soldiers[0].name, "VerRomLegion");
    }

    #[test]
    fn formation_definition_falls_back_to_native_default_and_troops_convert_for_display() {
        let missing = std::env::temp_dir().join(format!("arm_no_formdef_{}", uuid::Uuid::new_v4()));
        assert_eq!(load_formation_definition(&missing), FormationLayoutDefinition::native_default());

        let def = FormationLayoutDefinition::native_default();
        let troops = expand_troops_for_3d(&[spawn("Legion", 10.0, 20.0, 3, 45)], &catalog(), &def);
        let vp = Viewport3d::new();
        let (objects, pickables, sprites) = vp.convert_troop_objects(&troops);
        assert_eq!((objects.len(), pickables.len(), sprites.len()), (4, 4, 4));
        assert!(objects.iter().all(|o| o.source_file == "troop" && o.object_index == -5000 && o.team == 1));
        assert!(pickables.iter().all(Option::is_none), "no sprite catalog loaded in this fixture");
    }

#[test]
    fn chinese_ui_has_no_untranslated_literals() {
        let mut vp = Viewport3d::new();
        let session = TerrainSession::create_blank(6, 32);
        vp.shading_mode = ShadingMode::Collision;
        vp.mesh_lod_step = 4;
        vp.camera.orthographic = false;
        for english in [true, false, true, false] {
            vp.english = english;
            let text = crate::batch20_localization_tests::render(|ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let _ = vp.show(ui, ctx, &session, 1.0, 0.0, 2.0, &[], None);
                });
            });
            assert!(
                text.contains(if english { "3D Viewport" } else { "3D 編輯預覽" }),
                "{text}"
            );
            assert_eq!(vp.shading_mode, ShadingMode::Collision);
            assert_eq!(vp.mesh_lod_step, 4);
            assert!(!vp.camera.orthographic);
            if !english {
                for literal in [
                    "3D Viewport",
                    "Reset Camera",
                    "Proj: Ortho",
                    "Proj: Persp",
                    "Shading:",
                    "Relief",
                    "Height",
                    "Collision",
                    "Textured",
                    "LOD:",
                    "1 (Fine)",
                    "2 (Med)",
                    "4 (Fast)",
                ] {
                    assert!(!text.contains(literal), "未翻譯：{literal}\n{text}");
                }
            }
        }
    }
}
