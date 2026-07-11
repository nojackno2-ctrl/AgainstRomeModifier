using System;
using System.Collections.Generic;

namespace AgainstRomeModifier {
    public enum Language {
        TraditionalChinese,
        English
    }

    public static class Loc {
        public static Language CurrentLanguage { get; set; } = Language.TraditionalChinese;

        public static string Get(string key) {
            if (CurrentLanguage == Language.English) {
                return En.TryGetValue(key, out var valEn) ? valEn : key;
            }
            return Zh.TryGetValue(key, out var valZh) ? valZh : key;
        }

        public static string GetUnitName(string key) {
            if (CurrentLanguage == Language.English) {
                return EnUnitNames.TryGetValue(key, out var valEn) ? valEn : (TroopConfig.UnitNames.TryGetValue(key, out var zhVal) ? zhVal : key);
            }
            return TroopConfig.UnitNames.TryGetValue(key, out var valZh) ? valZh : key;
        }

        public static string GetUnitType(string utype) {
            if (CurrentLanguage == Language.English) {
                switch (utype) {
                    case "melee_inf": return "Melee Infantry";
                    case "ranged_inf": return "Ranged Infantry";
                    case "ranged_cav": return "Ranged Cavalry";
                    case "hybrid_inf": return "Hybrid Infantry";
                    case "cav": return "Cavalry";
                    case "leader_melee": return "Leader (Melee)";
                    case "leader_cav": return "Leader (Cavalry)";
                    case "priest": return "Priest";
                    case "siege": return "Siege Weapon";
                    default: return "Unknown";
                }
            }
            switch (utype) {
                case "melee_inf": return "近戰步兵";
                case "ranged_inf":
                case "ranged_cav": return "遠程部隊";
                case "hybrid_inf": return "混合步兵";
                case "cav": return "騎兵部隊";
                case "leader_melee":
                case "leader_cav": return "領袖";
                case "priest": return "祭司";
                case "siege": return "攻城武器";
                default: return "未知";
            }
        }

        public static string GetStyleText(string style) {
            if (CurrentLanguage == Language.English) {
                switch (style) {
                    case "shield": return "Shielded";
                    case "two_handed": return "Two-Handed";
                    case "dual_wield": return "Dual-Wield";
                    case "ranged": return "Ranged";
                    default: return "None";
                }
            }
            switch (style) {
                case "shield": return "持盾";
                case "two_handed": return "雙手武器";
                case "dual_wield": return "雙持武器";
                case "ranged": return "遠程";
                default: return "無";
            }
        }

        public static string GetTierText(string tier) {
            if (CurrentLanguage == Language.English) {
                switch (tier) {
                    case "low": return "Low Tier";
                    case "mid": return "Mid Tier";
                    case "high": return "High Tier";
                    case "ace": return "Ace";
                    case "leader": return "Leader";
                    case "siege": return "Siege";
                    default: return "Unknown";
                }
            }
            switch (tier) {
                case "low": return "低階";
                case "mid": return "中階";
                case "high": return "高階";
                case "ace": return "王牌";
                case "leader": return "領袖";
                case "siege": return "攻城武器";
                default: return "未知";
            }
        }

        public static string GetFactionName(string faction) {
            if (CurrentLanguage == Language.English) {
                return faction;
            }
            switch (faction) {
                case "Roman": return "羅馬";
                case "Teuton": return "條頓";
                case "Celt": return "塞爾特";
                case "Hun": return "匈奴";
                default: return faction;
            }
        }

