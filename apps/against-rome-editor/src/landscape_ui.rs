//! Landscape Brush & Planning Studio UI: Cliff, River, Road, Wall/Palisade.
//! Replicates C# `TerrainRegionDialog.cs` landscape tools and integrates `native/against-rome-landscape`.

use against_rome_landscape::{
    cliff::{detect_from_vertex_heights, CliffDetectionResult, DEFAULT_MIN_ELEVATION_DELTA, DEFAULT_MIN_SLOPE_DEGREES},
    fortification::{
        plan_wall_stroke, FortificationStyle, WallComponentKind, WallStrokeOptions,
        WallStrokePlan, WallTileCatalog,
    },
    river::{
        plan as plan_river, rasterize_orthogonal_path, RiverElevationMode, RiverPlannerOptions,
        RiverStrokePlan, RiverTileCatalog,
    },
    road::{build_available as build_road_tiles, plan as plan_road, RoadStrokePlan},
    simulate_hydraulic_erosion, simulate_thermal_erosion, BrushRegion, HydraulicErosionParams,
    TerrainHeightWorkBuffer, TerrainSampleChange, ThermalErosionParams, Point,
};
use against_rome_textures::{TerrainBlendEditSession, TerrainRegionPlanner};
use crate::ambience_ui::AmbienceUi;
use eframe::egui::{self, Color32, Vec2};

/// Which plan the "Apply to map" button asks the app to write into the edit sessions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LandscapeApply {
    Cliff,
    River,
    Road,
    Wall,
}

/// Result of an erosion simulation in a rectangle, faithful to C# MapEditorForm.Erosion.cs.
/// Provides a standalone apply contract to preserve existing application architecture.
#[derive(Clone, Debug, PartialEq)]
pub struct ErosionPlan {
    /// Bounding rectangle in tile coordinates: `(min_tile_x, min_tile_y, max_tile_x, max_tile_y)`.
    pub tile_bounds: (i32, i32, i32, i32),
    /// Bounding rectangle in vertex coordinates: `(min_v, min_z, max_v, max_z)`.
    pub vertex_bounds: (i32, i32, i32, i32),
    /// Simulation center and radius on the height grid.
    pub brush: BrushRegion,
    /// Detailed height changes inside the selection rectangle.
    pub changes: Vec<TerrainSampleChange>,
    /// Minimum height change (delta = after - before).
    pub min_delta: i16,
    /// Maximum height change.
    pub max_delta: i16,
    /// Hydraulic erosion parameters used.
    pub hydraulic_params: HydraulicErosionParams,
    /// Thermal erosion parameters used.
    pub thermal_params: ThermalErosionParams,
    /// Whether hydraulic simulation was active.
    pub hydraulic_enabled: bool,
    /// Whether thermal simulation was active.
    pub thermal_enabled: bool,
}

impl ErosionPlan {
    /// Applies the planned sample changes directly into a mutable slice of terrain heights.
    /// Returns the number of height vertices changed.
    pub fn apply_to_heights(&self, heights: &mut [u8]) -> usize {
        let mut count = 0;
        for change in &self.changes {
            if change.index < heights.len() && heights[change.index] != change.after {
                heights[change.index] = change.after;
                count += 1;
            }
        }
        count
    }
}

/// Live map state a plan is computed against.
#[derive(Clone, Copy)]
pub struct LandscapeContext<'a> {
    /// Height grid vertices per side.
    pub vertex_size: usize,
    pub heights: Option<&'a [u8]>,
    /// Per-tile texture names of the loaded `boden.txt` session (defines the tile grid).
    pub textures: Option<&'a [String]>,
    /// Texture names the map/floortex library really has.
    pub texture_names: &'a [String],
}

