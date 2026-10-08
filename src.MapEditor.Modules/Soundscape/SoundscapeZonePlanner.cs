namespace AgainstRomeMapEditor.Modules.Soundscape;

/// <summary>
/// 音效區域規劃器與智慧伴生環境音生成引擎。
/// 支援凸多邊形、圓形、定向矩形手動畫筆繪製，並能自動依水體地形與森林分佈推導伴生環境音。
/// </summary>
public sealed class SoundscapeZonePlanner
{
    private readonly SoundscapeZoneCatalog _catalog;

    public SoundscapeZonePlanner(SoundscapeZoneCatalog? catalog = null)
    {
        _catalog = catalog ?? SoundscapeZoneCatalog.Shared;
    }

    #region 手動幾何區域繪製建構式

    /// <summary>建立點音源區域。</summary>
    public SoundscapeZone CreatePointZone(
        string name,
        string soundDefId,
        float cx, float cy, float cz,
        float innerRadius = 100.0f,
        float outerRadius = 500.0f,
        float baseVolume = 1.0f,
        AttenuationCurveKind curve = AttenuationCurveKind.SmoothStep)
    {
        var def = _catalog.Resolve(soundDefId);
        return new SoundscapeZone
        {
            Name = name,
            SoundDefId = soundDefId,
            ShapeType = SoundscapeShapeType.Point,
            Category = def.Category,
            CenterWorldX = cx,
            CenterWorldY = cy,
            CenterWorldZ = cz,
            InnerRadius = innerRadius,
            OuterRadius = outerRadius,
            BaseVolume = baseVolume,
            AttenuationCurve = curve,
            Attributes = SoundscapeZoneAttributes.IsActive | (def.TriggerMode == SoundTriggerMode.ContinuousLoop ? SoundscapeZoneAttributes.IsLooping : SoundscapeZoneAttributes.None)
        };
    }

    /// <summary>建立圓形音效區域。</summary>
    public SoundscapeZone CreateCircleZone(
        string name,
        string soundDefId,
        float cx, float cy, float cz,
        float baseRadius,
        float innerRadius = 150.0f,
        float outerRadius = 600.0f,
        float baseVolume = 1.0f,
        AttenuationCurveKind curve = AttenuationCurveKind.SmoothStep)
    {
        var def = _catalog.Resolve(soundDefId);
        return new SoundscapeZone
        {
            Name = name,
            SoundDefId = soundDefId,
            ShapeType = SoundscapeShapeType.Circle,
            Category = def.Category,
            CenterWorldX = cx,
            CenterWorldY = cy,
            CenterWorldZ = cz,
            ParamA = Math.Max(10f, baseRadius),
            InnerRadius = innerRadius,
            OuterRadius = outerRadius,
            BaseVolume = baseVolume,
            AttenuationCurve = curve,
            Attributes = SoundscapeZoneAttributes.IsActive | (def.TriggerMode == SoundTriggerMode.ContinuousLoop ? SoundscapeZoneAttributes.IsLooping : SoundscapeZoneAttributes.None)
        };
    }

    /// <summary>建立定向矩形音效區域。</summary>
    public SoundscapeZone CreateRectangleZone(
        string name,
        string soundDefId,
        float cx, float cy, float cz,
        float halfWidth,
        float halfHeight,
        float angleDeg = 0.0f,
        float innerRadius = 150.0f,
        float outerRadius = 600.0f,
        float baseVolume = 1.0f,
        AttenuationCurveKind curve = AttenuationCurveKind.SmoothStep)
    {
        var def = _catalog.Resolve(soundDefId);
        return new SoundscapeZone
        {
            Name = name,
            SoundDefId = soundDefId,
            ShapeType = SoundscapeShapeType.OrientedRectangle,
            Category = def.Category,
            CenterWorldX = cx,
            CenterWorldY = cy,
            CenterWorldZ = cz,
            ParamA = Math.Max(10f, halfWidth),
            ParamB = Math.Max(10f, halfHeight),
            ParamAngleDeg = angleDeg,
            InnerRadius = innerRadius,
            OuterRadius = outerRadius,
            BaseVolume = baseVolume,
            AttenuationCurve = curve,
            Attributes = SoundscapeZoneAttributes.IsActive | (def.TriggerMode == SoundTriggerMode.ContinuousLoop ? SoundscapeZoneAttributes.IsLooping : SoundscapeZoneAttributes.None)
        };
    }

