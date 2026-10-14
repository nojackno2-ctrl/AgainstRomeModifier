namespace AgainstRomeMapEditor;

/// <summary>
/// 地形斜坡工具參數。
/// </summary>
internal sealed record TerrainRampParameters
{
    /// <summary>坡道本體平坦區的完整寬度（頂點單位）。</summary>
    public float Width { get; init; } = 4f;

    /// <summary>兩側柔和邊緣衰減過渡區的寬度（頂點單位）。</summary>
    public float Falloff { get; init; } = 2f;

    /// <summary>起點高度覆寫（0~255）。若為 null 則取樣起點處的地形高度。</summary>
    public float? StartHeight { get; init; }

    /// <summary>終點高度覆寫（0~255）。若為 null 則取樣終點處的地形高度。</summary>
    public float? EndHeight { get; init; }

    /// <summary>邊界衰減曲線模式，預設平滑三次多項式 (Smoothstep)。</summary>
    public TerrainFalloffType FalloffType { get; init; } = TerrainFalloffType.Smoothstep;

    /// <summary>是否在起終點兩端加上圓形端帽 (Round Caps)。若為 false 則為平直截斷。</summary>
    public bool RoundCaps { get; init; } = true;
}

/// <summary>
/// 直線雙端點斜坡地形工具（Terrain Ramp Tool）。
/// 
/// 依設計者在圖面上點選的起點與終點，沿著帶狀路徑（包含自訂寬度與邊緣柔和衰減）
/// 在兩端點高度之間進行線性內插。
/// 支援真實高度圖尺寸（如 193x193、257x257），並遵循既有 0~255 綠通道高度編碼。
/// </summary>
internal static class TerrainRampTool
{
    /// <summary>
    /// 計算雙端點直線斜坡對高度圖頂點造成的變更列表（純數學計算，無 UI/狀態依賴）。
    /// </summary>
    /// <param name="heights">當前高度圖頂點資料（長度須為 vertexSize * vertexSize）。</param>
    /// <param name="vertexSize">高度圖邊長（頂點數，如 193 或 257）。</param>
    /// <param name="startX">起點 X 座標（頂點單位）。</param>
    /// <param name="startY">起點 Y 座標（頂點單位）。</param>
    /// <param name="endX">終點 X 座標（頂點單位）。</param>
    /// <param name="endY">終點 Y 座標（頂點單位）。</param>
    /// <param name="parameters">斜坡參數（寬度、邊緣衰減、端點高度覆寫等）。</param>
    /// <returns>所有高度發生改變的頂點樣本清單。</returns>
    public static IReadOnlyList<TerrainSampleChange> CalculateRamp(
        IReadOnlyList<byte> heights,
        int vertexSize,
        float startX,
        float startY,
        float endX,
        float endY,
        TerrainRampParameters? parameters = null)
    {
        ArgumentNullException.ThrowIfNull(heights);
        if (vertexSize < 2 || heights.Count != vertexSize * vertexSize)
            throw new ArgumentException($"高度圖尺寸無效（長度 {heights.Count} != {vertexSize}x{vertexSize}）。", nameof(heights));
        if (!float.IsFinite(startX) || !float.IsFinite(startY) || !float.IsFinite(endX) || !float.IsFinite(endY))
            throw new ArgumentException("斜坡座標必須為有限浮點數。");

        parameters ??= new TerrainRampParameters();
        if (parameters.Width < 0f || parameters.Falloff < 0f)
            throw new ArgumentOutOfRangeException(nameof(parameters), "寬度與衰減距離不可為負值。");

        float dx = endX - startX;
        float dy = endY - startY;
        float lenSq = dx * dx + dy * dy;
        if (lenSq < 1e-6f)
        {
            // 起訖點重合，無法建立帶狀斜坡
            return Array.Empty<TerrainSampleChange>();
        }
        float len = MathF.Sqrt(lenSq);

        float h0 = parameters.StartHeight.HasValue
            ? Math.Clamp(parameters.StartHeight.Value, 0f, 255f)
            : SampleBilinear(heights, vertexSize, startX, startY);

        float h1 = parameters.EndHeight.HasValue
            ? Math.Clamp(parameters.EndHeight.Value, 0f, 255f)
            : SampleBilinear(heights, vertexSize, endX, endY);

        float halfWidth = parameters.Width / 2f;
        float totalRadius = halfWidth + parameters.Falloff;
        if (totalRadius <= 0f)
        {
            return Array.Empty<TerrainSampleChange>();
        }

        // 邊界外接矩形並向外擴展總影響半徑
        int minX = (int)MathF.Floor(Math.Min(startX, endX) - totalRadius);
        int maxX = (int)MathF.Ceiling(Math.Max(startX, endX) + totalRadius);
        int minY = (int)MathF.Floor(Math.Min(startY, endY) - totalRadius);
        int maxY = (int)MathF.Ceiling(Math.Max(startY, endY) + totalRadius);

        // 裁切至實際高度圖邊界 [0, vertexSize - 1]
        minX = Math.Max(0, minX);
        maxX = Math.Min(vertexSize - 1, maxX);
        minY = Math.Max(0, minY);
        maxY = Math.Min(vertexSize - 1, maxY);

        if (minX > maxX || minY > maxY)
        {
            return Array.Empty<TerrainSampleChange>();
        }

        float totalRadiusSq = totalRadius * totalRadius;
        var changes = new List<TerrainSampleChange>();

        for (int y = minY; y <= maxY; y++)
        {
            for (int x = minX; x <= maxX; x++)
            {
                float vx = x - startX;
                float vy = y - startY;

                // 投影至中心線段之標量參數 t
                float t = (vx * dx + vy * dy) / lenSq;

                float dist;
                float targetHeight;

                if (parameters.RoundCaps)
                {
                    float tClamped = Math.Clamp(t, 0f, 1f);
                    float projX = startX + tClamped * dx;
                    float projY = startY + tClamped * dy;
                    float dSq = (x - projX) * (x - projX) + (y - projY) * (y - projY);
                    if (dSq > totalRadiusSq) continue;

                    dist = MathF.Sqrt(dSq);
                    targetHeight = h0 + tClamped * (h1 - h0);
                }
                else
                {
                    if (t < 0f || t > 1f) continue;
                    dist = Math.Abs(vx * (-dy) + vy * dx) / len;
                    if (dist > totalRadius) continue;

                    targetHeight = h0 + t * (h1 - h0);
                }

                float weight;
                if (dist <= halfWidth)
                {
                    weight = 1f;
                }
                else if (parameters.Falloff > 1e-4f)
                {
                    float normFalloff = (dist - halfWidth) / parameters.Falloff;
                    weight = TerrainSculptFilter.CalculateFalloff(normFalloff, parameters.FalloffType);
                }
                else
                {
                    weight = 0f;
                }

                if (weight <= 0f) continue;

                int index = y * vertexSize + x;
                byte before = heights[index];
                float blended = weight * targetHeight + (1f - weight) * before;
                byte after = (byte)Math.Clamp((int)MathF.Round(blended), 0, 255);

                if (after != before)
                {
                    changes.Add(new TerrainSampleChange(index, before, after));
                }
            }
        }

        return changes;
    }

