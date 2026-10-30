//! 實機測試前檢核面板 (Ready to Test Checklist)
//!
//! 整合設計檢查 (Design Check) 錯誤／警告、勝負條件 (Victory/Defeat)、
//! 進攻波次 (Attack Waves) 敵對性與目的地、未儲存變更 (Unsaved Changes) 與
//! 安全儲存 (Safe Save) 狀態於單一檢核畫面；
//! 提供各子系統變更清單 (What changed since last save) 與一鍵複製遊戲內驗收指引 (Copy Test Instructions)。

use crate::app::EditorApp;
use crate::undo_journal::SubsystemId;
use against_rome_ambience::Language as AppLanguage;
use eframe::egui::{self, Color32, Grid, RichText, ScrollArea};
use std::path::Path;

fn tr(lang: AppLanguage, en: &'static str, zh: &'static str) -> &'static str {
    match lang {
        AppLanguage::English => en,
        AppLanguage::TraditionalChinese => zh,
    }
}

fn tl<'a>(english: bool, zh: &'a str, en: &'a str) -> &'a str {
    if english {
        en
    } else {
        zh
    }
}

/// 測試前檢核報告資料模型。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestReadinessReport {
    pub design_errors: usize,
    pub design_warnings: usize,
    pub has_design_report: bool,
    pub has_victory: bool,
    pub has_defeat: bool,
    pub total_waves: usize,
    pub waves_with_destination: usize,
    pub wave_destination_missing: usize,
    pub has_wave_not_hostile: bool,
    pub unsaved_changes: bool,
    pub save_blockers_count: usize,
    pub save_blocker_messages: Vec<String>,
    pub is_ready: bool,
    pub is_warning_only: bool,
}

/// 子系統變更狀態。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubsystemState {
    Clean,
    UnsavedChanges,
    PendingDraft,
}

/// 個別子系統變更項目。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubsystemChange {
    pub id: &'static str,
    pub name_en: &'static str,
    pub name_zh: &'static str,
    pub state: SubsystemState,
    pub journal_steps: usize,
    pub details_en: String,
    pub details_zh: String,
}

#[derive(Default)]
pub struct ReadyToTestUi {
    pub open: bool,
    pub cached_readiness: Option<TestReadinessReport>,
    pub last_copied_instructions: Option<String>,
    pub show_instructions_preview: bool,
    pub show_clean_subsystems: bool,
    cache_key: String,
    cached_preview: String,
    pub recompute_count: usize,
    /// 已載入地圖目錄的檔案指紋（相對路徑、長度、修改時間）；偵測外部檔案異動。
    map_fingerprint: String,
    map_fingerprint_at: Option<std::time::Instant>,
}

/// 指紋重新掃描的最短間隔，避免面板開啟時每幀走訪目錄。
const MAP_FINGERPRINT_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1000);

/// 唯讀掃描地圖目錄（最多 3 層、1024 項），回傳排序後的「路徑|長度|修改時間」指紋。
pub(crate) fn map_dir_fingerprint(dir: &Path) -> String {
    fn walk(root: &Path, dir: &Path, depth: usize, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            if out.len() >= 1024 {
                return;
            }
            let path = entry.path();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_dir() {
                if depth < 3 {
                    walk(root, &path, depth + 1, out);
                }
                continue;
            }
            let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().to_lowercase();
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            out.push(format!("{rel}|{}|{mtime}", meta.len()));
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, 0, &mut out);
    out.sort();
    out.join("\n")
}

impl ReadyToTestUi {
    /// 依節流間隔更新地圖目錄指紋；`force` 為真時立即重掃（手動重新整理）。
    fn refresh_map_fingerprint(&mut self, dir: Option<&Path>, force: bool) -> &str {
        let due = force
            || self
                .map_fingerprint_at
                .map_or(true, |t| t.elapsed() >= MAP_FINGERPRINT_INTERVAL);
        if due {
            self.map_fingerprint = dir.map(map_dir_fingerprint).unwrap_or_default();
            self.map_fingerprint_at = Some(std::time::Instant::now());
        }
        &self.map_fingerprint
    }
}

impl EditorApp {
    /// 評估目前地圖的實機測試就緒狀態（只讀取記憶體候選資料，不寫入檔案）。
    pub fn evaluate_test_readiness(&mut self) -> TestReadinessReport {
        let report = self.design_check_report();
        let design_errors = report.error_count();
        let design_warnings = report.warning_count();
        let (has_design_report, has_wave_not_hostile, wave_destination_missing) = match &report {
            crate::design_check_ui::DesignCheckReport::Checked { design_issues, .. } => (
                true,
                design_issues.iter().any(|i| i.code == "wave-not-hostile"),
                design_issues.iter().filter(|i| i.code == "wave-no-destination").count(),
            ),
            crate::design_check_ui::DesignCheckReport::Unavailable(_) => (false, false, 0),
        };
        self.autoload_scenario_if_needed();
        let balance = self.compute_balance_summary();
        let has_victory = balance.objectives.has_victory;
        let has_defeat = balance.objectives.has_defeat;
        let total_waves = balance.waves.len();
        let waves_with_destination = balance.waves_with_destination;

        let unsaved_changes = self.has_unsaved_changes();
        let blockers = self.save_blockers();
        let save_blockers_count = blockers.len();
        let lang = self.lang();
        let save_blocker_messages: Vec<String> = blockers
            .iter()
            .map(|b| match lang {
                AppLanguage::English => b.english.clone(),
                AppLanguage::TraditionalChinese => b.chinese.clone(),
            })
            .collect();

        let has_critical_blocker =
            design_errors > 0 || save_blockers_count > 0 || unsaved_changes || !has_design_report;
        let has_gameplay_warning =
            design_warnings > 0;

        let is_ready = !has_critical_blocker && !has_gameplay_warning;
        let is_warning_only = !has_critical_blocker && has_gameplay_warning;

        TestReadinessReport {
            design_errors,
            design_warnings,
            has_design_report,
            has_victory,
            has_defeat,
            total_waves,
            waves_with_destination,
            wave_destination_missing,
            has_wave_not_hostile,
            unsaved_changes,
            save_blockers_count,
            save_blocker_messages,
            is_ready,
            is_warning_only,
        }
    }

