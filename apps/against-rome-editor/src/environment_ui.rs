//! Environment Studio UI: Flora Scatter, Neutral Lairs, Settlement Generator.
//! Replicates C# `SettlementGeneratorDialog.cs`, `WildLairDialog.cs` and integrates `native/against-rome-environment`.

use against_rome_environment::{
    nature::{
        BiomeEcologyProfile, BiomeType, FloraScatterContext, FloraScatterEngine,
        FloraScatterParameters, NatureAddition,
    },
    settlement::{
        MapLayoutPreset, MultiplayerDistributionResult, MultiplayerFairnessBalancer,
        ResourceClusterPlanner, SettlementGeneratorEngine, SettlementSiteEvaluator,
        SettlementTribe, SymmetryMode,
    },
    wild_lair::{
        NeutralLairCatalog, PlacedNeutralLair, ScenarioEvent, WildLairScenarioEventBinder,
    },
};
use std::collections::HashMap;
use eframe::egui::{self, Color32, Vec2};

const DEFAULT_FLORA_TEMPLATES: &str = "1,LanGerNad00_Tanne_gross\n2,LanGerBus01_Busch\n3,LanGerGra01_Gras\n4,LanGerSch01_Schilf\n5,LanGerSte01_Stein";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvironmentTab {
    FloraScatter,
    NeutralLairs,
    SettlementGenerator,
}

pub struct EnvironmentUi {
    pub open: bool,
    pub current_tab: EnvironmentTab,

    // --- Flora Scatter state ---
    pub selected_biome: BiomeType,
    pub flora_seed: i32,
    pub flora_density: f32,
    pub forest_coverage: f32,
    pub forest_patch_scale: f32,
    pub flora_jitter: f32,
    pub avoid_roads: bool,
    /// One `id,name` object template per line; replace with the loaded game's catalog.
    pub flora_templates_text: String,
    pub generated_flora: Vec<NatureAddition>,

    // --- Neutral Lairs state ---
    pub lair_catalog: NeutralLairCatalog,
    pub selected_lair_id: String,
    pub lair_world_x: f32,
    pub lair_world_z: f32,
    pub lair_rotation_deg: f32,
    pub generated_lairs: Vec<PlacedNeutralLair>,
    pub generated_lair_events: Vec<ScenarioEvent>,

    // --- Settlement Generator state ---
    pub settlement_players: usize,
    pub settlement_tribe: SettlementTribe,
    pub settlement_symmetry: SymmetryMode,
    pub settlement_seed: i32,
    pub settlement_include_nature: bool,
    pub settlement_spawn_radius: f32,
    pub last_distribution: Option<MultiplayerDistributionResult>,
    pub last_placement_preset: Option<MapLayoutPreset>,
    pub last_nature_preset: Option<MapLayoutPreset>,

    // Dirty / guards
    pub dirty: bool,
    pub pending_discard: bool,
    pub status_message: String,
    pub status_is_error: bool,
}

impl Default for EnvironmentUi {
    fn default() -> Self {
        let catalog = NeutralLairCatalog::default();
        let default_lair_id = catalog
            .all_definitions()
            .first()
            .map(|d| d.id.clone())
            .unwrap_or_else(|| "LAIR_WOLF".to_string());

        Self {
            open: false,
            current_tab: EnvironmentTab::FloraScatter,
            selected_biome: BiomeType::GermanicForest,
            flora_seed: 1337,
            flora_density: 1.0,
            forest_coverage: 1.0,
            forest_patch_scale: 0.035,
            flora_jitter: 0.65,
            avoid_roads: true,
            flora_templates_text: DEFAULT_FLORA_TEMPLATES.to_string(),
            generated_flora: Vec::new(),
            lair_catalog: catalog,
            selected_lair_id: default_lair_id,
            lair_world_x: 4000.0,
            lair_world_z: 6000.0,
            lair_rotation_deg: 0.0,
            generated_lairs: Vec::new(),
            generated_lair_events: Vec::new(),
            settlement_players: 2,
            settlement_tribe: SettlementTribe::Germanic,
            settlement_symmetry: SymmetryMode::RotationalSymmetry,
            settlement_seed: 42,
            settlement_include_nature: true,
            settlement_spawn_radius: 0.55,
            last_distribution: None,
            last_placement_preset: None,
            last_nature_preset: None,
            dirty: false,
            pending_discard: false,
            status_message: "Ready.".to_string(),
            status_is_error: false,
        }
    }
}

