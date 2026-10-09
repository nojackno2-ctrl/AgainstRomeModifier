using System.Runtime.CompilerServices;
using System.Text;
using AgainstRomeMapEditor.Modules.Events;
using AgainstRomeMapEditor.Modules.WildLair;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NeutralLairAndRegenTests
{
    // Identifier-only INI fixture, transcribed from TEMP cl_scint.ini [ObjDefName].
    // No PFIL, original asset file, objdef row or game bytecode is committed.
    private static IReadOnlyList<ScriptObjectAlias> Aliases => ScriptObjectAliases.Parse("""
        [ObjDefName]
        ALL_ZIVMAN00=FigZivMan00_Zivilist
        ALL_ZIVWEI00=FigZivWei00_Zivilistin
        ALL_PACKPF00=FigTiePac00_Packpferd
        ALL_BAE00=FigTieBae00_Baer
        ALL_RAU00=FigTieRau00_Raubkatze
        ALL_WOL00=FigTieWol00_Wilder_Wolf
        ALL_EBE00=FigTieEbe00_Wildschwein
        GER_INF01=FigGerInf01_Schwert
        ROM_INF01=FigRomInf01_Schwert_Schild
        GER_HAU00=BauGerHau00_Haupthaus
        GER_MIN00=BauGerMin00_Mine
        GER_GOL00=BauGerGol00_Goldschmiede
        [ObjDefScript]
        ALL_WOL00=ak_landtier
        """);

    private static NeutralLairDefinition Blueprint() => new("AUTHORED_OUTPOST", "Outpost", "Outpost",
        LairCategory.BarbarianCamp, LairDifficultyTier.Unrated, "BauGerHau00_Haupthaus", 1,
        [], [new("timer", "GER_INF01", 2, 60, 60, SpawnRadiusTiles: 1)], new());
    private static PlacedNeutralLair Placed(NeutralLairDefinition d, Guid? id = null, Guid core = default) =>
        new(id ?? Guid.NewGuid(), d.Id, 4000, 0, 6000, Team: 7, CoreStructureSpawnId: core);
    private static IReadOnlyList<ScenarioEvent> Bind(NeutralLairDefinition d, PlacedNeutralLair? l = null) =>
        WildLairScenarioEventBinder.GenerateEventsForLair(l ?? Placed(d), d, aliases: Aliases);

    [Fact]
    public void Catalog_contains_real_land_animals_without_invented_mechanics()
    {
        var catalog = NeutralLairCatalog.Default.FilteredBy(Aliases);
        Assert.Equal(4, catalog.AllDefinitions.Count);
        Assert.Equal("FigTieWol00_Wilder_Wolf", catalog.GetById("ALL_WOL00")!.NativeBuildingOrLandscapeType);
        Assert.Null(catalog.GetById("LAIR_WOLF_DEN_SMALL"));
        Assert.Throws<NotSupportedException>(() => NeutralLairCatalog.CalculateThreatRating(Blueprint()));
        Assert.Empty(catalog.GetByCategory(LairCategory.BarbarianCamp));
        Assert.Equal(4, catalog.Filter(category: LairCategory.WildAnimal, tier: LairDifficultyTier.Unrated).Count());
        foreach (var animal in catalog.AllDefinitions)
        {
            NeutralLairCatalog.ValidateDefinition(animal);
            Assert.Empty(animal.DefaultGuards); Assert.Empty(animal.WaveRules);
            Assert.Equal(new LairLootReward(), animal.Loot);
            Assert.Equal(0, NeutralLairCatalog.CalculateThreatRating(animal));
        }
        Assert.Empty(NeutralLairCatalog.Default.FilteredBy([new("ALL_WOL00", "FigGerInf01_Schwert")]).AllDefinitions);
    }

    [Theory]
    [InlineData("ALL_WOL00")]
    [InlineData("ALL_BAE00")]
    [InlineData("ALL_EBE00")]
    [InlineData("ALL_RAU00")]
    public void Animal_alias_existence_does_not_imply_timer_spawn_support(string id)
    {
        var d = NeutralLairCatalog.Default.GetById(id)!;
        Assert.Throws<NotSupportedException>(() => Bind(d));
    }

    [Fact]
    public void Authored_timer_uses_real_alias_world_scale_core_condition_and_existing_compiler()
    {
        var d = Blueprint() with { Loot = new(CompletionMessage: "Core removed") };
        Guid core = Guid.NewGuid(); var l = Placed(d, core: core);
        var events = Bind(d, l);
        var timer = Assert.Single(events, e => e.Repeat);
        Assert.Equal(60, timer.DelaySeconds);
        var action = Assert.Single(timer.Actions);
        Assert.Equal(ScenarioActionKind.SpawnUnit, action.Kind);
        Assert.Equal("GER_INF01", action.Alias);
        Assert.Equal(4256f, action.X); Assert.Equal(6000f, action.Z); Assert.Equal(7, action.Team);
        Assert.Equal(new ScenarioCondition(ScenarioConditionKind.ObjectExists, core), Assert.Single(timer.Conditions));
        var cleared = Assert.Single(events, e => !e.Repeat);
        Assert.Equal(new ScenarioCondition(ScenarioConditionKind.ObjectDeadOrRemoved, core), Assert.Single(cleared.Conditions));
        Assert.DoesNotContain(events.SelectMany(e => e.Actions), a => a.Kind == ScenarioActionKind.Diplomacy);
        var scenario = new ScenarioDocument { Spawns = [new("GER_HAU00", 4000, 6000, 7) { Id = core }], Events = events.ToList() };
        ScenarioEventValidator.ValidateConditions(events, scenario);
        var image = BciImage.CreateIdleLevel(); int originalMain = image.MainAddress;
        ScenarioEventCompiler.Inject(image, events, originalMain, scenario);
        var parsed = BciImage.Parse(image.Serialize());
        string constants = Encoding.Latin1.GetString(parsed.ConstBlob);
        Assert.Contains("s_createUnitAndMems", constants);
        Assert.Contains("GER_INF01", constants);
        Assert.Contains("s_showTextBox", constants);
        Assert.DoesNotContain("s_createObj\0", constants);
    }

    [Theory]
    [InlineData("ALL_WOL00")]
    [InlineData("ALL_ZIVMAN00")]
    [InlineData("GER_MIN00")]
    [InlineData("LanGerNad00_Tanne_gross")]
    [InlineData("MISSING")]
    public void Non_troop_or_unknown_alias_is_gated(string alias)
    {
        var d = Blueprint(); d = d with { WaveRules = [d.WaveRules[0] with { UnitAlias = alias }] };
        Assert.Throws<NotSupportedException>(() => Bind(d));
    }

    [Fact]
    public void Unsupported_mechanics_and_invalid_coordinates_do_not_get_silently_clamped()
    {
        var d = Blueprint(); var w = d.WaveRules[0];
        foreach (var bad in new[] { w with { InitialDelaySeconds = 30 }, w with { MaxActiveWaves = 2 }, w with { AggroBehavior = "PatrolHostile" } })
            Assert.Throws<NotSupportedException>(() => Bind(d with { WaveRules = [bad] }));
        Assert.Throws<NotSupportedException>(() => Bind(d with { DefaultGuards = [new("GER_INF01", 1)] }));
        Assert.Throws<NotSupportedException>(() => Bind(d with { Loot = new(Gold: 1) }));
        Assert.Throws<NotSupportedException>(() => Bind(d, Placed(d) with { Team = 8 }));
        Assert.Throws<NotSupportedException>(() => Bind(d with { Loot = new(CompletionMessage: "Cleared") }));
        Assert.Throws<ArgumentNullException>(() => WildLairScenarioEventBinder.GenerateEventsForLair(Placed(d), d));
        Assert.Throws<ArgumentOutOfRangeException>(() => Bind(d, Placed(d) with { WorldX = float.NaN }));
        Assert.Throws<InvalidDataException>(() => Bind(d, Placed(d) with { WorldX = 16300 }));
        Assert.Throws<ArgumentOutOfRangeException>(() => Bind(d with { WaveRules = [w with { SpawnCount = 21 }] }));
        Assert.Throws<ArgumentOutOfRangeException>(() => Bind(d with { WaveRules = [w with { IntervalSeconds = 5 }] }));
    }

    [Fact]
    public void Session_sync_is_validated_before_mutation_and_uses_complete_instance_id()
    {
        var d = Blueprint();
        var one = Placed(d, Guid.Parse("12345678-0000-0000-0000-000000000001"));
        var two = Placed(d, Guid.Parse("12345678-0000-0000-0000-000000000002"));
        var session = new ScenarioEventSession();
        session.Add(new("PLAYER", 10) { Actions = [new(ScenarioActionKind.Message, Text: "Hello")] });
        WildLairScenarioEventBinder.SyncLairEvents(session, one, d, aliases: Aliases);
        WildLairScenarioEventBinder.SyncLairEvents(session, two, d, aliases: Aliases);
        WildLairScenarioEventBinder.SyncLairEvents(session, one, d, aliases: Aliases);
        Assert.Equal(3, session.Count);
        var before = session.Capture().Select(e => e.Name).ToArray();
        Assert.Throws<NotSupportedException>(() => WildLairScenarioEventBinder.SyncLairEvents(session, one,
            d with { Loot = new(Wood: 1) }, aliases: Aliases));
        Assert.Equal(before, session.Capture().Select(e => e.Name));
        Assert.Empty(WildLairScenarioEventBinder.ValidateLairBindings(session.Capture(), [one, two]));
        Assert.Equal(1, WildLairScenarioEventBinder.RemoveLairEvents(session, one.InstanceId));
        Assert.Equal(2, session.Count);
        Assert.Single(WildLairScenarioEventBinder.ValidateLairBindings(session.Capture(), []));
    }

    [Fact]
    public void Capacity_failure_preserves_existing_events()
    {
        var d = Blueprint(); var l = Placed(d); var session = new ScenarioEventSession();
        for (int i = 0; i < 256; i++) session.Add(new($"PLAYER_{i}") { Actions = [new(ScenarioActionKind.Message, Text: "Hello")] });
        Assert.Throws<InvalidOperationException>(() => WildLairScenarioEventBinder.SyncLairEvents(session, l, d, aliases: Aliases));
        Assert.Equal(256, session.Count);
        Assert.All(session.Capture(), e => Assert.StartsWith("PLAYER_", e.Name));
    }

    [Theory]
    [InlineData("LanGerNad00_Tanne_gross")]
    [InlineData("LanBriLau00_Laubbaum_gross")]
    [InlineData("LanGerSte00_1Stein")]
    [InlineData("LanItaWei00_Weizenfeld")]
    [InlineData("BauGerMin00_Mine")]
    [InlineData("BauGerGol00_Goldschmiede")]
    public void Real_resource_requests_are_explicit_and_timed_game_regeneration_is_gated(string name)
    {
        var planner = new ResourceRegenerationPlanner();
        var request = planner.PlanReplacement(name, 4000, 6000, 120);
        Assert.Equal(8, request.Team); Assert.Equal(120, request.DelaySeconds);
        Assert.Throws<NotSupportedException>(() => WildLairScenarioEventBinder.GenerateEventsForResource(request));
    }

    [Fact]
    public void Resource_planner_rejects_guessed_names_and_requires_real_editor_template()
    {
        var p = new ResourceRegenerationPlanner();
        Assert.Throws<ArgumentException>(() => p.PlanReplacement("LanGerTanne01", 1, 1, 60));
        Assert.Throws<ArgumentOutOfRangeException>(() => p.PlanReplacement("LanGerNad00_Tanne_gross", float.PositiveInfinity, 1, 60));
        var r = p.PlanReplacement("LanGerNad00_Tanne_gross", 4000, 6000, 60);
        Assert.Throws<InvalidOperationException>(() => p.CreateEditorAddition(r, _ => null));
        var template = (LevelObjectTemplate)RuntimeHelpers.GetUninitializedObject(typeof(LevelObjectTemplate));
        Assert.Throws<InvalidOperationException>(() => p.CreateEditorAddition(r, _ => template));
        var stone = p.PlanReplacement("LanGerSte00_1Stein", 4000, 6000, 60);
        var addition = p.CreateEditorAddition(stone, name => name == stone.NameDef ? template : null, 150);
        Assert.Equal(stone.NameDef, addition.Name); Assert.Equal(4000, addition.X);
        Assert.Equal(6000, addition.Z); Assert.Equal(150, addition.Y);
        var mine = p.PlanReplacement("BauGerMin00_Mine", 1, 1, 60);
        Assert.Throws<NotSupportedException>(() => p.CreateEditorAddition(mine, _ => template));
    }
}
