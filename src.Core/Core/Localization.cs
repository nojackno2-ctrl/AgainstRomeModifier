using System;
using System.Collections.Generic;
using System.IO;

namespace AgainstRomeModifier {
    public enum Language {
        TraditionalChinese,
        English
    }

    public class AppSettings {
        public string Language { get; set; } = "TraditionalChinese";
        public List<string> PromotedFeatures { get; set; } = new();
    }

    public static class Loc {
        private static Language _currentLanguage = Language.TraditionalChinese;
        private static readonly HashSet<string> _promotedFeatures = new(StringComparer.OrdinalIgnoreCase);

        static Loc() {
            LoadLanguagePreference();
        }

        public static Language CurrentLanguage {
            get => _currentLanguage;
            set {
                if (_currentLanguage != value) {
                    _currentLanguage = value;
                    SaveSettings();
                }
            }
        }

        public static IReadOnlyCollection<string> PromotedFeatures => _promotedFeatures;

        public static bool IsPromoted(string featureId) => _promotedFeatures.Contains(featureId);

        public static void PromoteFeature(string featureId) {
            if (_promotedFeatures.Add(featureId)) {
                SaveSettings();
            }
        }

        public static void DemoteFeature(string featureId) {
            if (_promotedFeatures.Remove(featureId)) {
                SaveSettings();
            }
        }

        public static void ReloadLanguage() {
            LoadLanguagePreference();
        }

        // 測試專用：直接覆寫目前語言且不寫入使用者設定檔，
        // 讓測試結果不受執行環境的系統語系或本機偏好影響。
        internal static void OverrideLanguageForTesting(Language language) {
            _currentLanguage = language;
        }

        private static void LoadLanguagePreference() {
            bool languageLoaded = false;
            try {
                string appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
                string configFile = Path.Combine(appData, "AgainstRomeModifier", "settings.json");
                if (File.Exists(configFile)) {
                    using var doc = System.Text.Json.JsonDocument.Parse(File.ReadAllText(configFile));
                    if (doc.RootElement.ValueKind == System.Text.Json.JsonValueKind.Object) {
                        if (doc.RootElement.TryGetProperty("Language", out var lang)) {
                            // 舊版曾以列舉數字（0/1）儲存語言，現行為字串；兩種格式皆接受。
                            if (lang.ValueKind == System.Text.Json.JsonValueKind.Number) {
                                _currentLanguage = lang.GetInt32() == 1 ? Language.English : Language.TraditionalChinese;
                                languageLoaded = true;
                            } else if (lang.ValueKind == System.Text.Json.JsonValueKind.String) {
                                string value = lang.GetString() ?? "";
                                if (value.Equals("English", StringComparison.OrdinalIgnoreCase)) {
                                    _currentLanguage = Language.English;
                                    languageLoaded = true;
                                } else if (value.Equals("TraditionalChinese", StringComparison.OrdinalIgnoreCase)) {
                                    _currentLanguage = Language.TraditionalChinese;
                                    languageLoaded = true;
                                }
                            }
                        }
                        _promotedFeatures.Clear();
                        if (doc.RootElement.TryGetProperty("PromotedFeatures", out var promoted) &&
                            promoted.ValueKind == System.Text.Json.JsonValueKind.Array) {
                            foreach (var feature in promoted.EnumerateArray()) {
                                if (feature.ValueKind == System.Text.Json.JsonValueKind.String &&
                                    feature.GetString() is string id && id.Length > 0) {
                                    _promotedFeatures.Add(id);
                                }
                            }
                        }
                    }
                }
            }
            catch {
                // 設定檔損毀或無法讀取時視同不存在，改用系統語言。
            }

            // 只有在完全讀不到已儲存的語言偏好時才回退到系統語言；
            // 不可用「推廣清單是否為空」判斷（清單為空是常態，會蓋掉使用者選擇）。
            if (!languageLoaded) {
                string sysLang = System.Globalization.CultureInfo.CurrentUICulture.Name;
                _currentLanguage = sysLang.StartsWith("en", StringComparison.OrdinalIgnoreCase)
                    ? Language.English
                    : Language.TraditionalChinese;
            }
        }

        public static void SaveSettings() {
            try {
                string appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
                string configDir = Path.Combine(appData, "AgainstRomeModifier");
                if (!Directory.Exists(configDir)) {
                    Directory.CreateDirectory(configDir);
                }
                string configFile = Path.Combine(configDir, "settings.json");
                var settings = new AppSettings {
                    Language = _currentLanguage.ToString(),
                    PromotedFeatures = _promotedFeatures.ToList()
                };
                string json = System.Text.Json.JsonSerializer.Serialize(settings, new System.Text.Json.JsonSerializerOptions { WriteIndented = true });
                File.WriteAllText(configFile, json);
            }
            catch {
                // ignore
            }
        }

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
            // Launcher elements
            { "GamePathLabel", "遊戲目錄：" },
            { "BrowseButton", "瀏覽..." },

