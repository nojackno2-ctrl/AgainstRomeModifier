namespace AgainstRomeMapEditor;

/// <summary>
/// 《反抗羅馬》水系與河流圖塊目錄系統。
/// 負責辨識 floortex.dat 中的水系圖塊（FLUSS 河道、ErdeFlussR 河岸邊緣、SEE 湖泊水體與 4S 岸邊過渡），
/// 並將其結構化映射至水文方向、轉向角、匯流處與河岸方位。
/// </summary>
public sealed class RiverTileCatalog
{
    // 原版標準河道素材（FLUSS 系列）
    private static readonly string[] NativeHorizontalStraights = ["FLUSS1", "FLUSS2"];
    private static readonly string[] NativeVerticalStraights = ["FLUSS1", "FLUSS3"];
    private static readonly string[] NativeTurnsNeNw = ["FLUSS1", "FLUSS5"];
    private static readonly string[] NativeTurnsSeSw = ["FLUSS1", "FLUSS4"];
    private static readonly string[] NativeJunctions = ["FLUSS1", "FLUSS4", "FLUSS5"];
    private static readonly string[] NativeAllFluss = ["FLUSS1", "FLUSS2", "FLUSS3", "FLUSS4", "FLUSS5"];

    // 原版河岸泥地過渡素材（ErdeFlussR 系列：泥土到河床邊緣）
    private static readonly string NativeNorthBank = "ErdeFlussR1";
    private static readonly string NativeEastBank = "ErdeFlussR2";
    private static readonly string NativeSouthBank = "ErdeFlussR3";
    private static readonly string NativeWestBank = "ErdeFlussR4";

    private readonly Dictionary<RiverConnections, List<RiverTile>> _tilesByConnection = new();
    private readonly Dictionary<RiverBankSide, List<string>> _bankTilesBySide = new();
    private readonly HashSet<string> _allWaterTextures = new(StringComparer.OrdinalIgnoreCase);
    private readonly HashSet<string> _allBankTextures = new(StringComparer.OrdinalIgnoreCase);

    public IReadOnlyList<RiverTile> AvailableRiverTiles { get; }
    public IReadOnlyDictionary<RiverBankSide, IReadOnlyList<string>> AvailableBankTiles => _bankTilesBySide.ToDictionary(k => k.Key, v => (IReadOnlyList<string>)v.Value);
    public string? PreferredRiverTexture { get; }
    public string? PreferredBankTexture { get; }

    public RiverTileCatalog(
        IEnumerable<RiverTile> riverTiles,
        IDictionary<RiverBankSide, IEnumerable<string>>? bankTiles = null,
        string? preferredRiver = null,
        string? preferredBank = null)
    {
        PreferredRiverTexture = preferredRiver;
        PreferredBankTexture = preferredBank;
        var allRiver = new List<RiverTile>();

        foreach (var tile in riverTiles)
        {
            allRiver.Add(tile);
            _allWaterTextures.Add(tile.Texture);
            if (!_tilesByConnection.TryGetValue(tile.Connections, out var list))
            {
                list = new List<RiverTile>();
                _tilesByConnection[tile.Connections] = list;
            }
            list.Add(tile);
        }
        AvailableRiverTiles = allRiver;

        if (bankTiles is not null)
        {
            foreach (var (side, textures) in bankTiles)
            {
                var sideList = new List<string>();
                foreach (var tex in textures)
                {
                    sideList.Add(tex);
                    _allBankTextures.Add(tex);
                }
                _bankTilesBySide[side] = sideList;
            }
        }
    }