        private static readonly Dictionary<string, string> EnUnitNames = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase) {
            {"FigRomAnf00_Anfuehrer", "Roman Leader"},
            {"FigRomInf00_Lanze_Schild", "Roman Light Infantry"},
            {"FigRomInf01_Schwert_Schild", "Roman Sword & Shieldman"},
            {"FigRomKav00_Schwert_Schild", "Roman Assault Cavalry"},
            {"FigRomSch00_Speer_Schild", "Roman Heavy Infantry"},
            {"FigRomSch01_Bogen", "Roman Archer"},
            {"FigGerAnf00_Anfuehrer", "Teuton Leader"},
            {"FigGerInf00_Hammer_Schild", "Teuton Mace & Shieldman"},
            {"FigGerInf01_Schwert", "Teuton Swordsman"},
            {"FigGerInf02_Zweihandaxt", "Teuton Two-Handed Axeman"},
            {"FigGerInf03_Doppelhammer", "Teuton Dual Hammerer"},
            {"FigGerKav00_Schwert_Schild", "Teuton Cavalry"},
            {"FigGerPri00_Priester", "Teuton Priest"},
            {"FigGerSch00_Speer", "Teuton Spearman"},
            {"FigGerSch01_Axt_Schild", "Teuton Ax & Shieldman"},
            {"FigKelAnf00_Anfuehrer", "Celt Leader"},
            {"FigKelInf00_Schwert", "Celt Swordsman"},
            {"FigKelInf01_Lanze", "Celt Spearman"},
            {"FigKelInf02_Doppelschwert", "Celt Heavy Infantry"},
            {"FigKelKav00_Lanze_Schild", "Celt Lancer"},
            {"FigKelPri00_Priester", "Celt Priest"},
            {"FigKelSch00_Bogen", "Celt Archer"},
            {"FigKelSch01_Schleuder", "Celt Slinger"},
            {"FigKelSch02_Schwere_Schleuder", "Celt Heavy Slinger"},
            {"FigHunAnf00_Anfuehrer", "Hun Leader"},
            {"FigHunInf00_Keule", "Hun Clubman"},
            {"FigHunInf01_Schwert_Schild", "Hun Sword & Shieldman"},
            {"FigHunKav00_Schwert_Schild", "Hun Light Cavalry"},
            {"FigHunKav01_Bogen", "Hun Archer Cavalry"},
            {"FigHunKav02_Lanze_Schild", "Hun Heavy Cavalry"},
            {"FigHunKav03_Geisterreiter", "Hun Ghost Warrior"},
            {"FigHunPri00_Priester", "Hun Priest"},
            {"FigHunSch00_Bogen", "Hun Archer"},
            {"FigGerArt00_Katapult", "Teuton Catapult"},
            {"FigGerArt00_Katapult_Aufbau", "Teuton Deployed Catapult"},
            {"FigRomArt00_Speerschleuder", "Roman Ballista"},
            {"FigRomArt00_Speerschleuder_Auf", "Roman Deployed Ballista"},
            {"FigRomArt01_Katapult", "Roman Catapult"},
            {"FigRomArt01_Katapult_Aufbau", "Roman Deployed Catapult"},
            {"FigKelArt00_Speerschleuder", "Celt Ballista"},
            {"FigKelArt00_Speerschleuder_A", "Celt Deployed Ballista"},
            {"FigKelArt01_Katapult", "Celt Catapult"},
            {"FigKelArt01_Katapult_Aufbau", "Celt Deployed Catapult"}
        };

