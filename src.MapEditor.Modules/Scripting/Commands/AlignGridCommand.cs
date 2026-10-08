using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.Modules.Scripting.Commands;

/// <summary>
/// 物件網格吸附對齊指令 (/align-grid)。
/// </summary>
internal sealed class AlignGridCommand : IEditorCommand
{
    public string Name => "align-grid";
    public IReadOnlyList<string> Aliases => new[] { "snap-grid", "grid" };
    public string Category => "Placement";
    public string DescriptionZh => "將選取或全部放置物件的世界坐標對齊至指定間距的網格點（如 32 或 64 單位），並重算貼地高度。";
    public string DescriptionEn => "Snap selected or all placed object coordinates to specified grid intervals and re-clamp terrain height.";
    public string UsageSyntax => "/align-grid [step=32] [--selected]";

    public IReadOnlyList<CommandParameter> Parameters => new[]
    {
        new CommandParameter("step", CommandParameterType.Float, "網格對齊步長 (預設 32.0)", "Grid snap step", IsRequired: false, DefaultValue: "32"),
        new CommandParameter("selected", CommandParameterType.Flag, "僅針對當前選取的物件對齊", "Only snap selected objects", IsRequired: false)
    };

    public CommandResult Execute(ParsedCommand command, CommandExecutionContext context)
    {
        if (context.PlacementSession is null)
        {
            return CommandResult.Fail("物件放置會話 (PlacementEditSession) 未就緒。");
        }

        float step = 32f;
        if (command.TryGetPositionalFloat(0, out float pStep) && pStep > 0) step = pStep;
        else if (command.TryGetFlagFloat("step", out float fStep) && fStep > 0) step = fStep;

        bool onlySelected = command.HasFlag("selected");
        var targetIndices = new List<int>();

        if (onlySelected && context.SelectedPlacementIndices.Count > 0)
        {
            targetIndices.AddRange(context.SelectedPlacementIndices.Where(i => i >= 0 && i < context.PlacementSession.Count));
        }
        else
        {
            for (int i = 0; i < context.PlacementSession.Count; i++) targetIndices.Add(i);
        }

        if (targetIndices.Count == 0)
        {
            return CommandResult.Ok("無符合或選取之物件可對齊。", 0);
        }

        int alignedCount = 0;
        foreach (int idx in targetIndices)
        {
            SdlPlacedObject current = context.PlacementSession[idx];
            float newX = MathF.Round(current.WorldX / step) * step;
            float newZ = MathF.Round(current.WorldZ / step) * step;
            float newY = context.GetHeightAtWorld(newX, newZ);

            if (Math.Abs(newX - current.WorldX) > 0.01f ||
                Math.Abs(newZ - current.WorldZ) > 0.01f ||
                Math.Abs(newY - current.WorldY) > 0.01f)
            {
                context.PlacementSession.Edit(idx, current.Team, newX, newY, newZ, current.Angle, current.UnitCount);
                // Each Edit creates its own placement history entry.
                context.RecordStep(() => context.PlacementSession.Undo(), () => context.PlacementSession.Redo());
                alignedCount++;
            }
        }

        return CommandResult.Ok($"成功對齊 {alignedCount} 個物件至 {step:F0} 單位網格。", alignedCount);
    }
}
