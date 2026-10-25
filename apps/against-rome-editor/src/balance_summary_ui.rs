//! Read-only Map Balance Summary panel.
//!
//! Provides a designer-facing sanity check:
//! - Per-team unit and building counts (teams 0..7)
//! - Wave attack timeline and enemy unit escalation
//! - Scenario win/loss condition validation

use crate::app::{tr, EditorApp};
use eframe::egui::{self, Color32, Grid, RichText, ScrollArea};

#[derive(Default)]
pub struct BalanceSummaryUi {
    pub open: bool,
}

impl EditorApp {
    /// Pure in-memory computation of the balance summary for testing or UI.
    pub(crate) fn compute_balance_summary(&self) -> crate::balance_summary::BalanceSummary {
        let document = self.scenario.document();
        let objects = if self.placement.is_loaded() {
            Some(self.placement.objects())
        } else {
            None
        };
        crate::balance_summary::summarize(
            document,
            objects.as_deref().map(|objs| (objs, self.placement.type_names())),
        )
    }

    /// Renders the read-only Map Balance Summary window.
    pub fn show_balance_summary(&mut self, ctx: &egui::Context) {
        if !self.balance_summary.open {
            return;
        }

        let lang = self.lang();
        let mut open = true;
        let summary = self.compute_balance_summary();

        egui::Window::new(tr(lang, "Map Balance Summary (read-only)", "地圖平衡摘要（唯讀）"))
            .open(&mut open)
            .default_width(620.0)
            .resizable(true)
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    // 1. Objectives sanity check
                    ui.heading(tr(lang, "Scenario Objectives", "劇本勝負目標"));
                    if !summary.objectives.has_victory {
                        ui.colored_label(
                            Color32::from_rgb(230, 80, 80),
                            tr(
                                lang,
                                "⚠ Warning: Missing enabled Victory objective (Action kind 3)",
                                "⚠ 警告：缺少已啟用的勝利目標（動作類型 3）",
                            ),
                        );
                    }
                    if !summary.objectives.has_defeat {
                        ui.colored_label(
                            Color32::from_rgb(230, 80, 80),
                            tr(
                                lang,
                                "⚠ Warning: Missing enabled Defeat objective (Action kind 4)",
                                "⚠ 警告：缺少已啟用的失敗目標（動作類型 4）",
                            ),
                        );
                    }
                    if summary.objectives.has_victory && summary.objectives.has_defeat {
                        ui.colored_label(
                            Color32::from_rgb(80, 200, 80),
                            tr(
                                lang,
                                "✓ Both Victory and Defeat objectives are present.",
                                "✓ 已具備已啟用的勝利與失敗目標。",
                            ),
                        );
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // 2. Per-team forces (0..7)
                    ui.heading(tr(lang, "Forces per Team (0..7)", "各隊伍兵力與建築 (0..7)"));
                    Grid::new("balance_teams_grid")
                        .striped(true)
                        .min_col_width(80.0)
                        .show(ui, |ui| {
                            ui.label(RichText::new(tr(lang, "Team", "隊伍")).strong());
                            ui.label(RichText::new(tr(lang, "Scripted Spawns", "腳本生成點")).strong());
                            ui.label(RichText::new(tr(lang, "Total Units", "總單位數")).strong());
                            ui.label(RichText::new(tr(lang, "DATA Buildings", "DATA 建築")).strong());
                            ui.end_row();

                            for tb in &summary.teams {
                                let team_label = if tb.team == 0 {
                                    tr(lang, "Team 0 (Player)", "隊伍 0（玩家）").to_string()
                                } else {
                                    format!("{} {}", tr(lang, "Team", "隊伍"), tb.team)
                                };
                                ui.label(team_label);
                                ui.label(tb.spawn_count.to_string());
                                ui.label(tb.total_units.to_string());
                                match tb.placed_buildings {
                                    Some(b) => {
                                        ui.label(b.to_string());
                                    }
                                    None => {
                                        ui.label(tr(lang, "- (placement not loaded)", "-（放置未載入）"));
                                    }
                                }
                                ui.end_row();
                            }
                        });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // 3. Attack waves (teams 1..7)
                    ui.heading(tr(lang, "Attack Waves (Teams 1..7)", "進攻波次時間表 (隊伍 1..7)"));
                    ui.label(format!(
                        "{}: {} | {}: {} | {}: {}/{}",
                        tr(lang, "Wave actions", "波次動作數"),
                        summary.total_wave_actions,
                        tr(lang, "Enemy wave units", "敵軍波次單位數"),
                        summary.total_enemy_wave_units,
                        tr(lang, "With destination", "具有目的地"),
                        summary.waves_with_destination,
                        summary.total_wave_actions,
                    ));

                    if summary.waves.is_empty() {
                        ui.label(tr(lang, "No enabled attack waves found.", "未發現任何已啟用的進攻波次。"));
                    } else {
                        Grid::new("balance_waves_grid")
                            .striped(true)
                            .min_col_width(70.0)
                            .show(ui, |ui| {
                                ui.label(RichText::new(tr(lang, "Time", "時間")).strong());
                                ui.label(RichText::new(tr(lang, "Event", "事件")).strong());
                                ui.label(RichText::new(tr(lang, "Team", "隊伍")).strong());
                                ui.label(RichText::new(tr(lang, "Units", "單位數")).strong());
                                ui.label(RichText::new(tr(lang, "Cumulative", "累計敵軍")).strong());
                                ui.label(RichText::new(tr(lang, "Destination", "移動目的地")).strong());
                                ui.end_row();

                                for wave in &summary.waves {
                                    ui.label(&wave.time_formatted);
                                    ui.label(&wave.event_name);
                                    ui.label(format!("{} {}", tr(lang, "Team", "隊伍"), wave.team));
                                    ui.label(wave.units.to_string());
                                    ui.label(wave.cumulative_units.to_string());
                                    if wave.has_move_destination {
                                        ui.colored_label(Color32::from_rgb(80, 200, 80), tr(lang, "Yes", "有"));
                                    } else {
                                        ui.colored_label(Color32::from_rgb(230, 160, 60), tr(lang, "None", "無"));
                                    }
                                    ui.end_row();
                                }
                            });
                    }
                });
            });

        self.balance_summary.open = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_balance_summary_ui_renders_default_app() {
        let mut app = EditorApp::initial_state();
        assert!(!app.balance_summary.open);

        let summary = app.compute_balance_summary();
        assert_eq!(summary.total_wave_actions, 0);
        assert_eq!(summary.total_enemy_wave_units, 0);
        assert!(!summary.objectives.has_victory);
        assert!(!summary.objectives.has_defeat);

        app.balance_summary.open = true;
        let ctx = egui::Context::default();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
                ..Default::default()
            },
            |ctx| {
                app.show_balance_summary(ctx);
            },
        );
        assert!(app.balance_summary.open);
    }
}
