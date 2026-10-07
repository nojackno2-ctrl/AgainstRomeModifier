using System.Runtime.CompilerServices;
using AgainstRomeMapEditor.Modules.Nature;
using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class MapLayoutPresetTests
{
    private static SdlObjectType Unit => new("FigGerInf00", -1, SdlObjectCategory.Figure, "Ger", 0, new Dictionary<string, string> { ["alias"] = "UNIT" });
    private static MapLayoutPreset Layout => new(1, MapLayoutKind.Placement, [new("UNIT", 0, 0, 5, 45, 0, 10), new("UNIT", 256, 0, 0, 0, 1, 5)]);

    [Fact]
    public void Portable_json_preserves_transforms_rotates_on_ground_and_assigns_new_identities_each_time()
    {
        string json = MapLayoutPresets.Serialize(Layout);
        var read = MapLayoutPresets.Deserialize(json);
        Assert.Equal(Layout.Entries, read.Entries);
        var first = MapLayoutPresets.PlanPlacements(read, [Unit], 1000, 1000, 90, (_, _) => 100, 2);
        Assert.Equal((1000f, 105f, 1000f, 135f, 2, 10), (first[0].WorldX, first[0].WorldY, first[0].WorldZ, first[0].Angle, first[0].Team, first[0].UnitCount));
        Assert.Equal((1000f, 1256f), (first[1].WorldX, first[1].WorldZ));
        var second = MapLayoutPresets.PlanPlacements(read, [Unit], 1000, 1000, 90, (_, _) => 100);
        Assert.DoesNotContain(first[0].ScenarioId, second.Select(item => item.ScenarioId));
        Assert.DoesNotContain(first[0].ScenarioId.ToString(), json);
        Assert.Equal(1, Layout.Entries[1].Team);
        var session = new PlacementEditSession(); session.AddMany(first); Assert.True(session.Undo()); Assert.Empty(session.Capture());
        Assert.False(session.CanUndo); Assert.True(session.Redo()); Assert.Equal(first.Select(i => i.ScenarioId), session.Capture().Select(i => i.ScenarioId));
    }

    [Fact]
    public void Unknown_types_out_of_bounds_and_invalid_schema_reject_before_mutation()
    {
        var session = new PlacementEditSession(); session.AddMany(MapLayoutPresets.PlanPlacements(Layout, [Unit], 100, 100, 0, (_, _) => 0)); session.Undo();
        Assert.Throws<InvalidDataException>(() => MapLayoutPresets.PlanPlacements(Layout, [], 100, 100, 0, (_, _) => 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => MapLayoutPresets.PlanPlacements(Layout, [Unit], 16300, 100, 0, (_, _) => 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => MapLayoutPresets.PlanPlacements(Layout, [Unit], 100, 100, 0, (_, _) => 0, 8));
        Assert.Throws<InvalidDataException>(() => MapLayoutPresets.Deserialize("{\"version\":2,\"kind\":\"Placement\",\"entries\":[]}"));
        Assert.Throws<InvalidDataException>(() => MapLayoutPresets.Validate(Layout with { Entries = [new("UNIT", float.NaN, 0, 0, 0)] }));
        Assert.True(session.CanRedo); Assert.Empty(session.Capture());
    }

    [Fact]
    public void Nature_layout_converts_degrees_to_radians_and_entire_batch_undo_preserves_redo_after_rejection()
    {
        // Pure planner does not read native bytes; host acceptance supplies real templates.
        var template = (LevelObjectTemplate)RuntimeHelpers.GetUninitializedObject(typeof(LevelObjectTemplate));
        var layout = new MapLayoutPreset(1, MapLayoutKind.Nature, [new("LanGerNad18", 0, 0, 5, 90), new("LanGerNad18", 256, 0, 0, 0)]);
        var templates = new Dictionary<string, LevelObjectTemplate> { ["LanGerNad18"] = template };
        var additions = MapLayoutPresets.PlanNature(layout, templates, 0, 100, 90, (_, _) => 80);
        Assert.Equal((0f, 356f), (additions[1].X, additions[1].Z)); Assert.Equal(85, additions[0].Y); Assert.Equal(MathF.PI, additions[0].Rotation, 5);
        var session = new NatureEditSession(); Assert.True(session.PlantMany(additions));
        session.Undo(); Assert.Empty(session.Additions); Assert.False(session.CanUndo);
        Assert.Throws<ArgumentException>(() => session.PlantMany([additions[0], additions[1] with { X = 16384 }]));
        Assert.Empty(session.Additions); Assert.True(session.CanRedo); Assert.False(session.PlantMany([])); Assert.True(session.CanRedo);
        session.Redo(); Assert.Equal(additions, session.Additions);
        Assert.Throws<InvalidDataException>(() => MapLayoutPresets.PlanNature(layout, new Dictionary<string, LevelObjectTemplate>(), 0, 0, 0, (_, _) => 0));
    }
}
