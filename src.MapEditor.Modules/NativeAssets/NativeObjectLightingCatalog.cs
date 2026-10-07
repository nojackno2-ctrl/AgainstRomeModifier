using System.Globalization;
using AgainstRomeModifier;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>
/// 從 objdef 讀取與光照相關之欄位定義：
/// lidef (col 66): 物件光照定義索引 (-1 為無光照)
/// lihei (col 67): 物件光照高度偏移
/// aptli (col 162): APT 建築光源定義索引 (-1 為無光源)
/// aptlh (col 163): APT 建築光源高度偏移
/// aptix (col 14): APT 建築模型索引
/// </summary>
public sealed record NativeObjectLightingDefinition(
    int TypeId,
    string Name,
    int AptIndex,
    int LightDefIndex,
    int LightHeightOffset,
    int AptLightDefIndex,
    int AptLightHeightOffset
);

/// <summary>
/// 輕量化 objdef 光照定義解析器（獨立於 NativeSpriteCatalog，專門讀取光照欄位）。
/// 包含 fail-soft 容錯機制，支援純文字與 PFIL 壓縮之 objdef.dau。
/// </summary>
public sealed class NativeObjectLightingCatalog
{
    private readonly Dictionary<string, NativeObjectLightingDefinition> _definitionsByName;
    private readonly Dictionary<int, NativeObjectLightingDefinition> _definitionsById;

    public NativeObjectLightingCatalog(IEnumerable<NativeObjectLightingDefinition> definitions)
    {
        _definitionsByName = new Dictionary<string, NativeObjectLightingDefinition>(StringComparer.OrdinalIgnoreCase);
        _definitionsById = new Dictionary<int, NativeObjectLightingDefinition>();
        foreach (var def in definitions)
        {
            _definitionsByName.TryAdd(def.Name, def);
            _definitionsById.TryAdd(def.TypeId, def);
        }
    }

    public int Count => _definitionsByName.Count;

    public bool TryGetDefinition(string name, out NativeObjectLightingDefinition definition)
        => _definitionsByName.TryGetValue(name.Trim(), out definition!);

    public bool TryGetDefinition(int typeId, out NativeObjectLightingDefinition definition)
        => _definitionsById.TryGetValue(typeId, out definition!);

    public static NativeObjectLightingCatalog? Open(string gamePath)
    {
        string objdefPath = Path.Combine(gamePath, "SYSTEM", "DATA_MP", "DEFAULTS", "objdef.dau");
        if (!File.Exists(objdefPath)) return null;
        try
        {
            byte[] bytes = File.ReadAllBytes(objdefPath);
            if (bytes.Length >= 4 && bytes.AsSpan(0, 4).SequenceEqual("PFIL"u8))
            {
                bytes = GameLZSS.DecompressPfil(bytes);
            }
            string text = MapTextEncoding.Game.GetString(bytes);
            return Parse(text);
        }
        catch
        {
            return null;
        }
    }

    public static NativeObjectLightingCatalog Parse(string text)
    {
        ArgumentNullException.ThrowIfNull(text);
        var list = new List<NativeObjectLightingDefinition>();

        using var reader = new StringReader(text);
        string? line;
        while ((line = reader.ReadLine()) != null)
        {
            line = line.Trim();
            if (line.Length == 0 || line.StartsWith(';') || line.StartsWith('#') || line.StartsWith('[')) continue;

            string[] columns = line.Split(',');
            if (columns.Length <= 67 || !int.TryParse(columns[0].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int typeId))
                continue;

            int aptIndex = (columns.Length > 14 && int.TryParse(columns[14].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int aIdx)) ? aIdx : -1;
            string name = columns.Length > 52 ? columns[52].Trim() : string.Empty;
            if (string.IsNullOrEmpty(name)) continue;

            int lidef = int.TryParse(columns[66].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int ld) ? ld : -1;
            int lihei = int.TryParse(columns[67].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int lh) ? lh : 0;

            int aptli = (columns.Length > 162 && int.TryParse(columns[162].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int ali)) ? ali : -1;
            int aptlh = (columns.Length > 163 && int.TryParse(columns[163].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int alh)) ? alh : 0;

            list.Add(new NativeObjectLightingDefinition(typeId, name, aptIndex, lidef, lihei, aptli, aptlh));
        }

        return new NativeObjectLightingCatalog(list);
    }
}
