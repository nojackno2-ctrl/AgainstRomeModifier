using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class PlacementDirectionTests
{
    private static SdlPlacedObject Item(float angle) => new(new("House", 1, SdlObjectCategory.Building, "Ger", 1,
        new Dictionary<string, string>()), 1000, 0, 1000, 0, angle) { ScenarioId = Guid.NewGuid() };

    [Fact]
    public void Relative_rotation_wraps_and_preserves_identity_as_one_undo_step()
    {
        var session = new PlacementEditSession();
        session.Load([Item(315), Item(90)]);
        var before = session.Capture();
        session.RotateMany([0, 1]);
        Assert.Equal(new float[] { 0, 135 }, session.Capture().Select(x => x.Angle));
        Assert.Equal(before.Select(x => x.ScenarioId), session.Capture().Select(x => x.ScenarioId));
        Assert.True(session.Undo());
        Assert.False(session.CanUndo);
        Assert.False(session.IsDirty);
        Assert.True(session.Redo());
        Assert.Equal(new float[] { 0, 135 }, session.Capture().Select(x => x.Angle));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Bound_native_group_rejects_entire_rotation_and_preserves_redo(bool nativeIdentity)
    {
        var session = new PlacementEditSession();
        var bound = Item(90) with { RawRotation = 1.5707964f };
        if (nativeIdentity) bound = bound with { RawRotation = null, NativeSourceIdentity = new(1, 42) };
        session.Load([Item(0), bound]);
        session.RotateMany([0]);
        session.Undo();
        Assert.Throws<InvalidOperationException>(() => session.RotateMany([0, 1]));
        Assert.False(session.IsDirty);
        Assert.True(session.CanRedo);
        Assert.False(session.CanUndo);
    }
}