    /// <summary>
    /// 從現有地圖貼圖清單或 floortex.dat 項目中自動檢索並建立水系圖塊目錄。
    /// 包含風格優先挑選、缺少專用轉角片時之安全回退與單端點直線平滑延伸。
    /// </summary>
    public static RiverTileCatalog BuildAvailable(
        IEnumerable<string>? existingTextureNames,
        string? preferredRiverTexture = null,
        string? preferredBankTexture = null)
    {
        HashSet<string>? existing = existingTextureNames is null
            ? null
            : new HashSet<string>(existingTextureNames, StringComparer.OrdinalIgnoreCase);

        bool IsAvailable(string name) => existing is null || existing.Contains(name);

        var riverTiles = new List<RiverTile>();
        var bankTiles = new Dictionary<RiverBankSide, IEnumerable<string>>();

        // 1. 檢索河道圖塊
        var availFluss = NativeAllFluss.Where(IsAvailable).ToArray();
        string baseRiver = preferredRiverTexture is not null && IsAvailable(preferredRiverTexture)
            ? preferredRiverTexture
            : (availFluss.FirstOrDefault() ?? "FLUSS1");

        var hStraights = NativeHorizontalStraights.Where(IsAvailable).ToArray();
        var vStraights = NativeVerticalStraights.Where(IsAvailable).ToArray();
        var turnsNeNw = NativeTurnsNeNw.Where(IsAvailable).ToArray();
        var turnsSeSw = NativeTurnsSeSw.Where(IsAvailable).ToArray();
        var junctions = NativeJunctions.Where(IsAvailable).ToArray();

        // 確保至少有一種可用河道圖塊
        if (hStraights.Length == 0) hStraights = [baseRiver];
        if (vStraights.Length == 0) vStraights = [baseRiver];
        if (turnsNeNw.Length == 0) turnsNeNw = [baseRiver];
        if (turnsSeSw.Length == 0) turnsSeSw = [baseRiver];
        if (junctions.Length == 0) junctions = [baseRiver];

        // 直線水平 (East | West)
        foreach (var t in hStraights)
            riverTiles.Add(new RiverTile(t, RiverConnections.East | RiverConnections.West, RiverTileKind.Straight));

        // 直線垂直 (North | South)
        foreach (var t in vStraights)
            riverTiles.Add(new RiverTile(t, RiverConnections.North | RiverConnections.South, RiverTileKind.Straight));

        // 轉角 (NE, NW, SE, SW)
        foreach (var t in turnsNeNw)
        {
            riverTiles.Add(new RiverTile(t, RiverConnections.North | RiverConnections.East, RiverTileKind.Turn));
            riverTiles.Add(new RiverTile(t, RiverConnections.North | RiverConnections.West, RiverTileKind.Turn));
        }
        foreach (var t in turnsSeSw)
        {
            riverTiles.Add(new RiverTile(t, RiverConnections.South | RiverConnections.East, RiverTileKind.Turn));
            riverTiles.Add(new RiverTile(t, RiverConnections.South | RiverConnections.West, RiverTileKind.Turn));
        }

        // T型匯流 (3向) 與 十字交叉 (4向)
        RiverConnections[] tConns =
        [
            RiverConnections.North | RiverConnections.East | RiverConnections.South,
            RiverConnections.North | RiverConnections.East | RiverConnections.West,
            RiverConnections.North | RiverConnections.South | RiverConnections.West,
            RiverConnections.East | RiverConnections.South | RiverConnections.West
        ];
        foreach (var conn in tConns)
        {
            foreach (var j in junctions)
                riverTiles.Add(new RiverTile(j, conn, RiverTileKind.TConfluence));
        }

        foreach (var j in junctions)
            riverTiles.Add(new RiverTile(j, RiverConnections.All, RiverTileKind.Cross));

        // 單端點 (水源/出海口/河口)
        RiverConnections[] singleEnds = [RiverConnections.North, RiverConnections.East, RiverConnections.South, RiverConnections.West];
        foreach (var end in singleEnds)
        {
            string endTex = (end is RiverConnections.East or RiverConnections.West) ? hStraights[0] : vStraights[0];
            riverTiles.Add(new RiverTile(endTex, end, RiverTileKind.Source));
            riverTiles.Add(new RiverTile(endTex, end, RiverTileKind.Estuary));
        }

        // 孤立水塘/開闊水體 (None)
        riverTiles.Add(new RiverTile(baseRiver, RiverConnections.None, RiverTileKind.LakeWater));

        // 2. 檢索河岸過渡圖塊 (ErdeFlussR1..4)
        RegisterBankIfAvailable(RiverBankSide.North, NativeNorthBank);
        RegisterBankIfAvailable(RiverBankSide.East, NativeEastBank);
        RegisterBankIfAvailable(RiverBankSide.South, NativeSouthBank);
        RegisterBankIfAvailable(RiverBankSide.West, NativeWestBank);

        void RegisterBankIfAvailable(RiverBankSide side, string nativeName)
        {
            if (IsAvailable(nativeName))
            {
                bankTiles[side] = [nativeName];
            }
            else if (!string.IsNullOrEmpty(preferredBankTexture) && IsAvailable(preferredBankTexture))
            {
                bankTiles[side] = [preferredBankTexture];
            }
        }

        return new RiverTileCatalog(riverTiles, bankTiles, baseRiver, preferredBankTexture);
    }

