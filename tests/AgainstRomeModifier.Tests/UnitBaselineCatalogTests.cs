using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class UnitBaselineCatalogTests
{
    [RequiresBackupZipFact]
    public void BackupManager_clear_invalidates_cached_unit_rows_and_stats()
    {
        using BackupZipGameFixture fixture = BackupZipGameFixture.Create();
        const string unitKey = "FigRomSch01_Bogen";

        Assert.Contains(unitKey, fixture.Backup.GetBackupUnitRows().Keys);
        Assert.True(fixture.Backup.GetOriginalStats(unitKey)[0] > 0);

        fixture.Backup.Clear();

        Assert.Empty(fixture.Backup.GetBackupUnitRows());
        Assert.Equal(new double[9], fixture.Backup.GetOriginalStats(unitKey));
    }

    [RequiresBackupZipFact]
    public void Auto_heal_invalidates_an_empty_unit_row_cache()
    {
        using BackupZipGameFixture fixture = BackupZipGameFixture.Create();
        const string unitKey = "FigRomSch01_Bogen";

        fixture.Backup.Clear();
        Assert.Empty(fixture.Backup.GetBackupUnitRows());

        fixture.Backup.TryAutoHealBackupFiles(fixture.RootPath);

        Assert.True(fixture.Backup.HasFile("SYSTEM/DATA_MP/DEFAULTS/objdef.dau"));
        Assert.Contains(unitKey, fixture.Backup.GetBackupUnitRows().Keys);
    }
}
