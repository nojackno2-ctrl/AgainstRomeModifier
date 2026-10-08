namespace AgainstRomeMapEditor;

/// <summary>
/// 懸崖/陡坡岩壁圖塊的朝向分類。
/// 幾何定義為坡度下落方向（峭壁朝向）以及山脊外凸角、峽谷內凹角。
/// </summary>
public enum CliffFacing
{
    None = 0,

    // 四向正坡（坡度向下傾斜之方向）
    North = 1, // 北向坡：高處在南(+Y)，峭壁朝北(-Y)
    South = 2, // 南向坡：高處在北(-Y)，峭壁朝南(+Y)
    East = 3,  // 東向坡：高處在西(-X)，峭壁朝東(+X)
    West = 4,  // 西向坡：高處在東(+X)，峭壁朝西(-X)

    // 四向外凸角（凸起山脊角點，外側為低地）
    NorthEastOuter = 5, // 東北外凸角：高台在西南，向東北兩面下落
    NorthWestOuter = 6, // 西北外凸角：高台在東南，向西北兩面下落
    SouthEastOuter = 7, // 東南外凸角：高台在西北，向東南兩面下落
    SouthWestOuter = 8, // 西南外凸角：高台在東北，向西南兩面下落

    // 四向內凹角（峽谷/凹灣內角，高壁環繞）
    NorthEastInner = 9,  // 東北內凹角：高壁在北與東，凹角低地在西南
    NorthWestInner = 10, // 西北內凹角：高壁在北與西，凹角低地在東南
    SouthEastInner = 11, // 東南內凹角：高壁在南與東，凹角低地在西北
    SouthWestInner = 12  // 西南內凹角：高壁在南與西，凹角低地在東北
}

/// <summary>單一懸崖圖塊定義。</summary>
public sealed record CliffTileEntry(string Texture, CliffFacing Facing, string Family, int Weight = 1);

/// <summary>懸崖材質家族定義（如溫帶 FELS、義大利 ITA_FELS、高山 Berg 等）。</summary>
public sealed record CliffFamilyInfo(
    string FamilyId,
    string DisplayName,
    string TalusMaterialId,
    string CrestMaterialId);

/// <summary>
/// 懸崖與岩壁圖塊分類目錄。
/// 支援朝向篩選、風格家族隔離、多變體偽隨機挑選與缺件容錯回退。
/// </summary>
public sealed class CliffTileCatalog
{
    private readonly List<CliffTileEntry> _entries = [];
    private readonly Dictionary<string, CliffFamilyInfo> _families = new(StringComparer.OrdinalIgnoreCase);

    // Real-name catalogs must never substitute a different or unverified facing.
    internal bool RequiresExactFacing { get; private init; }

    public IReadOnlyList<CliffTileEntry> Entries => _entries;
    public IReadOnlyDictionary<string, CliffFamilyInfo> Families => _families;

    /// <summary>
    /// 只保留遊戲貼圖庫中實際存在的圖塊（預設目錄的名稱是推測的；2026-10-08 遊戲內實測貼了不存在的名稱會顯示 File not found）。
    /// </summary>
    public CliffTileCatalog FilteredBy(Func<string, bool> textureExists)
    {
        ArgumentNullException.ThrowIfNull(textureExists);
        var filtered = new CliffTileCatalog { RequiresExactFacing = RequiresExactFacing };
        foreach (CliffFamilyInfo family in _families.Values) filtered.RegisterFamily(family);
        foreach (CliffTileEntry entry in _entries.Where(item => textureExists(item.Texture))) filtered.Register(entry);
        return filtered;
    }

