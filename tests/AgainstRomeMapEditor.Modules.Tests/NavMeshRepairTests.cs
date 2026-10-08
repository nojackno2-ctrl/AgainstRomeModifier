using AgainstRomeMapEditor.Modules.Diagnostics;
using AgainstRomeMapEditor.Modules.Pathfinding;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NavMeshRepairTests
{
    private static MapCheckObject Unit(float x = 1000, float z = 1000, int count = 10, int team = 0) =>
        new(new("UNIT", x, z, team, count) { Id = Guid.NewGuid() }, false, false);

    private static MapCheckObject House(float x, float z, int team = 0) =>
        new(new("HOUSE", x, z, team, Prebuilt: true) { Id = Guid.NewGuid() }, true, true);

    [Fact]
    public void ConnectivityAnalyzer_Detects_Isolated_Land_And_Finds_Bridge()
    {
        // 16x16 grid with a vertical wall at x = 8, splitting the grid into Left (x 0..7) and Right (x 9..15)
        int size = 16;
        byte[] collision = new byte[size * size];
        for (int z = 0; z < size; z++)
        {
            collision[z * size + 8] = 255; // vertical wall
        }

        var grid = new NavMeshPassabilityGrid(size, collision);
        var primarySeed = grid.ToCoordinate(2, 2); // on Left side
        var isolatedInterest = grid.ToCoordinate(12, 12); // on Right side

        var isolated = NavMeshConnectivityAnalyzer.Analyze(
            grid,
            [primarySeed],
            [isolatedInterest],
            minIsolatedSize: 1);

        Assert.Single(isolated);
        var region = isolated[0];
        Assert.True(region.ContainsTroopOrBuilding);
        Assert.Equal(16 * 7, region.TileCount); // 7 columns of passable tiles
        Assert.True(region.Centroid.TileX >= 9);
        Assert.NotNull(region.RecommendedBridgePoint);
        Assert.Equal(1, region.BridgeDistance); // wall thickness is 1 tile
        Assert.Equal(8, region.RecommendedBridgePoint.Value.TileX);
    }

    [Theory]
    [InlineData(2)]
    [InlineData(12)]
    [InlineData(13)]
    public void ConnectivityAnalyzer_Counts_wall_thickness_and_respects_search_limit(int thickness)
    {
        const int size = 32;
        var collision = new byte[size * size];
        for (int z = 0; z < size; z++)
        for (int x = 8; x < 8 + thickness; x++)
            collision[z * size + x] = 255;
        var grid = new NavMeshPassabilityGrid(size, collision);

        var region = Assert.Single(NavMeshConnectivityAnalyzer.Analyze(grid, [grid.ToCoordinate(2, 2)]));

        if (thickness <= 12)
        {
            Assert.Equal(thickness, region.BridgeDistance);
            Assert.NotNull(region.RecommendedBridgePoint);
            var bridge = region.RecommendedBridgePoint.Value;
            Assert.True(grid.IsBlockedByCollision(bridge.TileX, bridge.TileZ));
            Assert.InRange(bridge.TileX, 8 + (thickness - 1) / 2, 8 + thickness / 2);
        }
        else
        {
            Assert.Null(region.RecommendedBridgePoint);
            Assert.Equal(0, region.BridgeDistance);
        }
    }

    [Theory]
    [InlineData("H_WEG1", "WEG_V1ROM", false)]
    [InlineData("V_WEG1", "WEG_H1ROM", true)]
    public void RoadGapDetector_Does_not_join_incompatible_straight_road_directions(
        string startTexture, string endTexture, bool vertical)
    {
        const int size = 16;
        var textures = Enumerable.Repeat("Gras1", size * size).ToArray();
        textures[4 * size + 4] = startTexture;
        textures[(vertical ? 7 : 4) * size + (vertical ? 4 : 7)] = endTexture;

        Assert.Empty(RoadGapDetector.DetectGaps(size, textures));
    }

    [Fact]
    public void RoadGapDetector_Finds_1Tile_And_2Tile_Gaps_With_Correct_Style()
    {
        int dim = 16;
        var textures = Enumerable.Repeat("Gras1", dim * dim).ToArray();

        // 1. Horizontal standard road with 1 tile gap at (5, 4)
        textures[4 * dim + 3] = "H_WEG1";
        textures[4 * dim + 4] = "H_WEG1"; // endpoint at (4, 4)
        // gap at (5, 4)
        textures[4 * dim + 6] = "H_WEG2"; // endpoint at (6, 4)
        textures[4 * dim + 7] = "H_WEG2";

        // 2. Vertical Roman road with 2 tile gap at (10, 5) and (10, 6)
        textures[4 * dim + 10] = "WEG_V1ROM"; // endpoint at (10, 4)
        // gap at (10, 5) and (10, 6)
        textures[7 * dim + 10] = "WEG_V2ROM"; // endpoint at (10, 7)
        textures[8 * dim + 10] = "WEG_V2ROM";

        byte[] collision = new byte[dim * dim];
        collision[4 * dim + 5] = 255; // collision blocked at standard gap

        var passability = new NavMeshPassabilityGrid(dim, collision);
        var gaps = RoadGapDetector.DetectGaps(dim, textures, passability, maxGapDistance: 2);

        Assert.Equal(2, gaps.Count);

        var stdGap = gaps.First(g => g.Style == "Standard");
        Assert.Equal(1, stdGap.GapDistance);
        Assert.Single(stdGap.GapTiles);
        Assert.Equal(5, stdGap.GapTiles[0].TileX);
        Assert.Equal(4, stdGap.GapTiles[0].TileZ);
        Assert.Equal("H_WEG1", stdGap.SuggestedTexture);
        Assert.True(stdGap.RequiresCollisionClear);

        var romGap = gaps.First(g => g.Style == "Roman");
        Assert.Equal(2, romGap.GapDistance);
        Assert.Equal(2, romGap.GapTiles.Count);
        Assert.Equal("WEG_V1ROM", romGap.SuggestedTexture);
        Assert.False(romGap.RequiresCollisionClear);
    }

    [Fact]
    public void FormationWidthValidator_Identifies_Choke_Points_For_Squads()
    {
        // 16x16 grid with a single-tile corridor at x = 8, z in 4..8
        int size = 16;
        byte[] collision = new byte[size * size];

        // Fill wall on column 7 and 9, leaving column 8 open
        for (int z = 4; z <= 8; z++)
        {
            collision[z * size + 7] = 255;
            collision[z * size + 9] = 255;
        }

        var grid = new NavMeshPassabilityGrid(size, collision);
        var start = grid.ToCoordinate(8, 2);
        var target = grid.ToCoordinate(8, 12);

        // A 10-man squad requires 4 tiles clearance!
        var chokes = FormationWidthValidator.EvaluateCorridors(grid, [(start, target, 10)]);

        Assert.NotEmpty(chokes);
        var choke = chokes[0];
        Assert.Equal(1, choke.MeasuredClearanceTiles); // single tile between two walls
        Assert.Equal(10, choke.BlockedTroopCount);
        Assert.Equal(4, choke.RequiredClearanceTiles);
        Assert.Equal(1, choke.MaxSafeUnitCount); // only single unit can pass comfortably
    }

    [Fact]
    public void RoadPathHealer_Creates_Actionable_Repairs()
    {
        int dim = 16;
        var start = NavMeshCoordinate.FromTile(3, 5, dim);
        var end = NavMeshCoordinate.FromTile(5, 5, dim);
        var gapTile = NavMeshCoordinate.FromTile(4, 5, dim);

        var candidate = new RoadGapCandidate(start, end, [gapTile], 1, "H_WEG1", true, "Standard");
        var actions = RoadPathHealer.CreateRepairActions([candidate]);

        Assert.Single(actions);
        var action = actions[0];
        Assert.Equal(NavMeshIssueKind.RoadGap, action.Kind);
        Assert.Single(action.Changes);
        Assert.Equal(4, action.Changes[0].TileX);
        Assert.Equal(5, action.Changes[0].TileZ);
        Assert.Equal("H_WEG1", action.Changes[0].NewTexture);
        Assert.Equal((byte)0, action.Changes[0].NewCollision);
    }

    [Fact]
    public void MapDiagnostics_Integration_Emits_NavMesh_Issues_Seamlessly()
    {
        int size = 16;
        byte[] collision = new byte[size * size];
        // Vertical wall dividing the map
        for (int z = 0; z < size; z++) collision[z * size + 8] = 255;

        // Player start troop at (2, 2)
        var playerTroop = Unit(x: 2 * 64 + 32, z: 2 * 64 + 32, count: 10, team: 0);
        // Enemy building isolated at (12, 12)
        var enemyHouse = House(x: 12 * 64 + 32, z: 12 * 64 + 32, team: 1);

        var snapshot = new MapCheckSnapshot(
            [playerTroop, enemyHouse],
            [],
            ["UNIT", "HOUSE"],
            CollisionSize: size,
            Collision: collision);

        var navResult = MapDiagnostics.AnalyzeNavMesh(snapshot);
        var issues = MapDiagnostics.ConvertToIssues(navResult);

        Assert.NotEmpty(navResult.IsolatedRegions);
        Assert.Contains(issues, i => i.Code == "isolated-land");
        Assert.All(issues, i => Assert.Equal(MapIssueSeverity.Warning, i.Severity));
    }
}
