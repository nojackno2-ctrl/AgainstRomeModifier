using AgainstRomeModifier.Core.Features;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class UnitStatsProjectionServiceTests
{
    [RequiresBackupZipFact]
    public void Project_applies_independent_display_modifiers_after_baseline_merge()
    {
        using BackupZipGameFixture fixture = BackupZipGameFixture.Create();
        const string key = "FigKelPri00_Priester";
        double[] original = fixture.Backup.GetOriginalStats(key);
        double[] balanced = fixture.Backup.GetDefaultBalancedStats(key);
        var profile = new PatchProfile
        {
            Balance = true,
            UnitMovementSpeed2x = true,
            SpellEntireMap = true,
            SpellRange3x = true,
            CustomUnitStats = new Dictionary<string, double[]> { [key] = (double[])original.Clone() },
        };

        double[] projected = new UnitStatsProjectionService(fixture.Backup).Project(key, profile);

        Assert.Equal(balanced[4] * 2.0, projected[4]);
        Assert.Equal(30000.0, projected[7]);
        Assert.Equal(balanced[8] * 3.0, projected[8]);
    }

    [RequiresBackupZipFact]
    public void Editor_normalization_restores_independent_fields_without_mutating_input()
    {
        using BackupZipGameFixture fixture = BackupZipGameFixture.Create();
        const string key = "FigKelPri00_Priester";
        double[] original = fixture.Backup.GetOriginalStats(key);
        double[] input = { 111, 22, 33, 44, 999, 888, 77, 666, 555 };

        double[] normalized = new UnitStatsEditorService(fixture.Backup).NormalizeIndependentFields(key, input);

        Assert.Equal(999, input[4]);
        Assert.Equal(original[4], normalized[4]);
        Assert.Equal(original[5], normalized[5]);
        Assert.Equal(original[7], normalized[7]);
        Assert.Equal(original[8], normalized[8]);
        Assert.Equal(111, normalized[0]);
        Assert.Equal(77, normalized[6]);
    }
}
