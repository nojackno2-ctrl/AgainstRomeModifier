using System.Runtime.CompilerServices;
using AgainstRomeMapEditor.Modules.Nature;
using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeMapEditor.Modules.Settlement;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;
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
    public void Evaluator_places_main_house_and_six_core_buildings_without_collisions()
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
        Assert.Equal(7, result.PlannedBuildings.Count); // 主屋 + 6 棟核心建築

        var mainHouse = result.PlannedBuildings[0];
        Assert.Equal("GER_HAU00", mainHouse.TypeName);

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
        Assert.Equal("GER_HAU00", p1.Buildings[0].TypeName);
        Assert.Equal("ROM_HAU00", p2.Buildings[0].TypeName);

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
        Assert.True(placementPreset.Entries.Count >= 14); // 至少 2 * 7 棟建築

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
        var catalog = RealPlacementCatalog();

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
        var templateDict = RealNatureNames.ToDictionary(name => name, _ => natureTemplate, StringComparer.OrdinalIgnoreCase);

        var additions = MapLayoutPresets.PlanNature(readNature, templateDict, 0, 0, 0, (_, _) => 20f);
        var natureSession = new NatureEditSession();
        Assert.True(natureSession.PlantMany(additions));
        Assert.Equal(additions.Count, natureSession.Additions.Count);

        Assert.True(natureSession.Undo());
        Assert.Empty(natureSession.Additions);
        Assert.True(natureSession.Redo());
        Assert.Equal(additions.Count, natureSession.Additions.Count);
    }

    [Fact]
    public void Engine_resolves_short_type_names_against_real_game_catalog_and_skips_missing()
    {
        var evaluator = new SettlementSiteEvaluator(Dimension, (_, _) => 20f, (_, _) => false, 0f, TileSize);
        var planner = new ResourceClusterPlanner(Dimension, (_, _) => 20f, (_, _) => false, 0f, TileSize);
        var balancer = new MultiplayerFairnessBalancer(evaluator, planner, Dimension, TileSize);
        var result = balancer.Generate(2, SymmetryMode.CentralSymmetry, baseSeed: 42);

        // 真實 cl_scint.ini 命名：別名 GER_HAU00 對應 BauGerHau00_Haupthaus，且沒有 Kas／Tur 等猜測型別。
        var catalog = new List<SdlObjectType>
        {
            new("BauGerHau00_Haupthaus", -1, SdlObjectCategory.Building, "Ger", 0, new Dictionary<string, string> { ["alias"] = "GER_HAU00" }),
            new("BauGerLag00_Lagerhaus", -1, SdlObjectCategory.Building, "Ger", 0, new Dictionary<string, string> { ["alias"] = "GER_LAG00" }),
            new("BauGerWoh00_Wohnhaus", -1, SdlObjectCategory.Building, "Ger", 0, new Dictionary<string, string> { ["alias"] = "GER_WOH00" }),
        };
        var session = new PlacementEditSession();
        var placed = SettlementGeneratorEngine.ApplyToPlacementSession(result, catalog, session, (_, _) => 20f);

        Assert.NotEmpty(placed);
        Assert.All(Enumerable.Range(0, session.Count), index => Assert.Contains(session[index].Type.NameDef, catalog.Select(item => item.NameDef)));
        Assert.True(session.Undo());
        Assert.Equal(0, session.Count);

        Assert.Equal("GER_HAU00", LayoutTypeResolver.ResolveAlias(catalog, "BauGerHau00"));
        Assert.Null(LayoutTypeResolver.ResolveAlias(catalog, "BauGerKas00"));
        var natureKeys = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase) { ["LanGerNad00_Tanne_gross"] = 1 };
        Assert.Equal("LanGerNad00_Tanne_gross", LayoutTypeResolver.ResolveKey(natureKeys, "LanGerNad00"));
    }

    [Fact]
    public void Wildlife_is_a_neutral_object_and_rejects_troop_counts_or_invalid_teams()
    {
        var animal = RealPlacementCatalog().Single(t => t.IsAnimal);
        var placement = new SdlPlacedObject(animal, 100f, 20f, 100f, -1, 0f, 0);
        PlacementEditSession.ValidateBounds(placement, strictUnitCount: true);
        Assert.Throws<ArgumentOutOfRangeException>(() => PlacementEditSession.ValidateBounds(placement with { Team = -2 }));
        Assert.Throws<ArgumentOutOfRangeException>(() => PlacementEditSession.ValidateBounds(placement with { Team = 16 }));
        Assert.Throws<ArgumentOutOfRangeException>(() => PlacementEditSession.ValidateBounds(placement with { UnitCount = 1 }));
        var human = placement with { Type = animal with { NameDef = "FigGerSch00_Schwert" } };
        Assert.False(human.Type.IsAnimal);
        Assert.Throws<ArgumentOutOfRangeException>(() => PlacementEditSession.ValidateBounds(human with { UnitCount = 1 }, strictUnitCount: true));
    }

    private static IReadOnlyList<SdlObjectType> RealPlacementCatalog() => ScriptObjectAliases.Parse("""
        [ObjDefName]
        GER_HAU00 = BauGerHau00_Haupthaus
        GER_LAG00 = BauGerLag00_Lagerhaus
        GER_WOH00 = BauGerWoh00_Wohnhaus
        GER_BAU00 = BauGerBau00_Bauernhof
        GER_WAF00 = BauGerWaf00_Waffenschmiede
        GER_STA00 = BauGerSta00_Pferdestall
        GER_SCHRE00 = BauGerSchre00_Schreinerei
        ROM_HAU00 = BauRomHau00_Hauptzelt
        ROM_LAG00 = BauRomLag00_Lagerzelt
        ROM_WOH00 = BauRomWoh00_Wohnzelt
        ROM_BAU00 = BauRomBau00_Bauernhof
        ROM_WAF00 = BauRomWaf00_Waffenschmiede
        ROM_STA00 = BauRomSta00_Pferdestall
        ROM_SCHRE00 = BauRomSchre00_Schreinerei
        KEL_HAU00 = BauKelHau00_Haupthaus
        KEL_LAG00 = BauKelLag00_Lagerhaus
        KEL_WOH00 = BauKelWoh00_Wohnhaus
        KEL_BAU00 = BauKelBau00_Bauernhof
        KEL_WAF00 = BauKelWaf00_Waffenschmiede
        KEL_STA00 = BauKelSta00_Pferdestall
        KEL_SCHRE00 = BauKelSchre00_Schreinerei
        HUN_HAU00 = BauHunHau00_Haupthaus
        HUN_LAG00 = BauHunLag00_Lagerhaus
        HUN_WOH00 = BauHunWoh00_Wohnhaus
        HUN_SCHLA00 = BauHunSchla00_Schlachter
        HUN_WAF00 = BauHunWaf00_Waffenschmiede
        HUN_STA00 = BauHunSta00_Pferdestall
        HUN_SCHRE00 = BauHunSchre00_Schreinerei
        ALL_EBE00 = FigTieEbe00_Wildschwein
        """).Select(alias => new SdlObjectType(alias.NameDef, -1, alias.Category, alias.Tribe, 0,
            new Dictionary<string, string> { ["alias"] = alias.Alias })).ToArray();

    private static readonly string[] RealNatureNames =
    [
        "LanGerNad00_Tanne_gross", "LanGerNad05_Tanne_gross", "LanGerNad18_Tanne_klein",
        "LanGerNad24_Tanne_mittel", "LanGerNabu00_Nadelbusch",
        "LanGerSte00_1Stein", "LanGerSte01_1Stein", "LanGerSte02_1Stein", "LanGerSte05_1Stein",
        "LanItaPin00_Pinie", "LanItaPin01_Pinie", "LanItaZyp00_Zypresse", "LanItaZyp01_Zypresse",
        "LanItaBus08_Kleiner_Busch", "LanItaSte00_1Stein", "LanItaSte01_1Stein", "LanItaSte02_1Stein"
    ];

    [Theory]
    [InlineData(SettlementTribe.Germanic, "GER", "Farm")]
    [InlineData(SettlementTribe.Roman, "ROM", "Farm")]
    [InlineData(SettlementTribe.Celtic, "KEL", "Farm")]
    [InlineData(SettlementTribe.Hun, "HUN", "Butcher")]
    public void Every_tribe_resolves_all_roles_and_resources_without_dropping_entries(
        SettlementTribe tribe, string prefix, string foodRole)
    {
        var catalog = RealPlacementCatalog();
        var (main, core) = SettlementTribalPresets.GetBlueprints(tribe);
        Assert.Equal(prefix + "_HAU00", main.TypeName);
        Assert.Equal("MainHouse", main.Category);
        Assert.Equal(new[] { "Warehouse", "House", foodRole, "Blacksmith", "Stable", "Workshop" }, core.Select(b => b.Category));
        Assert.All(core.Prepend(main), b =>
        {
            Assert.Equal(b.TypeName, LayoutTypeResolver.ResolveAlias(catalog, b.TypeName));
            Assert.Equal(SdlObjectCategory.Building, catalog.Single(t => MapLayoutPresets.Alias(t) == b.TypeName).Category);
            Assert.InRange(b.PreferredAngleDeg, 0f, 360f);
        });
        Assert.Equal(new[] { "ALL_EBE00" }, SettlementTribalPresets.GetWildlifePalette());
        Assert.Equal(SdlObjectCategory.Figure, catalog.Single(t => MapLayoutPresets.Alias(t) == "ALL_EBE00").Category);
        Assert.All(SettlementTribalPresets.GetForestTreePalette(tribe).Concat(SettlementTribalPresets.GetStoneQuarryPalette(tribe)),
            name => Assert.Contains(name, RealNatureNames));

        var evaluator = new SettlementSiteEvaluator(Dimension, (_, _) => 20f, (_, _) => false, 0f, TileSize);
        var planner = new ResourceClusterPlanner(Dimension, (_, _) => 20f, (_, _) => false, 0f, TileSize);
        var distribution = new MultiplayerFairnessBalancer(evaluator, planner, Dimension, TileSize)
            .Generate(2, SymmetryMode.CentralSymmetry, [tribe, tribe], baseSeed: 42);
        Assert.All(distribution.Players, p =>
        {
            Assert.True(p.SiteEvaluation.IsValid);
            Assert.Equal(7, p.Buildings.Count);
            Assert.NotEmpty(p.ForestTrees);
            Assert.NotEmpty(p.StoneQuarries);
            Assert.NotEmpty(p.WildlifeAndFood);
            Assert.All(p.WildlifeAndFood, w => { Assert.Equal("ALL_EBE00", w.TypeName); Assert.Equal(-1, w.Team); Assert.Equal(0, w.Count); });
            for (int i = 0; i < p.Buildings.Count; i++)
                for (int j = i + 1; j < p.Buildings.Count; j++)
                {
                    var a = p.Buildings[i]; var b = p.Buildings[j];
                    float dx = (a.WorldX - b.WorldX) / TileSize, dz = (a.WorldZ - b.WorldZ) / TileSize;
                    Assert.True(MathF.Sqrt(dx * dx + dz * dz) >= a.FootprintRadius + b.FootprintRadius + 1.2f);
                }
        });
        var (placementJson, natureJson) = SettlementGeneratorEngine.ExportLayoutJsons(distribution);
        var placementPreset = MapLayoutPresets.Deserialize(placementJson);
        var naturePreset = MapLayoutPresets.Deserialize(natureJson);
        var placementSession = new PlacementEditSession();
        var placed = SettlementGeneratorEngine.ApplyToPlacementSession(distribution, catalog, placementSession, (_, _) => 20f);
        Assert.Equal(placementPreset.Entries.Count, placed.Count);
        Assert.Equal(placementPreset.Entries.Select(e => e.Type), Enumerable.Range(0, placementSession.Count).Select(i => MapLayoutPresets.Alias(placementSession[i].Type)));
        Assert.True(placementSession.Undo());
        Assert.Equal(0, placementSession.Count);
        Assert.True(placementSession.Redo());
        Assert.Equal(placed.Count, placementSession.Count);

        var template = (LevelObjectTemplate)RuntimeHelpers.GetUninitializedObject(typeof(LevelObjectTemplate));
        var templates = RealNatureNames.ToDictionary(name => name, _ => template, StringComparer.OrdinalIgnoreCase);
        var natureSession = new NatureEditSession();
        Assert.True(SettlementGeneratorEngine.ApplyToNatureSession(distribution, templates, natureSession, (_, _) => 20f));
        Assert.Equal(naturePreset.Entries.Count, natureSession.Additions.Count);
        Assert.True(natureSession.Undo());
        Assert.Empty(natureSession.Additions);
        Assert.True(natureSession.Redo());
        Assert.Equal(naturePreset.Entries.Count, natureSession.Additions.Count);
    }
}
