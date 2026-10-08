using System.Runtime.CompilerServices;

namespace AgainstRomeMapEditor.Rendering;

/// <summary>
/// MaxRects 啟發式裝箱規則。
/// </summary>
public enum MaxRectsHeuristic
{
    /// <summary>最佳短邊匹配 (Best Short Side Fit) - 優先選擇剩餘較短邊最小的空閒空間，適合一般精靈</summary>
    BestShortSideFit,

    /// <summary>最佳面積匹配 (Best Area Fit) - 優先選擇剩餘空閒面積最小的空間，提升緻密性</summary>
    BestAreaFit,

    /// <summary>最佳長邊匹配 (Best Long Side Fit) - 優先選擇剩餘較長邊最小的空間</summary>
    BestLongSideFit,
}

/// <summary>
/// 圖集內部矩形區域結構 (整數坐標)。
/// </summary>
public readonly record struct AtlasRect(int X, int Y, int Width, int Height)
{
    public int Right => X + Width;
    public int Bottom => Y + Height;
    public int Area => Width * Height;

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public bool Contains(in AtlasRect other) =>
        other.X >= X && other.Y >= Y &&
        other.Right <= Right && other.Bottom <= Bottom;

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public bool Intersects(in AtlasRect other) =>
        X < other.Right && Right > other.X &&
        Y < other.Bottom && Bottom > other.Y;
}

/// <summary>
/// 封裝已裝箱精靈的 UV 紋理映射區間。
/// </summary>
public readonly record struct AtlasUv(float U0, float V0, float U1, float V1);

/// <summary>
/// 針對 4096² 大圖集的高效二維裝箱演算法 (MaxRects) 與快取管線。
/// 支援靜態圖元、多影格動畫事務原子裝箱、1-pixel Gutter 防滲透、以及 R8 陰影遮罩快取最佳化。
/// </summary>
public sealed class DynamicAtlasPacker
{
    private readonly int _maxSize;
    private readonly int _gutter;
    private readonly List<AtlasRect> _freeRectangles = [];
    private readonly List<AtlasRect> _usedRectangles = [];

    // 事務快照點 (用於動畫序列失敗時原子回滾)
    private struct SavePoint
    {
        public AtlasRect[] FreeSnapshot;
        public int UsedCount;
    }

    private readonly Stack<SavePoint> _transactionStack = new();

    /// <summary>圖集最大邊長 (預設 4096)。</summary>
    public int MaxSize => _maxSize;

    /// <summary>像素保護留白 (預設 1 像素)。</summary>
    public int Gutter => _gutter;

    /// <summary>當前空閒矩形數量。</summary>
    public int FreeRectanglesCount => _freeRectangles.Count;

    /// <summary>已裝箱矩形數量。</summary>
    public int PlacedCount => _usedRectangles.Count;

    /// <summary>當前實際外接包圍邊界之最大寬度。</summary>
    public int UsedWidth { get; private set; }

    /// <summary>當前實際外接包圍邊界之最大高度。</summary>
    public int UsedHeight { get; private set; }

    public DynamicAtlasPacker(int maxSize = 4096, int gutter = 1)
    {
        ArgumentOutOfRangeException.ThrowIfLessThan(maxSize, 16);
        ArgumentOutOfRangeException.ThrowIfLessThan(gutter, 0);

        _maxSize = maxSize;
        _gutter = gutter;
        Reset();
    }

    /// <summary>重置圖集空間。</summary>
    public void Reset()
    {
        _freeRectangles.Clear();
        _usedRectangles.Clear();
        _transactionStack.Clear();
        _freeRectangles.Add(new AtlasRect(0, 0, _maxSize, _maxSize));
        UsedWidth = 0;
        UsedHeight = 0;
    }

