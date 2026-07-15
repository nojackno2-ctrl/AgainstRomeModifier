using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using AgainstRomeModifier.Core.Features;
using AgainstRomeModifier.Core.Features.Bci;
using AgainstRomeModifier.Core.Features.Exe;
using AgainstRomeModifier.Core.Features.Install;
using AgainstRomeModifier.Core.Features.Map;
using AgainstRomeModifier.Core.Patches;

namespace AgainstRomeModifier.Core.Services;

internal sealed class PatchRestoreService
{
    private readonly ILogger _logger;

    internal PatchRestoreService(ILogger logger) => _logger = logger;

    internal void RestoreCategories(
        string gamePath,
        BackupManager? backupManager,
        FileRollbackScope rollback,
        IEnumerable<FeatureCategory> categories,
        EndlessAiOrchestrator? sharedOrchestrator = null,
        bool saveOrchestrator = true)
    {
        HashSet<FeatureCategory> requested = categories.ToHashSet();
        bool restoreStats = requested.Contains(FeatureCategory.Stats);
        bool restoreCompat = requested.Contains(FeatureCategory.Compat);
        bool restoreLanguage = requested.Contains(FeatureCategory.Language);
        var patchedFiles = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
        EndlessAiOrchestrator? orchestrator = null;

        if (restoreCompat)
        {
            string exePath = Path.Combine(gamePath, "Against_Rome.exe");
            byte[] exeBytes = File.ReadAllBytes(exePath);
            bool keepRomanEndless = !restoreStats &&
                ExePatchModel.GetRomanEndlessPatchState(exeBytes) == ExeRomanEndlessPatchState.Patched;
            if (ExeFeaturePatcher.Apply(exeBytes, false, false, false, keepRomanEndless, 1, _logger))
                patchedFiles[exePath] = exeBytes;

            orchestrator = sharedOrchestrator ?? new EndlessAiOrchestrator();
            foreach (EndlessAiModule module in orchestrator.UserModules)
                orchestrator.ApplyModule(gamePath, module, false);
            orchestrator.ApplyMandatoryRepair(gamePath);
        }

        if (restoreStats)
        {
            string exePath = Path.Combine(gamePath, "Against_Rome.exe");
            byte[] exeBytes = patchedFiles.TryGetValue(exePath, out byte[]? pendingExe)
                ? pendingExe
                : File.ReadAllBytes(exePath);
            bool exeChanged = false;
            if (!restoreCompat)
                exeChanged |= ExeFeaturePatcher.ApplyRomanEndless(exeBytes, false, _logger);
            exeChanged |= ExeFeaturePatcher.ApplyCiviProduce20(exeBytes, false, _logger);
            exeChanged |= ExeFeaturePatcher.ApplyUnitRecruit20(exeBytes, false, _logger);
            if (exeChanged)
                patchedFiles[exePath] = exeBytes;

            RestoreStatsFiles(gamePath,
                backupManager ?? throw new ArgumentNullException(nameof(backupManager)), rollback);

            orchestrator ??= sharedOrchestrator ?? new EndlessAiOrchestrator();
            FoodHealingFeature.Apply(gamePath, false, backupManager, orchestrator, _logger);
            CiviProduce20Feature.RestoreAkNpcOriginal(gamePath, orchestrator);
        }

        foreach ((string path, byte[] bytes) in patchedFiles)
            SafeFileWriter.WriteAllBytes(path, bytes, rollback);

        if (orchestrator != null && saveOrchestrator)
            orchestrator.SaveAll(gamePath, rollback);
        if (restoreLanguage)
            new LanguagePackFeature(_logger).Apply(gamePath, false, rollback);
        if (restoreCompat)
            new DgVoodooFeature(_logger).Apply(gamePath, false, rollback);
    }

    private void RestoreStatsFiles(string gamePath, BackupManager backupManager, FileRollbackScope rollback)
    {
        RestoreMemoryFile(backupManager, "SYSTEM/cl_script.ini", Path.Combine(gamePath, @"SYSTEM\cl_script.ini"), rollback);
        RestoreMemoryFile(backupManager, "SYSTEM/cl_epara.ini", Path.Combine(gamePath, @"SYSTEM\cl_epara.ini"), rollback);
        RestoreMemoryFile(backupManager, "SYSTEM/CLAK/cl_scint.ini", Path.Combine(gamePath, @"SYSTEM\CLAK\cl_scint.ini"), rollback);
        RestoreMemoryFile(backupManager, "SYSTEM/ress.ini", Path.Combine(gamePath, @"SYSTEM\ress.ini"), rollback);
        RestoreMemoryFile(backupManager, "SYSTEM/DATA_MP/DEFAULTS/objdef.dau", Path.Combine(gamePath, @"SYSTEM\DATA_MP\DEFAULTS\objdef.dau"), rollback);
        RestoreMemoryFile(backupManager, "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau", Path.Combine(gamePath, @"SYSTEM\DATA_MP\DEFAULTS\partgeo.dau"), rollback);

        foreach ((string key, byte[] bytes) in backupManager.BackupFiles)
        {
            if (!key.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) ||
                !key.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase))
                continue;

            string destination = Path.Combine(gamePath, key.Replace('/', '\\'));
            if (!File.Exists(destination)) continue;
            byte[] restoredBytes = TeamDatPatcher.GetPatchedBytes(bytes, new TeamDatOptions(false));
            SafeFileWriter.WriteAllBytes(destination, restoredBytes, rollback);
            _logger.Log(string.Format(Loc.Get("SvcLogRestoredPopulation"), destination));
        }
    }

    private void RestoreMemoryFile(
        BackupManager backupManager,
        string key,
        string destination,
        FileRollbackScope rollback)
    {
        if (!backupManager.BackupFiles.TryGetValue(key, out byte[]? bytes)) return;
        SafeFileWriter.WriteAllBytes(destination, bytes, rollback);
        _logger.Log(string.Format(Loc.Get("SvcLogRestoredFile"), destination));
    }
}
