namespace AgainstRomeMapEditor;

/// <summary>地形高度筆刷的操作。</summary>
internal enum TerrainHeightOperation { Raise, Lower, Smooth, Flatten, Roughen }

/// <summary>通行區域筆刷：阻擋（collision 255）或清除（collision 0）。</summary>
internal enum TerrainCollisionOperation { Block, Clear }

/// <summary>單一頂點／像素的前後值，供復原、重做與視圖局部更新使用。</summary>
internal readonly record struct TerrainSampleChange(int Index, byte Before, byte After);

internal sealed record TerrainLayerStroke(IReadOnlyList<TerrainSampleChange> Heights, IReadOnlyList<TerrainSampleChange> Collision);

/// <summary>
/// 地形高度與通行區域的純狀態（無 WinForms/GL 相依），供 2D／3D 共用並可在 CI 回歸測試。
///
/// 逆向依據（Against_Rome.exe 載入管線 LDFLOORHMAP／LDFLOORCMAP／LDFLOOREMBMAP）：
/// - boden.bmp 為 (64·Heightmapstep+1)² 頂點高度圖，遊戲只讀綠通道，世界高度 = G × Heightmapstep。
/// - emboss.bmp 為同尺寸的每頂點光照亮度（綠通道 0–255），缺檔時遊戲預設 255。
/// - collision.bmp 為 256² tile-pixel 圖，非黑像素取 (R+G+B)/3；0 = 無碰撞，255 = 阻擋。
/// - skydens/visible/cliprect/shadows.dat 以「全部高度總和」為標頭鍵，不符或缺檔時遊戲自動重算並寫回。
/// </summary>
internal sealed class TerrainHeightEditSession
{
    private readonly byte[] _heights;
    private byte[] _baselineHeights;
    private readonly byte[]? _baselineEmboss;
    private byte[]? _savedEmboss;
    private readonly byte[]? _collision;
    private byte[]? _baselineCollision;
    private readonly Stack<TerrainLayerStroke> _undo = new();
    private readonly Stack<TerrainLayerStroke> _redo = new();
    private readonly Dictionary<int, TerrainSampleChange> _pendingHeights = new();
    private readonly Dictionary<int, TerrainSampleChange> _pendingCollision = new();
    private EmbossLightModel _light;
    private int? _flatEmbossBase;

    public TerrainHeightEditSession(int vertexSize, byte[] heights, byte[]? emboss, int collisionSize, byte[]? collision)
    {
        if (vertexSize < 2 || heights.Length != vertexSize * vertexSize) throw new ArgumentException("高度圖尺寸無效。", nameof(heights));
        if (emboss is not null && emboss.Length != heights.Length) throw new ArgumentException("emboss.bmp 尺寸必須與高度圖相同。", nameof(emboss));
        if (collision is not null && (collisionSize < 1 || collision.Length != collisionSize * collisionSize))
            throw new ArgumentException("collision.bmp 尺寸無效。", nameof(collision));
        VertexSize = vertexSize;
        CollisionSize = collisionSize;
        _heights = heights.ToArray();
        _baselineHeights = heights.ToArray();
        _baselineEmboss = emboss?.ToArray();
        _savedEmboss = emboss?.ToArray();
        _collision = collision?.ToArray();
        _baselineCollision = collision?.ToArray();
        _light = _baselineEmboss is null ? EmbossLightModel.None : EmbossLightModel.Fit(vertexSize, _baselineHeights, _baselineEmboss);
    }

    public int VertexSize { get; }
    public int CollisionSize { get; }
    public IReadOnlyList<byte> Heights => _heights;
    public IReadOnlyList<byte>? Collision => _collision;
    public bool HasCollision => _collision is not null;
    public bool HasEmboss => _savedEmboss is not null;
    public bool HeightsDirty => !_heights.AsSpan().SequenceEqual(_baselineHeights);
    public bool CollisionDirty => _collision is not null && !_collision.AsSpan().SequenceEqual(_baselineCollision);
    public bool IsDirty => HeightsDirty || CollisionDirty || EmbossRelightPending;
    public bool CanUndo => _undo.Count > 0 || _pendingHeights.Count > 0 || _pendingCollision.Count > 0;
    public bool CanRedo => _redo.Count > 0;
    /// <summary>光照擬合的決定係數；供診斷顯示，0 表示無法由原圖推得光照（emboss 為常數或缺檔）。</summary>
    public double LightFitQuality => _light.RSquared;

