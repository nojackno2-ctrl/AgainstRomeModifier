using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.Modules.Nature;

/// <summary>
/// 自然生態植被散播演算法參數。
/// 控制生成覆蓋率、隨機種子、筆刷/範圍區域與避障特徵。
/// </summary>
public sealed class FloraScatterParameters
{
    /// <summary>隨機數與雜訊種子碼。相同種子與地貌保證生成完全相同的植被分佈。</summary>
    public int Seed { get; init; } = 1337;

    /// <summary>總體植被密度倍率（0.1 ~ 3.0，預設 1.0）。支援即時微調林冠疏密度。</summary>
    public float DensityMultiplier { get; init; } = 1.0f;

    /// <summary>宏觀森林斑塊覆蓋率（0.1 ~ 2.0，預設 1.0）。控制大地圖上林地所佔的宏觀比例。</summary>
    public float ForestCoverage { get; init; } = 1.0f;

    /// <summary>宏觀森林雜訊取樣頻率（預設 0.035）。數值越小林地斑塊越大，數值越大則林地越破碎。</summary>
    public float ForestPatchScale { get; init; } = 0.035f;

    /// <summary>散播範圍最小 Tile X（0 ~ Dimension - 1）。預設 0。</summary>
    public int MinTileX { get; init; }

    /// <summary>散播範圍最小 Tile Y（0 ~ Dimension - 1）。預設 0。</summary>
    public int MinTileY { get; init; }

    /// <summary>散播範圍最大 Tile X（0 ~ Dimension - 1）。預設 255。</summary>
    public int MaxTileX { get; init; } = 255;

    /// <summary>散播範圍最大 Tile Y（0 ~ Dimension - 1）。預設 255。</summary>
    public int MaxTileY { get; init; } = 255;

    /// <summary>格內隨機抖動幅度（0.0 ~ 0.95，預設 0.65）。消除網格人工感。</summary>
    public float JitterAmount { get; init; } = 0.65f;

    /// <summary>是否智慧回避道路與建築基地（預設 true）。</summary>
    public bool AvoidRoadsAndBuildings { get; init; } = true;

    /// <summary>是否在水岸邊緣自動生成伴生水生植物（蘆葦、浮萍、垂柳，預設 true）。</summary>
    public bool EnableRiparianFlora { get; init; } = true;

    /// <summary>是否在過陡坡地與懸崖邊緣自動散播岩石（預設 true）。</summary>
    public bool EnableCliffRocks { get; init; } = true;
}

/// <summary>
/// 生態散播引擎的輸入環境上下文。
/// 提供地表高度、水面、碰撞、道路、既有物件及可用物件範本。
/// </summary>
public sealed class FloraScatterContext
{
    public int Dimension { get; init; } = 256;
    public float TileWorldSize { get; init; } = 64.0f;
    public IReadOnlyList<byte>? Heights { get; init; }
    public int VertexSize { get; init; } = 257;
    public float HeightMapStep { get; init; } = 4.0f;
    public byte WaterLevel { get; init; } = 120;
    public IReadOnlyList<byte>? Collision { get; init; }
    public IReadOnlyList<(float X, float Z, float RadiusTiles)>? ObstacleCircles { get; init; }
    public IReadOnlySet<int>? RoadTiles { get; init; }
    public IReadOnlyList<(float X, float Z)>? ExistingAdditions { get; init; }
    public required BiomeEcologyProfile Profile { get; init; }
    public required IReadOnlyDictionary<int, LevelObjectTemplate> Templates { get; init; }
    public required IReadOnlyDictionary<int, string> ObjDefNames { get; init; }
}

/// <summary>
/// 散播執行統計報告。
/// </summary>
public sealed record FloraScatterReport(
    int TotalPlanted,
    int CanopyCount,
    int UnderstoryCount,
    int GroundFloraCount,
    int RiparianCount,
    int RockCount,
    int AvoidedObstacles,
    string BiomeName);

/// <summary>
/// 散播引擎生成結果。包含產生的植被清單與統計摘要。
/// </summary>
public sealed record FloraScatterResult(
    IReadOnlyList<NatureAddition> Additions,
    FloraScatterReport Report);

