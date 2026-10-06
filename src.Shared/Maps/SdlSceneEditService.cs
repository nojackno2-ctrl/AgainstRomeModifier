namespace AgainstRomeModifier.Maps;

/// <summary>
/// 以既有物件為模板新增的 SDL 物件；模板欄位（namedef/def/nation 等）原樣沿用，只覆寫隊伍、相對座標與（若提供）角度。
/// <paramref name="TargetFile"/> 為 null 時寫入模板所在檔；否則把模板物件新增到同地圖的另一個聚落檔（自由新增）。
/// </summary>
public sealed record SdlSceneObjectAddition(string SourceFile, int TemplateIndex, int Team, float LocalX, float LocalY, float LocalZ,
    float? Angle = null, string? TargetFile = null)
{
    public string EffectiveTargetFile => TargetFile ?? SourceFile;
}

/// <summary>暫存刪除既有 SDL 物件；儲存時移除該 [objectNNNN] 區塊並重新連續編號。</summary>
public sealed record SdlSceneObjectRemoval(string SourceFile, int ObjectIndex);

/// <summary>整體平移一個聚落：只改 [settlement] refpos，所有物件的相對 pos 不變，因此整個聚落一起移動。</summary>
public sealed record SdlSettlementTranslation(string SourceFile, float DeltaX, float DeltaY, float DeltaZ);

public static class SdlSceneEditService
{
    public static void SaveChanges(
        string mapDirectory,
        IReadOnlyList<MapSceneObject> savedObjects,
        IReadOnlyList<MapSceneObject> currentObjects,
        FileRollbackScope rollback,
        IReadOnlyList<SdlSceneObjectRemoval>? removals = null,
        IReadOnlyList<SdlSceneObjectAddition>? additions = null,
        IReadOnlyList<SdlSettlementTranslation>? translations = null)
    {
        ArgumentNullException.ThrowIfNull(rollback);
        string mapPath = ValidateCustomMapDirectory(mapDirectory);
        Dictionary<SceneKey, MapSceneObject> saved = Index(savedObjects);
        Dictionary<SceneKey, MapSceneObject> current = Index(currentObjects);
        if (saved.Count != current.Count || saved.Keys.Any(key => !current.ContainsKey(key)))
            throw new InvalidOperationException("SDL 場景物件集合已改變，請重新開啟地圖後再試。");
        IReadOnlyList<SdlSceneObjectRemoval> pendingRemovals = ValidateRemovals(removals, current);
        IReadOnlyList<SdlSceneObjectAddition> pendingAdditions = ValidateAdditions(additions, current);
        IReadOnlyList<SdlSettlementTranslation> pendingTranslations = ValidateTranslations(translations);
        // 模板欄位在任何寫入前讀取：跨檔新增或「複製後刪除原件」都不受後續重新編號影響。
        var templateFields = new Dictionary<SdlSceneObjectAddition, Dictionary<string, string>>(ReferenceEqualityComparer.Instance);
        foreach (IGrouping<string, SdlSceneObjectAddition> group in pendingAdditions.GroupBy(item => item.SourceFile, StringComparer.OrdinalIgnoreCase))
        {
            var templateDocument = SdlDocument.Load(ResolveSdlPath(mapPath, group.Key));
            foreach (SdlSceneObjectAddition addition in group)
            {
                SdlObjectSection template = templateDocument.Objects.FirstOrDefault(section => section.Index == addition.TemplateIndex)
                    ?? throw new InvalidDataException($"SDL 複製模板不存在：{addition.SourceFile} / {addition.TemplateIndex}。");
                var fields = new Dictionary<string, string>(template.Fields, StringComparer.OrdinalIgnoreCase)
                {
                    ["team"] = addition.Team.ToString(System.Globalization.CultureInfo.InvariantCulture),
                    ["pos"] = new SdlVector3(addition.LocalX, addition.LocalY, addition.LocalZ).ToString("0.00"),
                };
                if (addition.Angle is float angle) fields["angle"] = angle.ToString("0.00", System.Globalization.CultureInfo.InvariantCulture);
                templateFields[addition] = fields;
            }
        }

        IEnumerable<string> files = current.Values
            .Where(item => HasChanged(saved[Key(item)], item))
            .Select(item => item.SourceFile)
            .Concat(pendingRemovals.Select(item => item.SourceFile))
            .Concat(pendingAdditions.Select(item => item.EffectiveTargetFile))
            .Concat(pendingTranslations.Select(item => item.SourceFile))
            .Distinct(StringComparer.OrdinalIgnoreCase);
        foreach (string sourceFile in files)
        {
            string path = ResolveSdlPath(mapPath, sourceFile);
            var document = SdlDocument.Load(path);
            foreach (MapSceneObject item in current.Values.Where(item =>
                item.SourceFile.Equals(sourceFile, StringComparison.OrdinalIgnoreCase) && HasChanged(saved[Key(item)], item)))
            {
                document.SetObjectTeam(item.ObjectIndex, item.Team);
                document.SetObjectPosition(item.ObjectIndex, new SdlVector3(item.LocalX, item.LocalY, item.LocalZ));
                if (item.Angle is float angle && saved[Key(item)].Angle != angle) document.SetObjectAngle(item.ObjectIndex, angle);
            }
            foreach (SdlSettlementTranslation translation in pendingTranslations.Where(item =>
                item.SourceFile.Equals(sourceFile, StringComparison.OrdinalIgnoreCase)))
                document.TranslateSettlement(translation.DeltaX, translation.DeltaY, translation.DeltaZ);
            // 先新增再刪除：新區塊附加在檔尾，刪除只影響原始索引；遞減刪除保持其餘原始索引有效。
            foreach (SdlSceneObjectAddition addition in pendingAdditions.Where(item =>
                item.EffectiveTargetFile.Equals(sourceFile, StringComparison.OrdinalIgnoreCase)))
                document.AddObject(templateFields[addition]);
            // 由大到小刪除：RemoveObject 會立即重新編號，遞減順序可保持其餘原始索引有效。
            foreach (SdlSceneObjectRemoval removal in pendingRemovals
                .Where(item => item.SourceFile.Equals(sourceFile, StringComparison.OrdinalIgnoreCase))
                .OrderByDescending(item => item.ObjectIndex))
                document.RemoveObject(removal.ObjectIndex);
            document.Save(rollback);
        }
    }

