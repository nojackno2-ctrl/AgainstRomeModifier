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

    private static AgainstRomeMapEditor.Modules.Scripting.Models.CommandResult Run(EditorConsoleControl console, string line)
    {
        var task = console.ExecuteLineAsync(line);
        var deadline = DateTime.UtcNow.AddSeconds(20);
        while (!task.IsCompleted && DateTime.UtcNow < deadline) { System.Windows.Forms.Application.DoEvents(); Thread.Sleep(5); }
        Assert.True(task.IsCompleted, "控制台指令逾時");
        return task.Result;
    }
}
