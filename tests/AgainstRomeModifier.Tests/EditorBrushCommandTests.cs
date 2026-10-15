using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Scripting.Commands;
using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;
using AgainstRomeMapEditor.Modules.Scripting.Parsing;

namespace AgainstRomeModifier.Tests;

public sealed class EditorBrushCommandTests
{
    private static TerrainHeightEditSession Heights(byte value = 100) =>
        new(33, Enumerable.Repeat(value, 33 * 33).ToArray(), null, 32, new byte[32 * 32]);

    [Theory]
    [InlineData(1, 0)]
    [InlineData(3, 1)]
    [InlineData(5, 2)]
    [InlineData(9, 1)]
    [InlineData(15, 2)]
    public void Single_raise_equals_direct_editor_brush(int size, int strength)
    {
        var actual = Heights();
        var expected = Heights();
        var stroke = new EditorBrushStroke(EditorBrushTool.Height, new[] { (3, 4) }, size, strength);
        var result = EditorBrushExecutor.Execute(stroke, actual, null, 8, 20);
        var changes = expected.PaintHeight(3.5f * 4, 4.5f * 4, stroke.RadiusTiles * 4 + 1,
            TerrainHeightOperation.Raise, EditorBrushStroke.StrengthValue(strength));
        Assert.Equal(expected.Heights, actual.Heights);
        Assert.Equal(changes.Count, result.HeightSamplesChanged);
    }

    [Fact]
    public void Path_interpolates_and_deduplicates_revisited_tiles()
    {
        var actual = Heights();
        var expected = Heights();
        var stroke = new EditorBrushStroke(EditorBrushTool.Height, new[] { (1, 2), (5, 2), (1, 2), (1, 2) });
        EditorBrushExecutor.Execute(stroke, actual, null, 8, 20);
        for (int x = 1; x <= 5; x++)
            expected.PaintHeight((x + .5f) * 4, 2.5f * 4, stroke.RadiusTiles * 4 + 1, TerrainHeightOperation.Raise, 6);
        Assert.Equal(expected.Heights, actual.Heights);
    }

    [Theory]
    [InlineData("Raise")]
    [InlineData("Water")]
    public void Protected_tile_and_shared_vertices_remain_unchanged(string operation)
    {
        var session = Heights(40);
        var before = session.Heights.ToArray();
        var stroke = new EditorBrushStroke(EditorBrushTool.Height, new[] { (3, 3) }, 5, 2, Enum.Parse<EditorHeightBrush>(operation));
        var result = EditorBrushExecutor.Execute(stroke, session, null, 8, 32, allowsTile: (x, y) => x != 4 || y != 3);
        Assert.True(result.HeightSamplesChanged > 0);
        for (int y = 12; y <= 16; y++)
        for (int x = 16; x <= 20; x++)
            Assert.Equal(before[y * 33 + x], session.Heights[y * 33 + x]);
        for (int y = 12; y < 16; y++)
        for (int x = 16; x < 20; x++)
            Assert.Equal(0, session.Collision![y * 32 + x]);
        Assert.Equal(before.Where((value, index) => value != session.Heights[index]).Count(), result.HeightSamplesChanged);
        Assert.Equal(session.Collision!.Count(value => value != 0), result.CollisionPixelsChanged);
        Assert.True(session.CommitStroke());
        session.Undo();
        Assert.Equal(before, session.Heights);
        Assert.All(session.Collision!, value => Assert.Equal(0, value));
    }

    [Fact]
    public void Collision_respects_protection_and_block_passable()
    {
        var session = Heights();
        var stroke = new EditorBrushStroke(EditorBrushTool.Collision, new[] { (3, 3) }, 5);
        var result = EditorBrushExecutor.Execute(stroke, session, null, 8, 0, allowsTile: (x, _) => x < 4);
        Assert.True(result.CollisionPixelsChanged > 0);
        for (int y = 0; y < 32; y++)
        for (int x = 16; x < 32; x++) Assert.Equal(0, session.Collision![y * 32 + x]);
        EditorBrushExecutor.Execute(stroke with { Block = false }, session, null, 8, 0);
        Assert.All(session.Collision!, value => Assert.Equal(0, value));
        Assert.False(session.CommitStroke());
    }

