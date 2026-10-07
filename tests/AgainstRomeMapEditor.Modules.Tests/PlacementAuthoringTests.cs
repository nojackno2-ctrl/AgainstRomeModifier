using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class PlacementAuthoringTests
{
    private static SdlPlacedObject Item(SdlObjectCategory category, float x = 1000) => new(new("Object", 1, category, "Ger", 1, new Dictionary<string, string>()), x, 0, 1000, 0, 0, category == SdlObjectCategory.Figure ? 10 : 0) { ScenarioId = Guid.NewGuid() };

    [Fact]
    public void Batch_team_direction_change_preserves_identity_position_count_and_undo_is_atomic()
    {
        var session = new PlacementEditSession(); session.Load([Item(SdlObjectCategory.Building), Item(SdlObjectCategory.Figure, 2000)]);
        var before = session.Capture();
        session.EditMany([0, 1], team: 3, angle: 135);
        Assert.All(session.Capture(), item => { Assert.Equal(3, item.Team); Assert.Equal(135, item.Angle); });
        Assert.Equal(before.Select(item => (item.ScenarioId, item.WorldX, item.UnitCount)), session.Capture().Select(item => (item.ScenarioId, item.WorldX, item.UnitCount)));
        Assert.True(session.Undo()); Assert.Equal(before.Select(item => (item.Team, item.Angle)), session.Capture().Select(item => (item.Team, item.Angle)));
        Assert.True(session.Redo()); Assert.All(session.Capture(), item => Assert.Equal(3, item.Team));
    }

    [Fact]
    public void Invalid_mixed_team_batch_leaves_objects_and_redo_untouched()
    {
        var session = new PlacementEditSession(); session.Load([Item(SdlObjectCategory.Building), Item(SdlObjectCategory.Figure)]);
        session.EditMany([0, 1], angle: 90); session.Undo();
        Assert.Throws<ArgumentOutOfRangeException>(() => session.EditMany([0, 1], team: 8));
        Assert.True(session.CanRedo); Assert.All(session.Capture(), item => Assert.Equal(0, item.Team));
    }

    [Fact]
    public void Layout_batch_rejects_partial_insertion_and_duplicate_ids_then_undoes_as_one_step()
    {
        var session = new PlacementEditSession(); var first = Item(SdlObjectCategory.Building);
        Assert.Throws<ArgumentOutOfRangeException>(() => session.AddMany([first, Item(SdlObjectCategory.Figure, 20000)]));
        Assert.Equal(0, session.Count);
        Assert.Throws<ArgumentException>(() => session.AddMany([first, first])); Assert.Equal(0, session.Count);
        Assert.Equal(new[] { 0, 1 }, session.AddMany([first, Item(SdlObjectCategory.Figure)]));
        Assert.True(session.Undo()); Assert.Equal(0, session.Count); Assert.True(session.Redo()); Assert.Equal(2, session.Count);
    }
}
