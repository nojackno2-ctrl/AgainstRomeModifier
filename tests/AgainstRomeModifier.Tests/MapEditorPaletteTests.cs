using System.IO.Compression;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Palette_hides_materials_that_cannot_blend_into_the_map_unless_searched()
    {
        string map = CreateFixture();
        using (ZipArchive zip = ZipFile.Open(Path.Combine(_root, "floortex.dat"), ZipArchiveMode.Update))
        using (Stream stream = zip.CreateEntry("SYSTEM/DATA/FLOORTEXTURE/4BX___50.bmp").Open())
            stream.Write(Encode(128, Enumerable.Repeat((byte)200, 128 * 128).ToArray()));
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Palette", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var palette = GetField<ListBox>(form, "_palette");
            string[] Names() => palette.Items.Cast<object>().Select(item => item.ToString()!).ToArray();
            Assert.Equal(2, GetField<FloorMaterialCatalog>(form, "_floorMaterials").Materials.Count);
            Assert.Single(palette.Items); // 只剩地圖已使用的 BB
            GetField<TextBox>(form, "_paletteSearch").Text = "紅土";
            Assert.Single(palette.Items);
            Assert.Contains("難以銜接", Names()[0]);
        });
    }
}
