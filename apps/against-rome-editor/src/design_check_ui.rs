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
use std::path::Path;
use uuid::Uuid;

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

#[derive(Default)]
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
}

impl EditorApp {
    /// Evaluate the current effective state without side effects and show the result.
    pub fn run_design_check(&mut self) {
        let report = self.design_check_report();
        let (path, _) = self.check_candidate_inputs().unwrap_or_default();
        self.design_check.extra_issues = missing_data_files_issue(&path).into_iter().collect();
        self.design_check.report = Some(report);
        self.design_check.selected = None;
        self.design_check.open = true;
    }

    /// Pure (`&self`) computation of the report; used by the button and by tests.
    pub fn design_check_report(&self) -> DesignCheckReport {
        let (path, staged) = match self.check_candidate_inputs() {
            Ok(v) => v,
            Err(e) => return DesignCheckReport::Unavailable(e),
        };
        match self.candidate_diagnostics(&path, &staged) {
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

    pub fn show_design_check(&mut self, ctx: &egui::Context) {
        if !self.design_check.open {
            return;
        }
        let lang = self.lang();
        let mut open = true;
        let mut rerun = false;
        let mut locate: Option<usize> = None;
        let rows = match &self.design_check.report {
            Some(report) => report_rows(report, &self.design_check.extra_issues, lang),
            None => Vec::new(),
        };
        let mut selected = self.design_check.selected.filter(|i| *i < rows.len());
        egui::Window::new(tr(lang, "Design Check (read-only)", "設計檢查（唯讀）"))
            .open(&mut open)
            .default_width(600.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, "Re-run check", "重新檢查")).clicked() {
                        rerun = true;
                    }
                    let can_locate = selected
                        .and_then(|i| rows.get(i))
                        .is_some_and(|r| r.focus_world().is_some() || r.event_index >= 0);
                    if ui
                        .add_enabled(can_locate, egui::Button::new(tr(lang, "Locate", "定位")))
                        .clicked()
                    {
                        locate = selected;
                    }
                });
                ui.label(tr(
                    lang,
                    "Checks the current in-memory state; nothing is saved or changed.",
                    "檢查目前記憶體中的狀態；不會儲存或變更任何內容。",
                ));
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
                            ui.label(issue_summary(&rows, lang));
                            ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                                for (index, row) in rows.iter().enumerate() {
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
                                    let response = ui.selectable_label(selected == Some(index), text);
                                    if response.clicked() {
                                        selected = Some(index);
                                    }
                                    if response.double_clicked() {
                                        locate = Some(index);
                                    }
                                }
                            });
                            if let Some(row) = selected.and_then(|i| rows.get(i)) {
                                ui.separator();
                                ui.label(row.text(lang));
                                if let Some((x, z)) = row.focus_tile() {
                                    ui.small(format!("tile ({x:.1}, {z:.1})"));
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
        }
        self.design_check.open = open;
        if rerun {
            self.run_design_check();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
