using AgainstRomeMapEditor.Modules.Diagnostics;
using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor.Modules.Scripting.Commands;

/// <summary>
/// 地圖全域診斷與健康檢查指令 (/diagnose)。
/// </summary>
public sealed class DiagnoseCommand : IEditorCommand
{
    public string Name => "diagnose";
    public IReadOnlyList<string> Aliases => new[] { "check", "audit", "lint" };
    public string Category => "Diagnostics";
    public string DescriptionZh => "執行全地圖健康度診斷檢查（持久 ID、座標有效性、重疊碰撞、事件目標與通行性連通）。";
    public string DescriptionEn => "Run full map diagnostic health check on objects, IDs, events, coordinates and reachability.";
    public string UsageSyntax => "/diagnose [--severity <all|error|warning>]";

    public IReadOnlyList<CommandParameter> Parameters => new[]
    {
        new CommandParameter("severity", CommandParameterType.String, "嚴重性過濾 (all, error, warning)", "Severity filter", IsRequired: false, DefaultValue: "all")
    };

    public CommandResult Execute(ParsedCommand command, CommandExecutionContext context)
    {
        string filter = command.GetFlag("severity", "all")?.ToLowerInvariant() ?? "all";

        var checkObjects = new List<MapCheckObject>();
        if (context.PlacementSession is not null)
        {
            for (int i = 0; i < context.PlacementSession.Count; i++)
            {
                SdlPlacedObject obj = context.PlacementSession[i];
                var spawn = new ScenarioSpawn(
                    obj.Type.NameDef,
                    obj.WorldX, obj.WorldZ,
                    obj.Team,
                    obj.UnitCount,
                    (int)obj.Angle,
                    obj.WorldY,
                    obj.Type.Category == SdlObjectCategory.Building)
                {
                    Id = obj.ScenarioId
                };

                bool isBuilding = obj.Type.Category == SdlObjectCategory.Building;
                bool hasCompleted = !isBuilding || obj.Type.TemplateFields.ContainsKey("alias");

                checkObjects.Add(new MapCheckObject(spawn, isBuilding, hasCompleted));
            }
        }

        var snapshot = new MapCheckSnapshot(
            checkObjects,
            context.Events ?? Array.Empty<ScenarioEvent>(),
            context.KnownAliases,
            context.HeightSession?.CollisionSize ?? 0,
            context.HeightSession?.Collision,
            context.HeightSession?.VertexSize ?? 0,
            context.HeightSession?.Heights,
            context.HeightStep,
            context.WaterLevel);

        IReadOnlyList<MapIssue> rawIssues = MapDiagnostics.Check(snapshot);

        var filtered = rawIssues.Where(issue =>
        {
            if (filter is "error") return issue.Severity == MapIssueSeverity.Error;
            if (filter is "warning") return issue.Severity == MapIssueSeverity.Warning;
            return true;
        }).ToList();

        int errCount = filtered.Count(i => i.Severity == MapIssueSeverity.Error);
        int warnCount = filtered.Count(i => i.Severity == MapIssueSeverity.Warning);

        var details = new List<string>();
        foreach (var issue in filtered)
        {
            string loc = issue.WorldX.HasValue && issue.WorldZ.HasValue
                ? $" at ({issue.WorldX.Value:F0}, {issue.WorldZ.Value:F0})"
                : "";
            string line = $"[{issue.Severity}] {issue.Code}{loc}: {issue.Chinese} ({issue.English})";
            details.Add(line);
            context.Log(line, issue.Severity == MapIssueSeverity.Error ? LogLevel.Error : LogLevel.Warning);
        }

        string summary = $"診斷完成。共檢測出 {filtered.Count} 項議題 (錯誤: {errCount}, 警告: {warnCount})。";
        context.Log(summary, errCount > 0 ? LogLevel.Warning : LogLevel.Success);

        return CommandResult.Ok(summary, filtered.Count, details);
    }
}