impl<'a> LandscapeContext<'a> {
    pub fn plain(vertex_size: usize, heights: Option<&'a [u8]>) -> Self {
        Self { vertex_size, heights, textures: None, texture_names: &[] }
    }
    /// (tile dimension, height vertices per tile). Game tiles are 4 vertices wide on retail
    /// (257 vertices = 64 tiles); other sizes fall back to one vertex cell per tile.
    pub fn geometry(&self) -> (usize, usize) {
        let cells = self.vertex_size.saturating_sub(1);
        if let Some(t) = self.textures {
            let dim = (t.len() as f64).sqrt().round() as usize;
            if dim > 0 && dim * dim == t.len() && cells % dim == 0 {
                return (dim, cells / dim);
            }
        }
        if cells >= 4 && cells % 4 == 0 { (cells / 4, 4) } else { (cells.max(1), 1) }
    }
    /// Real texture grid when it matches the geometry, else a placeholder grid for preview only.
    fn texture_grid(&self) -> (i32, Vec<String>) {
        let (dim, _) = self.geometry();
        match self.textures {
            Some(t) if t.len() == dim * dim => (dim as i32, t.to_vec()),
            _ => (dim as i32, vec!["GRAS1".to_string(); dim * dim]),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LandscapeTab {
    CliffPlanner,
    RiverPlanner,
    RoadPlanner,
    WallPlanner,
    ErosionPlanner,
    RegionTools,
}

pub struct LandscapeUi {
    pub open: bool,
    pub current_tab: LandscapeTab,

    // --- Cliff state ---
    pub cliff_min_slope: f32,
    pub cliff_min_delta: f32,
    pub cliff_generate_scree: bool,
    pub last_cliff_result: Option<CliffDetectionResult>,

    // --- River state ---
    pub river_start_x: i32,
    pub river_start_y: i32,
    pub river_end_x: i32,
    pub river_end_y: i32,
    pub river_preferred_texture: String,
    pub river_mode: RiverElevationMode,
    pub last_river_plan: Option<RiverStrokePlan>,

    // --- Road state ---
    pub road_waypoints_text: String,
    pub road_preferred_texture: String,
    pub last_road_plan: Option<RoadStrokePlan>,

    // --- Wall state ---
    pub wall_style: FortificationStyle,
    pub wall_waypoints_text: String,
    pub wall_place_towers: bool,
    pub wall_place_gates: bool,
    /// One `id,name` DATA template per line; walls need an existing object of each type on the map.
    pub wall_templates_text: String,
    pub last_wall_plan: Option<WallStrokePlan>,

    // --- Erosion state ---
    /// Text input for rectangle vertices, faithful to C# TerrainRegionDialog format ("10,10\n30,30").
    pub erosion_rectangle_text: String,
    pub erosion_start_x: i32,
    pub erosion_start_y: i32,
    pub erosion_end_x: i32,
    pub erosion_end_y: i32,
    pub erosion_enable_hydraulic: bool,
    pub erosion_enable_thermal: bool,
    pub erosion_hydraulic_params: HydraulicErosionParams,
    pub erosion_thermal_params: ThermalErosionParams,
    pub last_erosion_plan: Option<ErosionPlan>,
    pub erosion_apply_requested: bool,

    // --- Region tools state (C# TerrainRegionDialog) ---
    pub region: RegionToolState,

    // Dirty / guards
    pub apply_requested: Option<LandscapeApply>,

    /// Cached read-out of the ambience boden water level (not persisted separately).
    pub cached_water_level: Option<f32>,

    pub english: bool,
    pub dirty: bool,
    pub pending_discard: bool,
    pub status_message: String,
    pub status_is_error: bool,
}

fn tl<'a>(english: bool, zh: &'a str, en: &'a str) -> &'a str {
    if english {
        en
    } else {
        zh
    }
}

impl Default for LandscapeUi {
    fn default() -> Self {
        Self {
            open: false,
            english: false,
            current_tab: LandscapeTab::CliffPlanner,
            cliff_min_slope: DEFAULT_MIN_SLOPE_DEGREES,
            cliff_min_delta: DEFAULT_MIN_ELEVATION_DELTA,
            cliff_generate_scree: false,
            last_cliff_result: None,
            river_start_x: 10,
            river_start_y: 10,
            river_end_x: 50,
            river_end_y: 50,
            river_preferred_texture: "FLUSS1".to_string(),
            river_mode: RiverElevationMode::ValidateOnly,
            last_river_plan: None,
            road_waypoints_text: "15,15\n30,15\n30,45".to_string(),
            road_preferred_texture: "PFAD1".to_string(),
            last_road_plan: None,
            wall_style: FortificationStyle::GermanicPalisade,
            wall_waypoints_text: "20,20\n40,20\n40,40\n20,40\n20,20".to_string(),
            wall_place_towers: true,
            wall_place_gates: true,
            wall_templates_text: String::new(),
            last_wall_plan: None,
            erosion_rectangle_text: "10,10\n30,30".to_string(),
            erosion_start_x: 10,
            erosion_start_y: 10,
            erosion_end_x: 30,
            erosion_end_y: 30,
            erosion_enable_hydraulic: true,
            erosion_enable_thermal: true,
            erosion_hydraulic_params: HydraulicErosionParams {
                droplet_count: 8000,
                ..Default::default()
            },
            erosion_thermal_params: ThermalErosionParams::default(),
            last_erosion_plan: None,
            erosion_apply_requested: false,
            region: RegionToolState::default(),
            apply_requested: None,
            cached_water_level: None,
            dirty: false,
            pending_discard: false,
            status_message: "Ready.".to_string(),
            status_is_error: false,
        }
    }
}

impl LandscapeUi {
    fn sync_water_level(&mut self, level: f32) {
        if self.cached_water_level.map(f32::to_bits) != Some(level.to_bits()) {
            if self.last_river_plan.take().is_some() {
                if self.apply_requested == Some(LandscapeApply::River) {
                    self.apply_requested = None;
                }
                self.status_message = tl(self.english, "水位已變更，請重新規劃河流。", "Water level changed; plan the river again.").to_string();
                self.status_is_error = false;
            }
            self.cached_water_level = Some(level);
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
            || self.last_cliff_result.is_some()
            || self.last_river_plan.is_some()
            || self.last_road_plan.is_some()
            || self.last_wall_plan.is_some()
            || self.last_erosion_plan.is_some()
            || self.region.plan.is_some()
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn clear_dirty(&mut self) {
        self.reset_session();
    }

    /// Resets all map-bound plans, detection results, and pending apply requests (cliff, river, road, wall, erosion).
    /// Preserves user preferences (open state, current tab, brush/slope options, waypoint text, erosion params).
    pub fn reset_session(&mut self) {
        self.dirty = false;
        self.pending_discard = false;
        self.last_cliff_result = None;
        self.last_river_plan = None;
        self.last_road_plan = None;
        self.last_wall_plan = None;
        self.last_erosion_plan = None;
        self.erosion_apply_requested = false;
        self.region.reset();
        self.apply_requested = None;
        self.status_message = "Ready.".to_string();
        self.status_is_error = false;
    }

    pub fn request_close(&mut self) {
        if self.is_dirty() {
            self.pending_discard = true;
        } else {
            self.open = false;
        }
    }

    pub fn parse_waypoints(text: &str) -> Vec<Point> {
        text.lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split(',').map(str::trim).collect();
                if parts.len() == 2 {
                    if let (Ok(x), Ok(y)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                        return Some((x, y));
                    }
                }
                None
            })
            .collect()
    }

    /// `vertex_size` is the height-grid vertex count.
    pub fn run_cliff_detection(&mut self, vertex_size: usize, heights: Option<&[u8]>) -> Result<(), String> {
        self.plan_cliffs(&LandscapeContext::plain(vertex_size, heights))
    }

    pub fn plan_cliffs(&mut self, cx: &LandscapeContext) -> Result<(), String> {
        let (tiles, step) = cx.geometry();
        let result = match cx.heights {
            Some(h) => detect_from_vertex_heights(
                cx.vertex_size,
                h,
                tiles,
                step,
                self.cliff_min_slope,
                self.cliff_min_delta,
            ),
            None => Err("No terrain loaded.".to_string()),
        };
        match result {
            Ok(result) => {
                let cell_count = result.cliff_cells.len();
                let contour_count = result.contours.len();
                self.last_cliff_result = Some(result);
                self.dirty = true;
                self.status_message = format!(
                    "Cliff detection completed: {} cliff cells, {} contour paths.",
                    cell_count, contour_count
                );
                self.status_is_error = false;
                Ok(())
            }
            Err(e) => {
                self.status_message = format!("Cliff detection error: {e}");
                self.status_is_error = true;
                Err(e)
            }
        }
    }

    pub fn run_river_planning(&mut self, vertex_size: usize, heights: Option<&[u8]>) -> Result<(), String> {
        self.plan_river(&LandscapeContext::plain(vertex_size, heights))
    }

    pub fn plan_river(&mut self, cx: &LandscapeContext) -> Result<(), String> {
        let path = rasterize_orthogonal_path(&[
            (self.river_start_x, self.river_start_y),
            (self.river_end_x, self.river_end_y),
        ]);
        let preferred = self.river_preferred_texture.trim();
        let preferred = (!preferred.is_empty()).then_some(preferred);
        let names = (!cx.texture_names.is_empty()).then_some(cx.texture_names);
        let catalog = RiverTileCatalog::build_available(names, preferred, None);
        let options = RiverPlannerOptions {
            preferred_river_texture: preferred.map(str::to_string),
            elevation_mode: self.river_mode,
            water_level: self.cached_water_level,
            ..Default::default()
        };
        let (tiles, textures) = cx.texture_grid();
        let heights = cx.heights.filter(|h| h.len() == cx.vertex_size * cx.vertex_size);

        match plan_river(tiles, &textures, heights, cx.vertex_size as i32, &path, &catalog, &options) {
            Ok(plan) => {
                self.status_message = format!(
                    "River planned: {} water tiles, {} bank tiles, {} height edits, {} unsupported.",
                    plan.water_tiles.len(),
                    plan.bank_tiles.len(),
                    plan.height_adjustments.len(),
                    plan.unsupported.len()
                );
                self.status_is_error = false;
                self.last_river_plan = Some(plan);
                self.dirty = true;
                Ok(())
            }
            Err(e) => {
                self.status_message = format!("River planning error: {e}");
                self.status_is_error = true;
                Err(e)
            }
        }
    }

    pub fn run_road_planning(&mut self, vertex_size: usize) -> Result<(), String> {
        self.plan_road(&LandscapeContext::plain(vertex_size, None))
    }

    pub fn plan_road(&mut self, cx: &LandscapeContext) -> Result<(), String> {
        self.last_road_plan = None;
        if self.apply_requested == Some(LandscapeApply::Road) {
            self.apply_requested = None;
        }
        let points = Self::parse_waypoints(&self.road_waypoints_text);
        if points.len() < 2 {
            return Err("At least 2 waypoints are required for road planning.".into());
        }

        let (tiles, textures) = cx.texture_grid();
        let preferred = self.road_preferred_texture.trim();
        let names = (!cx.texture_names.is_empty()).then_some(cx.texture_names);
        let available = build_road_tiles(names, (!preferred.is_empty()).then_some(preferred));

        match plan_road(tiles, &textures, &points, &available, preferred) {
            Ok(plan) => {
                self.status_message = format!(
                    "Road planned: {} tiles (Success: {}).",
                    plan.tiles.len(),
                    plan.succeeded
                );
                self.status_is_error = false;
                self.last_road_plan = Some(plan);
                self.dirty = true;
                Ok(())
            }
            Err(e) => {
                self.status_message = format!("Road planning error: {e}");
                self.status_is_error = true;
                Err(e)
            }
        }
    }

    pub fn run_wall_planning(&mut self, vertex_size: usize) -> Result<(), String> {
        self.plan_wall(&LandscapeContext::plain(vertex_size, None))
    }

    pub fn plan_wall(&mut self, cx: &LandscapeContext) -> Result<(), String> {
        let points = Self::parse_waypoints(&self.wall_waypoints_text);
        if points.len() < 2 {
            return Err("At least 2 waypoints are required for wall planning.".into());
        }

        let catalog = WallTileCatalog::default();
        let options = WallStrokeOptions {
            style: self.wall_style,
            auto_corner_towers: self.wall_place_towers,
            auto_gate_on_road_crossing: self.wall_place_gates,
            ..Default::default()
        };

        let (tiles, _) = cx.geometry();
        let plan = plan_wall_stroke(tiles as i32, &points, &options, &catalog, None, None, None);
        let count = |kind| plan.placements.iter().filter(|p| p.kind == kind).count();
        self.status_message = format!(
            "Wall planned: {} placements, {} towers, {} gates, {} footprint tiles{}.",
            plan.placements.len(),
            count(WallComponentKind::Tower),
            count(WallComponentKind::Gate),
            plan.footprint_tiles.len(),
            plan.failure_reason.as_ref().map(|r| format!(" ({r})")).unwrap_or_default()
        );
        self.status_is_error = !plan.succeeded;
        self.last_wall_plan = Some(plan);
        self.dirty = true;
        Ok(())
    }

    /// Parses opposite corner coordinates from `erosion_rectangle_text`, faithful to C# `TerrainRegionDialog`.
    pub fn parse_erosion_rectangle(&self, tile_dimension: usize) -> Result<((i32, i32), (i32, i32)), String> {
        let lines: Vec<&str> = self
            .erosion_rectangle_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();

        if lines.len() != 2 {
            return Err("Rectangle erosion requires exactly 2 coordinate points (two opposite corners).".to_string());
        }

        let mut pts = Vec::with_capacity(2);
        for line in lines {
            let parts: Vec<&str> = line.split(',').map(str::trim).collect();
            if parts.len() != 2 {
                return Err("Use X,Y on each line (e.g. 10,10).".to_string());
            }
            let x = parts[0]
                .parse::<i32>()
                .map_err(|_| format!("Invalid coordinate integer: '{}'", parts[0]))?;
            let y = parts[1]
                .parse::<i32>()
                .map_err(|_| format!("Invalid coordinate integer: '{}'", parts[1]))?;
            pts.push((x, y));
        }

        let dim = tile_dimension as i32;
        for &(x, y) in &pts {
            if x < 0 || y < 0 || x >= dim || y >= dim {
                return Err(format!(
                    "Coordinate ({x}, {y}) out of bounds (allowed range: 0..{}).",
                    dim.saturating_sub(1)
                ));
            }
        }

        Ok((pts[0], pts[1]))
    }

    pub fn sync_erosion_text_from_coords(&mut self) {
        self.erosion_rectangle_text = format!(
            "{},{}\n{},{}",
            self.erosion_start_x, self.erosion_start_y, self.erosion_end_x, self.erosion_end_y
        );
    }

    pub fn sync_coords_from_erosion_text(&mut self) {
        let pts = Self::parse_waypoints(&self.erosion_rectangle_text);
        if pts.len() >= 2 {
            self.erosion_start_x = pts[0].0;
            self.erosion_start_y = pts[0].1;
            self.erosion_end_x = pts[1].0;
            self.erosion_end_y = pts[1].1;
        }
    }

    /// Convenience wrapper for testing and headless calls.
    pub fn run_erosion_planning(&mut self, vertex_size: usize, heights: Option<&[u8]>) -> Result<ErosionPlan, String> {
        self.plan_erosion(&LandscapeContext::plain(vertex_size, heights))
    }

    /// Plans hydraulic and thermal erosion in the selected tile rectangle, faithful to C# `MapEditorForm.Erosion.cs`.
    pub fn plan_erosion(&mut self, cx: &LandscapeContext) -> Result<ErosionPlan, String> {
        let Some(heights) = cx.heights else {
            let msg = "Terrain heights are not available.".to_string();
            self.status_message = format!("Erosion planning error: {msg}");
            self.status_is_error = true;
            return Err(msg);
        };
        if heights.len() != cx.vertex_size * cx.vertex_size {
            let msg = format!(
                "Terrain height buffer length ({}) does not match vertex size {}x{}.",
                heights.len(),
                cx.vertex_size,
                cx.vertex_size
            );
            self.status_message = format!("Erosion planning error: {msg}");
            self.status_is_error = true;
            return Err(msg);
        }

        let (tiles, step) = cx.geometry();
        let (p1, p2) = match self.parse_erosion_rectangle(tiles) {
            Ok(pair) => pair,
            Err(e) => {
                self.status_message = format!("Erosion planning error: {e}");
                self.status_is_error = true;
                return Err(e);
            }
        };

        if !self.erosion_enable_hydraulic && !self.erosion_enable_thermal {
            let msg = "Both hydraulic and thermal erosion are disabled; nothing to simulate.".to_string();
            self.status_message = format!("Erosion planning error: {msg}");
            self.status_is_error = true;
            return Err(msg);
        }

        let min_x = p1.0.min(p2.0);
        let max_x = p1.0.max(p2.0);
        let min_y = p1.1.min(p2.1);
        let max_y = p1.1.max(p2.1);

        // Faithful to C# MapEditorForm.Erosion.cs:
        // int step = Math.Max(1, (layers.VertexSize - 1) / _texturesDocument.Dimension);
        // int minV = tiles.Left * step, maxV = tiles.Right * step, minZ = tiles.Top * step, maxZ = tiles.Bottom * step;
        // float cx = (minV + maxV) / 2f, cz = (minZ + maxZ) / 2f, radius = MathF.Max(maxV - minV, maxZ - minZ) / 2f + 1f;
        let step_i = step.max(1) as i32;
        let min_v = min_x * step_i;
        let max_v = max_x * step_i;
        let min_z = min_y * step_i;
        let max_z = max_y * step_i;

        let center_x = (min_v + max_v) as f32 / 2.0;
        let center_z = (min_z + max_z) as f32 / 2.0;
        let radius = ((max_v - min_v).max(max_z - min_z) as f32) / 2.0 + 1.0;
        let brush = BrushRegion::new(center_x, center_z, radius);

        let mut buffer = TerrainHeightWorkBuffer::from_bytes(cx.vertex_size, heights)
            .map_err(|e| {
                self.status_message = format!("Erosion buffer error: {e}");
                self.status_is_error = true;
                e
            })?;

        if self.erosion_enable_hydraulic {
            simulate_hydraulic_erosion(&mut buffer, Some(&self.erosion_hydraulic_params), Some(brush));
        }

        if self.erosion_enable_thermal {
            simulate_thermal_erosion(&mut buffer, Some(&self.erosion_thermal_params), Some(brush), None);
        }

        let size = cx.vertex_size;
        let inside: Vec<TerrainSampleChange> = buffer
            .extract_changes(heights)
            .into_iter()
            .filter(|change| {
                let x = (change.index % size) as i32;
                let z = (change.index / size) as i32;
                x >= min_v && x <= max_v && z >= min_z && z <= max_z
            })
            .collect();

        if inside.is_empty() {
            self.status_message = "Erosion made no visible change.".to_string();
            self.status_is_error = false;
            let plan = ErosionPlan {
                tile_bounds: (min_x, min_y, max_x, max_y),
                vertex_bounds: (min_v, min_z, max_v, max_z),
                brush,
                changes: Vec::new(),
                min_delta: 0,
                max_delta: 0,
                hydraulic_params: self.erosion_hydraulic_params,
                thermal_params: self.erosion_thermal_params,
                hydraulic_enabled: self.erosion_enable_hydraulic,
                thermal_enabled: self.erosion_enable_thermal,
            };
            self.last_erosion_plan = Some(plan.clone());
            self.dirty = true;
            return Ok(plan);
        }

        let mut min_delta = i16::MAX;
        let mut max_delta = i16::MIN;
        for c in &inside {
            let delta = c.after as i16 - c.before as i16;
            if delta < min_delta {
                min_delta = delta;
            }
            if delta > max_delta {
                max_delta = delta;
            }
        }

        self.status_message = format!("Erosion planned: {} height samples.", inside.len());
        self.status_is_error = false;

        let plan = ErosionPlan {
            tile_bounds: (min_x, min_y, max_x, max_y),
            vertex_bounds: (min_v, min_z, max_v, max_z),
            brush,
            changes: inside,
            min_delta,
            max_delta,
            hydraulic_params: self.erosion_hydraulic_params,
            thermal_params: self.erosion_thermal_params,
            hydraulic_enabled: self.erosion_enable_hydraulic,
            thermal_enabled: self.erosion_enable_thermal,
        };

        self.last_erosion_plan = Some(plan.clone());
        self.dirty = true;
        Ok(plan)
    }

    /// Request an apply of the currently planned erosion.
    ///
    /// NOTE on Architecture / Contract:
    /// `app.rs` handles `LandscapeApply` which currently covers Cliff, River, Road, Wall.
    /// To preserve other agents' concurrent edits without modifying `app.rs` or `lib.rs`,
    /// `LandscapeUi` provides this explicit Apply Contract for erosion:
    /// - `take_erosion_apply_request(&mut self) -> Option<ErosionPlan>`
    /// - `apply_erosion_to_slice(&mut self, heights: &mut [u8]) -> Result<usize, String>`
    /// - `erosion_applied(&mut self, summary: &str)`
    pub fn request_erosion_apply(&mut self) {
        if let Some(plan) = &self.last_erosion_plan {
            if !plan.changes.is_empty() {
                self.erosion_apply_requested = true;
                self.status_message = format!(
                    "Erosion apply requested: {} height samples ready.",
                    plan.changes.len()
                );
                self.status_is_error = false;
            } else {
                self.status_message = "Erosion made no visible change; nothing to apply.".to_string();
                self.status_is_error = false;
            }
        } else {
            self.status_message = "No erosion plan available to apply.".to_string();
            self.status_is_error = true;
        }
    }

    pub fn take_erosion_apply_request(&mut self) -> Option<ErosionPlan> {
        if self.erosion_apply_requested {
            self.erosion_apply_requested = false;
            self.last_erosion_plan.clone()
        } else {
            None
        }
    }

    pub fn erosion_applied(&mut self, summary: &str) {
        self.last_erosion_plan = None;
        self.erosion_apply_requested = false;
        self.dirty = false;
        self.status_message = format!("Erosion applied: {summary}");
        self.status_is_error = false;
    }

    /// Directly applies the current erosion plan changes to a mutable height slice.
    pub fn apply_erosion_to_slice(&mut self, heights: &mut [u8]) -> Result<usize, String> {
        let Some(plan) = self.last_erosion_plan.take() else {
            return Err("No erosion plan available to apply.".to_string());
        };
        let count = plan.apply_to_heights(heights);
        self.erosion_apply_requested = false;
        self.dirty = false;
        self.status_message = format!("Erosion applied: {count} height samples.");
        self.status_is_error = false;
        Ok(count)
    }

    pub fn take_apply_request(&mut self) -> Option<LandscapeApply> {
        self.apply_requested.take()
    }

    /// The plan was written into the edit sessions, whose own dirty/undo state now tracks it.
    pub fn plan_applied(&mut self, kind: LandscapeApply, summary: &str) {
        match kind {
            LandscapeApply::Cliff => self.last_cliff_result = None,
            LandscapeApply::River => self.last_river_plan = None,
            LandscapeApply::Road => self.last_road_plan = None,
            LandscapeApply::Wall => self.last_wall_plan = None,
        }
        self.dirty = false;
        self.status_message = format!("Applied: {summary}");
        self.status_is_error = false;
    }

    /// A rejected apply keeps the plan so it can be adjusted and retried.
    pub fn apply_failed(&mut self, message: impl Into<String>) {
        self.status_message = format!("Apply rejected: {}", message.into());
        self.status_is_error = true;
    }

    fn apply_button(&mut self, ui: &mut egui::Ui, kind: LandscapeApply, enabled: bool) {
        if ui
            .add_enabled(enabled, egui::Button::new(tl(self.english, "套用至地圖", "Apply to map")))
            .on_hover_text(tl(
                self.english,
                "將規劃寫入地形／材質／物件工作階段（可復原編輯）。",
                "Writes the plan into the terrain / material / object sessions as undoable edits.",
            ))
            .clicked()
        {
            self.apply_requested = Some(kind);
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, cx: &LandscapeContext, mut ambience: Option<&mut AmbienceUi>) {
        if !self.open {
            return;
        }

        let mut is_open = self.open;
        egui::Window::new(tl(self.english, "地貌與防禦工事工作室", "Landscape & Fortification Studio"))
            .open(&mut is_open)
            .min_size(Vec2::new(720.0, 520.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.current_tab, LandscapeTab::CliffPlanner, tl(self.english, "懸崖邊緣與崖壁", "Cliff Edge & Face"));
                    ui.selectable_value(&mut self.current_tab, LandscapeTab::RiverPlanner, tl(self.english, "河流流向", "River Flow"));
                    ui.selectable_value(&mut self.current_tab, LandscapeTab::RoadPlanner, tl(self.english, "道路折線", "Road Polyline"));
                    ui.selectable_value(&mut self.current_tab, LandscapeTab::WallPlanner, tl(self.english, "城牆 / 柵欄", "Wall / Palisade"));
                    ui.selectable_value(&mut self.current_tab, LandscapeTab::ErosionPlanner, tl(self.english, "侵蝕（水力與熱力）", "Erosion (Hydraulic & Thermal)"));
                    ui.selectable_value(&mut self.current_tab, LandscapeTab::RegionTools, tl(self.english, "區域工具", "Region Tools"));
                });
                ui.separator();

                if let Some(ambience) = ambience.as_mut() {
                    self.sync_water_level(ambience.profile.boden.waterlevel);
                    ui.horizontal(|ui| {
                        ui.label(tl(self.english, "目前水位 / Current water level:", "Current water level:"));
                        ui.label(format!("{:.1}", ambience.profile.boden.waterlevel));
                        ui.separator();
                        ui.label("Water level / 水位:");
                        let resp = ui.add(
                            egui::DragValue::new(&mut ambience.profile.boden.waterlevel).speed(0.5)
                        ).on_hover_text(tl(
                            self.english,
                            "與「氛圍 → 地表」為相同設定（boden.ini 水位）",
                            "Same setting as Ambience → Boden (boden.ini [Waterlevel])",
                        ));
                        if resp.changed() {
                            ambience.dirty = true;
                            ambience.validate_current();
                            self.sync_water_level(ambience.profile.boden.waterlevel);
                        }
                    });
                    ui.separator();
                } else if let Some(wl) = self.cached_water_level {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}: {:.1}", tl(self.english, "目前水位", "Current water level"), wl));
                    });
                    ui.separator();
                }

                if !self.status_message.is_empty() {
                    let color = if self.status_is_error {
                        Color32::from_rgb(250, 100, 100)
                    } else {
                        Color32::from_rgb(120, 220, 120)
                    };
                    ui.colored_label(color, &self.status_message);
                    ui.separator();
                }

                match self.current_tab {
                    LandscapeTab::CliffPlanner => self.render_cliff_tab(ui, cx),
                    LandscapeTab::RiverPlanner => self.render_river_tab(ui, cx),
                    LandscapeTab::RoadPlanner => self.render_road_tab(ui, cx),
                    LandscapeTab::WallPlanner => self.render_wall_tab(ui, cx),
                    LandscapeTab::ErosionPlanner => self.render_erosion_tab(ui, cx),
                    LandscapeTab::RegionTools => self.render_region_tab(ui, cx),
                }
            });

        if !is_open {
            self.request_close();
        }

        if self.pending_discard {
            let mut close_modal = false;
            let mut confirm = false;

            egui::Window::new(tl(self.english, "捨棄未儲存的地貌規劃？", "Discard Unsaved Landscape Plans?"))
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(tl(
                        self.english,
                        "關閉將捨棄目前規劃中的懸崖、河流、道路、城牆及侵蝕筆畫。",
                        "Closing will discard currently planned cliff, river, road, wall, and erosion strokes.",
                    ));
                    ui.horizontal(|ui| {
                        if ui.button(tl(self.english, "捨棄並關閉", "Discard & Close")).clicked() {
                            confirm = true;
                        }
                        if ui.button(tl(self.english, "繼續編輯", "Keep Editing")).clicked() {
                            close_modal = true;
                        }
                    });
                });

            if confirm {
                self.clear_dirty();
                self.open = false;
            } else if close_modal {
                self.pending_discard = false;
            }
        }
    }

    pub fn show_without_ambience(&mut self, ctx: &egui::Context, cx: &LandscapeContext) {
        self.show(ctx, cx, None);
    }

    fn render_cliff_tab(&mut self, ui: &mut egui::Ui, cx: &LandscapeContext) {
        ui.heading(tl(self.english, "懸崖邊緣偵測與崖壁規劃", "Cliff Edge Detection & Face Planning"));
        ui.label(tl(self.english, "偵測陡峭的高程坡度並擷取連續的懸崖等高線路徑。", "Detects steep elevation gradients and extracts continuous cliff contour lines."));

        ui.add_space(6.0);
        egui::Grid::new("cliff_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(tl(self.english, "最小坡度角度：", "Min Slope Degrees:"));
            ui.add(egui::Slider::new(&mut self.cliff_min_slope, 15.0..=75.0).text("°"));
            ui.end_row();

            ui.label(tl(self.english, "最小高程落差：", "Min Elevation Delta:"));
            ui.add(egui::Slider::new(&mut self.cliff_min_delta, 5.0..=64.0));
            ui.end_row();

            ui.label(tl(self.english, "崖腳混合碎石：", "Blend scree at cliff foot:"));
            ui.checkbox(&mut self.cliff_generate_scree, tl(self.english, "需要原版過渡圖塊", "needs the original transition tiles"));
            ui.end_row();
        });

        ui.add_space(8.0);
        if ui.button(tl(self.english, "在高度圖上偵測懸崖", "Detect Cliffs on Heightmap")).clicked() {
            let _ = self.plan_cliffs(cx);
        }

        if let Some((cell_count, contour_count, paths)) = self.last_cliff_result.as_ref().map(|res| {
            let paths: Vec<_> = res.contours.iter().enumerate().map(|(i, c)| (i + 1, c.cells.len(), c.is_closed_loop)).collect();
            (res.cliff_cells.len(), res.contours.len(), paths)
        }) {
            ui.separator();
            ui.label(format!("{}: {}", tl(self.english, "偵測到懸崖網格數", "Detected Cliff Cells"), cell_count));
            ui.label(format!("{}: {}", tl(self.english, "等高線路徑數", "Contour Paths"), contour_count));
            let ready = cell_count > 0 && cx.textures.is_some();
            self.apply_button(ui, LandscapeApply::Cliff, ready);
            if cx.textures.is_none() {
                ui.label(tl(self.english, "在材質面板中載入地圖以套用懸崖材質。", "Load the map in the Materials panel to apply cliff textures."));
            }

            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for (idx, len, closed) in paths {
                    ui.label(format!(
                        "{} #{}: {} {} ({}: {})",
                        tl(self.english, "路徑", "Path"),
                        idx,
                        len,
                        tl(self.english, "個網格", "cells"),
                        tl(self.english, "閉合", "Closed"),
                        if closed { tl(self.english, "是", "true") } else { tl(self.english, "否", "false") }
                    ));
                }
            });
        }
    }

    fn render_river_tab(&mut self, ui: &mut egui::Ui, cx: &LandscapeContext) {
        ui.heading(tl(self.english, "河流流向規劃器", "River Flow Planner"));
        ui.label(tl(self.english, "循自然順坡走向追蹤水流路徑，配置無縫水面與河岸材質。", "Traces natural downhill water courses with seamless water and riverbank textures."));

        ui.add_space(6.0);
        egui::Grid::new("river_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(tl(self.english, "起點座標 (X, Y)：", "Start Coordinate (X, Y):"));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.river_start_x));
                ui.add(egui::DragValue::new(&mut self.river_start_y));
            });
            ui.end_row();

            ui.label(tl(self.english, "終點座標 (X, Y)：", "End Coordinate (X, Y):"));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.river_end_x));
                ui.add(egui::DragValue::new(&mut self.river_end_y));
            });
            ui.end_row();

            ui.label(tl(self.english, "水面材質：", "Water Texture:"));
            ui.text_edit_singleline(&mut self.river_preferred_texture);
            ui.end_row();

            ui.label(tl(self.english, "河床高程模式：", "Riverbed:"));
            let mode_label = match (self.river_mode, self.english) {
                (RiverElevationMode::ValidateOnly, false) => "僅驗證（不修改高程）",
                (RiverElevationMode::ValidateOnly, true) => "ValidateOnly (no height edits)",
                (RiverElevationMode::AutoCarveDescending, false) => "自動順坡向下雕刻",
                (RiverElevationMode::AutoCarveDescending, true) => "AutoCarveDescending",
                (RiverElevationMode::AutoCarveTrench, false) => "自動雕刻凹槽河床",
                (RiverElevationMode::AutoCarveTrench, true) => "AutoCarveTrench",
            };
            egui::ComboBox::from_id_source("river_mode_select")
                .selected_text(mode_label)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.river_mode, RiverElevationMode::ValidateOnly, tl(self.english, "僅驗證（不修改高程）", "ValidateOnly (no height edits)"));
                    ui.selectable_value(&mut self.river_mode, RiverElevationMode::AutoCarveDescending, tl(self.english, "自動順坡向下雕刻", "AutoCarveDescending"));
                    ui.selectable_value(&mut self.river_mode, RiverElevationMode::AutoCarveTrench, tl(self.english, "自動雕刻凹槽河床", "AutoCarveTrench"));
                });
            ui.end_row();

            ui.label(tl(self.english, "水位：", "Water Level:"));
            if let Some(wl) = self.cached_water_level {
                ui.label(format!("{:.1}", wl));
            } else {
                ui.label(tl(self.english, "未指定", "Unspecified"));
            }
            ui.end_row();
        });

        ui.add_space(8.0);
        if ui.button(tl(self.english, "規劃水流路徑", "Plan River Stroke")).clicked() {
            let _ = self.plan_river(cx);
        }

        if let Some((water_count, bank_count, height_count, unsupported_count, succeeded)) = self.last_river_plan.as_ref().map(|plan| {
            (plan.water_tiles.len(), plan.bank_tiles.len(), plan.height_adjustments.len(), plan.unsupported.len(), plan.succeeded)
        }) {
            ui.separator();
            ui.label(format!("{}: {}", tl(self.english, "已規劃水面圖塊數", "Water Tiles Planned"), water_count));
            ui.label(format!("{}: {}", tl(self.english, "河岸過渡圖塊數", "Bank Transition Tiles"), bank_count));
            ui.label(format!("{}: {}", tl(self.english, "河床高度編輯頂點數", "Riverbed height edits"), height_count));
            let ready = succeeded && cx.textures.is_some();
            self.apply_button(ui, LandscapeApply::River, ready);
            if unsupported_count > 0 {
                ui.label(tl(self.english, "路徑超出地圖邊界；請修正座標後再套用。", "The path leaves the map; fix the coordinates before applying."));
                ui.colored_label(Color32::from_rgb(255, 180, 50), format!("{}: {}", tl(self.english, "不支援／越界圖塊數", "Unsupported Tiles"), unsupported_count));
            }
        }
    }

    fn render_road_tab(&mut self, ui: &mut egui::Ui, cx: &LandscapeContext) {
        ui.heading(tl(self.english, "道路與小徑折線規劃器", "Road & Path Polyline Planner"));
        ui.label(tl(self.english, "沿折線繪製連續道路筆畫，自動配置直道、轉角與 T 型交叉口材質。", "Draws continuous road strokes with proper straight, turn, and T-junction textures."));

        ui.add_space(6.0);
        ui.label(tl(self.english, "路徑節點（每行一組 X,Y）：", "Waypoints (One X,Y per line):"));
        ui.text_edit_multiline(&mut self.road_waypoints_text);

        ui.horizontal(|ui| {
            ui.label(tl(self.english, "道路材質：", "Road Texture:"));
            ui.text_edit_singleline(&mut self.road_preferred_texture);
        });

        ui.add_space(8.0);
        if ui.button(tl(self.english, "規劃道路筆畫", "Plan Road Stroke")).clicked() {
            let _ = self.plan_road(cx);
        }

        if let Some((tile_count, succeeded)) = self.last_road_plan.as_ref().map(|plan| (plan.tiles.len(), plan.succeeded)) {
            ui.separator();
            ui.label(format!("{}: {}", tl(self.english, "規劃道路配置圖塊數", "Road Placements Planned"), tile_count));
            let ready = succeeded && cx.textures.is_some();
            self.apply_button(ui, LandscapeApply::Road, ready);
            if !succeeded {
                ui.colored_label(Color32::from_rgb(255, 100, 100), tl(self.english, "道路筆畫遇到超出地圖邊界的圖塊。", "Road stroke encountered out-of-bounds tiles."));
            }
        }
    }

    fn render_wall_tab(&mut self, ui: &mut egui::Ui, cx: &LandscapeContext) {
        ui.heading(tl(self.english, "城牆與木柵規劃器", "Wall & Palisade Planner"));
        ui.label(tl(self.english, "沿路徑節點配置直牆、轉角段、防禦塔樓與城門。", "Places straight walls, corner segments, defensive towers, and gates along waypoints."));

        ui.add_space(6.0);
        egui::Grid::new("wall_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(tl(self.english, "防禦工事風格：", "Fortification Style:"));
            let style_label = match (self.wall_style, self.english) {
                (FortificationStyle::GermanicPalisade, false) => "日耳曼木柵",
                (FortificationStyle::GermanicPalisade, true) => "Germanic Palisade",
                (FortificationStyle::RomanStoneWall, false) => "羅馬石牆",
                (FortificationStyle::RomanStoneWall, true) => "Roman Stone Wall",
                (FortificationStyle::CelticPalisade, false) => "塞爾特木柵",
                (FortificationStyle::CelticPalisade, true) => "Celtic Palisade",
                (FortificationStyle::HunBarricade, false) => "匈人路障",
                (FortificationStyle::HunBarricade, true) => "Hun Barricade",
            };
            egui::ComboBox::from_id_source("wall_style_select")
                .selected_text(style_label)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.wall_style, FortificationStyle::GermanicPalisade, tl(self.english, "日耳曼木柵", "Germanic Palisade"));
                    ui.selectable_value(&mut self.wall_style, FortificationStyle::RomanStoneWall, tl(self.english, "羅馬石牆", "Roman Stone Wall"));
                    ui.selectable_value(&mut self.wall_style, FortificationStyle::CelticPalisade, tl(self.english, "塞爾特木柵", "Celtic Palisade"));
                    ui.selectable_value(&mut self.wall_style, FortificationStyle::HunBarricade, tl(self.english, "匈人路障", "Hun Barricade"));
                });
            ui.end_row();

            ui.label(tl(self.english, "轉角／間距放置防禦塔：", "Towers on Corners/Intervals:"));
            ui.checkbox(&mut self.wall_place_towers, "");
            ui.end_row();

            ui.label(tl(self.english, "道路交叉處放置城門：", "Gates on Road Intersections:"));
            ui.checkbox(&mut self.wall_place_gates, "");
            ui.end_row();
        });

        ui.label(tl(self.english, "路徑節點（每行一組 X,Y）：", "Waypoints (One X,Y per line):"));
        ui.text_edit_multiline(&mut self.wall_waypoints_text);
        ui.label(tl(self.english, "物件範本（每行 id,name；地圖需有各類型既有物件）：", "Object templates (id,name per line; the map needs an existing object of each type):"));
        ui.add(egui::TextEdit::multiline(&mut self.wall_templates_text).desired_rows(2));

        ui.add_space(8.0);
        if ui.button(tl(self.english, "規劃城牆筆畫", "Plan Wall Stroke")).clicked() {
            let _ = self.plan_wall(cx);
        }

        if let Some((placement_count, tower_count, gate_count, footprint_count, succeeded)) = self.last_wall_plan.as_ref().map(|plan| {
            let count = |kind| plan.placements.iter().filter(|p| p.kind == kind).count();
            (plan.placements.len(), count(WallComponentKind::Tower), count(WallComponentKind::Gate), plan.footprint_tiles.len(), plan.succeeded)
        }) {
            ui.separator();
            ui.label(format!("{}: {}", tl(self.english, "規劃配置物件數", "Placements Planned"), placement_count));
            ui.label(format!("{}: {}, {}: {}", tl(self.english, "防禦塔", "Towers"), tower_count, tl(self.english, "城門", "Gates"), gate_count));
            ui.label(format!("{}: {}", tl(self.english, "阻擋佔地圖塊數", "Blocked Footprint Tiles"), footprint_count));
            let ready = succeeded;
            self.apply_button(ui, LandscapeApply::Wall, ready);
        }
    }

    fn reset_enabled_erosion_parameters(&mut self) {
        if self.erosion_enable_hydraulic {
            self.erosion_hydraulic_params = HydraulicErosionParams { droplet_count: 8000, ..Default::default() };
        }
        if self.erosion_enable_thermal {
            self.erosion_thermal_params = ThermalErosionParams::default();
        }
    }

    fn render_erosion_tab(&mut self, ui: &mut egui::Ui, cx: &LandscapeContext) {
        let (dim, _) = cx.geometry();
        ui.heading(tl(self.english, "矩形區域水力與熱力侵蝕", "Hydraulic & Thermal Erosion in Rectangle"));
        ui.label(tl(
            self.english,
            "在選取的矩形圖塊區域內模擬降雨水滴逕流與碎石邊坡熱力分散。",
            "Simulates rainfall droplet runoff and talus slope dispersion inside a selected rectangular tile region.",
        ));

        ui.add_space(6.0);
        ui.label(format!(
            "{} 0–{}。{}（{}）：",
            tl(self.english, "圖塊座標範圍", "Tile coordinates:"),
            dim.saturating_sub(1),
            tl(self.english, "對角頂點", "Opposite corners"),
            tl(self.english, "每行一組 X,Y", "One X,Y per line"),
        ));
        if ui
            .add(egui::TextEdit::multiline(&mut self.erosion_rectangle_text).desired_rows(2))
            .changed()
        {
            self.sync_coords_from_erosion_text();
        }

        ui.horizontal(|ui| {
            ui.label(tl(self.english, "頂點 1 (X, Y)：", "Corner 1 (X, Y):"));
            let c1 = ui.add(egui::DragValue::new(&mut self.erosion_start_x)).changed()
                | ui.add(egui::DragValue::new(&mut self.erosion_start_y)).changed();
            ui.label(tl(self.english, "頂點 2 (X, Y)：", "Corner 2 (X, Y):"));
            let c2 = ui.add(egui::DragValue::new(&mut self.erosion_end_x)).changed()
                | ui.add(egui::DragValue::new(&mut self.erosion_end_y)).changed();
            if c1 || c2 {
                self.sync_erosion_text_from_coords();
            }
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.erosion_enable_hydraulic, tl(self.english, "水力侵蝕", "Hydraulic Erosion"));
            ui.checkbox(&mut self.erosion_enable_thermal, tl(self.english, "熱力侵蝕", "Thermal Erosion"));
        });

        ui.separator();
        egui::CollapsingHeader::new(tl(self.english, "模擬參數", "Simulation Parameters"))
            .default_open(true)
            .show(ui, |ui| {
                if self.erosion_enable_hydraulic {
                    ui.strong(tl(self.english, "水滴水力參數", "Hydraulic Droplet Parameters"));
                    egui::Grid::new("hydraulic_params_grid")
                        .num_columns(2)
                        .spacing([12.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(tl(self.english, "水滴數量：", "Droplet Count:"));
                            ui.add(egui::Slider::new(&mut self.erosion_hydraulic_params.droplet_count, 100..=50000));
                            ui.end_row();

                            ui.label(tl(self.english, "最大生命週期：", "Max Lifetime:"));
                            ui.add(egui::Slider::new(&mut self.erosion_hydraulic_params.max_lifetime, 10..=100));
                            ui.end_row();

                            ui.label(tl(self.english, "慣性：", "Inertia:"));
                            ui.add(egui::Slider::new(&mut self.erosion_hydraulic_params.inertia, 0.0..=0.5));
                            ui.end_row();

                            ui.label(tl(self.english, "重力：", "Gravity:"));
                            ui.add(egui::Slider::new(&mut self.erosion_hydraulic_params.gravity, 1.0..=10.0));
                            ui.end_row();

                            ui.label(tl(self.english, "侵蝕速率：", "Erosion Rate:"));
                            ui.add(egui::Slider::new(&mut self.erosion_hydraulic_params.erosion_rate, 0.01..=1.0));
                            ui.end_row();

                            ui.label(tl(self.english, "堆積速率：", "Deposition Rate:"));
                            ui.add(egui::Slider::new(&mut self.erosion_hydraulic_params.deposition_rate, 0.01..=1.0));
                            ui.end_row();

                            ui.label(tl(self.english, "侵蝕半徑：", "Erosion Radius:"));
                            ui.add(egui::Slider::new(&mut self.erosion_hydraulic_params.erosion_radius, 1..=5));
                            ui.end_row();

                            ui.label(tl(self.english, "隨機種子碼：", "Random Seed:"));
                            ui.add(egui::DragValue::new(&mut self.erosion_hydraulic_params.random_seed));
                            ui.end_row();
                        });
                }

                if self.erosion_enable_thermal {
                    if self.erosion_enable_hydraulic {
                        ui.add_space(4.0);
                    }
                    ui.strong(tl(self.english, "邊坡熱力參數", "Thermal Talus Parameters"));
                    egui::Grid::new("thermal_params_grid")
                        .num_columns(2)
                        .spacing([12.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(tl(self.english, "休止角（邊坡角度）：", "Talus Angle:"));
                            ui.add(egui::Slider::new(&mut self.erosion_thermal_params.talus_angle_degrees, 15.0..=75.0).text("°"));
                            ui.end_row();

                            ui.label(tl(self.english, "侵蝕速率：", "Erosion Rate:"));
                            ui.add(egui::Slider::new(&mut self.erosion_thermal_params.erosion_rate, 0.05..=1.0));
                            ui.end_row();

                            ui.label(tl(self.english, "反覆運算次數：", "Iterations:"));
                            ui.add(egui::Slider::new(&mut self.erosion_thermal_params.iterations, 1..=32));
                            ui.end_row();
                        });
                }

                ui.add_space(4.0);
                if ui.button(tl(self.english, "重設為預設值", "Reset to Defaults")).clicked() {
                    self.reset_enabled_erosion_parameters();
                }
            });

        ui.add_space(8.0);
        if ui.button(tl(self.english, "在選區內模擬侵蝕", "Simulate Erosion in Selection")).clicked() {
            let _ = self.plan_erosion(cx);
        }

        let mut apply_clicked = false;
        if let Some(plan) = &self.last_erosion_plan {
            ui.separator();
            ui.label(format!(
                "{}: ({}, {}) {} ({}, {}) [{}: ({}, {}) {} ({}, {})]",
                tl(self.english, "選取圖塊範圍", "Tile Selection"),
                plan.tile_bounds.0,
                plan.tile_bounds.1,
                tl(self.english, "至", "to"),
                plan.tile_bounds.2,
                plan.tile_bounds.3,
                tl(self.english, "頂點範圍", "Vertices"),
                plan.vertex_bounds.0,
                plan.vertex_bounds.1,
                tl(self.english, "至", "to"),
                plan.vertex_bounds.2,
                plan.vertex_bounds.3,
            ));
            ui.label(format!("{}: {}", tl(self.english, "高度變更取樣點數", "Height Samples Changed"), plan.changes.len()));

            if !plan.changes.is_empty() {
                ui.label(format!(
                    "{}: [{}, {}]",
                    tl(self.english, "高程落差變更範圍", "Elevation Delta Range"),
                    plan.min_delta, plan.max_delta
                ));

                let ready = cx.heights.is_some();
                if ui
                    .add_enabled(ready, egui::Button::new(tl(self.english, "套用至地圖", "Apply to map")))
                    .on_hover_text(tl(self.english, "請求將侵蝕高度變更套用至已載入的地形。", "Requests applying the erosion height changes to the loaded terrain."))
                    .clicked()
                {
                    apply_clicked = true;
                }

                if self.erosion_apply_requested {
                    ui.colored_label(
                        Color32::from_rgb(100, 200, 255),
                        tl(self.english, "已請求套用：套用合約已就緒。", "Apply requested: ready in apply contract. (app.rs integration pending)"),
                    );
                }

                egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                    let preview_count = plan.changes.len().min(50);
                    for c in &plan.changes[..preview_count] {
                        let delta = c.after as i16 - c.before as i16;
                        ui.label(format!(
                            "{} #{}: {}={}, {}={} ({}={:+})",
                            tl(self.english, "頂點", "Vertex"),
                            c.index,
                            tl(self.english, "變更前", "before"),
                            c.before,
                            tl(self.english, "變更後", "after"),
                            c.after,
                            tl(self.english, "落差", "delta"),
                            delta
                        ));
                    }
                    if plan.changes.len() > 50 {
                        ui.label(format!(
                            "{} {} {}",
                            tl(self.english, "... 以及另外", "... and"),
                            plan.changes.len() - 50,
                            tl(self.english, "個頂點", "more vertices")
                        ));
                    }
                });
            } else {
                ui.label(tl(self.english, "使用目前參數在此地形上侵蝕未產生高度變化。", "Erosion produced no height changes on this terrain with current parameters."));
            }
        }

        if apply_clicked {
            self.request_erosion_apply();
        }
    }
}

