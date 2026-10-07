using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class PlacementCommandTests
{
    private static SdlPlacedObject CreateSampleFigure(Guid? id = null, int team = 0, int count = 10, float x = 100, float z = 200) =>
        new(new SdlObjectType("FigGerLeader", 1, SdlObjectCategory.Figure, "Ger", 1, new Dictionary<string, string> { ["alias"] = "LEADER" }),
            x, 0, z, team, 0, count)
        {
            ScenarioId = id ?? Guid.NewGuid()
        };

    private static SdlPlacedObject CreateSampleBuilding(Guid? id = null, int team = 0, float x = 500, float z = 600) =>
        new(new SdlObjectType("BauGerHouse", 2, SdlObjectCategory.Building, "Ger", 1, new Dictionary<string, string> { ["alias"] = "HOUSE" }),
            x, 0, z, team, 0, 0)
        {
            ScenarioId = id ?? Guid.NewGuid()
        };

    [Fact]
    public void Edit_Preserves_persistent_ScenarioId_and_supports_undo_redo()
    {
        var session = new PlacementEditSession();
        Guid persistentId = Guid.NewGuid();
        var initial = CreateSampleFigure(persistentId, team: 0, count: 5, x: 100, z: 200);
        session.Load([initial]);

        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);

        // Edit team, position, angle, unit count
        session.Edit(0, team: 1, worldX: 300, worldY: 10, worldZ: 400, angle: 90, unitCount: 15);

        Assert.True(session.IsDirty);
        Assert.True(session.CanUndo);
        var edited = session.Capture()[0];
        Assert.Equal(persistentId, edited.ScenarioId);
        Assert.Equal(1, edited.Team);
        Assert.Equal(300f, edited.WorldX);
        Assert.Equal(10f, edited.WorldY);
        Assert.Equal(400f, edited.WorldZ);
        Assert.Equal(90f, edited.Angle);
        Assert.Equal(15, edited.UnitCount);

        // Undo edit
        Assert.True(session.Undo());
        var reverted = session.Capture()[0];
        Assert.Equal(persistentId, reverted.ScenarioId);
        Assert.Equal(0, reverted.Team);
        Assert.Equal(100f, reverted.WorldX);
        Assert.Equal(5, reverted.UnitCount);
        Assert.False(session.IsDirty);
        Assert.True(session.CanRedo);

        // Redo edit
        Assert.True(session.Redo());
        var redone = session.Capture()[0];
        Assert.Equal(persistentId, redone.ScenarioId);
        Assert.Equal(1, redone.Team);
        Assert.Equal(300f, redone.WorldX);
        Assert.Equal(15, redone.UnitCount);
        Assert.True(session.IsDirty);
    }

    [Fact]
    public void Edit_Forces_original_ScenarioId_even_if_passed_object_has_different_id()
    {
        var session = new PlacementEditSession();
        Guid originalId = Guid.NewGuid();
        session.Load([CreateSampleFigure(originalId)]);

        Guid impostorId = Guid.NewGuid();
        var replacementAttempt = CreateSampleFigure(impostorId, team: 2, count: 12, x: 250, z: 350);
        session.Edit(0, replacementAttempt);

        var current = session.Capture()[0];
        Assert.Equal(originalId, current.ScenarioId);
        Assert.NotEqual(impostorId, current.ScenarioId);
        Assert.Equal(2, current.Team);
        Assert.Equal(12, current.UnitCount);
    }

    [Fact]
    public void Duplicate_Generates_new_guid_and_offsets_position_and_supports_undo_redo()
    {
        var session = new PlacementEditSession();
        Guid originalId = Guid.NewGuid();
        var original = CreateSampleFigure(originalId, team: 0, count: 8, x: 100, z: 200);
        session.Load([original]);

        int dupIndex = session.Duplicate(0, offsetX: 256f, offsetZ: 256f);
        Assert.Equal(1, dupIndex);
        Assert.Equal(2, session.Count);
        Assert.True(session.IsDirty);
        Assert.True(session.CanUndo);

        var duplicate = session.Capture()[1];
        Assert.NotEqual(Guid.Empty, duplicate.ScenarioId);
        Assert.NotEqual(originalId, duplicate.ScenarioId);
        Assert.Equal(356f, duplicate.WorldX);
        Assert.Equal(456f, duplicate.WorldZ);
        Assert.Equal(original.Team, duplicate.Team);
        Assert.Equal(original.UnitCount, duplicate.UnitCount);
        Assert.Equal(original.Type.NameDef, duplicate.Type.NameDef);

        // Undo duplicate
        Assert.True(session.Undo());
        Assert.Equal(1, session.Count);
        Assert.Equal(originalId, session.Capture()[0].ScenarioId);
        Assert.False(session.IsDirty);

        // Redo duplicate
        Assert.True(session.Redo());
        Assert.Equal(2, session.Count);
        Assert.Equal(duplicate.ScenarioId, session.Capture()[1].ScenarioId);
        Assert.True(session.IsDirty);
    }

    [Fact]
    public void Bounds_validation_Enforces_figure_team_0_to_7_and_count_1_to_20()
    {
        var session = new PlacementEditSession();
        session.Load([CreateSampleFigure(team: 0, count: 10)]);

        // Valid Figure edits
        session.Edit(0, team: 0, worldX: 0, worldY: 0, worldZ: 0, angle: 0, unitCount: 1);
        session.Edit(0, team: 7, worldX: 0, worldY: 0, worldZ: 0, angle: 0, unitCount: 20);

        // Invalid Figure team < 0
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            session.Edit(0, team: -1, worldX: 0, worldY: 0, worldZ: 0, angle: 0, unitCount: 10));

        // Invalid Figure team > 7
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            session.Edit(0, team: 8, worldX: 0, worldY: 0, worldZ: 0, angle: 0, unitCount: 10));

        // Invalid Figure count < 1
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            session.Edit(0, team: 0, worldX: 0, worldY: 0, worldZ: 0, angle: 0, unitCount: 0));

        // Invalid Figure count > 20
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            session.Edit(0, team: 0, worldX: 0, worldY: 0, worldZ: 0, angle: 0, unitCount: 21));
    }

    [Fact]
    public void Bounds_validation_Enforces_building_team_bounds_and_clears_unit_count()
    {
        var session = new PlacementEditSession();
        session.Load([CreateSampleBuilding(team: 0)]);

        // Valid building teams (-1 to 15)
        session.Edit(0, team: 8, worldX: 100, worldY: 0, worldZ: 200, angle: 45, unitCount: 0);
        Assert.Equal(8, session.Capture()[0].Team);
        Assert.Equal(0, session.Capture()[0].UnitCount);

        // Building team out of range > 15
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            session.Edit(0, team: 16, worldX: 100, worldY: 0, worldZ: 200, angle: 45, unitCount: 0));

        // Building team < -1
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            session.Edit(0, team: -2, worldX: 100, worldY: 0, worldZ: 200, angle: 45, unitCount: 0));
    }

    [Fact]
    public void Add_and_RemoveAt_Support_undo_redo_and_restore_state()
    {
        var session = new PlacementEditSession();
        var item1 = CreateSampleBuilding();
        var item2 = CreateSampleFigure();

        session.Add(item1);
        Assert.Equal(1, session.Count);
        Assert.True(session.CanUndo);

        session.Add(item2);
        Assert.Equal(2, session.Count);

        session.RemoveAt(0);
        Assert.Equal(1, session.Count);
        Assert.Equal(item2.ScenarioId, session.Capture()[0].ScenarioId);

        // Undo remove
        Assert.True(session.Undo());
        Assert.Equal(2, session.Count);
        Assert.Equal(item1.ScenarioId, session.Capture()[0].ScenarioId);
        Assert.Equal(item2.ScenarioId, session.Capture()[1].ScenarioId);

        // Undo add item2
        Assert.True(session.Undo());
        Assert.Equal(1, session.Count);
        Assert.Equal(item1.ScenarioId, session.Capture()[0].ScenarioId);

        // Undo add item1
        Assert.True(session.Undo());
        Assert.Equal(0, session.Count);

        // Redo all
        Assert.True(session.Redo());
        Assert.True(session.Redo());
        Assert.True(session.Redo());
        Assert.Equal(1, session.Count);
        Assert.Equal(item2.ScenarioId, session.Capture()[0].ScenarioId);
    }

    [Fact]
    public void BatchPlacementCommand_Executes_and_reverts_multiple_operations()
    {
        var session = new PlacementEditSession();
        var item1 = CreateSampleFigure(x: 10);
        var item2 = CreateSampleFigure(x: 20);
        session.Load([item1, item2]);

        var batch = new BatchPlacementCommand("Swap positions", [
            new EditPlacementCommand(0, item1, item1 with { WorldX = 999 }),
            new EditPlacementCommand(1, item2, item2 with { WorldX = 888 })
        ]);

        session.Execute(batch);
        Assert.Equal(999f, session.Capture()[0].WorldX);
        Assert.Equal(888f, session.Capture()[1].WorldX);

        Assert.True(session.Undo());
        Assert.Equal(10f, session.Capture()[0].WorldX);
        Assert.Equal(20f, session.Capture()[1].WorldX);

        Assert.True(session.Redo());
        Assert.Equal(999f, session.Capture()[0].WorldX);
        Assert.Equal(888f, session.Capture()[1].WorldX);
    }
}