    /// 取得各子系統自上次儲存以來的變更摘要。
    pub fn subsystem_change_summary(&self) -> Vec<SubsystemChange> {
        let mut list = Vec::new();

        // 1. 地形 (Terrain)
        let terrain_dirty = self.terrain_is_dirty();
        let terrain_steps = self.undo_journal.subsystem_step_count(SubsystemId::Terrain);
        list.push(SubsystemChange {
            id: "terrain",
            name_en: "Terrain & Heights",
            name_zh: "地形高程與水域",
            state: if terrain_dirty {
                SubsystemState::UnsavedChanges
            } else {
                SubsystemState::Clean
            },
            journal_steps: terrain_steps,
            details_en: if terrain_dirty {
                format!("Heightmap or collision modified ({terrain_steps} undo history steps, including saved)")
            } else {
                "Unmodified".into()
            },
            details_zh: if terrain_dirty {
                format!("高度圖或通行圖已更動（{terrain_steps} 步可復原歷程，含已存檔）")
            } else {
                "未更動".into()
            },
        });

        // 2. 地表貼圖與材質 (Textures & Materials)
        let textures_dirty = self.textures.is_dirty();
        let texture_steps = self.undo_journal.subsystem_step_count(SubsystemId::Materials);
        list.push(SubsystemChange {
            id: "textures",
            name_en: "Materials & Textures",
            name_zh: "地表材質與貼圖",
            state: if textures_dirty {
                SubsystemState::UnsavedChanges
            } else {
                SubsystemState::Clean
            },
            journal_steps: texture_steps,
            details_en: if textures_dirty {
                format!("Surface textures modified ({texture_steps} undo history steps, including saved)")
            } else {
                "Unmodified".into()
            },
            details_zh: if textures_dirty {
                format!("地表材質貼圖已更動（{texture_steps} 步可復原歷程，含已存檔）")
            } else {
                "未更動".into()
            },
        });

        // 3. 物件與建築 (Placement & Objects)
        let placement_dirty = self.placement.is_dirty();
        let placement_steps = self.undo_journal.subsystem_step_count(SubsystemId::Placement);
        list.push(SubsystemChange {
            id: "placement",
            name_en: "Objects & Buildings",
            name_zh: "物件與建築放置",
            state: if placement_dirty {
                SubsystemState::UnsavedChanges
            } else {
                SubsystemState::Clean
            },
            journal_steps: placement_steps,
            details_en: if placement_dirty {
                format!("DATA world objects modified ({placement_steps} undo history steps, including saved)")
            } else {
                "Unmodified".into()
            },
            details_zh: if placement_dirty {
                format!("DATA 世界物件已更動（{placement_steps} 步可復原歷程，含已存檔）")
            } else {
                "未更動".into()
            },
        });

        // 4. 自然植被 (Nature & Flora)
        let nature_dirty = self.nature_is_dirty();
        let nature_steps = self.undo_journal.subsystem_step_count(SubsystemId::Nature);
        list.push(SubsystemChange {
            id: "nature",
            name_en: "Nature & Vegetation",
            name_zh: "自然植被與樹木",
            state: if nature_dirty {
                SubsystemState::UnsavedChanges
            } else {
                SubsystemState::Clean
            },
            journal_steps: nature_steps,
            details_en: if nature_dirty {
                format!("Flora density or tree placement modified ({nature_steps} undo history steps, including saved)")
            } else {
                "Unmodified".into()
            },
            details_zh: if nature_dirty {
                format!("植被密度或樹木已更動（{nature_steps} 步可復原歷程，含已存檔）")
            } else {
                "未更動".into()
            },
        });

        // 5. 劇本與生成點 (Scenario & Spawns)
        let scenario_dirty = self.scenario.dirty() || self.scenario.graph_ui().is_dirty();
        let scenario_steps = self.undo_journal.subsystem_step_count(SubsystemId::Scenario)
            + self.undo_journal.subsystem_step_count(SubsystemId::Spawns);
        let scenario_draft = self.scenario.graph_ui().is_dirty();
        list.push(SubsystemChange {
            id: "scenario",
            name_en: "Scenario & Spawns",
            name_zh: "劇本事件與生成點",
            state: if scenario_draft {
                SubsystemState::PendingDraft
            } else if scenario_dirty {
                SubsystemState::UnsavedChanges
            } else {
                SubsystemState::Clean
            },
            journal_steps: scenario_steps,
            details_en: if scenario_draft {
                "Scenario Event Graph has unapplied changes".into()
            } else if scenario_dirty {
                format!("Scripted events or unit spawns modified ({scenario_steps} undo history steps, including saved)")
            } else {
                "Unmodified".into()
            },
            details_zh: if scenario_draft {
                "劇本事件圖存在未套用草稿".into()
            } else if scenario_dirty {
                format!("腳本事件或部隊生成點已更動（{scenario_steps} 步可復原歷程，含已存檔）")
            } else {
                "未更動".into()
            },
        });

        // 6. 天候與音景氛圍 (Ambience & Weather)
        let ambience_dirty = self.ambience.is_dirty();
        list.push(SubsystemChange {
            id: "ambience",
            name_en: "Ambience & Weather",
            name_zh: "天候光影與音景氛圍",
            state: if ambience_dirty {
                SubsystemState::UnsavedChanges
            } else {
                SubsystemState::Clean
            },
            journal_steps: 0,
            details_en: if ambience_dirty {
                "Boden atmosphere, water level, or audio modified".into()
            } else {
                "Unmodified".into()
            },
            details_zh: if ambience_dirty {
                "大氣天候、地表水位或音效區域已更動".into()
            } else {
                "未更動".into()
            },
        });

        // 7. 場景光照 (Scene Lighting)
        let scene_dirty = self.scene.is_dirty();
        let scene_steps = self.undo_journal.subsystem_step_count(SubsystemId::Scene);
        list.push(SubsystemChange {
            id: "scene",
            name_en: "Scene Lighting & Sun",
            name_zh: "場景日照與光影",
            state: if scene_dirty {
                SubsystemState::UnsavedChanges
            } else {
                SubsystemState::Clean
            },
            journal_steps: scene_steps,
            details_en: if scene_dirty {
                format!("Sun lighting direction or color modified ({scene_steps} undo history steps, including saved)")
            } else {
                "Unmodified".into()
            },
            details_zh: if scene_dirty {
                format!("太陽方向或光照色彩已更動（{scene_steps} 步可復原歷程，含已存檔）")
            } else {
                "未更動".into()
            },
        });

        // 8. 地貌工程計畫 (Landscape Plans)
        let landscape_dirty = self.landscape.is_dirty();
        list.push(SubsystemChange {
            id: "landscape",
            name_en: "Landscape Engineering",
            name_zh: "地貌防禦工程計畫",
            state: if landscape_dirty {
                SubsystemState::PendingDraft
            } else {
                SubsystemState::Clean
            },
            journal_steps: 0,
            details_en: if landscape_dirty {
                "Pending cliff, river, road, wall, or erosion plan".into()
            } else {
                "Unmodified".into()
            },
            details_zh: if landscape_dirty {
                "存在未套用的懸崖、河流、道路、城牆或侵蝕計畫".into()
            } else {
                "未更動".into()
            },
        });

        // 9. 環境與聚落計畫 (Environment Plans)
        let env_dirty = self.environment.is_dirty();
        list.push(SubsystemChange {
            id: "environment",
            name_en: "Environment & Settlements",
            name_zh: "環境與聚落生成計畫",
            state: if env_dirty {
                SubsystemState::PendingDraft
            } else {
                SubsystemState::Clean
            },
            journal_steps: 0,
            details_en: if env_dirty {
                "Pending settlement distribution or flora scatter plan".into()
            } else {
                "Unmodified".into()
            },
            details_zh: if env_dirty {
                "存在未套用的聚落配置或植被散布計畫".into()
            } else {
                "未更動".into()
            },
        });

        // 10. 戰役與 AI 企劃草稿 (Campaign & AI Planner)
        let campaign_dirty = self.campaign.is_dirty();
        let ai_dirty = self.ai_planner.is_dirty();
        let event_graph_dirty = self.event_graph.is_dirty();
        let draft_active = campaign_dirty || ai_dirty || event_graph_dirty;
        list.push(SubsystemChange {
            id: "campaign_ai",
            name_en: "Campaign, AI & Graph Drafts",
            name_zh: "戰役、AI 企劃與節點圖草稿",
            state: if draft_active {
                SubsystemState::PendingDraft
            } else {
                SubsystemState::Clean
            },
            journal_steps: 0,
            details_en: if draft_active {
                "Pending draft in Campaign, AI Planner, or standalone EventGraph".into()
            } else {
                "Unmodified".into()
            },
            details_zh: if draft_active {
                "戰役企劃、AI 企劃或獨立事件圖存在未套用草稿".into()
            } else {
                "未更動".into()
            },
        });

        for (id, name_en, name_zh, dirty) in [
            ("maps", "Map Catalog", "地圖目錄", self.maps.is_dirty()),
            ("packaging", "Packaging", "封裝設定", self.packaging.is_dirty()),
            ("toolbox", "Toolbox", "工具箱", self.toolbox.is_dirty()),
        ] {
            list.push(SubsystemChange {
                id, name_en, name_zh,
                state: if dirty { SubsystemState::PendingDraft } else { SubsystemState::Clean },
                journal_steps: 0,
                details_en: if dirty { "Pending draft" } else { "Unmodified" }.into(),
                details_zh: if dirty { "尚未處理的草稿" } else { "未更動" }.into(),
            });
        }
        list
    }

