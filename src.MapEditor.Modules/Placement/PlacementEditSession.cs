using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.Modules.Placement;

/// <summary>放置指令介面，支援獨立的執行與撤銷。</summary>
internal interface IPlacementCommand
{
    string Description { get; }
    void Execute(List<SdlPlacedObject> objects);
    void Undo(List<SdlPlacedObject> objects);
}

/// <summary>新增放置物件指令。</summary>
internal sealed record AddPlacementCommand(SdlPlacedObject Item) : IPlacementCommand
{
    public string Description => $"Add {Item.Type.NameDef}";
    public void Execute(List<SdlPlacedObject> objects) => objects.Add(PlacementEditSession.Clone(Item));
    public void Undo(List<SdlPlacedObject> objects) => objects.RemoveAt(objects.Count - 1);
}

/// <summary>移除放置物件指令。</summary>
internal sealed record RemovePlacementCommand(int Index, SdlPlacedObject Item) : IPlacementCommand
{
    public string Description => $"Remove {Item.Type.NameDef} at {Index}";
    public void Execute(List<SdlPlacedObject> objects) => objects.RemoveAt(Index);
    public void Undo(List<SdlPlacedObject> objects) => objects.Insert(Index, PlacementEditSession.Clone(Item));
}

/// <summary>編輯放置物件指令（保留持久識別碼）。</summary>
internal sealed record EditPlacementCommand(int Index, SdlPlacedObject Before, SdlPlacedObject After) : IPlacementCommand
{
    public string Description => $"Edit {After.Type.NameDef} at {Index}";
    public void Execute(List<SdlPlacedObject> objects) => objects[Index] = PlacementEditSession.Clone(After);
    public void Undo(List<SdlPlacedObject> objects) => objects[Index] = PlacementEditSession.Clone(Before);
}

/// <summary>複製放置物件指令（產生新 GUID）。</summary>
internal sealed record DuplicatePlacementCommand(int SourceIndex, int InsertIndex, SdlPlacedObject DuplicatedItem) : IPlacementCommand
{
    public string Description => $"Duplicate {DuplicatedItem.Type.NameDef} to {InsertIndex}";
    public void Execute(List<SdlPlacedObject> objects) => objects.Insert(InsertIndex, PlacementEditSession.Clone(DuplicatedItem));
    public void Undo(List<SdlPlacedObject> objects) => objects.RemoveAt(InsertIndex);
}

/// <summary>批次指令，按順序執行並倒序撤銷。</summary>
internal sealed record BatchPlacementCommand(string Description, IReadOnlyList<IPlacementCommand> Commands) : IPlacementCommand
{
    public void Execute(List<SdlPlacedObject> objects)
    {
        foreach (IPlacementCommand cmd in Commands) cmd.Execute(objects);
    }
    public void Undo(List<SdlPlacedObject> objects)
    {
        for (int i = Commands.Count - 1; i >= 0; i--) Commands[i].Undo(objects);
    }
}

/// <summary>放置物件的持久身份、內容快照與儲存基準；支援獨立命令與 Undo/Redo。</summary>
public sealed class PlacementEditSession : IEditorModule<IReadOnlyList<SdlPlacedObject>>
{
    private readonly List<SdlPlacedObject> _objects = new();
    private readonly Stack<IPlacementCommand> _undo = new();
    private readonly Stack<IPlacementCommand> _redo = new();
    private IReadOnlyList<SdlPlacedObject> _baseline = [];

    public string ModuleId => "placement";
    public bool IsDirty => _objects.Count != _baseline.Count || _objects.Where((item, index) => !Equal(item, _baseline[index])).Any();
    public int Count => _objects.Count;
    public bool CanUndo => _undo.Count > 0;
    public bool CanRedo => _redo.Count > 0;

    public SdlPlacedObject this[int index] => Clone(_objects[index]);

    public IReadOnlyList<SdlPlacedObject> Capture() => Copy(_objects);

    public void Load(IReadOnlyList<SdlPlacedObject> snapshot)
    {
        var copy = Copy(snapshot);
        _objects.Clear();
        _objects.AddRange(copy);
        ClearHistory();
        AcceptChanges();
    }

    public void AcceptChanges()
    {
        _baseline = Capture();
        ClearHistory();
    }

    public void Reset()
    {
        _objects.Clear();
        _objects.AddRange(Copy(_baseline));
        ClearHistory();
    }

    public void ClearHistory()
    {
        _undo.Clear();
        _redo.Clear();
    }

    internal void Execute(IPlacementCommand command)
    {
        ArgumentNullException.ThrowIfNull(command);
        command.Execute(_objects);
        _undo.Push(command);
        _redo.Clear();
    }

    public bool Undo()
    {
        if (!_undo.TryPop(out IPlacementCommand? command)) return false;
        command.Undo(_objects);
        _redo.Push(command);
        return true;
    }

    public bool Redo()
    {
        if (!_redo.TryPop(out IPlacementCommand? command)) return false;
        command.Execute(_objects);
        _undo.Push(command);
        return true;
    }

