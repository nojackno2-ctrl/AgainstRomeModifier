using System.Globalization;
using System.Text.RegularExpressions;

namespace AgainstRomeModifier.Maps;

public sealed record EndlessMapInfo(int Slot, string DirectoryPath, bool IsCustom, string? DisplayName)
{
    public string Id => $"ENDL_{Slot:000}";
}

public sealed class EndlessMapCatalog
{
    private static readonly Regex MapDirectoryName = new("^ENDL_(\\d{3})$", RegexOptions.Compiled | RegexOptions.CultureInvariant | RegexOptions.IgnoreCase);

    public IReadOnlyList<EndlessMapInfo> List(string gamePath)
    {
        string mapsPath = Path.Combine(ValidateGamePath(gamePath), "MAPS");
        if (!Directory.Exists(mapsPath)) return Array.Empty<EndlessMapInfo>();

        return Directory.GetDirectories(mapsPath)
            .Select(path => (path, match: MapDirectoryName.Match(Path.GetFileName(path))))
            .Where(x => x.match.Success)
            .Select(x => new EndlessMapInfo(
                int.Parse(x.match.Groups[1].Value, CultureInfo.InvariantCulture),
                x.path,
                File.Exists(Path.Combine(x.path, CustomMapManifest.MarkerFileName)),
                TryReadTitle(x.path)))
            .OrderBy(x => x.Slot)
            .ToArray();
    }

    public int GetNextFreeSlot(string gamePath)
    {
        var occupied = List(gamePath).Select(x => x.Slot).ToHashSet();
        for (int slot = 5; slot <= 999; slot++)
            if (!occupied.Contains(slot)) return slot;
        throw new InvalidOperationException("沒有可用的 ENDL_005 至 ENDL_999 地圖槽位。");
    }

    public EndlessMapInfo Require(string gamePath, int slot)
    {
        if (slot is < 0 or > 999) throw new ArgumentOutOfRangeException(nameof(slot));
        return List(gamePath).SingleOrDefault(x => x.Slot == slot)
            ?? throw new DirectoryNotFoundException($"找不到 ENDL_{slot:000}。");
    }

    internal static string ValidateGamePath(string gamePath)
    {
        if (string.IsNullOrWhiteSpace(gamePath)) throw new ArgumentException("未提供遊戲路徑。", nameof(gamePath));
        string fullPath = Path.GetFullPath(gamePath);
        if (!Directory.Exists(fullPath)) throw new DirectoryNotFoundException("找不到遊戲路徑: " + fullPath);
        return fullPath;
    }

    private static string? TryReadTitle(string mapDirectory)
    {
        string path = Path.Combine(mapDirectory, "TEXT", "US", "briefing.put");
        if (!File.Exists(path)) return null;
        try { return PutTextDocument.Load(path).GetValue("briefing_titel_1"); }
        catch { return null; }
    }
}
