using AgainstRomeModifier.Core.Services;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class CustomMapRestoreTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "ArmCustomRestore_" + Guid.NewGuid().ToString("N"));

    [Fact]
    public void Default_restore_preserves_custom_bytes_empty_directories_and_manifest_even_if_restore_touches_them()
    {
        string first = Map(5), last = Map(999);
        byte[] manifest = File.ReadAllBytes(ManifestPath);
        using var rollback = new FileRollbackScope();
        CustomMapRestoreService.Execute(_root, rollback, () =>
        {
            File.WriteAllText(Path.Combine(first, "unknown.bin"), "legacy overwrite");
            File.WriteAllText(Path.Combine(last, "new.bin"), "AI repair");
            Directory.Delete(Path.Combine(first, "Empty"));
        });
        rollback.Commit();
        AssertMaps(first, last);
        Assert.False(File.Exists(Path.Combine(last, "new.bin")));
        Assert.Equal(manifest, File.ReadAllBytes(ManifestPath));
    }

    [Fact]
    public void Delete_restore_removes_only_registered_custom_maps_and_can_commit()
    {
        string first = Map(5), last = Map(999), original = Map(0, false), foreign = Map(6, false);
        using var rollback = new FileRollbackScope();
        bool restored = false;
        CustomMapRestoreService.Execute(_root, rollback, () => restored = true, false);
        Assert.True(restored);
        rollback.Commit();
        Assert.False(Directory.Exists(first));
        Assert.False(Directory.Exists(last));
        AssertMaps(original, foreign);
        Assert.Empty(CustomMapManifest.Load(_root).Entries);
    }

    [Theory]
    [InlineData(true)]
    [InlineData(false)]
    public void Restore_failure_rolls_back_original_files_and_custom_maps(bool preserve)
    {
        string first = Map(5), last = Map(999);
        string original = Path.Combine(_root, "original.ini");
        File.WriteAllText(original, "before");
        using (var rollback = new FileRollbackScope())
        {
            Assert.Throws<IOException>(() => CustomMapRestoreService.Execute(_root, rollback, () =>
            {
                rollback.TrackFile(original);
                File.WriteAllText(original, "after");
                File.WriteAllText(Path.Combine(first, "unknown.bin"), "modified");
                throw new IOException("restore failed");
            }, preserve));
        }
        Assert.Equal("before", File.ReadAllText(original));
        AssertMaps(first, last);
        Assert.Equal(2, CustomMapManifest.Load(_root).Entries.Count);
    }

    [Fact]
    public void Later_transaction_failure_restores_deleted_maps_and_manifest()
    {
        string first = Map(5), last = Map(999);
        byte[] manifest = File.ReadAllBytes(ManifestPath);
        using (var rollback = new FileRollbackScope())
        {
            CustomMapRestoreService.Execute(_root, rollback, () => { }, false);
            Assert.False(Directory.Exists(first));
            Assert.False(Directory.Exists(last));
            // No commit: a later transaction step failed.
        }
        AssertMaps(first, last);
        Assert.Equal(manifest, File.ReadAllBytes(ManifestPath));
    }

    [Fact]
    public void Unregistered_marked_map_rejects_delete_before_restoring_or_removing_other_maps()
    {
        string first = Map(5), foreign = Map(6, true, false);
        bool called = false;
        using var rollback = new FileRollbackScope();
        Assert.Throws<InvalidOperationException>(() => CustomMapRestoreService.Execute(_root, rollback, () => called = true, false));
        Assert.False(called);
        AssertMaps(first, foreign);
        Assert.Single(CustomMapManifest.Load(_root).Entries);
    }

    private string ManifestPath => Path.Combine(_root, "MAPS", CustomMapManifest.FileName);

    [Theory]
    [InlineData(true)]
    [InlineData(false)]
    public async Task Actual_engine_refusal_of_unknown_exe_keeps_maps_with_either_option(bool preserve)
    {
        string first = Map(5), last = Map(999);
        File.WriteAllBytes(Path.Combine(_root, "Against_Rome.exe"), [0, 1, 2]);
        var logger = new SilentLogger();
        var engine = new PatchEngine(logger);
        await Assert.ThrowsAsync<InvalidDataException>(() => new PatchOperationRunner(logger.Log).ExecuteAsync(
            rollback => engine.RestoreOriginalFiles(_root, new BackupManager(logger), rollback, preserve),
            "start", "rollback", "done"));
        AssertMaps(first, last);
        Assert.Equal(2, CustomMapManifest.Load(_root).Entries.Count);
    }

    private sealed class SilentLogger : ILogger { public void Log(string message) { } }

    [Fact]
    public void Restore_orchestrator_does_not_open_custom_scripts_but_normal_ai_workflow_still_includes_them()
    {
        string map = Map(5);
        string scripts = Path.Combine(map, "SCRIPT");
        Directory.CreateDirectory(scripts);
        string script = Path.Combine(scripts, "ak_level.bci");
        File.WriteAllBytes(script, [0, 1, 2]);
        using var locked = new FileStream(script, FileMode.Open, FileAccess.ReadWrite, FileShare.None);
        Assert.False(new EndlessAiOrchestrator(true).ApplyMandatoryRepair(_root));
        Assert.Throws<IOException>(() => new EndlessAiOrchestrator().ApplyMandatoryRepair(_root));
    }

    [Theory]
    [InlineData(true)]
    [InlineData(false)]
    public void Actual_engine_can_restore_with_corrupt_custom_scripts_and_legacy_custom_team_backup(bool preserve)
    {
        string first = Map(5);
        Directory.CreateDirectory(Path.Combine(first, "SCRIPT"));
        File.WriteAllBytes(Path.Combine(first, "SCRIPT", "ak_level.bci"), [0, 1, 2]);
        File.WriteAllBytes(Path.Combine(first, "team.dat"), [3, 4, 5]);
        var exe = new byte[0x205000];
        var focus = Core.Patches.ExePatchModel.FocusOriginalBytes;
        Buffer.BlockCopy(focus, 0, exe, (int)Core.Patches.ExePatchModel.FocusPatchOffset, focus.Length);
        foreach (var site in Core.Patches.ExePatchModel.SpellAltarPatchSites)
            Buffer.BlockCopy(site.Original, 0, exe, (int)site.Offset, site.Original.Length);
        File.WriteAllBytes(Path.Combine(_root, "Against_Rome.exe"), exe);
        string scripts = Path.Combine(_root, "SYSTEM", "CLAK", "SCRIPT");
        Directory.CreateDirectory(scripts);
        foreach (var site in new (string Name, int Symbol)[] {
            ("ak_anfuehrer", 87), ("ak_artillerie", 69), ("ak_geisterreiter", 68), ("ak_kampfverband", 89),
            ("ak_krieger", 73), ("ak_kundschafterwolf", 77), ("ak_landtier", 72), ("ak_packpferd", 57),
            ("ak_priester", 92), ("ak_verbandswolf", 68), ("ak_zivilist", 83), ("ak_zivilverband", 87) })
        {
            int[] signature = [66, 1, 81, 10, 81, 98, 128, site.Symbol, 73, -3, 86];
            File.WriteAllBytes(Path.Combine(scripts, site.Name + ".bci"), signature.SelectMany(BitConverter.GetBytes).ToArray());
        }
        var logger = new SilentLogger();
        var backup = new BackupManager(logger);
        backup.SetBackupFile("MAPS/ENDL_005/team.dat", [6, 7, 8]);
        using var rollback = new FileRollbackScope();
        new PatchEngine(logger).RestoreOriginalFiles(_root, backup, rollback, preserve);
        rollback.Commit();
        if (preserve)
        {
            AssertMaps(first);
            Assert.Equal(new byte[] { 0, 1, 2 }, File.ReadAllBytes(Path.Combine(first, "SCRIPT", "ak_level.bci")));
            Assert.Equal(new byte[] { 3, 4, 5 }, File.ReadAllBytes(Path.Combine(first, "team.dat")));
        }
        else Assert.False(Directory.Exists(first));
    }

    [Fact]
    public void Delete_failure_after_first_map_restores_all_maps_and_manifest()
    {
        string first = Map(5), last = Map(999);
        byte[] manifest = File.ReadAllBytes(ManifestPath);
        using (var rollback = new FileRollbackScope())
        {
            // Simulate a stale marker after preflight, so the second deletion must refuse.
            Assert.Throws<InvalidOperationException>(() => CustomMapRestoreService.Execute(_root, rollback,
                () => File.Delete(Path.Combine(last, CustomMapManifest.MarkerFileName)), false));
            Assert.False(Directory.Exists(first));
        }
        AssertMaps(first, last);
        Assert.True(File.Exists(Path.Combine(last, CustomMapManifest.MarkerFileName)));
        Assert.Equal(manifest, File.ReadAllBytes(ManifestPath));
    }

    [Fact]
    public void Snapshot_failure_does_not_start_restore_or_delete_maps()
    {
        string first = Map(5), last = Map(999);
        bool called = false;
        using (var locked = new FileStream(Path.Combine(last, "unknown.bin"), FileMode.Open, FileAccess.ReadWrite, FileShare.None))
        using (var rollback = new FileRollbackScope())
            Assert.Throws<IOException>(() => CustomMapRestoreService.Execute(_root, rollback, () => called = true, false));
        Assert.False(called);
        AssertMaps(first, last);
        Assert.Equal(2, CustomMapManifest.Load(_root).Entries.Count);
    }

    [Fact]
    public async Task Operation_runner_rolls_back_custom_deletion_on_later_failure()
    {
        string first = Map(5), last = Map(999);
        var logs = new List<string>();
        await Assert.ThrowsAsync<IOException>(() => new PatchOperationRunner(logs.Add).ExecuteAsync(rollback =>
        {
            CustomMapRestoreService.Execute(_root, rollback, () => { }, false);
            throw new IOException("later failure");
        }, "start", "rollback", "done"));
        AssertMaps(first, last);
        Assert.Equal(2, CustomMapManifest.Load(_root).Entries.Count);
        Assert.Equal(new[] { "start", "rollback", "done" }, logs);
    }
    private string Map(int slot, bool marker = true, bool register = true)
    {
        string path = Path.Combine(_root, "MAPS", $"ENDL_{slot:000}");
        Directory.CreateDirectory(Path.Combine(path, "Empty"));
        File.WriteAllText(Path.Combine(path, "unknown.bin"), "original map bytes");
        if (marker) File.WriteAllText(Path.Combine(path, CustomMapManifest.MarkerFileName), "{}");
        if (marker && register)
        {
            var manifest = CustomMapManifest.Load(_root);
            manifest.Register(new(slot, 0, DateTimeOffset.UtcNow, "test"));
            manifest.Save(_root);
        }
        return path;
    }
    private static void AssertMaps(params string[] maps)
    {
        foreach (string map in maps)
        {
            Assert.Equal("original map bytes", File.ReadAllText(Path.Combine(map, "unknown.bin")));
            Assert.True(Directory.Exists(Path.Combine(map, "Empty")));
        }
    }
    public void Dispose() { if (Directory.Exists(_root)) Directory.Delete(_root, true); }
}