// ================================================================================================
// Region tools (C# `TerrainRegionDialog` + `MapEditorForm.ApplyRegionTool`)
//
// Rectangle fill, connected-material fill, dirt-road polyline fill and "select objects in
// rectangle", plus the drag-a-rectangle hooks the 2D canvas feeds. River / cliff / wall / erosion
// of the C# dialog already have their own planner tabs in this window; the erosion tab is kept in
// sync with the dragged rectangle.
// ================================================================================================

/// Operations of the C# region dialog that act on a tile selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RegionOperation {
    #[default]
    RectangleFill,
    ConnectedFill,
    RoadPolyline,
    SelectObjects,
}

impl RegionOperation {
    pub const ALL: [RegionOperation; 4] = [
        RegionOperation::RectangleFill,
        RegionOperation::ConnectedFill,
        RegionOperation::RoadPolyline,
        RegionOperation::SelectObjects,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::RectangleFill => "Rectangle fill (矩形填色)",
            Self::ConnectedFill => "Connected material fill (同材質連通區填色)",
            Self::RoadPolyline => "Dirt road polyline (土路折線)",
            Self::SelectObjects => "Select objects in rectangle (矩形選取物件)",
        }
    }
    /// Minimum vertex count, as in C# (`Connected` needs one seed, everything else two).
    pub fn min_vertices(self) -> usize {
        if self == Self::ConnectedFill {
            1
        } else {
            2
        }
    }
    /// Only polylines accept more vertices than the minimum.
    pub fn is_polyline(self) -> bool {
        self == Self::RoadPolyline
    }
}

