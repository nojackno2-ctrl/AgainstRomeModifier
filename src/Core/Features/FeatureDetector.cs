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
    private const int HousingCapacityMultiplier = 20;
    private const int StorageCapacityMultiplier = 10;
    private readonly ILogger _logger;
    internal FeatureDetector(ILogger logger) => _logger = logger;
        internal PatchProfile Detect(string gamePath, BackupManager backupManager)
        {
            var options = new PatchProfile();

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

                    int speed = ExePatchModel.GetGameSpeedMultiplier(exeBytes);
                    options.GameSpeed = speed;
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

                        bool hasDiff = Math.Abs(curHp - origHp) > 0.01 ||
                                       Math.Abs(curVw - origVw) > 0.01 ||
                                       Math.Abs(curAw - origAw) > 0.01 ||
                                       Math.Abs(curMoves - origMoves) > 0.01 ||
                                       Math.Abs(curSight - origSight) > 0.01 ||
                                       Math.Abs(curMeleeDmg - origMeleeDmg) > 0.01 ||
                                       Math.Abs(curRangedDmg - origRangedDmg) > 0.01 ||
                                       Math.Abs(curMeleeRelt - origMeleeRelt) > 0.01 ||
                                       Math.Abs(curRangedRelt - origRangedRelt) > 0.01 ||
                                       Math.Abs(curRange - origRange) > 0.01;

                        if (hasDiff)
                        {
                            isFileBalanced = true;
                            break;
                        }
                    }
                    options.Balance = isFileBalanced;
                }
                catch (Exception ex) { _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "objdef.dau", ex.Message)); }
            }

            // E. team.dat (maxPopulation)
            // 比較當前地圖的 team.dat 是否與備份檔案不一致（若有不一致，代表已套用修改）
            try
            {
                bool maxPop = false;
                foreach (var kvp in backupManager.BackupFiles)
                {
                    if (kvp.Key.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) && kvp.Key.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase))
                    {
                        string path = Path.Combine(gamePath, kvp.Key.Replace('/', '\\'));
                        if (File.Exists(path))
                        {
                            byte[] currentBytes = File.ReadAllBytes(path);
                            if (!currentBytes.SequenceEqual(kvp.Value))
                            {
                                maxPop = true;
                                break;
                            }
                        }
                    }
                }
                options.MaxPopulation = maxPop;
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
