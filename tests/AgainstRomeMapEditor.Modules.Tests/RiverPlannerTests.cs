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
