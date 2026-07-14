namespace AgainstRomeMapEditor;

internal sealed record TerrainBlendPaintResult(
    bool Succeeded,
    IReadOnlyList<TerrainTextureChange> TextureChanges,
    IReadOnlyList<NativeTerrainBakeIssue> Issues)
{
    public static TerrainBlendPaintResult Success(IReadOnlyList<TerrainTextureChange> changes) => new(true, changes, Array.Empty<NativeTerrainBakeIssue>());
    public static TerrainBlendPaintResult Rejected(IReadOnlyList<TerrainTextureChange> rollback, IReadOnlyList<NativeTerrainBakeIssue> issues) => new(false, rollback, issues);
}

internal sealed record TerrainCornerChange(int Index, string Before, string After);

internal sealed record TerrainBlendStroke(IReadOnlyList<TerrainCornerChange> Corners, IReadOnlyList<TerrainTextureChange> Textures);

/// <summary>
/// Transactional authoring state for native terrain blending. A pointer stroke changes corner materials
/// and their baked game tiles as one unit; an unsupported native junction rejects and rolls back the
/// entire pending stroke instead of leaving a partially baked or editor-only result.
/// </summary>
internal sealed class TerrainBlendEditSession
{
    private readonly FloorMaterialCatalog _catalog;
    private readonly TerrainBlendAuthoringMap _map;
    private readonly string[] _currentTextures;
    private string[] _baselineTextures;
    private string[] _baselineCorners;
    private readonly Dictionary<int, TerrainCornerChange> _pendingCorners = new();
    private readonly Dictionary<int, TerrainTextureChange> _pendingTextures = new();
    private readonly Stack<TerrainBlendStroke> _undo = new();
    private readonly Stack<TerrainBlendStroke> _redo = new();

    public TerrainBlendEditSession(NativeTerrainImportResult import, IReadOnlyList<string> sourceTextures, FloorMaterialCatalog catalog)
    {
        ArgumentNullException.ThrowIfNull(import);
        ArgumentNullException.ThrowIfNull(catalog);
        if (sourceTextures.Count != import.Map.TileDimension * import.Map.TileDimension) throw new ArgumentException("材質數量與地圖尺寸不符。", nameof(sourceTextures));
        _catalog = catalog;
        _map = import.Map;
        _currentTextures = sourceTextures.ToArray();
        _baselineTextures = sourceTextures.ToArray();
        _baselineCorners = _map.CornerMaterials.ToArray();
        UnresolvedTileIndices = import.UnresolvedTileIndices;
        InitialCornerConflicts = import.CornerConflicts;
    }

    public IReadOnlyList<string> CurrentTextures => _currentTextures;
    public IReadOnlyList<int> UnresolvedTileIndices { get; }
    public IReadOnlyList<NativeTerrainCornerConflict> InitialCornerConflicts { get; }
    public bool CanUndo => _undo.Count > 0 || _pendingCorners.Count > 0 || _pendingTextures.Count > 0;
    public bool CanRedo => _redo.Count > 0;
    public bool IsDirty => !_currentTextures.SequenceEqual(_baselineTextures, StringComparer.OrdinalIgnoreCase) ||
                           !_map.CornerMaterials.SequenceEqual(_baselineCorners, StringComparer.OrdinalIgnoreCase);

    public TerrainBlendPaintResult PaintCircle(float centerX, float centerY, float radius, string materialId)
    {
        string[] cornerBefore = _map.CornerMaterials.ToArray();
        IReadOnlyList<int> changedCorners = _map.PaintCircle(centerX, centerY, radius, materialId);
        if (changedCorners.Count == 0) return TerrainBlendPaintResult.Success(Array.Empty<TerrainTextureChange>());

        int[] affectedTiles = AffectedTiles(changedCorners).ToArray();
        var baked = new List<(int Index, string Texture)>();
        var issues = new List<NativeTerrainBakeIssue>();
        foreach (int tileIndex in affectedTiles)
        {
            int x = tileIndex % _map.TileDimension, y = tileIndex / _map.TileDimension;
            string[] corners = [_map.GetCorner(x, y), _map.GetCorner(x + 1, y), _map.GetCorner(x + 1, y + 1), _map.GetCorner(x, y + 1)];
            string? texture = _catalog.ResolveNativeTile(corners, x, y);
            if (texture is null) issues.Add(new NativeTerrainBakeIssue(x, y, corners));
            else baked.Add((tileIndex, texture));
        }
        if (issues.Count > 0)
        {
            foreach (int index in changedCorners) SetCorner(index, cornerBefore[index]);
            IReadOnlyList<TerrainTextureChange> rollback = CancelStroke();
            return TerrainBlendPaintResult.Rejected(rollback, issues);
        }

        foreach (int index in changedCorners)
        {
            string before = cornerBefore[index], after = _map.CornerMaterials[index];
            if (_pendingCorners.TryGetValue(index, out TerrainCornerChange? pending)) _pendingCorners[index] = pending with { After = after };
            else _pendingCorners[index] = new TerrainCornerChange(index, before, after);
        }

        var changes = new List<TerrainTextureChange>();
        foreach ((int index, string texture) in baked)
        {
            string immediateBefore = _currentTextures[index];
            if (StringComparer.OrdinalIgnoreCase.Equals(immediateBefore, texture)) continue;
            var change = new TerrainTextureChange(index % _map.TileDimension, index / _map.TileDimension, immediateBefore, texture);
            _currentTextures[index] = texture;
            changes.Add(change);
            if (_pendingTextures.TryGetValue(index, out TerrainTextureChange? pending)) _pendingTextures[index] = pending with { After = texture };
            else _pendingTextures[index] = change;
        }
        return TerrainBlendPaintResult.Success(changes);
    }

