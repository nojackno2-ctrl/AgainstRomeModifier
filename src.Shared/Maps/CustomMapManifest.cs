using System.Text.Json;

namespace AgainstRomeModifier.Maps;

public sealed record CustomMapEntry(int Slot, int SourceSlot, DateTimeOffset CreatedAt, string ToolVersion);

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
    private static bool IsValid(CustomMapEntry entry) => entry.Slot is >= 5 and <= 999 && entry.SourceSlot is >= 0 and <= 999 && !string.IsNullOrWhiteSpace(entry.ToolVersion);
}
