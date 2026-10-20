//! Packaging, Diff & Budget Profiler Studio UI.
//! Replicates C# MapEditor packaging & profiling tools and integrates `native/against-rome-packaging`.

use against_rome_packaging::{
    diff::{DiffObjectItem, MapDiffEngine, MapDiffReport, MapVersionSnapshot},
    exporter::{ModBundleExporter, ModExportOptions, ModExportResult},
    preflight::{MapExportPreflightChecker, MapPreflightReport},
    profiler::{
        MapBudgetLimits, MapBudgetProfiler, MapBudgetReport, MapBudgetSnapshot, MapHealthGrade,
    },
};
use against_rome_scenario::model::{ScenarioDocument, Spawn};
use eframe::egui::{self, Color32, Vec2};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackagingTab {
    ModExporter,
    MapDiff,
    BudgetProfiler,
}

/// Diff layers in report order.
pub const DIFF_LAYERS: [&str; 8] = [
    "Heightmap",
    "Collision",
    "Textures",
    "Objects",
    "Scenario events",
    "Object alias/unit count",
    "Scenario spawns",
    "Scenario data slots",
];

/// Per-layer availability of one snapshot: `None` = captured, `Some(reason)` = unavailable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LayerAvailability {
    pub height: Option<String>,
    pub collision: Option<String>,
    pub textures: Option<String>,
    pub objects: Option<String>,
    pub events: Option<String>,
    pub object_metadata: Option<String>,
    pub spawns: Option<String>,
    pub data_slots: Option<String>,
}

impl LayerAvailability {
    fn slots(&self) -> [&Option<String>; 8] {
        [
            &self.height,
            &self.collision,
            &self.textures,
            &self.objects,
            &self.events,
            &self.object_metadata,
            &self.spawns,
            &self.data_slots,
        ]
    }
}

/// Outcome of one layer in the last comparison.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayerState {
    Compared,
    Unavailable(String),
}

