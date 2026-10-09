using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.WildLair;
using AgainstRomeModifier.Scripting;
using static AgainstRomeModifier.Tests.DialogControlTestSupport;

namespace AgainstRomeModifier.Tests;

public sealed class WildLairDialogTests
{
    private static IReadOnlyList<ScriptObjectAlias> Aliases => ScriptObjectAliases.Parse("""
        [ObjDefName]
        ALL_ZIVMAN00=FigZivMan00_Zivilist
        ALL_ZIVWEI00=FigZivWei00_Zivilistin
        ALL_PACKPF00=FigTiePac00_Packpferd
        ALL_BAE00=FigTieBae00_Baer
        ALL_RAU00=FigTieRau00_Raubkatze
        ALL_WOL00=FigTieWol00_Wilder_Wolf
        ALL_EBE00=FigTieEbe00_Wildschwein
        GER_INF00=FigGerInf00_Keule
        GER_INF01=FigGerInf01_Schwert
        ROM_INF01=FigRomInf01_Schwert_Schild
        GER_HAU00=BauGerHau00_Haupthaus
        """);

    [Fact]
    public void Dpi_scaling_and_catalog_definitions_populate_controls() => InSta(() =>
    {
        using var dialog = new WildLairDialog(Aliases);
        Assert.Equal(AutoScaleMode.Dpi, dialog.AutoScaleMode);
        var combo = Assert.Single(Descendants<ComboBox>(dialog), c => c.DropDownStyle == ComboBoxStyle.DropDownList);
        Assert.Equal(NeutralLairCatalog.Default.AllDefinitions.Count, combo.Items.Count);
        Assert.NotNull(dialog.SelectedDefinition);
        Assert.False(dialog.CanApply);
        Assert.NotEmpty(dialog.ValidationErrors);
        Assert.Contains("[Error]", dialog.PreviewText);
    });

    [Fact]
    public void Unsupported_options_blocked_team8_animals_and_rewards() => InSta(() =>
    {
        using var dialog = new WildLairDialog(Aliases);

        // Animal selection is blocked
        dialog.SelectedLairId = "ALL_WOL00";
        Assert.False(dialog.CanApply);
        Assert.Contains(dialog.ValidationErrors, err => err.Contains("野生動物") || err.Contains("animals"));

        // Switch to an authored camp blueprint
        dialog.SelectedDefinition = WildLairDialog.CreateAuthoredBlueprint();
        Assert.True(dialog.CanApply);
        Assert.Empty(dialog.ValidationErrors);

        // Team 8 is blocked
        dialog.Team = 8;
        Assert.False(dialog.CanApply);
        Assert.Contains(dialog.ValidationErrors, err => err.Contains('8'));
        dialog.Team = 1;
        Assert.True(dialog.CanApply);

        // Gold reward is blocked
        dialog.RewardGold = 100;
        Assert.False(dialog.CanApply);
        Assert.Contains(dialog.ValidationErrors, err => err.Contains("獎勵") || err.Contains("reward"));
        dialog.RewardGold = 0;
        Assert.True(dialog.CanApply);

        // Wood reward is blocked
        dialog.RewardWood = 50;
        Assert.False(dialog.CanApply);
        dialog.RewardWood = 0;
        Assert.True(dialog.CanApply);

        // Separate initial delay is blocked
        dialog.InitialDelaySeconds = 120;
        dialog.IntervalSeconds = 60;
        Assert.False(dialog.CanApply);
        dialog.InitialDelaySeconds = 60;
        Assert.True(dialog.CanApply);

        // Active waves cap is blocked
        dialog.MaxActiveWaves = 2;
        Assert.False(dialog.CanApply);
        dialog.MaxActiveWaves = 0;
        Assert.True(dialog.CanApply);

        // Aggro orders are blocked
        dialog.AggroBehavior = "Patrol";
        Assert.False(dialog.CanApply);
        dialog.AggroBehavior = "None";
        Assert.True(dialog.CanApply);
    });

