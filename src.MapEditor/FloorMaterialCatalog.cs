using System.Drawing;
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
    private static readonly Regex TwoMaterialTransitionName = new("^4U(?<first>[0-9A-Z])(?<second>[0-9A-Z])__(?<shape>[1-46-9])(?<variant>[0-9A-Z])$", RegexOptions.Compiled | RegexOptions.CultureInvariant | RegexOptions.IgnoreCase);
    private static readonly Regex ThreeMaterialTransitionName = new("^4T(?<first>[0-9A-Z])(?<second>[0-9A-Z])(?<third>[0-9A-Z])_(?<shape>[2468])(?<variant>[0-9A-Z])$", RegexOptions.Compiled | RegexOptions.CultureInvariant | RegexOptions.IgnoreCase);
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
    private readonly Dictionary<string, FloorMaterial> _byId;
    private readonly IReadOnlyList<FloorTransition> _transitions;
    private readonly IReadOnlyList<ThreeMaterialFloorTransition> _threeMaterialTransitions;
    private readonly Dictionary<string, string[]> _nativeCornersByTexture = new(StringComparer.OrdinalIgnoreCase);
    private readonly Dictionary<string, IReadOnlyList<string>> _nativeTexturesByCorners = new(StringComparer.OrdinalIgnoreCase);

    public FloorMaterialCatalog(IEnumerable<string> textureNames) : this(textureNames, null)
    {
    }

    public FloorMaterialCatalog(FloorTextureLibrary library) : this(library.Names, library.Get)
    {
    }

    private FloorMaterialCatalog(IEnumerable<string> textureNames, Func<string, Bitmap?>? textureResolver)
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
        _byId = Materials.ToDictionary(material => material.Id, StringComparer.OrdinalIgnoreCase);
        _transitions = names
            .Select(name => (name, match: TwoMaterialTransitionName.Match(name)))
            .Where(item => item.match.Success)
            .GroupBy(item => (First: "B" + item.match.Groups["first"].Value.ToUpperInvariant(), Second: "B" + item.match.Groups["second"].Value.ToUpperInvariant()))
            .Where(group => _byId.ContainsKey(group.Key.First) && _byId.ContainsKey(group.Key.Second))
            .Select(group => new FloorTransition(
                group.Key.First,
                group.Key.Second,
                group.GroupBy(item => int.Parse(item.match.Groups["shape"].Value))
                    .ToDictionary(shape => shape.Key, shape => (IReadOnlyList<string>)shape.Select(item => item.name).Distinct(StringComparer.OrdinalIgnoreCase).OrderBy(name => name, StringComparer.OrdinalIgnoreCase).ToArray())))
            .ToArray();
        _threeMaterialTransitions = names
            .Select(name => (name, match: ThreeMaterialTransitionName.Match(name)))
            .Where(item => item.match.Success)
            .GroupBy(item => (
                First: "B" + item.match.Groups["first"].Value.ToUpperInvariant(),
                Second: "B" + item.match.Groups["second"].Value.ToUpperInvariant(),
                Third: "B" + item.match.Groups["third"].Value.ToUpperInvariant()))
            .Where(group => _byId.ContainsKey(group.Key.First) && _byId.ContainsKey(group.Key.Second) && _byId.ContainsKey(group.Key.Third))
            .Select(group => new ThreeMaterialFloorTransition(
                group.Key.First,
                group.Key.Second,
                group.Key.Third,
                group.GroupBy(item => int.Parse(item.match.Groups["shape"].Value))
                    .ToDictionary(shape => shape.Key, shape => (IReadOnlyList<string>)shape.Select(item => item.name).Distinct(StringComparer.OrdinalIgnoreCase).OrderBy(name => name, StringComparer.OrdinalIgnoreCase).ToArray())))
            .ToArray();

        _byTexture = Materials.SelectMany(material => material.Variants.Select(texture => (texture, material)))
            .ToDictionary(item => item.texture, item => item.material, StringComparer.OrdinalIgnoreCase);
        foreach (FloorTransition transition in _transitions)
        {
            FloorMaterial owner = _byId[transition.FirstMaterialId];
            foreach ((int shape, IReadOnlyList<string> variants) in transition.VariantsByShape)
            {
                foreach (string texture in variants)
                {
                    _byTexture[texture] = owner;
                    string[]? corners = textureResolver is null
                        ? TwoMaterialCorners(transition.FirstMaterialId, transition.SecondMaterialId, shape)
                        : InferCorners(textureResolver, texture, [transition.FirstMaterialId, transition.SecondMaterialId]);
                    if (corners is not null) _nativeCornersByTexture[texture] = corners;
                }
            }
        }
        foreach (ThreeMaterialFloorTransition transition in _threeMaterialTransitions)
        {
            FloorMaterial owner = _byId[transition.FirstMaterialId];
            foreach ((int shape, IReadOnlyList<string> variants) in transition.VariantsByShape)
            {
                foreach (string texture in variants)
                {
                    _byTexture[texture] = owner;
                    string[]? corners = textureResolver is null
                        ? ThreeMaterialCorners(transition.FirstMaterialId, transition.SecondMaterialId, transition.ThirdMaterialId, shape)
                        : InferCorners(textureResolver, texture, [transition.FirstMaterialId, transition.SecondMaterialId, transition.ThirdMaterialId]);
                    if (corners is not null) _nativeCornersByTexture[texture] = corners;
                }
            }
        }
        _nativeTexturesByCorners = _nativeCornersByTexture
            .GroupBy(item => CornerKey(item.Value), item => item.Key, StringComparer.OrdinalIgnoreCase)
            .ToDictionary(group => group.Key, group => (IReadOnlyList<string>)group.OrderBy(value => value, StringComparer.OrdinalIgnoreCase).ToArray(), StringComparer.OrdinalIgnoreCase);
    }

    public IReadOnlyList<FloorMaterial> Materials { get; }
    internal int TwoMaterialTransitionFamilyCount => _transitions.Count;
    internal int ThreeMaterialTransitionFamilyCount => _threeMaterialTransitions.Count;
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
            string? resolved = ResolveTwoMaterialTransition(transition.FirstMaterialId, transition.SecondMaterialId, shape, x, y);
            if (resolved is not null) return resolved;
        }
        return current.PickVariant(x, y);
    }

    internal string? ResolveTwoMaterialTransition(string firstMaterialId, string secondMaterialId, int shape, int x, int y)
    {
        FloorTransition? transition = _transitions.FirstOrDefault(item =>
            StringComparer.OrdinalIgnoreCase.Equals(item.FirstMaterialId, firstMaterialId) &&
            StringComparer.OrdinalIgnoreCase.Equals(item.SecondMaterialId, secondMaterialId));
        return transition is not null && transition.VariantsByShape.TryGetValue(shape, out IReadOnlyList<string>? variants)
            ? PickStable(firstMaterialId + secondMaterialId + shape, variants, x, y)
            : null;
    }

    internal string? ResolveThreeMaterialTransition(string firstMaterialId, string secondMaterialId, string thirdMaterialId, int shape, int x, int y)
    {
        ThreeMaterialFloorTransition? transition = _threeMaterialTransitions.FirstOrDefault(item =>
            StringComparer.OrdinalIgnoreCase.Equals(item.FirstMaterialId, firstMaterialId) &&
            StringComparer.OrdinalIgnoreCase.Equals(item.SecondMaterialId, secondMaterialId) &&
            StringComparer.OrdinalIgnoreCase.Equals(item.ThirdMaterialId, thirdMaterialId));
        return transition is not null && transition.VariantsByShape.TryGetValue(shape, out IReadOnlyList<string>? variants)
            ? PickStable(firstMaterialId + secondMaterialId + thirdMaterialId + shape, variants, x, y)
            : null;
    }

    /// <summary>
    /// Compiles four texture-space corner materials (top-left, top-right, bottom-right, bottom-left)
    /// into an existing native floor tile. Returning null is intentional: unsupported junctions must
    /// be surfaced to the authoring layer instead of silently degrading to a hard square.
    /// </summary>
    internal string? ResolveNativeTile(IReadOnlyList<string> cornerMaterialIds, int x, int y)
    {
        if (cornerMaterialIds.Count != 4) throw new ArgumentException("原生地表 tile 必須提供四個角的材質。", nameof(cornerMaterialIds));
        if (cornerMaterialIds.All(value => StringComparer.OrdinalIgnoreCase.Equals(value, cornerMaterialIds[0])))
            return _byId.TryGetValue(cornerMaterialIds[0], out FloorMaterial? material) ? material.PickVariant(x, y) : null;

        string key = CornerKey(cornerMaterialIds);
        return _nativeTexturesByCorners.TryGetValue(key, out IReadOnlyList<string>? matching)
            ? PickStable(key, matching, x, y)
            : null;
    }

    internal bool TryResolveNativeCorners(string texture, out IReadOnlyList<string> corners)
    {
        if (_nativeCornersByTexture.TryGetValue(texture, out string[]? resolved))
        {
            corners = resolved;
            return true;
        }
        FloorMaterial? material = FindByTexture(texture);
        if (material is not null)
        {
            corners = [material.Id, material.Id, material.Id, material.Id];
            return true;
        }
        corners = Array.Empty<string>();
        return false;
    }

    private static (string Category, string Name, int Order) PlayerDefinition(string id)
        => PlayerNames.TryGetValue(id, out var definition) ? definition : ("其他地表", "其他地表", 1000);

    private static bool HasMaterial(IReadOnlyList<string?> materials, int dimension, int x, int y, string expected)
        => x >= 0 && y >= 0 && x < dimension && y < dimension && StringComparer.OrdinalIgnoreCase.Equals(materials[y * dimension + x], expected);

    private static bool CornersEqual(IReadOnlyList<string> actual, IReadOnlyList<string> expected)
        => Enumerable.Range(0, 4).All(index => StringComparer.OrdinalIgnoreCase.Equals(actual[index], expected[index]));

    private static string CornerKey(IReadOnlyList<string> corners)
        => string.Join("|", corners.Select(value => value.ToUpperInvariant()));

    private string[]? InferCorners(Func<string, Bitmap?> textureResolver, string texture, IReadOnlyList<string> materialIds)
    {
        Bitmap? transition = textureResolver(texture);
        if (transition is null) return null;
        var references = materialIds.Select(id =>
        {
            FloorMaterial material = _byId[id];
            Bitmap? bitmap = textureResolver(material.RepresentativeTexture);
            return (id, color: bitmap is null ? ((double R, double G, double B)?)null : MeanColor(bitmap, 0, 0, bitmap.Width, bitmap.Height));
        }).Where(item => item.color is not null).Select(item => (item.id, color: item.color!.Value)).ToArray();
        if (references.Length != materialIds.Count) return null;
        int halfWidth = transition.Width / 2, halfHeight = transition.Height / 2;
        (int X, int Y, int Width, int Height)[] quadrants =
        [
            (0, 0, halfWidth, halfHeight),
            (halfWidth, 0, transition.Width - halfWidth, halfHeight),
            (halfWidth, halfHeight, transition.Width - halfWidth, transition.Height - halfHeight),
            (0, halfHeight, halfWidth, transition.Height - halfHeight)
        ];
        return quadrants.Select(quadrant =>
        {
            var sample = MeanColor(transition, quadrant.X, quadrant.Y, quadrant.Width, quadrant.Height);
            return references.MinBy(reference => ColorDistanceSquared(sample, reference.color)).id;
        }).ToArray();
    }

    private static (double R, double G, double B) MeanColor(Bitmap bitmap, int x, int y, int width, int height)
    {
        long red = 0, green = 0, blue = 0, count = 0;
        int stepX = Math.Max(1, width / 8), stepY = Math.Max(1, height / 8);
        for (int sampleY = y + stepY / 2; sampleY < y + height; sampleY += stepY)
        for (int sampleX = x + stepX / 2; sampleX < x + width; sampleX += stepX)
        {
            Color color = bitmap.GetPixel(Math.Min(bitmap.Width - 1, sampleX), Math.Min(bitmap.Height - 1, sampleY));
            red += color.R; green += color.G; blue += color.B; count++;
        }
        return (red / (double)count, green / (double)count, blue / (double)count);
    }

    private static double ColorDistanceSquared((double R, double G, double B) left, (double R, double G, double B) right)
        => Math.Pow(left.R - right.R, 2) + Math.Pow(left.G - right.G, 2) + Math.Pow(left.B - right.B, 2);

    // Corner order is texture-space TL, TR, BR, BL, verified against the original floortex BMP quadrants.
    private static string[]? TwoMaterialCorners(string first, string second, int shape) => shape switch
    {
        1 => [second, first, second, second],
        2 => [first, first, second, second],
        3 => [first, second, second, second],
        4 => [second, first, first, second],
        6 => [first, second, second, first],
        7 => [second, second, first, second],
        8 => [second, second, first, first],
        9 => [second, second, second, first],
        _ => null
    };

    private static string[]? ThreeMaterialCorners(string first, string second, string third, int shape) => shape switch
    {
        2 => [third, second, first, first],
        4 => [first, third, second, first],
        6 => [third, first, first, second],
        8 => [first, first, second, third],
        _ => null
    };

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
    private sealed record ThreeMaterialFloorTransition(string FirstMaterialId, string SecondMaterialId, string ThirdMaterialId, IReadOnlyDictionary<int, IReadOnlyList<string>> VariantsByShape);
}
