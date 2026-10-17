namespace AgainstRomeMapEditor;

/// <summary>編輯器筆刷工具（與 UI 工具列相同）：高度、通行、材質。</summary>
internal enum EditorBrushTool { Height, Collision, Material }

/// <summary>高度筆刷操作；Water 與 UI「水域（挖到水面下）」相同。</summary>
internal enum EditorHeightBrush { Raise, Lower, Smooth, Flatten, Roughen, Water }

/// <summary>
/// 一筆「按下滑鼠→拖曳→放開」的筆刷操作。Path 為 tile 座標，相鄰點之間與 UI 拖曳相同以
/// <see cref="TerrainStrokePath.Between"/> 補齊，同一筆畫內每格只套用一次。
/// </summary>
internal sealed record EditorBrushStroke(
    EditorBrushTool Tool,
    IReadOnlyList<(int X, int Y)> Path,
    int Size = 1,
    int StrengthLevel = 1,
    EditorHeightBrush HeightOperation = EditorHeightBrush.Raise,
    bool Block = true,
    string? Material = null,
    bool AutoBridge = false)
{
    /// <summary>UI 筆刷大小下拉選單的可用值。</summary>
    public static readonly int[] BrushSizes = [1, 3, 5, 9, 15];

    /// <summary>UI 強度「輕／中／強」對應的筆刷強度。</summary>
    public static int StrengthValue(int level) => level switch { 0 => 2, 2 => 14, _ => 6 };

    /// <summary>UI 以 tile 為單位的筆刷半徑。</summary>
    public float RadiusTiles => Size / 2f + .26f;
}

internal sealed record EditorBrushResult(int HeightSamplesChanged, int CollisionPixelsChanged, int MaterialTilesChanged, int RejectedMaterialDabs, string? Notice);

/// <summary>
/// UI 筆刷與 AI／控制台共用的唯一執行路徑：每一格的效果與玩家在編輯器中手動拖曳筆刷完全相同。
/// 呼叫端負責 CommitStroke（一筆 = 一個復原步驟）與視圖同步。
/// </summary>
internal static class EditorBrushExecutor
{
    /// <summary>水域筆刷挖到水面下的深度（高度圖單位）。</summary>
    public const int WaterBedDepth = 16;
    public const int MinimumWaterDepth = 4;

    /// <summary>水域筆刷的湖底高度；水面太低（不足 <see cref="MinimumWaterDepth"/>）時回傳 -1。</summary>
    public static int WaterBedHeight(float surfaceHeight)
    {
        int surface = (int)MathF.Floor(surfaceHeight);
        return surface < MinimumWaterDepth ? -1 : surface - Math.Min(WaterBedDepth, surface);
    }

    /// <param name="stroke">本次筆畫的工具、路徑與設定。</param>
    /// <param name="heights">高度與碰撞編輯工作階段。</param>
    /// <param name="blend">材質混合編輯工作階段。</param>
    /// <param name="dimension">地圖 tile 邊長。</param>
    /// <param name="roughnessSeed">粗糙化的亂數種子。</param>
    /// <param name="onTexture">材質變更通知。</param>
    /// <param name="onRejectedDab">材質筆刷被拒絕時的通知。</param>
    /// <param name="surfaceHeight">水面高度（Waterlevel ÷ Heightmapstep）。</param>
    /// <param name="allowsTile">可選的 tile 保護（AI 範圍／鎖定）；高度變更會排除受保護 tile 周圍的頂點。</param>
    public static EditorBrushResult Execute(EditorBrushStroke stroke, TerrainHeightEditSession? heights, TerrainBlendEditSession? blend,
        int dimension, float surfaceHeight, int roughnessSeed = 0, Func<int, int, bool>? allowsTile = null,
        Action<TerrainTextureChange>? onTexture = null, Action<int, int>? onRejectedDab = null)
    {
        int flattenTarget = -1;
        return Execute(stroke, heights, blend, dimension, surfaceHeight, ref flattenTarget, roughnessSeed, allowsTile, onTexture, onRejectedDab: onRejectedDab);
    }

