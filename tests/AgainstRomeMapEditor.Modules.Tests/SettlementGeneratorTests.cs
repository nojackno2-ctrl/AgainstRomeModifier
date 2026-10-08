using System.Runtime.CompilerServices;
using AgainstRomeMapEditor.Modules.Nature;
using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeMapEditor.Modules.Settlement;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class SettlementGeneratorTests
{
    private const int Dimension = 256;
    private const float TileSize = 64f;

    [Fact]
    public void Evaluator_rejects_steep_terrain_and_water_hazards()
    {
        // 模擬懸崖地形
        var evaluator = new SettlementSiteEvaluator(
            dimension: Dimension,
            sampleHeight: (x, z) => (x > 100 * TileSize) ? 50f : 0f,
            isBlocked: (_, _) => false,
            waterLevel: 0f,
            tileWorldSize: TileSize);

        // 位於陡坡交界處
        var steepResult = evaluator.Evaluate(100f, 100f, SettlementTribe.Germanic);
        Assert.False(steepResult.IsValid);
        Assert.NotNull(steepResult.RejectionReason);

        // 模擬水域淹沒
        var waterEvaluator = new SettlementSiteEvaluator(
            dimension: Dimension,
            sampleHeight: (_, _) => 5f,
            isBlocked: (_, _) => false,
            waterLevel: 8f, // 水位高於地面
            tileWorldSize: TileSize);

        var floodResult = waterEvaluator.Evaluate(128f, 128f, SettlementTribe.Germanic);
        Assert.False(floodResult.IsValid);
        Assert.Contains("water", floodResult.RejectionReason, StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public void Evaluator_places_main_house_and_five_core_buildings_without_collisions()
    {
        var evaluator = new SettlementSiteEvaluator(
            dimension: Dimension,
            sampleHeight: (_, _) => 20f, // 平坦高地
            isBlocked: (_, _) => false,
            waterLevel: 0f,
            tileWorldSize: TileSize);

        var result = evaluator.Evaluate(128f, 128f, SettlementTribe.Germanic);
        Assert.True(result.IsValid);
        Assert.True(result.TotalScore > 75f);
        Assert.Equal(6, result.PlannedBuildings.Count); // 主屋 + 5 棟核心建築

        var mainHouse = result.PlannedBuildings[0];
        Assert.Equal("BauGerHau00", mainHouse.TypeName);

        // 檢查任意兩棟建築之間的距離必須大於各自足跡半徑之和
        for (int i = 0; i < result.PlannedBuildings.Count; i++)
        {
            for (int j = i + 1; j < result.PlannedBuildings.Count; j++)
            {
                var b1 = result.PlannedBuildings[i];
                var b2 = result.PlannedBuildings[j];
                float dx = (b1.WorldX - b2.WorldX) / TileSize;
                float dz = (b1.WorldZ - b2.WorldZ) / TileSize;
                float dist = MathF.Sqrt(dx * dx + dz * dz);
                float minDist = b1.FootprintRadius + b2.FootprintRadius;
                Assert.True(dist >= minDist, $"Buildings {b1.TypeName} and {b2.TypeName} overlap! dist={dist}, minDist={minDist}");
            }
        }
    }

    [Fact]
    public void ResourcePlanner_generates_forest_quarry_and_wildlife_with_proper_constraints()
    {
        var planner = new ResourceClusterPlanner(
            dimension: Dimension,
            sampleHeight: (_, _) => 15f,
            isBlocked: (_, _) => false,
            waterLevel: 0f,
            tileWorldSize: TileSize);

        var (forest, quarry, wildlife) = planner.PlanClusters(128f, 128f, SettlementTribe.Germanic, seed: 12345);

        // 森林驗證
        Assert.NotEmpty(forest);
        foreach (var tree in forest)
        {
            float dx = (tree.WorldX / TileSize) - 128f;
            float dz = (tree.WorldZ / TileSize) - 128f;
            float dist = MathF.Sqrt(dx * dx + dz * dz);
            Assert.InRange(dist, 13.5f, 33f); // 距主屋 14-32 格
        }

        // 採石場驗證
        Assert.NotEmpty(quarry);
        Assert.All(quarry, rock => Assert.StartsWith("LanGerSte", rock.TypeName, StringComparison.OrdinalIgnoreCase));

        // 野生動物群驗證
        Assert.NotEmpty(wildlife);
        Assert.All(wildlife, animal => Assert.Equal(-1, animal.Team)); // 中立
    }

    [Fact]
    public void Balancer_generates_perfect_2p_central_symmetry()
    {
        var evaluator = new SettlementSiteEvaluator(Dimension, (_, _) => 20f, (_, _) => false, 0f, TileSize);
        var planner = new ResourceClusterPlanner(Dimension, (_, _) => 20f, (_, _) => false, 0f, TileSize);
        var balancer = new MultiplayerFairnessBalancer(evaluator, planner, Dimension, TileSize);

        var result = balancer.Generate(2, SymmetryMode.CentralSymmetry, [SettlementTribe.Germanic, SettlementTribe.Roman], baseSeed: 999);

        Assert.Equal(2, result.PlayerCount);
        var p1 = result.Players[0];
        var p2 = result.Players[1];

        // 驗證點對稱中心 (128, 128)
        float midX = (p1.SiteEvaluation.AnchorTileX + p2.SiteEvaluation.AnchorTileX) * 0.5f;
        float midZ = (p1.SiteEvaluation.AnchorTileZ + p2.SiteEvaluation.AnchorTileZ) * 0.5f;
        Assert.Equal(128f, midX, precision: 1);
        Assert.Equal(128f, midZ, precision: 1);

        // 驗證建築數與隊伍
        Assert.Equal(0, p1.Buildings[0].Team);
        Assert.Equal(1, p2.Buildings[0].Team);
        Assert.Equal("BauGerHau00", p1.Buildings[0].TypeName);
        Assert.Equal("BauRomHau00", p2.Buildings[0].TypeName);

        Assert.True(result.FairnessReport.IsBalanced);
    }

    [Fact]
    public void Balancer_generates_fair_4p_rotational_symmetry()
    {
        var evaluator = new SettlementSiteEvaluator(Dimension, (_, _) => 20f, (_, _) => false, 0f, TileSize);
        var planner = new ResourceClusterPlanner(Dimension, (_, _) => 20f, (_, _) => false, 0f, TileSize);
        var balancer = new MultiplayerFairnessBalancer(evaluator, planner, Dimension, TileSize);

        var tribes = new[] { SettlementTribe.Germanic, SettlementTribe.Roman, SettlementTribe.Celtic, SettlementTribe.Hun };
        var result = balancer.Generate(4, SymmetryMode.RotationalSymmetry, tribes, baseSeed: 777);

        Assert.Equal(4, result.PlayerCount);
        Assert.All(result.Players, p => Assert.True(p.SiteEvaluation.IsValid));
        Assert.True(result.FairnessReport.MinInterPlayerDistance > 50f * TileSize);
    }

    [Fact]
    public void Engine_converts_to_presets_and_integrates_with_edit_sessions_and_json()
    {
        var evaluator = new SettlementSiteEvaluator(Dimension, (_, _) => 20f, (_, _) => false, 0f, TileSize);
        var planner = new ResourceClusterPlanner(Dimension, (_, _) => 20f, (_, _) => false, 0f, TileSize);
        var balancer = new MultiplayerFairnessBalancer(evaluator, planner, Dimension, TileSize);

        var result = balancer.Generate(2, SymmetryMode.CentralSymmetry, baseSeed: 42);

        // 1. 轉為 Placement Preset
        var placementPreset = SettlementGeneratorEngine.ToPlacementLayoutPreset(result);
        Assert.Equal(MapLayoutKind.Placement, placementPreset.Kind);
        Assert.True(placementPreset.Entries.Count >= 12); // 至少 2 * 6 棟建築

        // 2. 轉為 Nature Preset
        var naturePreset = SettlementGeneratorEngine.ToNatureLayoutPreset(result);
        Assert.Equal(MapLayoutKind.Nature, naturePreset.Kind);
        Assert.NotEmpty(naturePreset.Entries);

        // 3. JSON 序列化與反序列化測試
        var (placementJson, natureJson) = SettlementGeneratorEngine.ExportLayoutJsons(result);
        var readPlacement = MapLayoutPresets.Deserialize(placementJson);
        var readNature = MapLayoutPresets.Deserialize(natureJson);
        Assert.Equal(placementPreset.Entries.Count, readPlacement.Entries.Count);
        Assert.Equal(naturePreset.Entries.Count, readNature.Entries.Count);

        // 4. 驗證與 PlacementEditSession 整合（AddMany 與 Undo/Redo）
        var catalog = new List<SdlObjectType>();
        foreach (var entry in readPlacement.Entries.DistinctBy(e => e.Type))
        {
            catalog.Add(new SdlObjectType(entry.Type, 1, SdlObjectCategory.Building, "Ger", 1, new Dictionary<string, string> { ["alias"] = entry.Type }));
        }

        var placements = MapLayoutPresets.PlanPlacements(readPlacement, catalog, 0, 0, 0, (_, _) => 20f);
        var session = new PlacementEditSession();
        var placedIndices = session.AddMany(placements);
        Assert.Equal(placements.Count, placedIndices.Count);
        Assert.Equal(placements.Count, session.Count);

        // Undo & Redo 驗證
        Assert.True(session.CanUndo);
        Assert.True(session.Undo());
        Assert.Equal(0, session.Count);
        Assert.True(session.CanRedo);
        Assert.True(session.Redo());
        Assert.Equal(placements.Count, session.Count);

        // 5. 驗證與 NatureEditSession 整合（PlantMany 與 Undo/Redo）
        var natureTemplate = (LevelObjectTemplate)RuntimeHelpers.GetUninitializedObject(typeof(LevelObjectTemplate));
        var templateDict = readNature.Entries.DistinctBy(e => e.Type)
            .ToDictionary(e => e.Type, _ => natureTemplate, StringComparer.OrdinalIgnoreCase);

        var additions = MapLayoutPresets.PlanNature(readNature, templateDict, 0, 0, 0, (_, _) => 20f);
        var natureSession = new NatureEditSession();
        Assert.True(natureSession.PlantMany(additions));
        Assert.Equal(additions.Count, natureSession.Additions.Count);

        Assert.True(natureSession.Undo());
        Assert.Empty(natureSession.Additions);
        Assert.True(natureSession.Redo());
        Assert.Equal(additions.Count, natureSession.Additions.Count);
    }
}
