//! Read-only Map Design Check panel (C# `MapEditorForm.DesignCheck.cs`).
//!
//! Evaluates the effective in-memory candidate (terrain, pending placement, pending scenario
//! and events, pending water level) with the same logic Safe Save uses. It never writes files,
//! clears dirty flags or touches edit sessions.

use crate::app::EditorApp;
use against_rome_maps::{
    design_validator::{DesignIssueSeverity, MapDesignIssue},
    diagnostics::{MapIssue, MapIssueSeverity},
};
use eframe::egui::{self, Color32, RichText, ScrollArea};

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

#[derive(Default)]
pub struct DesignCheckUi {
    pub open: bool,
    pub report: Option<DesignCheckReport>,
}

impl EditorApp {
    /// Evaluate the current effective state without side effects and show the result.
    pub fn run_design_check(&mut self) {
        let report = self.design_check_report();
        self.design_check.report = Some(report);
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
        let mut open = true;
        let mut rerun = false;
        egui::Window::new("Design Check (read-only)")
            .open(&mut open)
            .default_width(560.0)
            .show(ctx, |ui| {
                if ui.button("Re-run check").clicked() {
                    rerun = true;
                }
                ui.label("Checks the current in-memory state; nothing is saved or changed.");
                ui.separator();
                match &self.design_check.report {
                    None => {
                        ui.label("Not run yet.");
                    }
                    Some(report) => draw_report(ui, report),
                }
            });
        self.design_check.open = open;
        if rerun {
            self.run_design_check();
        }
    }
}

fn draw_report(ui: &mut egui::Ui, report: &DesignCheckReport) {
    let error = Color32::from_rgb(220, 70, 70);
    let warning = Color32::from_rgb(220, 170, 40);
    match report {
        DesignCheckReport::Unavailable(msg) => {
            ui.colored_label(error, format!("Check unavailable: {msg}"));
        }
        DesignCheckReport::Checked {
            map_issues,
            preflight_error,
            design_issues,
        } => {
            if report.is_clean() {
                ui.colored_label(
                    Color32::from_rgb(80, 180, 90),
                    "PASS: no design issues found.",
                );
                return;
            }
            ui.label(format!(
                "{} error(s), {} warning(s)",
                report.error_count(),
                report.warning_count()
            ));
            ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                if let Some(e) = preflight_error {
                    ui.colored_label(error, RichText::new(format!("[ERROR] {e}")));
                }
                for i in map_issues {
                    let (tag, color) = match i.severity {
                        MapIssueSeverity::Error => ("ERROR", error),
                        MapIssueSeverity::Warning => ("WARNING", warning),
                    };
                    ui.colored_label(color, format!("[{tag}] {}: {}", i.code, i.english));
                }
                for i in design_issues {
                    let (tag, color) = match i.severity {
                        DesignIssueSeverity::Error => ("ERROR", error),
                        DesignIssueSeverity::Warning => ("WARNING", warning),
                    };
                    ui.colored_label(color, format!("[{tag}] {}: {}", i.code, i.english));
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