    // UI dispatches one dab at a time; retain the first flatten target until mouse-up.
    internal static EditorBrushResult Execute(EditorBrushStroke stroke, TerrainHeightEditSession? heights, TerrainBlendEditSession? blend,
        int dimension, float surfaceHeight, ref int flattenTarget, int roughnessSeed = 0, Func<int, int, bool>? allowsTile = null,
        Action<TerrainTextureChange>? onTexture = null, Action<TerrainBlendPaintResult>? onMaterial = null, Action<int, int>? onRejectedDab = null)
    {
        ArgumentNullException.ThrowIfNull(stroke);
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(dimension);
        if (stroke.Path.Count == 0) return new(0, 0, 0, 0, null);
        var visited = new HashSet<int>();
        int heightCount = 0, collisionCount = 0, textureCount = 0, rejected = 0;
        string? notice = null;
        (int X, int Y)? last = null;
        foreach ((int px, int py) in stroke.Path)
        {
            IEnumerable<(int X, int Y)> tiles = last is { } l ? TerrainStrokePath.Between(l.X, l.Y, px, py) : [(px, py)];
            last = (px, py);
            foreach ((int x0, int y0) in tiles)
            {
                int x = Math.Clamp(x0, 0, dimension - 1), y = Math.Clamp(y0, 0, dimension - 1);
                if (!visited.Add(y * dimension + x)) continue;
                if (allowsTile is not null && !allowsTile(x, y)) continue;
                switch (stroke.Tool)
                {
                    case EditorBrushTool.Height:
                        if (heights is null) return new(heightCount, collisionCount, textureCount, rejected, "此地圖沒有可編輯的高度圖。");
                        (int h, int c, string? n) = PaintHeightTile(stroke, heights, dimension, x, y, surfaceHeight, ref flattenTarget, roughnessSeed, allowsTile);
                        heightCount += h; collisionCount += c; notice ??= n;
                        if (n is not null && c == 0 && h == 0 && stroke.HeightOperation == EditorHeightBrush.Water && flattenTarget < 0) return new(heightCount, collisionCount, textureCount, rejected, n);
                        break;
                    case EditorBrushTool.Collision:
                        if (heights is null || !heights.HasCollision) return new(heightCount, collisionCount, textureCount, rejected, "此地圖沒有可編輯的 collision.bmp。");
                        float cstep = heights.CollisionSize / (float)dimension;
                        collisionCount += heights.PaintCollision((x + .5f) * cstep, (y + .5f) * cstep, stroke.RadiusTiles * cstep,
                            stroke.Block ? TerrainCollisionOperation.Block : TerrainCollisionOperation.Clear,
                            allowsTile is null ? null : (cx, cy) => allowsTile(Math.Min(dimension - 1, (int)(cx / cstep)), Math.Min(dimension - 1, (int)(cy / cstep)))).Count;
                        break;
                    case EditorBrushTool.Material:
                        if (blend is null || stroke.Material is null) return new(heightCount, collisionCount, textureCount, rejected, "此地圖沒有可編輯的材質。");
                        TerrainBlendPaintResult paint = blend.PaintCircle(x + .5f, y + .5f, stroke.RadiusTiles, stroke.Material, autoBridge: stroke.AutoBridge, allowsTile: allowsTile);
                        onMaterial?.Invoke(paint);
                        if (!paint.Succeeded) { rejected++; onRejectedDab?.Invoke(x, y); }
                        foreach (TerrainTextureChange change in paint.TextureChanges) { textureCount++; onTexture?.Invoke(change); }
                        break;
                }
            }
        }
        return new(heightCount, collisionCount, textureCount, rejected, notice);
    }

