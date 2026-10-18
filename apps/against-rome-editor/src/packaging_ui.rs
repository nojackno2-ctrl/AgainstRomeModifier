//! Packaging, Diff & Budget Profiler Studio UI.
//! Replicates C# MapEditor packaging & profiling tools and integrates `native/against-rome-packaging`.

use against_rome_packaging::{
    diff::{DiffObjectItem, MapDiffEngine, MapDiffReport, MapVersionSnapshot},
    exporter::{ModBundleExporter, ModExportOptions, ModExportResult},
    preflight::{MapExportPreflightChecker, MapPreflightReport},
    profiler::{MapBudgetLimits, MapBudgetProfiler, MapBudgetReport, MapBudgetSnapshot, MapHealthGrade},
};
use against_rome_scenario::model::Spawn;
use eframe::egui::{self, Color32, Vec2};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackagingTab {
    ModExporter,
    MapDiff,
    BudgetProfiler,
}

pub struct PackagingUi {
    pub open: bool,
    pub current_tab: PackagingTab,

    // --- Mod Exporter state ---
    pub map_dir_input: String,
    pub output_zip_input: String,
    pub package_id: String,
    pub package_author: String,
    pub package_version: String,
    pub clean_caches: bool,
    pub generate_thumbnail: bool,
    pub thumbnail_resolution: usize,
    pub last_export_result: Option<ModExportResult>,
    pub last_preflight_report: Option<MapPreflightReport>,

    // --- Map Diff state ---
    pub baseline_snapshot: Option<MapVersionSnapshot>,
    pub current_snapshot: Option<MapVersionSnapshot>,
    pub diff_report: Option<MapDiffReport>,
    pub diff_summary_text: String,

    // --- Budget Profiler state ---
    pub budget_report: Option<MapBudgetReport>,

    // Dirty / guards
    pub dirty: bool,
    pub pending_discard: bool,
    pub status_message: String,
    pub status_is_error: bool,
}

impl Default for PackagingUi {
    fn default() -> Self {
        Self {
            open: false,
            current_tab: PackagingTab::ModExporter,
            map_dir_input: String::new(),
            output_zip_input: String::new(),
            package_id: "custom-map-mod".to_string(),
            package_author: "Map Designer".to_string(),
            package_version: "1.0.0".to_string(),
            clean_caches: true,
            generate_thumbnail: true,
            thumbnail_resolution: 512,
            last_export_result: None,
            last_preflight_report: None,
            baseline_snapshot: None,
            current_snapshot: None,
            diff_report: None,
            diff_summary_text: String::new(),
            budget_report: None,
            dirty: false,
            pending_discard: false,
            status_message: "Ready.".to_string(),
            status_is_error: false,
        }
    }
}

impl PackagingUi {
    pub fn is_dirty(&self) -> bool {
        self.dirty || self.diff_report.is_some() || self.budget_report.is_some()
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
        self.diff_report = None;
        self.diff_summary_text.clear();
        self.budget_report = None;
        self.pending_discard = false;
    }

    pub fn request_close(&mut self) {
        if self.is_dirty() {
            self.pending_discard = true;
        } else {
            self.open = false;
        }
    }

    pub fn run_preflight_check(&mut self) {
        let dir = Path::new(self.map_dir_input.trim());
        if !dir.is_dir() {
            self.status_message = format!("Invalid map directory: {}", dir.display());
            self.status_is_error = true;
            return;
        }

        let report = match MapExportPreflightChecker::check(dir, None, None) {
            Ok(report) => report,
            Err(e) => {
                self.status_message = format!("Preflight failed: {e}");
                self.status_is_error = true;
                return;
            }
        };
        let error_count = report.error_count();
        let warn_count = report.warning_count();
        self.status_message = format!(
            "Preflight completed: {} errors, {} warnings.",
            error_count, warn_count
        );
        self.status_is_error = error_count > 0;
        self.last_preflight_report = Some(report);
    }

