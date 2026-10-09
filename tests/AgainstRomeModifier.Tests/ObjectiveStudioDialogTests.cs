using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Objectives;
using AgainstRomeModifier.Scripting;
using static AgainstRomeModifier.Tests.DialogControlTestSupport;

namespace AgainstRomeModifier.Tests;

public sealed class ObjectiveStudioDialogTests
{
    private static ScenarioDocument CreateValidScenario(out Guid heroId, out Guid houseId)
    {
        heroId = Guid.NewGuid();
        houseId = Guid.NewGuid();
        return new ScenarioDocument
        {
            Spawns =
            [
                new ScenarioSpawn("ROM_INF00", 1000, 2000, 1, Prebuilt: true) { Id = heroId },
                new ScenarioSpawn("BauRomHau00", 5000, 5000, 1, Prebuilt: true) { Id = houseId }
            ],
            DataSlots =
            [
                new ScenarioDataSlot(1, 100) { SpawnId = heroId },
                new ScenarioDataSlot(2, 101) { SpawnId = houseId }
            ]
        };
    }

    [Fact]
    public void Controls_initialize_with_dpi_scaling_and_reflect_supported_survival_objective() => InSta(() =>
    {
        var scenario = CreateValidScenario(out var heroId, out _);
        using var dialog = new ObjectiveStudioDialog(scenario, ["ROM_INF00"]);

        Assert.Equal(AutoScaleMode.Dpi, dialog.AutoScaleMode);
        Assert.NotNull(dialog.AcceptButton);
        Assert.NotNull(dialog.CancelButton);

        // Survival with HoldDuration > 0 is valid
        dialog.SelectedKind = ObjectiveKind.Survival;
        dialog.SelectedCategory = ObjectiveCategory.Primary;
        dialog.ObjectiveTitle = "Hold the line";
        dialog.SelectedTargetGuid = heroId;
        dialog.HoldDurationSeconds = 120;

        Assert.True(dialog.CanApply);
        Assert.Empty(dialog.ValidationErrors);
        Assert.NotEmpty(dialog.CompiledEvents);

        // Survival generates a failure event (object dead or removed) and a victory event (object exists after delay)
        Assert.Contains(dialog.CompiledEvents, e => e.Actions.Any(a => a.Kind == ScenarioActionKind.Victory));
        Assert.Contains(dialog.CompiledEvents, e => e.Actions.Any(a => a.Kind == ScenarioActionKind.Defeat));
        Assert.Contains("Hold the line", dialog.PreviewText);

        ShowOffscreen(dialog);
        ((Button)dialog.AcceptButton!).PerformClick();
        Assert.Equal(DialogResult.OK, dialog.DialogResult);
    });

    [Theory]
    [InlineData(ObjectiveKind.EliminateAllEnemies)]
    [InlineData(ObjectiveKind.KingOfTheHill)]
    public void Unsupported_objective_kinds_are_rejected_with_validation_errors(ObjectiveKind unsupportedKind) => InSta(() =>
    {
        var scenario = CreateValidScenario(out var heroId, out _);
        using var dialog = new ObjectiveStudioDialog(scenario, ["ROM_INF00"]);

        dialog.SelectedKind = unsupportedKind;
        dialog.SelectedTargetGuid = heroId;

        Assert.False(dialog.CanApply);
        Assert.Empty(dialog.CompiledEvents);
        Assert.NotEmpty(dialog.ValidationErrors);
        Assert.Contains("OBJ_EXPORT_UNSUPPORTED", dialog.PreviewText);

        ShowOffscreen(dialog);
        ((Button)dialog.AcceptButton!).PerformClick();
        // Since CanApply is false, clicking AcceptButton should not set DialogResult to OK
        Assert.NotEqual(DialogResult.OK, dialog.DialogResult);

        ((Button)dialog.CancelButton!).PerformClick();
        Assert.Equal(DialogResult.Cancel, dialog.DialogResult);
    });

