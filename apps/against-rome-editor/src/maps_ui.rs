//! Map Browser, Catalog, Cloner, Deleter, Design Validator, Diagnostics, and Preflight UI.
//! Replicates C# `MapSelectionForm.cs` and integrates `native/against-rome-maps`.

use against_rome_maps::{
    catalog::{EndlessMapCatalog, EndlessMapInfo, GameMapCatalog, GameMapInfo},
    cloner::EndlessMapCloner,
    custom_map::{CustomMapAccess, CustomMapEntry},
    deleter::EndlessMapDeleter,
    design_validator::{DesignIssueSeverity, MapDesignIssue, MapDesignValidationRequest, MapDesignValidator},
    diagnostics::{MapCheckObject, MapCheckSnapshot, MapDiagnostics, MapIssue, MapIssueSeverity},
    preflight::ScenarioSavePreflight,
};
use against_rome_scenario::model::{Event, ScenarioDocument, Spawn};
use eframe::egui::{self, Color32, Vec2};
use std::path::{Path, PathBuf};

/// Read-only view of the live editor state handed to the panel every frame.
#[derive(Clone, Copy, Default)]
pub struct MapsContext<'a> {
    pub scenario: Option<&'a ScenarioDocument>,
    pub aliases: &'a [String],
    pub heights: Option<&'a [u8]>,
    /// Height grid vertex count; the collision layer is `height_size - 1` square.
    pub height_size: usize,
    pub collision: Option<&'a [u8]>,
    pub height_step: f32,
    pub water_level: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapsTab {
    Catalog,
    ClonerDeleter,
    DesignValidator,
    Diagnostics,
    Preflight,
}

#[derive(Clone, Debug)]
pub enum MapsPendingAction {
    DeleteCustom(i32),
    DiscardAndClose,
}

pub struct MapsUi {
    pub open: bool,
    pub current_tab: MapsTab,
    pub game_path_input: String,
    pub endless_maps: Vec<EndlessMapInfo>,
    pub general_maps: Vec<GameMapInfo>,
    pub selected_map_id: Option<String>,
    pub selected_map_path: Option<PathBuf>,
    pub selected_is_custom: bool,

    // Cloner inputs
    pub clone_source_slot: i32,
    pub clone_target_slot: i32,
    pub clone_new_name: String,

    // Deleter confirmation
    pub delete_confirm_slot: Option<i32>,

    // Validation & Diagnostics results
    pub design_issues: Vec<MapDesignIssue>,
    pub diagnostic_issues: Vec<MapIssue>,
    pub preflight_result: Option<Result<String, String>>,

    // Pending confirmation & dirty state
    pub pending_action: Option<MapsPendingAction>,
    pub status_message: String,
    pub status_is_error: bool,
    pub dirty: bool,
}

impl Default for MapsUi {
    fn default() -> Self {
        Self {
            open: false,
            current_tab: MapsTab::Catalog,
            game_path_input: String::new(),
            endless_maps: Vec::new(),
            general_maps: Vec::new(),
            selected_map_id: None,
            selected_map_path: None,
            selected_is_custom: false,
            clone_source_slot: 0,
            clone_target_slot: 5,
            clone_new_name: "Custom Map".to_string(),
            delete_confirm_slot: None,
            design_issues: Vec::new(),
            diagnostic_issues: Vec::new(),
            preflight_result: None,
            pending_action: None,
            status_message: "Ready.".to_string(),
            status_is_error: false,
            dirty: false,
        }
    }
}

impl MapsUi {
    pub fn is_dirty(&self) -> bool {
        self.dirty || self.pending_action.is_some()
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
        self.pending_action = None;
    }

    pub fn request_close(&mut self) {
        if self.is_dirty() {
            self.pending_action = Some(MapsPendingAction::DiscardAndClose);
        } else {
            self.open = false;
        }
    }

