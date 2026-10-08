using System.Runtime.CompilerServices;
using AgainstRomeModifier.Maps;
using AgainstRomeMapEditor.Modules.Nature;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class FloraScatterEngineTests
{
    private static LevelObjectTemplate CreateMockTemplate()
        => (LevelObjectTemplate)RuntimeHelpers.GetUninitializedObject(typeof(LevelObjectTemplate));

    private static (IReadOnlyDictionary<int, LevelObjectTemplate> Templates, IReadOnlyDictionary<int, string> ObjDefNames) CreateTestCatalog()
    {
        var templates = new Dictionary<int, LevelObjectTemplate>
        {
            [1] = CreateMockTemplate(),
            [2] = CreateMockTemplate(),
            [3] = CreateMockTemplate(),
            [4] = CreateMockTemplate(),
            [5] = CreateMockTemplate(),
        };

        var names = new Dictionary<int, string>
        {
            [1] = "LanGerNad00_Tanne_gross",
            [2] = "LanGerBus01_Busch",
            [3] = "LanGerGra01_Gras",
            [4] = "LanGerSch01_Schilf",
            [5] = "LanGerSte01_Stein",
        };

        return (templates, names);
    }

    [Fact]
    public void Deterministic_generation_produces_identical_results_for_same_seed()
    {
        var (templates, names) = CreateTestCatalog();
        var context = new FloraScatterContext
        {
            Dimension = 64,
            TileWorldSize = 64.0f,
            Profile = BiomeEcologyProfile.GermanicForest,
            Templates = templates,
            ObjDefNames = names,
        };

        var parameters = new FloraScatterParameters
        {
            Seed = 4242,
            MinTileX = 10,
            MinTileY = 10,
            MaxTileX = 30,
            MaxTileY = 30,
            DensityMultiplier = 1.0f,
        };

        var result1 = FloraScatterEngine.Generate(context, parameters);
        var result2 = FloraScatterEngine.Generate(context, parameters);

        Assert.NotEmpty(result1.Additions);
        Assert.Equal(result1.Additions.Count, result2.Additions.Count);
        Assert.Equal(result1.Report.TotalPlanted, result2.Report.TotalPlanted);

        for (int i = 0; i < result1.Additions.Count; i++)
        {
            var a = result1.Additions[i];
            var b = result2.Additions[i];
            Assert.Equal(a.Name, b.Name);
            Assert.Equal(a.X, b.X);
            Assert.Equal(a.Y, b.Y);
            Assert.Equal(a.Z, b.Z);
            Assert.Equal(a.Rotation, b.Rotation);
        }
    }

    [Fact]
    public void Different_seeds_produce_different_distributions()
    {
        var (templates, names) = CreateTestCatalog();
        var context = new FloraScatterContext
        {
            Dimension = 64,
            TileWorldSize = 64.0f,
            Profile = BiomeEcologyProfile.GermanicForest,
            Templates = templates,
            ObjDefNames = names,
        };

        var result1 = FloraScatterEngine.Generate(context, new FloraScatterParameters { Seed = 1001, MinTileX = 5, MinTileY = 5, MaxTileX = 25, MaxTileY = 25 });
        var result2 = FloraScatterEngine.Generate(context, new FloraScatterParameters { Seed = 2002, MinTileX = 5, MinTileY = 5, MaxTileX = 25, MaxTileY = 25 });

        Assert.NotEmpty(result1.Additions);
        Assert.NotEmpty(result2.Additions);
        // 坐標分佈應不完全相同
        Assert.False(result1.Additions.SequenceEqual(result2.Additions));
    }

    [Fact]
    public void Riparian_adaptation_generates_water_plants_along_shoreline_and_avoids_deep_water()
    {
        var (templates, names) = CreateTestCatalog();
        int dimension = 32;
        int vertexSize = 33;
        byte[] heights = new byte[vertexSize * vertexSize];

        // 建立地形：左側 (tx < 10) 為深水 (H = 110, waterLevel = 120, depth = 10 * 4 = 40)
        // 中間 (tx = 10 ~ 12) 為水岸 (H = 120, waterLevel = 120)
        // 右側 (tx > 12) 為陸地 (H = 135)
        for (int z = 0; z < vertexSize; z++)
        {
            for (int x = 0; x < vertexSize; x++)
            {
                if (x < 10) heights[z * vertexSize + x] = 110;
                else if (x <= 12) heights[z * vertexSize + x] = 120;
                else heights[z * vertexSize + x] = 135;
            }
        }

        var context = new FloraScatterContext
        {
            Dimension = dimension,
            VertexSize = vertexSize,
            Heights = heights,
            HeightMapStep = 4.0f,
            WaterLevel = 120,
            Profile = BiomeEcologyProfile.GermanicForest,
            Templates = templates,
            ObjDefNames = names,
        };

        var parameters = new FloraScatterParameters
        {
            Seed = 5566,
            MinTileX = 0,
            MinTileY = 0,
            MaxTileX = 31,
            MaxTileY = 31,
            EnableRiparianFlora = true,
            DensityMultiplier = 1.5f,
        };

        var result = FloraScatterEngine.Generate(context, parameters);

        // 深水區（X < 10 * 64 = 640）嚴格禁止有任何植物
        var deepWaterAdditions = result.Additions.Where(a => a.X < 9.5f * 64.0f).ToList();
        Assert.Empty(deepWaterAdditions);

        // 水岸帶（X 約 10 ~ 13 格）應生成水岸植物（Schilf / 蘆葦）
        var riparianAdditions = result.Additions.Where(a => a.Name.Contains("Schilf")).ToList();
        Assert.NotEmpty(riparianAdditions);
        Assert.True(result.Report.RiparianCount > 0);
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Missing_height_data_does_not_imply_deep_water_or_shoreline(bool emptyHeights)
    {
        var (templates, names) = CreateTestCatalog();
        foreach (byte waterLevel in new byte[] { 0, 120 })
        {
            var context = new FloraScatterContext
            {
                Dimension = 32,
                VertexSize = 33,
                Heights = emptyHeights ? Array.Empty<byte>() : null,
                WaterLevel = waterLevel,
                Profile = BiomeEcologyProfile.GermanicForest,
                Templates = templates,
                ObjDefNames = names,
            };
            var result = FloraScatterEngine.Generate(context, new FloraScatterParameters { Seed = 4242 });

            Assert.NotEmpty(result.Additions);
            Assert.Equal(0, result.Report.RiparianCount);
            Assert.All(result.Additions, addition => Assert.Equal(0f, addition.Y));
        }
    }

    [Fact]
    public void Present_zero_height_data_is_still_treated_as_deep_water()
    {
        var (templates, names) = CreateTestCatalog();
        var context = new FloraScatterContext
        {
            Dimension = 32,
            VertexSize = 33,
            Heights = new byte[33 * 33],
            WaterLevel = 120,
            Profile = BiomeEcologyProfile.GermanicForest,
            Templates = templates,
            ObjDefNames = names,
        };

        var result = FloraScatterEngine.Generate(context, new FloraScatterParameters());
        Assert.Empty(result.Additions);
        Assert.Equal(0, result.Report.TotalPlanted);
    }

    [Fact]
    public void Cliff_steep_slope_excludes_canopy_trees_in_favor_of_rocks()
    {
        var (templates, names) = CreateTestCatalog();
        int dimension = 32;
        int vertexSize = 33;
        byte[] heights = new byte[vertexSize * vertexSize];

        // 建立斷崖地形：X = 15 ~ 16 處有垂直懸崖高差 (100 -> 160)
        for (int z = 0; z < vertexSize; z++)
        {
            for (int x = 0; x < vertexSize; x++)
            {
                if (x <= 15) heights[z * vertexSize + x] = 100;
                else heights[z * vertexSize + x] = 160;
            }
        }

        var context = new FloraScatterContext
        {
            Dimension = dimension,
            VertexSize = vertexSize,
            Heights = heights,
            HeightMapStep = 4.0f,
            WaterLevel = 50,
            Profile = BiomeEcologyProfile.GermanicForest,
            Templates = templates,
            ObjDefNames = names,
        };

        var parameters = new FloraScatterParameters
        {
            Seed = 7788,
            MinTileX = 14,
            MinTileY = 0,
            MaxTileX = 17,
            MaxTileY = 31,
            EnableCliffRocks = true,
            DensityMultiplier = 1.5f,
        };

        var result = FloraScatterEngine.Generate(context, parameters);

        // 在懸崖格上（tx=15~16），不可生成高大喬木 (LanGerNad)
        var cliffAdditions = result.Additions
            .Where(a => a.X >= 15f * 64f && a.X <= 17f * 64f)
            .ToList();

        // 懸崖上生成的應為岩石或灌木，無高大冷杉
        foreach (var item in cliffAdditions)
        {
            Assert.DoesNotContain("Tanne", item.Name);
        }

        Assert.True(result.Report.RockCount > 0 || result.Report.UnderstoryCount > 0);
    }

    [Fact]
    public void Obstacle_avoidance_clears_roads_and_building_zones()
    {
        var (templates, names) = CreateTestCatalog();
        int dimension = 32;

        // 設立道路：垂直貫穿 X = 10 的直線
        var roadTiles = new HashSet<int>();
        for (int y = 0; y < dimension; y++)
            roadTiles.Add(y * dimension + 10);

        // 設立建築基地：中心在 (20 * 64, 20 * 64)，半徑 3 tiles
        var buildingObstacles = new List<(float X, float Z, float RadiusTiles)>
        {
            (20f * 64f, 20f * 64f, 3.0f)
        };

        var context = new FloraScatterContext
        {
            Dimension = dimension,
            TileWorldSize = 64.0f,
            RoadTiles = roadTiles,
            ObstacleCircles = buildingObstacles,
            Profile = BiomeEcologyProfile.GermanicForest,
            Templates = templates,
            ObjDefNames = names,
        };

        var parameters = new FloraScatterParameters
        {
            Seed = 999,
            MinTileX = 0,
            MinTileY = 0,
            MaxTileX = 31,
            MaxTileY = 31,
            AvoidRoadsAndBuildings = true,
            DensityMultiplier = 2.0f, // 高密度加強檢測
        };

        var result = FloraScatterEngine.Generate(context, parameters);

        Assert.NotEmpty(result.Additions);
        Assert.True(result.Report.AvoidedObstacles > 0);

        // 驗證沒有任何植物落在道路上 (tile 10: 640 <= X < 704)
        foreach (var addition in result.Additions)
        {
            int tileX = (int)(addition.X / 64f);
            Assert.NotEqual(10, tileX);

            // 驗證沒有植物落在建築防護半徑內 (20*64, 20*64, r=3*64=192)
            float dx = addition.X - 20f * 64f;
            float dz = addition.Z - 20f * 64f;
            float dist = MathF.Sqrt(dx * dx + dz * dz);
            Assert.True(dist >= 3.0f * 64f, $"Plant at ({addition.X}, {addition.Z}) overlapped building footprint! Dist: {dist}");
        }
    }

    [Fact]
    public void Integration_with_NatureEditSession_supports_single_step_undo_and_redo()
    {
        var (templates, names) = CreateTestCatalog();
        var context = new FloraScatterContext
        {
            Dimension = 32,
            TileWorldSize = 64.0f,
            Profile = BiomeEcologyProfile.GermanicForest,
            Templates = templates,
            ObjDefNames = names,
        };

        var parameters = new FloraScatterParameters
        {
            Seed = 1234,
            MinTileX = 5,
            MinTileY = 5,
            MaxTileX = 15,
            MaxTileY = 15,
            DensityMultiplier = 1.0f,
        };

        var result = FloraScatterEngine.Generate(context, parameters);
        Assert.NotEmpty(result.Additions);

        var session = new NatureEditSession();
        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);

        // 批次散播種植
        bool planted = session.PlantMany(result.Additions);
        Assert.True(planted);
        Assert.True(session.IsDirty);
        Assert.True(session.CanUndo);
        Assert.Equal(result.Additions.Count, session.Additions.Count);

        // 單次 Undo 撤銷整批散播
        Assert.True(session.Undo());
        Assert.Empty(session.Additions);
        Assert.False(session.IsDirty);
        Assert.True(session.CanRedo);

        // Redo 重新套用整批散播
        Assert.True(session.Redo());
        Assert.Equal(result.Additions.Count, session.Additions.Count);
        Assert.True(session.IsDirty);
    }
}
