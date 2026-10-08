using System.Reflection;
using System.Drawing;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Region_dialog_can_be_rendered_with_coordinate_instructions()
    {
        string? output = Environment.GetEnvironmentVariable("ARM_REGION_OUTPUT");
        RunInSta(() =>
        {
            using var dialog = new TerrainRegionDialog(64);
            dialog.StartPosition = FormStartPosition.Manual; dialog.Location = new Point(-30000, -30000);
            dialog.Show(); Application.DoEvents();
            Assert.Equal(new[] { (10, 10), (20, 20) }, dialog.ReadVertices());
            if (output is not null)
            {
                Directory.CreateDirectory(output);
                using var image = new Bitmap(dialog.Width, dialog.Height);
                dialog.DrawToBitmap(image, new Rectangle(Point.Empty, dialog.Size));
                image.Save(Path.Combine(output, "region-tools.png"));
            }
        });
    }

    [Fact]
    public void Region_dialog_cancel_box_selection_and_fill_save_reload_preserve_disk_until_save()
    {
        string map = CreateFixture(); var originalDisk = SnapshotDirectory(map);
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Region", "Test"));
            _ = form.Handle; Invoke(form, "LoadSelectedMap");
            var real = GetField<TerrainBlendEditSession>(form, "_terrainBlendSession");
            var materials = GetField<FloorMaterialCatalog>(form, "_floorMaterials");
            var material = materials.Materials[0];
            var resolver = new RegionResolver(material.Id);
            string[] textures = real.CurrentTextures.ToArray();
            var session = new TerrainBlendEditSession(new NativeTerrainImportResult(new TerrainBlendAuthoringMap(64, "unpainted"), [], []), textures, resolver);
            typeof(MapEditorForm).GetField("_terrainBlendSession", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, session);
            typeof(MapEditorForm).GetField("_activeMaterial", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, material);
            form.RegionDialogRunner = _ => DialogResult.Cancel; form.RunRegionTool(); Assert.False(session.CanUndo);
            var type = new SdlObjectType("House", 1, SdlObjectCategory.Building, "Ger", 1, new Dictionary<string, string>());
            form.PlacementSession.Load([new(type, 2560, 0, 2560, 0), new(type, 2816, 0, 2816, 1), new(type, 3072, 0, 3072, 0)]);
            Invoke(form, "RefreshPlacedList"); _ = form.PlacedList.Handle;
            form.BeginBoxSelection();
            var canvas = GetField<MapCanvasControl>(form, "_canvas");
            canvas.Size = new Size(640, 640); _ = canvas.Handle;
            Rectangle bounds = (Rectangle)typeof(MapCanvasControl).GetMethod("SceneBounds", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(canvas, null)!;
            Point TilePoint(int x, int y) => new(bounds.X + (int)((x + .5f) * bounds.Width / 64), bounds.Y + (int)((y + .5f) * bounds.Height / 64));
            void Mouse(string method, Point point) => typeof(MapCanvasControl).GetMethod(method, BindingFlags.Instance | BindingFlags.NonPublic)!
                .Invoke(canvas, [new MouseEventArgs(MouseButtons.Left, 1, point.X, point.Y, 0)]);
            Mouse("OnMouseDown", TilePoint(10, 10));
            Mouse("OnMouseMove", TilePoint(11, 11));
            Assert.Equal(new Rectangle(10, 10, 2, 2), canvas.SelectionTiles);
            Mouse("OnMouseUp", TilePoint(11, 11));
            Assert.Null(canvas.SelectionTiles);
            Assert.Equal(2, form.PlacedList.SelectedItems.Count);
            Assert.False(form.PlacementSession.IsDirty);
            form.BeginBoxSelection(); Mouse("OnMouseDown", TilePoint(0, 0));
            object[] escapeArgs = [new Message(), Keys.Escape];
            Assert.True((bool)typeof(MapEditorForm).GetMethod("ProcessCmdKey", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(form, escapeArgs)!);
            Mouse("OnMouseMove", TilePoint(5, 5)); Mouse("OnMouseUp", TilePoint(5, 5));
            Assert.False(session.CanUndo); Assert.Null(canvas.SelectionTiles); Assert.Equal(2, form.PlacedList.SelectedItems.Count);
            form.RegionDialogRunner = dialog => { dialog.VerticesText = "10,10\r\n11,11"; return DialogResult.OK; };
            form.RunRegionTool(); Assert.True(session.CanUndo); Assert.True(session.IsDirty);
            AssertSnapshotUnchanged(map, originalDisk);
            string[] painted = session.CurrentTextures.ToArray();
            Invoke(form, "Undo"); Assert.Equal(textures, session.CurrentTextures); Assert.False(session.CanUndo);
            Invoke(form, "Redo"); Assert.Equal(painted, session.CurrentTextures);
            form.PlacementSession.Load([]); Invoke(form, "RefreshPlacedList");
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
            using var fresh = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Reload", "Test"));
            _ = fresh.Handle; Invoke(fresh, "LoadSelectedMap");
            Assert.Equal(painted, GetField<TerrainBlendEditSession>(fresh, "_terrainBlendSession").CurrentTextures);
        });
    }

    private sealed class RegionResolver(string material) : INativeTerrainMaterialResolver
    {
        public bool TryResolveNativeCorners(string texture, out IReadOnlyList<string> corners) { corners = Enumerable.Repeat("unpainted", 4).ToArray(); return true; }
        public string? ResolveNativeTile(IReadOnlyList<string> corners, int x, int y) => corners.Contains(material) ? "4BB___52" : "4BB___51";
    }
}