    /// <summary>以圓形、平滑衰減的筆刷修改頂點高度（單位：boden.bmp 綠通道 0–255）。</summary>
    public IReadOnlyList<TerrainSampleChange> PaintHeight(float centerX, float centerY, float radius, TerrainHeightOperation operation, int strength, int flattenTarget = -1, int roughnessSeed = 0)
    {
        if (!float.IsFinite(centerX) || !float.IsFinite(centerY) || !float.IsFinite(radius) || radius <= 0) throw new ArgumentOutOfRangeException(nameof(radius));
        strength = Math.Clamp(strength, 1, 64);
        int size = VertexSize;
        int minX = Math.Max(0, (int)MathF.Floor(centerX - radius)), maxX = Math.Min(size - 1, (int)MathF.Ceiling(centerX + radius));
        int minY = Math.Max(0, (int)MathF.Floor(centerY - radius)), maxY = Math.Min(size - 1, (int)MathF.Ceiling(centerY + radius));
        if (operation == TerrainHeightOperation.Flatten && flattenTarget < 0)
            flattenTarget = _heights[Math.Clamp((int)MathF.Round(centerY), 0, size - 1) * size + Math.Clamp((int)MathF.Round(centerX), 0, size - 1)];
        flattenTarget = Math.Clamp(flattenTarget, 0, 255);
        byte[]? source = operation == TerrainHeightOperation.Smooth ? _heights.ToArray() : null;
        var changes = new List<TerrainSampleChange>();
        for (int y = minY; y <= maxY; y++)
        for (int x = minX; x <= maxX; x++)
        {
            float distance = MathF.Sqrt((x - centerX) * (x - centerX) + (y - centerY) * (y - centerY));
            if (distance > radius) continue;
            float falloff = .5f + .5f * MathF.Cos(MathF.PI * distance / radius); // 中心 1、邊緣 0，避免筆刷邊界出現台階。
            int index = y * size + x;
            byte before = _heights[index];
            float value = operation switch
            {
                TerrainHeightOperation.Raise => before + strength * falloff,
                TerrainHeightOperation.Lower => before - strength * falloff,
                TerrainHeightOperation.Flatten => before + (flattenTarget - before) * Math.Min(1f, strength / 16f) * falloff,
                TerrainHeightOperation.Roughen => before + strength * falloff * RoughnessNoise(x, y, roughnessSeed),
                _ => before + (Average3x3(source!, x, y) - before) * Math.Min(1f, strength / 8f) * falloff,
            };
            byte after = (byte)Math.Clamp((int)MathF.Round(value), 0, 255);
            if (after == before) continue;
            _heights[index] = after;
            Track(_pendingHeights, index, before, after);
            changes.Add(new TerrainSampleChange(index, before, after));
        }
        if (changes.Count > 0) _redo.Clear();
        return changes;
    }

    /// <summary>
    /// 粗糙化用的平滑值雜訊，範圍約 [-1, 1]：在每 <see cref="RoughnessCell"/> 個頂點的格點上取雜湊值，再以 smoothstep 雙線性內插，
    /// 產生數個頂點寬的自然起伏而非逐點尖刺。同一 (x, y, seed) 結果固定，同一筆畫重複塗抹會沿同一方向加深起伏。
    /// </summary>
    internal static float RoughnessNoise(int x, int y, int seed)
    {
        float fx = x / (float)RoughnessCell, fy = y / (float)RoughnessCell;
        int x0 = (int)MathF.Floor(fx), y0 = (int)MathF.Floor(fy);
        float tx = Smooth(fx - x0), ty = Smooth(fy - y0);
        float top = Lerp(Lattice(x0, y0, seed), Lattice(x0 + 1, y0, seed), tx);
        float bottom = Lerp(Lattice(x0, y0 + 1, seed), Lattice(x0 + 1, y0 + 1, seed), tx);
        return Lerp(top, bottom, ty);

        static float Smooth(float t) => t * t * (3 - 2 * t);
        static float Lerp(float a, float b, float t) => a + (b - a) * t;
        static float Lattice(int lx, int ly, int s)
        {
            uint h = unchecked((uint)(lx * 374761393 + ly * 668265263 + s * 1442695041));
            h = (h ^ (h >> 13)) * 1274126177u;
            h ^= h >> 16;
            return (h & 0xFFFF) / 32767.5f - 1f;
        }
    }

