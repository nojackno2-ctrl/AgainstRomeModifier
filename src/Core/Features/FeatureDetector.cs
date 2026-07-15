using System.Globalization;
using System.Text;
using System.Text.RegularExpressions;
using AgainstRomeModifier.Core.Features.Bci;
using AgainstRomeModifier.Core.Features.Install;
using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features;

internal sealed class FeatureDetector
{
    private static readonly Regex RegexCiviLoad = new(@"CiviDelay\s*=\s*([A-Z]{3})\s*,\s*(\d+)", RegexOptions.Compiled);
    private static readonly Regex RegexMoraleLostMemLoad = new(@"MoralsDecLostMem\s*=\s*GER\s*,\s*(\d+)", RegexOptions.Compiled);
    private static readonly Regex RegexMoraleFleeLoad = new(@"MoralsDecFlee\s*=\s*GER\s*,\s*(\d+)", RegexOptions.Compiled);
    private static readonly Regex RegexMoraleOverPopLoad = new(@"MoralsDecOverPop\s*=\s*GER\s*,\s*(\d+)", RegexOptions.Compiled);
    private static readonly Regex RegexMoraleIdleLoad = new(@"MoralsIncIdle\s*=\s*GER\s*,\s*(\d+)", RegexOptions.Compiled);
    private static readonly Regex RegexSpellValueLoad = new(@"^(Value\d*)\s*=\s*([A-Z]{3})\s*,\s*(Spell\d+)\s*,\s*([^;\r\n]+)", RegexOptions.Compiled | RegexOptions.Multiline);
    private static readonly Regex RegexRadiusLoad = new(@"^Radius\s*=\s*([A-Z]{3})\s*,\s*(Spell\d+)\s*,\s*([^;\r\n]+)", RegexOptions.Compiled | RegexOptions.Multiline);
    private static readonly Regex RegexAbilityValueLoad = new(@"^(Value\d*)\s*=\s*([A-Z]{3})\s*,\s*(SAbility\d+)\s*,\s*([^;\r\n]+)", RegexOptions.Compiled | RegexOptions.Multiline);
    private const int HousingCapacityMultiplier = 20;
    private const int StorageCapacityMultiplier = 10;
    private readonly ILogger _logger;
    internal FeatureDetector(ILogger logger) => _logger = logger;
        internal PatchProfile Detect(string gamePath, BackupManager backupManager)
        {
            var options = new PatchProfile();
            ExeRomanEndlessPatchState romanExeState = ExeRomanEndlessPatchState.Unknown;

            // A. Against_Rome.exe
            string exePath = Path.Combine(gamePath, "Against_Rome.exe");
            if (File.Exists(exePath))
            {
                try
                {
                    byte[] exeBytes = File.ReadAllBytes(exePath);
                    ExePatchState exeState = ExePatchModel.GetExePatchState(exeBytes);
                    if (exeState != ExePatchState.Unknown)
                    {
                        options.FocusLoss = (exeState == ExePatchState.FocusPatched);
                    }
                    
                    var setterState = ExePatchModel.GetVillageSetterPatchState(exeBytes);
                    if (setterState != ExeVillageSetterPatchState.Unknown)
                    {
                        options.VillageBuildRange = (setterState == ExeVillageSetterPatchState.Legacy2x ||
                            setterState == ExeVillageSetterPatchState.Legacy2Point5x ||
                            setterState == ExeVillageSetterPatchState.Legacy3x ||
                            setterState == ExeVillageSetterPatchState.Legacy5x ||
                            setterState == ExeVillageSetterPatchState.EntireMap);
                    }

                    var altarState = ExePatchModel.GetSpellAltarPatchState(exeBytes);
                    if (altarState != ExeSpellAltarPatchState.Unknown)
                    {
                        options.NoSpellAltar = (altarState == ExeSpellAltarPatchState.Patched);
                    }

                    romanExeState = ExePatchModel.GetRomanEndlessPatchState(exeBytes);

                    int speed = ExePatchModel.GetGameSpeedMultiplier(exeBytes);
                    options.GameSpeed = speed;

                    options.CiviProduce20 =
                        ExePatchModel.GetCiviProduce20PatchState(exeBytes) == ExeCiviProduce20PatchState.Patched;
                    options.UnitRecruit20 =
                        ExePatchModel.GetUnitRecruit20PatchState(exeBytes) == ExeUnitRecruit20PatchState.Patched;
                }
                catch (Exception ex) { _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "Against_Rome.exe", ex.Message)); }
            }

            // B. cl_script.ini (CiviDelay, InfiniteMorale, Balance)
            string clPath = Path.Combine(gamePath, @"SYSTEM\cl_script.ini");
            if (File.Exists(clPath))
            {
                try
                {
                    byte[] clBytes = File.ReadAllBytes(clPath);
                    byte[] decomp = GameLZSS.DecompressPfil(clBytes);
                    string text = Encoding.GetEncoding(1251).GetString(decomp);

                    MatchCollection civiMatches = RegexCiviLoad.Matches(text);
                    options.FastCiviProduction = civiMatches.Count > 0 && civiMatches.Cast<Match>().All(match =>
                        double.TryParse(match.Groups[2].Value, NumberStyles.Any, CultureInfo.InvariantCulture, out double delay) &&
                        delay > 0 && delay <= 500.0);

                    var matchLost = RegexMoraleLostMemLoad.Match(text);
                    var matchFlee = RegexMoraleFleeLoad.Match(text);
                    var matchOverPop = RegexMoraleOverPopLoad.Match(text);
                    var matchIdle = RegexMoraleIdleLoad.Match(text);
                    options.InfiniteMorale = matchLost.Success && matchFlee.Success && matchOverPop.Success && matchIdle.Success &&
                        int.TryParse(matchLost.Groups[1].Value, out int lost) && lost == 0 &&
                        int.TryParse(matchFlee.Groups[1].Value, out int flee) && flee == 0 &&
                        int.TryParse(matchOverPop.Groups[1].Value, out int overPop) && overPop >= 99999999 &&
                        int.TryParse(matchIdle.Groups[1].Value, out int idle) && idle == 500;

                    // 法術/技能強化偵測：與備份原檔逐鍵比對倍率（與 ClScriptPatcher 寫入邏輯對稱）
                    if (backupManager.BackupFiles.TryGetValue("SYSTEM/cl_script.ini", out byte[]? clOriginalBytes))
                    {
                        string originalText = Encoding.GetEncoding(1251).GetString(GameLZSS.DecompressPfil(clOriginalBytes));
                        var currentSpells = ReadKeyedValues(text, RegexSpellValueLoad);
                        var originalSpells = ReadKeyedValues(originalText, RegexSpellValueLoad);
                        var currentRadius = ReadRadiusValues(text);
                        var originalRadius = ReadRadiusValues(originalText);
                        options.SpellDamage5x = AllScaled(currentSpells, originalSpells, ClScriptPatcher.DamageSpellKeys.Select(k => $"Value_{k}"), 5);
                        options.SpellHealing10x = AllScaled(currentSpells, originalSpells, new[] { "Value_KEL_Spell1" }, 10);
                        options.SpellResurrection = DetectResurrection(currentSpells, originalSpells);
                        options.SpellRange3x = RadiusScaledByThree(currentRadius, originalRadius);

                        var currentAbilities = ReadKeyedValues(text, RegexAbilityValueLoad);
                        var originalAbilities = ReadKeyedValues(originalText, RegexAbilityValueLoad);
                        options.GeneralSkills = AllScaled(currentAbilities, originalAbilities, originalAbilities.Keys, 5);
                    }
                }
                catch (Exception ex) { _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "cl_script.ini", ex.Message)); }
            }

            // C. ress.ini (FreeProduction, FreeUpgrade, NoSpellCost)
            string ressPath = Path.Combine(gamePath, @"SYSTEM\ress.ini");
            if (File.Exists(ressPath))
            {
                try
                {
                    byte[] ressBytes = File.ReadAllBytes(ressPath);
                    byte[] decomp = GameLZSS.DecompressPfil(ressBytes);
                    string text = Encoding.GetEncoding(1251).GetString(decomp);

                    var mProd = Regex.Match(text, @"^FigRomInf00_Lanze_Schild\s*,.*$", RegexOptions.Multiline);
                    if (mProd.Success)
                    {
                        string[] cols = PatchText.ParseCsvLine(mProd.Value);
                        options.FreeProduction = Enumerable.Range((int)RessIndex.FigProdCostStart, (int)RessIndex.FigProdCostEnd - (int)RessIndex.FigProdCostStart + 1)
                            .All(idx => idx < cols.Length && cols[idx].Trim() == "0");
                    }

                    var mUp = Regex.Match(text, @"^.*Ger_Kampf.*$", RegexOptions.Multiline);
                    if (mUp.Success)
                    {
                        string[] cols = PatchText.ParseCsvLine(mUp.Value);
                        options.FreeUpgrade = Enumerable.Range((int)VolkresIndex.UnitUpgradeStart, (int)VolkresIndex.UnitUpgradeEnd - (int)VolkresIndex.UnitUpgradeStart + 1)
                            .All(idx => idx < cols.Length && cols[idx].Trim() == "0");
                    }

                    var mPri = Regex.Match(text, @"^FigGerPri00_Priester\s*,.*", RegexOptions.Multiline);
                    if (mPri.Success)
                    {
                        string[] cols = PatchText.ParseCsvLine(mPri.Value);
                        options.NoSpellCost = Enumerable.Range((int)RessIndex.FigPriestSpellCostStart, (int)RessIndex.FigPriestSpellCostEnd - (int)RessIndex.FigPriestSpellCostStart + 1)
                            .All(idx => idx < cols.Length && cols[idx].Trim() == "0");
                    }
                }
                catch (Exception ex) { _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "ress.ini", ex.Message)); }
            }

            // D. objdef.dau (housingCapacity20x, storageCapacity10x, fastBuildUpgradeRepair, balance)
            string objdefPath = Path.Combine(gamePath, @"SYSTEM\DATA_MP\DEFAULTS\objdef.dau");
            if (File.Exists(objdefPath))
            {
                try
                {
                    byte[] raw = File.ReadAllBytes(objdefPath);
                    byte[] decomp = GameLZSS.DecompressPfil(raw);
                    string currentObjdef = Encoding.GetEncoding(1251).GetString(decomp);
                    // 當前與原版文本各解析一次，四項偵測（Housing/Storage/FastBuild/Balance）共用
                    List<string[]> currentRows = ParseObjdefRows(currentObjdef);

                    if (backupManager.BackupFiles.TryGetValue("SYSTEM/DATA_MP/DEFAULTS/objdef.dau", out byte[]? originalObjdefBytes))
                    {
                        string originalObjdef = Encoding.GetEncoding(1251).GetString(GameLZSS.DecompressPfil(originalObjdefBytes));
                        List<string[]> originalRows = ParseObjdefRows(originalObjdef);
                        options.HousingCapacity20x = HasHousingCapacityMultiplier(currentRows, originalRows, HousingCapacityMultiplier);
                        options.StorageCapacity10x = HasStorageCapacityMultiplier(currentRows, originalRows, StorageCapacityMultiplier);
                        options.HqHp10x = HasHqHpMultiplier(currentRows, originalRows, 10);
                        options.FastBuildUpgradeRepair = HasFastBuildUpgradeRepair(currentRows, originalRows);
                        options.ProjectileArcHeight = HasProjectileWeaponScale(currentRows, originalRows, useDrad: false);
                    }

                    // 偵測是否已套用 Balance (若有任何一個兵種屬性被修改)
                    var unitRows = new Dictionary<string, string[]>(StringComparer.OrdinalIgnoreCase);
                    foreach (string[] cols in currentRows)
                    {
                        if (cols.Length < 192) continue;
                        string name = cols[52].Trim();
                        if (TroopConfig.UnitMeta.ContainsKey(name) || name == "FigZivMan00_Zivilist")
                        {
                            unitRows[name] = cols;
                        }
                    }

                    var origUnitRows = backupManager.GetBackupUnitRows();

                    // 射程 3 倍會同步擴大遠程單位的 Sirad，讓 AI 能鎖定新射程外
                    // 的目標；在平衡偵測前先識別此狀態，避免把該視野變更誤判為平衡。
                    bool range3x = false;
                    if (unitRows.TryGetValue("FigRomSch01_Bogen", out var rangeProbeCols))
                    {
                        double currentRange = BackupManager.GetUnitMaxRange(rangeProbeCols, "ranged_inf");
                        double originalRange = backupManager.GetOriginalStats("FigRomSch01_Bogen")[7];
                        range3x = currentRange > 0 && Math.Abs(currentRange - originalRange * 3.0) < 5.0;
                    }

                    bool isFileBalanced = false;
                    foreach (string key in TroopConfig.UnitMeta.Keys)
                    {
                        if (!unitRows.ContainsKey(key) || !origUnitRows.ContainsKey(key)) continue;
                        string utype = TroopConfig.UnitMeta[key].UnitType;

                        string[] cols = unitRows[key];
                        string[] origCols = origUnitRows[key];

                        double curHp = 0, origHp = 0;
                        double.TryParse(cols[(int)ObjdefIndex.Hp].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curHp);
                        double.TryParse(origCols[(int)ObjdefIndex.Hp].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origHp);

                        double curVw = 0, origVw = 0;
                        double.TryParse(cols[(int)ObjdefIndex.Vw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curVw);
                        double.TryParse(origCols[(int)ObjdefIndex.Vw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origVw);

                        double curAw = 0, origAw = 0;
                        double.TryParse(cols[(int)ObjdefIndex.Aw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curAw);
                        double.TryParse(origCols[(int)ObjdefIndex.Aw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origAw);

                        double curMoves = 0, origMoves = 0;
                        double.TryParse(cols[(int)ObjdefIndex.Moves].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curMoves);
                        double.TryParse(origCols[(int)ObjdefIndex.Moves].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origMoves);

                        double curSight = 0, origSight = 0;
                        double.TryParse(cols[(int)ObjdefIndex.Sirad].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curSight);
                        double.TryParse(origCols[(int)ObjdefIndex.Sirad].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origSight);

                        double curMeleeDmg = 0, curRangedDmg = 0;
                        BackupManager.GetMeleeAndRangedDmg(cols, utype, out curMeleeDmg, out curRangedDmg);
                        double origMeleeDmg = 0, origRangedDmg = 0;
                        BackupManager.GetMeleeAndRangedDmg(origCols, utype, out origMeleeDmg, out origRangedDmg);

                        double curMeleeRelt = 0, curRangedRelt = 0;
                        BackupManager.GetMeleeAndRangedRelt(cols, utype, out curMeleeRelt, out curRangedRelt);
                        double origMeleeRelt = 0, origRangedRelt = 0;
                        BackupManager.GetMeleeAndRangedRelt(origCols, utype, out origMeleeRelt, out origRangedRelt);

                        double curRange = BackupManager.GetUnitMaxRange(cols, utype);
                        double origRange = BackupManager.GetUnitMaxRange(origCols, utype);

                        // 排除領袖以防 LeaderGlory 的干擾，且只比對不受 Range3x/Speed2x 影響的屬性 (Hp, Vw, Aw, Sight)
                        if (TroopConfig.UnitMeta[key].Tier != "leader")
                        {
                            bool isPriest = utype == "priest";
                            bool hasExpandedRangeSight = range3x && TroopConfig.SupportsRangedRange3x(utype);
                            bool hasDiff = Math.Abs(curHp - origHp) > 0.01 ||
                                           Math.Abs(curVw - origVw) > 0.01 ||
                                           Math.Abs(curAw - origAw) > 0.01 ||
                                           (!isPriest && !hasExpandedRangeSight && Math.Abs(curSight - origSight) > 0.01);

                            if (hasDiff)
                            {
                                isFileBalanced = true;
                            }
                        }
                    }
                    options.Balance = isFileBalanced;
                    options.LeaderGlory = DetectLeaderGlory(currentRows, origUnitRows);

                    // 偵測遠程單位射程 3 倍與單位移動速度提升 2 倍
                    bool speed2x = false;

                    // 使用羅馬輕裝步兵 FigRomInf00_Lanze_Schild 偵測速度 2 倍
                    if (unitRows.TryGetValue("FigRomInf00_Lanze_Schild", out var testMovesCols))
                    {
                        double curMoves = 0;
                        double.TryParse(testMovesCols[(int)ObjdefIndex.Moves].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curMoves);
                        double expectedBaseMoves = backupManager.GetBaseStatsForUnit("FigRomInf00_Lanze_Schild", options)[4] / 2.0;
                        if (curMoves > 0 && Math.Abs(curMoves - expectedBaseMoves * 2.0) < 0.05)
                        {
                            speed2x = true;
                        }
                    }

                    // 使用羅馬弓箭手 FigRomSch01_Bogen 偵測射程 3 倍
                    if (unitRows.TryGetValue("FigRomSch01_Bogen", out var testRangeCols))
                    {
                        double curRange = BackupManager.GetUnitMaxRange(testRangeCols, "ranged_inf");
                        double expectedBaseRange = backupManager.GetBaseStatsForUnit("FigRomSch01_Bogen", options)[7];
                        if (curRange > 0 && Math.Abs(curRange - expectedBaseRange * 3.0) < 5.0)
                        {
                            range3x = true;
                        }
                    }

                    options.UnitMovementSpeed2x = speed2x;
                    options.RangedRange3x = range3x;

                    // 使用塞爾特祭司 FigKelPri00_Priester 偵測法師施法距離
                    bool spellEntireMap = false;
                    if (unitRows.TryGetValue("FigKelPri00_Priester", out var testSpellCols) &&
                        origUnitRows.ContainsKey("FigKelPri00_Priester"))
                    {
                        double curRange = BackupManager.GetUnitMaxRange(testSpellCols, "priest");
                        if (curRange > 0)
                        {
                            if (Math.Abs(curRange - 30000.0) < 100.0)
                            {
                                spellEntireMap = true;
                            }
                        }
                    }
                    options.SpellEntireMap = spellEntireMap;
                }
                catch (Exception ex) { _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "objdef.dau", ex.Message)); }
            }

            // E. team.dat (maxPopulation + romanEndless)
            // 由原版備份重合成四種合法組合，避免只變更玩家陣營時誤報人口上限。
            try
            {
                bool maxPop = false;
                bool romanEndless = false;
                foreach (var kvp in backupManager.BackupFiles)
                {
                    if (kvp.Key.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) && kvp.Key.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase))
                    {
                        string path = Path.Combine(gamePath, kvp.Key.Replace('/', '\\'));
                        if (File.Exists(path))
                        {
                            byte[] currentBytes = File.ReadAllBytes(path);
                            bool isEndless = kvp.Key.StartsWith("MAPS/ENDL_", StringComparison.OrdinalIgnoreCase);
                            if (TryDetectTeamDatOptions(kvp.Value, currentBytes, isEndless, out bool fileMaxPop, out bool fileRoman))
                            {
                                maxPop |= fileMaxPop;
                                romanEndless |= fileRoman;
                            }
                            else
                            {
                                // 保留舊版對未知 team.dat 修改的保守相容行為。
                                maxPop = true;
                                _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), kvp.Key, "unknown team.dat state"));
                            }
                        }
                    }
                }
                options.MaxPopulation = maxPop;
                if (romanExeState == ExeRomanEndlessPatchState.Unknown)
                {
                    options.RomanEndless = romanEndless;
                    _logger.Log(Loc.Get("SvcLogRomanEndlessExeUnknown"));
                }
                else
                {
                    bool exeEnabled = romanExeState == ExeRomanEndlessPatchState.Patched;
                    options.RomanEndless = exeEnabled;
                    if (exeEnabled != romanEndless)
                        _logger.Log(Loc.Get("SvcLogRomanEndlessMismatch"));
                }
            }
            catch (Exception ex) { _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "team.dat", ex.Message)); }

            // F. Endless AI (M1..M5)
            try
            {
                var orchestrator = new EndlessAiOrchestrator();
                foreach (var module in orchestrator.UserModules)
                {
                    var aiState = orchestrator.DetectModule(gamePath, module);
                    options.EndlessAiModules[module.Id] = (aiState == PatchState.Ultimate);
                }
            }
            catch (Exception ex) { _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "Endless AI", ex.Message)); }

            // G. toEnglish
            try
            {
                new LanguagePackFeature(_logger).TryGetLanguageOverlayState(gamePath, out bool langEng);
                options.ToEnglish = langEng;
            }
            catch (Exception ex) { _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "ToEng overlay", ex.Message)); }

            // H. DgVoodoo
            try
            {
                options.DgVoodoo = new DgVoodooFeature(_logger).IsInstalled(gamePath);
            }
            catch (Exception ex) { _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "dgVoodoo2", ex.Message)); }

            // I. FoodHealing10x
            try
            {
                FoodHealingFeature.TryDetect(gamePath, out bool foodHealing);
                options.FoodHealing10x = foodHealing;
            }
            catch (Exception ex) { _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "food healing", ex.Message)); }

            return options;
        }

        internal static bool TryDetectTeamDatOptions(byte[] original, byte[] current, bool isEndless,
            out bool maxPopulation, out bool romanPlayer)
        {
            (bool MaxPopulation, bool RomanPlayer)[] combinations =
            {
                (false, false),
                (true, false),
                (false, true),
                (true, true),
            };

            foreach (var combination in combinations)
            {
                bool effectiveRoman = combination.RomanPlayer && isEndless;
                byte[] candidate = TeamDatPatcher.GetPatchedBytes(original,
                    new TeamDatOptions(combination.MaxPopulation, RomanPlayer: effectiveRoman));
                if (!current.SequenceEqual(candidate)) continue;

                maxPopulation = combination.MaxPopulation;
                romanPlayer = effectiveRoman;
                return true;
            }

            maxPopulation = false;
            romanPlayer = false;
            return false;
        }

        // --- cl_script.ini 法術/技能強化偵測輔助函數 ---

        /// <summary>以行錨定 regex 解析 ValueN 行為 {key}_{faction}_{spell|ability} → 數值 的字典。</summary>
        private static Dictionary<string, double> ReadKeyedValues(string text, Regex regex)
        {
            var values = new Dictionary<string, double>(StringComparer.OrdinalIgnoreCase);
            foreach (Match m in regex.Matches(text))
            {
                if (double.TryParse(m.Groups[4].Value.Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double value))
                    values[$"{m.Groups[1].Value.Trim()}_{m.Groups[2].Value.Trim()}_{m.Groups[3].Value.Trim()}"] = value;
            }
            return values;
        }

        private static Dictionary<string, double> ReadRadiusValues(string text)
        {
            var values = new Dictionary<string, double>(StringComparer.OrdinalIgnoreCase);
            foreach (Match m in RegexRadiusLoad.Matches(text))
            {
                if (double.TryParse(m.Groups[3].Value.Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double value))
                    values[$"Radius_{m.Groups[1].Value.Trim()}_{m.Groups[2].Value.Trim()}"] = value;
            }
            return values;
        }

        /// <summary>指定鍵的當前值是否全部等於原值 × multiplier（與 patcher 相同的 (int) 截斷）；至少須有一鍵可比對。</summary>
        private static bool AllScaled(Dictionary<string, double> current, Dictionary<string, double> original, IEnumerable<string> keys, int multiplier)
        {
            bool any = false;
            foreach (string key in keys)
            {
                if (!original.TryGetValue(key, out double orig) || orig <= 0) continue;
                int expected = (int)(orig * multiplier);
                if (expected == (int)orig) continue; // 倍率後不變（無法區分），跳過
                if (!current.TryGetValue(key, out double cur) || (int)cur != expected) return false;
                any = true;
            }
            return any;
        }

        private static bool RadiusScaledByThree(Dictionary<string, double> current, Dictionary<string, double> original)
        {
            bool any = false;
            foreach (string key in original.Keys)
            {
                double baseline = original[key];
                if (baseline <= 0) continue;
                if (!current.TryGetValue(key, out double value)) return false;
                double ratio = value / baseline;
                // Balance/custom radius may contribute its own baseline (the shipped balance is 2.5x).
                if (Math.Abs(ratio - 3.0) > 0.05 && Math.Abs(ratio - 7.5) > 0.1) return false;
                any = true;
            }
            return any;
        }

        /// <summary>復活術強化偵測：KEL Spell3 的 Value/Value2 是否已被寫為 100（且原值非 100，否則無法區分）。</summary>
        private static bool DetectResurrection(Dictionary<string, double> current, Dictionary<string, double> original)
        {
            bool any = false;
            foreach (string key in new[] { "Value_KEL_Spell3", "Value2_KEL_Spell3" })
            {
                if (!original.TryGetValue(key, out double orig) || (int)orig == 100) continue;
                if (!current.TryGetValue(key, out double cur) || (int)cur != 100) return false;
                any = true;
            }
            return any;
        }

        /// <summary>首領榮譽偵測：四族首領列的成長/光環欄位是否全部等於備份原值 × 5。</summary>
        private static bool DetectLeaderGlory(List<string[]> currentRows, Dictionary<string, string[]> originalRows)
        {
            bool any = false;
            foreach (string leader in ObjdefPatcher.GloryLeaders)
            {
                if (!originalRows.TryGetValue(leader, out string[]? orig)) continue;
                string[]? cur = currentRows.FirstOrDefault(r => r.Length > 52 && r[52].Trim().Equals(leader, StringComparison.OrdinalIgnoreCase));
                if (cur == null) continue;
                foreach (int index in ObjdefPatcher.GloryColumns)
                {
                    if (index >= orig.Length || index >= cur.Length) continue;
                    if (!double.TryParse(orig[index].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double origValue) || origValue <= 0) continue;
                    if (!double.TryParse(cur[index].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double curValue)) return false;
                    if (Math.Abs(curValue - origValue * ObjdefPatcher.GloryMultiplier) > 0.01) return false;
                    any = true;
                }
            }
            return any;
        }

        // --- Objdef dau 偵測細部輔助函數 ---

        /// <summary>把 objdef 文本解析為資料列（長度 >= 100 的行），供各偵測共用、避免重複 split 與解析。</summary>
        private static List<string[]> ParseObjdefRows(string content)
        {
            var rows = new List<string[]>();
            foreach (string line in content.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None))
            {
                if (line.Length < 100) continue;
                rows.Add(PatchText.ParseCsvLine(line));
            }
            return rows;
        }

        private static bool HasHousingCapacityMultiplier(List<string[]> currentRows, List<string[]> originalRows, int multiplier)
        {
            var currentValues = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
            foreach (string[] cols in currentRows)
            {
                if (cols.Length <= (int)ObjdefIndex.HousingCapacity || cols.Length <= (int)ObjdefIndex.Name) continue;
                if (int.TryParse(cols[(int)ObjdefIndex.HousingCapacity].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int value))
                {
                    currentValues[cols[(int)ObjdefIndex.Name].Trim()] = value;
                }
            }

            bool foundHousing = false;
            foreach (string[] cols in originalRows)
            {
                if (cols.Length <= (int)ObjdefIndex.HousingCapacity || cols.Length <= (int)ObjdefIndex.Name) continue;
                if (!int.TryParse(cols[(int)ObjdefIndex.HousingCapacity].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int originalValue) || originalValue <= 0) continue;

                foundHousing = true;
                string name = cols[(int)ObjdefIndex.Name].Trim();
                if (!currentValues.TryGetValue(name, out int currentValue) || currentValue != checked(originalValue * multiplier))
                {
                    return false;
                }
            }
            return foundHousing;
        }

        private static bool HasStorageCapacityMultiplier(List<string[]> currentRows, List<string[]> originalRows, int multiplier)
        {
            var currentValues = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
            foreach (string[] cols in currentRows)
            {
                if (cols.Length <= (int)ObjdefIndex.StorageCapacity || cols.Length <= (int)ObjdefIndex.Name) continue;
                if (int.TryParse(cols[(int)ObjdefIndex.StorageCapacity].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int value))
                {
                    currentValues[cols[(int)ObjdefIndex.Name].Trim()] = value;
                }
            }

            bool foundStorage = false;
            foreach (string[] cols in originalRows)
            {
                if (cols.Length <= (int)ObjdefIndex.StorageCapacity || cols.Length <= (int)ObjdefIndex.Name) continue;
                string name = cols[(int)ObjdefIndex.Name].Trim();
                if (!name.StartsWith("Bau") || !(name.Contains("Hau") || name.Contains("Lag"))) continue;
                if (!int.TryParse(cols[(int)ObjdefIndex.StorageCapacity].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int originalValue) || originalValue <= 0) continue;

                foundStorage = true;
                if (!currentValues.TryGetValue(name, out int currentValue) || currentValue != checked(originalValue * multiplier))
                {
                    return false;
                }
            }
            return foundStorage;
        }

        private static bool HasHqHpMultiplier(List<string[]> currentRows, List<string[]> originalRows, int multiplier)
        {
            var currentValues = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
            foreach (string[] cols in currentRows)
            {
                if (cols.Length <= (int)ObjdefIndex.Hp || cols.Length <= (int)ObjdefIndex.Name) continue;
                if (int.TryParse(cols[(int)ObjdefIndex.Hp].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int value))
                {
                    currentValues[cols[(int)ObjdefIndex.Name].Trim()] = value;
                }
            }

            bool foundHq = false;
            foreach (string[] cols in originalRows)
            {
                if (cols.Length <= (int)ObjdefIndex.Hp || cols.Length <= (int)ObjdefIndex.Name) continue;
                string name = cols[(int)ObjdefIndex.Name].Trim();
                if (!name.StartsWith("Bau") || !name.Contains("Hau")) continue;
                if (!int.TryParse(cols[(int)ObjdefIndex.Hp].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int originalValue) || originalValue <= 0) continue;

                foundHq = true;
                if (!currentValues.TryGetValue(name, out int currentValue) || currentValue != checked(originalValue * multiplier))
                {
                    return false;
                }
            }
            return foundHq;
        }

        /// <summary>拋射武器縮放偵測：所有「akti=1 且 emit&gt;0」的武器欄位，是否全部等於原值 × 對應倍率。
        /// useDrad=false 比對 w*_emit ×ArcEmitMultiplier（拋射彈道增高）；useDrad=true 比對 w*_drad ×AccuracyDradMultiplier（遠程命中強化）。
        /// 與 ObjdefPatcher.PatchProjectileWeapons 的寫入邏輯對稱。</summary>
        private static bool HasProjectileWeaponScale(List<string[]> currentRows, List<string[]> originalRows, bool useDrad)
        {
            var currentByName = new Dictionary<string, string[]>(StringComparer.OrdinalIgnoreCase);
            foreach (string[] cols in currentRows)
            {
                if (cols.Length > (int)ObjdefIndex.Name) currentByName[cols[(int)ObjdefIndex.Name].Trim()] = cols;
            }

            bool any = false;
            foreach (string[] orig in originalRows)
            {
                if (orig.Length <= (int)ObjdefIndex.Name) continue;
                if (!currentByName.TryGetValue(orig[(int)ObjdefIndex.Name].Trim(), out string[]? cur)) continue;
                for (int w = 1; w <= 8; w++)
                {
                    int active = (int)ObjdefIndex.Weapon1Akti + (w - 1) * 8;
                    int emit = (int)ObjdefIndex.Weapon1Emit + (w - 1) * 8;
                    int drad = (int)ObjdefIndex.Weapon1Drad + (w - 1) * 2;
                    int target = useDrad ? drad : emit;
                    if (emit >= orig.Length || target >= orig.Length || target >= cur.Length) continue;
                    if (orig[active].Trim() != "1") continue;
                    if (!long.TryParse(orig[emit].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out long emitValue) || emitValue <= 0) continue;
                    if (!long.TryParse(orig[target].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out long origValue) || origValue <= 0) continue;
                    long expected = useDrad
                        ? origValue * ObjdefPatcher.AccuracyDradMultiplier
                        : (long)Math.Round(origValue * ObjdefPatcher.ArcEmitMultiplier);
                    if (expected == origValue) continue; // 倍率後不變，無法區分
                    if (!long.TryParse(cur[target].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out long curValue) || curValue != expected) return false;
                    any = true;
                }
            }
            return any;
        }

        private static bool HasFastBuildUpgradeRepair(List<string[]> currentRows, List<string[]> originalRows)
        {
            var currentBuildValues = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
            var currentUpgValues = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);

            foreach (string[] cols in currentRows)
            {
                if (cols.Length < 192) continue;
                string name = cols[52].Trim();
                if (!name.StartsWith("Bau")) continue;

                if (int.TryParse(cols[73].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int buildVal))
                {
                    currentBuildValues[name] = buildVal;
                }
                if (int.TryParse(cols[74].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int upgVal))
                {
                    currentUpgValues[name] = upgVal;
                }
            }

            bool foundBuilding = false;
            foreach (string[] cols in originalRows)
            {
                if (cols.Length < 192) continue;
                string name = cols[52].Trim();
                if (!name.StartsWith("Bau")) continue;

                bool hasBuild = int.TryParse(cols[73].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int origBuildVal) && origBuildVal > 0;
                bool hasUpg = int.TryParse(cols[74].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int origUpgVal) && origUpgVal > 0;

                if (!hasBuild && !hasUpg) continue;

                foundBuilding = true;

                if (hasBuild)
                {
                    int expectedBuild = Math.Max(1, origBuildVal / 10);
                    if (!currentBuildValues.TryGetValue(name, out int curBuild) || curBuild != expectedBuild)
                    {
                        return false;
                    }
                }

                if (hasUpg)
                {
                    int expectedUpg = Math.Max(1, origUpgVal / 10);
                    if (!currentUpgValues.TryGetValue(name, out int curUpg) || curUpg != expectedUpg)
                    {
                        return false;
                    }
                }
            }
            return foundBuilding;
        }

}
