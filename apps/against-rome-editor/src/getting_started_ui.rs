//! 新手入門與地圖設計工作流程指引面板 (Getting Started Workflow Guide)
//! 提供標準製圖流程五大步驟導引，並提供一鍵開啟對應工具面板之快捷按鈕。

use against_rome_ambience::Language as AppLanguage;
use crate::app::tr;
use eframe::egui::{self, Color32, RichText, Vec2};

/// 點選流程導引按鈕時發送之動作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GettingStartedAction {
    OpenMaps,
    OpenSkirmishWizard,
    OpenMapProperties,
    OpenToolbox,
    Toggle3DView,
    OpenTextures,
    OpenNavigation,
    OpenPlacement,
    OpenSpawns,
    OpenScenario,
    RunDesignCheck,
    OpenMinimap,
}

impl GettingStartedAction {
    pub fn requires_map(self) -> bool {
        !matches!(self, Self::OpenMaps | Self::OpenSkirmishWizard)
    }
}

#[derive(Debug, Clone)]
pub struct GettingStartedUi {
    pub open: bool,
}

impl Default for GettingStartedUi {
    fn default() -> Self {
        Self { open: false }
    }
}

impl GettingStartedUi {
    pub fn new() -> Self {
        Self::default()
    }

    /// 繪製「新手入門」導引視窗。若使用者點選面板快捷按鈕，回傳相應之動作枚舉。
    pub fn show(&mut self, ctx: &egui::Context, lang: AppLanguage, map_loaded: bool) -> Option<GettingStartedAction> {
        if !self.open {
            return None;
        }

        let mut action = None;
        let mut open = self.open;
        let mut close_clicked = false;
        let title = tr(
            lang,
            "Getting Started: Map Design Guide",
            "新手入門：地圖製作流程指引",
        );

        egui::Window::new(title)
            .id(egui::Id::new("getting_started_window"))
            .open(&mut open)
            .default_size(Vec2::new(480.0, 520.0))
            .resizable(true)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(tr(
                        lang,
                        "Welcome to Against Rome Map Editor! Follow the standard 5-step workflow below to create balanced maps.",
                        "歡迎使用 Against Rome 地圖編輯器！請依照以下 5 大核心步驟製作平衡完整的自製地圖。",
                    )).color(Color32::from_rgb(200, 220, 255)));
                    ui.add_space(6.0);
                    if !map_loaded {
                        ui.label(tr(lang, "Load a map before using map tools.", "請先載入地圖，再使用地圖工具。"));
                    }

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        // 步驟一：地圖設定
                        ui.group(|ui| {
                            ui.heading(tr(lang, "1. Map Setup & Initialization", "1. 地圖建立與基本設定"));
                            ui.label(tr(
                                lang,
                                "Open an existing map, create a blank template, or use the 4-step Skirmish Wizard to quickly build a balanced scenario.",
                                "開啟現有地圖、建立空白範本，或使用四步驟對戰精靈快速生成兼具聚落與勝負目標的地圖架構。",
                            ));
                            ui.horizontal_wrapped(|ui| {
                                if ui.button(tr(lang, "Map Browser", "地圖瀏覽")).on_hover_text(tr(lang, "Open map file browser and template list", "開啟地圖檔案瀏覽器與範本清單")).clicked() {
                                    action = Some(GettingStartedAction::OpenMaps);
                                }
                                if ui.button(tr(lang, "Skirmish Wizard…", "新增對戰精靈…")).on_hover_text(tr(lang, "Launch 4-step skirmish map setup wizard", "啟動 4 步驟對戰地圖設定精靈")).clicked() {
                                    action = Some(GettingStartedAction::OpenSkirmishWizard);
                                }
                                if ui.add_enabled(map_loaded, egui::Button::new(tr(lang, "Map Properties", "地圖屬性"))).on_hover_text(tr(lang, "Edit map title, briefing, and starting camera", "編輯地圖標題、簡報與初始相機位置")).clicked() {
                                    action = Some(GettingStartedAction::OpenMapProperties);
                                }
                            });
                        });
                        ui.add_space(4.0);