    private static IReadOnlyList<SdlSceneObjectRemoval> ValidateRemovals(
        IReadOnlyList<SdlSceneObjectRemoval>? removals, Dictionary<SceneKey, MapSceneObject> current)
    {
        if (removals is null || removals.Count == 0) return Array.Empty<SdlSceneObjectRemoval>();
        var seen = new HashSet<SceneKey>();
        foreach (SdlSceneObjectRemoval removal in removals)
        {
            var key = new SceneKey(removal.SourceFile.ToUpperInvariant(), removal.ObjectIndex);
            if (!current.ContainsKey(key))
                throw new InvalidDataException($"待刪除的 SDL 物件不存在：{removal.SourceFile} / {removal.ObjectIndex}。");
            if (!seen.Add(key))
                throw new InvalidDataException($"重複的 SDL 刪除項目：{removal.SourceFile} / {removal.ObjectIndex}。");
        }
        return removals;
    }

    private static IReadOnlyList<SdlSceneObjectAddition> ValidateAdditions(
        IReadOnlyList<SdlSceneObjectAddition>? additions, Dictionary<SceneKey, MapSceneObject> current)
    {
        if (additions is null || additions.Count == 0) return Array.Empty<SdlSceneObjectAddition>();
        foreach (SdlSceneObjectAddition addition in additions)
        {
            if (!current.ContainsKey(new SceneKey(addition.SourceFile.ToUpperInvariant(), addition.TemplateIndex)))
                throw new InvalidDataException($"SDL 複製模板不存在：{addition.SourceFile} / {addition.TemplateIndex}。");
            if (addition.Team is < -1 or > 15) throw new InvalidDataException("SDL 場景物件隊伍必須介於 -1 與 15。");
            if (addition.TargetFile is not null && !IsSdlFileName(addition.TargetFile))
                throw new InvalidDataException("SDL 新增物件的目標檔名無效。");
            if (addition.Angle is float angle && !IsValidAngle(angle)) throw new InvalidDataException("SDL 場景物件角度必須介於 -360 與 360。");
            if (!float.IsFinite(addition.LocalX) || !float.IsFinite(addition.LocalY) || !float.IsFinite(addition.LocalZ))
                throw new InvalidDataException("SDL 場景物件座標必須是有限數值。");
        }
        return additions;
    }

    private static IReadOnlyList<SdlSettlementTranslation> ValidateTranslations(IReadOnlyList<SdlSettlementTranslation>? translations)
    {
        if (translations is null || translations.Count == 0) return Array.Empty<SdlSettlementTranslation>();
        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (SdlSettlementTranslation translation in translations)
        {
            if (!IsSdlFileName(translation.SourceFile)) throw new InvalidDataException("SDL 聚落平移的來源檔名無效。");
            if (!seen.Add(translation.SourceFile)) throw new InvalidDataException($"重複的聚落平移項目：{translation.SourceFile}。");
            if (!float.IsFinite(translation.DeltaX) || !float.IsFinite(translation.DeltaY) || !float.IsFinite(translation.DeltaZ))
                throw new InvalidDataException("聚落平移量必須是有限數值。");
        }
        return translations;
    }

    private static bool IsSdlFileName(string file)
        => !string.IsNullOrWhiteSpace(file) && file == Path.GetFileName(file) && file.EndsWith(".sdl", StringComparison.OrdinalIgnoreCase);

    private static bool IsValidAngle(float angle) => float.IsFinite(angle) && angle is >= -360 and <= 360;

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
        => before.Team != after.Team || before.LocalX != after.LocalX || before.LocalY != after.LocalY || before.LocalZ != after.LocalZ || before.Angle != after.Angle;

    private static SceneKey Key(MapSceneObject item) => new(item.SourceFile.ToUpperInvariant(), item.ObjectIndex);

    private static void ValidateSceneObject(MapSceneObject item)
    {
        if (!IsSdlFileName(item.SourceFile))
            throw new InvalidDataException("SDL 場景來源檔名無效。");
        if (item.Team is < -1 or > 15) throw new InvalidDataException("SDL 場景物件隊伍必須介於 -1 與 15。");
        if (!float.IsFinite(item.LocalX) || !float.IsFinite(item.LocalY) || !float.IsFinite(item.LocalZ))
            throw new InvalidDataException("SDL 場景物件座標必須是有限數值。");
        if (item.Angle is float angle && !IsValidAngle(angle)) throw new InvalidDataException("SDL 場景物件角度必須介於 -360 與 360。");
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
