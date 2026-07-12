using AgainstRomeModifier.Core.Features.Bci;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class BciCacheIntegrationTests
{
    [RequiresBackupZipFact]
    public void FoodHealing_plans_in_shared_cache_and_writes_only_on_save_all()
    {
        using var fixture = BackupZipGameFixture.Create();
        string path = Path.Combine(fixture.RootPath, "SYSTEM", "CLAK", "SCRIPT", "ak_artillerie.bci");
        byte[] before = File.ReadAllBytes(path);
        var orchestrator = new EndlessAiOrchestrator();

        FoodHealingFeature.Apply(fixture.RootPath, true, fixture.Backup, orchestrator, new NullLogger());

        Assert.Equal(before, File.ReadAllBytes(path));
        using (var rollback = new FileRollbackScope())
        {
            orchestrator.SaveAll(fixture.RootPath, rollback);
            rollback.Commit();
        }
        Assert.NotEqual(before, File.ReadAllBytes(path));
        Assert.True(FoodHealingFeature.TryDetect(fixture.RootPath, out bool enabled));
        Assert.True(enabled);
    }
}