                        // 步驟二：地形起伏與水域
                        ui.group(|ui| {
                            ui.heading(tr(lang, "2. Terrain Sculpting & Water", "2. 地形起伏與水面雕刻"));
                            ui.label(tr(
                                lang,
                                "Sculpt hills, plateaus, slopes, and rivers using height tools. Switch to 3D View to inspect elevation in true game scale.",
                                "使用升高、降低、整平、平滑與水面工具雕刻山脈與河道。可切換 3D 視角檢視真實遊戲等角比例。",
                            ));
                            ui.horizontal_wrapped(|ui| {
                                if ui.add_enabled(map_loaded, egui::Button::new(tr(lang, "Terrain Toolbox", "開啟地形工具箱"))).on_hover_text(tr(lang, "Toggle terrain sculpt and brush tools", "切換高度筆刷與水面調整工具箱")).clicked() {
                                    action = Some(GettingStartedAction::OpenToolbox);
                                }
                                if ui.add_enabled(map_loaded, egui::Button::new(tr(lang, "Toggle 3D View", "切換 2D/3D 視角"))).on_hover_text(tr(lang, "Switch between 2D canvas and 3D preview", "切換 2D 畫布與 3D 原生視角")).clicked() {
                                    action = Some(GettingStartedAction::Toggle3DView);
                                }
                            });
                        });
                        ui.add_space(4.0);

                        // 步驟三：地表材質與通行性
                        ui.group(|ui| {
                            ui.heading(tr(lang, "3. Textures & Passability", "3. 地表材質與通行性驗證"));
                            ui.label(tr(
                                lang,
                                "Paint grass, sand, rock, and roads. Use the Eyedropper (I) to pick existing materials. Check passability routes and APAT footprints.",
                                "繪製草地、沙地、碎石與道路材質（按 I 可吸管取樣）。檢查通行網格與建築占地是否暢通。",
                            ));
                            ui.horizontal_wrapped(|ui| {
                                if ui.add_enabled(map_loaded, egui::Button::new(tr(lang, "Materials", "地表材質"))).on_hover_text(tr(lang, "Paint surface textures and blending layers", "繪製地表材質與過渡圖層")).clicked() {
                                    action = Some(GettingStartedAction::OpenTextures);
                                }
                                if ui.add_enabled(map_loaded, egui::Button::new(tr(lang, "Navigation", "通行導航"))).on_hover_text(tr(lang, "Inspect passability grid, APAT footprints, and routes", "檢視通行網格、APAT 建築占地與尋路路徑")).clicked() {
                                    action = Some(GettingStartedAction::OpenNavigation);
                                }
                            });
                        });
                        ui.add_space(4.0);

                        // 步驟四：物件放置與部隊生成
                        ui.group(|ui| {
                            ui.heading(tr(lang, "4. Objects & Unit Spawns", "4. 物件放置與部隊生成"));
                            ui.label(tr(
                                lang,
                                "Place Townhalls, defensive towers, trees, and rocks. Set script-spawned armies and rally points. Press F to frame selected objects in 3D/2D.",
                                "配置主屋、防禦塔、城牆與自然景觀；配置腳本生成的初始部隊與集結點。按 F 可快速將鏡頭對齊選取物件。",
                            ));
                            ui.horizontal_wrapped(|ui| {
                                if ui.add_enabled(map_loaded, egui::Button::new(tr(lang, "Placed Objects", "已放置物件"))).on_hover_text(tr(lang, "Open DATA objects placement inspector", "開啟 DATA 物件檢查器與屬性面板")).clicked() {
                                    action = Some(GettingStartedAction::OpenPlacement);
                                }
                                if ui.add_enabled(map_loaded, egui::Button::new(tr(lang, "Spawns", "腳本生成點"))).on_hover_text(tr(lang, "Place and manage script-spawned units and buildings", "配置與管理腳本生成的部隊與建築")).clicked() {
                                    action = Some(GettingStartedAction::OpenSpawns);
                                }
                            });
                        });
                        ui.add_space(4.0);

