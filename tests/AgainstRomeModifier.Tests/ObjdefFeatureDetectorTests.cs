using AgainstRomeModifier.Core.Features;
using AgainstRomeModifier.Core.Features.Objdef;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class ObjdefFeatureDetectorTests
{
    [RequiresBackupZipFact]
    public void Detector_reads_all_objdef_backed_features_from_composer_output()
    {
        using BackupZipGameFixture fixture = BackupZipGameFixture.Create();
        var enabled = new PatchProfile
        {
            AllUnitsEntireMapVision = true,
            RangedRange3x = true,
            UnitMovementSpeed2x = true,
            VillagerMovementSpeed5x = true,
            SpellEntireMap = true,
            ProjectileArcHeight = true,
            HousingCapacity20x = true,
            StorageCapacity10x = true,
            HqHp10x = true,
            FastBuildUpgradeRepair = true,
            LeaderGlory = true,
            NoRunHpLoss = true,
        };
        byte[] patched = ObjdefFeaturePatcher.Build(fixture.Backup, enabled);
        string path = Path.Combine(fixture.RootPath, "SYSTEM", "DATA_MP", "DEFAULTS", "objdef.dau");
        File.WriteAllBytes(path, patched);

        var detected = new PatchProfile();
        new ObjdefFeatureDetector(new NullLogger()).Detect(fixture.RootPath, fixture.Backup, detected);

        Assert.True(detected.AllUnitsEntireMapVision);
        Assert.True(detected.RangedRange3x);
        Assert.True(detected.UnitMovementSpeed2x);
        Assert.True(detected.VillagerMovementSpeed5x);
        Assert.True(detected.SpellEntireMap);
        Assert.True(detected.ProjectileArcHeight);
        Assert.True(detected.HousingCapacity20x);
        Assert.True(detected.StorageCapacity10x);
        Assert.True(detected.HqHp10x);
        Assert.True(detected.FastBuildUpgradeRepair);
        Assert.True(detected.LeaderGlory);
        Assert.True(detected.NoRunHpLoss);
        Assert.False(detected.Balance);
    }

    private sealed class NullLogger : ILogger
    {
        public void Log(string message) { }
    }
}
