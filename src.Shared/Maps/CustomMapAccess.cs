using System.Text.RegularExpressions;

namespace AgainstRomeModifier.Maps;

/// <summary>Editing requires a marked custom endless slot; a marker never makes an original map writable.</summary>
public static class CustomMapAccess
{
    private static readonly Regex CustomSlot = new(@"^ENDL_[0-9]{3}$", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);

    public static bool IsEditableDirectory(string mapDirectory)
    {
        string path = Path.TrimEndingDirectorySeparator(Path.GetFullPath(mapDirectory));
        string name = Path.GetFileName(path);
        string? parent = Path.GetDirectoryName(path);
        return CustomSlot.IsMatch(name) && int.Parse(name.AsSpan(5)) >= 5
            && parent is not null && Path.GetFileName(parent).Equals("MAPS", StringComparison.OrdinalIgnoreCase)
            && CustomMapManifest.IsCustomMapDirectory(path);
    }

    public static string RequireEditableDirectory(string mapDirectory, string? gamePath = null)
    {
        string path = Path.TrimEndingDirectorySeparator(Path.GetFullPath(mapDirectory));
        if (gamePath is not null && !StringComparer.OrdinalIgnoreCase.Equals(Path.GetDirectoryName(path), Path.GetFullPath(Path.Combine(gamePath, "MAPS"))))
            throw new InvalidOperationException("地圖資料夾不在選取的遊戲 MAPS 目錄，已取消儲存。");
        if (!IsEditableDirectory(path))
            throw new InvalidOperationException("編輯只允許 marker-backed 的 ENDL_005–999 自製地圖。");
        return path;
    }
}
