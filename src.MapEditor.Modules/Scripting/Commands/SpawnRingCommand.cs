using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.Modules.Scripting.Commands;

/// <summary>
/// 環形哨塔/防禦工事陣列放置指令 (/spawn-ring)。
/// </summary>
internal sealed class SpawnRingCommand : IEditorCommand
{
    public string Name => "spawn-ring";
    public IReadOnlyList<string> Aliases => new[] { "ring", "circle-spawn" };
    public string Category => "Placement";
    public string DescriptionZh => "圍繞指定中心點與半徑，等間距環狀擺放哨塔、城牆或防禦陣列，並自動調整朝向與地形貼地高程。";
    public string DescriptionEn => "Place objects in a circular ring formation around a center point with automatic heading alignment and terrain height clamping.";
    public string UsageSyntax => "/spawn-ring <template> <centerX> <centerZ> <radius> <count> [--team <teamId>] [--angle <outward|inward|tangent|fixed>]";

    public IReadOnlyList<CommandParameter> Parameters => new[]
    {
        new CommandParameter("template", CommandParameterType.TemplateName, "欲放置之建築或物件範本名稱", "Template name", IsRequired: true),
        new CommandParameter("centerX", CommandParameterType.Float, "圓心世界 X 座標", "Center X coordinate", IsRequired: true),
        new CommandParameter("centerZ", CommandParameterType.Float, "圓心世界 Z 座標", "Center Z coordinate", IsRequired: true),
        new CommandParameter("radius", CommandParameterType.Float, "環狀擺放半徑 (世界單位)", "Radius in world units", IsRequired: true),
        new CommandParameter("count", CommandParameterType.Integer, "擺放數量 (件數)", "Number of objects to spawn", IsRequired: true),
        new CommandParameter("team", CommandParameterType.TeamId, "所屬陣營隊伍編號 (預設 0)", "Team ID", IsRequired: false, DefaultValue: "0"),
        new CommandParameter("angle", CommandParameterType.String, "朝向模式 (outward, inward, tangent, fixed)", "Heading angle mode", IsRequired: false, DefaultValue: "outward")
    };

    public CommandResult Execute(ParsedCommand command, CommandExecutionContext context)
    {
        if (context.PlacementSession is null)
        {
            return CommandResult.Fail("物件放置會話 (PlacementEditSession) 未就緒。");
        }

        string? templateName = command.GetPositional(0);
        if (string.IsNullOrWhiteSpace(templateName))
        {
            return CommandResult.Fail("語法錯誤。請指定欲擺放之物件範本。用法: /spawn-ring <template> <cx> <cz> <radius> <count>");
        }

        if (!command.TryGetPositionalFloat(1, out float cx) ||
            !command.TryGetPositionalFloat(2, out float cz) ||
            !command.TryGetPositionalFloat(3, out float radius) ||
            !command.TryGetPositionalInt(4, out int count))
        {
            return CommandResult.Fail("語法錯誤。座標、半徑與數量必須為數值。用法: /spawn-ring <template> <cx> <cz> <radius> <count>");
        }

        if (count <= 0 || count > 360)
        {
            return CommandResult.Fail("擺放數量 (count) 必須介於 1 到 360 之間。");
        }

        if (radius <= 0)
        {
            return CommandResult.Fail("擺放半徑 (radius) 必須大於 0。");
        }

        SdlObjectType? type = context.FindObjectType(templateName);
        if (type is null)
        {
            return CommandResult.Fail($"未找到名為「{templateName}」的放置物件範本。");
        }

        int team = 0;
        if (command.TryGetFlagInt("team", out int tVal)) team = tVal;

        string angleMode = command.GetFlag("angle", "outward")?.ToLowerInvariant() ?? "outward";

        var additions = new List<SdlPlacedObject>();
        double angleStep = (Math.PI * 2) / count;

        for (int i = 0; i < count; i++)
        {
            double theta = i * angleStep;
            float px = cx + (float)(radius * Math.Cos(theta));
            float pz = cz + (float)(radius * Math.Sin(theta));

            if (px < 0 || px >= context.WorldDimension || pz < 0 || pz >= context.WorldDimension)
            {
                return CommandResult.Fail($"環形第 {i + 1} 個物件超出地圖邊界 (X: {px:F1}, Z: {pz:F1})。");
            }

            float py = context.GetHeightAtWorld(px, pz);
            double deg = (theta * 180.0 / Math.PI);

            float objectAngle = angleMode switch
            {
                "inward" => (float)((deg + 180.0) % 360.0),
                "tangent" => (float)((deg + 90.0) % 360.0),
                "fixed" => 0f,
                _ => (float)(deg % 360.0) // outward
            };

            while (objectAngle < 0) objectAngle += 360f;

            additions.Add(new SdlPlacedObject(
                type,
                px, py, pz,
                team,
                objectAngle,
                UnitCount: type.Category == SdlObjectCategory.Figure ? 1 : 0)
            {
                ScenarioId = Guid.NewGuid()
            });
        }

        context.PlacementSession.AddMany(additions);
        context.RecordStep(() => context.PlacementSession.Undo(), () => context.PlacementSession.Redo());

        return CommandResult.Ok($"成功以環狀擺放 {additions.Count} 個「{templateName}」(半徑: {radius:F0}, 隊伍: {team})。", additions.Count);
    }
}
