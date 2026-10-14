using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

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
public sealed partial class PlacementEditSession : IEditorModule<IReadOnlyList<SdlPlacedObject>>
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

    /// <summary>儲存成功後清除複製來源追蹤（provenance）並以新狀態作為基準線。</summary>
    public void AcceptSavedChanges()
    {
        for (int i = 0; i < _objects.Count; i++)
        {
            if (_objects[i].CopySourceScenarioId != Guid.Empty || _objects[i].NativeSourceIdentity.HasValue)
            {
                _objects[i] = _objects[i] with
                {
                    CopySourceScenarioId = Guid.Empty,
                    NativeSourceIdentity = null
                };
            }
        }
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

    /// <summary>編輯放置物件（隊伍/座標/角度/人數），強制保留原 persistent ScenarioId 與原生 metadata。</summary>
    public void Edit(int index, SdlPlacedObject updated)
    {
        if (index < 0 || index >= _objects.Count) throw new ArgumentOutOfRangeException(nameof(index));
        SdlPlacedObject before = _objects[index];
        if (before.RawRotation.HasValue || before.NativeSourceIdentity.HasValue)
        {
            if (updated.Team != before.Team)
                throw new InvalidOperationException("原生群組物件目前僅支援平移；變更隊伍尚未支援。");
            if (ScenarioLevelObjects.NormalizeAngle((int)MathF.Round(updated.Angle)) != ScenarioLevelObjects.NormalizeAngle((int)MathF.Round(before.Angle)))
                throw new InvalidOperationException("原生群組物件目前僅支援平移；變更方向尚未支援。");
        }
        // 關鍵約束：編輯必須保留持久 ScenarioId 與原生 metadata
        SdlPlacedObject fixedItem = updated with
        {
            ScenarioId = before.ScenarioId,
            CopySourceScenarioId = before.CopySourceScenarioId,
            NativeSourceIdentity = before.NativeSourceIdentity,
            RawRotation = before.RawRotation
        };
        ValidateBounds(fixedItem, strictUnitCount: true);
        Execute(new EditPlacementCommand(index, before, Clone(fixedItem)));
    }

    /// <summary>編輯放置物件數值，強制保留原 persistent ScenarioId 與原生 metadata。</summary>
    public void Edit(int index, int team, float worldX, float worldY, float worldZ, float angle, int unitCount)
    {
        if (index < 0 || index >= _objects.Count) throw new ArgumentOutOfRangeException(nameof(index));
        SdlPlacedObject before = _objects[index];
        if (before.RawRotation.HasValue || before.NativeSourceIdentity.HasValue)
        {
            if (team != before.Team)
                throw new InvalidOperationException("原生群組物件目前僅支援平移；變更隊伍尚未支援。");
            if (ScenarioLevelObjects.NormalizeAngle((int)MathF.Round(angle)) != ScenarioLevelObjects.NormalizeAngle((int)MathF.Round(before.Angle)))
                throw new InvalidOperationException("原生群組物件目前僅支援平移；變更方向尚未支援。");
        }
        bool isFigure = before.Type.Category == SdlObjectCategory.Figure;
        SdlPlacedObject updated = before with
        {
            Team = team,
            WorldX = worldX,
            WorldY = worldY,
            WorldZ = worldZ,
            Angle = angle,
            UnitCount = isFigure && !before.RawRotation.HasValue ? unitCount : 0,
            ScenarioId = before.ScenarioId,
            CopySourceScenarioId = before.CopySourceScenarioId,
            NativeSourceIdentity = before.NativeSourceIdentity,
            RawRotation = before.RawRotation
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
            Guid sourceProvenance;
            NativeObjectIdentity? nativeIdentity = null;
            float? rawRotation = source.RawRotation;

            if (source.NativeSourceIdentity.HasValue)
            {
                // 來源是 pending native copy（尚未 saved）：duplicate-of-duplicate 沿原 native identity，不編造 previous sourceGuid
                sourceProvenance = Guid.Empty;
                nativeIdentity = source.NativeSourceIdentity;
            }
            else
            {
                // 普通物件或已 saved native copied root：沿用普通 ScenarioGuid，且保留相同 RawRotation
                sourceProvenance = source.CopySourceScenarioId != Guid.Empty ? source.CopySourceScenarioId : source.ScenarioId;
            }

            SdlPlacedObject duplicate = Clone(source with
            {
                WorldX = source.WorldX + offsetX,
                WorldZ = source.WorldZ + offsetZ,
                ScenarioId = Guid.NewGuid(),
                CopySourceScenarioId = sourceProvenance,
                NativeSourceIdentity = nativeIdentity,
                RawRotation = rawRotation
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
            if (before.RawRotation.HasValue || before.NativeSourceIdentity.HasValue)
            {
                if (team.HasValue && team.Value != before.Team)
                    throw new InvalidOperationException("所選物件中包含原生群組物件，目前僅支援平移；變更隊伍尚未支援。");
                if (angle.HasValue && ScenarioLevelObjects.NormalizeAngle((int)MathF.Round(angle.Value)) != ScenarioLevelObjects.NormalizeAngle((int)MathF.Round(before.Angle)))
                    throw new InvalidOperationException("所選物件中包含原生群組物件，目前僅支援平移；變更方向尚未支援。");
            }
            SdlPlacedObject after = Clone(before with
            {
                Team = team ?? before.Team,
                Angle = angle ?? before.Angle,
                NativeSourceIdentity = before.NativeSourceIdentity,
                RawRotation = before.RawRotation
            });
            ValidateBounds(after, strictUnitCount: true);
            if (after.Team != before.Team || after.Angle != before.Angle)
                commands.Add(new EditPlacementCommand(index, before, after));
        }
        if (commands.Count > 0) Execute(new BatchPlacementCommand("Set selected teams/directions", commands));
    }

    /// <summary>Rotate each selected object relative to its own direction, as one atomic undo step.</summary>
    public void RotateMany(IEnumerable<int> indices, int degrees = 45)
    {
        int[] selected = ValidateIndices(indices);
        var commands = new List<IPlacementCommand>();
        foreach (int index in selected)
        {
            SdlPlacedObject before = _objects[index];
            if (before.RawRotation.HasValue || before.NativeSourceIdentity.HasValue)
                throw new InvalidOperationException("原生群組物件目前僅支援平移；變更方向尚未支援。");
            // Scenario angle is an integer degree, normalized by the existing save path to 0..359.
            float angle = ScenarioLevelObjects.NormalizeAngle((int)MathF.Round(before.Angle) + degrees % 360);
            SdlPlacedObject after = Clone(before with { Angle = angle });
            ValidateBounds(after, strictUnitCount: false);
            if (after.Angle != before.Angle) commands.Add(new EditPlacementCommand(index, before, after));
        }
        if (commands.Count > 0) Execute(new BatchPlacementCommand("Rotate selected objects", commands));
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

    /// <summary>
    /// 從放置物件清單映射出具有有效 previous 預建綁定或原生來源的群組副本請求對照表。
    /// </summary>
    public static IReadOnlyDictionary<Guid, ScenarioFamilyCopyRequest> BuildFamilyCopyRequests(
        IReadOnlyList<SdlPlacedObject> placedObjects,
        ScenarioDocument previousScenario,
        LevelObjectStore store)
        => BuildFamilyCopyRequests(placedObjects, previousScenario, store, null);

    /// <summary>
    /// 從放置物件清單映射出具有有效 previous 預建綁定或原生來源的群組副本請求對照表。
    /// 僅有有效來源綁定或有效原生來源的預建副本會建立請求；未綁定或腳本副本則不建立請求，由呼叫端進行範本或腳本回退。
    /// 若有綁定但來源狀態不符（例如不同隊伍或方向變更），則擲出例外明確拒絕，不靜默降級。
    /// </summary>
    public static IReadOnlyDictionary<Guid, ScenarioFamilyCopyRequest> BuildFamilyCopyRequests(
        IReadOnlyList<SdlPlacedObject> placedObjects,
        ScenarioDocument previousScenario,
        LevelObjectStore store,
        Func<int, string?>? nativeAliasResolver)
    {
        ArgumentNullException.ThrowIfNull(placedObjects);
        ArgumentNullException.ThrowIfNull(previousScenario);
        ArgumentNullException.ThrowIfNull(store);

        var bindingsBySpawnId = previousScenario.DataSlots
            .Where(b => b.SpawnId != Guid.Empty)
            .GroupBy(b => b.SpawnId)
            .ToDictionary(g => g.Key, g => g.ToList());

        var previousSpawns = previousScenario.Spawns
            .ToDictionary(s => s.Id);

        var worldObjects = store.Objects()
            .ToDictionary(obj => obj.Slot);

        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>();

        foreach (var item in placedObjects)
        {
            if (item.CopySourceScenarioId == Guid.Empty && !item.NativeSourceIdentity.HasValue) continue;

            if (item.NativeSourceIdentity is { } nativeId)
            {
                if (nativeAliasResolver is null)
                    throw new InvalidDataException("複製原生群組時必須提供可信原生別名解析器。");

                // 已 bound 到 Scenario 的 root 不能走 NativeRequest
                if (previousScenario.DataSlots.Any(b => b.Slot == nativeId.Slot && b.Uid == nativeId.Uid && b.SpawnId != Guid.Empty))
                    throw new InvalidDataException($"原生副本來源槽位 {nativeId.Slot} 已存在場景綁定，應使用一般副本請求。");

                if (!worldObjects.TryGetValue(nativeId.Slot, out var sourceRoot) || sourceRoot.Uid != nativeId.Uid)
                    throw new InvalidDataException($"原生副本來源槽位 {nativeId.Slot} 的 root UID 無效或物件已不存在。");

                var sourceFamily = store.FamilyAt(nativeId.Slot, nativeId.Uid);
                if (sourceFamily is null)
                    throw new InvalidDataException($"副本來源槽位 {nativeId.Slot} 的群組無效或非獨立支援群組。");

                string? resolvedAlias = nativeAliasResolver(sourceRoot.TypeId);
                string itemAlias = item.Type.TemplateFields.TryGetValue("alias", out string? a) ? a : item.Type.NameDef;
                if (string.IsNullOrWhiteSpace(resolvedAlias) || !resolvedAlias.Equals(itemAlias, StringComparison.OrdinalIgnoreCase))
                    throw new InvalidDataException($"原生副本來源 TypeId {sourceRoot.TypeId} 解析的別名與目標別名不一致。");

                if (sourceRoot.Team != item.Team)
                    throw new InvalidDataException($"群組副本目前只能平移；來源隊伍 ({sourceRoot.Team}) 與目標隊伍 ({item.Team}) 不一致，變更隊伍尚未支援。");

                if (item.RawRotation is not { } targetRaw || !float.IsFinite(targetRaw))
                    throw new InvalidDataException("原生群組副本目標必須明確提供有限的原生旋轉弧度。");

                if (!float.IsFinite(sourceRoot.Rotation) || BitConverter.SingleToInt32Bits(targetRaw) != BitConverter.SingleToInt32Bits(sourceRoot.Rotation))
                    throw new InvalidDataException("原生群組副本目標旋轉弧度必須與來源 bit-exact 一致。");

                int expectedDisplayAngle = ScenarioLevelObjects.RadiansToDisplayAngle(sourceRoot.Rotation);
                if (ScenarioLevelObjects.NormalizeAngle((int)MathF.Round(item.Angle)) != expectedDisplayAngle)
                    throw new InvalidDataException("群組副本目前只能平移；變更方向尚未支援。");

                requests[item.ScenarioId] = new ScenarioFamilyCopyRequest(nativeId.Slot, nativeId.Uid, isNativeSource: true);
            }
            else
            {
                // 僅預建建築或保留 RawRotation 物件需要考慮 saved-family copy
                bool isPrebuilt = (item.Type.Category == SdlObjectCategory.Building && item.Team is >= 0 and <= 8) || item.RawRotation.HasValue;
                // 尋找 previous 中對應來源 Guid 的綁定
                if (!bindingsBySpawnId.TryGetValue(item.CopySourceScenarioId, out var matchingBindings))
                    continue; // 未 bound，保持一般 template / fallback
                if (matchingBindings.Count != 1)
                    throw new InvalidDataException("群組副本來源具有重複的 DATA 綁定。");
                if (!isPrebuilt)
                    throw new InvalidDataException("已綁定的預建群組副本不能降級為腳本物件或使用不支援的隊伍。");

                var sourceBinding = matchingBindings[0];

                if (!previousSpawns.TryGetValue(item.CopySourceScenarioId, out var sourceSpawn) || !sourceSpawn.Prebuilt)
                    throw new InvalidDataException("群組副本的 DATA 綁定缺少有效預建來源物件。");

                if (!worldObjects.TryGetValue(sourceBinding.Slot, out var sourceRoot) || sourceRoot.Uid != sourceBinding.Uid)
                    throw new InvalidDataException($"副本來源槽位 {sourceBinding.Slot} 的 root UID 無效或物件已不存在。");

                var sourceFamily = store.FamilyAt(sourceBinding.Slot, sourceBinding.Uid);
                if (sourceFamily is null)
                    throw new InvalidDataException($"副本來源槽位 {sourceBinding.Slot} 的群組無效或非獨立支援群組。");

                string itemAlias = item.Type.TemplateFields.TryGetValue("alias", out string? a) ? a : item.Type.NameDef;
                if (!sourceSpawn.Alias.Equals(itemAlias, StringComparison.OrdinalIgnoreCase))
                    throw new InvalidDataException("副本物件的別名必須與來源一致。");

                // 驗證契約（隊伍 / 方向 / 別名）
                if (sourceSpawn.Team != item.Team || sourceRoot.Team != item.Team)
                    throw new InvalidDataException($"群組副本目前只能平移；來源隊伍 ({sourceSpawn.Team}) 與目標隊伍 ({item.Team}) 不一致，變更隊伍尚未支援。");

                if (sourceSpawn.RawRotation is { } sourceRaw || item.RawRotation is { } targetRaw)
                {
                    if (sourceSpawn.RawRotation is not { } sRaw || item.RawRotation is not { } tRaw
                        || !float.IsFinite(sRaw) || !float.IsFinite(tRaw)
                        || BitConverter.SingleToInt32Bits(sRaw) != BitConverter.SingleToInt32Bits(tRaw)
                        || BitConverter.SingleToInt32Bits(sourceRoot.Rotation) != BitConverter.SingleToInt32Bits(tRaw)
                        || ScenarioLevelObjects.NormalizeAngle((int)MathF.Round(sourceSpawn.Angle)) != ScenarioLevelObjects.NormalizeAngle((int)MathF.Round(item.Angle)))
                        throw new InvalidDataException("群組副本目前只能平移；變更隊伍或方向尚未支援。");
                }
                else
                {
                    double actualAngle = sourceRoot.Rotation * (180d / Math.PI);
                    double desiredTargetAngle = ((item.Angle % 360d) + 360d) % 360d;
                    double angleDifference = Math.Abs(((actualAngle - desiredTargetAngle + 540d) % 360d + 360d) % 360d - 180d);

                    double sourceSpawnAngle = ((sourceSpawn.Angle % 360d) + 360d) % 360d;
                    if (sourceSpawnAngle != desiredTargetAngle || !double.IsFinite(actualAngle) || angleDifference > .001d)
                        throw new InvalidDataException("群組副本目前只能平移；變更方向尚未支援。");
                }

                requests[item.ScenarioId] = new ScenarioFamilyCopyRequest(sourceBinding.Slot, sourceBinding.Uid, isNativeSource: false);
            }
        }

        return requests;
    }

    public static void ValidateBounds(SdlPlacedObject item, bool strictUnitCount = false)
    {
        ArgumentNullException.ThrowIfNull(item);
        if (!float.IsFinite(item.WorldX) || !float.IsFinite(item.WorldY) || !float.IsFinite(item.WorldZ)
            || item.WorldX is < 0 or > 16383 || item.WorldZ is < 0 or > 16383)
            throw new ArgumentOutOfRangeException(nameof(item), "Position must be finite with X/Z between 0 and 16383.");
        if (!float.IsFinite(item.Angle))
            throw new ArgumentOutOfRangeException(nameof(item), "Angle must be finite.");
        if (item.Type.Category == SdlObjectCategory.Figure && !item.Type.IsAnimal && !item.RawRotation.HasValue)
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
            if (item.Type.IsAnimal && item.UnitCount != 0)
                throw new ArgumentOutOfRangeException(nameof(item), "Animals are individual objects and must have unit count 0.");
            if (item.Team is < -1 or > 15)
                throw new ArgumentOutOfRangeException(nameof(item), $"Object team must be between -1 and 15, got {item.Team}.");
        }
    }

    internal static SdlPlacedObject Clone(SdlPlacedObject item) => item with
    {
        Type = item.Type with { TemplateFields = new Dictionary<string, string>(item.Type.TemplateFields, StringComparer.OrdinalIgnoreCase) }
    };

    private static IReadOnlyList<SdlPlacedObject> Copy(IReadOnlyList<SdlPlacedObject> items) => items.Select(Clone).ToArray();

    private static bool NullableFloatBitsEqual(float? a, float? b)
    {
        if (!a.HasValue && !b.HasValue) return true;
        if (!a.HasValue || !b.HasValue) return false;
        return BitConverter.SingleToInt32Bits(a.Value) == BitConverter.SingleToInt32Bits(b.Value);
    }

    private static bool Equal(SdlPlacedObject a, SdlPlacedObject b) =>
        a.ScenarioId == b.ScenarioId && a.CopySourceScenarioId == b.CopySourceScenarioId
        && a.NativeSourceIdentity == b.NativeSourceIdentity
        && NullableFloatBitsEqual(a.RawRotation, b.RawRotation)
        && a.WorldX == b.WorldX && a.WorldY == b.WorldY && a.WorldZ == b.WorldZ
        && a.Team == b.Team && a.Angle == b.Angle && a.UnitCount == b.UnitCount
        && a.Type.NameDef == b.Type.NameDef && a.Type.Definition == b.Type.Definition
        && a.Type.Category == b.Type.Category && a.Type.Tribe == b.Type.Tribe && a.Type.Occurrences == b.Type.Occurrences
        && a.Type.TemplateFields.Count == b.Type.TemplateFields.Count
        && a.Type.TemplateFields.All(pair => b.Type.TemplateFields.TryGetValue(pair.Key, out string? value) && pair.Value == value);
}
