using AgainstRomeMapEditor.Modules.AI;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests.AI;

public class AiArchetypeTests
{
    private static readonly HashSet<string> KnownTestAliases = new(StringComparer.OrdinalIgnoreCase)
    {
        "GER_INF00", "GER_INF01", "GER_INF02", "GER_INF03", "GER_SCH00", "GER_SCH01", "GER_KAVINF00", "GER_HAU00",
        "ROM_INF00", "ROM_INF01", "ROM_SCH00", "ROM_SCH01", "ROM_KAVINF00", "ROM_HAU00",
        "HUN_INF00", "HUN_INF01", "HUN_SCH00", "HUN_KAVINF00", "HUN_KAVINF01", "HUN_KAVINF02", "HUN_KAVSCH00", "HUN_HAU00",
        "KEL_INF00", "KEL_INF01", "KEL_INF02", "KEL_SCH00", "KEL_SCH01", "KEL_SCH02", "KEL_KAVINF00", "KEL_HAU00",
        "ALL_BAE00", "ALL_EBE00", "ALL_WOL00", "ALL_RAU00", "ALL_PACKPF00", "ALL_ZIVMAN00", "ALL_ZIVWEI00"
    };

    [Fact]
    public void AiArchetypeCatalog_ContainsStandardFourArchetypes()
    {
        var all = AiArchetypeCatalog.All;
        Assert.NotNull(all);
        Assert.True(all.Count >= 4);

        var nomadic = AiArchetypeCatalog.Get(AiArchetypeCatalog.IdNomadicRaider);
        Assert.Equal("Hun", nomadic.PreferredTribe);
        Assert.True(nomadic.Aggressiveness >= 0.8f);
        Assert.Equal(TacticalPosture.HitAndRun, nomadic.TacticalPosture);

        var roman = AiArchetypeCatalog.Get(AiArchetypeCatalog.IdRomanFortress);
        Assert.Equal("Roman", roman.PreferredTribe);
        Assert.True(roman.DefensePriority >= 0.9f);
        Assert.Equal(TacticalPosture.DefensiveHold, roman.TacticalPosture);

        var germanic = AiArchetypeCatalog.Get(AiArchetypeCatalog.IdGermanicSettlement);
        Assert.Equal("German", germanic.PreferredTribe);
        Assert.True(germanic.EconomicFocus >= 0.8f);

        var outpost = AiArchetypeCatalog.Get(AiArchetypeCatalog.IdBarbarianOutpost);
        Assert.Equal(EngagementRule.DefendTerritory, outpost.EngagementRule);
    }

    [Fact]
    public void WaypointPathPlanner_CalculatesDistancesAndProgression()
    {
        var nodes = new List<WaypointNode>
        {
            new(Guid.NewGuid(), 0, 1000f, 1000f),
            new(Guid.NewGuid(), 1, 1000f, 2000f),
            new(Guid.NewGuid(), 2, 2000f, 2000f)
        };

        var pingPongPath = new WaypointPath(
            Id: Guid.NewGuid(),
            Name: "TestPingPong",
            Mode: WaypointMovementMode.PingPong,
            Nodes: nodes,
            AssignedSpawnIds: []);

        pingPongPath.Validate();

        float dist = WaypointPathPlanner.CalculateTotalDistance(pingPongPath);
        Assert.Equal(2000f, dist, precision: 1);

        bool isReversing = false;
        int next = WaypointPathPlanner.GetNextNodeIndex(pingPongPath, 0, ref isReversing);
        Assert.Equal(1, next);
        Assert.False(isReversing);

        next = WaypointPathPlanner.GetNextNodeIndex(pingPongPath, 1, ref isReversing);
        Assert.Equal(2, next);
        Assert.False(isReversing);

        // At end: should reverse
        next = WaypointPathPlanner.GetNextNodeIndex(pingPongPath, 2, ref isReversing);
        Assert.Equal(1, next);
        Assert.True(isReversing);

        // At start while reversing: should turn forward
        next = WaypointPathPlanner.GetNextNodeIndex(pingPongPath, 0, ref isReversing);
        Assert.Equal(1, next);
        Assert.False(isReversing);
    }