    /// <summary>建立凸多邊形音效區域（自動計算凸包頂點順序與中心點）。</summary>
    public SoundscapeZone CreateConvexPolygonZone(
        string name,
        string soundDefId,
        IEnumerable<SoundscapeVertex> inputPoints,
        float cy = 0.0f,
        float innerRadius = 150.0f,
        float outerRadius = 600.0f,
        float baseVolume = 1.0f,
        AttenuationCurveKind curve = AttenuationCurveKind.SmoothStep)
    {
        var def = _catalog.Resolve(soundDefId);
        var convexHull = ComputeConvexHull(inputPoints);

        if (convexHull.Count < 3)
            throw new ArgumentException("凸多邊形至少需要 3 個非共線點。", nameof(inputPoints));

        // 計算頂點質心
        float sumX = 0f, sumZ = 0f;
        foreach (var v in convexHull)
        {
            sumX += v.X;
            sumZ += v.Z;
        }
        float cx = sumX / convexHull.Count;
        float cz = sumZ / convexHull.Count;

        var zone = new SoundscapeZone
        {
            Name = name,
            SoundDefId = soundDefId,
            ShapeType = SoundscapeShapeType.ConvexPolygon,
            Category = def.Category,
            CenterWorldX = cx,
            CenterWorldY = cy,
            CenterWorldZ = cz,
            InnerRadius = innerRadius,
            OuterRadius = outerRadius,
            BaseVolume = baseVolume,
            AttenuationCurve = curve,
            Attributes = SoundscapeZoneAttributes.IsActive | (def.TriggerMode == SoundTriggerMode.ContinuousLoop ? SoundscapeZoneAttributes.IsLooping : SoundscapeZoneAttributes.None)
        };
        zone.PolygonVertices.AddRange(convexHull);
        return zone;
    }

    #endregion

    #region 智慧伴生環境音自動生成

    /// <summary>
    /// 水體水文伴生環境音生成輸入資料。
    /// </summary>
    public sealed record WaterCompanionContext(
        int Dimension,
        float TileWorldSize,
        IReadOnlyList<byte>? Heights,
        int VertexSize,
        float HeightMapStep,
        byte WaterLevel,
        IReadOnlySet<int>? RiverTileIndices);