    public void Add(SdlPlacedObject item)
    {
        ArgumentNullException.ThrowIfNull(item);
        SdlPlacedObject fixedItem = item.ScenarioId == Guid.Empty ? item with { ScenarioId = Guid.NewGuid() } : item;
        ValidateBounds(fixedItem, strictUnitCount: false);
        Execute(new AddPlacementCommand(Clone(fixedItem)));
    }

    public void Replace(int index, SdlPlacedObject item)
    {
        if (index < 0 || index >= _objects.Count) throw new ArgumentOutOfRangeException(nameof(index));
        SdlPlacedObject before = _objects[index];
        SdlPlacedObject fixedItem = item.ScenarioId == Guid.Empty ? item with { ScenarioId = before.ScenarioId } : item;
        ValidateBounds(fixedItem, strictUnitCount: false);
        Execute(new EditPlacementCommand(index, before, Clone(fixedItem)));
    }

    public void RemoveAt(int index)
    {
        if (index < 0 || index >= _objects.Count) throw new ArgumentOutOfRangeException(nameof(index));
        SdlPlacedObject removed = _objects[index];
        Execute(new RemovePlacementCommand(index, removed));
    }

    /// <summary>編輯放置物件（隊伍/座標/角度/人數），強制保留原 persistent ScenarioId。</summary>
    public void Edit(int index, SdlPlacedObject updated)
    {
        if (index < 0 || index >= _objects.Count) throw new ArgumentOutOfRangeException(nameof(index));
        SdlPlacedObject before = _objects[index];
        // 關鍵約束：編輯必須保留持久 ScenarioId
        SdlPlacedObject fixedItem = updated with { ScenarioId = before.ScenarioId };
        ValidateBounds(fixedItem, strictUnitCount: true);
        Execute(new EditPlacementCommand(index, before, Clone(fixedItem)));
    }

    /// <summary>編輯放置物件數值，強制保留原 persistent ScenarioId。</summary>
    public void Edit(int index, int team, float worldX, float worldY, float worldZ, float angle, int unitCount)
    {
        if (index < 0 || index >= _objects.Count) throw new ArgumentOutOfRangeException(nameof(index));
        SdlPlacedObject before = _objects[index];
        bool isFigure = before.Type.Category == SdlObjectCategory.Figure;
        SdlPlacedObject updated = before with
        {
            Team = team,
            WorldX = worldX,
            WorldY = worldY,
            WorldZ = worldZ,
            Angle = angle,
            UnitCount = isFigure ? unitCount : 0,
            ScenarioId = before.ScenarioId
        };
        Edit(index, updated);
    }

    /// <summary>複製放置物件，偏移座標並產生全新 Guid 作為 ScenarioId。</summary>
    public int Duplicate(int index, float offsetX = 256f, float offsetZ = 256f)
    {
        return DuplicateMany([index], offsetX, offsetZ)[0];
    }

    /// <summary>先驗證全部複本，再以單一撤銷步驟插入；偏移超出地圖時拒絕整批。</summary>
    public IReadOnlyList<int> DuplicateMany(IEnumerable<int> indices, float offsetX = 256f, float offsetZ = 256f)
    {
        int[] selected = ValidateIndices(indices);
        if (selected.Length == 0) return Array.Empty<int>();
        var commands = new List<IPlacementCommand>();
        foreach (int index in Enumerable.Reverse(selected))
        {
            SdlPlacedObject source = _objects[index];
            SdlPlacedObject duplicate = Clone(source with
            {
                WorldX = source.WorldX + offsetX,
                WorldZ = source.WorldZ + offsetZ,
                ScenarioId = Guid.NewGuid()
            });
            ValidateBounds(duplicate, strictUnitCount: false);
            commands.Add(new DuplicatePlacementCommand(index, index + 1, duplicate));
        }
        Execute(new BatchPlacementCommand("Duplicate selected objects", commands));
        return selected.Select((index, shift) => index + shift + 1).ToArray();
    }

    /// <summary>先驗證全部索引，再以單一撤銷步驟刪除，撤銷時恢復原順序與身份。</summary>
    public void RemoveMany(IEnumerable<int> indices)
    {
        int[] selected = ValidateIndices(indices);
        if (selected.Length == 0) return;
        Execute(new BatchPlacementCommand("Remove selected objects", Enumerable.Reverse(selected)
            .Select(index => (IPlacementCommand)new RemovePlacementCommand(index, _objects[index])).ToArray()));
    }

    /// <summary>Validate every selected object before changing team/direction as one undoable batch.</summary>
    public void EditMany(IEnumerable<int> indices, int? team = null, float? angle = null)
    {
        int[] selected = ValidateIndices(indices);
        if (selected.Length == 0 || team is null && angle is null) return;
        var commands = new List<IPlacementCommand>();
        foreach (int index in selected)
        {
            SdlPlacedObject before = _objects[index];
            SdlPlacedObject after = Clone(before with { Team = team ?? before.Team, Angle = angle ?? before.Angle });
            ValidateBounds(after, strictUnitCount: true);
            if (after.Team != before.Team || after.Angle != before.Angle)
                commands.Add(new EditPlacementCommand(index, before, after));
        }
        if (commands.Count > 0) Execute(new BatchPlacementCommand("Set selected teams/directions", commands));
    }