/// Overall verdict; `Incomplete` is never reported as identical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffVerdict {
    Differences,
    Identical,
    /// No differences in the compared layers, but at least one layer could not be compared.
    Incomplete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BaselineSource {
    Memory,
    SavedMap(PathBuf),
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
    /// Core terrain/object/event report. Scenario record differences also contribute to
    /// `diff_verdict` and `diff_summary_text`, since MapDiffReport has no spawn/binding fields.
    pub diff_report: Option<MapDiffReport>,
    pub diff_summary_text: String,
    pub baseline_source: Option<BaselineSource>,
    pub baseline_availability: LayerAvailability,
    pub baseline_dir_input: String,
    pub layer_coverage: Vec<(&'static str, LayerState)>,
    pub diff_verdict: Option<DiffVerdict>,
    // Frozen with the baseline; never re-read it during comparison.
    baseline_scenario: Option<ScenarioDocument>,

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
            baseline_source: None,
            baseline_availability: LayerAvailability::default(),
            baseline_dir_input: String::new(),
            layer_coverage: Vec::new(),
            diff_verdict: None,
            baseline_scenario: None,
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
        self.layer_coverage.clear();
        self.diff_verdict = None;
        self.budget_report = None;
        self.pending_discard = false;
    }

    /// Resets all map-bound packaging reports, export results, preflight findings, diff snapshots,
    /// and updates the map directory path.
    /// Preserves user preferences (open state, current tab, package metadata, clean_caches, thumbnail settings).
    pub fn reset_session(&mut self, map_path: Option<&Path>) {
        self.dirty = false;
        self.pending_discard = false;
        self.map_dir_input = map_path
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        self.last_export_result = None;
        self.last_preflight_report = None;
        self.baseline_snapshot = None;
        self.baseline_source = None;
        self.baseline_scenario = None;
        self.baseline_availability = LayerAvailability::default();
        self.baseline_dir_input.clear();
        self.current_snapshot = None;
        self.diff_report = None;
        self.diff_summary_text.clear();
        self.layer_coverage.clear();
        self.diff_verdict = None;
        self.budget_report = None;
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
        let (snap, availability) = memory_snapshot(label, dimension, heights, collision, objects);
        self.baseline_snapshot = Some(snap);
        self.baseline_availability = availability;
        self.baseline_source = Some(BaselineSource::Memory);
        self.baseline_scenario = None;
        self.status_message = format!("Captured baseline snapshot '{}'.", label);
        self.status_is_error = false;
    }

    /// Loads a saved map directory (read-only) as the diff baseline. Layers that cannot be read
    /// are recorded as unavailable instead of being treated as empty.
    pub fn load_saved_baseline(&mut self, dir: &Path) -> Result<(), String> {
        let label = format!(
            "Saved: {}",
            dir.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| dir.display().to_string())
        );
        match load_saved_snapshot_details(dir, &label) {
            Ok((snap, availability, scenario)) => {
                self.baseline_scenario = scenario;
                let unavailable = availability.slots().iter().filter(|s| s.is_some()).count();
                self.baseline_snapshot = Some(snap);
                self.baseline_availability = availability;
                self.baseline_source = Some(BaselineSource::SavedMap(dir.to_path_buf()));
                self.status_message = format!(
                    "Loaded saved baseline '{}' ({} of {} layers unavailable).",
                    label,
                    unavailable,
                    DIFF_LAYERS.len()
                );
                self.status_is_error = false;
                Ok(())
            }
            Err(e) => {
                self.status_message = format!("Cannot load saved baseline: {e}");
                self.status_is_error = true;
                Err(e)
            }
        }
    }

    /// Compares the working terrain in memory against the baseline. Textures, objects and scenario
    /// events of the current side are read from the saved map at `current_map_dir`, if given;
    /// otherwise those layers are reported as unavailable.
    pub fn compare_with_current_map(
        &mut self,
        current_label: &str,
        dimension: usize,
        heights: Option<&[u8]>,
        collision: Option<&[u8]>,
        current_map_dir: Option<&Path>,
    ) -> Result<(), String> {
        let (mut curr, mut av) =
            memory_snapshot(current_label, dimension, heights, collision, None);
        let mut scenario = None;
        match current_map_dir {
            Some(dir) if dir.is_dir() => {
                let (disk, disk_av, disk_scenario) =
                    load_saved_snapshot_details(dir, current_label).unwrap_or_else(|e| {
                        let reason = Some(e);
                        let av = LayerAvailability {
                            height: reason.clone(),
                            collision: reason.clone(),
                            textures: reason.clone(),
                            objects: reason.clone(),
                            events: reason.clone(),
                            object_metadata: reason.clone(),
                            spawns: reason.clone(),
                            data_slots: reason,
                        };
                        (
                            MapVersionSnapshot::new(current_label, 0, 0, None, 4.0),
                            av,
                            None,
                        )
                    });
                scenario = disk_scenario;
                av.object_metadata = disk_av.object_metadata;
                av.spawns = disk_av.spawns;
                av.data_slots = disk_av.data_slots;
                curr.textures = disk.textures;
                if disk.tile_dimension != 0 && disk_av.textures.is_none() {
                    curr.tile_dimension = disk.tile_dimension;
                }
                curr.objects = disk.objects;
                curr.events = disk.events;
                av.textures = disk_av.textures;
                av.objects = disk_av.objects;
                av.events = disk_av.events;
            }
            _ => {
                let reason = "current map directory is not set or not a directory";
                av.textures = Some(reason.into());
                av.objects = Some(reason.into());
                av.events = Some(reason.into());
                av.object_metadata = Some(reason.into());
                av.spawns = Some(reason.into());
                av.data_slots = Some(reason.into());
            }
        }
        self.run_compare(curr, av, scenario)
    }

    pub fn compare_snapshots(
        &mut self,
        current_label: &str,
        dimension: usize,
        heights: Option<&[u8]>,
        collision: Option<&[u8]>,
        objects: Option<&[DiffObjectItem]>,
    ) -> Result<(), String> {
        let (curr, av) = memory_snapshot(current_label, dimension, heights, collision, objects);
        self.run_compare(curr, av, None)
    }

    fn run_compare(
        &mut self,
        curr: MapVersionSnapshot,
        curr_av: LayerAvailability,
        scenario: Option<ScenarioDocument>,
    ) -> Result<(), String> {
        let Some(base) = self.baseline_snapshot.clone() else {
            let msg =
                "No baseline snapshot captured. Capture or load a baseline first.".to_string();
            self.status_message = msg.clone();
            self.status_is_error = true;
            return Err(msg);
        };
        let base_av = self.baseline_availability.clone();

        // A layer is compared only if both sides have it with matching dimensions.
        let mismatch = |a: usize, b: usize, name: &str| {
            (a != b).then(|| format!("{name} dimension mismatch (baseline {a}, current {b})"))
        };
        let mut states: Vec<Option<String>> = base_av
            .slots()
            .iter()
            .zip(curr_av.slots())
            .map(|(b, c)| {
                b.as_ref()
                    .map(|r| format!("baseline: {r}"))
                    .or_else(|| c.as_ref().map(|r| format!("current: {r}")))
            })
            .collect();
        for (i, m) in [
            (
                0,
                mismatch(base.height_dimension, curr.height_dimension, "height"),
            ),
            (
                1,
                mismatch(
                    base.collision_dimension,
                    curr.collision_dimension,
                    "collision",
                ),
            ),
            (
                2,
                mismatch(base.tile_dimension, curr.tile_dimension, "texture"),
            ),
        ] {
            if states[i].is_none() {
                states[i] = m;
            }
        }

        // Blank unavailable layers on both sides so the engine never reports them as "no change".
        let (mut b, mut c) = (base, curr.clone());
        if states[0].is_some() {
            b.heights = None;
            c.heights = None;
        }
        if states[1].is_some() {
            b.collision = None;
            c.collision = None;
        }
        if states[2].is_some() {
            b.textures = None;
            c.textures = None;
        }
        if states[3].is_some() {
            b.objects = None;
            c.objects = None;
        }
        if states[4].is_some() {
            b.events = None;
            c.events = None;
        }

        // Unknown metadata must not become an empty alias or a fabricated zero count comparison.
        if states[5].is_some() {
            for snapshot in [&mut b, &mut c] {
                for object in snapshot.objects.iter_mut().flatten() {
                    object.alias.clear();
                    object.unit_count = 0;
                    object.prebuilt = false;
                }
            }
        }
        let mut supplemental = String::new();
        let mut supplemental_changed = false;
        if let (Some(before), Some(after)) = (&self.baseline_scenario, &scenario) {
            if states[6].is_none() {
                supplemental_changed |= describe_records(
                    "Scenario spawns",
                    &before.spawns,
                    &after.spawns,
                    &mut supplemental,
                );
            }
            if states[7].is_none() {
                supplemental_changed |= describe_records(
                    "Scenario data slots",
                    &before.data_slots,
                    &after.data_slots,
                    &mut supplemental,
                );
            }
        }
        let report = MapDiffEngine::compare(&b, &c, None);
        let coverage: Vec<(&'static str, LayerState)> = DIFF_LAYERS
            .iter()
            .zip(states)
            .map(|(name, s)| {
                (
                    *name,
                    s.map_or(LayerState::Compared, LayerState::Unavailable),
                )
            })
            .collect();
        let incomplete = coverage
            .iter()
            .any(|(_, s)| matches!(s, LayerState::Unavailable(_)));
        let verdict = if report.has_differences() || supplemental_changed {
            DiffVerdict::Differences
        } else if incomplete {
            DiffVerdict::Incomplete
        } else {
            DiffVerdict::Identical
        };

        let mut text = report.generate_summary_text();
        if verdict != DiffVerdict::Identical {
            text = text.replace(
                "兩版本完全一致",
                match verdict {
                    DiffVerdict::Differences => "檢測到差異變更（含劇本記錄差異）",
                    _ => "已比對圖層無差異（比較不完整，不代表兩版本一致）",
                },
            );
        }
        for change in &report.objects.modified {
            text.push_str(&format!(
                "\nObject slot {} UID {}: {}\n",
                change.current.slot,
                change.current.uid,
                change.changed_attributes.join("; ")
            ));
        }
        text.push_str(&supplemental);
        text.push_str("\nObject alias/unit count: scenario metadata verified by prebuilt slot/UID bindings; not native DATA unit population.\nCurrent textures/objects/scenario use saved disk data; pending editor edits are excluded.\nA complete comparison applies only to the listed document fields and engine tolerances, not whole-map bytes or game behavior.\n");
        text.push_str("\n--- Layer coverage ---\n");
        for (name, state) in &coverage {
            match state {
                LayerState::Compared => text.push_str(&format!("  [compared]    {name}\n")),
                LayerState::Unavailable(r) => {
                    text.push_str(&format!("  [UNAVAILABLE] {name}: {r}\n"))
                }
            }
        }
        text.push_str(match verdict {
            DiffVerdict::Differences => "Verdict: DIFFERENCES FOUND\n",
            DiffVerdict::Identical => "Verdict: IDENTICAL (all layers compared)\n",
            DiffVerdict::Incomplete => "Verdict: INCOMPLETE - no differences in compared layers, but not all layers were compared (NOT a pass)\n",
        });

        self.diff_summary_text = text;
        self.layer_coverage = coverage;
        self.diff_verdict = Some(verdict);
        self.current_snapshot = Some(curr);
        self.diff_report = Some(report);
        self.dirty = true;
        self.status_message = match verdict {
            DiffVerdict::Incomplete => {
                "Diff generated, but some layers were unavailable (incomplete).".to_string()
            }
            _ => "Diff report generated successfully.".to_string(),
        };
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
                    ui.selectable_value(
                        &mut self.current_tab,
                        PackagingTab::ModExporter,
                        "Mod Exporter",
                    );
                    ui.selectable_value(&mut self.current_tab, PackagingTab::MapDiff, "Map Diff");
                    ui.selectable_value(
                        &mut self.current_tab,
                        PackagingTab::BudgetProfiler,
                        "Budget Profiler",
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
                    PackagingTab::ModExporter => self.render_exporter_tab(ui),
                    PackagingTab::MapDiff => {
                        self.render_diff_tab(ui, dimension, heights, collision)
                    }
                    PackagingTab::BudgetProfiler => {
                        self.render_budget_tab(ui, dimension, heights, collision, spawns)
                    }
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
        egui::Grid::new("exporter_settings")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
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
            ui.label(format!(
                "Preflight Issues: {} (Errors: {}, Warnings: {})",
                pre.issues.len(),
                pre.error_count(),
                pre.warning_count()
            ));
            egui::ScrollArea::vertical()
                .max_height(160.0)
                .show(ui, |ui| {
                    for issue in &pre.issues {
                        let color = match issue.severity {
                            against_rome_packaging::preflight::PreflightSeverity::Error => {
                                Color32::from_rgb(255, 90, 90)
                            }
                            against_rome_packaging::preflight::PreflightSeverity::Warning => {
                                Color32::from_rgb(255, 200, 70)
                            }
                            against_rome_packaging::preflight::PreflightSeverity::Info => {
                                Color32::LIGHT_GRAY
                            }
                        };
                        ui.colored_label(
                            color,
                            format!("[{:?}] {}: {}", issue.category, issue.code, issue.english),
                        );
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
                self.capture_baseline_from_memory(
                    "Working Baseline",
                    dimension,
                    heights,
                    collision,
                    None,
                );
            }
            if ui.button("Compare Working vs Baseline").clicked() {
                if matches!(self.baseline_source, Some(BaselineSource::SavedMap(_))) {
                    let dir = PathBuf::from(self.map_dir_input.trim());
                    let dir = (!self.map_dir_input.trim().is_empty()).then_some(dir);
                    let _ = self.compare_with_current_map(
                        "Latest Edits",
                        dimension,
                        heights,
                        collision,
                        dir.as_deref(),
                    );
                } else {
                    let _ =
                        self.compare_snapshots("Latest Edits", dimension, heights, collision, None);
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Saved baseline map folder:");
            ui.text_edit_singleline(&mut self.baseline_dir_input);
            if ui.button("Load Saved Baseline").clicked() {
                let dir = PathBuf::from(self.baseline_dir_input.trim());
                let _ = self.load_saved_baseline(&dir);
            }
        });
        if let Some(src) = &self.baseline_source {
            match src {
                BaselineSource::Memory => ui.label("Baseline: in-memory (heightmap/collision only)."),
                BaselineSource::SavedMap(p) => ui.label(format!(
                    "Baseline: saved map {} (current textures/objects/scenario are read from the saved Map folder on the Mod Exporter tab).",
                    p.display()
                )),
            };
        }
        for (name, state) in &self.layer_coverage {
            match state {
                LayerState::Compared => {
                    ui.colored_label(Color32::LIGHT_GREEN, format!("✔ {name}: compared"))
                }
                LayerState::Unavailable(r) => ui.colored_label(
                    Color32::from_rgb(255, 180, 50),
                    format!("⚠ {name}: unavailable - {r}"),
                ),
            };
        }

        ui.separator();
        if !self.diff_summary_text.is_empty() {
            egui::ScrollArea::vertical()
                .max_height(340.0)
                .show(ui, |ui| {
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
                MapHealthGrade::Critical | MapHealthGrade::EngineExceeded => {
                    Color32::from_rgb(255, 70, 70)
                }
            };

            ui.horizontal(|ui| {
                ui.heading("Overall Grade:");
                ui.colored_label(
                    grade_color,
                    format!("{:?} ({}/100)", report.overall_grade, report.overall_score),
                );
            });

            ui.add_space(4.0);
            ui.label(format!(
                "Object Slots: {} active (Max: {})",
                report.object_budget.total_active_objects,
                MapBudgetLimits::MAX_OBJECTS_DAT_SLOTS
            ));
            ui.label(format!(
                "Light Clusters: {} hotspots (peak {} lights/cluster, limit {})",
                report.light_budget.hotspots.len(),
                report.light_budget.peak_cluster_lights,
                MapBudgetLimits::HARD_LIMIT_LIGHTS_PER_CLUSTER
            ));
            ui.label(format!(
                "Estimated Draw Calls: {} (terrain {}, sprites {}, shadows {})",
                report.draw_budget.estimated_total_draw_calls,
                report.draw_budget.terrain_draw_calls,
                report.draw_budget.sprite_batch_draw_calls,
                report.draw_budget.shadow_batch_draw_calls
            ));
            ui.label(format!(
                "Pathfinding: {} chokepoints, {} dead ends ({})",
                report.path_budget.chokepoint_count,
                report.path_budget.dead_end_count,
                report.path_budget.complexity_rating
            ));

            if !report.bottlenecks.is_empty() {
                ui.separator();
                ui.heading("Detected Bottlenecks:");
                for b in &report.bottlenecks {
                    ui.colored_label(
                        Color32::from_rgb(255, 180, 50),
                        format!("• [{}] {}: {}", b.category, b.code, b.english_description),
                    );
                }
            }
        }
    }
}

fn memory_snapshot(
    label: &str,
    dimension: usize,
    heights: Option<&[u8]>,
    collision: Option<&[u8]>,
    objects: Option<&[DiffObjectItem]>,
) -> (MapVersionSnapshot, LayerAvailability) {
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
    let missing = |present: bool, why: &str| (!present).then(|| why.to_string());
    let av = LayerAvailability {
        height: missing(heights.is_some(), "no heightmap in memory"),
        collision: missing(collision.is_some(), "no collision map in memory"),
        textures: Some("not captured from memory".into()),
        objects: missing(objects.is_some(), "no object list supplied"),
        events: Some("not captured from memory".into()),
        object_metadata: missing(objects.is_some(), "no object metadata supplied"),
        spawns: Some("not captured from memory".into()),
        data_slots: Some("not captured from memory".into()),
    };
    (snap, av)
}

/// Reads a saved map directory into a diff snapshot without modifying it. Each layer that is
/// absent, unreadable or inconsistent is reported as unavailable with a reason.
pub fn load_saved_snapshot(
    dir: &Path,
    label: &str,
) -> Result<(MapVersionSnapshot, LayerAvailability), String> {
    load_saved_snapshot_details(dir, label).map(|(snap, av, _)| (snap, av))
}

fn load_saved_snapshot_details(
    dir: &Path,
    label: &str,
) -> Result<
    (
        MapVersionSnapshot,
        LayerAvailability,
        Option<ScenarioDocument>,
    ),
    String,
> {
    if !dir.is_dir() {
        return Err(format!("not a directory: {}", dir.display()));
    }
    let mut snap = MapVersionSnapshot::new(label, 0, 0, None, 4.0);
    let mut av = LayerAvailability::default();

    match crate::storage::load_map(dir) {
        Ok(terrain) => {
            snap.height_dimension = terrain.vertex_size;
            snap.tile_dimension = terrain.vertex_size.saturating_sub(1);
            snap.heights = Some(terrain.heights().to_vec());
            if dir.join("collision.bmp").is_file() {
                snap.collision_dimension = terrain.collision_size;
                snap.collision = Some(terrain.collision().to_vec());
            }
            if snap.collision.is_none() {
                av.collision = Some("collision.bmp missing or unreadable".into());
            }
        }
        Err(e) => {
            av.height = Some(e.clone());
            av.collision = Some(e);
        }
    }

    match against_rome_textures::document::BodenTexturesDocument::load(dir.join("boden.txt")) {
        Ok(doc) if doc.dimension() > 0 && doc.textures().len() == doc.tile_count() => {
            snap.tile_dimension = doc.dimension();
            snap.textures = Some(doc.textures());
        }
        Ok(_) => av.textures = Some("boden.txt grid is empty or inconsistent".into()),
        Err(e) => av.textures = Some(format!("boden.txt: {e}")),
    }

    match against_rome_placement::ObjectStore::load(dir) {
        Ok(store) => {
            let objects = store.objects();
            let unplaced = objects
                .iter()
                .filter(|o| o.position.iter().any(|v| !v.is_finite()))
                .count();
            if unplaced > 0 {
                av.objects = Some(format!(
                    "{unplaced} active object(s) have no position record"
                ));
            } else {
                snap.objects = Some(
                    objects
                        .iter()
                        .map(|o| {
                            DiffObjectItem::new(
                                uuid::Uuid::from_u128(((o.slot as u128) << 32) | o.uid as u128),
                                o.slot as i32,
                                o.uid,
                                "",
                                o.type_id as i32,
                                o.team as i32,
                                o.position[0],
                                o.position[1],
                                o.position[2],
                                o.rotation_degrees,
                                0,
                                false,
                            )
                        })
                        .collect(),
                );
            }
        }
        Err(e) => av.objects = Some(format!("DATA objects: {e}")),
    }

    let scenario = dir.join("arm_scenario.json");
    let mut document = None;
    match std::fs::read(&scenario) {
        Ok(bytes) => match against_rome_scenario::model::ScenarioDocument::from_json(&bytes) {
            Ok(doc) => {
                snap.events = Some(doc.events.clone());
                document = Some(doc);
            }
            Err(e) => av.events = Some(format!("arm_scenario.json: {e}")),
        },
        Err(e) => av.events = Some(format!("arm_scenario.json: {e}")),
    }

    av.spawns = av.events.clone();
    av.data_slots = av.events.clone();
    av.object_metadata = av.objects.clone().or_else(|| av.events.clone());
    if let (Some(objects), Some(doc)) = (&mut snap.objects, &document) {
        // A scenario label alone is not evidence for a native slot. Require a unique, live
        // slot/UID binding and a prebuilt spawn for EVERY active object before comparing metadata.
        let complete = objects.iter().all(|o| {
            doc.data_slots
                .iter()
                .filter(|b| {
                    b.slot == o.slot
                        && b.uid == o.uid
                        && doc.spawns.iter().any(|s| s.id == b.spawn_id && s.prebuilt)
                })
                .count()
                == 1
        });
        if complete {
            for object in objects {
                let binding = doc
                    .data_slots
                    .iter()
                    .find(|b| b.slot == object.slot && b.uid == object.uid)
                    .unwrap();
                let spawn = doc
                    .spawns
                    .iter()
                    .find(|s| s.id == binding.spawn_id && s.prebuilt)
                    .unwrap();
                object.alias = spawn.alias.clone();
                object.unit_count = spawn.count;
                object.prebuilt = true;
            }
        } else {
            av.object_metadata = Some("active DATA objects lack complete prebuilt scenario slot/UID bindings; native alias/unit count are not exposed by ObjectStore".into());
        }
    }
    if av.slots().iter().all(|s| s.is_some()) {
        return Err(format!("no readable map layers in {}", dir.display()));
    }
    Ok((snap, av, document))
}

/// Preserve record order and every typed field (including identity/raw rotation); no fuzzy matching.
fn describe_records<T: PartialEq + std::fmt::Debug>(
    name: &str,
    before: &[T],
    after: &[T],
    text: &mut String,
) -> bool {
    text.push_str(&format!("\n--- {name} (saved document order) ---\n"));
    let mut changed = false;
    for index in 0..before.len().max(after.len()) {
        let (old, new) = (before.get(index), after.get(index));
        if old != new {
            changed = true;
            text.push_str(&format!("  [{index}] {old:?} -> {new:?}\n"));
        }
    }
    if !changed {
        text.push_str("  No differences in typed records.\n");
    }
    changed
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

        assert!(ui
            .compare_snapshots("V2", 16, Some(&heights_b), None, None)
            .is_ok());
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
