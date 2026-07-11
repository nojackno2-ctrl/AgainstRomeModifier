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
using AgainstRomeModifier.Core.Features.Ini;
using AgainstRomeModifier.Core.Features.Map;
using AgainstRomeModifier.Core.Features.Objdef;
using AgainstRomeModifier.Core.Features.Exe;
using AgainstRomeModifier.Core.Features.Bci;
using AgainstRomeModifier.Core.Features.Install;
using AgainstRomeModifier.Core.Features;

namespace AgainstRomeModifier.Core.Services
{
    public class PatchEngine
    {
        private readonly ILogger _logger;

        public PatchEngine(ILogger logger)
        {
            _logger = logger;
        }

        public bool IsDgVoodooInstalled(string gamePath) => new DgVoodooFeature(_logger).IsInstalled(gamePath);

        public PatchProfile DetectCurrentPatchState(string gamePath, BackupManager backupManager)
        {
            PatchProfile detected = new FeatureDetector(_logger).Detect(gamePath, backupManager);
            detected.NormalizeCompositeValues();
            var context = new DetectContext(detected);
            var result = new PatchProfile();
            foreach (IFeatureModule module in FeatureRegistry.All)
                result.Set(module.Id, module.Detect(context));
            return result;
        }

        public PatchProfile DetectCurrentPatchProfile(string gamePath, BackupManager backupManager) =>
            DetectCurrentPatchState(gamePath, backupManager);

        // --- 核心安全寫入工具（實作集中於 SafeFileWriter，此處保留原有呼叫介面）---

        public void SafeWriteAllBytes(string dest, byte[] bytes, FileRollbackScope? rollback = null)
        {
            SafeFileWriter.WriteAllBytes(dest, bytes, rollback);
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

        public void ApplyPatches(string gamePath, PatchProfile profile, BackupManager backupManager, FileRollbackScope rollback)
        {
            profile.NormalizeCompositeValues();
            var context = new PatchContext();
            foreach (IFeatureModule module in FeatureRegistry.All)
                module.Plan(context, profile.Get(module.Id));
            PatchProfile options = context.Profile;
            // 還原與重套共用同一個 orchestrator：同一批 BCI 腳本只解壓一次，
            // 且中間的還原狀態留在快取、由最後的 SaveAll 一次寫入最終狀態。
            var orchestrator = new EndlessAiOrchestrator();

            // 總是先在同個交易中執行原版恢復
            _logger.Log(Loc.Get("SvcLogPreApplyRestore"));
            RestoreCategories(gamePath, backupManager, rollback,
                new[] { FeatureCategory.Stats, FeatureCategory.Compat, FeatureCategory.Language },
                orchestrator, saveOrchestrator: false);

            // 1. Dry Run 階段：在記憶體中生成所有補丁 byte[] 並驗證
            var patchedFiles = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);

            // A. Against_Rome.exe
            string exePath = Path.Combine(gamePath, @"Against_Rome.exe");
            byte[] exeBytes = File.ReadAllBytes(exePath);
            bool exeModified = false;
            exeModified = ExeFeaturePatcher.Apply(exeBytes, options.FocusLoss, options.VillageBuildRange, options.NoSpellAltar, options.GameSpeed, _logger);
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

            // H. Endless AI 模組（沿用開頭建立、已含還原後快取狀態的 orchestrator）
            // 以模組 Id 查表，不依賴 UserModules 的排列順序。
            foreach (var module in orchestrator.UserModules)
            {
                orchestrator.ApplyModule(gamePath, module, options.GetEndlessAiModule(module.Id));
            }
            orchestrator.ApplyMandatoryRepair(gamePath);
            FoodHealingFeature.Apply(gamePath, options.FoodHealing10x, backupManager, orchestrator, _logger);

            // Dry Run 順利結束，進行實體檔案寫入與交易範圍
            foreach (var kvp in patchedFiles)
            {
                SafeWriteAllBytes(kvp.Key, kvp.Value, rollback);
            }
            orchestrator.SaveAll(gamePath, rollback);

            // 其它補丁
            new LanguagePackFeature(_logger).Apply(gamePath, options.ToEnglish, rollback);
            new DgVoodooFeature(_logger).Apply(gamePath, options.DgVoodoo, rollback);
        }

