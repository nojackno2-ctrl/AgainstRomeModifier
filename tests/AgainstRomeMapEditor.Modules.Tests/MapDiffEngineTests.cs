namespace AgainstRomeMapEditor.Modules.Tests;

using AgainstRomeMapEditor.Modules.Diff;
using AgainstRomeModifier.Scripting;
using Xunit;

public sealed class MapDiffEngineTests
{
    [Fact]
    public void MapDiffEngine_IdenticalSnapshots_ReportsNoDifferences()
    {
        byte[] heights = new byte[65 * 65];
        Array.Fill(heights, (byte)100);

        string[] textures = Enumerable.Repeat("GRAS01", 64 * 64).ToArray();
        byte[] collision = new byte[256 * 256];

        var objId = Guid.NewGuid();
        var objects = new[]
        {
            new DiffObjectItem(objId, 1, 1001, "GER_HAU00", 42, 0, 5000, 100, 5000, 45, 0, true)
        };

        var events = new[]
        {
            new ScenarioEvent("StartMessage", 5, Repeat: false, Enabled: true)
            {
                Actions = new() { new ScenarioAction(ScenarioActionKind.Message, Text: "Welcome") }
            }
        };

        var snapshotA = new MapVersionSnapshot(
            "Backup_A", 64, 65, heights, 4.0f, textures, 256, collision, objects, events);
        var snapshotB = new MapVersionSnapshot(
            "Current_B", 64, 65, heights.ToArray(), 4.0f, textures.ToArray(), 256, collision.ToArray(), objects.ToArray(), events.ToArray());

        MapDiffReport report = MapDiffEngine.Compare(snapshotA, snapshotB);

        Assert.False(report.HasDifferences);
        Assert.Equal(0, report.Height.ChangedVertexCount);
        Assert.Equal(0, report.Textures.ChangedTileCount);
        Assert.Equal(0, report.Collision.NewlyBlockedCount);
        Assert.Equal(0, report.Collision.NewlyClearedCount);
        Assert.Equal(0, report.Objects.TotalChanges);
        Assert.Equal(0, report.Events.TotalChanges);
    }

    [Fact]
    public void MapDiffEngine_HeightDifferences_DetectsRaiseLowerAndVolume()
    {
        byte[] baseHeights = new byte[65 * 65];
        byte[] curHeights = new byte[65 * 65];
        Array.Fill(baseHeights, (byte)50);
        Array.Fill(curHeights, (byte)50);

        // 抬升頂點 (10, 10) 由 50 -> 60 (提升 10 階 = 40 世界高度)
        curHeights[10 * 65 + 10] = 60;

        // 下陷頂點 (20, 20) 由 50 -> 45 (降低 5 階 = -20 世界高度)
        curHeights[20 * 65 + 20] = 45;

        var snapBase = new MapVersionSnapshot("Base", 64, 65, baseHeights, 4.0f);
        var snapCur = new MapVersionSnapshot("Current", 64, 65, curHeights, 4.0f);

        MapDiffReport report = MapDiffEngine.Compare(snapBase, snapCur);

        Assert.True(report.HasDifferences);
        Assert.Equal(2, report.Height.ChangedVertexCount);
        Assert.Equal(40.0f, report.Height.MaxRaise);
        Assert.Equal(20.0f, report.Height.MaxLower);
        Assert.Equal(20.0f, report.Height.VolumeDelta); // +40 - 20 = +20

        float[] delta = report.Height.DeltaGrid;
        Assert.Equal(40.0f, delta[10 * 65 + 10]);
        Assert.Equal(-20.0f, delta[20 * 65 + 20]);
    }

