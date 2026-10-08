using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;
using AgainstRomeMapEditor.Modules.Scripting.Parsing;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests.Scripting;

public sealed class EditorCommandParserTests
{
    [Fact]
    public void Parse_BasicCommandWithPositionalArgs_ParsesCorrectly()
    {
        string input = "/elevate rect 10 20 30 40 15";
        ParsedCommand? cmd = EditorCommandParser.Parse(input);

        Assert.NotNull(cmd);
        Assert.Equal("elevate", cmd.CommandName);
        Assert.Equal(6, cmd.PositionalArgs.Count);
        Assert.Equal("rect", cmd.PositionalArgs[0]);
        Assert.Equal("10", cmd.PositionalArgs[1]);
        Assert.Equal("20", cmd.PositionalArgs[2]);
        Assert.Equal("30", cmd.PositionalArgs[3]);
        Assert.Equal("40", cmd.PositionalArgs[4]);
        Assert.Equal("15", cmd.PositionalArgs[5]);
        Assert.Empty(cmd.NamedFlags);
    }

    [Fact]
    public void Parse_WithQuotesAndFlags_HandlesEscapesAndNamedArguments()
    {
        string input = "/scatter \"BauGer Wald 01\" 25 100,200,300,400 --spacing 64 --seed 42 --selected";
        ParsedCommand? cmd = EditorCommandParser.Parse(input);

        Assert.NotNull(cmd);
        Assert.Equal("scatter", cmd.CommandName);
        Assert.Equal(3, cmd.PositionalArgs.Count);
        Assert.Equal("BauGer Wald 01", cmd.PositionalArgs[0]);
        Assert.Equal("25", cmd.PositionalArgs[1]);
        Assert.Equal("100,200,300,400", cmd.PositionalArgs[2]);

        Assert.True(cmd.HasFlag("spacing"));
        Assert.Equal("64", cmd.GetFlag("spacing"));

        Assert.True(cmd.HasFlag("seed"));
        Assert.Equal("42", cmd.GetFlag("seed"));

        Assert.True(cmd.HasFlag("selected"));
        Assert.Null(cmd.GetFlag("selected")); // boolean flag has null value
    }

    [Fact]
    public void Parse_CommentsAndWhitespace_ReturnsNull()
    {
        Assert.Null(EditorCommandParser.Parse("   "));
        Assert.Null(EditorCommandParser.Parse("# This is a comment"));
        Assert.Null(EditorCommandParser.Parse("// Another comment style"));
    }

    [Fact]
    public void Parse_VariableSubstitution_ReplacesVariablesCorrectly()
    {
        var vars = new Dictionary<string, string>
        {
            ["tree"] = "LanGerTanne01",
            ["radius"] = "500"
        };

        string input = "/spawn-ring $tree 1000 2000 $radius 8";
        ParsedCommand? cmd = EditorCommandParser.Parse(input, vars);

        Assert.NotNull(cmd);
        Assert.Equal("LanGerTanne01", cmd.PositionalArgs[0]);
        Assert.Equal("500", cmd.PositionalArgs[3]);
    }

    [Fact]
    public void Geometry_RectAndCircleParsing_Succeeds()
    {
        Assert.True(ParsedCommand.TryParseRect("10,20,30,40", out MapRegionRect r));
        Assert.Equal(10, r.MinX);
        Assert.Equal(20, r.MinZ);
        Assert.Equal(30, r.MaxX);
        Assert.Equal(40, r.MaxZ);
        Assert.True(r.Contains(15, 25));
        Assert.False(r.Contains(5, 25));

        Assert.True(ParsedCommand.TryParseCircle("100,200,50", out MapRegionCircle c));
        Assert.Equal(100f, c.CenterX);
        Assert.Equal(200f, c.CenterZ);
        Assert.Equal(50f, c.Radius);
        Assert.True(c.Contains(110f, 210f));
        Assert.False(c.Contains(200f, 200f));
    }

    [Fact]
    public void TabCompletion_CommandNames_MatchesPrefix()
    {
        var registry = CommandRegistry.CreateDefault();
        var completions = EditorCommandParser.GetCompletions("/el", 3, null, registry.GetAllCommands());

        Assert.Contains("/elevate", completions);
    }

    [Fact]
    public void TabCompletion_Flags_MatchesParameterNames()
    {
        var registry = CommandRegistry.CreateDefault();
        var completions = EditorCommandParser.GetCompletions("/elevate --", 11, null, registry.GetAllCommands());

        Assert.Contains("--rect", completions);
        Assert.Contains("--circle", completions);
    }
}