    [Fact]
    public void WaypointPathPlanner_LoopProgressionCycles()
    {
        var nodes = new List<WaypointNode>
        {
            new(Guid.NewGuid(), 0, 100f, 100f),
            new(Guid.NewGuid(), 1, 100f, 200f),
            new(Guid.NewGuid(), 2, 200f, 100f)
        };

        var loopPath = new WaypointPath(
            Id: Guid.NewGuid(),
            Name: "TestLoop",
            Mode: WaypointMovementMode.Loop,
            Nodes: nodes,
            AssignedSpawnIds: []);

        loopPath.Validate();

        bool reversing = false;
        Assert.Equal(1, WaypointPathPlanner.GetNextNodeIndex(loopPath, 0, ref reversing));
        Assert.Equal(2, WaypointPathPlanner.GetNextNodeIndex(loopPath, 1, ref reversing));
        Assert.Equal(0, WaypointPathPlanner.GetNextNodeIndex(loopPath, 2, ref reversing));
    }

    [Fact]
    public void CampaignMissionCompiler_CompilesObjectivesAndWavesToValidEvents()
    {
        var targetBuildingId = Guid.NewGuid();
        var targetGeneralId = Guid.NewGuid();

        var plan = new CampaignMissionPlan(
            Title: "Siege of Aquileia",
            Briefing: "Protect our town center and withstand the Hun attacks!",
            Objectives:
            [
                new CampaignObjective(Guid.NewGuid(), CampaignObjectiveType.DefendTarget, "保護城鎮中心", "城鎮中心不可被毀", TargetId: targetBuildingId),
                new CampaignObjective(Guid.NewGuid(), CampaignObjectiveType.DestroyTarget, "斬首敵軍酋長", "擊殺敵軍騎兵統帥", TargetId: targetGeneralId),
                new CampaignObjective(Guid.NewGuid(), CampaignObjectiveType.SurviveTime, "堅守至黎明", "生存 1200 秒", RequiredSeconds: 1200)
            ],
            Waves:
            [
                new WaveAttackDefinition(
                    WaveIndex: 1,
                    TriggerDelaySeconds: 60,
                    Announcement: "Enemy vanguard approaching!",
                    SpawnX: 2000f,
                    SpawnZ: 2000f,
                    Squads: [new WaveSquadDefinition("HUN_KAVINF00", Count: 10, Team: 2)]),
                new WaveAttackDefinition(
                    WaveIndex: 2,
                    TriggerDelaySeconds: 180,
                    Announcement: "The second enemy wave is approaching!",
                    SpawnX: 2100f,
                    SpawnZ: 2100f,
                    Squads: [new WaveSquadDefinition("HUN_KAVSCH00", Count: 12, Team: 2)])
            ],
            Reinforcements:
            [
                new ReinforcementDefinition(
                    Id: Guid.NewGuid(),
                    Name: "羅馬同盟軍援軍",
                    TriggerDelaySeconds: 300,
                    Team: 0,
                    SpawnX: 500f,
                    SpawnZ: 500f,
                    Squads: [new WaveSquadDefinition("ROM_INF01", Count: 15, Team: 0)],
                    NotificationText: "Roman reinforcements arrived in the southwest!")
            ],
            FactionProfiles:
            [
                new FactionAiProfile(Team: 2, ArchetypeId: AiArchetypeCatalog.IdNomadicRaider)
            ]);

        // Mock existing scenario with the target objects
        var scenario = new ScenarioDocument
        {
            Spawns =
            [
                new ScenarioSpawn("GER_HAU00", 5000f, 5000f, Team: 0, Count: 0, Prebuilt: true) { Id = targetBuildingId },
                new ScenarioSpawn("HUN_KAVINF00", 3000f, 3000f, Team: 2, Count: 1, Prebuilt: false) { Id = targetGeneralId }
            ],
            DataSlots =
            [
                new ScenarioDataSlot(10, 5001) { SpawnId = targetBuildingId }
            ]
        };

        var result = CampaignMissionCompiler.Compile(plan, null, KnownTestAliases, scenario);

        Assert.True(result.Success, string.Join(Environment.NewLine, result.Diagnostics));
        Assert.NotNull(result.CompiledEvents);
        Assert.NotEmpty(result.CompiledEvents);

        // Must validate with vanilla ScenarioEventValidator
        ScenarioEventValidator.Validate(result.CompiledEvents, KnownTestAliases);
        ScenarioEventValidator.ValidateConditions(result.CompiledEvents, scenario);
        ScenarioEventValidator.ValidateTerminalActions(result.CompiledEvents);

        // Verify Briefing event
        Assert.Contains(result.CompiledEvents, e => e.Name.StartsWith("MissionBriefing", StringComparison.Ordinal));

        // Verify Wave events
        Assert.Contains(result.CompiledEvents, e => e.Name.StartsWith("Wave_01", StringComparison.Ordinal) && e.DelaySeconds == 60);
        Assert.Contains(result.CompiledEvents, e => e.Name.StartsWith("Wave_02", StringComparison.Ordinal) && e.DelaySeconds == 180);

        // Verify Reinforcement event
        Assert.Contains(result.CompiledEvents, e => e.Name.StartsWith("Reinf_", StringComparison.Ordinal) && e.DelaySeconds == 300);

        // Verify DefendFail event (Defeat action)
        var defendFail = Assert.Single(result.CompiledEvents, e => e.Name.StartsWith("DefendFail_", StringComparison.Ordinal));
        Assert.Contains(defendFail.Actions, a => a.Kind == ScenarioActionKind.Defeat);

        // Verify DestroyWin event (Victory action)
        var destroyWin = Assert.Single(result.CompiledEvents, e => e.Name.StartsWith("DestroyWin_", StringComparison.Ordinal));
        Assert.Contains(destroyWin.Actions, a => a.Kind == ScenarioActionKind.Victory);

        // Verify SurviveWin event (Victory action)
        var surviveWin = Assert.Single(result.CompiledEvents, e => e.Name.StartsWith("SurviveWin_", StringComparison.Ordinal));
        Assert.Equal(1200, surviveWin.DelaySeconds);
        Assert.Contains(surviveWin.Actions, a => a.Kind == ScenarioActionKind.Victory);
    }