/// World units per tile (matches C# `WorldX / 256f`).
pub const REGION_WORLD_UNITS_PER_TILE: f32 = 256.0;

/// Inclusive tile rectangle (`max` is the last selected tile, like the dialog's second corner).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileRect {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

impl TileRect {
    pub fn from_corners(a: (i32, i32), b: (i32, i32)) -> Self {
        Self { min_x: a.0.min(b.0), min_y: a.1.min(b.1), max_x: a.0.max(b.0), max_y: a.1.max(b.1) }
    }
    /// Clamps to `0..dimension` tiles; `None` when the dimension is zero.
    pub fn clamped(self, dimension: usize) -> Option<Self> {
        let hi = dimension as i32 - 1;
        (hi >= 0).then(|| Self {
            min_x: self.min_x.clamp(0, hi),
            min_y: self.min_y.clamp(0, hi),
            max_x: self.max_x.clamp(0, hi),
            max_y: self.max_y.clamp(0, hi),
        })
    }
    pub fn tile_count(&self) -> usize {
        ((self.max_x - self.min_x + 1).max(0) * (self.max_y - self.min_y + 1).max(0)) as usize
    }
    /// C# `SelectObjects` test: tile coordinate = world / 256, half-open `[min, max + 1)`.
    pub fn contains_world(&self, world_x: f32, world_z: f32) -> bool {
        let x = world_x / REGION_WORLD_UNITS_PER_TILE;
        let y = world_z / REGION_WORLD_UNITS_PER_TILE;
        x >= self.min_x as f32 && x < (self.max_x + 1) as f32 && y >= self.min_y as f32 && y < (self.max_y + 1) as f32
    }
    pub fn to_text(&self) -> String {
        format!("{},{}\n{},{}", self.min_x, self.min_y, self.max_x, self.max_y)
    }
}