    /// 統計目前存在未存檔變更或未套用草稿的子系統數量。
    pub fn count_modified_subsystems(&self) -> usize {
        self.subsystem_change_summary()
            .iter()
            .filter(|s| s.state != SubsystemState::Clean)
            .count()
    }

    /// 針對目前載入地圖動態產生 Markdown 格式的遊戲內驗收清單指引。
    pub fn generate_test_instructions(&mut self, lang: AppLanguage) -> String {
        let readiness = self.evaluate_test_readiness();
        let balance = self.compute_balance_summary();
        let doc = self.scenario.document();

        let map_name = self
            .loaded_map_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                let trimmed = self.map_path_input.trim();
                if trimmed.is_empty() {
                    "UNTITLED_MAP".to_string()
                } else {
                    Path::new(trimmed)
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "UNTITLED_MAP".to_string())
                }
            });

        let map_loaded = self.loaded_map_path.is_some();

        // 檢查 C：區域觸發條件
        let area_conditions: Vec<(String, i32, i32, i32, i32)> = doc
            .events
            .iter()
            .filter(|e| e.enabled)
            .flat_map(|e| {
                e.conditions
                    .iter()
                    .filter(|c| c.kind == 2)
                    .map(|c| (e.name.clone(), c.min_x, c.min_z, c.max_x, c.max_z))
            })
            .collect();

        match lang {
            AppLanguage::TraditionalChinese => {
                let mut out = String::new();
                out.push_str(&format!("# 遊戲內驗收清單：{map_name}\n\n"));
                out.push_str("> 本指引由羅馬帝國地圖編輯器根據當前地圖動態產生，供進入遊戲主程式進行實機驗收。\n\n");

                out.push_str("## 前置作業\n");
                if map_loaded {
                    out.push_str(&format!("- [ ] 編輯器載入的地圖：`{map_name}`；確認遊戲測試使用的自製地圖及其實際位置，測試前完整備份至 `%TEMP%`\n"));
                } else {
                    out.push_str("- [ ] 尚未載入地圖；請先載入自製測試地圖，確認實際位置並在測試前完整備份至 `%TEMP%`\n");
                }
                out.push_str("- [ ] 確認備份完成；切勿修改任何原版遊戲地圖\n");
                if readiness.unsaved_changes {
                    out.push_str("- [ ] ⚠ 注意：編輯器存在未儲存變更，測試前請務必執行 Safe Save (Ctrl+S)\n");
                } else {
                    out.push_str("- [ ] 測試前執行 Safe Save，並確認「設計檢查」無重大錯誤\n");
                }
                out.push('\n');

                out.push_str("## 檢查 A：進攻波次\n");
                if balance.waves.is_empty() {
                    out.push_str("（此地圖無敵方進攻波次設定，可略過此項檢查）\n");
                } else {
                    out.push_str(&format!(
                        "- [ ] 本地圖共規劃 {} 波敵方進攻，請確認波次時間與行軍路線：\n",
                        balance.waves.len()
                    ));
                    for (i, wave) in balance.waves.iter().enumerate() {
                        let dest_status = if wave.has_move_destination {
                            "已設定行軍目的地"
                        } else {
                            "⚠ 未設定目的地（可能定點發呆）"
                        };
                        out.push_str(&format!(
                            "  - [ ] 第 {} 波（{}）：時間 {}，隊伍 {}，兵力 {} 人，{}\n",
                            i + 1,
                            wave.event_name,
                            wave.time_formatted,
                            wave.team,
                            wave.units,
                            dest_status
                        ));
                    }
                    out.push_str("- [ ] 進入遊戲，確認各波敵軍於排定時間依序出現\n");
                    out.push_str("- [ ] 確認每波敵軍顯示紅色（敵對）血條、朝目標移動並發動攻擊\n");
                    out.push_str("- [ ] 記錄：通過／失敗、實際觀察時間、截圖路徑、問題描述\n");
                }
                out.push('\n');

                out.push_str("## 檢查 B：勝負條件\n- [ ] 勝利與失敗請分別重新開局測試\n");
                if readiness.has_victory {
                    out.push_str("- [ ] 勝利條件測試：滿足勝利目標時，任務正常結算並判定為「勝利」\n");
                } else {
                    out.push_str("- [ ] ⚠ 警告：本地圖尚未設定勝利條件 (Victory Action)。\n");
                }
                if readiness.has_defeat {
                    out.push_str("- [ ] 失敗條件測試：滿足失敗目標時（如主屋被毀），任務正常結算並判定為「失敗」\n");
                } else {
                    out.push_str("- [ ] ⚠ 警告：本地圖尚未設定失敗條件 (Defeat Action)。\n");
                }
                out.push_str("- [ ] 記錄：通過／失敗、觀察時間、截圖路徑\n\n");

                out.push_str("## 檢查 C：區域條件\n");
                if area_conditions.is_empty() {
                    out.push_str("（此地圖無區域觸發條件，可略過此項檢查）\n");
                } else {
                    out.push_str(&format!(
                        "- [ ] 本地圖具備 {} 處區域觸發條件：\n",
                        area_conditions.len()
                    ));
                    for (name, min_x, min_z, max_x, max_z) in &area_conditions {
                        out.push_str(&format!(
                            "  - [ ] 事件「{}」：單位進入區域 [{}, {} ~ {}, {}] 前動作未觸發；進入後正常觸發\n",
                            name, min_x, min_z, max_x, max_z
                        ));
                    }
                    out.push_str("- [ ] 記錄：通過／失敗、觀察時間、截圖路徑\n");
                }
                out.push('\n');

                out.push_str("## 檢查 D：存檔與開局驗收\n");
                out.push_str("- [ ] 於編輯器執行 Safe Save，關閉編輯器後重新載入地圖，確認所有設定完整保留\n");
                out.push_str("- [ ] 啟動遊戲主程式，於自製對戰地圖選單確認可見該地圖\n");
                out.push_str("- [ ] 點擊開始遊戲，確認順利進入地圖無崩潰、開局鏡頭與起始聚落正常\n");
                out.push_str("- [ ] 記錄：通過／失敗、觀察時間、截圖路徑\n\n");

                out.push_str("## 收尾\n");
                out.push_str("- [ ] 測試結束後，使用 `%TEMP%` 備份還原測試地圖\n\n");

                out.push_str("## 驗收回報格式\n\n");
                out.push_str("| 檢查項目 | 結果 | 備註（預期 vs 實際、時間、截圖） |\n");
                out.push_str("|:---|:---:|:---|\n");
                out.push_str("| A. 進攻波次 | [ ] 通過 / [ ] 失敗 | |\n");
                out.push_str("| B. 勝負條件 | [ ] 通過 / [ ] 失敗 | |\n");
                out.push_str("| C. 區域條件 | [ ] 通過 / [ ] 失敗 | |\n");
                out.push_str("| D. 存檔開局 | [ ] 通過 / [ ] 失敗 | |\n");

                out
            }
            AppLanguage::English => {
                let mut out = String::new();
                out.push_str(&format!("# In-Game Test Checklist: {map_name}\n\n"));
                out.push_str("> Dynamically generated by Against Rome Map Editor for in-game verification.\n\n");

                out.push_str("## Prerequisites\n");
                if map_loaded {
                    out.push_str(&format!("- [ ] Map loaded in editor: `{map_name}`; confirm the custom map and actual location used by the game, and back it up to `%TEMP%` before testing.\n"));
                } else {
                    out.push_str("- [ ] No map is loaded; load a custom test map, confirm its actual location, and back it up to `%TEMP%` before testing.\n");
                }
                out.push_str("- [ ] Confirm backup is complete; never modify original game maps.\n");
                if readiness.unsaved_changes {
                    out.push_str("- [ ] ⚠ Notice: editor has unsaved changes; please execute Safe Save (Ctrl+S) before testing.\n");
                } else {
                    out.push_str("- [ ] Before testing, execute Safe Save and confirm zero Design Check errors.\n");
                }
                out.push('\n');

                out.push_str("## Check A: Attack Waves\n");
                if balance.waves.is_empty() {
                    out.push_str("(No enemy attack waves configured on this map; skip this check)\n");
                } else {
                    out.push_str(&format!(
                        "- [ ] Total {} attack wave(s) configured. Confirm timing and route:\n",
                        balance.waves.len()
                    ));
                    for (i, wave) in balance.waves.iter().enumerate() {
                        let dest_status = if wave.has_move_destination {
                            "destination set"
                        } else {
                            "⚠ no destination set (units may idle)"
                        };
                        out.push_str(&format!(
                            "  - [ ] Wave {} ({}): time {}, team {}, units {}, {}\n",
                            i + 1,
                            wave.event_name,
                            wave.time_formatted,
                            wave.team,
                            wave.units,
                            dest_status
                        ));
                    }
                    out.push_str("- [ ] Launch game and verify each enemy wave spawns on schedule\n");
                    out.push_str("- [ ] Confirm enemy units show red health bars, march towards target, and engage in battle\n");
                    out.push_str("- [ ] Record: Pass / Fail, observed time, screenshot path, issue details\n");
                }
                out.push('\n');

                out.push_str("## Check B: Win/Loss Objectives\n- [ ] Test victory and defeat in separate fresh matches\n");
                if readiness.has_victory {
                    out.push_str("- [ ] Victory condition test: mission ends in Victory when objective is met\n");
                } else {
                    out.push_str("- [ ] ⚠ Warning: no Victory Action configured on this map; review the Scenario panel.\n");
                }
                if readiness.has_defeat {
                    out.push_str("- [ ] Defeat condition test: mission ends in Defeat when loss objective is met\n");
                } else {
                    out.push_str("- [ ] ⚠ Warning: no Defeat Action configured on this map; review the Scenario panel.\n");
                }
                out.push_str("- [ ] Record: Pass / Fail, observed time, screenshot path\n\n");

                out.push_str("## Check C: Area Conditions\n");
                if area_conditions.is_empty() {
                    out.push_str("(No area trigger conditions on this map; skip this check)\n");
                } else {
                    out.push_str(&format!(
                        "- [ ] Total {} area trigger condition(s) configured:\n",
                        area_conditions.len()
                    ));
                    for (name, min_x, min_z, max_x, max_z) in &area_conditions {
                        out.push_str(&format!(
                            "  - [ ] Event \"{}\": actions do not fire before entering [{}, {} ~ {}, {}]; fires properly upon entry\n",
                            name, min_x, min_z, max_x, max_z
                        ));
                    }
                    out.push_str("- [ ] Record: Pass / Fail, observed time, screenshot path\n");
                }
                out.push('\n');

                out.push_str("## Check D: Save & Initial Launch\n");
                out.push_str("- [ ] Execute Safe Save in editor, restart editor and reopen map, confirm all settings are preserved\n");
                out.push_str("- [ ] Launch game client, verify map appears in skirmish map selection\n");
                out.push_str("- [ ] Start match, confirm smooth launch without crash, camera and starting settlement are normal\n");
                out.push_str("- [ ] Record: Pass / Fail, observed time, screenshot path\n\n");

                out.push_str("## Cleanup\n");
                out.push_str("- [ ] After testing, restore map folder from `%TEMP%` backup\n\n");

                out.push_str("## Test Report Format\n\n");
                out.push_str("| Check Item | Result | Notes (Expected vs Actual, Time, Screenshot) |\n");
                out.push_str("|:---|:---:|:---|\n");
                out.push_str("| A. Attack Waves | [ ] Pass / [ ] Fail | |\n");
                out.push_str("| B. Objectives | [ ] Pass / [ ] Fail | |\n");
                out.push_str("| C. Area Conditions | [ ] Pass / [ ] Fail | |\n");
                out.push_str("| D. Save & Launch | [ ] Pass / [ ] Fail | |\n");

                out
            }
        }
    }

    /// 一鍵複製遊戲內驗收指引至剪貼簿。
    pub fn copy_test_instructions(&mut self, ctx: &egui::Context) {
        let lang = self.lang();
        let instructions = self.generate_test_instructions(lang);
        ctx.output_mut(|out| out.copied_text = instructions.clone());
        self.ready_to_test.last_copied_instructions = Some(instructions);
        self.status_message = tr(
            lang,
            "Copied in-game test instructions to clipboard.",
            "已將遊戲內驗收指引複製至剪貼簿。",
        )
        .into();
        self.status_is_error = false;
    }

    /// 渲染「實機測試前檢核面板 (Ready to Test Checklist)」。
    pub fn show_ready_to_test(&mut self, ctx: &egui::Context) {
        if !self.ready_to_test.open {
            return;
        }

        self.autoload_scenario_if_needed();
        let lang = self.lang();
        let is_en = lang == AppLanguage::English;

        let mut open = true;
        let mut trigger_safe_save = false;
        let mut trigger_open_design_check = false;
        let mut trigger_open_scenario = false;
        let mut trigger_fix_destinations = false;
        let mut trigger_fix_hostility = false;
        let mut trigger_copy_instructions = false;

        // 快取是檢查快照；磁碟外部異動或無歷程的草稿變更可手動重新整理。
        use crate::undo_journal::UndoTarget;
        let tokens: Vec<_> = [SubsystemId::Terrain, SubsystemId::Materials, SubsystemId::Placement,
            SubsystemId::Nature, SubsystemId::Spawns, SubsystemId::Scenario, SubsystemId::Scene,
            SubsystemId::Auxiliary].iter().map(|id| self.token(*id)).collect();
        let map_dir = self.loaded_map_path.clone();
        let fingerprint = self
            .ready_to_test
            .refresh_map_fingerprint(map_dir.as_deref(), false)
            .to_string();
        let key = format!("{:?}|{}|{:?}|{:?}|{:?}|{}|{}|{}", self.loaded_map_path,
            self.map_path_input, lang, tokens, self.subsystem_change_summary(),
            self.scenario.is_loaded(), self.placement.is_loaded(), fingerprint);
        if self.ready_to_test.cached_readiness.is_none() || self.ready_to_test.cache_key != key {
            let report = self.evaluate_test_readiness();
            self.ready_to_test.cached_readiness = Some(report);
            self.ready_to_test.cache_key = key;
            self.ready_to_test.cached_preview.clear();
            self.ready_to_test.recompute_count += 1;
        }
        let report = self.ready_to_test.cached_readiness.clone().expect("已建立檢查快取");
        let changes = self.subsystem_change_summary();
        let modified_count = changes
            .iter()
            .filter(|s| s.state != SubsystemState::Clean)
            .count();

        egui::Window::new(tr(
            lang,
            "Ready to Test Checklist",
            "實機測試前檢核清單",
        ))
        .open(&mut open)
        .default_width(640.0)
        .resizable(true)
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                if ui.button(tr(lang, "Refresh checks", "重新整理檢查")).clicked() {
                    self.ready_to_test.cached_readiness = None;
                    self.ready_to_test.map_fingerprint_at = None;
                }
                ui.label(tr(lang, "Checks auto-refresh when map files change on disk; refresh manually after unjournaled draft edits.", "地圖檔案於磁碟變動時會自動重新檢查；未記錄歷程的草稿異動請手動重新整理。"));
                // 1. 總體就緒評級狀態橫幅
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        if report.is_ready {
                            ui.label(
                                RichText::new(tl(
                                    is_en,
                                    "✔ 準備就緒：可以進行遊戲內測試",
                                    "✔ Ready for in-game testing",
                                ))
                                .color(Color32::from_rgb(46, 204, 113))
                                .strong(),
                            );
                        } else if report.is_warning_only {
                            ui.label(
                                RichText::new(tl(
                                    is_en,
                                    "⚠ 可測試（存在遊戲性注意事項）",
                                    "⚠ Ready with gameplay warnings",
                                ))
                                .color(Color32::from_rgb(241, 196, 15))
                                .strong(),
                            );
                        } else {
                            ui.label(
                                RichText::new(tl(
                                    is_en,
                                    "✖ 尚無法測試：存在未解決的阻擋項目",
                                    "✖ Action required before testing",
                                ))
                                .color(Color32::from_rgb(231, 76, 60))
                                .strong(),
                            );
                        }
                    });

                    let summary_desc = if report.is_ready {
                        tl(
                            is_en,
                            "設計檢查無錯誤或警告、無存檔阻擋且無未存檔變更。",
                            "Design Check has no errors or warnings; no save blockers or unsaved changes.",
                        )
                    } else if report.is_warning_only {
                        tl(
                            is_en,
                            "目前無阻擋項目；請檢視設計檢查警告。",
                            "No current blockers; review Design Check warnings.",
                        )
                    } else {
                        tl(
                            is_en,
                            "請先解決下方標示之存檔阻擋原因或設計檢查錯誤，方可安全進入遊戲實測。",
                            "Please resolve save blockers or design check errors below before running in-game tests.",
                        )
                    };
                    ui.label(summary_desc);
                });

                ui.add_space(6.0);

                // 2. 四大核心檢核項目
                ui.heading(tr(
                    lang,
                    "Pre-flight Sanity Checks",
                    "核心前置檢核項目",
                ));

                Grid::new("ready_to_test_grid")
                    .striped(true)
                    .num_columns(3)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        // A. 設計檢查 (Design Check)
                        ui.label(tr(lang, "1. Design Check", "1. 設計檢查"));
                        if report.design_errors > 0 {
                            ui.label(
                                RichText::new(match lang {
                                    AppLanguage::English => format!(
                                        "✖ {} Error(s), {} Warning(s)",
                                        report.design_errors, report.design_warnings
                                    ),
                                    AppLanguage::TraditionalChinese => format!(
                                        "✖ {} 項錯誤、{} 項警告",
                                        report.design_errors, report.design_warnings
                                    ),
                                })
                                .color(Color32::from_rgb(231, 76, 60)),
                            );
                        } else if report.design_warnings > 0 {
                            ui.label(
                                RichText::new(match lang {
                                    AppLanguage::English => format!(
                                        "⚠ 0 Errors, {} Warning(s)",
                                        report.design_warnings
                                    ),
                                    AppLanguage::TraditionalChinese => format!(
                                        "⚠ 0 項錯誤、{} 項警告",
                                        report.design_warnings
                                    ),
                                })
                                .color(Color32::from_rgb(241, 196, 15)),
                            );
                        } else if report.has_design_report {
                            ui.label(
                                RichText::new(tl(
                                    is_en,
                                    "✔ 無重大問題 (0 Errors)",
                                    "✔ Clean (0 Errors)",
                                ))
                                .color(Color32::from_rgb(46, 204, 113)),
                            );
                        } else {
                            ui.label(tl(
                                is_en,
                                "尚未執行檢查",
                                "Not evaluated yet",
                            ));
                        }
                        if ui
                            .button(tr(
                                lang,
                                "Open Design Check",
                                "開啟設計檢查",
                            ))
                            .clicked()
                        {
                            trigger_open_design_check = true;
                        }
                        ui.end_row();

                        // B. 勝負目標 (Objectives)
                        ui.label(tr(lang, "2. Objectives", "2. 勝負目標"));
                        let obj_text = if report.has_victory && report.has_defeat {
                            RichText::new(tl(
                                is_en,
                                "✔ 勝利與失敗條件齊全",
                                "✔ Both Victory and Defeat set",
                            ))
                            .color(Color32::from_rgb(46, 204, 113))
                        } else if report.has_victory {
                            RichText::new(tl(
                                is_en,
                                "⚠ 具勝利條件、缺少失敗條件",
                                "⚠ Victory set, Defeat missing",
                            ))
                            .color(Color32::from_rgb(241, 196, 15))
                        } else if report.has_defeat {
                            RichText::new(tl(
                                is_en,
                                "⚠ 具失敗條件、缺少勝利條件",
                                "⚠ Defeat set, Victory missing",
                            ))
                            .color(Color32::from_rgb(241, 196, 15))
                        } else {
                            RichText::new(tl(
                                is_en,
                                "⚠ 未設定勝負條件",
                                "⚠ Missing Victory and Defeat",
                            ))
                            .color(Color32::from_rgb(241, 196, 15))
                        };
                        ui.label(obj_text);
                        if ui
                            .button(tr(lang, "Open Scenario", "開啟劇本"))
                            .clicked()
                        {
                            trigger_open_scenario = true;
                        }
                        ui.end_row();

                        // C. 進攻波次 (Attack Waves)
                        ui.label(tr(lang, "3. Attack Waves", "3. 進攻波次"));
                        if report.total_waves == 0 {
                            ui.label(tl(
                                is_en,
                                "（無進攻波次）",
                                "(No attack waves)",
                            ));
                            ui.label("-");
                        } else if report.has_wave_not_hostile {
                            ui.label(
                                RichText::new(tl(
                                    is_en,
                                    "⚠ 波次所屬隊伍未設為敵對",
                                    "⚠ Wave teams not set hostile",
                                ))
                                .color(Color32::from_rgb(241, 196, 15)),
                            );
                            if ui
                                .button(tr(
                                    lang,
                                    "Make Teams Hostile",
                                    "將隊伍設為敵對",
                                ))
                                .clicked()
                            {
                                trigger_fix_hostility = true;
                            }
                        } else if report.wave_destination_missing > 0 {
                            ui.label(
                                RichText::new(match lang {
                                    AppLanguage::English => format!(
                                        "⚠ {} wave(s) missing destination",
                                        report.wave_destination_missing
                                    ),
                                    AppLanguage::TraditionalChinese => format!(
                                        "⚠ {} 個波次缺少行軍目的地",
                                        report.wave_destination_missing
                                    ),
                                })
                                .color(Color32::from_rgb(241, 196, 15)),
                            );
                            if ui
                                .button(tr(
                                    lang,
                                    "Send Waves to Base",
                                    "讓波次進攻主屋",
                                ))
                                .clicked()
                            {
                                trigger_fix_destinations = true;
                            }
                        } else {
                            ui.label(
                                RichText::new(match lang {
                                    AppLanguage::English => format!(
                                        "✔ {} wave(s) ready",
                                        report.total_waves
                                    ),
                                    AppLanguage::TraditionalChinese => format!(
                                        "✔ 共 {} 波設定完整",
                                        report.total_waves
                                    ),
                                })
                                .color(Color32::from_rgb(46, 204, 113)),
                            );
                            ui.label("-");
                        }
                        ui.end_row();

                        // D. 存檔與阻擋狀態 (Save Status)
                        ui.label(tr(lang, "4. Safe Save", "4. 安全儲存狀態"));
                        if report.save_blockers_count > 0 {
                            ui.label(
                                RichText::new(match lang {
                                    AppLanguage::English => format!(
                                        "✖ {} Save Blocker(s)",
                                        report.save_blockers_count
                                    ),
                                    AppLanguage::TraditionalChinese => format!(
                                        "✖ 存在 {} 項存檔阻擋",
                                        report.save_blockers_count
                                    ),
                                })
                                .color(Color32::from_rgb(231, 76, 60)),
                            );
                        } else if report.unsaved_changes {
                            ui.label(
                                RichText::new(tl(
                                    is_en,
                                    "⚠ 存在未儲存變更",
                                    "⚠ Unsaved changes pending",
                                ))
                                .color(Color32::from_rgb(241, 196, 15)),
                            );
                        } else {
                            ui.label(
                                RichText::new(tl(
                                    is_en,
                                    "✔ 已儲存至最新狀態",
                                    "✔ Saved & up to date",
                                ))
                                .color(Color32::from_rgb(46, 204, 113)),
                            );
                        }
                        if ui
                            .button(tr(
                                lang,
                                "Safe Save (Ctrl+S)",
                                "安全儲存 (Ctrl+S)",
                            ))
                            .clicked()
                        {
                            trigger_safe_save = true;
                        }
                        ui.end_row();
                    });

                // 若存在存檔阻擋，條列細節
                if !report.save_blocker_messages.is_empty() {
                    ui.add_space(4.0);
                    ui.group(|ui| {
                        ui.label(
                            RichText::new(tr(
                                lang,
                                "Active Save Blockers:",
                                "阻擋儲存的原因清單：",
                            ))
                            .color(Color32::from_rgb(231, 76, 60))
                            .strong(),
                        );
                        for msg in &report.save_blocker_messages {
                            ui.label(format!("• {msg}"));
                        }
                    });
                }

                ui.add_space(8.0);

                // 3. 自上次存檔以來的子系統變更清單 (What changed since last save)
                ui.heading(match lang {
                    AppLanguage::English => format!(
                        "Subsystem Changes Since Last Save ({modified_count} modified)"
                    ),
                    AppLanguage::TraditionalChinese => {
                        format!("自上次存檔以來的子系統變更（共 {modified_count} 個項目變動）")
                    }
                });

                ui.checkbox(&mut self.ready_to_test.show_clean_subsystems,
                    tr(lang, "Show unchanged subsystems", "顯示未更動子系統"));
                if modified_count == 0 && !self.ready_to_test.show_clean_subsystems {
                    ui.label(tl(
                        is_en,
                        "✔ 所有子系統皆已儲存，無未提交草稿或修改。",
                        "✔ All subsystems are saved; no pending drafts or uncommitted edits.",
                    ));
                } else {
                    Grid::new("ready_subsystem_changes_grid")
                        .striped(true)
                        .num_columns(4)
                        .spacing([10.0, 6.0])
                        .show(ui, |ui| {
                            ui.strong(tr(lang, "Subsystem", "子系統"));
                            ui.strong(tr(lang, "Status", "狀態"));
                            ui.strong(tr(lang, "Undo History (includes saved)", "可復原歷程（含已存檔）"));
                            ui.strong(tr(lang, "Details", "說明"));
                            ui.end_row();

                            for item in &changes {
                                if !self.ready_to_test.show_clean_subsystems && item.state == SubsystemState::Clean {
                                    continue;
                                }
                                let (status_text, status_color) = match item.state {
                                    SubsystemState::Clean => (
                                        tl(is_en, "未更動", "Clean"),
                                        Color32::from_rgb(46, 204, 113),
                                    ),
                                    SubsystemState::UnsavedChanges => (
                                        tl(is_en, "未存檔", "Modified"),
                                        Color32::from_rgb(241, 196, 15),
                                    ),
                                    SubsystemState::PendingDraft => (
                                        tl(is_en, "草稿待套用", "Pending Draft"),
                                        Color32::from_rgb(230, 126, 34),
                                    ),
                                };

                                ui.label(tl(is_en, item.name_zh, item.name_en));
                                ui.label(RichText::new(status_text).color(status_color));
                                ui.label(format!("{}", item.journal_steps));
                                ui.label(tl(
                                    is_en,
                                    &item.details_zh,
                                    &item.details_en,
                                ));
                                ui.end_row();
                            }
                        });
                }

                ui.add_space(8.0);

                // 4. 一鍵複製遊戲內驗收指引 (Copy Test Instructions)
                ui.heading(tr(
                    lang,
                    "In-Game Test Instructions",
                    "遊戲內實機驗收指引",
                ));
                ui.horizontal(|ui| {
                    if ui
                        .button(tr(
                            lang,
                            "📋 Copy In-Game Test Instructions",
                            "📋 複製遊戲內驗收指引至剪貼簿",
                        ))
                        .clicked()
                    {
                        trigger_copy_instructions = true;
                    }

                    ui.checkbox(
                        &mut self.ready_to_test.show_instructions_preview,
                        tr(lang, "Show Preview", "顯示指引預覽"),
                    );
                });

                if self.ready_to_test.show_instructions_preview {
                    ui.add_space(4.0);
                    if self.ready_to_test.cached_preview.is_empty() {
                        self.ready_to_test.cached_preview = self.generate_test_instructions(lang);
                    }
                    let preview_text = self.ready_to_test.cached_preview.clone();
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ScrollArea::vertical()
                            .max_height(200.0)
                            .show(ui, |ui| {
                                ui.monospace(&preview_text);
                            });
                    });
                }
            });
        });

        self.ready_to_test.open = open;

        // 處理跨面板操作請求
        if trigger_copy_instructions {
            self.copy_test_instructions(ctx);
        }
        if trigger_safe_save {
            self.request_safe_save();
        }
        if trigger_open_design_check {
            self.run_design_check();
            self.design_check.open = true;
        }
        if trigger_open_scenario {
            self.scenario.open = true;
            self.autoload_scenario_if_needed();
        }
        if trigger_fix_destinations {
            match self.fix_wave_destinations() {
                Ok(n) => {
                    self.status_message = match lang {
                        AppLanguage::English => {
                            format!("Set destinations for {n} wave action(s); Save to write them.")
                        }
                        AppLanguage::TraditionalChinese => {
                            format!("已設定 {n} 個波次動作的目的地；請存檔以寫入變更。")
                        }
                    };
                    self.status_is_error = false;
                }
                Err(e) => {
                    self.status_message = e;
                    self.status_is_error = true;
                }
            }
        }
        if trigger_fix_hostility {
            match self.fix_hostile_waves() {
                Ok(n) => {
                    self.status_message = match lang {
                        AppLanguage::English => {
                            format!("Added {n} hostile diplomacy event(s); Save to write them.")
                        }
                        AppLanguage::TraditionalChinese => {
                            format!("已新增 {n} 個敵對外交事件；請存檔以寫入變更。")
                        }
                    };
                    self.status_is_error = false;
                }
                Err(e) => {
                    self.status_message = e;
                    self.status_is_error = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ready_to_test_default_app_evaluation() {
        let mut app = EditorApp::default();
        let report = app.evaluate_test_readiness();
        // 預設空白 app 未載入地圖，可能缺少勝負條件與存在存檔阻擋
        assert!(!report.is_ready);
        assert_eq!(report.total_waves, 0);

        let changes = app.subsystem_change_summary();
        assert_eq!(changes.len(), 13);
        // 預設乾淨狀態，所有子系統皆為 Clean
        assert_eq!(app.count_modified_subsystems(), 0);
    }

    #[test]
    fn test_ready_to_test_dirty_subsystem_tracking() {
        let mut app = EditorApp::default();
        // 模擬戰役草稿與天候更動
        app.campaign.mark_dirty();
        app.ambience.mark_dirty();

        let changes = app.subsystem_change_summary();
        let campaign_change = changes.iter().find(|c| c.id == "campaign_ai").unwrap();
        assert_eq!(campaign_change.state, SubsystemState::PendingDraft);

        let ambience_change = changes.iter().find(|c| c.id == "ambience").unwrap();
        assert_eq!(ambience_change.state, SubsystemState::UnsavedChanges);

        assert_eq!(app.count_modified_subsystems(), 2);
    }

    #[test]
    fn test_generate_test_instructions_bilingual_and_clipboard() {
        let mut app = EditorApp::default();
        let ctx = egui::Context::default();

        let zh_instructions = app.generate_test_instructions(AppLanguage::TraditionalChinese);
        assert!(zh_instructions.contains("# 遊戲內驗收清單"));
        assert!(zh_instructions.contains("前置作業"));
        assert!(zh_instructions.contains("檢查 A：進攻波次"));
        assert!(zh_instructions.contains("檢查 B：勝負條件"));
        assert!(zh_instructions.contains("驗收回報格式"));

        let en_instructions = app.generate_test_instructions(AppLanguage::English);
        assert!(en_instructions.contains("# In-Game Test Checklist"));
        assert!(en_instructions.contains("Prerequisites"));
        assert!(en_instructions.contains("Check A: Attack Waves"));
        assert!(en_instructions.contains("Check B: Win/Loss Objectives"));
        assert!(en_instructions.contains("Test Report Format"));

        // 測試剪貼簿寫入
        app.copy_test_instructions(&ctx);
        assert!(app.ready_to_test.last_copied_instructions.is_some());
        let copied = ctx.output(|o| o.copied_text.clone());
        assert_eq!(copied, app.ready_to_test.last_copied_instructions.unwrap());
    }

    #[test]
    fn 驗收指引區分載入狀態且不猜測遊戲目錄() {
        let mut app = EditorApp::default();
        app.map_path_input = "尚未載入的地圖".into();
        for lang in [AppLanguage::TraditionalChinese, AppLanguage::English] {
            let text = app.generate_test_instructions(lang);
            assert!(text.contains(if lang == AppLanguage::English { "No map is loaded" } else { "尚未載入地圖" }));
            assert!(!text.contains("MAPS/尚未載入的地圖"));
        }
        app.loaded_map_path = Some(std::path::PathBuf::from(r"D:\工作副本\ENDL_005"));
        for lang in [AppLanguage::TraditionalChinese, AppLanguage::English] {
            let text = app.generate_test_instructions(lang);
            assert!(text.contains("ENDL_005"));
            assert!(!text.contains("工作副本"));
            assert!(!text.contains("MAPS/ENDL_005"));
        }
    }
    #[test]
    fn test_ready_to_test_window_render_without_crash() {
        let mut app = EditorApp::default();
        app.ready_to_test.open = true;
        let ctx = egui::Context::default();

        let _ = ctx.run(Default::default(), |ctx| {
            app.show_ready_to_test(ctx);
        });

        assert!(app.ready_to_test.open);
        assert!(app.ready_to_test.cached_readiness.is_some());
    }

    #[test]
    fn map_dir_fingerprint_detects_external_file_changes() {
        let dir = std::env::temp_dir().join(format!("arm_rtt_fp_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.dat"), b"one").unwrap();
        let first = map_dir_fingerprint(&dir);
        assert_eq!(first, map_dir_fingerprint(&dir));
        std::fs::write(dir.join("sub").join("b.dat"), b"x").unwrap();
        let second = map_dir_fingerprint(&dir);
        assert_ne!(first, second);
        std::fs::write(dir.join("a.dat"), b"longer").unwrap();
        assert_ne!(second, map_dir_fingerprint(&dir));
        assert_eq!(map_dir_fingerprint(&dir.join("missing")), "");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ready_to_test_recomputes_after_external_map_file_change() {
        let dir = std::env::temp_dir().join(format!("arm_rtt_recompute_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.dat"), b"one").unwrap();
        let mut app = EditorApp::default();
        app.loaded_map_path = Some(dir.clone());
        app.ready_to_test.open = true;
        let ctx = egui::Context::default();
        let run = |app: &mut EditorApp| {
            let _ = ctx.run(Default::default(), |ctx| app.show_ready_to_test(ctx));
        };
        run(&mut app);
        let base = app.ready_to_test.recompute_count;
        // 節流間隔內無異動：不重算。
        run(&mut app);
        assert_eq!(app.ready_to_test.recompute_count, base);
        // 外部異動但仍在節流間隔內：維持快照。
        std::fs::write(dir.join("a.dat"), b"changed!").unwrap();
        run(&mut app);
        assert_eq!(app.ready_to_test.recompute_count, base);
        // 間隔到期後偵測外部異動並重算。
        app.ready_to_test.map_fingerprint_at = None;
        run(&mut app);
        assert_eq!(app.ready_to_test.recompute_count, base + 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