    pub fn refresh_catalog(&mut self) {
        let path = Path::new(self.game_path_input.trim());
        if !path.exists() {
            self.status_message = format!("Game path does not exist: {}", path.display());
            self.status_is_error = true;
            return;
        }

        match EndlessMapCatalog::new().list(path) {
            Ok(maps) => {
                self.endless_maps = maps;
                self.status_message = format!("Discovered {} endless maps.", self.endless_maps.len());
                self.status_is_error = false;
            }
            Err(e) => {
                self.status_message = format!("Failed to list endless maps: {e}");
                self.status_is_error = true;
            }
        }

        match GameMapCatalog::new().list(path) {
            Ok(maps) => {
                self.general_maps = maps;
            }
            Err(e) => {
                self.status_message = format!("Failed to list general maps: {e}");
                self.status_is_error = true;
            }
        }
    }

    pub fn execute_clone(&mut self) -> Result<EndlessMapInfo, String> {
        let game_path = Path::new(self.game_path_input.trim());
        if !game_path.exists() {
            return Err("Game path does not exist.".into());
        }

        let cloner = EndlessMapCloner::new();
        let result = cloner.clone_map(
            game_path,
            self.clone_source_slot,
            self.clone_target_slot,
            &self.clone_new_name,
        )?;

        self.dirty = true;
        self.refresh_catalog();
        self.status_message = format!(
            "Cloned map to {} successfully! ({})",
            result.id(),
            result.display_name.as_deref().unwrap_or("Untitled")
        );
        self.status_is_error = false;
        Ok(result)
    }

    pub fn execute_delete(&mut self, slot: i32) -> Result<(), String> {
        let game_path = Path::new(self.game_path_input.trim());
        if !game_path.exists() {
            return Err("Game path does not exist.".into());
        }

        let deleter = EndlessMapDeleter::new();
        deleter.delete(game_path, slot)?;

        self.dirty = true;
        self.delete_confirm_slot = None;
        self.refresh_catalog();
        self.status_message = format!("Successfully deleted custom map ENDL_{:03}.", slot);
        self.status_is_error = false;
        Ok(())
    }

    pub fn run_design_validator(
        &mut self,
        objects: &[Spawn],
        events: &[Event],
        heights: Option<&[u8]>,
        height_size: usize,
        collision: Option<&[u8]>,
        collision_size: usize,
        water_level: f32,
    ) {
        let req = MapDesignValidationRequest {
            objects: objects.to_vec(),
            events: events.to_vec(),
            collision: collision.map(|c| c.to_vec()),
            collision_size,
            heights: heights.map(|h| h.to_vec()),
            height_size,
            height_step: 4.0,
            water_level,
            map_dimension: 16384.0,
        };

        self.design_issues = MapDesignValidator::validate(&req);
        self.status_message = format!(
            "Design Validator completed: {} issues found.",
            self.design_issues.len()
        );
        self.status_is_error = false;
    }

    pub fn run_diagnostics(&mut self, cx: &MapsContext<'_>) {
        let Some(scenario) = cx.scenario else {
            self.status_message = "No scenario loaded for diagnostics.".to_string();
            self.status_is_error = true;
            return;
        };
        // Spawns without a troop count are treated as buildings; no template data is available here,
        // so templates are assumed complete rather than raising false alarms.
        let objects = scenario
            .spawns
            .iter()
            .map(|s| MapCheckObject::new(s.clone(), s.count == 0, true))
            .collect();
        let collision_size = cx.height_size.saturating_sub(1);
        let mut snapshot = MapCheckSnapshot::new(
            objects,
            scenario.events.clone(),
            cx.aliases.to_vec(),
            collision_size,
            cx.collision.map(<[u8]>::to_vec),
        );
        snapshot.height_size = cx.height_size;
        snapshot.heights = cx.heights.map(<[u8]>::to_vec);
        snapshot.height_step = cx.height_step;
        snapshot.water_level = cx.water_level;
        self.diagnostic_issues = MapDiagnostics::check(&snapshot);
        self.status_message = format!(
            "Diagnostics completed: {} issues found.",
            self.diagnostic_issues.len()
        );
        self.status_is_error = false;
    }

