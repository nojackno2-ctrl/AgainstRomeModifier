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
use crate::layout_io::{LayoutDialogKind, LayoutFilePicker};

pub type LayoutFolderPicker = Box<dyn Fn(&str) -> Option<PathBuf> + Send + Sync>;
use eframe::egui::{self, Color32, Vec2};
use std::path::{Component, Path, PathBuf};

/// Borrowed live editor state. Capture is read-only and never saves or reloads a session.
#[derive(Clone, Copy)]
pub struct LiveDiffSessions<'a> {
    pub textures: &'a crate::textures_ui::TexturesUi,
    pub placement: &'a crate::placement_ui::PlacementUi,
    /// `None` when ScenarioUi is not loaded; never substitute a default or disk document.
    pub scenario: Option<&'a ScenarioDocument>,
}

impl<'a> LiveDiffSessions<'a> {
    pub fn from_sessions(
        textures: &'a crate::textures_ui::TexturesUi,
        placement: &'a crate::placement_ui::PlacementUi,
        scenario: &'a crate::scenario_ui::ScenarioUi,
    ) -> Self {
        Self {
            textures,
            placement,
            scenario: scenario.is_loaded().then(|| scenario.document()),
        }
    }
}

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
    pub baseline_scenario: Option<ScenarioDocument>,

    // --- Budget Profiler state ---
    pub budget_report: Option<MapBudgetReport>,

    // Bilingual / language
    pub english: bool,

    // File picker injection (layout_picker and export_picker supported interchangeably)
    pub layout_picker: Option<LayoutFilePicker>,
    pub export_picker: Option<LayoutFilePicker>,
    pub folder_picker: Option<LayoutFolderPicker>,

    // Dirty / guards / overwrite confirmation
    pub dirty: bool,
    pub pending_discard: bool,
    pub pending_overwrite_confirm: bool,
    pending_export_request: Option<PendingExportRequest>,
    pending_overwrite_approval: Option<(PathBuf, PathBuf)>,
    pub status_message: String,
    pub status_is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingExportRequest {
    requested_source: PathBuf,
    confirmed_overwrite: Option<(PathBuf, PathBuf)>,
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
            english: false,
            layout_picker: None,
            export_picker: None,
            folder_picker: None,
            dirty: false,
            pending_discard: false,
            pending_overwrite_confirm: false,
            pending_export_request: None,
            pending_overwrite_approval: None,
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
        self.pending_overwrite_confirm = false;
        self.pending_export_request = None;
        self.pending_overwrite_approval = None;
    }

    /// Resets all map-bound packaging reports, export results, preflight findings, diff snapshots,
    /// and updates the map directory path.
    /// Preserves user preferences (open state, current tab, package metadata, clean_caches, thumbnail settings).
    pub fn reset_session(&mut self, map_path: Option<&Path>) {
        self.dirty = false;
        self.pending_discard = false;
        self.pending_overwrite_confirm = false;
        self.pending_export_request = None;
        self.pending_overwrite_approval = None;
        self.map_dir_input = map_path
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        self.output_zip_input = map_path
            .map(|p| default_export_zip_path(p).to_string_lossy().to_string())
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

    fn pick_export_file(&self, kind: &LayoutDialogKind) -> Option<PathBuf> {
        if let Some(picker) = self.export_picker.as_ref().or(self.layout_picker.as_ref()) {
            picker(kind)
        } else {
            let suggested = match kind {
                LayoutDialogKind::Save { suggested_name } => suggested_name.as_str(),
                LayoutDialogKind::Open => "",
            };
            rfd::FileDialog::new()
                .set_title("Export Mod ZIP Bundle / 匯出模組 ZIP")
                .add_filter("ZIP Archive (*.zip)", &["zip"])
                .set_file_name(suggested)
                .save_file()
        }
    }

    pub fn browse_export_zip(&mut self) {
        let suggested_name = if !self.output_zip_input.trim().is_empty() {
            Path::new(self.output_zip_input.trim())
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "custom-map.zip".to_string())
        } else {
            let map_id = Path::new(self.map_dir_input.trim())
                .file_stem()
                .and_then(|s| s.to_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("custom-map");
            format!("{map_id}.zip")
        };

        let kind = LayoutDialogKind::Save { suggested_name };
        if let Some(path) = self.pick_export_file(&kind) {
            self.output_zip_input = path.to_string_lossy().to_string();
        }
    }

    pub fn browse_map_dir(&mut self) {
        let title = if self.english {
            "Select Source Map Folder / 選擇來源地圖資料夾"
        } else {
            "選擇來源地圖資料夾 / Select Source Map Folder"
        };
        let picked = if let Some(picker) = self.folder_picker.as_ref() {
            picker(title)
        } else {
            rfd::FileDialog::new().set_title(title).pick_folder()
        };
        if let Some(path) = picked {
            self.map_dir_input = path.to_string_lossy().to_string();
        }
    }

    pub fn browse_baseline_dir(&mut self) {
        let title = if self.english {
            "Select Saved Baseline Map Folder / 選擇儲存的基準地圖資料夾"
        } else {
            "選擇儲存的基準地圖資料夾 / Select Saved Baseline Map Folder"
        };
        let picked = if let Some(picker) = self.folder_picker.as_ref() {
            picker(title)
        } else {
            rfd::FileDialog::new().set_title(title).pick_folder()
        };
        if let Some(path) = picked {
            self.baseline_dir_input = path.to_string_lossy().to_string();
        }
    }

    pub fn run_preflight_check(&mut self) {
        let dir = Path::new(self.map_dir_input.trim());
        if !dir.is_dir() {
            self.status_message = if self.english {
                format!("Invalid map directory: {}", dir.display())
            } else {
                format!("無效的地圖資料夾路徑：{}", dir.display())
            };
            self.status_is_error = true;
            return;
        }

        let report = match MapExportPreflightChecker::check(dir, None, None) {
            Ok(report) => report,
            Err(e) => {
                self.status_message = if self.english {
                    format!("Preflight failed: {e}")
                } else {
                    format!("預檢驗收失敗：{e}")
                };
                self.status_is_error = true;
                return;
            }
        };
        let error_count = report.error_count();
        let warn_count = report.warning_count();
        self.status_message = if self.english {
            format!(
                "Preflight completed: {} errors, {} warnings.",
                error_count, warn_count
            )
        } else {
            format!(
                "預檢驗收完成：{} 個錯誤，{} 個警告。",
                error_count, warn_count
            )
        };
        self.status_is_error = error_count > 0;
        self.last_preflight_report = Some(report);
    }

    pub fn request_export(&mut self) {
        self.pending_export_request = Some(PendingExportRequest {
            requested_source: PathBuf::from(self.map_dir_input.trim()),
            confirmed_overwrite: None,
        });
    }

    /// Confirms the source and output shown by the overwrite prompt. The host rechecks both
    /// paths and the source map's dirty state immediately before writing the ZIP.
    pub fn confirm_overwrite_and_export(&mut self) {
        self.pending_overwrite_confirm = false;
        let confirmed_overwrite = self.pending_overwrite_approval.take();
        self.pending_export_request = Some(PendingExportRequest {
            requested_source: PathBuf::from(self.map_dir_input.trim()),
            confirmed_overwrite,
        });
    }

    pub(crate) fn take_export_request(
        &mut self,
    ) -> Option<(PathBuf, Option<(PathBuf, PathBuf)>, PathBuf, PathBuf)> {
        let request = self.pending_export_request.take()?;
        Some((
            request.requested_source,
            request.confirmed_overwrite,
            PathBuf::from(self.map_dir_input.trim()),
            PathBuf::from(self.output_zip_input.trim()),
        ))
    }

    pub(crate) fn block_export_for_unsaved_map(&mut self) {
        self.pending_overwrite_confirm = false;
        self.status_message = tl(
            self.english,
            "目前載入的地圖有未儲存修改；請先執行 Safe Save，再重新匯出。",
            "The loaded map has unsaved edits. Run Safe Save before exporting again.",
        )
        .to_string();
        self.status_is_error = true;
    }

    pub(crate) fn export_target_changed(&mut self) {
        self.status_message = tl(
            self.english,
            "輸出 ZIP 路徑已變更；請重新確認匯出目標。",
            "The output ZIP path changed. Confirm the export target again.",
        )
        .to_string();
        self.status_is_error = true;
    }

    pub(crate) fn set_overwrite_approval(&mut self, source: PathBuf, target: PathBuf) {
        self.pending_overwrite_approval = Some((source, target));
        self.pending_overwrite_confirm = true;
    }

    pub(crate) fn run_export(&mut self) -> Result<(), String> {
        let dir = Path::new(self.map_dir_input.trim());
        let zip = Path::new(self.output_zip_input.trim());

        if !dir.is_dir() {
            let msg = if self.english {
                format!("Invalid map directory: {}", dir.display())
            } else {
                format!("無效的地圖資料夾路徑：{}", dir.display())
            };
            self.status_message = msg.clone();
            self.status_is_error = true;
            return Err(msg);
        }
        if zip.as_os_str().is_empty() {
            let msg = tl(
                self.english,
                "請指定輸出 ZIP 檔案路徑。",
                "Please specify output ZIP file path.",
            )
            .to_string();
            self.status_message = msg.clone();
            self.status_is_error = true;
            return Err(msg);
        }

        if let Err(reason) = validate_export_destination(dir, zip) {
            let msg = if self.english {
                format!("Unsafe ZIP export destination: {reason}")
            } else {
                format!("ZIP 匯出目的地不安全：{reason}")
            };
            self.status_message = msg.clone();
            self.status_is_error = true;
            return Err(msg);
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
                // Read the finished ZIP back (archive invariants + SHA-256 of every payload) so a
                // truncated or corrupt write is never reported as a good package.
                if let Err(e) = ModBundleExporter::verify_zip(Path::new(&result.output_zip_path)) {
                    let msg = if self.english {
                        format!(
                            "Export wrote {} but read-back verification failed: {e}. Do not distribute this ZIP; export again.",
                            result.output_zip_path
                        )
                    } else {
                        format!(
                            "已寫出 {}，但讀回驗證失敗：{e}。請勿散布此 ZIP，請重新匯出。",
                            result.output_zip_path
                        )
                    };
                    self.status_message = msg.clone();
                    self.status_is_error = true;
                    return Err(msg);
                }
                self.status_message = if self.english {
                    format!(
                        "Bundle exported and verified: {} ({} files, {} bytes, checksums re-read OK)",
                        result.output_zip_path, result.file_count, result.total_size_bytes
                    )
                } else {
                    format!(
                        "模組套件匯出並通過讀回驗證：{}（共 {} 個檔案，{} 位元組，雜湊已重新讀取確認）",
                        result.output_zip_path, result.file_count, result.total_size_bytes
                    )
                };
                self.status_is_error = false;
                self.last_export_result = Some(result);
                self.dirty = true;
                Ok(())
            }
            Err(e) => {
                self.status_message = if self.english {
                    format!("Export failed: {e}")
                } else {
                    format!("匯出失敗：{e}")
                };
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

    /// Captures the actual live sessions at Capture time as the baseline, including live
    /// textures, objects, and scenario.
    pub fn capture_baseline_from_live_sessions(
        &mut self,
        label: &str,
        dimension: usize,
        heights: Option<&[u8]>,
        collision: Option<&[u8]>,
        live: LiveDiffSessions<'_>,
    ) {
        let (mut snap, mut av) = memory_snapshot(label, dimension, heights, collision, None);
        match (
            live.textures.tile_dimension(),
            live.textures.session_textures(),
        ) {
            (Some(d), Some(textures)) if d > 0 && d.checked_mul(d) == Some(textures.len()) => {
                snap.tile_dimension = d;
                snap.textures = Some(textures.to_vec());
                av.textures = None;
            }
            _ => {
                av.textures =
                    Some("live texture session missing or grid dimensions inconsistent".into())
            }
        }
        if live.placement.is_loaded() {
            capture_objects(&mut snap, &mut av, &live.placement.objects());
        }
        let scenario = live.scenario.cloned();
        if let Some(doc) = &scenario {
            snap.events = Some(doc.events.clone());
            av.events = None;
            av.spawns = None;
            av.data_slots = None;
        }
        bind_object_metadata(&mut snap, &mut av, scenario.as_ref());
        self.baseline_scenario = scenario;
        self.baseline_snapshot = Some(snap);
        self.baseline_availability = av;
        self.baseline_source = Some(BaselineSource::Memory);
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

    /// Compatibility entry point without live sessions. Disk values must never stand in for
    /// unavailable editor state; callers with sessions should use `compare_live_sessions`.
    pub fn compare_with_current_map(
        &mut self,
        current_label: &str,
        dimension: usize,
        heights: Option<&[u8]>,
        collision: Option<&[u8]>,
        _current_map_dir: Option<&Path>,
    ) -> Result<(), String> {
        self.compare_snapshots(current_label, dimension, heights, collision, None)
    }

    /// Captures the actual sessions at Compare time, including unsaved changes. No disk fallback.
    pub fn compare_live_sessions(
        &mut self,
        label: &str,
        dimension: usize,
        heights: Option<&[u8]>,
        collision: Option<&[u8]>,
        live: LiveDiffSessions<'_>,
    ) -> Result<(), String> {
        let (mut snap, mut av) = memory_snapshot(label, dimension, heights, collision, None);
        match (
            live.textures.tile_dimension(),
            live.textures.session_textures(),
        ) {
            (Some(d), Some(textures)) if d > 0 && d.checked_mul(d) == Some(textures.len()) => {
                snap.tile_dimension = d;
                snap.textures = Some(textures.to_vec());
                av.textures = None;
            }
            _ => {
                av.textures =
                    Some("live texture session missing or grid dimensions inconsistent".into())
            }
        }
        if live.placement.is_loaded() {
            capture_objects(&mut snap, &mut av, &live.placement.objects());
        }
        let scenario = live.scenario.cloned();
        if let Some(doc) = &scenario {
            snap.events = Some(doc.events.clone());
            av.events = None;
            av.spawns = None;
            av.data_slots = None;
        }
        bind_object_metadata(&mut snap, &mut av, scenario.as_ref());
        self.run_compare(snap, av, scenario)
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
            if states[4].is_none() {
                supplemental_changed |= describe_records(
                    "Scenario events",
                    &before.events,
                    &after.events,
                    &mut supplemental,
                );
            }
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
        text.push_str("\nObject alias/unit count: scenario metadata requires unique prebuilt slot/UID bindings; not native DATA unit population.\nCurrent layers use live editor sessions, including unsaved edits. Missing sessions are unavailable.\nA complete comparison applies only to the listed document fields and engine tolerances, not whole-map bytes or game behavior.\n");
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
        live: LiveDiffSessions<'_>,
    ) {
        if !self.open {
            return;
        }

        let mut is_open = self.open;
        egui::Window::new(tl(
            self.english,
            "打包、差異比對與預算分析工作室",
            "Packaging, Diff & Budget Profiler",
        ))
        .open(&mut is_open)
        .min_size(Vec2::new(740.0, 540.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.current_tab,
                    PackagingTab::ModExporter,
                    tl(self.english, "模組匯出工具", "Mod Exporter"),
                );
                ui.selectable_value(
                    &mut self.current_tab,
                    PackagingTab::MapDiff,
                    tl(self.english, "地圖版本比對", "Map Diff"),
                );
                ui.selectable_value(
                    &mut self.current_tab,
                    PackagingTab::BudgetProfiler,
                    tl(self.english, "預算效能分析", "Budget Profiler"),
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
                    self.render_diff_tab(ui, dimension, heights, collision, live)
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

            egui::Window::new(tl(
                self.english,
                "捨棄差異比對與分析狀態？",
                "Discard Diff & Profiling State?",
            ))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(tl(
                    self.english,
                    "關閉將會清除目前計算的地圖差異比較與預算報告。",
                    "Closing will clear currently calculated diff comparisons and budget reports.",
                ));
                ui.horizontal(|ui| {
                    if ui
                        .button(tl(self.english, "捨棄並關閉", "Discard & Close"))
                        .clicked()
                    {
                        confirm = true;
                    }
                    if ui
                        .button(tl(self.english, "繼續編輯", "Keep Editing"))
                        .clicked()
                    {
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

        if self.pending_overwrite_confirm {
            let mut do_overwrite = false;
            let mut cancel = false;

            egui::Window::new(tl(self.english, "確認覆蓋檔案？", "Confirm File Overwrite?"))
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    let (source, target) = self
                        .pending_overwrite_approval
                        .as_ref()
                        .map(|(source, target)| (source.display().to_string(), target.display().to_string()))
                        .unwrap_or_else(|| (self.map_dir_input.trim().to_string(), self.output_zip_input.trim().to_string()));
                    ui.label(format!(
                        "{}\n{}\n{}",
                        tl(
                            self.english,
                            "匯出來源地圖：",
                            "Source map:"
                        ),
                        source,
                        format!("{}\n{}", tl(self.english, "輸出 ZIP（將覆蓋）：", "Output ZIP (will be overwritten):"), target)
                    ));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui
                            .button(tl(self.english, "覆蓋既有檔案", "Overwrite Existing File"))
                            .clicked()
                        {
                            do_overwrite = true;
                        }
                        if ui.button(tl(self.english, "取消", "Cancel")).clicked() {
                            cancel = true;
                        }
                    });
                });

            if do_overwrite {
                self.confirm_overwrite_and_export();
            } else if cancel {
                self.pending_overwrite_confirm = false;
            }
        }
    }

    fn render_exporter_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading(tl(self.english, "模組套件匯出工具", "Mod Bundle Exporter"));
        ui.label(tl(
            self.english,
            "將可編輯的地圖資料夾打包成經由驗證的 Against Rome 發布用 ZIP 模組套件，包含清單。",
            "Packs editable map folders into verified Against Rome distribution ZIP bundles with manifests.",
        ));

        ui.add_space(6.0);
        egui::Grid::new("exporter_settings")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label(tl(self.english, "來源地圖資料夾：", "Source Map Folder:"));
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.map_dir_input)
                            .desired_width(280.0),
                    );
                    if ui.button(tl(self.english, "瀏覽…", "Browse…")).clicked() {
                        self.browse_map_dir();
                    }
                });
                ui.end_row();

                ui.label(tl(self.english, "輸出 ZIP 路徑：", "Output ZIP Path:"));
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.output_zip_input)
                            .desired_width(280.0),
                    );
                    if ui.button(tl(self.english, "瀏覽…", "Browse…")).clicked() {
                        self.browse_export_zip();
                    }
                });
                ui.end_row();

                ui.label(tl(self.english, "套件識別碼：", "Package ID:"));
                ui.text_edit_singleline(&mut self.package_id);
                ui.end_row();

                ui.label(tl(self.english, "作者名稱：", "Author Name:"));
                ui.text_edit_singleline(&mut self.package_author);
                ui.end_row();

                ui.label(tl(self.english, "模組版本：", "Mod Version:"));
                ui.text_edit_singleline(&mut self.package_version);
                ui.end_row();

                ui.label(tl(self.english, "清除中介快取：", "Clean Intermediate Caches:"));
                ui.checkbox(&mut self.clean_caches, "");
                ui.end_row();

                ui.label(tl(self.english, "產生小地圖縮圖：", "Generate Minimap Thumbnail:"));
                ui.checkbox(&mut self.generate_thumbnail, "");
                ui.end_row();
            });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui
                .button(tl(self.english, "發布前預檢驗收", "Preflight Verification"))
                .clicked()
            {
                self.run_preflight_check();
            }
            if ui
                .button(tl(self.english, "匯出模組 ZIP 套件", "Export Mod ZIP Bundle"))
                .clicked()
            {
                self.request_export();
            }
        });

        if let Some(pre) = &self.last_preflight_report {
            ui.separator();
            ui.label(format!(
                "{}: {} ({}: {}, {}: {})",
                tl(self.english, "預檢問題", "Preflight Issues"),
                pre.issues.len(),
                tl(self.english, "錯誤", "Errors"),
                pre.error_count(),
                tl(self.english, "警告", "Warnings"),
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
                        let msg = if self.english {
                            &issue.english
                        } else {
                            &issue.chinese
                        };
                        ui.colored_label(
                            color,
                            format!("[{:?}] {}: {}", issue.category, issue.code, msg),
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
        live: LiveDiffSessions<'_>,
    ) {
        ui.heading(tl(self.english, "地圖版本比對引擎", "Map Version Diff Engine"));
        ui.label(tl(
            self.english,
            "比對記憶體中編輯中的圖層與基準快照（高度、碰撞、物件與腳本）。",
            "Compares working memory edits against a baseline snapshot across height, collision, objects, and scripts.",
        ));

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui
                .button(tl(
                    self.english,
                    "擷取目前狀態為基準快照",
                    "Capture Current as Baseline",
                ))
                .clicked()
            {
                self.capture_baseline_from_live_sessions(
                    "Working Baseline",
                    dimension,
                    heights,
                    collision,
                    live,
                );
            }
            if ui
                .button(tl(
                    self.english,
                    "比對目前編輯與基準快照",
                    "Compare Working vs Baseline",
                ))
                .clicked()
            {
                let _ = self.compare_live_sessions(
                    "Latest Edits",
                    dimension,
                    heights,
                    collision,
                    live,
                );
            }
        });
        ui.horizontal(|ui| {
            ui.label(tl(
                self.english,
                "儲存的基準地圖資料夾：",
                "Saved baseline map folder:",
            ));
            ui.add(
                egui::TextEdit::singleline(&mut self.baseline_dir_input)
                    .desired_width(280.0),
            );
            if ui
                .button(tl(self.english, "瀏覽…", "Browse…"))
                .clicked()
            {
                self.browse_baseline_dir();
            }
            if ui
                .button(tl(self.english, "載入已儲存基準", "Load Saved Baseline"))
                .clicked()
            {
                let dir = PathBuf::from(self.baseline_dir_input.trim());
                let _ = self.load_saved_baseline(&dir);
            }
        });
        if let Some(src) = &self.baseline_source {
            match src {
                BaselineSource::Memory => {
                    let has_live = self.baseline_scenario.is_some()
                        || self.baseline_availability.textures.is_none()
                        || self.baseline_availability.objects.is_none();
                    let text = if has_live {
                        tl(
                            self.english,
                            "基準：記憶體快照（包含編輯工作階段）。",
                            "Baseline: working memory snapshot (captured live sessions).",
                        )
                    } else {
                        tl(
                            self.english,
                            "基準：記憶體快照（僅高度圖／阻擋）。",
                            "Baseline: in-memory (heightmap/collision only).",
                        )
                    };
                    ui.label(text);
                }
                BaselineSource::SavedMap(p) => {
                    ui.label(if self.english {
                        format!(
                            "Baseline: saved map {} (current layers are captured from live editor sessions).",
                            p.display()
                        )
                    } else {
                        format!(
                            "基準：已儲存地圖 {}（目前圖層從編輯工作階段擷取）。",
                            p.display()
                        )
                    });
                }
            };
        }
        for (name, state) in &self.layer_coverage {
            match state {
                LayerState::Compared => ui.colored_label(
                    Color32::LIGHT_GREEN,
                    if self.english {
                        format!("✔ {name}: compared")
                    } else {
                        format!("✔ {name}：已比對")
                    },
                ),
                LayerState::Unavailable(r) => ui.colored_label(
                    Color32::from_rgb(255, 180, 50),
                    if self.english {
                        format!("⚠ {name}: unavailable - {r}")
                    } else {
                        format!("⚠ {name}：無法使用 - {r}")
                    },
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
            ui.label(tl(
                self.english,
                "尚無比對報告。請先擷取基準快照並編輯地形以檢視差異。",
                "No diff report available. Capture a baseline first and edit terrain to see differences.",
            ));
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
        ui.heading(tl(
            self.english,
            "地圖資源預算與效能分析器",
            "Map Budget & Performance Profiler",
        ));
        ui.label(tl(
            self.english,
            "分析物件槽位、光源叢集熱點、繪製呼叫與尋路複雜度。",
            "Analyzes object slots, light cluster hotspots, draw calls, and pathfinding complexity.",
        ));

        ui.add_space(6.0);
        if ui
            .button(tl(self.english, "執行預算效能分析", "Run Budget Profiler"))
            .clicked()
        {
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
                ui.heading(tl(self.english, "綜合評級：", "Overall Grade:"));
                ui.colored_label(
                    grade_color,
                    format!("{:?} ({}/100)", report.overall_grade, report.overall_score),
                );
            });

            ui.add_space(4.0);
            ui.label(if self.english {
                format!(
                    "Object Slots: {} active (Max: {})",
                    report.object_budget.total_active_objects,
                    MapBudgetLimits::MAX_OBJECTS_DAT_SLOTS
                )
            } else {
                format!(
                    "物件槽位：{} 個使用中（上限：{}）",
                    report.object_budget.total_active_objects,
                    MapBudgetLimits::MAX_OBJECTS_DAT_SLOTS
                )
            });
            ui.label(if self.english {
                format!(
                    "Light Clusters: {} hotspots (peak {} lights/cluster, limit {})",
                    report.light_budget.hotspots.len(),
                    report.light_budget.peak_cluster_lights,
                    MapBudgetLimits::HARD_LIMIT_LIGHTS_PER_CLUSTER
                )
            } else {
                format!(
                    "光源叢集：{} 個熱點（峰值 {} 光源／叢集，上限 {}）",
                    report.light_budget.hotspots.len(),
                    report.light_budget.peak_cluster_lights,
                    MapBudgetLimits::HARD_LIMIT_LIGHTS_PER_CLUSTER
                )
            });
            ui.label(if self.english {
                format!(
                    "Estimated Draw Calls: {} (terrain {}, sprites {}, shadows {})",
                    report.draw_budget.estimated_total_draw_calls,
                    report.draw_budget.terrain_draw_calls,
                    report.draw_budget.sprite_batch_draw_calls,
                    report.draw_budget.shadow_batch_draw_calls
                )
            } else {
                format!(
                    "預估繪製呼叫：{}（地形 {}、精靈 {}、陰影 {}）",
                    report.draw_budget.estimated_total_draw_calls,
                    report.draw_budget.terrain_draw_calls,
                    report.draw_budget.sprite_batch_draw_calls,
                    report.draw_budget.shadow_batch_draw_calls
                )
            });
            ui.label(if self.english {
                format!(
                    "Pathfinding: {} chokepoints, {} dead ends ({})",
                    report.path_budget.chokepoint_count,
                    report.path_budget.dead_end_count,
                    report.path_budget.complexity_rating
                )
            } else {
                format!(
                    "尋路分析：{} 處瓶頸隘口、{} 處死胡同（{}）",
                    report.path_budget.chokepoint_count,
                    report.path_budget.dead_end_count,
                    report.path_budget.complexity_rating
                )
            });

            if !report.bottlenecks.is_empty() {
                ui.separator();
                ui.heading(tl(self.english, "偵測到的效能瓶頸：", "Detected Bottlenecks:"));
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

/// Resolves a path through existing symlinks/junctions while allowing missing trailing
/// components, then rejects ZIP destinations within the source map or game MAPS tree.
fn validate_export_destination(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.is_dir() {
        return Err("the ZIP path is an existing folder; enter a file name ending in .zip".to_string());
    }
    let is_zip = destination
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    if !is_zip {
        return Err("the file name must end in .zip, otherwise the package is not recognized as a ZIP".to_string());
    }
    let source = resolve_path_through_existing_ancestors(source)
        .map_err(|e| format!("cannot resolve source map path: {e}"))?;
    let destination = resolve_path_through_existing_ancestors(destination)
        .map_err(|e| format!("cannot resolve ZIP destination: {e}"))?;

    if destination.starts_with(&source) {
        return Err("the ZIP must be outside the source map directory".to_string());
    }
    let game_root = against_rome_maps::detect_game_root(&destination, None)
        .or_else(|| against_rome_maps::detect_game_root(&source, None));
    if let Some(game_root) = game_root {
        let maps_dir = resolve_path_through_existing_ancestors(&game_root.join("MAPS"))
            .map_err(|e| format!("cannot resolve game MAPS directory: {e}"))?;
        if destination.starts_with(&maps_dir) {
            return Err("the ZIP must be outside the game MAPS directory".to_string());
        }
    }
    Ok(())
}

/// Makes the path absolute and lexically clean, then canonicalizes its nearest existing
/// ancestor so existing symlink/junction components cannot hide an unsafe destination.
fn resolve_path_through_existing_ancestors(path: &Path) -> std::io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    let mut existing = normalized.clone();
    let mut missing = Vec::new();
    loop {
        match std::fs::symlink_metadata(&existing) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let Some(name) = existing.file_name().map(|name| name.to_os_string()) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("no existing ancestor for {}", normalized.display()),
            ));
        };
        missing.push(name);
        if !existing.pop() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("no existing ancestor for {}", normalized.display()),
            ));
        }
    }
    let mut resolved = std::fs::canonicalize(existing)?;
    for component in missing.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn memory_snapshot(
    label: &str,
    dimension: usize,
    heights: Option<&[u8]>,
    collision: Option<&[u8]>,
    objects: Option<&[DiffObjectItem]>,
) -> (MapVersionSnapshot, LayerAvailability) {
    // Collision is a separate square grid; storage accepts dimensions independent of heights.
    let collision_dimension = collision.map_or(0, |c| c.len().isqrt());
    let snap = MapVersionSnapshot {
        version_label: label.to_string(),
        tile_dimension: dimension.saturating_sub(1),
        height_dimension: dimension,
        heights: heights.map(|h| h.to_vec()),
        height_step: 4.0,
        textures: None,
        collision_dimension,
        collision: collision.map(|c| c.to_vec()),
        objects: objects.map(|o| o.to_vec()),
        events: None,
    };
    let missing = |present: bool, why: &str| (!present).then(|| why.to_string());
    let av = LayerAvailability {
        height: missing(
            dimension > 1
                && heights.is_some_and(|h| dimension.checked_mul(dimension) == Some(h.len())),
            "heightmap missing or dimensions inconsistent in memory",
        ),
        collision: missing(
            collision_dimension > 0
                && collision.is_some_and(|c| {
                    collision_dimension.checked_mul(collision_dimension) == Some(c.len())
                }),
            "collision map missing or dimensions inconsistent in memory",
        ),
        textures: Some("not captured from memory".into()),
        objects: missing(objects.is_some(), "no object list supplied"),
        events: Some("not captured from memory".into()),
        object_metadata: Some("prebuilt slot/UID bindings not captured from memory".into()),
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
        Ok(store) => capture_objects(&mut snap, &mut av, &store.objects()),
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
    bind_object_metadata(&mut snap, &mut av, document.as_ref());
    if av.slots().iter().all(|s| s.is_some()) {
        return Err(format!("no readable map layers in {}", dir.display()));
    }
    Ok((snap, av, document))
}

fn capture_objects(
    snap: &mut MapVersionSnapshot,
    av: &mut LayerAvailability,
    objects: &[against_rome_placement::WorldObject],
) {
    if objects
        .iter()
        .any(|o| o.position.iter().any(|v| !v.is_finite()) || !o.rotation_degrees.is_finite())
    {
        av.objects =
            Some("active objects have missing/nonfinite position or rotation records".into());
        snap.objects = None;
        return;
    }
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
    av.objects = None;
}

fn bind_object_metadata(
    snap: &mut MapVersionSnapshot,
    av: &mut LayerAvailability,
    document: Option<&ScenarioDocument>,
) {
    av.object_metadata =
        Some("active DATA objects require unique prebuilt scenario slot/UID/spawn bindings".into());
    let (Some(objects), Some(doc)) = (&mut snap.objects, document) else {
        return;
    };
    // Validate the entire binding before assigning any metadata. Duplicate spawn identities or
    // multiple native slots sharing one spawn are ambiguous, even if a first match exists.
    if !objects.iter().all(|o| {
        let bindings: Vec<_> = doc.data_slots.iter().filter(|b| b.slot == o.slot).collect();
        bindings.len() == 1
            && bindings[0].uid == o.uid
            && doc
                .data_slots
                .iter()
                .filter(|b| b.spawn_id == bindings[0].spawn_id)
                .count()
                == 1
            && doc
                .spawns
                .iter()
                .filter(|s| s.id == bindings[0].spawn_id)
                .count()
                == 1
            && doc
                .spawns
                .iter()
                .any(|s| s.id == bindings[0].spawn_id && s.prebuilt)
    }) {
        return;
    }
    for object in objects {
        let binding = doc
            .data_slots
            .iter()
            .find(|b| b.slot == object.slot)
            .unwrap();
        let spawn = doc
            .spawns
            .iter()
            .find(|s| s.id == binding.spawn_id)
            .unwrap();
        object.alias = spawn.alias.clone();
        object.unit_count = spawn.count;
        object.prebuilt = true;
    }
    av.object_metadata = None;
}
/// Preserve record order and every typed field (including identity/raw rotation); no fuzzy matching.
fn describe_records<T: PartialEq + std::fmt::Debug>(
    name: &str,
    before: &[T],
    after: &[T],
    text: &mut String,
) -> bool {
    text.push_str(&format!("\n--- {name} (document order) ---\n"));
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

// ---------------------------------------------------------------------------
// Test in game (C# MapEditorForm.PreviewInGame)
// ---------------------------------------------------------------------------

/// Game install directory implied by a map folder `<game>/MAPS/<map>` (None if the layout does not fit).
pub fn game_dir_for_map(map_dir: &Path) -> Option<PathBuf> {
    let game = map_dir.parent()?.parent()?;
    (!game.as_os_str().is_empty()).then(|| game.to_path_buf())
}

/// Process spawner: `(exe, working_dir)`. Injectable so tests never start the real game.
pub type GameSpawner<'a> = &'a mut dyn FnMut(&Path, &Path) -> std::io::Result<()>;

/// Production spawner: starts `Against_Rome.exe` detached with the game directory as cwd.
pub fn spawn_game_process(exe: &Path, cwd: &Path) -> std::io::Result<()> {
    std::process::Command::new(exe)
        .current_dir(cwd)
        .spawn()
        .map(|_| ())
}

/// Validates `Against_Rome.exe` exists in `game_dir`, then spawns it. Returns the exe path.
pub fn launch_game(game_dir: &Path, spawn: GameSpawner<'_>) -> Result<PathBuf, String> {
    let exe = game_dir.join("Against_Rome.exe");
    if !exe.is_file() {
        return Err(format!(
            "Against_Rome.exe not found in game folder: {}",
            game_dir.display()
        ));
    }
    spawn(&exe, game_dir).map_err(|e| format!("Failed to launch game: {e}"))?;
    Ok(exe)
}

/// Returns true if the path is inside the game installation MAPS folder or Against Rome game root.
pub fn is_game_maps_dir(path: &Path) -> bool {
    let has_maps_component = path
        .components()
        .any(|c| c.as_os_str().to_string_lossy().eq_ignore_ascii_case("MAPS"));
    if has_maps_component {
        return true;
    }
    against_rome_maps::detect_game_root(path, None).is_some()
}

/// Computes the default directory for exporting map mod zip bundles.
/// Priority: user's Documents folder -> editor_data_dir parent -> temp dir.
/// Never returns a path inside the game's MAPS folder.
pub fn default_export_dir() -> PathBuf {
    if let Some(user_profile) = std::env::var_os("USERPROFILE") {
        let docs = PathBuf::from(user_profile).join("Documents");
        if docs.is_dir() && !is_game_maps_dir(&docs) {
            return docs;
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let docs = PathBuf::from(home).join("Documents");
        if docs.is_dir() && !is_game_maps_dir(&docs) {
            return docs;
        }
    }
    if let Some(editor_dir) = crate::storage::editor_data_dir() {
        if let Some(parent) = editor_dir.parent() {
            if !is_game_maps_dir(parent) {
                return parent.to_path_buf();
            }
        }
    }
    if let Some(user_profile) = std::env::var_os("USERPROFILE") {
        let docs = PathBuf::from(user_profile).join("Documents");
        if !is_game_maps_dir(&docs) {
            return docs;
        }
    }
    let tmp = std::env::temp_dir();
    if !is_game_maps_dir(&tmp) {
        return tmp;
    }
    PathBuf::from(".")
}

/// Generates a default ZIP export path named after the map ID.
/// Guarantees that the resulting path is never located in the game MAPS folder
/// or inside the source map directory itself.
pub fn default_export_zip_path(map_path: &Path) -> PathBuf {
    let map_id = map_path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("custom-map");
    let zip_name = format!("{map_id}.zip");

    let dir = default_export_dir();
    let dir = if is_game_maps_dir(&dir) || dir.starts_with(map_path) {
        crate::storage::editor_data_dir()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .filter(|p| !is_game_maps_dir(p) && !p.starts_with(map_path))
            .unwrap_or_else(std::env::temp_dir)
    } else {
        dir
    };
    dir.join(zip_name)
}

#[inline]
pub fn tr(english: bool, en: &'static str, zh: &'static str) -> &'static str {
    if english {
        en
    } else {
        zh
    }
}

#[inline]
pub fn tl(english: bool, zh: &'static str, en: &'static str) -> &'static str {
    if english {
        en
    } else {
        zh
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn launch_game_uses_injected_spawner_and_checks_exe() {
        let dir = crate::packaging_ui::temp_fixture::dir("ar_launch");
        std::fs::create_dir_all(dir.join("MAPS").join("ENDL_007")).unwrap();
        let mut calls = Vec::new();
        let mut spawn = |exe: &Path, cwd: &Path| {
            calls.push((exe.to_path_buf(), cwd.to_path_buf()));
            Ok(())
        };
        assert!(launch_game(&dir, &mut spawn)
            .unwrap_err()
            .contains("not found"));
        std::fs::write(dir.join("Against_Rome.exe"), b"x").unwrap();
        assert_eq!(
            game_dir_for_map(&dir.join("MAPS").join("ENDL_007")).unwrap(),
            dir
        );
        launch_game(&dir, &mut spawn).unwrap();
        assert_eq!(calls, vec![(dir.join("Against_Rome.exe"), dir.clone())]);
        let mut failing = |_: &Path, _: &Path| Err(std::io::Error::other("boom"));
        assert!(launch_game(&dir, &mut failing)
            .unwrap_err()
            .contains("boom"));
        let _ = std::fs::remove_dir_all(&dir);
    }

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
    fn diff_report_data_labels_do_not_follow_ui_language() {
        let textures = crate::textures_ui::TexturesUi::default();
        let placement = crate::placement_ui::PlacementUi::default();
        let scenario = crate::scenario_ui::ScenarioUi::default();
        let live = LiveDiffSessions::from_sessions(&textures, &placement, &scenario);
        let heights = vec![32u8; 16 * 16];
        for english in [false, true] {
            let mut panel = PackagingUi { english, open: true, current_tab: PackagingTab::MapDiff, ..Default::default() };
            crate::batch20_localization_tests::click(
                |ctx| panel.show(ctx, 16, Some(&heights), None, None, live),
                tl(english, "擷取目前狀態為基準快照", "Capture Current as Baseline"));
            crate::batch20_localization_tests::click(
                |ctx| panel.show(ctx, 16, Some(&heights), None, None, live),
                tl(english, "比對目前編輯與基準快照", "Compare Working vs Baseline"));
            let report = panel.diff_report.as_ref().expect("已產生比對報告");
            assert_eq!(report.baseline_label, "Working Baseline");
            assert_eq!(report.current_label, "Latest Edits");
        }
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

    #[test]
    fn test_packaging_default_export_path_filled_and_never_inside_game_maps_dir() {
        let mut ui = PackagingUi::default();
        assert!(ui.output_zip_input.is_empty());

        let fake_game_map = Path::new(r"C:\Program Files (x86)\Against Rome\MAPS\ENDL_005");
        ui.reset_session(Some(fake_game_map));

        assert!(!ui.output_zip_input.is_empty());
        assert!(
            ui.output_zip_input.ends_with("ENDL_005.zip"),
            "Expected output_zip_input to end with ENDL_005.zip, got: {}",
            ui.output_zip_input
        );
        let out_path = PathBuf::from(&ui.output_zip_input);
        assert!(
            !is_game_maps_dir(&out_path),
            "Export path must never be inside game MAPS directory: {}",
            out_path.display()
        );
        assert!(
            !out_path.starts_with(fake_game_map),
            "Export path must never be inside the map folder itself: {}",
            out_path.display()
        );

        // Custom map folder
        let custom_map = Path::new(r"D:\Modding\MyCustomMaps\SiegeOfRome");
        ui.reset_session(Some(custom_map));
        assert!(ui.output_zip_input.ends_with("SiegeOfRome.zip"));
        let out_path = PathBuf::from(&ui.output_zip_input);
        assert!(!is_game_maps_dir(&out_path));
        assert!(!out_path.starts_with(custom_map));

        // Reset to None (blank map or closed session) clears the output zip input
        ui.reset_session(None);
        assert!(ui.output_zip_input.is_empty());
    }

    #[test]
    fn test_packaging_browse_picker_sets_path_and_cancel_preserves() {
        let mut ui = PackagingUi::default();
        let initial_path = r"C:\Users\User\Documents\initial_export.zip";
        ui.output_zip_input = initial_path.to_string();

        // 1. Injected picker returns Some(path)
        let picked_path = PathBuf::from(r"D:\Modding\Exports\custom_pack.zip");
        let picked_clone = picked_path.clone();
        ui.layout_picker = Some(Box::new(move |kind| {
            if let LayoutDialogKind::Save { suggested_name } = kind {
                assert_eq!(suggested_name, "initial_export.zip");
            }
            Some(picked_clone.clone())
        }));

        ui.browse_export_zip();
        assert_eq!(ui.output_zip_input, picked_path.to_string_lossy().to_string());

        // 2. Injected picker returns None (user cancels)
        ui.layout_picker = Some(Box::new(|_| None));
        ui.browse_export_zip();
        assert_eq!(
            ui.output_zip_input,
            picked_path.to_string_lossy().to_string(),
            "Cancelled picker must leave the output zip input unchanged"
        );

        // 3. Test export_picker alias
        let export_picked = PathBuf::from(r"D:\Modding\Exports\alias_pack.zip");
        let export_clone = export_picked.clone();
        ui.layout_picker = None;
        ui.export_picker = Some(Box::new(move |_| Some(export_clone.clone())));
        ui.browse_export_zip();
        assert_eq!(ui.output_zip_input, export_picked.to_string_lossy().to_string());
    }

    #[test]
    fn test_packaging_overwrite_confirmation_behavior() {
        let dir = crate::packaging_ui::temp_fixture::dir("ar_packaging_overwrite");
        let existing_zip = dir.join("existing.zip");
        std::fs::write(&existing_zip, b"dummy zip content").unwrap();
        let non_existent_zip = dir.join("brand_new.zip");

        let mut ui = PackagingUi::default();
        ui.map_dir_input = dir.to_string_lossy().to_string();

        // The host app decides whether to block, request overwrite confirmation, or export.
        ui.output_zip_input = existing_zip.to_string_lossy().to_string();
        ui.request_export();
        assert!(ui.pending_export_request.is_some());
        assert!(!ui.pending_overwrite_confirm);
        let (_, confirmed, _, target) = ui.take_export_request().unwrap();
        assert!(confirmed.is_none());
        assert_eq!(target, existing_zip);

        // A new target also becomes a host request; file existence is checked at execution time.
        ui.output_zip_input = non_existent_zip.to_string_lossy().to_string();
        ui.request_export();
        assert!(!ui.pending_overwrite_confirm);
        assert!(ui.take_export_request().is_some());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_destination_must_be_zip_file_not_folder_or_other_extension() {
        let dir = crate::packaging_ui::temp_fixture::dir("ar_packaging_zip_ext");
        let map = dir.join("map");
        std::fs::create_dir_all(&map).unwrap();
        let folder = dir.join("out.zip");
        std::fs::create_dir_all(&folder).unwrap();

        let mut ui = PackagingUi::default();
        ui.map_dir_input = map.to_string_lossy().to_string();
        for bad in [dir.join("pack.txt"), dir.join("pack"), folder.clone()] {
            ui.output_zip_input = bad.to_string_lossy().to_string();
            let err = ui.run_export().unwrap_err();
            assert!(err.contains("ZIP"), "{err}");
            assert!(ui.status_is_error);
        }
        assert!(validate_export_destination(&map, &dir.join("OK.ZIP")).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_packaging_bilingual_labels() {
        assert_eq!(tr(true, "Hello", "你好"), "Hello");
        assert_eq!(tr(false, "Hello", "你好"), "你好");
        assert_eq!(tl(true, "你好", "Hello"), "Hello");
        assert_eq!(tl(false, "你好", "Hello"), "你好");
    }

    #[test]
    fn chinese_ui_has_no_untranslated_literals() {
        let mut panel = PackagingUi::default();
        panel.open = true;
        let textures = crate::textures_ui::TexturesUi::default();
        let placement = crate::placement_ui::PlacementUi::default();
        let scenario = crate::scenario_ui::ScenarioUi::default();
        let live = LiveDiffSessions::from_sessions(&textures, &placement, &scenario);

        panel.current_tab = PackagingTab::MapDiff;
        for english in [true, false, true, false] {
            panel.english = english;
            let text = crate::batch20_localization_tests::render(|ctx| {
                panel.show(ctx, 256, None, None, None, live);
            });
            assert!(
                text.contains(if english { "Map Version Diff Engine" } else { "地圖版本比對引擎" }),
                "{text}"
            );
            if !english {
                for literal in [
                    "Map Version Diff Engine",
                    "Compares working memory edits against a baseline snapshot",
                    "Capture Current as Baseline",
                    "Compare Working vs Baseline",
                    "Saved baseline map folder:",
                    "Load Saved Baseline",
                    "Baseline: in-memory",
                    "No diff report available",
                ] {
                    assert!(!text.contains(literal), "未翻譯：{literal}\n{text}");
                }
            }
        }

        panel.current_tab = PackagingTab::BudgetProfiler;
        for english in [true, false, true, false] {
            panel.english = english;
            let text = crate::batch20_localization_tests::render(|ctx| {
                panel.show(ctx, 256, None, None, None, live);
            });
            assert!(
                text.contains(if english { "Map Budget & Performance Profiler" } else { "地圖資源預算與效能分析器" }),
                "{text}"
            );
            if !english {
                for literal in [
                    "Map Budget & Performance Profiler",
                    "Analyzes object slots, light cluster hotspots",
                    "Run Budget Profiler",
                ] {
                    assert!(!text.contains(literal), "未翻譯：{literal}\n{text}");
                }
            }
        }
    }
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
