using System.Text.RegularExpressions;

namespace AgainstRomeModifier.Maps;

public sealed record GameMapInfo(string Id, string DirectoryPath, bool IsCustom, string? DisplayName, string Category)
{
    public int? EndlessSlot => Id.StartsWith("ENDL_", StringComparison.OrdinalIgnoreCase) && int.TryParse(Id.AsSpan(5), out int slot) ? slot : null;
}

public sealed class GameMapCatalog
{
    private static readonly Regex SafeMapId = new("^[A-Za-z0-9_]+$", RegexOptions.Compiled | RegexOptions.CultureInvariant);

    public IReadOnlyList<GameMapInfo> List(string gamePath)
    {
        string mapsPath = Path.Combine(EndlessMapCatalog.ValidateGamePath(gamePath), "MAPS");
        if (!Directory.Exists(mapsPath)) return Array.Empty<GameMapInfo>();
        return Directory.GetDirectories(mapsPath)
            .Where(path => SafeMapId.IsMatch(Path.GetFileName(path)))
            .Where(path => File.Exists(Path.Combine(path, "boden.txt")) && File.Exists(Path.Combine(path, "minimap.bmp")))
            .Select(path => new GameMapInfo(Path.GetFileName(path), path,
                CustomMapManifest.IsCustomMapDirectory(path), TryReadTitle(path), Category(Path.GetFileName(path))))
            .OrderBy(x => CategoryOrder(x.Category)).ThenBy(x => x.Id, StringComparer.OrdinalIgnoreCase)
            .ToArray();
    }

    public GameMapInfo Require(string gamePath, string mapId)
    {
        if (!SafeMapId.IsMatch(mapId)) throw new ArgumentException("無效的地圖代號。", nameof(mapId));
        return List(gamePath).SingleOrDefault(x => StringComparer.OrdinalIgnoreCase.Equals(x.Id, mapId))
            ?? throw new DirectoryNotFoundException("找不到地圖: " + mapId);
    }

    private static string Category(string id) => id.ToUpperInvariant() switch
    {
        string value when value.StartsWith("KAMP_", StringComparison.Ordinal) => "劇情戰役",
        string value when value.StartsWith("HIST_", StringComparison.Ordinal) => "歷史戰役",
        string value when value.StartsWith("TUTOR_", StringComparison.Ordinal) => "教學",
        string value when value.StartsWith("MP_", StringComparison.Ordinal) => "多人地圖",
        string value when value.StartsWith("ENDL_", StringComparison.Ordinal) => "無盡模式",
        _ => "其他地圖"
    };

    private static int CategoryOrder(string category) => category switch
    {
        "劇情戰役" => 0, "歷史戰役" => 1, "教學" => 2, "無盡模式" => 3, "多人地圖" => 4, _ => 5
    };

    private static string? TryReadTitle(string mapDirectory)
    {
        string path = Path.Combine(mapDirectory, "TEXT", "US", "briefing.put");
        if (!File.Exists(path)) return null;
        try { return PutTextDocument.Load(path).GetValue("briefing_titel_1"); }
        catch (Exception ex) {
            System.Diagnostics.Debug.WriteLine($"讀取地圖標題失敗 ({path}): {ex.Message}");
            return null;
        }
    }
}