    [Fact]
    public void MapDiffEngine_TextureDifferences_DetectsChangedTilesAndTransitions()
    {
        string[] baseTex = Enumerable.Repeat("GRAS01", 64 * 64).ToArray();
        string[] curTex = Enumerable.Repeat("GRAS01", 64 * 64).ToArray();

        // 鋪設土路
        curTex[5 * 64 + 10] = "PFAD01";
        curTex[5 * 64 + 11] = "PFAD01";
        curTex[6 * 64 + 10] = "H_WEG1";

        var snapBase = new MapVersionSnapshot("Base", 64, 65, null, 4.0f, baseTex);
        var snapCur = new MapVersionSnapshot("Current", 64, 65, null, 4.0f, curTex);

        MapDiffReport report = MapDiffEngine.Compare(snapBase, snapCur);

        Assert.True(report.HasDifferences);
        Assert.Equal(3, report.Textures.ChangedTileCount);
        Assert.Equal(2, report.Textures.Transitions[(From: "GRAS01", To: "PFAD01")]);
        Assert.Equal(1, report.Textures.Transitions[(From: "GRAS01", To: "H_WEG1")]);
    }

    [Fact]
    public void MapDiffEngine_CollisionDifferences_DetectsBlockedAndCleared()
    {
        byte[] baseCol = new byte[256 * 256];
        byte[] curCol = new byte[256 * 256];

        // 新增障礙
        baseCol[10 * 256 + 10] = 0;
        curCol[10 * 256 + 10] = 255;

        // 清除通行
        baseCol[50 * 256 + 50] = 255;
        curCol[50 * 256 + 50] = 0;

        var snapBase = new MapVersionSnapshot("Base", 64, 65, null, 4f, null, 256, baseCol);
        var snapCur = new MapVersionSnapshot("Current", 64, 65, null, 4f, null, 256, curCol);

        MapDiffReport report = MapDiffEngine.Compare(snapBase, snapCur);

        Assert.True(report.HasDifferences);
        Assert.Equal(1, report.Collision.NewlyBlockedCount);
        Assert.Equal(1, report.Collision.NewlyClearedCount);
    }

    [Fact]
    public void MapDiffEngine_ObjectDifferences_DetectsAddedDeletedMovedModified()
    {
        var idKeep = Guid.NewGuid();
        var idDelete = Guid.NewGuid();
        var idAdd = Guid.NewGuid();
        var idMove = Guid.NewGuid();

        var baseObjs = new List<DiffObjectItem>
        {
            new(idKeep, 1, 100, "GER_HAU00", 1, 0, 4000, 0, 4000, 0, 0, true),
            new(idDelete, 2, 200, "LanGerNabu01", 10, 8, 3000, 0, 3000, 0, 0, false),
            new(idMove, 3, 300, "ROM_LEG01", 5, 1, 6000, 0, 6000, 90, 10, false)
        };

        var curObjs = new List<DiffObjectItem>
        {
            new(idKeep, 1, 100, "GER_HAU00", 1, 0, 4000, 0, 4000, 0, 0, true),
            new(idAdd, 4, 400, "ROM_BALLISTA", 8, 1, 8000, 0, 8000, 0, 0, false),
            new(idMove, 3, 300, "ROM_LEG01", 5, 1, 6100, 0, 6100, 180, 15, false) // 移動且改角度/人數
        };

        var snapBase = new MapVersionSnapshot("Base", 64, 65, null, 4f, null, 0, null, baseObjs);
        var snapCur = new MapVersionSnapshot("Current", 64, 65, null, 4f, null, 0, null, curObjs);

        MapDiffReport report = MapDiffEngine.Compare(snapBase, snapCur);

        Assert.True(report.HasDifferences);
        Assert.Single(report.Objects.Added);
        Assert.Equal(idAdd, report.Objects.Added[0].Id);

        Assert.Single(report.Objects.Deleted);
        Assert.Equal(idDelete, report.Objects.Deleted[0].Id);

        Assert.Single(report.Objects.Modified);
        var mod = report.Objects.Modified[0];
        Assert.Equal(idMove, mod.Current.Id);
        Assert.True(mod.DistanceMoved > 100f);
        Assert.Contains(mod.ChangedAttributes, a => a.Contains("坐標移動"));
        Assert.Contains(mod.ChangedAttributes, a => a.Contains("旋轉角度"));
        Assert.Contains(mod.ChangedAttributes, a => a.Contains("部隊人數"));
    }

