using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;

namespace AgainstRomeMapEditor.Modules.Scripting.Commands;

/// <summary>
/// 高程批量調整指令 (/elevate)。
/// </summary>
internal sealed class ElevateCommand : IEditorCommand
{
    public string Name => "elevate";
    public IReadOnlyList<string> Aliases => new[] { "height-add", "raise" };
    public string Category => "Terrain";
    public string DescriptionZh => "在指定矩形或圓形範圍內批量增加或降低地形高度。";
    public string DescriptionEn => "Bulk elevate or lower terrain height in a rectangular or circular region.";
    public string UsageSyntax => "/elevate rect <x1> <z1> <x2> <z2> <delta> 或 /elevate circle <cx> <cz> <radius> <delta>";

    public IReadOnlyList<CommandParameter> Parameters => new[]
    {
        new CommandParameter("shape", CommandParameterType.String, "區域幾何形狀 (rect 或 circle)", "Region shape", IsRequired: true, PossibleValues: new[] { "rect", "circle" }),
        new CommandParameter("delta", CommandParameterType.Integer, "高度變化量 (-255 ~ 255)", "Height delta", IsRequired: true),
        new CommandParameter("rect", CommandParameterType.Rectangle, "矩形區域 (x1,z1,x2,z2)", "Rectangle region", IsRequired: false),
        new CommandParameter("circle", CommandParameterType.Circle, "圓形區域 (cx,cz,radius)", "Circle region", IsRequired: false)
    };

    public CommandResult Execute(ParsedCommand command, CommandExecutionContext context)
    {
        if (context.HeightSession is null)
        {
            return CommandResult.Fail("地形高度會話 (TerrainHeightEditSession) 未就緒。");
        }

        int size = context.HeightSession.VertexSize;
        if (size <= 0) return CommandResult.Fail("無效的高程場網格尺寸。");

        var targets = new List<(int Index, byte TargetHeight)>();
        int delta = 0;

        // 判斷旗標模式或位置引數模式
        if (command.HasFlag("rect") || command.HasFlag("circle"))
        {
            if (!command.TryGetFlagInt("delta", out delta) && !command.TryGetPositionalInt(0, out delta))
            {
                return CommandResult.Fail("缺少必需的 --delta 數值。");
            }

            if (command.HasFlag("rect"))
            {
                string rectRaw = command.GetFlag("rect") ?? "";
                if (!ParsedCommand.TryParseRect(rectRaw, out MapRegionRect r))
                    return CommandResult.Fail("無法解析 --rect 區域。格式: x1,z1,x2,z2");

                targets = CollectRect(context.HeightSession, r, delta);
            }
            else if (command.HasFlag("circle"))
            {
                string circleRaw = command.GetFlag("circle") ?? "";
                if (!ParsedCommand.TryParseCircle(circleRaw, out MapRegionCircle c))
                    return CommandResult.Fail("無法解析 --circle 區域。格式: cx,cz,radius");

                targets = CollectCircle(context.HeightSession, c, delta);
            }
        }
        else
        {
            string? mode = command.GetPositional(0)?.ToLowerInvariant();
            if (mode is "rect")
            {
                if (!command.TryGetPositionalInt(1, out int x1) ||
                    !command.TryGetPositionalInt(2, out int z1) ||
                    !command.TryGetPositionalInt(3, out int x2) ||
                    !command.TryGetPositionalInt(4, out int z2) ||
                    !command.TryGetPositionalInt(5, out delta))
                {
                    return CommandResult.Fail("語法錯誤。用法: /elevate rect <x1> <z1> <x2> <z2> <delta>");
                }
                var r = new MapRegionRect(Math.Min(x1, x2), Math.Min(z1, z2), Math.Max(x1, x2), Math.Max(z1, z2));
                targets = CollectRect(context.HeightSession, r, delta);
            }
            else if (mode is "circle")
            {
                if (!command.TryGetPositionalFloat(1, out float cx) ||
                    !command.TryGetPositionalFloat(2, out float cz) ||
                    !command.TryGetPositionalFloat(3, out float radius) ||
                    !command.TryGetPositionalInt(4, out delta))
                {
                    return CommandResult.Fail("語法錯誤。用法: /elevate circle <cx> <cz> <radius> <delta>");
                }
                var c = new MapRegionCircle(cx, cz, radius);
                targets = CollectCircle(context.HeightSession, c, delta);
            }
            else
            {
                return CommandResult.Fail($"未知的幾何形狀「{mode}」。請指定 rect 或 circle。");
            }
        }

        if (targets.Count == 0)
        {
            return CommandResult.Ok("目標區域無符合頂點或高度無變化。", 0);
        }

        context.HeightSession.ApplyHeightAdjustments(targets);
        bool committed = context.HeightSession.CommitStroke();

        if (committed)
        {
            context.RecordStep(() => context.HeightSession.Undo(), () => context.HeightSession.Redo());
        }

        return CommandResult.Ok($"成功調整 {targets.Count} 個高程頂點 (delta = {delta})。", targets.Count);
    }

    private static List<(int Index, byte TargetHeight)> CollectRect(TerrainHeightEditSession session, MapRegionRect r, int delta)
    {
        int size = session.VertexSize;
        int minX = Math.Clamp(r.MinX, 0, size - 1);
        int maxX = Math.Clamp(r.MaxX, 0, size - 1);
        int minZ = Math.Clamp(r.MinZ, 0, size - 1);
        int maxZ = Math.Clamp(r.MaxZ, 0, size - 1);

        var list = new List<(int Index, byte TargetHeight)>();
        for (int z = minZ; z <= maxZ; z++)
        {
            for (int x = minX; x <= maxX; x++)
            {
                int idx = z * size + x;
                byte cur = session.Heights[idx];
                byte next = (byte)Math.Clamp(cur + delta, 0, 255);
                if (cur != next) list.Add((idx, next));
            }
        }
        return list;
    }

    private static List<(int Index, byte TargetHeight)> CollectCircle(TerrainHeightEditSession session, MapRegionCircle c, int delta)
    {
        int size = session.VertexSize;
        int minX = Math.Clamp((int)MathF.Floor(c.CenterX - c.Radius), 0, size - 1);
        int maxX = Math.Clamp((int)MathF.Ceiling(c.CenterX + c.Radius), 0, size - 1);
        int minZ = Math.Clamp((int)MathF.Floor(c.CenterZ - c.Radius), 0, size - 1);
        int maxZ = Math.Clamp((int)MathF.Ceiling(c.CenterZ + c.Radius), 0, size - 1);

        var list = new List<(int Index, byte TargetHeight)>();
        for (int z = minZ; z <= maxZ; z++)
        {
            for (int x = minX; x <= maxX; x++)
            {
                if (!c.Contains(x, z)) continue;
                int idx = z * size + x;
                byte cur = session.Heights[idx];
                byte next = (byte)Math.Clamp(cur + delta, 0, 255);
                if (cur != next) list.Add((idx, next));
            }
        }
        return list;
    }
}