    /// <summary>
    /// 依據水文地質高程與水體分佈，自動分析並生成伴生水系環境音（急流、平緩流水、斷崖瀑布、海浪、池塘）。
    /// </summary>
    public IReadOnlyList<SoundscapeZone> GenerateHydrologySoundscapes(WaterCompanionContext context)
    {
        ArgumentNullException.ThrowIfNull(context);

        var generated = new List<SoundscapeZone>();
        int dim = context.Dimension;
        float tileSize = context.TileWorldSize;
        bool[] isWater = new bool[dim * dim];

        // 1. 識別水域格子
        for (int ty = 0; ty < dim; ty++)
        {
            for (int tx = 0; tx < dim; tx++)
            {
                int tileIdx = ty * dim + tx;
                bool water = false;

                if (context.RiverTileIndices is not null && context.RiverTileIndices.Contains(tileIdx))
                {
                    water = true;
                }
                else if (context.Heights is not null && context.Heights.Count >= (context.VertexSize * context.VertexSize))
                {
                    int vSize = context.VertexSize;
                    // 取四個頂點的平均高度
                    float h0 = context.Heights[ty * vSize + tx];
                    float h1 = context.Heights[ty * vSize + (tx + 1)];
                    float h2 = context.Heights[(ty + 1) * vSize + tx];
                    float h3 = context.Heights[(ty + 1) * vSize + (tx + 1)];
                    float avgH = (h0 + h1 + h2 + h3) * 0.25f;

                    if (avgH < context.WaterLevel)
                        water = true;
                }

                isWater[tileIdx] = water;
            }
        }

        // 2. 泛洪 BFS 連通水體分析
        bool[] visited = new bool[dim * dim];
        int zoneCounter = 1;

        for (int y = 0; y < dim; y++)
        {
            for (int x = 0; x < dim; x++)
            {
                int startIdx = y * dim + x;
                if (!isWater[startIdx] || visited[startIdx])
                    continue;

                // 搜尋連通水體
                var componentTiles = new List<(int X, int Y)>();
                var queue = new Queue<(int X, int Y)>();
                queue.Enqueue((x, y));
                visited[startIdx] = true;

                bool touchesBorder = false;
                int minX = x, maxX = x, minY = y, maxY = y;

                while (queue.Count > 0)
                {
                    var curr = queue.Dequeue();
                    componentTiles.Add(curr);

                    minX = Math.Min(minX, curr.X);
                    maxX = Math.Max(maxX, curr.X);
                    minY = Math.Min(minY, curr.Y);
                    maxY = Math.Max(maxY, curr.Y);

                    if (curr.X == 0 || curr.X == dim - 1 || curr.Y == 0 || curr.Y == dim - 1)
                        touchesBorder = true;

                    // 四向相鄰檢查
                    ReadOnlySpan<(int DX, int DY)> neighbors = [(1, 0), (-1, 0), (0, 1), (0, -1)];
                    foreach (var n in neighbors)
                    {
                        int nx = curr.X + n.DX;
                        int ny = curr.Y + n.DY;
                        if (nx >= 0 && nx < dim && ny >= 0 && ny < dim)
                        {
                            int nIdx = ny * dim + nx;
                            if (isWater[nIdx] && !visited[nIdx])
                            {
                                visited[nIdx] = true;
                                queue.Enqueue((nx, ny));
                            }
                        }
                    }
                }

                int tileCount = componentTiles.Count;
                float spanX = (maxX - minX + 1) * tileSize;
                float spanY = (maxY - minY + 1) * tileSize;

                // 3. 依據形貌分類與聲場生成
                if (tileCount < 20)
                {
                    // 小水潭/池沼
                    float cx = (minX + maxX) * 0.5f * tileSize;
                    float cz = (minY + maxY) * 0.5f * tileSize;
                    var pond = CreateCircleZone(
                        $"水潭蛙鳴 #{zoneCounter++}",
                        "Amb_Pond_Marsh",
                        cx, 0f, cz,
                        baseRadius: Math.Max(spanX, spanY) * 0.5f,
                        innerRadius: 100f,
                        outerRadius: 400f,
                        baseVolume: 0.65f);
                    pond.Attributes |= SoundscapeZoneAttributes.IsAutoGenerated;
                    generated.Add(pond);
                }
                else if (touchesBorder && tileCount > 800)
                {
                    // 大海或巨型湖泊海岸線
                    // 抽樣提取水陸交界海岸線
                    var coastTiles = componentTiles.Where(t =>
                    {
                        ReadOnlySpan<(int DX, int DY)> deltas = [(1, 0), (-1, 0), (0, 1), (0, -1)];
                        foreach (var d in deltas)
                        {
                            int nx = t.X + d.DX, ny = t.Y + d.DY;
                            if (nx >= 0 && nx < dim && ny >= 0 && ny < dim && !isWater[ny * dim + nx])
                                return true;
                        }
                        return false;
                    }).ToList();

                    // 每隔約 16 個海岸點均勻佈設海浪拍岸音源
                    int step = Math.Max(1, coastTiles.Count / 12);
                    for (int ci = 0; ci < coastTiles.Count; ci += step)
                    {
                        var ct = coastTiles[ci];
                        var wave = CreateCircleZone(
                            $"海岸潮聲 #{zoneCounter++}",
                            "Amb_Coast_Waves",
                            ct.X * tileSize, 0f, ct.Y * tileSize,
                            baseRadius: tileSize * 3f,
                            innerRadius: 200f,
                            outerRadius: 800f,
                            baseVolume: 0.8f);
                        wave.Attributes |= SoundscapeZoneAttributes.IsAutoGenerated;
                        generated.Add(wave);
                    }
                }
                else
                {
                    // 河道/急流/瀑布系統
                    // 檢測高程陡峭落差處（瀑布）
                    bool hasWaterfall = false;
                    if (context.Heights is not null)
                    {
                        int vSize = context.VertexSize;
                        foreach (var t in componentTiles)
                        {
                            int tx = t.X, ty = t.Y;
                            float hCenter = context.Heights[ty * vSize + tx];

                            // 檢查周邊水格的高度差
                            ReadOnlySpan<(int DX, int DY)> deltas = [(1, 0), (-1, 0), (0, 1), (0, -1)];
                            foreach (var d in deltas)
                            {
                                int nx = tx + d.DX, ny = ty + d.DY;
                                if (nx >= 0 && nx < dim && ny >= 0 && ny < dim && isWater[ny * dim + nx])
                                {
                                    float hNeighbor = context.Heights[ny * vSize + nx];
                                    float drop = (hCenter - hNeighbor) * context.HeightMapStep;

                                    if (drop > 16.0f) // 顯著垂直落差
                                    {
                                        var waterfall = CreatePointZone(
                                            $"萬鈞瀑布 #{zoneCounter++}",
                                            "Amb_Waterfall_Roar",
                                            tx * tileSize, hCenter * context.HeightMapStep, ty * tileSize,
                                            innerRadius: 350f,
                                            outerRadius: 1200f,
                                            baseVolume: 1.0f);
                                        waterfall.Attributes |= SoundscapeZoneAttributes.IsAutoGenerated;
                                        generated.Add(waterfall);
                                        hasWaterfall = true;
                                        break;
                                    }
                                }
                            }
                            if (hasWaterfall) break;
                        }
                    }

                    // 沿河流主幹每隔一定距離散佈水流聲
                    int riverSampleStep = Math.Max(1, componentTiles.Count / 8);
                    for (int ri = 0; ri < componentTiles.Count; ri += riverSampleStep)
                    {
                        var rt = componentTiles[ri];
                        var river = CreateCircleZone(
                            $"流水潺潺 #{zoneCounter++}",
                            "Amb_River_Gentle",
                            rt.X * tileSize, 0f, rt.Y * tileSize,
                            baseRadius: tileSize * 2f,
                            innerRadius: 150f,
                            outerRadius: 550f,
                            baseVolume: 0.7f);
                        river.Attributes |= SoundscapeZoneAttributes.IsAutoGenerated;
                        generated.Add(river);
                    }
                }
            }
        }

        return generated;
    }

