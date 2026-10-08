namespace AgainstRomeMapEditor;

/// <summary>
/// 河道正交連通方向遮罩（以 X, Y 地圖座標軸為基準）。
/// North: (0, -1), East: (+1, 0), South: (0, +1), West: (-1, 0)。
/// </summary>
[Flags]
public enum RiverConnections : byte
{
    None = 0,
    North = 1 << 0, // 1
    East = 1 << 1,  // 2
    South = 1 << 2, // 4
    West = 1 << 3,  // 8
    All = North | East | South | West
}

/// <summary>
/// 水流流入與流出方向（八向水文遮罩）。
/// 支援單進單出（直河/彎河）、多進單出（匯流/Confluence）、單進多出（分流/Delta）與源頭/河口。
/// </summary>
[Flags]
public enum RiverFlowDirections : byte
{
    None = 0,
    InflowNorth = 1 << 0,
    InflowEast = 1 << 1,
    InflowSouth = 1 << 2,
    InflowWest = 1 << 3,
    OutflowNorth = 1 << 4,
    OutflowEast = 1 << 5,
    OutflowSouth = 1 << 6,
    OutflowWest = 1 << 7
}

/// <summary>
/// 河道圖塊形態分類。
/// </summary>
public enum RiverTileKind
{
    /// <summary>水源 / 泉水起點（單端點開口）</summary>
    Source,
    /// <summary>直線河道（南北 N|S 或 東西 E|W）</summary>
    Straight,
    /// <summary>90度轉角河道（NE, NW, SE, SW）</summary>
    Turn,
    /// <summary>T型匯流 / 三向交界（如 N|E|S）</summary>
    TConfluence,
    /// <summary>四向分流 / 十字交叉（N|E|S|W）</summary>
    Cross,
    /// <summary>出水口 / 入湖河口（流入開闊水體）</summary>
    Estuary,
    /// <summary>開闊水體 / 湖泊本體（SEE 系列或開放水面）</summary>
    LakeWater
}

/// <summary>
/// 河岸過渡方位（相對於河道本體之所在方位，或相對於水流方向的左右岸）。
/// </summary>
[Flags]
public enum RiverBankSide : byte
{
    None = 0,
    /// <summary>位於河道北方之河岸（南邊緊鄰河道，例如 ErdeFlussR1）</summary>
    North = 1 << 0,
    /// <summary>位於河道東方之河岸（西邊緊鄰河道，例如 ErdeFlussR2）</summary>
    East = 1 << 1,
    /// <summary>位於河道南方之河岸（北邊緊鄰河道，例如 ErdeFlussR3）</summary>
    South = 1 << 2,
    /// <summary>位於河道西方之河岸（東邊緊鄰河道，例如 ErdeFlussR4）</summary>
    West = 1 << 3
}

/// <summary>
/// 已註冊之河道原生圖塊元資料。
/// </summary>
public sealed record RiverTile(
    string Texture,
    RiverConnections Connections,
    RiverTileKind Kind = RiverTileKind.Straight,
    RiverFlowDirections Flow = RiverFlowDirections.None,
    int Variant = 1
);

/// <summary>
/// 河水水面圖塊規劃擺放。
/// </summary>
public sealed record RiverTilePlacement(
    int X,
    int Y,
    string Texture,
    RiverConnections Connections,
    RiverTileKind Kind,
    RiverFlowDirections Flow
);

/// <summary>
/// 河岸過渡圖塊規劃擺放（例如 ErdeFlussR 系列）。
/// </summary>
public sealed record RiverBankPlacement(
    int X,
    int Y,
    string Texture,
    RiverBankSide Side
);

/// <summary>
/// 高度場調整（頂點高度）。
/// </summary>
public sealed record RiverHeightAdjustment(
    int Index,
    int VertexX,
    int VertexY,
    byte Before,
    byte After
);

/// <summary>
/// 高度落差與逆流警示。
/// </summary>
public sealed record RiverElevationWarning(
    int X,
    int Y,
    int CurrentElevation,
    int TargetElevation,
    string Message
);

/// <summary>
/// 高度場處理模式。
/// </summary>
public enum RiverElevationMode
{
    /// <summary>僅驗證高度落差，遇到逆流回報警示但不改動高度場</summary>
    ValidateOnly,
    /// <summary>自動向下微調河床頂點高度，保證由上游往下游嚴格非遞增（避免逆流倒灌）</summary>
    AutoCarveDescending,
    /// <summary>保證順流下降的同時，相較於周邊河岸向下挖深指定河床深度（TrenchDepth）</summary>
    AutoCarveTrench
}

/// <summary>
/// 河流水系規劃參數設定。
/// </summary>
public sealed record RiverPlannerOptions(
    RiverElevationMode ElevationMode = RiverElevationMode.AutoCarveDescending,
    bool AutoDetectDownhillFlow = false,
    bool GenerateBankTransitions = true,
    int TrenchDepth = 2,
    int MinSlopeDrop = 1,
    string? PreferredRiverTexture = null,
    string? PreferredBankTexture = null
);

/// <summary>
/// 河道筆畫完整規劃產出。
/// </summary>
public sealed record RiverStrokePlan(
    bool Succeeded,
    IReadOnlyList<RiverTilePlacement> WaterTiles,
    IReadOnlyList<RiverBankPlacement> BankTiles,
    IReadOnlyList<RiverHeightAdjustment> HeightAdjustments,
    IReadOnlyList<RiverElevationWarning> Warnings,
    IReadOnlyList<(int X, int Y)> Unsupported
);

/// <summary>
/// 與 TerrainBlendEditSession 整合之河道繪製回傳成果。
/// </summary>
internal sealed record TerrainRiverPaintResult(
    bool Succeeded,
    IReadOnlyList<TerrainTextureChange> TextureChanges,
    IReadOnlyList<TerrainSampleChange> HeightChanges,
    IReadOnlyList<RiverElevationWarning> Warnings,
    IReadOnlyList<(int X, int Y)> Unsupported
);