    [Fact]
    public void Valid_configuration_produces_scenario_events_and_accept_closes_with_ok() => InSta(() =>
    {
        using var dialog = new WildLairDialog(Aliases);
        dialog.SelectedDefinition = WildLairDialog.CreateAuthoredBlueprint();
        dialog.WaveCount = 2;
        dialog.IntervalSeconds = 45;
        dialog.SpawnCount = 4;
        dialog.UnitAlias = "GER_INF01";
        dialog.Team = 2;
        dialog.WorldX = 4200;
        dialog.WorldZ = 6100;

        Assert.True(dialog.CanApply);
        Assert.Empty(dialog.ValidationErrors);
        Assert.Equal(2, dialog.ResultingEvents.Count);
        Assert.Equal(2, dialog.ScenarioEvents.Count);
        Assert.All(dialog.ResultingEvents, e =>
        {
            Assert.True(e.Repeat);
            Assert.Equal(45, e.DelaySeconds);
            var action = Assert.Single(e.Actions);
            Assert.Equal(ScenarioActionKind.SpawnUnit, action.Kind);
            Assert.Equal("GER_INF01", action.Alias);
            Assert.Equal(4, action.Count);
            Assert.Equal(2, action.Team);
        });

        Assert.Contains("GER_INF01", dialog.PreviewText);
        Assert.Contains("WAVE_0", dialog.PreviewText);

        ShowOffscreen(dialog);
        ((Button)dialog.AcceptButton!).PerformClick();
        Assert.Equal(DialogResult.OK, dialog.DialogResult);
    });

    [Fact]
    public void Cancel_button_closes_with_cancel() => InSta(() =>
    {
        using var dialog = new WildLairDialog(Aliases);
        ShowOffscreen(dialog);
        ((Button)dialog.CancelButton!).PerformClick();
        Assert.Equal(DialogResult.Cancel, dialog.DialogResult);
    });

    [Fact]
    public void Empty_aliases_or_invalid_unit_alias_blocks_accept() => InSta(() =>
    {
        using var missing = new WildLairDialog(new List<ScriptObjectAlias>());
        missing.SelectedDefinition = WildLairDialog.CreateAuthoredBlueprint();
        Assert.False(missing.CanApply);
        Assert.Contains("[Error]", missing.PreviewText);

        using var dialog = new WildLairDialog(Aliases);
        dialog.SelectedDefinition = WildLairDialog.CreateAuthoredBlueprint();
        dialog.UnitAlias = "ALL_WOL00"; // not a troop member
        Assert.False(dialog.CanApply);
        Assert.Contains("[Error]", dialog.PreviewText);
    });

    [Fact]
    public void Merged_scenario_event_capacity_limit_exceeded_blocks_accept() => InSta(() =>
    {
        var existingEvents = Enumerable.Range(0, 255)
            .Select(i => new ScenarioEvent($"EXISTING_{i}", 10) { Actions = [new(ScenarioActionKind.Message, Text: "Test")] })
            .ToList();
        var scenario = new ScenarioDocument { Events = existingEvents };

        using var dialog = new WildLairDialog(Aliases, null, scenario);
        dialog.SelectedDefinition = WildLairDialog.CreateAuthoredBlueprint();
        dialog.WaveCount = 2; // 255 + 2 = 257 > 256
        Assert.False(dialog.CanApply);
        Assert.Contains("256", dialog.PreviewText);

        dialog.WaveCount = 1; // 255 + 1 = 256
        Assert.True(dialog.CanApply);
    });

    [Fact]
    public void Completion_message_requires_core_object_id() => InSta(() =>
    {
        using var dialog = new WildLairDialog(Aliases);
        dialog.SelectedDefinition = WildLairDialog.CreateAuthoredBlueprint();
        dialog.CompletionMessage = "Outpost destroyed!";

        // Without core ID, completion message is unsupported
        Assert.False(dialog.CanApply);
        Assert.Contains(dialog.ValidationErrors, err => err.Contains("核心") || err.Contains("core"));

        // Setting a core structure ID makes it valid
        dialog.CoreStructureSpawnId = Guid.NewGuid();
        Assert.True(dialog.CanApply);
        Assert.Equal(3, dialog.ResultingEvents.Count); // 2 waves + 1 CLEARED event
        var cleared = Assert.Single(dialog.ResultingEvents, e => !e.Repeat);
        Assert.Equal(new ScenarioCondition(ScenarioConditionKind.ObjectDeadOrRemoved, dialog.CoreStructureSpawnId), Assert.Single(cleared.Conditions));
        Assert.Equal("Outpost destroyed!", Assert.Single(cleared.Actions).Text);
    });
}
