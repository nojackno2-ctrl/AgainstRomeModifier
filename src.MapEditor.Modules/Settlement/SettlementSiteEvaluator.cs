namespace AgainstRomeMapEditor.Modules.Settlement;

/// <summary>
/// 聚落腹地評估器（SettlementSiteEvaluator）：
/// 負責評估候選聚落地點之地形平坦度、距水體安全距離、主屋（Haupthaus）及初始 5 棟核心建築的無碰撞排布佔地空間。
/// </summary>
public sealed class SettlementSiteEvaluator
{
    private readonly int _dimension;
    private readonly float _tileWorldSize;
    private readonly Func<float, float, float> _sampleHeight;
    private readonly Func<int, int, bool> _isBlocked;
    private readonly float _waterLevel;

    public SettlementSiteEvaluator(
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
    /// 評估指定地圖格網坐標 (centerTileX, centerTileZ) 是否適合作為聚落中心，並規劃主屋與周圍 5 棟核心建築。
    /// </summary>
    public SettlementSiteEvaluationResult Evaluate(
        float centerTileX,
        float centerTileZ,
        SettlementTribe tribe,
        int team = 0,
        float orientationAngleDeg = 0f)
    {
        float worldCenterX = centerTileX * _tileWorldSize;
        float worldCenterZ = centerTileZ * _tileWorldSize;

        // 1. 邊界檢查（聚落周邊需至少預留 20 格腹地）
        const float marginTiles = 18f;
        if (centerTileX < marginTiles || centerTileX > _dimension - marginTiles ||
            centerTileZ < marginTiles || centerTileZ > _dimension - marginTiles)
        {
            return new SettlementSiteEvaluationResult(
                IsValid: false, TotalScore: 0, FlatnessScore: 0, WaterSafetyScore: 0,
                SpaceClearanceScore: 0, ExpansionPotentialScore: 0,
                AnchorTileX: centerTileX, AnchorTileZ: centerTileZ,
                WorldX: worldCenterX, WorldZ: worldCenterZ,
                PlannedBuildings: Array.Empty<PlacedBuildingPlan>(),
                RejectionReason: "Too close to map edge");
        }

        // 2. 水體安全評估
        float waterSafetyScore = EvaluateWaterSafety(centerTileX, centerTileZ, out float minWaterDist);
        if (minWaterDist < 6.0f) // 小於 6 格過於接近水體或已有淹水危險
        {
            return new SettlementSiteEvaluationResult(
                IsValid: false, TotalScore: 0, FlatnessScore: 0, WaterSafetyScore: waterSafetyScore,
                SpaceClearanceScore: 0, ExpansionPotentialScore: 0,
                AnchorTileX: centerTileX, AnchorTileZ: centerTileZ,
                WorldX: worldCenterX, WorldZ: worldCenterZ,
                PlannedBuildings: Array.Empty<PlacedBuildingPlan>(),
                RejectionReason: $"Too close to water (distance {minWaterDist:F1} tiles < 6.0)");
        }

        // 3. 取得部族建築藍圖
        var (mainBlueprint, coreBlueprints) = SettlementTribalPresets.GetBlueprints(tribe);

        // 4. 主屋足跡檢驗
        float mainRadius = MathF.Max(mainBlueprint.FootprintWidth, mainBlueprint.FootprintHeight) * 0.5f;
        if (!IsFootprintBuildable(centerTileX, centerTileZ, mainRadius, out float mainHeightDelta, out float avgMainHeight))
        {
            return new SettlementSiteEvaluationResult(
                IsValid: false, TotalScore: 0, FlatnessScore: 0, WaterSafetyScore: waterSafetyScore,
                SpaceClearanceScore: 0, ExpansionPotentialScore: 0,
                AnchorTileX: centerTileX, AnchorTileZ: centerTileZ,
                WorldX: worldCenterX, WorldZ: worldCenterZ,
                PlannedBuildings: Array.Empty<PlacedBuildingPlan>(),
                RejectionReason: $"Main house footprint obstructed or terrain too steep (delta {mainHeightDelta:F2})");
        }

        var plannedBuildings = new List<PlacedBuildingPlan>
        {
            new(mainBlueprint.TypeName, worldCenterX, worldCenterZ, 0f, orientationAngleDeg, team, mainRadius)
        };

        // 5. 核心 5 棟建築佈局規劃（無碰撞、通道保留、適應地形坡度）
        float totalBuildingHeightDelta = mainHeightDelta;
        bool allBuildingsPlaced = true;

        foreach (var bp in coreBlueprints)
        {
            float bpRadius = MathF.Max(bp.FootprintWidth, bp.FootprintHeight) * 0.5f;
            float targetAngle = (bp.PreferredAngleDeg + orientationAngleDeg) % 360f;
            float targetDist = bp.PreferredDistanceToMain;

            if (TryFindClearBuildingSpot(
                centerTileX, centerTileZ, targetDist, targetAngle, bpRadius,
                plannedBuildings, out float spotTileX, out float spotTileZ, out float spotAngle, out float spotHeightDelta))
            {
                plannedBuildings.Add(new PlacedBuildingPlan(
                    bp.TypeName,
                    spotTileX * _tileWorldSize,
                    spotTileZ * _tileWorldSize,
                    0f,
                    spotAngle,
                    team,
                    bpRadius));
                totalBuildingHeightDelta += spotHeightDelta;
            }
            else
            {
                allBuildingsPlaced = false;
                break;
            }
        }

        if (!allBuildingsPlaced || plannedBuildings.Count < 6)
        {
            return new SettlementSiteEvaluationResult(
                IsValid: false, TotalScore: 0, FlatnessScore: 0, WaterSafetyScore: waterSafetyScore,
                SpaceClearanceScore: 0, ExpansionPotentialScore: 0,
                AnchorTileX: centerTileX, AnchorTileZ: centerTileZ,
                WorldX: worldCenterX, WorldZ: worldCenterZ,
                PlannedBuildings: plannedBuildings,
                RejectionReason: "Insufficient clear space to pack 5 core settlement buildings");
        }

        // 6. 計算平坦度得分（以平均高差為基準）
        float avgHeightDelta = totalBuildingHeightDelta / plannedBuildings.Count;
        float flatnessScore = Math.Clamp(100f - avgHeightDelta * 15f, 0f, 100f);

        // 7. 計算腹地擴充潛力（外圍 16 格半徑內的平坦無障礙比率）
        float expansionScore = EvaluateExpansionPotential(centerTileX, centerTileZ, 16f);

        // 8. 空間排布緊湊與通路充裕度得分
        float spaceClearanceScore = 95f - (plannedBuildings.Count - 6) * 5f;

        // 9. 總分加權合成
        float totalScore = flatnessScore * 0.35f +
                           waterSafetyScore * 0.25f +
                           spaceClearanceScore * 0.20f +
                           expansionScore * 0.20f;

        return new SettlementSiteEvaluationResult(
            IsValid: true,
            TotalScore: totalScore,
            FlatnessScore: flatnessScore,
            WaterSafetyScore: waterSafetyScore,
            SpaceClearanceScore: spaceClearanceScore,
            ExpansionPotentialScore: expansionScore,
            AnchorTileX: centerTileX,
            AnchorTileZ: centerTileZ,
            WorldX: worldCenterX,
            WorldZ: worldCenterZ,
            PlannedBuildings: plannedBuildings);
    }

    /// <summary>
    /// 在主屋周邊尋找無碰撞、具備安全間距與平坦地形之建築擺放點。
    /// </summary>
    private bool TryFindClearBuildingSpot(
        float centerTileX,
        float centerTileZ,
        float preferredDist,
        float preferredAngleDeg,
        float footprintRadius,
        IReadOnlyList<PlacedBuildingPlan> existingBuildings,
        out float bestTileX,
        out float bestTileZ,
        out float bestAngle,
        out float bestHeightDelta)
    {
        bestTileX = 0;
        bestTileZ = 0;
        bestAngle = 0;
        bestHeightDelta = float.MaxValue;

        // 角度偏移候選（從偏好角度逐步向兩側展開）
        float[] angleOffsets = [0f, 15f, -15f, 30f, -30f, 45f, -45f, 60f, -60f, 75f, -75f, 90f, -90f];
        // 距離微調候選
        float[] distOffsets = [0f, 0.8f, -0.8f, 1.5f, -1.2f];

        const float minCorridorClearance = 1.2f; // 建築間走廊間隔

        foreach (float dOffset in distOffsets)
        {
            float dist = preferredDist + dOffset;
            if (dist < 3.0f) continue;

            foreach (float aOffset in angleOffsets)
            {
                float angleDeg = (preferredAngleDeg + aOffset + 360f) % 360f;
                float rad = angleDeg * MathF.PI / 180f;

                float candX = centerTileX + MathF.Cos(rad) * dist;
                float candZ = centerTileZ + MathF.Sin(rad) * dist;

                // 邊界檢查
                if (candX < 4 || candX > _dimension - 5 || candZ < 4 || candZ > _dimension - 5) continue;

                // 檢驗足跡地形平坦與無阻擋
                if (!IsFootprintBuildable(candX, candZ, footprintRadius, out float heightDelta, out _)) continue;

                // 檢驗與既有建築之碰撞（半徑加和 + 走廊）
                bool collides = false;
                foreach (var placed in existingBuildings)
                {
                    float placedTileX = placed.WorldX / _tileWorldSize;
                    float placedTileZ = placed.WorldZ / _tileWorldSize;
                    float minDist = placed.FootprintRadius + footprintRadius + minCorridorClearance;
                    float dx = candX - placedTileX;
                    float dz = candZ - placedTileZ;
                    if (dx * dx + dz * dz < minDist * minDist)
                    {
                        collides = true;
                        break;
                    }
                }

                if (!collides)
                {
                    bestTileX = candX;
                    bestTileZ = candZ;
                    bestAngle = angleDeg;
                    bestHeightDelta = heightDelta;
                    return true;
                }
            }
        }

        return false;
    }

    /// <summary>
    /// 檢驗指定圓形區域是否可建造（無碰撞標記、未被淹水、坡度高差在允許閾值內）。
    /// </summary>
    private bool IsFootprintBuildable(float tileX, float tileZ, float radius, out float heightDelta, out float avgHeight)
    {
        heightDelta = 0f;
        avgHeight = 0f;

        int minX = (int)MathF.Floor(tileX - radius);
        int maxX = (int)MathF.Ceiling(tileX + radius);
        int minZ = (int)MathF.Floor(tileZ - radius);
        int maxZ = (int)MathF.Ceiling(tileZ + radius);

        float minH = float.MaxValue;
        float maxH = float.MinValue;
        float sumH = 0f;
        int count = 0;

        for (int z = minZ; z <= maxZ; z++)
        {
            for (int x = minX; x <= maxX; x++)
            {
                if (x < 0 || x >= _dimension || z < 0 || z >= _dimension)
                {
                    heightDelta = float.MaxValue;
                    return false;
                }

                float dx = x - tileX;
                float dz = z - tileZ;
                if (dx * dx + dz * dz > radius * radius) continue;

                // 阻擋標記檢查
                if (_isBlocked(x, z))
                {
                    heightDelta = float.MaxValue;
                    return false;
                }

                // 高度取樣
                float h = _sampleHeight(x * _tileWorldSize, z * _tileWorldSize);
                if (h < _waterLevel + 0.8f) // 淹水區域
                {
                    heightDelta = float.MaxValue;
                    return false;
                }

                minH = MathF.Min(minH, h);
                maxH = MathF.Max(maxH, h);
                sumH += h;
                count++;
            }
        }

        if (count == 0) return false;

        heightDelta = maxH - minH;
        avgHeight = sumH / count;

        // 若半徑內高差超過 5.0，視為大斜坡不可建
        return heightDelta <= 5.0f;
    }

    /// <summary>
    /// 評估聚落與水體的安全距離與安全得分。
    /// </summary>
    private float EvaluateWaterSafety(float centerTileX, float centerTileZ, out float minWaterDist)
    {
        minWaterDist = float.MaxValue;
        const int scanRadius = 18;

        for (int dz = -scanRadius; dz <= scanRadius; dz += 2)
        {
            for (int dx = -scanRadius; dx <= scanRadius; dx += 2)
            {
                int x = (int)MathF.Round(centerTileX + dx);
                int z = (int)MathF.Round(centerTileZ + dz);
                if (x < 0 || x >= _dimension || z < 0 || z >= _dimension) continue;

                float h = _sampleHeight(x * _tileWorldSize, z * _tileWorldSize);
                if (h <= _waterLevel + 0.5f)
                {
                    float dist = MathF.Sqrt(dx * dx + dz * dz);
                    minWaterDist = MathF.Min(minWaterDist, dist);
                }
            }
        }

        if (minWaterDist < 6.0f) return 0f;
        if (minWaterDist >= 18.0f) return 90f; // 充足安全距離

        // 6 到 18 格之間線性平滑計分
        return 50f + (minWaterDist - 6.0f) / 12.0f * 45f;
    }

    /// <summary>
    /// 評估半徑內平坦可擴展空地之比例。
    /// </summary>
    private float EvaluateExpansionPotential(float centerTileX, float centerTileZ, float radius)
    {
        int validCount = 0;
        int totalSampled = 0;
        int r = (int)MathF.Ceiling(radius);

        for (int dz = -r; dz <= r; dz += 2)
        {
            for (int dx = -r; dx <= r; dx += 2)
            {
                int x = (int)MathF.Round(centerTileX + dx);
                int z = (int)MathF.Round(centerTileZ + dz);
                if (x < 0 || x >= _dimension || z < 0 || z >= _dimension) continue;
                if (dx * dx + dz * dz > radius * radius) continue;

                totalSampled++;
                if (!_isBlocked(x, z))
                {
                    float h = _sampleHeight(x * _tileWorldSize, z * _tileWorldSize);
                    if (h > _waterLevel + 1.0f)
                    {
                        validCount++;
                    }
                }
            }
        }

        return totalSampled > 0 ? (float)validCount / totalSampled * 100f : 0f;
    }
}
