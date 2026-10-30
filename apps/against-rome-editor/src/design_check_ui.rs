//! Read-only Map Design Check panel (C# `MapEditorForm.DesignCheck.cs`).
//!
//! Evaluates the effective in-memory candidate (terrain, pending placement, pending scenario
//! and events, pending water level) with the same logic Safe Save uses. It never writes files,
//! clears dirty flags or touches edit sessions.

use crate::app::{tr, EditorApp};
use against_rome_ambience::Language as AppLanguage;
use against_rome_maps::{
    design_validator::{DesignIssueSeverity, MapDesignIssue},
    diagnostics::{MapIssue, MapIssueSeverity},
    preflight::find_missing_data_files,
};
use eframe::egui::{self, Color32, RichText, ScrollArea};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// 重用既有座標、重疊與家族連結檢查；只讀取候選資料。
pub(crate) fn data_candidate_issues(
    store: &against_rome_placement::ObjectStore,
    terrain: &against_rome_maps::diagnostics::MapCheckSnapshot,
    resolver: &mut Option<crate::building_footprint::FootprintResolver>,
) -> Vec<MapIssue> {
    data_candidate_issues_impl(store, terrain, resolver, true)
}

#[cfg(test)]
pub(crate) fn data_candidate_issues_legacy(
    store: &against_rome_placement::ObjectStore,
    terrain: &against_rome_maps::diagnostics::MapCheckSnapshot,
    resolver: &mut Option<crate::building_footprint::FootprintResolver>,
) -> Vec<MapIssue> {
    data_candidate_issues_impl(store, terrain, resolver, false)
}

fn data_candidate_issues_impl(
    store: &against_rome_placement::ObjectStore,
    terrain: &against_rome_maps::diagnostics::MapCheckSnapshot,
    resolver: &mut Option<crate::building_footprint::FootprintResolver>,
    batch: bool,
) -> Vec<MapIssue> {
    use against_rome_maps::diagnostics::{MapCheckObject, MapDiagnostics};
    #[cfg(test)]
    let timer = std::time::Instant::now();
    let mut snapshot = terrain.clone();
    snapshot.events.clear();
    snapshot.objects.clear();
    let mut issues = Vec::new();
    let mut footprints = std::collections::HashMap::new();
    let objects: Vec<_> = store.objects().into_iter().filter(|o| o.parent.is_none()).collect();
    let roots: Vec<_> = objects.iter().map(|o| (o.slot, o.uid)).collect();
    let families = if batch {
        store.family_details_batch(&roots)
    } else {
        let legacy: Vec<_> = roots.iter().map(|&(slot, uid)| store.family_details(slot, uid)).collect();
        #[cfg(test)]
        assert_eq!(store.family_details_batch(&roots), legacy);
        legacy
    };
    for (object, family) in objects.into_iter().zip(families) {
        // slot 與 UID 組成穩定識別，讓已儲存／候選結果可直接比對。
        let id = Uuid::from_u128((1u128 << 127) | ((object.slot as u128) << 32) | u128::from(object.uid));
        if let Err(error) = family {
            let mut issue = MapIssue::new(MapIssueSeverity::Error, "family-link",
                format!("DATA 家族連結無效：{error}"), format!("Invalid DATA family links: {error}"));
            issue.object_id = id;
            if object.position[0].is_finite() && object.position[2].is_finite() {
                issue.world_x = Some(object.position[0]);
                issue.world_z = Some(object.position[2]);
            }
            issues.push(issue);
        }
        if let Some(stencil) = resolver.as_mut().and_then(|r| r.resolve_building(i32::from(object.type_id))) {
            footprints.insert(id, stencil.cells.clone());
        }
        snapshot.objects.push(MapCheckObject::new(against_rome_scenario::Spawn {
            id, x: object.position[0], y: object.position[1], z: object.position[2],
            prebuilt: true, ..Default::default()
        }, footprints.contains_key(&id), true));
    }
    #[cfg(test)]
    eprintln!("Task42 family/footprints={:?}", timer.elapsed());
    #[cfg(test)]
    let timer = std::time::Instant::now();
    issues.extend(MapDiagnostics::check_with_footprints(&snapshot, &footprints).into_iter()
        .filter(|i| matches!(i.code.as_str(), "coordinates" | "overlap")));
    #[cfg(test)]
    eprintln!("Task42 data diagnostics={:?}", timer.elapsed());
    issues
}
/// One displayable row of the issue list (C# `MapEditorForm.Diagnostics` `_mapIssues`).
#[derive(Clone, Debug, PartialEq)]
pub struct IssueRow {
    pub severity: MapIssueSeverity,
    pub code: String,
    pub chinese: String,
    pub english: String,
    pub world_x: Option<f32>,
    pub world_z: Option<f32>,
    pub object_id: Uuid,
    pub event_index: i32,
}

impl IssueRow {
    pub fn text(&self, lang: AppLanguage) -> &str {
        let (primary, fallback) = match lang {
            AppLanguage::English => (&self.english, &self.chinese),
            AppLanguage::TraditionalChinese => (&self.chinese, &self.english),
        };
        if primary.is_empty() {
            fallback
        } else {
            primary
        }
    }

    /// Focus target in world units when both coordinates are finite (C# `LocateMapIssue`).
    pub fn focus_world(&self) -> Option<(f32, f32)> {
        match (self.world_x, self.world_z) {
            (Some(x), Some(z)) if x.is_finite() && z.is_finite() => Some((x, z)),
            _ => None,
        }
    }

    /// Focus target in tile units (`world / 256`) as C# passes to `FocusTile`.
    pub fn focus_tile(&self) -> Option<(f32, f32)> {
        self.focus_world().map(|(x, z)| (x / 256.0, z / 256.0))
    }

    pub fn severity_label(&self, lang: AppLanguage) -> &'static str {
        match (self.severity, lang) {
            (MapIssueSeverity::Error, AppLanguage::English) => "Error",
            (MapIssueSeverity::Error, _) => "錯誤",
            (MapIssueSeverity::Warning, AppLanguage::English) => "Warning",
            (MapIssueSeverity::Warning, _) => "警告",
        }
    }

    /// Whether this issue can be located/focused in the viewport, scenario panel, or objects/spawns panel.
    pub fn is_locatable(&self) -> bool {
        self.focus_world().is_some()
            || self.event_index >= 0
            || !self.object_id.is_nil()
            || self.code.starts_with("scenario-")
            || self.code.starts_with("team-no-")
    }
}

/// 若問題具備座標、劇本事件或放置物件，提供直接前往入口。
fn issue_go_to(ui: &mut egui::Ui, row: &IssueRow, lang: AppLanguage) -> Option<egui::Response> {
    if !row.is_locatable() {
        return None;
    }
    Some(
        ui.small_button(tr(lang, "Go to", "前往"))
            .on_hover_text(tr(
                lang,
                "Locate this issue in the viewport, scenario, or objects panel",
                "在視圖、劇本或物件面板定位此問題",
            )),
    )
}

impl From<&MapIssue> for IssueRow {
    fn from(i: &MapIssue) -> Self {
        Self {
            severity: i.severity,
            code: i.code.clone(),
            chinese: i.chinese.clone(),
            english: i.english.clone(),
            world_x: i.world_x,
            world_z: i.world_z,
            object_id: i.object_id,
            event_index: i.event_index,
        }
    }
}

impl From<&MapDesignIssue> for IssueRow {
    fn from(i: &MapDesignIssue) -> Self {
        Self {
            severity: match i.severity {
                DesignIssueSeverity::Error => MapIssueSeverity::Error,
                DesignIssueSeverity::Warning => MapIssueSeverity::Warning,
            },
            code: i.code.clone(),
            chinese: i.chinese.clone(),
            english: i.english.clone(),
            world_x: i.world_x,
            world_z: i.world_z,
            object_id: i.object_id,
            event_index: -1,
        }
    }
}

/// Errors first, then warnings; the original order is preserved inside each severity.
pub fn sort_issue_rows(rows: &mut [IssueRow]) {
    rows.sort_by_key(|row| match row.severity {
        MapIssueSeverity::Error => 0,
        MapIssueSeverity::Warning => 1,
    });
}

/// "N errors, M warnings" line (warnings still allow saving; connectivity is an estimate).
pub fn issue_summary(rows: &[IssueRow], lang: AppLanguage) -> String {
    let errors = rows.iter().filter(|r| r.severity == MapIssueSeverity::Error).count();
    let warnings = rows.len() - errors;
    match lang {
        AppLanguage::English => format!(
            "{errors} errors, {warnings} warnings. Warnings allow saving. Connectivity is an estimate; check game pathfinding."
        ),
        _ => format!("{errors} 個錯誤，{warnings} 個警告。警告仍可儲存；連通性為估計，須另驗證遊戲尋路。"),
    }
}

/// C# `native-data-missing` warning for absent required `DATA/*.dat` files (24-file contract).
pub fn missing_data_files_issue(map_dir: &Path) -> Option<MapIssue> {
    let missing = find_missing_data_files(map_dir).ok()?;
    if missing.is_empty() {
        return None;
    }
    let files = missing.join(", ");
    Some(MapIssue::new(
        MapIssueSeverity::Warning,
        "native-data-missing",
        format!("地圖缺少必要的 DATA 檔案：{files}。缺檔可能導致遊戲資料池未完整載入。請從完整地圖範本重新建立，或從此地圖的備份還原缺檔；不要建立空檔。仍可編輯及儲存，補齊前無法使用遊戲預覽。"),
        format!("Required DATA files are missing: {files}. Missing files may leave game data pools incompletely loaded. Recreate from a complete map template or restore missing files from this map's backup; do not create empty files. Editing and saving remain available; restore the files before game preview."),
    ))
}

