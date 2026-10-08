namespace AgainstRomeMapEditor.Modules.Settlement;

/// <summary>
/// 多勢力平衡分配演算法（MultiplayerFairnessBalancer）：
/// 支援 2-8 玩家對稱或公平距離分佈（中心對稱、旋轉對稱、拓撲等距），並提供資源配額公平校驗報告。
/// </summary>
public sealed class MultiplayerFairnessBalancer
{
    private readonly SettlementSiteEvaluator _evaluator;
    private readonly ResourceClusterPlanner _planner;
    private readonly int _dimension;
    private readonly float _tileWorldSize;

    public int Dimension => _dimension;
    public float TileWorldSize => _tileWorldSize;

    public MultiplayerFairnessBalancer(
        SettlementSiteEvaluator evaluator,
        ResourceClusterPlanner planner,
        int dimension = 256,
        float tileWorldSize = 64f)
    {
        _evaluator = evaluator ?? throw new ArgumentNullException(nameof(evaluator));
        _planner = planner ?? throw new ArgumentNullException(nameof(planner));
        _dimension = dimension;
        _tileWorldSize = tileWorldSize;
    }

    /// <summary>
    /// 根據指定之玩家人數、對稱模式與部族分配，生成多勢力平衡分佈佈局。
    /// </summary>
    public MultiplayerDistributionResult Generate(
        int playerCount,
        SymmetryMode mode,
        IReadOnlyList<SettlementTribe>? tribes = null,
        int baseSeed = 42,
        float spawnRadiusRatio = 0.60f)
    {
        if (playerCount is < 2 or > 8)
            throw new ArgumentOutOfRangeException(nameof(playerCount), "Player count must be between 2 and 8.");

        var tribeList = tribes ?? Enumerable.Repeat(SettlementTribe.Germanic, playerCount).ToArray();
        if (tribeList.Count < playerCount)
            throw new ArgumentException("Tribes list count must match or exceed player count.", nameof(tribes));

        float centerTile = _dimension * 0.5f;
        float spawnRadiusTiles = centerTile * Math.Clamp(spawnRadiusRatio, 0.4f, 0.75f);

        var playerLayouts = new List<PlayerSettlementLayout>();

        switch (mode)
        {
            case SymmetryMode.CentralSymmetry when playerCount == 2:
                playerLayouts.AddRange(GenerateCentralSymmetry2P(centerTile, spawnRadiusTiles, tribeList, baseSeed));
                break;

            case SymmetryMode.RotationalSymmetry:
            default:
                if (mode == SymmetryMode.CentralSymmetry && playerCount > 2)
                {
                    // 3人以上自動轉為旋轉對稱
                    mode = SymmetryMode.RotationalSymmetry;
                }
                if (mode == SymmetryMode.RotationalSymmetry)
                {
                    playerLayouts.AddRange(GenerateRotationalSymmetry(playerCount, centerTile, spawnRadiusTiles, tribeList, baseSeed));
                }
                else
                {
                    playerLayouts.AddRange(GenerateTopologicalEquidistant(playerCount, centerTile, spawnRadiusTiles, tribeList, baseSeed));
                }
                break;
        }

        var report = ComputeFairnessReport(playerLayouts);

        return new MultiplayerDistributionResult(
            Mode: mode,
            PlayerCount: playerCount,
            Players: playerLayouts,
            FairnessReport: report);
    }

    /// <summary>
    /// 2 玩家絕對中心對稱生成（Point Symmetry across map center）。
    /// </summary>
    private IEnumerable<PlayerSettlementLayout> GenerateCentralSymmetry2P(
        float centerTile,
        float spawnRadiusTiles,
        IReadOnlyList<SettlementTribe> tribes,
        int seed)
    {
        var random = new Random(seed);
        float angleRad = (float)(random.NextDouble() * Math.PI); // 0 to 180 deg
        float dx = MathF.Cos(angleRad) * spawnRadiusTiles;
        float dz = MathF.Sin(angleRad) * spawnRadiusTiles;

        // Player 1
        float p1X = centerTile + dx;
        float p1Z = centerTile + dz;
        var eval1 = _evaluator.Evaluate(p1X, p1Z, tribes[0], team: 0, orientationAngleDeg: angleRad * 180f / MathF.PI);

        // Player 2（點對稱：P2 = 2*Center - P1）
        float p2X = centerTile - dx;
        float p2Z = centerTile - dz;
        var eval2 = _evaluator.Evaluate(p2X, p2Z, tribes[1], team: 1, orientationAngleDeg: (angleRad * 180f / MathF.PI + 180f) % 360f);

        // 若直接候選點不可行，以步進搜尋最近合法對稱點
        if (!eval1.IsValid || !eval2.IsValid)
        {
            (eval1, eval2) = SearchSymmetricPair(centerTile, spawnRadiusTiles, tribes, angleRad);
        }

        var (f1, s1, w1) = _planner.PlanClusters(eval1.AnchorTileX, eval1.AnchorTileZ, tribes[0], seed + 1, angleRad * 180f / MathF.PI);
        var (f2, s2, w2) = _planner.PlanClusters(eval2.AnchorTileX, eval2.AnchorTileZ, tribes[1], seed + 2, (angleRad * 180f / MathF.PI + 180f) % 360f);

        yield return new PlayerSettlementLayout(0, tribes[0], eval1, f1, s1, w1, eval1.PlannedBuildings);
        yield return new PlayerSettlementLayout(1, tribes[1], eval2, f2, s2, w2, eval2.PlannedBuildings);
    }