    /// <summary>
    /// 依據植被與樹木物件位置密度，自動分析並生成伴生森林環境音（密林風動、鳥鳴群、疏林）。
    /// </summary>
    public IReadOnlyList<SoundscapeZone> GenerateForestSoundscapes(
        IReadOnlyList<(float X, float Z)> treePositions,
        float clusterBinWorldSize = 512.0f,
        int denseClusterThreshold = 8)
    {
        ArgumentNullException.ThrowIfNull(treePositions);

        if (treePositions.Count < 3)
            return [];

        var generated = new List<SoundscapeZone>();
        var bins = new Dictionary<(int BX, int BZ), List<(float X, float Z)>>();

        // 1. 空間分箱網格聚類
        foreach (var pos in treePositions)
        {
            int bx = (int)MathF.Floor(pos.X / clusterBinWorldSize);
            int bz = (int)MathF.Floor(pos.Z / clusterBinWorldSize);
            var key = (bx, bz);

            if (!bins.TryGetValue(key, out var list))
            {
                list = [];
                bins[key] = list;
            }
            list.Add(pos);
        }

        int clusterCounter = 1;

        // 2. 評估每個森林簇群
        foreach (var (binKey, trees) in bins)
        {
            if (trees.Count >= denseClusterThreshold)
            {
                // 密集森林核心 -> 生成凸多邊形邊界
                var hull = ComputeConvexHull(trees.Select(t => new SoundscapeVertex(t.X, t.Z)));
                if (hull.Count >= 3)
                {
                    var denseForest = CreateConvexPolygonZone(
                        $"茂密林區 #{clusterCounter++}",
                        "Amb_Forest_Dense_Day",
                        hull,
                        cy: 0f,
                        innerRadius: 250f,
                        outerRadius: 850f,
                        baseVolume: 0.85f);
                    denseForest.Attributes |= SoundscapeZoneAttributes.IsAutoGenerated;
                    generated.Add(denseForest);

                    // 伴生隨機鳥鳴發聲點
                    var birds = CreatePointZone(
                        $"林冠飛鳥鳴啼 #{clusterCounter}",
                        "Amb_Forest_Birds",
                        denseForest.CenterWorldX, 0f, denseForest.CenterWorldZ,
                        innerRadius: 120f,
                        outerRadius: 600f,
                        baseVolume: 0.9f);
                    birds.Attributes |= SoundscapeZoneAttributes.IsAutoGenerated;
                    generated.Add(birds);
                }
            }
            else if (trees.Count >= 3)
            {
                // 稀疏林地
                float avgX = trees.Average(t => t.X);
                float avgZ = trees.Average(t => t.Z);
                var lightForest = CreateCircleZone(
                    $"疏林伴隨音 #{clusterCounter++}",
                    "Amb_Forest_Light",
                    avgX, 0f, avgZ,
                    baseRadius: clusterBinWorldSize * 0.4f,
                    innerRadius: 180f,
                    outerRadius: 650f,
                    baseVolume: 0.65f);
                lightForest.Attributes |= SoundscapeZoneAttributes.IsAutoGenerated;
                generated.Add(lightForest);
            }
        }

        return generated;
    }