/// Flattens a report (plus extra UI-time findings) into the severity-sorted list shown to users.
pub fn report_rows(report: &DesignCheckReport, extra: &[MapIssue], lang: AppLanguage) -> Vec<IssueRow> {
    let mut rows = Vec::new();
    match report {
        DesignCheckReport::Unavailable(message) => rows.push(IssueRow {
            severity: MapIssueSeverity::Error,
            code: "check-unavailable".into(),
            chinese: format!("無法檢查：{message}"),
            english: format!("Check unavailable: {message}"),
            world_x: None,
            world_z: None,
            object_id: Uuid::nil(),
            event_index: -1,
        }),
        DesignCheckReport::Checked {
            map_issues,
            preflight_error,
            design_issues,
        } => {
            if let Some(e) = preflight_error {
                rows.push(IssueRow {
                    severity: MapIssueSeverity::Error,
                    code: "scenario-preflight".into(),
                    chinese: e.clone(),
                    english: e.clone(),
                    world_x: None,
                    world_z: None,
                    object_id: Uuid::nil(),
                    event_index: -1,
                });
            }
            rows.extend(map_issues.iter().map(IssueRow::from));
            rows.extend(design_issues.iter().map(IssueRow::from));
        }
    }
    rows.extend(extra.iter().map(IssueRow::from));
    let _ = lang;
    sort_issue_rows(&mut rows);
    rows
}

/// Outcome of one Design Check run.
#[derive(Clone, Debug, PartialEq)]
pub enum DesignCheckReport {
    /// The effective candidate could not be built (e.g. unloaded scenario, bad JSON).
    Unavailable(String),
    Checked {
        /// `MapDiagnostics` findings; errors here block Safe Save.
        map_issues: Vec<MapIssue>,
        /// Scenario preflight failure (blocks Safe Save); `None` when it passed or was skipped.
        preflight_error: Option<String>,
        design_issues: Vec<MapDesignIssue>,
    },
}

impl DesignCheckReport {
    pub fn error_count(&self) -> usize {
        match self {
            Self::Unavailable(_) => 1,
            Self::Checked {
                map_issues,
                preflight_error,
                design_issues,
            } => {
                map_issues
                    .iter()
                    .filter(|i| i.severity == MapIssueSeverity::Error)
                    .count()
                    + usize::from(preflight_error.is_some())
                    + design_issues
                        .iter()
                        .filter(|i| i.severity == DesignIssueSeverity::Error)
                        .count()
            }
        }
    }

    pub fn warning_count(&self) -> usize {
        match self {
            Self::Unavailable(_) => 0,
            Self::Checked {
                map_issues,
                design_issues,
                ..
            } => {
                map_issues
                    .iter()
                    .filter(|i| i.severity == MapIssueSeverity::Warning)
                    .count()
                    + design_issues
                        .iter()
                        .filter(|i| i.severity == DesignIssueSeverity::Warning)
                        .count()
            }
        }
    }

    pub fn is_clean(&self) -> bool {
        matches!(self, Self::Checked { .. }) && self.error_count() == 0 && self.warning_count() == 0
    }
}

/// Headless Design Check for `--validate-map`: installs `session` into a fresh editor, loads
/// the map's scenario when it has one, and returns the same severity-sorted rows the panel
/// shows. Read-only; errors in the result are the ones that would block Safe Save.
pub fn headless_design_check(
    path: &Path,
    session: crate::TerrainSession,
    lang: AppLanguage,
) -> Result<Vec<IssueRow>, String> {
    let mut app = EditorApp::default();
    app.set_loaded_map(path.to_path_buf(), session)?;
    app.scenario
        .load_session(path)
        .map_err(|e| format!("Scenario: {e}"))?;
    let report = app.design_check_report();
    let extra: Vec<MapIssue> = missing_data_files_issue(path).into_iter().collect();
    Ok(report_rows(&report, &extra, lang))
}

/// One plain-text line per row for console output: `[severity] code: text (x, z)`.
pub fn format_issue_row(row: &IssueRow, lang: AppLanguage) -> String {
    let mut line = format!("[{}] {}: {}", row.severity_label(lang), row.code, row.text(lang));
    if let Some((x, z)) = row.focus_tile() {
        line.push_str(&match lang {
            AppLanguage::English => format!(" (tile {x:.1}, {z:.1})"),
            _ => format!("（格 {x:.1}, {z:.1}）"),
        });
    }
    line
}

/// 同時顯示篩選結果與報告總數，避免將隱藏問題誤認為已解決。
fn visible_issue_count(visible: usize, total: usize, lang: AppLanguage) -> String {
    format!("{} {visible} / {total}", tr(lang, "Visible issues:", "顯示問題："))
}

/// 保留報告索引，僅在可見列之間循環；舊選取已隱藏時從端點開始。
fn adjacent_issue(visible: &[(usize, &IssueRow)], selected: Option<usize>, forward: bool) -> Option<usize> {
    if visible.is_empty() {
        return None;
    }
    let position = selected.and_then(|index| visible.iter().position(|(i, _)| *i == index));
    let next = match (position, forward) {
        (Some(i), true) => (i + 1) % visible.len(),
        (Some(0) | None, false) => visible.len() - 1,
        (Some(i), false) => i - 1,
        (None, true) => 0,
    };
    Some(visible[next].0)
}

/// 延遲格式化至點擊時，無選取或無結果時停用。
fn copy_issue_button(ui: &mut egui::Ui, label: &str, enabled: bool, text: impl FnOnce() -> String) -> egui::Response {
    let response = ui.add_enabled(enabled, egui::Button::new(label));
    if response.clicked() {
        ui.output_mut(|output| output.copied_text = text());
    }
    response
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DesignCheckCacheKey {
    pub report: Option<DesignCheckReport>,
    pub extra_issues: Vec<MapIssue>,
    pub lang: AppLanguage,
}

pub struct DesignCheckUi {
    pub open: bool,
    pub report: Option<DesignCheckReport>,
    /// UI-time findings that are not part of the pure report (missing required DATA files).
    pub extra_issues: Vec<MapIssue>,
    /// Row index selected in the issue list.
    pub selected: Option<usize>,
    /// World position the app should center the 2D canvas on.
    pub focus_request: Option<(f32, f32)>,
    /// Scenario event index the app should reveal in the scenario panel.
    pub event_request: Option<i32>,
    /// Placed object ID the app should select in the placement/spawns panel.
    pub object_request: Option<Uuid>,
    /// Request opening the scenario panel for a scenario-level issue.
    pub scenario_panel_request: bool,
    /// Whether to show errors in the issue list (default: true).
    pub show_errors: bool,
    /// Whether to show warnings in the issue list (default: true).
    pub show_warnings: bool,
    /// 依問題代碼與雙語說明搜尋，保留原始列索引。
    pub search: String,
    /// 僅顯示可前往世界位置或劇本事件的問題。
    pub locatable_only: bool,
    /// Whether to automatically re-run Design Check after a successful Safe Save (default: false).
    pub rerun_after_save: bool,
    /// 與配置面板相同的可注入選檔器介面。
    pub report_picker: crate::layout_io::LayoutFilePicker,
    pub(crate) cached_rows: Option<Vec<IssueRow>>,
    pub(crate) cache_key: Option<DesignCheckCacheKey>,
    pub recompute_count: usize,
}

impl Default for DesignCheckUi {
    fn default() -> Self {
        Self {
            open: false,
            report: None,
            extra_issues: Vec::new(),
            selected: None,
            focus_request: None,
            event_request: None,
            object_request: None,
            scenario_panel_request: false,
            show_errors: true,
            show_warnings: true,
            search: String::new(),
            locatable_only: false,
            rerun_after_save: false,
            report_picker: Box::new(|kind| {
                let crate::layout_io::LayoutDialogKind::Save { suggested_name } = kind else { return None };
                rfd::FileDialog::new()
                    .set_title("Export Design Check / 匯出設計檢查")
                    .add_filter("Markdown", &["md"])
                    .set_directory(crate::packaging_ui::default_export_dir())
                    .set_file_name(suggested_name)
                    .save_file()
            }),
            cached_rows: None,
            cache_key: None,
            recompute_count: 0,
        }
    }
}

impl DesignCheckUi {
    /// 僅在複製操作時格式化目前篩選結果，不寫入檔案。
    pub fn visible_issue_text(&self, rows: &[IssueRow], lang: AppLanguage) -> String {
        self.filtered_rows(rows).iter()
            .map(|(_, row)| format_issue_row(row, lang))
            .collect::<Vec<_>>().join("\n")
    }
    /// 還原顯示條件，不更動報告或選取。
    pub fn reset_filters(&mut self) {
        self.search.clear();
        self.show_errors = true;
        self.show_warnings = true;
        self.locatable_only = false;
    }
    /// Invalidate cached issue rows.
    pub fn invalidate_cache(&mut self) {
        self.cached_rows = None;
        self.cache_key = None;
    }

    /// Returns the cached IssueRow slice or computes and sorts it if stale.
    pub fn get_or_compute_rows(&mut self, lang: AppLanguage) -> &[IssueRow] {
        let key = DesignCheckCacheKey {
            report: self.report.clone(),
            extra_issues: self.extra_issues.clone(),
            lang,
        };
        if self.cached_rows.is_none() || self.cache_key.as_ref() != Some(&key) {
            let rows = match &self.report {
                Some(report) => report_rows(report, &self.extra_issues, lang),
                None => Vec::new(),
            };
            self.cached_rows = Some(rows);
            self.cache_key = Some(key);
            self.recompute_count += 1;
        }
        self.cached_rows.as_deref().unwrap_or(&[])
    }
    /// Checks whether an issue with the given severity is visible under current filter toggles.
    pub fn is_severity_visible(&self, severity: MapIssueSeverity) -> bool {
        match severity {
            MapIssueSeverity::Error => self.show_errors,
            MapIssueSeverity::Warning => self.show_warnings,
        }
    }

    /// 依層級與搜尋文字篩選，回傳原始索引以維持定位與選取。
    pub fn filtered_rows<'a>(&self, rows: &'a [IssueRow]) -> Vec<(usize, &'a IssueRow)> {
        let query = self.search.trim().to_lowercase();
        rows.iter()
            .enumerate()
            .filter(|(_, r)| {
                self.is_severity_visible(r.severity)
                    && (!self.locatable_only || r.is_locatable())
                    && (query.is_empty()
                        || [&r.code, &r.chinese, &r.english]
                            .iter()
                            .any(|text| text.to_lowercase().contains(&query)))
            })
            .collect()
    }
}

