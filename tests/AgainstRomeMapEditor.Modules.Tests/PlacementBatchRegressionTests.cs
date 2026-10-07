using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class PlacementBatchRegressionTests
{
    private static SdlPlacedObject Item(float x) => new(new("Building", 1, SdlObjectCategory.Building, "Ger", 1,
        new Dictionary<string, string> { ["alias"] = "HOUSE" }), x, 0, 100, 0) { ScenarioId = Guid.NewGuid() };

    [Fact]
    public void Duplicate_batch_restores_order_in_one_undo_and_reuses_new_ids_on_redo()
    {
        var session = new PlacementEditSession();
        SdlPlacedObject[] original = [Item(100), Item(200), Item(300)];
        session.Load(original);
        Assert.Equal(new[] { 1, 4 }, session.DuplicateMany([2, 0, 2]));
        var duplicated = session.Capture();
        Assert.Equal(new[] { 100f, 356f, 200f, 300f, 556f }, duplicated.Select(item => item.WorldX));
        Assert.Equal(5, duplicated.Select(item => item.ScenarioId).Distinct().Count());
        ((Dictionary<string, string>)duplicated[1].Type.TemplateFields)["alias"] = "CHANGED";
        Assert.Equal("HOUSE", session[0].Type.TemplateFields["alias"]);
        Assert.Equal("HOUSE", session[1].Type.TemplateFields["alias"]);
        Assert.True(session.Undo());
        Assert.Equal(original.Select(item => item.ScenarioId), session.Capture().Select(item => item.ScenarioId));
        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);
        Assert.True(session.Redo());
        Assert.Equal(duplicated.Select(item => item.ScenarioId), session.Capture().Select(item => item.ScenarioId));
    }

    [Fact]
    public void Invalid_duplicate_rejects_whole_batch_and_preserves_redo()
    {
        var session = new PlacementEditSession();
        session.Load([Item(16300), Item(100)]);
        session.Add(Item(300));
        session.Undo();
        // Index 1 is valid and visited first; index 0 fails after it was prepared.
        Assert.Throws<ArgumentOutOfRangeException>(() => session.DuplicateMany([0, 1]));
        Assert.Equal(new[] { 16300f, 100f }, session.Capture().Select(item => item.WorldX));
        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);
        Assert.True(session.CanRedo);
        Assert.True(session.Redo());
        Assert.Equal(3, session.Count);
    }

    [Fact]
    public void Remove_batch_validates_before_mutation_and_restores_identity_in_one_undo()
    {
        var session = new PlacementEditSession();
        SdlPlacedObject[] original = [Item(100), Item(200), Item(300), Item(400)];
        session.Load(original);
        Assert.Throws<ArgumentOutOfRangeException>(() => session.RemoveMany([0, 4]));
        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);
        session.RemoveMany([2, 0, 2]);
        Assert.Equal(new[] { 200f, 400f }, session.Capture().Select(item => item.WorldX));
        Assert.True(session.Undo());
        Assert.Equal(original.Select(item => item.ScenarioId), session.Capture().Select(item => item.ScenarioId));
        Assert.False(session.CanUndo);
        Assert.True(session.Redo());
        Assert.Equal(new[] { 200f, 400f }, session.Capture().Select(item => item.WorldX));
    }

    [Theory]
    [InlineData(float.NaN, 0, 100, 0)]
    [InlineData(100, float.PositiveInfinity, 100, 0)]
    [InlineData(100, 0, float.NegativeInfinity, 0)]
    [InlineData(100, 0, 100, float.NaN)]
    [InlineData(-1, 0, 100, 0)]
    [InlineData(100, 0, 16384, 0)]
    public void Invalid_values_cannot_enter_through_add_replace_or_edit(float x, float y, float z, float angle)
    {
        var session = new PlacementEditSession();
        var original = Item(100);
        session.Load([original]);
        var invalid = original with { WorldX = x, WorldY = y, WorldZ = z, Angle = angle };
        Assert.Throws<ArgumentOutOfRangeException>(() => session.Add(invalid));
        Assert.Throws<ArgumentOutOfRangeException>(() => session.Replace(0, invalid));
        Assert.Throws<ArgumentOutOfRangeException>(() => session.Edit(0, invalid));
        Assert.Equal(original, session[0] with { Type = original.Type });
        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);
    }

    [Fact]
    public void Empty_batches_do_not_create_history_and_boundary_positions_are_valid()
    {
        var session = new PlacementEditSession();
        session.Load([Item(0) with { WorldZ = 16383 }]);
        session.RemoveMany([]);
        Assert.Empty(session.DuplicateMany([]));
        Assert.False(session.CanUndo);
        session.Duplicate(0, 16383, -16383);
        Assert.Equal(16383, session[1].WorldX);
        Assert.Equal(0, session[1].WorldZ);
    }
}