        /// <summary>
        /// 啟動時的安全遷移：只修復已知會造成問題的舊版寫入——
        /// (1) R0 常駐修復（ak_npc / ak_produktion 的被否決舊補丁、DELETE_TEAM 終結狀態），
        /// (2) 會導致戰鬥閃退的舊版首領榮耀腳本（在 ApplyFoodHealingAmountPatch 內以原版重建，
        ///     並保留使用者目前的待機回血設定值）。
        /// 刻意不呼叫完整的 ApplyPatches：完整套用會先整體還原再依「偵測到的選項」重套，
        /// 但自訂兵種屬性無法從檔案偵測回來，會在啟動瞬間被無聲覆蓋。
        /// </summary>
        public void RunStartupSafeMigrations(string gamePath, BackupManager backupManager, FileRollbackScope rollback)
        {
            var orchestrator = new EndlessAiOrchestrator();
            orchestrator.ApplyMandatoryRepair(gamePath);

            // 保留現況：可判定時沿用目前的待機回血狀態；無法判定時視為原版。
            if (!FoodHealingFeature.TryDetect(gamePath, out bool foodHealingEnabled))
            {
                foodHealingEnabled = false;
            }
            FoodHealingFeature.Apply(gamePath, foodHealingEnabled, backupManager, orchestrator, _logger);
            orchestrator.SaveAll(gamePath, rollback);
        }

        public void RestoreOriginalFiles(string gamePath, BackupManager backupManager, FileRollbackScope rollback)
        {
            RestoreCategories(gamePath, backupManager, rollback,
                new[] { FeatureCategory.Stats, FeatureCategory.Compat, FeatureCategory.Language });
        }

        public void RestoreStatsOnly(string gamePath, BackupManager backupManager, FileRollbackScope rollback)
        {
            RestoreCategories(gamePath, backupManager, rollback, new[] { FeatureCategory.Stats });
        }

        public void RestoreCompatOnly(string gamePath, BackupManager backupManager, FileRollbackScope rollback)
        {
            RestoreCategories(gamePath, backupManager, rollback, new[] { FeatureCategory.Compat });
        }

        public void RestoreLanguageOnly(string gamePath, FileRollbackScope rollback)
        {
            RestoreCategories(gamePath, null, rollback, new[] { FeatureCategory.Language });
        }

        /// <param name="sharedOrchestrator">
        /// 供 ApplyPatches 傳入共用的 orchestrator：還原與後續重套共用同一份 BCI 檔案快取，
        /// 同一批腳本不必解壓兩次。共用時由呼叫端負責最終 SaveAll（saveOrchestrator = false）。
        /// </param>
        private void RestoreCategories(string gamePath, BackupManager? backupManager, FileRollbackScope rollback,
            IEnumerable<FeatureCategory> categories, EndlessAiOrchestrator? sharedOrchestrator = null,
            bool saveOrchestrator = true)
        {
            var requested = categories.ToHashSet();
            bool restoreStats = requested.Contains(FeatureCategory.Stats);
            bool restoreCompat = requested.Contains(FeatureCategory.Compat);
            bool restoreLanguage = requested.Contains(FeatureCategory.Language);
            var patchedFiles = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
            EndlessAiOrchestrator? orchestrator = null;

            if (restoreCompat)
            {
                string exePath = Path.Combine(gamePath, @"Against_Rome.exe");
                byte[] exeBytes = File.ReadAllBytes(exePath);
                if (ExeFeaturePatcher.Apply(exeBytes, false, false, false, 1, _logger))
                    patchedFiles[exePath] = exeBytes;

                orchestrator = sharedOrchestrator ?? new EndlessAiOrchestrator();
                foreach (var module in orchestrator.UserModules)
                    orchestrator.ApplyModule(gamePath, module, false);
                orchestrator.ApplyMandatoryRepair(gamePath);
            }

            if (restoreStats)
                RestoreStatsFiles(gamePath, backupManager ?? throw new ArgumentNullException(nameof(backupManager)), rollback);

            if (restoreStats)
            {
                orchestrator ??= sharedOrchestrator ?? new EndlessAiOrchestrator();
                FoodHealingFeature.Apply(gamePath, false, backupManager!, orchestrator, _logger);
            }

            foreach (var kvp in patchedFiles)
                SafeWriteAllBytes(kvp.Key, kvp.Value, rollback);

            if (orchestrator != null && saveOrchestrator)
                orchestrator.SaveAll(gamePath, rollback);
            if (restoreLanguage)
                new LanguagePackFeature(_logger).Apply(gamePath, false, rollback);
            if (restoreCompat)
                new DgVoodooFeature(_logger).Apply(gamePath, false, rollback);
        }

