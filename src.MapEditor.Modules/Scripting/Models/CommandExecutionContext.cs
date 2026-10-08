using AgainstRomeMapEditor.Modules.Nature;
using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor.Modules.Scripting.Models;

/// <summary>
/// 指令與巨集執行時的共享上下文環境，匯聚所有地圖編輯 Session、範本目錄、狀態與變數符號表。
/// </summary>
public sealed class CommandExecutionContext
{
    internal TerrainHeightEditSession? HeightSession { get; set; }
    internal TerrainBlendEditSession? BlendSession { get; set; }
    internal PlacementEditSession? PlacementSession { get; set; }
    internal NatureEditSession? NatureSession { get; set; }

    public IReadOnlyList<SdlObjectType>? AvailableObjectTypes { get; set; }
    public IReadOnlyDictionary<string, LevelObjectTemplate>? AvailableNatureTemplates { get; set; }

    public int MapTileDimension { get; set; } = 256;
    public float WorldDimension { get; set; } = 16384f;
    public float WaterLevel { get; set; }
    public float HeightStep { get; set; } = 4f;

    public IReadOnlyList<ScenarioEvent>? Events { get; set; }
    public IReadOnlyCollection<string> KnownAliases { get; set; } = Array.Empty<string>();
    public IReadOnlyList<string> KnownTextures { get; set; } = Array.Empty<string>();

    public List<int> SelectedPlacementIndices { get; } = new();
    public Dictionary<string, string> Variables { get; } = new(StringComparer.OrdinalIgnoreCase);

    public Action<string, LogLevel>? OutputHandler { get; set; }
    public Action<Action, Action>? StepRecorder { get; set; }
    internal Execution.MacroScriptRunner? ScriptRunner { get; set; }

    public void Log(string message, LogLevel level = LogLevel.Info) =>
        OutputHandler?.Invoke(message, level);

    public void RecordStep(Action undo, Action redo) =>
        StepRecorder?.Invoke(undo, redo);

    public SdlObjectType? FindObjectType(string nameOrAlias)
    {
        if (AvailableObjectTypes is null || string.IsNullOrWhiteSpace(nameOrAlias)) return null;
        return AvailableObjectTypes.FirstOrDefault(t =>
            string.Equals(t.NameDef, nameOrAlias, StringComparison.OrdinalIgnoreCase) ||
            (t.TemplateFields.TryGetValue("alias", out string? a) && string.Equals(a, nameOrAlias, StringComparison.OrdinalIgnoreCase)));
    }

    public LevelObjectTemplate? FindNatureTemplate(string name)
    {
        if (AvailableNatureTemplates is null || string.IsNullOrWhiteSpace(name)) return null;
        if (AvailableNatureTemplates.TryGetValue(name, out var template)) return template;
        return null;
    }

    /// <summary>
    /// 取樣指定世界坐標對應的地形高度值 (World Y)。
    /// </summary>
    public float GetHeightAtWorld(float worldX, float worldZ)
    {
        if (HeightSession is null || HeightSession.Heights.Count == 0) return 0f;
        int vertexSize = HeightSession.VertexSize;
        if (vertexSize <= 1) return 0f;

        float step = WorldDimension / (vertexSize - 1);
        int vx = Math.Clamp((int)MathF.Round(worldX / step), 0, vertexSize - 1);
        int vz = Math.Clamp((int)MathF.Round(worldZ / step), 0, vertexSize - 1);
        int index = vz * vertexSize + vx;

        if (index < 0 || index >= HeightSession.Heights.Count) return 0f;
        byte rawG = HeightSession.Heights[index];
        return rawG * HeightStep;
    }
}