    pub fn run_preflight(
        &mut self,
        scenario: &ScenarioDocument,
        aliases: &[String],
    ) {
        match ScenarioSavePreflight::validate(scenario, aliases) {
            Ok(()) => {
                self.preflight_result = Some(Ok("Preflight verification passed! Safe to export.".to_string()));
                self.status_message = "Preflight verification passed.".to_string();
                self.status_is_error = false;
            }
            Err(e) => {
                self.preflight_result = Some(Err(e.clone()));
                self.status_message = format!("Preflight blocked: {e}");
                self.status_is_error = true;
            }
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, cx: &MapsContext<'_>, mut on_load_requested: Option<&mut dyn FnMut(PathBuf)>) {
        if !self.open {
            return;
        }

        let mut is_open = self.open;
        egui::Window::new("Maps & Diagnostics Studio")
            .open(&mut is_open)
            .min_size(Vec2::new(720.0, 520.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.current_tab, MapsTab::Catalog, "Map Catalog");
                    ui.selectable_value(&mut self.current_tab, MapsTab::ClonerDeleter, "Clone / Delete");
                    ui.selectable_value(&mut self.current_tab, MapsTab::DesignValidator, "Design Validator");
                    ui.selectable_value(&mut self.current_tab, MapsTab::Diagnostics, "Diagnostics");
                    ui.selectable_value(&mut self.current_tab, MapsTab::Preflight, "Save Preflight");
                });
                ui.separator();

                // Status banner
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
                    MapsTab::Catalog => self.render_catalog_tab(ui, &mut on_load_requested),
                    MapsTab::ClonerDeleter => self.render_cloner_tab(ui),
                    MapsTab::DesignValidator => self.render_validator_tab(ui, cx),
                    MapsTab::Diagnostics => self.render_diagnostics_tab(ui, cx),
                    MapsTab::Preflight => self.render_preflight_tab(ui, cx),
                }
            });

        if !is_open {
            self.request_close();
        }

        // Pending discard confirmation modal
        if let Some(pending) = &self.pending_action {
            let mut close_modal = false;
            let mut confirm = false;
            let action_copy = pending.clone();

            egui::Window::new("Confirm Action")
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    match action_copy {
                        MapsPendingAction::DeleteCustom(slot) => {
                            ui.label(format!("Are you sure you want to permanently delete custom map ENDL_{:03}?", slot));
                        }
                        MapsPendingAction::DiscardAndClose => {
                            ui.label("You have uncommitted map changes. Discard and close?");
                        }
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Confirm").clicked() {
                            confirm = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_modal = true;
                        }
                    });
                });

            if confirm {
                match action_copy {
                    MapsPendingAction::DeleteCustom(slot) => {
                        let _ = self.execute_delete(slot);
                    }
                    MapsPendingAction::DiscardAndClose => {
                        self.clear_dirty();
                        self.open = false;
                    }
                }
                self.pending_action = None;
            } else if close_modal {
                self.pending_action = None;
            }
        }
    }

    fn render_catalog_tab(&mut self, ui: &mut egui::Ui, on_load_requested: &mut Option<&mut dyn FnMut(PathBuf)>) {
        ui.horizontal(|ui| {
            ui.label("Game Root Path:");
            ui.text_edit_singleline(&mut self.game_path_input);
            if ui.button("Refresh Catalog").clicked() {
                self.refresh_catalog();
            }
        });

        ui.separator();

        egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
            ui.heading("Endless Maps");
            if self.endless_maps.is_empty() {
                ui.label("No endless maps found or catalog not yet scanned.");
            } else {
                for map in &self.endless_maps {
                    let is_selected = self.selected_map_id.as_deref() == Some(&map.id());
                    let label = format!(
                        "[{}] {} ({}) - {}",
                        if map.is_custom { "Custom" } else { "Retail" },
                        map.id(),
                        map.display_name.as_deref().unwrap_or("Unnamed"),
                        map.directory_path.display()
                    );
                    if ui.selectable_label(is_selected, label).clicked() {
                        self.selected_map_id = Some(map.id());
                        self.selected_map_path = Some(map.directory_path.clone());
                        self.selected_is_custom = map.is_custom;
                        self.clone_source_slot = map.slot;
                    }
                }
            }

            ui.separator();
            ui.heading("General Game Maps");
            if self.general_maps.is_empty() {
                ui.label("No general maps catalogued.");
            } else {
                for map in &self.general_maps {
                    let is_selected = self.selected_map_id.as_deref() == Some(&map.id);
                    let label = format!(
                        "[{}] {} - {}",
                        if map.is_custom { "Custom" } else { "Retail" },
                        map.id,
                        map.directory_path.display()
                    );
                    if ui.selectable_label(is_selected, label).clicked() {
                        self.selected_map_id = Some(map.id.clone());
                        self.selected_map_path = Some(map.directory_path.clone());
                        self.selected_is_custom = map.is_custom;
                    }
                }
            }
        });

        ui.separator();
        ui.horizontal(|ui| {
            if let Some(path) = &self.selected_map_path {
                ui.label(format!("Selected: {}", path.display()));
                if ui.button("Load Map into Editor").clicked() {
                    if let Some(cb) = on_load_requested.as_deref_mut() {
                        cb(path.clone());
                    }
                }
            } else {
                ui.label("Select a map from the catalog to inspect or load.");
            }
        });
    }

    fn render_cloner_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Map Cloner & Safe Deleter");
        ui.label("Duplicate any retail or custom map into an isolated custom slot (5..999).");

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Source Slot (ENDL_###):");
            ui.add(egui::DragValue::new(&mut self.clone_source_slot).clamp_range(0..=999));
        });

        ui.horizontal(|ui| {
            ui.label("Target Slot (5..=999):");
            ui.add(egui::DragValue::new(&mut self.clone_target_slot).clamp_range(5..=999));
        });

        ui.horizontal(|ui| {
            ui.label("New Map Name:");
            ui.text_edit_singleline(&mut self.clone_new_name);
        });

        if ui.button("Clone Map").clicked() {
            match self.execute_clone() {
                Ok(_) => {}
                Err(e) => {
                    self.status_message = format!("Clone failed: {e}");
                    self.status_is_error = true;
                }
            }
        }

        ui.separator();
        ui.heading("Custom Map Deleter");
        ui.label("Only custom maps (slots 5..999) can be deleted. Retail maps are strictly guarded.");

        if let Some(slot) = self.selected_map_id.as_deref().and_then(|id| {
            if id.starts_with("ENDL_") {
                id[5..].parse::<i32>().ok()
            } else {
                None
            }
        }) {
            if slot >= 5 && self.selected_is_custom {
                if ui.button(format!("Delete Custom Map ENDL_{:03}", slot)).clicked() {
                    self.pending_action = Some(MapsPendingAction::DeleteCustom(slot));
                }
            } else {
                ui.label("Current selected map is retail or non-custom: Deletion disabled.");
            }
        } else {
            ui.label("Select a custom map in the catalog to delete.");
        }
    }

    fn render_validator_tab(&mut self, ui: &mut egui::Ui, cx: &MapsContext<'_>) {
        ui.heading("Design Validator");
        ui.label("Static gameplay sanity rules: bounds, water/blocked spawn, main house & unit presence.");
        if ui.button("Run Design Validator on current map").clicked() {
            match cx.scenario {
                Some(sc) => self.run_design_validator(
                    &sc.spawns,
                    &sc.events,
                    cx.heights,
                    cx.height_size,
                    cx.collision,
                    cx.height_size.saturating_sub(1),
                    cx.water_level,
                ),
                None => {
                    self.status_message = "No scenario loaded.".to_string();
                    self.status_is_error = true;
                }
            }
        }

        egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
            if self.design_issues.is_empty() {
                ui.colored_label(Color32::LIGHT_GREEN, "No design issues found or validator has not been run yet.");
            } else {
                for (idx, issue) in self.design_issues.iter().enumerate() {
                    let color = match issue.severity {
                        DesignIssueSeverity::Error => Color32::from_rgb(255, 90, 90),
                        DesignIssueSeverity::Warning => Color32::from_rgb(255, 200, 70),
                    };
                    ui.horizontal(|ui| {
                        ui.colored_label(color, format!("[{:?}]", issue.severity));
                        ui.label(format!("{}. [{}] {}", idx + 1, issue.code, issue.chinese));
                        if let (Some(x), Some(z)) = (issue.world_x, issue.world_z) {
                            ui.label(format!("(Pos: {:.1}, {:.1})", x, z));
                        }
                    });
                }
            }
        });
    }

    fn render_diagnostics_tab(&mut self, ui: &mut egui::Ui, cx: &MapsContext<'_>) {
        ui.heading("Map Diagnostics");
        ui.label("Checks isolated passability regions, connectivity chokepoints, road gaps, and event issues.");
        if ui.button("Run Diagnostics on current map").clicked() {
            self.run_diagnostics(cx);
        }

        egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
            if self.diagnostic_issues.is_empty() {
                ui.colored_label(Color32::LIGHT_GREEN, "No diagnostic issues found.");
            } else {
                for (idx, issue) in self.diagnostic_issues.iter().enumerate() {
                    let color = match issue.severity {
                        MapIssueSeverity::Error => Color32::from_rgb(255, 90, 90),
                        MapIssueSeverity::Warning => Color32::from_rgb(255, 200, 70),
                    };
                    ui.horizontal(|ui| {
                        ui.colored_label(color, format!("[{:?}]", issue.severity));
                        ui.label(format!("{}. [{}] {}", idx + 1, issue.code, issue.chinese));
                    });
                }
            }
        });
    }

    fn render_preflight_tab(&mut self, ui: &mut egui::Ui, cx: &MapsContext<'_>) {
        ui.heading("Scenario Save Preflight");
        ui.label("Pre-save contract checks for BCI bytecode bounds, scenario UUID consistency, and actions.");
        if ui.button("Run Save Preflight on current scenario").clicked() {
            match cx.scenario {
                Some(sc) => self.run_preflight(sc, cx.aliases),
                None => {
                    self.status_message = "No scenario loaded.".to_string();
                    self.status_is_error = true;
                }
            }
        }

        if let Some(res) = &self.preflight_result {
            match res {
                Ok(msg) => {
                    ui.colored_label(Color32::LIGHT_GREEN, format!("Success: {msg}"));
                }
                Err(err) => {
                    ui.colored_label(Color32::from_rgb(255, 90, 90), format!("Validation Error: {err}"));
                }
            }
        } else {
            ui.label("Preflight check has not been run for the current scenario document.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maps_ui_initial_state_and_dirty_lifecycle() {
        let mut ui = MapsUi::default();
        assert!(!ui.open);
        assert!(!ui.is_dirty());

        ui.mark_dirty();
        assert!(ui.is_dirty());

        ui.request_close();
        assert!(ui.pending_action.is_some());

        ui.clear_dirty();
        assert!(!ui.is_dirty());
        assert!(ui.pending_action.is_none());
    }

    #[test]
    fn test_maps_ui_validator_detects_out_of_bounds() {
        let mut ui = MapsUi::default();
        let mut spawn = Spawn::default();
        spawn.x = 99999.0; // out of map bounds
        spawn.z = 100.0;

        ui.run_design_validator(&[spawn], &[], None, 0, None, 0, 0.0);
        assert!(!ui.design_issues.is_empty());
        assert!(ui.design_issues.iter().any(|i| i.code == "bounds"));
    }

    #[test]
    fn test_maps_ui_preflight_passes_valid_scenario() {
        let mut ui = MapsUi::default();
        let mut scenario = ScenarioDocument::default();
        let mut spawn = Spawn::default();
        spawn.id = uuid::Uuid::new_v4();
        spawn.alias = "GER_INF00".to_string();
        spawn.x = 1000.0;
        spawn.z = 1000.0;
        spawn.count = 5;
        spawn.team = 0;
        scenario.spawns.push(spawn);

        let aliases = vec!["GER_INF00".to_string()];
        ui.run_preflight(&scenario, &aliases);
        assert!(ui.preflight_result.as_ref().unwrap().is_ok());
    }
}
