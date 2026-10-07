using System.Runtime.ExceptionServices;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class AgyPlacementUiTests
{
    private static SdlPlacedObject CreateSampleFigure(Guid? id = null, int team = 0, int count = 10, float x = 100, float z = 200) =>
        new(new SdlObjectType("FigGerLeader", 1, SdlObjectCategory.Figure, "Ger", 1, new Dictionary<string, string> { ["alias"] = "LEADER" }),
            x, 0, z, team, 45, count)
        {
            ScenarioId = id ?? Guid.NewGuid()
        };

    private static SdlPlacedObject CreateSampleBuilding(Guid? id = null, int team = 0, float x = 500, float z = 600) =>
        new(new SdlObjectType("BauGerHouse", 2, SdlObjectCategory.Building, "Ger", 1, new Dictionary<string, string> { ["alias"] = "HOUSE" }),
            x, 0, z, team, 90, 0)
        {
            ScenarioId = id ?? Guid.NewGuid()
        };

    [Fact]
    public void PlacedObjectEditDialog_Initializes_with_seed_values_and_preserves_ScenarioId()
    {
        InSta(() =>
        {
            Guid persistentId = Guid.NewGuid();
            var seed = CreateSampleFigure(persistentId, team: 2, count: 8, x: 512, z: 1024);
            using var dialog = new PlacedObjectEditDialog(seed, isEn: true);
            _ = dialog.Handle; // Force HWND creation without blocking ShowDialog

            Assert.Equal(2, dialog.TeamInput.Value);
            Assert.Equal(512, dialog.PosXInput.Value);
            Assert.Equal(1024, dialog.PosZInput.Value);
            Assert.Equal(45, dialog.AngleInput.Value);
            Assert.Equal(8, dialog.CountInput.Value);
            Assert.True(dialog.CountInput.Enabled);

            dialog.ClickOk();

            Assert.Equal(DialogResult.OK, dialog.DialogResult);
            Assert.NotNull(dialog.Result);
            Assert.Equal(persistentId, dialog.Result!.ScenarioId);
            Assert.Equal(seed.Team, dialog.Result.Team);
            Assert.Equal(seed.WorldX, dialog.Result.WorldX);
            Assert.Equal(seed.WorldZ, dialog.Result.WorldZ);
            Assert.Equal(seed.Angle, dialog.Result.Angle);
            Assert.Equal(seed.UnitCount, dialog.Result.UnitCount);
        });
    }

    [Fact]
    public void PlacedObjectEditDialog_Injectable_inputs_updates_result_fields_and_preserves_id()
    {
        InSta(() =>
        {
            Guid persistentId = Guid.NewGuid();
            var seed = CreateSampleFigure(persistentId, team: 0, count: 5, x: 100, z: 200);
            using var dialog = new PlacedObjectEditDialog(seed, isEn: false);
            _ = dialog.Handle;

            dialog.SetInputs(team: 3, worldX: 1280f, worldY: 15f, worldZ: 2560f, angle: 180f, unitCount: 16);
            dialog.ClickOk();

            Assert.Equal(DialogResult.OK, dialog.DialogResult);
            Assert.NotNull(dialog.Result);
            Assert.Equal(persistentId, dialog.Result!.ScenarioId); // Preserved!
            Assert.Equal(3, dialog.Result.Team);
            Assert.Equal(1280f, dialog.Result.WorldX);
            Assert.Equal(15f, dialog.Result.WorldY);
            Assert.Equal(2560f, dialog.Result.WorldZ);
            Assert.Equal(180f, dialog.Result.Angle);
            Assert.Equal(16, dialog.Result.UnitCount);
        });
    }

    [Fact]
    public void PlacedObjectEditDialog_Disables_unit_count_for_buildings_and_allows_building_teams()
    {
        InSta(() =>
        {
            Guid persistentId = Guid.NewGuid();
            var seed = CreateSampleBuilding(persistentId, team: 8, x: 2048, z: 4096);
            using var dialog = new PlacedObjectEditDialog(seed, isEn: true);
            _ = dialog.Handle;

            Assert.False(dialog.CountInput.Enabled);
            Assert.Equal(8, dialog.TeamInput.Value);

            // Change to neutral team 8 or another team
            dialog.SetInputs(team: 12, worldX: 2304f, worldZ: 4352f, angle: 270f);
            dialog.ClickOk();

            Assert.Equal(DialogResult.OK, dialog.DialogResult);
            Assert.NotNull(dialog.Result);
            Assert.Equal(persistentId, dialog.Result!.ScenarioId);
            Assert.Equal(12, dialog.Result.Team);
            Assert.Equal(2304f, dialog.Result.WorldX);
            Assert.Equal(4352f, dialog.Result.WorldZ);
            Assert.Equal(270f, dialog.Result.Angle);
            Assert.Equal(0, dialog.Result.UnitCount);
        });
    }

    [Fact]
    public void Duplicate_and_edit_operations_integrate_cleanly_with_session()
    {
        var session = new PlacementEditSession();
        Guid id = Guid.NewGuid();
        session.Load([CreateSampleFigure(id, team: 1, count: 10, x: 500, z: 500)]);

        // Duplicate
        int dupIndex = session.Duplicate(0, offsetX: 256f, offsetZ: 256f);
        Assert.Equal(1, dupIndex);
        Assert.Equal(2, session.Count);

        var original = session.Capture()[0];
        var duplicate = session.Capture()[1];

        Assert.Equal(id, original.ScenarioId);
        Assert.NotEqual(Guid.Empty, duplicate.ScenarioId);
        Assert.NotEqual(id, duplicate.ScenarioId); // NEW Guid on duplicate!
        Assert.Equal(756f, duplicate.WorldX);
        Assert.Equal(756f, duplicate.WorldZ);

        // Edit duplicate
        session.Edit(1, team: 4, worldX: 1000f, worldY: 0, worldZ: 1000f, angle: 180f, unitCount: 18);
        var editedDuplicate = session.Capture()[1];

        Assert.Equal(duplicate.ScenarioId, editedDuplicate.ScenarioId); // Preserved on edit!
        Assert.Equal(4, editedDuplicate.Team);
        Assert.Equal(18, editedDuplicate.UnitCount);

        // Undo edit
        Assert.True(session.Undo());
        Assert.Equal(1, session.Capture()[1].Team);
        Assert.Equal(10, session.Capture()[1].UnitCount);

        // Undo duplicate
        Assert.True(session.Undo());
        Assert.Equal(1, session.Count);
        Assert.Equal(id, session.Capture()[0].ScenarioId);
    }

    [Fact]
    public void PlacedObjectEditDialog_Canceling_leaves_result_null()
    {
        InSta(() =>
        {
            var seed = CreateSampleFigure();
            using var dialog = new PlacedObjectEditDialog(seed, isEn: true);
            _ = dialog.Handle;

            dialog.SetInputs(team: 5);
            dialog.Show();
            dialog.CancelButtonControl.PerformClick();

            Assert.Equal(DialogResult.Cancel, dialog.DialogResult);
            Assert.Null(dialog.Result);
        });
    }

    [Fact]
    public void PlacedObjectEditDialog_Enforces_figure_control_bounds()
    {
        InSta(() =>
        {
            var seed = CreateSampleFigure();
            using var dialog = new PlacedObjectEditDialog(seed, isEn: true);
            _ = dialog.Handle;

            // Figures have team 0..7 and count 1..20
            Assert.Equal(0, dialog.TeamInput.Minimum);
            Assert.Equal(7, dialog.TeamInput.Maximum);
            Assert.Equal(1, dialog.CountInput.Minimum);
            Assert.Equal(20, dialog.CountInput.Maximum);
        });
    }

    [Fact]
    public void Placement_session_multiple_duplicates_generate_all_distinct_guids()
    {
        var session = new PlacementEditSession();
        Guid seedId = Guid.NewGuid();
        session.Load([CreateSampleFigure(seedId)]);

        for (int i = 0; i < 5; i++)
        {
            session.Duplicate(0, offsetX: 100f * (i + 1), offsetZ: 100f * (i + 1));
        }

        Assert.Equal(6, session.Count);
        var ids = session.Capture().Select(obj => obj.ScenarioId).ToList();
        Assert.Equal(6, ids.Distinct().Count());
        Assert.Contains(seedId, ids);
    }

    private static void InSta(Action action)
    {
        Exception? failure = null;
        var thread = new Thread(() => { try { action(); } catch (Exception ex) { failure = ex; } });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        Assert.True(thread.Join(TimeSpan.FromSeconds(60)));
        if (failure is not null) ExceptionDispatchInfo.Capture(failure).Throw();
    }
}