    /// <summary>
    /// 3-8 玩家環狀旋轉對稱生成（Rotational Symmetry）。
    /// </summary>
    private IEnumerable<PlayerSettlementLayout> GenerateRotationalSymmetry(
        int count,
        float centerTile,
        float spawnRadiusTiles,
        IReadOnlyList<SettlementTribe> tribes,
        int seed)
    {
        var random = new Random(seed);
        float baseAngle = (float)random.NextDouble() * (MathF.PI * 2f / count);

        for (int i = 0; i < count; i++)
        {
            float angle = baseAngle + i * (MathF.PI * 2f / count);
            float tileX = centerTile + MathF.Cos(angle) * spawnRadiusTiles;
            float tileZ = centerTile + MathF.Sin(angle) * spawnRadiusTiles;
            float angleDeg = angle * 180f / MathF.PI;

            var eval = _evaluator.Evaluate(tileX, tileZ, tribes[i], team: i, orientationAngleDeg: angleDeg);
            if (!eval.IsValid)
            {
                // 局部搜尋最近有效點
                eval = SearchNearValidSite(tileX, tileZ, tribes[i], team: i, angleDeg);
            }

            var (f, s, w) = _planner.PlanClusters(eval.AnchorTileX, eval.AnchorTileZ, tribes[i], seed + i * 7 + 1, angleDeg);
            yield return new PlayerSettlementLayout(i, tribes[i], eval, f, s, w, eval.PlannedBuildings);
        }
    }

    /// <summary>
    /// 拓撲等距生成：在非對稱/自然地形中，利用多起點 Lloyd's 鬆弛與排斥場，達到最大化玩家間最小安全距離。
    /// </summary>
    private IEnumerable<PlayerSettlementLayout> GenerateTopologicalEquidistant(
        int count,
        float centerTile,
        float spawnRadiusTiles,
        IReadOnlyList<SettlementTribe> tribes,
        int seed)
    {
        var random = new Random(seed);
        var positions = new List<(float X, float Z)>();

        // 初始化環狀位置
        for (int i = 0; i < count; i++)
        {
            float a = i * (MathF.PI * 2f / count);
            positions.Add((centerTile + MathF.Cos(a) * spawnRadiusTiles, centerTile + MathF.Sin(a) * spawnRadiusTiles));
        }

        // 執行 5 次鬆弛排斥迭代
        for (int iter = 0; iter < 5; iter++)
        {
            for (int i = 0; i < count; i++)
            {
                float fx = 0, fz = 0;
                for (int j = 0; j < count; j++)
                {
                    if (i == j) continue;
                    float dx = positions[i].X - positions[j].X;
                    float dz = positions[i].Z - positions[j].Z;
                    float dist = MathF.Sqrt(dx * dx + dz * dz);
                    if (dist > 0.01f)
                    {
                        float force = 100f / (dist * dist);
                        fx += (dx / dist) * force;
                        fz += (dz / dist) * force;
                    }
                }

                // 牽引力向 spawnRadiusTiles 軌道
                float cdx = positions[i].X - centerTile;
                float cdz = positions[i].Z - centerTile;
                float currentR = MathF.Sqrt(cdx * cdx + cdz * cdz);
                if (currentR > 0.01f)
                {
                    float rError = spawnRadiusTiles - currentR;
                    fx += (cdx / currentR) * (rError * 0.1f);
                    fz += (cdz / currentR) * (rError * 0.1f);
                }

                positions[i] = (
                    Math.Clamp(positions[i].X + fx, 20f, _dimension - 21f),
                    Math.Clamp(positions[i].Z + fz, 20f, _dimension - 21f)
                );
            }
        }

        // 評估與聚落建構
        for (int i = 0; i < count; i++)
        {
            float tx = positions[i].X;
            float tz = positions[i].Z;
            float angleDeg = MathF.Atan2(tz - centerTile, tx - centerTile) * 180f / MathF.PI;

            var eval = _evaluator.Evaluate(tx, tz, tribes[i], team: i, orientationAngleDeg: angleDeg);
            if (!eval.IsValid)
            {
                eval = SearchNearValidSite(tx, tz, tribes[i], team: i, angleDeg);
            }

            var (f, s, w) = _planner.PlanClusters(eval.AnchorTileX, eval.AnchorTileZ, tribes[i], seed + i * 11 + 3, angleDeg);
            yield return new PlayerSettlementLayout(i, tribes[i], eval, f, s, w, eval.PlannedBuildings);
        }
    }

