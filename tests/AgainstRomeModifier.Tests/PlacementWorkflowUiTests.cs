using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class PlacementWorkflowUiTests
{
    private static SdlPlacedObject Item(float x) => new(new("BauGerHau00_Haupthaus", 1,
        SdlObjectCategory.Building, "Ger", 1, new Dictionary<string, string> { ["alias"] = "GER_HAU00" }),
        x, 20, 600, 0) { ScenarioId = Guid.NewGuid() };

    [Fact]
    public void Search_combines_localized_name_alias_category_and_tribe()
    {
        Run(form =>
        {
            var house = Item(400).Type;
            var other = house with { NameDef = "BauRomMine", Tribe = "Rom", TemplateFields = new Dictionary<string, string> { ["alias"] = "ROM_MINE" } };
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, new[] { house, other });
            Invoke(form, "PopulatePlacementFilters");
            var search = (TextBox)typeof(MapEditorForm).GetField("_placeSearch", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
            var list = (ListBox)typeof(MapEditorForm).GetField("_placeTypes", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
            search.Text = "主屋 ger_hau"; Assert.Single(list.Items.Cast<object>());
            Assert.True(MapEditorForm.MatchesPlacementSearch(house, "buildings Germans Haupthaus"));
            var tribe = (ComboBox)typeof(MapEditorForm).GetField("_placeTribe", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
            tribe.SelectedIndex = 2; Invoke(form, "RefreshPlacementTypes"); Assert.Empty(list.Items.Cast<object>());
            tribe.SelectedIndex = 0; search.Text = "  "; Assert.Equal(2, list.Items.Count);
            search.Text = "missing"; Assert.Equal(-1, list.SelectedIndex);
        });
    }

    [Fact]
    public void Keyboard_move_duplicate_select_all_delete_and_undo_are_batches()
    {
        Run(form =>
        {
            form.PlacementSession.Load([Item(400), Item(900)]); Invoke(form, "RefreshPlacedList"); Select(form, 0, 1);
            var key = new KeyEventArgs(Keys.Control | Keys.Right); form.HandlePlacementWorkflowKey(key);
            Assert.True(key.SuppressKeyPress); Assert.Equal(656, form.PlacementSession[0].WorldX);
            Assert.Equal(2, form.PlacedList.SelectedItems.Count);
            form.HandlePlacementWorkflowKey(new(Keys.Control | Keys.Shift | Keys.Up));
            Assert.Equal(536, form.PlacementSession[0].WorldZ);
            Invoke(form, "Undo"); Assert.Equal(600, form.PlacementSession[0].WorldZ);
            Invoke(form, "Undo"); Assert.False(form.PlacementSession.IsDirty);
            Select(form, 0, 1); form.HandlePlacementWorkflowKey(new(Keys.Control | Keys.D));
            Assert.Equal(4, form.PlacementSession.Count); Assert.Equal(2, form.PlacedList.SelectedItems.Count);
            Assert.All(form.PlacedList.SelectedItems.Cast<ListViewItem>(), row => Assert.Contains((int)row.Tag!, new[] { 1, 3 }));
            form.HandlePlacementWorkflowKey(new(Keys.Control | Keys.A)); Assert.Equal(4, form.PlacedList.SelectedItems.Count);
            form.HandlePlacementWorkflowKey(new(Keys.Delete)); Assert.Empty(form.PlacementSession.Capture());
            Invoke(form, "Undo"); Assert.Equal(4, form.PlacementSession.Count);
        });
    }

    [Fact]
    public void Rejected_move_preserves_selection_and_snap_is_single_undo()
    {
        Run(form =>
        {
            form.PlacementSession.Load([Item(400), Item(16300)]); Invoke(form, "RefreshPlacedList"); Select(form, 0, 1);
            form.MoveSelectedPlacements(256, 0);
            Assert.False(form.PlacementSession.IsDirty); Assert.False(form.PlacementSession.CanUndo);
            Assert.Equal(2, form.PlacedList.SelectedItems.Count);
            form.MoveSelectedPlacements(0, 0, snap: true);
            Assert.Equal(384, form.PlacementSession[0].WorldX); Assert.Equal(16284, form.PlacementSession[1].WorldX);
            Invoke(form, "Undo"); Assert.False(form.PlacementSession.IsDirty); Assert.False(form.PlacementSession.CanUndo);
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