impl EnvironmentUi {
    pub fn is_dirty(&self) -> bool {
        self.dirty
            || !self.generated_flora.is_empty()
            || !self.generated_lairs.is_empty()
            || self.last_distribution.is_some()
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
        self.generated_flora.clear();
        self.generated_lairs.clear();
        self.generated_lair_events.clear();
        self.last_distribution = None;
        self.last_placement_preset = None;
        self.last_nature_preset = None;
        self.pending_discard = false;
    }

    pub fn request_close(&mut self) {
        if self.is_dirty() {
            self.pending_discard = true;
        } else {
            self.open = false;
        }
    }

    fn parse_templates(text: &str) -> Result<(Vec<i32>, HashMap<i32, String>), String> {
        let mut ids = Vec::new();
        let mut names = HashMap::new();
        for (i, line) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
            let (id, name) = line
                .split_once(',')
                .ok_or_else(|| format!("Template line {} must be 'id,name'.", i + 1))?;
            let id: i32 = id
                .trim()
                .parse()
                .map_err(|_| format!("Template line {} has a non-numeric id.", i + 1))?;
            ids.push(id);
            names.insert(id, name.trim().to_string());
        }
        if ids.is_empty() {
            return Err("No object templates configured.".into());
        }
        Ok((ids, names))
    }

    /// `dimension` is the height-grid vertex count; `heights` is the matching raw u8 grid.
    pub fn generate_flora_scatter(&mut self, dimension: usize, heights: Option<&[u8]>, water_level: f32) {
        let (ids, names) = match Self::parse_templates(&self.flora_templates_text) {
            Ok(t) => t,
            Err(e) => {
                self.status_message = e;
                self.status_is_error = true;
                return;
            }
        };
        let tiles = dimension.saturating_sub(1) as i32;
        let heights = heights.filter(|h| h.len() == dimension * dimension);
        let context = FloraScatterContext {
            dimension: tiles,
            vertex_size: dimension as i32,
            heights: heights.map(|h| h.to_vec()),
            water_level: water_level.clamp(0.0, 255.0) as u8,
            profile: BiomeEcologyProfile::get_preset(self.selected_biome),
            available_templates: ids,
            objdef_names: names,
            ..Default::default()
        };
        let params = FloraScatterParameters {
            seed: self.flora_seed,
            density_multiplier: self.flora_density,
            forest_coverage: self.forest_coverage,
            forest_patch_scale: self.forest_patch_scale,
            min_tile_x: 0,
            min_tile_y: 0,
            max_tile_x: (tiles - 1).max(0),
            max_tile_y: (tiles - 1).max(0),
            jitter_amount: self.flora_jitter,
            avoid_roads_and_buildings: self.avoid_roads,
            ..Default::default()
        };

        let result = FloraScatterEngine::generate(&context, &params);
        self.generated_flora = result.additions;
        self.dirty = true;
        self.status_message = format!(
            "Generated {} flora objects for {:?} ecology (canopy {}, understory {}, ground {}, rocks {}).",
            result.report.total_planted,
            self.selected_biome,
            result.report.canopy_count,
            result.report.understory_count,
            result.report.ground_flora_count,
            result.report.rock_count,
        );
        self.status_is_error = false;
    }

    pub fn add_neutral_lair(&mut self) -> Result<(), String> {
        let def = self
            .lair_catalog
            .get_by_id(&self.selected_lair_id)
            .ok_or_else(|| format!("Unknown lair definition '{}'", self.selected_lair_id))?
            .clone();

        let mut placed = PlacedNeutralLair::new(
            uuid::Uuid::new_v4(),
            def.id.clone(),
            self.lair_world_x,
            0.0,
            self.lair_world_z,
        );
        placed.rotation_deg = self.lair_rotation_deg;

        // The binder is deliberately strict (e.g. FigTie animals cannot be spawned by events);
        // a rejection keeps the placed lair but reports why no scenario events were produced.
        let aliases = vec![(
            def.script_alias.clone().unwrap_or_else(|| def.id.clone()),
            def.native_building_or_landscape_type.clone(),
        )];
        let (events, note) = match WildLairScenarioEventBinder::generate_events_for_lair(
            &placed, &def, 0, 64.0, &aliases,
        ) {
            Ok(events) => (events, String::new()),
            Err(e) => (Vec::new(), format!(" No scenario events: {e}")),
        };

        self.generated_lairs.push(placed);
        self.generated_lair_events.extend(events);
        self.dirty = true;
        self.status_message = format!(
            "Added neutral lair '{}'; {} scenario events total.{}",
            def.display_name_en,
            self.generated_lair_events.len(),
            note
        );
        self.status_is_error = false;
        Ok(())
    }

    /// `dimension` is the height-grid vertex count.
    pub fn generate_settlement(&mut self, dimension: usize, heights: Option<&[u8]>, water_level: f32) -> Result<(), String> {
        let tiles = dimension.saturating_sub(1).max(1);
        let heights = heights.filter(|h| h.len() == dimension * dimension);
        let sample_height = |x: f32, y: f32| -> f32 {
            match heights {
                Some(h) => {
                    let ix = ((x / 64.0).round().max(0.0) as usize).min(dimension - 1);
                    let iy = ((y / 64.0).round().max(0.0) as usize).min(dimension - 1);
                    h[iy * dimension + ix] as f32
                }
                None => water_level + 10.0,
            }
        };
        let is_blocked = |_x: i32, _y: i32| -> bool { false };

        let evaluator = SettlementSiteEvaluator::new(tiles as i32, &sample_height, &is_blocked, water_level, 64.0);
        let planner = ResourceClusterPlanner::new(tiles as i32, &sample_height, &is_blocked, water_level, 64.0);
        let balancer = MultiplayerFairnessBalancer::new(&evaluator, &planner, tiles as i32, 64.0);
        let tribes = vec![self.settlement_tribe; self.settlement_players];

        let result = balancer.generate(
            self.settlement_players,
            self.settlement_symmetry,
            Some(&tribes),
            self.settlement_seed,
            self.settlement_spawn_radius,
        )?;

        let placement_preset = SettlementGeneratorEngine::to_placement_layout_preset(&result);
        let nature_preset = self
            .settlement_include_nature
            .then(|| SettlementGeneratorEngine::to_nature_layout_preset(&result));

        self.status_message = format!(
            "Settlement generation complete: {} player bases, {}.",
            result.players.len(),
            if result.fairness_report.is_balanced { "balanced" } else { "NOT balanced" }
        );
        self.last_distribution = Some(result);
        self.last_placement_preset = Some(placement_preset);
        self.last_nature_preset = nature_preset;
        self.dirty = true;
        self.status_is_error = false;
        Ok(())
    }

    pub fn show(&mut self, ctx: &egui::Context, dimension: usize, heights: Option<&[u8]>, water_level: f32) {
        if !self.open {
            return;
        }

        let mut is_open = self.open;
        egui::Window::new("Environment & Settlement Studio")
            .open(&mut is_open)
            .min_size(Vec2::new(740.0, 540.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.current_tab, EnvironmentTab::FloraScatter, "Flora Scatter");
                    ui.selectable_value(&mut self.current_tab, EnvironmentTab::NeutralLairs, "Neutral Lairs");
                    ui.selectable_value(
                        &mut self.current_tab,
                        EnvironmentTab::SettlementGenerator,
                        "Settlement Generator",
                    );
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
                    EnvironmentTab::FloraScatter => self.render_flora_tab(ui, dimension, heights, water_level),
                    EnvironmentTab::NeutralLairs => self.render_lairs_tab(ui),
                    EnvironmentTab::SettlementGenerator => self.render_settlement_tab(ui, dimension, heights, water_level),
                }
            });

        if !is_open {
            self.request_close();
        }

        if self.pending_discard {
            let mut close_modal = false;
            let mut confirm = false;

            egui::Window::new("Discard Unsaved Environment Changes?")
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Closing will discard currently generated flora, lairs, and settlement presets.");
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

    fn render_flora_tab(&mut self, ui: &mut egui::Ui, dimension: usize, heights: Option<&[u8]>, water_level: f32) {
        ui.heading("Flora & Ecology Auto-Scatter");
        ui.label("Generates organic nature foliage and rock formations based on Perlin hydrology and biome profiles.");

        ui.add_space(6.0);
        egui::Grid::new("flora_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("Biome Ecology:");
            egui::ComboBox::from_id_source("biome_select")
                .selected_text(format!("{:?}", self.selected_biome))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.selected_biome, BiomeType::GermanicForest, "Germanic Forest");
                    ui.selectable_value(&mut self.selected_biome, BiomeType::DanubeBroadleaf, "Danube Broadleaf");
                    ui.selectable_value(&mut self.selected_biome, BiomeType::ItalianHills, "Italian Hills");
                    ui.selectable_value(&mut self.selected_biome, BiomeType::BritishMarsh, "British Marsh");
                });
            ui.end_row();

            ui.label("Random Seed:");
            ui.add(egui::DragValue::new(&mut self.flora_seed));
            ui.end_row();

            ui.label("Density Multiplier:");
            ui.add(egui::Slider::new(&mut self.flora_density, 0.1..=3.0).text("x"));
            ui.end_row();

            ui.label("Forest Coverage:");
            ui.add(egui::Slider::new(&mut self.forest_coverage, 0.1..=2.0).text("ratio"));
            ui.end_row();

            ui.label("Forest Patch Scale:");
            ui.add(egui::Slider::new(&mut self.forest_patch_scale, 0.01..=0.1));
            ui.end_row();

            ui.label("Jitter Amount:");
            ui.add(egui::Slider::new(&mut self.flora_jitter, 0.0..=1.0));
            ui.end_row();

            ui.label("Avoid Roads & Buildings:");
            ui.checkbox(&mut self.avoid_roads, "");
            ui.end_row();

            ui.label("Object templates (id,name):");
            ui.add(egui::TextEdit::multiline(&mut self.flora_templates_text).desired_rows(4));
            ui.end_row();
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Generate Flora Scatter").clicked() {
                self.generate_flora_scatter(dimension, heights, water_level);
            }
            if ui.button("Clear Generated Flora").clicked() {
                self.generated_flora.clear();
                self.status_message = "Cleared generated flora.".to_string();
                self.status_is_error = false;
            }
        });

        ui.separator();
        ui.label(format!("Generated Flora Count: {}", self.generated_flora.len()));
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            for (i, item) in self.generated_flora.iter().take(50).enumerate() {
                ui.label(format!("{}. {} (Pos: {:.1}, {:.1}, {:.1})", i + 1, item.name, item.x, item.y, item.z));
            }
            if self.generated_flora.len() > 50 {
                ui.label(format!("... and {} more flora objects.", self.generated_flora.len() - 50));
            }
        });
    }

    fn render_lairs_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Neutral Lair & Monster Den Setup");
        ui.label("Places neutral wolf dens, bear caves, and bandit camps with automated scenario spawn waves and rewards.");

        ui.add_space(6.0);
        egui::Grid::new("lair_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("Lair Definition:");
            egui::ComboBox::from_id_source("lair_def_select")
                .selected_text(&self.selected_lair_id)
                .show_ui(ui, |ui| {
                    for def in self.lair_catalog.all_definitions() {
                        ui.selectable_value(
                            &mut self.selected_lair_id,
                            def.id.clone(),
                            format!("{} ({})", def.display_name_en, def.id),
                        );
                    }
                });
            ui.end_row();

            ui.label("World Location X / Z:");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.lair_world_x).clamp_range(0.0..=16383.0).prefix("X: "));
                ui.add(egui::DragValue::new(&mut self.lair_world_z).clamp_range(0.0..=16383.0).prefix("Z: "));
            });
            ui.end_row();

            ui.label("Rotation (deg):");
            ui.add(egui::DragValue::new(&mut self.lair_rotation_deg).clamp_range(0.0..=359.0));
            ui.end_row();
        });

        ui.add_space(8.0);
        if ui.button("Add Neutral Lair").clicked() {
            if let Err(e) = self.add_neutral_lair() {
                self.status_message = format!("Failed to add neutral lair: {e}");
                self.status_is_error = true;
            }
        }

        ui.separator();
        ui.label(format!("Placed Lairs: {} (Scenario Events: {})", self.generated_lairs.len(), self.generated_lair_events.len()));
        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
            for (i, lair) in self.generated_lairs.iter().enumerate() {
                ui.label(format!("{}. {} at ({:.1}, {:.1}) rot {:.0}", i + 1, lair.definition_id, lair.world_x, lair.world_z, lair.rotation_deg));
            }
        });
    }

    fn render_settlement_tab(&mut self, ui: &mut egui::Ui, dimension: usize, heights: Option<&[u8]>, water_level: f32) {
        ui.heading("One-Click Settlement / Base Generator");
        ui.label("Instantly balances starting town centers, barracks, resource depots, and surrounding forests/stone.");

        ui.add_space(6.0);
        egui::Grid::new("settlement_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("Player Count (2..8):");
            ui.add(egui::Slider::new(&mut self.settlement_players, 2..=8));
            ui.end_row();

            ui.label("Tribe Architecture:");
            egui::ComboBox::from_id_source("settlement_tribe_select")
                .selected_text(format!("{:?}", self.settlement_tribe))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.settlement_tribe, SettlementTribe::Germanic, "Germanic (日耳曼)");
                    ui.selectable_value(&mut self.settlement_tribe, SettlementTribe::Roman, "Roman (羅馬)");
                    ui.selectable_value(&mut self.settlement_tribe, SettlementTribe::Celtic, "Celtic (塞爾特)");
                    ui.selectable_value(&mut self.settlement_tribe, SettlementTribe::Hun, "Hun (匈人)");
                });
            ui.end_row();

            ui.label("Layout Symmetry:");
            egui::ComboBox::from_id_source("settlement_symmetry_select")
                .selected_text(format!("{:?}", self.settlement_symmetry))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.settlement_symmetry, SymmetryMode::RotationalSymmetry, "Rotational (環形對稱)");
                    ui.selectable_value(&mut self.settlement_symmetry, SymmetryMode::CentralSymmetry, "Central (中心點對稱)");
                    ui.selectable_value(&mut self.settlement_symmetry, SymmetryMode::TopologicalEquidistant, "Topological equidistant (拓撲等距)");
                });
            ui.end_row();

            ui.label("Seed:");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.settlement_seed));
                if ui.button("Randomize").clicked() {
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(1, |d| d.subsec_nanos());
                    self.settlement_seed = (nanos % 99_999) as i32 + 1;
                }
            });
            ui.end_row();

            ui.label("Include Nature Objects:");
            ui.checkbox(&mut self.settlement_include_nature, "Generate Forests & Stone Quarries");
            ui.end_row();

            ui.label("Spawn Radius Ratio:");
            ui.add(egui::Slider::new(&mut self.settlement_spawn_radius, 0.4..=0.75));
            ui.end_row();
        });

        ui.add_space(8.0);
        if ui.button("Generate Settlement Bases").clicked() {
            if let Err(e) = self.generate_settlement(dimension, heights, water_level) {
                self.status_message = format!("Generation error: {e}");
                self.status_is_error = true;
            }
        }

        if let Some(dist) = &self.last_distribution {
            ui.separator();
            let f = &dist.fairness_report;
            ui.colored_label(
                if f.is_balanced { Color32::LIGHT_GREEN } else { Color32::from_rgb(250, 180, 80) },
                format!(
                    "Generation Result: {} bases, {}",
                    dist.players.len(),
                    if f.is_balanced { "balanced" } else { "NOT balanced" }
                ),
            );
            ui.label(format!(
                "Inter-player distance avg/min/max: {:.1}/{:.1}/{:.1}; resource variance wood {:.2} stone {:.2} food {:.2}",
                f.average_inter_player_distance,
                f.min_inter_player_distance,
                f.max_inter_player_distance,
                f.wood_resource_variance,
                f.stone_resource_variance,
                f.food_resource_variance,
            ));

            if let Some(placement) = &self.last_placement_preset {
                ui.label(format!("Placement Preset Entries: {}", placement.entries.len()));
            }
            if let Some(nature) = &self.last_nature_preset {
                ui.label(format!("Nature Preset Entries: {}", nature.entries.len()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_ui_initial_state_and_dirty_guards() {
        let mut ui = EnvironmentUi::default();
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
    fn test_environment_ui_flora_generation() {
        let mut ui = EnvironmentUi::default();
        ui.flora_density = 0.5;
        ui.generate_flora_scatter(65, None, 0.0);
        assert!(!ui.generated_flora.is_empty());
        assert!(ui.is_dirty());
    }

    #[test]
    fn test_environment_ui_neutral_lair_addition() {
        let mut ui = EnvironmentUi::default();
        assert!(ui.add_neutral_lair().is_ok());
        assert_eq!(ui.generated_lairs.len(), 1);
        // Canonical definitions are FigTie animals; the binder rejects them and the panel must say so.
        assert!(ui.generated_lair_events.is_empty());
        assert!(ui.status_message.contains("No scenario events"));
        assert!(ui.is_dirty());
    }

    #[test]
    fn test_environment_ui_settlement_generation() {
        let mut ui = EnvironmentUi::default();
        ui.settlement_players = 2;
        assert!(ui.generate_settlement(129, None, 0.0).is_ok());
        assert!(ui.last_distribution.is_some());
        assert!(ui.last_placement_preset.is_some());
        assert!(ui.is_dirty());
    }
}
