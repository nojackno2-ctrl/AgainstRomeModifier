namespace AgainstRomeMapEditor.Modules.Settlement;

/// <summary>
/// 資源聚落規劃器（ResourceClusterPlanner）：
/// 負責在聚落週邊規劃木材資源（自然森林群聚）、岩壁坡腳採石場與石礦露頭、以及開闊平地糧食/野生動物群。
/// </summary>
public sealed class ResourceClusterPlanner
{
    private readonly int _dimension;
    private readonly float _tileWorldSize;
    private readonly Func<float, float, float> _sampleHeight;
    private readonly Func<int, int, bool> _isBlocked;
    private readonly float _waterLevel;

    public ResourceClusterPlanner(
        int dimension,
        Func<float, float, float> sampleHeight,
        Func<int, int, bool> isBlocked,
        float waterLevel,
        float tileWorldSize = 64f)
    {
        _dimension = dimension > 0 ? dimension : throw new ArgumentOutOfRangeException(nameof(dimension));
        _sampleHeight = sampleHeight ?? throw new ArgumentNullException(nameof(sampleHeight));
        _isBlocked = isBlocked ?? throw new ArgumentNullException(nameof(isBlocked));
        _waterLevel = waterLevel;
        _tileWorldSize = tileWorldSize > 0 ? tileWorldSize : 64f;
    }

    /// <summary>
    /// 為指定聚落規劃週邊的完整資源群（森林、採石場、糧食狩獵）。
    /// </summary>
    public (IReadOnlyList<NatureClusterItem> ForestTrees,
            IReadOnlyList<NatureClusterItem> StoneQuarries,
            IReadOnlyList<PlacedResourceItem> Wildlife)
        PlanClusters(
            float centerTileX,
            float centerTileZ,
            SettlementTribe tribe,
            int seed = 1337,
            float baseAngleOffsetDeg = 0f)
    {
        var random = new Random(seed);

        // 1. 規劃木材資源（森林）——位於聚落 16-28 格，佔據約 60-90 度扇區
        float forestAngleDeg = (baseAngleOffsetDeg + 30f + (float)random.NextDouble() * 30f) % 360f;
        var forest = PlanForestCluster(centerTileX, centerTileZ, forestAngleDeg, tribe, random);

        // 2. 規劃採石場與石礦——優先尋找鄰近岩壁坡腳，角度與森林錯開
        float stonePreferredAngleDeg = (forestAngleDeg + 120f + (float)random.NextDouble() * 40f) % 360f;
        var stone = PlanStoneQuarryCluster(centerTileX, centerTileZ, stonePreferredAngleDeg, tribe, random);

        // 3. 規劃糧食/野生動物狩獵區——在剩餘方位之開闊平坦草地
        float foodPreferredAngleDeg = (forestAngleDeg + 220f + (float)random.NextDouble() * 50f) % 360f;
        var wildlife = PlanWildlifeHerd(centerTileX, centerTileZ, foodPreferredAngleDeg, random);

        return (forest, stone, wildlife);
    }

