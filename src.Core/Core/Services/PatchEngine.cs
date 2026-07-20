using System.IO;
using AgainstRomeModifier.Core.Features;
using AgainstRomeModifier.Core.Features.Bci;
using AgainstRomeModifier.Core.Features.Install;

namespace AgainstRomeModifier.Core.Services;

public class PatchEngine
{
    private readonly ILogger _logger;

    public PatchEngine(ILogger logger) => _logger = logger;

    public bool IsDgVoodooInstalled(string gamePath) =>
        new DgVoodooFeature(_logger).IsInstalled(gamePath);

    public bool IsArgmTraceInstalled(string gamePath) =>
        new ArgmTraceFeature(_logger).IsInstalled(gamePath);

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

    // Retained as a compatibility endpoint; SafeFileWriter owns the implementation.
    public void SafeWriteAllBytes(string destination, byte[] bytes, FileRollbackScope? rollback = null) =>
        SafeFileWriter.WriteAllBytes(destination, bytes, rollback);

    public void ApplyPatches(
        string gamePath,
        PatchProfile profile,
        BackupManager backupManager,
        FileRollbackScope rollback)
    {
        profile.NormalizeCompositeValues();
        var context = new PatchContext();
        foreach (IFeatureModule module in FeatureRegistry.All)
            module.Plan(context, profile.Get(module.Id));
        PatchProfile options = context.Profile;

        // Restore and reapply share one cache so every BCI file is decompressed and saved only once.
        var orchestrator = new EndlessAiOrchestrator();
        _logger.Log(Loc.Get("SvcLogPreApplyRestore"));
        new PatchRestoreService(_logger).RestoreCategories(
            gamePath,
            backupManager,
            rollback,
            new[] { FeatureCategory.Stats, FeatureCategory.Compat, FeatureCategory.Language },
            orchestrator,
            saveOrchestrator: false);

        PatchFilePlan filePlan = new PatchFileComposer(_logger).Compose(
            gamePath, backupManager, options);

        orchestrator.SetVillageGarrisonQuotaMultiplier(options.VillageGarrisonQuotaMultiplier);
        foreach (var module in orchestrator.UserModules)
            orchestrator.ApplyModule(gamePath, module, options.GetEndlessAiModule(module.Id));
        orchestrator.ApplyMandatoryRepair(gamePath);
        FoodHealingFeature.Apply(
            gamePath, options.FoodHealing10x, backupManager, orchestrator, _logger);
        RetiredDefaultSpecialArrowsMigration.RestoreIfPresent(gamePath, orchestrator, _logger);
        CiviProduce20Feature.RestoreAkNpcOriginal(gamePath, orchestrator);

        foreach ((string path, byte[] bytes) in filePlan.Files)
            SafeWriteAllBytes(path, bytes, rollback);
        orchestrator.SaveAll(gamePath, rollback);

        new LanguagePackFeature(_logger).Apply(gamePath, options.ToEnglish, rollback);
        new DgVoodooFeature(_logger).Apply(
            gamePath,
            options.DgVoodoo,
            rollback,
            nativeWidescreenWindow: options.NativeWidescreen1920x1080);
        new ArgmTraceFeature(_logger).Apply(gamePath, options.ArgmTrace, rollback);
    }

    public void RunStartupSafeMigrations(
        string gamePath,
        BackupManager backupManager,
        FileRollbackScope rollback)
    {
        var orchestrator = new EndlessAiOrchestrator();
        orchestrator.ApplyMandatoryRepair(gamePath);

        if (!FoodHealingFeature.TryDetect(gamePath, out bool foodHealingEnabled))
            foodHealingEnabled = false;
        FoodHealingFeature.Apply(
            gamePath, foodHealingEnabled, backupManager, orchestrator, _logger);
        RetiredDefaultSpecialArrowsMigration.RestoreIfPresent(gamePath, orchestrator, _logger);
        CiviProduce20Feature.RestoreAkNpcOriginal(gamePath, orchestrator);
        orchestrator.SaveAll(gamePath, rollback);
    }

    public void RestoreOriginalFiles(
        string gamePath,
        BackupManager backupManager,
        FileRollbackScope rollback) =>
        new PatchRestoreService(_logger).RestoreCategories(
            gamePath,
            backupManager,
            rollback,
            new[] { FeatureCategory.Stats, FeatureCategory.Compat, FeatureCategory.Language });

    public void RestoreStatsOnly(
        string gamePath,
        BackupManager backupManager,
        FileRollbackScope rollback) =>
        new PatchRestoreService(_logger).RestoreCategories(
            gamePath, backupManager, rollback, new[] { FeatureCategory.Stats });

    public void RestoreCompatOnly(
        string gamePath,
        BackupManager backupManager,
        FileRollbackScope rollback) =>
        new PatchRestoreService(_logger).RestoreCategories(
            gamePath, backupManager, rollback, new[] { FeatureCategory.Compat });

    public void RestoreLanguageOnly(string gamePath, FileRollbackScope rollback) =>
        new PatchRestoreService(_logger).RestoreCategories(
            gamePath, null, rollback, new[] { FeatureCategory.Language });
}