    internal const int RoughnessCell = 4;

    /// <summary>
    /// 對整張高度圖套用逐頂點轉換（AI 製圖等批次編輯用）；變更與筆刷相同地記入待提交筆畫，呼叫端再 CommitStroke 成為單一復原步驟。
    /// </summary>
    public IReadOnlyList<TerrainSampleChange> TransformHeights(Func<int, int, byte, float> transform)
    {
        ArgumentNullException.ThrowIfNull(transform);
        int size = VertexSize;
        var changes = new List<TerrainSampleChange>();
        for (int y = 0; y < size; y++)
        for (int x = 0; x < size; x++)
        {
            int index = y * size + x;
            byte before = _heights[index];
            float value = transform(x, y, before);
            if (!float.IsFinite(value)) continue;
            byte after = (byte)Math.Clamp((int)MathF.Round(value), 0, 255);
            if (after == before) continue;
            _heights[index] = after;
            Track(_pendingHeights, index, before, after);
            changes.Add(new TerrainSampleChange(index, before, after));
        }
        if (changes.Count > 0) _redo.Clear();
        return changes;
    }

    /// <summary>以 tile 為中心、tile 半徑的圓形範圍設定 collision tile-pixel。</summary>
    public IReadOnlyList<TerrainSampleChange> PaintCollision(float centerX, float centerY, float radius, TerrainCollisionOperation operation, Func<int, int, bool>? allowsPixel = null)
    {
        if (_collision is null) return Array.Empty<TerrainSampleChange>();
        if (!float.IsFinite(centerX) || !float.IsFinite(centerY) || !float.IsFinite(radius) || radius <= 0) throw new ArgumentOutOfRangeException(nameof(radius));
        byte target = operation == TerrainCollisionOperation.Block ? (byte)255 : (byte)0;
        int size = CollisionSize;
        var changes = new List<TerrainSampleChange>();
        for (int y = Math.Max(0, (int)MathF.Floor(centerY - radius)); y <= Math.Min(size - 1, (int)MathF.Ceiling(centerY + radius)); y++)
        for (int x = Math.Max(0, (int)MathF.Floor(centerX - radius)); x <= Math.Min(size - 1, (int)MathF.Ceiling(centerX + radius)); x++)
        {
            float dx = x + .5f - centerX, dy = y + .5f - centerY;
            if (dx * dx + dy * dy > radius * radius) continue;
            if (allowsPixel is not null && !allowsPixel(x, y)) continue;
            int index = y * size + x;
            byte before = _collision[index];
            if (before == target) continue;
            _collision[index] = target;
            Track(_pendingCollision, index, before, target);
            changes.Add(new TerrainSampleChange(index, before, target));
        }
        if (changes.Count > 0) _redo.Clear();
        return changes;
    }

    /// <summary>放棄本次尚未提交的待提交筆畫（包含頂點高度與通行區域變更），恢復至筆畫開始前狀態。</summary>
    public IReadOnlyList<TerrainSampleChange> CancelStroke()
    {
        if (_pendingHeights.Count == 0 && _pendingCollision.Count == 0) return Array.Empty<TerrainSampleChange>();
        var rollback = new List<TerrainSampleChange>();
        foreach (var change in _pendingHeights.Values)
        {
            _heights[change.Index] = change.Before;
            rollback.Add(change);
        }
        foreach (var change in _pendingCollision.Values)
        {
            if (_collision is not null) _collision[change.Index] = change.Before;
        }
        _pendingHeights.Clear();
        _pendingCollision.Clear();
        return rollback;
    }

