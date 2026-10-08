namespace AgainstRomeMapEditor;

/// <summary>
/// 高精度 32-bit 浮點數地形高度工作緩衝區。
/// 
/// 解決 8-bit 整數（0~255）在微量物理侵蝕（水力沉積/溶解、熱力重力崩塌）與幾何濾鏡中
/// 容易產生的量化截斷（Quantization Banding）與步進停滯問題。
/// 提供雙線性插值取樣、坡度梯度計算、曲率（Laplacian）與高效變更抽取。
/// </summary>
internal sealed class TerrainHeightWorkBuffer
{
    private readonly float[] _heights;

    public int Size { get; }
    public float[] RawData => _heights;

    public TerrainHeightWorkBuffer(int size)
    {
        if (size < 2) throw new ArgumentOutOfRangeException(nameof(size), "高度圖尺寸至少須為 2x2。");
        Size = size;
        _heights = new float[size * size];
    }

    public TerrainHeightWorkBuffer(int size, IReadOnlyList<byte> initialHeights) : this(size)
    {
        ArgumentNullException.ThrowIfNull(initialHeights);
        if (initialHeights.Count != size * size)
            throw new ArgumentException($"輸入的高度陣列長度 ({initialHeights.Count}) 與尺寸規格 ({size}x{size}={size * size}) 不符。", nameof(initialHeights));

        for (int i = 0; i < initialHeights.Count; i++)
        {
            _heights[i] = initialHeights[i];
        }
    }

    public TerrainHeightWorkBuffer(TerrainHeightWorkBuffer source)
    {
        ArgumentNullException.ThrowIfNull(source);
        Size = source.Size;
        _heights = source._heights.ToArray();
    }

    public float this[int index]
    {
        get => _heights[index];
        set => _heights[index] = value;
    }

    public float this[int x, int y]
    {
        get => _heights[y * Size + x];
        set => _heights[y * Size + x] = value;
    }

    public bool IsValidCoord(int x, int y) => x >= 0 && x < Size && y >= 0 && y < Size;

    public float GetClamped(int x, int y)
    {
        int cx = Math.Clamp(x, 0, Size - 1);
        int cy = Math.Clamp(y, 0, Size - 1);
        return _heights[cy * Size + cx];
    }

    /// <summary>
    /// 雙線性插值取樣（Bilinear Interpolation）。
    /// </summary>
    public float SampleBilinear(float x, float y)
    {
        x = Math.Clamp(x, 0f, Size - 1f);
        y = Math.Clamp(y, 0f, Size - 1f);

        int x0 = (int)MathF.Floor(x);
        int y0 = (int)MathF.Floor(y);
        int x1 = Math.Min(x0 + 1, Size - 1);
        int y1 = Math.Min(y0 + 1, Size - 1);

        float tx = x - x0;
        float ty = y - y0;

        float h00 = _heights[y0 * Size + x0];
        float h10 = _heights[y0 * Size + x1];
        float h01 = _heights[y1 * Size + x0];
        float h11 = _heights[y1 * Size + x1];

        float top = h00 + (h10 - h00) * tx;
        float bottom = h01 + (h11 - h01) * tx;
        return top + (bottom - top) * ty;
    }

    /// <summary>
    /// 計算浮點座標處的坡度梯度向量 (gx, gy)。
    /// 梯度方向指向地勢上升最陡處；沿梯度相反方向 (-gx, -gy) 即為水流下滑方向。
    /// </summary>
    public (float X, float Y) CalculateGradient(float x, float y)
    {
        x = Math.Clamp(x, 0f, Size - 1f);
        y = Math.Clamp(y, 0f, Size - 1f);

        int x0 = (int)MathF.Floor(x);
        int y0 = (int)MathF.Floor(y);
        int x1 = Math.Min(x0 + 1, Size - 1);
        int y1 = Math.Min(y0 + 1, Size - 1);

        float tx = x - x0;
        float ty = y - y0;

        float h00 = _heights[y0 * Size + x0];
        float h10 = _heights[y0 * Size + x1];
        float h01 = _heights[y1 * Size + x0];
        float h11 = _heights[y1 * Size + x1];

        float gx = (h10 - h00) * (1f - ty) + (h11 - h01) * ty;
        float gy = (h01 - h00) * (1f - tx) + (h11 - h10) * tx;
        return (gx, gy);
    }

    /// <summary>
    /// 離散拉普拉斯算子（Laplacian / 二階微分曲率）。
    /// Laplacian &lt; 0 代表局部凸起結構（山脊 / 頂峰）；
    /// Laplacian &gt; 0 代表局部凹陷結構（山谷 / 溝壑）。
    /// </summary>
    public float CalculateLaplacian(int x, int y)
    {
        float center = GetClamped(x, y);
        float left = GetClamped(x - 1, y);
        float right = GetClamped(x + 1, y);
        float top = GetClamped(x, y - 1);
        float bottom = GetClamped(x, y + 1);

        return left + right + top + bottom - 4f * center;
    }

    /// <summary>
    /// 將工作緩衝區中的浮點數高度量化為 0~255 位元組，並萃取所有發生實質變化的頂點記錄。
    /// </summary>
    public IReadOnlyList<TerrainSampleChange> ExtractChanges(IReadOnlyList<byte> baselineHeights)
    {
        ArgumentNullException.ThrowIfNull(baselineHeights);
        int length = Math.Min(_heights.Length, baselineHeights.Count);
        var changes = new List<TerrainSampleChange>();

        for (int i = 0; i < length; i++)
        {
            byte before = baselineHeights[i];
            byte after = (byte)Math.Clamp((int)MathF.Round(_heights[i]), 0, 255);
            if (before != after)
            {
                changes.Add(new TerrainSampleChange(i, before, after));
            }
        }

        return changes;
    }

    /// <summary>
    /// 拷貝當前緩衝至新執行個體。
    /// </summary>
    public TerrainHeightWorkBuffer Clone() => new(this);
}