    /// <summary>
    /// 計算當前圖集利用率百分比 (0.0% ~ 100.0%)。
    /// </summary>
    public double CalculateEfficiency(int actualTextureWidth, int actualTextureHeight)
    {
        if (actualTextureWidth <= 0 || actualTextureHeight <= 0) return 0.0;
        long placedArea = 0;
        foreach (var rect in _usedRectangles)
        {
            placedArea += (long)(rect.Width - _gutter * 2) * (rect.Height - _gutter * 2);
        }
        return (double)placedArea / ((long)actualTextureWidth * actualTextureHeight) * 100.0;
    }

    /// <summary>
    /// 嘗試裝入單一精靈。
    /// </summary>
    public bool TryInsert(int width, int height, out AtlasRect placedRect, out AtlasUv uv,
        MaxRectsHeuristic heuristic = MaxRectsHeuristic.BestShortSideFit)
    {
        placedRect = default;
        uv = default;

        int totalWidth = width + _gutter * 2;
        int totalHeight = height + _gutter * 2;

        if (totalWidth > _maxSize || totalHeight > _maxSize) return false;

        int bestIndex = -1;
        int bestScore1 = int.MaxValue;
        int bestScore2 = int.MaxValue;
        AtlasRect bestNode = default;

        for (int i = 0; i < _freeRectangles.Count; i++)
        {
            var free = _freeRectangles[i];
            if (free.Width < totalWidth || free.Height < totalHeight) continue;

            int score1, score2;
            switch (heuristic)
            {
                case MaxRectsHeuristic.BestShortSideFit:
                    int remainingX = free.Width - totalWidth;
                    int remainingY = free.Height - totalHeight;
                    score1 = Math.Min(remainingX, remainingY);
                    score2 = Math.Max(remainingX, remainingY);
                    break;

                case MaxRectsHeuristic.BestAreaFit:
                    score1 = free.Area - totalWidth * totalHeight;
                    score2 = Math.Min(free.Width - totalWidth, free.Height - totalHeight);
                    break;

                case MaxRectsHeuristic.BestLongSideFit:
                default:
                    score1 = Math.Max(free.Width - totalWidth, free.Height - totalHeight);
                    score2 = Math.Min(free.Width - totalWidth, free.Height - totalHeight);
                    break;
            }

            if (score1 < bestScore1 || (score1 == bestScore1 && score2 < bestScore2))
            {
                bestScore1 = score1;
                bestScore2 = score2;
                bestIndex = i;
                bestNode = new AtlasRect(free.X, free.Y, totalWidth, totalHeight);
            }
        }

        if (bestIndex == -1) return false;

        // 執行分割與修剪
        SplitFreeRectangles(bestNode);
        PruneFreeList();

        _usedRectangles.Add(bestNode);
        UsedWidth = Math.Max(UsedWidth, bestNode.Right);
        UsedHeight = Math.Max(UsedHeight, bestNode.Bottom);

        // 回傳扣除 Gutter 的像素空間
        placedRect = new AtlasRect(bestNode.X + _gutter, bestNode.Y + _gutter, width, height);

        // 預設以當前最大空間計算 UV (呼叫端可在壓縮圖集尺寸後重新換算)
        uv = new AtlasUv(
            (float)placedRect.X / _maxSize,
            (float)placedRect.Y / _maxSize,
            (float)placedRect.Right / _maxSize,
            (float)placedRect.Bottom / _maxSize);

        return true;
    }

    /// <summary>
    /// 開始一個原子裝箱事務（通常用於整套動畫影格或連鎖素材）。
    /// </summary>
    public void BeginTransaction()
    {
        _transactionStack.Push(new SavePoint
        {
            FreeSnapshot = _freeRectangles.ToArray(),
            UsedCount = _usedRectangles.Count
        });
    }

    /// <summary>
    /// 提交當前事務。
    /// </summary>
    public void CommitTransaction()
    {
        if (_transactionStack.Count > 0)
        {
            _transactionStack.Pop();
        }
    }

