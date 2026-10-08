using System.Drawing;
using System.Reflection;
using System.Text.Json;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Nature;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    /// <summary>選擇性啟用（ARM_TOOLS_SHOWCASE=1）：在真實素材 TEMP 副本上套用河流／懸崖／植被／侵蝕／城牆／天候並存檔，保留成果供遊戲內檢視。</summary>
    [Fact]
    public void Tools_showcase_real_copy_applies_river_cliff_flora_erosion_wall_weather_and_saves()
    {
        if (Environment.GetEnvironmentVariable("ARM_TOOLS_SHOWCASE") != "1") return;
        string source = RequireDemoTempPath(Environment.GetEnvironmentVariable("ARM_COMPARE_GAME"));
        string output = RequireDemoTempPath(Environment.GetEnvironmentVariable("ARM_SHOWCASE_OUTPUT"));
        Assert.False(Directory.Exists(output), "Use a new output directory.");
        string game = Path.Combine(output, "game"), map = Path.Combine(game, "MAPS", "ENDL_005");
        CopyDemoDirectory(Path.Combine(source, "ENDL_005"), map);
        CopyDemoDirectory(Path.Combine(source, "ENDL_000"), Path.Combine(game, "MAPS", "ENDL_000"));
        CopyDemoDirectory(Path.Combine(source, "SYSTEM"), Path.Combine(game, "SYSTEM"));
        foreach (string asset in new[] { "floortex.dat", "alr.dat", "apt.dat", "shad.dat" }) File.Copy(Path.Combine(source, asset), Path.Combine(game, asset));
        var report = new Dictionary<string, object>();

        RunInSta(() =>
        {
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(game, new GameMapInfo("ENDL_005", map, true, "ToolsShowcase", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents();
            string Status() => GetField<ToolStripStatusLabel>(form, "_status").Text ?? "";
            var layers = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            int size = layers.VertexSize, step = (size - 1) / 64;
            var textures = GetField<BodenTexturesDocument>(form, "_texturesDocument");

            // 高台：tile (36..42, 26..32) 抬高，邊緣形成陡坡供懸崖工具使用。
            var plateau = new List<TerrainSampleChange>();
            for (int z = 26 * step; z <= 32 * step; z++)
                for (int x = 36 * step; x <= 42 * step; x++)
                    plateau.Add(new TerrainSampleChange(z * size + x, layers.Heights[z * size + x], (byte)Math.Min(255, layers.Heights[z * size + x] + 110)));
            layers.ApplySampleChanges(plateau); layers.CommitStroke();
            form.ApplyCliffTool(new Rectangle(34, 24, 11, 11));
            report["cliff"] = Status();
            form.ApplyErosionTool(new Rectangle(34, 24, 11, 11));
            report["erosion"] = Status();
            form.ApplyRiverTool([(20, 22), (26, 28), (24, 36)]);
            report["river"] = Status();

            SetWorkflowMode(form, "Nature");
            var natureTask = GetField<Task>(form, "_natureCatalogTask");
            DateTime deadline = DateTime.UtcNow.AddSeconds(60);
            var templates = typeof(MapEditorForm).GetField("_natureTemplates", BindingFlags.Instance | BindingFlags.NonPublic)!;
            while ((!natureTask.IsCompleted || ((System.Collections.ICollection)templates.GetValue(form)!).Count == 0) && DateTime.UtcNow < deadline) { Application.DoEvents(); Thread.Sleep(20); }
            Assert.True(natureTask.IsCompletedSuccessfully);
            report["floraPlanted"] = form.ApplyFloraScatter(new Rectangle(26, 36, 16, 12)); report["flora"] = Status();

            report["wallPieces"] = form.ApplyWallTool([(24, 18), (32, 18), (32, 22)], team: 0); report["wall"] = Status();
            report["weather"] = form.ApplyWeatherPreset("storm");

            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
            report["saved"] = true;
            using var reopened = new MapEditorForm(game, new GameMapInfo("ENDL_005", map, true, "ToolsShowcase", "Test"));
            _ = reopened.Handle; Invoke(reopened, "LoadSelectedMap");
            Assert.Equal(layers.Heights.ToArray(), GetField<TerrainHeightEditSession>(reopened, "_terrainLayers").Heights.ToArray());
            Assert.Equal(textures.Textures.ToArray(), GetField<BodenTexturesDocument>(reopened, "_texturesDocument").Textures.ToArray());
            report["reloadMatches"] = true;
            Invoke(form, "SetActiveView", true); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (view.IsReady) { view.FocusTile(32, 28); using var image = view.CaptureFrame(1280, 800)!; image.Save(Path.Combine(output, "showcase-3d.png")); report["screenshot"] = true; }
        }, TimeSpan.FromMinutes(3));
        File.WriteAllText(Path.Combine(output, "result.json"), JsonSerializer.Serialize(report, new JsonSerializerOptions { WriteIndented = true }));
    }
}
