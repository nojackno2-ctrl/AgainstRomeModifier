using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Region_brush_roughen_stroke_covers_the_brush_and_survives_save_and_reload()
    {
        string map = CreateFixture();
        byte[]? saved = null;
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Tools", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var brush = GetField<ComboBox>(form, "_brushSize");
            Assert.Equal(5, brush.Items.Count);
            brush.SelectedIndex = 4;
            Assert.Equal(15, GetField<MapCanvasControl>(form, "_canvas").BrushSize);

            Type mode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
            Invoke(form, "SetEditMode", Enum.Parse(mode, "Height"));
            var operation = GetField<ToolStripComboBox>(form, "_terrainOperation");
            Assert.Equal(6, operation.Items.Count);
            operation.SelectedIndex = (int)TerrainHeightOperation.Roughen;
            var layers = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            byte[] before = layers.Heights.ToArray();
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(32, 32, "", ""));
            Invoke(form, "CommitStroke");

            int size = layers.VertexSize; float step = (size - 1) / 64f;
            var changed = Enumerable.Range(0, before.Length).Where(index => before[index] != layers.Heights[index]).ToArray();
            Assert.Contains(changed, index => layers.Heights[index] > before[index]);
            Assert.Contains(changed, index => layers.Heights[index] < before[index]);
            // 15×15 筆刷：變更延伸超過 5 格（舊最大筆刷），但不超出半徑 7.76 格。
            float Distance(int index) => MathF.Sqrt(MathF.Pow(index % size / step - 32.5f, 2) + MathF.Pow(index / size / step - 32.5f, 2));
            Assert.Contains(changed, index => Distance(index) > 4);
            Assert.All(changed, index => Assert.True(Distance(index) <= 8.1f));

            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
            saved = layers.Heights.ToArray();
        });
        RunInSta(() =>
        {
            using var reopened = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Reopen", "Test"));
            _ = reopened.Handle;
            Invoke(reopened, "LoadSelectedMap");
            Assert.Equal(saved, GetField<TerrainHeightEditSession>(reopened, "_terrainLayers").Heights);
        });
    }

    [Fact]
    public void Water_operation_carves_below_the_water_surface_and_needs_a_water_level()
    {
        string map = CreateFixture(); // Waterlevel 120、Heightmapstep 4 → 水面 30、水底 24
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Water", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            GetField<ComboBox>(form, "_brushSize").SelectedIndex = 2;
            Type mode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
            Invoke(form, "SetEditMode", Enum.Parse(mode, "Height"));
            var operation = GetField<ToolStripComboBox>(form, "_terrainOperation");
            operation.SelectedIndex = 5;
            GetField<ToolStripComboBox>(form, "_terrainStrength").SelectedIndex = 2;
            var layers = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            int size = layers.VertexSize, center = 130 * size + 130; // tile (32,32) 中心頂點
            byte[] before = layers.Heights.ToArray();
            Assert.True(before[center] > 30);
            for (int pass = 0; pass < 8; pass++) { Invoke(form, "PaintTexture", new TexturePaintEventArgs(32, 32, "", "")); Invoke(form, "CommitStroke"); }
            Assert.Equal(24, layers.Heights[center]);
            Assert.Equal(before[0], layers.Heights[0]);
            for (int pass = 0; pass < 8; pass++) Invoke(form, "Undo");
            Assert.Equal(before, layers.Heights);

            GetField<NumericUpDown>(form, "_waterLevel").Value = 0;
            byte[] dry = layers.Heights.ToArray();
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(20, 20, "", ""));
            Invoke(form, "CommitStroke");
            Assert.Equal(dry, layers.Heights);
            Assert.Contains("水面", GetField<ToolStripStatusLabel>(form, "_status").Text);
        });
    }
}