    [Fact]
    public void MapDiffEngine_EventDifferences_DetectsEventChanges()
    {
        var baseEvents = new List<ScenarioEvent>
        {
            new("Intro", 5, Repeat: false, Enabled: true),
            new("Wave1", 60, Repeat: false, Enabled: true)
        };

        var curEvents = new List<ScenarioEvent>
        {
            new("Intro", 10, Repeat: false, Enabled: true), // 延遲改為 10
            new("BossFight", 120, Repeat: false, Enabled: true) // 新增 BossFight，刪除 Wave1
        };

        var snapBase = new MapVersionSnapshot("Base", 64, 65, null, 4f, null, 0, null, null, baseEvents);
        var snapCur = new MapVersionSnapshot("Current", 64, 65, null, 4f, null, 0, null, null, curEvents);

        MapDiffReport report = MapDiffEngine.Compare(snapBase, snapCur);

        Assert.True(report.HasDifferences);
        Assert.Single(report.Events.Added);
        Assert.Equal("BossFight", report.Events.Added[0].Name);

        Assert.Single(report.Events.Deleted);
        Assert.Equal("Wave1", report.Events.Deleted[0].Name);

        Assert.Single(report.Events.Modified);
        Assert.Equal("Intro", report.Events.Modified[0].EventName);
    }

    [Fact]
    public void MapVisualDiffOverlay_GeneratesHeatmapAndMinimapBuffer()
    {
        byte[] baseHeights = new byte[65 * 65];
        byte[] curHeights = new byte[65 * 65];
        curHeights[10 * 65 + 10] = 50;

        string[] baseTex = Enumerable.Repeat("GRAS01", 64 * 64).ToArray();
        string[] curTex = Enumerable.Repeat("GRAS01", 64 * 64).ToArray();
        curTex[5 * 64 + 5] = "PFAD01";

        var snapBase = new MapVersionSnapshot("Base", 64, 65, baseHeights, 4f, baseTex);
        var snapCur = new MapVersionSnapshot("Current", 64, 65, curHeights, 4f, curTex);

        MapDiffReport report = MapDiffEngine.Compare(snapBase, snapCur);

        // 產出 256x256 遮罩
        HeatmapMaskBuffer canvasMask = MapVisualDiffOverlay.GenerateHeatmapMask(report, 256, 256);
        Assert.Equal(256, canvasMask.Width);
        Assert.Equal(256, canvasMask.Height);
        Assert.True(canvasMask.Pixels.Any(p => p != 0));

        // 產出小地圖遮罩
        HeatmapMaskBuffer minimapMask = MapVisualDiffOverlay.GenerateMinimapOverlay(report, 256, 256);
        Assert.Equal(256, minimapMask.Width);
        Assert.True(minimapMask.Pixels.Any(p => p != 0));

        // 產出向量標記
        var markers = MapVisualDiffOverlay.GenerateVectorMarkers(report);
        Assert.NotNull(markers);
    }

