using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;

namespace AgainstRomeMapEditor.Modules.Scripting.Commands;

/// <summary>
/// 材質圖塊批量替換指令 (/replace-texture)。
/// </summary>
internal sealed class ReplaceTextureCommand : IEditorCommand
{
    public string Name => "replace-texture";
    public IReadOnlyList<string> Aliases => new[] { "swap-tex", "subst-tex" };
    public string Category => "Terrain";
    public string DescriptionZh => "在整張地圖或指定矩形區域內，批量將特定圖塊材質替換為新材質。";
    public string DescriptionEn => "Bulk replace specific terrain tile textures with new texture globally or in a rectangle.";
    public string UsageSyntax => "/replace-texture <oldTexture> <newTexture> [--rect <x1,z1,x2,z2>]";

    public IReadOnlyList<CommandParameter> Parameters => new[]
    {
        new CommandParameter("oldTexture", CommandParameterType.TextureName, "欲被替換的原始材質名稱", "Original texture to replace", IsRequired: true),
        new CommandParameter("newTexture", CommandParameterType.TextureName, "替換後的新材質名稱", "Target new texture", IsRequired: true),
        new CommandParameter("rect", CommandParameterType.Rectangle, "限制範圍矩形 (省略則為整張地圖)", "Bounding rectangle", IsRequired: false)
    };

    public CommandResult Execute(ParsedCommand command, CommandExecutionContext context)
    {
        var blendSession = context.BlendSession;
        if (blendSession is null)
        {
            return CommandResult.Fail("地形材質會話 (TerrainBlendEditSession) 未就緒。");
        }

        string? oldTex = command.GetPositional(0);
        string? newTex = command.GetPositional(1);

        if (string.IsNullOrWhiteSpace(oldTex) || string.IsNullOrWhiteSpace(newTex))
        {
            return CommandResult.Fail("語法錯誤。用法: /replace-texture <oldTexture> <newTexture> [--rect x1,z1,x2,z2]");
        }

        int dim = blendSession.TileDimension > 0 ? blendSession.TileDimension : (context.MapTileDimension > 0 ? context.MapTileDimension : 64);
        int minX = 0, minZ = 0, maxX = dim - 1, maxZ = dim - 1;

        if (command.HasFlag("rect"))
        {
            string rectRaw = command.GetFlag("rect") ?? "";
            if (ParsedCommand.TryParseRect(rectRaw, out MapRegionRect r))
            {
                minX = Math.Clamp(r.MinX, 0, dim - 1);
                maxX = Math.Clamp(r.MaxX, 0, dim - 1);
                minZ = Math.Clamp(r.MinZ, 0, dim - 1);
                maxZ = Math.Clamp(r.MaxZ, 0, dim - 1);
            }
        }
        else if (command.PositionalArgs.Count >= 6)
        {
            if (command.TryGetPositionalInt(2, out int x1) &&
                command.TryGetPositionalInt(3, out int z1) &&
                command.TryGetPositionalInt(4, out int x2) &&
                command.TryGetPositionalInt(5, out int z2))
            {
                minX = Math.Clamp(Math.Min(x1, x2), 0, dim - 1);
                maxX = Math.Clamp(Math.Max(x1, x2), 0, dim - 1);
                minZ = Math.Clamp(Math.Min(z1, z2), 0, dim - 1);
                maxZ = Math.Clamp(Math.Max(z1, z2), 0, dim - 1);
            }
        }

        int replacedCount = 0;
        for (int z = minZ; z <= maxZ; z++)
        {
            for (int x = minX; x <= maxX; x++)
            {
                int idx = z * dim + x;
                if (idx < 0 || idx >= blendSession.CurrentTextures.Count) continue;
                string cur = blendSession.CurrentTextures[idx];
                if (string.Equals(cur, oldTex, StringComparison.OrdinalIgnoreCase))
                {
                    if (blendSession.StampTexture(x, z, newTex) is not null)
                    {
                        replacedCount++;
                    }
                }
            }
        }

        if (replacedCount == 0)
        {
            return CommandResult.Ok($"未找到相符之圖塊「{oldTex}」。", 0);
        }

        bool committed = blendSession.CommitStroke();
        if (committed)
        {
            context.RecordStep(() => blendSession.Undo(), () => blendSession.Redo());
        }

        return CommandResult.Ok($"成功將 {replacedCount} 個圖塊從「{oldTex}」替換為「{newTex}」。", replacedCount);
    }
}
