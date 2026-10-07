using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor.Modules.Events;

/// <summary>事件編輯狀態，不依賴 Form、遊戲路徑、檔案存取或渲染器。</summary>
public sealed class ScenarioEventSession : IEditorModule<IReadOnlyList<ScenarioEvent>>
{
    private readonly List<ScenarioEvent> _events;
    private IReadOnlyList<ScenarioEvent> _baseline = Array.Empty<ScenarioEvent>();
    public ScenarioEventSession() : this(new List<ScenarioEvent>()) { }
    // 舊表單的唯讀儲存接口保留原 List；只有相容 adapter 可以取得此接口。
    internal ScenarioEventSession(List<ScenarioEvent> events) => _events = events;
    public string ModuleId => "events";
    public int Count => _events.Count;
    public IReadOnlyList<ScenarioEvent> Baseline => Copy(_baseline);
    public bool IsDirty => _events.Count != _baseline.Count || _events.Where((item, index) =>
        item.Name != _baseline[index].Name || item.DelaySeconds != _baseline[index].DelaySeconds
        || item.Repeat != _baseline[index].Repeat || item.Enabled != _baseline[index].Enabled
        || !item.Actions.SequenceEqual(_baseline[index].Actions) || !item.Conditions.SequenceEqual(_baseline[index].Conditions)).Any();
    public void Load(IReadOnlyList<ScenarioEvent> snapshot)
    {
        var copy = Copy(snapshot); _events.Clear(); _events.AddRange(copy); AcceptChanges();
    }
    public IReadOnlyList<ScenarioEvent> Capture() => Copy(_events);
    public void AcceptChanges() => AcceptBaseline(_events);
    public void AcceptBaseline(IReadOnlyList<ScenarioEvent> snapshot) => _baseline = Copy(snapshot);
    public void Reset() { _events.Clear(); _events.AddRange(Copy(_baseline)); }
    public int Add(ScenarioEvent item)
    {
        if (_events.Count >= 256) throw new InvalidOperationException("事件上限為 256 個。");
        _events.Add(Clone(item)); return _events.Count - 1;
    }
    public void Replace(int index, ScenarioEvent item) => _events[index] = Clone(item);
    public void RemoveAt(int index) => _events.RemoveAt(index);
    public int Duplicate(int index, string suffix)
    {
        if (_events.Count >= 256) throw new InvalidOperationException("事件上限為 256 個。");
        if (suffix.Length > 100) throw new ArgumentException("副本名稱後綴過長。", nameof(suffix));
        ScenarioEvent source = _events[index];
        _events.Insert(index + 1, Clone(source with { Name = source.Name[..Math.Min(source.Name.Length, 100 - suffix.Length)] + suffix }));
        return index + 1;
    }
    private static ScenarioEvent Clone(ScenarioEvent item) => item with
        { Actions = item.Actions.ToList(), Conditions = item.Conditions.ToList() };
    private static IReadOnlyList<ScenarioEvent> Copy(IReadOnlyList<ScenarioEvent> items) => items.Select(Clone).ToArray();
}
