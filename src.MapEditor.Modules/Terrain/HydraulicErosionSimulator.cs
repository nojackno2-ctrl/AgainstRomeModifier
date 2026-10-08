namespace AgainstRomeMapEditor;

/// <summary>
/// 粒子級水力侵蝕演算法參數。
/// </summary>
internal sealed record HydraulicErosionParams
{
    /// <summary>降落水滴總數（全圖推薦 20,000~100,000；局部筆刷推薦 500~3,000）。</summary>
    public int DropletCount { get; init; } = 30000;

    /// <summary>每顆水滴最大流動步數（生命週期），避免死循環。</summary>
    public int MaxLifetime { get; init; } = 40;

    /// <summary>水滴動量慣性權重 [0, 1)。越高則越傾向維持原前進方向，越低則越完全貼隨坡度轉向。</summary>
    public float Inertia { get; init; } = 0.12f;

    /// <summary>重力加速度常數，決定下坡時水流加速比率。</summary>
    public float Gravity { get; init; } = 4.0f;

    /// <summary>攜沙能力係數 Kc。流速快、坡度陡、水量大時能搬運更多泥沙。</summary>
    public float CapacityFactor { get; init; } = 4.0f;

    /// <summary>最小坡度下限，防止平坦區域攜沙力直接歸零。</summary>
    public float MinSlope { get; init; } = 0.01f;

    /// <summary>侵蝕溶解土壤速率 Ke in (0, 1]。</summary>
    public float ErosionRate { get; init; } = 0.35f;

    /// <summary>泥沙沉積堆積速率 Kd in (0, 1]。</summary>
    public float DepositionRate { get; init; } = 0.30f;

    /// <summary>水滴蒸發率 Kv in (0, 1)。每步蒸發使水量減少，流速衰退。</summary>
    public float EvaporationRate { get; init; } = 0.02f;

    /// <summary>侵蝕核影響半徑（單位：頂點格數）。使用半徑加權分散侵蝕，避免形成針孔狀坑洞。</summary>
    public int ErosionRadius { get; init; } = 2;

    /// <summary>隨機種子，固定種子可確保模擬結果具備確定性。</summary>
    public int RandomSeed { get; init; } = 42;
}

/// <summary>
/// 粒子級水力侵蝕模擬器（Lagrangian Droplet Hydraulic Erosion Simulator）。
/// 模擬雨滴降雨、溶解表土形成溝壑（Rills / Gullies）、沿坡度梯度流動搬運沉積物，
/// 並在窪地、平原與山腳沉積堆積為自然沖積扇（Alluvial Fan）。
/// </summary>
internal static class HydraulicErosionSimulator
{
    /// <summary>
    /// 執行水力侵蝕物理模擬。
    /// </summary>
    /// <param name="buffer">高度工作緩衝區（32-bit float）</param>
    /// <param name="parameters">模擬物理參數</param>
    /// <param name="brushCenterX">局部筆刷中心 X（小於 0 則為全圖降雨）</param>
    /// <param name="brushCenterY">局部筆刷中心 Y</param>
    /// <param name="brushRadius">局部筆刷半徑（小於等於 0 則為全圖降雨）</param>
    public static void Simulate(
        TerrainHeightWorkBuffer buffer,
        HydraulicErosionParams? parameters = null,
        float? brushCenterX = null,
        float? brushCenterY = null,
        float? brushRadius = null)
    {
        ArgumentNullException.ThrowIfNull(buffer);
        parameters ??= new HydraulicErosionParams();

        int size = buffer.Size;
        if (size < 3) return;

        bool isBrush = brushCenterX.HasValue && brushCenterY.HasValue && brushRadius.HasValue && brushRadius.Value > 0f;
        float bcx = brushCenterX ?? 0f;
        float bcy = brushCenterY ?? 0f;
        float brad = brushRadius ?? 0f;

        var rng = new Random(parameters.RandomSeed);

        // 預先計算侵蝕半徑核的相對座標與距離權重
        var erosionOffsets = PrecomputeErosionKernel(parameters.ErosionRadius);

        for (int i = 0; i < parameters.DropletCount; i++)
        {
            // 決定水滴降落起點
            float px, py;
            if (isBrush)
            {
                // 在筆刷圓盤範圍內取樣起點（均勻圓盤）
                float angle = (float)(rng.NextDouble() * Math.PI * 2.0);
                float r = MathF.Sqrt((float)rng.NextDouble()) * brad;
                px = bcx + MathF.Cos(angle) * r;
                py = bcy + MathF.Sin(angle) * r;
                if (px < 1f || px >= size - 2 || py < 1f || py >= size - 2) continue;
            }
            else
            {
                // 全圖隨機投擲水滴（避開最外圍邊界保護環）
                px = 1f + (float)rng.NextDouble() * (size - 3f);
                py = 1f + (float)rng.NextDouble() * (size - 3f);
            }

            float dirX = 0f;
            float dirY = 0f;
            float speed = 1.0f;
            float water = 1.0f;
            float sediment = 0.0f;

            for (int step = 0; step < parameters.MaxLifetime; step++)
            {
                int nodeX = (int)MathF.Floor(px);
                int nodeY = (int)MathF.Floor(py);

                // 計算局部坡度梯度向量
                (float gx, float gy) = buffer.CalculateGradient(px, py);

                // 結合慣性計算流向 (向下坡方向即為梯度反方向)
                dirX = dirX * parameters.Inertia - gx * (1f - parameters.Inertia);
                dirY = dirY * parameters.Inertia - gy * (1f - parameters.Inertia);

                float dirLen = MathF.Sqrt(dirX * dirX + dirY * dirY);
                if (dirLen < 1e-5f)
                {
                    // 局部極平區，隨機微弱擾動避免死鎖
                    dirX = (float)(rng.NextDouble() * 2.0 - 1.0);
                    dirY = (float)(rng.NextDouble() * 2.0 - 1.0);
                    dirLen = MathF.Sqrt(dirX * dirX + dirY * dirY);
                    if (dirLen < 1e-5f) break;
                }

                dirX /= dirLen;
                dirY /= dirLen;

                float nextPx = px + dirX;
                float nextPy = py + dirY;

                // 若流出地圖可侵蝕安全區域，水滴入海終止
                if (nextPx < 1f || nextPx >= size - 2 || nextPy < 1f || nextPy >= size - 2)
                {
                    break;
                }

                float hOld = buffer.SampleBilinear(px, py);
                float hNew = buffer.SampleBilinear(nextPx, nextPy);
                float deltaH = hNew - hOld;

                // 泥沙攜帶上限容量 Capacity
                float capacity = MathF.Max(-deltaH, parameters.MinSlope) * speed * water * parameters.CapacityFactor;

                if (deltaH > 0f)
                {
                    // 水滴遇阻或掉入凹坑（窪地 / 水坑）
                    // 嘗試填平凹坑
                    float fillAmount = MathF.Min(sediment, deltaH);
                    DepositBilinear(buffer, px, py, fillAmount);
                    sediment -= fillAmount;

                    // 若泥沙不足以越過障礙，水滴停滯終止
                    if (sediment <= 0f)
                    {
                        break;
                    }
                }
                else
                {
                    // 向下坡流動
                    if (sediment > capacity)
                    {
                        // 攜沙過載，沉積多餘泥沙
                        float depositAmount = (sediment - capacity) * parameters.DepositionRate;
                        DepositBilinear(buffer, px, py, depositAmount);
                        sediment -= depositAmount;
                    }
                    else
                    {
                        // 具有侵蝕能力，溶解表土
                        float erodeAmount = MathF.Min((capacity - sediment) * parameters.ErosionRate, -deltaH);
                        ErodeKernel(buffer, nodeX, nodeY, erodeAmount, erosionOffsets);
                        sediment += erodeAmount;
                    }
                }

                // 更新流速（重力做功轉化為動能）
                speed = MathF.Sqrt(MathF.Max(0.01f, speed * speed + (-deltaH) * parameters.Gravity));
                water *= (1f - parameters.EvaporationRate);

                if (water < 0.005f)
                {
                    // 水分蒸發殆盡，沉積所有剩餘泥沙
                    DepositBilinear(buffer, px, py, sediment);
                    break;
                }

                px = nextPx;
                py = nextPy;
            }
        }
    }

