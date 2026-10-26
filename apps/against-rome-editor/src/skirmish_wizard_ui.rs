//! "New skirmish map… / 新增對戰地圖…" wizard that chains existing steps:
//! 1. Template selection (Flat template default vs Empty Scene experimental)
//! 2. Title & Briefing (synced with maps_ui / flat template briefing path)
//! 3. Bases & Resources (Settlement generator, attack waves, objectives)
//! 4. Summary & In-Memory Execution (runs build + open + settlement + waves + objectives + Design Check + Safe Save reminder)

use std::path::{Path, PathBuf};
use eframe::egui::{self, Color32, Vec2};
use against_rome_maps::{
    blank_builder::BlankMapSpec,
    catalog::EndlessMapCatalog,
};
use against_rome_environment::settlement::{SettlementTribe, SymmetryMode};
use against_rome_ambience::Language as AppLanguage;
use crate::app::{tr, EditorApp};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkirmishTemplateKind {
    FlatTemplate,
    EmptyScene,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SkirmishWizardStep {
    Template = 0,
    TitleBriefing = 1,
    BasesResources = 2,
    Summary = 3,
}

pub enum SkirmishWizardAction {
    Create,
    Cancel,
}

#[derive(Clone, Debug)]
pub struct SkirmishWizardUi {
    pub open: bool,
    pub step: SkirmishWizardStep,
    pub template_kind: SkirmishTemplateKind,
    pub game_path_input: String,
    pub source_slot: i32,
    pub target_slot: i32,
    pub target_name: String,
    pub briefing: String,

    // Step 3: Bases & resources
    pub players: usize,
    pub tribe: SettlementTribe,
    pub symmetry: SymmetryMode,
    pub seed: i32,
    pub include_nature: bool,
    pub spawn_radius: f32,

    // Step 3: Attack waves
    pub add_attack_waves: bool,
    pub wave_alias: String,
    pub wave_team: i32,
    pub wave_count: i32,
    pub wave_first: i32,
    pub wave_interval: i32,

    // Step 3: Objectives
    pub add_objectives: bool,
    pub survive_seconds: i32,
    pub lose_if_main_house_destroyed: bool,

    // Status and feedback
    pub error_message: Option<String>,
}

impl Default for SkirmishWizardUi {
    fn default() -> Self {
        Self {
            open: false,
            step: SkirmishWizardStep::Template,
            template_kind: SkirmishTemplateKind::FlatTemplate,
            game_path_input: String::new(),
            source_slot: 1,
            target_slot: 5,
            target_name: "New Skirmish Map".to_string(),
            briefing: "Skirmish Battle: defend your settlement and destroy the invaders.".to_string(),
            players: 2,
            tribe: SettlementTribe::Germanic,
            symmetry: SymmetryMode::RotationalSymmetry,
            seed: 42,
            include_nature: true,
            spawn_radius: 0.55,
            add_attack_waves: true,
            wave_alias: String::new(),
            wave_team: 1,
            wave_count: 5,
            wave_first: 60,
            wave_interval: 60,
            add_objectives: true,
            survive_seconds: 300,
            lose_if_main_house_destroyed: true,
            error_message: None,
        }
    }
}

impl SkirmishWizardUi {
    pub fn cancel(&mut self) {
        self.open = false;
        self.error_message = None;
    }

    pub fn validate_current_step(&self) -> Result<(), String> {
        match self.step {
            SkirmishWizardStep::Template => {
                let path = Path::new(self.game_path_input.trim());
                if self.game_path_input.trim().is_empty() {
                    return Err("Game root path must not be empty. / 遊戲路徑不得為空。".to_string());
                }
                if !path.is_dir() {
                    return Err(format!(
                        "Game root path does not exist or is not a directory: {} / 遊戲資料夾無效或不存在",
                        path.display()
                    ));
                }
                if self.target_name.trim().is_empty() {
                    return Err("Map name must not be blank. / 地圖名稱不得為空。".to_string());
                }
                if self.source_slot < 0 {
                    return Err("Source slot must be non-negative. / 來源槽位不得為負數。".to_string());
                }
                Ok(())
            }
            SkirmishWizardStep::TitleBriefing => {
                if self.target_name.trim().is_empty() {
                    return Err("Map name must not be blank. / 地圖名稱不得為空。".to_string());
                }
                Ok(())
            }
            SkirmishWizardStep::BasesResources => {
                if !(2..=8).contains(&self.players) {
                    return Err("Number of players must be between 2 and 8. / 勢力數量需介於 2 至 8 之間。".to_string());
                }
                if self.add_attack_waves {
                    self.validate_wave_alias()?;
                    if !(1..=7).contains(&self.wave_team) {
                        return Err("Attack wave team must be 1–7. / 進攻隊伍需為 1–7。".to_string());
                    }
                    if self.wave_count == 0 {
                        return Err("Wave count must be at least 1. / 波數需至少為 1。".to_string());
                    }
                }
                if self.add_objectives && self.survive_seconds <= 0 {
                    return Err("Survival seconds must be greater than 0. / 存活秒數需大於 0。".to_string());
                }
                Ok(())
            }
            SkirmishWizardStep::Summary => Ok(()),
        }
    }

    fn validate_wave_alias(&self) -> Result<(), String> {
        if self.add_attack_waves && self.wave_alias.trim().is_empty() {
            return Err("Choose a wave unit alias. / 請選擇波次單位別名。".into());
        }
        Ok(())
    }

    pub fn next_step(&mut self) -> Result<(), String> {
        self.validate_current_step()?;
        self.error_message = None;
        match self.step {
            SkirmishWizardStep::Template => self.step = SkirmishWizardStep::TitleBriefing,
            SkirmishWizardStep::TitleBriefing => self.step = SkirmishWizardStep::BasesResources,
            SkirmishWizardStep::BasesResources => self.step = SkirmishWizardStep::Summary,
            SkirmishWizardStep::Summary => {}
        }
        Ok(())
    }

    pub fn prev_step(&mut self) {
        self.error_message = None;
        match self.step {
            SkirmishWizardStep::Template => {}
            SkirmishWizardStep::TitleBriefing => self.step = SkirmishWizardStep::Template,
            SkirmishWizardStep::BasesResources => self.step = SkirmishWizardStep::TitleBriefing,
            SkirmishWizardStep::Summary => self.step = SkirmishWizardStep::BasesResources,
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        lang: AppLanguage,
        folder_picker: Option<&crate::maps_ui::GameFolderPicker>,
    ) -> Option<SkirmishWizardAction> {
        if !self.open {
            return None;
        }

        let mut action = None;
        let mut is_open = self.open;

        egui::Window::new(tr(lang, "New skirmish map…", "新增對戰地圖…"))
            .open(&mut is_open)
            .resizable(true)
            .default_size(Vec2::new(540.0, 500.0))
            .min_size(Vec2::new(480.0, 420.0))
            .show(ctx, |ui| {
                // Header: Step indicators
                ui.horizontal(|ui| {
                    let steps = [
                        (SkirmishWizardStep::Template, tr(lang, "1. Template", "1. 範本")),
                        (SkirmishWizardStep::TitleBriefing, tr(lang, "2. Briefing", "2. 簡報")),
                        (SkirmishWizardStep::BasesResources, tr(lang, "3. Bases & Rules", "3. 基地與規則")),
                        (SkirmishWizardStep::Summary, tr(lang, "4. Summary", "4. 摘要")),
                    ];
                    for (step, label) in steps {
                        if self.step == step {
                            ui.colored_label(Color32::from_rgb(100, 200, 255), format!("[{label}]"));
                        } else {
                            ui.label(label);
                        }
                        if step < SkirmishWizardStep::Summary {
                            ui.label(">");
                        }
                    }
                });

                ui.separator();

                if let Some(err) = &self.error_message {
                    ui.colored_label(Color32::from_rgb(255, 100, 100), err);
                    ui.separator();
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    match self.step {
                        SkirmishWizardStep::Template => {
                            self.show_step_template(ui, lang, folder_picker);
                        }
                        SkirmishWizardStep::TitleBriefing => {
                            self.show_step_title_briefing(ui, lang);
                        }
                        SkirmishWizardStep::BasesResources => {
                            self.show_step_bases_resources(ui, lang);
                        }
                        SkirmishWizardStep::Summary => {
                            self.show_step_summary(ui, lang);
                        }
                    }
                });

                ui.separator();

                // Footer: Navigation Buttons
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, "Cancel", "取消")).clicked() {
                        action = Some(SkirmishWizardAction::Cancel);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        match self.step {
                            SkirmishWizardStep::Summary => {
                                if ui.button(tr(lang, "Create", "建立")).clicked() {
                                    action = Some(SkirmishWizardAction::Create);
                                }
                                if ui.button(tr(lang, "Back", "上一步")).clicked() {
                                    self.prev_step();
                                }
                            }
                            SkirmishWizardStep::Template => {
                                if ui.button(tr(lang, "Next", "下一步")).clicked() {
                                    if let Err(e) = self.next_step() {
                                        self.error_message = Some(e);
                                    }
                                }
                            }
                            _ => {
                                let validation = self.validate_current_step();
                                if ui.add_enabled(validation.is_ok(), egui::Button::new(tr(lang, "Next", "下一步")))
                                    .on_disabled_hover_text(validation.err().unwrap_or_default()).clicked() {
                                    if let Err(e) = self.next_step() {
                                        self.error_message = Some(e);
                                    }
                                }
                                if ui.button(tr(lang, "Back", "上一步")).clicked() {
                                    self.prev_step();
                                }
                            }
                        }
                    });
                });
            });

        if !is_open {
            action = Some(SkirmishWizardAction::Cancel);
        }

        action
    }

    fn show_step_template(
        &mut self,
        ui: &mut egui::Ui,
        lang: AppLanguage,
        folder_picker: Option<&crate::maps_ui::GameFolderPicker>,
    ) {
        ui.heading(tr(lang, "Step 1: Choose Template & Map Identity", "步驟 1：選擇地圖範本與名稱識別"));
        ui.add_space(4.0);

        ui.label(tr(lang, "Game Root Directory:", "遊戲根目錄："));
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.game_path_input);
            if ui.button(tr(lang, "Browse…", "瀏覽…")).clicked() {
                if let Some(picker) = folder_picker {
                    if let Some(path) = picker() {
                        self.game_path_input = path.to_string_lossy().into_owned();
                    }
                } else {
                    let mut dialog = rfd::FileDialog::new()
                        .set_title(tr(lang, "Select Game Folder", "選擇遊戲資料夾"));
                    let current = Path::new(self.game_path_input.trim());
                    if current.is_dir() {
                        dialog = dialog.set_directory(current);
                    }
                    if let Some(picked) = dialog.pick_folder() {
                        self.game_path_input = picked.to_string_lossy().into_owned();
                    }
                }
            }
        });

        ui.add_space(8.0);
        ui.label(tr(lang, "Template Base:", "底圖範本："));
        ui.radio_value(
            &mut self.template_kind,
            SkirmishTemplateKind::FlatTemplate,
            tr(lang, "Flat Template (Stable, recommended)", "平坦範本（穩定，推薦）"),
        ).on_hover_text(tr(
            lang,
            "Clones template map unchanged, then flattens terrain in memory as an unsaved edit.",
            "複製範本地圖（保留聚落與腳本），開啟後於記憶體整平地形（未儲存）。"
        ));

        ui.radio_value(
            &mut self.template_kind,
            SkirmishTemplateKind::EmptyScene,
            tr(lang, "Empty Scene (experimental)", "空白場景（實驗性）"),
        ).on_hover_text(tr(
            lang,
            "Constructs an empty scene with starter kit and flat terrain from scratch (experimental).",
            "從零建置具備起始包與平坦地形之空白場景（實驗性功能）。"
        ));

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(tr(lang, "Source slot (ENDL_###):", "來源槽位 (ENDL_###)："));
            ui.add(egui::DragValue::new(&mut self.source_slot).clamp_range(0..=999));
        });

        ui.horizontal(|ui| {
            ui.label(tr(lang, "Target slot (ENDL_###):", "目標槽位 (ENDL_###)："));
            ui.add(egui::DragValue::new(&mut self.target_slot).clamp_range(5..=999));
            if ui.button(tr(lang, "Auto-detect next free slot", "自動偵測下一可用槽位")).clicked() {
                let game = Path::new(self.game_path_input.trim());
                if game.is_dir() {
                    if let Ok(slot) = EndlessMapCatalog::new().get_next_free_slot(game) {
                        self.target_slot = slot;
                    }
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label(tr(lang, "Map Name:", "地圖名稱："));
            ui.text_edit_singleline(&mut self.target_name);
        });
    }

    fn show_step_title_briefing(&mut self, ui: &mut egui::Ui, lang: AppLanguage) {
        ui.heading(tr(lang, "Step 2: Map Title & Mission Briefing", "步驟 2：地圖標題與任務簡報"));
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label(tr(lang, "Map Name / Title:", "地圖名稱／標題："));
            ui.text_edit_singleline(&mut self.target_name);
        });

        ui.add_space(8.0);
        ui.label(tr(lang, "Mission Briefing Description:", "任務簡報說明："));
        ui.label(tr(
            lang,
            "Shown in the pre-game briefing screen and map objectives dialog.",
            "顯示於遊戲進入前的任務簡報畫面與地圖目標視窗。"
        ));
        ui.add(
            egui::TextEdit::multiline(&mut self.briefing)
                .desired_rows(6)
                .desired_width(f32::INFINITY)
        );
    }

    fn show_step_bases_resources(&mut self, ui: &mut egui::Ui, lang: AppLanguage) {
        // 沿用劇本面板的遊戲別名選單模式；此時新地圖尚未載入。
        if self.add_attack_waves {
            let aliases = against_rome_scenario::load_aliases(Path::new(self.game_path_input.trim()))
                .unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label(tr(lang, "Unit alias", "單位別名"));
                ui.text_edit_singleline(&mut self.wave_alias);
                egui::ComboBox::from_id_source("wizard_object_alias_catalog")
                    .selected_text(tr(lang, "Choose game alias", "從遊戲別名選擇"))
                    .show_ui(ui, |ui| {
                        for name in &aliases {
                            ui.selectable_value(&mut self.wave_alias, name.clone(), name);
                        }
                    });
            });
        }
        ui.heading(tr(lang, "Step 3: Bases, Resources & Game Rules", "步驟 3：勢力基地、資源配置與遊戲規則"));
        ui.add_space(4.0);

        ui.group(|ui| {
            ui.label(egui::RichText::new(tr(lang, "Settlement & Resource Generator", "聚落基地與資源產生器")).strong());
            ui.horizontal(|ui| {
                ui.label(tr(lang, "Number of Players:", "玩家數量："));
                ui.add(egui::DragValue::new(&mut self.players).clamp_range(2..=8));
            });

            ui.horizontal(|ui| {
                ui.label(tr(lang, "Tribe Culture:", "部落文化："));
                egui::ComboBox::from_id_source("wizard_tribe")
                    .selected_text(match self.tribe {
                        SettlementTribe::Germanic => tr(lang, "Germanic", "日耳曼"),
                        SettlementTribe::Celtic => tr(lang, "Celtic", "塞爾特"),
                        SettlementTribe::Roman => tr(lang, "Roman", "羅馬"),
                        SettlementTribe::Hun => tr(lang, "Hun", "匈人"),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.tribe, SettlementTribe::Germanic, tr(lang, "Germanic", "日耳曼"));
                        ui.selectable_value(&mut self.tribe, SettlementTribe::Celtic, tr(lang, "Celtic", "塞爾特"));
                        ui.selectable_value(&mut self.tribe, SettlementTribe::Roman, tr(lang, "Roman", "羅馬"));
                        ui.selectable_value(&mut self.tribe, SettlementTribe::Hun, tr(lang, "Hun", "匈人"));
                    });
            });

            ui.horizontal(|ui| {
                ui.label(tr(lang, "Symmetry Mode:", "對稱模式："));
                egui::ComboBox::from_id_source("wizard_symmetry")
                    .selected_text(match self.symmetry {
                        SymmetryMode::RotationalSymmetry => tr(lang, "Rotational Symmetry", "旋轉對稱"),
                        SymmetryMode::CentralSymmetry => tr(lang, "Central Symmetry", "中心對稱"),
                        SymmetryMode::TopologicalEquidistant => tr(lang, "Topological Equidistant", "拓撲等距"),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.symmetry, SymmetryMode::RotationalSymmetry, tr(lang, "Rotational Symmetry", "旋轉對稱"));
                        ui.selectable_value(&mut self.symmetry, SymmetryMode::CentralSymmetry, tr(lang, "Central Symmetry", "中心對稱"));
                        ui.selectable_value(&mut self.symmetry, SymmetryMode::TopologicalEquidistant, tr(lang, "Topological Equidistant", "拓撲等距"));
                    });
            });

            ui.horizontal(|ui| {
                ui.label(tr(lang, "Random Seed:", "隨機種子："));
                ui.add(egui::DragValue::new(&mut self.seed));
            });

            ui.checkbox(&mut self.include_nature, tr(lang, "Include Nature / Resources (trees, stone, animals)", "包含自然資源（森林樹木、石礦、動物）"));

            ui.horizontal(|ui| {
                ui.label(tr(lang, "Spawn radius ratio:", "生成半徑比例："));
                ui.add(egui::DragValue::new(&mut self.spawn_radius).speed(0.01).clamp_range(0.1..=0.9));
            });
        });

        ui.add_space(6.0);
        ui.group(|ui| {
            ui.label(egui::RichText::new(tr(lang, "Attack Waves", "進攻波次")).strong());
            ui.checkbox(&mut self.add_attack_waves, tr(lang, "Add attack waves", "新增進攻波次"));
            if self.add_attack_waves {
                ui.label(tr(
                    lang,
                    "Sensible defaults: 5 waves from enemy team 1, interval 60s, targeting player main house.",
                    "預設配置：敵方隊伍 1 派出 5 波進攻，間隔 60 秒，目的地為玩家主屋。"
                ));
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Enemy Team:", "敵方隊伍："));
                    ui.add(egui::DragValue::new(&mut self.wave_team).clamp_range(1..=7));
                    ui.label(tr(lang, "Wave Count:", "波數："));
                    ui.add(egui::DragValue::new(&mut self.wave_count).clamp_range(1..=255));
                });
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "First Wave (s):", "首波時間 (秒)："));
                    ui.add(egui::DragValue::new(&mut self.wave_first).clamp_range(0..=86400));
                    ui.label(tr(lang, "Interval (s):", "間隔時間 (秒)："));
                    ui.add(egui::DragValue::new(&mut self.wave_interval).clamp_range(1..=86400));
                });
            }
        });

        ui.add_space(6.0);
        ui.group(|ui| {
            ui.label(egui::RichText::new(tr(lang, "Victory & Defeat Objectives", "勝負目標")).strong());
            ui.checkbox(&mut self.add_objectives, tr(lang, "Add objectives", "新增勝負目標"));
            if self.add_objectives {
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Survive time limit (Victory):", "存活指定時間（勝利）："));
                    ui.add(egui::DragValue::new(&mut self.survive_seconds).clamp_range(10..=86400));
                    ui.label(tr(lang, "seconds", "秒"));
                });
                ui.checkbox(
                    &mut self.lose_if_main_house_destroyed,
                    tr(lang, "Lose if player main house is destroyed (Defeat)", "玩家主屋被摧毀即失敗")
                );
            }
        });
    }

    fn show_step_summary(&mut self, ui: &mut egui::Ui, lang: AppLanguage) {
        ui.heading(tr(lang, "Step 4: Summary & Creation", "步驟 4：建立摘要與確認"));
        ui.add_space(4.0);

        ui.group(|ui| {
            ui.label(format!("{}: {}", tr(lang, "Map Name", "地圖名稱"), self.target_name));
            ui.label(format!("{}: ENDL_{:03}", tr(lang, "Target Slot", "目標槽位"), self.target_slot));
            ui.label(format!("{}: {}", tr(lang, "Template", "範本模式"), match self.template_kind {
                SkirmishTemplateKind::FlatTemplate => tr(lang, "Flat Template (Stable)", "平坦範本（穩定）"),
                SkirmishTemplateKind::EmptyScene => tr(lang, "Empty Scene (experimental)", "空白場景（實驗性）"),
            }));
            ui.label(format!("{}: ENDL_{:03}", tr(lang, "Source Slot", "來源槽位"), self.source_slot));
            ui.label(format!("{}: {}", tr(lang, "Players", "玩家數量"), self.players));
            ui.label(format!(
                "{}: {}",
                tr(lang, "Attack Waves", "進攻波次"),
                if self.add_attack_waves {
                    format!("{} waves (team {}, interval {}s, destination = player main house)", self.wave_count, self.wave_team, self.wave_interval)
                } else {
                    tr(lang, "Disabled", "已停用").to_string()
                }
            ));
            ui.label(format!(
                "{}: {}",
                tr(lang, "Objectives", "勝負目標"),
                if self.add_objectives {
                    format!("Survive {}s + Defeat if main house destroyed", self.survive_seconds)
                } else {
                    tr(lang, "Disabled", "已停用").to_string()
                }
            ));
        });

        ui.add_space(8.0);
        ui.colored_label(
            Color32::from_rgb(255, 200, 80),
            tr(
                lang,
                "Notice: Clicking 'Create' will build the map, open it, and apply all chosen generators as normal undoable in-memory edits. Design Check will run and display results. Nothing is saved automatically — please remember to Safe Save.",
                "提示：點擊「建立」將會建置並開啟地圖，隨後將所選產生器作為一般可復原的記憶體編輯套用。系統將自動執行設計檢查並顯示結果。所有步驟均不會自動儲存——請記得手動執行安全儲存。"
            )
        );
    }
}