/// Parses the dialog's `X,Y`-per-line text and bounds-checks against `dimension` tiles.
pub fn parse_region_vertices(text: &str, dimension: usize) -> Result<Vec<(usize, usize)>, String> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let parts: Vec<&str> = line.split(',').map(str::trim).collect();
        if parts.len() != 2 {
            return Err("Use X,Y on each line.".into());
        }
        let x: i64 = parts[0].parse().map_err(|_| format!("Invalid coordinate integer: '{}'", parts[0]))?;
        let y: i64 = parts[1].parse().map_err(|_| format!("Invalid coordinate integer: '{}'", parts[1]))?;
        if x < 0 || y < 0 || x >= dimension as i64 || y >= dimension as i64 {
            return Err(format!(
                "Coordinate ({x}, {y}) out of bounds (allowed range: 0..{}).",
                dimension.saturating_sub(1)
            ));
        }
        out.push((x as usize, y as usize));
    }
    Ok(out)
}

/// Tile cells an operation covers, via the native `TerrainRegionPlanner`.
///
/// `textures` (row-major tile texture names) is only needed by `ConnectedFill`.
/// `SelectObjects` returns the rectangle cells too so the caller can show a count.
pub fn plan_region_cells(
    op: RegionOperation,
    dimension: usize,
    vertices: &[(usize, usize)],
    road_width: usize,
    textures: Option<&[String]>,
) -> Result<Vec<(usize, usize)>, String> {
    let need = op.min_vertices();
    if vertices.len() < need || (!op.is_polyline() && vertices.len() != need) {
        return Err("Invalid vertex count.".into());
    }
    match op {
        RegionOperation::RectangleFill | RegionOperation::SelectObjects => {
            TerrainRegionPlanner::rectangle(dimension, vertices[0], vertices[1]).map_err(|e| e.to_string())
        }
        RegionOperation::RoadPolyline => {
            TerrainRegionPlanner::polyline(dimension, vertices, road_width.max(1)).map_err(|e| e.to_string())
        }
        RegionOperation::ConnectedFill => {
            let textures = textures
                .filter(|t| t.len() == dimension * dimension)
                .ok_or("Load the map in the Materials panel to use the connected fill.")?;
            TerrainRegionPlanner::connected(dimension, textures, vertices[0]).map_err(|e| e.to_string())
        }
    }
}

