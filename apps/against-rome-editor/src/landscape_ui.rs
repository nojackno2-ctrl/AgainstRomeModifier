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
    Point,
};
use eframe::egui::{self, Color32, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LandscapeTab {
    CliffPlanner,
    RiverPlanner,
    RoadPlanner,
    WallPlanner,
}

pub struct LandscapeUi {
    pub open: bool,
    pub current_tab: LandscapeTab,

    // --- Cliff state ---
    pub cliff_min_slope: f32,
    pub cliff_min_delta: f32,
    pub last_cliff_result: Option<CliffDetectionResult>,

    // --- River state ---
    pub river_start_x: i32,
    pub river_start_y: i32,
    pub river_end_x: i32,
    pub river_end_y: i32,
    pub river_preferred_texture: String,
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
    pub last_wall_plan: Option<WallStrokePlan>,

    // Dirty / guards
    pub dirty: bool,
    pub pending_discard: bool,
    pub status_message: String,
    pub status_is_error: bool,
}

impl Default for LandscapeUi {
    fn default() -> Self {
        Self {
            open: false,
            current_tab: LandscapeTab::CliffPlanner,
            cliff_min_slope: DEFAULT_MIN_SLOPE_DEGREES,
            cliff_min_delta: DEFAULT_MIN_ELEVATION_DELTA,
            last_cliff_result: None,
            river_start_x: 10,
            river_start_y: 10,
            river_end_x: 50,
            river_end_y: 50,
            river_preferred_texture: "FLUSS1".to_string(),
            last_river_plan: None,
            road_waypoints_text: "15,15\n30,15\n30,45".to_string(),
            road_preferred_texture: "PFAD1".to_string(),
            last_road_plan: None,
            wall_style: FortificationStyle::GermanicPalisade,
            wall_waypoints_text: "20,20\n40,20\n40,40\n20,40\n20,20".to_string(),
            wall_place_towers: true,
            wall_place_gates: true,
            last_wall_plan: None,
            dirty: false,
            pending_discard: false,
            status_message: "Ready.".to_string(),
            status_is_error: false,
        }
    }
}

impl LandscapeUi {
    pub fn is_dirty(&self) -> bool {
        self.dirty
            || self.last_cliff_result.is_some()
            || self.last_river_plan.is_some()
            || self.last_road_plan.is_some()
            || self.last_wall_plan.is_some()
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
        self.last_cliff_result = None;
        self.last_river_plan = None;
        self.last_road_plan = None;
        self.last_wall_plan = None;
        self.pending_discard = false;
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

    /// `vertex_size` is the height-grid vertex count; planners work on `vertex_size - 1` tiles.
    pub fn run_cliff_detection(&mut self, vertex_size: usize, heights: Option<&[u8]>) -> Result<(), String> {
        let result = match heights {
            Some(h) => detect_from_vertex_heights(
                vertex_size,
                h,
                vertex_size.saturating_sub(1),
                1,
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

    fn tile_textures(vertex_size: usize) -> (i32, Vec<String>) {
        let tiles = vertex_size.saturating_sub(1);
        (tiles as i32, vec!["GRAS1".to_string(); tiles * tiles])
    }

    pub fn run_river_planning(&mut self, vertex_size: usize, heights: Option<&[u8]>) -> Result<(), String> {
        let path = rasterize_orthogonal_path(&[
            (self.river_start_x, self.river_start_y),
            (self.river_end_x, self.river_end_y),
        ]);
        let preferred = self.river_preferred_texture.trim();
        let preferred = (!preferred.is_empty()).then_some(preferred);
        let catalog = RiverTileCatalog::build_available(None, preferred, None);
        let options = RiverPlannerOptions {
            preferred_river_texture: preferred.map(str::to_string),
            elevation_mode: RiverElevationMode::ValidateOnly,
            ..Default::default()
        };
        let (tiles, textures) = Self::tile_textures(vertex_size);
        let heights = heights.filter(|h| h.len() == vertex_size * vertex_size);

        match plan_river(tiles, &textures, heights, vertex_size as i32, &path, &catalog, &options) {
            Ok(plan) => {
                self.status_message = format!(
                    "River planned: {} water tiles, {} bank tiles, {} unsupported.",
                    plan.water_tiles.len(),
                    plan.bank_tiles.len(),
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
        let points = Self::parse_waypoints(&self.road_waypoints_text);
        if points.len() < 2 {
            return Err("At least 2 waypoints are required for road planning.".into());
        }

        let (tiles, textures) = Self::tile_textures(vertex_size);
        let preferred = self.road_preferred_texture.trim();
        let available = build_road_tiles(None, (!preferred.is_empty()).then_some(preferred));

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

        let tiles = vertex_size.saturating_sub(1) as i32;
        let plan = plan_wall_stroke(tiles, &points, &options, &catalog, None, None, None);
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

    pub fn show(&mut self, ctx: &egui::Context, dimension: usize, heights: Option<&[u8]>) {
        if !self.open {
            return;
        }

        let mut is_open = self.open;
        egui::Window::new("Landscape & Fortification Studio")
            .open(&mut is_open)
            .min_size(Vec2::new(720.0, 520.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.current_tab, LandscapeTab::CliffPlanner, "Cliff Edge & Face");
                    ui.selectable_value(&mut self.current_tab, LandscapeTab::RiverPlanner, "River Flow");
                    ui.selectable_value(&mut self.current_tab, LandscapeTab::RoadPlanner, "Road Polyline");
                    ui.selectable_value(&mut self.current_tab, LandscapeTab::WallPlanner, "Wall / Palisade");
                });
                ui.separator();

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
                    LandscapeTab::CliffPlanner => self.render_cliff_tab(ui, dimension, heights),
                    LandscapeTab::RiverPlanner => self.render_river_tab(ui, dimension, heights),
                    LandscapeTab::RoadPlanner => self.render_road_tab(ui, dimension),
                    LandscapeTab::WallPlanner => self.render_wall_tab(ui, dimension),
                }
            });

        if !is_open {
            self.request_close();
        }

        if self.pending_discard {
            let mut close_modal = false;
            let mut confirm = false;

            egui::Window::new("Discard Unsaved Landscape Plans?")
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Closing will discard currently planned cliff, river, road, and wall strokes.");
                    ui.horizontal(|ui| {
                        if ui.button("Discard & Close").clicked() {
                            confirm = true;
                        }
                        if ui.button("Keep Editing").clicked() {
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

    fn render_cliff_tab(&mut self, ui: &mut egui::Ui, dimension: usize, heights: Option<&[u8]>) {
        ui.heading("Cliff Edge Detection & Face Planning");
        ui.label("Detects steep elevation gradients and extracts continuous cliff contour lines.");

        ui.add_space(6.0);
        egui::Grid::new("cliff_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("Min Slope Degrees:");
            ui.add(egui::Slider::new(&mut self.cliff_min_slope, 15.0..=75.0).text("°"));
            ui.end_row();

            ui.label("Min Elevation Delta:");
            ui.add(egui::Slider::new(&mut self.cliff_min_delta, 5.0..=64.0));
            ui.end_row();
        });

        ui.add_space(8.0);
        if ui.button("Detect Cliffs on Heightmap").clicked() {
            let _ = self.run_cliff_detection(dimension, heights);
        }

        if let Some(res) = &self.last_cliff_result {
            ui.separator();
            ui.label(format!("Detected Cliff Cells: {}", res.cliff_cells.len()));
            ui.label(format!("Contour Paths: {}", res.contours.len()));

            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for (i, contour) in res.contours.iter().enumerate() {
                    ui.label(format!("Path #{}: {} cells (Closed: {})", i + 1, contour.cells.len(), contour.is_closed_loop));
                }
            });
        }
    }

    fn render_river_tab(&mut self, ui: &mut egui::Ui, dimension: usize, heights: Option<&[u8]>) {
        ui.heading("River Flow Planner");
        ui.label("Traces natural downhill water courses with seamless water and riverbank textures.");

        ui.add_space(6.0);
        egui::Grid::new("river_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("Start Coordinate (X, Y):");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.river_start_x));
                ui.add(egui::DragValue::new(&mut self.river_start_y));
            });
            ui.end_row();

            ui.label("End Coordinate (X, Y):");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.river_end_x));
                ui.add(egui::DragValue::new(&mut self.river_end_y));
            });
            ui.end_row();

            ui.label("Water Texture Prefix:");
            ui.text_edit_singleline(&mut self.river_preferred_texture);
            ui.end_row();
        });

        ui.add_space(8.0);
        if ui.button("Plan River Stroke").clicked() {
            let _ = self.run_river_planning(dimension, heights);
        }

        if let Some(plan) = &self.last_river_plan {
            ui.separator();
            ui.label(format!("Water Tiles Planned: {}", plan.water_tiles.len()));
            ui.label(format!("Bank Transition Tiles: {}", plan.bank_tiles.len()));
            if !plan.unsupported.is_empty() {
                ui.colored_label(Color32::from_rgb(255, 180, 50), format!("Unsupported Tiles: {}", plan.unsupported.len()));
            }
        }
    }

    fn render_road_tab(&mut self, ui: &mut egui::Ui, dimension: usize) {
        ui.heading("Road & Path Polyline Planner");
        ui.label("Draws continuous road strokes with proper straight, turn, and T-junction textures.");

        ui.add_space(6.0);
        ui.label("Waypoints (One X,Y per line):");
        ui.text_edit_multiline(&mut self.road_waypoints_text);

        ui.horizontal(|ui| {
            ui.label("Road Texture:");
            ui.text_edit_singleline(&mut self.road_preferred_texture);
        });

        ui.add_space(8.0);
        if ui.button("Plan Road Stroke").clicked() {
            let _ = self.run_road_planning(dimension);
        }

        if let Some(plan) = &self.last_road_plan {
            ui.separator();
            ui.label(format!("Road Placements Planned: {}", plan.tiles.len()));
            if !plan.succeeded {
                ui.colored_label(Color32::from_rgb(255, 100, 100), "Road stroke encountered out-of-bounds tiles.");
            }
        }
    }

    fn render_wall_tab(&mut self, ui: &mut egui::Ui, dimension: usize) {
        ui.heading("Wall & Palisade Planner");
        ui.label("Places straight walls, corner segments, defensive towers, and gates along waypoints.");

        ui.add_space(6.0);
        egui::Grid::new("wall_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("Fortification Style:");
            egui::ComboBox::from_id_source("wall_style_select")
                .selected_text(format!("{:?}", self.wall_style))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.wall_style, FortificationStyle::GermanicPalisade, "Germanic Palisade (日耳曼木柵)");
                    ui.selectable_value(&mut self.wall_style, FortificationStyle::RomanStoneWall, "Roman Stone Wall (羅馬石牆)");
                    ui.selectable_value(&mut self.wall_style, FortificationStyle::CelticPalisade, "Celtic Palisade (塞爾特木柵)");
                    ui.selectable_value(&mut self.wall_style, FortificationStyle::HunBarricade, "Hun Barricade (匈人路障)");
                });
            ui.end_row();

            ui.label("Towers on Corners/Intervals:");
            ui.checkbox(&mut self.wall_place_towers, "");
            ui.end_row();

            ui.label("Gates on Road Intersections:");
            ui.checkbox(&mut self.wall_place_gates, "");
            ui.end_row();
        });

        ui.label("Waypoints (One X,Y per line):");
        ui.text_edit_multiline(&mut self.wall_waypoints_text);

        ui.add_space(8.0);
        if ui.button("Plan Wall Stroke").clicked() {
            let _ = self.run_wall_planning(dimension);
        }

        if let Some(plan) = &self.last_wall_plan {
            ui.separator();
            ui.label(format!("Placements Planned: {}", plan.placements.len()));
            let count = |kind| plan.placements.iter().filter(|p| p.kind == kind).count();
            ui.label(format!("Towers: {}, Gates: {}", count(WallComponentKind::Tower), count(WallComponentKind::Gate)));
            ui.label(format!("Blocked Footprint Tiles: {}", plan.footprint_tiles.len()));
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
    fn test_landscape_ui_road_planning() {
        let mut ui = LandscapeUi::default();
        ui.road_waypoints_text = "2,2\n2,8".to_string();
        assert!(ui.run_road_planning(16).is_ok());
        assert!(ui.last_road_plan.is_some());
        assert!(ui.is_dirty());
    }

    #[test]
    fn test_landscape_ui_wall_planning() {
        let mut ui = LandscapeUi::default();
        ui.wall_waypoints_text = "4,4\n8,4\n8,8".to_string();
        assert!(ui.run_wall_planning(16).is_ok());
        assert!(ui.last_wall_plan.is_some());
        assert!(ui.is_dirty());
    }
}
