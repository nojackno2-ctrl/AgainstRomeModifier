using System;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using System.Collections.Generic;
using System.Globalization;
using System.Security.Cryptography;
using AgainstRomeModifier.Core.Patches;

namespace AgainstRomeModifier.Core.Services
{
    public class PatchEngine
    {
        private static readonly object LogLock = new object();
        private const string LanguageBackupDirectoryName = ".against-rome-modifier-language-backup";
        private const string LanguageBackupManifestName = "manifest.json";

        private static readonly Regex RegexRadiusPatch = new Regex(@"^Radius\s*=\s*([A-Z]{3})\s*,\s*(Spell\d+)\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexCiviPatch = new Regex(@"^CiviDelay\s*=\s*([A-Z]{3})\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleLostMemPatch = new Regex(@"^(MoralsDecLostMem\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleFleePatch = new Regex(@"^(MoralsDecFlee\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleOverPopPatch = new Regex(@"^(MoralsDecOverPop\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleIncIdlePatch = new Regex(@"^(MoralsIncIdle\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleKey = new Regex(@"^(MoralsDecLostMem|MoralsDecFlee|MoralsDecOverPop|MoralsIncIdle)\s*=\s*([A-Z]{3})\s*,", RegexOptions.Compiled);
        private static readonly Regex RegexSpellValuePatch = new Regex(@"^(Value\d*)\s*=\s*([A-Z]{3})\s*,\s*(Spell\d+)\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexSpecialAbilityValuePatch = new Regex(@"^(Value\d*)\s*=\s*([A-Z]{3})\s*,\s*(SAbility\d+)\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
        
        // Regex for status detection
        private static readonly Regex RegexSpellLoad = new Regex(@"Radius\s*=\s*(?:HUN|KEL|GER)\s*,\s*Spell\d+\s*,\s*(\d+)", RegexOptions.Compiled);
        private static readonly Regex RegexCiviLoad = new Regex(@"CiviDelay\s*=\s*([A-Z]{3})\s*,\s*(\d+)", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleLostMemLoad = new Regex(@"MoralsDecLostMem\s*=\s*GER\s*,\s*(\d+)", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleFleeLoad = new Regex(@"MoralsDecFlee\s*=\s*GER\s*,\s*(\d+)", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleOverPopLoad = new Regex(@"MoralsDecOverPop\s*=\s*GER\s*,\s*(\d+)", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleIdleLoad = new Regex(@"MoralsIncIdle\s*=\s*GER\s*,\s*(\d+)", RegexOptions.Compiled);

        private const int HousingCapacityMultiplier = 20;
        private const int StorageCapacityMultiplier = 10;
        private const int FoodHealAmountOriginal = 1;
        private const int FoodHealAmountUltimate = 10;

        private const string RetiredLeaderGloryScriptSha256 = "42B07605D20C81A93BE983252DF1CB34104EB7AE9F8456AB31A842D4CD9232DD";
        private const string VanillaLeaderScriptSha256 = "778A6E01B99664136AC0420B9F48212C41A5D6297A9952EA9D7D3A5D4851272C";

        private static readonly (string File, int AddLpSymbolIndex)[] FoodHealAmountSites = new[] {
            ("ak_anfuehrer", 87),
            ("ak_artillerie", 69),
            ("ak_geisterreiter", 68),
            ("ak_kampfverband", 89),
            ("ak_krieger", 73),
            ("ak_kundschafterwolf", 77),
            ("ak_landtier", 72),
            ("ak_packpferd", 57),
            ("ak_priester", 92),
            ("ak_verbandswolf", 68),
            ("ak_zivilist", 83),
            ("ak_zivilverband", 87),
        };

        private const string DgVoodooEmbeddedVersion = "v2.87.3";
        private const string DgVoodooMarkerFileName = ".against-rome-modifier-dgvoodoo.json";
        private static readonly string[] DgVoodooManagedFiles = {
            "D3D8.dll",
            "DDraw.dll",
            "dgVoodooCpl.exe",
            "dgVoodoo.conf"
        };
        private static readonly Dictionary<string, string> DgVoodooResourceNames = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase) {
            { "D3D8.dll", "dgVoodoo2.D3D8.dll" },
            { "DDraw.dll", "dgVoodoo2.DDraw.dll" },
            { "dgVoodooCpl.exe", "dgVoodoo2.dgVoodooCpl.exe" },
            { "dgVoodoo.conf", "dgVoodoo2.dgVoodoo.conf" }
        };

        private sealed class DgVoodooManifest
        {
            public string Version { get; set; } = "";
            public Dictionary<string, string> Files { get; set; } = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        }

        private sealed class LanguageBackupManifest
        {
            public List<string> ExistingFiles { get; set; } = new List<string>();
            public List<string> MissingFiles { get; set; } = new List<string>();
        }

        private readonly ILogger _logger;

        public PatchEngine(ILogger logger)
        {
            _logger = logger;
        }

        // --- 核心安全寫入工具 ---

        public void SafeWriteAllBytes(string dest, byte[] bytes, FileRollbackScope? rollback = null)
        {
            int maxRetries = 3;
            int delayMs = 500;
            string? dir = Path.GetDirectoryName(dest);
            if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir))
            {
                Directory.CreateDirectory(dir);
            }

            for (int i = 0; i < maxRetries; i++)
            {
                string tempFile = Path.Combine(dir ?? AppContext.BaseDirectory, Path.GetFileName(dest) + "." + Guid.NewGuid().ToString("N") + ".tmp");
                try
                {
                    rollback?.TrackFile(dest);
                    File.WriteAllBytes(tempFile, bytes);

                    if (File.Exists(dest))
                    {
                        File.SetAttributes(dest, FileAttributes.Normal);
                        File.Replace(tempFile, dest, null, true);
                    }
                    else
                    {
                        File.Move(tempFile, dest);
                    }
                    return;
                }
                catch (IOException ioEx)
                {
                    try
                    {
                        if (File.Exists(tempFile))
                        {
                            File.SetAttributes(tempFile, FileAttributes.Normal);
                            File.Delete(tempFile);
                        }
                    }
                    catch { }

                    if (i == maxRetries - 1)
                    {
                        throw new Exception(string.Format("寫入檔案失敗，檔案可能被佔用或權限不足：{0}。錯誤訊息：{1}", dest, ioEx.Message), ioEx);
                    }
                    System.Threading.Thread.Sleep(delayMs);
                }
                catch (UnauthorizedAccessException accessEx)
                {
                    try
                    {
                        if (File.Exists(tempFile))
                        {
                            File.SetAttributes(tempFile, FileAttributes.Normal);
                            File.Delete(tempFile);
                        }
                    }
                    catch { }

                    if (i == maxRetries - 1)
                    {
                        throw new Exception(string.Format("寫入檔案失敗，檔案可能被佔用或權限不足：{0}。錯誤訊息：{1}", dest, accessEx.Message), accessEx);
                    }
                    System.Threading.Thread.Sleep(delayMs);
                }
            }
        }

        public void SafeCopyFile(string src, string dest, bool overwrite, FileRollbackScope? rollback = null)
        {
            if (!overwrite && File.Exists(dest))
            {
                throw new IOException("目標檔案已存在: " + dest);
            }
            SafeWriteAllBytes(dest, File.ReadAllBytes(src), rollback);
        }

        public void SafeDeleteFile(string path, FileRollbackScope? rollback = null)
        {
            if (!File.Exists(path)) return;
            rollback?.TrackFile(path);
            File.SetAttributes(path, FileAttributes.Normal);
            File.Delete(path);
        }

        // --- 補丁套用與還原引擎端點 ---

        public void ApplyPatches(string gamePath, PatchOptions options, BackupManager backupManager, FileRollbackScope rollback)
        {
            // 總是先在同個交易中執行原版恢復
            _logger.Log("正在套用前將相關檔案復原為乾淨狀態，以清除殘留修改...");
            RestoreOriginalFilesInternal(gamePath, backupManager, rollback);

            // 1. Dry Run 階段：在記憶體中生成所有補丁 byte[] 並驗證
            var patchedFiles = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);

            // A. Against_Rome.exe
            string exePath = Path.Combine(gamePath, @"Against_Rome.exe");
            byte[] exeBytes = File.ReadAllBytes(exePath);
            bool exeModified = false;
            ApplyExePatch(exeBytes, options.FocusLoss, options.VillageBuildRange, options.NoSpellAltar, options.GameSpeed, ref exeModified);
            if (exeModified)
            {
                patchedFiles[exePath] = exeBytes;
            }

            // B. cl_script.ini
            byte[] clBytes = GetPatchedClScriptBytes(gamePath, backupManager, options.FastCiviProduction, options.InfiniteMorale, options.Balance);
            patchedFiles[Path.Combine(gamePath, @"SYSTEM\cl_script.ini")] = clBytes;

            // C. cl_epara.ini — 一律還原為原版備份
            patchedFiles[Path.Combine(gamePath, @"SYSTEM\cl_epara.ini")] = backupManager.GetBackupBytes("SYSTEM/cl_epara.ini");

            // D. cl_scint.ini — 一律還原為原版備份
            patchedFiles[Path.Combine(gamePath, @"SYSTEM\CLAK\cl_scint.ini")] = backupManager.GetBackupBytes("SYSTEM/CLAK/cl_scint.ini");

            // E. ress.ini
            byte[] ressBytes = GetPatchedRessBytes(backupManager, options.FreeProduction, options.FreeUpgrade, options.NoSpellCost);
            patchedFiles[Path.Combine(gamePath, @"SYSTEM\ress.ini")] = ressBytes;

            // F. objdef.dau
            byte[] objdefBytes = GetPatchedObjdefBytes(backupManager, options);
            patchedFiles[Path.Combine(gamePath, @"SYSTEM\DATA_MP\DEFAULTS\objdef.dau")] = objdefBytes;

            // G. team.dat
            var teamDatPatches = GetPatchedTeamDatBytes(backupManager, options.MaxPopulation);
            foreach (var kvp in teamDatPatches)
            {
                patchedFiles[Path.Combine(gamePath, kvp.Key.Replace('/', '\\'))] = kvp.Value;
            }

            // H. Endless AI 模組
            var orchestrator = new EndlessAiOrchestrator();
            for (int i = 0; i < orchestrator.UserModules.Count && i < options.EndlessAiModules.Length; i++)
            {
                orchestrator.ApplyModule(gamePath, orchestrator.UserModules[i], options.EndlessAiModules[i]);
            }
            orchestrator.ApplyMandatoryRepair(gamePath);

            // Dry Run 順利結束，進行實體檔案寫入與交易範圍
            foreach (var kvp in patchedFiles)
            {
                SafeWriteAllBytes(kvp.Key, kvp.Value, rollback);
            }
            orchestrator.SaveAll(gamePath, rollback);

            // 其它補丁
            ApplyLanguagePatch(gamePath, options.ToEnglish, rollback);
            ApplyDgVoodooPatch(gamePath, options.DgVoodoo, rollback);
            ApplyFoodHealingAmountPatch(gamePath, options.FoodHealing10x, backupManager, rollback);
        }

        public void RestoreOriginalFiles(string gamePath, BackupManager backupManager, FileRollbackScope rollback)
        {
            RestoreOriginalFilesInternal(gamePath, backupManager, rollback);
        }

        public void RestoreStatsOnly(string gamePath, BackupManager backupManager, FileRollbackScope rollback)
        {
            RestoreStatsOnlyInternal(gamePath, backupManager, rollback);
            ApplyFoodHealingAmountPatch(gamePath, false, backupManager, rollback);
        }

        public void RestoreCompatOnly(string gamePath, BackupManager backupManager, FileRollbackScope rollback)
        {
            var patchedFiles = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
            string exePath = Path.Combine(gamePath, @"Against_Rome.exe");
            byte[] exeBytes = File.ReadAllBytes(exePath);
            bool exeModified = false;
            ApplyExePatch(exeBytes, false, false, false, 1, ref exeModified);
            if (exeModified)
            {
                patchedFiles[exePath] = exeBytes;
            }

            var orchestrator = new EndlessAiOrchestrator();
            foreach (var module in orchestrator.UserModules)
            {
                orchestrator.ApplyModule(gamePath, module, false);
            }
            orchestrator.ApplyMandatoryRepair(gamePath);

            foreach (var kvp in patchedFiles)
            {
                SafeWriteAllBytes(kvp.Key, kvp.Value, rollback);
            }
            orchestrator.SaveAll(gamePath, rollback);
            ApplyDgVoodooPatch(gamePath, false, rollback);
        }

        public void RestoreLanguageOnly(string gamePath, FileRollbackScope rollback)
        {
            ApplyLanguagePatch(gamePath, false, rollback);
        }

        private void RestoreOriginalFilesInternal(string gamePath, BackupManager backupManager, FileRollbackScope rollback)
        {
            var patchedFiles = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);

            string exePath = Path.Combine(gamePath, @"Against_Rome.exe");
            byte[] exeBytes = File.ReadAllBytes(exePath);
            bool exeModified = false;
            ApplyExePatch(exeBytes, false, false, false, 1, ref exeModified);
            if (exeModified)
            {
                patchedFiles[exePath] = exeBytes;
            }

            var orchestrator = new EndlessAiOrchestrator();
            foreach (var module in orchestrator.UserModules)
            {
                orchestrator.ApplyModule(gamePath, module, false);
            }
            orchestrator.ApplyMandatoryRepair(gamePath);

            RestoreStatsOnlyInternal(gamePath, backupManager, rollback);

            foreach (var kvp in patchedFiles)
            {
                SafeWriteAllBytes(kvp.Key, kvp.Value, rollback);
            }
            orchestrator.SaveAll(gamePath, rollback);
            ApplyFoodHealingAmountPatch(gamePath, false, backupManager, rollback);
            ApplyLanguagePatch(gamePath, false, rollback);
            ApplyDgVoodooPatch(gamePath, false, rollback);
        }

        private void RestoreStatsOnlyInternal(string gamePath, BackupManager backupManager, FileRollbackScope? rollback = null)
        {
            RestoreMemoryFile(backupManager, "SYSTEM/cl_script.ini", Path.Combine(gamePath, @"SYSTEM\cl_script.ini"), rollback);
            RestoreMemoryFile(backupManager, "SYSTEM/cl_epara.ini", Path.Combine(gamePath, @"SYSTEM\cl_epara.ini"), rollback);
            RestoreMemoryFile(backupManager, "SYSTEM/CLAK/cl_scint.ini", Path.Combine(gamePath, @"SYSTEM\CLAK\cl_scint.ini"), rollback);
            RestoreMemoryFile(backupManager, "SYSTEM/ress.ini", Path.Combine(gamePath, @"SYSTEM\ress.ini"), rollback);
            RestoreMemoryFile(backupManager, "SYSTEM/DATA_MP/DEFAULTS/objdef.dau", Path.Combine(gamePath, @"SYSTEM\DATA_MP\DEFAULTS\objdef.dau"), rollback);
            foreach (var kvp in backupManager.BackupFiles)
            {
                if (kvp.Key.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) && kvp.Key.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase))
                {
                    string destPath = Path.Combine(gamePath, kvp.Key.Replace('/', '\\'));
                    byte[] patchedBytes = TeamDatPatcher.GetPatchedBytes(kvp.Value, new TeamDatOptions(false));
                    SafeWriteAllBytes(destPath, patchedBytes, rollback);
                    _logger.Log(string.Format("已還原人口上限: {0}", destPath));
                }
            }
        }

        private void RestoreMemoryFile(BackupManager backupManager, string key, string dest, FileRollbackScope? rollback = null)
        {
            if (backupManager.BackupFiles.TryGetValue(key, out byte[]? fileBytes))
            {
                SafeWriteAllBytes(dest, fileBytes!, rollback);
                _logger.Log(string.Format("已還原: {0}", dest));
            }
        }

        // --- 補丁生成與檔案偵測邏輯 ---

        private void ApplyExePatch(byte[] exeBytes, bool focusLossChecked, bool villageBuildRangeChecked, bool noSpellAltarChecked, int gameSpeedMultiplier, ref bool exeModified)
        {
            ExePatchState state = ExePatchModel.GetExePatchState(exeBytes);
            if (state == ExePatchState.Unknown)
            {
                throw new Exception("Against_Rome.exe 版本或位元組特徵不符合預期，已停止相容性補丁以避免覆蓋未知版本。");
            }

            IReadOnlyList<ExeWriteOp> focusOps = ExePatchModel.PlanFocus(focusLossChecked, state);
            if (focusOps.Count > 0)
            {
                ExePatchModel.Apply(exeBytes, focusOps);
                exeModified = true;
            }
            _logger.Log(focusLossChecked ? "已套用視窗失去焦點不暫停補丁。" : "已還原視窗失去焦點暫停設定。");

            RestoreLegacyVillageBuildRangePatch(exeBytes, ref exeModified);
            if (villageBuildRangeChecked &&
                ExePatchModel.GetVillageBuildRangePatchState(exeBytes) != ExeVillageRangePatchState.Original)
            {
                throw new InvalidOperationException("遊戲主程式不支援村莊建造半徑擴張補丁（特徵碼不符）。");
            }
            ApplyVillageSetterRangePatch(exeBytes, villageBuildRangeChecked, ref exeModified);

            ApplySpellAltarPatch(exeBytes, noSpellAltarChecked, ref exeModified);

            ApplyGameSpeedPatch(exeBytes, gameSpeedMultiplier, ref exeModified);
        }

        private void RestoreLegacyVillageBuildRangePatch(byte[] exeBytes, ref bool exeModified)
        {
            ExeVillageRangePatchState state = ExePatchModel.GetVillageBuildRangePatchState(exeBytes);
            if (state == ExeVillageRangePatchState.Unknown)
            {
                _logger.Log("無法還原舊版村莊建造半徑補丁：主程式特徵碼不符合。");
                return;
            }

            IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanVillageRangeRestore(state);
            if (ops.Count > 0)
            {
                ExePatchModel.Apply(exeBytes, ops);
                exeModified = true;
                _logger.Log("已移除舊版村莊建造半徑補丁。");
            }
        }

        private void ApplyVillageSetterRangePatch(byte[] exeBytes, bool enabled, ref bool exeModified)
        {
            ExeVillageSetterPatchState state = ExePatchModel.GetVillageSetterPatchState(exeBytes);
            if (state == ExeVillageSetterPatchState.Unknown)
            {
                if (enabled)
                {
                    throw new InvalidOperationException("無法套用村莊建造半徑補丁：主程式特徵碼不符合。");
                }
                _logger.Log("無法套用村莊建造半徑補丁：主程式特徵碼不符合。");
                return;
            }

            IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanVillageSetter(enabled, state);
            if (ops.Count > 0)
            {
                ExePatchModel.Apply(exeBytes, ops);
                exeModified = true;
            }

            if (enabled)
            {
                _logger.Log("已套用村莊建造範圍擴大補丁。");
            }
            else if (ops.Count > 0)
            {
                _logger.Log("已還原村莊建造範圍設定。");
            }
        }

        private void ApplySpellAltarPatch(byte[] exeBytes, bool noAltarChecked, ref bool exeModified)
        {
            ExeSpellAltarPatchState state = ExePatchModel.GetSpellAltarPatchState(exeBytes);
            if (state == ExeSpellAltarPatchState.Unknown)
            {
                throw new Exception("Against_Rome.exe 版本或法術祭壇特徵碼不符合預期，已停止套用法術祭壇補丁。");
            }

            IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanSpellAltar(noAltarChecked, state);
            if (ops.Count > 0)
            {
                ExePatchModel.Apply(exeBytes, ops);
                exeModified = true;
                _logger.Log(noAltarChecked ? "已套用法術免祭壇需求補丁。" : "已還原法術祭壇需求設定。");
            }
        }

        private void ApplyGameSpeedPatch(byte[] exeBytes, int multiplier, ref bool exeModified)
        {
            int current = ExePatchModel.GetGameSpeedMultiplier(exeBytes);
            if (current == 0)
            {
                _logger.Log("偵測到未知的遊戲時脈常數，已略過遊戲加速補丁以免覆蓋未知版本。");
                return;
            }
            IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanGameSpeed(multiplier, current);
            if (ops.Count > 0)
            {
                ExePatchModel.Apply(exeBytes, ops);
                exeModified = true;
            }
            _logger.Log(multiplier > 1 ? $"遊戲整體運行速度：{multiplier}× 加速。" : "遊戲整體運行速度：原版（未加速）。");
        }

        private byte[] GetPatchedClScriptBytes(string gamePath, BackupManager backupManager, bool fastCiviProduction, bool infiniteMoraleChecked, bool balanceChecked)
        {
            byte[] original = backupManager.GetBackupBytes("SYSTEM/cl_script.ini");
            byte[] decompressed = GameLZSS.DecompressPfil(original);
            string originalText = Encoding.GetEncoding(1251).GetString(decompressed);
            var managedKeys = GetClScriptManagedKeys(originalText);

            string[] lines = originalText.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None);
            string lineEnding = originalText.Contains("\r\n") ? "\r\n" : "\n";
            var resultLines = new List<string>();

            foreach (string line in lines)
            {
                string processedLine = line;

                // A. 祭司法術半徑補丁：若啟用平衡，將法術半徑調整為 Balanced 數值 (從 500 提高，非 priest 則仍維持 0)
                Match radiusMatch = RegexRadiusPatch.Match(line);
                if (radiusMatch.Success)
                {
                    string faction = radiusMatch.Groups[1].Value.Trim();
                    string spell = radiusMatch.Groups[2].Value.Trim();
                    string key = $"Radius|{faction}|{spell}";
                    if (managedKeys.Contains(key))
                    {
                        string pName = $"Fig{faction}Pri00_Priester";
                        double spellRadiusValue = 500;
                        if (balanceChecked)
                        {
                            double[] balanced = backupManager.GetDefaultBalancedStats(pName);
                            spellRadiusValue = balanced[8]; // SpellRadius is index 8
                        }
                        processedLine = $"Radius = {faction}, {spell}, {spellRadiusValue.ToString("0", CultureInfo.InvariantCulture)}{radiusMatch.Groups[4].Value}";
                    }
                }

                // B. 村民生產延遲 (fastCiviProduction)
                Match civiMatch = RegexCiviPatch.Match(line);
                if (civiMatch.Success)
                {
                    string faction = civiMatch.Groups[1].Value.Trim();
                    string key = $"CiviDelay|{faction}";
                    if (managedKeys.Contains(key))
                    {
                        double targetDelay = fastCiviProduction ? 500.0 : 15000.0;
                        processedLine = $"CiviDelay = {faction}, {targetDelay.ToString("0", CultureInfo.InvariantCulture)}{civiMatch.Groups[3].Value}";
                    }
                }

                // C. 無限士氣補丁
                Match moraleMatch = RegexMoraleKey.Match(line);
                if (moraleMatch.Success)
                {
                    string faction = moraleMatch.Groups[2].Value.Trim();
                    string key = $"{moraleMatch.Groups[1].Value.Trim()}|{faction}";
                    if (managedKeys.Contains(key))
                    {
                        if (line.StartsWith("MoralsDecLostMem"))
                        {
                            int val = infiniteMoraleChecked ? 0 : 3;
                            processedLine = RegexMoraleLostMemPatch.Replace(line, "${1}" + val + "$2");
                        }
                        else if (line.StartsWith("MoralsDecFlee"))
                        {
                            int val = infiniteMoraleChecked ? 0 : 5;
                            processedLine = RegexMoraleFleePatch.Replace(line, "${1}" + val + "$2");
                        }
                        else if (line.StartsWith("MoralsDecOverPop"))
                        {
                            int val = infiniteMoraleChecked ? 99999999 : 50;
                            processedLine = RegexMoraleOverPopPatch.Replace(line, "${1}" + val + "$2");
                        }
                        else if (line.StartsWith("MoralsIncIdle"))
                        {
                            int val = infiniteMoraleChecked ? 500 : 200;
                            processedLine = RegexMoraleIncIdlePatch.Replace(line, "${1}" + val + "$2");
                        }
                    }
                }

                resultLines.Add(processedLine);
            }

            byte[] plainBytes = Encoding.GetEncoding(1251).GetBytes(string.Join(lineEnding, resultLines));
            byte[] headerToUse = original;
            if (original.Length < 64 || original[0] != 'P' || original[1] != 'F' || original[2] != 'I' || original[3] != 'L')
            {
                headerToUse = new byte[64];
                headerToUse[0] = (byte)'P';
                headerToUse[1] = (byte)'F';
                headerToUse[2] = (byte)'I';
                headerToUse[3] = (byte)'L';
            }
            return GameLZSS.CompressPfil(plainBytes, headerToUse);
        }

        private static HashSet<string> GetClScriptManagedKeys(string text)
        {
            var keys = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            string[] lines = text.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None);
            foreach (string line in lines)
            {
                Match radius = RegexRadiusPatch.Match(line);
                if (radius.Success)
                {
                    keys.Add($"Radius|{radius.Groups[1].Value.Trim()}|{radius.Groups[2].Value.Trim()}");
                    continue;
                }

                Match civi = RegexCiviPatch.Match(line);
                if (civi.Success)
                {
                    keys.Add($"CiviDelay|{civi.Groups[1].Value.Trim()}");
                    continue;
                }

                Match morale = RegexMoraleKey.Match(line);
                if (morale.Success)
                {
                    keys.Add($"{morale.Groups[1].Value.Trim()}|{morale.Groups[2].Value.Trim()}");
                }
            }
            return keys;
        }

        private byte[] GetPatchedRessBytes(BackupManager backupManager, bool freeProdChecked, bool freeUpgradeChecked, bool noSpellCostChecked)
        {
            return RessPatcher.GetPatchedBytes(backupManager.GetBackupBytes("SYSTEM/ress.ini"), new RessOptions(freeProdChecked, freeUpgradeChecked, noSpellCostChecked));
        }

        private byte[] GetPatchedObjdefBytes(BackupManager backupManager, PatchOptions options)
        {
            byte[] original = backupManager.GetBackupBytes("SYSTEM/DATA_MP/DEFAULTS/objdef.dau");
            var unitStats = new Dictionary<string, double[]>(StringComparer.OrdinalIgnoreCase);
            foreach (string key in TroopConfig.UnitMeta.Keys)
            {
                unitStats[key] = backupManager.GetBaseStatsForUnit(key, options);
            }
            return ObjdefPatcher.GetPatchedBytes(original, new ObjdefOptions(options.Balance, options.HousingCapacity20x, options.StorageCapacity10x, options.FastBuildUpgradeRepair, new Dictionary<string, double[]>(), unitStats));
        }

        private Dictionary<string, byte[]> GetPatchedTeamDatBytes(BackupManager backupManager, bool maxPopulation)
        {
            var results = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
            foreach (var item in backupManager.BackupFiles.Where(item => item.Key.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) && item.Key.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase)))
            {
                results[item.Key] = TeamDatPatcher.GetPatchedBytes(item.Value, new TeamDatOptions(maxPopulation));
            }
            _logger.Log(maxPopulation ? string.Format("已修改所有地圖的 team.dat 人口上限為 {0} (共處理 {1} 個檔案)。", 1600, results.Count) : "已將所有地圖的 team.dat 人口上限還原為原版。");
            return results;
        }