    /// <summary>套用特定頂點之高度微調（如河流河床下挖），納入待提交筆畫。</summary>
    public IReadOnlyList<TerrainSampleChange> ApplyHeightAdjustments(IReadOnlyCollection<(int Index, byte TargetHeight)> targets)
    {
        ArgumentNullException.ThrowIfNull(targets);
        var changes = new List<TerrainSampleChange>();
        foreach (var (index, target) in targets)
        {
            if (index < 0 || index >= _heights.Length) continue;
            byte before = _heights[index];
            if (before == target) continue;
            _heights[index] = target;
            Track(_pendingHeights, index, before, target);
            changes.Add(new TerrainSampleChange(index, before, target));
        }
        if (changes.Count > 0) _redo.Clear();
        return changes;
    }

    /// <summary>套用批次頂點高度樣本變更，納入待提交筆畫。</summary>
    public IReadOnlyList<TerrainSampleChange> ApplySampleChanges(IReadOnlyCollection<TerrainSampleChange> sampleChanges)
    {
        ArgumentNullException.ThrowIfNull(sampleChanges);
        var applied = new List<TerrainSampleChange>();
        foreach (var change in sampleChanges)
        {
            if (change.Index < 0 || change.Index >= _heights.Length) continue;
            byte before = _heights[change.Index];
            if (before == change.After) continue;
            _heights[change.Index] = change.After;
            Track(_pendingHeights, change.Index, before, change.After);
            applied.Add(new TerrainSampleChange(change.Index, before, change.After));
        }
        if (applied.Count > 0) _redo.Clear();
        return applied;
    }

    public bool CommitStroke()
    {
        if (_pendingHeights.Count == 0 && _pendingCollision.Count == 0) return false;
        _undo.Push(new TerrainLayerStroke(_pendingHeights.Values.ToArray(), _pendingCollision.Values.ToArray()));
        _pendingHeights.Clear(); _pendingCollision.Clear();
        return true;
    }

    public TerrainLayerStroke? Undo()
    {
        CommitStroke();
        if (!_undo.TryPop(out TerrainLayerStroke? stroke)) return null;
        foreach (TerrainSampleChange change in stroke.Heights) _heights[change.Index] = change.Before;
        foreach (TerrainSampleChange change in stroke.Collision) _collision![change.Index] = change.Before;
        _redo.Push(stroke);
        return Invert(stroke);
    }

    public TerrainLayerStroke? Redo()
    {
        if (!_redo.TryPop(out TerrainLayerStroke? stroke)) return null;
        foreach (TerrainSampleChange change in stroke.Heights) _heights[change.Index] = change.After;
        foreach (TerrainSampleChange change in stroke.Collision) _collision![change.Index] = change.After;
        _undo.Push(stroke);
        return stroke;
    }

    /// <summary>放棄所有未儲存的高度與通行變更，回到上次儲存（或開啟）時的狀態。</summary>
    public void ResetToBaseline()
    {
        _flatEmbossBase = null;
        _baselineHeights.CopyTo(_heights, 0);
        _baselineCollision?.CopyTo(_collision!, 0);
        _undo.Clear(); _redo.Clear(); _pendingHeights.Clear(); _pendingCollision.Clear();
    }