                        // 步驟五：劇本事件、驗證與安全儲存
                        ui.group(|ui| {
                            ui.heading(tr(lang, "5. Scenario, Validation & Save", "5. 劇本事件、檢查與安全儲存"));
                            ui.label(tr(
                                lang,
                                "Define victory/defeat conditions, attack waves, and cutscene cameras. Run Design Check (F5) before Safe Save. Save the loaded map before exporting its ZIP bundle; exports read saved files.",
                                "設定勝利／失敗目標、波次進攻與劇本事件。Safe Save 前執行設計檢查 (F5)。匯出 ZIP 套件前先安全儲存目前地圖，套件會讀取磁碟上已儲存的檔案。",
                            ));
                            ui.horizontal_wrapped(|ui| {
                                if ui.add_enabled(map_loaded, egui::Button::new(tr(lang, "Scenario / Camera", "劇本／鏡頭"))).on_hover_text(tr(lang, "Edit triggers, conditions, and camera actions", "編輯觸發事件、判斷條件與鏡頭動作")).clicked() {
                                    action = Some(GettingStartedAction::OpenScenario);
                                }
                                if ui.add_enabled(map_loaded, egui::Button::new(tr(lang, "Design Check (F5)", "設計檢查 (F5)"))).on_hover_text(tr(lang, "Run design validation and diagnose errors", "執行設計驗證並診斷潛在錯誤")).clicked() {
                                    action = Some(GettingStartedAction::RunDesignCheck);
                                }
                                if ui.add_enabled(map_loaded, egui::Button::new(tr(lang, "Minimap", "小地圖"))).on_hover_text(tr(lang, "Toggle minimap overview window", "切換小地圖總覽視窗")).clicked() {
                                    action = Some(GettingStartedAction::OpenMinimap);
                                }
                            });
                        });
                        ui.add_space(8.0);

                        // 常用快捷鍵速查
                        ui.separator();
                        ui.label(RichText::new(tr(lang, "Quick Keyboard Shortcuts Reference", "常用快捷鍵速查")).strong());
                        egui::Grid::new("shortcuts_grid").striped(true).show(ui, |ui| {
                            ui.label(RichText::new("1 – 9").strong());
                            ui.label(tr(lang, "Switch terrain sculpting & material tools", "切換地形雕刻與材質工具"));
                            ui.end_row();

                            ui.label(RichText::new("[ / ]").strong());
                            ui.label(tr(lang, "Adjust brush radius (1–64)", "調整筆刷半徑 (1–64)"));
                            ui.end_row();

                            ui.label(RichText::new("F").strong());
                            ui.label(tr(lang, "Frame / center view on selected object", "視角立即置中聚焦對齊選取物件"));
                            ui.end_row();

                            ui.label(RichText::new("I").strong());
                            ui.label(tr(lang, "Eyedropper to sample terrain material", "地表材質吸管取樣"));
                            ui.end_row();

                            ui.label(RichText::new("F5").strong());
                            ui.label(tr(lang, "Run design check & validation", "立即執行地圖設計檢查與驗證"));
                            ui.end_row();

                            ui.label(RichText::new("PageUp / PageDn").strong());
                            ui.label(tr(lang, "Zoom view in / out", "視角縮放 (Zoom In / Out)"));
                            ui.end_row();

                            ui.label(RichText::new("Home").strong());
                            ui.label(tr(lang, "Reset view to default scale", "重設視角至預設比例"));
                            ui.end_row();

                            ui.label(RichText::new("Ctrl+Z / Ctrl+Y").strong());
                            ui.label(tr(lang, "Undo / Redo action", "復原 / 重做編輯操作"));
                            ui.end_row();
                        });
                    });

                    ui.add_space(6.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(tr(lang, "Close", "關閉導引")).clicked() {
                            close_clicked = true;
                        }
                    });
                });
            });

        if close_clicked {
            open = false;
        }
        self.open = open;
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_getting_started_ui_default_and_toggle() {
        let mut ui = GettingStartedUi::default();
        assert!(!ui.open);
        ui.open = true;
        assert!(ui.open);
    }

    #[test]
    fn test_getting_started_action_variants() {
        let actions = [
            GettingStartedAction::OpenMaps,
            GettingStartedAction::OpenSkirmishWizard,
            GettingStartedAction::OpenMapProperties,
            GettingStartedAction::OpenToolbox,
            GettingStartedAction::Toggle3DView,
            GettingStartedAction::OpenTextures,
            GettingStartedAction::OpenNavigation,
            GettingStartedAction::OpenPlacement,
            GettingStartedAction::OpenSpawns,
            GettingStartedAction::OpenScenario,
            GettingStartedAction::RunDesignCheck,
            GettingStartedAction::OpenMinimap,
        ];
        assert_eq!(actions.len(), 12);
    }
}