        private void ApplyFoodHealingAmountPatch(string gamePath, bool enabled, BackupManager backupManager, FileRollbackScope? rollback)
        {
            string scriptRoot = Path.Combine(gamePath, @"SYSTEM\CLAK\SCRIPT");
            int targetValue = enabled ? FoodHealAmountUltimate : FoodHealAmountOriginal;

            foreach (var (file, symbolIndex) in FoodHealAmountSites)
            {
                string scriptPath = Path.Combine(scriptRoot, file + ".bci");
                if (!File.Exists(scriptPath))
                {
                    throw new FileNotFoundException("找不到待機回血 AI 腳本。", scriptPath);
                }

                byte[] raw = File.ReadAllBytes(scriptPath);
                byte[] decomp = GameLZSS.DecompressPfil(raw);
                bool retiredLeaderScriptMigrated = false;

                if (file.Equals("ak_anfuehrer", StringComparison.OrdinalIgnoreCase))
                {
                    byte[] normalized = (byte[])decomp.Clone();
                    var legacyOriginalSites = BciPattern.FindAllBciWordPatternSites(normalized, BuildFoodHealAmountSignature(symbolIndex, FoodHealAmountOriginal));
                    var legacyUltimateSites = BciPattern.FindAllBciWordPatternSites(normalized, BuildFoodHealAmountSignature(symbolIndex, FoodHealAmountUltimate));
                    if (legacyOriginalSites.Count + legacyUltimateSites.Count == 1)
                    {
                        if (legacyUltimateSites.Count == 1)
                        {
                            int valueOffset = legacyUltimateSites[0] + 4;
                            BciPattern.WriteBciInt32(normalized, valueOffset, FoodHealAmountUltimate, FoodHealAmountOriginal, "retired leader glory probe");
                        }

                        string normalizedHash = Convert.ToHexString(SHA256.HashData(normalized));
                        if (normalizedHash.Equals(RetiredLeaderGloryScriptSha256, StringComparison.Ordinal))
                        {
                            raw = backupManager.GetBackupBytes("SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci");
                            decomp = GameLZSS.DecompressPfil(raw);
                            string cleanHash = Convert.ToHexString(SHA256.HashData(decomp));
                            if (!cleanHash.Equals(VanillaLeaderScriptSha256, StringComparison.Ordinal))
                            {
                                throw new InvalidDataException("乾淨的原版 ak_anfuehrer.bci 備份不存在或版本不符，已取消安全遷移。");
                            }
                            retiredLeaderScriptMigrated = true;
                            _logger.Log("已移除會造成戰鬥閃退的舊版首領榮耀腳本，並以原版 ak_anfuehrer.bci 重建。");
                        }
                    }
                }

                var originalSites = BciPattern.FindAllBciWordPatternSites(decomp, BuildFoodHealAmountSignature(symbolIndex, FoodHealAmountOriginal));
                var ultimateSites = BciPattern.FindAllBciWordPatternSites(decomp, BuildFoodHealAmountSignature(symbolIndex, FoodHealAmountUltimate));
                if (originalSites.Count + ultimateSites.Count != 1)
                {
                    throw new InvalidDataException(string.Format("待機回血 AI 腳本特徵數量不符（預期 1、實際 {0}）: {1}", originalSites.Count + ultimateSites.Count, scriptPath));
                }

                int currentValue = originalSites.Count == 1 ? FoodHealAmountOriginal : FoodHealAmountUltimate;
                int patchOffset = (originalSites.Count == 1 ? originalSites[0] : ultimateSites[0]) + 4;
                if (currentValue == targetValue)
                {
                    if (retiredLeaderScriptMigrated)
                    {
                        SafeWriteAllBytes(scriptPath, raw, rollback);
                    }
                    continue;
                }

                BciPattern.WriteBciInt32(decomp, patchOffset, currentValue, targetValue, "待機回血量");
                byte[] compressed = GameLZSS.CompressPfil(decomp, raw);
                SafeWriteAllBytes(scriptPath, compressed, rollback);
            }
        }

