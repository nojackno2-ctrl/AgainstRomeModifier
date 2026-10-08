namespace AgainstRomeMapEditor.Modules.Profiling;

/// <summary>
/// 地圖整體健康度評級。
/// </summary>
public enum MapHealthGrade
{
    /// <summary>健康：各項指標均在安全預算範圍內 (綠色)。</summary>
    Healthy,

    /// <summary>注意：部分指標達到 60% ~ 80% 警戒區間 (黃色)。</summary>
    Caution,

    /// <summary>警告：指標超過 80% 或存在局部隘口/光源聚集 (橘色)。</summary>
    Warning,

    /// <summary>嚴重：指標接近引擎極限 (95%+) 或局部光源達到 60+ (紅色)。</summary>
    Critical,

    /// <summary>超標：已突破原生引擎硬限制，將直接導致遊戲崩潰或重大渲染瑕疵 (紫紅色)。</summary>
    EngineExceeded
}

/// <summary>
/// 具體效能瓶頸條目，支援坐標跳轉定位與中英雙語。
/// </summary>
public sealed record MapHealthBottleneck(
    string Category,
    string Code,
    string ChineseDescription,
    string EnglishDescription,
    float? WorldX = null,
    float? WorldZ = null,
    bool IsEngineLimitViolation = false
);

/// <summary>
/// 物件插槽與資料池預算指標。
/// </summary>
public sealed record ObjectSlotBudget(
    int TotalActiveObjects,
    int ObjectsDatLimit,
    float ObjectsUsagePercent,
    int EstimatedPositionRecords,
    int PositionLimit,
    float PositionUsagePercent,
    int ScenarioSpawnsCount,
    int LandscapeCount,
    int BuildingCount,
    int UnitCount,
    int FxCount,
    int OtherCount
);

/// <summary>
/// 局部高密度光源熱點。
/// </summary>
public sealed record LightClusterHotspot(
    float WorldX,
    float WorldZ,
    int LightCount,
    float Radius,
    bool ExceedsHardLimit
);

/// <summary>
/// 光源密度與熱力圖分析指標。
/// </summary>
public sealed record LightDensityBudget(
    int TotalSceneLights,
    int PeakClusterLights,
    float PeakDensityRatio, // Peak / 64.0f
    IReadOnlyList<LightClusterHotspot> Hotspots,
    int ClusterGridResolution, // 例如 32 (代表 32x32 網格)
    float[,] DensityGrid       // 0.0 ~ 1.0 正規化密度 (峰值基準)
);

/// <summary>
/// 繪圖呼叫 (Draw Call) 與圖集頁面切換開銷預估指標。
/// </summary>
public sealed record DrawCallBudget(
    int EstimatedTotalDrawCalls,
    int TerrainDrawCalls,
    int SpriteBatchDrawCalls,
    int ShadowBatchDrawCalls,
    int EstimatedAtlasPages,
    int EstimatedAtlasSwitches,
    float BatchFragmentationScore, // 0.0 ~ 1.0 (越接近 1.0 碎片化越嚴重)
    int TotalSpriteQuads,
    int TotalShadowQuads
);

/// <summary>
/// 尋路隘口瓶頸坐標。
/// </summary>
public sealed record ChokepointLocation(
    int CellX,
    int CellZ,
    float WorldX,
    float WorldZ,
    int PassageWidthCells
);

/// <summary>
/// 尋路網格負載與隘口複雜度指標。
/// </summary>
public sealed record PathfindingBudget(
    float ObstacleDensityPercent,
    int PassableRegionsCount,
    int ChokepointCount,
    IReadOnlyList<ChokepointLocation> Chokepoints,
    int DeadEndCount,
    int PathfindingComplexityScore, // 0 ~ 100 分
    string ComplexityRating         // Low, Medium, High, Severe
);

/// <summary>
/// 完整的地圖預算與效能分析報告。
/// </summary>
public sealed record MapBudgetReport(
    DateTimeOffset Timestamp,
    MapHealthGrade OverallGrade,
    int OverallScore, // 0 ~ 100 (100 為最健康，0 為最危險)
    ObjectSlotBudget ObjectBudget,
    LightDensityBudget LightBudget,
    DrawCallBudget DrawBudget,
    PathfindingBudget PathBudget,
    IReadOnlyList<MapHealthBottleneck> Bottlenecks
);
