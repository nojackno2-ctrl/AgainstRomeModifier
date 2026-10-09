using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor.Modules.Events;

/// <summary>事件編輯狀態，不依賴 Form、遊戲路徑、檔案存取或渲染器。</summary>
public sealed class ScenarioEventSession : IEditorModule<IReadOnlyList<ScenarioEvent>>
{
    private readonly List<ScenarioEvent> _events;
    private IReadOnlyList<ScenarioEvent> _baseline = Array.Empty<ScenarioEvent>();
    private readonly Stack<IReadOnlyList<ScenarioEvent>> _undo = new();
    private readonly Stack<IReadOnlyList<ScenarioEvent>> _redo = new();
    public ScenarioEventSession() : this(new List<ScenarioEvent>()) { }
    // 舊表單的唯讀儲存接口保留原 List；只有相容 adapter 可以取得此接口。
    internal ScenarioEventSession(List<ScenarioEvent> events) => _events = events;
    public string ModuleId => "events";
    public int Count => _events.Count;
    public bool CanUndo => _undo.Count > 0;
    public bool CanRedo => _redo.Count > 0;
    public IReadOnlyList<ScenarioEvent> Baseline => Copy(_baseline);
    public bool IsDirty => _events.Count != _baseline.Count || _events.Where((item, index) =>
        item.Name != _baseline[index].Name || item.DelaySeconds != _baseline[index].DelaySeconds
        || item.Repeat != _baseline[index].Repeat || item.Enabled != _baseline[index].Enabled
        || !item.Actions.SequenceEqual(_baseline[index].Actions) || !item.Conditions.SequenceEqual(_baseline[index].Conditions)).Any();
    public void Load(IReadOnlyList<ScenarioEvent> snapshot)
    {
        Restore(snapshot); _undo.Clear(); _redo.Clear(); AcceptChanges();
    }
    public IReadOnlyList<ScenarioEvent> Capture() => Copy(_events);
    public void AcceptChanges() => AcceptBaseline(_events);
    public void AcceptBaseline(IReadOnlyList<ScenarioEvent> snapshot) => _baseline = Copy(snapshot);
    public void Reset() { Restore(_baseline); _undo.Clear(); _redo.Clear(); }
    public int Add(ScenarioEvent item)
    {
        int index = _events.Count; AddRange([item]); return index;
    }
    /// <summary>Append one atomic, undoable batch while retaining the save baseline.</summary>
    public void AddRange(IReadOnlyList<ScenarioEvent> items)
    {
        if (items.Count > 256 - _events.Count) throw new InvalidOperationException("事件上限為 256 個。");
        var copy = Copy(items);
        if (copy.Count == 0) return;
        Remember(); _events.AddRange(copy);
    }
    public void ReplaceAll(IReadOnlyList<ScenarioEvent> items)
    {
        if (items.Count > 256) throw new InvalidOperationException("事件上限為 256 個。");
        var copy = Copy(items); Remember(); Restore(copy);
    }
    public void Replace(int index, ScenarioEvent item)
    {
        _ = _events[index]; var copy = Clone(item); Remember(); _events[index] = copy;
    }
    public void RemoveAt(int index) { _ = _events[index]; Remember(); _events.RemoveAt(index); }
    public int Duplicate(int index, string suffix)
    {
        if (_events.Count >= 256) throw new InvalidOperationException("事件上限為 256 個。");
        if (suffix.Length > 100) throw new ArgumentException("副本名稱後綴過長。", nameof(suffix));
        ScenarioEvent source = _events[index];
        var copy = Clone(source with { Name = source.Name[..Math.Min(source.Name.Length, 100 - suffix.Length)] + suffix });
        Remember(); _events.Insert(index + 1, copy);
        return index + 1;
    }
    public bool Undo()
    {
        if (!CanUndo) return false;
        _redo.Push(Capture()); Restore(_undo.Pop()); return true;
    }
    public bool Redo()
    {
        if (!CanRedo) return false;
        _undo.Push(Capture()); Restore(_redo.Pop()); return true;
    }
    private void Remember() { _undo.Push(Capture()); _redo.Clear(); }
    private void Restore(IReadOnlyList<ScenarioEvent> items)
    {
        var copy = Copy(items); _events.Clear(); _events.AddRange(copy);
    }
    private static ScenarioEvent Clone(ScenarioEvent item) => item with
        { Actions = item.Actions.ToList(), Conditions = item.Conditions.ToList() };
    private static IReadOnlyList<ScenarioEvent> Copy(IReadOnlyList<ScenarioEvent> items) => items.Select(Clone).ToArray();
}
