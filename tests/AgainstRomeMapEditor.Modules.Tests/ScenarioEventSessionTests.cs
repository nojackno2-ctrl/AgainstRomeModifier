using AgainstRomeMapEditor.Modules.Events;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class ScenarioEventSessionTests
{
    private static ScenarioEvent Seed() => new("Event") { Actions = [new(ScenarioActionKind.Message, "Ready")],
        Conditions = [new(ScenarioConditionKind.ObjectExists, Guid.NewGuid())] };

    [Fact]
    public void Snapshots_and_input_lists_cannot_mutate_loaded_or_saved_state()
    {
        ScenarioEvent source = Seed(); var session = new ScenarioEventSession(); session.Load([source]);
        source.Actions.Clear(); source.Conditions.Clear();
        ScenarioEvent captured = session.Capture()[0]; captured.Actions.Clear(); captured.Conditions.Clear();
        Assert.Single(session.Capture()[0].Actions); Assert.Single(session.Capture()[0].Conditions);
        Assert.False(session.IsDirty);
        session.Replace(0, session.Capture()[0] with { Name = "Changed" }); Assert.True(session.IsDirty);
        session.Reset(); Assert.Equal("Event", session.Capture()[0].Name); Assert.False(session.IsDirty);
    }

    [Fact]
    public void Undoing_edits_by_value_and_accepting_save_restore_clean_state()
    {
        var session = new ScenarioEventSession(); ScenarioEvent seed = Seed(); session.Load([seed]);
        session.Replace(0, seed with { DelaySeconds = 20 }); Assert.True(session.IsDirty);
        session.Replace(0, seed); Assert.False(session.IsDirty); // 新 list 參照仍以內容比較。
        session.Replace(0, seed with { Conditions = [] }); Assert.True(session.IsDirty);
        session.AcceptChanges(); Assert.False(session.IsDirty);
        session.RemoveAt(0); Assert.True(session.IsDirty); session.Reset(); Assert.Single(session.Capture());
    }

    [Fact]
    public void Duplicate_preserves_order_and_targets_and_enforces_event_limit()
    {
        var session = new ScenarioEventSession(); ScenarioEvent seed = Seed() with { Name = new string('T', 100) };
        session.Load([seed]); Assert.Equal(1, session.Duplicate(0, " (copy)"));
        Assert.Equal(100, session.Capture()[1].Name.Length);
        Assert.Equal(seed.Conditions, session.Capture()[1].Conditions);
        session.Replace(1, session.Capture()[1] with { Conditions = [] }); Assert.Single(session.Capture()[0].Conditions);
        while (session.Count < 256) session.Add(seed);
        Assert.Throws<InvalidOperationException>(() => session.Duplicate(0, " (copy)"));
        Assert.Throws<InvalidOperationException>(() => session.Add(seed)); Assert.Equal(256, session.Count);
        session.RemoveAt(255); Assert.Equal(255, session.Add(seed));
    }
}