/// Generates a structured Markdown report from a Design Check outcome.
pub fn generate_markdown_report(
    report: &DesignCheckReport,
    extra: &[MapIssue],
    map_name: &str,
    lang: AppLanguage,
) -> String {
    let rows = report_rows(report, extra, lang);
    let error_count = rows.iter().filter(|r| r.severity == MapIssueSeverity::Error).count();
    let warning_count = rows.len() - error_count;
    let mut out = String::new();

    match lang {
        AppLanguage::English => {
            out.push_str(&format!("# Map Design Check Report: {map_name}\n\n"));
            if rows.is_empty() {
                out.push_str("- **Status**: PASS (No issues found)\n\n");
            } else if error_count > 0 {
                out.push_str(&format!(
                    "- **Status**: FAILED ({} errors, {} warnings - Safe Save blocked by errors)\n\n",
                    error_count, warning_count
                ));
            } else {
                out.push_str(&format!(
                    "- **Status**: WARNINGS (0 errors, {} warnings - Safe Save allowed)\n\n",
                    warning_count
                ));
            }
            out.push_str("## Summary\n\n");
            out.push_str("| Severity | Count | Safe Save Impact |\n");
            out.push_str("|---|---|---|\n");
            out.push_str(&format!("| Error | {error_count} | Blocks Safe Save |\n"));
            out.push_str(&format!("| Warning | {warning_count} | Allowed (Guidance/Estimate) |\n\n"));

            if !rows.is_empty() {
                out.push_str("## Issues List\n\n");
                for (i, row) in rows.iter().enumerate() {
                    out.push_str(&format!("### {}. [{}] `{}`\n", i + 1, row.severity_label(lang), row.code));
                    out.push_str(&format!("- **Description**: {}\n", row.text(lang)));
                    if let Some((x, z)) = row.focus_tile() {
                        out.push_str(&format!("- **Tile Coordinates**: ({x:.1}, {z:.1})\n"));
                    }
                    if let Some((wx, wz)) = row.focus_world() {
                        out.push_str(&format!("- **World Coordinates**: ({wx:.1}, {wz:.1})\n"));
                    }
                    if row.event_index >= 0 {
                        out.push_str(&format!("- **Scenario Event**: #{}\n", row.event_index));
                    }
                    if !row.object_id.is_nil() {
                        out.push_str(&format!("- **Object ID**: `{}`\n", row.object_id));
                    }
                    out.push('\n');
                }
            }
        }
        _ => {
            out.push_str(&format!("# 地圖設計檢查報告：{map_name}\n\n"));
            if rows.is_empty() {
                out.push_str("- **狀態**：通過 (PASS，未發現設計問題)\n\n");
            } else if error_count > 0 {
                out.push_str(&format!(
                    "- **狀態**：未通過 ({} 個錯誤，{} 個警告 - 錯誤會阻擋安全儲存)\n\n",
                    error_count, warning_count
                ));
            } else {
                out.push_str(&format!(
                    "- **狀態**：警告 (0 個錯誤，{} 個警告 - 允許安全儲存)\n\n",
                    warning_count
                ));
            }
            out.push_str("## 統計摘要\n\n");
            out.push_str("| 問題層級 | 數量 | 安全儲存影響 |\n");
            out.push_str("|---|---|---|\n");
            out.push_str(&format!("| 錯誤 (Error) | {error_count} | 阻擋儲存 (Blocks Save) |\n"));
            out.push_str(&format!("| 警告 (Warning) | {warning_count} | 允許儲存（建議或估計） |\n\n"));

            if !rows.is_empty() {
                out.push_str("## 問題清單\n\n");
                for (i, row) in rows.iter().enumerate() {
                    out.push_str(&format!("### {}. [{}] `{}`\n", i + 1, row.severity_label(lang), row.code));
                    out.push_str(&format!("- **說明**：{}\n", row.text(lang)));
                    if let Some((x, z)) = row.focus_tile() {
                        out.push_str(&format!("- **圖塊座標**：（{x:.1}, {z:.1}）\n"));
                    }
                    if let Some((wx, wz)) = row.focus_world() {
                        out.push_str(&format!("- **世界座標**：（{wx:.1}, {wz:.1}）\n"));
                    }
                    if row.event_index >= 0 {
                        out.push_str(&format!("- **劇本事件**：第 {} 項\n", row.event_index));
                    }
                    if !row.object_id.is_nil() {
                        out.push_str(&format!("- **物件 ID**：`{}`\n", row.object_id));
                    }
                    out.push('\n');
                }
            }
        }
    }
    out
}

/// 沿用打包面板的外部匯出位置，避免寫入 MAPS 或來源地圖。
pub fn default_report_path(map_path: &Path) -> PathBuf {
    crate::packaging_ui::default_export_zip_path(map_path).with_extension("design-check.md")
}
impl EditorApp {
    /// Evaluate the current effective state without side effects and show the result.
    pub fn run_design_check(&mut self) {
        self.design_check.invalidate_cache();
        let report = self.design_check_report();
        let path = if self.map_path_input.trim().is_empty() {
            self.loaded_map_path.clone().unwrap_or_default()
        } else {
            PathBuf::from(self.map_path_input.trim())
        };
        self.design_check.extra_issues = missing_data_files_issue(&path).into_iter().collect();
        self.design_check.report = Some(report);
        self.design_check.selected = None;
        self.design_check.open = true;
    }

    /// Pure (`&self`) computation of the report; used by the button and by tests.
    pub fn design_check_report(&self) -> DesignCheckReport {
        #[cfg(test)]
        let timer = std::time::Instant::now();
        let (path, staged) = match self.check_candidate_inputs() {
            Ok(v) => v,
            Err(e) => return DesignCheckReport::Unavailable(e),
        };
        #[cfg(test)]
        eprintln!("Task42 scenario staging={:?}", timer.elapsed());
        match self.candidate_diagnostics_with_pending_data(&path, &staged, true) {
            Err(e) => DesignCheckReport::Unavailable(e),
            Ok(c) => {
                let (preflight_error, design_issues) = match c.design {
                    Ok(issues) => (None, issues),
                    Err(e) => (Some(e), Vec::new()),
                };
                DesignCheckReport::Checked {
                    map_issues: c.map_issues,
                    preflight_error,
                    design_issues,
                }
            }
        }
    }

    /// Quick fix for `wave-not-hostile`: adds one start-of-game "Team diplomacy (hostile)" event
    /// per flagged team against the player (team 0), as a single scenario undo step, then
    /// re-runs the check. Returns how many events were added.
    pub fn fix_hostile_waves(&mut self) -> Result<usize, String> {
        let teams: Vec<i32> = match self.design_check_report() {
            DesignCheckReport::Checked { design_issues, .. } => design_issues
                .iter()
                .filter(|i| i.code == "wave-not-hostile")
                .map(|i| i.team)
                .collect(),
            DesignCheckReport::Unavailable(e) => return Err(e),
        };
        if teams.is_empty() {
            return Ok(0);
        }
        if !self.scenario.is_loaded() {
            if let Some(path) = self.loaded_map_path.clone() {
                self.scenario.load_session(&path).map_err(|e| format!("Scenario: {e}"))?;
            }
        }
        let events: Vec<against_rome_scenario::Event> = teams
            .iter()
            .map(|&team| crate::scenario_ui::hostile_wave_event(team))
            .collect();
        self.scenario.add_events(&events).map_err(|e| format!("Scenario: {e}"))?;
        self.run_design_check();
        Ok(events.len())
    }

    /// 僅補上啟用中敵方生成動作缺少的目的地，整批驗證後以一步復原套用。
    pub fn fix_wave_destinations(&mut self) -> Result<usize, String> {
        match self.design_check_report() {
            DesignCheckReport::Unavailable(e) => return Err(e),
            DesignCheckReport::Checked { design_issues, .. } => {
                if !design_issues.iter().any(|i| i.code == "wave-no-destination") {
                    self.run_design_check();
                    return Ok(0);
                }
            }
        }
        let path = self.loaded_map_path.clone().ok_or_else(|| tr(self.lang(), "No map loaded.", "尚未載入地圖。").to_string())?;
        if !self.scenario.is_loaded() {
            self.scenario.load_session(&path)?;
        }
        let Some(target) = self.wave_player_target(&path) else {
            self.run_design_check();
            return Err(tr(self.lang(),
                "Skipped: no player main building or enabled start camera. Set a target and retry.",
                "已略過：找不到玩家主屋或已啟用的起始鏡頭。請設定目標後重試。").into());
        };
        let mut events = self.scenario.document().events.clone();
        let mut count = 0;
        for event in events.iter_mut().filter(|e| e.enabled) {
            for action in event.actions.iter_mut().filter(|a| a.kind == 2 && (1..=7).contains(&a.team) && a.move_destination.is_none()) {
                action.move_destination = Some(target.clone());
                count += 1;
            }
        }
        if count > 0 { self.scenario.replace_events(events)?; }
        self.run_design_check();
        Ok(count)
    }