    private (SettlementSiteEvaluationResult, SettlementSiteEvaluationResult) SearchSymmetricPair(
        float centerTile,
        float spawnRadiusTiles,
        IReadOnlyList<SettlementTribe> tribes,
        float initialAngleRad)
    {
        for (int step = 1; step <= 12; step++)
        {
            float dAngle = step * (MathF.PI / 24f);
            foreach (float sign in new[] { 1f, -1f })
            {
                float a = initialAngleRad + sign * dAngle;
                float dx = MathF.Cos(a) * spawnRadiusTiles;
                float dz = MathF.Sin(a) * spawnRadiusTiles;

                var e1 = _evaluator.Evaluate(centerTile + dx, centerTile + dz, tribes[0], team: 0, orientationAngleDeg: a * 180f / MathF.PI);
                var e2 = _evaluator.Evaluate(centerTile - dx, centerTile - dz, tribes[1], team: 1, orientationAngleDeg: (a * 180f / MathF.PI + 180f) % 360f);

                if (e1.IsValid && e2.IsValid) return (e1, e2);
            }
        }

        // 回退原始評估（即使包含警示）
        float defDx = MathF.Cos(initialAngleRad) * spawnRadiusTiles;
        float defDz = MathF.Sin(initialAngleRad) * spawnRadiusTiles;
        return (
            _evaluator.Evaluate(centerTile + defDx, centerTile + defDz, tribes[0], team: 0),
            _evaluator.Evaluate(centerTile - defDx, centerTile - defDz, tribes[1], team: 1)
        );
    }

    private SettlementSiteEvaluationResult SearchNearValidSite(
        float tileX,
        float tileZ,
        SettlementTribe tribe,
        int team,
        float angleDeg)
    {
        for (float r = 2f; r <= 14f; r += 2f)
        {
            for (float a = 0; a < 360f; a += 30f)
            {
                float rad = a * MathF.PI / 180f;
                float cx = Math.Clamp(tileX + MathF.Cos(rad) * r, 20f, _dimension - 21f);
                float cz = Math.Clamp(tileZ + MathF.Sin(rad) * r, 20f, _dimension - 21f);

                var res = _evaluator.Evaluate(cx, cz, tribe, team, angleDeg);
                if (res.IsValid) return res;
            }
        }

        // 回退原始
        return _evaluator.Evaluate(tileX, tileZ, tribe, team, angleDeg);
    }

    private static FairnessScoreReport ComputeFairnessReport(IReadOnlyList<PlayerSettlementLayout> layouts)
    {
        if (layouts.Count < 2)
        {
            return new FairnessScoreReport(0, 0, 0, 0, 0, 0, true);
        }

        var distances = new List<float>();
        for (int i = 0; i < layouts.Count; i++)
        {
            for (int j = i + 1; j < layouts.Count; j++)
            {
                float dx = layouts[i].SiteEvaluation.WorldX - layouts[j].SiteEvaluation.WorldX;
                float dz = layouts[i].SiteEvaluation.WorldZ - layouts[j].SiteEvaluation.WorldZ;
                distances.Add(MathF.Sqrt(dx * dx + dz * dz));
            }
        }

        float avgDist = distances.Average();
        float minDist = distances.Min();
        float maxDist = distances.Max();

        // 資源總量方差
        float woodVar = ComputeVariance(layouts.Select(l => (float)l.ForestTrees.Count));
        float stoneVar = ComputeVariance(layouts.Select(l => (float)l.StoneQuarries.Count));
        float foodVar = ComputeVariance(layouts.Select(l => (float)l.WildlifeAndFood.Count));

        bool isBalanced = (maxDist - minDist) / Math.Max(1f, avgDist) < 0.35f &&
                          woodVar <= 9.0f &&
                          stoneVar <= 2.0f &&
                          foodVar <= 1.0f;

        return new FairnessScoreReport(avgDist, minDist, maxDist, woodVar, stoneVar, foodVar, isBalanced);
    }

    private static float ComputeVariance(IEnumerable<float> values)
    {
        var list = values.ToList();
        if (list.Count == 0) return 0f;
        float avg = list.Average();
        return list.Select(v => (v - avg) * (v - avg)).Average();
    }
}
