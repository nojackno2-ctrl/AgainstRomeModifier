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
internal sealed record TerrainRoadPaintResult(bool Succeeded, IReadOnlyList<TerrainTextureChange> TextureChanges, IReadOnlyList<(int X, int Y)> Unsupported);

/// <summary>
/// Transactional authoring state for native terrain blending. A pointer stroke changes corner materials
/// and their baked game tiles as one unit; an unsupported native junction rejects and rolls back the
/// entire pending stroke instead of leaving a partially baked or editor-only result.
/// </summary>
internal sealed class TerrainBlendEditSession
{
    private INativeTerrainMaterialResolver _catalog;
    private readonly TerrainBlendAuthoringMap _map;
    private readonly string[] _currentTextures;
    private string[] _baselineTextures;
    private string[] _baselineCorners;
    private readonly Dictionary<int, TerrainCornerChange> _pendingCorners = new();
    private readonly Dictionary<int, TerrainTextureChange> _pendingTextures = new();
    private readonly Stack<TerrainBlendStroke> _undo = new();
    private readonly Stack<TerrainBlendStroke> _redo = new();

    public TerrainBlendEditSession(NativeTerrainImportResult import, IReadOnlyList<string> sourceTextures, INativeTerrainMaterialResolver catalog)
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
    public int TileDimension => _map.TileDimension;
    public string GetTexture(int x, int y) => _currentTextures[TextureIndex(x, y)];
    public IReadOnlyList<int> UnresolvedTileIndices { get; }
    public IReadOnlyList<NativeTerrainCornerConflict> InitialCornerConflicts { get; }
    public bool CanUndo => _undo.Count > 0 || _pendingCorners.Count > 0 || _pendingTextures.Count > 0;
    public bool CanRedo => _redo.Count > 0;
    public bool IsDirty => !_currentTextures.SequenceEqual(_baselineTextures, StringComparer.OrdinalIgnoreCase) ||
                           !_map.CornerMaterials.SequenceEqual(_baselineCorners, StringComparer.OrdinalIgnoreCase);

    /// <summary>Replace resource metadata while retaining authoring corners, textures, baseline and history.</summary>
    public void RebindMaterialResolver(INativeTerrainMaterialResolver catalog)
    {
        ArgumentNullException.ThrowIfNull(catalog);
        _catalog = catalog;
    }

    /// <summary>Independent authoring snapshot for dry runs; history and baseline belong to the copy.</summary>
    internal TerrainBlendEditSession Fork()
    {
        var map = new TerrainBlendAuthoringMap(_map.TileDimension, _map.CornerMaterials[0]);
        for (int y = 0; y < map.CornerDimension; y++)
            for (int x = 0; x < map.CornerDimension; x++) map.SetCorner(x, y, _map.GetCorner(x, y));
        return new(new NativeTerrainImportResult(map, UnresolvedTileIndices, InitialCornerConflicts), _currentTextures, _catalog);
    }

    /// <summary>自動過渡最多向外插入幾圈中介材質。</summary>
    internal const int MaxBridgeRings = 2;

    /// <summary>
    /// By default, a rejected area cancels the pending stroke. Set <paramref name="rollbackStrokeOnFailure"/> to false to preserve earlier accepted areas.
    /// <paramref name="autoBridge"/> 為 true 時，若筆刷邊緣找不到原版過渡 tile，會在外側一圈角點插入同時能銜接兩側的中介材質（最多 <see cref="MaxBridgeRings"/> 圈），
    /// 結果仍全部是原版 tile；仍無法銜接才拒絕。
    /// </summary>
    public TerrainBlendPaintResult PaintCircle(float centerX, float centerY, float radius, string materialId, bool rollbackStrokeOnFailure = true, bool autoBridge = false, Func<int, int, bool>? allowsTile = null)
    {
        if (allowsTile is not null && autoBridge) throw new ArgumentException("Protected painting cannot auto-bridge beyond its boundary.", nameof(autoBridge));
        string[] cornerBefore = _map.CornerMaterials.ToArray();
        IReadOnlyList<int> changedCorners = _map.PaintCircle(centerX, centerY, radius, materialId);
        if (allowsTile is not null)
        {
            var allowed = new List<int>();
            foreach (int corner in changedCorners)
            {
                if (AffectedTiles([corner]).All(tile => allowsTile(tile % _map.TileDimension, tile / _map.TileDimension))) allowed.Add(corner);
                else SetCorner(corner, cornerBefore[corner]);
            }
            changedCorners = allowed;
        }
        return BakeChanges(cornerBefore, changedCorners, materialId, rollbackStrokeOnFailure, autoBridge);
    }