    /// <summary>
    /// 木材資源群聚生成演算法：在聚落 15-30 格範圍內，結合泊松取樣與柏林雜訊遮罩，生成自然多樣的樹林。
    /// </summary>
    public IReadOnlyList<NatureClusterItem> PlanForestCluster(
        float centerTileX,
        float centerTileZ,
        float directionAngleDeg,
        SettlementTribe tribe,
        Random random,
        int targetTreeCount = 45)
    {
        var palette = SettlementTribalPresets.GetForestTreePalette(tribe);
        var trees = new List<NatureClusterItem>();

        // 森林重心錨點（距離中心 20 格）
        float rad = directionAngleDeg * MathF.PI / 180f;
        float forestCenterTileX = centerTileX + MathF.Cos(rad) * 21f;
        float forestCenterTileZ = centerTileZ + MathF.Sin(rad) * 21f;

        const float minSpacing = 0.85f; // 樹木間最小自然間距
        var occupiedPoints = new List<(float X, float Z)>();

        // 柏林雜訊參數
        float noiseScale = 0.12f;
        float noiseOffsetX = (float)random.NextDouble() * 1000f;
        float noiseOffsetZ = (float)random.NextDouble() * 1000f;

        // 在森林重心半徑 12 格內採樣候選格點
        int scanR = 12;
        int minX = Math.Max(2, (int)MathF.Floor(forestCenterTileX - scanR));
        int maxX = Math.Min(_dimension - 3, (int)MathF.Ceiling(forestCenterTileX + scanR));
        int minZ = Math.Max(2, (int)MathF.Floor(forestCenterTileZ - scanR));
        int maxZ = Math.Min(_dimension - 3, (int)MathF.Ceiling(forestCenterTileZ + scanR));

        var candidates = new List<(float X, float Z, float Density)>();

        for (int z = minZ; z <= maxZ; z++)
        {
            for (int x = minX; x <= maxX; x++)
            {
                float dx = x - forestCenterTileX;
                float dz = z - forestCenterTileZ;
                float distToClusterCenter = MathF.Sqrt(dx * dx + dz * dz);
                if (distToClusterCenter > scanR) continue;

                // 檢查與聚落主體的距離（必須保持在 14 格以外，避免堵死主屋腹地）
                float distToSettlement = MathF.Sqrt(MathF.Pow(x - centerTileX, 2) + MathF.Pow(z - centerTileZ, 2));
                if (distToSettlement < 13.5f || distToSettlement > 32f) continue;

                if (_isBlocked(x, z)) continue;

                float h = _sampleHeight(x * _tileWorldSize, z * _tileWorldSize);
                if (h <= _waterLevel + 0.8f) continue; // 不在水面種樹

                // 計算柏林雜訊與距離衰減
                float n = SamplePerlinNoise(x * noiseScale + noiseOffsetX, z * noiseScale + noiseOffsetZ);
                float radialFalloff = 1.0f - (distToClusterCenter / scanR);
                float density = (n * 0.6f + radialFalloff * 0.4f);

                if (density > 0.35f)
                {
                    candidates.Add((x, z, density));
                }
            }
        }

        // 按照密度排序並做泊松間距過濾
        candidates = candidates.OrderByDescending(c => c.Density).ToList();

        foreach (var cand in candidates)
        {
            if (trees.Count >= targetTreeCount) break;

            // 隨機抖動位置（0.15~0.85 格）
            float jitterX = cand.X + 0.15f + (float)random.NextDouble() * 0.7f;
            float jitterZ = cand.Z + 0.15f + (float)random.NextDouble() * 0.7f;

            // 泊松間隙檢查
            bool tooClose = false;
            foreach (var pt in occupiedPoints)
            {
                float distSq = MathF.Pow(jitterX - pt.X, 2) + MathF.Pow(jitterZ - pt.Z, 2);
                if (distSq < minSpacing * minSpacing)
                {
                    tooClose = true;
                    break;
                }
            }
            if (tooClose) continue;

            // 決定樹種（林心以高大針葉/闊葉樹為主，林邊以灌木過渡）
            string typeName;
            if (cand.Density < 0.45f && palette.Count > 4)
            {
                // 林緣灌木
                typeName = palette[^1];
            }
            else
            {
                // 主幹喬木
                int mainIdx = random.Next(0, Math.Min(3, palette.Count));
                typeName = palette[mainIdx];
            }

            float rotDeg = (float)random.NextDouble() * 360f;
            occupiedPoints.Add((jitterX, jitterZ));
            trees.Add(new NatureClusterItem(
                typeName,
                jitterX * _tileWorldSize,
                jitterZ * _tileWorldSize,
                0f,
                rotDeg));
        }

        return trees;
    }

