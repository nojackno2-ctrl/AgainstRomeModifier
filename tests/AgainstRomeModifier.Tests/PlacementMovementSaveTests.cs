using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Placement_group_move_save_reload_preserves_targets_and_coordinates()
    {
        string map = CreateFixture("ENDL_008");
        Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
        File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), ScenarioEventsTests.Fixture().Serialize());
        RunInSta(() =>
        {
            var type = new SdlObjectType("FigGerUnit", 1, SdlObjectCategory.Figure, "Ger", 1,
                new Dictionary<string, string> { ["alias"] = "GER_INF01" });
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_008", map, true, "Move", "Test"));
            _ = form.Handle;
            typeof(MapEditorForm).GetField("_objectCatalog", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!.SetValue(form, new[] { type });
            Invoke(form, "LoadSelectedMap");
            form.PlacementSession.AddMany([new(type, 4000, 0, 5000, 0, 45, 1), new(type, 5000, 0, 6000, 0, 90, 10)]);
            Guid[] ids = form.PlacementSession.Capture().Select(item => item.ScenarioId).ToArray();
            Invoke(form, "RefreshPlacedList"); _ = form.PlacedList.Handle;
            foreach (ListViewItem row in form.PlacedList.Items) row.Selected = true;
            form.MoveSelectedPlacements(256, -64);
            Assert.True(form.PlacementSession.Undo()); Assert.Equal(4000, form.PlacementSession[0].WorldX);
            Assert.True(form.PlacementSession.Redo());
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
            Invoke(form, "LoadSelectedMap");
            Assert.Equal(ids, form.PlacementSession.Capture().Select(item => item.ScenarioId));
            Assert.Equal(new float[] { 4256, 5256 }, form.PlacementSession.Capture().Select(item => item.WorldX));
            Assert.Equal(new float[] { 4936, 5936 }, form.PlacementSession.Capture().Select(item => item.WorldZ));
            Assert.Equal(new float[] { 45, 90 }, form.PlacementSession.Capture().Select(item => item.Angle));
            Assert.Equal(new[] { 1, 10 }, form.PlacementSession.Capture().Select(item => item.UnitCount));
        });
    }
}
