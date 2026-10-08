using System.Drawing;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void River_tool_applies_with_single_transaction_and_supports_undo_redo()
    {
        string map = CreateFixture("ENDL_005");

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "RiverTest", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            // Apply river from (10, 10) to (15, 10)
            form.ApplyRiverTool([(10, 10), (15, 10)]);

            var texturesDoc = (BodenTexturesDocument)typeof(MapEditorForm)
                .GetField("_texturesDocument", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!
                .GetValue(form)!;

            // Check that texture at (12, 10) has changed to a river tile (e.g. FLUSS)
            string textureAt12 = texturesDoc.GetTexture(12, 10);
            Assert.Contains("FLUSS", textureAt12, StringComparison.OrdinalIgnoreCase);

            var status = (ToolStripStatusLabel)typeof(MapEditorForm)
                .GetField("_status", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!
                .GetValue(form)!;
            Assert.Contains("河流已套用", status.Text);

            // Undo -> texture restored
            Invoke(form, "Undo");
            string restoredTexture = texturesDoc.GetTexture(12, 10);
            Assert.DoesNotContain("FLUSS", restoredTexture, StringComparison.OrdinalIgnoreCase);

            // Redo -> river reapplied
            Invoke(form, "Redo");
            Assert.Contains("FLUSS", texturesDoc.GetTexture(12, 10), StringComparison.OrdinalIgnoreCase);
        });
    }

    [Fact]
    public void River_tool_via_region_dialog_applies_from_selection_rectangle()
    {
        string map = CreateFixture("ENDL_005");

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "RiverRegionTest", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            // Set canvas selection box
            var canvas = (MapCanvasControl)typeof(MapEditorForm)
                .GetField("_canvas", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!
                .GetValue(form)!;
            canvas.SelectionTiles = new Rectangle(5, 5, 6, 1); // 5,5 to 10,5

            // RegionDialogRunner mocks OK with River operation using prefilled coordinates
            form.RegionDialogRunner = dialog =>
            {
                dialog.Operation = TerrainRegionOperation.River;
                Assert.Contains("5,5", dialog.VerticesText);
                return DialogResult.OK;
            };

            form.RunRegionTool();

            var texturesDoc = (BodenTexturesDocument)typeof(MapEditorForm)
                .GetField("_texturesDocument", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!
                .GetValue(form)!;

            string textureAt7 = texturesDoc.GetTexture(7, 5);
            Assert.Contains("FLUSS", textureAt7, StringComparison.OrdinalIgnoreCase);

            // Single Undo rolls back
            Invoke(form, "Undo");
            Assert.DoesNotContain("FLUSS", texturesDoc.GetTexture(7, 5), StringComparison.OrdinalIgnoreCase);
        });
    }

    [Fact]
    public void Cliff_tool_applies_on_steep_slope_region_with_single_transaction_and_supports_undo_redo()
    {
        string map = CreateFixture("ENDL_005");

        // Overwrite boden.bmp with steep slope at y=32 (vertices)
        byte[] heights = Grid(257, (x, y) => y <= 32 ? 140 : 20);
        WriteGray(Path.Combine(map, "boden.bmp"), 257, heights);

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "CliffTest", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            var texturesDoc = (BodenTexturesDocument)typeof(MapEditorForm)
                .GetField("_texturesDocument", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!
                .GetValue(form)!;

            // Before applying cliff, tiles are original
            string initialTex = texturesDoc.GetTexture(10, 8);
            Assert.False(initialTex.StartsWith("FELS", StringComparison.OrdinalIgnoreCase));

            // Apply cliff tool to rectangle covering the slope (tiles y from 6 to 10)
            form.ApplyCliffTool(new Rectangle(5, 6, 15, 5));

            var status = (ToolStripStatusLabel)typeof(MapEditorForm)
                .GetField("_status", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!
                .GetValue(form)!;
            Assert.Contains("懸崖已套用", status.Text);

            // Undo restores initial textures
            Invoke(form, "Undo");
            Assert.Equal(initialTex, texturesDoc.GetTexture(10, 8));

            // Redo reapplies cliff
            Invoke(form, "Redo");
            Assert.NotEqual(initialTex, texturesDoc.GetTexture(10, 8));
        });
    }

    [Fact]
    public void Cliff_tool_via_region_dialog_applies_from_selection_rectangle()
    {
        string map = CreateFixture("ENDL_005");

        // Overwrite boden.bmp with steep slope at y=32 (vertices)
        byte[] heights = Grid(257, (x, y) => y <= 32 ? 140 : 20);
        WriteGray(Path.Combine(map, "boden.bmp"), 257, heights);

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "CliffRegionTest", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            // Set canvas selection box covering slope (5,6 to 20,10)
            var canvas = (MapCanvasControl)typeof(MapEditorForm)
                .GetField("_canvas", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!
                .GetValue(form)!;
            canvas.SelectionTiles = new Rectangle(5, 6, 16, 5); // 5,6 to 20,10

            // RegionDialogRunner mocks OK with Cliff operation using prefilled selection coordinates
            form.RegionDialogRunner = dialog =>
            {
                dialog.Operation = TerrainRegionOperation.Cliff;
                Assert.Contains("5,6", dialog.VerticesText);
                return DialogResult.OK;
            };

            form.RunRegionTool();

            var texturesDoc = (BodenTexturesDocument)typeof(MapEditorForm)
                .GetField("_texturesDocument", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!
                .GetValue(form)!;

            string textureAt10 = texturesDoc.GetTexture(10, 8);
            Assert.StartsWith("FELS", textureAt10, StringComparison.OrdinalIgnoreCase);

            // Single Undo rolls back
            Invoke(form, "Undo");
            Assert.False(texturesDoc.GetTexture(10, 8).StartsWith("FELS", StringComparison.OrdinalIgnoreCase));
        });
    }
}
