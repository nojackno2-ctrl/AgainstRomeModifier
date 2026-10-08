namespace AgainstRomeMapEditor;

/// <summary>
/// 筆刷邊界衰減曲線模式。
/// </summary>
internal enum TerrainFalloffType
{
    /// <summary>平滑三次多項式 (3t^2 - 2t^3)，邊界無縫銜接。</summary>
    Smoothstep,
    /// <summary>半週期餘弦衰減 (0.5 * (1 + cos(pi * t)))。</summary>
    Cosine,
    /// <summary>半球體外形 (sqrt(1 - t^2))，適合穹頂與平緩隆起。</summary>
    Spherical,
    /// <summary>高斯尖峰 (exp(-3 * t^2))，適合尖銳山峰。</summary>
    Gaussian,
    /// <summary>線性錐狀衰減 (1 - t)。</summary>
    Linear,
    /// <summary>高原平頂型衰減：中心 40% 保持全開平坦，邊緣平滑下降。</summary>
    Plateau
}

/// <summary>
/// 幾何地形雕刻濾鏡庫（Terrain Sculpting Filter）。
/// 包含隆起（Elevate）、下壓（Depress）、高原平頂化（Terrace）、山脊尖銳化（Ridge Sharpening）與山谷刻蝕等。
/// </summary>
internal static class TerrainSculptFilter
{
    /// <summary>
    /// 計算特定衰減型別在正規化距離 t in [0, 1] 處的權重值（中心 1.0，邊緣 0.0）。
    /// </summary>
    public static float CalculateFalloff(float t, TerrainFalloffType type)
    {
        if (t <= 0f) return 1f;
        if (t >= 1f) return 0f;

        return type switch
        {
            TerrainFalloffType.Smoothstep => (1f - t * t * (3f - 2f * t)),
            TerrainFalloffType.Cosine => 0.5f * (1f + MathF.Cos(MathF.PI * t)),
            TerrainFalloffType.Spherical => MathF.Sqrt(Math.Max(0f, 1f - t * t)),
            TerrainFalloffType.Gaussian => MathF.Exp(-3.5f * t * t),
            TerrainFalloffType.Linear => 1f - t,
            TerrainFalloffType.Plateau => t < 0.4f ? 1f : 1f - Smoothstep((t - 0.4f) / 0.6f),
            _ => 1f - t
        };

        static float Smoothstep(float v) => v * v * (3f - 2f * v);
    }

    /// <summary>
    /// 局部或區域隆起（Elevate）。
    /// </summary>
    public static void Elevate(
        TerrainHeightWorkBuffer buffer,
        float cx, float cy, float radius,
        float strength,
        TerrainFalloffType falloff = TerrainFalloffType.Smoothstep,
        float maxHeight = 255f)
    {
        ArgumentNullException.ThrowIfNull(buffer);
        if (radius <= 0f || strength <= 0f) return;

        GetBrushBounds(buffer.Size, cx, cy, radius, out int minX, out int maxX, out int minY, out int maxY);

        for (int y = minY; y <= maxY; y++)
        for (int x = minX; x <= maxX; x++)
        {
            float dist = MathF.Sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy));
            if (dist > radius) continue;

