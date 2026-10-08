using AgainstRomeMapEditor.Modules.AI;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests.AI;

public class AiArchetypeTests
{
    private static readonly HashSet<string> KnownTestAliases = new(StringComparer.OrdinalIgnoreCase)
    {
        "GER_INF01", "GER_ARC01", "GER_CAV01",
        "ROM_INF01", "ROM_INF02", "ROM_ARC01",
        "HUN_CAV01", "HUN_ARC01", "BauGerHau00"
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
            Briefing: "守護我方城鎮中心，抵擋北方遊牧匈人的四波進攻！",
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
                    Announcement: "敵軍先鋒部隊出現！",
                    SpawnX: 2000f,
                    SpawnZ: 2000f,
                    Squads: [new WaveSquadDefinition("HUN_CAV01", Count: 10, Team: 2)]),
                new WaveAttackDefinition(
                    WaveIndex: 2,
                    TriggerDelaySeconds: 180,
                    Announcement: "敵軍第二波大軍逼近！",
                    SpawnX: 2100f,
                    SpawnZ: 2100f,
                    Squads: [new WaveSquadDefinition("HUN_ARC01", Count: 12, Team: 2)])
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
                    NotificationText: "羅馬軍團援軍已抵達西南側！")
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
                new ScenarioSpawn("BauGerHau00", 5000f, 5000f, Team: 0, Count: 0, Prebuilt: true) { Id = targetBuildingId },
                new ScenarioSpawn("HUN_CAV01", 3000f, 3000f, Team: 2, Count: 1, Prebuilt: false) { Id = targetGeneralId }
            ],
            DataSlots =
            [
                new ScenarioDataSlot(10, 5001) { SpawnId = targetBuildingId }
            ]
        };

        var result = CampaignMissionCompiler.Compile(plan, null, KnownTestAliases, scenario);

        Assert.True(result.Success);
        Assert.NotNull(result.CompiledEvents);
        Assert.NotEmpty(result.CompiledEvents);

        // Must validate with vanilla ScenarioEventValidator
        ScenarioEventValidator.Validate(result.CompiledEvents, KnownTestAliases);
        ScenarioEventValidator.ValidateConditions(result.CompiledEvents, scenario);
        ScenarioEventValidator.ValidateTerminalActions(result.CompiledEvents);

        // Verify Briefing event
        Assert.Contains(result.CompiledEvents, e => e.Name.StartsWith("MissionBriefing"));

        // Verify Wave events
        Assert.Contains(result.CompiledEvents, e => e.Name.StartsWith("Wave_01") && e.DelaySeconds == 60);
        Assert.Contains(result.CompiledEvents, e => e.Name.StartsWith("Wave_02") && e.DelaySeconds == 180);

        // Verify Reinforcement event
        Assert.Contains(result.CompiledEvents, e => e.Name.StartsWith("Reinf_") && e.DelaySeconds == 300);

        // Verify DefendFail event (Defeat action)
        var defendFail = Assert.Single(result.CompiledEvents, e => e.Name.StartsWith("DefendFail_"));
        Assert.Contains(defendFail.Actions, a => a.Kind == ScenarioActionKind.Defeat);

        // Verify DestroyWin event (Victory action)
        var destroyWin = Assert.Single(result.CompiledEvents, e => e.Name.StartsWith("DestroyWin_"));
        Assert.Contains(destroyWin.Actions, a => a.Kind == ScenarioActionKind.Victory);

        // Verify SurviveWin event (Victory action)
        var surviveWin = Assert.Single(result.CompiledEvents, e => e.Name.StartsWith("SurviveWin_"));
        Assert.Equal(1200, surviveWin.DelaySeconds);
        Assert.Contains(surviveWin.Actions, a => a.Kind == ScenarioActionKind.Victory);
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
}