        private static int?[] BuildFoodHealAmountSignature(int addLpSymbolIndex, int amountLiteral)
        {
            return new int?[] { 66, amountLiteral, 81, 10, 81, 98, 128, addLpSymbolIndex, 73, -3, 86 };
        }

        // --- 語言與 dgVoodoo2 補丁邏輯 ---

        private void ApplyLanguagePatch(string gamePath, bool toEnglish, FileRollbackScope? rollback = null)
        {
            string localToEngDir = Path.Combine(gamePath, "ToEng");

            if (toEnglish)
            {
                if (!Directory.Exists(localToEngDir))
                {
                    throw new DirectoryNotFoundException("找不到 ToEng 英文包資源目錄。");
                }

                string[] files = Directory.GetFiles(localToEngDir, "*", SearchOption.AllDirectories);
                if (files.Length == 0)
                {
                    throw new InvalidDataException("ToEng 英文包資源目錄內無任何檔案。");
                }
                EnsureLanguageBackup(gamePath, localToEngDir, files);
                foreach (string file in files)
                {
                    string relPath = Path.GetRelativePath(localToEngDir, file);
                    string destPath = GetSafeLanguagePath(gamePath, relPath);
                    SafeCopyFile(file, destPath, true, rollback);
                }
                _logger.Log("已成功套用英文介面與地圖語言包。");
                return;
            }

            string languageManifestPath = Path.Combine(GetLanguageBackupDirectory(gamePath), LanguageBackupManifestName);
            if (!File.Exists(languageManifestPath))
            {
                if (TryGetLanguageOverlayState(gamePath, out bool overlayEnabled) && overlayEnabled)
                {
                    throw new InvalidOperationException("找不到語言還原資訊 manifest.json 備份檔。");
                }
                return;
            }
            RestoreLanguageBackup(gamePath, rollback);
            _logger.Log("已將英文介面與地圖語言包還原為原版。");
        }

