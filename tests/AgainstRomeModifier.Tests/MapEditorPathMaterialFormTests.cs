using System.Drawing;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    /// <summary>真實素材副本（ARM_COMPARE_GAME）：表單調色盤列出土路，於複製出的 ENDL_005（B8 地表）上繪製後產生原版 PFAD 邊界，並輸出 3D 截圖供目視。</summary>
    [Fact]
    public void Real_copy_form_paints_dirt_path_with_native_borders()
    {
        string? game = Environment.GetEnvironmentVariable("ARM_COMPARE_GAME");
        if (string.IsNullOrWhiteSpace(game) || !Directory.Exists(Path.Combine(game, "ENDL_005")) || !File.Exists(Path.Combine(game, "floortex.dat"))) return;
        string map = Path.Combine(_root, "PathCopy", "ENDL_005");
        CopyDirectory(Path.Combine(game, "ENDL_005"), map);
        string output = Environment.GetEnvironmentVariable("ARM_OPENGL_OUTPUT") ?? Path.Combine(_root, "PathCopy");
        Directory.CreateDirectory(output);
        RunInSta(() =>
        {
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(game, new GameMapInfo("ENDL_005", map, true, "Path copy", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents();

            var palette = GetField<ListBox>(form, "_palette");
            object path = palette.Items.Cast<object>().Single(item => item.ToString()!.Contains("土路") || item.ToString()!.Contains("Dirt Path"));
            palette.SelectedItem = path;
            Assert.Equal(FloorMaterialCatalog.PathMaterialId, GetField<FloorMaterial?>(form, "_activeMaterial")?.Id);
            foreach (var (x, y) in Enumerable.Range(20, 11).Select(x => (x, 30)).Concat(Enumerable.Range(31, 6).Select(y => (30, y))))
                Invoke(form, "PaintTexture", new TexturePaintEventArgs(x, y, "", ""));
            Invoke(form, "CommitStroke");

            var session = GetField<TerrainBlendEditSession>(form, "_terrainBlendSession");
            string[] borders = session.CurrentTextures.Where(texture => texture.StartsWith("PFAD", StringComparison.OrdinalIgnoreCase) && texture.Length > 5).Distinct().ToArray();
            Assert.NotEmpty(borders);
            Assert.All(borders, texture => Assert.Contains("_Erde", texture, StringComparison.OrdinalIgnoreCase)); // B8 是土地色，應選土地邊界
            Assert.Contains(session.CurrentTextures, texture => texture.Equals("PFAD1", StringComparison.OrdinalIgnoreCase) || texture.Length == 5);

            Invoke(form, "SetActiveView", true); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady) { Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED")); return; }
            view.FocusTile(26f, 32f);
            var camera = (EditorCamera)typeof(Map3DViewControl).GetField("_camera", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!; camera.ZoomToGameScale(1900);
            using Bitmap rendered = view.CaptureFrame(1024, 640)!;
            rendered.Save(Path.Combine(output, "editor-dirt-path.png"));
        }, TimeSpan.FromMinutes(2));
    }

    private static void CopyDirectory(string source, string target)
    {
        Directory.CreateDirectory(target);
        foreach (string file in Directory.GetFiles(source)) File.Copy(file, Path.Combine(target, Path.GetFileName(file)));
        foreach (string directory in Directory.GetDirectories(source)) CopyDirectory(directory, Path.Combine(target, Path.GetFileName(directory)));
    }
}
