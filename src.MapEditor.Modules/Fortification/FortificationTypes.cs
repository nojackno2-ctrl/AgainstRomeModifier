namespace AgainstRomeMapEditor.Modules.Fortification;

/// <summary>
/// 碰撞像素變更紀錄。
/// </summary>
public readonly record struct CollisionPixelChange(int Index, byte Before, byte After);

/// <summary>
/// 防禦工事文化與建築風格。
/// </summary>
public enum FortificationStyle
{
    /// <summary>羅馬石牆與城樓（堅固石砌、垛口石階、重型城門）</summary>
    RomanStoneWall,

    /// <summary>日耳曼木質拒馬與原木柵欄（削尖木樁、木造角樓、吊門）</summary>
    GermanicPalisade,

    /// <summary>凱爾特木質防禦工事</summary>
    CelticPalisade,

    /// <summary>匈人石木拒馬與野戰工事</summary>
    HunBarricade
}

/// <summary>
/// 防禦工事元件拓撲類型。
/// </summary>
public enum WallComponentKind
{
    /// <summary>直牆（雙向通，南北或東西）</summary>
    Straight,

    /// <summary>90度直角拐角（雙向折轉，L型）</summary>
    Corner,

    /// <summary>T型分岐三向接頭</summary>
    TJunction,

    /// <summary>十字交叉四向接頭</summary>
    CrossJunction,

    /// <summary>城門 / 柵門（具備受控通行通道與門柱）</summary>
    Gate,

    /// <summary>防禦塔 / 堡壘底座（具備獨立視野與射程，常配置於轉角或接頭）</summary>
    Tower,

    /// <summary>端點平滑收尾（垛口終端、石墩或端柱）</summary>
    EndCap
}

/// <summary>
/// 四向正交連通遮罩（以地圖網格 X, Z 軸向為基準）。
/// North: Z-1, East: X+1, South: Z+1, West: X-1。
/// </summary>
[Flags]
public enum WallConnections : byte
{
    None = 0,
    North = 1 << 0,
    East = 1 << 1,
    South = 1 << 2,
    West = 1 << 3,
    All = North | East | South | West
}

/// <summary>
/// 城門通行閘門控制狀態。
/// </summary>
public enum GatePassabilityState
{
    /// <summary>門戶開放：中央通道通行網格 collision=0，友軍與平民可自由進出</summary>
    Open,

    /// <summary>戰時關閉：中央通道通行網格 collision=255，完全封鎖阻擋</summary>
    Closed
}

/// <summary>
/// 單一防禦工事原生物件之圖元定義。
/// </summary>
public sealed record WallComponentDefinition(
    string NameDef,
    WallComponentKind Kind,
    WallConnections SupportedConnections,
    float DefaultAngleDeg = 0f,
    int FootprintTiles = 1,
    int CollisionPixelSpan = 4,
    string? FoundationTexture = null)
{
    public bool Matches(WallComponentKind kind, WallConnections connections) =>
        Kind == kind && (SupportedConnections & connections) == connections;
}

/// <summary>
/// 規劃生成之單一防禦工事擺放資料實體。
/// </summary>
public sealed record FortificationPlacement(
    Guid Id,
    string NameDef,
    WallComponentKind Kind,
    int TileX,
    int TileZ,
    float WorldX,
    float WorldY,
    float WorldZ,
    float AngleDeg,
    int Team = 0,
    GatePassabilityState? GateState = null,
    string? FoundationTexture = null);

/// <summary>
/// 筆畫路徑規劃選項參數。
/// </summary>
public sealed record WallStrokeOptions
{
    /// <summary>使用的防禦工事風格（預設羅馬石牆）</summary>
    public FortificationStyle Style { get; init; } = FortificationStyle.RomanStoneWall;

    /// <summary>歸屬隊伍（0..7，預設 0）</summary>
    public int Team { get; init; }

    /// <summary>轉角與十字路口自動晉升為防禦塔</summary>
    public bool AutoCornerTowers { get; init; } = true;

    /// <summary>與既有道路（石道/土路）交會時自動嵌合城門</summary>
    public bool AutoGateOnRoadCrossing { get; init; } = true;

    /// <summary>起訖端點平滑收尾（EndCap 端柱）</summary>
    public bool SmoothEndCaps { get; init; } = true;

    /// <summary>地表自動印製地基碎石/夯土材質</summary>
    public bool StampFoundations { get; init; } = true;

    /// <summary>城門預設通行狀態（開放或關閉）</summary>
    public GatePassabilityState DefaultGateState { get; init; } = GatePassabilityState.Open;

    /// <summary>自訂指定城門放置的格位座標清單（強制嵌門）</summary>
    public IReadOnlyCollection<(int X, int Z)>? ForcedGateLocations { get; init; }
}

/// <summary>
/// 規劃成果報告。
/// </summary>
public sealed record WallStrokePlan(
    bool Succeeded,
    IReadOnlyList<FortificationPlacement> Placements,
    IReadOnlyList<(int TileX, int TileZ)> FootprintTiles,
    IReadOnlyList<(int TileX, int TileZ)> GatePassageTiles,
    IReadOnlyList<(int TileX, int TileZ)> BlockedTiles,
    IReadOnlyList<(int TileX, int TileZ)> UnsupportedTiles,
    string? FailureReason = null);

/// <summary>
/// 防禦工事閉合度與要塞圍閉分析報告。
/// </summary>
public sealed record FortressEnclosureReport(
    bool IsFullyEnclosed,
    int EnclosedAreaTiles,
    IReadOnlyList<(int TileX, int TileZ)> Gaps,
    IReadOnlyList<FortificationPlacement> Gates);
