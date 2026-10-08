using AgainstRomeMapEditor.Modules.Fortification;
using AgainstRomeMapEditor.Modules.Placement;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class FortificationGuardTests
{
    [Fact]
    public void PalisadeRunPlanner_GuardsAndDegenerateInputs()
    {
        Assert.Throws<ArgumentNullException>(() =>
            PalisadeRunPlanner.Plan(null!, "Z", "X", "C"));

        Assert.Empty(PalisadeRunPlanner.Plan([], "Z", "X", "C"));
        Assert.Empty(PalisadeRunPlanner.Plan([(100, 100)], "Z", "X", "C"));
        Assert.Empty(PalisadeRunPlanner.Plan([(100, 100), (100, 100)], "Z", "X", "C"));

        Assert.Equal(64f, PalisadeRunPlanner.Spacing);
    }

    [Fact]
    public void WallTileCatalog_GuardsAgainstNullArguments()
    {
        var catalog = new WallTileCatalog();
        Assert.Throws<ArgumentNullException>(() =>
            catalog.Register(FortificationStyle.RomanStoneWall, null!));

        Assert.Throws<ArgumentNullException>(() =>
            catalog.Filtered(null!));
    }

    [Fact]
    public void WallTileCatalog_FilterRemovesNonExistentAndFallsBack()
    {
        var catalog = new WallTileCatalog();
        // 過濾掉所有元件
        var emptyCatalog = catalog.Filtered(_ => false);
        Assert.Empty(emptyCatalog.GetComponents(FortificationStyle.RomanStoneWall));

        // 過濾只保留特定 straight 元件
        var filtered = catalog.Filtered(name => name == "BauRomMau00_Mauer");
        var components = filtered.GetComponents(FortificationStyle.RomanStoneWall);
        Assert.Single(components);
        Assert.Equal("BauRomMau00_Mauer", components[0].NameDef);

        // 即使要求 Corner 也會優雅回退到 straight
        var (resolved, _) = filtered.ResolveComponent(
            FortificationStyle.RomanStoneWall,
            WallConnections.North | WallConnections.East,
            preferTowerOnCorner: false);
        Assert.Equal("BauRomMau00_Mauer", resolved.NameDef);
    }

    [Fact]
    public void WallTileCatalog_ResolveTJunctionAngles_AllFourDirections()
    {
        var catalog = new WallTileCatalog();

        // 預設羅馬石牆未註冊 TJunction 元件時，degree 3 自動升級為防禦塔 (angle 0)
        var (defaultTower, towerAngle) = catalog.ResolveComponent(
            FortificationStyle.RomanStoneWall,
            WallConnections.East | WallConnections.South | WallConnections.West,
            preferTowerOnCorner: false);
        Assert.Equal(WallComponentKind.Tower, defaultTower.Kind);
        Assert.Equal(0f, towerAngle);

        // 註冊 TJunction 元件後，解析正確的旋轉角度 (0, 90, 180, 270)
        catalog.Register(FortificationStyle.RomanStoneWall, new WallComponentDefinition(
            "BauRomMau_TJunction", WallComponentKind.TJunction, WallConnections.All));

        // 缺西（北-東-南）：0 度
        var (_, angleNoWest) = catalog.ResolveComponent(
            FortificationStyle.RomanStoneWall,
            WallConnections.North | WallConnections.East | WallConnections.South,
            preferTowerOnCorner: false);
        Assert.Equal(0f, angleNoWest);

        // 缺北（東-南-西）：90 度
        var (_, angleNoNorth) = catalog.ResolveComponent(
            FortificationStyle.RomanStoneWall,
            WallConnections.East | WallConnections.South | WallConnections.West,
            preferTowerOnCorner: false);
        Assert.Equal(90f, angleNoNorth);

        // 缺東（南-西-北）：180 度
        var (_, angleNoEast) = catalog.ResolveComponent(
            FortificationStyle.RomanStoneWall,
            WallConnections.South | WallConnections.West | WallConnections.North,
            preferTowerOnCorner: false);
        Assert.Equal(180f, angleNoEast);

        // 缺南（西-北-東）：270 度
        var (_, angleNoSouth) = catalog.ResolveComponent(
            FortificationStyle.RomanStoneWall,
            WallConnections.West | WallConnections.North | WallConnections.East,
            preferTowerOnCorner: false);
        Assert.Equal(270f, angleNoSouth);
    }

    [Fact]
    public void WallStrokePlanner_GuardsAgainstNullArguments()
    {
        var catalog = new WallTileCatalog();
        var options = new WallStrokeOptions();

        Assert.Throws<ArgumentNullException>(() =>
            WallStrokePlanner.Plan(64, null!, options, catalog));

        Assert.Throws<ArgumentNullException>(() =>
            WallStrokePlanner.Plan(64, [(1, 1)], null!, catalog));

        Assert.Throws<ArgumentNullException>(() =>
            WallStrokePlanner.Plan(64, [(1, 1)], options, null!));
    }

    [Fact]
    public void WallStrokePlanner_InvalidDimensionAndEmptyPath()
    {
        var catalog = new WallTileCatalog();
        var options = new WallStrokeOptions();

        var invalidDimPlan = WallStrokePlanner.Plan(0, [(1, 1)], options, catalog);
        Assert.False(invalidDimPlan.Succeeded);
        Assert.Contains("大於 0", invalidDimPlan.FailureReason);

        var emptyPlan = WallStrokePlanner.Plan(64, [], options, catalog);
        Assert.True(emptyPlan.Succeeded);
        Assert.Empty(emptyPlan.Placements);
    }

    [Fact]
    public void WallStrokePlanner_OutOfBoundsAndObstructionGuards()
    {
        var catalog = new WallTileCatalog();
        var options = new WallStrokeOptions();

        // 越界點 (-1, 5)
        var oobPlan = WallStrokePlanner.Plan(64, [(-1, 5), (5, 5)], options, catalog);
        Assert.False(oobPlan.Succeeded);
        Assert.Contains("超出地圖邊界", oobPlan.FailureReason);

        // 障礙物衝突
        var blockedPlan = WallStrokePlanner.Plan(
            64,
            [(10, 10), (10, 12)],
            options,
            catalog,
            isObstructedAtTile: (x, z) => x == 10 && z == 11);
        Assert.False(blockedPlan.Succeeded);
        Assert.Contains("衝突", blockedPlan.FailureReason);
    }

    [Fact]
    public void WallStrokePlanner_ForcedGateAndClosedGateState()
    {
        var catalog = new WallTileCatalog();
        var options = new WallStrokeOptions
        {
            ForcedGateLocations = new HashSet<(int X, int Z)> { (10, 11) },
            DefaultGateState = GatePassabilityState.Closed
        };

        var plan = WallStrokePlanner.Plan(64, [(10, 10), (10, 12)], options, catalog);
        Assert.True(plan.Succeeded);

        var forcedGate = plan.Placements.FirstOrDefault(p => p.TileX == 10 && p.TileZ == 11);
        Assert.NotNull(forcedGate);
        Assert.Equal(WallComponentKind.Gate, forcedGate.Kind);
        Assert.Equal(GatePassabilityState.Closed, forcedGate.GateState);
        Assert.Contains((10, 11), plan.BlockedTiles);
    }

    [Fact]
    public void FortificationPassabilitySync_GuardsAgainstInvalidArguments()
    {
        var plan = new WallStrokePlan(true, [], [], [], [], []);
        byte[] validGrid = new byte[16 * 16];

        Assert.Throws<ArgumentNullException>(() =>
            FortificationPassabilitySync.SynchronizePassability(null!, 16, 4, plan));

        Assert.Throws<ArgumentNullException>(() =>
            FortificationPassabilitySync.SynchronizePassability(validGrid, 16, 4, null!));

        // 尺寸不符
        Assert.Throws<ArgumentException>(() =>
            FortificationPassabilitySync.SynchronizePassability(validGrid, 32, 4, plan));

        // mapDimension <= 0
        Assert.Throws<ArgumentException>(() =>
            FortificationPassabilitySync.SynchronizePassability(validGrid, 16, 0, plan));
    }

    [Fact]
    public void FortificationPassabilitySync_SetGateState_GuardsAndNonGateRejection()
    {
        byte[] grid = new byte[16 * 16];
        var nonGate = new FortificationPlacement(
            Guid.NewGuid(), "Wall", WallComponentKind.Straight, 2, 2, 0, 0, 0, 0);

        Assert.Throws<ArgumentNullException>(() =>
            FortificationPassabilitySync.SetGateState(null!, 16, 4, nonGate, GatePassabilityState.Open));

        Assert.Throws<ArgumentNullException>(() =>
            FortificationPassabilitySync.SetGateState(grid, 16, 4, null!, GatePassabilityState.Open));

        var ex = Assert.Throws<ArgumentException>(() =>
            FortificationPassabilitySync.SetGateState(grid, 16, 4, nonGate, GatePassabilityState.Open));
        Assert.Contains("不是城門", ex.Message);
    }

    [Fact]
    public void FortificationPassabilitySync_AnalyzeFortressEnclosure_InvalidDimensionAndUnenclosed()
    {
        var invalid = FortificationPassabilitySync.AnalyzeFortressEnclosure(0, []);
        Assert.False(invalid.IsFullyEnclosed);
        Assert.Equal(0, invalid.EnclosedAreaTiles);

        // 開放式直牆，四周皆通向地圖外側，未形成閉合
        var openWall = new FortificationPlacement(
            Guid.NewGuid(), "Wall", WallComponentKind.Straight, 5, 5, 0, 0, 0, 0);
        var report = FortificationPassabilitySync.AnalyzeFortressEnclosure(16, [openWall]);
        Assert.False(report.IsFullyEnclosed);
        Assert.Equal(0, report.EnclosedAreaTiles);
    }

    [Fact]
    public void FortificationEditSessionCoordinator_GuardsAndEmptyUndoRedo()
    {
        var coordinator = new FortificationEditSessionCoordinator();
        var session = new PlacementEditSession();
        var plan = new WallStrokePlan(true, [], [], [], [], []);

        Assert.Throws<ArgumentNullException>(() =>
            coordinator.CommitStroke(null!, session, null, 0, 0));

        Assert.Throws<ArgumentNullException>(() =>
            coordinator.CommitStroke(plan, null!, null, 0, 0));

        // 失敗的計劃提交應為空事務
        var failedPlan = new WallStrokePlan(false, [], [], [], [], [], "Failed");
        var tx = coordinator.CommitStroke(failedPlan, session, null, 0, 0);
        Assert.Empty(tx.Placements);
        Assert.False(coordinator.CanUndo);

        // 空棧 Undo / Redo
        Assert.False(coordinator.Undo(session, null));
        Assert.False(coordinator.Redo(session, null));
    }
}