    [Fact]
    public void Low_water_returns_notice_without_changes()
    {
        var session = Heights();
        var result = EditorBrushExecutor.Execute(new(EditorBrushTool.Height, new[] { (3, 3) }, HeightOperation: EditorHeightBrush.Water),
            session, null, 8, 3.9f);
        Assert.NotNull(result.Notice);
        Assert.Equal(0, result.HeightSamplesChanged);
        Assert.Equal(0, result.CollisionPixelsChanged);
        Assert.False(session.CommitStroke());
    }

    [Fact]
    public void Incremental_UI_flatten_keeps_first_target_like_whole_path()
    {
        var samples = Enumerable.Range(0, 33 * 33).Select(i => (byte)(30 + i % 33 * 3)).ToArray();
        var actual = new TerrainHeightEditSession(33, samples, null, 0, null);
        var expected = new TerrainHeightEditSession(33, samples, null, 0, null);
        var stroke = new EditorBrushStroke(EditorBrushTool.Height, new[] { (1, 3), (2, 3), (3, 3) }, 3, 2, EditorHeightBrush.Flatten);
        EditorBrushExecutor.Execute(stroke, expected, null, 8, 0);
        int target = -1;
        foreach (var point in stroke.Path)
            EditorBrushExecutor.Execute(stroke with { Path = new[] { point } }, actual, null, 8, 0, ref target);
        Assert.Equal(expected.Heights, actual.Heights);
    }

    [Theory]
    [InlineData("raise")]
    [InlineData("lower")]
    [InlineData("smooth")]
    [InlineData("flatten")]
    [InlineData("roughen")]
    [InlineData("water")]
    public void Height_format_parse_execute_matches_executor_and_undo(string operation)
    {
        byte[] samples = Enumerable.Range(0, 33 * 33).Select(i => (byte)(60 + i % 33 + (i % 7 == 0 ? 20 : 0))).ToArray();
        var actual = new TerrainHeightEditSession(33, samples, null, 32, new byte[32 * 32]);
        var expected = new TerrainHeightEditSession(33, samples, null, 32, new byte[32 * 32]);
        var brush = Enum.Parse<EditorHeightBrush>(operation, true);
        var stroke = new EditorBrushStroke(EditorBrushTool.Height, new[] { (2, 3), (4, 3) }, 3, 2, brush);
        string text = BrushCommand.Format(stroke);
        Assert.Equal($"/brush height {operation} 2,3 4,3 --size 3 --strength strong", text);
        var parsed = EditorCommandParser.Parse(text)!;
        Assert.Equal(new[] { "height", operation, "2,3", "4,3" }, parsed.PositionalArgs);
        var context = new CommandExecutionContext { HeightSession = actual, MapTileDimension = 8, WaterLevel = 128, HeightStep = 4 };
        var runner = new MacroScriptRunner(CommandRegistry.CreateDefault());
        var before = actual.Heights.ToArray();
        var result = runner.ExecuteLine(text, context, recordSingleActionAsCompound: true);
        Assert.True(result.Success, result.Message);
        EditorBrushExecutor.Execute(stroke, expected, null, 8, 32);
        Assert.Equal(expected.Heights, actual.Heights);
        Assert.Equal(expected.Collision, actual.Collision);
        if (result.AffectedCount == 0) { Assert.False(runner.CanUndo); return; }
        Assert.NotNull(runner.Undo());
        Assert.Equal(before, actual.Heights);
        Assert.All(actual.Collision!, value => Assert.Equal(0, value));
        Assert.NotNull(runner.Redo());
        Assert.Equal(expected.Heights, actual.Heights);
        Assert.Equal(expected.Collision, actual.Collision);
    }

