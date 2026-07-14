using System.Text.RegularExpressions;

namespace AgainstRomeMapEditor;

internal sealed record FloorMaterial(string Id, string Category, string DisplayName, string RepresentativeTexture, IReadOnlyList<string> Variants)
{
    public string PickVariant(int x, int y)
        => RepresentativeTexture;
}

/// <summary>
/// Converts low-level floortex transition tiles into the base materials used as painter-style terrain pigments.
/// The verified 4B?___5? family contains solid material variants; 4T/4U/L… entries remain advanced splice tiles.
/// </summary>
internal sealed class FloorMaterialCatalog
{
    private static readonly Regex BaseMaterialName = new("^4B(?<code>[0-9A-Z])___5[0-9A-Z]$", RegexOptions.Compiled | RegexOptions.CultureInvariant | RegexOptions.IgnoreCase);
    private static readonly Regex TwoMaterialTransitionName = new("^4U(?<first>[0-9A-Z])(?<second>[0-9A-Z])__(?<shape>[1-9])(?<variant>[0-9A-Z])$", RegexOptions.Compiled | RegexOptions.CultureInvariant | RegexOptions.IgnoreCase);
    private static readonly IReadOnlyDictionary<string, (string Category, string Name, int Order)> PlayerNames =
        new Dictionary<string, (string, string, int)>(StringComparer.OrdinalIgnoreCase)
        {
            ["BB"] = ("草地", "草地", 0), ["BA"] = ("草地", "深綠草地", 1), ["BC"] = ("草地", "鮮綠草地", 2),
            ["BD"] = ("草地", "橄欖草地", 3), ["BW"] = ("草地", "淺綠草地", 4), ["BS"] = ("草地", "野草地", 5),
            ["BM"] = ("草地", "稀疏草地", 6), ["BT"] = ("草地", "泥濘草地", 7), ["BU"] = ("草地", "乾草地", 8),
            ["BE"] = ("草地", "枯草地", 9), ["BX"] = ("草地", "紅土草地", 10),
            ["BV"] = ("荒地", "灰綠荒地", 20), ["BL"] = ("荒地", "淺褐荒地", 21),
            ["B5"] = ("沙地", "沙地", 30),
            ["BJ"] = ("土地", "黃土地", 40), ["B2"] = ("土地", "乾燥土壤", 41), ["B4"] = ("土地", "黃褐土壤", 42),
            ["B6"] = ("土地", "深色泥土", 43), ["B7"] = ("土地", "粗糙泥地", 44), ["B9"] = ("土地", "淺色泥地", 45),
            ["B1"] = ("土地", "淺灰泥地", 46), ["BI"] = ("土地", "灰褐土壤", 47),
            ["B8"] = ("岩地", "深褐砂礫", 60), ["B3"] = ("岩地", "灰色岩地", 61), ["BG"] = ("岩地", "礫石地", 62),
            ["BK"] = ("岩地", "灰色碎石地", 63), ["BR"] = ("岩地", "風化岩地", 64), ["BO"] = ("岩地", "苔蘚岩地", 65),
        };
    private readonly Dictionary<string, FloorMaterial> _byTexture;
    private readonly IReadOnlyList<FloorTransition> _transitions;

    public FloorMaterialCatalog(IEnumerable<string> textureNames)
    {
        string[] names = textureNames.Distinct(StringComparer.OrdinalIgnoreCase).ToArray();
        var groups = names
            .Select(name => (name, match: BaseMaterialName.Match(name)))
            .Where(item => item.match.Success)
            .GroupBy(item => "B" + item.match.Groups["code"].Value.ToUpperInvariant(), StringComparer.OrdinalIgnoreCase)
            .OrderBy(group => PlayerDefinition(group.Key).Order)
            .ThenBy(group => group.Key, StringComparer.OrdinalIgnoreCase)
            .ToArray();

        Materials = groups.Select(group =>
        {
            string[] variants = group.Select(item => item.name).Distinct(StringComparer.OrdinalIgnoreCase).OrderBy(name => name, StringComparer.OrdinalIgnoreCase).ToArray();
            string representative = variants.FirstOrDefault(name => name.EndsWith("50", StringComparison.OrdinalIgnoreCase)) ?? variants[0];
            (string category, string name, _) = PlayerDefinition(group.Key);
            return new FloorMaterial(group.Key, category, name, representative, variants);
        }).ToArray();
        Dictionary<string, FloorMaterial> byId = Materials.ToDictionary(material => material.Id, StringComparer.OrdinalIgnoreCase);
        _transitions = names
            .Select(name => (name, match: TwoMaterialTransitionName.Match(name)))
            .Where(item => item.match.Success)
            .GroupBy(item => (First: "B" + item.match.Groups["first"].Value.ToUpperInvariant(), Second: "B" + item.match.Groups["second"].Value.ToUpperInvariant()))
            .Where(group => byId.ContainsKey(group.Key.First) && byId.ContainsKey(group.Key.Second))
            .Select(group => new FloorTransition(
                group.Key.First,
                group.Key.Second,
                group.GroupBy(item => int.Parse(item.match.Groups["shape"].Value))
                    .ToDictionary(shape => shape.Key, shape => (IReadOnlyList<string>)shape.Select(item => item.name).Distinct(StringComparer.OrdinalIgnoreCase).OrderBy(name => name, StringComparer.OrdinalIgnoreCase).ToArray())))
            .ToArray();

        _byTexture = Materials.SelectMany(material => material.Variants.Select(texture => (texture, material)))
            .ToDictionary(item => item.texture, item => item.material, StringComparer.OrdinalIgnoreCase);
        foreach (FloorTransition transition in _transitions)
        {
            FloorMaterial owner = byId[transition.FirstMaterialId];
            foreach (string texture in transition.VariantsByShape.Values.SelectMany(value => value))
            {
                _byTexture[texture] = owner;
            }
        }
    }