        private static string GetLanguageBackupDirectory(string gamePath)
        {
            return Path.Combine(gamePath, LanguageBackupDirectoryName);
        }

        private static string GetSafeLanguagePath(string rootPath, string relativePath)
        {
            if (string.IsNullOrWhiteSpace(relativePath) || Path.IsPathRooted(relativePath))
            {
                throw new InvalidDataException("不安全的語言還原目標路徑。");
            }

            string root = Path.GetFullPath(rootPath).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
            string fullPath = Path.GetFullPath(Path.Combine(root, relativePath.Replace('/', Path.DirectorySeparatorChar)));
            if (!fullPath.StartsWith(root, StringComparison.OrdinalIgnoreCase))
            {
                throw new InvalidDataException("不安全的語言還原目標路徑。");
            }
            return fullPath;
        }

        private void EnsureLanguageBackup(string gamePath, string sourceRoot, string[] sourceFiles)
        {
            string backupRoot = GetLanguageBackupDirectory(gamePath);
            string manifestPath = Path.Combine(backupRoot, LanguageBackupManifestName);
            if (File.Exists(manifestPath))
            {
                LanguageBackupManifest? existingManifest = JsonSerializer.Deserialize<LanguageBackupManifest>(File.ReadAllText(manifestPath, Encoding.UTF8));
                if (existingManifest == null || existingManifest.ExistingFiles == null || existingManifest.MissingFiles == null)
                {
                    throw new InvalidDataException("語言還原資訊 manifest.json 格式損毀。");
                }
                var coveredPaths = new HashSet<string>(existingManifest.ExistingFiles.Concat(existingManifest.MissingFiles), StringComparer.OrdinalIgnoreCase);
                foreach (string sourcePath in sourceFiles)
                {
                    string relativePath = Path.GetRelativePath(sourceRoot, sourcePath).Replace('\\', '/');
                    if (!coveredPaths.Contains(relativePath)) throw new InvalidDataException("語言還原資訊 manifest.json 格式損毀。");
                }
                foreach (string relativePath in existingManifest.ExistingFiles)
                {
                    if (!File.Exists(GetSafeLanguagePath(Path.Combine(backupRoot, "files"), relativePath)))
                    {
                        throw new InvalidDataException("語言還原資訊 manifest.json 格式損毀。");
                    }
                }
                return;
            }
            if (Directory.Exists(backupRoot))
            {
                throw new InvalidDataException("語言還原資訊 manifest.json 格式損毀。");
            }
            if (TryGetLanguageOverlayState(gamePath, out bool alreadyEnabled) && alreadyEnabled)
            {
                throw new InvalidOperationException("檢測到已套用英文套件，但找不到語言還原資訊備份檔。");
            }

            string tempRoot = backupRoot + ".tmp-" + Guid.NewGuid().ToString("N");
            var manifest = new LanguageBackupManifest();
            try
            {
                foreach (string sourcePath in sourceFiles)
                {
                    string relativePath = Path.GetRelativePath(sourceRoot, sourcePath).Replace('\\', '/');
                    string destinationPath = GetSafeLanguagePath(gamePath, relativePath);
                    if (File.Exists(destinationPath))
                    {
                        string backupPath = GetSafeLanguagePath(Path.Combine(tempRoot, "files"), relativePath);
                        Directory.CreateDirectory(Path.GetDirectoryName(backupPath)!);
                        File.Copy(destinationPath, backupPath, true);
                        manifest.ExistingFiles.Add(relativePath);
                    }
                    else
                    {
                        manifest.MissingFiles.Add(relativePath);
                    }
                }

                Directory.CreateDirectory(tempRoot);
                File.WriteAllText(
                    Path.Combine(tempRoot, LanguageBackupManifestName),
                    JsonSerializer.Serialize(manifest, new JsonSerializerOptions { WriteIndented = true }),
                    Encoding.UTF8);
                Directory.Move(tempRoot, backupRoot);
            }
            finally
            {
                if (Directory.Exists(tempRoot)) Directory.Delete(tempRoot, true);
            }
        }

