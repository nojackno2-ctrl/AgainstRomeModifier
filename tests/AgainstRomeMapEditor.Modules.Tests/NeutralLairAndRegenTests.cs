using System.Runtime.CompilerServices;
using AgainstRomeMapEditor.Modules.Events;
using AgainstRomeMapEditor.Modules.WildLair;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NeutralLairAndRegenTests
{
    [Fact]
    public void NeutralLairCatalog_Default_ContainsCanonicalLairsAndValidates()
    {
        var catalog = NeutralLairCatalog.Default;
        Assert.NotNull(catalog);
        Assert.True(catalog.AllDefinitions.Count >= 7);

        var wolf = catalog.GetById("LAIR_WOLF_DEN_SMALL");
        Assert.NotNull(wolf);
        Assert.Equal(LairCategory.BeastDen, wolf.Category);
        Assert.Equal(LairDifficultyTier.Tier1Scout, wolf.Tier);
        Assert.NotEmpty(wolf.DefaultGuards);
        Assert.NotEmpty(wolf.WaveRules);

        var bear = catalog.GetById("LAIR_BEAR_CAVE_FEROCIOUS");
        Assert.NotNull(bear);
        Assert.Equal("Alpine", bear.BiomeAffinity);

        var raider = catalog.GetById("LAIR_BARBARIAN_RAIDER_CAMP");
        Assert.NotNull(raider);
        Assert.Equal(LairCategory.BarbarianCamp, raider.Category);

        // 威脅度評估：大型掠奪者要塞威脅度應顯著高於小型狼穴
        float wolfThreat = NeutralLairCatalog.CalculateThreatRating(wolf);
        float raiderThreat = NeutralLairCatalog.CalculateThreatRating(raider);
        Assert.True(raiderThreat > wolfThreat, $"Raider threat {raiderThreat} should be greater than wolf threat {wolfThreat}");
    }

    [Fact]
    public void NeutralLairCatalog_FilteringAndQuerying_WorksAsExpected()
    {
        var catalog = NeutralLairCatalog.Default;

        var beastDens = catalog.GetByCategory(LairCategory.BeastDen).ToList();
        Assert.Contains(beastDens, d => d.Id == "LAIR_WOLF_DEN_SMALL");
        Assert.Contains(beastDens, d => d.Id == "LAIR_BEAR_CAVE_FEROCIOUS");

        var tier1Lairs = catalog.GetByTier(LairDifficultyTier.Tier1Scout).ToList();
        Assert.All(tier1Lairs, d => Assert.Equal(LairDifficultyTier.Tier1Scout, d.Tier));

        var filtered = catalog.Filter(category: LairCategory.BarbarianCamp, tier: LairDifficultyTier.Tier2Standard).ToList();
        Assert.Contains(filtered, d => d.Id == "LAIR_BARBARIAN_RAIDER_CAMP");
    }

    [Fact]
    public void NeutralLairCatalog_Validation_RejectsInvalidRules()
    {
        // 週期過短 (< 10s)
        Assert.Throws<ArgumentOutOfRangeException>(() =>
        {
            var invalid = new NeutralLairDefinition(
                "TEST_BAD", "無效巢穴", "Bad Lair",
                LairCategory.BeastDen, LairDifficultyTier.Tier1Scout,
                "LanGerStein01", 2.0f,
                [],
                [new LairWaveSpawnRule("bad_wave", "GER_INF01", SpawnCount: 2, IntervalSeconds: 5)],
                new LairLootReward());
            NeutralLairCatalog.ValidateDefinition(invalid);
        });

        // 刷怪人數超出上限 (> 20)
        Assert.Throws<ArgumentOutOfRangeException>(() =>
        {
            var invalid = new NeutralLairDefinition(
                "TEST_BAD_COUNT", "無效人數", "Bad Count",
                LairCategory.BeastDen, LairDifficultyTier.Tier1Scout,
                "LanGerStein01", 2.0f,
                [],
                [new LairWaveSpawnRule("bad_wave", "GER_INF01", SpawnCount: 25, IntervalSeconds: 60)],
                new LairLootReward());
            NeutralLairCatalog.ValidateDefinition(invalid);
        });
    }

    [Fact]
    public void ResourceRegenerationPlanner_CellularAutomata_SimulatesStumpDecayAndSaplingGrowth()
    {
        int dim = 16;
        var planner = new ResourceRegenerationPlanner(dim, tileWorldSize: 64f, seed: 42);

        // 初始化：在 (5, 5) 放置一株成熟喬木
        planner.InitializeFromMap(new[] { (5.5f * 64f, 5.5f * 64f, "LanGerTanne01") });

        var initialTree = planner.GetCell(5, 5);
        Assert.Equal(ForestCellState.MatureTree, initialTree.State);

        // 採伐 (5, 5) 成為殘樁
        planner.MarkHarvestedStump(5, 5, "LanGerTanne01");
        Assert.Equal(ForestCellState.Stump, planner.GetCell(5, 5).State);
        Assert.Single(planner.ActiveStumps);

        // 迭代 6 步，殘樁應風化轉為肥沃土地 Barren
        var config = new RegenerationParameters { StumpDecayTicks = 5, SeedDispersalProbability = 0.5f };
        planner.StepSimulation(steps: 6, parameters: config);

        var decayedCell = planner.GetCell(5, 5);
        Assert.Equal(ForestCellState.Barren, decayedCell.State);
        Assert.True(decayedCell.Fertility > 0.6f);
        Assert.Empty(planner.ActiveStumps);
    }

    [Fact]
    public void ResourceRegenerationPlanner_SeedDispersal_SpreadsForestCanopy()
    {
        int dim = 12;
        var planner = new ResourceRegenerationPlanner(dim, tileWorldSize: 64f, seed: 1234);

        // 中心區域放置成片成樹 (4,4), (4,5), (5,4), (5,5)
        var trees = new List<(float, float, string)>
        {
            (4.5f * 64f, 4.5f * 64f, "LanGerTanne01"),
            (4.5f * 64f, 5.5f * 64f, "LanGerTanne01"),
            (5.5f * 64f, 4.5f * 64f, "LanGerTanne01"),
            (5.5f * 64f, 5.5f * 64f, "LanGerTanne01")
        };
        planner.InitializeFromMap(trees);

        // 高機率擴散並迭代 15 步
        var config = new RegenerationParameters
        {
            SeedDispersalProbability = 0.45f,
            SaplingToYoungTicks = 2,
            YoungToMatureTicks = 3
        };

        var statsBefore = planner.GetStatistics();
        Assert.Equal(4, statsBefore.MatureTreeCount);

        planner.StepSimulation(steps: 10, parameters: config);

        var statsAfter = planner.GetStatistics();
        // 經過 10 步，鄰域空地應長出新生幼苗、小樹或成樹，樹木總量應增加
        int totalCanopyAfter = statsAfter.SaplingCount + statsAfter.YoungTreeCount + statsAfter.MatureTreeCount + statsAfter.AncientCanopyCount;
        Assert.True(totalCanopyAfter > 4, $"Forest should expand from 4 trees, got {totalCanopyAfter}");
    }

    [Fact]
    public void ResourceRegenerationPlanner_ExportRegeneratedNature_ProducesValidAdditions()
    {
        int dim = 8;
        var planner = new ResourceRegenerationPlanner(dim, tileWorldSize: 64f, seed: 99);
        planner.InitializeFromMap(new[] { (2.5f * 64f, 2.5f * 64f, "LanGerTanne01") });

        var mockTemplate = (LevelObjectTemplate)RuntimeHelpers.GetUninitializedObject(typeof(LevelObjectTemplate));
        var additions = planner.ExportRegeneratedNature(species => mockTemplate);

        Assert.Single(additions);
        var item = additions[0];
        Assert.Equal("LanGerTanne01", item.Name);
        Assert.Same(mockTemplate, item.Template);
        Assert.True(item.X > 0 && item.Z > 0);
    }

    [Fact]
    public void WildLairScenarioEventBinder_GeneratesSpawnAndDestructionEvents()
    {
        var lairDef = NeutralLairCatalog.Default.GetById("LAIR_WOLF_DEN_SMALL")!;
        var coreSpawnId = Guid.NewGuid();
        var lair = new PlacedNeutralLair(
            InstanceId: Guid.NewGuid(),
            DefinitionId: lairDef.Id,
            WorldX: 4000f,
            WorldY: 150f,
            WorldZ: 6000f,
            Team: 7,
            CoreStructureSpawnId: coreSpawnId);

        var events = WildLairScenarioEventBinder.GenerateEventsForLair(lair, lairDef, playerTeam: 0);

        Assert.NotEmpty(events);

        // 1. 週期刷怪事件
        var spawnEvent = Assert.Single(events, e => e.Repeat);
        Assert.Contains(WildLairScenarioEventBinder.LairEventPrefix, spawnEvent.Name);
        Assert.Equal(lairDef.WaveRules[0].IntervalSeconds, spawnEvent.DelaySeconds);
        Assert.True(spawnEvent.Repeat);
        Assert.Single(spawnEvent.Conditions);
        Assert.Equal(ScenarioConditionKind.ObjectExists, spawnEvent.Conditions[0].Kind);
        Assert.Equal(coreSpawnId, spawnEvent.Conditions[0].TargetId);

        var spawnAction = Assert.Single(spawnEvent.Actions);
        Assert.Equal(ScenarioActionKind.SpawnUnit, spawnAction.Kind);
        Assert.Equal("GER_INF01", spawnAction.Alias);
        Assert.Equal(7, spawnAction.Team);

        // 2. 巢穴殲滅獎勵事件
        var destroyEvent = Assert.Single(events, e => !e.Repeat && e.Conditions.Count > 0);
        Assert.Equal(ScenarioConditionKind.ObjectDeadOrRemoved, destroyEvent.Conditions[0].Kind);
        Assert.Equal(coreSpawnId, destroyEvent.Conditions[0].TargetId);
        Assert.Equal(ScenarioActionKind.Message, destroyEvent.Actions[0].Kind);

        // 3. 敵對外交鎖定
        var diploEvent = Assert.Single(events, e => e.DelaySeconds == 0 && e.Conditions.Count == 0);
        Assert.Equal(ScenarioActionKind.Diplomacy, diploEvent.Actions[0].Kind);
        Assert.Equal(7, diploEvent.Actions[0].Team);
        Assert.Equal(0, diploEvent.Actions[0].OtherTeam);
        Assert.True(diploEvent.Actions[0].Hostile);
    }

    [Fact]
    public void WildLairScenarioEventBinder_SyncAndRemoveSessionEvents_LeavesNoOrphans()
    {
        var lairDef = NeutralLairCatalog.Default.GetById("LAIR_BARBARIAN_RAIDER_CAMP")!;
        var coreSpawnId = Guid.NewGuid();
        var lair = new PlacedNeutralLair(
            InstanceId: Guid.NewGuid(),
            DefinitionId: lairDef.Id,
            WorldX: 5000f,
            WorldY: 200f,
            WorldZ: 5000f,
            Team: 7,
            CoreStructureSpawnId: coreSpawnId);

        var session = new ScenarioEventSession();
        // 預先加入既有玩家事件
        session.Add(new ScenarioEvent("PLAYER_VICTORY_CHECK", 10, Repeat: false)
        {
            Actions = [new ScenarioAction(ScenarioActionKind.Victory)]
        });

        // 同步巢穴事件
        WildLairScenarioEventBinder.SyncLairEvents(session, lair, lairDef, playerTeam: 0);
        Assert.True(session.Count > 1);

        // 驗證完整性
        var validationErrors = WildLairScenarioEventBinder.ValidateLairBindings(session.Capture(), [lair]);
        Assert.Empty(validationErrors);

        // 移除該巢穴
        int removed = WildLairScenarioEventBinder.RemoveLairEvents(session, lair.InstanceId);
        Assert.True(removed >= 2);

        // 原有玩家事件必須保留
        Assert.Single(session.Capture());
        Assert.Equal("PLAYER_VICTORY_CHECK", session.Capture()[0].Name);
    }
}
