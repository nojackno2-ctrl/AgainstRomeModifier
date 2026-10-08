using AgainstRomeMapEditor.Modules.Pathfinding;
using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;

namespace AgainstRomeMapEditor.Modules.Scripting.Commands;

/// <summary>
/// 自動修復道路網中斷與尋路網格微調指令 (/heal-navmesh)。
/// </summary>
internal sealed class HealNavMeshCommand : IEditorCommand
{
    public string Name => "heal-navmesh";
    public IReadOnlyList<string> Aliases => new[] { "heal-roads", "fix-navmesh", "repair-roads" };
    public string Category => "Pathfinding";
    public string DescriptionZh => "自動掃描地圖中 1~2 格的微小道路中斷或端點缺口，並自動填補過渡圖塊與清除通行障礙。";
    public string DescriptionEn => "Automatically scan and repair 1-2 tile road gaps and dead ends, patching textures and clearing blockage.";
    public string UsageSyntax => "/heal-navmesh [--mode <gaps|bridges|all>] [--max-gap <1|2>]";

    public IReadOnlyList<CommandParameter> Parameters => new[]
    {
        new CommandParameter("mode", CommandParameterType.String, "修復模式 (gaps, bridges, all)", "Repair mode", IsRequired: false, DefaultValue: "gaps"),
        new CommandParameter("max-gap", CommandParameterType.Integer, "最大修復間隙跨度 (1 ~ 3 格)", "Maximum gap distance", IsRequired: false, DefaultValue: "2")
    };

    public CommandResult Execute(ParsedCommand command, CommandExecutionContext context)
    {
        var blendSession = context.BlendSession;
        if (blendSession is null)
        {
            return CommandResult.Fail("地形材質會話 (TerrainBlendEditSession) 未就緒。");
        }

        int dim = blendSession.TileDimension > 0 ? blendSession.TileDimension : (context.MapTileDimension > 0 ? context.MapTileDimension : 64);
        IReadOnlyList<string> textures = blendSession.CurrentTextures;

        int maxGap = 2;
        if (command.TryGetFlagInt("max-gap", out int mgVal) && mgVal > 0)
        {
            maxGap = Math.Clamp(mgVal, 1, 3);
        }

        // 1. 偵測道路中斷
        var gapCandidates = RoadGapDetector.DetectGaps(dim, textures, passability: null, maxGapDistance: maxGap);
        if (gapCandidates.Count == 0)
        {
            return CommandResult.Ok("未偵測到任何道路中斷間隙，拓撲結構完整。", 0);
        }

        // 2. 產出修復動作
        var actions = RoadPathHealer.CreateRepairActions(gapCandidates);
        int healedCount = 0;

        foreach (var act in actions)
        {
            if (RoadPathHealer.ApplyRepair(act, blendSession, context.HeightSession))
            {
                healedCount++;
            }
        }

        if (healedCount > 0)
        {
            blendSession.CommitStroke();
            context.RecordStep(() => blendSession.Undo(), () => blendSession.Redo());
        }

        return CommandResult.Ok($"成功自動修復 {healedCount} 處道路間隙 (候選總數: {gapCandidates.Count})。", healedCount);
    }
}