    /// <summary>
    /// 套用斜坡至 TerrainHeightEditSession 並提交筆畫成為單一復原／重做步驟。
    /// 使用現有公用 API（ApplySampleChanges 與 CommitStroke）。
    /// </summary>
    public static IReadOnlyList<TerrainSampleChange> ApplyRamp(
        TerrainHeightEditSession session,
        float startX,
        float startY,
        float endX,
        float endY,
        TerrainRampParameters? parameters = null)
    {
        ArgumentNullException.ThrowIfNull(session);
        var changes = CalculateRamp(session.Heights, session.VertexSize, startX, startY, endX, endY, parameters);
        if (changes.Count > 0)
        {
            session.ApplySampleChanges(changes);
            session.CommitStroke();
        }
        return changes;
    }

    /// <summary>
    /// 雙線性插值取樣頂點高度。
    /// </summary>
    public static float SampleBilinear(IReadOnlyList<byte> heights, int vertexSize, float x, float y)
    {
        ArgumentNullException.ThrowIfNull(heights);
        if (vertexSize < 2) return 0f;

        x = Math.Clamp(x, 0f, vertexSize - 1f);
        y = Math.Clamp(y, 0f, vertexSize - 1f);
        int left = Math.Min((int)x, vertexSize - 2);
        int top = Math.Min((int)y, vertexSize - 2);
        float tx = x - left, ty = y - top;

        float a = heights[top * vertexSize + left];
        float b = heights[top * vertexSize + left + 1];
        float c = heights[(top + 1) * vertexSize + left];
        float d = heights[(top + 1) * vertexSize + left + 1];

        return (a + (b - a) * tx) * (1f - ty) + (c + (d - c) * tx) * ty;
    }

    /// <summary>
    /// 將圖塊中心座標換算為高度圖頂點座標。
    /// </summary>
    public static (float X, float Y) TileToVertex(int tileX, int tileY, int dimension, int vertexSize)
    {
        if (dimension <= 0) return (0f, 0f);
        float step = (vertexSize - 1) / (float)dimension;
        return ((tileX + 0.5f) * step, (tileY + 0.5f) * step);
    }
}
