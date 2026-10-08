using System.Text.Json;

namespace AgainstRomeModifier.Maps;

public sealed record CustomMapEntry(int Slot, int SourceSlot, DateTimeOffset CreatedAt, string ToolVersion)
{
    public bool StandaloneLevel { get; init; }
}

public sealed class CustomMapManifest
{
    public const string FileName = "arm_custom_maps.json";
    public const string MarkerFileName = ".arm_custom_map";
    private readonly List<CustomMapEntry> _entries = new();
    public IReadOnlyList<CustomMapEntry> Entries => _entries;

    public static CustomMapManifest Load(string gamePath)
    {
        string path = Path.Combine(EndlessMapCatalog.ValidateGamePath(gamePath), "MAPS", FileName);
        if (!File.Exists(path)) return new CustomMapManifest();
        var entries = JsonSerializer.Deserialize<List<CustomMapEntry>>(File.ReadAllText(path)) ?? new List<CustomMapEntry>();
        var manifest = new CustomMapManifest();
        manifest._entries.AddRange(entries.Where(IsValid));
        return manifest;
    }

    public void Register(CustomMapEntry entry)
    {
        if (!IsValid(entry)) throw new ArgumentException("無效的自製地圖登記資料。", nameof(entry));
        _entries.RemoveAll(x => x.Slot == entry.Slot);
        _entries.Add(entry);
    }

    public void Remove(int slot) => _entries.RemoveAll(x => x.Slot == slot);

    public void Save(string gamePath, FileRollbackScope? rollback = null)
    {
        string mapsPath = Path.Combine(EndlessMapCatalog.ValidateGamePath(gamePath), "MAPS");
        Directory.CreateDirectory(mapsPath);
        string json = JsonSerializer.Serialize(_entries.OrderBy(x => x.Slot), Core.Services.JsonDefaults.Indented);
        Core.Services.SafeFileWriter.WriteAllBytes(Path.Combine(mapsPath, FileName), System.Text.Encoding.UTF8.GetBytes(json), rollback);
    }

    public static bool IsCustomMapDirectory(string path) => File.Exists(Path.Combine(path, MarkerFileName));

    /// <summary>沒有原無盡 main 的自製場景；原廠槽位即使有誤放標記也不排除。</summary>
    public static bool HasStandaloneLevel(string path)
    {
        if (!CustomMapAccess.IsEditableDirectory(path)) return false;
        try { return JsonSerializer.Deserialize<CustomMapEntry>(File.ReadAllText(Path.Combine(path, MarkerFileName)))?.StandaloneLevel == true; }
        catch (Exception ex) when (ex is IOException or JsonException)
        { System.Diagnostics.Debug.WriteLine($"讀取地圖脚本種類失敗 ({path}): {ex.Message}"); return false; }
    }

    /// <summary>Checks the owning map's marker, including files in that map's subdirectories.</summary>
    public static bool IsCustomMapFile(string mapsPath, string filePath)
    {
        string root = Path.TrimEndingDirectorySeparator(Path.GetFullPath(mapsPath)) + Path.DirectorySeparatorChar;
        string path = Path.GetFullPath(filePath);
        if (!path.StartsWith(root, StringComparison.OrdinalIgnoreCase)) return false;
        string relative = path[root.Length..];
        int separator = relative.IndexOf(Path.DirectorySeparatorChar);
        return separator > 0 && IsCustomMapDirectory(Path.Combine(root, relative[..separator]));
    }
    private static bool IsValid(CustomMapEntry entry) => entry.Slot is >= 5 and <= 999 && entry.SourceSlot is >= 0 and <= 999 && !string.IsNullOrWhiteSpace(entry.ToolVersion);
}
