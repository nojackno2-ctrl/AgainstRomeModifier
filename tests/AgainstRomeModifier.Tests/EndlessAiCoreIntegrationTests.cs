using AgainstRomeModifier.Core.Features;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class EndlessAiCoreIntegrationTests
{
    [Fact]
    public void Legacy_partial_selection_migrates_to_complete_core()
    {
        var profile = new PatchProfile();
        profile.EndlessAiModules["M3"] = true;

        profile.NormalizeCompositeValues();

        Assert.True(profile.Get(FeatureKeys.EndlessAiCore));
        Assert.True(profile.GetEndlessAiModule("Core"));
        Assert.True(profile.GetEndlessAiModule("M2"));
        Assert.True(profile.GetEndlessAiModule("M3"));
        Assert.True(profile.GetEndlessAiModule("M4"));
    }

    [Fact]
    public void Orchestrator_exposes_one_atomic_core_module()
    {
        var orchestrator = new EndlessAiOrchestrator();

        Assert.Contains(orchestrator.RespawnCore, orchestrator.UserModules);
        Assert.DoesNotContain(orchestrator.M2, orchestrator.UserModules);
        Assert.DoesNotContain(orchestrator.M3, orchestrator.UserModules);
        Assert.DoesNotContain(orchestrator.M4, orchestrator.UserModules);

        string[] expectedPatchIds = orchestrator.M2.Patches
            .Concat(orchestrator.M3.Patches)
            .Concat(orchestrator.M4.Patches)
            .Select(patch => patch.Id)
            .ToArray();
        Assert.Equal(expectedPatchIds, orchestrator.RespawnCore.Patches.Select(patch => patch.Id));
    }

    [RequiresBackupZipFact]
    public void Applying_core_enables_and_detects_all_three_lifecycle_modules()
    {
        using var fixture = BackupZipGameFixture.Create();
        var engine = new PatchEngine(new NullLogger());
        var profile = new PatchProfile();
        profile.Set(FeatureKeys.EndlessAiCore, true);

        using (var rollback = new FileRollbackScope())
        {
            engine.ApplyPatches(fixture.RootPath, profile, fixture.Backup, rollback);
            rollback.Commit();
        }

        var orchestrator = new EndlessAiOrchestrator();
        Assert.Equal(PatchState.Ultimate, orchestrator.DetectModule(fixture.RootPath, orchestrator.RespawnCore));
        Assert.Equal(PatchState.Ultimate, orchestrator.DetectModule(fixture.RootPath, orchestrator.M2));
        Assert.Equal(PatchState.Ultimate, orchestrator.DetectModule(fixture.RootPath, orchestrator.M3));
        Assert.Equal(PatchState.Ultimate, orchestrator.DetectModule(fixture.RootPath, orchestrator.M4));

        PatchProfile detected = engine.DetectCurrentPatchState(fixture.RootPath, fixture.Backup);
        Assert.True(detected.Get(FeatureKeys.EndlessAiCore));
        Assert.True(detected.GetEndlessAiModule("M2"));
        Assert.True(detected.GetEndlessAiModule("M3"));
        Assert.True(detected.GetEndlessAiModule("M4"));
    }

    [RequiresBackupZipFact]
    public void Detecting_a_partial_legacy_state_selects_core_for_next_apply_migration()
    {
        using var fixture = BackupZipGameFixture.Create();
        var orchestrator = new EndlessAiOrchestrator();
        orchestrator.ApplyModule(fixture.RootPath, orchestrator.M3, true);
        orchestrator.SaveAll(fixture.RootPath, rollback: null);

        Assert.Equal(PatchState.Legacy, orchestrator.DetectModule(fixture.RootPath, orchestrator.RespawnCore));

        var engine = new PatchEngine(new NullLogger());
        PatchProfile detected = engine.DetectCurrentPatchState(fixture.RootPath, fixture.Backup);
        Assert.True(detected.Get(FeatureKeys.EndlessAiCore));

        using (var rollback = new FileRollbackScope())
        {
            engine.ApplyPatches(fixture.RootPath, detected, fixture.Backup, rollback);
            rollback.Commit();
        }

        orchestrator.ClearCache();
        Assert.Equal(PatchState.Ultimate, orchestrator.DetectModule(fixture.RootPath, orchestrator.RespawnCore));
    }

    [RequiresBackupZipFact]
    public void Detecting_a_legacy_m6_state_selects_standalone_garrison_for_next_apply_migration()
    {
        using var fixture = BackupZipGameFixture.Create();
        var orchestrator = new EndlessAiOrchestrator();
        IEndlessPatch p8 = Assert.Single(orchestrator.M6.Patches, patch => patch.Id == "P8");

        foreach (string path in EndlessAiOrchestrator.ResolvePaths(fixture.RootPath, p8.TargetPattern))
        {
            BciScriptFile script = orchestrator.GetScriptFile(path);
            byte[] decompressed = script.DecompressedBytes;
            Assert.True(p8.Apply(ref decompressed, enabled: true));
            script.UpdateDecompressedBytes(decompressed);
        }
        orchestrator.SaveAll(fixture.RootPath, rollback: null);

        Assert.Equal(PatchState.Legacy, orchestrator.DetectModule(fixture.RootPath, orchestrator.M6));

        var engine = new PatchEngine(new NullLogger());
        PatchProfile detected = engine.DetectCurrentPatchState(fixture.RootPath, fixture.Backup);
        Assert.True(detected.RomanReinforcementGarrison);
        Assert.True(detected.Get(FeatureKeys.EndlessAiM6));

        using (var rollback = new FileRollbackScope())
        {
            engine.ApplyPatches(fixture.RootPath, detected, fixture.Backup, rollback);
            rollback.Commit();
        }

        orchestrator.ClearCache();
        Assert.Equal(PatchState.Ultimate, orchestrator.DetectModule(fixture.RootPath, orchestrator.M6));
    }
}
