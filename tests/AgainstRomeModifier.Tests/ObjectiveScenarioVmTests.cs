using AgainstRomeMapEditor.Modules.Objectives;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

// Reuse the existing bytecode VM rather than simulating the high-level objective state machine.
public sealed partial class ScenarioEventsTests
{
    [Theory]
    [InlineData(false, false, 1)]
    [InlineData(true, false, 0)]
    [InlineData(true, true, 0)]
    public void Objective_survival_checks_loss_before_due_success(bool dead, bool removed, int expected)
    {
        Guid id = Guid.NewGuid(); var scenario = BuildingTargets(id);
        var graph = new ObjectiveDependencyGraph();
        graph.AddObjective(ObjectiveRuleCatalog.CreateSurvival("Survive", "", id, 2));
        var result = BciObjectiveCompiler.CompileToBci(graph, Fixture().Serialize(), scenario, Aliases);
        Assert.True(result.Success, result.Summary);
        var vm = new TestVm(BciImage.Parse(result.BciBytes!));
        vm.Objects[43] = (123, false); vm.Tick(0); vm.Tick(1000); Assert.Empty(vm.MissionResults);
        if (removed) vm.Objects.Clear(); else vm.Objects[43] = (123, dead);
        vm.Tick(2000); vm.Tick(4000);
        Assert.Equal(new[] { expected }, vm.MissionResults); Assert.Equal(1, vm.StackDepth);
    }

    [Theory]
    [InlineData(false, 1)]
    [InlineData(true, 0)]
    public void Objective_escort_corpse_in_destination_cannot_win(bool dead, int expected)
    {
        Guid id = Guid.NewGuid(); var scenario = BuildingTargets(id);
        var graph = new ObjectiveDependencyGraph();
        graph.AddObjective(ObjectiveRuleCatalog.CreateEscortUnit("Arrive", "", id, ObjectiveAreaBounds.FromRectangle(100, 200, 300, 400)));
        var result = BciObjectiveCompiler.CompileToBci(graph, Fixture().Serialize(), scenario, Aliases);
        Assert.True(result.Success, result.Summary);
        var vm = new TestVm(BciImage.Parse(result.BciBytes!));
        vm.Objects[43] = (123, false); vm.Positions[43] = (50, 300); vm.Tick(0);
        Assert.Empty(vm.MissionResults);
        vm.Objects[43] = (123, dead); vm.Positions[43] = (200, 300); vm.Tick(1000); vm.Tick(2000);
        Assert.Equal(new[] { expected }, vm.MissionResults); Assert.Equal(1, vm.StackDepth);
    }

    [Fact]
    public void Objective_destroy_requires_all_targets_and_preserves_existing_events()
    {
        Guid a = Guid.NewGuid(), b = Guid.NewGuid(); var scenario = BuildingTargets(a, b);
        scenario.Events = [Event(0)];
        var graph = new ObjectiveDependencyGraph();
        var destroy = ObjectiveRuleCatalog.CreateDestroyBuilding("Destroy both", "", a);
        graph.AddObjective(destroy with { Parameters = destroy.Parameters with { TargetGuids = [a, b] } });
        var result = BciObjectiveCompiler.CompileToBci(graph, Fixture().Serialize(), scenario, Aliases);
        Assert.True(result.Success, result.Summary);
        var vm = new TestVm(BciImage.Parse(result.BciBytes!));
        vm.Objects[43] = (123, false); vm.Objects[44] = (124, false); vm.Tick(0);
        vm.Objects[43] = (123, true); vm.Tick(1000); Assert.Empty(vm.MissionResults);
        vm.Objects[44] = (124, true); vm.Tick(2000); Assert.Equal(new[] { 1 }, vm.MissionResults);
        Assert.Equal(2, vm.Messages.Count); Assert.Single(scenario.Events);
    }