impl EditorApp {
    pub fn open_skirmish_wizard(&mut self) {
        self.skirmish_wizard.open = true;
        self.skirmish_wizard.step = SkirmishWizardStep::Template;
        self.skirmish_wizard.error_message = None;

        if self.skirmish_wizard.game_path_input.trim().is_empty() {
            if !self.maps.game_path_input.trim().is_empty() {
                self.skirmish_wizard.game_path_input = self.maps.game_path_input.clone();
            } else if !self.map_path_input.trim().is_empty() {
                if let Some(parent) = Path::new(self.map_path_input.trim()).parent() {
                    if let Some(game_root) = parent.parent() {
                        self.skirmish_wizard.game_path_input = game_root.to_string_lossy().into_owned();
                    }
                }
            }
        }

        if self.skirmish_wizard.target_name.trim().is_empty()
            || self.skirmish_wizard.target_name == "New Blank Map"
        {
            self.skirmish_wizard.target_name = "New Skirmish Map".to_string();
        }
        if self.skirmish_wizard.briefing.trim().is_empty() {
            self.skirmish_wizard.briefing =
                "Skirmish Battle: defend your settlement and destroy the invaders.".to_string();
        }

        let game_path = PathBuf::from(self.skirmish_wizard.game_path_input.trim());
        if game_path.is_dir() {
            if let Ok(slot) = EndlessMapCatalog::new().get_next_free_slot(&game_path) {
                self.skirmish_wizard.target_slot = slot;
            }
        }
    }

