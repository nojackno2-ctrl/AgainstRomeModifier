using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.Modules.Placement;

/// <summary>
/// 把規劃器使用的短型別名（例如 BauGerHau00）解析成目前遊戲目錄實際存在的別名／範本鍵
/// （例如 GER_HAU00／LanGerNad00_Tanne_gross）。找不到者回傳 null，由呼叫端略過並回報，而非整批失敗。
/// </summary>
internal static class LayoutTypeResolver
{
    public static string? ResolveAlias(IReadOnlyList<SdlObjectType> catalog, string name)
    {
        ArgumentNullException.ThrowIfNull(catalog);
        SdlObjectType? hit = catalog.FirstOrDefault(type => MapLayoutPresets.Alias(type).Equals(name, StringComparison.OrdinalIgnoreCase))
            ?? catalog.FirstOrDefault(type => type.NameDef.Equals(name, StringComparison.OrdinalIgnoreCase))
            ?? catalog.Where(type => type.NameDef.StartsWith(name + "_", StringComparison.OrdinalIgnoreCase))
                .OrderBy(type => type.NameDef, StringComparer.OrdinalIgnoreCase).FirstOrDefault();
        return hit is null ? null : MapLayoutPresets.Alias(hit);
    }

    public static string? ResolveKey<T>(IReadOnlyDictionary<string, T> templates, string name)
    {
        ArgumentNullException.ThrowIfNull(templates);
        if (templates.ContainsKey(name)) return name;
        return templates.Keys.Where(key => key.StartsWith(name + "_", StringComparison.OrdinalIgnoreCase))
            .OrderBy(key => key, StringComparer.OrdinalIgnoreCase).FirstOrDefault();
    }
}