    [Fact]
    public void Objective_failure_criterion_has_priority_over_simultaneous_primary()
    {
        Guid goal = Guid.NewGuid(), protect = Guid.NewGuid(); var scenario = BuildingTargets(goal, protect);
        var graph = new ObjectiveDependencyGraph();
        graph.AddObjective(ObjectiveRuleCatalog.CreateDestroyBuilding("Destroy", "", goal));
        graph.AddObjective(ObjectiveRuleCatalog.CreateDefendFailureCriterion("Protect", "", protect));
        var result = BciObjectiveCompiler.CompileToBci(graph, Fixture().Serialize(), scenario, Aliases);
        Assert.True(result.Success, result.Summary);
        var vm = new TestVm(BciImage.Parse(result.BciBytes!));
        vm.Objects[43] = (123, false); vm.Objects[44] = (124, false); vm.Tick(0);
        vm.Objects[43] = (123, true); vm.Objects[44] = (124, true); vm.Tick(1000);
        Assert.Equal(new[] { 0 }, vm.MissionResults);
    }

    [Theory]
    [InlineData("ENDL_000", "ak_level.bci")]
    [InlineData("ENDL_005", "ak_level.arm_original")]
    public void Objective_Temp_original_round_trip_and_apply_are_identical(string map, string script)
    {
        string? root = Environment.GetEnvironmentVariable("ARM_OBJECTIVE_SOURCE");
        if (root is null) return; // Explicit read-only TEMP fixture opt-in.
        string source = Path.GetFullPath(root);
        Assert.StartsWith(Path.GetFullPath(Path.GetTempPath()), source, StringComparison.OrdinalIgnoreCase);
        byte[] original = File.ReadAllBytes(Path.Combine(source, map, "SCRIPT", script));
        Guid id = Guid.NewGuid(); var scenario = BuildingTargets(id);
        var graph = new ObjectiveDependencyGraph();
        graph.AddObjective(ObjectiveRuleCatalog.CreateCaptureArea("Reach rectangle", "", ObjectiveAreaBounds.FromRectangle(100, 200, 300, 400), targetGuid: id));
        var result = BciObjectiveCompiler.CompileToBci(graph, original, scenario, Aliases);
        Assert.True(result.Success, string.Join("; ", result.Diagnostics.Select(d => d.Message)));
        byte[] raw = GameLZSS.DecompressPfil(result.BciBytes!);
        Assert.Equal(raw, BciImage.Parse(raw).Serialize());
        Directory.CreateDirectory(Path.Combine(_root, "SCRIPT"));
        File.WriteAllBytes(Path.Combine(_root, "SCRIPT", "ak_level.bci"), original);
        scenario.Events.AddRange(result.CompiledEvents);
        using (var rollback = new FileRollbackScope()) { LevelScriptInjector.Apply(_root, scenario, Aliases, rollback); rollback.Commit(); }
        Assert.Equal(result.BciBytes, File.ReadAllBytes(Path.Combine(_root, "SCRIPT", "ak_level.bci")));
        using (var rollback = new FileRollbackScope()) { LevelScriptInjector.Apply(_root, scenario, Aliases, rollback); rollback.Commit(); }
        Assert.Equal(result.BciBytes, File.ReadAllBytes(Path.Combine(_root, "SCRIPT", "ak_level.bci")));
        Assert.Equal(original, File.ReadAllBytes(Path.Combine(source, map, "SCRIPT", script)));
        string? output = Environment.GetEnvironmentVariable("ARM_OBJECTIVE_DUMP");
        if (output is not null)
        {
            string dump = Path.GetFullPath(output);
            var repo = new DirectoryInfo(AppContext.BaseDirectory);
            while (repo is not null && !File.Exists(Path.Combine(repo.FullName, "AgainstRomeModifier.slnx"))) repo = repo.Parent;
            Assert.NotNull(repo);
            Assert.StartsWith(Path.Combine(repo!.FullName, "TEMP") + Path.DirectorySeparatorChar, dump + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase);
            Directory.CreateDirectory(dump); File.WriteAllBytes(Path.Combine(dump, map + ".bci"), result.BciBytes!);
        }
    }
}