    /// <summary>
    /// 依目前高度產生要寫回的 emboss 光照。只在坡度改變處以「原值 + 擬合係數 × 坡度變化」調整，
    /// 未修改的地形保留原版烘焙的光影（含投影與手工修飾）不變。
    /// </summary>
    public byte[]? BuildEmboss()
    {
        if (_savedEmboss is null) return null;
        int size = VertexSize;
        if (_flatEmbossBase is int flat)
        {
            // 空白地形：原版烘焙的光影（含投影）屬於舊地勢，改以「平地亮度 + 擬合係數 × 目前坡度」重新產生整張光照。
            var lit = new byte[_savedEmboss.Length];
            for (int y = 0; y < size; y++)
            for (int x = 0; x < size; x++)
            {
                (float gx, float gy) = Gradient(_heights, size, x, y);
                float value = _light.IsUsable ? flat + _light.SlopeX * gx + _light.SlopeY * gy : flat;
                lit[y * size + x] = (byte)Math.Clamp((int)MathF.Round(value), 0, 255);
            }
            return lit;
        }
        byte[] result = _savedEmboss.ToArray();
        if (!HeightsDirty || !_light.IsUsable) return result;
        for (int y = 0; y < size; y++)
        for (int x = 0; x < size; x++)
        {
            (float gx, float gy) = Gradient(_heights, size, x, y);
            (float bx, float by) = Gradient(_baselineHeights, size, x, y);
            if (gx == bx && gy == by) continue;
            int index = y * size + x;
            result[index] = (byte)Math.Clamp((int)MathF.Round(_savedEmboss[index] + _light.SlopeX * (gx - bx) + _light.SlopeY * (gy - by)), 0, 255);
        }
        return result;
    }

    /// <summary>儲存成功後把目前狀態設為新的基準；光照模型仍沿用開圖時由原版資料擬合的係數。</summary>
    public void CommitBaseline(byte[]? savedEmboss)
    {
        CommitStroke();
        _baselineHeights = _heights.ToArray();
        _baselineCollision = _collision?.ToArray();
        if (savedEmboss is not null) _savedEmboss = savedEmboss.ToArray();
        _flatEmbossBase = null;
        _undo.Clear(); _redo.Clear();
    }

    /// <summary>是否在下次儲存時整張重新產生光照（空白地形）。</summary>
    public bool EmbossRelightPending => _flatEmbossBase is not null;

    /// <summary>
    /// 套用空白地形：整張高度設為 <paramref name="height"/>、清除所有阻擋，並讓下次儲存整張重新產生光照。
    /// 變更記入待提交筆畫（呼叫端 CommitStroke 後成為單一復原步驟；復原不會撤銷光照重算旗標，直到 ResetToBaseline）。
    /// </summary>
    public void ApplyBlankTerrain(byte height)
    {
        TransformHeights((_, _, _) => height);
        if (_collision is not null) PaintCollision(CollisionSize / 2f, CollisionSize / 2f, CollisionSize, TerrainCollisionOperation.Clear);
        // 平地亮度：擬合截距（坡度為 0 時的光照）；無法擬合時用原圖平均亮度，再退回遊戲缺檔預設 255。
        _flatEmbossBase = _light.IsUsable ? Math.Clamp((int)MathF.Round(_light.Intercept), 0, 255)
            : _savedEmboss is { Length: > 0 } emboss ? (int)Math.Round(emboss.Average(value => (double)value)) : 255;
    }

    private static void Track(Dictionary<int, TerrainSampleChange> pending, int index, byte before, byte after)
    {
        if (pending.TryGetValue(index, out TerrainSampleChange existing))
        {
            if (existing.Before == after) pending.Remove(index);
            else pending[index] = existing with { After = after };
        }
        else pending[index] = new TerrainSampleChange(index, before, after);
    }

    private static TerrainLayerStroke Invert(TerrainLayerStroke stroke) => new(
        stroke.Heights.Select(change => new TerrainSampleChange(change.Index, change.After, change.Before)).ToArray(),
        stroke.Collision.Select(change => new TerrainSampleChange(change.Index, change.After, change.Before)).ToArray());

    private float Average3x3(byte[] source, int x, int y)
    {
        int size = VertexSize, sum = 0, count = 0;
        for (int dy = -1; dy <= 1; dy++)
        for (int dx = -1; dx <= 1; dx++)
        {
            int sx = x + dx, sy = y + dy;
            if (sx < 0 || sy < 0 || sx >= size || sy >= size) continue;
            sum += source[sy * size + sx]; count++;
        }
        return sum / (float)count;
    }

    internal static (float X, float Y) Gradient(byte[] heights, int size, int x, int y)
    {
        int left = Math.Max(0, x - 1), right = Math.Min(size - 1, x + 1), top = Math.Max(0, y - 1), bottom = Math.Min(size - 1, y + 1);
        return (heights[y * size + right] - heights[y * size + left], heights[bottom * size + x] - heights[top * size + x]);
    }
}