    public IReadOnlyList<FloorMaterial> Materials { get; }
    public FloorMaterial? FindByTexture(string? texture)
        => texture is not null && _byTexture.TryGetValue(texture, out FloorMaterial? material) ? material : null;

    public IReadOnlyList<int> ApplyPlayerMaterial(string?[] materialIds, int dimension, int x, int y, string materialId)
    {
        if (dimension <= 0 || materialIds.Length != dimension * dimension) throw new ArgumentException("邏輯地表格與地圖尺寸不符。", nameof(materialIds));
        if (x < 0 || y < 0 || x >= dimension || y >= dimension) throw new ArgumentOutOfRangeException(nameof(x));
        int target = y * dimension + x;
        materialIds[target] = materialId;
        return new[] { target };
    }

    public string? ResolveTexture(IReadOnlyList<string?> materialIds, int dimension, int x, int y)
    {
        if (dimension <= 0 || materialIds.Count != dimension * dimension || x < 0 || y < 0 || x >= dimension || y >= dimension) return null;
        string? currentId = materialIds[y * dimension + x];
        FloorMaterial? current = Materials.FirstOrDefault(material => StringComparer.OrdinalIgnoreCase.Equals(material.Id, currentId));
        if (current is null) return null;

        foreach (FloorTransition transition in _transitions.Where(item => StringComparer.OrdinalIgnoreCase.Equals(item.FirstMaterialId, current.Id)))
        {
            bool up = HasMaterial(materialIds, dimension, x, y - 1, transition.SecondMaterialId);
            bool right = HasMaterial(materialIds, dimension, x + 1, y, transition.SecondMaterialId);
            bool down = HasMaterial(materialIds, dimension, x, y + 1, transition.SecondMaterialId);
            bool left = HasMaterial(materialIds, dimension, x - 1, y, transition.SecondMaterialId);
            int shape = SecondMaterialRegionShape(up, right, down, left);
            if (shape != 0 && transition.VariantsByShape.TryGetValue(shape, out IReadOnlyList<string>? variants))
                return PickStable(transition.FirstMaterialId + transition.SecondMaterialId + shape, variants, x, y);
        }
        return current.PickVariant(x, y);
    }

    private static (string Category, string Name, int Order) PlayerDefinition(string id)
        => PlayerNames.TryGetValue(id, out var definition) ? definition : ("其他地表", "其他地表", 1000);

    private static bool HasMaterial(IReadOnlyList<string?> materials, int dimension, int x, int y, string expected)
        => x >= 0 && y >= 0 && x < dimension && y < dimension && StringComparer.OrdinalIgnoreCase.Equals(materials[y * dimension + x], expected);

    // 4U 尾碼第一位使用數字鍵盤方向：第二種材質位於 2/4/6/8 的半邊，或 1/3/7/9 的角落。
    private static int SecondMaterialRegionShape(bool secondAbove, bool secondRight, bool secondBelow, bool secondLeft)
    {
        if (secondAbove && !secondBelow)
        {
            if (secondLeft && !secondRight) return 7;
            if (secondRight && !secondLeft) return 9;
            return 8;
        }
        if (secondBelow && !secondAbove)
        {
            if (secondLeft && !secondRight) return 1;
            if (secondRight && !secondLeft) return 3;
            return 2;
        }
        if (secondLeft && !secondRight) return 4;
        if (secondRight && !secondLeft) return 6;
        return 0;
    }

    private static string PickStable(string seed, IReadOnlyList<string> variants, int x, int y)
    {
        uint hash = 2166136261;
        foreach (char value in seed) hash = (hash ^ value) * 16777619;
        hash = (hash ^ (uint)x) * 16777619;
        hash = (hash ^ (uint)y) * 16777619;
        return variants[(int)(hash % variants.Count)];
    }

    private sealed record FloorTransition(string FirstMaterialId, string SecondMaterialId, IReadOnlyDictionary<int, IReadOnlyList<string>> VariantsByShape);
}
