using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class PlacementBatchUiTests
{
    private static SdlPlacedObject Item(float x) => new(new("Building", 1, SdlObjectCategory.Building, "Ger", 1,
        new Dictionary<string, string> { ["alias"] = "HOUSE" }), x, 0, 100, 0) { ScenarioId = Guid.NewGuid() };

    [Fact]
    public void Multi_selection_duplicate_and_delete_use_one_host_undo_step()
    {
        Run(form =>
        {
            SdlPlacedObject[] original = [Item(100), Item(200), Item(300)];
            form.PlacementSession.Load(original);
            Invoke(form, "RefreshPlacedList");
            Select(form, 0, 2);
            form.DuplicateSelectedPlacedObjects();
            Assert.Equal(5, form.PlacedList.Items.Count);
            Assert.Equal(5, form.PlacementSession.Count);
            Invoke(form, "Undo");
            Assert.Equal(3, form.PlacedList.Items.Count);
            Assert.False(form.PlacementSession.CanUndo);
            Invoke(form, "Redo");
            Assert.Equal(5, form.PlacedList.Items.Count);
            Invoke(form, "Undo");
            Select(form, 0, 2);
            Invoke(form, "DeleteSelectedPlacedObjects");
            Assert.Single(form.PlacementSession.Capture());
            Assert.Single(form.PlacedList.Items.Cast<ListViewItem>());
            Invoke(form, "Undo");
            Assert.Equal(original.Select(item => item.ScenarioId), form.PlacementSession.Capture().Select(item => item.ScenarioId));
            Assert.Equal(3, form.PlacedList.Items.Count);
            Assert.False(form.PlacementSession.CanUndo);
        });
    }

    [Fact]
    public void Out_of_bounds_duplicate_reports_rejection_without_mutating_list_or_history()
    {
        Run(form =>
        {
            form.PlacementSession.Load([Item(16300), Item(100)]);
            Invoke(form, "RefreshPlacedList");
            Select(form, 0, 1);
            form.DuplicateSelectedPlacedObjects();
            Assert.Equal(2, form.PlacementSession.Count);
            Assert.Equal(2, form.PlacedList.Items.Count);
            Assert.False(form.PlacementSession.IsDirty);
            Assert.False(form.PlacementSession.CanUndo);
            var status = (ToolStripStatusLabel)typeof(MapEditorForm).GetField("_status", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
            Assert.True(status.Text?.Contains("無法複製") == true || status.Text?.Contains("Cannot duplicate") == true);
        });
    }

    private static void Select(MapEditorForm form, params int[] indices)
    {
        _ = form.PlacedList.Handle;
        foreach (int index in indices) form.PlacedList.Items[index].Selected = true;
        Assert.Equal(indices.Length, form.PlacedList.SelectedItems.Count);
    }

    private static void Invoke(MapEditorForm form, string name, params object[] args) =>
        typeof(MapEditorForm).GetMethod(name, BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(form, args);

    private static void Run(Action<MapEditorForm> action)
    {
        Exception? failure = null;
        var thread = new Thread(() =>
        {
            string root = Path.Combine(Path.GetTempPath(), "ArmPlacementUi_" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(root);
            try
            {
                using var form = new MapEditorForm(root, new GameMapInfo("TEST", root, true, "Test", "Test"));
                _ = form.Handle;
                var mode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
                Invoke(form, "SetEditMode", Enum.Parse(mode, "PlaceObject"));
                action(form);
            }
            catch (Exception ex) { failure = ex; }
            finally { Directory.Delete(root, recursive: true); }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        Assert.True(thread.Join(TimeSpan.FromSeconds(30)), "Placement STA test timed out.");
        if (failure is not null) ExceptionDispatchInfo.Capture(failure).Throw();
    }
}