/// <summary>
/// 自然生態圈植被與生物群落自動散播引擎（Biome Ecology &amp; Flora Auto-Scatterer）。
/// 基於多八度柏林雜訊、海拔水文與地形坡度權重圖，自動規劃出高度自然、多層次過渡的林冠群落。
/// </summary>
public static class FloraScatterEngine
{
    /// <summary>
    /// 執行生態群落散播計算。產出的結果可直接透過 <see cref="NatureEditSession.PlantMany"/> 一鍵提交至編輯器交易歷史。
    /// </summary>
    public static FloraScatterResult Generate(FloraScatterContext context, FloraScatterParameters parameters)
    {
        ArgumentNullException.ThrowIfNull(context);
        ArgumentNullException.ThrowIfNull(parameters);

        ResolvedEcologyRoster roster = context.Profile.ResolveTemplates(context.Templates, context.ObjDefNames);
        if (roster.TotalSpeciesCount == 0)
        {
            return new FloraScatterResult(Array.Empty<NatureAddition>(),
                new FloraScatterReport(0, 0, 0, 0, 0, 0, 0, context.Profile.DisplayNameZh));
        }

        var additions = new List<NatureAddition>();
        var spatialGrid = new SpatialClearanceGrid(context.Dimension * context.TileWorldSize, cellSize: 2.0f * context.TileWorldSize);

        // 預填既有物件至空間網格，避免新植被重疊
        if (context.ExistingAdditions is not null)
        {
            float defaultSpacing = 0.65f * context.TileWorldSize;
            foreach (var pos in context.ExistingAdditions)
                spatialGrid.Add(pos.X, pos.Z, defaultSpacing);
        }

        // 預填建築與特殊障礙圓形
        if (parameters.AvoidRoadsAndBuildings && context.ObstacleCircles is not null)
        {
            foreach (var obs in context.ObstacleCircles)
                spatialGrid.Add(obs.X, obs.Z, obs.RadiusTiles * context.TileWorldSize);
        }

        var random = new Random(parameters.Seed);
        int avoidedObstacles = 0;
        int canopyCount = 0, understoryCount = 0, groundFloraCount = 0, riparianCount = 0, rockCount = 0;

        int minX = Math.Clamp(parameters.MinTileX, 0, context.Dimension - 1);
        int maxX = Math.Clamp(parameters.MaxTileX, 0, context.Dimension - 1);
        int minY = Math.Clamp(parameters.MinTileY, 0, context.Dimension - 1);
        int maxY = Math.Clamp(parameters.MaxTileY, 0, context.Dimension - 1);

        float waterWorldY = context.WaterLevel * context.HeightMapStep;
        // 缺少高度圖時，零高度僅供位置回傳；不能據此推定深水或水岸生境。
        bool hasHeights = context.Heights is { Count: > 0 };

        for (int tz = minY; tz <= maxY; tz++)
        {
            for (int tx = minX; tx <= maxX; tx++)
            {
                int tileIndex = tz * context.Dimension + tx;

                // 1. 道路避障
                if (parameters.AvoidRoadsAndBuildings && context.RoadTiles?.Contains(tileIndex) == true)
                {
                    avoidedObstacles++;
                    continue;
                }

                // 2. 碰撞阻擋避障（collision 255）
                if (parameters.AvoidRoadsAndBuildings && context.Collision is not null &&
                    tileIndex < context.Collision.Count && context.Collision[tileIndex] == 255)
                {
                    avoidedObstacles++;
                    continue;
                }

                // 3. 高度與坡度計算
                byte elevationByte = SampleTileElevationByte(tx, tz, context);
                float tileWorldY = elevationByte * context.HeightMapStep;
                float slopeDegrees = CalculateSlopeDegrees(tx, tz, context);

                // 4. 水位檢查：過深水域（水深超過 2.5 步長單位）禁止陸生植被生長
                float waterDepth = waterWorldY - tileWorldY;
                if (hasHeights && waterDepth > 2.5f * context.HeightMapStep)
                {
                    continue;
                }

                // 5. 生態生境決策
                VegetationLayer chosenLayer;

                // 生境 A: 水岸濱水帶 (Riparian)
                if (hasHeights && parameters.EnableRiparianFlora &&
                    MathF.Abs(tileWorldY - waterWorldY) <= context.Profile.RiparianElevationDelta * context.HeightMapStep &&
                    roster.HasLayer(VegetationLayer.Riparian))
                {
                    float ripNoise = EcologyNoise.Perlin2D(tx * 0.12f, tz * 0.12f, parameters.Seed + 333);
                    float ripProbability = 0.40f * parameters.DensityMultiplier;
                    if (ripNoise > 0.35f && random.NextDouble() < ripProbability)
                    {
                        chosenLayer = VegetationLayer.Riparian;
                    }
                    else
                    {
                        continue;
                    }
                }
                // 生境 B: 懸崖陡坡岩石 (Cliff / Rock)
                else if (parameters.EnableCliffRocks && slopeDegrees >= context.Profile.CliffSlopeThreshold)
                {
                    // 坡度過陡，高大喬木無法立足；散落岩石或耐旱灌木
                    float rockNoise = EcologyNoise.Perlin2D(tx * 0.15f, tz * 0.15f, parameters.Seed + 555);
                    if (rockNoise > 0.45f && roster.HasLayer(VegetationLayer.Rock))
                    {
                        chosenLayer = VegetationLayer.Rock;
                    }
                    else if (rockNoise > 0.28f && roster.HasLayer(VegetationLayer.Understory))
                    {
                        chosenLayer = VegetationLayer.Understory;
                    }
                    else
                    {
                        continue; // 陡峭裸岩，不生長物件
                    }
                }
                // 生境 C: 森林矩陣 (Forest & Ecotone Matrix)
                else
                {
                    // 多八度雜訊：宏觀大斑塊 + 微觀聚落
                    float macroNoise = EcologyNoise.OctaveNoise2D(
                        tx * parameters.ForestPatchScale,
                        tz * parameters.ForestPatchScale,
                        3, 0.5f, 2.0f, parameters.Seed);

                    float microNoise = EcologyNoise.OctaveNoise2D(
                        tx * 0.14f,
                        tz * 0.14f,
                        2, 0.5f, 2.0f, parameters.Seed + 777);

                    float forestIndex = (macroNoise * 0.72f + microNoise * 0.28f) * parameters.ForestCoverage;

                    if (forestIndex >= context.Profile.CanopyThreshold)
                    {
                        // 深林核心區（Core Forest）：高密度、以高大喬木為主
                        float chance = 0.82f * parameters.DensityMultiplier;
                        if (random.NextDouble() > chance) continue;

                        if (random.NextDouble() < 0.85 && roster.HasLayer(VegetationLayer.Canopy))
                            chosenLayer = VegetationLayer.Canopy;
                        else if (roster.HasLayer(VegetationLayer.Understory))
                            chosenLayer = VegetationLayer.Understory;
                        else
                            chosenLayer = VegetationLayer.Canopy;
                    }
                    else if (forestIndex >= context.Profile.UnderstoryThreshold)
                    {
                        // 林緣過渡帶（Canopy Ecotone）：灌木繁茂，次冠喬木
                        float chance = 0.58f * parameters.DensityMultiplier;
                        if (random.NextDouble() > chance) continue;

                        double roll = random.NextDouble();
                        if (roll < 0.62 && roster.HasLayer(VegetationLayer.Understory))
                            chosenLayer = VegetationLayer.Understory;
                        else if (roll < 0.85 && roster.HasLayer(VegetationLayer.Canopy))
                            chosenLayer = VegetationLayer.Canopy;
                        else if (roster.HasLayer(VegetationLayer.GroundFlora))
                            chosenLayer = VegetationLayer.GroundFlora;
                        else
                            chosenLayer = VegetationLayer.Understory;
                    }
                    else if (forestIndex >= context.Profile.GroundFloraThreshold)
                    {
                        // 林窗草甸（Meadow Glade）：地表草花為主，偶有點綴矮灌木
                        float chance = 0.38f * parameters.DensityMultiplier;
                        if (random.NextDouble() > chance) continue;

                        double roll = random.NextDouble();
                        if (roll < 0.70 && roster.HasLayer(VegetationLayer.GroundFlora))
                            chosenLayer = VegetationLayer.GroundFlora;
                        else if (roll < 0.88 && roster.HasLayer(VegetationLayer.Understory))
                            chosenLayer = VegetationLayer.Understory;
                        else if (roster.HasLayer(VegetationLayer.Rock))
                            chosenLayer = VegetationLayer.Rock;
                        else
                            chosenLayer = VegetationLayer.GroundFlora;
                    }
                    else
                    {
                        // 開闊平原（Open Plains）：極稀疏野草，留白維持視野通透
                        float chance = 0.08f * parameters.DensityMultiplier;
                        if (random.NextDouble() > chance) continue;

                        if (roster.HasLayer(VegetationLayer.GroundFlora))
                            chosenLayer = VegetationLayer.GroundFlora;
                        else
                            continue;
                    }
                }

                // 6. 挑選物種並驗證物種專屬坡度/海拔容許度
                if (!roster.TryPick(chosenLayer, random, out ResolvedSpecies species)) continue;
                if (slopeDegrees > species.MaxSlope) continue;
                if (elevationByte < species.MinElevation || elevationByte > species.MaxElevation) continue;

                // 7. 計算格內抖動坐標與世界空間位置
                float jitter = parameters.JitterAmount;
                float jitterX = 0.5f + (float)(random.NextDouble() - 0.5) * jitter;
                float jitterZ = 0.5f + (float)(random.NextDouble() - 0.5) * jitter;

                float worldX = (tx + jitterX) * context.TileWorldSize;
                float worldZ = (tz + jitterZ) * context.TileWorldSize;

                // 限制在有效世界座標 [0, 16383] 內（Against Rome 關卡儲存規格）
                worldX = Math.Clamp(worldX, 0f, 16383f);
                worldZ = Math.Clamp(worldZ, 0f, 16383f);

                float footprintWorld = species.FootprintRadiusTiles * context.TileWorldSize;

                // 8. 空間佔位防重疊檢查（Poisson-like Clearance）
                if (!spatialGrid.IsFree(worldX, worldZ, footprintWorld))
                {
                    avoidedObstacles++;
                    continue;
                }

                // 9. 精確表面高度雙線性插值取樣
                float surfaceY = InterpolateSurfaceWorldHeight(worldX, worldZ, context);

                // 10. 隨機旋轉角 [0, 2π)
                float rotation = (float)(random.NextDouble() * Math.PI * 2.0);

                // 11. 註冊至空間網格並記錄新增
                spatialGrid.Add(worldX, worldZ, footprintWorld);
                additions.Add(new NatureAddition(species.Template, species.Name, worldX, surfaceY, worldZ, rotation));

                // 統計層級
                switch (chosenLayer)
                {
                    case VegetationLayer.Canopy: canopyCount++; break;
                    case VegetationLayer.Understory: understoryCount++; break;
                    case VegetationLayer.GroundFlora: groundFloraCount++; break;
                    case VegetationLayer.Riparian: riparianCount++; break;
                    case VegetationLayer.Rock: rockCount++; break;
                }
            }
        }

        var report = new FloraScatterReport(
            TotalPlanted: additions.Count,
            CanopyCount: canopyCount,
            UnderstoryCount: understoryCount,
            GroundFloraCount: groundFloraCount,
            RiparianCount: riparianCount,
            RockCount: rockCount,
            AvoidedObstacles: avoidedObstacles,
            BiomeName: context.Profile.DisplayNameZh);

        return new FloraScatterResult(additions, report);
    }