    /// <summary>
    /// 採石場與石礦資源規劃：沿鄰近岩壁坡腳（Cliff Foot）偵測平地與斜坡交界處，群聚生成石礦資源點。
    /// </summary>
    public IReadOnlyList<NatureClusterItem> PlanStoneQuarryCluster(
        float centerTileX,
        float centerTileZ,
        float preferredAngleDeg,
        SettlementTribe tribe,
        Random random,
        int targetRockCount = 7)
    {
        var palette = SettlementTribalPresets.GetStoneQuarryPalette(tribe);
        var rocks = new List<NatureClusterItem>();

        // 1. 尋找坡腳（Cliff Foot）候選點
        if (!TryFindCliffFootAnchor(centerTileX, centerTileZ, preferredAngleDeg, out float anchorTileX, out float anchorTileZ))
        {
            // 若為完全平坦地形，則以偏好距離 22 格作為露頭點
            float rad = preferredAngleDeg * MathF.PI / 180f;
            anchorTileX = Math.Clamp(centerTileX + MathF.Cos(rad) * 22f, 10f, _dimension - 11f);
            anchorTileZ = Math.Clamp(centerTileZ + MathF.Sin(rad) * 22f, 10f, _dimension - 11f);
        }

        var occupied = new List<(float X, float Z)>();
        const float rockSpacing = 1.3f;

        // 在採石點半徑 4.5 格內生成石礦堆
        for (int attempt = 0; attempt < 40 && rocks.Count < targetRockCount; attempt++)
        {
            float r = 0.5f + (float)random.NextDouble() * 4.0f;
            float angle = (float)random.NextDouble() * MathF.PI * 2f;
            float candX = anchorTileX + MathF.Cos(angle) * r;
            float candZ = anchorTileZ + MathF.Sin(angle) * r;

            if (candX < 3 || candX > _dimension - 4 || candZ < 3 || candZ > _dimension - 4) continue;
            if (_isBlocked((int)candX, (int)candZ)) continue;

            float h = _sampleHeight(candX * _tileWorldSize, candZ * _tileWorldSize);
            if (h <= _waterLevel + 0.5f) continue;

            bool tooClose = occupied.Any(pt => MathF.Pow(candX - pt.X, 2) + MathF.Pow(candZ - pt.Z, 2) < rockSpacing * rockSpacing);
            if (tooClose) continue;

            occupied.Add((candX, candZ));
            string rockType = palette[random.Next(0, palette.Count)];
            float rotDeg = (float)random.NextDouble() * 360f;

            rocks.Add(new NatureClusterItem(
                rockType,
                candX * _tileWorldSize,
                candZ * _tileWorldSize,
                0f,
                rotDeg));
        }

        return rocks;
    }

    /// <summary>
    /// 糧食與狩獵區規劃：在開闊平坦草地生成 3-6 隻中立野生動物（如鹿群或野豬群）。
    /// </summary>
    public IReadOnlyList<PlacedResourceItem> PlanWildlifeHerd(
        float centerTileX,
        float centerTileZ,
        float preferredAngleDeg,
        Random random,
        int herdSize = 4)
    {
        var wildlifePalette = SettlementTribalPresets.GetWildlifePalette();
        var herd = new List<PlacedResourceItem>();

        float rad = preferredAngleDeg * MathF.PI / 180f;
        float pastureCenterX = centerTileX + MathF.Cos(rad) * 18f;
        float pastureCenterZ = centerTileZ + MathF.Sin(rad) * 18f;

        pastureCenterX = Math.Clamp(pastureCenterX, 10f, _dimension - 11f);
        pastureCenterZ = Math.Clamp(pastureCenterZ, 10f, _dimension - 11f);

        string animalType = wildlifePalette[random.Next(0, wildlifePalette.Count)];
        var occupied = new List<(float X, float Z)>();
        const float animalSpacing = 2.0f;

        for (int i = 0; i < 30 && herd.Count < herdSize; i++)
        {
            float r = 0.8f + (float)random.NextDouble() * 5.0f;
            float angle = (float)random.NextDouble() * MathF.PI * 2f;
            float candX = pastureCenterX + MathF.Cos(angle) * r;
            float candZ = pastureCenterZ + MathF.Sin(angle) * r;

            if (candX < 3 || candX > _dimension - 4 || candZ < 3 || candZ > _dimension - 4) continue;
            if (_isBlocked((int)candX, (int)candZ)) continue;

            float h = _sampleHeight(candX * _tileWorldSize, candZ * _tileWorldSize);
            if (h <= _waterLevel + 1.0f) continue;

            bool tooClose = occupied.Any(pt => MathF.Pow(candX - pt.X, 2) + MathF.Pow(candZ - pt.Z, 2) < animalSpacing * animalSpacing);
            if (tooClose) continue;

            occupied.Add((candX, candZ));
            float rotDeg = (float)random.NextDouble() * 360f;

            // 野生動物通常以 Figure 類別放置，中立隊伍 (-1)，UnitCount=1
            herd.Add(new PlacedResourceItem(
                animalType,
                candX * _tileWorldSize,
                candZ * _tileWorldSize,
                0f,
                rotDeg,
                Team: -1,
                Count: 1));
        }

        return herd;
    }

