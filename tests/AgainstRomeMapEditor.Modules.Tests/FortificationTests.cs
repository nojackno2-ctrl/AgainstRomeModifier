using AgainstRomeMapEditor.Modules.Fortification;
using AgainstRomeMapEditor.Modules.Placement;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class FortificationTests
{
    [Fact]
    public void WallTileCatalog_Resolves_Roman_and_Germanic_Straight_And_Corner_Components()
    {
        var catalog = new WallTileCatalog();

        // 羅馬東西直牆
        var (romanStraightEW, angleEW) = catalog.ResolveComponent(
            FortificationStyle.RomanStoneWall,
            WallConnections.East | WallConnections.West,
            preferTowerOnCorner: false);
        Assert.Equal(WallComponentKind.Straight, romanStraightEW.Kind);
        Assert.Equal(0f, angleEW);
        Assert.Equal("BauRomMau00_Mauer", romanStraightEW.NameDef);

        // 羅馬南北直牆
        var (romanStraightNS, angleNS) = catalog.ResolveComponent(
            FortificationStyle.RomanStoneWall,
            WallConnections.North | WallConnections.South,
            preferTowerOnCorner: false);
        Assert.Equal(WallComponentKind.Straight, romanStraightNS.Kind);
        Assert.Equal(90f, angleNS);

        // 羅馬轉角（關閉自動塔樓）
        var (corner, cornerAngle) = catalog.ResolveComponent(
            FortificationStyle.RomanStoneWall,
            WallConnections.North | WallConnections.East,
            preferTowerOnCorner: false);
        Assert.Equal(WallComponentKind.Corner, corner.Kind);
        Assert.Equal(0f, cornerAngle);
        Assert.Equal("BauRomMau02_Mauerecke", corner.NameDef);

        // 轉角（啟用自動塔樓）：升級為防禦塔
        var (cornerTower, _) = catalog.ResolveComponent(
            FortificationStyle.RomanStoneWall,
            WallConnections.North | WallConnections.East,
            preferTowerOnCorner: true);
        Assert.Equal(WallComponentKind.Tower, cornerTower.Kind);
        Assert.Equal("BauRomTur00_Turm", cornerTower.NameDef);
    }

    [Fact]
    public void WallTileCatalog_Resolves_Gate_Orientation_Correctly()
    {
        var catalog = new WallTileCatalog();

        // 城牆為東西走向，門戶向南北開放
        var (gateEW, angleEW) = catalog.ResolveGate(
            FortificationStyle.RomanStoneWall,
            WallConnections.East | WallConnections.West);
        Assert.Equal(WallComponentKind.Gate, gateEW.Kind);
        Assert.Equal(0f, angleEW);

        // 城牆為南北走向，門戶向東西開放
        var (gateNS, angleNS) = catalog.ResolveGate(
            FortificationStyle.RomanStoneWall,
            WallConnections.North | WallConnections.South);
        Assert.Equal(WallComponentKind.Gate, gateNS.Kind);
        Assert.Equal(90f, angleNS);
    }

    [Fact]
    public void WallStrokePlanner_Plans_Orthogonal_L_Turn_With_Corner_Tower_And_EndCaps()
    {
        var catalog = new WallTileCatalog();
        var options = new WallStrokeOptions
        {
            Style = FortificationStyle.RomanStoneWall,
            AutoCornerTowers = true,
            SmoothEndCaps = true
        };

        // L 形路徑：(5, 5) -> (5, 8) -> (8, 8)
        var rawPath = new List<(int X, int Z)> { (5, 5), (5, 8), (8, 8) };

        var plan = WallStrokePlanner.Plan(
            mapDimension: 64,
            rawPath: rawPath,
            options: options,
            catalog: catalog);

        Assert.True(plan.Succeeded);
        Assert.NotEmpty(plan.Placements);

        // 轉角點 (5, 8) 必須被自動晉升為防禦塔
        var corner = plan.Placements.FirstOrDefault(p => p.TileX == 5 && p.TileZ == 8);
        Assert.NotNull(corner);
        Assert.Equal(WallComponentKind.Tower, corner.Kind);

        // 起點 (5, 5) 與終點 (8, 8) 必須為 EndCap 收尾
        var startEnd = plan.Placements.FirstOrDefault(p => p.TileX == 5 && p.TileZ == 5);
        Assert.NotNull(startEnd);
        Assert.Equal(WallComponentKind.EndCap, startEnd.Kind);

        var endEnd = plan.Placements.FirstOrDefault(p => p.TileX == 8 && p.TileZ == 8);
        Assert.NotNull(endEnd);
        Assert.Equal(WallComponentKind.EndCap, endEnd.Kind);
    }

    [Fact]
    public void WallStrokePlanner_Auto_Inserts_Gate_At_Road_Crossing()
    {
        var catalog = new WallTileCatalog();
        var options = new WallStrokeOptions
        {
            Style = FortificationStyle.RomanStoneWall,
            AutoCornerTowers = false,
            AutoGateOnRoadCrossing = true,
            DefaultGateState = GatePassabilityState.Open
        };

        // 直線牆體通過道路格 (10, 5)
        var path = new List<(int X, int Z)> { (10, 3), (10, 7) };

        var plan = WallStrokePlanner.Plan(
            mapDimension: 64,
            rawPath: path,
            options: options,
            catalog: catalog,
            isRoadAtTile: (x, z) => x == 10 && z == 5);

        Assert.True(plan.Succeeded);

        // 檢查 (10, 5) 是否被自動判定為城門
        var gatePlacement = plan.Placements.FirstOrDefault(p => p.TileX == 10 && p.TileZ == 5);
        Assert.NotNull(gatePlacement);
        Assert.Equal(WallComponentKind.Gate, gatePlacement.Kind);
        Assert.Equal(GatePassabilityState.Open, gatePlacement.GateState);
        Assert.Contains((10, 5), plan.GatePassageTiles);
    }

    [Fact]
    public void FortificationPassabilitySync_Synchronizes_Wall_And_Gate_Passability()
    {
        int mapDim = 64;
        int collDim = 256; // 4x4 pixels per tile
        byte[] collisionGrid = new byte[collDim * collDim]; // 預設全部為 0 (可通行)

        var catalog = new WallTileCatalog();
        var options = new WallStrokeOptions
        {
            Style = FortificationStyle.RomanStoneWall,
            AutoCornerTowers = false,
            AutoGateOnRoadCrossing = true,
            DefaultGateState = GatePassabilityState.Open
        };

        // 放置一小段牆：(10, 10) 直牆，(10, 11) 城門（開放）
        var path = new List<(int X, int Z)> { (10, 10), (10, 11) };

        var plan = WallStrokePlanner.Plan(
            mapDimension: mapDim,
            rawPath: path,
            options: options,
            catalog: catalog,
            isRoadAtTile: (x, z) => x == 10 && z == 11);

        var changes = FortificationPassabilitySync.SynchronizePassability(
            collisionGrid, collDim, mapDim, plan);

        Assert.NotEmpty(changes);

        // 驗證 (10, 10) 直牆區域：中心 4x4 像素全為 255
        int scale = 4;
        for (int dz = 0; dz < scale; dz++)
        {
            for (int dx = 0; dx < scale; dx++)
            {
                int px = 10 * scale + dx;
                int pz = 10 * scale + dz;
                Assert.Equal(FortificationPassabilitySync.CollisionBlocked, collisionGrid[pz * collDim + px]);
            }
        }

        // 驗證 (10, 11) 開放城門：中央通道 (dx=1..2, dz=1..2) 必須為 0 (可通行)
        int gatePxCenter = 10 * scale + 1;
        int gatePzCenter = 11 * scale + 1;
        Assert.Equal(FortificationPassabilitySync.CollisionPassable, collisionGrid[gatePzCenter * collDim + gatePxCenter]);

        // 動態切換城門為 Closed：中央通道變為 255
        var gate = plan.Placements.Single(p => p.Kind == WallComponentKind.Gate);
        var toggleChanges = FortificationPassabilitySync.SetGateState(
            collisionGrid, collDim, mapDim, gate, GatePassabilityState.Closed);

        Assert.NotEmpty(toggleChanges);
        Assert.Equal(FortificationPassabilitySync.CollisionBlocked, collisionGrid[gatePzCenter * collDim + gatePxCenter]);
    }

    [Fact]
    public void FortificationPassabilitySync_Analyzes_Fortress_Enclosure()
    {
        int mapDim = 16;
        var placements = new List<FortificationPlacement>();

        // 建造 5x5 的矩形封閉城牆：(2,2) 到 (6,6)
        for (int x = 2; x <= 6; x++)
        {
            placements.Add(CreateMockPlacement(x, 2, WallComponentKind.Straight));
            placements.Add(CreateMockPlacement(x, 6, WallComponentKind.Straight));
        }
        for (int z = 3; z <= 5; z++)
        {
            placements.Add(CreateMockPlacement(2, z, WallComponentKind.Straight));
            placements.Add(CreateMockPlacement(6, z, WallComponentKind.Straight));
        }

        var report = FortificationPassabilitySync.AnalyzeFortressEnclosure(mapDim, placements);

        Assert.True(report.IsFullyEnclosed);
        // 內部受保護區域為 3x3 = 9 格
        Assert.Equal(9, report.EnclosedAreaTiles);
    }

    [Fact]
    public void FortificationEditSessionCoordinator_Executes_And_Undoes_Composite_Transaction()
    {
        var coordinator = new FortificationEditSessionCoordinator();
        var placementSession = new PlacementEditSession();
        int mapDim = 64;
        int collDim = 256;
        byte[] collisionGrid = new byte[collDim * collDim];
        var textures = new Dictionary<(int, int), string>();

        var catalog = new WallTileCatalog();
        var options = new WallStrokeOptions
        {
            Style = FortificationStyle.RomanStoneWall,
            StampFoundations = true
        };

        var path = new List<(int X, int Z)> { (4, 4), (4, 6) };
        var plan = WallStrokePlanner.Plan(mapDim, path, options, catalog);

        // 提交筆畫
        var tx = coordinator.CommitStroke(
            plan,
            placementSession,
            collisionGrid,
            collDim,
            mapDim,
            getTextureAtTile: (x, z) => textures.GetValueOrDefault((x, z), "Gras1"),
            setTextureAtTile: (x, z, tex) => textures[(x, z)] = tex);

        Assert.True(coordinator.CanUndo);
        Assert.Equal(plan.Placements.Count, placementSession.Count);
        Assert.NotEmpty(tx.CollisionChanges);
        Assert.NotEmpty(tx.TextureChanges);

        // 執行撤銷 (Undo)
        bool undone = coordinator.Undo(
            placementSession,
            collisionGrid,
            setTextureAtTile: (x, z, tex) => textures[(x, z)] = tex);

        Assert.True(undone);
        Assert.Equal(0, placementSession.Count);
        Assert.True(coordinator.CanRedo);

        // 驗證 collision 網格已全部還原為 0
        Assert.All(tx.CollisionChanges, c => Assert.Equal(0, collisionGrid[c.Index]));

        // 執行重做 (Redo)
        bool redone = coordinator.Redo(
            placementSession,
            collisionGrid,
            setTextureAtTile: (x, z, tex) => textures[(x, z)] = tex);

        Assert.True(redone);
        Assert.Equal(plan.Placements.Count, placementSession.Count);
    }

    private static FortificationPlacement CreateMockPlacement(int x, int z, WallComponentKind kind)
    {
        return new FortificationPlacement(
            Id: Guid.NewGuid(),
            NameDef: "MockWall",
            Kind: kind,
            TileX: x,
            TileZ: z,
            WorldX: x * 256f,
            WorldY: 0f,
            WorldZ: z * 256f,
            AngleDeg: 0f);
    }
}
