using System.IO.Compression;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Tile_stamp_places_an_original_road_tile_that_saves_reloads_and_samples()
    {
        string map = CreateFixture();
        using (ZipArchive zip = ZipFile.Open(Path.Combine(_root, "floortex.dat"), ZipArchiveMode.Update))
        using (Stream stream = zip.CreateEntry("SYSTEM/DATA/FLOORTEXTURE/weg1.bmp").Open())
            stream.Write(Encode(128, Enumerable.Repeat((byte)60, 128 * 128).ToArray()));
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Stamp", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            Assert.Equal(0, MapEditorForm.StampCategory("weg1"));
            Assert.Equal(1, MapEditorForm.StampCategory("FLUSS3"));
            Assert.Equal(5, MapEditorForm.StampCategory("L2B02T5A"));
            GetField<CheckBox>(form, "_stampMode").Checked = true;
            var palette = GetField<ListBox>(form, "_palette");
            int road = palette.Items.Cast<object>().ToList().FindIndex(item => item.ToString()!.Contains("weg1") || item.GetType().GetProperty("Key")!.GetValue(item)!.Equals("tile:weg1"));
            Assert.True(road >= 0);
            Assert.Equal(0, road); // 道路類排在最前
            palette.SelectedIndex = road;
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(5, 6, "", ""));
            Invoke(form, "CommitStroke");
            Assert.Equal("weg1", GetField<BodenTexturesDocument>(form, "_texturesDocument").GetTexture(5, 6));
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());

            palette.SelectedIndex = palette.Items.Count - 1;
            Invoke(form, "SelectSampledTexture", "weg1");
            Assert.Equal("weg1", GetField<string>(form, "_stampTexture"));
        });
        RunInSta(() =>
        {
            using var reopened = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Reopen", "Test"));
            _ = reopened.Handle;
            Invoke(reopened, "LoadSelectedMap");
            Assert.Equal("weg1", GetField<BodenTexturesDocument>(reopened, "_texturesDocument").GetTexture(5, 6));
        });
    }
}
