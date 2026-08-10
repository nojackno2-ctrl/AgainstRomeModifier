namespace AgainstRomeMapEditor;

internal sealed record NativeTerrainBakeIssue(int TileX, int TileY, IReadOnlyList<string> CornerMaterialIds);

internal sealed record NativeTerrainCornerConflict(int CornerX, int CornerY, IReadOnlyDictionary<string, int> MaterialVotes);

internal sealed record NativeTerrainImportResult(
    TerrainBlendAuthoringMap Map,
    IReadOnlyList<int> UnresolvedTileIndices,
    IReadOnlyList<NativeTerrainCornerConflict> CornerConflicts);

internal sealed record NativeTerrainBakeResult(IReadOnlyList<string?> Textures, IReadOnlyList<NativeTerrainBakeIssue> Issues)
{
    public bool IsComplete => Issues.Count == 0;
}

/// <summary>
/// Authoring grid for original-quality terrain transitions. Materials live on tile corners instead of
/// tile centers so one native tile can represent base, two-material 4U, or three-material 4T content.
/// </summary>
internal sealed class TerrainBlendAuthoringMap
{
    private readonly string[] _cornerMaterials;

    public TerrainBlendAuthoringMap(int tileDimension, string initialMaterialId)
    {
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(tileDimension);
        if (string.IsNullOrWhiteSpace(initialMaterialId)) throw new ArgumentException("初始地表材質不可為空。", nameof(initialMaterialId));
        TileDimension = tileDimension;
        CornerDimension = tileDimension + 1;
        _cornerMaterials = Enumerable.Repeat(initialMaterialId, CornerDimension * CornerDimension).ToArray();
    }

    public int TileDimension { get; }
    public int CornerDimension { get; }
    public IReadOnlyList<string> CornerMaterials => _cornerMaterials;

    public static NativeTerrainImportResult Import(int tileDimension, IReadOnlyList<string> textures, FloorMaterialCatalog catalog, string fallbackMaterialId)
    {
        if (tileDimension <= 0 || textures.Count != tileDimension * tileDimension) throw new ArgumentException("Terrain texture dimensions do not match.", nameof(textures));
        ArgumentNullException.ThrowIfNull(catalog);
        var map = new TerrainBlendAuthoringMap(tileDimension, fallbackMaterialId);
        var votes = Enumerable.Range(0, map.CornerDimension * map.CornerDimension)
            .Select(_ => new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase)).ToArray();
        var unresolved = new List<int>();
        for (int y = 0; y < tileDimension; y++)
        {
            for (int x = 0; x < tileDimension; x++)
            {
                int tileIndex = y * tileDimension + x;
                if (!catalog.TryResolveNativeCorners(textures[tileIndex], out IReadOnlyList<string> corners) || corners.Count != 4)
                {
                    unresolved.Add(tileIndex);
                    continue;
                }
                AddVote(x, y, corners[0]);
                AddVote(x + 1, y, corners[1]);
                AddVote(x + 1, y + 1, corners[2]);
                AddVote(x, y + 1, corners[3]);
            }
        }

        var conflicts = new List<NativeTerrainCornerConflict>();
        for (int y = 0; y < map.CornerDimension; y++)
        {
            for (int x = 0; x < map.CornerDimension; x++)
            {
                Dictionary<string, int> cornerVotes = votes[y * map.CornerDimension + x];
                if (cornerVotes.Count == 0) continue;
                string selected = cornerVotes.OrderByDescending(item => item.Value).ThenBy(item => item.Key, StringComparer.OrdinalIgnoreCase).First().Key;
                map.SetCorner(x, y, selected);
                if (cornerVotes.Count > 1) conflicts.Add(new NativeTerrainCornerConflict(x, y, new Dictionary<string, int>(cornerVotes, StringComparer.OrdinalIgnoreCase)));
            }
        }
        return new NativeTerrainImportResult(map, unresolved, conflicts);

        void AddVote(int cornerX, int cornerY, string materialId)
        {
            Dictionary<string, int> cornerVotes = votes[cornerY * map.CornerDimension + cornerX];
            cornerVotes[materialId] = cornerVotes.GetValueOrDefault(materialId) + 1;
        }
    }

    public string GetCorner(int x, int y) => _cornerMaterials[CornerIndex(x, y)];

    public void SetCorner(int x, int y, string materialId)
    {
        if (string.IsNullOrWhiteSpace(materialId)) throw new ArgumentException("地表材質不可為空。", nameof(materialId));
        _cornerMaterials[CornerIndex(x, y)] = materialId;
    }

    public IReadOnlyList<int> PaintCircle(float centerX, float centerY, float radius, string materialId)
    {
        if (!float.IsFinite(centerX) || !float.IsFinite(centerY)) throw new ArgumentOutOfRangeException(nameof(centerX));
        if (!float.IsFinite(radius) || radius < 0) throw new ArgumentOutOfRangeException(nameof(radius));
        if (string.IsNullOrWhiteSpace(materialId)) throw new ArgumentException("地表材質不可為空。", nameof(materialId));
        float radiusSquared = radius * radius;
        int minX = Math.Max(0, (int)MathF.Floor(centerX - radius));
        int maxX = Math.Min(CornerDimension - 1, (int)MathF.Ceiling(centerX + radius));
        int minY = Math.Max(0, (int)MathF.Floor(centerY - radius));
        int maxY = Math.Min(CornerDimension - 1, (int)MathF.Ceiling(centerY + radius));
        var changed = new List<int>();
        for (int y = minY; y <= maxY; y++)
        {
            for (int x = minX; x <= maxX; x++)
            {
                if ((x - centerX) * (x - centerX) + (y - centerY) * (y - centerY) > radiusSquared) continue;
                int index = y * CornerDimension + x;
                if (StringComparer.OrdinalIgnoreCase.Equals(_cornerMaterials[index], materialId)) continue;
                _cornerMaterials[index] = materialId;
                changed.Add(index);
            }
        }
        return changed;
    }

    public NativeTerrainBakeResult Bake(FloorMaterialCatalog catalog)
    {
        ArgumentNullException.ThrowIfNull(catalog);
        var textures = new string?[TileDimension * TileDimension];
        var issues = new List<NativeTerrainBakeIssue>();
        for (int y = 0; y < TileDimension; y++)
        {
            for (int x = 0; x < TileDimension; x++)
            {
                string[] corners = [GetCorner(x, y), GetCorner(x + 1, y), GetCorner(x + 1, y + 1), GetCorner(x, y + 1)];
                string? texture = catalog.ResolveNativeTile(corners, x, y);
                textures[y * TileDimension + x] = texture;
                if (texture is null) issues.Add(new NativeTerrainBakeIssue(x, y, corners));
            }
        }
        return new NativeTerrainBakeResult(textures, issues);
    }

    private int CornerIndex(int x, int y)
    {
        if (x < 0 || y < 0 || x >= CornerDimension || y >= CornerDimension) throw new ArgumentOutOfRangeException(nameof(x));
        return y * CornerDimension + x;
    }
}