    /// <summary>Validate a complete layout before inserting any objects. Missing IDs receive fresh identities.</summary>
    public IReadOnlyList<int> AddMany(IEnumerable<SdlPlacedObject> items)
    {
        ArgumentNullException.ThrowIfNull(items);
        var preparedItems = new List<SdlPlacedObject>();
        foreach (SdlPlacedObject item in items)
        {
            ArgumentNullException.ThrowIfNull(item);
            preparedItems.Add(Clone(item.ScenarioId == Guid.Empty ? item with { ScenarioId = Guid.NewGuid() } : item));
        }
        SdlPlacedObject[] prepared = preparedItems.ToArray();
        var ids = _objects.Select(item => item.ScenarioId).ToHashSet();
        foreach (SdlPlacedObject item in prepared)
        {
            ValidateBounds(item, strictUnitCount: true);
            if (!ids.Add(item.ScenarioId)) throw new ArgumentException("Layout objects require distinct persistent IDs.", nameof(items));
        }
        int start = _objects.Count;
        if (prepared.Length > 0) Execute(new BatchPlacementCommand("Place layout", prepared.Select(item => (IPlacementCommand)new AddPlacementCommand(item)).ToArray()));
        return Enumerable.Range(start, prepared.Length).ToArray();
    }

    private int[] ValidateIndices(IEnumerable<int> indices)
    {
        ArgumentNullException.ThrowIfNull(indices);
        int[] selected = indices.Distinct().Order().ToArray();
        if (selected.Any(index => index < 0 || index >= _objects.Count))
            throw new ArgumentOutOfRangeException(nameof(indices));
        return selected;
    }

    public static bool IsValidUnitTeam(int team) => team is >= 0 and <= 7;
    public static bool IsValidUnitCount(int count) => count is >= 1 and <= 20;

    public static void ValidateBounds(SdlPlacedObject item, bool strictUnitCount = false)
    {
        ArgumentNullException.ThrowIfNull(item);
        if (!float.IsFinite(item.WorldX) || !float.IsFinite(item.WorldY) || !float.IsFinite(item.WorldZ)
            || item.WorldX is < 0 or > 16383 || item.WorldZ is < 0 or > 16383)
            throw new ArgumentOutOfRangeException(nameof(item), "Position must be finite with X/Z between 0 and 16383.");
        if (!float.IsFinite(item.Angle))
            throw new ArgumentOutOfRangeException(nameof(item), "Angle must be finite.");
        if (item.Type.Category == SdlObjectCategory.Figure)
        {
            if (item.Team is < 0 or > 7)
                throw new ArgumentOutOfRangeException(nameof(item), $"Figure team must be between 0 and 7, got {item.Team}.");
            if (strictUnitCount && item.UnitCount is < 1 or > 20)
                throw new ArgumentOutOfRangeException(nameof(item), $"Figure unit count must be between 1 and 20, got {item.UnitCount}.");
            if (!strictUnitCount && item.UnitCount is < 0 or > 20)
                throw new ArgumentOutOfRangeException(nameof(item), $"Figure unit count must be between 0 and 20, got {item.UnitCount}.");
        }
        else
        {
            if (item.Team is < -1 or > 15)
                throw new ArgumentOutOfRangeException(nameof(item), $"Object team must be between -1 and 15, got {item.Team}.");
        }
    }

    internal static SdlPlacedObject Clone(SdlPlacedObject item) => item with
    {
        Type = item.Type with { TemplateFields = new Dictionary<string, string>(item.Type.TemplateFields, StringComparer.OrdinalIgnoreCase) }
    };

    private static IReadOnlyList<SdlPlacedObject> Copy(IReadOnlyList<SdlPlacedObject> items) => items.Select(Clone).ToArray();

    private static bool Equal(SdlPlacedObject a, SdlPlacedObject b) =>
        a.ScenarioId == b.ScenarioId && a.WorldX == b.WorldX && a.WorldY == b.WorldY && a.WorldZ == b.WorldZ
        && a.Team == b.Team && a.Angle == b.Angle && a.UnitCount == b.UnitCount
        && a.Type.NameDef == b.Type.NameDef && a.Type.Definition == b.Type.Definition
        && a.Type.Category == b.Type.Category && a.Type.Tribe == b.Type.Tribe && a.Type.Occurrences == b.Type.Occurrences
        && a.Type.TemplateFields.Count == b.Type.TemplateFields.Count
        && a.Type.TemplateFields.All(pair => b.Type.TemplateFields.TryGetValue(pair.Key, out string? value) && pair.Value == value);
}