    [Theory]
    [InlineData(CampaignObjectiveType.ReachArea)]
    [InlineData(CampaignObjectiveType.WaveSurvival)]
    public void CampaignMissionCompiler_GeneratedMessages_UseNativeSupportedText(CampaignObjectiveType type)
    {
        var targetId = Guid.NewGuid();
        var plan = new CampaignMissionPlan("Campaign", "",
            [new CampaignObjective(Guid.NewGuid(), type, "目標", "", TargetId: targetId, RequiredSeconds: 60)],
            [], [], []);
        var result = CampaignMissionCompiler.Compile(plan, null, KnownTestAliases);
        Assert.True(result.Success, string.Join(Environment.NewLine, result.Diagnostics));
        var compiledEvent = Assert.Single(result.CompiledEvents);
        Assert.Contains(compiledEvent.Actions, a => a.Kind == ScenarioActionKind.Message);
        Assert.Contains(compiledEvent.Actions, a => a.Kind == ScenarioActionKind.Victory);
        ScenarioEventValidator.Validate(result.CompiledEvents, KnownTestAliases);
    }

    [Fact]
    public void CampaignMissionCompiler_UnsupportedBriefing_ReturnsFailure()
    {
        var plan = new CampaignMissionPlan("Campaign", "中文簡報", [], [], [], []);
        var result = CampaignMissionCompiler.Compile(plan, null, KnownTestAliases);
        Assert.False(result.Success);
        Assert.Contains(result.Diagnostics, d => d.StartsWith("[Error]", StringComparison.Ordinal));
        Assert.Equal(plan.Briefing, Assert.Single(result.CompiledEvents).Actions[0].Text);
    }
    [Fact]
    public void FactionAiSession_TracksDirtyAndMaintainsBaseline()
    {
        var session = new FactionAiSession();
        Assert.False(session.IsDirty);

        var path = new WaypointPath(
            Id: Guid.NewGuid(),
            Name: "Patrol Alpha",
            Mode: WaypointMovementMode.Loop,
            Nodes:
            [
                new(Guid.NewGuid(), 0, 100f, 100f),
                new(Guid.NewGuid(), 1, 200f, 200f)
            ],
            AssignedSpawnIds: []);

        session.AddWaypointPath(path);
        Assert.True(session.IsDirty);

        session.AcceptChanges();
        Assert.False(session.IsDirty);

        session.RemoveWaypointPath(path.Id);
        Assert.True(session.IsDirty);

        session.Reset();
        Assert.False(session.IsDirty);
        Assert.NotNull(session.FindPath(path.Id));
    }

    [Fact]
    public void AiArchetypeCatalog_AllProfiles_UseRealAliasesInKnownCatalog()
    {
        var allProfiles = AiArchetypeCatalog.All;
        Assert.NotEmpty(allProfiles);

        foreach (var profile in allProfiles)
        {
            Assert.NotEmpty(profile.UnitPreferences);
            foreach (var pref in profile.UnitPreferences)
            {
                Assert.True(pref.IsValid, $"Profile {profile.Id} has invalid preference: {pref.Alias}");
                Assert.Contains(pref.Alias, KnownTestAliases);
            }
        }
    }