        private static readonly Dictionary<string, string> Zh = new Dictionary<string, string> {
            // UI elements
            { "NavSystem", "⚙   主控制台" },
            { "NavDefaultStats", "📊   自訂兵種屬性" },
            { "NavCurrentStats", "📈   當前兵種數值" },
            { "NavSaveManager", "💾   遊戲存檔管理" },
            { "NavDoc", "📝   修改技術文件" },
            { "MainTitle", "AGAINST ROME MODIFIER PRO" },
            { "NumericTitle", "數值偏好與系統設定" },
            { "PopLimit", "最大人口上限:" },
            { "CiviSpeed", "村民生產速度倍率:" },
            { "FocusLoss", "遊戲視窗失焦時不自動暫停 (背景執行)" },
            { "ToEng", "強制英文語系 (介面圖示與核心文字)" },
            { "PresetSave", "匯出設定" },
            { "PresetLoad", "匯入設定" },
            { "SwitchesTitle", "核心修改開關設定" },
            { "FreeProd", "建造、修復與所有單位生產完全免費" },
            { "FreeUpgrade", "陣型、研發、屬性解鎖升級免費" },
            { "NoSpellCost", "祭司與賢者法術無消耗 (MP 零消耗)" },
            { "InfiniteMorale", "部隊無限士氣 (士氣不減且極速恢復)" },
            { "AiUltimateMode", "AI終極模式（無盡軍隊最高、重生 5 秒、迴圈 5~10 秒、解除同時上場限制）" },
            { "VillageBuildRange", "村莊建造範圍候選補丁（已停用）" },
            { "TipsTitle", "修改器使用指引與操作指南" },
            { "ConsoleTitle", "系統控制台與操作" },
            { "GamePath", "遊戲路徑:" },
            { "Browse", "瀏覽..." },
            { "LoadCurrent", "讀取現有設定" },
            { "Restore", "恢復原版" },
            { "Apply", "執行修改" },
            { "StartGame", "啟動遊戲" },
            { "RestoreAll", "全部還原" },
            { "RestoreStats", "僅還原兵種屬性" },
            { "RestoreCompat", "僅還原相容性修正" },
            { "RestoreLang", "僅還原語系設定" },
            { "DefaultStatsTitle", "自訂兵種屬性對比 (無自訂加成)" },
            { "EnableBalance", "啟用自訂兵種屬性平衡與陣營特色" },
            { "TroopTemplateLabel", "選擇範本:" },
            { "TroopTemplateSelect", "(請選擇範本)" },
            { "TroopTemplateBalanced", "修改器內建平衡" },
            { "BtnTroopPreset", "修改兵種檔案" },
            { "TroopPresetDefault", "屬性檔案：預設範本" },
            { "TroopPresetManual", "屬性檔案：自訂配置 (手動)" },
            { "TroopPresetFile", "屬性檔案：{0}" },
            { "TroopPresetLoaded", "屬性檔案：設定檔載入 ({0})" },
            { "CurrentStatsTitle", "當前兵種數值 (原版與當前對比)" },
            { "GameSavesTitle", "遊戲中存檔列表" },
            { "BackupsTitle", "備份歷史列表" },
            { "DetailTitle", "存檔詳細與預覽" },
            { "BackupSave", "備份此存檔" },
            { "DeleteSave", "刪除此存檔" },
            { "Refresh", "重新整理" },
            { "RestoreBackup", "還原此備份" },
            { "DeleteBackup", "刪除此備份" },
            { "LanguageLabel", "🌏   修改器語系 / Language" },
            { "LangZhButton", "繁體中文" },
            { "LangEnButton", "English" },

            // Headers Default Grid
            { "HeaderName", "兵種名稱" },
            { "HeaderIcon", "圖示" },
            { "HeaderType", "部隊類型" },
            { "HeaderStyle", "裝備分類" },
            { "HeaderHp", "生命值" },
            { "HeaderMeleeDmg", "近戰傷害" },
            { "HeaderRangedDmg", "遠程傷害" },
            { "HeaderMeleeRelt", "近戰冷卻" },
            { "HeaderRangedRelt", "遠程冷卻" },
            { "HeaderVw", "防禦力" },
            { "HeaderAw", "戰鬥力" },
            { "HeaderSpeed", "移動速度" },
            { "HeaderSight", "視野" },
            { "HeaderRange", "射程/技能距離" },
            { "HeaderSpellRadius", "法術半徑" },
            { "HeaderTier", "階級" },

            // Headers Current Grid
            { "HeaderHpComp", "生命值對比" },
            { "HeaderMeleeDmgComp", "近戰傷害對比" },
            { "HeaderRangedDmgComp", "遠程傷害對比" },
            { "HeaderMeleeReltComp", "近戰冷卻對比" },
            { "HeaderRangedReltComp", "遠程冷卻對比" },
            { "HeaderVwComp", "防禦對比" },
            { "HeaderAwComp", "戰鬥對比" },
            { "HeaderSpeedComp", "移動速度對比" },
            { "HeaderSightComp", "視野對比" },
            { "HeaderRangeComp", "射程對比" },
            { "HeaderSpellRadiusComp", "法術半徑對比" },

            // Save grid specific headers
            { "HeaderFolder", "資料夾" },
            { "HeaderSaveTitle", "存檔標題" },
            { "HeaderLevel", "原版關卡" },
            { "HeaderTime", "存檔時間" },
            { "HeaderBackupFile", "備份檔名" },
            { "HeaderBackupTime", "備份時間" },
            { "HeaderOrigFolder", "原資料夾" },

            // Tabs
            { "TabRoman", " 羅馬 " },
            { "TabTeuton", " 條頓 " },
            { "TabCelt", " 塞爾特 " },
            { "TabHun", " 匈奴 " },

            // Dialog / Log / Message titles
            { "TitlePathError", "路徑錯誤" },
            { "TitleConfirm", "確認執行" },
            { "TitleSuccess", "成功" },
            { "TitleError", "錯誤" },
            { "TitleWarning", "確認覆蓋" },
            { "TitleConfirmDelete", "確認刪除" },
            { "TitleTips", "提示" },

            // Dialog / Log / Message strings
            { "LogConstructCompleted", "修改器視窗建構完成，開始載入資料..." },
            { "LogLoadIconFailed", "載入圖示失敗: " },
            { "LogLoadIconCount", "成功載入 {0} 個兵種圖示。" },
            { "LogDefaultStatsLoaded", "自訂兵種屬性載入完成，共 {0} 筆資料。" },
            { "LogDefaultStatsLoadError", "載入自訂屬性數據錯誤: " },
            { "LogBalanceToggled", "自訂兵種屬性平衡與陣營特色已{0}。" },
            { "LogReadCurrent", "正在讀取現有設定..." },
            { "LogReadCurrentDone", "現有屬性設定讀取完成，共 {0} 筆比對資料。" },
            { "LogBrowseTitle", "請選擇《羅馬的榮耀》(Against Rome) 遊戲安裝目錄" },
            { "MsgSelectGameDir", "請先設定正確的遊戲路徑。" },
            { "MsgWrongGameDir", "設定的目錄不包含遊戲主程式 Against_Rome.exe，請重新選擇正確的安裝路徑。" },
            { "MsgConfirmApply", "確定要套用所有修改嗎？\n此操作將覆蓋遊戲檔案。" },
            { "MsgApplySuccess", "修改成功套用！" },
            { "MsgApplyFailed", "修改失敗: " },
            { "MsgRestoreAllSuccess", "已成功恢復全部原版設定！" },
            { "MsgRestoreStatsSuccess", "已成功恢復兵種屬性設定！" },
            { "MsgRestoreCompatSuccess", "已成功恢復相容性設定！" },
            { "MsgRestoreLangSuccess", "已成功恢復語言設定！" },
            { "MsgRestoreFailed", "還原失敗: " },
            { "MsgExeNotFound", "在遊戲目錄中找不到 Against_Rome.exe。" },
            { "MsgLaunchFailed", "啟動遊戲失敗: " },
            { "LogStartApply", "開始套用修改..." },
            { "LogApplyAllSuccess", "所有修改已成功套用！" },
            { "LogStartRestoreAll", "開始恢復全部原版設定..." },
            { "LogRestoreAllDone", "已成功恢復全部原版設定。" },
            { "LogStartRestoreStats", "開始恢復兵種屬性原版設定..." },
            { "LogRestoreStatsDone", "已成功恢復兵種屬性設定。" },
            { "LogStartRestoreCompat", "開始恢復相容性原版設定..." },
            { "LogRestoreCompatDone", "已成功恢復相容性設定。" },
            { "LogStartRestoreLang", "開始恢復語言原版設定..." },
            { "LogRestoreLangDone", "已成功恢復語言設定。" },
            { "LogRestored", "已還原: {0}" },
            { "LogGameStarted", "遊戲已成功啟動。" },
            { "LogLangToEng", "已套用英文介面與地圖語言包。" },
            { "LogLangToOrig", "已恢復原版介面與地圖語言包。" },
            { "LogExePatchFocus", "已套用 Against_Rome.exe 視窗失焦不暫停修正。" },
            { "LogExePatchWarning", "[警告] Against_Rome.exe 版本或特徵碼不符合預期，跳過失焦暫停修正。" },
            { "LogExePatchOrig", "已套用 Against_Rome.exe 原版。" },
            { "LogVillageBuildRangeApplied", "村莊建造範圍候選 EXE patch 已停用，未寫入 Against_Rome.exe。" },
            { "LogVillageBuildRangeRestored", "已還原舊版村莊建造範圍候選 EXE patch 為原版 bytes。" },
            { "LogVillageBuildRangeWarning", "[警告] Against_Rome.exe 村莊建造範圍特徵碼不符合預期，無法判斷目前狀態。" },
            { "LogClScriptPatch", "已成功修改與寫入 cl_script.ini (法術半徑與村民生產速度)。" },
            { "LogRessPatch", "已成功修改與寫入 ress.ini (免費建造、生產、升級與法術消耗)。" },
            { "LogObjdefPatch", "已成功修改與寫入 objdef.dau (兵種屬性平衡與速度)。" },
            { "LogAptPatch", "已成功修改與寫入 apt.dat 等地圖檔案。" },
            { "LogTeamDatPatch", "已成功修改與寫入地圖 team.dat 人口設定。" },
            { "LogEndlessAiNoMaps", "找不到 MAPS 資料夾，略過 AI終極模式。" },
            { "LogEndlessAiNoScripts", "找不到無盡模式 ak_level.bci，略過 AI終極模式。" },
            { "LogEndlessAiPatternMissing", "[警告] 找不到無盡軍隊數量 bytecode 特徵，略過: {0}" },
            { "LogEndlessAiUltimateApplied", "AI終極模式已套用：{0} 個無盡腳本的軍隊數量改為 {1}，重生等待改為 5 秒，行動迴圈等待改為 5~10 秒，並解除同時上場限制。" },
            { "LogEndlessAiUltimateRestored", "AI終極模式已還原：{0} 個無盡腳本的軍隊數量改回 {1}，重生等待改回 180 秒，行動迴圈等待與同時上場限制改回原始值。" },
            { "LogNoToEngDir", "找不到遊戲目錄下的 ToEng 資料夾，無法切換語系。" },
            { "LogNoZipResource", "找不到內嵌的 Backup.zip 資源。" },
            { "LogGamePathNotSetIcon", "遊戲路徑未設定或不存在，無法載入兵種圖示。" },
            { "LogGuiDatNotFound", "找不到 gui.dat，無法載入兵種圖示。" },
            { "LogIconIniNotFound", "記憶體備份中找不到 icon.ini，無法載入兵種圖示。" },
            { "LogObjdefNotFound", "記憶體備份中找不到 objdef.dau，無法載入自訂兵種屬性。" },
            { "LogNoObjdefForRead", "找不到任何 objdef.dau 檔案，無法讀取設定。" },
            { "LogSavePathNotSet", "遊戲路徑未設定，無法載入存檔。" },
            { "LogRefreshSavesFailed", "重新整理存檔列表失敗: " },
            { "SaveDetailGameSave", "存檔類型: 遊戲存檔\n\n資料夾: {0}\n\n存檔標題: {1}\n\n原版關卡: {2}\n\n存檔時間: {3}" },
            { "SaveDetailBackup", "存檔類型: 備份檔案\n\n備份檔名: {0}\n\n原資料夾: {1}\n\n存檔標題: {2}\n\n原版關卡: {3}\n\n備份時間: {4}" },
            { "MsgSelectBackup", "請先選擇要還原的備份。" },
            { "MsgCannotResolveOrigFolder", "無法判斷該備份的原資料夾，無法還原。" },
            { "MsgGamePathNotSet", "遊戲路徑未設定，無法還原。" },
            { "MsgConfirmOverwriteSave", "目標存檔資料夾 [{0}] 已存在，是否覆蓋？" },
            { "MsgRestoreBackupSuccess", "還原備份成功！" },
            { "MsgRestoreBackupFailed", "還原備份失敗: " },
            { "MsgSelectSaveToDelete", "請先選擇要刪除的存檔。" },
            { "MsgInvalidSaveDir", "無效的存檔目錄，操作已取消。" },
            { "MsgConfirmDeleteSave", "確定要永久刪除遊戲存檔 [{0}] 嗎？此操作不可還原！" },
            { "MsgDeleteSaveSuccess", "已刪除存檔！" },
            { "MsgDeleteSaveFailed", "刪除存檔失敗: " },
            { "MsgSelectBackupToDelete", "請先選擇要刪除的備份。" },
            { "MsgConfirmDeleteBackup", "確定要永久刪除備份檔案 [{0}] 嗎？" },
            { "MsgDeleteBackupSuccess", "已刪除備份！" },
            { "MsgDeleteBackupFailed", "刪除備份失敗: " },
            { "MsgSelectSaveToBackup", "請先選擇要備份的存檔。" },
            { "MsgNoOrigFolderToBackup", "找不到該存檔的原始資料夾。" },
            { "MsgBackupSaveSuccess", "備份存檔成功！" },
            { "MsgBackupSaveFailed", "備份存檔失敗: " },
            { "LogPresetExportDone", "設定已成功匯出至: {0}" },
            { "LogPresetExportError", "匯出設定失敗: " },
            { "LogPresetImportDone", "設定已成功匯入。" },
            { "LogPresetImportError", "匯入設定失敗: " },
            { "LogPresetImportFormatError", "設定檔格式錯誤。" },
            { "LogPresetImportErrorField", "解析設定欄位 {0}={1} 失敗: {2}" },
            { "MsgPresetExportSuccess", "設定匯出成功！" },
            { "MsgPresetImportSuccess", "設定匯入成功！" },
            { "MsgPresetLoadNotFound", "找不到設定檔。" },
            { "LogLoadTechDocFailed", "載入技術文件資源失敗: " },
            { "LogParseObjdefBackupFailed", "解析備份 objdef.dau 失敗: " },
            { "LogFreeUpgradeAdded", "補齊 ress.ini 缺失的升級建築..." },
            { "Unparsable", "無法解析" },
            { "Unknown", "未知" },
            { "LogBackupSaveSuccessDetail", "備份存檔成功: {0} -> {1}" },
            { "LogBackupSaveFailedDetail", "備份存檔失敗: " },
            { "LogRestoreBackupSuccessDetail", "還原備份成功: {0} -> {1}" },
            { "LogRestoreBackupFailedDetail", "還原備份失敗: " },
            { "LogDeleteSaveSuccessDetail", "已刪除遊戲存檔: {0}" },
            { "LogDeleteSaveFailedDetail", "刪除存檔失敗: " },
            { "LogDeleteBackupSuccessDetail", "已刪除備份檔案: {0}" },
            { "LogDeleteBackupFailedDetail", "刪除備份失敗: " },
            { "TipsContent", "💡 快速操作指南：\n\n1. 設定遊戲路徑：請在右側「系統控制台」指定 Against Rome 安裝目錄（修改器會自動嘗試讀取註冊表以取得路徑）。\n\n2. 讀取現有設定：點擊右側「讀取現有設定」，修改器會自動從遊戲實體檔案（objdef.dau, ress.ini, cl_script.ini 等）解析目前套用的參數，並呈現於兵種列表對比中。\n\n3. 調整偏好與開關：在主控制台完成您喜好的修改配置（如人口上限、倍率開關等）。\n\n4. 執行修改與啟動：點擊右側「執行修改」按鈕將設定套入遊戲；完成後即可點擊「啟動遊戲」按鈕立刻開啟遊戲進入戰鬥！\n\n5. 兵種屬性觀察：可在左側導覽列切換至「自訂兵種屬性」與「當前兵種數值」頁面，即時比對原版與修改後的細部屬性資料。" }
        };