    private static byte SampleTileElevationByte(int tx, int tz, FloraScatterContext context)
    {
        if (context.Heights is null || context.Heights.Count == 0) return 0;
        int step = (context.VertexSize - 1) / context.Dimension;
        int vx = Math.Clamp(tx * step, 0, context.VertexSize - 1);
        int vz = Math.Clamp(tz * step, 0, context.VertexSize - 1);
        int index = vz * context.VertexSize + vx;
        return index < context.Heights.Count ? context.Heights[index] : (byte)0;
    }

    private static float CalculateSlopeDegrees(int tx, int tz, FloraScatterContext context)
    {
        if (context.Heights is null || context.Heights.Count == 0) return 0f;
        int step = (context.VertexSize - 1) / context.Dimension;
        int vx = Math.Clamp(tx * step, 0, context.VertexSize - 1);
        int vz = Math.Clamp(tz * step, 0, context.VertexSize - 1);

        int vxPrev = Math.Max(0, vx - 1);
        int vxNext = Math.Min(context.VertexSize - 1, vx + 1);
        int vzPrev = Math.Max(0, vz - 1);
        int vzNext = Math.Min(context.VertexSize - 1, vz + 1);

        int vSize = context.VertexSize;
        float hxPrev = context.Heights[vz * vSize + vxPrev] * context.HeightMapStep;
        float hxNext = context.Heights[vz * vSize + vxNext] * context.HeightMapStep;
        float hzPrev = context.Heights[vzPrev * vSize + vx] * context.HeightMapStep;
        float hzNext = context.Heights[vzNext * vSize + vx] * context.HeightMapStep;

        float dx = MathF.Abs(hxNext - hxPrev);
        float dz = MathF.Abs(hzNext - hzPrev);
        float dist = 2.0f * context.TileWorldSize;

        float slopeRad = MathF.Atan2(MathF.Sqrt(dx * dx + dz * dz), dist);
        return slopeRad * (180.0f / MathF.PI);
    }

