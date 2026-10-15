using System.Globalization;
using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;

namespace AgainstRomeMapEditor.Modules.Scripting.Commands;

internal sealed class BrushCommand : IEditorCommand
{
    public string Name => "brush";
    public IReadOnlyList<string> Aliases => Array.Empty<string>();
    public string Category => "Terrain";
    public string DescriptionZh => "使用編輯器筆刷沿 tile 座標路徑編輯高度、通行或材質。";
    public string DescriptionEn => "Apply the editor brush along a path of tile coordinates.";
    public string UsageSyntax => "/brush height <raise|lower|smooth|flatten|roughen|water> <x,y> ... [--size 1|3|5|9|15] [--strength gentle|medium|strong] | /brush collision <block|passable> <x,y> ... | /brush material <materialId> <x,y> ... [--size N] [--auto-bridge]";
    public IReadOnlyList<CommandParameter> Parameters => new[]
    {
        new CommandParameter("tool", CommandParameterType.String, "筆刷工具", "Brush tool", IsRequired: true, PossibleValues: new[] { "height", "collision", "material" }),
        new CommandParameter("size", CommandParameterType.Integer, "筆刷大小", "Brush size", IsRequired: false),
        new CommandParameter("strength", CommandParameterType.String, "筆刷強度", "Brush strength", IsRequired: false, PossibleValues: new[] { "gentle", "medium", "strong" }),
        new CommandParameter("auto-bridge", CommandParameterType.Flag, "自動過渡", "Automatic material bridge", IsRequired: false)
    };

    public CommandResult Execute(ParsedCommand command, CommandExecutionContext context)
    {
        string? toolName = command.GetPositional(0)?.ToLowerInvariant();
        EditorBrushTool tool;
        switch (toolName)
        {
            case "height": tool = EditorBrushTool.Height; break;
            case "collision": tool = EditorBrushTool.Collision; break;
            case "material": tool = EditorBrushTool.Material; break;
            default: return CommandResult.Fail(UsageSyntax);
        }
        string? argument = command.GetPositional(1);
        EditorHeightBrush operation = EditorHeightBrush.Raise;
        if (tool == EditorBrushTool.Height &&
            (!Enum.TryParse(argument, true, out operation) || !Enum.IsDefined(operation) || int.TryParse(argument, out _)))
            return CommandResult.Fail("未知的高度筆刷操作。");
        if (tool == EditorBrushTool.Collision && argument?.ToLowerInvariant() is not ("block" or "passable"))
            return CommandResult.Fail("通行筆刷須指定 block 或 passable。");
        if (string.IsNullOrWhiteSpace(argument)) return CommandResult.Fail(UsageSyntax);
        foreach (string flag in command.NamedFlags.Keys)
            if (flag.ToLowerInvariant() is not ("size" or "strength" or "auto-bridge"))
                return CommandResult.Fail($"未知的筆刷旗標：--{flag}");
        if (command.HasFlag("strength") && tool != EditorBrushTool.Height || command.HasFlag("auto-bridge") && tool != EditorBrushTool.Material)
            return CommandResult.Fail("此旗標不適用於選取的筆刷。");
        int size = 1;
        if (command.HasFlag("size") && (!command.TryGetFlagInt("size", out size) || size <= 0 ||
            tool != EditorBrushTool.Material && !EditorBrushStroke.BrushSizes.Contains(size)))
            return CommandResult.Fail("筆刷大小無效。");
        int strength = command.GetFlag("strength", "medium")?.ToLowerInvariant() switch
        {
            "gentle" => 0, "medium" => 1, "strong" => 2, _ => -1
        };
        if (strength < 0 || command.HasFlag("strength") && command.GetFlag("strength") is null)
            return CommandResult.Fail("筆刷強度須為 gentle、medium 或 strong。");
        if (command.HasFlag("auto-bridge") && command.GetFlag("auto-bridge") is not null)
            return CommandResult.Fail("--auto-bridge 不接受參數，請放在座標之後。");
        var points = new List<(int X, int Y)>();
        foreach (string raw in command.PositionalArgs.Skip(2))
        {
            string[] xy = raw.Split(',');
            if (xy.Length != 2 || !int.TryParse(xy[0], NumberStyles.Integer, CultureInfo.InvariantCulture, out int x) ||
                !int.TryParse(xy[1], NumberStyles.Integer, CultureInfo.InvariantCulture, out int y) ||
                x < 0 || y < 0 || x >= context.MapTileDimension || y >= context.MapTileDimension)
                return CommandResult.Fail($"tile 座標無效：{raw}");
            points.Add((x, y));
        }
        if (points.Count == 0 || context.MapTileDimension <= 0) return CommandResult.Fail(UsageSyntax);
        var heights = context.HeightSession;
        var blend = context.BlendSession;
        if (tool == EditorBrushTool.Material ? blend is null : heights is null)
            return CommandResult.Fail("此地圖沒有可編輯的筆刷圖層。");
        if (tool == EditorBrushTool.Collision && !heights!.HasCollision)
            return CommandResult.Fail("此地圖沒有可編輯的 collision.bmp。");
        if (tool == EditorBrushTool.Height && operation == EditorHeightBrush.Water &&
            (!float.IsFinite(context.HeightStep) || context.HeightStep <= 0 || !float.IsFinite(context.WaterLevel)))
            return CommandResult.Fail("水面或 HeightStep 無效。");
        var stroke = new EditorBrushStroke(tool, points, size, strength, operation,
            Block: !string.Equals(argument, "passable", StringComparison.OrdinalIgnoreCase),
            Material: tool == EditorBrushTool.Material ? argument : null, AutoBridge: command.HasFlag("auto-bridge"));
        EditorBrushResult result = EditorBrushExecutor.Execute(stroke, heights, blend, context.MapTileDimension,
            context.HeightStep > 0 ? context.WaterLevel / context.HeightStep : 0);
        bool heightCommitted = tool != EditorBrushTool.Material && heights!.CommitStroke();
        bool blendCommitted = tool == EditorBrushTool.Material && blend!.CommitStroke();
        if (heightCommitted || blendCommitted)
            context.RecordStep(
                () => { if (blendCommitted) blend!.Undo(); if (heightCommitted) heights!.Undo(); },
                () => { if (heightCommitted) heights!.Redo(); if (blendCommitted) blend!.Redo(); });
        int count = result.HeightSamplesChanged + result.CollisionPixelsChanged + result.MaterialTilesChanged;
        if (result.RejectedMaterialDabs > 0) return CommandResult.Fail("材質筆觸無法由原版 transition tile 表達。");
        if (result.Notice is not null && count == 0) return CommandResult.Fail(result.Notice);
        return CommandResult.Ok(result.Notice ?? $"筆刷已套用，變更 {count} 個樣本。", count);
    }

