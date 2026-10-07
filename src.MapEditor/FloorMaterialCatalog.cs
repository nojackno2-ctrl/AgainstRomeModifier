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
internal sealed class FloorMaterialCatalog : INativeTerrainMaterialResolver
{
    private static readonly Regex BaseMaterialName = new("^4B(?<code>[0-9A-Z])___5[0-9A-Z]$", RegexOptions.Compiled | RegexOptions.CultureInvariant | RegexOptions.IgnoreCase);
    private static readonly Regex TwoMaterialTransitionName = new("^4U(?<first>[0-9A-Z])(?<second>[0-9A-Z])__(?<shape>[1-46-9])(?<variant>[0-9A-Z])$", RegexOptions.Compiled | RegexOptions.CultureInvariant | RegexOptions.IgnoreCase);
    // L 系列（原版地區地表，如 L2B02T5A）：L<組>B<兩位編號>T<九宮格形狀><變體>；只有 T5 的編號是純材質，T1–T9（缺 T5）的編號是兩材質過渡，配對須由貼圖推斷。
    private static readonly Regex RegionalTileName = new("^L(?<set>[0-9]+)B(?<index>[0-9]{2})T(?<shape>[1-9])(?<variant>[A-Z])$", RegexOptions.Compiled | RegexOptions.CultureInvariant | RegexOptions.IgnoreCase);
    /// <summary>L 系列過渡配對可接受的平均角落色差（RGB 歐氏距離）；超過代表貼圖不是兩個已知材質的過渡，不採用以免錯配。</summary>
    internal const double RegionalTransitionMaxCornerDistance = 32;
    // 4U 的第二種命名：尾碼為兩位數 01–14 的角點遮罩（兩種材質在四角的 14 種組合），例如 4UJX__05、4UMX__14。
    // 同一族只要出現 0 開頭的尾碼就屬於此命名，整族不能用九宮格形狀解析（否則 10–14 會被誤判為形狀 1）。
    private static readonly Regex MaskTransitionName = new("^4U(?<first>[0-9A-Z])(?<second>[0-9A-Z])__(?<mask>0[1-9]|1[0-4])$", RegexOptions.Compiled | RegexOptions.CultureInvariant | RegexOptions.IgnoreCase);
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
        var regionalTiles = names
            .Select(name => (name, match: RegionalTileName.Match(name)))
            .Where(item => item.match.Success)
            .Select(item => (Name: item.name, Set: item.match.Groups["set"].Value, Index: item.match.Groups["index"].Value, Shape: int.Parse(item.match.Groups["shape"].Value)))
            .ToArray();
        var regionalBases = regionalTiles
            .GroupBy(tile => (tile.Set, tile.Index))
            .Where(group => group.All(tile => tile.Shape == 5))
            .OrderBy(group => group.Key.Set, StringComparer.Ordinal).ThenBy(group => group.Key.Index, StringComparer.Ordinal)
            .Select(group =>
            {
                string[] variants = group.Select(tile => tile.Name).OrderBy(name => name, StringComparer.OrdinalIgnoreCase).ToArray();
                return new FloorMaterial(RegionalMaterialId(group.Key.Set, group.Key.Index), "地區地表 L" + group.Key.Set, $"L{group.Key.Set} 地表 {group.Key.Index}", variants[0], variants);
            });
        Materials = Materials.Concat(regionalBases).ToArray();
        _byId = Materials.ToDictionary(material => material.Id, StringComparer.OrdinalIgnoreCase);
        var maskFamilies = names
            .Select(name => MaskTransitionName.Match(name))
            .Where(match => match.Success && match.Groups["mask"].Value.StartsWith('0'))
            .Select(match => "B" + match.Groups["first"].Value.ToUpperInvariant() + "|B" + match.Groups["second"].Value.ToUpperInvariant())
            .ToHashSet(StringComparer.OrdinalIgnoreCase);
        _transitions = names
            .Select(name => (name, match: TwoMaterialTransitionName.Match(name)))
            .Where(item => item.match.Success)
            .Where(item => !maskFamilies.Contains("B" + item.match.Groups["first"].Value.ToUpperInvariant() + "|B" + item.match.Groups["second"].Value.ToUpperInvariant()))
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
        if (textureResolver is not null)
        {
            RegisterMaskTransitions(names, maskFamilies, textureResolver);
            RegisterRegionalTransitions(regionalTiles, textureResolver);
        }
        _nativeTexturesByCorners = _nativeCornersByTexture
            .GroupBy(item => CornerKey(item.Value), item => item.Key, StringComparer.OrdinalIgnoreCase)
            .ToDictionary(group => group.Key, group => (IReadOnlyList<string>)group.OrderBy(value => value, StringComparer.OrdinalIgnoreCase).ToArray(), StringComparer.OrdinalIgnoreCase);
    }

    public IReadOnlyList<FloorMaterial> Materials { get; }
    public IReadOnlyList<string> MaterialIds => _materialIds ??= Materials.Select(material => material.Id).ToArray();
    private string[]? _materialIds;
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
    public string? ResolveNativeTile(IReadOnlyList<string> cornerMaterialIds, int x, int y)
    {
        if (cornerMaterialIds.Count != 4) throw new ArgumentException("原生地表 tile 必須提供四個角的材質。", nameof(cornerMaterialIds));
        if (cornerMaterialIds.All(value => StringComparer.OrdinalIgnoreCase.Equals(value, cornerMaterialIds[0])))
            return _byId.TryGetValue(cornerMaterialIds[0], out FloorMaterial? material) ? material.PickVariant(x, y) : null;

        string key = CornerKey(cornerMaterialIds);
        return _nativeTexturesByCorners.TryGetValue(key, out IReadOnlyList<string>? matching)
            ? PickStable(key, matching, x, y)
            : null;
    }

    public bool TryResolveNativeCorners(string texture, out IReadOnlyList<string> corners)
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
            return (id, color: bitmap is null ? ((double R, double G, double B)?)null : MeanColor(BitmapPixels.Read(bitmap), bitmap.Width, bitmap.Height, 0, 0, bitmap.Width, bitmap.Height));
        }).Where(item => item.color is not null).Select(item => (item.id, color: item.color!.Value)).ToArray();
        if (references.Length != materialIds.Count) return null;
        int cornerWidth = Math.Max(1, transition.Width / 4), cornerHeight = Math.Max(1, transition.Height / 4);
        int[] transitionPixels = BitmapPixels.Read(transition);
        (int X, int Y, int Width, int Height)[] cornerSamples =
        [
            (0, 0, cornerWidth, cornerHeight),
            (transition.Width - cornerWidth, 0, cornerWidth, cornerHeight),
            (transition.Width - cornerWidth, transition.Height - cornerHeight, cornerWidth, cornerHeight),
            (0, transition.Height - cornerHeight, cornerWidth, cornerHeight)
        ];
        return cornerSamples.Select(sampleArea =>
        {
            var sample = MeanColor(transitionPixels, transition.Width, transition.Height, sampleArea.X, sampleArea.Y, sampleArea.Width, sampleArea.Height);
            return references.MinBy(reference => ColorDistanceSquared(sample, reference.color)).id;
        }).ToArray();
    }

    /// <summary>角點遮罩命名的 4U 族已登記的 tile 數（診斷與測試用）。</summary>
    internal int MaskTransitionTileCount { get; private set; }

    /// <summary>
    /// 角點遮罩命名的 4U 族：兩種材質已由名稱給定，每張 tile 的四角以顏色推斷（與形狀式相同的取樣），
    /// 四角必須同時含兩種材質才登記；遮罩位元的方向未經證實，因此不從編號推導角點。
    /// </summary>
    private void RegisterMaskTransitions(IEnumerable<string> names, HashSet<string> maskFamilies, Func<string, Bitmap?> textureResolver)
    {
        foreach (string name in names)
        {
            Match match = MaskTransitionName.Match(name);
            if (!match.Success) continue;
            string first = "B" + match.Groups["first"].Value.ToUpperInvariant(), second = "B" + match.Groups["second"].Value.ToUpperInvariant();
            if (!maskFamilies.Contains(first + "|" + second) || !_byId.ContainsKey(first) || !_byId.ContainsKey(second)) continue;
            string[]? corners = InferCorners(textureResolver, name, [first, second]);
            if (corners is null || corners.Distinct(StringComparer.OrdinalIgnoreCase).Count() != 2) continue;
            _nativeCornersByTexture[name] = corners;
            _byTexture[name] = _byId[first];
            MaskTransitionTileCount++;
        }
    }

    internal static string RegionalMaterialId(string set, string index) => $"L{set}:{index}";

    /// <summary>L 系列過渡編號的推斷結果（供診斷與測試）：鍵為 L{組}B{編號}，值為推得的兩種材質與平均角落色差；未採用者材質為 null。</summary>
    internal IReadOnlyDictionary<string, (string? First, string? Second, double Distance)> RegionalTransitionFits => _regionalTransitionFits;
    private readonly Dictionary<string, (string? First, string? Second, double Distance)> _regionalTransitionFits = new(StringComparer.OrdinalIgnoreCase);

    /// <summary>
    /// 推斷 L 系列過渡編號連接的兩種材質：先取同組純材質（同組不足兩種時改用所有 L 組），對每一組候選配對，
    /// 讓每張過渡圖塊的四個角各自取兩者中較近的顏色，總色差最小者勝出；每張圖塊的四角必須同時包含兩種材質，
    /// 且平均角落色差不超過門檻，才登記為可烘焙的原生 tile。
    /// </summary>
    private void RegisterRegionalTransitions((string Name, string Set, string Index, int Shape)[] tiles, Func<string, Bitmap?> textureResolver)
    {
        var baseColors = Materials.Where(material => material.Id.StartsWith('L'))
            .Select(material => (material.Id, Set: material.Id[1..material.Id.IndexOf(':')], Color: MeanTextureColor(textureResolver, material.RepresentativeTexture)))
            .Where(item => item.Color is not null).Select(item => (item.Id, item.Set, Color: item.Color!.Value)).ToArray();
        foreach (var group in tiles.Where(tile => tile.Shape != 5).GroupBy(tile => (tile.Set, tile.Index)))
        {
            if (_byId.ContainsKey(RegionalMaterialId(group.Key.Set, group.Key.Index))) continue;
            var corners = group.Select(tile => (tile.Name, Corners: CornerColors(textureResolver, tile.Name))).Where(item => item.Corners is not null)
                .Select(item => (item.Name, Corners: item.Corners!)).ToArray();
            if (corners.Length == 0) continue;
            var candidates = baseColors.Where(item => item.Set == group.Key.Set).ToArray();
            if (candidates.Length < 2) candidates = baseColors;
            (string First, string Second, double Total) best = ("", "", double.MaxValue);
            for (int a = 0; a < candidates.Length; a++)
            for (int b = a + 1; b < candidates.Length; b++)
            {
                double total = 0;
                foreach (var tile in corners)
                foreach (var corner in tile.Corners)
                    total += Math.Sqrt(Math.Min(ColorDistanceSquared(corner, candidates[a].Color), ColorDistanceSquared(corner, candidates[b].Color)));
                if (total < best.Total) best = (candidates[a].Id, candidates[b].Id, total);
            }
            string key = $"L{group.Key.Set}B{group.Key.Index}";
            if (best.Total == double.MaxValue) { _regionalTransitionFits[key] = (null, null, double.NaN); continue; }
            double average = best.Total / (corners.Length * 4);
            if (average > RegionalTransitionMaxCornerDistance) { _regionalTransitionFits[key] = (null, null, average); continue; }
            var first = baseColors.First(item => item.Id == best.First);
            var second = baseColors.First(item => item.Id == best.Second);
            var accepted = new List<(string Name, string[] Corners)>();
            foreach (var tile in corners)
            {
                string[] assigned = tile.Corners.Select(corner => ColorDistanceSquared(corner, first.Color) <= ColorDistanceSquared(corner, second.Color) ? first.Id : second.Id).ToArray();
                if (assigned.Distinct(StringComparer.OrdinalIgnoreCase).Count() == 2) accepted.Add((tile.Name, assigned));
            }
            if (accepted.Count == 0) { _regionalTransitionFits[key] = (null, null, average); continue; }
            _regionalTransitionFits[key] = (first.Id, second.Id, average);
            foreach (var (name, assigned) in accepted)
            {
                _nativeCornersByTexture[name] = assigned;
                _byTexture[name] = _byId[assigned[0]];
            }
        }
    }

    private static (double R, double G, double B)? MeanTextureColor(Func<string, Bitmap?> textureResolver, string texture)
    {
        Bitmap? bitmap = textureResolver(texture);
        return bitmap is null ? null : MeanColor(BitmapPixels.Read(bitmap), bitmap.Width, bitmap.Height, 0, 0, bitmap.Width, bitmap.Height);
    }

    // 與 InferCorners 相同的四角取樣：TL、TR、BR、BL 各 1/4×1/4 區塊。
    private static (double R, double G, double B)[]? CornerColors(Func<string, Bitmap?> textureResolver, string texture)
    {
        Bitmap? bitmap = textureResolver(texture);
        if (bitmap is null) return null;
        int width = Math.Max(1, bitmap.Width / 4), height = Math.Max(1, bitmap.Height / 4);
        int[] pixels = BitmapPixels.Read(bitmap);
        return
        [
            MeanColor(pixels, bitmap.Width, bitmap.Height, 0, 0, width, height),
            MeanColor(pixels, bitmap.Width, bitmap.Height, bitmap.Width - width, 0, width, height),
            MeanColor(pixels, bitmap.Width, bitmap.Height, bitmap.Width - width, bitmap.Height - height, width, height),
            MeanColor(pixels, bitmap.Width, bitmap.Height, 0, bitmap.Height - height, width, height)
        ];
    }

    // 以單次 LockBits 讀到的 ARGB 陣列取樣，取代逐像素 GetPixel（開啟編輯器時對每個 transition tile 都會呼叫）。
    private static (double R, double G, double B) MeanColor(int[] pixels, int imageWidth, int imageHeight, int x, int y, int width, int height)
    {
        long red = 0, green = 0, blue = 0, count = 0;
        int stepX = Math.Max(1, width / 8), stepY = Math.Max(1, height / 8);
        for (int sampleY = y + stepY / 2; sampleY < y + height; sampleY += stepY)
        for (int sampleX = x + stepX / 2; sampleX < x + width; sampleX += stepX)
        {
            int pixel = pixels[Math.Min(imageHeight - 1, sampleY) * imageWidth + Math.Min(imageWidth - 1, sampleX)];
            red += (pixel >> 16) & 0xff; green += (pixel >> 8) & 0xff; blue += pixel & 0xff; count++;
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