/// <summary>
/// 由同一張地圖的原版高度與 emboss 光照，以最小平方法擬合「光照 ≈ c0 + cx·∂h/∂x + cy·∂h/∂y」。
/// 光源方向因地圖而異，所以每次開圖都用該圖本身的資料校準，不寫死任何方向。
/// </summary>
internal readonly record struct EmbossLightModel(float Intercept, float SlopeX, float SlopeY, double RSquared)
{
    public static EmbossLightModel None => new(255, 0, 0, 0);
    public bool IsUsable => RSquared > 0;

    public static EmbossLightModel Fit(int size, byte[] heights, byte[] emboss)
    {
        // 正規方程 3×3：所有內部頂點等權納入（平地決定截距、坡地決定斜率係數）。
        double s00 = 0, s01 = 0, s02 = 0, s11 = 0, s12 = 0, s22 = 0, t0 = 0, t1 = 0, t2 = 0, sumE = 0, sumE2 = 0;
        int count = 0;
        for (int y = 1; y < size - 1; y++)
        for (int x = 1; x < size - 1; x++)
        {
            (float gx, float gy) = TerrainHeightEditSession.Gradient(heights, size, x, y);
            double e = emboss[y * size + x];
            s00 += 1; s01 += gx; s02 += gy; s11 += gx * gx; s12 += gx * gy; s22 += gy * gy;
            t0 += e; t1 += gx * e; t2 += gy * e; sumE += e; sumE2 += e * e; count++;
        }
        if (count < 16) return None;
        double[,] m = { { s00, s01, s02 }, { s01, s11, s12 }, { s02, s12, s22 } };
        double[] v = { t0, t1, t2 };
        if (!Solve(m, v, out double[] c)) return None;
        double mean = sumE / count, total = sumE2 - count * mean * mean;
        if (total <= 1e-6) return None; // emboss 為常數（例如全黑或全白）：無光照資訊，保持原值。
        double residual = 0;
        for (int y = 1; y < size - 1; y++)
        for (int x = 1; x < size - 1; x++)
        {
            (float gx, float gy) = TerrainHeightEditSession.Gradient(heights, size, x, y);
            double predicted = c[0] + c[1] * gx + c[2] * gy, error = emboss[y * size + x] - predicted;
            residual += error * error;
        }
        double r2 = Math.Max(0, 1 - residual / total);
        return new EmbossLightModel((float)c[0], (float)c[1], (float)c[2], r2);
    }

    private static bool Solve(double[,] m, double[] v, out double[] result)
    {
        result = new double[3];
        double det = Det(m);
        if (Math.Abs(det) < 1e-9) return false;
        for (int column = 0; column < 3; column++)
        {
            var copy = (double[,])m.Clone();
            for (int row = 0; row < 3; row++) copy[row, column] = v[row];
            result[column] = Det(copy) / det;
        }
        return result.All(double.IsFinite);
    }

    private static double Det(double[,] m) =>
        m[0, 0] * (m[1, 1] * m[2, 2] - m[1, 2] * m[2, 1])
        - m[0, 1] * (m[1, 0] * m[2, 2] - m[1, 2] * m[2, 0])
        + m[0, 2] * (m[1, 0] * m[2, 1] - m[1, 1] * m[2, 0]);
}

/// <summary>筆畫路徑補點：回傳兩個 tile 之間（不含起點、含終點）的 Bresenham 直線。</summary>
internal static class TerrainStrokePath
{
    public static IEnumerable<(int X, int Y)> Between(int fromX, int fromY, int toX, int toY)
    {
        int dx = Math.Abs(toX - fromX), dy = -Math.Abs(toY - fromY);
        int sx = fromX < toX ? 1 : -1, sy = fromY < toY ? 1 : -1, error = dx + dy;
        int x = fromX, y = fromY;
        while (x != toX || y != toY)
        {
            int twice = 2 * error;
            if (twice >= dy) { error += dy; x += sx; }
            if (twice <= dx) { error += dx; y += sy; }
            yield return (x, y);
        }
    }
}