        private static readonly Dictionary<string, string> En = new Dictionary<string, string> {
            // UI elements
            { "NavSystem", "⚙   Main Console" },
            { "NavDefaultStats", "📊   Custom Unit Stats" },
            { "NavCurrentStats", "📈   Current Unit Stats" },
            { "NavSaveManager", "💾   Save Manager" },
            { "NavDoc", "📝   Technical Doc" },
            { "MainTitle", "AGAINST ROME MODIFIER PRO" },
            { "NumericTitle", "Preferences & System Settings" },
            { "PopLimit", "Max Population Limit:" },
            { "CiviSpeed", "Civilian Spawn Speed Mult:" },
            { "FocusLoss", "Run in Background (No Auto-Pause on Focus Loss)" },
            { "ToEng", "Force English Language (UI Icons & Core Text)" },
            { "PresetSave", "Export Preset" },
            { "PresetLoad", "Import Preset" },
            { "SwitchesTitle", "Core Mod Switches" },
            { "FreeProd", "Free Construction, Repair & Unit Production" },
            { "FreeUpgrade", "Free Formations, Research & Upgrades" },
            { "NoSpellCost", "No Spell Cost for Priests & Druids (Zero MP)" },
            { "InfiniteMorale", "Infinite Morale (No Decay & Instant Recovery)" },
            { "AiUltimateMode", "AI Ultimate Mode (Max Endless Army, 5s Respawn, 5-10s Loop, No Active Limit)" },
            { "VillageBuildRange", "Village Build Range Candidate Patch (Disabled)" },
            { "TipsTitle", "Modifier Guide & Instructions" },
            { "ConsoleTitle", "System Console & Operations" },
            { "GamePath", "Game Path:" },
            { "Browse", "Browse..." },
            { "LoadCurrent", "Load Current" },
            { "Restore", "Restore Original" },
            { "Apply", "Apply Changes" },
            { "StartGame", "Launch Game" },
            { "RestoreAll", "Restore All" },
            { "RestoreStats", "Restore Stats Only" },
            { "RestoreCompat", "Restore Compatibility Only" },
            { "RestoreLang", "Restore Language Only" },
            { "DefaultStatsTitle", "Custom Unit Stats Comparison (No Custom Buffs)" },
            { "EnableBalance", "Enable Custom Unit Attribute Balance & Faction Traits" },
            { "TroopTemplateLabel", "Template:" },
            { "TroopTemplateSelect", "(Select Template)" },
            { "TroopTemplateBalanced", "Balanced Base Stats" },
            { "BtnTroopPreset", "Edit Troop File" },
            { "TroopPresetDefault", "Stats File: Default Template" },
            { "TroopPresetManual", "Stats File: Custom Config (Manual)" },
            { "TroopPresetFile", "Stats File: {0}" },
            { "TroopPresetLoaded", "Stats File: Loaded from Preset ({0})" },
            { "CurrentStatsTitle", "Current Unit Stats (Original vs. Current)" },
            { "GameSavesTitle", "In-Game Save List" },
            { "BackupsTitle", "Backup History List" },
            { "DetailTitle", "Save Details & Preview" },
            { "BackupSave", "Backup Save" },
            { "DeleteSave", "Delete Save" },
            { "Refresh", "Refresh" },
            { "RestoreBackup", "Restore Backup" },
            { "DeleteBackup", "Delete Backup" },
            { "LanguageLabel", "🌏   Language" },
            { "LangZhButton", "繁體中文" },
            { "LangEnButton", "English" },

            // Headers Default Grid
            { "HeaderName", "Unit Name" },
            { "HeaderIcon", "Icon" },
            { "HeaderType", "Type" },
            { "HeaderStyle", "Category" },
            { "HeaderHp", "HP" },
            { "HeaderMeleeDmg", "Melee Dmg" },
            { "HeaderRangedDmg", "Ranged Dmg" },
            { "HeaderMeleeRelt", "Melee Cooldown" },
            { "HeaderRangedRelt", "Ranged Cooldown" },
            { "HeaderVw", "Defense" },
            { "HeaderAw", "Combat Power" },
            { "HeaderSpeed", "Speed" },
            { "HeaderSight", "Sight" },
            { "HeaderRange", "Range/Skill Dist" },
            { "HeaderSpellRadius", "Spell Radius" },
            { "HeaderTier", "Tier" },

            // Headers Current Grid
            { "HeaderHpComp", "HP Comparison" },
            { "HeaderMeleeDmgComp", "Melee Dmg Comparison" },
            { "HeaderRangedDmgComp", "Ranged Dmg Comparison" },
            { "HeaderMeleeReltComp", "Melee Cooldown Comp" },
            { "HeaderRangedReltComp", "Ranged Cooldown Comp" },
            { "HeaderVwComp", "Defense Comparison" },
            { "HeaderAwComp", "Combat Comparison" },
            { "HeaderSpeedComp", "Speed Comparison" },
            { "HeaderSightComp", "Sight Comparison" },
            { "HeaderRangeComp", "Range Comparison" },
            { "HeaderSpellRadiusComp", "Spell Radius Comp" },

            // Save grid specific headers
            { "HeaderFolder", "Folder" },
            { "HeaderSaveTitle", "Save Title" },
            { "HeaderLevel", "Orig Level" },
            { "HeaderTime", "Save Time" },
            { "HeaderBackupFile", "Backup File" },
            { "HeaderBackupTime", "Backup Time" },
            { "HeaderOrigFolder", "Orig Folder" },

            // Tabs
            { "TabRoman", " Roman " },
            { "TabTeuton", " Teuton " },
            { "TabCelt", " Celt " },
            { "TabHun", " Hun " },

            // Dialog / Log / Message titles
            { "TitlePathError", "Path Error" },
            { "TitleConfirm", "Confirm Action" },
            { "TitleSuccess", "Success" },
            { "TitleError", "Error" },
            { "TitleWarning", "Confirm Overwrite" },
            { "TitleConfirmDelete", "Confirm Deletion" },
            { "TitleTips", "Tips" },

            // Dialog / Log / Message strings
            { "LogConstructCompleted", "Modifier form constructed. Loading data..." },
            { "LogLoadIconFailed", "Failed to load unit icon: " },
            { "LogLoadIconCount", "Successfully loaded {0} unit icons." },
            { "LogDefaultStatsLoaded", "Custom unit stats loaded. Total {0} records." },
            { "LogDefaultStatsLoadError", "Error loading custom unit stats: " },
            { "LogBalanceToggled", "Custom unit attribute balance & faction traits {0}." },
            { "LogReadCurrent", "Reading current settings..." },
            { "LogReadCurrentDone", "Current settings read. Total {0} records for comparison." },
            { "LogBrowseTitle", "Please select the Against Rome installation folder" },
            { "MsgSelectGameDir", "Please set the correct game path first." },
            { "MsgWrongGameDir", "The selected directory does not contain Against_Rome.exe. Please select the correct path." },
            { "MsgConfirmApply", "Are you sure you want to apply all changes?\nThis will overwrite game files." },
            { "MsgApplySuccess", "Changes successfully applied!" },
            { "MsgApplyFailed", "Failed to apply changes: " },
            { "MsgRestoreAllSuccess", "All original settings restored successfully!" },
            { "MsgRestoreStatsSuccess", "Unit stats restored successfully!" },
            { "MsgRestoreCompatSuccess", "Compatibility settings restored successfully!" },
            { "MsgRestoreLangSuccess", "Language settings restored successfully!" },
            { "MsgRestoreFailed", "Failed to restore: " },
            { "MsgExeNotFound", "Against_Rome.exe not found in game directory." },
            { "MsgLaunchFailed", "Failed to launch game: " },
            { "LogStartApply", "Applying changes..." },
            { "LogApplyAllSuccess", "All changes applied successfully!" },
            { "LogStartRestoreAll", "Restoring all original settings..." },
            { "LogRestoreAllDone", "All original settings successfully restored." },
            { "LogStartRestoreStats", "Restoring unit stats to original..." },
            { "LogRestoreStatsDone", "Unit stats successfully restored." },
            { "LogStartRestoreCompat", "Restoring compatibility to original..." },
            { "LogRestoreCompatDone", "Compatibility successfully restored." },
            { "LogStartRestoreLang", "Restoring language pack to original..." },
            { "LogRestoreLangDone", "Language successfully restored." },
            { "LogRestored", "Restored: {0}" },
            { "LogGameStarted", "Game successfully launched." },
            { "LogLangToEng", "English UI & Map language pack applied." },
            { "LogLangToOrig", "Original UI & Map language pack restored." },
            { "LogExePatchFocus", "Applied Against_Rome.exe background execution patch." },
            { "LogExePatchWarning", "[Warning] Against_Rome.exe version or pattern mismatch. Skipping background execution patch." },
            { "LogExePatchOrig", "Restored Against_Rome.exe to original." },
            { "LogVillageBuildRangeApplied", "Village build range candidate EXE patch is disabled; Against_Rome.exe was not modified." },
            { "LogVillageBuildRangeRestored", "Restored legacy village build range candidate EXE patch to original bytes." },
            { "LogVillageBuildRangeWarning", "[Warning] Against_Rome.exe village build range pattern mismatch. Cannot determine current state." },
            { "LogClScriptPatch", "Successfully patched and wrote cl_script.ini (Spell Radius & Civilian Spawn)." },
            { "LogRessPatch", "Successfully patched and wrote ress.ini (free construction, production, upgrades, and spell costs)." },
            { "LogObjdefPatch", "Successfully patched and wrote objdef.dau (Unit stats balance & speed)." },
            { "LogAptPatch", "Successfully patched and wrote apt.dat map files." },
            { "LogTeamDatPatch", "Successfully patched and wrote map team.dat population settings." },
            { "LogEndlessAiNoMaps", "MAPS folder not found. Skipping AI Ultimate Mode." },
            { "LogEndlessAiNoScripts", "Endless ak_level.bci scripts not found. Skipping AI Ultimate Mode." },
            { "LogEndlessAiPatternMissing", "[Warning] Endless army-count bytecode pattern not found. Skipping: {0}" },
            { "LogEndlessAiUltimateApplied", "AI Ultimate Mode applied: changed army count to {1}, respawn wait to 5 seconds, action-loop waits to 5-10 seconds, and bypassed the active-AI limit in {0} endless scripts." },
            { "LogEndlessAiUltimateRestored", "AI Ultimate Mode restored: changed army count back to {1}, respawn wait back to 180 seconds, and action-loop waits plus the active-AI limit back to original values in {0} endless scripts." },
            { "LogNoToEngDir", "'ToEng' folder not found in game directory. Cannot switch game language." },
            { "LogNoZipResource", "Embedded Backup.zip resource not found." },
            { "LogGamePathNotSetIcon", "Game path not set or invalid. Cannot load unit icons." },
            { "LogGuiDatNotFound", "gui.dat not found. Cannot load unit icons." },
            { "LogIconIniNotFound", "icon.ini not found in backup memory. Cannot load unit icons." },
            { "LogObjdefNotFound", "objdef.dau not found in backup memory. Cannot load custom unit stats." },
            { "LogNoObjdefForRead", "No objdef.dau file found to read." },
            { "LogSavePathNotSet", "Game path not set. Cannot load saves." },
            { "LogRefreshSavesFailed", "Failed to refresh save list: " },
            { "SaveDetailGameSave", "Save Type: Game Save\n\nFolder: {0}\n\nSave Title: {1}\n\nOriginal Level: {2}\n\nSave Time: {3}" },
            { "SaveDetailBackup", "Save Type: Backup File\n\nBackup File: {0}\n\nOriginal Folder: {1}\n\nSave Title: {2}\n\nOriginal Level: {3}\n\nBackup Time: {4}" },
            { "MsgSelectBackup", "Please select a backup to restore first." },
            { "MsgCannotResolveOrigFolder", "Cannot resolve the original folder for this backup. Cannot restore." },
            { "MsgGamePathNotSet", "Game path not set first. Cannot restore." },
            { "MsgConfirmOverwriteSave", "The target save folder [{0}] already exists. Overwrite?" },
            { "MsgRestoreBackupSuccess", "Backup restored successfully!" },
            { "MsgRestoreBackupFailed", "Failed to restore backup: " },
            { "MsgSelectSaveToDelete", "Please select a save to delete first." },
            { "MsgInvalidSaveDir", "Invalid save directory. Operation cancelled." },
            { "MsgConfirmDeleteSave", "Are you sure you want to permanently delete the save [{0}]? This action cannot be undone!" },
            { "MsgDeleteSaveSuccess", "Save deleted successfully!" },
            { "MsgDeleteSaveFailed", "Failed to delete save: " },
            { "MsgSelectBackupToDelete", "Please select a backup to delete first." },
            { "MsgConfirmDeleteBackup", "Are you sure you want to permanently delete the backup file [{0}]?" },
            { "MsgDeleteBackupSuccess", "Backup deleted successfully!" },
            { "MsgDeleteBackupFailed", "Failed to delete backup: " },
            { "MsgSelectSaveToBackup", "Please select a save to backup first." },
            { "MsgNoOrigFolderToBackup", "Cannot find the original folder for this save." },
            { "MsgBackupSaveSuccess", "Save backed up successfully!" },
            { "MsgBackupSaveFailed", "Failed to backup save: " },
            { "LogPresetExportDone", "Preset successfully exported to: {0}" },
            { "LogPresetExportError", "Failed to export preset: " },
            { "LogPresetImportDone", "Preset successfully imported." },
            { "LogPresetImportError", "Failed to import preset: " },
            { "LogPresetImportFormatError", "Invalid preset file format." },
            { "LogPresetImportErrorField", "Failed parsing configuration field {0}={1}: {2}" },
            { "MsgPresetExportSuccess", "Preset exported successfully!" },
            { "MsgPresetImportSuccess", "Preset imported successfully!" },
            { "MsgPresetLoadNotFound", "Preset file not found." },
            { "LogLoadTechDocFailed", "Failed to load technical document resource: " },
            { "LogParseObjdefBackupFailed", "Failed to parse backup objdef.dau: " },
            { "LogFreeUpgradeAdded", "Complementing missing upgrade buildings in ress.ini..." },
            { "Unparsable", "Unparsable" },
            { "Unknown", "Unknown" },
            { "LogBackupSaveSuccessDetail", "Save backup successful: {0} -> {1}" },
            { "LogBackupSaveFailedDetail", "Failed to backup save: " },
            { "LogRestoreBackupSuccessDetail", "Backup restore successful: {0} -> {1}" },
            { "LogRestoreBackupFailedDetail", "Failed to restore backup: " },
            { "LogDeleteSaveSuccessDetail", "Deleted game save: {0}" },
            { "LogDeleteSaveFailedDetail", "Failed to delete save: " },
            { "LogDeleteBackupSuccessDetail", "Deleted backup file: {0}" },
            { "LogDeleteBackupFailedDetail", "Failed to delete backup: " },
            { "TipsContent", "💡 Quick Start Guide:\n\n1. Set Game Path: Specify the Against Rome installation directory in the right 'System Console' (the modifier will automatically try to detect the path from the registry).\n\n2. Load Current Settings: Click 'Load Current Settings' on the right. The modifier will parse the currently applied parameters from game files (objdef.dau, ress.ini, cl_script.ini, etc.) and display them in the unit list.\n\n3. Adjust Preferences & Switches: Configure your preferences (such as population limit, feature toggles, etc.) on the Main Console.\n\n4. Apply & Launch: Click 'Apply Changes' to write the configurations to the game. Once done, click 'Launch Game' to start playing immediately!\n\n5. Inspect Unit Stats: Switch to 'Custom Unit Stats' or 'Current Unit Stats' in the sidebar to compare the original and modified attribute details." }
        };
    }
}