    public bool CommitStroke()
    {
        TerrainCornerChange[] corners = _pendingCorners.Values.Where(change => !StringComparer.OrdinalIgnoreCase.Equals(change.Before, change.After)).OrderBy(change => change.Index).ToArray();
        TerrainTextureChange[] textures = _pendingTextures.Values.Where(change => !StringComparer.OrdinalIgnoreCase.Equals(change.Before, change.After)).OrderBy(change => change.Y).ThenBy(change => change.X).ToArray();
        _pendingCorners.Clear();
        _pendingTextures.Clear();
        if (corners.Length == 0 && textures.Length == 0) return false;
        _undo.Push(new TerrainBlendStroke(corners, textures));
        _redo.Clear();
        return true;
    }

    public IReadOnlyList<TerrainTextureChange>? Undo()
    {
        CommitStroke();
        if (_undo.Count == 0) return null;
        TerrainBlendStroke stroke = _undo.Pop();
        foreach (TerrainCornerChange change in stroke.Corners.Reverse()) SetCorner(change.Index, change.Before);
        foreach (TerrainTextureChange change in stroke.Textures.Reverse()) _currentTextures[TextureIndex(change.X, change.Y)] = change.Before;
        _redo.Push(stroke);
        return stroke.Textures.Select(change => new TerrainTextureChange(change.X, change.Y, change.After, change.Before)).Reverse().ToArray();
    }

    public IReadOnlyList<TerrainTextureChange>? Redo()
    {
        if (_redo.Count == 0) return null;
        TerrainBlendStroke stroke = _redo.Pop();
        foreach (TerrainCornerChange change in stroke.Corners) SetCorner(change.Index, change.After);
        foreach (TerrainTextureChange change in stroke.Textures) _currentTextures[TextureIndex(change.X, change.Y)] = change.After;
        _undo.Push(stroke);
        return stroke.Textures;
    }

    public IReadOnlyList<TerrainTextureChange> ResetToBaseline()
    {
        var changes = new List<TerrainTextureChange>();
        for (int index = 0; index < _currentTextures.Length; index++)
        {
            if (StringComparer.OrdinalIgnoreCase.Equals(_currentTextures[index], _baselineTextures[index])) continue;
            changes.Add(new TerrainTextureChange(index % _map.TileDimension, index / _map.TileDimension, _currentTextures[index], _baselineTextures[index]));
            _currentTextures[index] = _baselineTextures[index];
        }
        for (int index = 0; index < _baselineCorners.Length; index++) SetCorner(index, _baselineCorners[index]);
        _undo.Clear(); _redo.Clear(); _pendingCorners.Clear(); _pendingTextures.Clear();
        return changes;
    }

    public void CommitBaseline()
    {
        CommitStroke();
        _baselineTextures = _currentTextures.ToArray();
        _baselineCorners = _map.CornerMaterials.ToArray();
    }

    private IReadOnlyList<TerrainTextureChange> CancelStroke()
    {
        foreach (TerrainCornerChange change in _pendingCorners.Values) SetCorner(change.Index, change.Before);
        var rollback = new List<TerrainTextureChange>();
        foreach (TerrainTextureChange change in _pendingTextures.Values)
        {
            int index = TextureIndex(change.X, change.Y);
            string current = _currentTextures[index];
            if (!StringComparer.OrdinalIgnoreCase.Equals(current, change.Before)) rollback.Add(new TerrainTextureChange(change.X, change.Y, current, change.Before));
            _currentTextures[index] = change.Before;
        }
        _pendingCorners.Clear();
        _pendingTextures.Clear();
        return rollback;
    }

    private IEnumerable<int> AffectedTiles(IEnumerable<int> cornerIndices)
    {
        var affected = new HashSet<int>();
        foreach (int cornerIndex in cornerIndices)
        {
            int cornerX = cornerIndex % _map.CornerDimension, cornerY = cornerIndex / _map.CornerDimension;
            for (int y = cornerY - 1; y <= cornerY; y++)
            for (int x = cornerX - 1; x <= cornerX; x++)
                if (x >= 0 && y >= 0 && x < _map.TileDimension && y < _map.TileDimension) affected.Add(y * _map.TileDimension + x);
        }
        return affected;
    }

    private int TextureIndex(int x, int y) => y * _map.TileDimension + x;

    private void SetCorner(int index, string materialId) => _map.SetCorner(index % _map.CornerDimension, index / _map.CornerDimension, materialId);
}
