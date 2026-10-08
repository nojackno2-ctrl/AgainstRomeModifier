namespace AgainstRomeMapEditor.Modules.Pathfinding;

/// <summary>
/// 封裝 Against Rome 地圖通行性網格的查詢模型（結合 collision.bmp 與水位高度判定）。
/// </summary>
public sealed class NavMeshPassabilityGrid
{
    private readonly byte[]? _collision;
    private readonly byte[]? _heights;
    private readonly int _heightSize;
    private readonly float _heightStep;
    private readonly float _waterLevel;

    public int Size { get; }
    public float WorldDimension { get; }
    public float TileScale { get; }

    public NavMeshPassabilityGrid(
        int size,
        IReadOnlyList<byte>? collision,
        int heightSize = 0,
        IReadOnlyList<byte>? heights = null,
        float heightStep = 4.0f,
        float waterLevel = 0.0f,
        float worldDimension = 16384f)
    {
        if (size <= 0) throw new ArgumentOutOfRangeException(nameof(size), "網格尺寸必須大於 0。");
        Size = size;
        WorldDimension = worldDimension;
        TileScale = worldDimension / size;

        if (collision is not null && collision.Count == size * size)
        {
            _collision = collision.ToArray();
        }

        if (heights is not null && heightSize > 1 && heights.Count == heightSize * heightSize && float.IsFinite(heightStep) && heightStep > 0)
        {
            _heightSize = heightSize;
            _heights = heights.ToArray();
            _heightStep = heightStep;
            _waterLevel = float.IsFinite(waterLevel) ? waterLevel : 0.0f;
        }
    }

    public bool IsInBounds(int x, int z) => x >= 0 && z >= 0 && x < Size && z < Size;

    /// <summary>
    /// 檢查指定座標是否為可通行的陸地（無碰撞阻擋且未沒入水中）。
    /// </summary>
    public bool IsPassable(int x, int z)
    {
        if (!IsInBounds(x, z)) return false;
        if (_collision is not null && _collision[z * Size + x] != 0) return false;
        if (IsSubmerged(x, z)) return false;
        return true;
    }

    /// <summary>
    /// 檢查指定座標是否具有碰撞阻擋硬閘（collision != 0）。
    /// </summary>
    public bool IsBlockedByCollision(int x, int z)
    {
        if (!IsInBounds(x, z)) return true;
        return _collision is not null && _collision[z * Size + x] != 0;
    }

    /// <summary>
    /// 檢查指定座標是否沒入水下。
    /// </summary>
    public bool IsSubmerged(int x, int z)
    {
        if (!IsInBounds(x, z) || _heights is null || _heightSize <= 1) return false;
        int hx = Math.Clamp((int)((x + 0.5f) * (_heightSize - 1) / Size), 0, _heightSize - 1);
        int hz = Math.Clamp((int)((z + 0.5f) * (_heightSize - 1) / Size), 0, _heightSize - 1);
        return _heights[hz * _heightSize + hx] * _heightStep < _waterLevel;
    }

    /// <summary>
    /// 取得指定座標的碰撞位元組（若無碰撞層則回傳 0）。
    /// </summary>
    public byte GetCollisionByte(int x, int z)
    {
        if (!IsInBounds(x, z) || _collision is null) return 0;
        return _collision[z * Size + x];
    }

    public NavMeshCoordinate ToCoordinate(int x, int z) =>
        NavMeshCoordinate.FromTile(x, z, Size, WorldDimension);

    public (int X, int Z) WorldToTile(float worldX, float worldZ)
    {
        int x = Math.Clamp((int)(worldX / TileScale), 0, Size - 1);
        int z = Math.Clamp((int)(worldZ / TileScale), 0, Size - 1);
        return (x, z);
    }
}