    [Theory]
    [InlineData(true)]
    [InlineData(false)]
    public void Collision_format_round_trip_and_undo(bool block)
    {
        var session = Heights();
        if (!block) { session.PaintCollision(16, 16, 50, TerrainCollisionOperation.Block); session.CommitStroke(); }
        var before = session.Collision!.ToArray();
        var stroke = new EditorBrushStroke(EditorBrushTool.Collision, new[] { (3, 3), (4, 3) }, Block: block);
        var context = new CommandExecutionContext { HeightSession = session, MapTileDimension = 8 };
        var runner = new MacroScriptRunner(CommandRegistry.CreateDefault());
        var result = runner.ExecuteLine(BrushCommand.Format(stroke), context, true);
        Assert.True(result.Success, result.Message);
        Assert.True(result.AffectedCount > 0);
        var after = session.Collision!.ToArray();
        runner.Undo(); Assert.Equal(before, session.Collision);
        runner.Redo(); Assert.Equal(after, session.Collision);
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Material_format_round_trip_preserves_id_and_undo(bool autoBridge)
    {
        const string material = "grass \\\"quoted";
        var resolver = new Resolver(material);
        string[] textures = Enumerable.Repeat("base", 64).ToArray();
        var session = new TerrainBlendEditSession(new NativeTerrainImportResult(new TerrainBlendAuthoringMap(8, "base"), [], []), textures, resolver);
        var stroke = new EditorBrushStroke(EditorBrushTool.Material, new[] { (2, 3), (4, 3) }, 3, Material: material, AutoBridge: autoBridge);
        string text = BrushCommand.Format(stroke);
        var parsed = EditorCommandParser.Parse(text)!;
        Assert.Equal(material, parsed.GetPositional(1));
        Assert.Equal(autoBridge, parsed.HasFlag("auto-bridge"));
        var context = new CommandExecutionContext { BlendSession = session, MapTileDimension = 8 };
        var runner = new MacroScriptRunner(CommandRegistry.CreateDefault());
        var result = runner.ExecuteLine(text, context, true);
        Assert.True(result.Success, result.Message);
        Assert.True(result.AffectedCount > 0);
        var after = session.CurrentTextures.ToArray();
        runner.Undo(); Assert.Equal(textures, session.CurrentTextures);
        runner.Redo(); Assert.Equal(after, session.CurrentTextures);
    }

    [Theory]
    [InlineData("/brush height raise 1,1 --size 2")]
    [InlineData("/brush height raise 1,1 --strength invalid")]
    [InlineData("/brush height raise 1,1 --size")]
    [InlineData("/brush height raise 1,1 --typo 3")]
    [InlineData("/brush height raise 1,1 invalid")]
    [InlineData("/brush height raise 8,1")]
    [InlineData("/brush height 1 1,1")]
    [InlineData("/brush collision invalid 1,1")]
    [InlineData("/brush height raise")]
    [InlineData("/brush height raise 1,1 --auto-bridge")]
    public void Invalid_command_is_rejected_before_editing(string text)
    {
        var session = Heights();
        var result = new BrushCommand().Execute(EditorCommandParser.Parse(text)!, new CommandExecutionContext { HeightSession = session, MapTileDimension = 8 });
        Assert.False(result.Success);
        Assert.False(session.CommitStroke());
        Assert.False(session.IsDirty);
    }

    private sealed class Resolver(string material) : INativeTerrainMaterialResolver
    {
        public bool TryResolveNativeCorners(string texture, out IReadOnlyList<string> corners)
        { corners = Enumerable.Repeat("base", 4).ToArray(); return true; }
        public string? ResolveNativeTile(IReadOnlyList<string> corners, int x, int y) => corners.Contains(material) ? "painted" : "base";
    }
}
