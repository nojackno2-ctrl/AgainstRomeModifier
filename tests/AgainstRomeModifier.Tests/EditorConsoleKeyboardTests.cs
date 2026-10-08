using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;
using static AgainstRomeModifier.Tests.DialogControlTestSupport;

namespace AgainstRomeModifier.Tests;

public sealed class EditorConsoleKeyboardTests
{
    [Fact]
    public void Tab_completes_and_cycles_matching_commands_and_escape_resets_the_cycle() => InSta(() =>
    {
        using var console = new EditorConsoleControl();
        console.Bind(new CommandExecutionContext(), new MacroScriptRunner(CommandRegistry.CreateDefault()));
        var input = Assert.Single(Descendants<TextBox>(console));
        var observedKeys = new List<(Keys Key, bool Handled, bool Suppressed)>();
        input.KeyDown += (_, e) =>
        {
            observedKeys.Add((e.KeyCode, e.Handled, e.SuppressKeyPress));
        };
        input.Text = "/he";
        input.SelectionStart = input.TextLength;
        PressKey(input, Keys.Tab);
        Assert.Equal("/heal-navmesh", input.Text);
        Assert.Equal(input.TextLength, input.SelectionStart);
        PressKey(input, Keys.Tab);
        Assert.Equal("/heal-roads", input.Text);
        PressKey(input, Keys.Tab);
        Assert.Equal("/height-add", input.Text);
        PressKey(input, Keys.Tab);
        Assert.Equal("/help", input.Text);
        PressKey(input, Keys.Tab);
        Assert.Equal("/heal-navmesh", input.Text);
        PressKey(input, Keys.Escape);
        Assert.Empty(input.Text);
        input.Text = "/und";
        input.SelectionStart = input.TextLength;
        PressKey(input, Keys.Tab);
        Assert.Equal("/undo", input.Text);
        Assert.Equal(new[] { Keys.Tab, Keys.Tab, Keys.Tab, Keys.Tab, Keys.Tab, Keys.Escape, Keys.Tab }, observedKeys.Select(e => e.Key));
        Assert.All(observedKeys, e => Assert.True(e.Handled));
        Assert.All(observedKeys.Where(e => e.Key == Keys.Tab), e => Assert.True(e.Suppressed));
    });

    [Fact]
    public void Tab_without_a_matching_command_preserves_input() => InSta(() =>
    {
        using var console = new EditorConsoleControl();
        console.Bind(new CommandExecutionContext(), new MacroScriptRunner(CommandRegistry.CreateDefault()));
        var input = Assert.Single(Descendants<TextBox>(console));
        input.Text = "/no-such-command";
        input.SelectionStart = input.TextLength;
        PressKey(input, Keys.Tab);
        Assert.Equal("/no-such-command", input.Text);
    });

    [Fact]
    public void Up_down_history_respects_boundaries_and_restores_the_unsent_draft() => InSta(() =>
    {
        using var console = new EditorConsoleControl();
        var input = Assert.Single(Descendants<TextBox>(console));
        var observedKeys = new List<(Keys Key, bool Handled, bool Suppressed)>();
        input.KeyDown += (_, e) => observedKeys.Add((e.KeyCode, e.Handled, e.SuppressKeyPress));
        input.Text = "unsent draft";
        PressKey(input, Keys.Up);
        PressKey(input, Keys.Down);
        Assert.Equal("unsent draft", input.Text);
        input.Text = "   ";
        PressKey(input, Keys.Enter);
        input.Text = "unsent draft";
        PressKey(input, Keys.Up);
        Assert.Equal("unsent draft", input.Text);

        // History is recorded before execution; leave unbound to avoid asynchronous command work.
        input.Text = "  /echo first  ";
        PressKey(input, Keys.Enter);
        Assert.Empty(input.Text);
        input.Text = "/echo second";
        PressKey(input, Keys.Enter);
        Assert.Empty(input.Text);
        input.Text = "unsent draft";
        PressKey(input, Keys.Up);
        Assert.Equal("/echo second", input.Text);
        PressKey(input, Keys.Up);
        Assert.Equal("/echo first", input.Text);
        PressKey(input, Keys.Up);
        Assert.Equal("/echo first", input.Text);
        PressKey(input, Keys.Down);
        Assert.Equal("/echo second", input.Text);
        PressKey(input, Keys.Down);
        Assert.Equal("unsent draft", input.Text);
        PressKey(input, Keys.Down);
        Assert.Equal("unsent draft", input.Text);
        Assert.Equal(input.TextLength, input.SelectionStart);
        Assert.Equal(new[] { Keys.Up, Keys.Down, Keys.Enter, Keys.Up, Keys.Enter, Keys.Enter, Keys.Up, Keys.Up, Keys.Up, Keys.Down, Keys.Down, Keys.Down }, observedKeys.Select(e => e.Key));
        Assert.All(observedKeys, e => Assert.True(e.Handled));
    });
}