    private static (int Heights, int Collision, string? Notice) PaintHeightTile(EditorBrushStroke stroke, TerrainHeightEditSession heights, int dimension,
        int tileX, int tileY, float surfaceHeight, ref int flattenTarget, int roughnessSeed, Func<int, int, bool>? allowsTile)
    {
        float step = (heights.VertexSize - 1) / (float)dimension;
        float centerX = (tileX + .5f) * step, centerY = (tileY + .5f) * step, radius = stroke.RadiusTiles * step + 1;
        int strength = EditorBrushStroke.StrengthValue(stroke.StrengthLevel);
        Func<int, bool>? allowsVertex = allowsTile is null ? null : index => AllowsVertex(index % heights.VertexSize / step, index / heights.VertexSize / step, dimension, allowsTile);
        if (stroke.HeightOperation == EditorHeightBrush.Water)
        {
            int bed = WaterBedHeight(surfaceHeight);
            if (bed < 0) return (0, 0, "水面太低，無法使用水域筆刷；請先設定水面高度。");
            flattenTarget = bed;
            TerrainLayerStroke water = heights.PaintWater(centerX, centerY, radius, strength, bed, surfaceHeight);
            int protectedHeights = Protect(heights, water.Heights, allowsVertex);
            int protectedCollision = 0;
            if (allowsTile is not null && water.Collision.Count > 0)
            {
                float cstep = heights.CollisionSize / (float)dimension;
                var rejected = water.Collision.Where(change => !allowsTile(
                    Math.Min(dimension - 1, (int)(change.Index % heights.CollisionSize / cstep)),
                    Math.Min(dimension - 1, (int)(change.Index / heights.CollisionSize / cstep))))
                    .Select(change => change.Index).ToHashSet();
                protectedCollision = heights.PaintCollision(centerX * heights.CollisionSize / (heights.VertexSize - 1),
                    centerY * heights.CollisionSize / (heights.VertexSize - 1), radius * heights.CollisionSize / (heights.VertexSize - 1),
                    TerrainCollisionOperation.Clear, (x, y) => rejected.Contains(y * heights.CollisionSize + x)).Count;
            }
            return (water.Heights.Count - protectedHeights, water.Collision.Count - protectedCollision, heights.HasCollision ? null : "此地圖沒有 collision.bmp；水域通行區域無法自動更新。");
        }
        var operation = stroke.HeightOperation switch
        {
            EditorHeightBrush.Lower => TerrainHeightOperation.Lower,
            EditorHeightBrush.Smooth => TerrainHeightOperation.Smooth,
            EditorHeightBrush.Flatten => TerrainHeightOperation.Flatten,
            EditorHeightBrush.Roughen => TerrainHeightOperation.Roughen,
            _ => TerrainHeightOperation.Raise,
        };
        if (operation == TerrainHeightOperation.Flatten && flattenTarget < 0)
            flattenTarget = heights.Heights[Math.Clamp((int)MathF.Round(centerY), 0, heights.VertexSize - 1) * heights.VertexSize + Math.Clamp((int)MathF.Round(centerX), 0, heights.VertexSize - 1)];
        IReadOnlyList<TerrainSampleChange> changes = heights.PaintHeight(centerX, centerY, radius, operation, strength, flattenTarget, roughnessSeed);
        int protectedCount = Protect(heights, changes, allowsVertex);
        return (changes.Count - protectedCount, 0, null);
    }

    private static int Protect(TerrainHeightEditSession heights, IReadOnlyList<TerrainSampleChange> changes, Func<int, bool>? allowsVertex)
    {
        if (allowsVertex is null) return 0;
        TerrainSampleChange[] revert = changes.Where(change => !allowsVertex(change.Index)).Select(change => new TerrainSampleChange(change.Index, change.After, change.Before)).ToArray();
        if (revert.Length > 0) heights.ApplySampleChanges(revert);
        return revert.Length;
    }

    /// <summary>頂點屬於最多四個 tile；任一相鄰 tile 受保護即不可修改。</summary>
    internal static bool AllowsVertex(float x, float y, int dimension, Func<int, int, bool> allowsTile)
    {
        int left = (int)MathF.Ceiling(x) - 1, right = (int)MathF.Floor(x);
        int top = (int)MathF.Ceiling(y) - 1, bottom = (int)MathF.Floor(y);
        for (int ty = top; ty <= bottom; ty++)
            for (int tx = left; tx <= right; tx++)
                if (tx >= 0 && ty >= 0 && tx < dimension && ty < dimension && !allowsTile(tx, ty)) return false;
        return true;
    }
}