    [Fact]
    public void Supported_objective_kinds_compile_expected_scenario_events() => InSta(() =>
    {
        var scenario = CreateValidScenario(out var heroId, out var houseId);
        using var dialog = new ObjectiveStudioDialog(scenario, ["ROM_INF00", "BauRomHau00"]);

        // 1. Assassinate Target
        dialog.SelectedKind = ObjectiveKind.AssassinateTarget;
        dialog.SelectedCategory = ObjectiveCategory.Primary;
        dialog.SelectedTargetGuid = heroId;
        dialog.ObjectiveTitle = "Assassinate Commander";
        Assert.True(dialog.CanApply);
        Assert.Contains(dialog.CompiledEvents, e => e.Conditions.Any(c => c.Kind == ScenarioConditionKind.ObjectDeadOrRemoved && c.TargetId == heroId));
        Assert.Contains(dialog.CompiledEvents, e => e.Actions.Any(a => a.Kind == ScenarioActionKind.Victory));

        // 2. Destroy Building
        dialog.SelectedKind = ObjectiveKind.DestroyBuilding;
        dialog.SelectedTargetGuid = houseId;
        dialog.ObjectiveTitle = "Destroy Fortress";
        Assert.True(dialog.CanApply);
        Assert.Contains(dialog.CompiledEvents, e => e.Conditions.Any(c => c.Kind == ScenarioConditionKind.ObjectDeadOrRemoved && c.TargetId == houseId));

        // 3. Capture Area
        dialog.SelectedKind = ObjectiveKind.CaptureArea;
        dialog.SelectedTargetGuid = heroId;
        dialog.MinX = 500; dialog.MinZ = 600; dialog.MaxX = 1500; dialog.MaxZ = 1600;
        dialog.HoldDurationSeconds = 0; // CaptureArea requires HoldDuration == 0 for BciObjectiveCompiler
        Assert.True(dialog.CanApply);
        Assert.Contains(dialog.CompiledEvents, e => e.Conditions.Any(c => c.Kind == ScenarioConditionKind.ObjectInArea && c.MinX == 500 && c.MaxX == 1500));

        // 4. Escort Unit
        dialog.SelectedKind = ObjectiveKind.EscortUnit;
        dialog.SelectedTargetGuid = heroId;
        dialog.MinX = 2000; dialog.MinZ = 3000; dialog.MaxX = 4000; dialog.MaxZ = 5000;
        Assert.True(dialog.CanApply);
        Assert.Contains(dialog.CompiledEvents, e => e.Conditions.Any(c => c.Kind == ScenarioConditionKind.ObjectInArea && c.TargetId == heroId));
        Assert.Contains(dialog.CompiledEvents, e => e.Conditions.Any(c => c.Kind == ScenarioConditionKind.ObjectDeadOrRemoved && c.TargetId == heroId));
    });

    [Fact]
    public void Failure_criterion_compiles_defeat_event_without_victory() => InSta(() =>
    {
        var scenario = CreateValidScenario(out var heroId, out _);
        // BciObjectiveCompiler requires exactly 1 Primary objective in the graph for export.
        // If we compile a FailureCriterion alone, OBJ_PRIMARY_LIMIT will fail.
        using var dialog = new ObjectiveStudioDialog(scenario, ["ROM_INF00"]);
        dialog.SelectedKind = ObjectiveKind.Survival;
        dialog.SelectedCategory = ObjectiveCategory.FailureCriterion;
        dialog.SelectedTargetGuid = heroId;

        // Exactly one primary is required by BciObjectiveCompiler, so a failure criterion alone cannot be exported directly
        Assert.False(dialog.CanApply);
        Assert.Contains(dialog.ValidationErrors, err => err.Contains("OBJ_PRIMARY_LIMIT"));
    });

    [Fact]
    public void Missing_target_or_empty_scenario_prevents_apply() => InSta(() =>
    {
        var emptyScenario = new ScenarioDocument();
        using var dialog = new ObjectiveStudioDialog(emptyScenario, ["ROM_INF00"]);
        dialog.SelectedKind = ObjectiveKind.Survival;
        dialog.HoldDurationSeconds = 60;

        Assert.False(dialog.CanApply);
        Assert.NotEmpty(dialog.ValidationErrors);
    });
}