    /// <summary>Paint all four corners of each selected tile, then bake once as an atomic stroke.</summary>
    public TerrainBlendPaintResult PaintTiles(IReadOnlyCollection<(int X, int Y)> tiles, string materialId, bool autoBridge = false)
    {
        ArgumentNullException.ThrowIfNull(tiles);
        if (string.IsNullOrWhiteSpace(materialId)) throw new ArgumentException("Material is required.", nameof(materialId));
        if (tiles.Any(tile => tile.X < 0 || tile.Y < 0 || tile.X >= _map.TileDimension || tile.Y >= _map.TileDimension))
            throw new ArgumentOutOfRangeException(nameof(tiles));
        string[] before = _map.CornerMaterials.ToArray();
        var changed = new HashSet<int>();
        foreach (var tile in tiles)
            foreach (var corner in new[] { (tile.X, tile.Y), (tile.X + 1, tile.Y), (tile.X + 1, tile.Y + 1), (tile.X, tile.Y + 1) })
            {
                int index = corner.Item2 * _map.CornerDimension + corner.Item1;
                if (StringComparer.OrdinalIgnoreCase.Equals(_map.GetCorner(corner.Item1, corner.Item2), materialId)) continue;
                _map.SetCorner(corner.Item1, corner.Item2, materialId);
                changed.Add(index);
            }
        return BakeChanges(before, changed.ToArray(), materialId, false, autoBridge);
    }

