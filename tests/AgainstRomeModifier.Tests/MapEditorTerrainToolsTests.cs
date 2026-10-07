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
            Assert.Equal(5, operation.Items.Count);
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
}
