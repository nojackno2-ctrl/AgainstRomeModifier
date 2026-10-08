using AgainstRomeMapEditor.Modules.Nature;
using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.Modules.Scripting.Commands;

/// <summary>
/// 自然地景/物件隨機散佈指令 (/scatter)。
/// </summary>
internal sealed class ScatterCommand : IEditorCommand
{
    public string Name => "scatter";
    public IReadOnlyList<string> Aliases => new[] { "plant", "populate" };
    public string Category => "Nature";
    public string DescriptionZh => "在指定矩形區域內，依數量或密度隨機散佈地景樹木、岩石或物件，支援最小間距與隨機朝向。";
    public string DescriptionEn => "Scatter nature trees, rocks or props randomly within a bounding rectangle with optional spacing and random rotation.";
    public string UsageSyntax => "/scatter <template> <count> <rect> [--spacing <N>] [--seed <N>]";

    public IReadOnlyList<CommandParameter> Parameters => new[]
    {
        new CommandParameter("template", CommandParameterType.TemplateName, "欲散佈的地景或物件範本名稱", "Template name", IsRequired: true),
        new CommandParameter("count", CommandParameterType.Integer, "欲產生的物件數量", "Count of items", IsRequired: true),
        new CommandParameter("rect", CommandParameterType.Rectangle, "散佈矩形範圍 (x1,z1,x2,z2)", "Bounding rectangle", IsRequired: true),
        new CommandParameter("spacing", CommandParameterType.Float, "物件間最小保護間距 (預設 48.0)", "Minimum spacing", IsRequired: false, DefaultValue: "48"),
        new CommandParameter("seed", CommandParameterType.Integer, "隨機種子碼", "Random seed", IsRequired: false)
    };

    public CommandResult Execute(ParsedCommand command, CommandExecutionContext context)
    {
        string? templateName = command.GetPositional(0);
        if (string.IsNullOrWhiteSpace(templateName))
        {
            return CommandResult.Fail("語法錯誤。請指定欲散佈之範本名稱。用法: /scatter <template> <count> <rect>");
        }

        if (!command.TryGetPositionalInt(1, out int count) || count <= 0)
        {
            return CommandResult.Fail("散佈數量 (count) 必須大於 0。");
        }

        MapRegionRect rect;
        string? rectRaw = command.GetPositional(2) ?? command.GetFlag("rect");
        if (!ParsedCommand.TryParseRect(rectRaw ?? "", out rect))
        {
            return CommandResult.Fail("無法解析矩形範圍。格式: x1,z1,x2,z2 (例如: 1000,1000,3000,3000)");
        }

        float minSpacing = 48f;
        if (command.TryGetFlagFloat("spacing", out float sp) && sp > 0)
        {
            minSpacing = sp;
        }

        int seed = Environment.TickCount;
        if (command.TryGetFlagInt("seed", out int sVal))
        {
            seed = sVal;
        }

        var rand = new Random(seed);

        // 1. 優先嘗試作為 Nature 範本
        LevelObjectTemplate? natureTpl = context.FindNatureTemplate(templateName);
        if (natureTpl is not null)
        {
            if (context.NatureSession is null) return CommandResult.Fail("自然地景會話 (NatureEditSession) 未就緒。");

            var placedPoints = new List<(float X, float Z)>();
            var additions = new List<NatureAddition>();

            int maxAttempts = count * 25;
            for (int attempt = 0; attempt < maxAttempts && additions.Count < count; attempt++)
            {
                float px = (float)(rect.MinX + rand.NextDouble() * (rect.MaxX - rect.MinX));
                float pz = (float)(rect.MinZ + rand.NextDouble() * (rect.MaxZ - rect.MinZ));

                if (placedPoints.Any(pt =>
                {
                    float dx = pt.X - px, dz = pt.Z - pz;
                    return (dx * dx + dz * dz) < (minSpacing * minSpacing);
                }))
                {
                    continue;
                }

                placedPoints.Add((px, pz));
                float py = context.GetHeightAtWorld(px, pz);
                float rot = (float)(rand.NextDouble() * Math.PI * 2);

                additions.Add(new NatureAddition(natureTpl, templateName, px, py, pz, rot));
            }

            if (additions.Count == 0) return CommandResult.Fail("無法在給定區域與間距條件下放置任何自然物件。");

            context.NatureSession.PlantMany(additions);
            context.RecordStep(() => context.NatureSession.Undo(), () => context.NatureSession.Redo());

            return CommandResult.Ok($"成功散佈 {additions.Count} 株「{templateName}」於自然層 (種子: {seed})。", additions.Count);
        }

        // 2. 嘗試作為 Placement (SDL) 物件
        SdlObjectType? sdlTpl = context.FindObjectType(templateName);
        if (sdlTpl is not null)
        {
            if (context.PlacementSession is null) return CommandResult.Fail("物件放置會話 (PlacementEditSession) 未就緒。");

            var placedPoints = new List<(float X, float Z)>();
            var additions = new List<SdlPlacedObject>();

            int maxAttempts = count * 25;
            for (int attempt = 0; attempt < maxAttempts && additions.Count < count; attempt++)
            {
                float px = (float)(rect.MinX + rand.NextDouble() * (rect.MaxX - rect.MinX));
                float pz = (float)(rect.MinZ + rand.NextDouble() * (rect.MaxZ - rect.MinZ));

                if (placedPoints.Any(pt =>
                {
                    float dx = pt.X - px, dz = pt.Z - pz;
                    return (dx * dx + dz * dz) < (minSpacing * minSpacing);
                }))
                {
                    continue;
                }

                placedPoints.Add((px, pz));
                float py = context.GetHeightAtWorld(px, pz);
                float rot = (float)(rand.NextDouble() * 360.0);

                additions.Add(new SdlPlacedObject(sdlTpl, px, py, pz, Team: 0, Angle: rot, UnitCount: sdlTpl.Category == SdlObjectCategory.Figure ? 1 : 0)
                {
                    ScenarioId = Guid.NewGuid()
                });
            }

            if (additions.Count == 0) return CommandResult.Fail("無法在給定區域與間距條件下放置任何 SDL 物件。");

            context.PlacementSession.AddMany(additions);
            context.RecordStep(() => context.PlacementSession.Undo(), () => context.PlacementSession.Redo());

            return CommandResult.Ok($"成功散佈 {additions.Count} 個「{templateName}」物件 (種子: {seed})。", additions.Count);
        }

        return CommandResult.Fail($"未找到名為「{templateName}」的自然或放置物件範本。");
    }
}