            float weight = CalculateFalloff(dist / radius, falloff);
            float current = buffer[x, y];
            float delta = strength * weight;
            buffer[x, y] = Math.Min(maxHeight, current + delta);
        }
    }

    /// <summary>
    /// 局部或區域下壓（Depress）。
    /// </summary>
    public static void Depress(
        TerrainHeightWorkBuffer buffer,
        float cx, float cy, float radius,
        float strength,
        TerrainFalloffType falloff = TerrainFalloffType.Smoothstep,
        float minHeight = 0f)
    {
        ArgumentNullException.ThrowIfNull(buffer);
        if (radius <= 0f || strength <= 0f) return;

        GetBrushBounds(buffer.Size, cx, cy, radius, out int minX, out int maxX, out int minY, out int maxY);

        for (int y = minY; y <= maxY; y++)
        for (int x = minX; x <= maxX; x++)
        {
            float dist = MathF.Sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy));
            if (dist > radius) continue;

            float weight = CalculateFalloff(dist / radius, falloff);
            float current = buffer[x, y];
            float delta = strength * weight;
            buffer[x, y] = Math.Max(minHeight, current - delta);
        }
    }

    /// <summary>
    /// 高原平頂化 / 階梯化濾鏡（Terrace Filter）。
    /// 將連續斜坡量化為梯田／層狀岩層，階頂平坦，階緣陡峭。
    /// </summary>
    /// <param name="buffer">高度工作緩衝區</param>
    /// <param name="cx">筆刷中心 X（小於 0 則套用全圖）</param>
    /// <param name="cy">筆刷中心 Y</param>
    /// <param name="radius">筆刷半徑（小於等於 0 則套用全圖）</param>
    /// <param name="stepInterval">每階階梯高差間隔（如 16~32）</param>
    /// <param name="flatness">階面平坦度權重 (0.0=無變化, 1.0=完全平整台階)</param>
    /// <param name="edgeSharpness">台階邊緣陡峭程度 (1.0=自然 smoothstep, 2.0+=急劇陡崖)</param>
    /// <param name="strength">整體套用強度 (0.0~1.0)</param>
    /// <param name="baseOffset">階梯起算基準高度偏移</param>
    public static void Terrace(
        TerrainHeightWorkBuffer buffer,
        float cx, float cy, float radius,
        float stepInterval,
        float flatness = 0.85f,
        float edgeSharpness = 1.5f,
        float strength = 1.0f,
        float baseOffset = 0f)
    {
        ArgumentNullException.ThrowIfNull(buffer);
        if (stepInterval <= 0.1f || strength <= 0f) return;

        bool isGlobal = radius <= 0f || cx < 0f || cy < 0f;
        int minX = 0, maxX = buffer.Size - 1, minY = 0, maxY = buffer.Size - 1;
        if (!isGlobal)
        {
            GetBrushBounds(buffer.Size, cx, cy, radius, out minX, out maxX, out minY, out maxY);
        }

        flatness = Math.Clamp(flatness, 0f, 1f);
        edgeSharpness = Math.Clamp(edgeSharpness, 0.5f, 4.0f);
        strength = Math.Clamp(strength, 0f, 1f);

        for (int y = minY; y <= maxY; y++)
        for (int x = minX; x <= maxX; x++)
        {
            float brushWeight = 1f;
            if (!isGlobal)
            {
                float dist = MathF.Sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy));
                if (dist > radius) continue;
                brushWeight = CalculateFalloff(dist / radius, TerrainFalloffType.Smoothstep);
            }

            float currentH = buffer[x, y];
            float normalized = (currentH - baseOffset) / stepInterval;
            float stepIndex = MathF.Floor(normalized);
            float frac = normalized - stepIndex; // [0, 1)

            // S 型邊界曲線：階面中段平坦，兩端邊緣快速銜接
            float shapedFrac;
            if (edgeSharpness <= 1.0f)
            {
                shapedFrac = frac * frac * (3f - 2f * frac);
            }
            else
            {
                // 高階多項式塑形
                float p = edgeSharpness;
                shapedFrac = frac < 0.5f
                    ? 0.5f * MathF.Pow(2f * frac, p)
                    : 1f - 0.5f * MathF.Pow(2f * (1f - frac), p);
            }

            // 混合階梯平坦化效果
            float terraceFrac = Lerp(frac, shapedFrac, flatness);
            float targetH = baseOffset + (stepIndex + terraceFrac) * stepInterval;

            // 結合筆刷衰減與強度
            float blend = brushWeight * strength;
            buffer[x, y] = Math.Clamp(Lerp(currentH, targetH, blend), 0f, 255f);
        }

        static float Lerp(float a, float b, float t) => a + (b - a) * t;
    }

    /// <summary>
    /// 山脊尖銳化濾鏡（Ridge Sharpening / Arête Filter）。
    /// 利用拉普拉斯曲率算子（Laplacian Curvature）偵測凸面結構，
    /// 對圓鈍的山丘頂端進行收縮聚攏與垂直拉伸，塑造險峻尖銳的山脊與冰蝕角峰。
    /// </summary>
    /// <param name="buffer">高度工作緩衝區</param>
    /// <param name="cx">筆刷中心 X（負數代表全圖）</param>
    /// <param name="cy">筆刷中心 Y</param>
    /// <param name="radius">筆刷半徑（小於等於 0 代表全圖）</param>
    /// <param name="gain">銳化增益強度 (例如 0.3~1.0)</param>
    /// <param name="ridgeOnly">若為 true，只銳化加強凸起的山脊頂部，不加深下凹的山谷；若為 false，山脊與山谷同時立體化</param>
    /// <param name="iterations">迭代計算次數</param>
    public static void SharpenRidge(
        TerrainHeightWorkBuffer buffer,
        float cx, float cy, float radius,
        float gain = 0.5f,
        bool ridgeOnly = true,
        int iterations = 1)
    {
        ArgumentNullException.ThrowIfNull(buffer);
        if (gain <= 0f || iterations <= 0) return;

        bool isGlobal = radius <= 0f || cx < 0f || cy < 0f;
        int minX = 0, maxX = buffer.Size - 1, minY = 0, maxY = buffer.Size - 1;
        if (!isGlobal)
        {
            GetBrushBounds(buffer.Size, cx, cy, radius, out minX, out maxX, out minY, out maxY);
        }

        for (int it = 0; it < iterations; it++)
        {
            var temp = buffer.Clone();

            for (int y = minY; y <= maxY; y++)
            for (int x = minX; x <= maxX; x++)
            {
                float brushWeight = 1f;
                if (!isGlobal)
                {
                    float dist = MathF.Sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy));
                    if (dist > radius) continue;
                    brushWeight = CalculateFalloff(dist / radius, TerrainFalloffType.Smoothstep);
                }

                // Laplacian: 凸出區域 < 0, 凹陷區域 > 0
                float laplacian = temp.CalculateLaplacian(x, y);

                float delta = 0f;
                if (laplacian < 0f)
                {
                    // 山脊/山頂凸起：向外拉尖 (拉普拉斯為負，-gain * laplacian 為正值增益)
                    delta = -gain * laplacian;
                }
                else if (!ridgeOnly)
                {
                    // 谷底凹陷：向下深切
                    delta = -gain * laplacian;
                }

                if (MathF.Abs(delta) > 0.001f)
                {
                    float currentH = temp[x, y];
                    float newH = Math.Clamp(currentH + delta * brushWeight, 0f, 255f);
                    buffer[x, y] = newH;
                }
            }
        }
    }

    /// <summary>
    /// 山谷凹雕刻蝕濾鏡（Valley Carving）。
    /// 針對凹陷谷底向下加深並收窄兩側斜坡，雕塑 V 形峽谷。
    /// </summary>
    public static void CarveValley(
        TerrainHeightWorkBuffer buffer,
        float cx, float cy, float radius,
        float depth = 1.0f,
        float strength = 1.0f)
    {
        ArgumentNullException.ThrowIfNull(buffer);
        if (depth <= 0f || strength <= 0f || radius <= 0f) return;

        GetBrushBounds(buffer.Size, cx, cy, radius, out int minX, out int maxX, out int minY, out int maxY);
        var temp = buffer.Clone();

        for (int y = minY; y <= maxY; y++)
        for (int x = minX; x <= maxX; x++)
        {
            float dist = MathF.Sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy));
            if (dist > radius) continue;

            float weight = CalculateFalloff(dist / radius, TerrainFalloffType.Smoothstep);
            float laplacian = temp.CalculateLaplacian(x, y);

            // 僅作用於凹陷區域 (laplacian > 0)
            if (laplacian > 0f)
            {
                float carve = laplacian * depth * weight * strength;
                buffer[x, y] = Math.Max(0f, buffer[x, y] - carve);
            }
        }
    }

    private static void GetBrushBounds(int size, float cx, float cy, float radius, out int minX, out int maxX, out int minY, out int maxY)
    {
        minX = Math.Max(0, (int)MathF.Floor(cx - radius));
        maxX = Math.Min(size - 1, (int)MathF.Ceiling(cx + radius));
        minY = Math.Max(0, (int)MathF.Floor(cy - radius));
        maxY = Math.Min(size - 1, (int)MathF.Ceiling(cy + radius));
    }
}