    /// <summary>Canonical command text; coordinates precede flags so boolean flags cannot consume a point.</summary>
    public static string Format(EditorBrushStroke stroke)
    {
        ArgumentNullException.ThrowIfNull(stroke);
        if (stroke.Path.Count == 0) throw new ArgumentException("Brush path must not be empty.", nameof(stroke));
        string argument = stroke.Tool switch
        {
            EditorBrushTool.Height => stroke.HeightOperation.ToString().ToLowerInvariant(),
            EditorBrushTool.Collision => stroke.Block ? "block" : "passable",
            EditorBrushTool.Material when !string.IsNullOrWhiteSpace(stroke.Material) =>
                "\"" + stroke.Material.Replace("\\", "\\\\").Replace("\"", "\\\"") + "\"",
            _ => throw new ArgumentException("Invalid brush tool or material.", nameof(stroke))
        };
        string path = string.Join(" ", stroke.Path.Select(p => FormattableString.Invariant($"{p.X},{p.Y}")));
        string text = FormattableString.Invariant($"/brush {stroke.Tool.ToString().ToLowerInvariant()} {argument} {path} --size {stroke.Size}");
        if (stroke.Tool == EditorBrushTool.Height)
            text += " --strength " + (stroke.StrengthLevel switch { 0 => "gentle", 1 => "medium", 2 => "strong", _ => throw new ArgumentException("Invalid strength.", nameof(stroke)) });
        if (stroke.Tool == EditorBrushTool.Material && stroke.AutoBridge) text += " --auto-bridge";
        return text;
    }
}