/// Ids of objects whose `(world_x, world_z)` falls inside the rectangle.
pub fn objects_in_rect(rect: &TileRect, objects: &[(usize, f32, f32)]) -> Vec<usize> {
    objects.iter().filter(|(_, x, z)| rect.contains_world(*x, *z)).map(|(id, _, _)| *id).collect()
}

/// Planned region, cached until applied or discarded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionPlan {
    pub op: RegionOperation,
    pub cells: Vec<(usize, usize)>,
    pub rect: TileRect,
}

/// A texture fill the host applies to the material session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionPaintRequest {
    pub cells: Vec<(usize, usize)>,
    pub material: String,
    pub auto_bridge: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegionRequest {
    Paint(RegionPaintRequest),
    SelectObjects(TileRect),
}

/// Result of [`LandscapeUi::apply_pending_region`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegionOutcome {
    /// Number of tiles whose texture changed (host should mark textures dirty / redraw when > 0).
    pub textures_changed: usize,
    /// Object ids to select in the placement list, when the request was `SelectObjects`.
    pub selected_objects: Option<Vec<usize>>,
}

/// Applies a fill as one undoable stroke on a fork; the real session is replaced only when every
/// tile bakes (C#: "Unsupported native transition ...; region unchanged").
pub fn apply_region_paint(
    session: &mut TerrainBlendEditSession,
    request: &RegionPaintRequest,
) -> Result<usize, String> {
    let mut fork = session.fork();
    let result = fork
        .paint_tiles(&request.cells, &request.material, request.auto_bridge)
        .map_err(|e| e.to_string())?;
    if !result.succeeded {
        return Err(match result.issues.first() {
            Some(i) => format!("Unsupported native transition at {},{}; region unchanged.", i.tile_x, i.tile_y),
            None => "Region could not be baked; region unchanged.".into(),
        });
    }
    let changed = result.texture_changes.len();
    if changed > 0 {
        fork.commit_stroke();
        *session = fork;
    }
    Ok(changed)
}

/// Drag-a-rectangle state for the 2D canvas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegionDrag {
    start: Option<(i32, i32)>,
    current: Option<(i32, i32)>,
}

impl RegionDrag {
    pub fn begin(&mut self, tile: (i32, i32)) {
        self.start = Some(tile);
        self.current = Some(tile);
    }
    pub fn update(&mut self, tile: (i32, i32)) {
        if self.start.is_some() {
            self.current = Some(tile);
        }
    }
    /// Finishes the drag and returns the rectangle.
    pub fn finish(&mut self) -> Option<TileRect> {
        let r = self.rect();
        self.cancel();
        r
    }
    pub fn cancel(&mut self) {
        self.start = None;
        self.current = None;
    }
    pub fn is_active(&self) -> bool {
        self.start.is_some()
    }
    pub fn rect(&self) -> Option<TileRect> {
        Some(TileRect::from_corners(self.start?, self.current?))
    }
}

/// All state of the region tab.
#[derive(Clone, Debug)]
pub struct RegionToolState {
    pub op: RegionOperation,
    /// One `X,Y` per line, as in the C# dialog.
    pub vertices_text: String,
    pub road_width: usize,
    /// Material id painted by fill / road operations.
    pub material: String,
    pub auto_bridge: bool,
    /// While armed, the host should route 2D canvas drags to `region_drag_input`.
    pub pick_armed: bool,
    pub drag: RegionDrag,
    /// Last rectangle produced by a canvas drag.
    pub selection: Option<TileRect>,
    pub plan: Option<RegionPlan>,
    pub request: Option<RegionRequest>,
}

impl Default for RegionToolState {
    fn default() -> Self {
        Self {
            op: RegionOperation::default(),
            vertices_text: "10,10\n20,20".to_string(),
            road_width: 3,
            material: String::new(),
            auto_bridge: true,
            pick_armed: false,
            drag: RegionDrag::default(),
            selection: None,
            plan: None,
            request: None,
        }
    }
}

impl RegionToolState {
    /// Clears map-bound state; keeps the user's preferences (operation, material, width).
    pub fn reset(&mut self) {
        self.drag.cancel();
        self.selection = None;
        self.plan = None;
        self.request = None;
    }
}

/// Pointer events from the 2D or 3D viewport for drag-selecting a tile region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LandscapeRegionPointer {
    Press((i32, i32)),
    Drag((i32, i32)),
    Release(Option<(i32, i32)>),
    Cancel,
}

impl LandscapeUi {
    /// True when the host should forward 2D canvas drags to [`Self::region_drag_input`].
    pub fn region_pick_active(&self) -> bool {
        self.open && self.current_tab == LandscapeTab::RegionTools && self.region.pick_armed
    }