    [Fact]
    public void CampaignMissionCompiler_ValidatesCoordinateBounds()
    {
        // 1 editor tile = 256 world units, 1 collision pixel = 64 world units. Max coordinate = 16383.
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            new WaveAttackDefinition(
                WaveIndex: 1,
                TriggerDelaySeconds: 10,
                Announcement: "Test",
                SpawnX: 16384f,
                SpawnZ: 1000f,
                Squads: [new WaveSquadDefinition("GER_INF01", 10, 0)]).Validate());

        Assert.Throws<ArgumentOutOfRangeException>(() =>
            new WaveAttackDefinition(
                WaveIndex: 1,
                TriggerDelaySeconds: 10,
                Announcement: "Test",
                SpawnX: -1f,
                SpawnZ: 1000f,
                Squads: [new WaveSquadDefinition("GER_INF01", 10, 0)]).Validate());

        Assert.Throws<ArgumentOutOfRangeException>(() =>
            new ReinforcementDefinition(
                Id: Guid.NewGuid(),
                Name: "OutOfBounds",
                TriggerDelaySeconds: 10,
                Team: 0,
                SpawnX: 500f,
                SpawnZ: 16384f,
                Squads: [new WaveSquadDefinition("GER_INF01", 10, 0)]).Validate());

        Assert.Throws<ArgumentOutOfRangeException>(() =>
            new WaypointNode(Guid.NewGuid(), 0, WorldX: 16384f, WorldZ: 0f).Validate());
    }

    [Fact]
    public void CampaignMissionCompiler_CompiledActions_HaveValidCoordinatesAndSupportedKinds()
    {
        var plan = new CampaignMissionPlan(
            Title: "BoundsTest",
            Briefing: "Testing world units bounds.",
            Objectives:
            [
                new CampaignObjective(Guid.NewGuid(), CampaignObjectiveType.ReachArea, "Reach", "",
                    TargetId: Guid.NewGuid(),
                    TargetMinX: 0, TargetMinZ: 0, TargetMaxX: 16383, TargetMaxZ: 16383)
            ],
            Waves:
            [
                new WaveAttackDefinition(1, 10, "Wave 1", SpawnX: 64f, SpawnZ: 256f,
                    Squads: [new WaveSquadDefinition("GER_INF00", 10, 1)])
            ],
            Reinforcements:
            [
                new ReinforcementDefinition(Guid.NewGuid(), "Reinf 1", 20, 0, SpawnX: 16000f, SpawnZ: 16383f,
                    Squads: [new WaveSquadDefinition("ROM_INF00", 15, 0)])
            ],
            FactionProfiles: []);

        var result = CampaignMissionCompiler.Compile(plan, null, KnownTestAliases);
        Assert.True(result.Success, string.Join(Environment.NewLine, result.Diagnostics));

        foreach (var ev in result.CompiledEvents)
        {
            Assert.True(ev.DelaySeconds is >= 0 and <= 86400);
            foreach (var action in ev.Actions)
            {
                Assert.True(Enum.IsDefined(action.Kind));
                if (action.Kind == ScenarioActionKind.SpawnUnit)
                {
                    Assert.InRange(action.X, 0f, 16383f);
                    Assert.InRange(action.Z, 0f, 16383f);
                    Assert.Contains(action.Alias, KnownTestAliases);
                    Assert.InRange(action.Count, 1, 20);
                    Assert.InRange(action.Team, 0, 7);
                }
            }
        }
    }

    [Fact]
    public void TempGameCompare_AllInlineAliases_ExistInClScintIni()
    {
        string? tempPath = Environment.GetEnvironmentVariable("TEMP");
        if (string.IsNullOrEmpty(tempPath)) return;
        string gamePath = Path.Combine(tempPath, "ArmGameCompare_20261007");
        if (!Directory.Exists(gamePath)) return;

        var gameAliases = ScriptObjectAliases.Load(gamePath);
        Assert.NotEmpty(gameAliases);
        var aliasSet = gameAliases.Select(a => a.Alias).ToHashSet(StringComparer.OrdinalIgnoreCase);

        foreach (var inlineAlias in KnownTestAliases)
        {
            Assert.Contains(inlineAlias, aliasSet);
        }
    }
}