    [Fact]
    public void SelectiveRollback_RegionHeightRollbackWithEdgeBlend()
    {
        int dim = 65;
        byte[] baseHeights = new byte[dim * dim];
        byte[] curHeights = new byte[dim * dim];

        // 基準為 10，當前修改為 100
        Array.Fill(baseHeights, (byte)10);
        Array.Fill(curHeights, (byte)100);

        var snapBase = new MapVersionSnapshot("Base", 64, dim, baseHeights, 4f);
        var snapCur = new MapVersionSnapshot("Current", 64, dim, curHeights, 4f);

        MapDiffReport diff = MapDiffEngine.Compare(snapBase, snapCur);

        // 框選矩形區域 (X: 2000..6000, Z: 2000..6000)
        var scope = new RollbackRegionScope(
            2000, 2000, 6000, 6000,
            Layers: RollbackLayerFlags.Height,
            EnableEdgeBlend: true,
            EdgeBlendRadius: 2
        );

        RollbackTransaction tx = SelectiveRollbackController.ComputeRollback(snapBase, snapCur, diff, scope);

        Assert.NotEmpty(tx.HeightChanges);
        Assert.Empty(tx.TextureChanges);
        Assert.Empty(tx.ObjectsToRestore);

        // 驗證內部中心頂點完全被還原為 10
        float worldStep = 16384f / (dim - 1);
        int centerVx = (int)MathF.Round(4000f / worldStep);
        int centerVz = (int)MathF.Round(4000f / worldStep);
        int centerIdx = centerVz * dim + centerVx;

        DiffTerrainSampleChange? centerChange = tx.HeightChanges.FirstOrDefault(c => c.Index == centerIdx);
        Assert.NotNull(centerChange);
        Assert.Equal((byte)100, centerChange.Value.Before);
        Assert.Equal((byte)10, centerChange.Value.After);

        // 驗證選區外的頂點未被變更
        int outsideVx = (int)MathF.Round(10000f / worldStep);
        int outsideVz = (int)MathF.Round(10000f / worldStep);
        int outsideIdx = outsideVz * dim + outsideVx;

        Assert.DoesNotContain(tx.HeightChanges, c => c.Index == outsideIdx);
    }

    [Fact]
    public void SelectiveRollback_TextureAndObjectRollback_RespectsScopeAndFilters()
    {
        string[] baseTex = Enumerable.Repeat("GRAS01", 64 * 64).ToArray();
        string[] curTex = Enumerable.Repeat("GRAS01", 64 * 64).ToArray();

        // 區域內材質變更 (Tile 10, 10 -> 世界坐標約 2560, 2560)
        curTex[10 * 64 + 10] = "PFAD01";

        // 區域外材質變更 (Tile 50, 50 -> 世界坐標約 12800, 12800)
        curTex[50 * 64 + 50] = "PFAD01";

        var idInside = Guid.NewGuid();
        var idOutside = Guid.NewGuid();

        var baseObjs = new List<DiffObjectItem>
        {
            new(idInside, 1, 101, "TREE", 1, 8, 2500, 0, 2500, 0, 0, false),
            new(idOutside, 2, 102, "TREE", 1, 8, 12000, 0, 12000, 0, 0, false)
        };

        var curObjs = new List<DiffObjectItem>(); // 全部刪除

        var snapBase = new MapVersionSnapshot("Base", 64, 65, null, 4f, baseTex, 0, null, baseObjs);
        var snapCur = new MapVersionSnapshot("Current", 64, 65, null, 4f, curTex, 0, null, curObjs);

        MapDiffReport diff = MapDiffEngine.Compare(snapBase, snapCur);

        // 只框選區域 (X: 1000..4000, Z: 1000..4000)
        var scope = new RollbackRegionScope(
            1000, 1000, 4000, 4000,
            Layers: RollbackLayerFlags.Textures | RollbackLayerFlags.Objects
        );

        RollbackTransaction tx = SelectiveRollbackController.ComputeRollback(snapBase, snapCur, diff, scope);

        // 材質：只有區域內的 (10, 10) 被還原
        Assert.Single(tx.TextureChanges);
        Assert.Equal(10, tx.TextureChanges[0].TileX);
        Assert.Equal(10, tx.TextureChanges[0].TileY);
        Assert.Equal("PFAD01", tx.TextureChanges[0].OldTexture);
        Assert.Equal("GRAS01", tx.TextureChanges[0].NewTexture);

        // 物件：只有區域內的 idInside 被復原
        Assert.Single(tx.ObjectsToRestore);
        Assert.Equal(idInside, tx.ObjectsToRestore[0].Id);
    }
}