    /// <summary>
    /// 回滾當前事務，完全恢復之前的空閒空間與已裝箱狀態。
    /// </summary>
    public void RollbackTransaction()
    {
        if (_transactionStack.Count == 0) return;

        var save = _transactionStack.Pop();
        _freeRectangles.Clear();
        _freeRectangles.AddRange(save.FreeSnapshot);

        if (_usedRectangles.Count > save.UsedCount)
        {
            _usedRectangles.RemoveRange(save.UsedCount, _usedRectangles.Count - save.UsedCount);
        }

        // 重新評估 UsedWidth 與 UsedHeight
        int w = 0, h = 0;
        foreach (var r in _usedRectangles)
        {
            w = Math.Max(w, r.Right);
            h = Math.Max(h, r.Bottom);
        }
        UsedWidth = w;
        UsedHeight = h;
    }

    /// <summary>
    /// 嘗試整組原子裝箱（例如 24 格步兵動畫）。若有任何一格放不下，整套自動復原。
    /// </summary>
    public bool TryInsertBatch(IReadOnlyList<(int Width, int Height)> frames,
        List<AtlasRect> outPlacedRects, List<AtlasUv> outUvs)
    {
        BeginTransaction();
        outPlacedRects.Clear();
        outUvs.Clear();

        for (int i = 0; i < frames.Count; i++)
        {
            if (!TryInsert(frames[i].Width, frames[i].Height, out var rect, out var uv))
            {
                RollbackTransaction();
                outPlacedRects.Clear();
                outUvs.Clear();
                return false;
            }
            outPlacedRects.Add(rect);
            outUvs.Add(uv);
        }

        CommitTransaction();
        return true;
    }

    /// <summary>
    /// 計算下一個 2 的冪次方整數（用於壓縮輸出貼圖尺寸）。
    /// </summary>
    public static int NextPowerOfTwo(int value)
    {
        if (value <= 1) return 1;
        int result = 1;
        while (result < value) result <<= 1;
        return result;
    }

    /// <summary>
    /// 將所有與放置節點重疊的空閒矩形分割為至多 4 個極大子矩形。
    /// </summary>
    private void SplitFreeRectangles(in AtlasRect placed)
    {
        for (int i = 0; i < _freeRectangles.Count; i++)
        {
            var free = _freeRectangles[i];
            if (!free.Intersects(placed)) continue;

            // 移除被相交的原始矩形
            _freeRectangles.RemoveAt(i);
            i--;

            // 1. 上側子矩形
            if (placed.Y > free.Y && placed.Y < free.Bottom)
            {
                _freeRectangles.Add(new AtlasRect(free.X, free.Y, free.Width, placed.Y - free.Y));
            }

            // 2. 下側子矩形
            if (placed.Bottom < free.Bottom)
            {
                _freeRectangles.Add(new AtlasRect(free.X, placed.Bottom, free.Width, free.Bottom - placed.Bottom));
            }

            // 3. 左側子矩形
            if (placed.X > free.X && placed.X < free.Right)
            {
                _freeRectangles.Add(new AtlasRect(free.X, free.Y, placed.X - free.X, free.Height));
            }

            // 4. 右側子矩形
            if (placed.Right < free.Right)
            {
                _freeRectangles.Add(new AtlasRect(placed.Right, free.Y, free.Right - placed.Right, free.Height));
            }
        }
    }

    /// <summary>
    /// 修剪被其他空閒矩形完全包含的非極大矩形 (Non-Maximal Elimination)。
    /// </summary>
    private void PruneFreeList()
    {
        for (int i = 0; i < _freeRectangles.Count; i++)
        {
            for (int j = i + 1; j < _freeRectangles.Count; j++)
            {
                if (_freeRectangles[j].Contains(_freeRectangles[i]))
                {
                    _freeRectangles.RemoveAt(i);
                    i--;
                    break;
                }
                if (_freeRectangles[i].Contains(_freeRectangles[j]))
                {
                    _freeRectangles.RemoveAt(j);
                    j--;
                }
            }
        }
    }
}