    pub fn run_export(&mut self) -> Result<(), String> {
        let dir = Path::new(self.map_dir_input.trim());
        let zip = Path::new(self.output_zip_input.trim());

        if !dir.is_dir() {
            return Err(format!("Invalid map directory: {}", dir.display()));
        }
        if zip.as_os_str().is_empty() {
            return Err("Please specify output ZIP file path.".into());
        }

        let options = ModExportOptions {
            package_id: self.package_id.clone(),
            author: Some(self.package_author.clone()),
            version: Some(self.package_version.clone()),
            clean_caches: self.clean_caches,
            force_export_on_errors: false,
            generate_thumbnail: self.generate_thumbnail,
            thumbnail_resolution: self.thumbnail_resolution,
        };

        match ModBundleExporter::export_to_zip(dir, zip, Some(&options)) {
            Ok(result) => {
                self.status_message = format!(
                    "Bundle exported successfully: {} ({} files, {} bytes)",
                    result.output_zip_path, result.file_count, result.total_size_bytes
                );
                self.status_is_error = false;
                self.last_export_result = Some(result);
                self.dirty = true;
                Ok(())
            }
            Err(e) => {
                self.status_message = format!("Export failed: {e}");
                self.status_is_error = true;
                Err(e.to_string())
            }
        }
    }

    pub fn capture_baseline_from_memory(
        &mut self,
        label: &str,
        dimension: usize,
        heights: Option<&[u8]>,
        collision: Option<&[u8]>,
        objects: Option<&[DiffObjectItem]>,
    ) {
        let snap = MapVersionSnapshot {
            version_label: label.to_string(),
            tile_dimension: dimension.saturating_sub(1),
            height_dimension: dimension,
            heights: heights.map(|h| h.to_vec()),
            height_step: 4.0,
            textures: None,
            collision_dimension: dimension.saturating_sub(1),
            collision: collision.map(|c| c.to_vec()),
            objects: objects.map(|o| o.to_vec()),
            events: None,
        };
        self.baseline_snapshot = Some(snap);
        self.status_message = format!("Captured baseline snapshot '{}'.", label);
        self.status_is_error = false;
    }

    pub fn compare_snapshots(
        &mut self,
        current_label: &str,
        dimension: usize,
        heights: Option<&[u8]>,
        collision: Option<&[u8]>,
        objects: Option<&[DiffObjectItem]>,
    ) -> Result<(), String> {
        let Some(base) = &self.baseline_snapshot else {
            return Err("No baseline snapshot captured. Capture a baseline first.".into());
        };

        let curr = MapVersionSnapshot {
            version_label: current_label.to_string(),
            tile_dimension: dimension.saturating_sub(1),
            height_dimension: dimension,
            heights: heights.map(|h| h.to_vec()),
            height_step: 4.0,
            textures: None,
            collision_dimension: dimension.saturating_sub(1),
            collision: collision.map(|c| c.to_vec()),
            objects: objects.map(|o| o.to_vec()),
            events: None,
        };

        let report = MapDiffEngine::compare(base, &curr, None);
        self.diff_summary_text = report.generate_summary_text();
        self.current_snapshot = Some(curr);
        self.diff_report = Some(report);
        self.dirty = true;
        self.status_message = "Diff report generated successfully.".to_string();
        self.status_is_error = false;
        Ok(())
    }

