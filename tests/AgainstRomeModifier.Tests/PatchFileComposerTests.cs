using AgainstRomeModifier.Core.Features;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class PatchFileComposerTests
{
    [RequiresBackupZipFact]
    public void Compose_builds_target_files_without_writing_them()
    {
        using BackupZipGameFixture fixture = BackupZipGameFixture.Create();
        string objdefPath = Path.Combine(fixture.RootPath, "SYSTEM", "DATA_MP", "DEFAULTS", "objdef.dau");
        byte[] objdefBefore = File.ReadAllBytes(objdefPath);
        var profile = new PatchProfile
        {
            FocusLoss = true,
            FreeProduction = true,
            FreeUpgrade = true,
            NoSpellCost = true,
            MaxPopulation = true,
            RomanEndless = true,
            RangedRange3x = true,
            HousingCapacity20x = true,
            StorageCapacity10x = true,
            HqHp10x = true,
            FastBuildUpgradeRepair = true,
        };

        PatchFilePlan plan = new PatchFileComposer(new NullLogger()).Compose(
            fixture.RootPath, fixture.Backup, profile);

        Assert.Contains(Path.Combine(fixture.RootPath, "Against_Rome.exe"), plan.Files.Keys);
        Assert.Contains(objdefPath, plan.Files.Keys);
        Assert.Contains(Path.Combine(fixture.RootPath, "SYSTEM", "ress.ini"), plan.Files.Keys);
        Assert.Contains(Path.Combine(fixture.RootPath, "SYSTEM", "cl_script.ini"), plan.Files.Keys);
        Assert.Contains(plan.Files.Keys, path => path.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase));
        Assert.Equal(objdefBefore, File.ReadAllBytes(objdefPath));
    }

    [Fact]
    public void Compose_runs_injected_contributors_without_knowing_target_types()
    {
        var contributor = new StubContributor();
        var composer = new PatchFileComposer(new NullLogger(), new[] { contributor });

        PatchFilePlan plan = composer.Compose("root", new BackupManager(new NullLogger()), new PatchProfile());

        Assert.True(contributor.WasCalled);
        Assert.Equal(new byte[] { 1, 2, 3 }, plan.Files[Path.Combine("root", "SYSTEM", "stub.dat")]);
    }

    private sealed class StubContributor : IPatchFileContributor
    {
        public bool WasCalled { get; private set; }

        public void Contribute(PatchFileCompositionContext context)
        {
            WasCalled = true;
            context.Add("SYSTEM/stub.dat", new byte[] { 1, 2, 3 });
        }
    }

    private sealed class NullLogger : ILogger
    {
        public void Log(string message) { }
    }
}