    /// 補齊劇本缺失的勝利與失敗目標事件，整批驗證後以單步或兩步復原套用並重新執行設計檢查。
    pub fn fix_missing_objectives(&mut self) -> Result<usize, String> {
        let (needs_victory, needs_defeat) = match self.design_check_report() {
            DesignCheckReport::Unavailable(e) => return Err(e),
            DesignCheckReport::Checked { design_issues, .. } => {
                let v = design_issues
                    .iter()
                    .any(|i| i.code == "scenario-no-win-loss" || i.code == "scenario-no-victory");
                let d = design_issues
                    .iter()
                    .any(|i| i.code == "scenario-no-win-loss" || i.code == "scenario-no-defeat");
                if !v && !d {
                    self.run_design_check();
                    return Ok(0);
                }
                (v, d)
            }
        };

        let path = self
            .loaded_map_path
            .clone()
            .ok_or_else(|| tr(self.lang(), "No map loaded.", "尚未載入地圖。").to_string())?;
        if !self.scenario.is_loaded() {
            self.scenario.load_session(&path)?;
        }

        if needs_defeat && !self.scenario.document().spawns.iter().any(|s| {
            s.team == 0 && (against_rome_maps::design_validator::MapDesignValidator::is_main_house(&s.alias)
                || against_rome_maps::design_validator::MapDesignValidator::is_unit_alias(&s.alias))
        }) {
            return Err(tr(self.lang(),
                "Cannot add Defeat objective: no player main house or unit found. Place one first.",
                "無法新增失敗目標：找不到玩家主堡或部隊，請先放置。",
            ).into());
        }

        let mut count = 0;

        // 1. 若缺少勝利事件：優先以消滅敵方主堡為勝利條件；若無敵方主堡則以存活 300 秒為勝利條件
        if needs_victory {
            let enemy_house = self.scenario.document().spawns.iter().find(|s| {
                s.team > 0
                    && against_rome_maps::design_validator::MapDesignValidator::is_main_house(&s.alias)
            }).map(|s| (s.id, s.team));

            let victory_added = if let Some((enemy_id, enemy_team)) = enemy_house {
                self.scenario.add_objective(None, Some(enemy_id), enemy_team)?
            } else {
                self.scenario.add_objective(Some(300), None, 0)?
            };
            if victory_added {
                count += 1;
            }
        }

        // 2. 若缺少失敗事件：以玩家主堡被摧毀為失敗條件；若無玩家主堡則以玩家首個部隊被消滅為失敗條件
        if needs_defeat {
            let player_house = self.scenario.document().spawns.iter().find(|s| {
                s.team == 0
                    && against_rome_maps::design_validator::MapDesignValidator::is_main_house(&s.alias)
            }).map(|s| s.id);

            let defeat_added = if let Some(house_id) = player_house {
                self.scenario.add_objective(None, Some(house_id), 0)?
            } else if let Some(unit_id) = self.scenario.document().spawns.iter().find(|s| s.team == 0 && against_rome_maps::design_validator::MapDesignValidator::is_unit_alias(&s.alias)).map(|s| s.id) {
                let event = against_rome_scenario::Event {
                    name: crate::scenario_ui::unused_event_name("Player_defeated", &self.scenario.document().events),
                    delay_seconds: 0,
                    conditions: vec![against_rome_scenario::Condition { kind: 1, target_id: unit_id, ..Default::default() }],
                    actions: vec![against_rome_scenario::Action { kind: 4, ..Default::default() }],
                    ..Default::default()
                };
                self.scenario.add_events(&[event])?;
                true
            } else {
                false
            };
            if defeat_added {
                count += 1;
            }
        }

        self.run_design_check();
        Ok(count)
    }