    pub fn profile_budget(
        &mut self,
        dimension: usize,
        heights: Option<&[u8]>,
        collision: Option<&[u8]>,
        spawns: Option<&[Spawn]>,
    ) {
        let snapshot = MapBudgetSnapshot {
            scene_objects: None,
            spawns: spawns.map(|s| s.to_vec()),
            lights: None,
            collision_size: dimension.saturating_sub(1),
            collision: collision.map(|c| c.to_vec()),
            height_size: dimension,
            heights: heights.map(|h| h.to_vec()),
            height_step: 4.0,
            water_level: 0.0,
            decoded_sprite_count: 0,
            decoded_shadow_count: 0,
            decoded_sprite_types: 0,
        };

        let report = MapBudgetProfiler::analyze(&snapshot);
        self.status_message = format!(
            "Budget analysis finished: Health Grade {:?} (Score: {}/100, Bottlenecks: {})",
            report.overall_grade,
            report.overall_score,
            report.bottlenecks.len()
        );
        self.status_is_error = false;
        self.budget_report = Some(report);
        self.dirty = true;
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        dimension: usize,
        heights: Option<&[u8]>,
        collision: Option<&[u8]>,
        spawns: Option<&[Spawn]>,
    ) {
        if !self.open {
            return;
        }

        let mut is_open = self.open;
        egui::Window::new("Packaging, Diff & Budget Profiler")
            .open(&mut is_open)
            .min_size(Vec2::new(740.0, 540.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.current_tab, PackagingTab::ModExporter, "Mod Exporter");
                    ui.selectable_value(&mut self.current_tab, PackagingTab::MapDiff, "Map Diff");
                    ui.selectable_value(&mut self.current_tab, PackagingTab::BudgetProfiler, "Budget Profiler");
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
                    PackagingTab::ModExporter => self.render_exporter_tab(ui),
                    PackagingTab::MapDiff => self.render_diff_tab(ui, dimension, heights, collision),
                    PackagingTab::BudgetProfiler => self.render_budget_tab(ui, dimension, heights, collision, spawns),
                }
            });

        if !is_open {
            self.request_close();
        }

        if self.pending_discard {
            let mut close_modal = false;
            let mut confirm = false;

            egui::Window::new("Discard Diff & Profiling State?")
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Closing will clear currently calculated diff comparisons and budget reports.");
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

    fn render_exporter_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Mod Bundle Exporter");
        ui.label("Packs editable map folders into verified Against Rome distribution ZIP bundles with manifests.");

        ui.add_space(6.0);
        egui::Grid::new("exporter_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("Source Map Folder:");
            ui.text_edit_singleline(&mut self.map_dir_input);
            ui.end_row();

            ui.label("Output ZIP Path:");
            ui.text_edit_singleline(&mut self.output_zip_input);
            ui.end_row();

            ui.label("Package ID:");
            ui.text_edit_singleline(&mut self.package_id);
            ui.end_row();

            ui.label("Author Name:");
            ui.text_edit_singleline(&mut self.package_author);
            ui.end_row();

            ui.label("Mod Version:");
            ui.text_edit_singleline(&mut self.package_version);
            ui.end_row();

            ui.label("Clean Intermediate Caches:");
            ui.checkbox(&mut self.clean_caches, "");
            ui.end_row();

            ui.label("Generate Minimap Thumbnail:");
            ui.checkbox(&mut self.generate_thumbnail, "");
            ui.end_row();
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Preflight Verification").clicked() {
                self.run_preflight_check();
            }
            if ui.button("Export Mod ZIP Bundle").clicked() {
                let _ = self.run_export();
            }
        });

        if let Some(pre) = &self.last_preflight_report {
            ui.separator();
            ui.label(format!("Preflight Issues: {} (Errors: {}, Warnings: {})", pre.issues.len(), pre.error_count(), pre.warning_count()));
            egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                for issue in &pre.issues {
                    let color = match issue.severity {
                        against_rome_packaging::preflight::PreflightSeverity::Error => Color32::from_rgb(255, 90, 90),
                        against_rome_packaging::preflight::PreflightSeverity::Warning => Color32::from_rgb(255, 200, 70),
                        against_rome_packaging::preflight::PreflightSeverity::Info => Color32::LIGHT_GRAY,
                    };
                    ui.colored_label(color, format!("[{:?}] {}: {}", issue.category, issue.code, issue.english));
                }
            });
        }
    }

    fn render_diff_tab(
        &mut self,
        ui: &mut egui::Ui,
        dimension: usize,
        heights: Option<&[u8]>,
        collision: Option<&[u8]>,
    ) {
        ui.heading("Map Version Diff Engine");
        ui.label("Compares working memory edits against a baseline snapshot across height, collision, objects, and scripts.");

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Capture Current as Baseline").clicked() {
                self.capture_baseline_from_memory("Working Baseline", dimension, heights, collision, None);
            }
            if ui.button("Compare Working vs Baseline").clicked() {
                let _ = self.compare_snapshots("Latest Edits", dimension, heights, collision, None);
            }
        });

        ui.separator();
        if !self.diff_summary_text.is_empty() {
            egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                ui.monospace(&self.diff_summary_text);
            });
        } else {
            ui.label("No diff report available. Capture a baseline first and edit terrain to see differences.");
        }
    }

    fn render_budget_tab(
        &mut self,
        ui: &mut egui::Ui,
        dimension: usize,
        heights: Option<&[u8]>,
        collision: Option<&[u8]>,
        spawns: Option<&[Spawn]>,
    ) {
        ui.heading("Map Budget & Performance Profiler");
        ui.label("Analyzes object slots, light cluster hotspots, draw calls, and pathfinding complexity.");

        ui.add_space(6.0);
        if ui.button("Run Budget Profiler").clicked() {
            self.profile_budget(dimension, heights, collision, spawns);
        }

        if let Some(report) = &self.budget_report {
            ui.separator();
            let grade_color = match report.overall_grade {
                MapHealthGrade::Healthy => Color32::LIGHT_GREEN,
                MapHealthGrade::Caution => Color32::from_rgb(255, 210, 70),
                MapHealthGrade::Warning => Color32::from_rgb(255, 140, 50),
                MapHealthGrade::Critical | MapHealthGrade::EngineExceeded => Color32::from_rgb(255, 70, 70),
            };

            ui.horizontal(|ui| {
                ui.heading("Overall Grade:");
                ui.colored_label(grade_color, format!("{:?} ({}/100)", report.overall_grade, report.overall_score));
            });

            ui.add_space(4.0);
            ui.label(format!("Object Slots: {} active (Max: {})", report.object_budget.total_active_objects, MapBudgetLimits::MAX_OBJECTS_DAT_SLOTS));
            ui.label(format!("Light Clusters: {} hotspots (peak {} lights/cluster, limit {})", report.light_budget.hotspots.len(), report.light_budget.peak_cluster_lights, MapBudgetLimits::HARD_LIMIT_LIGHTS_PER_CLUSTER));
            ui.label(format!("Estimated Draw Calls: {} (terrain {}, sprites {}, shadows {})", report.draw_budget.estimated_total_draw_calls, report.draw_budget.terrain_draw_calls, report.draw_budget.sprite_batch_draw_calls, report.draw_budget.shadow_batch_draw_calls));
            ui.label(format!("Pathfinding: {} chokepoints, {} dead ends ({})", report.path_budget.chokepoint_count, report.path_budget.dead_end_count, report.path_budget.complexity_rating));

            if !report.bottlenecks.is_empty() {
                ui.separator();
                ui.heading("Detected Bottlenecks:");
                for b in &report.bottlenecks {
                    ui.colored_label(Color32::from_rgb(255, 180, 50), format!("• [{}] {}: {}", b.category, b.code, b.english_description));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packaging_ui_initial_state_and_dirty_guards() {
        let mut ui = PackagingUi::default();
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
    fn test_packaging_ui_snapshot_diff() {
        let mut ui = PackagingUi::default();
        let heights_a = vec![32u8; 16 * 16];
        let mut heights_b = vec![32u8; 16 * 16];
        heights_b[0] = 50; // introduce difference

        ui.capture_baseline_from_memory("V1", 16, Some(&heights_a), None, None);
        assert!(ui.baseline_snapshot.is_some());

        assert!(ui.compare_snapshots("V2", 16, Some(&heights_b), None, None).is_ok());
        assert!(ui.diff_report.is_some());
        assert!(ui.diff_report.as_ref().unwrap().has_differences());
        assert!(ui.is_dirty());
    }

    #[test]
    fn test_packaging_ui_budget_profiler() {
        let mut ui = PackagingUi::default();
        let heights = vec![32u8; 16 * 16];
        let mut spawns = Vec::new();
        let mut s = Spawn::default();
        s.alias = "FigGerInf00".to_string();
        spawns.push(s);

        ui.profile_budget(16, Some(&heights), None, Some(&spawns));
        assert!(ui.budget_report.is_some());
        assert!(ui.is_dirty());
    }
}
