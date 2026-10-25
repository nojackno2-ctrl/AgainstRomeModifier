//! Read-only Map Balance Summary panel.
//!
//! Provides a designer-facing sanity check:
//! - Per-team unit and building counts (teams 0..7)
//! - Wave attack timeline and enemy unit escalation
//! - Scenario win/loss condition validation

use crate::app::{tr, EditorApp};
use against_rome_ambience::Language as AppLanguage;
use eframe::egui::{self, Color32, Grid, RichText, ScrollArea};

/// Fingerprint used to invalidate cached balance summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BalanceCacheKey {
    pub map_path: Option<std::path::PathBuf>,
    pub scenario_loaded: bool,
    pub scenario_token: u64,
    pub scenario_spawns_len: usize,
    pub scenario_events_len: usize,
    pub placement_loaded: bool,
    pub placement_token: u64,
}

#[derive(Default)]
pub struct BalanceSummaryUi {
    pub open: bool,
    pub(crate) cached_summary: Option<crate::balance_summary::BalanceSummary>,
    pub(crate) cache_key: Option<BalanceCacheKey>,
    pub recompute_count: usize,
}

impl BalanceSummaryUi {
    /// Manually invalidate the cached balance summary.
    pub fn invalidate_cache(&mut self) {
        self.cached_summary = None;
        self.cache_key = None;
    }
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

        // Trigger the same one-time scenario autoload as Scenario/Spawns panels
        self.autoload_scenario_if_needed();

        let lang = self.lang();
        let mut open = true;

