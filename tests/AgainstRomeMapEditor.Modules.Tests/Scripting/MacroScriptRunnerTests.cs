using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests.Scripting;

public sealed class MacroScriptRunnerTests
{
    private static (TerrainHeightEditSession HeightSession, CommandExecutionContext Context, MacroScriptRunner Runner) CreateTestContext()
    {
        byte[] heights = new byte[16 * 16];
        Array.Fill(heights, (byte)50);

        var heightSession = new TerrainHeightEditSession(16, heights, emboss: null, collisionSize: 16, collision: null);
        var registry = CommandRegistry.CreateDefault();
        var runner = new MacroScriptRunner(registry);

        var context = new CommandExecutionContext
        {
            HeightSession = heightSession,
            MapTileDimension = 16,
            WorldDimension = 1024f
        };

        return (heightSession, context, runner);
    }

    [Fact]
    public void ExecuteLine_VariableAssignment_StoresVariable()
    {
        var (_, context, runner) = CreateTestContext();

        CommandResult res = runner.ExecuteLine("$delta = 20", context);
        Assert.True(res.Success);
        Assert.True(context.Variables.ContainsKey("delta"));
        Assert.Equal("20", context.Variables["delta"]);
    }

    [Fact]
    public void ExecuteScript_ValidMacro_SucceedsAndBundlesIntoSingleUndo()
    {
        var (heightSession, context, runner) = CreateTestContext();

        string script = @"
# @name TestElevationMacro
$step = 10
/elevate rect 2 2 4 4 $step
/elevate rect 5 5 7 7 15
";

        var report = runner.ExecuteScript(script, context, transactional: true);

        Assert.True(report.Success);
        // The variable assignment is also an executed statement.
        Assert.Equal(3, report.TotalCommands);
        Assert.Equal(3, report.SucceededCommands);
        Assert.Equal(0, report.FailedCommands);
        Assert.False(report.WasRolledBack);

        // 頂點 (3,3) 原本 50，現在應為 60
        Assert.Equal(60, heightSession.Heights[3 * 16 + 3]);
        // 頂點 (6,6) 原本 50，現在應為 65
        Assert.Equal(65, heightSession.Heights[6 * 16 + 6]);

        // 單次 Undo 撤銷全部巨集步驟
        Assert.True(runner.CanUndo);
        var undone = runner.Undo();
        Assert.NotNull(undone);
        Assert.Equal(2, undone.StepCount);
        Assert.False(runner.CanUndo);

        Assert.Equal(50, heightSession.Heights[3 * 16 + 3]);
        Assert.Equal(50, heightSession.Heights[6 * 16 + 6]);

        // 單次 Redo 重新套用全部巨集步驟
        Assert.True(runner.CanRedo);
        var redone = runner.Redo();
        Assert.NotNull(redone);

        Assert.Equal(60, heightSession.Heights[3 * 16 + 3]);
        Assert.Equal(65, heightSession.Heights[6 * 16 + 6]);
    }

    [Fact]
    public void ExecuteScript_FailsMidway_PerformsAtomicRollback()
    {
        var (heightSession, context, runner) = CreateTestContext();

        string failingScript = @"
# @name PartialFailMacro
/elevate rect 2 2 4 4 25
/non_existent_command_error
/elevate rect 5 5 7 7 25
";

        var report = runner.ExecuteScript(failingScript, context, transactional: true);

        Assert.False(report.Success);
        Assert.True(report.WasRolledBack);
        Assert.Equal(1, report.FailedCommands);

        // 關鍵斷言：第 1 步的 elevate 必須被原子性回滾，狀態恢復 50
        Assert.Equal(50, heightSession.Heights[3 * 16 + 3]);
        Assert.Equal(50, heightSession.Heights[6 * 16 + 6]);
    }
}
