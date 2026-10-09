using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Console_tab_runs_macro_against_live_sessions_and_undoes()
    {
        string map = CreateFixture("ENDL_005");

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "ConsoleTest", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var console = GetField<EditorConsoleControl>(form, "_console");
            var layers = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            byte before = layers.Heights[10 * layers.VertexSize + 10];

            var result = Run(console, "/elevate rect 8 8 12 12 20");
            Assert.True(result.Success, result.Message);
            Assert.NotEqual(before, layers.Heights[10 * layers.VertexSize + 10]);

            Assert.True(Run(console, "/undo").Success);
            Assert.Equal(before, layers.Heights[10 * layers.VertexSize + 10]);
        });
    }

    [Fact]
    public void Console_tab_runs_multiple_macros_and_lands_on_intended_tiles()
    {
        string map = CreateFixture("ENDL_005");

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "ConsoleTest", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var console = GetField<EditorConsoleControl>(form, "_console");
            var context = GetField<AgainstRomeMapEditor.Modules.Scripting.Models.CommandExecutionContext>(form, "_consoleContext");
            var blendSession = GetField<TerrainBlendEditSession>(form, "_terrainBlendSession");
            var texturesDoc = GetField<BodenTexturesDocument>(form, "_texturesDocument");
            var placementSession = GetField<AgainstRomeMapEditor.Modules.Placement.PlacementEditSession>(form, "_placementSession");
            var heightSession = GetField<TerrainHeightEditSession>(form, "_terrainLayers");

            // 1. Verify context dimensions populated correctly by RefreshConsoleContext
            Assert.NotNull(context);
            Assert.Equal(64, context.MapTileDimension);
            Assert.Equal(16384f, context.WorldDimension);

            // 2. Test /replace-texture lands at intended tile (10, 10)
            int targetX = 10, targetZ = 10;
            string beforeTex = blendSession.GetTexture(targetX, targetZ);
            string newTex = context.KnownTextures.FirstOrDefault(t => !string.Equals(t, beforeTex, StringComparison.OrdinalIgnoreCase)) ?? "Gras2";

            var replaceRes = Run(console, $"/replace-texture {beforeTex} {newTex} --rect {targetX},{targetZ},{targetX},{targetZ}");
            Assert.True(replaceRes.Success, replaceRes.Message);
            Assert.Equal(newTex, blendSession.GetTexture(targetX, targetZ));
            Assert.Equal(newTex, texturesDoc.Textures[targetZ * 64 + targetX]);

            Assert.True(Run(console, "/undo").Success);
            Assert.Equal(beforeTex, blendSession.GetTexture(targetX, targetZ));
            Assert.Equal(beforeTex, texturesDoc.Textures[targetZ * 64 + targetX]);

            // 3. Test /spawn-ring places objects in world units (corresponding to tile 10, 10 -> world 2560, 2560)
            var towerType = new SdlObjectType("BauRomTurm01", 1, SdlObjectCategory.Building, "Rom", 1, new Dictionary<string, string> { ["alias"] = "BauRomTurm01" });
            typeof(MapEditorForm).GetField("_objectCatalog", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!.SetValue(form, new[] { towerType });
            form.RefreshConsoleContext();

            int beforePlacementCount = placementSession.Count;

            var spawnRes = Run(console, "/spawn-ring BauRomTurm01 2560 2560 256 4 --team 1");
            Assert.True(spawnRes.Success, spawnRes.Message);
            Assert.Equal(beforePlacementCount + 4, placementSession.Count);

            // Check that the placed objects are within the expected radius around (2560, 2560)
            for (int i = beforePlacementCount; i < placementSession.Count; i++)
            {
                var obj = placementSession[i];
                float dx = obj.WorldX - 2560f;
                float dz = obj.WorldZ - 2560f;
                float dist = MathF.Sqrt(dx * dx + dz * dz);
                Assert.InRange(dist, 255f, 257f);
            }

            Assert.True(Run(console, "/undo").Success);
            Assert.Equal(beforePlacementCount, placementSession.Count);
        });
    }

    private static AgainstRomeMapEditor.Modules.Scripting.Models.CommandResult Run(EditorConsoleControl console, string line)
    {
        var task = console.ExecuteLineAsync(line);
        var deadline = DateTime.UtcNow.AddSeconds(20);
        while (!task.IsCompleted && DateTime.UtcNow < deadline) { System.Windows.Forms.Application.DoEvents(); Thread.Sleep(5); }
        Assert.True(task.IsCompleted, "控制台指令逾時");
        return task.Result;
    }
}
