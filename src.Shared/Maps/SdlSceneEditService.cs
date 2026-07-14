namespace AgainstRomeModifier.Maps;

public static class SdlSceneEditService
{
    public static void SaveChanges(
        string mapDirectory,
        IReadOnlyList<MapSceneObject> savedObjects,
        IReadOnlyList<MapSceneObject> currentObjects,
        FileRollbackScope rollback)
    {
        ArgumentNullException.ThrowIfNull(rollback);
        string mapPath = ValidateCustomMapDirectory(mapDirectory);
        Dictionary<SceneKey, MapSceneObject> saved = Index(savedObjects);
        Dictionary<SceneKey, MapSceneObject> current = Index(currentObjects);
        if (saved.Count != current.Count || saved.Keys.Any(key => !current.ContainsKey(key)))
            throw new InvalidOperationException("SDL 場景物件集合已改變，請重新開啟地圖後再試。");

        foreach (IGrouping<string, MapSceneObject> fileGroup in current.Values
            .Where(item => HasChanged(saved[Key(item)], item))
            .GroupBy(item => item.SourceFile, StringComparer.OrdinalIgnoreCase))
        {
            string path = ResolveSdlPath(mapPath, fileGroup.Key);
            var document = SdlDocument.Load(path);
            foreach (MapSceneObject item in fileGroup)
            {
                document.SetObjectTeam(item.ObjectIndex, item.Team);
                document.SetObjectPosition(item.ObjectIndex, new SdlVector3(item.LocalX, item.LocalY, item.LocalZ));
            }
            document.Save(rollback);
        }
    }

    public static bool HasChanges(IReadOnlyList<MapSceneObject> baseline, IReadOnlyList<MapSceneObject> current)
    {
        Dictionary<SceneKey, MapSceneObject> before = Index(baseline);
        Dictionary<SceneKey, MapSceneObject> after = Index(current);
        return before.Count != after.Count || before.Any(pair => !after.TryGetValue(pair.Key, out MapSceneObject? item) || HasChanged(pair.Value, item));
    }

    private static Dictionary<SceneKey, MapSceneObject> Index(IReadOnlyList<MapSceneObject> objects)
    {
        var result = new Dictionary<SceneKey, MapSceneObject>();
        foreach (MapSceneObject item in objects)
        {
            if (item.ObjectIndex < 0) throw new InvalidDataException("SDL 場景物件缺少有效的 object 索引。");
            ValidateSceneObject(item);
            var key = Key(item);
            if (!result.TryAdd(key, item)) throw new InvalidDataException($"SDL 場景物件識別重複：{item.SourceFile} / {item.ObjectIndex}。");
        }
        return result;
    }

    private static bool HasChanged(MapSceneObject before, MapSceneObject after)
        => before.Team != after.Team || before.LocalX != after.LocalX || before.LocalY != after.LocalY || before.LocalZ != after.LocalZ;

    private static SceneKey Key(MapSceneObject item) => new(item.SourceFile.ToUpperInvariant(), item.ObjectIndex);

    private static void ValidateSceneObject(MapSceneObject item)
    {
        if (string.IsNullOrWhiteSpace(item.SourceFile) || item.SourceFile != Path.GetFileName(item.SourceFile) || !item.SourceFile.EndsWith(".sdl", StringComparison.OrdinalIgnoreCase))
            throw new InvalidDataException("SDL 場景來源檔名無效。");
        if (item.Team is < -1 or > 15) throw new InvalidDataException("SDL 場景物件隊伍必須介於 -1 與 15。");
        if (!float.IsFinite(item.LocalX) || !float.IsFinite(item.LocalY) || !float.IsFinite(item.LocalZ))
            throw new InvalidDataException("SDL 場景物件座標必須是有限數值。");
    }

    private static string ValidateCustomMapDirectory(string mapDirectory)
    {
        string fullPath = Path.GetFullPath(mapDirectory);
        string name = Path.GetFileName(fullPath);
        string? parentDirectory = Path.GetDirectoryName(fullPath);
        string parent = parentDirectory is null ? string.Empty : Path.GetFileName(parentDirectory);
        if (!CustomMapManifest.IsCustomMapDirectory(fullPath) ||
            !parent.Equals("MAPS", StringComparison.OrdinalIgnoreCase) ||
            !System.Text.RegularExpressions.Regex.IsMatch(name, @"^ENDL_(?<slot>\d{3})$", System.Text.RegularExpressions.RegexOptions.IgnoreCase) ||
            !int.TryParse(name.AsSpan(5), out int slot) || slot < 5)
            throw new InvalidOperationException("SDL 場景編輯只允許 marker-backed 的 ENDL_005–999 自製地圖。");
        return fullPath;
    }

    private static string ResolveSdlPath(string mapPath, string sourceFile)
    {
        string path = Path.GetFullPath(Path.Combine(mapPath, sourceFile));
        if (!Path.GetDirectoryName(path)!.Equals(mapPath, StringComparison.OrdinalIgnoreCase) || !File.Exists(path))
            throw new FileNotFoundException("找不到自製地圖的 SDL 場景檔。", path);
        return path;
    }

    private readonly record struct SceneKey(string SourceFile, int ObjectIndex);
}
