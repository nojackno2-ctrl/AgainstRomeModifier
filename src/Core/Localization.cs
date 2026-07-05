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
            { "NavSystem", "主控制台" },
            { "NavDefaultStats", "自訂兵種屬性" },
            { "NavCurrentStats", "當前兵種數值" },
            { "NavSaveManager", "遊戲存檔管理" },
            { "NavDoc", "修改技術文件" },
            { "NavSkills", "技能屬性修改" },
            { "SkillsHeading", "技能與首領榮譽屬性設定" },
            { "SkillsSubtitle", "自訂部隊主動技能倍率、首領被動特殊技能，以及首領榮譽成長與光環屬性。" },
            { "ColSkillName", "技能與項目" },
            { "ColSkillValue", "當前數值" },
            { "ColSkillDefault", "原版預設值" },
            { "ColGloryLeader", "首領" },
            { "ColGloryAwStuf", "攻擊成長" },
            { "ColGloryVwStuf", "防禦成長" },
            { "ColGloryDamStuf", "傷害成長" },
            { "ColGloryMoraleBonus", "士氣光環" },
            { "ColGloryMoraleTime", "光環時間(ms)" },
            { "ColGloryMaxRuhm", "滿級榮耀" },
            { "GrpGeneralSkills", "部隊與首領特殊技能 (cl_epara.ini & cl_script.ini)" },
            { "GrpLeaderGlory", "首領榮譽升級與士氣光環屬性 (objdef.dau)" },
            { "MainTitle", "AGAINST ROME MODIFIER PRO" },
            { "SystemHeading", "修改器控制中心" },
            { "SystemSubtitle", "選擇要啟用的功能，確認遊戲路徑後再執行修改。" },
            { "NumericTitle", "系統與相容性設定" },
            { "MaxPopulation", "最大人口上限（1600）" },
            { "FastCiviProduction", "村民生產速度最快（10 倍）" },
            { "FocusLoss", "視窗失焦時不自動暫停" },
            { "GameSpeedLabel", "遊戲整體運行速度" },
            { "GameSpeedOff", "原版（不加速）" },
            { "GameSpeedItem", "{0}× 加速" },
            { "ModSkillsAndGlory", "套用自訂首領與單位技能" },
            { "ToEng", "強制英文語系" },
            { "EnableAll", "所有功能開啟" },
            { "DisableAll", "所有功能關閉" },
            { "SwitchesTitle", "資源與戰鬥修改" },
            { "BuildTitle", "建設與人口設定" },
            { "FreeProd", "建造、修復與生產完全免費" },
            { "FreeUpgrade", "陣型、研發與升級完全免費" },
            { "NoSpellCost", "祭司與賢者法術無消耗" },
            { "NoSpellAltar", "法術免除祭壇數量需求" },
            { "SpellEnhancement", "法師技能與復活術強化" },
            { "InfiniteMorale", "部隊無限士氣" },
            { "HousingCapacity20x", "所有人口建築容量提升 20 倍" },
            { "StorageCapacity10x", "主堡與倉庫儲存量提升 10 倍" },
            { "FastBuildUpgradeRepair", "建築建造、升級與維修速度提升 10 倍" },
            { "FoodHealing10x", "待機生命回復量提升 10 倍" },
            { "AiCardTitle", "AI 終極模式（無盡）" },
            { "AiM1", "增援規模（兵團 20 人）" },
            { "AiM2", "增援節奏（波次不斷）" },
            { "AiM3", "敗亡快速回收" },
            { "AiM4", "保證聚落生成與留守" },
            { "AiM5", "AI 開局資源" },
            { "AiM6", "四個定居 AI 配額" },
            { "DgVoodoo", "啟用 dgVoodoo2 圖形相容" },
            { "VillageBuildRange", "村莊建造／紅框範圍 5 倍" },
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
            { "LanguageLabel", "修改器語系 / Language" },
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
            { "LogDefaultStatsLoaded", "自訂兵種屬性載入完成，共 {0} 筆資料。" },
            { "LogDefaultStatsLoadError", "載入自訂屬性數據錯誤: " },
            { "LogBalanceToggled", "自訂兵種屬性平衡與陣營特色已{0}。" },
            { "LogReadCurrent", "正在讀取現有設定..." },
            { "LogReadCurrentDone", "現有屬性設定讀取完成，共 {0} 筆比對資料。" },
            { "LogPresetImportError", "讀取現有設定失敗: " },
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
            { "LogPreApplyRestore", "執行修改前先恢復原版，清除舊版修改殘留..." },
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
            { "LogVillageBuildRangeRestored", "已將村莊範圍四處候選補丁還原為原版 bytes。" },
            { "LogVillageBuildRangeWarning", "[警告] Against_Rome.exe 村莊範圍候選位置不是已知原版或候選 bytes，未進行寫入。" },
            { "LogVillageBuildRangeApplied", "已套用村莊建造／紅框範圍 5 倍補丁。" },
            { "LogVillageBuildRangeSetterRestored", "已還原村莊建造範圍補丁。" },
            { "LogDgVoodooInstalled", "已安裝 dgVoodoo2 {0}（32 位元 D3D8/DirectDraw wrapper）。" },
            { "LogDgVoodooRemoved", "已移除修改器管理的 dgVoodoo2 圖形相容層。" },
            { "LogDgVoodooNotManaged", "未找到由修改器管理的 dgVoodoo2 安裝；未變更手動安裝的檔案。" },
            { "LogDgVoodooPreserved", "保留已被修改的 dgVoodoo2 檔案：{0}" },
            { "DgVoodooUnmanagedConflict", "無法覆蓋非修改器管理的檔案「{0}」。請先移除或重新命名該檔案。" },
            { "DgVoodooModifiedConflict", "無法更新已被修改的檔案「{0}」。請先還原該檔案或移除既有的受管理安裝。" },
            { "LogNoToEngDir", "找不到遊戲目錄下的 ToEng 資料夾，無法切換語系。" },
            { "LogLanguageBackupMissing", "找不到英文資源套用前的完整備份，為避免假還原已中止操作。" },
            { "LogLanguageBackupInvalid", "英文資源原始備份不完整或路徑無效，已中止操作。" },
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
            { "LogLoadTechDocFailed", "載入技術文件資源失敗: " },
            { "Unparsable", "無法解析" },
            { "Unknown", "未知" },
            { "LogBackupSaveSuccessDetail", "備份存檔成功: {0} -> {1}" },
            { "LogBackupSaveFailedDetail", "備份存檔失敗: " },
            { "LogRestoreBackupSuccessDetail", "還原備份成功: {0} -> {1}" },
            { "LogRestoreBackupCleanupFailed", "存檔已還原，但舊存檔暫存資料夾無法清除: " },
            { "LogRestoreBackupFailedDetail", "還原備份失敗: " },
            { "LogDeleteSaveSuccessDetail", "已刪除遊戲存檔: {0}" },
            { "LogDeleteSaveFailedDetail", "刪除存檔失敗: " },
            { "LogDeleteBackupSuccessDetail", "已刪除備份檔案: {0}" },
            { "LogDeleteBackupFailedDetail", "刪除備份失敗: " },
            { "FocusLossTip", "使遊戲在視窗失去焦點（切換到桌面或其他程式）時繼續運行，不會自動暫停。" },
            { "ModSkillsAndGloryTip", "啟用此選項以套用在「技能屬性修改」分頁中所設定的部隊技能、首領被動特殊技能，以及首領榮譽成長與光環數值。若未啟用，則寫入原版預設值。" },
            { "ToEngTip", "將遊戲內的介面、地圖說明與核心文字強制切換為英文，避免中文編碼卡死。" },
            { "DgVoodooTip", "安裝並啟用 dgVoodoo2 圖形修補，解決舊遊戲在 Win 10/11 上的畫面卡頓、fps 低落或黑屏問題。" },
            { "GameSpeedTip", "以加速器方式縮放遊戲主時脈，使移動、生產、戰鬥與 AI 一起以所選倍率運行，最高可選 10×。建議由低倍率（2×～3×）開始測試再逐步調高；倍率越高越可能撞到單幀上限、造成物理不穩或音效不同步。修改 Against_Rome.exe，完全可逆。" },
            { "FreeProdTip", "建造建築物、維修以及在營房等生產所有村民與作戰部隊時，資源消耗均降為 0。" },
            { "FreeUpgradeTip", "陣型研究、科技研發以及兵種屬性解鎖升級時，資源消耗均降為 0。" },
            { "NoSpellCostTip", "祭司與賢者施放所有神術與法術時，魔法值（MP）不消耗。" },
            { "InfiniteMoraleTip", "戰鬥部隊的士氣值鎖定為最大，且在任何情況下不會下降，確保部隊不潰逃。" },
            { "BalanceTip", "啟用修改器內建的兵種屬性微調，平衡各陣營特色，提升整體遊戲性。" },
            { "NoSpellAltarTip", "解鎖並允許直接施放需要多個祭壇才能使用的進階法術，不需建造大量祭壇。" },
            { "SpellEnhancementTip", "傷害法術提升 5 倍，治療法術提升 50 倍；塞爾特人的復活術改為滿血滿士氣，且男屍體復活為槍盾兵、女屍體復活為女雙劍士。" },
            { "MaxPopulationTip", "將遊戲單局內所有玩家的最大人口上限提升至 1600 人（原版預設通常較低）。" },
            { "HousingCapacity20xTip", "帳篷、民房等所有增加人口容量的建築，提供的人口上限額度提升 20 倍。" },
            { "StorageCapacity10xTip", "主堡、倉庫等資源儲存建築的容量上限提升 10 倍，防止資源過早爆滿。" },
            { "FastCiviProductionTip", "主營房轉換或生產村民的速度提升 10 倍，實現極速人口增長。" },
            { "FastBuildUpgradeRepairTip", "建築物的建造速度、研發升級速度與工匠維修速度均提升 10 倍。" },
            { "FoodHealing10xTip", "單位處於待機活動時，每次自動回血由原版 1 點提升為 10 點；計時間隔不變。反編譯已確認這條回血路徑不讀取或扣除食物資源。" },
            { "VillageBuildRangeTip", "將各個村莊的主堡建造邊界（紅框範圍）半徑提升 5 倍，允許更寬廣的建構區域。" },
            { "AiM1Tip", "無盡模式：AI 兵團人數由 6 人提升為 20 人，涵蓋增援部隊、增援重建與村莊日常民轉兵三條路徑。" },
            { "AiM2Tip", "無盡模式：縮短增援等待與排程迴圈延遲、回收完工的 NPC 工作槽，使電腦增援波次快速且不間斷。" },
            { "AiM3Tip", "無盡模式：AI 敗亡後加速舊村莊與城牆的逐筆確認清理、死亡判定與撤退；保留原版安全的隊伍終結順序。" },
            { "AiM4Tip", "無盡模式：必定生成對手聚落、增援門檻提高至 40，並讓增援部隊全數移交村莊留守而非撤退（門檻與留守綁定套用）。" },
            { "AiM5Tip", "無盡模式：AI 聚落開局即擁有主堡儲備資源，加速其經濟與軍隊起步。僅影響電腦聚落，不影響玩家。" },
            { "AiM6Tip", "無盡模式：將 type-1 村莊型 AI 固定為 3，搭配獨立的 type-4 軍事定居 AI 維持四個定居對手，並保留軍事／討伐隊伍所需的 team 名額。只影響套用後新開的遊戲。" }
        };

        private static readonly Dictionary<string, string> En = new Dictionary<string, string> {
            // UI elements
            { "NavSystem", "Main Console" },
            { "NavDefaultStats", "Custom Unit Stats" },
            { "NavCurrentStats", "Current Unit Stats" },
            { "NavSaveManager", "Save Manager" },
            { "NavDoc", "Technical Doc" },
            { "NavSkills", "Skill Modifiers" },
            { "SkillsHeading", "Skill & Leader Glory Settings" },
            { "SkillsSubtitle", "Customize combat skill multipliers, chief special abilities, and leaders' glory scaling stats." },
            { "ColSkillName", "Skill / Attribute Item" },
            { "ColSkillValue", "Current Value" },
            { "ColSkillDefault", "Default Value" },
            { "ColGloryLeader", "Leader" },
            { "ColGloryAwStuf", "ATK Grow" },
            { "ColGloryVwStuf", "DEF Grow" },
            { "ColGloryDamStuf", "DMG Grow" },
            { "ColGloryMoraleBonus", "Aura Morale" },
            { "ColGloryMoraleTime", "Aura Time(ms)" },
            { "ColGloryMaxRuhm", "Max Glory" },
            { "GrpGeneralSkills", "Combat Skill & Tribe Ability Factors (cl_epara.ini & cl_script.ini)" },
            { "GrpLeaderGlory", "Chief Glory Scaling & Morale Aura (objdef.dau)" },
            { "MainTitle", "AGAINST ROME MODIFIER PRO" },
            { "SystemHeading", "Modifier Control Center" },
            { "SystemSubtitle", "Choose the features to enable, verify the game path, then apply the changes." },
            { "NumericTitle", "System & Compatibility Settings" },
            { "MaxPopulation", "Maximum Population Limit (1600)" },
            { "FastCiviProduction", "Fastest Villager Production (10x)" },
            { "FocusLoss", "Run in Background (No Auto-Pause)" },
            { "GameSpeedLabel", "Overall Game Speed" },
            { "GameSpeedOff", "Original (1x)" },
            { "GameSpeedItem", "{0}x Speed" },
            { "ModSkillsAndGlory", "Apply Custom Leader & Unit Skills" },
            { "ToEng", "Force English Language" },
            { "EnableAll", "Enable All" },
            { "DisableAll", "Disable All" },
            { "SwitchesTitle", "Resource & Combat Upgrades" },
            { "BuildTitle", "Build & Population Settings" },
            { "FreeProd", "Free Construction, Repair & Unit Production" },
            { "FreeUpgrade", "Free Formations, Research & Upgrades" },
            { "NoSpellCost", "No Spell Cost for Priests & Druids" },
            { "NoSpellAltar", "No Spell Altar Requirements" },
            { "SpellEnhancement", "Spell & Resurrection Enhancement" },
            { "InfiniteMorale", "Infinite Morale" },
            { "HousingCapacity20x", "20x Capacity for All Population Buildings" },
            { "StorageCapacity10x", "10x Storage Capacity for Town Halls & Warehouses" },
            { "FastBuildUpgradeRepair", "10x Faster Building Construction, Upgrade & Repair" },
            { "FoodHealing10x", "10x Idle HP Regeneration" },
            { "AiCardTitle", "AI Ultimate (Endless)" },
            { "AiM1", "Reinf. Size (20/unit)" },
            { "AiM2", "Reinf. Tempo (endless)" },
            { "AiM3", "Fast Defeat Recovery" },
            { "AiM4", "Guaranteed Spawn & Garrison" },
            { "AiM5", "AI Starting Resources" },
            { "AiM6", "Four Settled AI Quota" },
            { "DgVoodoo", "Enable dgVoodoo2 Wrapper" },
            { "VillageBuildRange", "5x Village Build / Red-Frame Range" },
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
            { "LanguageLabel", "Language" },
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
            { "LogDefaultStatsLoaded", "Custom unit stats loaded. Total {0} records." },
            { "LogDefaultStatsLoadError", "Error loading custom unit stats: " },
            { "LogBalanceToggled", "Custom unit attribute balance & faction traits {0}." },
            { "LogReadCurrent", "Reading current settings..." },
            { "LogReadCurrentDone", "Current settings read. Total {0} records for comparison." },
            { "LogPresetImportError", "Failed to read current settings: " },
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
            { "LogPreApplyRestore", "Restoring the original baseline before applying changes to remove leftovers from older builds..." },
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
            { "LogVillageBuildRangeRestored", "Restored all four village-range candidate sites to their original bytes." },
            { "LogVillageBuildRangeWarning", "[Warning] Village-range candidate sites contain neither known original nor candidate bytes; no write was performed." },
            { "LogVillageBuildRangeApplied", "Applied the 5x village build / red-frame range patch." },
            { "LogVillageBuildRangeSetterRestored", "Restored the village build-range patch." },
            { "LogDgVoodooInstalled", "Installed dgVoodoo2 {0} (32-bit D3D8/DirectDraw wrappers)." },
            { "LogDgVoodooRemoved", "Removed the dgVoodoo2 graphics wrapper managed by this modifier." },
            { "LogDgVoodooNotManaged", "No modifier-managed dgVoodoo2 installation was found; manually installed files were not changed." },
            { "LogDgVoodooPreserved", "Preserved a modified dgVoodoo2 file: {0}" },
            { "DgVoodooUnmanagedConflict", "Cannot overwrite the unmanaged file '{0}'. Remove or rename it first." },
            { "DgVoodooModifiedConflict", "Cannot update the modified file '{0}'. Restore it or remove the managed installation first." },
            { "LogNoToEngDir", "'ToEng' folder not found in game directory. Cannot switch game language." },
            { "LogLanguageBackupMissing", "The complete pre-overlay language backup is missing. Restore was aborted instead of reporting a false success." },
            { "LogLanguageBackupInvalid", "The original language backup is incomplete or contains an invalid path. Operation aborted." },
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
            { "LogLoadTechDocFailed", "Failed to load technical document resource: " },
            { "Unparsable", "Unparsable" },
            { "Unknown", "Unknown" },
            { "LogBackupSaveSuccessDetail", "Save backup successful: {0} -> {1}" },
            { "LogBackupSaveFailedDetail", "Failed to backup save: " },
            { "LogRestoreBackupSuccessDetail", "Backup restore successful: {0} -> {1}" },
            { "LogRestoreBackupCleanupFailed", "The save was restored, but the old-save staging folder could not be removed: " },
            { "LogRestoreBackupFailedDetail", "Failed to restore backup: " },
            { "LogDeleteSaveSuccessDetail", "Deleted game save: {0}" },
            { "LogDeleteSaveFailedDetail", "Failed to delete save: " },
            { "LogDeleteBackupSuccessDetail", "Deleted backup file: {0}" },
            { "LogDeleteBackupFailedDetail", "Failed to delete backup: " },
            { "FocusLossTip", "Prevents the game from automatically pausing when the game window loses focus (runs in background)." },
            { "ModSkillsAndGloryTip", "Enable to apply the custom unit active skills, chief passive special abilities, leader glory growth, and aura settings configured in the Skill tab. Otherwise, standard default game values are written." },
            { "ToEngTip", "Forces the game interface and core text to English to avoid encoding issues." },
            { "DgVoodooTip", "Installs and enables dgVoodoo2 to fix graphics lag, low FPS, or black screens on Windows 10/11." },
            { "GameSpeedTip", "Speedhack-style scaling of the game's master clock so movement, production, combat and AI all run at the chosen multiplier, up to 10x. Start low (2x-3x) and work up; higher factors are more likely to hit the per-frame delta cap, destabilize physics, or desync audio. Patches Against_Rome.exe and is fully reversible." },
            { "FreeProdTip", "Makes all construction, building repairs, and unit production free (0 resource cost)." },
            { "FreeUpgradeTip", "Makes all formations, research, and attribute upgrades free (0 resource cost)." },
            { "NoSpellCostTip", "Priests and Druids consume no mana (0 MP cost) when casting spells." },
            { "InfiniteMoraleTip", "Locks unit morale at maximum, preventing retreat and keeping combat effectiveness." },
            { "BalanceTip", "Enables custom attribute balancing and faction traits for units." },
            { "NoSpellAltarTip", "Removes the requirement of having multiple altars to cast advanced spells." },
            { "SpellEnhancementTip", "Buffs damage spells by 5x, healing by 50x; Celtic resurrection creates Spearmen (male) and Heavy Infantry (female) with 100% HP & morale." },
            { "MaxPopulationTip", "Increases the maximum game population limit to 1600." },
            { "HousingCapacity20xTip", "Increases the population capacity provided by tents and houses by 20x." },
            { "StorageCapacity10xTip", "Increases the resource storage capacity of Town Halls and Warehouses by 10x." },
            { "FastCiviProductionTip", "Increases the production or conversion speed of villagers by 10x." },
            { "FastBuildUpgradeRepairTip", "Increases building construction, upgrade, and repair speed by 10x." },
            { "FoodHealing10xTip", "Raises each automatic idle-regeneration tick from 1 HP to 10 HP while keeping the original interval. Decompilation confirms this path does not read or consume food resources." },
            { "VillageBuildRangeTip", "Increases the village construction/red-frame boundary radius by 5x." },
            { "AiM1Tip", "Endless mode: raises AI unit size from 6 to 20 members across all three paths (reinforcement units, reinforcement rebuild, and village day-to-day civilian conversion)." },
            { "AiM2Tip", "Endless mode: shortens reinforcement wait and scheduler loop delays and recycles completed NPC job slots so AI reinforcement waves arrive quickly and continuously." },
            { "AiM3Tip", "Endless mode: accelerates confirmed per-object cleanup of defeated AI villages and palisades, death detection, and retreat while preserving the vanilla safe terminal order." },
            { "AiM4Tip", "Endless mode: always spawns opponent settlements, raises the reinforcement threshold to 40, and makes reinforcement troops fully garrison the village instead of retreating (threshold and garrison applied together)." },
            { "AiM5Tip", "Endless mode: AI settlements start with main-hall stockpiles to speed up their economy and army. Affects AI settlements only, never the player." },
            { "AiM6Tip", "Endless mode: fixes type-1 village AI at 3 and leaves room for the separate type-4 military settlement, keeping four settled opponents without starving military or attack parties of team slots. Affects only newly started games." }
        };
    }
}
