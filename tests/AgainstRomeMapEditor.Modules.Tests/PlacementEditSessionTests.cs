using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class PlacementEditSessionTests
{
    [Fact]
    public void Snapshot_isolation_and_reset_preserve_persistent_identity_and_alias()
    {
        var fields = new Dictionary<string, string> { ["alias"] = "UNIT" };
        var seed = new SdlPlacedObject(new("Unit", 1, SdlObjectCategory.Figure, "Ger", 1, fields), 10, 0, 20, 0)
            { ScenarioId = Guid.NewGuid() };
        var session = new PlacementEditSession(); session.Load([seed]); fields["alias"] = "OTHER";
        var snapshot = session.Capture(); ((Dictionary<string, string>)snapshot[0].Type.TemplateFields)["alias"] = "MUTATED";
        Assert.Equal("UNIT", session.Capture()[0].Type.TemplateFields["alias"]); Assert.False(session.IsDirty);
        session.Replace(0, session.Capture()[0] with { WorldX = 30 }); Assert.True(session.IsDirty);
        session.Reset(); Assert.Equal(10, session.Capture()[0].WorldX); Assert.Equal(seed.ScenarioId, session.Capture()[0].ScenarioId);
        session.Replace(0, session.Capture()[0] with { WorldX = 40 }); session.AcceptChanges(); Assert.False(session.IsDirty);
        session.RemoveAt(0); Assert.True(session.IsDirty); session.Reset(); Assert.Equal(40, session.Capture()[0].WorldX);
    }
}
