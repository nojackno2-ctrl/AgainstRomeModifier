using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Events;
using AgainstRomeMapEditor.Modules.Events.Graph;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Event_graph_round_trip_saves_via_session_and_preserves_order_when_dragged()
    {
        string map = CreateFixture();
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
        File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), ScenarioEventsTests.Fixture().Serialize());
        var original = new ScenarioEvent("First", 7) { Actions = [new(ScenarioActionKind.Message, "First message")] };
        var second = new ScenarioEvent("Second", 10) { Actions = [new(ScenarioActionKind.Diplomacy, Team: 1, OtherTeam: 2), new(ScenarioActionKind.Victory)] };
        new ScenarioDocument { Events = [original, second] }.Save(map, rollback); rollback.Commit();
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Graph", ""));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var canvas = GetField<EventGraphCanvasControl>(form, "_eventGraphCanvas");
            var triggers = canvas.Graph!.Nodes.OfType<EventTriggerNode>().ToArray();
            triggers[0].Y = 1000;
            triggers[0].DelaySeconds = 13;
            canvas.Graph.Nodes.OfType<MessageActionNode>().Single().MessageText = "Updated";
            form.MarkEventGraphChanged();
            Assert.True(GetProperty<bool>(form, "IsDirty"));
            Assert.True(form.TrySaveMap(false, out var error), error?.ToString());
            Assert.False(GetProperty<bool>(form, "IsDirty"));
            var saved = ScenarioDocument.Load(map).Events;
            Assert.Equal(new[] { "First", "Second" }, saved.Select(e => e.Name));
            Assert.Equal(13, saved[0].DelaySeconds);
            Assert.Equal("Updated", saved[0].Actions[0].Text);
            Assert.Equal(second.Actions, saved[1].Actions);
        });
    }

    [Fact]
    public void Invalid_graph_is_rejected_before_writes_and_discard_restores_session()
    {
        string map = CreateFixture();
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        new ScenarioDocument { Events = [new("Graph") { Actions = [new(ScenarioActionKind.Message, "Keep")] }] }.Save(map, rollback); rollback.Commit();
        var before = SnapshotDirectory(map);
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Graph", ""));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var graph = GetField<EventGraphCanvasControl>(form, "_eventGraphCanvas").Graph!;
            var action = graph.Nodes.OfType<MessageActionNode>().Single();
            graph.Disconnect(Assert.Single(graph.Edges).Id);
            action.MessageText = ""; // invalid action plus disconnected node
            form.MarkEventGraphChanged();
            Assert.False(form.TrySaveMap(false, out var error));
            Assert.IsType<InvalidDataException>(error);
            Assert.True(GetProperty<bool>(form, "IsDirty"));
            Assert.Equal("Keep", GetField<List<ScenarioEvent>>(form, "_events").Single().Actions[0].Text);
            Invoke(form, "ReloadEventGraph");
            Assert.False(GetProperty<bool>(form, "IsDirty"));
        });
        var after = SnapshotDirectory(map);
        Assert.Equal(before.Keys.OrderBy(k => k), after.Keys.OrderBy(k => k));
        foreach (var file in before) Assert.Equal(file.Value, after[file.Key]);
    }
}