    public void Register(CliffTileEntry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);
        _entries.Add(entry);
    }

    public void RegisterFamily(CliffFamilyInfo family)
    {
        ArgumentNullException.ThrowIfNull(family);
        _families[family.FamilyId] = family;
    }

    public bool IsCliffTexture(string texture)
    {
        if (string.IsNullOrWhiteSpace(texture)) return false;
        return _entries.Any(e => StringComparer.OrdinalIgnoreCase.Equals(e.Texture, texture));
    }

    public IReadOnlyList<CliffTileEntry> GetEntries(CliffFacing facing, string? family = null)
    {
        IEnumerable<CliffTileEntry> query = _entries;
        if (!string.IsNullOrWhiteSpace(family))
        {
            query = query.Where(e => StringComparer.OrdinalIgnoreCase.Equals(e.Family, family));
        }

        if (facing != CliffFacing.None)
        {
            query = query.Where(e => e.Facing == facing);
        }

        return query.ToArray();
    }

    /// <summary>
    /// 依朝向、家族與座標種子挑選最適圖塊；具備層級回退機制避免缺件報錯。
    /// </summary>
    public string? PickTile(CliffFacing facing, int x, int y, int seed = 0, string? preferredFamily = null)
    {
        if (_entries.Count == 0 || (RequiresExactFacing && facing == CliffFacing.None)) return null;

        // 1. 嘗試完全匹配：指定家族 + 指定朝向
        IReadOnlyList<CliffTileEntry> candidates = GetEntries(facing, preferredFamily);

        // 2. 回退 A：跨家族相同朝向
        if (candidates.Count == 0 && facing != CliffFacing.None)
        {
            candidates = GetEntries(facing, null);
        }

        if (RequiresExactFacing) return candidates.Count == 0 ? null : PickVariant(candidates, x, y, seed);

        // 3. 回退 B：外凸角/內凹角回退至相鄰正向坡
        if (candidates.Count == 0 && facing != CliffFacing.None)
        {
            CliffFacing fallbackFacing = GetFallbackFacing(facing);
            if (fallbackFacing != CliffFacing.None)
            {
                candidates = GetEntries(fallbackFacing, preferredFamily);
                if (candidates.Count == 0)
                {
                    candidates = GetEntries(fallbackFacing, null);
                }
            }
        }

        // 4. 回退 C：同家族的通用岩壁圖塊
        if (candidates.Count == 0 && !string.IsNullOrWhiteSpace(preferredFamily))
        {
            candidates = GetEntries(CliffFacing.None, preferredFamily);
        }

        // 5. 回退 D：全局任一圖塊
        if (candidates.Count == 0)
        {
            candidates = _entries;
        }

        if (candidates.Count == 0) return null;

        // 6. 依 (x, y, seed) 偽隨機挑選變體
        return PickVariant(candidates, x, y, seed);
    }

    /// <summary>
    /// Native grass/rock transitions, verified from the authorized TEMP floortex pixels.
    /// Rock lies on the named downhill side; original-map elevation usage and runtime appearance
    /// are unverified. No corner, generic-rock, or guessed-name fallback is permitted.
    /// See docs/map-editor-cliff-tiles.md and the opt-in CliffTextureAnalysisTests.
    /// </summary>
    public static CliffTileCatalog BuildRealNames(IEnumerable<string>? existingTextures)
    {
        var catalog = new CliffTileCatalog { RequiresExactFacing = true };
        catalog.RegisterFamily(new CliffFamilyInfo("FELS", "Grass / rock cliffs", "BK", "BB"));
        if (existingTextures is null) return catalog;
        var verified = new Dictionary<string, CliffFacing>(StringComparer.OrdinalIgnoreCase)
        {
            ["Fels_AA_008"] = CliffFacing.North,
            ["Fels_AA_004"] = CliffFacing.East,
            ["Fels_AA_002"] = CliffFacing.South,
            ["Fels_AA_006"] = CliffFacing.West
        };
        foreach (string name in existingTextures.Distinct(StringComparer.Ordinal))
            if (verified.TryGetValue(name, out CliffFacing facing))
                catalog.Register(new CliffTileEntry(name, facing, "FELS")); // Preserve the archive's exact spelling.
        return catalog;
    }

    private static CliffFacing GetFallbackFacing(CliffFacing facing) => facing switch
    {
        CliffFacing.NorthEastOuter or CliffFacing.NorthWestOuter => CliffFacing.North,
        CliffFacing.SouthEastOuter or CliffFacing.SouthWestOuter => CliffFacing.South,
        CliffFacing.NorthEastInner or CliffFacing.SouthEastInner => CliffFacing.East,
        CliffFacing.NorthWestInner or CliffFacing.SouthWestInner => CliffFacing.West,
        _ => CliffFacing.None
    };

    private static string PickVariant(IReadOnlyList<CliffTileEntry> candidates, int x, int y, int seed)
    {
        if (candidates.Count == 1) return candidates[0].Texture;

        uint hash = unchecked((uint)(x * 374761393 + y * 668265263 + seed * 1442695041));
        hash = (hash ^ (hash >> 13)) * 1274126177u;
        hash ^= hash >> 16;

        int totalWeight = candidates.Sum(c => Math.Max(1, c.Weight));
        int roll = (int)(hash % (uint)totalWeight);

        int accumulated = 0;
        foreach (CliffTileEntry entry in candidates)
        {
            accumulated += Math.Max(1, entry.Weight);
            if (roll < accumulated) return entry.Texture;
        }

        return candidates[0].Texture;
    }

    /// <summary>
    /// 建立包含《反抗羅馬》預設家族（FELS、ITA_FELS、Berg、STEIN）的標準目錄。
    /// </summary>
    public static CliffTileCatalog CreateDefault()
    {
        var catalog = new CliffTileCatalog();

        // 1. 溫帶岩壁家族 (FELS)
        catalog.RegisterFamily(new CliffFamilyInfo("FELS", "標準岩壁 (Temperate Cliffs)", "BK", "B8"));
        RegisterStandardFamilyTiles(catalog, "FELS");

        // 2. 義大利岩壁家族 (ITA_FELS)
        catalog.RegisterFamily(new CliffFamilyInfo("ITA_FELS", "義大利岩壁 (Italian Cliffs)", "B8", "B4"));
        RegisterStandardFamilyTiles(catalog, "ITA_FELS");

        // 3. 高山岩壁家族 (Berg)
        catalog.RegisterFamily(new CliffFamilyInfo("Berg", "高山岩壁 (Alpine Cliffs)", "BK", "B3"));
        RegisterStandardFamilyTiles(catalog, "Berg");

        // 4. 石質陡坡家族 (STEIN)
        catalog.RegisterFamily(new CliffFamilyInfo("STEIN", "石質陡坡 (Stony Slopes)", "BG", "B2"));
        RegisterStandardFamilyTiles(catalog, "STEIN");

        return catalog;
    }

    private static void RegisterStandardFamilyTiles(CliffTileCatalog catalog, string family)
    {
        // 四向正坡（含雙變體）
        catalog.Register(new CliffTileEntry($"{family}_N1", CliffFacing.North, family));
        catalog.Register(new CliffTileEntry($"{family}_N2", CliffFacing.North, family));
        catalog.Register(new CliffTileEntry($"{family}_S1", CliffFacing.South, family));
        catalog.Register(new CliffTileEntry($"{family}_S2", CliffFacing.South, family));
        catalog.Register(new CliffTileEntry($"{family}_E1", CliffFacing.East, family));
        catalog.Register(new CliffTileEntry($"{family}_E2", CliffFacing.East, family));
        catalog.Register(new CliffTileEntry($"{family}_W1", CliffFacing.West, family));
        catalog.Register(new CliffTileEntry($"{family}_W2", CliffFacing.West, family));

        // 四向外凸角
        catalog.Register(new CliffTileEntry($"{family}_NE_OUT", CliffFacing.NorthEastOuter, family));
        catalog.Register(new CliffTileEntry($"{family}_NW_OUT", CliffFacing.NorthWestOuter, family));
        catalog.Register(new CliffTileEntry($"{family}_SE_OUT", CliffFacing.SouthEastOuter, family));
        catalog.Register(new CliffTileEntry($"{family}_SW_OUT", CliffFacing.SouthWestOuter, family));

        // 四向內凹角
        catalog.Register(new CliffTileEntry($"{family}_NE_IN", CliffFacing.NorthEastInner, family));
        catalog.Register(new CliffTileEntry($"{family}_NW_IN", CliffFacing.NorthWestInner, family));
        catalog.Register(new CliffTileEntry($"{family}_SE_IN", CliffFacing.SouthEastInner, family));
        catalog.Register(new CliffTileEntry($"{family}_SW_IN", CliffFacing.SouthWestInner, family));

        // 通用兜底印章圖塊
        catalog.Register(new CliffTileEntry($"{family}1", CliffFacing.None, family));
        catalog.Register(new CliffTileEntry($"{family}2", CliffFacing.None, family));
    }

    /// <summary>
    /// 依既有圖塊清單自動動態識別並構建可用圖塊目錄。
    /// 若現有圖塊中包含帶朝向命名（如 FELS_N、FELS_SO 等）則精準註冊；若僅有泛用印章（如 FELS1）則註冊為全向兜底。
    /// </summary>
    public static CliffTileCatalog BuildAvailable(IEnumerable<string>? existingTextures, string? preferredFamily = null)
    {
        var catalog = new CliffTileCatalog();
        catalog.RegisterFamily(new CliffFamilyInfo("FELS", "標準岩壁 (Temperate Cliffs)", "BK", "B8"));
        catalog.RegisterFamily(new CliffFamilyInfo("ITA_FELS", "義大利岩壁 (Italian Cliffs)", "B8", "B4"));
        catalog.RegisterFamily(new CliffFamilyInfo("Berg", "高山岩壁 (Alpine Cliffs)", "BK", "B3"));
        catalog.RegisterFamily(new CliffFamilyInfo("STEIN", "石質陡坡 (Stony Slopes)", "BG", "B2"));

        if (existingTextures is null) return CreateDefault();

        HashSet<string> existing = new(existingTextures, StringComparer.OrdinalIgnoreCase);

        foreach (string name in existing)
        {
            string upper = name.ToUpperInvariant();
            string? detectedFamily = null;
            if (upper.StartsWith("ITA_FELS", StringComparison.Ordinal)) detectedFamily = "ITA_FELS";
            else if (upper.StartsWith("FELS", StringComparison.Ordinal)) detectedFamily = "FELS";
            else if (upper.StartsWith("BERG", StringComparison.Ordinal)) detectedFamily = "Berg";
            else if (upper.StartsWith("STEIN", StringComparison.Ordinal)) detectedFamily = "STEIN";

            if (detectedFamily is null) continue;

            CliffFacing facing = InferFacingFromName(upper);
            catalog.Register(new CliffTileEntry(name, facing, detectedFamily));
        }

        // 若無任何可用圖塊被識別，返回預設目錄
        if (catalog.Entries.Count == 0)
        {
            return CreateDefault();
        }

        return catalog;
    }

    internal static CliffFacing InferFacingFromName(string upper)
    {
        // 內凹角
        if (upper.Contains("NE_IN", StringComparison.Ordinal) || upper.Contains("NO_IN", StringComparison.Ordinal)) return CliffFacing.NorthEastInner;
        if (upper.Contains("NW_IN", StringComparison.Ordinal)) return CliffFacing.NorthWestInner;
        if (upper.Contains("SE_IN", StringComparison.Ordinal) || upper.Contains("SO_IN", StringComparison.Ordinal)) return CliffFacing.SouthEastInner;
        if (upper.Contains("SW_IN", StringComparison.Ordinal)) return CliffFacing.SouthWestInner;

        // 外凸角
        if (upper.Contains("NE_OUT", StringComparison.Ordinal) || upper.Contains("NO_OUT", StringComparison.Ordinal) || upper.Contains("_NE", StringComparison.Ordinal) || upper.Contains("_NO", StringComparison.Ordinal)) return CliffFacing.NorthEastOuter;
        if (upper.Contains("NW_OUT", StringComparison.Ordinal) || upper.Contains("_NW", StringComparison.Ordinal)) return CliffFacing.NorthWestOuter;
        if (upper.Contains("SE_OUT", StringComparison.Ordinal) || upper.Contains("SO_OUT", StringComparison.Ordinal) || upper.Contains("_SE", StringComparison.Ordinal) || upper.Contains("_SO", StringComparison.Ordinal)) return CliffFacing.SouthEastOuter;
        if (upper.Contains("SW_OUT", StringComparison.Ordinal) || upper.Contains("_SW", StringComparison.Ordinal)) return CliffFacing.SouthWestOuter;

        // 正向坡
        if (upper.Contains("_N", StringComparison.Ordinal)) return CliffFacing.North;
        if (upper.Contains("_S", StringComparison.Ordinal)) return CliffFacing.South;
        if (upper.Contains("_E", StringComparison.Ordinal) || upper.Contains("_O", StringComparison.Ordinal)) return CliffFacing.East;
        if (upper.Contains("_W", StringComparison.Ordinal)) return CliffFacing.West;

        return CliffFacing.None;
    }
}