    /// <summary>
    /// 依據聚落建築物分佈，自動分析並生成生活與生產伴隨音（鐵匠鋪鍛鐵聲、神龕低語、營火）。
    /// </summary>
    public IReadOnlyList<SoundscapeZone> GenerateSettlementSoundscapes(
        IReadOnlyList<(string TypeName, float X, float Y, float Z)> buildings)
    {
        ArgumentNullException.ThrowIfNull(buildings);

        var generated = new List<SoundscapeZone>();
        int count = 1;

        foreach (var b in buildings)
        {
            string t = b.TypeName;
            if (t.Contains("Schmied", StringComparison.OrdinalIgnoreCase) ||
                t.Contains("Blacksmith", StringComparison.OrdinalIgnoreCase))
            {
                var anvil = CreatePointZone(
                    $"鐵砧鍛打聲 #{count++}",
                    "Snd_Blacksmith_Anvil",
                    b.X, b.Y, b.Z,
                    innerRadius: 100f,
                    outerRadius: 450f,
                    baseVolume: 0.85f);
                anvil.Attributes |= SoundscapeZoneAttributes.IsAutoGenerated;
                generated.Add(anvil);
            }
            else if (t.Contains("Tempel", StringComparison.OrdinalIgnoreCase) ||
                     t.Contains("Shrine", StringComparison.OrdinalIgnoreCase) ||
                     t.Contains("Heiligtum", StringComparison.OrdinalIgnoreCase))
            {
                var chant = CreatePointZone(
                    $"神龕祈禱低語 #{count++}",
                    "Snd_Shrine_Chant",
                    b.X, b.Y, b.Z,
                    innerRadius: 120f,
                    outerRadius: 400f,
                    baseVolume: 0.70f);
                chant.Attributes |= SoundscapeZoneAttributes.IsAutoGenerated;
                generated.Add(chant);
            }
            else if (t.Contains("Feuer", StringComparison.OrdinalIgnoreCase) ||
                     t.Contains("Campfire", StringComparison.OrdinalIgnoreCase))
            {
                var fire = CreatePointZone(
                    $"營火劈啪聲 #{count++}",
                    "Snd_Campfire_Crackling",
                    b.X, b.Y, b.Z,
                    innerRadius: 80f,
                    outerRadius: 350f,
                    baseVolume: 0.75f);
                fire.Attributes |= SoundscapeZoneAttributes.IsAutoGenerated;
                generated.Add(fire);
            }
        }

        return generated;
    }

    #endregion

    #region 凸包幾何計算 (Andrew's Monotone Chain 演算法)

    /// <summary>
    /// 使用 Andrew's Monotone Chain 凸包演算法，自一組二維點計算凸多邊形頂點序列（逆時針方向）。
    /// 時間複雜度 O(n log n)，數值穩定性高。
    /// </summary>
    public static List<SoundscapeVertex> ComputeConvexHull(IEnumerable<SoundscapeVertex> points)
    {
        var pts = points.Distinct().OrderBy(p => p.X).ThenBy(p => p.Z).ToList();
        if (pts.Count <= 2)
            return pts;

        var lower = new List<SoundscapeVertex>();
        foreach (var p in pts)
        {
            while (lower.Count >= 2 && CrossProduct(lower[^2], lower[^1], p) <= 0)
                lower.RemoveAt(lower.Count - 1);
            lower.Add(p);
        }

        var upper = new List<SoundscapeVertex>();
        for (int i = pts.Count - 1; i >= 0; i--)
        {
            var p = pts[i];
            while (upper.Count >= 2 && CrossProduct(upper[^2], upper[^1], p) <= 0)
                upper.RemoveAt(upper.Count - 1);
            upper.Add(p);
        }

        lower.RemoveAt(lower.Count - 1);
        upper.RemoveAt(upper.Count - 1);
        lower.AddRange(upper);
        return lower;
    }

    private static float CrossProduct(SoundscapeVertex a, SoundscapeVertex b, SoundscapeVertex c)
    {
        return (b.X - a.X) * (c.Z - a.Z) - (b.Z - a.Z) * (c.X - a.X);
    }

    #endregion
}