    pub fn show_skirmish_wizard(&mut self, ctx: &egui::Context) {
        if !self.skirmish_wizard.open {
            return;
        }
        let lang = self.lang();
        let picker = self.maps.game_folder_picker.as_ref();
        let action = self.skirmish_wizard.show(ctx, lang, picker);
        match action {
            Some(SkirmishWizardAction::Create) => {
                if let Err(e) = self.execute_skirmish_wizard() {
                    self.skirmish_wizard.error_message = Some(e.clone());
                    self.status_message = e;
                    self.status_is_error = true;
                }
            }
            Some(SkirmishWizardAction::Cancel) => {
                self.skirmish_wizard.cancel();
            }
            None => {}
        }
    }

    pub fn execute_skirmish_wizard(&mut self) -> Result<(), String> {
        self.skirmish_wizard.validate_wave_alias()
            .map_err(|e| format!("Step 3 (Attack waves) failed: {e}"))?;
        // Pre-validation and clean map switch check
        self.require_clean_map_switch()
            .map_err(|e| format!("Step 1 (Map creation) failed: {e}"))?;

        let game_path = PathBuf::from(self.skirmish_wizard.game_path_input.trim());
        if !game_path.is_dir() {
            return Err(format!(
                "Step 1 (Map creation) failed: game path is invalid or does not exist: {}",
                game_path.display()
            ));
        }

        let target_name = self.skirmish_wizard.target_name.trim().to_string();
        if target_name.is_empty() {
            return Err("Step 1 (Map creation) failed: map name must not be blank".into());
        }

        let target_slot = if self.skirmish_wizard.target_slot >= 5 {
            self.skirmish_wizard.target_slot
        } else {
            EndlessMapCatalog::new()
                .get_next_free_slot(&game_path)
                .map_err(|e| format!("Step 1 (Map creation) failed to find free slot: {e}"))?
        };
        self.skirmish_wizard.target_slot = target_slot;

        let source_map_id = format!("ENDL_{:03}", self.skirmish_wizard.source_slot);
        let mut spec = BlankMapSpec::new(source_map_id, target_slot, target_name.clone());
        let trimmed_briefing = self.skirmish_wizard.briefing.trim();
        if !trimmed_briefing.is_empty() {
            spec = spec.with_briefing(trimmed_briefing);
        }

        spec.validate(&game_path)
            .map_err(|e| format!("Step 1 (Map creation) failed: {e}"))?;

        // Sync to maps_ui fields
        self.maps.game_path_input = self.skirmish_wizard.game_path_input.clone();
        self.maps.blank_source_slot = self.skirmish_wizard.source_slot;
        self.maps.blank_target_slot = target_slot;
        self.maps.blank_target_name = target_name;
        self.maps.blank_briefing = self.skirmish_wizard.briefing.clone();

        // 1. Build and install map (Step 1 / 2)
        let map_dir = match self.skirmish_wizard.template_kind {
            SkirmishTemplateKind::FlatTemplate => {
                let (info, _) = self
                    .build_and_install_flat_template(&game_path, &spec)
                    .map_err(|e| format!("Step 1 (Map creation) failed: {e}"))?;
                info.directory_path
            }
            SkirmishTemplateKind::EmptyScene => {
                let res = self
                    .build_and_install_blank_map(&game_path, &spec)
                    .map_err(|e| format!("Step 1 (Map creation) failed: {e}"))?;
                res.map_path
            }
        };

        // Ensure DATA and Scenario are loaded
        if !self.placement.is_loaded() && !self.placement.is_dirty() {
            self.placement.load_session(&map_dir)
                .map_err(|e| format!("Step 2 (Load map) failed: {e}"))?;
        }
        if !self.scenario.is_loaded() && !self.scenario.dirty() {
            self.scenario.load_session(&map_dir)
                .map_err(|e| format!("Step 2 (Load map) failed: {e}"))?;
        }

        if self.skirmish_wizard.template_kind == SkirmishTemplateKind::EmptyScene {
            if !self.apply_starter_kit(0, 8192.0, 8192.0) {
                return Err(format!("Step 3 (Starter kit) failed: {}", self.spawn_ui.status().0));
            }
        }

        // 2. Step 3: Bases & resources (Settlement generator)
        if self.skirmish_wizard.template_kind == SkirmishTemplateKind::FlatTemplate {
            self.environment.settlement_players = self.skirmish_wizard.players;
            self.environment.settlement_tribe = self.skirmish_wizard.tribe;
            self.environment.settlement_symmetry = self.skirmish_wizard.symmetry;
            self.environment.settlement_seed = self.skirmish_wizard.seed;
            self.environment.settlement_include_nature = self.skirmish_wizard.include_nature;
            self.environment.settlement_spawn_radius = self.skirmish_wizard.spawn_radius;

            let s = self.session.as_ref().ok_or_else(|| {
                "Step 3 (Settlement generation) failed: terrain session unavailable".to_string()
            })?;

            self.environment
                .generate_settlement(
                    s.vertex_size,
                    Some(s.heights()),
                    Some((s.collision_size, s.collision())),
                    self.water_surface,
                    self.height_step,
                )
                .map_err(|e| format!("Step 3 (Settlement generation) failed: {e}"))?;

            let settlement_count = self.apply_settlement()
                .map_err(|e| format!("Step 3 (Settlement generation) failed: {e}"))?;

            self.environment.plan_queued(crate::environment_ui::EnvApply::Settlement, settlement_count, "");
        }

        // 3. Step 3: Attack waves (if toggled)
        if self.skirmish_wizard.add_attack_waves {
            if let Some(target) = self.wave_player_target(&map_dir) {
                self.scenario.wave_generator.destination = target;
            }
            self.scenario.wave_generator.alias = self.skirmish_wizard.wave_alias.trim().to_string();
            self.scenario.wave_generator.team = self.skirmish_wizard.wave_team;
            self.scenario.wave_generator.waves = self.skirmish_wizard.wave_count;
            self.scenario.wave_generator.first = self.skirmish_wizard.wave_first;
            self.scenario.wave_generator.interval = self.skirmish_wizard.wave_interval;

            self.scenario
                .generate_attack_waves()
                .map_err(|e| format!("Step 3 (Attack waves) failed: {e}"))?;
        }

        let mut defeat_skipped = false;
        // 4. Step 3: Objectives (if toggled)
        if self.skirmish_wizard.add_objectives {
            // Survive N seconds
            self.scenario
                .add_objective(Some(self.skirmish_wizard.survive_seconds), None, 0)
                .map_err(|e| format!("Step 3 (Objectives) failed: {e}"))?;

            // Lose if main house destroyed
            if self.skirmish_wizard.lose_if_main_house_destroyed {
                let house_target = self
                    .scenario
                    .document()
                    .spawns
                    .iter()
                    .find(|s| {
                        s.team == 0
                            && against_rome_maps::design_validator::MapDesignValidator::is_main_house(&s.alias)
                    })
                    .map(|s| s.id);

                if let Some(id) = house_target {
                    self.scenario
                        .add_objective(None, Some(id), 0)
                        .map_err(|e| format!("Step 3 (Objectives) failed: {e}"))?;
                } else {
                    defeat_skipped = true;
                }
            }
        }

        // 5. Run Design Check
        self.run_design_check();

        // 6. Remind user to Safe Save; status message
        self.status_message = format!(
            "Skirmish map ENDL_{:03} created with {} player bases! All edits are unsaved in memory. Please remember to Safe Save. / 對戰地圖 ENDL_{:03} 建立完成（{} 勢力）！所有變更暫存於記憶體，請記得執行安全儲存。",
            target_slot, self.skirmish_wizard.players, target_slot, self.skirmish_wizard.players
        );
        if defeat_skipped {
            self.status_message.push_str(" Defeat-on-main-house skipped: the main house is a DATA object, not a script spawn; add a defeat rule manually in the Scenario panel / 已略過「主屋被毀即失敗」：主屋是 DATA 物件而非腳本生成，請在劇本面板手動加入失敗條件");
        }
        self.status_is_error = false;
        self.skirmish_wizard.open = false;
        self.skirmish_wizard.error_message = None;

        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::storage::safe_save_map;
    use crate::terrain::TerrainSession;
    use std::collections::BTreeSet;

    fn temp_test_dir(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("arm_wizard_test_{tag}_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_test_data(map: &Path, n: usize, live: &[(u16, Option<u16>)], cursor: u32) {
        use against_rome_placement::{COLUMNS, SEGMENTS};
        std::fs::create_dir_all(map.join("DATA")).unwrap();
        let mut o = vec![0u8; 16 + n * (79 + COLUMNS.iter().sum::<usize>())];
        for (i, v) in [1u32, n as u32, 30, 30].iter().enumerate() {
            o[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        let mut p = vec![0u8; 8 + n * 3 * 17];
        p[..4].copy_from_slice(&1u32.to_le_bytes());
        p[4..8].copy_from_slice(&((n * 3) as u32).to_le_bytes());
        let mut d = vec![0u8; 8 + n * SEGMENTS.iter().sum::<usize>()];
        d[..4].copy_from_slice(&1u32.to_le_bytes());
        d[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        for s in 0..n {
            let r = 16 + s * 79;
            for off in [67, 69, 71, 73, 77] {
                o[r + off..r + off + 2].fill(255);
            }
            for c in 0..18 {
                if COLUMNS[c] == 2 {
                    let off = 16 + n * 79 + n * COLUMNS[..c].iter().sum::<usize>() + s * COLUMNS[c];
                    o[off..off + 2].fill(255);
                }
            }
            if let Some(&(typ, parent)) = live.get(s) {
                o[r] = 1;
                o[r + 3..r + 7].copy_from_slice(&(s as u32 + 1).to_le_bytes());
                o[r + 75..r + 77].copy_from_slice(&typ.to_le_bytes());
                for off in [67, 69] {
                    o[r + off..r + off + 2].copy_from_slice(&(s as u16).to_le_bytes());
                }
                for (c, v) in [(1, s as u16), (13, parent.unwrap_or(65535))] {
                    let off = 16 + n * 79 + n * COLUMNS[..c].iter().sum::<usize>() + s * COLUMNS[c];
                    o[off..off + 2].copy_from_slice(&v.to_le_bytes());
                }
                let off = 8 + s * 17;
                p[off] = 1;
                for (a, v) in [4000. + s as f32 * 20., 20., 4000.].iter().enumerate() {
                    p[off + 1 + a * 4..off + 5 + a * 4].copy_from_slice(&v.to_le_bytes());
                }
                p[off + 13..off + 17].copy_from_slice(&0f32.to_le_bytes());
                d[8 + s * 9] = 1;
            }
        }
        let mut l = vec![0u8; against_rome_data::pool::PAYLOAD_LEN];
        l[..4].copy_from_slice(&1u32.to_le_bytes());
        l[4..8].copy_from_slice(&1024u32.to_le_bytes());
        let mut e = vec![0u8; 94];
        e[..4].copy_from_slice(&1u32.to_le_bytes());
        e[34..38].copy_from_slice(&cursor.to_le_bytes());
        for (name, b) in [("objects", o), ("position", p), ("objdata", d), ("light", l), ("engine", e)] {
            std::fs::write(map.join(format!("DATA/{name}.dat")), b).unwrap();
        }
    }

    pub(crate) fn setup_test_game_fixture(tag: &str) -> (PathBuf, PathBuf) {
        let root = temp_test_dir(tag);
        let src = root.join("MAPS").join("ENDL_001");
        std::fs::create_dir_all(&src).unwrap();
        let mut terrain = TerrainSession::create_blank(129, 20);
        safe_save_map(&src, &mut terrain).unwrap();
        std::fs::write(src.join("level.put"), "var:map_name =\"Template\";\r\n").unwrap();
        std::fs::write(src.join("briefing.put"), "var:briefing_titel_1 =\"Original Title\";\r\n").unwrap();
        let us_dir = src.join("TEXT").join("US");
        std::fs::create_dir_all(&us_dir).unwrap();
        std::fs::write(us_dir.join("briefing.put"), "var:briefing_titel_1 =\"Original Title\";\r\n").unwrap();
        std::fs::write(src.join("map.sdl"), "[settlement]\r\nrefpos=100.0,0.0,100.0\r\n").unwrap();
        std::fs::write(src.join("boden.ini"), "[Dimension]\r\n32\r\n[Waterlevel]\r\n0.0\r\n[Heightmapstep]\r\n4.0\r\n").unwrap();
        let mut boden_txt = String::from("[Dimension]\n32\n\n[Texturen]\n");
        for _ in 0..1024 {
            boden_txt.push_str("GRAS_1\n");
        }
        std::fs::write(src.join("boden.txt"), boden_txt).unwrap();

        std::fs::create_dir_all(src.join("SCRIPT")).unwrap();
        std::fs::write(
            src.join("SCRIPT/ak_level.bci"),
            against_rome_scenario::BciImage::idle().serialize(),
        ).unwrap();

        // Write DATA for template
        write_test_data(&src, 200, &[], 100);

        // System defaults
        std::fs::create_dir_all(root.join("SYSTEM/CLAK")).unwrap();
        std::fs::create_dir_all(root.join("SYSTEM/DATA_MP/DEFAULTS")).unwrap();
        std::fs::write(
            root.join("SYSTEM/CLAK/cl_scint.ini"),
            "[ObjDefName]\nHOUSE=BauGerHau00_Haupthaus\nTROOP=FigGerInf02_Zweihandaxt\n",
        ).unwrap();

        // Object defaults for settlement generation
        let mut dummy_app = EditorApp::initial_state();
        dummy_app.environment.settlement_players = 2;
        dummy_app.environment.settlement_include_nature = true;
        let s = dummy_app.session.as_ref().unwrap();
        let _ = dummy_app.environment.generate_settlement(
            s.vertex_size,
            Some(s.heights()),
            Some((s.collision_size, s.collision())),
            0.0,
            4.0,
        );
        let mut names = BTreeSet::new();
        names.insert("BauGerHau00_Haupthaus".to_string());
        names.insert("FigGerInf02_Zweihandaxt".to_string());
        if let Some(dist) = &dummy_app.environment.last_distribution {
            for p in &dist.players {
                names.extend(p.buildings.iter().map(|x| x.type_name.clone()));
                names.extend(p.wildlife_and_food.iter().map(|x| x.type_name.clone()));
                names.extend(
                    p.forest_trees
                        .iter()
                        .chain(&p.stone_quarries)
                        .map(|x| x.type_name.clone()),
                );
            }
        }
        let mut definitions = String::from("[ObjectDefaults]\n");
        let mut live = Vec::new();
        for (i, name) in names.iter().enumerate() {
            let id = 1000 + i as u16;
            live.push((id, None));
            let mut columns = vec!["0".to_owned(); 53];
            columns[0] = id.to_string();
            columns[1] = "1".into();
            columns[52] = name.clone();
            definitions.push_str(&columns.join(","));
            definitions.push('\n');
        }
        std::fs::write(root.join("SYSTEM/DATA_MP/DEFAULTS/objdef.dau"), definitions).unwrap();

        // Update template DATA with live objects capacity
        write_test_data(&src, 200, &live, 100);

        (root, src)
    }

    #[test]
    fn test_skirmish_wizard_cancel_leaves_everything_unchanged() {
        let (root, _src) = setup_test_game_fixture("cancel");
        let mut app = EditorApp::initial_state();
        app.skirmish_wizard.game_path_input = root.to_string_lossy().into_owned();
        app.skirmish_wizard.target_name = "Cancelled Map".to_string();
        app.skirmish_wizard.target_slot = 5;
        app.skirmish_wizard.open = true;

        let before_loaded = app.loaded_map_path.clone();

        app.skirmish_wizard.cancel();

        assert!(!app.skirmish_wizard.open);
        assert_eq!(app.loaded_map_path, before_loaded);
        assert!(!root.join("MAPS/ENDL_005").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_skirmish_wizard_ui_navigation_and_validation() {
        let (root, _src) = setup_test_game_fixture("nav");
        let mut wizard = SkirmishWizardUi::default();
        wizard.open = true;

        // Step 1: empty game path must fail
        wizard.game_path_input = String::new();
        assert!(wizard.next_step().is_err());
        assert_eq!(wizard.step, SkirmishWizardStep::Template);

        // Step 1: valid game path, empty name must fail
        wizard.game_path_input = root.to_string_lossy().into_owned();
        wizard.target_name = "   ".to_string();
        assert!(wizard.next_step().is_err());
        assert_eq!(wizard.step, SkirmishWizardStep::Template);

        // Step 1 -> Step 2
        wizard.target_name = "Good Skirmish".to_string();
        assert!(wizard.next_step().is_ok());
        assert_eq!(wizard.step, SkirmishWizardStep::TitleBriefing);

        // Step 2 -> Step 3
        wizard.briefing = "Custom briefing text".to_string();
        assert!(wizard.next_step().is_ok());
        assert_eq!(wizard.step, SkirmishWizardStep::BasesResources);

        // Step 3: invalid players must fail
        wizard.players = 1;
        assert!(wizard.next_step().is_err());
        assert_eq!(wizard.step, SkirmishWizardStep::BasesResources);

        // Step 3 -> Step 4
        wizard.players = 2;
        assert!(wizard.next_step().is_err());
        wizard.add_attack_waves = false;
        assert!(wizard.validate_current_step().is_ok());
        wizard.add_attack_waves = true;
        wizard.wave_alias = "TROOP".into();
        assert!(wizard.next_step().is_ok());
        assert_eq!(wizard.step, SkirmishWizardStep::Summary);

        // Step 4: prev_step goes back
        wizard.prev_step();
        assert_eq!(wizard.step, SkirmishWizardStep::BasesResources);
        wizard.prev_step();
        assert_eq!(wizard.step, SkirmishWizardStep::TitleBriefing);
        wizard.prev_step();
        assert_eq!(wizard.step, SkirmishWizardStep::Template);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_skirmish_wizard_full_run_produces_map_with_briefing_settlements_waves_objectives() {
        let (root, _src) = setup_test_game_fixture("full_run");
        let mut app = EditorApp::initial_state();
        app.skirmish_wizard.game_path_input = root.to_string_lossy().into_owned();
        app.skirmish_wizard.source_slot = 1;
        app.skirmish_wizard.target_slot = 5;
        app.skirmish_wizard.target_name = "Alpine Clash".to_string();
        app.skirmish_wizard.briefing = "Alpine Skirmish: Hold the mountain pass!".to_string();
        app.skirmish_wizard.players = 2;
        app.skirmish_wizard.tribe = SettlementTribe::Germanic;
        app.skirmish_wizard.symmetry = SymmetryMode::RotationalSymmetry;
        app.skirmish_wizard.seed = 42;
        app.skirmish_wizard.include_nature = true;
        app.skirmish_wizard.add_attack_waves = true;
        app.skirmish_wizard.wave_alias = "TROOP".into();
        app.skirmish_wizard.wave_count = 5;
        app.skirmish_wizard.wave_first = 60;
        app.skirmish_wizard.wave_interval = 60;
        app.skirmish_wizard.add_objectives = true;
        app.skirmish_wizard.survive_seconds = 300;
        app.skirmish_wizard.lose_if_main_house_destroyed = true;

        let res = app.execute_skirmish_wizard();
        assert!(res.is_ok(), "execute_skirmish_wizard failed: {:?}", res);

        let target_dir = root.join("MAPS/ENDL_005");
        let expected = std::fs::canonicalize(&target_dir).unwrap();
        let actual = std::fs::canonicalize(app.loaded_map_path.as_ref().unwrap()).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(app.ambience.briefing_title, "Alpine Skirmish: Hold the mountain pass!");

        // Settlements present as undoable edit
        assert!(
            app.has_settlement_undo(),
            "settlement undo must contain the applied settlement"
        );
        assert!(app.placement.is_dirty(), "placement DATA must be dirty (unsaved)");

        // Waves present
        let events = app.scenario.document().events.clone();
        let wave_events: Vec<_> = events.iter().filter(|e| e.name.starts_with("Wave_")).collect();
        assert_eq!(wave_events.len(), 5, "must have 5 wave events, found: {:?}", events);

        assert_eq!(app.scenario.wave_generator.alias, "TROOP");
        assert!(app.scenario.document().spawns.iter().all(|s|
            !against_rome_maps::design_validator::MapDesignValidator::is_main_house(&s.alias)));
        assert!(app.status_message.contains("Defeat-on-main-house skipped:"));
        assert!(app.status_message.contains("已略過「主屋被毀即失敗」"));

        // Objectives present
        assert!(
            events.iter().any(|e| e.name.starts_with("Survive_")),
            "must have survival objective event, found: {:?}",
            events
        );

        // Unsaved edits status
        assert!(app.has_unsaved_changes(), "map must have unsaved edits in memory");

        // Design check executed and opened
        assert!(app.design_check.open, "Design Check panel must be opened");
        assert!(app.design_check.report.is_some(), "Design Check report must be generated");

        // Status message reminds Safe Save
        assert!(
            app.status_message.contains("Safe Save") || app.status_message.contains("安全儲存"),
            "status message must remind user to Safe Save: {}",
            app.status_message
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_skirmish_wizard_step3_failure_reports_step_and_leaves_created_map_without_waves() {
        let (root, _src) = setup_test_game_fixture("step3_fail");
        let mut app = EditorApp::initial_state();
        app.skirmish_wizard.game_path_input = root.to_string_lossy().into_owned();
        app.skirmish_wizard.source_slot = 1;
        app.skirmish_wizard.target_slot = 5;
        app.skirmish_wizard.target_name = "Broken Skirmish".to_string();
        app.skirmish_wizard.briefing = "Broken step 3".to_string();
        // Zero players will fail settlement generation in step 3
        app.skirmish_wizard.players = 0;
        app.skirmish_wizard.add_attack_waves = true;
        app.skirmish_wizard.wave_alias = "TROOP".into();
        app.skirmish_wizard.add_objectives = true;

        let res = app.execute_skirmish_wizard();
        assert!(res.is_err(), "must fail on step 3");
        let err = res.unwrap_err();

        // Must report step 3 failed
        assert!(
            err.contains("Step 3"),
            "error must state Step 3 failed: {err}"
        );

        // Created map from step 1 stays open
        let target_dir = root.join("MAPS/ENDL_005");
        let expected = std::fs::canonicalize(&target_dir).unwrap();
        let actual = std::fs::canonicalize(app.loaded_map_path.as_ref().unwrap()).unwrap();
        assert_eq!(
            actual,
            expected,
            "already created map must stay loaded"
        );

        // No waves or objectives were created
        assert!(
            app.scenario.document().events.is_empty(),
            "no wave or objective events should have been created on step 3 failure"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_skirmish_wizard_bilingual_and_window_toggle() {
        let mut app = EditorApp::initial_state();
        assert!(!app.skirmish_wizard.open);
        app.open_skirmish_wizard();
        assert!(app.skirmish_wizard.open);
        assert_eq!(app.skirmish_wizard.step, SkirmishWizardStep::Template);

        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            app.show_skirmish_wizard(ctx);
        });
        assert!(app.skirmish_wizard.open);

        // Cancel closes the window
        app.skirmish_wizard.cancel();
        assert!(!app.skirmish_wizard.open);
    }
}