        egui::Window::new(tr(lang, "Map Balance Summary (read-only)", "地圖平衡摘要（唯讀）"))
            .open(&mut open)
            .default_width(620.0)
            .resizable(true)
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button(tr(lang, "Refresh", "重新整理")).clicked() {
                            self.balance_summary.invalidate_cache();
                            if !self.scenario.is_loaded() {
                                self.scenario_autoload_tried = None;
                                self.autoload_scenario_if_needed();
                            }
                        }
                    });
                    ui.add_space(4.0);

                    // If scenario loading failed or is unavailable, show a bilingual notice
                    // instead of misleading empty results.
                    if !self.scenario.is_loaded() {
                        ui.colored_label(
                            Color32::from_rgb(230, 80, 80),
                            tr(lang, "Scenario unavailable", "劇本無法載入"),
                        );
                        return;
                    }

                    // Check cache token against scenario and placement state
                    let key = BalanceCacheKey {
                        map_path: self.loaded_map_path.clone(),
                        scenario_loaded: self.scenario.is_loaded(),
                        scenario_token: self.scenario.history_token(),
                        scenario_spawns_len: self.scenario.document().spawns.len(),
                        scenario_events_len: self.scenario.document().events.len(),
                        placement_loaded: self.placement.is_loaded(),
                        placement_token: self.placement.history_token(),
                    };

                    if self.balance_summary.cache_key.as_ref() != Some(&key)
                        || self.balance_summary.cached_summary.is_none()
                    {
                        let computed = self.compute_balance_summary();
                        self.balance_summary.recompute_count += 1;
                        self.balance_summary.cache_key = Some(key);
                        self.balance_summary.cached_summary = Some(computed);
                    }

                    let summary = self.balance_summary.cached_summary.clone().unwrap();
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
                        "{}: {} | {}: {} | {}: {} | {}: {}/{}",
                        tr(lang, "Wave actions", "波次動作數"),
                        summary.total_wave_actions,
                        tr(lang, "Enemy wave units (by mission time)", "敵軍波次單位數（按任務時間）"),
                        summary.total_enemy_wave_units,
                        tr(lang, "Condition-triggered units", "條件觸發兵力"),
                        summary.condition_triggered_units,
                        tr(lang, "With destination", "具有目的地"),
                        summary.waves_with_destination,
                        summary.total_wave_actions,
                    ));

                    if summary.waves.iter().any(|w| w.repeats) {
                        ui.label(
                            RichText::new(tr(
                                lang,
                                "Cumulative total excludes repeating events (repeats occur at least every delay_seconds)",
                                "累計不含重複事件（重複事件至少每隔 delay_seconds 再次發生）",
                            ))
                            .italics(),
                        );
                    }

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
                                    let time_display = crate::balance_summary::format_wave_time(
                                        wave.delay_seconds,
                                        wave.condition_triggered,
                                        lang == AppLanguage::English,
                                    );
                                    ui.label(time_display);

                                    let event_label = if wave.repeats {
                                        if wave.event_name.is_empty() {
                                            format!("[{}]", tr(lang, "repeats", "重複"))
                                        } else {
                                            format!("{} [{}]", wave.event_name, tr(lang, "repeats", "重複"))
                                        }
                                    } else {
                                        wave.event_name.clone()
                                    };
                                    let label_response = ui.label(event_label);
                                    if wave.repeats {
                                        let tooltip = if lang == AppLanguage::English {
                                            format!(
                                                "Repeats occur at least every {} (delay_seconds)",
                                                crate::balance_summary::format_time_mmss(wave.delay_seconds)
                                            )
                                        } else {
                                            format!(
                                                "每次重複至少間隔 {} (delay_seconds)",
                                                crate::balance_summary::format_time_mmss(wave.delay_seconds)
                                            )
                                        };
                                        label_response.on_hover_text(tooltip);
                                    }

                                    ui.label(format!("{} {}", tr(lang, "Team", "隊伍"), wave.team));
                                    ui.label(wave.units.to_string());
                                    match wave.cumulative_units {
                                        Some(cum) => {
                                            ui.label(cum.to_string());
                                        }
                                        None => {
                                            ui.label("-");
                                        }
                                    }
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
        assert_eq!(summary.condition_triggered_units, 0);
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
        assert!(!app.scenario.is_loaded());
        assert!(app.balance_summary.cached_summary.is_none());
    }

    #[test]
    fn test_balance_summary_ui_renders_repeating_and_condition_waves_bilingual() {
        use against_rome_scenario::{Action, Condition, Event, Spawn};

        let temp_dir = std::env::temp_dir().join(format!(
            "ar_test_balance_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(temp_dir.join("SCRIPT")).unwrap();
        std::fs::write(
            temp_dir.join("SCRIPT/ak_level.bci"),
            against_rome_scenario::BciImage::idle().serialize(),
        )
        .unwrap();

        let mut app = EditorApp::initial_state();
        app.scenario.load_session(&temp_dir).unwrap();

        let spawn_id = uuid::Uuid::new_v4();
        app.scenario
            .apply_spawns(vec![Spawn {
                id: spawn_id,
                alias: "Speer".into(),
                team: 0,
                count: 10,
                x: 1000.0,
                z: 1000.0,
                ..Default::default()
            }])
            .unwrap();

        app.scenario
            .add_events(&[
                Event {
                    name: "Repeating Wave".into(),
                    delay_seconds: 60,
                    repeat: true,
                    enabled: true,
                    conditions: vec![],
                    actions: vec![Action {
                        kind: 2,
                        team: 1,
                        count: 5,
                        alias: "Speer".into(),
                        x: 2000.0,
                        z: 2000.0,
                        ..Default::default()
                    }],
                },
                Event {
                    name: "Condition Wave".into(),
                    delay_seconds: 15,
                    repeat: false,
                    enabled: true,
                    conditions: vec![Condition {
                        target_id: spawn_id,
                        ..Default::default()
                    }],
                    actions: vec![Action {
                        kind: 2,
                        team: 2,
                        count: 10,
                        alias: "Speer".into(),
                        x: 3000.0,
                        z: 3000.0,
                        ..Default::default()
                    }],
                },
            ])
            .unwrap();

        let summary = app.compute_balance_summary();
        assert_eq!(summary.total_wave_actions, 2);
        assert_eq!(summary.total_enemy_wave_units, 0);
        assert_eq!(summary.condition_triggered_units, 10);
        assert!(summary.waves.iter().any(|w| w.repeats));
        assert_eq!(summary.waves[1].time_formatted, "earliest 0:15, when condition met");

        app.balance_summary.open = true;

        // 1. English rendering
        app.ambience.app_language = AppLanguage::English;
        let ctx_en = egui::Context::default();
        let _ = ctx_en.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
                ..Default::default()
            },
            |ctx| {
                app.show_balance_summary(ctx);
            },
        );
        assert!(app.balance_summary.open);
        assert_eq!(app.balance_summary.recompute_count, 1);

        // 2. Traditional Chinese rendering
        app.ambience.app_language = AppLanguage::TraditionalChinese;
        let ctx_zh = egui::Context::default();
        let _ = ctx_zh.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
                ..Default::default()
            },
            |ctx| {
                app.show_balance_summary(ctx);
            },
        );
        assert!(app.balance_summary.open);
        assert_eq!(app.balance_summary.recompute_count, 1, "Redraw in another language must use cache");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_summary_opening_autoloads_scenario_waves_and_handles_unavailable() {
        use against_rome_scenario::{Action, Event};

        // 1. Valid scenario map: opening summary should trigger scenario autoload
        let temp_dir = std::env::temp_dir().join(format!(
            "ar_test_balance_autoload_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(temp_dir.join("SCRIPT")).unwrap();
        std::fs::write(
            temp_dir.join("SCRIPT/ak_level.bci"),
            against_rome_scenario::BciImage::idle().serialize(),
        )
        .unwrap();

        // Populate SCRIPT/ak_level.bci with an attack wave and victory objective
        {
            let doc = against_rome_scenario::ScenarioDocument {
                events: vec![
                    Event {
                        name: "Autoload Wave".into(),
                        delay_seconds: 45,
                        enabled: true,
                        actions: vec![Action {
                            kind: 2,
                            team: 2,
                            count: 12,
                            alias: "Speer".into(),
                            x: 2000.0,
                            z: 2000.0,
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                    Event {
                        name: "Victory Condition".into(),
                        delay_seconds: 0,
                        enabled: true,
                        actions: vec![Action {
                            kind: 3,
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            };
            std::fs::write(temp_dir.join("arm_scenario.json"), doc.to_json().unwrap()).unwrap();
        }

        let mut app = EditorApp::initial_state();
        app.loaded_map_path = Some(temp_dir.clone());
        // Verify scenario is NOT loaded initially before opening summary
        assert!(!app.scenario.is_loaded());

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

        // Opening the summary triggered autoload:
        assert!(app.scenario.is_loaded(), "Opening summary must trigger scenario autoload");
        let cached = app.balance_summary.cached_summary.as_ref().expect("cached summary");
        assert_eq!(cached.waves.len(), 1);
        assert_eq!(cached.waves[0].event_name, "Autoload Wave");
        assert_eq!(cached.waves[0].units, 12);
        assert!(cached.objectives.has_victory);

        let _ = std::fs::remove_dir_all(&temp_dir);

        // 2. Scenario unavailable: invalid map directory without SCRIPT
        let empty_dir = std::env::temp_dir().join(format!(
            "ar_test_balance_unavailable_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&empty_dir).unwrap();

        let mut app_unavailable = EditorApp::initial_state();
        app_unavailable.loaded_map_path = Some(empty_dir.clone());
        app_unavailable.balance_summary.open = true;

        let ctx_unav = egui::Context::default();
        let _ = ctx_unav.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
                ..Default::default()
            },
            |ctx| {
                app_unavailable.show_balance_summary(ctx);
            },
        );

        // Scenario could not load, so scenario.is_loaded() is false and no misleading empty results are cached
        assert!(!app_unavailable.scenario.is_loaded());
        assert!(app_unavailable.balance_summary.cached_summary.is_none());

        let _ = std::fs::remove_dir_all(&empty_dir);
    }

    #[test]
    fn test_balance_summary_caching_and_recompute_on_edit() {
        use against_rome_scenario::{Action, Event};

        let temp_dir = std::env::temp_dir().join(format!(
            "ar_test_balance_cache_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(temp_dir.join("SCRIPT")).unwrap();
        std::fs::write(
            temp_dir.join("SCRIPT/ak_level.bci"),
            against_rome_scenario::BciImage::idle().serialize(),
        )
        .unwrap();

        let mut app = EditorApp::initial_state();
        app.scenario.load_session(&temp_dir).unwrap();
        app.balance_summary.open = true;

        let ctx = egui::Context::default();
        let run_frame = |app: &mut EditorApp| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
                    ..Default::default()
                },
                |ctx| {
                    app.show_balance_summary(ctx);
                },
            );
        };

        // Frame 1: initial computation
        run_frame(&mut app);
        assert_eq!(app.balance_summary.recompute_count, 1);

        // Frame 2: nothing changed -> cache reused
        run_frame(&mut app);
        assert_eq!(app.balance_summary.recompute_count, 1, "Cache must not recompute when nothing changed");

        // Explicit cache invalidation (e.g. Refresh button clicked)
        app.balance_summary.invalidate_cache();
        run_frame(&mut app);
        assert_eq!(app.balance_summary.recompute_count, 2, "Cache must recompute after invalidation");

        // Frame 4: nothing changed -> cache reused
        run_frame(&mut app);
        assert_eq!(app.balance_summary.recompute_count, 2);

        // Make an edit to scenario
        app.scenario
            .add_events(&[Event {
                name: "New Wave".into(),
                delay_seconds: 100,
                enabled: true,
                actions: vec![Action {
                    kind: 2,
                    team: 1,
                    count: 7,
                    alias: "Speer".into(),
                    x: 2000.0,
                    z: 2000.0,
                    ..Default::default()
                }],
                ..Default::default()
            }])
            .unwrap();

        // Frame 5: edit detected -> cache recomputed!
        run_frame(&mut app);
        assert_eq!(app.balance_summary.recompute_count, 3, "Cache must recompute after scenario edit");
        assert_eq!(app.balance_summary.cached_summary.as_ref().unwrap().waves.len(), 1);

        // Frame 6: nothing changed -> cache reused
        run_frame(&mut app);
        assert_eq!(app.balance_summary.recompute_count, 3);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
