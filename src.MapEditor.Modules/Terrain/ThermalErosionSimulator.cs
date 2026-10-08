namespace AgainstRomeMapEditor;

/// <summary>
/// 熱力滑坡侵蝕演算法參數。
/// </summary>
internal sealed record ThermalErosionParams
{
    /// <summary>自然休止角（Talus Angle / Angle of Repose，單位：度）。砂石與碎石通常為 32°~42°，堅硬岩壁可達 50°~60°。</summary>
    public float TalusAngleDegrees { get; init; } = 38.0f;

    /// <summary>每次迭代崩塌轉移速率鬆弛因子 [0, 1]。</summary>
    public float ErosionRate { get; init; } = 0.40f;

    /// <summary>擴散迭代次數（次數越多，坡度越完全收斂至休止角，山腳碎石坡面越平展）。</summary>
    public int Iterations { get; init; } = 8;

    /// <summary>水平網格間距（對應頂點間世界尺度，預設為 1.0f）。</summary>
    public float GridSpacing { get; init; } = 1.0f;
}

/// <summary>
/// 熱力滑坡侵蝕模擬器（Thermal / Talus Erosion Simulator）。
/// 依據物理休止角（Angle of Repose），模擬重力風化使過陡的懸崖岩壁自發崩塌，
/// 將滑落的碎石與土方質量守恆地堆積在懸崖腳部，形成真實自然的倒石錐與碎石坡（Talus / Scree Slope）。
/// </summary>
internal static class ThermalErosionSimulator
{
    private static readonly (int Dx, int Dy, float DistRatio)[] Neighbors =
    [
        (-1,  0, 1.0f),
        ( 1,  0, 1.0f),
        ( 0, -1, 1.0f),
        ( 0,  1, 1.0f),
        (-1, -1, 1.41421356f),
        ( 1, -1, 1.41421356f),
        (-1,  1, 1.41421356f),
        ( 1,  1, 1.41421356f)
    ];

    /// <summary>
    /// 執行熱力滑坡侵蝕模擬。
    /// </summary>
    /// <param name="buffer">高度工作緩衝區（32-bit float）</param>
    /// <param name="parameters">熱力侵蝕參數</param>
    /// <param name="brushCenterX">局部筆刷中心 X（小於 0 則為全圖模擬）</param>
    /// <param name="brushCenterY">局部筆刷中心 Y</param>
    /// <param name="brushRadius">局部筆刷半徑（小於等於 0 則為全圖模擬）</param>
    /// <param name="screeAccumulationMap">可選輸出：累計堆積的碎石厚度圖（可用於自動覆蓋碎石材質）</param>
    public static void Simulate(
        TerrainHeightWorkBuffer buffer,
        ThermalErosionParams? parameters = null,
        float? brushCenterX = null,
        float? brushCenterY = null,
        float? brushRadius = null,
        float[]? screeAccumulationMap = null)
    {
        ArgumentNullException.ThrowIfNull(buffer);
        parameters ??= new ThermalErosionParams();

        int size = buffer.Size;
        if (size < 3 || parameters.Iterations <= 0) return;

        bool isBrush = brushCenterX.HasValue && brushCenterY.HasValue && brushRadius.HasValue && brushRadius.Value > 0f;
        float bcx = brushCenterX ?? 0f;
        float bcy = brushCenterY ?? 0f;
        float brad = brushRadius ?? 0f;

        int minX = 0, maxX = size - 1, minY = 0, maxY = size - 1;
        if (isBrush)
        {
            minX = Math.Max(0, (int)MathF.Floor(bcx - brad));
            maxX = Math.Min(size - 1, (int)MathF.Ceiling(bcx + brad));
            minY = Math.Max(0, (int)MathF.Floor(bcy - brad));
            maxY = Math.Min(size - 1, (int)MathF.Ceiling(bcy + brad));
        }

        // 計算休止角在正交與對角距離上的臨界高度差 Threshold
        float angleRad = Math.Clamp(parameters.TalusAngleDegrees, 5f, 85f) * (MathF.PI / 180f);
        float tanAngle = MathF.Tan(angleRad);
        float tOrtho = tanAngle * parameters.GridSpacing;
        float tDiag = tanAngle * (parameters.GridSpacing * 1.41421356f);

        // 雙緩衝：在當前高度上讀取，在 delta 陣列中累加轉移量，防止單向順序偏斜
        float[] deltaHeights = new float[size * size];

        for (int it = 0; it < parameters.Iterations; it++)
        {
            Array.Clear(deltaHeights, 0, deltaHeights.Length);

            for (int y = minY; y <= maxY; y++)
            for (int x = minX; x <= maxX; x++)
            {
                float brushWeight = 1f;
                if (isBrush)
                {
                    float dist = MathF.Sqrt((x - bcx) * (x - bcx) + (y - bcy) * (y - bcy));
                    if (dist > brad) continue;
                    brushWeight = TerrainSculptFilter.CalculateFalloff(dist / brad, TerrainFalloffType.Smoothstep);
                }

                float currentH = buffer[x, y];
                float totalExcess = 0f;
                float minNeighborH = currentH;

                Span<float> excesses = stackalloc float[8];

                for (int i = 0; i < 8; i++)
                {
                    var (dx, dy, distRatio) = Neighbors[i];
                    int nx = x + dx;
                    int ny = y + dy;

                    if (nx < 0 || nx >= size || ny < 0 || ny >= size)
                    {
                        excesses[i] = 0f;
                        continue;
                    }

                    float nbrH = buffer[nx, ny];
                    if (nbrH < minNeighborH) minNeighborH = nbrH;

                    float threshold = distRatio > 1.2f ? tDiag : tOrtho;
                    float drop = currentH - nbrH;

                    if (drop > threshold)
                    {
                        float excess = drop - threshold;
                        excesses[i] = excess;
                        totalExcess += excess;
                    }
                    else
                    {
                        excesses[i] = 0f;
                    }
                }

                if (totalExcess <= 1e-4f) continue;

                // 數值防震盪：單次下移量不得超過落差中位，且受鬆弛因子與筆刷衰減調控
                float maxAllowed = (currentH - minNeighborH) * 0.5f;
                float transferAmount = MathF.Min(maxAllowed, totalExcess * parameters.ErosionRate) * brushWeight;

                if (transferAmount <= 0f) continue;

                // 中心點滑落
                deltaHeights[y * size + x] -= transferAmount;

                // 按各方向超額比例分配給較低鄰居
                float invTotal = 1f / totalExcess;
                for (int i = 0; i < 8; i++)
                {
                    if (excesses[i] <= 0f) continue;

                    var (dx, dy, _) = Neighbors[i];
                    int nx = x + dx;
                    int ny = y + dy;
                    int nIndex = ny * size + nx;

                    float received = transferAmount * (excesses[i] * invTotal);
                    deltaHeights[nIndex] += received;

                    if (screeAccumulationMap is not null && nIndex < screeAccumulationMap.Length)
                    {
                        screeAccumulationMap[nIndex] += received;
                    }
                }
            }

            // 套用累計的滑坡位移
            for (int i = 0; i < deltaHeights.Length; i++)
            {
                if (MathF.Abs(deltaHeights[i]) > 1e-5f)
                {
                    buffer[i] = Math.Clamp(buffer[i] + deltaHeights[i], 0f, 255f);
                }
            }
        }
    }
}
