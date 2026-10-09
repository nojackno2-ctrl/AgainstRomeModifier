using AgainstRomeMapEditor.Modules.Objectives;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests.Objectives;

public sealed class ObjectiveCompilationTests
{
    private static ObjectiveDependencyGraph Graph(ObjectiveDefinition objective)
    {
        var graph = new ObjectiveDependencyGraph(); graph.AddObjective(objective); return graph;
    }
    private static ObjectiveDefinition Target(ObjectiveKind kind) => new()
    {
        Title = "Goal", Kind = kind,
        Parameters = new() { TargetGuids = [Guid.NewGuid()], HoldDurationSeconds = kind == ObjectiveKind.Survival ? 10 : 0,
            Area = kind is ObjectiveKind.CaptureArea or ObjectiveKind.EscortUnit ? ObjectiveAreaBounds.FromRectangle(100, 200, 300, 400) : null,
            CustomConditions = kind == ObjectiveKind.CustomScripted ? [new(ScenarioConditionKind.ObjectExists, Guid.NewGuid())] : [] }
    };

    [Theory]
    [InlineData(ObjectiveKind.AssassinateTarget)]
    [InlineData(ObjectiveKind.DestroyBuilding)]
    [InlineData(ObjectiveKind.Survival)]
    [InlineData(ObjectiveKind.EscortUnit)]
    [InlineData(ObjectiveKind.CaptureArea)]
    [InlineData(ObjectiveKind.CustomScripted)]
    public void Supported_rules_validate_and_round_trip_through_existing_compiler(ObjectiveKind kind)
    {
        var objective = Target(kind);
        if (kind == ObjectiveKind.CustomScripted) objective = objective with { Parameters = objective.Parameters with { TargetGuids = [] } };
        var ids = objective.Parameters.TargetGuids.Concat(objective.Parameters.CustomConditions.Select(c => c.TargetId)).ToArray();
        var scenario = new ScenarioDocument
        {
            Spawns = ids.Select(id => new ScenarioSpawn("HOUSE", 0, 0, 0, Prebuilt: true) { Id = id }).ToList(),
            DataSlots = ids.Select((id, i) => new ScenarioDataSlot(i + 42, (uint)(i + 123)) { SpawnId = id }).ToList()
        };
        byte[] original = BciImage.CreateIdleLevel().Serialize();
        var result = BciObjectiveCompiler.CompileToBci(Graph(objective), original, scenario, []);
        Assert.True(result.Success, string.Join("; ", result.Diagnostics.Select(d => d.Message)));
        ScenarioEventValidator.Validate(result.CompiledEvents, []);
        ScenarioEventValidator.ValidateConditions(result.CompiledEvents, scenario);
        Assert.NotNull(result.BciBytes);
        var parsed = BciImage.Parse(result.BciBytes!);
        Assert.Equal(result.BciBytes, parsed.Serialize());
        Assert.Equal(original, BciImage.CreateIdleLevel().Serialize());
        Assert.Empty(scenario.Events);
        var constants = Enumerable.Range(0, parsed.ConstOffsets.Count).Select(parsed.Constant).ToArray();
        Assert.Contains("GLOBAL_MISSION_RESULT", constants); Assert.Contains("s_quitGame", constants);
        Assert.DoesNotContain("s_lgcSetMissionResult", constants);
    }

    [Theory]
    [InlineData(ObjectiveKind.EliminateAllEnemies)]
    [InlineData(ObjectiveKind.KingOfTheHill)]
    public void Unverified_kinds_never_emit_events(ObjectiveKind kind) => Rejected(Target(kind));

    [Fact]
    public void Unsupported_parameters_are_never_silently_approximated()
    {
        var capture = Target(ObjectiveKind.CaptureArea);
        Rejected(capture with { Parameters = capture.Parameters with { HoldDurationSeconds = 3 } });
        Rejected(capture with { Parameters = capture.Parameters with { TargetGuids = [] } });
        Rejected(capture with { Parameters = capture.Parameters with { Area = ObjectiveAreaBounds.FromCircle(200, 300, 20) } });
        Rejected(capture with { Parameters = capture.Parameters with { TimeLimitSeconds = 10 } });
        Rejected(capture with { Parameters = capture.Parameters with { RequiredCount = 2 } });
        Rejected(capture with { Parameters = capture.Parameters with { CustomData = "resource:gold" } });
        Rejected(capture with { InitialState = ObjectiveState.Hidden });
        Rejected(Target(ObjectiveKind.Survival) with { Category = ObjectiveCategory.Bonus });
        Rejected(Target(ObjectiveKind.EscortUnit) with { Category = ObjectiveCategory.Bonus });
        Rejected(Target(ObjectiveKind.DestroyBuilding) with { Category = ObjectiveCategory.FailureCriterion });
    }

    [Fact]
    public void Lists_are_not_truncated_and_rewards_cannot_bypass_validation()
    {
        var destroy = Target(ObjectiveKind.DestroyBuilding);
        Rejected(destroy with { Parameters = destroy.Parameters with { TargetGuids = Enumerable.Range(0, 33).Select(_ => Guid.NewGuid()).ToArray() } });
        Rejected(destroy with { Reward = new("", [new(ScenarioActionKind.Victory)]) });
        Rejected(destroy with { Reward = new("", [new(ScenarioActionKind.SpawnUnit, Alias: "UNKNOWN")]) });
        Rejected(destroy with { Reward = new("", Enumerable.Repeat(new ScenarioAction(ScenarioActionKind.Message, "Reward"), 31).ToArray()) });
        Rejected(destroy with { Title = "不可編碼" });
        var custom = Target(ObjectiveKind.CustomScripted);
        Rejected(custom with { Parameters = new() });
        Rejected(custom with { Parameters = new() { CustomConditions = [new((ScenarioConditionKind)99, Guid.NewGuid())] } });
        Rejected(destroy with { Kind = (ObjectiveKind)99 });
        Rejected(destroy with { Id = Guid.Empty });
    }

