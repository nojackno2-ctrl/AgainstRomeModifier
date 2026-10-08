using AgainstRomeMapEditor.Modules.Pathfinding;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class PathfindingGuardTests
{
    [Theory]
    [InlineData(-5, 0, 0f)]
    [InlineData(0, 0, 0f)]
    [InlineData(1, 1, 64f)]
    [InlineData(3, 2, 160f)]
    [InlineData(5, 2, 160f)]
    [InlineData(8, 4, 280f)]
    [InlineData(10, 4, 280f)]
    [InlineData(12, 5, 360f)]
    [InlineData(15, 5, 360f)]
    [InlineData(18, 6, 440f)]
    [InlineData(25, 6, 440f)]
    public void FormationWidthValidator_GetRequiredClearance_EvaluatesAllThresholds(
        int unitCount, int expectedTiles, float expectedWidth)
    {
        var (tiles, width) = FormationWidthValidator.GetRequiredClearance(unitCount);
        Assert.Equal(expectedTiles, tiles);
        Assert.Equal(expectedWidth, width);
    }

    [Theory]
    [InlineData(-1, 0)]
    [InlineData(0, 0)]
    [InlineData(1, 1)]
    [InlineData(2, 5)]
    [InlineData(3, 8)]
    [InlineData(4, 12)]
    [InlineData(5, 20)]
    [InlineData(10, 20)]
    public void FormationWidthValidator_GetMaxSafeUnitCount_EvaluatesAllThresholds(
        int clearanceTiles, int expectedMaxUnits)
    {
        Assert.Equal(expectedMaxUnits, FormationWidthValidator.GetMaxSafeUnitCount(clearanceTiles));
    }

    [Fact]
    public void FormationWidthValidator_GuardsAgainstNullArguments()
    {
        var grid = new NavMeshPassabilityGrid(16, new byte[16 * 16]);

        Assert.Throws<ArgumentNullException>(() =>
            FormationWidthValidator.ComputeClearanceField(null!));

        Assert.Throws<ArgumentNullException>(() =>
            FormationWidthValidator.EvaluateCorridors(null!, []));

        Assert.Throws<ArgumentNullException>(() =>
            FormationWidthValidator.EvaluateCorridors(grid, null!));

        Assert.Throws<ArgumentNullException>(() =>
            FormationWidthValidator.DetectTopologicalChokePoints(null!));
    }

    [Fact]
    public void FormationWidthValidator_EvaluateCorridors_IgnoresSingleUnitsAndBlockedPaths()
    {
        var grid = new NavMeshPassabilityGrid(16, new byte[16 * 16]);
        var start = grid.ToCoordinate(2, 2);
        var target = grid.ToCoordinate(10, 10);

        // 單兵 (count <= 1) 不產生瓶頸警告
        var chokesSingle = FormationWidthValidator.EvaluateCorridors(grid, [(start, target, 1)]);
        Assert.Empty(chokesSingle);

        // 建立阻擋牆使起點終點不連通
        byte[] blocked = new byte[16 * 16];
        for (int z = 0; z < 16; z++) blocked[z * 16 + 5] = 255;
        var blockedGrid = new NavMeshPassabilityGrid(16, blocked);

        var chokesBlocked = FormationWidthValidator.EvaluateCorridors(blockedGrid, [(start, target, 10)]);
        Assert.Empty(chokesBlocked); // 無可達路徑，優雅忽略
    }

    [Fact]
    public void NavMeshPassabilityGrid_GuardsAndOutOfBoundsBehavior()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            new NavMeshPassabilityGrid(0, null));

        Assert.Throws<ArgumentOutOfRangeException>(() =>
            new NavMeshPassabilityGrid(-10, null));

        const int size = 16;
        var grid = new NavMeshPassabilityGrid(size, new byte[size * size]);

        Assert.False(grid.IsInBounds(-1, 5));
        Assert.False(grid.IsInBounds(5, -1));
        Assert.False(grid.IsInBounds(size, 5));
        Assert.False(grid.IsInBounds(5, size));

        // 越界點查詢
        Assert.False(grid.IsPassable(-1, 5));
        Assert.True(grid.IsBlockedByCollision(-1, 5)); // 越界視為阻擋硬閘
        Assert.False(grid.IsSubmerged(-1, 5));
        Assert.Equal((byte)0, grid.GetCollisionByte(-1, 5));

        // WorldToTile 夾限
        var (minX, minZ) = grid.WorldToTile(-1000f, -500f);
        Assert.Equal(0, minX);
        Assert.Equal(0, minZ);

        var (maxX, maxZ) = grid.WorldToTile(99999f, 99999f);
        Assert.Equal(size - 1, maxX);
        Assert.Equal(size - 1, maxZ);

        // 無效 tileDimension
        Assert.True(grid.IsTileBlockedByCollision(0, 0, tileDimension: 0));
        Assert.False(grid.IsTileSubmerged(0, 0, tileDimension: 0));
    }

    [Fact]
    public void RoadGapDetector_GuardsAndTextureHelpers()
    {
        Assert.Throws<ArgumentNullException>(() =>
            RoadGapDetector.DetectGaps(16, null!));

        var textures = new string[16 * 16];
        Array.Fill(textures, "Gras1");

        // 維度 <= 0
        Assert.Throws<ArgumentException>(() =>
            RoadGapDetector.DetectGaps(0, textures));

        // 紋理長度不符
        Assert.Throws<ArgumentException>(() =>
            RoadGapDetector.DetectGaps(16, new string[10]));

        // 無道路圖塊的地圖
        var gaps = RoadGapDetector.DetectGaps(16, textures);
        Assert.Empty(gaps);

        // 紋理判斷輔助函式
        Assert.False(RoadGapDetector.IsRoadTexture(null));
        Assert.False(RoadGapDetector.IsRoadTexture(""));
        Assert.False(RoadGapDetector.IsRoadTexture("Gras_Green1"));
        Assert.True(RoadGapDetector.IsRoadTexture("H_WEG1"));
        Assert.True(RoadGapDetector.IsRoadTexture("V_WEG2"));
        Assert.True(RoadGapDetector.IsRoadTexture("Pflaster_braun1"));
        Assert.True(RoadGapDetector.IsRoadTexture("PFAD_Wald"));

        // 風格判斷輔助函式
        Assert.Equal("Standard", RoadGapDetector.DetectStyle(null!));
        Assert.Equal("Standard", RoadGapDetector.DetectStyle(""));
        Assert.Equal("Roman", RoadGapDetector.DetectStyle("WEG_H1ROM"));
        Assert.Equal("Roman", RoadGapDetector.DetectStyle("Pflaster_braun1"));
        Assert.Equal("Dirt", RoadGapDetector.DetectStyle("PFAD1"));
        Assert.Equal("Standard", RoadGapDetector.DetectStyle("H_WEG1"));
    }

    [Fact]
    public void RoadPathHealer_GuardsAgainstNullArguments()
    {
        Assert.Throws<ArgumentNullException>(() =>
            RoadPathHealer.CreateRepairActions(null!));

        Assert.Empty(RoadPathHealer.CreateRepairActions([]));

        Assert.Throws<ArgumentNullException>(() =>
            RoadPathHealer.CreateBridgeAction(null!));

        var regionWithoutBridge = new IsolatedRegion(
            ComponentId: 1,
            TileCount: 10,
            Centroid: NavMeshCoordinate.FromTile(5, 5, 16),
            MinTileX: 4, MinTileZ: 4, MaxTileX: 6, MaxTileZ: 6,
            ContainsTroopOrBuilding: false,
            RecommendedBridgePoint: null,
            BridgeDistance: 0);

        Assert.Null(RoadPathHealer.CreateBridgeAction(regionWithoutBridge));
    }

    [Fact]
    public void NavMeshConnectivityAnalyzer_GuardsAndFullyPassableMap()
    {
        var grid = new NavMeshPassabilityGrid(16, new byte[16 * 16]);

        Assert.Throws<ArgumentNullException>(() =>
            NavMeshConnectivityAnalyzer.Analyze(null!, [grid.ToCoordinate(2, 2)]));

        Assert.Throws<ArgumentNullException>(() =>
            NavMeshConnectivityAnalyzer.Analyze(grid, null!));

        // 種子點全在越界位置：優雅處理不拋異常
        var outOfBoundsSeed = NavMeshCoordinate.FromTile(-10, -10, 16);
        var isolated = NavMeshConnectivityAnalyzer.Analyze(grid, [outOfBoundsSeed]);
        Assert.NotEmpty(isolated); // 因沒有合法種子點覆蓋，整個可通行陸地都被判定為未通達分量

        // 全可通地圖，合法種子點覆蓋：孤立區域應為 0
        var validSeed = grid.ToCoordinate(5, 5);
        var noneIsolated = NavMeshConnectivityAnalyzer.Analyze(grid, [validSeed]);
        Assert.Empty(noneIsolated);
    }
}