            // UI elements
            { "NavSystem", "主控制台" },
            { "NavExperimental", "實驗性修改" },
            { "NavDefaultStats", "自訂兵種屬性" },
            { "NavCurrentStats", "當前兵種數值" },
            { "MainTitle", "AGAINST ROME MODIFIER PRO" },
            { "SystemHeading", "修改器控制中心" },
            { "SystemSubtitle", "選擇要啟用的功能，確認遊戲路徑後再執行修改。" },
            { "ExperimentalHeading", "實驗性修改控制中心" },
            { "ExperimentalSubtitle", "測試中的實驗性功能。確認測試成功後，可將其移至主控制台。" },
            { "ExperimentalCardTitle", "實驗性修改選項" },
            { "NumericTitle", "系統與相容性設定" },
            { "MaxPopulation", "最大人口上限 (1600)" },
            { "RomanEndless", "無盡模式羅馬陣營" },
            { "RomanReinforcementGarrison", "羅馬增援完全移交" },
            { "VillageGarrisonQuota3x", "村莊駐軍配額 3 倍（實驗）" },
            { "FastCiviProduction", "村民生產速度最快" },
            { "FocusLoss", "視窗失焦不自動暫停" },
            { "GameSpeedLabel", "整體遊戲運行 10 倍加速（實驗性）" },
            { "ToEng", "強制英文語系" },
            { "VillagerTitle", "村民與操作便利" },
            { "SpellTitle", "法術與祭司" },
            { "GeneralSkills", "將軍特殊技能強化（實驗性）" },
            { "LeaderGlory", "首領榮譽與士氣光環增強（實驗性）" },
            { "EnableAll", "所有功能開啟" },
            { "DisableAll", "所有功能關閉" },
            { "CombatTitle", "戰鬥與部隊強化" },
            { "BuildTitle", "建設、經濟與人口" },
            { "FreeProd", "建造與生產免費" },
            { "FreeUpgrade", "科技與升級免費" },
            { "NoSpellCost", "祭司與賢者無限法力" },
            { "NoSpellAltar", "法術免除祭壇需求" },
            { "SpellDamage5x", "法術傷害提升 5 倍" },
            { "SpellHealing10x", "治療術提升 10 倍（實驗性）" },
            { "SpellResurrection", "法師復活術強化（實驗性）" },
            { "InfiniteMorale", "部隊無限士氣" },
            { "HousingCapacity20x", "人口建築容量提升" },
            { "StorageCapacity10x", "主堡與倉庫容量提升" },
            { "HqHp10x", "主堡生命值提升" },
            { "FastBuildUpgradeRepair", "建築建造與維修加速" },
            { "FoodHealing10x", "待機回血速度提升" },
            { "CiviProduce20", "住宅帳篷一次生產 20 人" },
            { "UnitRecruit20", "招募面板一次選滿 20" },
            { "IdleSelect999", "閒置村民一次全選 999" },
            { "AiCardTitle", "無盡模式 (AI 終極戰爭)" },
            { "AiM1", "增援兵團擴軍" },
            { "AiCore", "AI 無盡重生核心" },
            { "AiM5", "提供開局資源" },
            { "DgVoodoo", "啟用圖形相容修補" },
            { "ArgmTrace", "啟用遊戲運作記錄（除錯）（實驗性）" },
            { "VillageBuildRange", "全地圖自由建設" },
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
            { "EnableBalance", "啟用自訂兵種屬性平衡與陣營特色（實驗性）" },
            { "TroopTemplateLabel", "選擇範本:" },
            { "TroopTemplateSelect", "(請選擇範本)" },
            { "TroopTemplateBalanced", "修改器內建平衡" },
            { "BtnTroopPreset", "修改兵種檔案" },
            { "TroopPresetDefault", "屬性檔案：預設範本" },
            { "TroopPresetManual", "屬性檔案：自訂配置 (手動)" },
            { "TroopPresetFile", "屬性檔案：{0}" },
            { "TroopPresetLoaded", "屬性檔案：設定檔載入 ({0})" },
            { "CurrentStatsTitle", "當前兵種數值 (原版與當前對比)" },
            { "Refresh", "重新整理" },
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
            { "LogApplyAllSuccess", "所有修改已成功套用！" },
            { "LogStartRestoreAll", "開始恢復全部原版設定..." },
            { "LogRestoreAllDone", "已成功恢復全部原版設定。" },
            { "LogStartRestoreStats", "開始恢復兵種屬性原版設定..." },
            { "LogRestoreStatsDone", "已成功恢復兵種屬性設定。" },
            { "LogStartRestoreCompat", "開始恢復相容性原版設定..." },
            { "LogRestoreCompatDone", "已成功恢復相容性設定。" },
            { "LogStartRestoreLang", "開始恢復語言原版設定..." },
            { "LogRestoreLangDone", "已成功恢復語言設定。" },
            { "LogGameStarted", "遊戲已成功啟動。" },
            { "LogGamePathNotSetIcon", "遊戲路徑未設定或不存在，無法載入兵種圖示。" },
            { "LogGuiDatNotFound", "找不到 gui.dat，無法載入兵種圖示。" },
            { "LogIconIniNotFound", "記憶體備份中找不到 icon.ini，無法載入兵種圖示。" },
            { "LogObjdefNotFound", "記憶體備份中找不到 objdef.dau，無法載入自訂兵種屬性。" },
            { "LogNoObjdefForRead", "找不到任何 objdef.dau 檔案，無法讀取設定。" },
            { "SaveDetailGameSave", "存檔類型: 遊戲存檔\n\n資料夾: {0}\n\n存檔標題: {1}\n\n原版關卡: {2}\n\n存檔時間: {3}" },
            { "SaveDetailBackup", "存檔類型: 備份檔案\n\n備份檔名: {0}\n\n原資料夾: {1}\n\n存檔標題: {2}\n\n原版關卡: {3}\n\n備份時間: {4}" },
            { "MsgSelectBackup", "請先選擇要還原的備份。" },
            { "MsgGamePathNotSet", "遊戲路徑未設定，無法還原。" },
            { "MsgConfirmOverwriteSave", "目標存檔資料夾 [{0}] 已存在，是否覆蓋？" },
            { "MsgRestoreBackupSuccess", "還原備份成功！" },
            { "MsgRestoreBackupFailed", "還原備份失敗: " },
            { "MsgInvalidSaveDir", "無效的存檔目錄，操作已取消。" },
            { "MsgConfirmDeleteSave", "確定要永久刪除遊戲存檔 [{0}] 嗎？此操作不可還原！" },
            { "MsgDeleteSaveFailed", "刪除存檔失敗: " },
            { "MsgConfirmDeleteBackup", "確定要永久刪除備份檔案 [{0}] 嗎？" },
            { "MsgDeleteBackupFailed", "刪除備份失敗: " },
            { "MsgSelectSaveToBackup", "請先選擇要備份的存檔。" },
            { "MsgNoOrigFolderToBackup", "找不到該存檔的原始資料夾。" },
            { "MsgBackupSaveSuccess", "備份存檔成功！" },
            { "MsgBackupSaveFailed", "備份存檔失敗: " },
            { "BtnRepairEndlessAi", "修復無盡 AI" },
            { "MsgSelectSaveToRepairAi", "請先選擇要修復的無盡模式存檔。" },
            { "MsgConfirmRepairEndlessAi", "修復前會先建立完整備份，再更新存檔內嵌的無盡 AI 排程與增援規則。已堆積的馱馬不會自動移除。是否繼續？" },
            { "MsgRepairEndlessAiSuccess", "無盡 AI 排程與增援規則已修復，並已建立修復前備份。已堆積的馱馬不會自動移除；若仍阻斷增援，請還原堆積前備份或新開無盡局。" },
            { "MsgRepairEndlessAiAlready", "這份存檔已包含目前的無盡 AI 排程與增援修復。" },
            { "MsgRepairEndlessAiFailed", "修復無盡 AI 失敗: " },
            { "Unparsable", "無法解析" },
            { "Unknown", "未知" },
            { "FocusLossTip", "當你切換到桌面、瀏覽器或其他程式（Alt+Tab）時，遊戲不會強制暫停，而是繼續在背景流暢運行，非常適合掛網等待生產與建造！" },
            { "ToEngTip", "將遊戲內的介面、按鈕、地圖與核心文字切換為英文版。能有效解決老遊戲在現代 Windows 系統上的字型亂碼或切換語系時造成的遊戲卡死問題。" },
            { "DgVoodooTip", "安裝 dgVoodoo2 相容層（Direct3D8 轉換器）。能完美修復老遊戲在 Windows 10/11 上常見的畫面卡頓、幀率（FPS）極低、視窗化失敗或啟動黑屏等圖形相容性問題。" },
            { "ArgmTraceTip", "安裝執行期飛行紀錄器（version.dll 代理），把電腦 AI 的實際動作（增援生成、聚落抵達、隊伍復活等）寫入遊戲目錄的 argm_trace.log。純除錯記錄、不改遊戲檔案；套用時會自動辨識遊戲組建以解鎖對應的追蹤項目。用於診斷無盡模式 AI 增援等問題後，可把 log 交給分析。與 dgVoodoo2 併用不衝突。" },
            { "GameSpeedTip", "全局縮放遊戲運作時脈。從生產、建造、移動到戰鬥和 AI 反應，通通同步加速 10 倍！掛機等待時極為實用！" },
            { "GeneralSkillsTip", "將所有將軍與首領的被動特殊技能數值大幅提升 5 倍。" },
            { "LeaderGloryTip", "將各陣營首領隨榮譽升級的攻擊成長、防禦成長、傷害成長與士氣光環數值提升 5 倍。" },
            { "AllUnitsEntireMapVision", "所有單位全地圖視野" },
            { "AllUnitsEntireMapVisionTip", "將所有已知戰鬥單位、領袖、祭司、攻城武器、村民與馱馬的視野（Sirad）設為 30000，敵我全陣營一體適用。與遠程射程 3 倍或法師全地圖施法同開時，本功能最後覆寫 Sirad，但不取消武器本身的 3 倍射程；全地圖視野效果已通過遊戲內實機驗證。" },
            { "LogAllUnitsEntireMapVisionToggled", "所有單位全地圖視野已{0}。" },
            { "NativeWidescreen1920x1080", "高解析度置中（4:3）" },
            { "NativeWidescreen1920x1080Tip", "還原已證實失敗的原生 1920×1080 EXE 實驗並保留遊戲支援的 1600×1200。全螢幕由 dgVoodoo 以 fake fullscreen 置中；視窗化維持原版正常視窗大小並置中，不再強制桌面尺寸。畫面不會拉伸，也不會增加 16:9 世界視野。" },
            { "CameraZoomOut1", "攝影機拉遠 0.5" },
            { "CameraZoomOut1Tip", "把遊戲原生攝影機縮放下限由 0 提高到 0.5，讓戰場溫和拉遠，同時讓整數資訊／LOD 層維持原版 zoom 0 的尺寸。這是針對 +1 實測時單位與資訊過小的修正版；選取、邊緣捲動、霧區、地圖邊界、存讀檔與任務相容性仍需遊戲內驗證。" },
            { "RangedRange3x", "遠程單位射程提升 3 倍" },
            { "RangedRange3xTip", "將所有遠程步兵、遠程騎兵以及攻城武器的攻擊/射擊射程提升 3 倍，並內建命中修正：拋射物落點的傷害判定半徑加倍（近失彈也算命中）、對移動目標預判的隨機散布歸零，避免拉遠射程後打不中。敵我全陣營一體適用。" },
            { "LogRangedRange3xToggled", "遠程單位射程提升 3 倍已{0}。" },
            { "UnitMovementSpeed2x", "單位移動速度提升 2 倍" },
            { "UnitMovementSpeed2xTip", "將所有戰鬥單位（包括士兵、領袖、祭司）的移動速度提升 2 倍。（不包含村民與馱馬）" },
            { "LogUnitMovementSpeed2xToggled", "單位移動速度提升 2 倍已{0}。" },
            { "VillagerMovementSpeed5x", "村民移動速度提升 5 倍" },
            { "VillagerMovementSpeed5xTip", "將平民單位（村民、馱馬）的移動速度提升 5 倍。" },
            { "LogVillagerMovementSpeed5xToggled", "村民移動速度提升 5 倍已{0}。" },
{ "SpellEntireMap", "法師施法距離提升至全地圖" },
{ "SpellEntireMapTip", "將所有祭司施放神術與復活術的最大施法距離提升至全地圖；不改變法術命中半徑。" },
{ "LogSpellEntireMapToggled", "法師施法距離提升至全地圖已{0}。" },
{ "SpellRange3x", "法師法術範圍提升 3 倍" },
{ "SpellRange3xTip", "將法術的效果與命中範圍提升為原版的 3 倍；不改變祭司的施法距離。" },
{ "LogSpellRange3xToggled", "法師法術範圍提升 3 倍已{0}。" },
            { "ProjectileArcHeight", "拋射彈道增高 2 倍" },
            { "ProjectileArcHeightTip", "將箭矢、標槍與投石等拋射物的垂直初速與重力同步提升 2 倍：彈道弧頂增高為原本的 2 倍、更容易越過樹木與建築等障礙物，落點與飛行時間維持不變。敵我全陣營一體適用。" },
            { "RomanEndlessTip", "將五張無盡地圖的玩家陣營強制改為羅馬人。部族選擇畫面的三面旗都會進入羅馬，旗幟可能顯示未選中；選項內的部族切換仍可由玩家手動改回蠻族。羅馬依原版設計沒有祭司與榮耀技能樹；只影響新開局，舊存檔不變。" },
            { "RomanReinforcementGarrisonTip", "讓無盡模式的羅馬增援抵達後，士兵、馱馬與平民全部解除增援隊撤退模式並移交村莊，不再依單位型別分流撤退；此完整移交行為已通過新開無盡局實機驗證。功能會一併提高增援單位門檻，避免已移交單位導致後續增援過早停止。只影響新開無盡局；舊存檔需在存檔管理員執行「修復無盡 AI」。" },
            { "VillageGarrisonQuota3xTip", "實驗功能：將村莊型 AI 由 OD_IPOS01～04 防守位置動態算出的四類兵團配額各乘以 3，保留原兵種比例與零值。只提高兵團數；每團人數仍由「增援兵團擴軍」控制。可能增加村民、人口、效能與存檔負擔，僅建議在新開無盡局測試。" },
            { "FreeProdTip", "建造所有建築物、指派工匠進行維修，以及在營房等軍事設施生產所有村民、步兵與騎兵單位時，完全不需要消耗任何木材、食物或金錢資源！" },
            { "FreeUpgradeTip", "研究兵種陣型、升級軍事科技（攻擊、防禦），以及提升兵種屬性解鎖時，通通免費、不消耗任何資源，助你瞬間研發滿科技！" },
            { "NoSpellCostTip", "施放所有祭司、賢者及法師的主動法術與復活技能時，魔法值（MP）零消耗，讓你的法師能夠無冷卻限制、源源不絕地降下法術天罰！" },
            { "InfiniteMoraleTip", "所有戰鬥單位的士氣值永久鎖定為最大，且在遭受奇襲、以少敵多或戰局不利時絕對不會下降。你的部隊將展現無畏的鋼鐵意志，決不潰逃！" },
            { "BalanceTip", "啟用修改器為各個陣營（羅馬、條頓、塞爾特、匈奴）精心調校的內建平衡屬性檔，修復原版兵種過強或過弱的問題，強化各個陣營的戰術特色。" },
            { "NoSpellAltarTip", "解除高級法術的建造前置條件。即使你只有 1 個祭壇，也能直接使用原版需要建造 3~5 個祭壇才能解鎖的進階強力神術，大幅省下建造成本與空間。" },
            { "SpellDamage5xTip", "將所有陣營的主動攻擊與傷害型法術傷害值提升 5 倍。" },
            { "SpellHealing10xTip", "將塞爾特人等祭司的治療術（Spell1）效果提升 10 倍，快速回復部隊生命。" },
            { "SpellResurrectionTip", "塞爾特人的復活術效果改為滿血滿士氣復活，且男屍體復活為槍盾兵、女屍體復活為女雙劍士。" },
            { "MaxPopulationTip", "解除遊戲原版的人口限制，讓單局戰場內所有陣營的最大總人口上限大幅提升至 1600 人，體驗千人同屏大會戰！" },
            { "HousingCapacity20xTip", "帳篷、民房等所有增加人口容量的建築物，所提供的人口上限額度暴增為原版的 20 倍！只需建造少量民房，即可輕鬆支持千人大軍。" },
            { "StorageCapacity10xTip", "城鎮中心（主堡）、倉庫等儲存資源建築的容量上限增加 10 倍。讓你大舉收集木材、食物等資源時，不再擔心倉庫過早爆滿而中斷生產。" },
            { "HqHp10xTip", "各個陣營城鎮中心（主帳篷/大本營）的生命值上限提升 10 倍。大大增強主營防禦力，避免敵軍偷襲一波爆地，也大幅提供維修緩衝時間。" },
            { "FastCiviProductionTip", "讓城鎮中心生產或轉換村民（農民）的速度加快 10 倍，實現開局人口爆發，快速發展經濟。" },
            { "FastBuildUpgradeRepairTip", "所有建築物的起建建造速度、升級研發速度，以及工匠的修復維修速度同步提升 10 倍！眨眼之間即可將廢墟修復或建好宏偉城塞。" },
            { "FoodHealing10xTip", "當受傷的士兵或單位處於閒置/待機狀態（沒有進行戰鬥或移動）時，每次自動回復生命值的數值由原版 1 點提升至 10 點，療傷速度奇快。已確認此回血不消耗任何食物資源。" },
            { "CiviProduce20Tip", "住宅帳篷生產面板的男/女生產鈕，點一次由原版排入 1 名村民改為一次排入 20 名（會自動夾到剩餘居住容量，建議搭配「20 倍居住容量」）。此為主程式層的玩家專屬修改，不影響 AI 電腦村莊。" },
            { "UnitRecruit20Tip", "村民轉部隊/裝備的招募面板，點一下兵種由原版「選取數 +1」改為「直接跳到上限 20」，不用一個一個點。受遊戲既有上限保護不會超過 20；玩家專屬，不影響 AI。實際生成仍需足夠的空閒村民。" },
            { "IdleSelect999Tip", "IGM「選取閒置村民」鈕原版一次最多只選 40 個，改為 999（引擎主選取清單的結構上限；1600 需搬移固定容量的全域陣列，超出安全補丁範圍）。搜尋鏈另有 1000 筆的掃描上限，故單次點擊實際最多加入 999 個閒置村民。" },
            { "VillageBuildRangeTip", "將各個村莊大本營的建造邊界（原版在地圖上限制建設的紅框限制）擴充至全地圖！您現在可以在地圖的任何角落、任何敵軍腹地前哨自由地建造防禦塔、民房或營房！" },
            { "AiM1Tip", "無盡模式：電腦 AI 的每波增援部隊規模、增援重建以及村莊日常招募的兵團，由原版少數人提升至每隊 20 人！大幅增強戰鬥張力。" },
            { "AiCoreTip", "無盡模式核心：整合加速增援、敗亡清理與聚落重生。三段流程會一起套用或還原，並包含讀檔計時回復保護，避免半套設定造成後期不再重生。" },
            { "AiM5Tip", "無盡模式：給予新生成的電腦 AI 聚落額外的主堡儲備資源。這能幫助電腦 AI 在開局時快速發展起步，提早招兵買馬。僅影響電腦，不會影響玩家。" },

