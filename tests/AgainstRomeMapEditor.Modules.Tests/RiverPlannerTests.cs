using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class RiverPlannerTests
{
    private static string[] BlankTextures(int dimension = 8, string fill = "grass") =>
        Enumerable.Repeat(fill, dimension * dimension).ToArray();

    private static RiverTileCatalog NativeCatalog() =>
        RiverTileCatalog.BuildAvailable(["FLUSS1", "FLUSS2", "FLUSS3", "FLUSS4", "FLUSS5", "ErdeFlussR1", "ErdeFlussR2", "ErdeFlussR3", "ErdeFlussR4"]);

    [Fact]
    public void Catalog_identifies_straights_turns_junctions_and_banks()
    {
        var catalog = NativeCatalog();

        Assert.NotEmpty(catalog.AvailableRiverTiles);
        Assert.NotEmpty(catalog.AvailableBankTiles);

        // 水平直河
        var hTile = catalog.FindWaterTile(RiverConnections.East | RiverConnections.West, RiverTileKind.Straight);
        Assert.NotNull(hTile);
        Assert.Contains(hTile.Texture, new[] { "FLUSS1", "FLUSS2" });

        // 垂直直河
        var vTile = catalog.FindWaterTile(RiverConnections.North | RiverConnections.South, RiverTileKind.Straight);
        Assert.NotNull(vTile);
        Assert.Contains(vTile.Texture, new[] { "FLUSS1", "FLUSS3" });

        // 轉角 (North | East)
        var turnTile = catalog.FindWaterTile(RiverConnections.North | RiverConnections.East, RiverTileKind.Turn);
        Assert.NotNull(turnTile);

        // T型匯流 (North | East | South)
        var tTile = catalog.FindWaterTile(RiverConnections.North | RiverConnections.East | RiverConnections.South, RiverTileKind.TConfluence);
        Assert.NotNull(tTile);

        // 河岸 (北岸 R1, 東岸 R2, 南岸 R3, 西岸 R4)
        Assert.Equal("ErdeFlussR1", catalog.FindBankTile(RiverBankSide.North));
        Assert.Equal("ErdeFlussR2", catalog.FindBankTile(RiverBankSide.East));
        Assert.Equal("ErdeFlussR3", catalog.FindBankTile(RiverBankSide.South));
        Assert.Equal("ErdeFlussR4", catalog.FindBankTile(RiverBankSide.West));
    }

    [Fact]
    public void Catalog_falls_back_gracefully_when_minimal_textures_available()
    {
        var catalog = RiverTileCatalog.BuildAvailable(["FLUSS1"]);
        Assert.NotNull(catalog.FindWaterTile(RiverConnections.East | RiverConnections.West));
        Assert.NotNull(catalog.FindWaterTile(RiverConnections.North | RiverConnections.East));
        Assert.Equal("FLUSS1", catalog.FindWaterTile(RiverConnections.All)?.Texture);
    }

    [Fact]
    public void Plan_straight_horizontal_and_vertical_river_path()
    {
        var catalog = NativeCatalog();
        var textures = BlankTextures(8);

        // (1, 3) -> (5, 3) 水平直河
        var planH = RiverFlowPlanner.Plan(8, textures, null, 0, [(1, 3), (5, 3)], catalog);
        Assert.True(planH.Succeeded);
        Assert.Equal(5, planH.WaterTiles.Count);

        // 中間河段應為水平直河連通
        var midH = planH.WaterTiles.Single(t => t.X == 3 && t.Y == 3);
        Assert.Equal(RiverConnections.East | RiverConnections.West, midH.Connections);
        Assert.Equal(RiverTileKind.Straight, midH.Kind);
        Assert.True(midH.Flow.HasFlag(RiverFlowDirections.InflowWest) || midH.Flow.HasFlag(RiverFlowDirections.OutflowEast));

        // (3, 1) -> (3, 5) 垂直直河
        var planV = RiverFlowPlanner.Plan(8, textures, null, 0, [(3, 1), (3, 5)], catalog);
        Assert.True(planV.Succeeded);
        Assert.Equal(5, planV.WaterTiles.Count);

        var midV = planV.WaterTiles.Single(t => t.X == 3 && t.Y == 3);
        Assert.Equal(RiverConnections.North | RiverConnections.South, midV.Connections);
        Assert.Equal(RiverTileKind.Straight, midV.Kind);
    }

    [Fact]
    public void Plan_turn_and_meander_selects_turn_tiles()
    {
        var catalog = NativeCatalog();
        var textures = BlankTextures(8);

        // L型河道：(1, 2) -> (4, 2) -> (4, 6)
        var plan = RiverFlowPlanner.Plan(8, textures, null, 0, [(1, 2), (4, 2), (4, 6)], catalog);
        Assert.True(plan.Succeeded);

        // 轉角 (4, 2) 連接 West 與 South
        var corner = plan.WaterTiles.Single(t => t.X == 4 && t.Y == 2);
        Assert.Equal(RiverConnections.West | RiverConnections.South, corner.Connections);
        Assert.Equal(RiverTileKind.Turn, corner.Kind);
    }

    [Fact]
    public void Plan_merges_with_existing_river_into_confluence()
    {
        var catalog = NativeCatalog();
        var textures = BlankTextures(8);

        // 先鋪設一條橫貫東西的主河道
        for (int x = 0; x < 8; x++) textures[3 * 8 + x] = "FLUSS1";

        // 繪製由北向南匯入的支流：(3, 0) -> (3, 3)
        var plan = RiverFlowPlanner.Plan(8, textures, null, 0, [(3, 0), (3, 3)], catalog);
        Assert.True(plan.Succeeded);

        // 匯流點 (3, 3) 應同時連通 North (支流) 以及 East/West (主河)
        var confluence = plan.WaterTiles.Single(t => t.X == 3 && t.Y == 3);
        Assert.True((confluence.Connections & RiverConnections.North) != 0);
        Assert.True((confluence.Connections & (RiverConnections.East | RiverConnections.West)) != 0);
        Assert.Equal(RiverTileKind.TConfluence, confluence.Kind);
    }

    [Fact]
    public void Elevation_detects_uphill_flow_and_emits_warnings_in_ValidateOnly()
    {
        var catalog = NativeCatalog();
        var textures = BlankTextures(8);
        int vertexSize = 9;
        byte[] heights = new byte[vertexSize * vertexSize];

        // 設定起點 (1, 1) 海拔較低，終點 (5, 1) 海拔較高（逆流）
        for (int y = 0; y < vertexSize; y++)
        for (int x = 0; x < vertexSize; x++)
        {
            heights[y * vertexSize + x] = (byte)(10 + x * 5); // x 越大越高
        }

        var options = new RiverPlannerOptions(RiverElevationMode.ValidateOnly, AutoDetectDownhillFlow: false);
        var plan = RiverFlowPlanner.Plan(8, textures, heights, vertexSize, [(1, 1), (5, 1)], catalog, options);

        Assert.True(plan.Succeeded);
        Assert.NotEmpty(plan.Warnings);
        Assert.Empty(plan.HeightAdjustments); // 驗證模式不修改高度
    }

    [Fact]
    public void Elevation_auto_reverses_flow_when_AutoDetectDownhillFlow_enabled()
    {
        var catalog = NativeCatalog();
        var textures = BlankTextures(8);
        int vertexSize = 9;
        byte[] heights = new byte[vertexSize * vertexSize];

        // (1, 1) 較低 (10), (5, 1) 較高 (50)
        for (int y = 0; y < vertexSize; y++)
        for (int x = 0; x < vertexSize; x++)
        {
            heights[y * vertexSize + x] = (byte)(10 + x * 10);
        }

        // 使用者從 (1, 1) 畫到 (5, 1)，但開啟 AutoDetectDownhillFlow
        var options = new RiverPlannerOptions(RiverElevationMode.ValidateOnly, AutoDetectDownhillFlow: true);
        var plan = RiverFlowPlanner.Plan(8, textures, heights, vertexSize, [(1, 1), (5, 1)], catalog, options);

        Assert.True(plan.Succeeded);
        // 自動反向後，起點變為 (5, 1)，終點為 (1, 1)，高處流向低處，無逆流警示！
        Assert.Empty(plan.Warnings);
    }

    [Fact]
    public void Elevation_AutoCarveDescending_enforces_downhill_heights()
    {
        var catalog = NativeCatalog();
        var textures = BlankTextures(8);
        int vertexSize = 9;
        byte[] heights = new byte[vertexSize * vertexSize];

        // 初始高度全為 80，但中間 (3, 1) 出現凸起障礙 (100)
        Array.Fill(heights, (byte)80);
        heights[1 * vertexSize + 3] = 100;
        heights[1 * vertexSize + 4] = 100;

        var options = new RiverPlannerOptions(RiverElevationMode.AutoCarveDescending, MinSlopeDrop: 2);
        var plan = RiverFlowPlanner.Plan(8, textures, heights, vertexSize, [(1, 1), (5, 1)], catalog, options);

        Assert.True(plan.Succeeded);
        Assert.NotEmpty(plan.HeightAdjustments);

        // 凸起障礙必須被下挖削平
        Assert.All(plan.HeightAdjustments, adj => Assert.True(adj.After <= 80));
    }

    [Fact]
    public void Plan_generates_riverbank_transition_tiles()
    {
        var catalog = NativeCatalog();
        var textures = BlankTextures(8);

        // 水平河道 (2, 3) -> (5, 3)
        var options = new RiverPlannerOptions(GenerateBankTransitions: true);
        var plan = RiverFlowPlanner.Plan(8, textures, null, 0, [(2, 3), (5, 3)], catalog, options);

        Assert.True(plan.Succeeded);
        Assert.NotEmpty(plan.BankTiles);

        // 河道北側 (Y=2) 應有北岸圖塊 ErdeFlussR1
        Assert.Contains(plan.BankTiles, b => b.Y == 2 && b.Texture == "ErdeFlussR1" && b.Side == RiverBankSide.North);
        // 河道南側 (Y=4) 應有南岸圖塊 ErdeFlussR3
        Assert.Contains(plan.BankTiles, b => b.Y == 4 && b.Texture == "ErdeFlussR3" && b.Side == RiverBankSide.South);
    }

    [Fact]
    public void TerrainBlendEditSession_PaintRiverPath_supports_single_step_undo()
    {
        var catalog = NativeCatalog();
        var map = new TerrainBlendAuthoringMap(8, "grass");
        var import = new NativeTerrainImportResult(map, [], []);
        var textures = BlankTextures(8);
        var session = new TerrainBlendEditSession(import, textures, new DummyResolver());

        int vertexSize = 9;
        byte[] heights = new byte[vertexSize * vertexSize];
        Array.Fill(heights, (byte)50);
        var heightSession = new TerrainHeightEditSession(vertexSize, heights, null, 0, null);

        // 繪製河流筆畫
        var result = session.PaintRiverPath([(1, 2), (4, 2)], catalog, new RiverPlannerOptions(), heightSession);
        Assert.True(result.Succeeded);
        Assert.NotEmpty(result.TextureChanges);

        // 提交整筆筆畫
        Assert.True(session.CommitStroke());
        heightSession.CommitStroke();

        // 驗證已印章寫入河道
        Assert.Equal("FLUSS1", session.CurrentTextures[2 * 8 + 2]);

        // 執行單步 Undo
        var undone = session.Undo();
        Assert.NotNull(undone);
        heightSession.Undo();

        // 驗證材質復原為 grass
        Assert.Equal("grass", session.CurrentTextures[2 * 8 + 2]);
    }

    [Theory]
    [InlineData(2)]
    [InlineData(4)]
    [InlineData(8)]
    public void Water_channel_uses_world_water_level_and_has_monotonic_local_banks(float step)
    {
        const int dimension = 64, size = 257;
        byte[] original = Enumerable.Repeat((byte)80, size * size).ToArray();
        var options = new RiverPlannerOptions(WaterLevel: 30.5f * step, HeightmapStep: step, MinSlopeDrop: 0);
        var plan = RiverFlowPlanner.Plan(dimension, BlankTextures(dimension), original, size,
            [(10, 10), (15, 10)], NativeCatalog(), options);
        Assert.True(plan.Succeeded);
        byte[] carved = original.ToArray();
        foreach (var a in plan.HeightAdjustments)
        {
            Assert.True(a.After <= a.Before);
            carved[a.Index] = a.After;
        }
        Assert.Equal(original, Enumerable.Repeat((byte)80, size * size).ToArray());
        // Tile (12,10) is centred at vertex (50,42), not (12,10).
        Assert.Equal((byte)14, carved[42 * size + 50]);
        Assert.True(carved[42 * size + 50] * step <= options.WaterLevel - 6 * step);
        for (int side = -1; side <= 1; side += 2)
        {
            byte previous = carved[42 * size + 50];
            for (int offset = 1; offset <= 4; offset++)
            {
                byte current = carved[(42 + side * offset) * size + 50];
                Assert.True(current >= previous);
                previous = current;
            }
            Assert.Equal((byte)80, previous);
            Assert.InRange(carved[(42 + side * 2) * size + 50], (byte)15, (byte)79);
        }
        Assert.All(plan.HeightAdjustments, a =>
        {
            Assert.InRange(a.VertexX, 39, 65);
            Assert.InRange(a.VertexY, 39, 45);
        });
        Assert.Equal((byte)80, carved[10 * size + 12]);
        Assert.Equal((byte)80, carved[100 * size + 100]);
    }

    [Theory]
    [InlineData(0, 0, 5, 0)]
    [InlineData(10, 10, 10, 10)]
    [InlineData(10, 10, 15, 15)]
    [InlineData(63, 60, 63, 63)]
    public void Channel_centres_stay_submerged_at_ends_turns_and_map_edges(int x1, int y1, int x2, int y2)
    {
        const int size = 257;
        byte[] heights = Enumerable.Repeat((byte)80, size * size).ToArray();
        var plan = RiverFlowPlanner.Plan(64, BlankTextures(64), heights, size,
            [(x1, y1), (x2, y2)], NativeCatalog(), new RiverPlannerOptions(WaterLevel: 120));
        Assert.True(plan.Succeeded);
        foreach (var a in plan.HeightAdjustments) heights[a.Index] = a.After;
        foreach (var tile in plan.WaterTiles)
            Assert.True(heights[(tile.Y * 4 + 2) * size + tile.X * 4 + 2] <= 24);
        Assert.Equal(plan.HeightAdjustments.Count, plan.HeightAdjustments.Select(a => a.Index).Distinct().Count());
    }

    [Fact]
    public void Insufficient_water_depth_fails_without_changes_and_validate_only_never_carves()
    {
        byte[] heights = Enumerable.Repeat((byte)80, 33 * 33).ToArray();
        var plan = RiverFlowPlanner.Plan(8, BlankTextures(), heights, 33, [(1, 1), (5, 1)],
            NativeCatalog(), new RiverPlannerOptions(WaterLevel: 20));
        Assert.False(plan.Succeeded);
        Assert.Empty(plan.HeightAdjustments);
        Assert.Empty(plan.WaterTiles);
        var validate = RiverFlowPlanner.Plan(8, BlankTextures(), heights, 33, [(1, 1), (5, 1)],
            NativeCatalog(), new RiverPlannerOptions(RiverElevationMode.ValidateOnly, WaterLevel: 120));
        Assert.True(validate.Succeeded);
        Assert.Empty(validate.HeightAdjustments);
    }
    private sealed class DummyResolver : INativeTerrainMaterialResolver
    {
        public bool TryResolveNativeCorners(string texture, out IReadOnlyList<string> corners)
        {
            corners = ["grass", "grass", "grass", "grass"];
            return true;
        }

        public string? ResolveNativeTile(IReadOnlyList<string> cornerMaterialIds, int x, int y) => "grass";
    }
}
