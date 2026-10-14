using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class PlacementMovementTests
{
    private static SdlPlacedObject Item(float x, float z = 600) => new(new("House", 1,
        SdlObjectCategory.Building, "Ger", 1, new Dictionary<string, string>()), x, 20, z, 0, 90)
        { ScenarioId = Guid.NewGuid() };

    [Fact]
    public void Group_translation_preserves_family_metadata_and_is_one_undo_step()
    {
        var session = new PlacementEditSession();
        session.Load([Item(400), Item(900) with { RawRotation = 1.5707964f,
            NativeSourceIdentity = new(1, 42), CopySourceScenarioId = Guid.NewGuid() }]);
        var before = session.Capture();
        session.MoveMany([1, 0, 1], 256, -64, (x, z) => x / 10);
        Assert.Equal(new float[] { 656, 1156 }, session.Capture().Select(x => x.WorldX));
        Assert.Equal(65.6f, session[0].WorldY);
        Assert.Equal(before[1].RawRotation, session[1].RawRotation);
        Assert.Equal(before[1].NativeSourceIdentity, session[1].NativeSourceIdentity);
        Assert.Equal(before[1].CopySourceScenarioId, session[1].CopySourceScenarioId);
        Assert.Equal(before.Select(x => x.ScenarioId), session.Capture().Select(x => x.ScenarioId));
        Assert.True(session.Undo()); Assert.False(session.CanUndo); Assert.False(session.IsDirty);
        Assert.True(session.Redo()); Assert.Equal(1156, session[1].WorldX);
    }

    [Theory]
    [InlineData(256, 0)]
    [InlineData(float.NaN, 0)]
    [InlineData(0, float.PositiveInfinity)]
    public void Invalid_batch_leaves_state_and_redo_untouched(float x, float z)
    {
        var session = new PlacementEditSession(); session.Load([Item(400), Item(16300)]);
        session.MoveMany([0], 64, 0); session.Undo();
        Assert.Throws<ArgumentOutOfRangeException>(() => session.MoveMany([0, 1], x, z));
        Assert.False(session.IsDirty); Assert.False(session.CanUndo); Assert.True(session.CanRedo);
    }

    [Fact]
    public void Snap_preserves_arrangement_and_height_and_noop_preserves_redo()
    {
        var session = new PlacementEditSession(); session.Load([Item(400), Item(900)]);
        session.SnapManyToGrid([1, 0]);
        Assert.Equal(384, session[0].WorldX); Assert.Equal(884, session[1].WorldX);
        Assert.Equal(640, session[0].WorldZ); Assert.Equal(20, session[0].WorldY);
        Assert.True(session.Undo()); Assert.False(session.IsDirty);
        session.MoveMany([0, 1], 0, 0); Assert.True(session.CanRedo);
    }

    [Fact]
    public void Invalid_height_or_index_rejects_whole_batch()
    {
        var session = new PlacementEditSession(); session.Load([Item(400), Item(900)]);
        Assert.Throws<ArgumentOutOfRangeException>(() => session.MoveMany([0, 1], 64, 0,
            (x, z) => x > 900 ? float.NaN : 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => session.SnapManyToGrid([0, 2]));
        Assert.False(session.IsDirty); Assert.False(session.CanUndo);
    }
}
