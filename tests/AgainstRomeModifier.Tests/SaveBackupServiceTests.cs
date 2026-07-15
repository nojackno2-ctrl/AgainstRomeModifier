using System.IO.Compression;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class SaveBackupServiceTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "AgainstRomeSaveTests_" + Guid.NewGuid().ToString("N"));

    [Fact]
    public void Create_scan_preview_restore_and_delete_round_trip()
    {
        string game = Path.Combine(_root, "game");
        string backupDirectory = Path.Combine(_root, "backups");
        string save = Path.Combine(game, "SAVE", "Slot_A");
        Directory.CreateDirectory(save);
        byte[] original = SyntheticFixture.Pfil("[titel]\r\nOriginal title\r\n[orglevelname]\r\nENDL_000\r\n");
        File.WriteAllBytes(Path.Combine(save, "save.ini"), original);
        File.WriteAllBytes(Path.Combine(save, "savepic.tga"), new byte[] { 1, 2, 3 });
        var service = new SaveBackupService(backupDirectory);

        SaveBackupCatalog before = service.Scan(game);
        GameSaveInfo gameSave = Assert.Single(before.Saves);
        Assert.True(gameSave.Parsed);
        Assert.Equal("Original title", gameSave.Title);

        DateTime timestamp = new(2026, 7, 15, 12, 34, 56, DateTimeKind.Local);
        string fileName = service.CreateBackup(game, "Slot_A", "Original title", "ENDL_000", timestamp);
        SaveBackupInfo backup = Assert.Single(service.Scan(game).Backups);
        Assert.Equal(fileName, backup.FileName);
        Assert.Equal("Slot_A", backup.OrigFolder);
        Assert.Equal("2026-07-15 12:34:56", backup.BackupTime);
        Assert.Equal(new byte[] { 1, 2, 3 }, service.ReadBackupPreview(fileName));

        File.WriteAllBytes(Path.Combine(save, "save.ini"), SyntheticFixture.Pfil("changed"));
        SaveRestoreResult restore = service.RestoreBackup(game, fileName, "Slot_A");
        Assert.Empty(restore.CleanupWarnings);
        Assert.Equal(original, File.ReadAllBytes(Path.Combine(save, "save.ini")));

        service.DeleteBackup(fileName);
        Assert.Empty(service.Scan(game).Backups);
        service.DeleteSave(game, "Slot_A");
        Assert.False(Directory.Exists(save));
    }

    [Fact]
    public void Scan_reads_legacy_file_name_when_manifest_is_absent()
    {
        string game = Path.Combine(_root, "game");
        string backupDirectory = Path.Combine(_root, "backups");
        Directory.CreateDirectory(backupDirectory);
        string path = Path.Combine(backupDirectory, "Backup_Slot_With_Underscore_20260715_010203.zip");
        using (ZipArchive archive = ZipFile.Open(path, ZipArchiveMode.Create))
        {
            ZipArchiveEntry entry = archive.CreateEntry("save.ini");
            using Stream stream = entry.Open();
            byte[] bytes = SyntheticFixture.Pfil("[titel]\nLegacy\n[orglevelname]\nHIST_001\n");
            stream.Write(bytes);
        }

        SaveBackupInfo backup = Assert.Single(new SaveBackupService(backupDirectory).Scan(game).Backups);

        Assert.Equal("Slot_With_Underscore", backup.OrigFolder);
        Assert.Equal("2026-07-15 01:02:03", backup.BackupTime);
        Assert.Equal("Legacy", backup.Title);
    }

    [Fact]
    public void Path_operations_reject_non_simple_names()
    {
        var service = new SaveBackupService(Path.Combine(_root, "backups"));
        Assert.Throws<InvalidDataException>(() => service.DeleteBackup("../outside.zip"));
        Assert.Throws<InvalidDataException>(() => service.DeleteSave(Path.Combine(_root, "game"), "../outside"));
    }

    [Fact]
    public void Restore_rejects_archive_without_save_ini_and_preserves_existing_save()
    {
        string game = Path.Combine(_root, "game");
        string backupDirectory = Path.Combine(_root, "backups");
        string save = Path.Combine(game, "SAVE", "Slot_A");
        Directory.CreateDirectory(save);
        byte[] original = SyntheticFixture.Pfil("original");
        File.WriteAllBytes(Path.Combine(save, "save.ini"), original);
        Directory.CreateDirectory(backupDirectory);
        string badArchive = Path.Combine(backupDirectory, "bad.zip");
        using (ZipArchive archive = ZipFile.Open(badArchive, ZipArchiveMode.Create))
        {
            ZipArchiveEntry entry = archive.CreateEntry("unrelated.txt");
            using var writer = new StreamWriter(entry.Open());
            writer.Write("not a save");
        }

        var service = new SaveBackupService(backupDirectory);
        Assert.Throws<InvalidDataException>(() => service.RestoreBackup(game, "bad.zip", "Slot_A"));

        Assert.Equal(original, File.ReadAllBytes(Path.Combine(save, "save.ini")));
        Assert.Empty(Directory.GetDirectories(Path.Combine(game, "SAVE"), ".restore_*"));
    }

    public void Dispose()
    {
        try { if (Directory.Exists(_root)) Directory.Delete(_root, true); } catch { }
    }
}