        private void RestoreLanguageBackup(string gamePath, FileRollbackScope? rollback)
        {
            string backupRoot = GetLanguageBackupDirectory(gamePath);
            string manifestPath = Path.Combine(backupRoot, LanguageBackupManifestName);
            if (!File.Exists(manifestPath))
            {
                throw new InvalidOperationException("找不到語言還原資訊 manifest.json 備份檔。");
            }

            LanguageBackupManifest? manifest = JsonSerializer.Deserialize<LanguageBackupManifest>(File.ReadAllText(manifestPath, Encoding.UTF8));
            if (manifest == null || manifest.ExistingFiles == null || manifest.MissingFiles == null)
            {
                throw new InvalidDataException("語言還原資訊 manifest.json 格式損毀。");
            }

            foreach (string relativePath in manifest.ExistingFiles)
            {
                string backupPath = GetSafeLanguagePath(Path.Combine(backupRoot, "files"), relativePath);
                if (!File.Exists(backupPath)) throw new InvalidDataException("語言還原資訊 manifest.json 格式損毀。");
                SafeCopyFile(backupPath, GetSafeLanguagePath(gamePath, relativePath), true, rollback);
            }
            foreach (string relativePath in manifest.MissingFiles)
            {
                string destinationPath = GetSafeLanguagePath(gamePath, relativePath);
                if (!File.Exists(destinationPath)) continue;
                SafeDeleteFile(destinationPath, rollback);
            }
        }

