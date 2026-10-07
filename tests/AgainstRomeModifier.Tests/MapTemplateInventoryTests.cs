using System.Drawing;
using System.Windows.Forms;
using AgainstRomeMapEditor;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Creation_inventory_is_read_only_and_reports_retained_blueprints_scripts_and_missing_native_data()
    {
        string map = CreateFixture();
        string sdl = Path.Combine(map, "Endlos_Rom_Siedlung1.sdl");
        File.AppendAllText(sdl, "onload=1\r\n");
        Directory.CreateDirectory(Path.Combine(map, "SCRIPT")); File.WriteAllBytes(Path.Combine(map, "SCRIPT", "ak_level.bci"), [1, 2, 3]);
        var before = SnapshotDirectory(map);
        var inventory = MapTemplateInventory.Read(_root, map);
        Assert.Equal((1, 1, 1, 1), (inventory.SdlFiles, inventory.SdlObjects, inventory.OnloadObjects, inventory.Scripts));
        Assert.Null(inventory.ProtectedNative); Assert.Contains("DATA native objects", inventory.Unknown);
        Assert.Contains("聚落與腳本保留", inventory.Describe("ENDL_005", true, false));
        Assert.Contains("flat template", inventory.Describe("ENDL_005", true, true));
        AssertSnapshotUnchanged(map, before);
    }

    [Fact]
    public void Creation_inventory_of_real_temp_template_matches_decoded_evidence_and_keeps_source_bytes()
    {
        string? game = Environment.GetEnvironmentVariable("ARM_COMPARE_GAME");
        if (string.IsNullOrWhiteSpace(game)) return;
        string source = RequireDemoTempPath(game), map = Path.Combine(source, "ENDL_000");
        var hashes = DemoHashes(map);
        var inventory = MapTemplateInventory.Read(source, map);
        Assert.Equal(8, inventory.SdlFiles); Assert.True(inventory.SdlObjects > 0);
        Assert.Equal(1, inventory.Scripts); Assert.NotNull(inventory.RemovableNature); Assert.True(inventory.RemovableNature > 0);
        Assert.NotNull(inventory.ProtectedNative); Assert.True(inventory.ProtectedNative > 0); Assert.Empty(inventory.Unknown);
        Assert.Equal(hashes, DemoHashes(map));
        RunInSta(() =>
        {
            foreach (bool en in new[] { false, true })
            {
                using var dialog = MapSelectionForm.BuildNameDialog(en ? "Build Flat Template" : "建立平坦範本地圖", "Flat Template Map",
                    inventory.Describe("ENDL_000", true, en), en, Color.FromArgb(24, 28, 34), Color.White, out TextBox input);
                dialog.StartPosition = FormStartPosition.Manual; dialog.Location = new Point(-30000, -30000); dialog.Show(); Application.DoEvents();
                Assert.Equal("Flat Template Map", input.Text);
                var details = Assert.Single(dialog.Controls.OfType<TextBox>(), control => control.ReadOnly);
                Assert.True(details.ClientSize.Height > 150); Assert.Contains("ENDL_000", details.Text);
                string? output = Environment.GetEnvironmentVariable("ARM_CREATION_OUTPUT");
                if (output is not null)
                {
                    Directory.CreateDirectory(output); using var image = new Bitmap(dialog.Width, dialog.Height);
                    dialog.DrawToBitmap(image, new Rectangle(Point.Empty, dialog.Size)); image.Save(Path.Combine(output, en ? "flat-creation-en.png" : "flat-creation-zh.png"));
                }
            }
        });
    }
}