    /// Feeds one canvas interaction. `tile_at` maps a screen position to a tile coordinate (it may
    /// return coordinates outside the map; they are clamped when the drag finishes). Returns the
    /// finished rectangle on release.
    pub fn region_drag_input(
        &mut self,
        response: &egui::Response,
        dimension: usize,
        tile_at: impl Fn(egui::Pos2) -> Option<(i32, i32)>,
    ) -> Option<TileRect> {
        if !self.region_pick_active() {
            return None;
        }
        let pos = response.interact_pointer_pos().or_else(|| response.hover_pos());
        let tile = pos.and_then(&tile_at);
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(t) = tile {
                self.region.drag.begin(t);
            }
        } else if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(t) = tile {
                self.region.drag.update(t);
            }
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            if let Some(t) = tile {
                self.region.drag.update(t);
            }
            let rect = self.region.drag.finish().and_then(|r| r.clamped(dimension))?;
            self.set_region_selection(rect);
            return Some(rect);
        }
        None
    }

    /// Cancels an in-progress drag (e.g. Escape).
    pub fn region_drag_cancel(&mut self) {
        self.region.drag.cancel();
    }

    /// Feeds one landscape region pointer event (e.g. from the 3D viewport).
    ///
    /// When a rectangle drag is completed, clamps the selection to `dimension` and updates
    /// the region selection (and erosion tab). Returns `Some(rect)` on completion.
    pub fn handle_region_pointer(
        &mut self,
        event: LandscapeRegionPointer,
        dimension: usize,
    ) -> Option<TileRect> {
        if !self.region_pick_active() {
            return None;
        }
        match event {
            LandscapeRegionPointer::Press(tile) => {
                self.region.drag.begin(tile);
                None
            }
            LandscapeRegionPointer::Drag(tile) => {
                self.region.drag.update(tile);
                None
            }
            LandscapeRegionPointer::Release(tile_opt) => {
                if let Some(tile) = tile_opt {
                    self.region.drag.update(tile);
                }
                let rect = self.region.drag.finish().and_then(|r| r.clamped(dimension))?;
                self.set_region_selection(rect);
                Some(rect)
            }
            LandscapeRegionPointer::Cancel => {
                self.region_drag_cancel();
                None
            }
        }
    }

    /// Stores a canvas selection into the dialog text and the erosion tab (C# `RunRegionTool`
    /// prefills the dialog from `_canvas.SelectionTiles`).
    pub fn set_region_selection(&mut self, rect: TileRect) {
        self.region.selection = Some(rect);
        self.region.vertices_text = rect.to_text();
        self.region.plan = None;
        self.erosion_start_x = rect.min_x;
        self.erosion_start_y = rect.min_y;
        self.erosion_end_x = rect.max_x;
        self.erosion_end_y = rect.max_y;
        self.sync_erosion_text_from_coords();
    }

    /// Draws the live drag rectangle and the last selection. `tile_to_screen` maps a tile *corner*
    /// coordinate (so `max + 1` for the far edge) to a screen position.
    pub fn paint_region_overlay(&self, painter: &egui::Painter, tile_to_screen: impl Fn(f32, f32) -> egui::Pos2) {
        let draw = |r: TileRect, color: Color32| {
            let a = tile_to_screen(r.min_x as f32, r.min_y as f32);
            let b = tile_to_screen((r.max_x + 1) as f32, (r.max_y + 1) as f32);
            let rect = egui::Rect::from_two_pos(a, b);
            painter.rect_filled(rect, 0.0, color.gamma_multiply(0.2));
            painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.5_f32, color));
        };
        if let Some(ref plan) = self.region.plan {
            let cell_color = Color32::from_rgb(70, 230, 160);
            for &(cx, cy) in plan.cells.iter().take(1024) {
                let a = tile_to_screen(cx as f32, cy as f32);
                let b = tile_to_screen((cx + 1) as f32, (cy + 1) as f32);
                let rect = egui::Rect::from_two_pos(a, b);
                painter.rect_filled(rect, 0.0, cell_color.gamma_multiply(0.3));
            }
        }
        if let Some(sel) = self.region.selection {
            draw(sel, Color32::from_rgb(90, 190, 255));
        }
        if let Some(live) = self.region.drag.rect() {
            draw(live, Color32::from_rgb(255, 210, 70));
        }
    }

    /// Plans the cells of the current operation from the dialog text.
    pub fn plan_region(&mut self, cx: &LandscapeContext) -> Result<(), String> {
        let (dimension, _) = cx.geometry();
        let result = parse_region_vertices(&self.region.vertices_text, dimension).and_then(|vertices| {
            let cells = plan_region_cells(self.region.op, dimension, &vertices, self.region.road_width, cx.textures)?;
            let rect = cells
                .iter()
                .fold(None::<TileRect>, |acc, &(x, y)| {
                    let (x, y) = (x as i32, y as i32);
                    Some(match acc {
                        None => TileRect { min_x: x, min_y: y, max_x: x, max_y: y },
                        Some(r) => TileRect {
                            min_x: r.min_x.min(x),
                            min_y: r.min_y.min(y),
                            max_x: r.max_x.max(x),
                            max_y: r.max_y.max(y),
                        },
                    })
                })
                .ok_or_else(|| "Region is empty.".to_string())?;
            Ok(RegionPlan { op: self.region.op, cells, rect })
        });
        match result {
            Ok(plan) => {
                self.status_message = format!("Region planned: {} tiles.", plan.cells.len());
                self.status_is_error = false;
                self.region.plan = Some(plan);
                self.dirty = true;
                Ok(())
            }
            Err(e) => {
                self.region.plan = None;
                self.status_message = format!("Region planning error: {e}");
                self.status_is_error = true;
                Err(e)
            }
        }
    }

    /// Turns the planned region into a host request (what the "Apply" button does).
    pub fn request_region_apply(&mut self) -> Result<(), String> {
        let Some(plan) = self.region.plan.clone() else {
            return Err("No region plan available.".into());
        };
        let request = match plan.op {
            RegionOperation::SelectObjects => RegionRequest::SelectObjects(plan.rect),
            _ => {
                let material = self.region.material.trim().to_string();
                if material.is_empty() {
                    let msg = "Select an available material first (e.g. 4BA___51).".to_string();
                    self.apply_failed(msg.clone());
                    return Err(msg);
                }
                RegionRequest::Paint(RegionPaintRequest {
                    cells: plan.cells,
                    material,
                    auto_bridge: self.region.auto_bridge,
                })
            }
        };
        self.region.request = Some(request);
        Ok(())
    }

    pub fn take_region_request(&mut self) -> Option<RegionRequest> {
        self.region.request.take()
    }

    /// One-call host integration: takes a pending request, applies it and records the status.
    ///
    /// `objects` is only evaluated for `SelectObjects` and must return `(id, world_x, world_z)`.
    pub fn apply_pending_region(
        &mut self,
        session: Option<&mut TerrainBlendEditSession>,
        objects: impl FnOnce() -> Vec<(usize, f32, f32)>,
    ) -> RegionOutcome {
        let Some(request) = self.take_region_request() else {
            return RegionOutcome::default();
        };
        match request {
            RegionRequest::SelectObjects(rect) => {
                let ids = objects_in_rect(&rect, &objects());
                self.region.plan = None;
                self.dirty = false;
                self.status_message = format!("Selected {} objects in the rectangle.", ids.len());
                self.status_is_error = false;
                RegionOutcome { textures_changed: 0, selected_objects: Some(ids) }
            }
            RegionRequest::Paint(paint) => {
                let Some(session) = session else {
                    self.apply_failed("Materials session (boden.txt) must be loaded to apply a region fill.");
                    return RegionOutcome::default();
                };
                match apply_region_paint(session, &paint) {
                    Ok(changed) => {
                        self.region.plan = None;
                        self.dirty = false;
                        self.status_message = format!("Region applied: {changed} tiles changed.");
                        self.status_is_error = false;
                        RegionOutcome { textures_changed: changed, selected_objects: None }
                    }
                    Err(e) => {
                        self.apply_failed(e);
                        RegionOutcome::default()
                    }
                }
            }
        }
    }

    fn render_region_tab(&mut self, ui: &mut egui::Ui, cx: &LandscapeContext) {
        let (dimension, _) = cx.geometry();
        ui.heading(tl(self.english, "區域工具", "Region Tools"));
        ui.label(format!(
            "{} 0-{}。{}：{}，{}，{}。{}（{}）。",
            tl(self.english, "圖塊座標範圍", "Tile coordinates:"),
            dimension.saturating_sub(1),
            tl(self.english, "每行一組 X,Y", "One X,Y per line"),
            tl(self.english, "填色／選取需兩組對角頂點", "fill/select take two opposite corners"),
            tl(self.english, "連通填色需一組種子點", "connected fill one seed"),
            tl(self.english, "道路可填入任意數量路徑點", "road any number of waypoints"),
            tl(self.english, "河流、懸崖、城牆與侵蝕擁有各自專屬分頁", "River, cliff, wall and erosion have their own tabs"),
            tl(self.english, "侵蝕矩形會同步跟隨拖曳框選區域", "the erosion rectangle follows the dragged selection"),
        ));
        ui.add_space(6.0);
        egui::ComboBox::from_id_source("region_op_select")
            .selected_text(self.region.op.label())
            .width(360.0)
            .show_ui(ui, |ui| {
                for op in RegionOperation::ALL {
                    ui.selectable_value(&mut self.region.op, op, op.label());
                }
            });
        ui.add(egui::TextEdit::multiline(&mut self.region.vertices_text).desired_rows(3));
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.region.pick_armed, tl(self.english, "在畫布／3D 視圖上拖曳框選矩形", "Drag a rectangle on the canvas / 3D viewport"));
            if let Some(sel) = self.region.selection {
                ui.label(format!(
                    "{} {}x{} {}",
                    tl(self.english, "選區尺寸", "selection"),
                    sel.max_x - sel.min_x + 1,
                    sel.max_y - sel.min_y + 1,
                    tl(self.english, "圖塊", "tiles")
                ));
            }
        });
        if self.region.op != RegionOperation::SelectObjects {
            egui::Grid::new("region_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label(tl(self.english, "材質代碼：", "Material id:"));
                ui.text_edit_singleline(&mut self.region.material);
                ui.end_row();
                if self.region.op == RegionOperation::RoadPolyline {
                    ui.label(tl(self.english, "道路寬度（圖塊）：", "Road width (tiles):"));
                    ui.add(egui::DragValue::new(&mut self.region.road_width).clamp_range(1..=dimension.max(1)));
                    ui.end_row();
                }
                ui.label(tl(self.english, "自動橋接過渡：", "Auto bridge transitions:"));
                ui.checkbox(&mut self.region.auto_bridge, "");
                ui.end_row();
            });
        }
        ui.add_space(8.0);
        if ui.button(tl(self.english, "規劃區域", "Plan Region")).clicked() {
            let _ = self.plan_region(cx);
        }
        if let Some((count, op)) = self.region.plan.as_ref().map(|p| (p.cells.len(), p.op)) {
            ui.separator();
            ui.label(format!("{}: {count}", tl(self.english, "涵蓋圖塊數", "Tiles covered")));
            let ready = op == RegionOperation::SelectObjects || cx.textures.is_some();
            if ui
                .add_enabled(ready, egui::Button::new(tl(self.english, "套用區域", "Apply region")))
                .on_hover_text(tl(
                    self.english,
                    "填色為單一步驟可復原之材質筆畫；選取僅會選定已放置之物件。",
                    "Fills as one undoable texture stroke; selection only picks placed objects.",
                ))
                .clicked()
            {
                let _ = self.request_region_apply();
            }
            if !ready {
                ui.label(tl(self.english, "在材質面板中載入地圖以套用填色。", "Load the map in the Materials panel to apply a fill."));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_landscape_ui_initial_state_and_dirty_guards() {
        let mut ui = LandscapeUi::default();
        assert!(!ui.open);
        assert!(!ui.is_dirty());

        ui.mark_dirty();
        assert!(ui.is_dirty());

        ui.request_close();
        assert!(ui.pending_discard);

        ui.clear_dirty();
        assert!(!ui.is_dirty());
        assert!(!ui.pending_discard);
    }

    #[test]
    fn test_landscape_ui_waypoints_parser() {
        let text = "10,20\n30,40\n\ninvalid\n50,60";
        let points = LandscapeUi::parse_waypoints(text);
        assert_eq!(points.len(), 3);
        assert_eq!(points[0], (10, 20));
        assert_eq!(points[1], (30, 40));
        assert_eq!(points[2], (50, 60));
    }

    #[test]
    fn test_landscape_ui_cliff_detection() {
        let mut ui = LandscapeUi::default();
        let mut heights = vec![0u8; 16 * 16];
        // Create an abrupt cliff
        for y in 0..16 {
            for x in 8..16 {
                heights[y * 16 + x] = 200;
            }
        }
        assert!(ui.run_cliff_detection(16, Some(&heights)).is_ok());
        assert!(ui.last_cliff_result.is_some());
        assert!(ui.is_dirty());
    }

    #[test]
    fn test_landscape_ui_river_planning() {
        let mut ui = LandscapeUi::default();
        ui.river_start_x = 2;
        ui.river_start_y = 2;
        ui.river_end_x = 6;
        ui.river_end_y = 6;
        assert!(ui.run_river_planning(16, None).is_ok());
        assert!(ui.last_river_plan.is_some());
        assert!(ui.is_dirty());
    }

    #[test]
    fn water_level_change_invalidates_river_plan_and_pending_apply() {
        let mut ui = LandscapeUi::default();
        ui.sync_water_level(20.0);
        ui.river_start_x = 0;
        ui.river_start_y = 0;
        ui.river_end_x = 2;
        ui.river_end_y = 2;
        ui.run_river_planning(16, None).unwrap();
        ui.apply_requested = Some(LandscapeApply::River);
        ui.sync_water_level(20.0);
        assert!(ui.last_river_plan.is_some());
        assert_eq!(ui.apply_requested, Some(LandscapeApply::River));
        ui.sync_water_level(25.0);
        assert!(ui.last_river_plan.is_none());
        assert_eq!(ui.apply_requested, None);
        assert_eq!(ui.cached_water_level, Some(25.0));
        assert!(ui.status_message.contains("重新規劃"));
        ui.run_river_planning(16, None).unwrap();
        assert!(ui.last_river_plan.is_some());
    }

    #[test]
    fn test_landscape_ui_road_planning() {
        let mut ui = LandscapeUi::default();
        ui.road_waypoints_text = "2,2\n2,8".to_string();
        assert!(ui.run_road_planning(16).is_ok());
        assert!(ui.last_road_plan.is_some());
        assert!(ui.is_dirty());
    }

    #[test]
    fn failed_road_replanning_discards_old_plan_and_pending_apply() {
        let mut ui = LandscapeUi::default();
        ui.road_waypoints_text = "2,2\n2,8".to_string();
        ui.run_road_planning(16).unwrap();
        ui.apply_requested = Some(LandscapeApply::Road);
        ui.road_waypoints_text = "2,2".to_string();
        assert!(ui.run_road_planning(16).is_err());
        assert!(ui.last_road_plan.is_none());
        assert_eq!(ui.take_apply_request(), None);

        ui.apply_requested = Some(LandscapeApply::River);
        assert!(ui.run_road_planning(16).is_err());
        assert_eq!(ui.take_apply_request(), Some(LandscapeApply::River));
    }

    #[test]
    fn test_landscape_ui_wall_planning() {
        let mut ui = LandscapeUi::default();
        ui.wall_waypoints_text = "4,4\n8,4\n8,8".to_string();
        assert!(ui.run_wall_planning(16).is_ok());
        assert!(ui.last_wall_plan.is_some());
        assert!(ui.is_dirty());
    }

    #[test]
    fn test_landscape_ui_erosion_valid_params_produce_changes_and_bounds_filter() {
        let mut ui = LandscapeUi::default();
        let size = 32;
        let mut heights = vec![0u8; size * size];
        // Create a steep slope in the center
        for y in 0..size {
            for x in 0..size {
                if x >= 10 && x <= 22 && y >= 10 && y <= 22 {
                    heights[y * size + x] = 180;
                } else {
                    heights[y * size + x] = 40;
                }
            }
        }

        ui.erosion_rectangle_text = "10,10\n22,22".to_string();
        ui.erosion_hydraulic_params.droplet_count = 2000;
        ui.erosion_thermal_params.iterations = 6;

        let res = ui.run_erosion_planning(size, Some(&heights));
        assert!(res.is_ok(), "planning failed: {:?}", res.err());
        let plan = res.unwrap();

        assert!(!plan.changes.is_empty(), "Erosion should produce changes on steep slope");
        assert!(ui.is_dirty(), "UI should be dirty after planning erosion");

        // Verify C# inside bounds filtering: all changes must fall within vertex bounds
        let (min_v, min_z, max_v, max_z) = plan.vertex_bounds;
        for c in &plan.changes {
            let vx = (c.index % size) as i32;
            let vz = (c.index / size) as i32;
            assert!(
                vx >= min_v && vx <= max_v && vz >= min_z && vz <= max_z,
                "Sample index {} ({vx}, {vz}) leaked outside bounds [{min_v}..={max_v}, {min_z}..={max_z}]",
                c.index
            );
            assert_ne!(c.before, c.after);
        }
    }

    #[test]
    fn test_landscape_ui_erosion_flat_terrain_reports_no_change() {
        let mut ui = LandscapeUi::default();
        let size = 16;
        let heights = vec![100u8; size * size];

        ui.erosion_rectangle_text = "2,2\n8,8".to_string();
        let res = ui.run_erosion_planning(size, Some(&heights));
        assert!(res.is_ok());
        let plan = res.unwrap();
        assert_eq!(plan.changes.len(), 0);
        assert_eq!(ui.status_message, "Erosion made no visible change.");
    }

    #[test]
    fn test_landscape_ui_erosion_bounds_and_invalid_input_guard() {
        let mut ui = LandscapeUi::default();
        let size = 16;
        let heights = vec![100u8; size * size];

        // 1. Missing terrain heights
        let res_no_heights = ui.run_erosion_planning(size, None);
        assert!(res_no_heights.is_err());
        assert!(ui.status_is_error);

        // 2. Out-of-bounds coordinates
        ui.erosion_rectangle_text = "0,0\n30,30".to_string(); // size 16 has tiles 0..15
        let res_oob = ui.run_erosion_planning(size, Some(&heights));
        assert!(res_oob.is_err());
        assert!(res_oob.unwrap_err().contains("out of bounds"));

        // 3. Not enough points
        ui.erosion_rectangle_text = "5,5".to_string();
        let res_too_few = ui.run_erosion_planning(size, Some(&heights));
        assert!(res_too_few.is_err());

        // 4. Malformed syntax
        ui.erosion_rectangle_text = "invalid\n5,5".to_string();
        let res_syntax = ui.run_erosion_planning(size, Some(&heights));
        assert!(res_syntax.is_err());

        // 5. Both simulations disabled
        ui.erosion_rectangle_text = "2,2\n6,6".to_string();
        ui.erosion_enable_hydraulic = false;
        ui.erosion_enable_thermal = false;
        let res_disabled = ui.run_erosion_planning(size, Some(&heights));
        assert!(res_disabled.is_err());
        assert!(res_disabled.unwrap_err().contains("disabled"));
    }

    #[test]
    fn test_landscape_ui_erosion_apply_contract_and_dirty_lifecycle() {
        let mut ui = LandscapeUi::default();
        let size = 16;
        let mut heights = vec![0u8; size * size];
        for y in 0..size {
            for x in 0..size {
                if x >= 4 && x <= 12 && y >= 4 && y <= 12 {
                    heights[y * size + x] = 160;
                } else {
                    heights[y * size + x] = 20;
                }
            }
        }

        ui.erosion_rectangle_text = "4,4\n12,12".to_string();
        let res = ui.run_erosion_planning(size, Some(&heights));
        assert!(res.is_ok());
        let plan = res.unwrap();
        assert!(!plan.changes.is_empty());
        assert!(ui.is_dirty());

        // Request apply via contract
        ui.request_erosion_apply();
        assert!(ui.erosion_apply_requested);

        // Take request
        let requested_plan = ui.take_erosion_apply_request();
        assert!(requested_plan.is_some());
        assert!(!ui.erosion_apply_requested);

        // Apply to height slice
        let mut test_heights = heights.clone();
        let applied_count = requested_plan.unwrap().apply_to_heights(&mut test_heights);
        assert_eq!(applied_count, plan.changes.len());
        assert_ne!(test_heights, heights);

        // Mark applied
        ui.erosion_applied("8 height samples modified");
        assert!(!ui.is_dirty());
        assert!(ui.last_erosion_plan.is_none());

        // Clear dirty lifecycle check
        ui.erosion_rectangle_text = "4,4\n12,12".to_string();
        let _ = ui.run_erosion_planning(size, Some(&heights));
        assert!(ui.is_dirty());
        ui.clear_dirty();
        assert!(!ui.is_dirty());
        assert!(ui.last_erosion_plan.is_none());
    }

    #[test]
    fn test_landscape_ui_erosion_apply_to_slice_convenience() {
        let mut ui = LandscapeUi::default();
        let size = 16;
        let mut heights = vec![0u8; size * size];
        for y in 0..size {
            for x in 0..size {
                if x >= 4 && x <= 12 && y >= 4 && y <= 12 {
                    heights[y * size + x] = 160;
                } else {
                    heights[y * size + x] = 20;
                }
            }
        }

        ui.erosion_rectangle_text = "4,4\n12,12".to_string();
        let _ = ui.run_erosion_planning(size, Some(&heights));
        let mut live_heights = heights.clone();
        let count_res = ui.apply_erosion_to_slice(&mut live_heights);
        assert!(count_res.is_ok());
        assert!(count_res.unwrap() > 0);
        assert!(!ui.is_dirty());
        assert!(ui.last_erosion_plan.is_none());
        assert!(ui.status_message.contains("Erosion applied"));
    }
}

#[cfg(test)]
mod small_ux_tests {
    use super::*;
    #[test]
    fn erosion_reset_preserves_disabled_mode() {
        let mut ui = LandscapeUi::default();
        ui.erosion_hydraulic_params.droplet_count = 123;
        ui.erosion_thermal_params.iterations = 29;
        ui.erosion_enable_hydraulic = true;
        ui.erosion_enable_thermal = false;
        let thermal = format!("{:?}", ui.erosion_thermal_params);
        ui.reset_enabled_erosion_parameters();
        assert_eq!(ui.erosion_hydraulic_params.droplet_count, 8000);
        assert_eq!(format!("{:?}", ui.erosion_thermal_params), thermal);
        ui.erosion_hydraulic_params.droplet_count = 456;
        let hydraulic = format!("{:?}", ui.erosion_hydraulic_params);
        ui.erosion_enable_hydraulic = false;
        ui.erosion_enable_thermal = true;
        ui.reset_enabled_erosion_parameters();
        assert_eq!(format!("{:?}", ui.erosion_hydraulic_params), hydraulic);
        assert_eq!(ui.erosion_thermal_params.iterations, ThermalErosionParams::default().iterations);
    }

    #[test]
    fn test_landscape_ui_water_level_drag_edits_ambience_and_marks_dirty() {
        let mut landscape = LandscapeUi::default();
        landscape.open = true;
        let mut ambience = AmbienceUi::default();
        ambience.profile.boden.waterlevel = 20.0;
        ambience.dirty = false;

        let ctx = egui::Context::default();
        let cx = LandscapeContext::plain(33, None);

        // Frame 1: render landscape window with ambience
        let _ = ctx.run(Default::default(), |ctx| {
            landscape.show(ctx, &cx, Some(&mut ambience));
        });
        assert_eq!(landscape.cached_water_level, Some(20.0));
        assert!(!ambience.dirty, "ambience should remain clean before edits");

        // Frame 2: find position of "Water level / 水位:" to locate DragValue
        let shapes = ctx.run(Default::default(), |ctx| {
            landscape.show(ctx, &cx, Some(&mut ambience));
        }).shapes;

        let label_rect = shapes.iter().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.text().contains("Water level / 水位:") => {
                Some(egui::Rect::from_min_size(t.pos, t.galley.size()))
            }
            _ => None,
        }).expect("found Water level label in landscape window");

        // Target DragValue center (placed immediately to the right of the label)
        let drag_pos = egui::pos2(label_rect.max.x + 20.0, label_rect.center().y);

        // Press pointer on the drag field
        let _ = ctx.run(egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(drag_pos),
                egui::Event::PointerButton {
                    pos: drag_pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
            ..Default::default()
        }, |ctx| {
            landscape.show(ctx, &cx, Some(&mut ambience));
        });

        // Drag to the right by 30 pixels (speed is 0.5, so delta is +15.0)
        let dragged_pos = egui::pos2(drag_pos.x + 30.0, drag_pos.y);
        let _ = ctx.run(egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(dragged_pos),
            ],
            ..Default::default()
        }, |ctx| {
            landscape.show(ctx, &cx, Some(&mut ambience));
        });

        // Release pointer
        let _ = ctx.run(egui::RawInput {
            events: vec![
                egui::Event::PointerButton {
                    pos: dragged_pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ],
            ..Default::default()
        }, |ctx| {
            landscape.show(ctx, &cx, Some(&mut ambience));
        });

        // Verify editing in landscape window updated ambience boden water level and marked ambience dirty
        assert!(ambience.dirty, "editing water level in landscape window must mark ambience dirty");
        assert_eq!(ambience.profile.boden.waterlevel, 35.0, "drag should increase waterlevel from 20.0 to 35.0");
        assert_eq!(landscape.cached_water_level, Some(35.0));
    }

    fn collect_shape_texts(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
        match shape {
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::epaint::Shape::Vec(shapes) => {
                for s in shapes {
                    collect_shape_texts(s, out);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn test_landscape_ui_chinese_localization_renders_no_untranslated_literals() {
        let tabs = [
            LandscapeTab::CliffPlanner,
            LandscapeTab::RiverPlanner,
            LandscapeTab::RoadPlanner,
            LandscapeTab::WallPlanner,
            LandscapeTab::ErosionPlanner,
            LandscapeTab::RegionTools,
        ];
        let english_literals_to_forbid = [
            "Landscape & Fortification Studio",
            "Cliff Edge Detection & Face Planning",
            "Detects steep elevation gradients",
            "River Flow Planner",
            "Traces natural downhill water courses",
            "Road & Path Polyline Planner",
            "Draws continuous road strokes",
            "Wall & Palisade Planner",
            "Places straight walls, corner segments",
            "Hydraulic & Thermal Erosion in Rectangle",
            "Simulates rainfall droplet runoff",
            "Apply to map",
            "needs the original transition tiles",
            "Min Slope Degrees:",
            "Min Elevation Delta:",
            "Blend scree at cliff foot:",
            "Start Coordinate (X, Y):",
            "End Coordinate (X, Y):",
            "Water Texture:",
            "Riverbed:",
            "Road Texture:",
            "Fortification Style:",
            "Towers on Corners/Intervals:",
            "Gates on Road Intersections:",
            "Waypoints (One X,Y per line):",
            "Corner 1 (X, Y):",
            "Corner 2 (X, Y):",
            "Droplet Count:",
            "Max Lifetime:",
            "Inertia:",
            "Gravity:",
            "Erosion Rate:",
            "Deposition Rate:",
            "Erosion Radius:",
            "Random Seed:",
            "Talus Angle:",
            "Iterations:",
            "Material id:",
            "Road width (tiles):",
            "Auto bridge transitions:",
            "Apply region",
            "Drag a rectangle on the 2D map",
        ];

        let ctx = egui::Context::default();
        let heights = vec![30u8; 33 * 33];
        let cx = LandscapeContext::plain(33, Some(&heights));

        for tab in tabs {
            let mut ui = LandscapeUi::default();
            ui.open = true;
            ui.english = false;
            ui.current_tab = tab;

            // egui windows measure on first frame and paint on subsequent frames
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))),
                    ..Default::default()
                },
                |ctx| {
                    ui.show_without_ambience(ctx, &cx);
                },
            );

            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))),
                    ..Default::default()
                },
                |ctx| {
                    ui.show_without_ambience(ctx, &cx);
                },
            );

            let mut texts = Vec::new();
            for shape in &output.shapes {
                collect_shape_texts(&shape.shape, &mut texts);
            }
            let all_text = texts.join(" ");

            // Assert Chinese header is present
            assert!(all_text.contains("地貌與防禦工事工作室"), "tab {:?} missing Chinese window title", tab);

            // Assert no untranslated literals remain in rendered UI
            for &forbidden in &english_literals_to_forbid {
                assert!(
                    !all_text.contains(forbidden),
                    "Tab {:?} rendered untranslated literal: '{}'",
                    tab,
                    forbidden
                );
            }
        }
    }
}
