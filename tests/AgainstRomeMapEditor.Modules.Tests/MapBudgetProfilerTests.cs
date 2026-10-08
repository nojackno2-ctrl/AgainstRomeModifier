using System.Numerics;
using AgainstRomeMapEditor.Modules.Profiling;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class MapBudgetProfilerTests
{
    private static MapSceneObject SceneObj(string name, float x, float z) =>
        new(name, x, 0, z, 0, "test.sdl", 0);

    private static NativeLightInstance Light(float x, float z, float radius = 350f) =>
        new(new Vector3(x, 0, z), new NativeLightDefinition { Index = 1, IsActive = true, Radius = radius, Color = Vector3.One });

    [Fact]
    public void Empty_map_returns_healthy_grade_and_perfect_score()
    {
        var snapshot = new MapBudgetSnapshot();
        var report = MapBudgetProfiler.Analyze(snapshot);

        Assert.Equal(MapHealthGrade.Healthy, report.OverallGrade);
        Assert.Equal(100, report.OverallScore);
        Assert.Equal(0, report.ObjectBudget.TotalActiveObjects);
        Assert.Equal(0, report.LightBudget.TotalSceneLights);
        Assert.Empty(report.Bottlenecks);
    }

    [Fact]
    public void Object_count_exceeding_14000_triggers_engine_exceeded()
    {
        var objects = new List<MapSceneObject>();
        for (int i = 0; i < 14_005; i++)
        {
            objects.Add(SceneObj("LanGerTree01", i % 1000, i / 1000));
        }

        var snapshot = new MapBudgetSnapshot(SceneObjects: objects);
        var report = MapBudgetProfiler.Analyze(snapshot);

        Assert.Equal(MapHealthGrade.EngineExceeded, report.OverallGrade);
        Assert.True(report.ObjectBudget.ObjectsUsagePercent > 100.0f);
        Assert.Contains(report.Bottlenecks, b => b.Code == "objects-dat-overflow" && b.IsEngineLimitViolation);
    }

    [Fact]
    public void Object_count_between_redline_and_limit_triggers_warning()
    {
        var objects = new List<MapSceneObject>();
        for (int i = 0; i < 10_500; i++)
        {
            objects.Add(SceneObj("LanGerTree01", i % 1000, i / 1000));
        }

        var snapshot = new MapBudgetSnapshot(SceneObjects: objects);
        var report = MapBudgetProfiler.Analyze(snapshot);

        Assert.Equal(MapHealthGrade.Warning, report.OverallGrade);
        Assert.Contains(report.Bottlenecks, b => b.Code == "objects-dat-redline" && !b.IsEngineLimitViolation);
    }

    [Fact]
    public void Local_light_cluster_exceeding_64_triggers_engine_exceeded_with_hotspot()
    {
        var lights = new List<NativeLightInstance>();
        // 在 (5000, 5000) 附近密集放置 70 個光源
        for (int i = 0; i < 70; i++)
        {
            float offsetX = (i % 8) * 10f;
            float offsetZ = (i / 8) * 10f;
            lights.Add(Light(5000f + offsetX, 5000f + offsetZ, radius: 350f));
        }

        var snapshot = new MapBudgetSnapshot(Lights: lights);
        var report = MapBudgetProfiler.Analyze(snapshot);

        Assert.Equal(MapHealthGrade.EngineExceeded, report.OverallGrade);
        Assert.True(report.LightBudget.PeakClusterLights >= 70);
        Assert.NotEmpty(report.LightBudget.Hotspots);

        var hotspot = report.LightBudget.Hotspots[0];
        Assert.True(hotspot.ExceedsHardLimit);
        Assert.True(Math.Abs(hotspot.WorldX - 5000f) < 600f);
        Assert.True(Math.Abs(hotspot.WorldZ - 5000f) < 600f);

        Assert.Contains(report.Bottlenecks, b => b.Code == "cluster-lights-overflow" && b.IsEngineLimitViolation);
        Assert.NotNull(report.Bottlenecks.First(b => b.Code == "cluster-lights-overflow").WorldX);
    }

    [Fact]
    public void Global_light_exceeding_1024_triggers_engine_exceeded()
    {
        var lights = new List<NativeLightInstance>();
        // 均勻分散 1050 個光源
        for (int i = 0; i < 1050; i++)
        {
            float x = (i % 32) * 500f + 100f;
            float z = (i / 32) * 500f + 100f;
            lights.Add(Light(x, z, radius: 100f));
        }

        var snapshot = new MapBudgetSnapshot(Lights: lights);
        var report = MapBudgetProfiler.Analyze(snapshot);

        Assert.Equal(MapHealthGrade.EngineExceeded, report.OverallGrade);
        Assert.Contains(report.Bottlenecks, b => b.Code == "global-lights-overflow" && b.IsEngineLimitViolation);
    }

    [Fact]
    public void Draw_call_and_atlas_estimation_with_large_sprite_quads_and_many_types()
    {
        var snapshot = new MapBudgetSnapshot(
            DecodedSpriteCount: 15_000,
            DecodedShadowCount: 3_000,
            DecodedSpriteTypes: 300 // 超過 1 頁 (220)
        );

        var report = MapBudgetProfiler.Analyze(snapshot);

        Assert.True(report.DrawBudget.EstimatedAtlasPages >= 2);
        Assert.True(report.DrawBudget.EstimatedAtlasSwitches > 0);
        Assert.True(report.DrawBudget.SpriteBatchDrawCalls > 1);
        Assert.True(report.DrawBudget.EstimatedTotalDrawCalls > 5);
        Assert.True(report.DrawBudget.BatchFragmentationScore > 0f);
    }

    [Fact]
    public void Pathfinding_chokepoint_detection_detects_corridors_and_scores_complexity()
    {
        // 建立 16x16 網格：左右兩個大房間，中間為寬度 1 的通道 (X=8, Y=8)
        int size = 16;
        byte[] collision = new byte[size * size];

        // 建造一堵中央隔牆，僅在 (X=8, Y=8) 留出 1 格通道
        for (int y = 0; y < size; y++)
        {
            if (y != 8)
            {
                collision[y * size + 8] = 255; // 阻擋
            }
        }

        var snapshot = new MapBudgetSnapshot(CollisionSize: size, Collision: collision);
        var report = MapBudgetProfiler.Analyze(snapshot);

        Assert.True(report.PathBudget.ChokepointCount > 0);
        var chokepoint = report.PathBudget.Chokepoints[0];
        Assert.Equal(8, chokepoint.CellX);
        Assert.Equal(8, chokepoint.CellZ);
        Assert.Equal(1, chokepoint.PassageWidthCells);
        Assert.True(report.PathBudget.PathfindingComplexityScore > 0);
    }

    [Fact]
    public void Dead_end_pockets_and_isolated_islands_contribute_to_path_complexity()
    {
        int size = 16;
        byte[] collision = new byte[size * size];

        // 建立封閉孤島：周圍一圈牆壁，內部有一格可通行
        for (int y = 1; y <= 3; y++)
        {
            for (int x = 1; x <= 3; x++)
            {
                if (x == 2 && y == 2) collision[y * size + x] = 0; // 孤島內部
                else collision[y * size + x] = 255;               // 環形牆壁
            }
        }

        // 建立死胡同：3 側為牆，只有一側開口
        collision[8 * size + 8] = 255;
        collision[8 * size + 10] = 255;
        collision[9 * size + 9] = 255;
        // (9, 8) 是通路，(9, 7) 是開口

        var snapshot = new MapBudgetSnapshot(CollisionSize: size, Collision: collision);
        var report = MapBudgetProfiler.Analyze(snapshot);

        // 孤島會使 PassableRegionsCount 增加
        Assert.True(report.PathBudget.PassableRegionsCount >= 2);
    }

    [Fact]
    public void Endl000_benchmark_profile_matches_known_metrics()
    {
        // 模擬 docs/map-editor-performance.md 中的 ENDL_000 基準：
        // 6,618 DATA 物件、7,231 場景物件、7,143 sprites、2,050 陰影、50 光源、212 sprite types
        var objects = new List<MapSceneObject>();
        for (int i = 0; i < 7_231; i++)
        {
            string name = i < 5000 ? "LanGerTree01" : (i < 6500 ? "BauGerHau00" : "GerInf01");
            objects.Add(SceneObj(name, i % 256 * 64, i / 256 * 64));
        }

        var lights = new List<NativeLightInstance>();
        for (int i = 0; i < 50; i++)
        {
            lights.Add(Light((i % 10) * 1500f + 500f, (i / 10) * 1500f + 500f, radius: 350f));
        }

        var snapshot = new MapBudgetSnapshot(
            SceneObjects: objects,
            Lights: lights,
            DecodedSpriteCount: 7_143,
            DecodedShadowCount: 2_050,
            DecodedSpriteTypes: 212
        );

        var report = MapBudgetProfiler.Analyze(snapshot);

        // ENDL_000 官方地圖應該在安全範圍內（Healthy 或 Caution）
        Assert.True(report.OverallGrade is MapHealthGrade.Healthy or MapHealthGrade.Caution);
        Assert.True(report.OverallScore >= 75);
        Assert.Equal(7_231, report.ObjectBudget.TotalActiveObjects);
        Assert.True(report.ObjectBudget.ObjectsUsagePercent < 60.0f); // 7231 / 14000 = 51.6%
        Assert.Equal(50, report.LightBudget.TotalSceneLights);
        Assert.True(report.LightBudget.PeakClusterLights < 48); // 均勻分佈，無超標熱點
        Assert.Equal(1, report.DrawBudget.EstimatedAtlasPages);  // 212 種類可完全裝入單一 4096 圖集
        Assert.Equal(0, report.DrawBudget.EstimatedAtlasSwitches);
        Assert.DoesNotContain(report.Bottlenecks, b => b.IsEngineLimitViolation);
    }
}