        private bool TryGetLanguageOverlayState(string gamePath, out bool enabled)
        {
            enabled = false;
            string sourceRoot = Path.Combine(gamePath, "ToEng");
            if (!Directory.Exists(sourceRoot)) return false;
            string[] sourceFiles = Directory.GetFiles(sourceRoot, "*", SearchOption.AllDirectories);
            if (sourceFiles.Length == 0) return false;

            foreach (string sourcePath in sourceFiles)
            {
                string relativePath = Path.GetRelativePath(sourceRoot, sourcePath);
                string destinationPath = GetSafeLanguagePath(gamePath, relativePath);
                if (!FilesAreEqual(sourcePath, destinationPath)) return true;
            }
            enabled = true;
            return true;
        }

        private static bool FilesAreEqual(string firstPath, string secondPath)
        {
            var firstInfo = new FileInfo(firstPath);
            var secondInfo = new FileInfo(secondPath);
            if (!firstInfo.Exists || !secondInfo.Exists || firstInfo.Length != secondInfo.Length) return false;

            const int bufferSize = 81920;
            byte[] firstBuffer = new byte[bufferSize];
            byte[] secondBuffer = new byte[bufferSize];
            using var first = new FileStream(firstPath, FileMode.Open, FileAccess.Read, FileShare.Read, bufferSize, FileOptions.SequentialScan);
            using var second = new FileStream(secondPath, FileMode.Open, FileAccess.Read, FileShare.Read, bufferSize, FileOptions.SequentialScan);
            while (true)
            {
                int firstRead = first.Read(firstBuffer, 0, firstBuffer.Length);
                int secondRead = second.Read(secondBuffer, 0, secondBuffer.Length);
                if (firstRead != secondRead) return false;
                if (firstRead == 0) return true;
                if (!firstBuffer.AsSpan(0, firstRead).SequenceEqual(secondBuffer.AsSpan(0, secondRead))) return false;
            }
        }