    [Theory]
    [InlineData(DependencyRelation.Prerequisite)]
    [InlineData(DependencyRelation.CompositeAnd)]
    [InlineData(DependencyRelation.CompositeOr)]
    [InlineData(DependencyRelation.MutuallyExclusive)]
    public void Dependencies_and_multiple_primaries_are_gated(DependencyRelation relation)
    {
        var a = Target(ObjectiveKind.DestroyBuilding); var b = Target(ObjectiveKind.AssassinateTarget);
        var graph = Graph(a); graph.AddObjective(b);
        Assert.False(BciObjectiveCompiler.Compile(graph).Success);
        graph.AddDependency(a.Id, b.Id, relation);
        var result = BciObjectiveCompiler.Compile(graph);
        Assert.False(result.Success); Assert.Empty(result.CompiledEvents);
        Assert.Contains(result.Diagnostics, d => d.Code == "OBJ_DEPENDENCY_UNSUPPORTED");
    }

    [Fact]
    public void Known_reward_actions_preserve_order_and_obey_shared_validation()
    {
        var objective = Target(ObjectiveKind.DestroyBuilding) with
        {
            Reward = new("Reinforcements", [new(ScenarioActionKind.SpawnUnit, Alias: "GER_INF01", Count: 5),
                new(ScenarioActionKind.Diplomacy, Team: 0, OtherTeam: 1, Hostile: false)])
        };
        var result = BciObjectiveCompiler.Compile(Graph(objective), knownAliases: ["GER_INF01"]);
        Assert.True(result.Success, result.Summary);
        Assert.Equal(new[] { ScenarioActionKind.Message, ScenarioActionKind.Message, ScenarioActionKind.SpawnUnit,
            ScenarioActionKind.Diplomacy, ScenarioActionKind.Victory }, result.CompiledEvents.Single().Actions.Select(a => a.Kind));
        Assert.Contains(result.Diagnostics, d => d.Code == "OBJ_BINDING_UNCHECKED");
        ScenarioEventValidator.Validate(result.CompiledEvents, ["GER_INF01"]);
    }

    [Fact]
    public void Missing_binding_bad_original_and_combined_event_limit_return_no_output()
    {
        var objective = Target(ObjectiveKind.DestroyBuilding); var graph = Graph(objective);
        var scenario = new ScenarioDocument();
        byte[] original = BciImage.CreateIdleLevel().Serialize();
        Assert.False(BciObjectiveCompiler.CompileToBci(graph, original, scenario, []).Success);
        var id = objective.Parameters.TargetGuids.Single();
        scenario.Spawns.Add(new("HOUSE", 0, 0, 0, Prebuilt: true) { Id = id });
        Assert.False(BciObjectiveCompiler.CompileToBci(graph, original, scenario, []).Success);
        scenario.DataSlots.Add(new(42, 123) { SpawnId = id });
        Assert.Null(BciObjectiveCompiler.CompileToBci(graph, new byte[64], scenario, []).BciBytes);
        scenario.Events = Enumerable.Range(0, 256).Select(i => new ScenarioEvent($"E{i}") { Actions = [new(ScenarioActionKind.Message, "Message")] }).ToList();
        Assert.False(BciObjectiveCompiler.CompileToBci(graph, original, scenario, []).Success);
    }

    [Fact]
    public void Ambiguous_identity_or_shared_physical_slot_is_rejected_without_mutating_scenario()
    {
        var objective = Target(ObjectiveKind.DestroyBuilding); var graph = Graph(objective);
        Guid a = objective.Parameters.TargetGuids.Single(), b = Guid.NewGuid();
        var scenario = new ScenarioDocument
        {
            Spawns = [new("HOUSE", 0, 0, 0, Prebuilt: true) { Id = a }, new("HOUSE", 1, 1, 0, Prebuilt: true) { Id = b }],
            DataSlots = [new(42, 123) { SpawnId = a }, new(42, 124) { SpawnId = b }]
        };
        var spawns = scenario.Spawns; var slots = scenario.DataSlots;
        Assert.False(BciObjectiveCompiler.Compile(graph, scenario).Success);
        Assert.Same(spawns, scenario.Spawns); Assert.Same(slots, scenario.DataSlots);
        scenario.DataSlots.RemoveAt(1); scenario.Spawns[1] = scenario.Spawns[1] with { Id = a };
        Assert.False(BciObjectiveCompiler.Compile(graph, scenario).Success);
    }

    [Fact]
    public void Sandbox_reports_export_rejection_even_when_offline_model_wins()
    {
        var graph = Graph(ObjectiveRuleCatalog.CreateEliminateEnemies("Team"));
        var sandbox = new ObjectiveSandboxSession(); sandbox.Initialize(graph);
        sandbox.SimulateTeamUnitCount(1, 0);
        Assert.Equal(CampaignResult.Victory, sandbox.CampaignStatus);
        Assert.Contains(sandbox.ExportDiagnostics, d => d.Severity == ObjectiveDiagnosticSeverity.Error);
    }

    private static void Rejected(ObjectiveDefinition objective)
    {
        var result = BciObjectiveCompiler.Compile(Graph(objective));
        Assert.False(result.Success); Assert.Empty(result.CompiledEvents); Assert.Null(result.BciBytes);
        Assert.Contains(result.Diagnostics, d => d.Severity == ObjectiveDiagnosticSeverity.Error);
    }
}