    private static float InterpolateSurfaceWorldHeight(float worldX, float worldZ, FloraScatterContext context)
    {
        if (context.Heights is null || context.Heights.Count == 0) return 0f;

        float tileX = worldX / context.TileWorldSize;
        float tileZ = worldZ / context.TileWorldSize;

        float step = (context.VertexSize - 1) / (float)context.Dimension;
        float vx = Math.Clamp(tileX * step, 0f, context.VertexSize - 1f);
        float vz = Math.Clamp(tileZ * step, 0f, context.VertexSize - 1f);

        int x0 = (int)MathF.Floor(vx);
        int z0 = (int)MathF.Floor(vz);
        int x1 = Math.Min(context.VertexSize - 1, x0 + 1);
        int z1 = Math.Min(context.VertexSize - 1, z0 + 1);

        float fx = vx - x0;
        float fz = vz - z0;

        int vSize = context.VertexSize;
        float h00 = context.Heights[z0 * vSize + x0];
        float h10 = context.Heights[z0 * vSize + x1];
        float h01 = context.Heights[z1 * vSize + x0];
        float h11 = context.Heights[z1 * vSize + x1];

        float h0 = h00 + fx * (h10 - h00);
        float h1 = h01 + fx * (h11 - h01);
        float h = h0 + fz * (h1 - h0);

        return h * context.HeightMapStep;
    }