    private TerrainBlendPaintResult BakeChanges(string[] cornerBefore, IReadOnlyList<int> changedCorners, string materialId, bool rollbackStrokeOnFailure, bool autoBridge)
    {
        if (changedCorners.Count == 0) return TerrainBlendPaintResult.Success(Array.Empty<TerrainTextureChange>());

        var touched = new HashSet<int>(changedCorners);
        List<NativeTerrainBakeIssue> issues = Resolve(touched, out List<(int Index, string Texture)> baked);
        if (autoBridge && issues.Count > 0)
        {
            var layer = new HashSet<int>(changedCorners);
            for (int ring = 0; ring < MaxBridgeRings && issues.Count > 0; ring++)
            {
                if (!TryBridge(touched, layer, issues, materialId, out HashSet<int>? next)) break;
                layer = next!;
                issues = Resolve(touched, out baked);
            }
            changedCorners = touched.ToArray();
        }
        if (issues.Count > 0)
        {
            foreach (int index in changedCorners) SetCorner(index, cornerBefore[index]);
            IReadOnlyList<TerrainTextureChange> rollback = rollbackStrokeOnFailure ? CancelStroke() : Array.Empty<TerrainTextureChange>();
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

    /// <summary>
    /// 圖塊印章：把單一 tile 直接設為指定的原版圖塊（道路、河流、岩壁等不屬於材質過渡系統的裝飾圖塊），
    /// 不改角點材質也不做過渡烘焙；與筆刷相同地記入待提交筆畫，可復原／重做。之後在相鄰處塗材質會依角點重新烘焙並覆蓋。
    /// </summary>
    public TerrainTextureChange? StampTexture(int x, int y, string texture)
    {
        if (x < 0 || y < 0 || x >= _map.TileDimension || y >= _map.TileDimension || string.IsNullOrWhiteSpace(texture)) return null;
        int index = TextureIndex(x, y);
        string before = _currentTextures[index];
        if (StringComparer.OrdinalIgnoreCase.Equals(before, texture)) return null;
        var change = new TerrainTextureChange(x, y, before, texture);
        _currentTextures[index] = texture;
        if (_pendingTextures.TryGetValue(index, out TerrainTextureChange? pending)) _pendingTextures[index] = pending with { After = texture };
        else _pendingTextures[index] = change;
        _redo.Clear();
        return change;
    }

    /// <summary>Paints one connected road stroke using native pieces; rejection restores the complete pending stroke.</summary>
    public TerrainRoadPaintResult PaintRoadPath(IReadOnlyList<(int X, int Y)> path, IReadOnlyList<RoadTile> available, string preferredTexture)
    {
        RoadStrokePlan plan = RoadStrokePlanner.Plan(_map.TileDimension, _currentTextures, path, available, preferredTexture);
        if (!plan.Succeeded) return new(false, CancelStroke(), plan.Unsupported);
        var changes = new List<TerrainTextureChange>();
        foreach (RoadTilePlacement tile in plan.Tiles)
            if (StampTexture(tile.X, tile.Y, tile.Texture) is { } change) changes.Add(change);
        return new(true, changes, []);
    }

    /// <summary>
    /// 繪製單段連續河流水系筆畫（包含河床水面選片、兩側 ErdeFlussR 河岸過渡圖塊與高度場落差自動微調）。
    /// 若遇未支援圖塊或規劃失敗，自動撤銷並還原整筆待提交變更。
    /// </summary>
    public TerrainRiverPaintResult PaintRiverPath(
        IReadOnlyList<(int X, int Y)> path,
        RiverTileCatalog catalog,
        RiverPlannerOptions? options = null,
        TerrainHeightEditSession? heightSession = null)
    {
        options ??= new RiverPlannerOptions();
        IReadOnlyList<byte>? heights = heightSession?.Heights;
        int vertexSize = heightSession?.VertexSize ?? 0;

        RiverStrokePlan plan = RiverFlowPlanner.Plan(
            _map.TileDimension,
            _currentTextures,
            heights,
            vertexSize,
            path,
            catalog,
            options);

        if (!plan.Succeeded)
        {
            var rollback = CancelStroke();
            heightSession?.CancelStroke();
            return new TerrainRiverPaintResult(false, rollback, [], plan.Warnings, plan.Unsupported);
        }

        var textureChanges = new List<TerrainTextureChange>();

        // 1. 套用河道水面圖塊
        foreach (var tile in plan.WaterTiles)
        {
            if (StampTexture(tile.X, tile.Y, tile.Texture) is { } change)
            {
                textureChanges.Add(change);
            }
        }

        // 2. 套用兩側河岸過渡圖塊
        foreach (var bank in plan.BankTiles)
        {
            if (StampTexture(bank.X, bank.Y, bank.Texture) is { } change)
            {
                textureChanges.Add(change);
            }
        }

        // 3. 套用高度場調整
        var heightChanges = new List<TerrainSampleChange>();
        if (heightSession is not null && plan.HeightAdjustments.Count > 0)
        {
            var adjustments = plan.HeightAdjustments.Select(a => (a.Index, a.After)).ToArray();
            var applied = heightSession.ApplyHeightAdjustments(adjustments);
            heightChanges.AddRange(applied);
        }

        return new TerrainRiverPaintResult(true, textureChanges, heightChanges, plan.Warnings, []);
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

    public IReadOnlyList<TerrainTextureChange> CancelStroke()
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

    private List<NativeTerrainBakeIssue> Resolve(IEnumerable<int> corners, out List<(int Index, string Texture)> baked)
    {
        baked = new List<(int Index, string Texture)>();
        var issues = new List<NativeTerrainBakeIssue>();
        foreach (int tileIndex in AffectedTiles(corners))
        {
            int x = tileIndex % _map.TileDimension, y = tileIndex / _map.TileDimension;
            string[] tileCorners = TileCorners(x, y);
            string? texture = _catalog.ResolveNativeTile(tileCorners, x, y);
            if (texture is null) issues.Add(new NativeTerrainBakeIssue(x, y, tileCorners));
            else baked.Add((tileIndex, texture));
        }
        return issues;
    }

    private string[] TileCorners(int x, int y) => [_map.GetCorner(x, y), _map.GetCorner(x + 1, y), _map.GetCorner(x + 1, y + 1), _map.GetCorner(x, y + 1)];

    private int[] TileCornerIndices(int x, int y)
        => [y * _map.CornerDimension + x, y * _map.CornerDimension + x + 1, (y + 1) * _map.CornerDimension + x + 1, (y + 1) * _map.CornerDimension + x];

    /// <summary>
    /// 對失敗 tile 中尚未屬於本筆畫的角點（下一圈）嘗試每一種中介材質：內側（含上一層角點的 tile）必須全部可解，
    /// 外側失敗數最少者勝出，同分時改動角點較少者優先。成功時套用並回傳新的一圈角點。
    /// </summary>
    private bool TryBridge(HashSet<int> touched, HashSet<int> layer, List<NativeTerrainBakeIssue> issues, string paintedMaterial, out HashSet<int>? ring)
    {
        ring = new HashSet<int>();
        foreach (NativeTerrainBakeIssue issue in issues)
            foreach (int corner in TileCornerIndices(issue.TileX, issue.TileY))
                if (!touched.Contains(corner)) ring.Add(corner);
        if (ring.Count == 0) { ring = null; return false; }
        var original = ring.ToDictionary(index => index, index => _map.CornerMaterials[index]);
        int[] innerTiles = AffectedTiles(ring).Where(tile => TileCornerIndices(tile % _map.TileDimension, tile / _map.TileDimension).Any(layer.Contains)).ToArray();
        int[] outerTiles = AffectedTiles(ring).Except(innerTiles).ToArray();
        (string? Material, int Outer, int Changed) best = (null, int.MaxValue, int.MaxValue);
        foreach (string candidate in _catalog.MaterialIds)
        {
            if (StringComparer.OrdinalIgnoreCase.Equals(candidate, paintedMaterial)) continue;
            foreach (int index in ring) SetCorner(index, candidate);
            bool innerOk = innerTiles.All(tile => _catalog.ResolveNativeTile(TileCorners(tile % _map.TileDimension, tile / _map.TileDimension), 0, 0) is not null);
            if (innerOk)
            {
                int outer = outerTiles.Count(tile => _catalog.ResolveNativeTile(TileCorners(tile % _map.TileDimension, tile / _map.TileDimension), 0, 0) is null);
                int changed = ring.Count(index => !StringComparer.OrdinalIgnoreCase.Equals(original[index], candidate));
                if (outer < best.Outer || (outer == best.Outer && changed < best.Changed)) best = (candidate, outer, changed);
            }
            foreach ((int index, string material) in original) SetCorner(index, material);
        }
        if (best.Material is null) { ring = null; return false; }
        foreach (int index in ring) SetCorner(index, best.Material);
        touched.UnionWith(ring);
        return true;
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