    /// <summary>
    /// 依連通遮罩、河道類型與風格挑選最合適之河道圖塊。
    /// </summary>
    public RiverTile? FindWaterTile(RiverConnections connections, RiverTileKind kind = RiverTileKind.Straight, string? preferred = null, int seed = 0)
    {
        string pref = preferred ?? PreferredRiverTexture ?? "";
        if (_tilesByConnection.TryGetValue(connections, out var list) && list.Count > 0)
        {
            // 優先挑選偏好材質
            if (!string.IsNullOrEmpty(pref))
            {
                var preferredMatch = list.FirstOrDefault(t => StringComparer.OrdinalIgnoreCase.Equals(t.Texture, pref));
                if (preferredMatch is not null) return preferredMatch;
            }
            int index = Math.Abs(seed) % list.Count;
            return list[index];
        }

        // 安全回退：端點連通可退化為直河片
        if (IsSingleEndpoint(connections))
        {
            var fallbackConn = (connections is RiverConnections.East or RiverConnections.West)
                ? (RiverConnections.East | RiverConnections.West)
                : (RiverConnections.North | RiverConnections.South);
            if (_tilesByConnection.TryGetValue(fallbackConn, out var fallbackList) && fallbackList.Count > 0)
                return fallbackList[Math.Abs(seed) % fallbackList.Count];
        }

        // 安全回退：任何可用河道圖塊
        if (AvailableRiverTiles.Count > 0)
            return AvailableRiverTiles[Math.Abs(seed) % AvailableRiverTiles.Count];

        return null;
    }

    /// <summary>
    /// 依河岸方位挑選對應的河岸泥地過渡圖塊（例如 ErdeFlussR1..4）。
    /// </summary>
    public string? FindBankTile(RiverBankSide side, string? preferred = null, int seed = 0)
    {
        string pref = preferred ?? PreferredBankTexture ?? "";
        if (_bankTilesBySide.TryGetValue(side, out var list) && list.Count > 0)
        {
            if (!string.IsNullOrEmpty(pref))
            {
                var match = list.FirstOrDefault(t => StringComparer.OrdinalIgnoreCase.Equals(t, pref));
                if (match is not null) return match;
            }
            return list[Math.Abs(seed) % list.Count];
        }

        // 若無特定朝向河岸圖塊，但有任何註冊之河岸圖塊
        if (!string.IsNullOrEmpty(PreferredBankTexture)) return PreferredBankTexture;
        var anySide = _bankTilesBySide.Values.FirstOrDefault(l => l.Count > 0);
        return anySide?[0];
    }

    public bool IsWaterTexture(string texture)
    {
        if (string.IsNullOrWhiteSpace(texture)) return false;
        if (_allWaterTextures.Contains(texture)) return true;
        string upper = texture.ToUpperInvariant();
        return upper.StartsWith("FLUSS", StringComparison.Ordinal)
            || upper.StartsWith("SEE", StringComparison.Ordinal)
            || upper.StartsWith("WASSER", StringComparison.Ordinal)
            || (upper.Length > 2 && upper.StartsWith("4S", StringComparison.Ordinal));
    }

    public bool IsBankTexture(string texture)
    {
        if (string.IsNullOrWhiteSpace(texture)) return false;
        if (_allBankTextures.Contains(texture)) return true;
        return texture.StartsWith("ErdeFlussR", StringComparison.OrdinalIgnoreCase);
    }

    private static bool IsSingleEndpoint(RiverConnections connections) =>
        connections is RiverConnections.North or RiverConnections.East or RiverConnections.South or RiverConnections.West;
}
