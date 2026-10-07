using System.Drawing;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Placement_layout_json_cancel_apply_undo_save_fresh_reload_preserve_existing_event_identity()
    {
        string map = CreateFixture();
        string aliases = Path.Combine(_root, "SYSTEM", "CLAK", "cl_scint.ini");
        Directory.CreateDirectory(Path.GetDirectoryName(aliases)!); byte[] header = new byte[64]; "PFIL"u8.CopyTo(header);
        File.WriteAllBytes(aliases, GameLZSS.CompressPfil(MapTextEncoding.Game.GetBytes("[ObjDefName]\r\nUNIT=FigGerInf00\r\n"), header));
        Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
        File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), ScenarioEventsTests.Fixture().Serialize());
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Layout", "Test"));
            _ = form.Handle; Invoke(form, "LoadSelectedMap");
            var type = Assert.Single(GetField<IReadOnlyList<SdlObjectType>>(form, "_objectCatalog"));
            form.PlacementSession.Load([new(type, 6000, 320, 7000, 0, 45, 10) { ScenarioId = Guid.NewGuid() }, new(type, 6256, 320, 7000, 1, 0, 5) { ScenarioId = Guid.NewGuid() }]);
            var original = form.PlacementSession.Capture();
            GetField<List<ScenarioEvent>>(form, "_events").Add(new("Original target", 2) { Conditions = [new(ScenarioConditionKind.ObjectExists, original[0].ScenarioId)], Actions = [new(ScenarioActionKind.Message, "Kept")] });
            var preset = form.CapturePlacementLayout([0, 1]);
            string path = Path.Combine(_root, "test.arm-layout.json"); File.WriteAllText(path, MapLayoutPresets.Serialize(preset));
            var read = MapLayoutPresets.Deserialize(File.ReadAllText(path));
            var before = SnapshotDirectory(map);
            form.LayoutDialogRunner = _ => DialogResult.Cancel; form.RunLayoutApply(read); Assert.Equal(2, form.PlacementSession.Count); Assert.False(form.PlacementSession.CanUndo);
            form.LayoutDialogRunner = dialog => { dialog.SetTransform(8000, 8000, 90, 2); return DialogResult.OK; };
            form.RunLayoutApply(read); Assert.Equal(4, form.PlacementSession.Count);
            AssertSnapshotUnchanged(map, before);
            var applied = form.PlacementSession.Capture();
            Assert.Equal(original.Select(item => item.ScenarioId), applied.Take(2).Select(item => item.ScenarioId));
            Assert.All(applied.Skip(2), item => { Assert.Equal(2, item.Team); Assert.DoesNotContain(item.ScenarioId, original.Select(i => i.ScenarioId)); });
            Invoke(form, "Undo"); Assert.Equal(2, form.PlacementSession.Count); Assert.False(form.PlacementSession.CanUndo);
            Invoke(form, "Redo"); Assert.Equal(4, form.PlacementSession.Count);
            Assert.Throws<ArgumentOutOfRangeException>(() => form.ApplyLayout(read, 16300, 16300)); Assert.Equal(4, form.PlacementSession.Count);
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
            var saved = SnapshotDirectory(map);
            using var reopened = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Fresh", "Test"));
            _ = reopened.Handle; Invoke(reopened, "LoadSelectedMap");
            Assert.Equal(original[0].ScenarioId, Assert.Single(GetField<List<ScenarioEvent>>(reopened, "_events")).Conditions[0].TargetId);
            Assert.Equal(applied.Select(item => (item.ScenarioId, item.WorldX, item.WorldY, item.WorldZ, item.Team, item.Angle, item.UnitCount)),
                reopened.PlacementSession.Capture().Select(item => (item.ScenarioId, item.WorldX, item.WorldY, item.WorldZ, item.Team, item.Angle, item.UnitCount)));
            Assert.True(reopened.TrySaveMap(false, out error), error?.ToString()); AssertSnapshotUnchanged(map, saved);
            string? output = Environment.GetEnvironmentVariable("ARM_LAYOUT_OUTPUT");
            if (output is not null)
            {
                Directory.CreateDirectory(output); using var dialog = new LayoutApplyDialog(read);
                dialog.StartPosition = FormStartPosition.Manual; dialog.Location = new Point(-30000, -30000); dialog.Show(); Application.DoEvents();
                using var image = new Bitmap(dialog.Width, dialog.Height); dialog.DrawToBitmap(image, new Rectangle(Point.Empty, dialog.Size)); image.Save(Path.Combine(output, "apply-layout.png"));
            }
        });
    }
}
