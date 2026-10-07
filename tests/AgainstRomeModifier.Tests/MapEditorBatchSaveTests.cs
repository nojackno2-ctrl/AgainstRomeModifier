using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Batch_team_direction_save_fresh_reload_and_repeat_save_preserve_ids_counts_and_positions()
    {
        string map = CreateFixture();
        string aliases = Path.Combine(_root, "SYSTEM", "CLAK", "cl_scint.ini");
        Directory.CreateDirectory(Path.GetDirectoryName(aliases)!);
        byte[] header = new byte[64]; "PFIL"u8.CopyTo(header);
        File.WriteAllBytes(aliases, GameLZSS.CompressPfil(MapTextEncoding.Game.GetBytes("[ObjDefName]\r\nUNIT=FigGerInf00\r\n"), header));
        Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
        File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), ScenarioEventsTests.Fixture().Serialize());
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Batch", "Test"));
            _ = form.Handle; Invoke(form, "LoadSelectedMap");
            var type = Assert.Single(GetField<IReadOnlyList<SdlObjectType>>(form, "_objectCatalog"));
            form.PlacementSession.AddMany([new(type, 6000, 0, 7000, 0, 0, 5), new(type, 6500, 0, 7500, 1, 45, 10)]);
            Invoke(form, "RefreshPlacedList"); _ = form.PlacedList.Handle;
            form.PlacedList.Items[0].Selected = true; form.PlacedList.Items[1].Selected = true;
            var original = form.PlacementSession.Capture();
            form.BatchEditDialogRunner = dialog => { dialog.Team = 2; dialog.Angle = 135; return DialogResult.OK; };
            var before = SnapshotDirectory(map);
            form.EditSelectedPlacedObjectsBatch();
            AssertSnapshotUnchanged(map, before);
            Assert.True(form.PlacementSession.Undo()); Assert.Equal(new[] { 0, 1 }, form.PlacementSession.Capture().Select(item => item.Team));
            Assert.True(form.PlacementSession.Redo());
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
            var saved = ScenarioDocument.Load(map);
            Assert.Equal(2, saved.Spawns.Count);
            Assert.All(saved.Spawns, spawn => { Assert.Equal(2, spawn.Team); Assert.Equal(135, spawn.Angle); });
            var savedBytes = SnapshotDirectory(map);
            using var reopened = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Fresh", "Test"));
            _ = reopened.Handle; Invoke(reopened, "LoadSelectedMap");
            var fresh = reopened.PlacementSession.Capture();
            Assert.Equal(original.Select(item => (item.ScenarioId, item.WorldX, item.WorldZ, item.UnitCount)), fresh.Select(item => (item.ScenarioId, item.WorldX, item.WorldZ, item.UnitCount)));
            Assert.All(fresh, item => { Assert.Equal(2, item.Team); Assert.Equal(135, item.Angle); });
            Assert.False(GetProperty<bool>(reopened, "IsDirty"));
            Assert.True(reopened.TrySaveMap(false, out error), error?.ToString()); AssertSnapshotUnchanged(map, savedBytes);
        });
    }
}
