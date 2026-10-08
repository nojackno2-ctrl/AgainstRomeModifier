using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;

namespace AgainstRomeMapEditor.Modules.Scripting.Commands;

/// <summary>
/// 平坦化/水平整地指令 (/flatten)。
/// </summary>
internal sealed class FlattenCommand : IEditorCommand
{
    public string Name => "flatten";
    public IReadOnlyList<string> Aliases => new[] { "level", "plateau" };
    public string Category => "Terrain";
    public string DescriptionZh => "將指定矩形或圓形範圍內的地形平整至目標高度（若未指定則自動採樣中心點高度）。";
    public string DescriptionEn => "Flatten terrain in a rectangular or circular region to target height (or center sampled height).";
    public string UsageSyntax => "/flatten rect <x1> <z1> <x2> <z2> [height] 或 /flatten circle <cx> <cz> <radius> [height]";

    public IReadOnlyList<CommandParameter> Parameters => new[]
    {
        new CommandParameter("shape", CommandParameterType.String, "區域幾何形狀 (rect 或 circle)", "Region shape", IsRequired: true, PossibleValues: new[] { "rect", "circle" }),
        new CommandParameter("height", CommandParameterType.Integer, "目標高程 (0 ~ 255，省略則取中心值)", "Target height", IsRequired: false),
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
        int targetHeight = -1;

        if (command.HasFlag("rect") || command.HasFlag("circle"))
        {
            if (command.TryGetFlagInt("height", out int hVal))
            {
                targetHeight = Math.Clamp(hVal, 0, 255);
            }

            if (command.HasFlag("rect"))
            {
                string rectRaw = command.GetFlag("rect") ?? "";
                if (!ParsedCommand.TryParseRect(rectRaw, out MapRegionRect r))
                    return CommandResult.Fail("無法解析 --rect 區域。格式: x1,z1,x2,z2");

                targets = CollectRect(context.HeightSession, r, targetHeight);
            }
            else if (command.HasFlag("circle"))
            {
                string circleRaw = command.GetFlag("circle") ?? "";
                if (!ParsedCommand.TryParseCircle(circleRaw, out MapRegionCircle c))
                    return CommandResult.Fail("無法解析 --circle 區域。格式: cx,cz,radius");

                targets = CollectCircle(context.HeightSession, c, targetHeight);
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
                    !command.TryGetPositionalInt(4, out int z2))
                {
                    return CommandResult.Fail("語法錯誤。用法: /flatten rect <x1> <z1> <x2> <z2> [height]");
                }

                if (command.TryGetPositionalInt(5, out int h)) targetHeight = Math.Clamp(h, 0, 255);
                var r = new MapRegionRect(Math.Min(x1, x2), Math.Min(z1, z2), Math.Max(x1, x2), Math.Max(z1, z2));
                targets = CollectRect(context.HeightSession, r, targetHeight);
            }
            else if (mode is "circle")
            {
                if (!command.TryGetPositionalFloat(1, out float cx) ||
                    !command.TryGetPositionalFloat(2, out float cz) ||
                    !command.TryGetPositionalFloat(3, out float radius))
                {
                    return CommandResult.Fail("語法錯誤。用法: /flatten circle <cx> <cz> <radius> [height]");
                }

                if (command.TryGetPositionalInt(4, out int h)) targetHeight = Math.Clamp(h, 0, 255);
                var c = new MapRegionCircle(cx, cz, radius);
                targets = CollectCircle(context.HeightSession, c, targetHeight);
            }
            else
            {
                return CommandResult.Fail($"未知的幾何形狀「{mode}」。請指定 rect 或 circle。");
            }
        }

        if (targets.Count == 0)
        {
            return CommandResult.Ok("目標區域已為目標高度，無需調整。", 0);
        }

        context.HeightSession.ApplyHeightAdjustments(targets);
        bool committed = context.HeightSession.CommitStroke();

        if (committed)
        {
            context.RecordStep(() => context.HeightSession.Undo(), () => context.HeightSession.Redo());
        }

        return CommandResult.Ok($"成功整平 {targets.Count} 個頂點。", targets.Count);
    }

    private static List<(int Index, byte TargetHeight)> CollectRect(TerrainHeightEditSession session, MapRegionRect r, int targetHeight)
    {
        int size = session.VertexSize;
        int minX = Math.Clamp(r.MinX, 0, size - 1);
        int maxX = Math.Clamp(r.MaxX, 0, size - 1);
        int minZ = Math.Clamp(r.MinZ, 0, size - 1);
        int maxZ = Math.Clamp(r.MaxZ, 0, size - 1);

        if (targetHeight < 0)
        {
            int midX = (minX + maxX) / 2;
            int midZ = (minZ + maxZ) / 2;
            targetHeight = session.Heights[midZ * size + midX];
        }
        byte finalH = (byte)Math.Clamp(targetHeight, 0, 255);

        var list = new List<(int Index, byte TargetHeight)>();
        for (int z = minZ; z <= maxZ; z++)
        {
            for (int x = minX; x <= maxX; x++)
            {
                int idx = z * size + x;
                byte cur = session.Heights[idx];
                if (cur != finalH) list.Add((idx, finalH));
            }
        }
        return list;
    }

    private static List<(int Index, byte TargetHeight)> CollectCircle(TerrainHeightEditSession session, MapRegionCircle c, int targetHeight)
    {
        int size = session.VertexSize;
        int minX = Math.Clamp((int)MathF.Floor(c.CenterX - c.Radius), 0, size - 1);
        int maxX = Math.Clamp((int)MathF.Ceiling(c.CenterX + c.Radius), 0, size - 1);
        int minZ = Math.Clamp((int)MathF.Floor(c.CenterZ - c.Radius), 0, size - 1);
        int maxZ = Math.Clamp((int)MathF.Ceiling(c.CenterZ + c.Radius), 0, size - 1);

        if (targetHeight < 0)
        {
            int cx = Math.Clamp((int)MathF.Round(c.CenterX), 0, size - 1);
            int cz = Math.Clamp((int)MathF.Round(c.CenterZ), 0, size - 1);
            targetHeight = session.Heights[cz * size + cx];
        }
        byte finalH = (byte)Math.Clamp(targetHeight, 0, 255);

        var list = new List<(int Index, byte TargetHeight)>();
        for (int z = minZ; z <= maxZ; z++)
        {
            for (int x = minX; x <= maxX; x++)
            {
                if (!c.Contains(x, z)) continue;
                int idx = z * size + x;
                byte cur = session.Heights[idx];
                if (cur != finalH) list.Add((idx, finalH));
            }
        }
        return list;
    }
}
