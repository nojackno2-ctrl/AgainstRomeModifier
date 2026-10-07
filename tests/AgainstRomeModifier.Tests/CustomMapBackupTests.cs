using AgainstRomeModifier.Core.Services;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class CustomMapBackupTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "ArmCustomBackup_" + Guid.NewGuid().ToString("N"));

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Directory_backup_excludes_custom_map_files_and_existing_baks_including_subdirectories(bool nested)
    {
        string original = TeamFile("ENDL_000", false, false);
        string custom = TeamFile("ENDL_005", true, nested);
        File.WriteAllBytes(custom + ".bak", [8, 9]);
        string extra = TeamFile("ENDL_999", true, nested);
        byte[] originalBytes = File.ReadAllBytes(original);
        using var locked = new FileStream(extra, FileMode.Open, FileAccess.ReadWrite, FileShare.None);
        var loader = new GameDirectoryBackupLoader(new SilentLogger(), () => [1]);
        Assert.True(loader.TryLoad(_root, true, out var files));
        Assert.Equal(originalBytes, files["MAPS/ENDL_000/team.dat"]);
        Assert.Equal(originalBytes, File.ReadAllBytes(original + ".bak"));
        Assert.DoesNotContain(files.Keys, key => key.StartsWith("MAPS/ENDL_005/", StringComparison.OrdinalIgnoreCase));
        Assert.DoesNotContain(files.Keys, key => key.StartsWith("MAPS/ENDL_999/", StringComparison.OrdinalIgnoreCase));
        Assert.Equal(new byte[] { 8, 9 }, File.ReadAllBytes(custom + ".bak"));
        Assert.False(File.Exists(extra + ".bak"));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Autoheal_excludes_custom_map_files_and_never_creates_their_baks(bool nested)
    {
        string original = TeamFile("ENDL_004", false, false);
        File.WriteAllBytes(original + ".bak", [5, 6]);
        string custom = TeamFile("ENDL_005", true, nested);
        using var locked = new FileStream(custom, FileMode.Open, FileAccess.ReadWrite, FileShare.None);
        var files = new Dictionary<string, byte[]>();
        var logger = new SilentLogger();
        Assert.True(new BackupAutoHealer(files, logger, () => [1]).Heal(_root));
        Assert.Equal(new byte[] { 5, 6 }, files["MAPS/ENDL_004/team.dat"]);
        Assert.DoesNotContain(files.Keys, key => key.StartsWith("MAPS/ENDL_005/", StringComparison.OrdinalIgnoreCase));
        Assert.False(File.Exists(custom + ".bak"));
        Assert.DoesNotContain(logger.Messages, message => message.Contains(custom, StringComparison.OrdinalIgnoreCase));
    }

    private string TeamFile(string id, bool marker, bool nested)
    {
        string map = Path.Combine(_root, "MAPS", id);
        Directory.CreateDirectory(map);
        if (marker) File.WriteAllText(Path.Combine(map, CustomMapManifest.MarkerFileName), "{}");
        string directory = nested ? Path.Combine(map, "Extra") : map;
        Directory.CreateDirectory(directory);
        string file = Path.Combine(directory, "team.dat");
        File.WriteAllBytes(file, [1, 2, 3]);
        return file;
    }

    [Fact]
    public void Marker_scope_is_the_owning_map_and_does_not_extend_to_neighbors_or_outside_maps()
    {
        string custom = TeamFile("ENDL_005", true, true);
        string original = TeamFile("ENDL_000", false, true);
        string maps = Path.Combine(_root, "MAPS");
        File.WriteAllText(Path.Combine(maps, CustomMapManifest.MarkerFileName), "{}");
        Assert.True(CustomMapManifest.IsCustomMapFile(maps + Path.DirectorySeparatorChar, custom));
        Assert.False(CustomMapManifest.IsCustomMapFile(maps, original));
        string neighbor = Path.Combine(_root, "MAPS-other", "ENDL_005", "team.dat");
        Directory.CreateDirectory(Path.GetDirectoryName(neighbor)!);
        File.WriteAllText(Path.Combine(Path.GetDirectoryName(neighbor)!, CustomMapManifest.MarkerFileName), "{}");
        Assert.False(CustomMapManifest.IsCustomMapFile(maps, neighbor));
        Assert.False(CustomMapManifest.IsCustomMapFile(maps, Path.Combine(maps, "team.dat")));
    }

    private sealed class SilentLogger : ILogger
    {
        public List<string> Messages { get; } = [];
        public void Log(string message) => Messages.Add(message);
    }
    public void Dispose() { if (Directory.Exists(_root)) Directory.Delete(_root, true); }
}
