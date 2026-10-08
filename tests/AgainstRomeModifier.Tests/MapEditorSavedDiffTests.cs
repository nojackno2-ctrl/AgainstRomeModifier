using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Saved_version_diff_reports_no_change_then_terrain_edit()
    {
        string map = CreateFixture("ENDL_005");

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "DiffTest", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            Assert.Contains("沒有差異", form.BuildSavedDiffReport());

            var layers = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            int index = 20 * layers.VertexSize + 20;
            layers.ApplySampleChanges([new TerrainSampleChange(index, layers.Heights[index], (byte)(layers.Heights[index] + 30))]);
            layers.CommitStroke();
            string report = form.BuildSavedDiffReport();
            Assert.Contains("地形起伏", report);
            Assert.Contains("變更頂點數: 1 個", report);
        });
    }
}