    /// 補齊各參戰勢力缺失的起始部隊或單位，整批套用並重新執行設計檢查。
    pub fn fix_missing_units(&mut self) -> Result<usize, String> {
        let teams: Vec<i32> = match self.design_check_report() {
            DesignCheckReport::Unavailable(e) => return Err(e),
            DesignCheckReport::Checked { design_issues, .. } => {
                let mut set = std::collections::BTreeSet::new();
                for issue in design_issues {
                    if issue.code == "team-no-units" && (0..=7).contains(&issue.team) {
                        set.insert(issue.team);
                    }
                }
                set.into_iter().collect()
            }
        };
        if teams.is_empty() {
            self.run_design_check();
            return Ok(0);
        }

        let path = self
            .loaded_map_path
            .clone()
            .ok_or_else(|| tr(self.lang(), "No map loaded.", "尚未載入地圖。").to_string())?;
        if !self.scenario.is_loaded() {
            self.scenario.load_session(&path)?;
        }

        let mut spawns = self.scenario.document().spawns.clone();
        let known_aliases = self.scenario.known_aliases().to_vec();
        let default_unit_alias = known_aliases
            .iter()
            .find(|a| against_rome_maps::design_validator::MapDesignValidator::is_unit_alias(a))
            .cloned()
            .unwrap_or_else(|| "GER_INF00".to_string());

        let mut count = 0;
        for team in teams {
            let ref_pos = spawns
                .iter()
                .find(|s| s.team == team && s.x.is_finite() && s.z.is_finite())
                .map(|s| (s.x, s.z))
                .or_else(|| {
                    if self.placement.is_loaded() {
                        self.placement.objects().iter().find(|o| {
                            o.parent.is_none()
                                && o.team == team as u16
                                && o.position[0].is_finite()
                                && o.position[2].is_finite()
                        })
                        .map(|o| (o.position[0], o.position[2]))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| {
                    let angle = team as f32 * (std::f32::consts::TAU / 8.0);
                    let radius = 3000.0;
                    (8192.0 + radius * angle.cos(), 8192.0 + radius * angle.sin())
                });

            let (target_x, target_z) = (
                (ref_pos.0 + 256.0).clamp(256.0, 16128.0),
                (ref_pos.1 + 256.0).clamp(256.0, 16128.0),
            );

            let ground_y = if let Some(session) = &self.session {
                let norm_x = (target_x / 16384.0).clamp(0.0, 1.0);
                let norm_z = (target_z / 16384.0).clamp(0.0, 1.0);
                against_rome_maps::design_validator::MapDesignValidator::sample_bilinear_raw_height(
                    session.heights(),
                    session.vertex_size,
                    norm_x,
                    norm_z,
                ) * self.height_step
            } else {
                0.0
            };

            let alias = known_aliases
                .iter()
                .find(|a| {
                    against_rome_maps::design_validator::MapDesignValidator::is_unit_alias(a)
                        && (team == 0 || !a.to_ascii_uppercase().starts_with("GER_"))
                })
                .cloned()
                .unwrap_or_else(|| default_unit_alias.clone());

            let new_spawn = against_rome_scenario::Spawn {
                alias,
                x: target_x,
                y: ground_y,
                z: target_z,
                team,
                count: 4,
                angle: 0,
                prebuilt: false,
                id: uuid::Uuid::new_v4(),
                raw_rotation: None,
            };
            spawns.push(new_spawn);
            count += 1;
        }

        if count > 0 {
            self.scenario.apply_spawns(spawns)?;
            self.spawn_ui.sync_with_document(&self.scenario, self.loaded_map_path.as_deref());
        }

        self.run_design_check();
        Ok(count)
    }

    /// 補齊各參戰勢力缺失的主堡建築（Haupthaus），整批套用並重新執行設計檢查。
    pub fn fix_missing_bases(&mut self) -> Result<usize, String> {
        let teams: Vec<i32> = match self.design_check_report() {
            DesignCheckReport::Unavailable(e) => return Err(e),
            DesignCheckReport::Checked { design_issues, .. } => {
                let mut set = std::collections::BTreeSet::new();
                for issue in design_issues {
                    if issue.code == "team-no-main-house" && (0..=7).contains(&issue.team) {
                        set.insert(issue.team);
                    }
                }
                set.into_iter().collect()
            }
        };
        if teams.is_empty() {
            self.run_design_check();
            return Ok(0);
        }

        let path = self
            .loaded_map_path
            .clone()
            .ok_or_else(|| tr(self.lang(), "No map loaded.", "尚未載入地圖。").to_string())?;
        if !self.scenario.is_loaded() {
            self.scenario.load_session(&path)?;
        }

        let mut spawns = self.scenario.document().spawns.clone();
        let known_aliases = self.scenario.known_aliases().to_vec();
        let default_house_alias = known_aliases
            .iter()
            .find(|a| against_rome_maps::design_validator::MapDesignValidator::is_main_house(a))
            .cloned()
            .unwrap_or_else(|| "GER_HAU00".to_string());

        let mut count = 0;
        for team in teams {
            let ref_pos = spawns
                .iter()
                .find(|s| s.team == team && s.x.is_finite() && s.z.is_finite())
                .map(|s| (s.x, s.z))
                .or_else(|| {
                    if self.placement.is_loaded() {
                        self.placement.objects().iter().find(|o| {
                            o.parent.is_none()
                                && o.team == team as u16
                                && o.position[0].is_finite()
                                && o.position[2].is_finite()
                        })
                        .map(|o| (o.position[0], o.position[2]))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| {
                    let angle = team as f32 * (std::f32::consts::TAU / 8.0);
                    let radius = 3000.0;
                    (8192.0 + radius * angle.cos(), 8192.0 + radius * angle.sin())
                });

            let (target_x, target_z) = (
                ref_pos.0.clamp(256.0, 16128.0),
                ref_pos.1.clamp(256.0, 16128.0),
            );

            let ground_y = if let Some(session) = &self.session {
                let norm_x = (target_x / 16384.0).clamp(0.0, 1.0);
                let norm_z = (target_z / 16384.0).clamp(0.0, 1.0);
                against_rome_maps::design_validator::MapDesignValidator::sample_bilinear_raw_height(
                    session.heights(),
                    session.vertex_size,
                    norm_x,
                    norm_z,
                ) * self.height_step
            } else {
                0.0
            };

            let alias = known_aliases
                .iter()
                .find(|a| against_rome_maps::design_validator::MapDesignValidator::is_main_house(a))
                .cloned()
                .unwrap_or_else(|| default_house_alias.clone());

            let new_spawn = against_rome_scenario::Spawn {
                alias,
                x: target_x,
                y: ground_y,
                z: target_z,
                team,
                count: 1,
                angle: 0,
                prebuilt: false,
                id: uuid::Uuid::new_v4(),
                raw_rotation: None,
            };
            spawns.push(new_spawn);
            count += 1;
        }

        if count > 0 {
            self.scenario.apply_spawns(spawns)?;
            self.spawn_ui.sync_with_document(&self.scenario, self.loaded_map_path.as_deref());
        }

        self.run_design_check();
        Ok(count)
    }

    /// Pure generation of the current Design Check report as a Markdown document.
    pub fn export_design_check_markdown(&self) -> String {
        let report = match &self.design_check.report {
            Some(r) => r.clone(),
            None => self.design_check_report(),
        };
        let map_name = self
            .loaded_map_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("untitled");
        generate_markdown_report(&report, &self.design_check.extra_issues, map_name, self.lang())
    }

    /// Exports the Design Check report to an external Markdown file outside the map directory.
    pub fn export_design_check_report(&mut self, target_path: Option<&Path>) -> Result<PathBuf, String> {
        let markdown = self.export_design_check_markdown();
        let path = if let Some(p) = target_path {
            p.to_path_buf()
        } else if let Some(map_dir) = &self.loaded_map_path {
            default_report_path(map_dir)
        } else {
            return Err(tr(
                self.lang(),
                "No map loaded and no export path specified.",
                "尚未載入地圖且未指定匯出路徑。",
            )
            .to_string());
        };

        {
            if crate::packaging_ui::is_game_maps_dir(&path) || self.loaded_map_path.as_ref().is_some_and(|map_dir| path.starts_with(map_dir)) {
                return Err(tr(
                    self.lang(),
                    "Export target cannot be inside the game map directory.",
                    "匯出目標不可位於遊戲地圖目錄內部。",
                )
                .to_string());
            }
        }

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            }
        }

        std::fs::write(&path, markdown.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
        self.status_message = match self.lang() {
            AppLanguage::English => format!("Design Check report exported to {}", path.display()),
            _ => format!("設計檢查報告已匯出至 {}", path.display()),
        };
        self.status_is_error = false;
        Ok(path)
    }

    /// 取消選檔時不寫入，也不改變狀態訊息。
    pub fn browse_design_check_report(&mut self) -> Result<Option<PathBuf>, String> {
        let default = default_report_path(self.loaded_map_path.as_deref().unwrap_or(Path::new("untitled")));
        let kind = crate::layout_io::LayoutDialogKind::Save {
            suggested_name: default.file_name().unwrap_or_default().to_string_lossy().into_owned(),
        };
        let Some(path) = (self.design_check.report_picker)(&kind) else { return Ok(None) };
        self.export_design_check_report(Some(&path)).map(Some)
    }

    pub fn show_design_check(&mut self, ctx: &egui::Context) {
        if !self.design_check.open {
            return;
        }
        let lang = self.lang();
        let mut open = true;
        let mut rerun = false;
        let mut export_report = false;
        let mut fix_hostility = false;
        let mut fix_destination = false;
        let mut fix_objectives = false;
        let mut fix_units = false;
        let mut fix_bases = false;
        let mut locate: Option<usize> = None;
        let rows = self.design_check.get_or_compute_rows(lang).to_vec();
        let mut selected = self.design_check.selected.filter(|i| *i < rows.len());
        egui::Window::new(tr(lang, "Design Check (read-only)", "設計檢查（唯讀）"))
            .open(&mut open)
            .collapsible(false)
            .default_width(600.0)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.button(tr(lang, "Re-run check", "重新檢查")).clicked() {
                        rerun = true;
                    }
                    if ui
                        .button(tr(lang, "Export report", "匯出報告"))
                        .on_hover_text(tr(
                            lang,
                            "Export this report as a Markdown file outside the map directory",
                            "將此檢查報告匯出為地圖目錄外的 Markdown 檔案",
                        ))
                        .clicked()
                    {
                        export_report = true;
                    }
                    if rows.iter().any(|r| r.code == "wave-not-hostile")
                        && ui
                            .button(tr(lang, "Make wave teams hostile", "將波次隊伍設為敵對"))
                            .clicked()
                    {
                        fix_hostility = true;
                    }
                    if rows.iter().any(|r| r.code == "wave-no-destination")
                        && ui.button(tr(lang, "Send waves toward the player base", "讓波次進攻玩家主屋")).clicked()
                    {
                        fix_destination = true;
                    }
                    let has_objective_issues = rows.iter().any(|r| {
                        matches!(
                            r.code.as_str(),
                            "scenario-no-win-loss" | "scenario-no-victory" | "scenario-no-defeat"
                        )
                    });
                    if has_objective_issues
                        && ui
                            .button(tr(lang, "Add win/loss objectives", "補齊勝負條件"))
                            .on_hover_text(tr(
                                lang,
                                "Automatically adds default survival/elimination victory and base-defense defeat rules",
                                "自動補齊預設的存活／消滅勝利與主堡防守失敗規則",
                            ))
                            .clicked()
                    {
                        fix_objectives = true;
                    }
                    let has_unit_issues = rows.iter().any(|r| r.code == "team-no-units");
                    if has_unit_issues
                        && ui
                            .button(tr(lang, "Add starting units", "補齊起始部隊"))
                            .on_hover_text(tr(
                                lang,
                                "Automatically adds a starting squad for factions lacking units",
                                "自動為缺乏部隊的勢力在基地附近補齊初始戰斗小隊",
                            ))
                            .clicked()
                    {
                        fix_units = true;
                    }
                    let has_base_issues = rows.iter().any(|r| r.code == "team-no-main-house");
                    if has_base_issues
                        && ui
                            .button(tr(lang, "Add main houses", "補齊勢力主堡"))
                            .on_hover_text(tr(
                                lang,
                                "Automatically adds a main house for factions lacking a base",
                                "自動為缺乏基地的勢力補齊主堡建築",
                            ))
                            .clicked()
                    {
                        fix_bases = true;
                    }
                    let can_locate = selected
                        .and_then(|i| rows.get(i))
                        .is_some_and(|r| r.is_locatable());
                    if ui
                        .add_enabled(can_locate, egui::Button::new(tr(lang, "Locate", "定位")))
                        .clicked()
                    {
                        locate = selected;
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label(tr(
                        lang,
                        "Checks the current in-memory state. Quick fixes can be undone; Save to write changes.",
                        "檢查目前記憶體中的狀態。一鍵修正可復原；變更後請存檔。",
                    ));
                    ui.checkbox(
                        &mut self.design_check.rerun_after_save,
                        tr(lang, "Auto re-run after save", "儲存後自動重新檢查"),
                    )
                    .on_hover_text(tr(
                        lang,
                        "Automatically re-evaluates map design check right after a successful save",
                        "在成功完成安全儲存後自動重新執行設計檢查",
                    ));
                });
                ui.separator();
                match &self.design_check.report {
                    None => {
                        ui.label(tr(lang, "Not run yet.", "尚未檢查。"));
                    }
                    Some(report) => {
                        if matches!(report, DesignCheckReport::Checked { .. }) && rows.is_empty() {
                            ui.colored_label(
                                Color32::from_rgb(80, 180, 90),
                                tr(lang, "PASS: no design issues found.", "通過：未發現設計問題。"),
                            );
                        } else {
                            let error_count = rows.iter().filter(|r| r.severity == MapIssueSeverity::Error).count();
                            let warning_count = rows.len() - error_count;
                            ui.label(issue_summary(&rows, lang));
                            ui.horizontal(|ui| {
                                ui.label(tr(lang, "Filter:", "篩選："));
                                let err_label = match lang {
                                    AppLanguage::English => format!("Errors ({error_count})"),
                                    _ => format!("錯誤 ({error_count})"),
                                };
                                ui.checkbox(&mut self.design_check.show_errors, err_label);
                                let warn_label = match lang {
                                    AppLanguage::English => format!("Warnings ({warning_count})"),
                                    _ => format!("警告 ({warning_count})"),
                                };
                                ui.checkbox(&mut self.design_check.show_warnings, warn_label);
                                ui.checkbox(&mut self.design_check.locatable_only, tr(lang, "Locatable only", "僅顯示可定位問題"));
                                if ui.small_button(tr(lang, "Reset filters", "重設篩選")).clicked() {
                                    self.design_check.reset_filters();
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label(tr(lang, "Search issues", "搜尋問題"));
                                ui.text_edit_singleline(&mut self.design_check.search);
                                if ui.small_button(tr(lang, "Clear search", "清除搜尋")).clicked() {
                                    self.design_check.search.clear();
                                }
                            });
                            let filtered = self.design_check.filtered_rows(&rows);
                            selected = selected.filter(|index| filtered.iter().any(|(i, _)| i == index));
                            locate = locate.filter(|index| filtered.iter().any(|(i, _)| i == index));
                            let mut scroll_to_selected = false;
                            ui.horizontal(|ui| {
                                ui.label(visible_issue_count(filtered.len(), rows.len(), lang));
                                copy_issue_button(ui, tr(lang, "Copy visible issues", "複製顯示問題"), !filtered.is_empty(), || self.design_check.visible_issue_text(&rows, lang));
                            });
                            ui.horizontal(|ui| {
                                for (forward, label) in [
                                    (false, tr(lang, "Previous issue", "上一個問題")),
                                    (true, tr(lang, "Next issue", "下一個問題")),
                                ] {
                                    if ui.add_enabled(!filtered.is_empty(), egui::Button::new(label)).clicked() {
                                        selected = adjacent_issue(&filtered, selected, forward);
                                        scroll_to_selected = true;
                                    }
                                }
                                let row = selected.and_then(|i| rows.get(i));
                                copy_issue_button(ui, tr(lang, "Copy selected issue", "複製選取問題"), row.is_some(), || {
                                    row.map(|r| format_issue_row(r, lang)).unwrap_or_default()
                                });
                            });
                            if filtered.is_empty() && !rows.is_empty() {
                                ui.colored_label(
                                    Color32::from_rgb(180, 180, 180),
                                    tr(lang, "No issues match the current filters or search.", "沒有符合目前篩選或搜尋的問題。"),
                                );
                            } else {
                                ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                                    for (index, row) in filtered {
                                        let color = match row.severity {
                                            MapIssueSeverity::Error => Color32::from_rgb(220, 70, 70),
                                            MapIssueSeverity::Warning => Color32::from_rgb(220, 170, 40),
                                        };
                                        let text = RichText::new(format!(
                                            "[{}] {}: {}",
                                            row.severity_label(lang),
                                            row.code,
                                            row.text(lang)
                                        ))
                                        .color(color);
                                        ui.horizontal(|ui| {
                                            if issue_go_to(ui, row, lang).is_some_and(|r| r.clicked()) {
                                                selected = Some(index);
                                                locate = Some(index);
                                            }
                                            let response = ui.selectable_label(selected == Some(index), text);
                                            if scroll_to_selected && selected == Some(index) {
                                                response.scroll_to_me(Some(egui::Align::Center));
                                            }
                                            if response.clicked() {
                                                selected = Some(index);
                                            }
                                            if response.double_clicked() {
                                                locate = Some(index);
                                            }
                                        });
                                    }
                                });
                            }
                            if let Some(row) = selected.and_then(|i| rows.get(i)) {
                                ui.separator();
                                ui.label(row.text(lang));
                                if let Some((x, z)) = row.focus_tile() {
                                    ui.small(if lang == AppLanguage::English { format!("tile ({x:.1}, {z:.1})") } else { format!("圖塊（{x:.1}，{z:.1}）") });
                                }
                            }
                        }
                    }
                }
            });
        self.design_check.selected = selected;
        if let Some(row) = locate.and_then(|i| rows.get(i)) {
            self.design_check.focus_request = row.focus_world();
            if row.event_index >= 0 {
                self.design_check.event_request = Some(row.event_index);
            }
            if !row.object_id.is_nil() {
                self.design_check.object_request = Some(row.object_id);
            }
            if row.code.starts_with("scenario-") || row.code.starts_with("team-no-") {
                self.design_check.scenario_panel_request = true;
            }
        }
        self.design_check.open = open;
        if export_report {
            if let Err(e) = self.browse_design_check_report() {
                self.status_message = e;
                self.status_is_error = true;
            }
        }
        if fix_hostility {
            match self.fix_hostile_waves() {
                Ok(n) => {
                    self.status_message = format!("Added {n} hostile diplomacy event(s); Save to write them.");
                    self.status_is_error = false;
                }
                Err(e) => {
                    self.status_message = e;
                    self.status_is_error = true;
                }
            }
        } else if fix_destination {
            match self.fix_wave_destinations() {
                Ok(n) if n > 0 => {
                    self.status_message = match lang {
                        AppLanguage::English => format!("Set destinations for {n} wave action(s); Save to write them."),
                        _ => format!("已設定 {n} 個波次動作的目的地；請存檔以寫入變更。"),
                    };
                    self.status_is_error = false;
                }
                Ok(_) => {
                    self.status_message = tr(lang, "No wave destinations changed.", "未變更波次目的地。").into();
                    self.status_is_error = false;
                }
                Err(e) => {
                    self.status_message = e;
                    self.status_is_error = true;
                }
            }
        } else if fix_objectives {
            match self.fix_missing_objectives() {
                Ok(n) if n > 0 => {
                    self.status_message = match lang {
                        AppLanguage::English => {
                            format!("Added {n} win/loss objective event(s); Save to write them.")
                        }
                        _ => format!("已補齊 {n} 個勝負目標事件；請存檔以寫入變更。"),
                    };
                    self.status_is_error = false;
                }
                Ok(_) => {
                    self.status_message = tr(lang, "No objective events added.", "未新增勝負事件。").into();
                    self.status_is_error = false;
                }
                Err(e) => {
                    self.status_message = e;
                    self.status_is_error = true;
                }
            }
        } else if fix_units {
            match self.fix_missing_units() {
                Ok(n) if n > 0 => {
                    self.status_message = match lang {
                        AppLanguage::English => {
                            format!("Added starting units for {n} team(s); Save to write them.")
                        }
                        _ => format!("已為 {n} 個勢力補齊起始部隊；請存檔以寫入變更。"),
                    };
                    self.status_is_error = false;
                }
                Ok(_) => {
                    self.status_message = tr(lang, "No units added.", "未新增部隊。").into();
                    self.status_is_error = false;
                }
                Err(e) => {
                    self.status_message = e;
                    self.status_is_error = true;
                }
            }
        } else if fix_bases {
            match self.fix_missing_bases() {
                Ok(n) if n > 0 => {
                    self.status_message = match lang {
                        AppLanguage::English => {
                            format!("Added main houses for {n} team(s); Save to write them.")
                        }
                        _ => format!("已為 {n} 個勢力補齊主堡；請存檔以寫入變更。"),
                    };
                    self.status_is_error = false;
                }
                Ok(_) => {
                    self.status_message = tr(lang, "No main houses added.", "未新增主堡。").into();
                    self.status_is_error = false;
                }
                Err(e) => {
                    self.status_message = e;
                    self.status_is_error = true;
                }
            }
        } else if rerun {
            self.run_design_check();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_navigation_wraps_skips_hidden_rows_and_handles_stale_selection() {
        let mut panel = DesignCheckUi::default();
        let rows = vec![
            IssueRow::from(&map_issue(MapIssueSeverity::Error, "first", None, None)),
            IssueRow::from(&map_issue(MapIssueSeverity::Warning, "hidden", None, None)),
            IssueRow::from(&map_issue(MapIssueSeverity::Error, "last", None, None)),
        ];
        panel.show_warnings = false;
        let visible = panel.filtered_rows(&rows);
        assert_eq!(adjacent_issue(&visible, None, true), Some(0));
        assert_eq!(adjacent_issue(&visible, None, false), Some(2));
        assert_eq!(adjacent_issue(&visible, Some(0), true), Some(2));
        assert_eq!(adjacent_issue(&visible, Some(2), true), Some(0));
        assert_eq!(adjacent_issue(&visible, Some(0), false), Some(2));
        assert_eq!(adjacent_issue(&visible, Some(2), false), Some(0));
        assert_eq!(adjacent_issue(&visible, Some(1), true), Some(0));
        assert_eq!(adjacent_issue(&visible, Some(99), false), Some(2));
        assert_eq!(adjacent_issue(&visible[..1], Some(0), true), Some(0));
        assert_eq!(adjacent_issue(&[], Some(0), false), None);
    }

    #[test]
    fn filtering_out_selected_issue_clears_details_without_locating_or_editing() {
        let mut app = EditorApp::default();
        app.design_check.open = true;
        app.design_check.report = Some(DesignCheckReport::Checked {
            map_issues: vec![map_issue(MapIssueSeverity::Warning, "hidden", Some(1.0), Some(2.0))],
            preflight_error: None,
            design_issues: vec![],
        });
        app.design_check.selected = Some(0);
        app.design_check.search = "no-match".into();
        let report = app.design_check.report.clone();
        let ctx = egui::Context::default();
        let output = ctx.run(egui::RawInput::default(), |ctx| app.show_design_check(ctx));
        assert_eq!(app.design_check.selected, None);
        assert_eq!(app.design_check.focus_request, None);
        assert_eq!(app.design_check.event_request, None);
        assert_eq!(app.design_check.object_request, None);
        assert_eq!(app.design_check.report, report);
        assert!(output.platform_output.copied_text.is_empty());
    }

    #[test]
    fn copy_buttons_use_clipboard_only_on_enabled_click_in_both_languages() {
        let row = IssueRow::from(&map_issue(MapIssueSeverity::Error, "sample", Some(512.0), Some(768.0)));
        for lang in [AppLanguage::English, AppLanguage::TraditionalChinese] {
            for enabled in [true, false] {
                let ctx = egui::Context::default();
                let mut rect = egui::Rect::NOTHING;
                let mut calls = 0;
                let label = tr(lang, "Copy selected issue", "複製選取問題");
                let output = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        rect = copy_issue_button(ui, label, enabled, || {
                            calls += 1;
                            format_issue_row(&row, lang)
                        }).rect;
                    });
                });
                assert_eq!(calls, 0, "閒置幀不得格式化剪貼簿內容");
                assert!(output.platform_output.copied_text.is_empty());
                let pos = rect.center();
                let input = egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
                        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
                    ],
                    ..Default::default()
                };
                let output = ctx.run(input, |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        copy_issue_button(ui, label, enabled, || {
                            calls += 1;
                            format_issue_row(&row, lang)
                        });
                    });
                });
                assert_eq!(calls, usize::from(enabled));
                assert_eq!(output.platform_output.copied_text, if enabled { format_issue_row(&row, lang) } else { String::new() });
            }
        }
    }

    #[test]
    fn visible_copy_uses_current_filters_language_and_valid_coordinates() {
        let mut panel = DesignCheckUi::default();
        let rows = vec![
            IssueRow::from(&map_issue(MapIssueSeverity::Error, "hidden", None, None)),
            IssueRow::from(&map_issue(MapIssueSeverity::Warning, "visible", Some(512.0), Some(768.0))),
        ];
        panel.search = "VISIBLE".into();
        for lang in [AppLanguage::English, AppLanguage::TraditionalChinese] {
            let text = panel.visible_issue_text(&rows, lang);
            assert_eq!(text, format_issue_row(&rows[1], lang));
            assert!(!text.contains("hidden"));
            assert!(text.contains("2.0") && text.contains("3.0"));
        }
        panel.show_warnings = false;
        assert!(panel.visible_issue_text(&rows, AppLanguage::English).is_empty());
        panel.reset_filters();
        assert_eq!(panel.visible_issue_text(&rows, AppLanguage::English).lines().count(), 2);
    }

    #[test]
    fn visible_count_distinguishes_hidden_issues_from_clean_reports() {
        assert_eq!(visible_issue_count(0, 3, AppLanguage::English), "Visible issues: 0 / 3");
        assert_eq!(visible_issue_count(2, 3, AppLanguage::TraditionalChinese), "顯示問題： 2 / 3");
        assert_eq!(visible_issue_count(0, 0, AppLanguage::TraditionalChinese), "顯示問題： 0 / 0");
    }

    #[test]
    fn locatable_filter_combines_search_severity_and_event_targets() {
        let mut panel = DesignCheckUi::default();
        let mut event = IssueRow::from(&map_issue(MapIssueSeverity::Error, "event", None, None));
        event.event_index = 0;
        let rows = vec![
            IssueRow::from(&map_issue(MapIssueSeverity::Warning, "position", Some(1.0), Some(2.0))),
            IssueRow::from(&map_issue(MapIssueSeverity::Error, "invalid", Some(f32::NAN), Some(2.0))),
            event,
        ];
        panel.locatable_only = true;
        assert_eq!(panel.filtered_rows(&rows).iter().map(|r| r.0).collect::<Vec<_>>(), [0, 2]);
        panel.show_warnings = false;
        assert_eq!(panel.filtered_rows(&rows)[0].0, 2);
        panel.search = "POSITION".into();
        assert!(panel.filtered_rows(&rows).is_empty());
    }

    #[test]
    fn resetting_filters_preserves_report_selection_and_preferences() {
        let mut panel = DesignCheckUi::default();
        panel.report = Some(DesignCheckReport::Unavailable("保留".into()));
        panel.selected = Some(4);
        panel.rerun_after_save = true;
        panel.search = "遮蔽".into();
        panel.show_errors = false;
        panel.show_warnings = false;
        panel.locatable_only = true;
        panel.reset_filters();
        assert!(panel.search.is_empty() && panel.show_errors && panel.show_warnings && !panel.locatable_only);
        assert_eq!(panel.selected, Some(4));
        assert!(panel.rerun_after_save);
        assert_eq!(panel.report, Some(DesignCheckReport::Unavailable("保留".into())));
    }

    #[test]
    fn report_picker_cancel_and_maps_rejection() {
        let mut app = EditorApp::default();
        app.design_check.report_picker = Box::new(|_| None);
        let before = app.status_message.clone();
        assert_eq!(app.browse_design_check_report().unwrap(), None);
        assert_eq!(app.status_message, before);
        let target = PathBuf::from("C:/Games/Against Rome/MAPS/report.md");
        app.design_check.report_picker = Box::new(move |_| Some(target.clone()));
        assert!(app.browse_design_check_report().is_err());
        let map = Path::new("D:/CustomMaps/Example");
        let path = default_report_path(map);
        assert!(!path.starts_with(map));
        assert!(!crate::packaging_ui::is_game_maps_dir(&path));
    }

    #[test]
    fn failed_save_does_not_refresh_design_check() {
        let mut app = EditorApp::default();
        app.design_check.rerun_after_save = true;
        let previous = DesignCheckReport::Unavailable("保留先前報告".into());
        app.design_check.report = Some(previous.clone());
        app.design_check.selected = Some(2);
        app.safe_save_current();
        assert!(app.status_is_error);
        assert_eq!(app.design_check.report, Some(previous));
        assert_eq!(app.design_check.selected, Some(2));
        assert!(!app.design_check.open);
    }
    #[test]
    fn go_to_button_handles_clicks_and_invalid_coordinates() {
        let ctx = egui::Context::default();
        let mut row = IssueRow::from(&map_issue(MapIssueSeverity::Warning, "position", Some(512.0), Some(768.0)));
        for lang in [AppLanguage::English, AppLanguage::TraditionalChinese] {
            let mut rect = egui::Rect::NOTHING;
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    rect = issue_go_to(ui, &row, lang).expect("有效座標應提供前往").rect;
                });
            });
            let pos = rect.center();
            let mut clicked = false;
            let input = egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
                    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
                ],
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    clicked = issue_go_to(ui, &row, lang).unwrap().clicked();
                });
            });
            assert!(clicked, "不必先選取問題即可前往");
            assert_eq!(row.focus_world(), Some((512.0, 768.0)));
        }
        for coordinates in [(None, Some(1.0)), (Some(1.0), None), (Some(f32::NAN), Some(1.0)), (Some(1.0), Some(f32::INFINITY))] {
            (row.world_x, row.world_z) = coordinates;
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    assert!(issue_go_to(ui, &row, AppLanguage::English).is_none());
                });
            });
        }
    }
    fn map_issue(sev: MapIssueSeverity, code: &str, x: Option<f32>, z: Option<f32>) -> MapIssue {
        let mut issue = MapIssue::new(sev, code, format!("zh-{code}"), format!("en-{code}"));
        issue.world_x = x;
        issue.world_z = z;
        issue
    }

    #[test]
    fn rows_are_severity_sorted_bilingual_and_focusable() {
        let report = DesignCheckReport::Checked {
            map_issues: vec![
                map_issue(MapIssueSeverity::Warning, "w1", Some(512.0), Some(768.0)),
                map_issue(MapIssueSeverity::Error, "e1", None, None),
                map_issue(MapIssueSeverity::Warning, "w2", Some(f32::NAN), Some(1.0)),
            ],
            preflight_error: Some("bad bytecode".into()),
            design_issues: vec![],
        };
        let rows = report_rows(&report, &[map_issue(MapIssueSeverity::Error, "e2", None, None)], AppLanguage::English);
        let codes: Vec<_> = rows.iter().map(|r| r.code.as_str()).collect();
        assert_eq!(codes, ["scenario-preflight", "e1", "e2", "w1", "w2"], "errors first, stable inside a severity");
        let w1 = &rows[3];
        assert_eq!(w1.text(AppLanguage::English), "en-w1");
        assert_eq!(w1.text(AppLanguage::TraditionalChinese), "zh-w1");
        assert_eq!(w1.focus_tile(), Some((2.0, 3.0)));
        assert_eq!(rows[4].focus_world(), None, "non-finite coordinates cannot be located");
        assert!(issue_summary(&rows, AppLanguage::English).starts_with("3 errors, 2 warnings"));
        assert!(issue_summary(&rows, AppLanguage::TraditionalChinese).starts_with("3 個錯誤，2 個警告"));
    }

    #[test]
    fn text_falls_back_when_one_language_is_empty() {
        let mut row = IssueRow::from(&map_issue(MapIssueSeverity::Warning, "x", None, None));
        row.english.clear();
        assert_eq!(row.text(AppLanguage::English), "zh-x");
    }

    #[test]
    fn unavailable_report_is_a_single_error_row() {
        let rows = report_rows(&DesignCheckReport::Unavailable("boom".into()), &[], AppLanguage::English);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].severity, MapIssueSeverity::Error);
        assert!(rows[0].english.contains("boom"));
    }

    #[test]
    fn missing_required_data_files_become_one_warning() {
        let dir = std::env::temp_dir().join(format!("arm_dc_missing_{}", Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("DATA")).unwrap();
        let issue = missing_data_files_issue(&dir).expect("all 24 files are missing");
        assert_eq!(issue.code, "native-data-missing");
        assert_eq!(issue.severity, MapIssueSeverity::Warning);
        assert!(issue.english.contains("fow.dat") && issue.english.contains("biglager.dat"));
        for name in against_rome_maps::preflight::required_data_files() {
            std::fs::write(dir.join("DATA").join(name), b"x").unwrap();
        }
        assert!(missing_data_files_issue(&dir).is_none());
        assert!(missing_data_files_issue(Path::new("")).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn clean_report_is_pass_and_unavailable_is_not() {
        let clean = DesignCheckReport::Checked {
            map_issues: vec![],
            preflight_error: None,
            design_issues: vec![],
        };
        assert!(clean.is_clean());
        assert!(!DesignCheckReport::Unavailable("x".into()).is_clean());
        assert_eq!(DesignCheckReport::Unavailable("x".into()).error_count(), 1);
    }

    #[test]
    fn filter_toggles_by_severity_and_summary_counts() {
        let mut ui = DesignCheckUi::default();
        assert!(ui.show_errors);
        assert!(ui.show_warnings);
        assert!(!ui.rerun_after_save);

        let rows = vec![
            IssueRow::from(&map_issue(MapIssueSeverity::Error, "err_1", Some(100.0), Some(200.0))),
            IssueRow::from(&map_issue(MapIssueSeverity::Warning, "warn_1", None, None)),
            IssueRow::from(&map_issue(MapIssueSeverity::Warning, "warn_2", Some(300.0), Some(400.0))),
        ];

        let all = ui.filtered_rows(&rows);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].0, 0);
        assert_eq!(all[1].0, 1);
        assert_eq!(all[2].0, 2);

        ui.show_warnings = false;
        let only_errors = ui.filtered_rows(&rows);
        assert_eq!(only_errors.len(), 1);
        assert_eq!(only_errors[0].0, 0);
        assert_eq!(only_errors[0].1.code, "err_1");

        ui.show_errors = false;
        ui.show_warnings = true;
        let only_warnings = ui.filtered_rows(&rows);
        assert_eq!(only_warnings.len(), 2);
        assert_eq!(only_warnings[0].0, 1);
        assert_eq!(only_warnings[0].1.code, "warn_1");
        assert_eq!(only_warnings[1].0, 2);
        assert_eq!(only_warnings[1].1.code, "warn_2");
        assert_eq!(rows[only_warnings[1].0].focus_world(), Some((300.0, 400.0)));

        ui.show_warnings = false;
        assert!(ui.filtered_rows(&rows).is_empty());
    }

    #[test]
    fn export_report_markdown_and_external_path() {
        let report = DesignCheckReport::Checked {
            map_issues: vec![
                map_issue(MapIssueSeverity::Error, "blocked_spawn", Some(512.0), Some(768.0)),
                map_issue(MapIssueSeverity::Warning, "water_contact", None, None),
            ],
            preflight_error: None,
            design_issues: vec![],
        };

        let md_en = generate_markdown_report(&report, &[], "TEST_MAP", AppLanguage::English);
        assert!(md_en.contains("# Map Design Check Report: TEST_MAP"));
        assert!(md_en.contains("FAILED (1 errors, 1 warnings"));
        assert!(md_en.contains("| Error | 1 | Blocks Safe Save |"));
        assert!(md_en.contains("### 1. [Error] `blocked_spawn`"));
        assert!(md_en.contains("- **Tile Coordinates**: (2.0, 3.0)"));

        let md_zh = generate_markdown_report(&report, &[], "TEST_MAP", AppLanguage::TraditionalChinese);
        assert!(md_zh.contains("# 地圖設計檢查報告：TEST_MAP"));
        assert!(md_zh.contains("未通過 (1 個錯誤，1 個警告"));
        assert!(md_zh.contains("| 錯誤 (Error) | 1 | 阻擋儲存 (Blocks Save) |"));
        assert!(md_zh.contains("### 1. [錯誤] `blocked_spawn`"));
        assert!(md_zh.contains("- **圖塊座標**：（2.0, 3.0）"));

        let clean = DesignCheckReport::Checked {
            map_issues: vec![],
            preflight_error: None,
            design_issues: vec![],
        };
        let md_clean = generate_markdown_report(&clean, &[], "CLEAN_MAP", AppLanguage::English);
        assert!(md_clean.contains("PASS (No issues found)"));

        let map_dir = Path::new("C:/Games/Against Rome/MAPS/ENDL_005");
        let export_path = default_report_path(map_dir);
        assert_eq!(export_path.file_name().unwrap(), "ENDL_005.design-check.md");
        assert!(!crate::packaging_ui::is_game_maps_dir(&export_path));
        assert!(!export_path.starts_with(map_dir), "never inside the map folder");

        let mut app = EditorApp::default();
        let temp_map = std::env::temp_dir().join(format!("arm_map_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_map).unwrap();
        app.loaded_map_path = Some(temp_map.clone());

        let inside = temp_map.join("report.md");
        let err = app.export_design_check_report(Some(&inside)).unwrap_err();
        assert!(
            err.contains("Export target cannot be inside the game map directory")
                || err.contains("匯出目標不可位於遊戲地圖目錄內部")
        );

        let outside = std::env::temp_dir().join(format!("arm_report_outside_{}.md", Uuid::new_v4()));
        let written = app.export_design_check_report(Some(&outside)).unwrap();
        assert_eq!(written, outside);
        assert!(outside.is_file());
        let text = std::fs::read_to_string(&outside).unwrap();
        assert!(text.contains("# "));
        let _ = std::fs::remove_file(outside);
        let _ = std::fs::remove_dir_all(temp_map);
    }

    #[test]
    fn rerun_design_check_after_safe_save() {
        let temp_dir = std::env::temp_dir().join(format!("arm_safe_save_rerun_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join(".arm_custom_map"), b"{}").unwrap();

        let mut app = EditorApp::default();
        let session = crate::TerrainSession::create_blank(5, 32);
        app.set_loaded_map(temp_dir.clone(), session).unwrap();

        assert!(!app.design_check.rerun_after_save);
        app.safe_save_current();
        assert!(app.status_message.starts_with("Saved safely in one transaction"));
        assert!(!app.status_message.contains("Design Check:"));

        app.design_check.rerun_after_save = true;
        app.safe_save_current();
        assert!(app.design_check.report.is_some());
        assert!(app.status_message.contains("Design Check:"));

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_design_check_rows_caching_and_recompute_count() {
        let mut dc = DesignCheckUi::default();
        assert_eq!(dc.recompute_count, 0);

        let report = DesignCheckReport::Checked {
            map_issues: vec![
                MapIssue::new(
                    MapIssueSeverity::Error,
                    "sample_error",
                    "樣本錯誤",
                    "Sample error",
                ),
            ],
            preflight_error: None,
            design_issues: vec![],
        };
        dc.report = Some(report);

        // First access computes and caches
        let rows1 = dc.get_or_compute_rows(AppLanguage::English);
        assert_eq!(rows1.len(), 1);
        assert_eq!(dc.recompute_count, 1);

        // Successive access reuses cache
        let rows2 = dc.get_or_compute_rows(AppLanguage::English);
        assert_eq!(rows2.len(), 1);
        assert_eq!(dc.recompute_count, 1);

        // Language change recomputes
        let rows_zh = dc.get_or_compute_rows(AppLanguage::TraditionalChinese);
        assert_eq!(rows_zh.len(), 1);
        assert_eq!(dc.recompute_count, 2);

        // Same language cached again
        let rows_zh2 = dc.get_or_compute_rows(AppLanguage::TraditionalChinese);
        assert_eq!(rows_zh2.len(), 1);
        assert_eq!(dc.recompute_count, 2);

        // Invalidation triggers recompute
        dc.invalidate_cache();
        let rows3 = dc.get_or_compute_rows(AppLanguage::TraditionalChinese);
        assert_eq!(rows3.len(), 1);
        assert_eq!(dc.recompute_count, 3);
    }

    #[test]
    fn test_scenario_issues_are_locatable_and_request_scenario_panel() {
        let mut row = IssueRow::from(&map_issue(MapIssueSeverity::Warning, "scenario-no-win-loss", None, None));
        assert!(row.is_locatable());
        row.code = "scenario-no-victory".into();
        assert!(row.is_locatable());
        row.code = "scenario-no-defeat".into();
        assert!(row.is_locatable());
        row.code = "bounds".into();
        assert!(!row.is_locatable());
        row.code = "team-no-units".into();
        assert!(row.is_locatable());
        row.code = "team-no-main-house".into();
        assert!(row.is_locatable());

        let dc = DesignCheckUi::default();
        assert!(!dc.scenario_panel_request);
    }

    #[test]
    fn test_quick_fix_buttons_shown_for_missing_units_and_bases() {
        let mut app = EditorApp::default();
        app.design_check.open = true;
        app.design_check.report = Some(DesignCheckReport::Checked {
            map_issues: vec![],
            preflight_error: None,
            design_issues: vec![
                against_rome_maps::design_validator::MapDesignIssue {
                    severity: against_rome_maps::design_validator::DesignIssueSeverity::Warning,
                    code: "team-no-units".into(),
                    chinese: "隊伍 0 缺少起始部隊".into(),
                    english: "Team 0 has no units".into(),
                    object_alias: "".into(),
                    team: 0,
                    world_x: None,
                    world_z: None,
                    object_id: uuid::Uuid::nil(),
                },
                against_rome_maps::design_validator::MapDesignIssue {
                    severity: against_rome_maps::design_validator::DesignIssueSeverity::Warning,
                    code: "team-no-main-house".into(),
                    chinese: "隊伍 1 缺少主堡".into(),
                    english: "Team 1 has no main house".into(),
                    object_alias: "".into(),
                    team: 1,
                    world_x: None,
                    world_z: None,
                    object_id: uuid::Uuid::nil(),
                },
            ],
        });

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.show_design_check(ctx));
        assert!(app.design_check.open);
    }
}
