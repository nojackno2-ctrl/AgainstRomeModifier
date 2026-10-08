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

            var layers = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            byte[] beforeHeights = layers.Heights.ToArray();

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

            byte[] carvedHeights = layers.Heights.ToArray();
            Assert.True(carvedHeights[42 * 257 + 50] <= 24);
            Assert.Equal(beforeHeights[100 * 257 + 100], carvedHeights[100 * 257 + 100]);
            Assert.Equal(beforeHeights[10 * 257 + 12], carvedHeights[10 * 257 + 12]);

            // Undo -> texture and entire height field restored
            Invoke(form, "Undo");
            Assert.Equal(beforeHeights, layers.Heights);
            string restoredTexture = texturesDoc.GetTexture(12, 10);
            Assert.DoesNotContain("FLUSS", restoredTexture, StringComparison.OrdinalIgnoreCase);

            // Redo -> river reapplied
            Invoke(form, "Redo");
            Assert.Contains("FLUSS", texturesDoc.GetTexture(12, 10), StringComparison.OrdinalIgnoreCase);
            Assert.Equal(carvedHeights, layers.Heights);
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
    public void Cliff_tool_does_not_stamp_tiles_missing_from_the_game_texture_library()
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

            // 測試素材庫沒有預設目錄預期的岩壁圖塊：工具不得貼上不存在的圖塊（遊戲內會顯示 File not found），而是不變更並說明。
            form.ApplyCliffTool(new Rectangle(5, 6, 15, 5));

            var status = (ToolStripStatusLabel)typeof(MapEditorForm)
                .GetField("_status", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!
                .GetValue(form)!;
            Assert.Contains("沒有可對應的岩壁圖塊", status.Text);
            Assert.Equal(initialTex, texturesDoc.GetTexture(10, 8));
        });
    }

    [Fact]
    public void Cliff_tool_via_region_dialog_leaves_map_unchanged_without_matching_tiles()
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

            // 沒有對應的真實岩壁圖塊時不貼任何東西。
            Assert.False(texturesDoc.GetTexture(10, 8).StartsWith("FELS", StringComparison.OrdinalIgnoreCase));
        });
    }
}
