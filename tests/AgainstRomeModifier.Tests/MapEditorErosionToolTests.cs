using System.Drawing;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Erosion_tool_changes_only_rectangle_heights_with_single_undo_and_redo()
    {
        string map = CreateFixture("ENDL_005");

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "ErosionTest", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var layers = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            int size = layers.VertexSize;
            // 在 tile (30..40) 區域造一座陡峭山丘，供侵蝕產生變化。
            var hill = new List<TerrainSampleChange>();
            for (int z = 100; z <= 180; z++)
                for (int x = 100; x <= 180; x++)
                    hill.Add(new TerrainSampleChange(z * size + x, layers.Heights[z * size + x], (byte)Math.Clamp(60 + (int)(140 * Math.Max(0, 1 - Math.Sqrt((x - 140) * (x - 140) + (z - 140) * (z - 140)) / 40.0)), 0, 255)));
            layers.ApplySampleChanges(hill); layers.CommitStroke();
            byte[] before = layers.Heights.ToArray();

            int changed = form.ApplyErosionTool(new Rectangle(30, 30, 10, 10)); // 頂點 120..160
            Assert.True(changed > 0, "侵蝕應改變高度：" + GetField<System.Windows.Forms.ToolStripStatusLabel>(form, "_status").Text);
            byte[] after = layers.Heights.ToArray();
            for (int i = 0; i < after.Length; i++)
                if (after[i] != before[i])
                    Assert.True(i % size is >= 120 and <= 160 && i / size is >= 120 and <= 160, "矩形外高度被修改");

            Invoke(form, "Undo");
            Assert.Equal(before, layers.Heights.ToArray());
            Invoke(form, "Redo");
            Assert.Equal(after, layers.Heights.ToArray());
        });
    }
}
