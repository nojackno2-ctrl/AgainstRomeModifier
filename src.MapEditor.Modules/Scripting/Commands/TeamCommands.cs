using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.Modules.Scripting.Commands;

/// <summary>
/// 隊伍物件批次選取指令 (/select-team)。
/// </summary>
internal sealed class SelectTeamCommand : IEditorCommand
{
    public string Name => "select-team";
    public IReadOnlyList<string> Aliases => new[] { "sel-team", "filter-team" };
    public string Category => "Placement";
    public string DescriptionZh => "批次選取屬於指定隊伍陣營的所有放置物件，可選按類型（建築或部隊）篩選。";
    public string DescriptionEn => "Select placed objects belonging to a specific team, with optional category filter.";
    public string UsageSyntax => "/select-team <teamId> [--filter <figure|building|all>]";

    public IReadOnlyList<CommandParameter> Parameters => new[]
    {
        new CommandParameter("teamId", CommandParameterType.TeamId, "欲選取的隊伍編號 (-1 ~ 15)", "Team ID", IsRequired: true),
        new CommandParameter("filter", CommandParameterType.String, "物件類型篩選 (figure, building, all)", "Category filter", IsRequired: false, DefaultValue: "all")
    };

    public CommandResult Execute(ParsedCommand command, CommandExecutionContext context)
    {
        if (context.PlacementSession is null)
        {
            return CommandResult.Fail("物件放置會話 (PlacementEditSession) 未就緒。");
        }

        if (!command.TryGetPositionalInt(0, out int teamId))
        {
            return CommandResult.Fail("語法錯誤。請指定隊伍編號。用法: /select-team <teamId>");
        }

        string filter = command.GetFlag("filter", "all")?.ToLowerInvariant() ?? "all";

        context.SelectedPlacementIndices.Clear();
        var matched = new List<int>();

        for (int i = 0; i < context.PlacementSession.Count; i++)
        {
            SdlPlacedObject item = context.PlacementSession[i];
            if (item.Team != teamId) continue;

            if (filter is "figure" && item.Type.Category != SdlObjectCategory.Figure) continue;
            if (filter is "building" && item.Type.Category != SdlObjectCategory.Building) continue;

            matched.Add(i);
        }

        context.SelectedPlacementIndices.AddRange(matched);

        return CommandResult.Ok($"已選取 {matched.Count} 個屬於隊伍 {teamId} 的物件 (篩選: {filter})。", matched.Count);
    }
}

/// <summary>
/// 隊伍所屬批次變更指令 (/set-team)。
/// </summary>
internal sealed class SetTeamCommand : IEditorCommand
{
    public string Name => "set-team";
    public IReadOnlyList<string> Aliases => new[] { "change-team", "assign-team" };
    public string Category => "Placement";
    public string DescriptionZh => "將選取的物件或指定矩形區域內的物件批量變更為指定隊伍陣營。";
    public string DescriptionEn => "Batch change team ownership for selected objects or objects inside a rectangle.";
    public string UsageSyntax => "/set-team <teamId> [--selected] [--rect <x1,z1,x2,z2>]";

    public IReadOnlyList<CommandParameter> Parameters => new[]
    {
        new CommandParameter("teamId", CommandParameterType.TeamId, "目標隊伍編號 (-1 ~ 15)", "Target team ID", IsRequired: true),
        new CommandParameter("selected", CommandParameterType.Flag, "僅針對當前選取的物件變更", "Apply only to selected objects", IsRequired: false),
        new CommandParameter("rect", CommandParameterType.Rectangle, "限制矩形範圍 (x1,z1,x2,z2)", "Bounding rectangle", IsRequired: false)
    };

    public CommandResult Execute(ParsedCommand command, CommandExecutionContext context)
    {
        if (context.PlacementSession is null)
        {
            return CommandResult.Fail("物件放置會話 (PlacementEditSession) 未就緒。");
        }

        if (!command.TryGetPositionalInt(0, out int teamId) && !command.TryGetFlagInt("team", out teamId))
        {
            return CommandResult.Fail("語法錯誤。請指定隊伍編號。用法: /set-team <teamId> [--selected] [--rect x1,z1,x2,z2]");
        }

        if (teamId is < -1 or > 15)
        {
            return CommandResult.Fail("隊伍編號無效，必須介於 -1 到 15 之間。");
        }

        var targetIndices = new List<int>();

        if (command.HasFlag("selected") && context.SelectedPlacementIndices.Count > 0)
        {
            targetIndices.AddRange(context.SelectedPlacementIndices.Where(i => i >= 0 && i < context.PlacementSession.Count));
        }
        else if (command.HasFlag("rect"))
        {
            string rectRaw = command.GetFlag("rect") ?? "";
            if (ParsedCommand.TryParseRect(rectRaw, out MapRegionRect r))
            {
                for (int i = 0; i < context.PlacementSession.Count; i++)
                {
                    SdlPlacedObject item = context.PlacementSession[i];
                    if (r.Contains((int)item.WorldX, (int)item.WorldZ))
                    {
                        targetIndices.Add(i);
                    }
                }
            }
        }
        else
        {
            // 若皆未指定，若有選取則依選取，否則全部
            if (context.SelectedPlacementIndices.Count > 0)
            {
                targetIndices.AddRange(context.SelectedPlacementIndices.Where(i => i >= 0 && i < context.PlacementSession.Count));
            }
            else
            {
                for (int i = 0; i < context.PlacementSession.Count; i++) targetIndices.Add(i);
            }
        }

        if (targetIndices.Count == 0)
        {
            return CommandResult.Ok("無目標物件需要變更隊伍。", 0);
        }

        context.PlacementSession.EditMany(targetIndices, team: teamId);
        context.RecordStep(() => context.PlacementSession.Undo(), () => context.PlacementSession.Redo());

        return CommandResult.Ok($"成功將 {targetIndices.Count} 個物件的隊伍設定為 {teamId}。", targetIndices.Count);
    }
}
