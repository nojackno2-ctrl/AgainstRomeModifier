using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.Modules.Nature;

public sealed record NatureAddition(LevelObjectTemplate Template, string Name, float X, float Y, float Z, float Rotation);
public sealed record NatureSnapshot(IReadOnlyList<int> RemovedSlots, IReadOnlyList<NatureAddition> Additions);

/// <summary>待存的自然物件變更與筆觸歷史；範本、座標及可移除判定由宿主提供。</summary>
public sealed class NatureEditSession : IEditorModule<NatureSnapshot>
{
    private sealed record IndexedAddition(int Index, NatureAddition Item);
    private sealed record Operation(IndexedAddition? Added, int[] Removed, IndexedAddition[] RemovedAdditions);
    private readonly HashSet<int> _removals = new();
    private readonly List<NatureAddition> _additions = new();
    private readonly Stack<Operation[]> _undo = new(), _redo = new();
    private readonly List<Operation> _stroke = new();
    private NatureSnapshot _baseline = new([], []);
    public string ModuleId => "nature";
    public bool IsDirty => !_removals.SetEquals(_baseline.RemovedSlots) || !_additions.SequenceEqual(_baseline.Additions);
    public bool CanUndo => _undo.Count > 0 || _stroke.Count > 0;
    public bool CanRedo => _redo.Count > 0;
    public IReadOnlySet<int> RemovedSlots => new HashSet<int>(_removals);
    public IReadOnlyList<NatureAddition> Additions => _additions.ToArray();
    public NatureSnapshot Capture() => new(_removals.Order().ToArray(), _additions.ToArray());
    public void Load(NatureSnapshot snapshot)
    {
        ArgumentNullException.ThrowIfNull(snapshot);
        int[] slots = snapshot.RemovedSlots.ToArray();
        NatureAddition[] additions = snapshot.Additions.ToArray();
        _removals.Clear(); _removals.UnionWith(slots);
        _additions.Clear(); _additions.AddRange(additions);
        AcceptChanges();
    }
    public void AcceptChanges() { _baseline = Capture(); ClearHistory(); }
    public void Reset() => Load(_baseline);
    public void Clear() => Load(new([], []));
    public void Plant(NatureAddition addition)
    {
        ArgumentNullException.ThrowIfNull(addition);
        var added = new IndexedAddition(_additions.Count, addition);
        _additions.Add(addition); Record(new(added, [], []));
    }
    public bool Remove(IEnumerable<int> slots, IEnumerable<NatureAddition> additions)
    {
        int[] removed = slots.Distinct().Where(slot => !_removals.Contains(slot)).ToArray();
        var remaining = _additions.Select((item, index) => new IndexedAddition(index, item)).ToList();
        var removedItems = new List<IndexedAddition>();
        foreach (NatureAddition addition in additions)
        {
            int index = remaining.FindIndex(entry => entry.Item == addition);
            if (index < 0) continue;
            removedItems.Add(remaining[index]); remaining.RemoveAt(index);
        }
        IndexedAddition[] removedAdditions = removedItems.OrderBy(entry => entry.Index).ToArray();
        if (removed.Length == 0 && removedAdditions.Length == 0) return false;
        _removals.UnionWith(removed);
        foreach (IndexedAddition addition in removedAdditions.AsEnumerable().Reverse()) _additions.RemoveAt(addition.Index);
        Record(new(null, removed, removedAdditions)); return true;
    }
    private void Record(Operation operation) { _redo.Clear(); _stroke.Add(operation); }
    private void ClearHistory() { _undo.Clear(); _redo.Clear(); _stroke.Clear(); }
    public bool CommitStroke()
    {
        if (_stroke.Count == 0) return false;
        _undo.Push(_stroke.ToArray()); _stroke.Clear(); return true;
    }
    public bool Undo()
    {
        CommitStroke();
        if (!_undo.TryPop(out Operation[]? stroke)) return false;
        foreach (Operation operation in stroke.AsEnumerable().Reverse())
        {
            if (operation.Added is { } added) _additions.RemoveAt(added.Index);
            _removals.ExceptWith(operation.Removed);
            foreach (IndexedAddition addition in operation.RemovedAdditions) _additions.Insert(addition.Index, addition.Item);
        }
        _redo.Push(stroke); return true;
    }
    public bool Redo()
    {
        CommitStroke();
        if (!_redo.TryPop(out Operation[]? stroke)) return false;
        foreach (Operation operation in stroke)
        {
            if (operation.Added is { } added) _additions.Insert(added.Index, added.Item);
            _removals.UnionWith(operation.Removed);
            foreach (IndexedAddition addition in operation.RemovedAdditions.AsEnumerable().Reverse()) _additions.RemoveAt(addition.Index);
        }
        _undo.Push(stroke); return true;
    }
}