            // 服務層 (PatchEngine / BackupManager) 日誌訊息
            { "SvcLogPreApplyRestore", "正在套用前將相關檔案復原為乾淨狀態，以清除殘留修改..." },
            { "SvcLogFocusApplied", "已套用視窗失去焦點不暫停補丁。" },
            { "SvcLogFocusRestored", "已還原視窗失去焦點暫停設定。" },
            { "SvcLogVillageLegacyRestoreUnknown", "無法還原舊版村莊建造半徑補丁：主程式特徵碼不符合。" },
            { "SvcLogVillageLegacyRemoved", "已移除舊版村莊建造半徑補丁。" },
            { "SvcLogVillageApplyUnknown", "無法套用村莊建造半徑補丁：主程式特徵碼不符合。" },
            { "SvcLogVillageApplied", "已套用村莊建造範圍擴大補丁。" },
            { "SvcLogVillageRestored", "已還原村莊建造範圍設定。" },
            { "SvcLogAltarApplied", "已套用法術免祭壇需求補丁。" },
            { "SvcLogAltarRestored", "已還原法術祭壇需求設定。" },
            { "SvcLogGameSpeedUnknown", "偵測到未知的遊戲時脈常數，已略過遊戲加速補丁以免覆蓋未知版本。" },
            { "SvcLogGameSpeedApplied", "遊戲整體運行速度：{0}× 加速。" },
            { "SvcLogGameSpeedOriginal", "遊戲整體運行速度：原版（未加速）。" },
            { "SvcLogTeamDatApplied", "已修改所有地圖的 team.dat 人口上限為 {0} (共處理 {1} 個檔案)。" },
            { "SvcLogTeamDatRestored", "已將所有地圖的 team.dat 人口上限還原為原版。" },
            { "SvcLogTeamDatRoman", "已將 {0} 張無盡地圖 (team.dat) 的玩家陣營改為羅馬。" },
            { "SvcLogTeamDatRomanRestored", "已將無盡地圖的玩家陣營還原為原版。" },
            { "SvcLogRomanEndlessExeApplied", "已將無盡模式部族選擇強制設為羅馬。" },
            { "SvcLogRomanEndlessExeRestored", "已還原無盡模式的原版部族選擇。" },
            { "SvcLogCiviProduce20Applied", "已將住宅帳篷生產鈕改為點一次 +20。" },
            { "SvcLogCiviProduce20Restored", "已還原住宅帳篷生產鈕為點一次 +1。" },
            { "SvcLogUnitRecruit20Applied", "已將招募面板改為點一次選滿 20。" },
            { "SvcLogUnitRecruit20Restored", "已還原招募面板為點一次 +1。" },
            { "SvcLogIdleSelect999Applied", "已將閒置村民選取上限由 40 提高為 999。" },
            { "SvcLogIdleSelect999Restored", "已還原閒置村民選取上限為 40。" },
            { "SvcLogIdleSelect999Unknown", "Against_Rome.exe 的閒置村民選取函式特徵碼無法辨識；已保留主程式原狀。" },
            { "SvcLogRetiredSpecialArrowsRestored", "已清除停用的特殊箭矢實驗並還原普通箭。" },
            { "SvcLogNativeWidescreenApplied", "已將原生 1600×1200 32-bit mode 0x22 替換為 1920×1080；下次啟動遊戲將強制選用此模式。" },
            { "SvcLogNativeWidescreenRestored", "已還原原生 1600×1200 32-bit 顯示模式。" },
            { "SvcLogNativeWidescreenUnknown", "Against_Rome.exe 的原生寬螢幕特徵碼無法辨識；已保留主程式原狀。" },
            { "SvcLogCameraZoomOutApplied", "已套用攝影機拉遠 0.5 實驗補丁。" },
            { "SvcLogCameraZoomOutRestored", "已還原原版攝影機縮放範圍。" },
            { "SvcLogCameraZoomOutUnknown", "Against_Rome.exe 的攝影機縮放特徵碼無法辨識；已保留主程式原狀。" },
            { "SvcLogRomanEndlessExeUnknown", "無法辨識無盡模式羅馬陣營的 EXE 特徵碼，已保留主程式原狀。" },
            { "SvcLogRomanEndlessMismatch", "警告：無盡模式羅馬陣營的 EXE 與 team.dat 狀態不一致；介面狀態以 EXE 為準。" },
            { "SvcLogLeaderScriptMigrated", "已移除會造成戰鬥閃退的舊版首領榮耀腳本，並以原版 ak_anfuehrer.bci 重建。" },
            { "SvcLogLangApplied", "已成功套用英文介面與地圖語言包。" },
            { "SvcLogLangRestored", "已將英文介面與地圖語言包還原為原版。" },
            { "SvcLogDgvInstalled", "已成功安裝與設定 dgVoodoo2 ({0}) 繪圖轉譯器。" },
            { "SvcLogDgvWidescreenWindow", "已啟用原生寬螢幕的 dgVoodoo2 視窗設定：視窗化時使用桌面尺寸的無邊框視窗，並保持畫面比例。" },
            { "SvcLogDgvWidescreenCustomConfig", "偵測到使用者自訂的 dgVoodoo.conf；已保留其他自訂值，並更新寬螢幕所需的縮放與無邊框桌面尺寸設定。" },
            { "SvcLogDgvNotManaged", "偵測到遊戲目錄中存在非本修改器部署的 dgVoodoo2 相關元件，為保護使用者資產將不主動進行刪除；若要乾淨卸載，請手動刪除遊戲目錄下的 DDraw.dll, D3D8.dll, dgVoodooCpl.exe。" },
            { "SvcLogDgvPreserved", "dgVoodoo2 託管設定檔 {0} 的雜湊值已變更，將予以保留不刪除。" },
            { "SvcLogDgvRemoved", "已成功移除 dgVoodoo2 所有受託管檔案。" },
            { "SvcLogArgmInstalled", "已安裝 argm-trace 執行期飛行紀錄器（version.dll 代理）與 argm_trace.ini。" },
            { "SvcLogArgmHookUnlocked", "已辨識遊戲組建（TimeDateStamp={0}），位址型 AI 事件追蹤已解鎖。啟動遊戲後可於遊戲目錄的 argm_trace.log 檢視記錄。" },
            { "SvcLogArgmHookLocked", "遊戲組建（TimeDateStamp={0}）與被逆向的版本（{1}）不符，僅安裝有簽章驗證的陣營 hook。若確認相同，可手動把 log 中的 TimeDateStamp 填入 argm_trace.ini 解鎖其餘追蹤。" },
            { "SvcLogArgmIniPreserved", "偵測到使用者自訂的 argm_trace.ini，予以保留不覆蓋。" },
            { "SvcLogArgmPreserved", "argm-trace 託管檔 {0} 的雜湊值已變更，將予以保留不刪除。" },
            { "SvcLogArgmNotManaged", "遊戲目錄中存在非本修改器部署的 version.dll，為保護使用者資產將不主動刪除；若要卸載請手動處理。" },
            { "SvcLogArgmRemoved", "已移除 argm-trace 受託管檔案（保留 argm_trace.log 擷取資料）。" },
            { "SvcLogArgmLegacyRemoved", "已移除先前部署的 version.dll 代理（改用 winmm.dll）。" },
            { "SvcLogRestoredFile", "已還原: {0}" },
            { "SvcLogRestoredPopulation", "已還原人口上限: {0}" },
            { "SvcLogDetectFailed", "[偵測] {0} 狀態偵測失敗，該群組選項將顯示為未勾選: {1}" },
            { "SvcErrArgmDllConflict", "遊戲目錄已存在非本工具託管的 version.dll：{0}。為避免覆蓋其它程式（如其他包裝器），已中止安裝遊戲運作記錄。請先手動移除或備份該檔案後再重試。" },
            { "SvcLogBackupLoadedEmbedded", "已載入內嵌 Backup.zip 備份資料。" },
            { "SvcLogBackupLoadedLocal", "已載入程式目錄中的 Backup.zip 備份資料。" },
            { "SvcLogBackupMissing", "找不到內嵌或本機 Backup.zip；請選擇合法的遊戲安裝目錄，程式會從該目錄建立本機記憶體備份。" },
            { "SvcLogEparaHealed", "已使用修改器內建乾淨預設值修復記憶體備份項目: SYSTEM/cl_epara.ini" },
            { "SvcLogEparaHealFailed", "修復記憶體備份項目 SYSTEM/cl_epara.ini 失敗: {0}" },
            { "SvcLogEparaBuildFailed", "以內建預設值建立 SYSTEM/cl_epara.ini 備份失敗: {0}" },
            { "SvcLogAutoHealed", "已從遊戲目錄自動修復缺少之記憶體備份項目: {0}" },
            { "SvcLogAutoHealFailed", "自動修復記憶體備份項目 {0} 失敗: {1}" },
            { "SvcLogTeamDatHealed", "已從遊戲目錄自動修復地圖團隊備份項目 (team.dat)。" },
            { "SvcLogTeamDatHealFailed", "自動修復地圖 team.dat 備份失敗: {0}" },
            { "SvcLogBackupIncomplete", "備份來源缺少必要檔案，修改與還原功能可能無法安全執行:" },
            { "SvcLogBackupFromGameDir", "已成功從指定遊戲目錄建立乾淨記憶體備份。" }
        };

