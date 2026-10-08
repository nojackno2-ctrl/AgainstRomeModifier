namespace AgainstRomeMapEditor.Modules.Pathfinding;

/// <summary>
/// 通行網格分析的問題類別。
/// </summary>
public enum NavMeshIssueKind
{
    /// <summary>無法從玩家起始部隊或基地通達的孤立陸地區域。</summary>
    IsolatedLand,
    /// <summary>道路網絡中出現 1~2 格的微小中斷或端點未閉合。</summary>
    RoadGap,
    /// <summary>通道瓶頸寬度不足以容納 1~20 人部隊方陣通行。</summary>
    FormationBottleneck,
    /// <summary>通路沒入水下導致通行中斷。</summary>
    SubmergedPassage
}

/// <summary>
/// 網格與世界空間雙重座標表示。
/// </summary>
public readonly record struct NavMeshCoordinate(int TileX, int TileZ, float WorldX, float WorldZ)
{
    public static NavMeshCoordinate FromTile(int x, int z, int gridSize = 256, float worldDimension = 16384f)
    {
        float scale = worldDimension / gridSize;
        return new(x, z, (x + 0.5f) * scale, (z + 0.5f) * scale);
    }

    public static NavMeshCoordinate FromWorld(float worldX, float worldZ, int gridSize = 256, float worldDimension = 16384f)
    {
        float scale = worldDimension / gridSize;
        int x = Math.Clamp((int)(worldX / scale), 0, gridSize - 1);
        int z = Math.Clamp((int)(worldZ / scale), 0, gridSize - 1);
        return new(x, z, worldX, worldZ);
    }
}

/// <summary>
/// 孤立陸地區域資訊。
/// </summary>
public sealed record IsolatedRegion(
    int ComponentId,
    int TileCount,
    NavMeshCoordinate Centroid,
    int MinTileX,
    int MinTileZ,
    int MaxTileX,
    int MaxTileZ,
    bool ContainsTroopOrBuilding,
    NavMeshCoordinate? RecommendedBridgePoint = null,
    int BridgeDistance = 0);

/// <summary>
/// 偵測到的道路微小中斷候選。
/// </summary>
public sealed record RoadGapCandidate(
    NavMeshCoordinate Start,
    NavMeshCoordinate End,
    IReadOnlyList<NavMeshCoordinate> GapTiles,
    int GapDistance,
    string SuggestedTexture,
    bool RequiresCollisionClear,
    string Style = "Standard");

/// <summary>
/// 部隊方陣通道瓶頸評估結果。
/// </summary>
public sealed record FormationChokePoint(
    NavMeshCoordinate Location,
    int MeasuredClearanceTiles,
    float MeasuredWidthUnits,
    int RequiredClearanceTiles,
    float RequiredWidthUnits,
    int MaxSafeUnitCount,
    int BlockedTroopCount,
    string ChokeType = "Corridor");

/// <summary>
/// 單項原子修復變更操作（包含材質與通行網格）。
/// </summary>
public sealed record NavMeshTileChange(int TileX, int TileZ, string? NewTexture = null, byte? NewCollision = null);

/// <summary>
/// 具備可執行與可撤銷語意的自動修復建議動作。
/// </summary>
public sealed record NavMeshRepairAction(
    string Id,
    NavMeshIssueKind Kind,
    string TitleZh,
    string TitleEn,
    string DescriptionZh,
    string DescriptionEn,
    IReadOnlyList<NavMeshTileChange> Changes,
    NavMeshCoordinate FocusLocation);

/// <summary>
/// NavMesh 全域分析結果總覽。
/// </summary>
public sealed record NavMeshAnalysisResult(
    IReadOnlyList<IsolatedRegion> IsolatedRegions,
    IReadOnlyList<RoadGapCandidate> RoadGaps,
    IReadOnlyList<FormationChokePoint> FormationChokePoints,
    IReadOnlyList<NavMeshRepairAction> RecommendedRepairs)
{
    public static NavMeshAnalysisResult Empty { get; } = new([], [], [], []);
}
