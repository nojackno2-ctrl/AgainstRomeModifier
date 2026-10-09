using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.AI;
using AgainstRomeMapEditor.Modules.Events;
using AgainstRomeModifier.Scripting;
using static AgainstRomeModifier.Tests.DialogControlTestSupport;

namespace AgainstRomeModifier.Tests;

public sealed class CampaignWaveDialogTests
{
    private static string[] Aliases => AiArchetypeCatalog.All.SelectMany(p => p.UnitPreferences).Select(p => p.Alias).Distinct().ToArray();

    [Fact]
    public void Controls_preview_compiler_output_and_accept_with_dpi_scaling() => InSta(() =>
    {
        using var dialog = new CampaignWaveDialog(Aliases, new ScenarioDocument());
        Assert.Equal(AutoScaleMode.Dpi, dialog.AutoScaleMode);
        Assert.Equal(AiArchetypeCatalog.All.Count, Assert.Single(Descendants<ComboBox>(dialog)).Items.Count);
        dialog.ArchetypeId = AiArchetypeCatalog.IdRomanFortress;
        dialog.WaveCount = 2;
        dialog.FirstDelaySeconds = 45;
        dialog.IntervalSeconds = 90;
        dialog.SquadCount = 7;
        Assert.True(dialog.CanApply);
        Assert.Equal(new[] { 45, 135 }, dialog.PreviewResult.CompiledEvents.Select(e => e.DelaySeconds));
        Assert.All(dialog.PreviewResult.CompiledEvents, e => Assert.All(e.Actions, a => Assert.Equal(7, a.Count)));
        Assert.Contains("ROM_INF00", dialog.PreviewText);
        Assert.Empty(dialog.Plan.FactionProfiles);
        ShowOffscreen(dialog);
        ((Button)dialog.AcceptButton!).PerformClick();
        Assert.Equal(DialogResult.OK, dialog.DialogResult);
    });

    [Fact]
    public void Unknown_aliases_and_invalid_timing_disable_accept_and_cancel_remains_available() => InSta(() =>
    {
        using var missing = new CampaignWaveDialog([], new ScenarioDocument());
        ShowOffscreen(missing);
        Assert.False(missing.CanApply);
        Assert.Contains("[Error]", missing.PreviewText);
        ((Button)missing.CancelButton!).PerformClick();
        Assert.Equal(DialogResult.Cancel, missing.DialogResult);
        using var timing = new CampaignWaveDialog(Aliases, new ScenarioDocument());
        timing.FirstDelaySeconds = 86400;
        timing.IntervalSeconds = 1;
        timing.WaveCount = 2;
        Assert.False(timing.CanApply);
        timing.WaveCount = 1;
        Assert.True(timing.CanApply);
    });

    [Fact]
    public void Preview_validates_merged_event_limit() => InSta(() =>
    {
        var scenario = new ScenarioDocument { Events = Enumerable.Range(0, 255).Select(i => Event($"Existing {i}")).ToList() };
        using var dialog = new CampaignWaveDialog(Aliases, scenario);
        dialog.WaveCount = 2;
        Assert.False(dialog.CanApply);
        Assert.Contains("256", dialog.PreviewText);
        dialog.WaveCount = 1;
        Assert.True(dialog.CanApply);
    });

    [Fact]
    public void Session_batch_preserves_baseline_and_snapshots_and_is_atomic()
    {
        var session = new ScenarioEventSession();
        session.Load([Event("Existing")]);
        var generated = new[] { Event("Wave 1"), Event("Wave 2") };
        session.AddRange(generated);
        generated[0].Actions.Clear();
        Assert.Equal(3, session.Count);
        Assert.Single(session.Capture()[1].Actions);
        Assert.True(session.IsDirty);
        Assert.Single(session.Baseline);
        Assert.True(session.Undo());
        Assert.False(session.IsDirty);
        Assert.Single(session.Capture());
        Assert.True(session.Redo());
        Assert.Equal(3, session.Count);
        session.AcceptChanges();
        Assert.False(session.IsDirty);
        Assert.True(session.Undo());
        Assert.True(session.IsDirty);
        session.Load(Enumerable.Range(0, 255).Select(i => Event($"Existing {i}")).ToArray());
        Assert.Throws<InvalidOperationException>(() => session.AddRange([Event("A"), Event("B")]));
        Assert.Equal(255, session.Count);
        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);
    }

    private static ScenarioEvent Event(string name) => new(name) { Actions = [new(ScenarioActionKind.Message, Text: "Existing message")] };
}