        private static readonly Dictionary<string, string> En = new Dictionary<string, string> {
            // Launcher elements
            { "GamePathLabel", "Game Path:" },
            { "BrowseButton", "Browse..." },

            // UI elements
            { "NavSystem", "Main Console" },
            { "NavExperimental", "Experimental Features" },
            { "NavDefaultStats", "Custom Unit Stats" },
            { "NavCurrentStats", "Current Unit Stats" },
            { "MainTitle", "AGAINST ROME MODIFIER PRO" },
            { "SystemHeading", "Modifier Control Center" },
            { "SystemSubtitle", "Choose the features to enable, verify the game path, then apply the changes." },
            { "ExperimentalHeading", "Experimental Control Center" },
            { "ExperimentalSubtitle", "Features currently in testing. Promote to Main Control Center once validated." },
            { "ExperimentalCardTitle", "Experimental Options" },
            { "NumericTitle", "System & Compatibility Settings" },
            { "MaxPopulation", "Maximum Population Limit (1600)" },
            { "RomanEndless", "Play as Romans in Endless Mode" },
            { "RomanReinforcementGarrison", "Hand Over All Roman Reinforcements" },
            { "VillageGarrisonQuota3x", "Village Garrison Quota 3x (Experimental)" },
            { "FastCiviProduction", "Fastest Villager Production (10x)" },
            { "FocusLoss", "Run in Background (No Auto-Pause)" },
            { "GameSpeedLabel", "Overall Game Speed (10x Acceleration) (Experimental)" },
            { "ToEng", "Force English Language" },
            { "VillagerTitle", "Villagers & Convenience" },
            { "SpellTitle", "Spells & Priests" },
            { "GeneralSkills", "5x General Special Skills (Experimental)" },
            { "LeaderGlory", "Leader Glory & Morale Aura Buff (Experimental)" },
            { "EnableAll", "Enable All" },
            { "DisableAll", "Disable All" },
            { "CombatTitle", "Combat & Unit Enhancements" },
            { "BuildTitle", "Build, Economy & Population" },
            { "FreeProd", "Free Construction, Repair & Unit Production" },
            { "FreeUpgrade", "Free Formations, Research & Upgrades" },
            { "NoSpellCost", "No Spell Cost for Priests & Druids" },
            { "NoSpellAltar", "No Spell Altar Requirements" },
            { "SpellDamage5x", "5x Spell Damage" },
            { "SpellHealing10x", "10x Healing Spell (Experimental)" },
            { "SpellResurrection", "Enhanced Resurrection (Experimental)" },
            { "InfiniteMorale", "Infinite Morale" },
            { "HousingCapacity20x", "20x Capacity for All Population Buildings" },
            { "StorageCapacity10x", "10x Storage Capacity for Town Halls & Warehouses" },
            { "HqHp10x", "10x HP for Town Halls" },
            { "FastBuildUpgradeRepair", "10x Faster Building Construction, Upgrade & Repair" },
            { "FoodHealing10x", "10x Idle HP Regeneration" },
            { "CiviProduce20", "Residential Tent: +20 Civilians Per Click" },
            { "UnitRecruit20", "Recruit Panel: Fill to 20 Per Click" },
            { "IdleSelect999", "Select Idle Villagers: Up to 999" },
            { "AiCardTitle", "Endless Mode (AI Ultimate)" },
            { "AiM1", "Reinf. Size (20/unit)" },
            { "AiCore", "Endless Respawn Core" },
            { "AiM5", "AI Starting Resources" },
            { "DgVoodoo", "Enable dgVoodoo2 Wrapper" },
            { "ArgmTrace", "Enable Gameplay Trace Logger (Debug) (Experimental)" },
            { "VillageBuildRange", "Village Build / Red-Frame Range (Entire Map)" },
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
            { "EnableBalance", "Enable Custom Unit Attribute Balance & Faction Traits (Experimental)" },
            { "TroopTemplateLabel", "Template:" },
            { "TroopTemplateSelect", "(Select Template)" },
            { "TroopTemplateBalanced", "Balanced Base Stats" },
            { "BtnTroopPreset", "Edit Troop File" },
            { "TroopPresetDefault", "Stats File: Default Template" },
            { "TroopPresetManual", "Stats File: Custom Config (Manual)" },
            { "TroopPresetFile", "Stats File: {0}" },
            { "TroopPresetLoaded", "Stats File: Loaded from Preset ({0})" },
            { "CurrentStatsTitle", "Current Unit Stats (Original vs. Current)" },
            { "Refresh", "Refresh" },
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
            { "LogApplyAllSuccess", "All changes applied successfully!" },
            { "LogStartRestoreAll", "Restoring all original settings..." },
            { "LogRestoreAllDone", "All original settings successfully restored." },
            { "LogStartRestoreStats", "Restoring unit stats to original..." },
            { "LogRestoreStatsDone", "Unit stats successfully restored." },
            { "LogStartRestoreCompat", "Restoring compatibility to original..." },
            { "LogRestoreCompatDone", "Compatibility successfully restored." },
            { "LogStartRestoreLang", "Restoring language pack to original..." },
            { "LogRestoreLangDone", "Language successfully restored." },
            { "LogGameStarted", "Game successfully launched." },
            { "LogGamePathNotSetIcon", "Game path not set or invalid. Cannot load unit icons." },
            { "LogGuiDatNotFound", "gui.dat not found. Cannot load unit icons." },
            { "LogIconIniNotFound", "icon.ini not found in backup memory. Cannot load unit icons." },
            { "LogObjdefNotFound", "objdef.dau not found in backup memory. Cannot load custom unit stats." },
            { "LogNoObjdefForRead", "No objdef.dau file found to read." },
            { "SaveDetailGameSave", "Save Type: Game Save\n\nFolder: {0}\n\nSave Title: {1}\n\nOriginal Level: {2}\n\nSave Time: {3}" },
            { "SaveDetailBackup", "Save Type: Backup File\n\nBackup File: {0}\n\nOriginal Folder: {1}\n\nSave Title: {2}\n\nOriginal Level: {3}\n\nBackup Time: {4}" },
            { "MsgSelectBackup", "Please select a backup to restore first." },
            { "MsgGamePathNotSet", "Game path not set first. Cannot restore." },
            { "MsgConfirmOverwriteSave", "The target save folder [{0}] already exists. Overwrite?" },
            { "MsgRestoreBackupSuccess", "Backup restored successfully!" },
            { "MsgRestoreBackupFailed", "Failed to restore backup: " },
            { "MsgInvalidSaveDir", "Invalid save directory. Operation cancelled." },
            { "MsgConfirmDeleteSave", "Are you sure you want to permanently delete the save [{0}]? This action cannot be undone!" },
            { "MsgDeleteSaveFailed", "Failed to delete save: " },
            { "MsgConfirmDeleteBackup", "Are you sure you want to permanently delete the backup file [{0}]?" },
            { "MsgDeleteBackupFailed", "Failed to delete backup: " },
            { "MsgSelectSaveToBackup", "Please select a save to backup first." },
            { "MsgNoOrigFolderToBackup", "Cannot find the original folder for this save." },
            { "MsgBackupSaveSuccess", "Save backed up successfully!" },
            { "MsgBackupSaveFailed", "Failed to backup save: " },
            { "BtnRepairEndlessAi", "Repair Endless AI" },
            { "MsgSelectSaveToRepairAi", "Please select an endless-mode save to repair." },
            { "MsgConfirmRepairEndlessAi", "A full backup will be created before updating the endless AI scheduler and reinforcement rules embedded in this save. Existing accumulated pack horses will not be removed automatically. Continue?" },
            { "MsgRepairEndlessAiSuccess", "The endless AI scheduler and reinforcement rules were repaired and a pre-repair backup was created. Existing pack horses were not removed; if they still block reinforcements, restore a backup from before the pileup or start a new endless game." },
            { "MsgRepairEndlessAiAlready", "This save already contains the current endless AI scheduler and reinforcement repairs." },
            { "MsgRepairEndlessAiFailed", "Failed to repair endless AI: " },
            { "Unparsable", "Unparsable" },
            { "Unknown", "Unknown" },
            { "FocusLossTip", "Prevents the game from automatically pausing when the game window loses focus (runs in background)." },
            { "ToEngTip", "Forces the game interface and core text to English to avoid encoding issues." },
            { "DgVoodooTip", "Installs and enables dgVoodoo2 to fix graphics lag, low FPS, or black screens on Windows 10/11." },
            { "ArgmTraceTip", "Installs a runtime flight-recorder (a version.dll proxy) that logs what the computer AI actually does (reinforcement spawns, settlement arrivals, team respawns) to argm_trace.log in the game folder. Log-only, never edits game files; it auto-detects the game build to unlock the matching trace hooks. Use it to diagnose endless-mode AI reinforcement issues, then hand the log over for analysis. Coexists with dgVoodoo2." },
            { "GameSpeedTip", "Speedhack-style scaling of the game's master clock so movement, production, combat and AI all run at 10x speed. Patches Against_Rome.exe and is fully reversible." },
            { "GeneralSkillsTip", "Greatly increases all passive special ability values of generals and chiefs by 5x." },
            { "LeaderGloryTip", "Multiplies each faction chief's glory-level attack/defense/damage growth and morale aura by 5x." },
            { "AllUnitsEntireMapVision", "Entire-Map Vision for All Units" },
            { "AllUnitsEntireMapVisionTip", "Sets sight (Sirad) to 30000 for all known combat units, leaders, priests, siege engines, villagers, and packhorses across every faction. When combined with 3x ranged range or entire-map priest casting, this feature owns Sirad last without cancelling the 3x weapon range; the entire-map vision effect is runtime-verified in game." },
            { "LogAllUnitsEntireMapVisionToggled", "Entire-Map Vision for All Units {0}." },
            { "NativeWidescreen1920x1080", "Centered High Resolution (4:3)" },
            { "NativeWidescreen1920x1080Tip", "Restores the runtime-rejected native 1920x1080 EXE experiment and keeps supported 1600x1200. dgVoodoo centers fullscreen through fake fullscreen; windowed mode keeps the normal stock window size and is centered instead of being forced to desktop size. It is not stretched and does not expand the world to 16:9." },
            { "CameraZoomOut1", "Camera Zoom Out 0.5" },
            { "CameraZoomOut1Tip", "Raises the native camera minimum from 0 to 0.5 for a milder pullback while integer information/LOD consumers retain the stock zoom-0 scale. This revises the +1 build after runtime testing made units and information too small. Selection, edge scrolling, fog, map bounds, save/load, and mission compatibility still require in-game verification." },
            { "RangedRange3x", "3x Range for Ranged Units" },
            { "RangedRange3xTip", "Triples the attack and firing range of all ranged infantry, ranged cavalry, and siege weapons. Includes a built-in accuracy fix: the projectile impact damage radius is doubled (near misses now hit) and the random lead-aim scatter against moving targets is removed, so shots still connect at the longer range. Applies to all factions." },
            { "LogRangedRange3xToggled", "3x Range for Ranged Units {0}." },
            { "UnitMovementSpeed2x", "2x Movement Speed for Units" },
            { "UnitMovementSpeed2xTip", "Doubles the movement speed of all combat units (including soldiers, leaders, priests). (Excludes villagers and packhorses)" },
            { "LogUnitMovementSpeed2xToggled", "2x Movement Speed for Units {0}." },
            { "VillagerMovementSpeed5x", "5x Movement Speed for Villagers" },
            { "VillagerMovementSpeed5xTip", "Multiplies the movement speed of civilian units (villagers, packhorses) by 5x." },
            { "LogVillagerMovementSpeed5xToggled", "5x Movement Speed for Villagers {0}." },
{ "SpellEntireMap", "Spell Casting Distance: Entire Map" },
{ "SpellEntireMapTip", "Raises priest spell and resurrection casting distance to the entire map; spell effect radius is unchanged." },
{ "LogSpellEntireMapToggled", "Spell Casting Distance: Entire Map {0}." },
{ "SpellRange3x", "3x Spell Effect Radius" },
{ "SpellRange3xTip", "Triples spell effect and hit radius from the original value; priest casting distance is unchanged." },
{ "LogSpellRange3xToggled", "3x Spell Effect Radius {0}." },
            { "ProjectileArcHeight", "2x Projectile Arc Height" },
            { "ProjectileArcHeightTip", "Raises projectile launch speed and gravity by 2x together: arrows, javelins and catapult stones fly 2x higher arcs that clear trees and buildings more easily, while landing point and flight time stay unchanged. Applies to all factions." },
            { "RomanEndlessTip", "Forces the player faction to Roman on all five endless maps. All three flags in the faction dialog will enter as Romans and may show no highlighted selection; the faction switch in Options can still be used manually to return to a barbarian faction. By original design Romans have no priest or glory skill tree; affects new games only." },
            { "RomanReinforcementGarrisonTip", "Releases every Roman reinforcement object from retreat mode and hands soldiers, pack horses, and civilians to the village instead of splitting them by unit type; this complete handoff is verified in a fresh endless game. The reinforcement unit threshold is raised with the handoff patch so retained units do not stop later waves prematurely. Affects new endless games only; use Repair Endless AI in Save Manager for an existing save." },
            { "VillageGarrisonQuota3xTip", "Experimental: triples each of the four village-AI squad quotas dynamically derived from OD_IPOS01-04, preserving the original unit mix and zero values. This raises squad count only; members per squad remain controlled by Reinforcement Size. May increase civilian, population, performance, and save-size load; test in a fresh endless game." },
            { "FreeProdTip", "Makes all construction, building repairs, and unit production free (0 resource cost)." },
            { "FreeUpgradeTip", "Makes all formations, research, and attribute upgrades free (0 resource cost)." },
            { "NoSpellCostTip", "Priests and Druids consume no mana (0 MP cost) when casting spells." },
            { "InfiniteMoraleTip", "Locks unit morale at maximum, preventing retreat and keeping combat effectiveness." },
            { "BalanceTip", "Enables custom attribute balancing and faction traits for units." },
            { "NoSpellAltarTip", "Removes the requirement of having multiple altars to cast advanced spells." },
            { "SpellDamage5xTip", "Increases active combat and damage spell values by 5x for all factions." },
            { "SpellHealing10xTip", "Multiplies Kelt priest healing spell (Spell1) effects by 10x for rapid recovery." },
            { "SpellResurrectionTip", "Celtic resurrection restores units with 100% HP & morale, spawning Spearmen (male) or Heavy Infantry (female)." },
            { "MaxPopulationTip", "Increases the maximum game population limit to 1600." },
            { "HousingCapacity20xTip", "Increases the population capacity provided by tents and houses by 20x." },
            { "StorageCapacity10xTip", "Increases the resource storage capacity of Town Halls and Warehouses by 10x." },
            { "HqHp10xTip", "Increases the hit points (HP) of all Town Halls by 10x, significantly boosting base defense." },
            { "FastCiviProductionTip", "Increases the production or conversion speed of villagers by 10x." },
            { "FastBuildUpgradeRepairTip", "Increases building construction, upgrade, and repair speed by 10x." },
            { "FoodHealing10xTip", "Raises each automatic idle-regeneration tick from 1 HP to 10 HP while keeping the original interval. Decompilation confirms this path does not read or consume food resources." },
            { "CiviProduce20Tip", "Each click on the male/female production button in the residential tent panel now queues 20 civilians instead of 1 (clamped to remaining housing capacity — pair with 20x Housing Capacity). This is a player-only executable patch and does not affect AI villages." },
            { "UnitRecruit20Tip", "In the villager-to-unit recruit panel, clicking a unit now sets its selected count straight to the cap of 20 instead of +1, so you don't click one at a time. Protected by the game's existing cap so it never exceeds 20; player-only, does not affect AI. Actual creation still needs enough free villagers." },
            { "IdleSelect999Tip", "The IGM \"select idle villagers\" button originally selects at most 40 villagers per click; this raises it to 999 (the structural cap of the engine's master selection list; 1600 would require relocating fixed-capacity global arrays, which is beyond safe patching). The search chain also scans at most 1000 entries, so one click adds up to 999 idle villagers." },
            { "VillageBuildRangeTip", "Sets the village construction/red-frame boundary radius to a very large value, allowing construction anywhere on the map." },
            { "AiM1Tip", "Endless mode: raises AI unit size from 6 to 20 members across all three paths (reinforcement units, reinforcement rebuild, and village day-to-day civilian conversion)." },
            { "AiCoreTip", "Endless-mode core: atomically combines accelerated reinforcement, defeat cleanup, and settlement respawn, including the save/load timer recovery guard. The three lifecycle stages are always applied or restored together." },
            { "AiM5Tip", "Endless mode: AI settlements start with main-hall stockpiles to speed up their economy and army. Affects AI settlements only, never the player." },

            // Service-layer (PatchEngine / BackupManager) log messages
            { "SvcLogPreApplyRestore", "Restoring affected files to a clean state before applying, to clear leftover modifications..." },
            { "SvcLogFocusApplied", "Applied the no-pause-on-focus-loss patch." },
            { "SvcLogFocusRestored", "Restored the original pause-on-focus-loss behavior." },
            { "SvcLogVillageLegacyRestoreUnknown", "Cannot revert the legacy village build-range patch: executable signature mismatch." },
            { "SvcLogVillageLegacyRemoved", "Removed the legacy village build-range patch." },
            { "SvcLogVillageApplyUnknown", "Cannot apply the village build-range patch: executable signature mismatch." },
            { "SvcLogVillageApplied", "Applied the expanded village build-range patch." },
            { "SvcLogVillageRestored", "Restored the original village build-range." },
            { "SvcLogAltarApplied", "Applied the no-altar-requirement spell patch." },
            { "SvcLogAltarRestored", "Restored the original spell altar requirement." },
            { "SvcLogGameSpeedUnknown", "Unknown game clock constants detected; skipped the game-speed patch to avoid overwriting an unknown build." },
            { "SvcLogGameSpeedApplied", "Overall game speed: {0}x acceleration." },
            { "SvcLogGameSpeedOriginal", "Overall game speed: original (no acceleration)." },
            { "SvcLogTeamDatApplied", "Set the population cap to {0} in every map's team.dat ({1} files processed)." },
            { "SvcLogTeamDatRestored", "Restored the original population cap in every map's team.dat." },
            { "SvcLogTeamDatRoman", "Set the player faction to Roman in {0} endless maps (team.dat)." },
            { "SvcLogTeamDatRomanRestored", "Restored the original player faction in endless maps." },
            { "SvcLogRomanEndlessExeApplied", "Forced the endless-mode faction selector to Romans." },
            { "SvcLogRomanEndlessExeRestored", "Restored the original endless-mode faction selector." },
            { "SvcLogCiviProduce20Applied", "Set the residential tent production button to +20 per click." },
            { "SvcLogCiviProduce20Restored", "Restored the residential tent production button to +1 per click." },
            { "SvcLogUnitRecruit20Applied", "Set the recruit panel to fill to 20 per click." },
            { "SvcLogUnitRecruit20Restored", "Restored the recruit panel to +1 per click." },
            { "SvcLogIdleSelect999Applied", "Raised the idle-villager selection cap from 40 to 999." },
            { "SvcLogIdleSelect999Restored", "Restored the idle-villager selection cap to 40." },
            { "SvcLogIdleSelect999Unknown", "The idle-villager selection EXE signature is unknown; the executable was left unchanged." },
            { "SvcLogRetiredSpecialArrowsRestored", "Removed the retired special-arrow experiment and restored normal arrows." },
            { "SvcLogNativeWidescreenApplied", "Replaced native 1600x1200 32-bit mode 0x22 with 1920x1080; the next game launch will force this mode." },
            { "SvcLogNativeWidescreenRestored", "Restored the native 1600x1200 32-bit display mode." },
            { "SvcLogNativeWidescreenUnknown", "The native-widescreen EXE signatures are unknown; the executable was left unchanged." },
            { "SvcLogCameraZoomOutApplied", "Applied the experimental Camera Zoom Out 0.5 patch." },
            { "SvcLogCameraZoomOutRestored", "Restored the stock camera zoom range." },
            { "SvcLogCameraZoomOutUnknown", "The camera-zoom EXE signatures are unknown; the executable was left unchanged." },
            { "SvcLogRomanEndlessExeUnknown", "The Roman endless-mode EXE signature is unknown; the executable was left unchanged." },
            { "SvcLogRomanEndlessMismatch", "Warning: Roman endless-mode EXE and team.dat states disagree; the UI follows the EXE state." },
            { "SvcLogLeaderScriptMigrated", "Removed the legacy leader-glory script that caused combat crashes and rebuilt ak_anfuehrer.bci from the original." },
            { "SvcLogLangApplied", "Applied the English UI and map language pack." },
            { "SvcLogLangRestored", "Restored the original language files." },
            { "SvcLogDgvInstalled", "Installed and configured the dgVoodoo2 ({0}) graphics wrapper." },
            { "SvcLogDgvWidescreenWindow", "Enabled the native-widescreen dgVoodoo2 window profile: windowed mode uses a borderless desktop-sized window while preserving aspect ratio." },
            { "SvcLogDgvWidescreenCustomConfig", "A customized dgVoodoo.conf was detected. Other custom values were preserved while the required scaling and borderless desktop-sized window settings were updated." },
            { "SvcLogDgvNotManaged", "Detected dgVoodoo2 components in the game folder that were not deployed by this modifier; they will not be deleted to protect your files. For a clean uninstall, manually remove DDraw.dll, D3D8.dll and dgVoodooCpl.exe from the game folder." },
            { "SvcLogDgvPreserved", "The managed dgVoodoo2 file {0} has been modified; it will be kept, not deleted." },
            { "SvcLogDgvRemoved", "Removed all modifier-managed dgVoodoo2 files." },
            { "SvcLogArgmInstalled", "Installed the argm-trace runtime flight recorder (version.dll proxy) and argm_trace.ini." },
            { "SvcLogArgmHookUnlocked", "Game build recognized (TimeDateStamp={0}); address-based AI event hooks unlocked. After launching, read argm_trace.log in the game folder." },
            { "SvcLogArgmHookLocked", "Game build (TimeDateStamp={0}) does not match the analyzed build ({1}); only the signature-verified faction hook was installed. If they are the same build, put the TimeDateStamp from the log into argm_trace.ini to unlock the rest." },
            { "SvcLogArgmIniPreserved", "A customized argm_trace.ini was detected; it was preserved and not overwritten." },
            { "SvcLogArgmPreserved", "The managed argm-trace file {0} has been modified; it will be kept, not deleted." },
            { "SvcLogArgmNotManaged", "A version.dll not deployed by this modifier exists in the game folder; it will not be deleted to protect your files. Remove it manually to uninstall." },
            { "SvcLogArgmRemoved", "Removed modifier-managed argm-trace files (argm_trace.log capture data is kept)." },
            { "SvcLogArgmLegacyRemoved", "Removed the previously deployed version.dll proxy (superseded by winmm.dll)." },
            { "SvcLogRestoredFile", "Restored: {0}" },
            { "SvcLogRestoredPopulation", "Restored population cap: {0}" },
            { "SvcLogDetectFailed", "[Detect] Failed to detect {0} state; options in this group will show as unchecked: {1}" },
            { "SvcErrArgmDllConflict", "A version.dll not managed by this tool already exists in the game folder: {0}. Installation of the gameplay trace logger was aborted to avoid overwriting another program (e.g. a different wrapper). Remove or back up that file manually and retry." },
            { "SvcLogBackupLoadedEmbedded", "Loaded the embedded Backup.zip baseline." },
            { "SvcLogBackupLoadedLocal", "Loaded Backup.zip from the program folder." },
            { "SvcLogBackupMissing", "No embedded or local Backup.zip found; select a valid game folder and the modifier will build an in-memory baseline from it." },
            { "SvcLogEparaHealed", "Rebuilt the in-memory backup entry SYSTEM/cl_epara.ini from the modifier's built-in clean defaults." },
            { "SvcLogEparaHealFailed", "Failed to rebuild backup entry SYSTEM/cl_epara.ini: {0}" },
            { "SvcLogEparaBuildFailed", "Failed to build the SYSTEM/cl_epara.ini backup from built-in defaults: {0}" },
            { "SvcLogAutoHealed", "Auto-restored missing backup entry from the game folder: {0}" },
            { "SvcLogAutoHealFailed", "Failed to auto-restore backup entry {0}: {1}" },
            { "SvcLogTeamDatHealed", "Auto-restored map team.dat backup entries from the game folder." },
            { "SvcLogTeamDatHealFailed", "Failed to auto-restore map team.dat backups: {0}" },
            { "SvcLogBackupIncomplete", "The backup source is missing required files; apply/restore may not run safely:" },
            { "SvcLogBackupFromGameDir", "Built a clean in-memory baseline from the selected game folder." }
        };
    }
}
