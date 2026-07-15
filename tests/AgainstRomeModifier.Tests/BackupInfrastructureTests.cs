using System.IO.Compression;
using System.Text;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class BackupInfrastructureTests
{
    [Fact]
    public void Zip_loader_reads_safe_entries_and_rejects_parent_segments()
    {
        using MemoryStream safeZip = CreateZip(("SYSTEM/test.ini", "value"));
        IReadOnlyDictionary<string, byte[]> files = BackupSourceLoader.LoadZip(safeZip);
        Assert.Equal("value", Encoding.UTF8.GetString(files["SYSTEM/test.ini"]));

        using MemoryStream unsafeZip = CreateZip(("../escape.bin", "bad"));
        InvalidDataException error = Assert.Throws<InvalidDataException>(
            () => BackupSourceLoader.LoadZip(unsafeZip));
        Assert.Contains("unsafe entry path", error.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void Validator_preserves_optional_partgeo_rule()
    {
        var files = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase)
        {
            ["Against_Rome.exe"] = [],
            ["SYSTEM/cl_script.ini"] = [],
            ["SYSTEM/cl_epara.ini"] = [],
            ["SYSTEM/ress.ini"] = [],
            ["SYSTEM/DATA_MP/DEFAULTS/objdef.dau"] = [],
            ["SYSTEM/CLMK/icon.ini"] = [],
            ["SYSTEM/CLAK/cl_scint.ini"] = [],
            ["SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci"] = [],
            ["MAPS/ENDL_001/team.dat"] = []
        };

        Assert.Empty(BackupValidator.FindMissing(files));
        Assert.DoesNotContain("SYSTEM/DATA_MP/DEFAULTS/partgeo.dau",
            BackupValidator.FindMissing(files));
    }

    [Fact]
    public void Original_file_validators_fail_closed_for_invalid_payloads()
    {
        byte[] invalid = [1, 2, 3, 4];

        Assert.False(OriginalFileValidator.IsExeOriginal(invalid));
        Assert.False(OriginalFileValidator.IsObjdefOriginal(invalid));
        Assert.False(OriginalFileValidator.IsPartgeoOriginal(invalid));
    }

    [RequiresBackupZipFact]
    public void Original_file_validators_accept_known_backup_baselines()
    {
        using BackupZipGameFixture fixture = BackupZipGameFixture.Create();

        Assert.True(OriginalFileValidator.IsExeOriginal(
            fixture.Backup.GetBackupBytes("Against_Rome.exe")));
        Assert.True(OriginalFileValidator.IsObjdefOriginal(
            fixture.Backup.GetBackupBytes("SYSTEM/DATA_MP/DEFAULTS/objdef.dau")));
        if (fixture.Backup.HasFile("SYSTEM/DATA_MP/DEFAULTS/partgeo.dau"))
        {
            Assert.True(OriginalFileValidator.IsPartgeoOriginal(
                fixture.Backup.GetBackupBytes("SYSTEM/DATA_MP/DEFAULTS/partgeo.dau")));
        }
    }

    [Fact]
    public void Clean_epara_baseline_round_trips_through_pfil()
    {
        Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);

        byte[] decompressed = GameLZSS.DecompressPfil(CleanEparaBaseline.CreateBytes());
        string text = Encoding.GetEncoding(1251).GetString(decompressed);

        Assert.Equal(BackupManager.GetCleanEparaText(), text);
        string normalized = text.Replace("\r\n", "\n", StringComparison.Ordinal);
        Assert.Contains("[ProjectileInitSpeedFactor]\n1.5", normalized, StringComparison.Ordinal);
        Assert.Contains("[ProjectileVarianceOnMove]\n0.5", normalized, StringComparison.Ordinal);
    }

    private static MemoryStream CreateZip(params (string Path, string Content)[] entries)
    {
        var stream = new MemoryStream();
        using (var archive = new ZipArchive(stream, ZipArchiveMode.Create, leaveOpen: true))
        {
            foreach ((string path, string content) in entries)
            {
                ZipArchiveEntry entry = archive.CreateEntry(path);
                using Stream destination = entry.Open();
                destination.Write(Encoding.UTF8.GetBytes(content));
            }
        }
        stream.Position = 0;
        return stream;
    }
}