    /// <summary>
    /// 在指定角度周圍 16-32 格掃描尋找天然岩壁坡腳（平地緊鄰陡峭斜坡）。
    /// </summary>
    private bool TryFindCliffFootAnchor(
        float centerTileX,
        float centerTileZ,
        float preferredAngleDeg,
        out float anchorTileX,
        out float anchorTileZ)
    {
        anchorTileX = 0;
        anchorTileZ = 0;
        float bestCliffScore = -1f;

        for (float dist = 16f; dist <= 30f; dist += 2.5f)
        {
            for (float dAngle = -45f; dAngle <= 45f; dAngle += 15f)
            {
                float angle = (preferredAngleDeg + dAngle + 360f) % 360f;
                float rad = angle * MathF.PI / 180f;

                float x = centerTileX + MathF.Cos(rad) * dist;
                float z = centerTileZ + MathF.Sin(rad) * dist;

                if (x < 5 || x > _dimension - 6 || z < 5 || z > _dimension - 6) continue;
                if (_isBlocked((int)x, (int)z)) continue;

                float centerH = _sampleHeight(x * _tileWorldSize, z * _tileWorldSize);
                if (centerH <= _waterLevel + 1.0f) continue;

                // 檢查附近 2 格內是否有顯著高度爬升（坡腳特徵：自身平坦，鄰近有顯著高差）
                float maxNeighborH = centerH;
                for (int dz = -2; dz <= 2; dz += 2)
                {
                    for (int dx = -2; dx <= 2; dx += 2)
                    {
                        float nh = _sampleHeight((x + dx) * _tileWorldSize, (z + dz) * _tileWorldSize);
                        maxNeighborH = MathF.Max(maxNeighborH, nh);
                    }
                }

                float cliffHeight = maxNeighborH - centerH;
                // 若鄰近有 3.5 單位以上的高差，表示此處為坡腳
                if (cliffHeight > 3.0f && cliffHeight > bestCliffScore)
                {
                    bestCliffScore = cliffHeight;
                    anchorTileX = x;
                    anchorTileZ = z;
                }
            }
        }

        return bestCliffScore > 0;
    }

    /// <summary>
    /// 輕量化 2D 雜訊函數（用於樹林自然群聚）。
    /// </summary>
    private static float SamplePerlinNoise(float x, float y)
    {
        int x0 = (int)MathF.Floor(x);
        int x1 = x0 + 1;
        int y0 = (int)MathF.Floor(y);
        int y1 = y0 + 1;

        float sx = x - x0;
        float sy = y - y0;

        // 平滑漸變曲線 3s^2 - 2s^3
        float u = sx * sx * (3f - 2f * sx);
        float v = sy * sy * (3f - 2f * sy);

        float n0 = DotGridGradient(x0, y0, x, y);
        float n1 = DotGridGradient(x1, y0, x, y);
        float ix0 = n0 + u * (n1 - n0);

        n0 = DotGridGradient(x0, y1, x, y);
        n1 = DotGridGradient(x1, y1, x, y);
        float ix1 = n0 + u * (n1 - n0);

        float val = ix0 + v * (ix1 - ix0);
        return Math.Clamp((val + 1f) * 0.5f, 0f, 1f);
    }

    private static float DotGridGradient(int ix, int iy, float x, float y)
    {
        // 簡單偽隨機梯度向量
        int hash = (ix * 374761393 + iy * 668265263) ^ 0x5bf03635;
        hash = (hash ^ (hash >> 13)) * 1274126177;
        float angle = (hash & 0xFFFF) / 65536f * MathF.PI * 2f;

        float dx = x - ix;
        float dy = y - iy;
        return dx * MathF.Cos(angle) + dy * MathF.Sin(angle);
    }
}
