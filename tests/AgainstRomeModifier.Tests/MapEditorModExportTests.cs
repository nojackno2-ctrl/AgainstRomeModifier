using System.IO.Compression;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Mod_packaging_export_button_saves_valid_zip_with_manifest_and_map()
    {
        string map = CreateFixture("ENDL_005");
        File.WriteAllBytes(Path.Combine(map, "DATA", "objects.dat"), new byte[64]);
        File.WriteAllBytes(Path.Combine(map, "DATA", "objdata.dat"), new byte[64]);
        File.WriteAllBytes(Path.Combine(map, "DATA", "pos.dat"), new byte[64]);
        Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
        File.WriteAllBytes(Path.Combine(map, "SCRIPT", "ak_level.bci"), new byte[] { (byte)'P', (byte)'F', (byte)'I', (byte)'L', 0, 0, 0, 0 });

        string zipPath = Path.Combine(_root, "test_exported_mod.zip");

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Mod Export Map", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            // 1. Cancel export dialog -> returns false, no file created
            form.ExportFileDialogRunner = _ => DialogResult.Cancel;
            bool cancelled = form.ExportModPackage(suppressMessage: true);
            Assert.False(cancelled);
            Assert.False(File.Exists(zipPath));

            // 2. Mock SaveFileDialog to return OK with zipPath
            form.ExportFileDialogRunner = dialog =>
            {
                dialog.FileName = zipPath;
                return DialogResult.OK;
            };

            bool exported = form.ExportModPackage(suppressMessage: true);
            Assert.True(exported);
            Assert.True(File.Exists(zipPath));

            // 3. Inspect ZIP contents
            using (var archive = ZipFile.OpenRead(zipPath))
            {
                Assert.NotNull(archive.GetEntry("manifest.json"));
                Assert.NotNull(archive.GetEntry("README.txt"));
                Assert.NotNull(archive.GetEntry("map/boden.bmp"));
                Assert.NotNull(archive.GetEntry("map/boden.ini"));
            }
        });
    }
}
