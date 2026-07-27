using AgainstRomeModifier.Core.Patches;

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
    [RequiresBackupZipFact]
    public void Temp_PrintObjdefHeader()
    {
        string path = @"c:\離線儲存\程式設計\Against_Rome_Modifier\遊戲原始檔案\SYSTEM\DATA_MP\DEFAULTS\objdef.dau";
        byte[] bytes = System.IO.File.ReadAllBytes(path);
        byte[] decompressed = GameLZSS.DecompressPfil(bytes);
        string text = PatchText.GameEncoding.GetString(decompressed);
        string lineEnding = text.Contains("\r\n", StringComparison.Ordinal) ? "\r\n" : "\n";
        string[] lines = text.Split(new[] { lineEnding }, StringSplitOptions.None);
        
        var sb = new System.Text.StringBuilder();
        string[] headers = PatchText.ParseCsvLine(lines[0]);
        for (int i = 0; i < headers.Length; i++)
        {
            sb.AppendLine($"Index {i}: {headers[i]}");
        }
        System.IO.File.WriteAllText(@"c:\離線儲存\程式設計\Against_Rome_Modifier\scratch\objdef_headers.txt", sb.ToString());
    }
}