    /// <summary>
    /// 雙線性權重將泥沙沉積至相鄰 4 個頂點。
    /// </summary>
    private static void DepositBilinear(TerrainHeightWorkBuffer buffer, float x, float y, float amount)
    {
        if (amount <= 0f) return;

        int size = buffer.Size;
        int x0 = (int)MathF.Floor(x);
        int y0 = (int)MathF.Floor(y);
        int x1 = Math.Min(x0 + 1, size - 1);
        int y1 = Math.Min(y0 + 1, size - 1);

        float tx = x - x0;
        float ty = y - y0;

        float w00 = (1f - tx) * (1f - ty);
        float w10 = tx * (1f - ty);
        float w01 = (1f - tx) * ty;
        float w11 = tx * ty;

        buffer[x0, y0] = Math.Clamp(buffer[x0, y0] + amount * w00, 0f, 255f);
        buffer[x1, y0] = Math.Clamp(buffer[x1, y0] + amount * w10, 0f, 255f);
        buffer[x0, y1] = Math.Clamp(buffer[x0, y1] + amount * w01, 0f, 255f);
        buffer[x1, y1] = Math.Clamp(buffer[x1, y1] + amount * w11, 0f, 255f);
    }

    /// <summary>
    /// 在局部半徑核內按權重扣減高度（質量守恆溶解表土）。
    /// </summary>
    private static void ErodeKernel(
        TerrainHeightWorkBuffer buffer,
        int cx, int cy,
        float totalErosion,
        IReadOnlyList<(int Dx, int Dy, float Weight)> kernel)
    {
        if (totalErosion <= 0f) return;

        int size = buffer.Size;
        for (int i = 0; i < kernel.Count; i++)
        {
            var (dx, dy, weight) = kernel[i];
            int nx = cx + dx;
            int ny = cy + dy;

            if (nx >= 0 && nx < size && ny >= 0 && ny < size)
            {
                float delta = totalErosion * weight;
                buffer[nx, ny] = Math.Max(0f, buffer[nx, ny] - delta);
            }
        }
    }

    private static List<(int Dx, int Dy, float Weight)> PrecomputeErosionKernel(int radius)
    {
        radius = Math.Max(1, radius);
        var list = new List<(int Dx, int Dy, float RawWeight)>();
        float sum = 0f;

        for (int dy = -radius; dy <= radius; dy++)
        for (int dx = -radius; dx <= radius; dx++)
        {
            float dist = MathF.Sqrt(dx * dx + dy * dy);
            if (dist <= radius)
            {
                float w = MathF.Max(0f, radius + 1f - dist);
                list.Add((dx, dy, w));
                sum += w;
            }
        }

        var result = new List<(int Dx, int Dy, float Weight)>(list.Count);
        float invSum = sum > 0f ? 1f / sum : 1f;
        foreach (var item in list)
        {
            result.Add((item.Dx, item.Dy, item.RawWeight * invSum));
        }

        return result;
    }
}