        private void RestoreStatsFiles(string gamePath, BackupManager backupManager, FileRollbackScope? rollback = null)
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
                    _logger.Log(string.Format(Loc.Get("SvcLogRestoredPopulation"), destPath));
                }
            }
        }

        private void RestoreMemoryFile(BackupManager backupManager, string key, string dest, FileRollbackScope? rollback = null)
        {
            if (backupManager.BackupFiles.TryGetValue(key, out byte[]? fileBytes))
            {
                SafeWriteAllBytes(dest, fileBytes!, rollback);
                _logger.Log(string.Format(Loc.Get("SvcLogRestoredFile"), dest));
            }
        }

        // --- 補丁生成與檔案偵測邏輯 ---

        /// <summary>
        /// 產生 cl_script.ini 的補丁位元組。改寫邏輯統一委派給 <see cref="ClScriptPatcher"/>
        ///（單一正本；先前此處有一份行為分歧的複本：還原時寫死 3/5/50/200/15000，
        /// 而非讀回備份中的真實原值）。行為換算說明：
        /// - 平衡模式的法術半徑目標值 = 各族祭司平衡表的 SpellRadius；ClScriptPatcher 以
        ///   「倍率 × 原始半徑」計算，故換算為 target / 500（原版半徑為 500）。
        ///   GER 祭司平衡表 SpellRadius 為 0，倍率 0 → 寫入 0，與既有出貨行為一致。
        /// - 還原（各選項為 false）時，一律寫回備份原檔中的原值。
        /// </summary>
        internal byte[] GetPatchedClScriptBytes(string gamePath, BackupManager backupManager, bool fastCiviProduction, bool infiniteMoraleChecked, bool balanceChecked)
        {
            return IniFeaturePatcher.BuildClScript(backupManager, fastCiviProduction, infiniteMoraleChecked, balanceChecked);
        }

        private byte[] GetPatchedRessBytes(BackupManager backupManager, bool freeProdChecked, bool freeUpgradeChecked, bool noSpellCostChecked)
        {
            return IniFeaturePatcher.BuildRess(backupManager, freeProdChecked, freeUpgradeChecked, noSpellCostChecked);
        }

        private byte[] GetPatchedObjdefBytes(BackupManager backupManager, PatchProfile options)
        {
            return ObjdefFeaturePatcher.Build(backupManager, options);
        }

        private Dictionary<string, byte[]> GetPatchedTeamDatBytes(BackupManager backupManager, bool maxPopulation)
        {
            var results = MaxPopulationFeature.Build(backupManager, maxPopulation);
            _logger.Log(maxPopulation ? string.Format(Loc.Get("SvcLogTeamDatApplied"), TeamDatPatcher.DefaultPopulationLimit, results.Count) : Loc.Get("SvcLogTeamDatRestored"));
            return results;
        }

        // --- 語言與 dgVoodoo2 補丁邏輯 ---

        // --- dgVoodoo2 專用核心邏輯 ---

        // --- 全方位修改狀態偵測方法 ---

    }
}
