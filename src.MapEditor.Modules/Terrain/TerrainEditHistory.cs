namespace AgainstRomeMapEditor;

internal sealed record TerrainTextureChange(int X, int Y, string Before, string After);

/// <summary>
/// Pure terrain-edit state shared by the 2D and 3D views. It deliberately has no WinForms or GL dependency,
/// so stroke grouping, dirty tracking, undo/redo and save baselines can be regression-tested in CI.
/// </summary>
internal sealed class TerrainEditHistory
{
    private readonly int _dimension;
    private readonly string[] _current;
    private string[] _baseline;
    private readonly Stack<IReadOnlyList<TerrainTextureChange>> _undo = new();
    private readonly Stack<IReadOnlyList<TerrainTextureChange>> _redo = new();
    private readonly Dictionary<int, TerrainTextureChange> _pending = new();
    private readonly List<int> _pendingOrder = new();
    private readonly HashSet<int> _pendingSeen = new();

    public TerrainEditHistory(int dimension, IReadOnlyList<string> textures)
    {
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(dimension);
        if (textures.Count != dimension * dimension) throw new ArgumentException("材質數量與地圖尺寸不符。", nameof(textures));
        _dimension = dimension;
        _current = textures.ToArray();
        _baseline = textures.ToArray();
    }

    public IReadOnlyList<string> Current => _current;
    public bool CanUndo => _undo.Count > 0 || _pending.Count > 0;
    public bool CanRedo => _redo.Count > 0;
    public bool IsDirty => !_current.SequenceEqual(_baseline, StringComparer.OrdinalIgnoreCase);

    public TerrainTextureChange? Paint(int x, int y, string texture)
    {
        if (x is < 0 || x >= _dimension || y is < 0 || y >= _dimension) throw new ArgumentOutOfRangeException(nameof(x));
        if (string.IsNullOrWhiteSpace(texture)) throw new ArgumentException("材質名稱不可為空。", nameof(texture));
        int index = y * _dimension + x;
        string immediateBefore = _current[index];
        if (StringComparer.OrdinalIgnoreCase.Equals(immediateBefore, texture)) return null;

        _current[index] = texture;
        if (_pending.TryGetValue(index, out TerrainTextureChange? existing))
        {
            if (StringComparer.OrdinalIgnoreCase.Equals(existing.Before, texture)) _pending.Remove(index);
            else _pending[index] = existing with { After = texture };
        }
        else
        {
            _pending[index] = new TerrainTextureChange(x, y, immediateBefore, texture);
            if (_pendingSeen.Add(index)) _pendingOrder.Add(index);
        }
        return new TerrainTextureChange(x, y, immediateBefore, texture);
    }

    public bool CommitStroke()
    {
        TerrainTextureChange[] stroke = _pendingOrder.Where(_pending.ContainsKey).Select(index => _pending[index]).ToArray();
        _pending.Clear(); _pendingOrder.Clear(); _pendingSeen.Clear();
        if (stroke.Length == 0) return false;
        _undo.Push(stroke); _redo.Clear();
        return true;
    }

    public IReadOnlyList<TerrainTextureChange>? Undo()
    {
        CommitStroke();
        if (_undo.Count == 0) return null;
        IReadOnlyList<TerrainTextureChange> stroke = _undo.Pop();
        for (int index = stroke.Count - 1; index >= 0; index--)
        {
            TerrainTextureChange change = stroke[index];
            _current[change.Y * _dimension + change.X] = change.Before;
        }
        _redo.Push(stroke);
        return stroke;
    }

    public IReadOnlyList<TerrainTextureChange>? Redo()
    {
        if (_redo.Count == 0) return null;
        IReadOnlyList<TerrainTextureChange> stroke = _redo.Pop();
        foreach (TerrainTextureChange change in stroke) _current[change.Y * _dimension + change.X] = change.After;
        _undo.Push(stroke);
        return stroke;
    }

    public IReadOnlyList<TerrainTextureChange> ResetToBaseline()
    {
        var changes = new List<TerrainTextureChange>();
        for (int index = 0; index < _current.Length; index++)
        {
            if (StringComparer.OrdinalIgnoreCase.Equals(_current[index], _baseline[index])) continue;
            changes.Add(new TerrainTextureChange(index % _dimension, index / _dimension, _current[index], _baseline[index]));
            _current[index] = _baseline[index];
        }
        _undo.Clear(); _redo.Clear(); _pending.Clear(); _pendingOrder.Clear(); _pendingSeen.Clear();
        return changes;
    }

    public void CommitBaseline()
    {
        CommitStroke();
        _baseline = _current.ToArray();
    }
}