        // --- dgVoodoo2 專用核心邏輯 ---

        private void ApplyDgVoodooPatch(string gamePath, bool enabled, FileRollbackScope? rollback = null)
        {
            if (!enabled)
            {
                RemoveDgVoodoo(gamePath, rollback);
                return;
            }

            DgVoodooManifest? existingManifest = ReadDgVoodooManifest(gamePath);
            Dictionary<string, byte[]> packageFiles = LoadEmbeddedDgVoodooFiles();

            foreach (string fileName in DgVoodooManagedFiles)
            {
                string destination = Path.Combine(gamePath, fileName);
                if (!File.Exists(destination)) continue;

                if (existingManifest == null || !existingManifest.Files.TryGetValue(fileName, out string? expectedHash))
                {
                    throw new IOException(string.Format("目標檔案已被其它程式佔用或已存在非修改器託管之同名衝突檔案：{0}。如果確認要覆蓋，請先手動移除該檔案後重試。", destination));
                }

                string currentHash = ComputeSha256(File.ReadAllBytes(destination));
                if (!string.Equals(currentHash, expectedHash, StringComparison.OrdinalIgnoreCase))
                {
                    if (string.Equals(fileName, "dgVoodoo.conf", StringComparison.OrdinalIgnoreCase))
                    {
                        packageFiles[fileName] = File.ReadAllBytes(destination);
                    }
                    else
                    {
                        throw new IOException(string.Format("目標已存在已修改或非託管之 dgVoodoo2 檔案：{0}。請先手動備份並移除該衝突檔案後重試。", destination));
                    }
                }
            }

            var newManifest = new DgVoodooManifest { Version = DgVoodooEmbeddedVersion };
            foreach (string fileName in DgVoodooManagedFiles)
            {
                byte[] bytes = packageFiles[fileName];
                SafeWriteAllBytes(Path.Combine(gamePath, fileName), bytes, rollback);
                newManifest.Files[fileName] = ComputeSha256(bytes);
            }

            byte[] markerBytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(newManifest, new JsonSerializerOptions { WriteIndented = true }));
            SafeWriteAllBytes(GetDgVoodooMarkerPath(gamePath), markerBytes, rollback);
            _logger.Log(string.Format("已成功安裝與設定 dgVoodoo2 ({0}) 繪圖轉譯器。", DgVoodooEmbeddedVersion));
        }

        private void RemoveDgVoodoo(string gamePath, FileRollbackScope? rollback)
        {
            DgVoodooManifest? manifest = ReadDgVoodooManifest(gamePath);
            if (manifest == null)
            {
                if (DgVoodooManagedFiles.Any(fileName => File.Exists(Path.Combine(gamePath, fileName))))
                {
                    _logger.Log("偵測到遊戲目錄中存在非本修改器部署的 dgVoodoo2 相關元件，為保護使用者資產將不主動進行刪除；若要乾淨卸載，請手動刪除遊戲目錄下的 DDraw.dll, D3D8.dll, dgVoodooCpl.exe。");
                }
                return;
            }

            var preserved = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            foreach (string fileName in DgVoodooManagedFiles)
            {
                if (!manifest.Files.TryGetValue(fileName, out string? expectedHash)) continue;
                string path = Path.Combine(gamePath, fileName);
                if (!File.Exists(path)) continue;

                string currentHash = ComputeSha256(File.ReadAllBytes(path));
                if (!string.Equals(currentHash, expectedHash, StringComparison.OrdinalIgnoreCase))
                {
                    if (string.Equals(fileName, "dgVoodoo.conf", StringComparison.OrdinalIgnoreCase))
                    {
                        _logger.Log(string.Format("dgVoodoo2 託管設定檔 {0} 的雜湊值已變更，將予以保留不刪除。", fileName));
                        continue;
                    }

                    preserved[fileName] = expectedHash;
                    _logger.Log(string.Format("dgVoodoo2 託管設定檔 {0} 的雜湊值已變更，將予以保留不刪除。", fileName));
                    continue;
                }

                SafeDeleteFile(path, rollback);
            }

            string markerPath = GetDgVoodooMarkerPath(gamePath);
            if (preserved.Count == 0)
            {
                SafeDeleteFile(markerPath, rollback);
                _logger.Log("已成功移除 dgVoodoo2 所有受託管檔案。");
            }
            else
            {
                manifest.Files = preserved;
                byte[] markerBytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(manifest, new JsonSerializerOptions { WriteIndented = true }));
                SafeWriteAllBytes(markerPath, markerBytes, rollback);
            }
        }

        public bool IsDgVoodooInstalled(string gamePath)
        {
            DgVoodooManifest? manifest = ReadDgVoodooManifest(gamePath);
            return manifest != null &&
                   manifest.Files.ContainsKey("D3D8.dll") &&
                   manifest.Files.ContainsKey("DDraw.dll") &&
                   File.Exists(Path.Combine(gamePath, "D3D8.dll")) &&
                   File.Exists(Path.Combine(gamePath, "DDraw.dll"));
        }

        private static string GetDgVoodooMarkerPath(string gamePath)
        {
            return Path.Combine(gamePath, DgVoodooMarkerFileName);
        }

        private static DgVoodooManifest? ReadDgVoodooManifest(string gamePath)
        {
            string markerPath = GetDgVoodooMarkerPath(gamePath);
            if (!File.Exists(markerPath)) return null;
            try
            {
                DgVoodooManifest? manifest = JsonSerializer.Deserialize<DgVoodooManifest>(File.ReadAllText(markerPath, Encoding.UTF8));
                if (manifest == null || manifest.Files == null) return null;
                manifest.Files = new Dictionary<string, string>(manifest.Files, StringComparer.OrdinalIgnoreCase);
                return manifest;
            }
            catch (Exception ex) when (ex is JsonException || ex is IOException || ex is UnauthorizedAccessException)
            {
                return null;
            }
        }

        private static Dictionary<string, byte[]> LoadEmbeddedDgVoodooFiles()
        {
            var result = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
            System.Reflection.Assembly assembly = typeof(PatchEngine).Assembly;
            foreach (var resource in DgVoodooResourceNames)
            {
                using Stream source = assembly.GetManifestResourceStream(resource.Value)
                    ?? throw new InvalidDataException("The embedded dgVoodoo2 resource is missing: " + resource.Value);
                using MemoryStream ms = new MemoryStream();
                source.CopyTo(ms);
                result[resource.Key] = ms.ToArray();
            }
            return result;
        }

        private static string ComputeSha256(byte[] bytes)
        {
            byte[] hash = SHA256.HashData(bytes);
            var sb = new StringBuilder(hash.Length * 2);
            foreach (byte b in hash) sb.Append(b.ToString("x2"));
            return sb.ToString();
        }

        // --- 全方位修改狀態偵測方法 ---

        public PatchOptions DetectCurrentPatchState(string gamePath, BackupManager backupManager)
        {
            var options = new PatchOptions();

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
                            setterState == ExeVillageSetterPatchState.Expanded5x);
                    }

                    var altarState = ExePatchModel.GetSpellAltarPatchState(exeBytes);
                    if (altarState != ExeSpellAltarPatchState.Unknown)
                    {
                        options.NoSpellAltar = (altarState == ExeSpellAltarPatchState.Patched);
                    }

                    int speed = ExePatchModel.GetGameSpeedMultiplier(exeBytes);
                    options.GameSpeed = speed;
                }
                catch { }
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
                catch { }
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
                catch { }
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

                    if (backupManager.BackupFiles.TryGetValue("SYSTEM/DATA_MP/DEFAULTS/objdef.dau", out byte[]? originalObjdefBytes))
                    {
                        string originalObjdef = Encoding.GetEncoding(1251).GetString(GameLZSS.DecompressPfil(originalObjdefBytes));
                        options.HousingCapacity20x = HasHousingCapacityMultiplier(currentObjdef, originalObjdef, HousingCapacityMultiplier);
                        options.StorageCapacity10x = HasStorageCapacityMultiplier(currentObjdef, originalObjdef, StorageCapacityMultiplier);
                        options.FastBuildUpgradeRepair = HasFastBuildUpgradeRepair(currentObjdef, originalObjdef);
                    }

                    // 偵測是否已套用 Balance (若有任何一個兵種屬性被修改)
                    string lineEnding = currentObjdef.Contains("\r\n") ? "\r\n" : "\n";
                    string[] lines = currentObjdef.Split(new string[] { lineEnding }, StringSplitOptions.None);
                    var unitRows = new Dictionary<string, string[]>(StringComparer.OrdinalIgnoreCase);
                    for (int idx = 2; idx < lines.Length; idx++)
                    {
                        string line = lines[idx];
                        if (line.Length < 100) continue;
                        string[] cols = PatchText.ParseCsvLine(line);
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
                        string utype = TroopConfig.UnitMeta[key].Item3;

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
                catch { }
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
            catch { }

            // F. Endless AI (M1..M5)
            try
            {
                var orchestrator = new EndlessAiOrchestrator();
                for (int i = 0; i < orchestrator.UserModules.Count && i < options.EndlessAiModules.Length; i++)
                {
                    var module = orchestrator.UserModules[i];
                    var aiState = orchestrator.DetectModule(gamePath, module);
                    options.EndlessAiModules[i] = (aiState == PatchState.Ultimate);
                }
            }
            catch { }

            // G. toEnglish
            try
            {
                TryGetLanguageOverlayState(gamePath, out bool langEng);
                options.ToEnglish = langEng;
            }
            catch { }

            // H. DgVoodoo
            try
            {
                options.DgVoodoo = IsDgVoodooInstalled(gamePath);
            }
            catch { }

            // I. FoodHealing10x
            try
            {
                TryReadFoodHealingAmountState(gamePath, out bool foodHealing);
                options.FoodHealing10x = foodHealing;
            }
            catch { }

            return options;
        }

        // --- Objdef dau 偵測細部輔助函數 ---

        private static bool HasHousingCapacityMultiplier(string currentContent, string originalContent, int multiplier)
        {
            var currentValues = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
            string[] currentLines = currentContent.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None);
            foreach (string line in currentLines)
            {
                if (line.Length < 100) continue;
                string[] cols = PatchText.ParseCsvLine(line);
                if (cols.Length <= (int)ObjdefIndex.HousingCapacity || cols.Length <= (int)ObjdefIndex.Name) continue;
                if (int.TryParse(cols[(int)ObjdefIndex.HousingCapacity].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int value))
                {
                    currentValues[cols[(int)ObjdefIndex.Name].Trim()] = value;
                }
            }

            bool foundHousing = false;
            string[] originalLines = originalContent.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None);
            foreach (string line in originalLines)
            {
                if (line.Length < 100) continue;
                string[] cols = PatchText.ParseCsvLine(line);
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

        private static bool HasStorageCapacityMultiplier(string currentContent, string originalContent, int multiplier)
        {
            var currentValues = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
            string[] currentLines = currentContent.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None);
            foreach (string line in currentLines)
            {
                if (line.Length < 100) continue;
                string[] cols = PatchText.ParseCsvLine(line);
                if (cols.Length <= (int)ObjdefIndex.StorageCapacity || cols.Length <= (int)ObjdefIndex.Name) continue;
                if (int.TryParse(cols[(int)ObjdefIndex.StorageCapacity].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int value))
                {
                    currentValues[cols[(int)ObjdefIndex.Name].Trim()] = value;
                }
            }

            bool foundStorage = false;
            string[] originalLines = originalContent.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None);
            foreach (string line in originalLines)
            {
                if (line.Length < 100) continue;
                string[] cols = PatchText.ParseCsvLine(line);
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

        private static bool HasFastBuildUpgradeRepair(string currentContent, string originalContent)
        {
            var currentBuildValues = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
            var currentUpgValues = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);

            string[] currentLines = currentContent.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None);
            foreach (string line in currentLines)
            {
                if (line.Length < 100) continue;
                string[] cols = PatchText.ParseCsvLine(line);
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
            string[] originalLines = originalContent.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None);
            foreach (string line in originalLines)
            {
                if (line.Length < 100) continue;
                string[] cols = PatchText.ParseCsvLine(line);
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

        private bool TryReadFoodHealingAmountState(string gamePath, out bool enabled)
        {
            enabled = false;
            string scriptRoot = Path.Combine(gamePath, @"SYSTEM\CLAK\SCRIPT");
            if (!Directory.Exists(scriptRoot)) return false;

            bool? detectedState = null;
            foreach (var (file, symbolIndex) in FoodHealAmountSites)
            {
                string scriptPath = Path.Combine(scriptRoot, file + ".bci");
                if (!File.Exists(scriptPath)) return false;

                byte[] decomp = GameLZSS.DecompressPfil(File.ReadAllBytes(scriptPath));
                var originalSites = BciPattern.FindAllBciWordPatternSites(decomp, BuildFoodHealAmountSignature(symbolIndex, FoodHealAmountOriginal));
                var ultimateSites = BciPattern.FindAllBciWordPatternSites(decomp, BuildFoodHealAmountSignature(symbolIndex, FoodHealAmountUltimate));
                if (originalSites.Count + ultimateSites.Count != 1)
                {
                    return false;
                }

                bool isEnabled = ultimateSites.Count == 1;
                if (detectedState.HasValue && detectedState.Value != isEnabled) return false;
                detectedState = isEnabled;
            }

            enabled = detectedState == true;
            return detectedState.HasValue;
        }
    }
}