    /// <summary>
    /// 高效二維空間雜湊網格，提供 O(1) 佔位半徑避碰查詢。
    /// </summary>
    private sealed class SpatialClearanceGrid
    {
        private readonly float _cellSize;
        private readonly int _cellsPerAxis;
        private readonly Dictionary<int, List<(float X, float Z, float Radius)>> _grid = new();

        public SpatialClearanceGrid(float worldExtent, float cellSize)
        {
            _cellSize = cellSize > 0 ? cellSize : 128.0f;
            _cellsPerAxis = (int)MathF.Ceiling(worldExtent / _cellSize) + 1;
        }

        public void Add(float x, float z, float radius)
        {
            int cellX = Math.Max(0, (int)(x / _cellSize));
            int cellZ = Math.Max(0, (int)(z / _cellSize));
            int key = cellZ * _cellsPerAxis + cellX;

            if (!_grid.TryGetValue(key, out var list))
            {
                list = new List<(float X, float Z, float Radius)>();
                _grid[key] = list;
            }
            list.Add((x, z, radius));
        }

        public bool IsFree(float x, float z, float radius)
        {
            int centerCellX = Math.Max(0, (int)(x / _cellSize));
            int centerCellZ = Math.Max(0, (int)(z / _cellSize));

            int reach = (int)MathF.Ceiling(radius / _cellSize) + 1;

            for (int cz = centerCellZ - reach; cz <= centerCellZ + reach; cz++)
            {
                if (cz < 0) continue;
                for (int cx = centerCellX - reach; cx <= centerCellX + reach; cx++)
                {
                    if (cx < 0) continue;
                    int key = cz * _cellsPerAxis + cx;
                    if (!_grid.TryGetValue(key, out var list)) continue;

                    foreach (var item in list)
                    {
                        float dx = item.X - x;
                        float dz = item.Z - z;
                        float minDistance = item.Radius + radius;
                        if (dx * dx + dz * dz < minDistance * minDistance)
                            return false;
                    }
                }
            }
            return true;
        }
    }
}
