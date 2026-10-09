using System.Numerics;
using AgainstRomeMapEditor.Modules.Profiling;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class MapBudgetProfilerGuardTests
{
    [Fact]
    public void MapBudgetProfiler_Analyze_ThrowsOnNullSnapshot()
    {
        Assert.Throws<ArgumentNullException>(() => MapBudgetProfiler.Analyze(null!));
    }

    [Fact]
    public void MapBudgetProfiler_Limits_VerifyEngineInvariants()
    {
        Assert.Equal(14000, MapBudgetLimits.MaxObjectsDatSlots);
        Assert.Equal(10000, MapBudgetLimits.RedlineObjectsDatSlots);
        Assert.Equal(8000, MapBudgetLimits.RecommendedObjectsDatSlots);

        Assert.Equal(33000, MapBudgetLimits.MaxPositionDatRecords);
        Assert.Equal(24000, MapBudgetLimits.RedlinePositionDatRecords);
        Assert.Equal(1000, MapBudgetLimits.RedlineScenarioSpawns);

        Assert.Equal(1024, MapBudgetLimits.MaxGlobalRuntimeLights);
        Assert.Equal(64, MapBudgetLimits.HardLimitLightsPerCluster);
        Assert.Equal(48, MapBudgetLimits.RedlineLightsPerCluster);
        Assert.Equal(32, MapBudgetLimits.RecommendedLightsPerCluster);

        Assert.True(MapBudgetLimits.RecommendedObjectsDatSlots < MapBudgetLimits.RedlineObjectsDatSlots);
        Assert.True(MapBudgetLimits.RedlineObjectsDatSlots < MapBudgetLimits.MaxObjectsDatSlots);
        Assert.True(MapBudgetLimits.RecommendedLightsPerCluster < MapBudgetLimits.RedlineLightsPerCluster);
        Assert.True(MapBudgetLimits.RedlineLightsPerCluster < MapBudgetLimits.HardLimitLightsPerCluster);
    }

    [Fact]
    public void MapBudgetProfiler_SpawnsClassificationAndRedline()
    {
        var spawns = new List<ScenarioSpawn>
        {
            new("LanTree01", 100, 100, 0, 1),
            new("BauHouse01", 200, 200, 0, 1),
            new("RomInf01", 300, 300, 0, 10),
            new("FXSmoke01", 400, 400, 0, 1),
            new("UnknownBox", 500, 500, 0, 1)
        };

        var snapshot = new MapBudgetSnapshot(Spawns: spawns);
        var report = MapBudgetProfiler.Analyze(snapshot);

        Assert.Equal(5, report.ObjectBudget.TotalActiveObjects);
        Assert.Equal(1, report.ObjectBudget.LandscapeCount);
        Assert.Equal(1, report.ObjectBudget.BuildingCount);
        Assert.Equal(1, report.ObjectBudget.UnitCount);
        Assert.Equal(1, report.ObjectBudget.FxCount);
        Assert.Equal(1, report.ObjectBudget.OtherCount);

        // 測試生成物件超過警戒線 (RedlineScenarioSpawns = 1000)
        var excessiveSpawns = new List<ScenarioSpawn>();
        for (int i = 0; i < 1050; i++)
        {
            excessiveSpawns.Add(new ScenarioSpawn($"RomInf{i:D2}", i * 10, i * 10, 0, 1));
        }

        var overflowSnapshot = new MapBudgetSnapshot(Spawns: excessiveSpawns);
        var overflowReport = MapBudgetProfiler.Analyze(overflowSnapshot);
        Assert.Contains(overflowReport.Bottlenecks, b => b.Code == "spawns-redline");
    }

    [Fact]
    public void MapBudgetProfiler_PositionDatRecordsOverflow_TriggersEngineLimitViolation()
    {
        // MaxPositionDatRecords = 33,000。每個物件預估 2 筆位置記錄，17000 個物件需要 34000 筆記錄
        var objects = new List<MapSceneObject>();
        for (int i = 0; i < 17000; i++)
        {
            objects.Add(new MapSceneObject($"LanTree{i}", i % 100, 0, i / 100, 0, "test.sdl", 0));
        }

        var snapshot = new MapBudgetSnapshot(SceneObjects: objects);
        var report = MapBudgetProfiler.Analyze(snapshot);

        Assert.Equal(MapHealthGrade.EngineExceeded, report.OverallGrade);
        Assert.Contains(report.Bottlenecks, b => b.Code == "position-dat-overflow" && b.IsEngineLimitViolation);
    }

    [Fact]
    public void MapBudgetProfiler_InvalidOrMismatchedCollision_HandledGracefully()
    {
        // 1. Collision 為 null
        var snapshotNull = new MapBudgetSnapshot(CollisionSize: 16, Collision: null);
        var reportNull = MapBudgetProfiler.Analyze(snapshotNull);
        Assert.Equal("Unknown", reportNull.PathBudget.ComplexityRating);
        Assert.Equal(0f, reportNull.PathBudget.ObstacleDensityPercent);

        // 2. Collision 長度與 Size 不匹配
        var snapshotMismatch = new MapBudgetSnapshot(CollisionSize: 16, Collision: new byte[10]);
        var reportMismatch = MapBudgetProfiler.Analyze(snapshotMismatch);
        Assert.Equal("Unknown", reportMismatch.PathBudget.ComplexityRating);
    }

    [Fact]
    public void MapBudgetProfiler_SubmergedHeights_MarkedAsBlockedObstacles()
    {
        int size = 16;
        int totalCells = size * size;
        byte[] collision = new byte[totalCells]; // 全部為 0 (可通行)

        // 高度圖 17x17，設定水位為 100f
        int heightSize = 17;
        byte[] heights = new byte[heightSize * heightSize];
        // 高度設為 10，HeightStep = 4.0f -> 實際高度 40f < 水位 100f (全部沒入水中)
        Array.Fill(heights, (byte)10);

        var snapshot = new MapBudgetSnapshot(
            CollisionSize: size,
            Collision: collision,
            HeightSize: heightSize,
            Heights: heights,
            HeightStep: 4.0f,
            WaterLevel: 100.0f);

        var report = MapBudgetProfiler.Analyze(snapshot);

        // 應將沒入水中的格子判定為障礙
        Assert.True(report.PathBudget.ObstacleDensityPercent > 90f);
    }

    [Fact]
    public void MapBudgetProfiler_OverallGrade_CautionAndCriticalThresholds()
    {
        // 1. Caution 評級：物件數介於 Recommended (8000) 與 Redline (10000) 之間，且無 draw call 懲罰
        var objects = new List<MapSceneObject>();
        for (int i = 0; i < 8100; i++)
        {
            objects.Add(new MapSceneObject("LanTree", 100, 0, 100, 0, "test.sdl", 0));
        }

        var cautionSnapshot = new MapBudgetSnapshot(
            SceneObjects: objects,
            DecodedSpriteCount: 100,
            DecodedSpriteTypes: 50);
        var cautionReport = MapBudgetProfiler.Analyze(cautionSnapshot);
        Assert.Equal(MapHealthGrade.Caution, cautionReport.OverallGrade);

        // 2. Critical 評級：局部密集光源達到 60 (> 58 門檻)
        var lights = new List<NativeLightInstance>();
        for (int i = 0; i < 60; i++)
        {
            lights.Add(new NativeLightInstance(
                new Vector3(1000f, 0, 1000f),
                new NativeLightDefinition { Index = 1, IsActive = true, Radius = 300f }));
        }

        var criticalSnapshot = new MapBudgetSnapshot(Lights: lights);
        var criticalReport = MapBudgetProfiler.Analyze(criticalSnapshot);
        Assert.Equal(MapHealthGrade.Critical, criticalReport.OverallGrade);
    }
}
