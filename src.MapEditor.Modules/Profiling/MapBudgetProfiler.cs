using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;
using global::AgainstRomeMapEditor.NativeAssets;

namespace AgainstRomeMapEditor.Modules.Profiling;

/// <summary>
/// 效能分析所需的地圖快照資料容器。
/// </summary>
public sealed record MapBudgetSnapshot(
    IReadOnlyList<MapSceneObject>? SceneObjects = null,
    IReadOnlyList<ScenarioSpawn>? Spawns = null,
    IReadOnlyList<NativeLightInstance>? Lights = null,
    int CollisionSize = 0,
    IReadOnlyList<byte>? Collision = null,
    int HeightSize = 0,
    IReadOnlyList<byte>? Heights = null,
    float HeightStep = 4.0f,
    float WaterLevel = 0.0f,
    int DecodedSpriteCount = 0,
    int DecodedShadowCount = 0,
    int DecodedSpriteTypes = 0
);

/// <summary>
/// Against Rome 地圖即時效能預警與引擎資源硬限制分析器。
/// 即時計算物件插槽、局部光源密度熱力圖、Draw Calls 與圖集切換開銷、尋路網格負載。
/// </summary>
public static class MapBudgetProfiler
{
    /// <summary>
    /// 對給定的地圖資料快照進行完整效能與預算分析。
    /// </summary>
    public static MapBudgetReport Analyze(MapBudgetSnapshot snapshot)
    {
        ArgumentNullException.ThrowIfNull(snapshot);

        var bottlenecks = new List<MapHealthBottleneck>();

        ObjectSlotBudget objectBudget = AnalyzeObjects(snapshot, bottlenecks);
        LightDensityBudget lightBudget = AnalyzeLights(snapshot, bottlenecks);
        DrawCallBudget drawBudget = AnalyzeDrawCalls(snapshot, objectBudget, bottlenecks);
        PathfindingBudget pathBudget = AnalyzePathfinding(snapshot, bottlenecks);

        (MapHealthGrade grade, int overallScore) = ComputeOverallHealth(
            objectBudget, lightBudget, drawBudget, pathBudget, bottlenecks);

        return new MapBudgetReport(
            DateTimeOffset.Now,
            grade,
            overallScore,
            objectBudget,
            lightBudget,
            drawBudget,
            pathBudget,
            bottlenecks
        );
    }

    // =========================================================================
    // 1. 物件插槽與資料池分析
    // =========================================================================

    private static ObjectSlotBudget AnalyzeObjects(
        MapBudgetSnapshot snapshot,
        List<MapHealthBottleneck> bottlenecks)
    {
        int sceneCount = snapshot.SceneObjects?.Count ?? 0;
        int spawnCount = snapshot.Spawns?.Count ?? 0;

        // 計算物件總數（合併場景物件與放置部隊）
        int totalActive = sceneCount + spawnCount;

        int landscapeCount = 0;
        int buildingCount = 0;
        int unitCount = 0;
        int fxCount = 0;
        int otherCount = 0;

        void Classify(string name)
        {
            if (name.StartsWith("Lan", StringComparison.OrdinalIgnoreCase)) landscapeCount++;
            else if (name.StartsWith("Bau", StringComparison.OrdinalIgnoreCase)) buildingCount++;
            else if (name.StartsWith("Fig", StringComparison.OrdinalIgnoreCase)
                     || name.StartsWith("Ger", StringComparison.OrdinalIgnoreCase)
                     || name.StartsWith("Rom", StringComparison.OrdinalIgnoreCase)
                     || name.StartsWith("Cel", StringComparison.OrdinalIgnoreCase)
                     || name.StartsWith("Hun", StringComparison.OrdinalIgnoreCase)) unitCount++;
            else if (name.StartsWith("FX", StringComparison.OrdinalIgnoreCase)
                     || name.StartsWith("Fil", StringComparison.OrdinalIgnoreCase)) fxCount++;
            else otherCount++;
        }

        if (snapshot.SceneObjects != null)
        {
            foreach (var obj in snapshot.SceneObjects) Classify(obj.Name);
        }
        if (snapshot.Spawns != null)
        {
            foreach (var spawn in snapshot.Spawns) Classify(spawn.Alias);
        }

        float objPercent = (float)totalActive / MapBudgetLimits.MaxObjectsDatSlots * 100.0f;
        int estimatedPositions = totalActive * 2;
        float posPercent = (float)estimatedPositions / MapBudgetLimits.MaxPositionDatRecords * 100.0f;

        // 規則檢查
        if (totalActive > MapBudgetLimits.MaxObjectsDatSlots)
        {
            bottlenecks.Add(new MapHealthBottleneck(
                "Objects",
                "objects-dat-overflow",
                $"物件總數 ({totalActive}) 已超出 objects.dat 原生硬限制 ({MapBudgetLimits.MaxObjectsDatSlots})！遊戲執行期將崩潰或無法載入。",
                $"Total object count ({totalActive}) exceeds native objects.dat hard limit ({MapBudgetLimits.MaxObjectsDatSlots})! The game will crash or fail to load.",
                IsEngineLimitViolation: true));
        }
        else if (totalActive > MapBudgetLimits.RedlineObjectsDatSlots)
        {
            bottlenecks.Add(new MapHealthBottleneck(
                "Objects",
                "objects-dat-redline",
                $"物件總數 ({totalActive}) 超過安全警戒線 ({MapBudgetLimits.RedlineObjectsDatSlots})，可能擠佔遊戲執行期動態生成空間。",
                $"Total object count ({totalActive}) exceeds safe redline ({MapBudgetLimits.RedlineObjectsDatSlots}), risking runtime dynamic spawn exhaustion."));
        }

        if (estimatedPositions > MapBudgetLimits.MaxPositionDatRecords)
        {
            bottlenecks.Add(new MapHealthBottleneck(
                "Positions",
                "position-dat-overflow",
                $"位置記錄需求 ({estimatedPositions}) 超出 position.dat 標頭容量 ({MapBudgetLimits.MaxPositionDatRecords})！",
                $"Required position records ({estimatedPositions}) exceed position.dat capacity ({MapBudgetLimits.MaxPositionDatRecords})!",
                IsEngineLimitViolation: true));
        }

        if (spawnCount > MapBudgetLimits.RedlineScenarioSpawns)
        {
            bottlenecks.Add(new MapHealthBottleneck(
                "Spawns",
                "spawns-redline",
                $"劇本預放生成物件數 ({spawnCount}) 超過建議警戒線 ({MapBudgetLimits.RedlineScenarioSpawns})，開局可能引發腳本解譯延遲。",
                $"Scenario spawn count ({spawnCount}) exceeds redline ({MapBudgetLimits.RedlineScenarioSpawns}), which may cause initial script lag."));
        }

        return new ObjectSlotBudget(
            totalActive,
            MapBudgetLimits.MaxObjectsDatSlots,
            objPercent,
            estimatedPositions,
            MapBudgetLimits.MaxPositionDatRecords,
            posPercent,
            spawnCount,
            landscapeCount,
            buildingCount,
            unitCount,
            fxCount,
            otherCount
        );
    }

    // =========================================================================
    // 2. 光源密度與熱力圖分析
    // =========================================================================

    private static LightDensityBudget AnalyzeLights(
        MapBudgetSnapshot snapshot,
        List<MapHealthBottleneck> bottlenecks)
    {
        IReadOnlyList<NativeLightInstance> lights = snapshot.Lights ?? Array.Empty<NativeLightInstance>();
        int totalLights = lights.Count;

        const int GridRes = 32;
        float[,] densityGrid = new float[GridRes, GridRes];
        const float CellWorldSpan = MapBudgetLimits.MapWorldSize / (float)GridRes; // 512.0f

        int peakClusterLights = 0;
        var rawHotspots = new List<(float X, float Z, int Count, float Radius)>();

        if (totalLights > 0)
        {
            // 建立 32x32 空間網格並統計每個網格的重疊光源
            int[,] countGrid = new int[GridRes, GridRes];

            foreach (var light in lights)
            {
                if (!light.IsActive) continue;
                float lx = Math.Clamp(light.WorldPosition.X, 0f, MapBudgetLimits.MapWorldSize - 1f);
                float lz = Math.Clamp(light.WorldPosition.Z, 0f, MapBudgetLimits.MapWorldSize - 1f);
                float radius = Math.Max(50f, light.Radius);

                int minCx = Math.Clamp((int)((lx - radius) / CellWorldSpan), 0, GridRes - 1);
                int maxCx = Math.Clamp((int)((lx + radius) / CellWorldSpan), 0, GridRes - 1);
                int minCz = Math.Clamp((int)((lz - radius) / CellWorldSpan), 0, GridRes - 1);
                int maxCz = Math.Clamp((int)((lz + radius) / CellWorldSpan), 0, GridRes - 1);

                for (int cz = minCz; cz <= maxCz; cz++)
                {
                    for (int cx = minCx; cx <= maxCx; cx++)
                    {
                        float cellCenterX = (cx + 0.5f) * CellWorldSpan;
                        float cellCenterZ = (cz + 0.5f) * CellWorldSpan;
                        float dx = cellCenterX - lx;
                        float dz = cellCenterZ - lz;
                        if (dx * dx + dz * dz <= (radius + CellWorldSpan * 0.707f) * (radius + CellWorldSpan * 0.707f))
                        {
                            countGrid[cz, cx]++;
                        }
                    }
                }
            }

            // 尋找網格峰值
            for (int cz = 0; cz < GridRes; cz++)
            {
                for (int cx = 0; cx < GridRes; cx++)
                {
                    int c = countGrid[cz, cx];
                    if (c > peakClusterLights) peakClusterLights = c;
                    if (c >= MapBudgetLimits.RedlineLightsPerCluster)
                    {
                        rawHotspots.Add((
                            (cx + 0.5f) * CellWorldSpan,
                            (cz + 0.5f) * CellWorldSpan,
                            c,
                            CellWorldSpan
                        ));
                    }
                }
            }

            // 計算正規化密度 (0.0 ~ 1.0)
            float maxCount = Math.Max(1.0f, peakClusterLights);
            for (int cz = 0; cz < GridRes; cz++)
            {
                for (int cx = 0; cx < GridRes; cx++)
                {
                    densityGrid[cz, cx] = countGrid[cz, cx] / maxCount;
                }
            }
        }

        // 局部熱點去重聚集 (Non-maximum suppression within 600 world units)
        var filteredHotspots = new List<LightClusterHotspot>();
        var sorted = rawHotspots.OrderByDescending(h => h.Count).ToList();
        var visited = new bool[sorted.Count];

        for (int i = 0; i < sorted.Count; i++)
        {
            if (visited[i]) continue;
            var h = sorted[i];
            bool exceeds = h.Count > MapBudgetLimits.HardLimitLightsPerCluster;
            filteredHotspots.Add(new LightClusterHotspot(h.X, h.Z, h.Count, h.Radius, exceeds));
            visited[i] = true;

            for (int j = i + 1; j < sorted.Count; j++)
            {
                if (visited[j]) continue;
                float distSq = (h.X - sorted[j].X) * (h.X - sorted[j].X) + (h.Z - sorted[j].Z) * (h.Z - sorted[j].Z);
                if (distSq < 600f * 600f) visited[j] = true;
            }
        }

        // 規則檢查
        if (totalLights > MapBudgetLimits.MaxGlobalRuntimeLights)
        {
            bottlenecks.Add(new MapHealthBottleneck(
                "Lighting",
                "global-lights-overflow",
                $"全域動態光源數量 ({totalLights}) 超出執行期陣列硬限制 ({MapBudgetLimits.MaxGlobalRuntimeLights})！",
                $"Total dynamic lights ({totalLights}) exceed runtime array limit ({MapBudgetLimits.MaxGlobalRuntimeLights})!",
                IsEngineLimitViolation: true));
        }

        if (peakClusterLights > MapBudgetLimits.HardLimitLightsPerCluster)
        {
            var worst = filteredHotspots.FirstOrDefault(h => h.ExceedsHardLimit);
            bottlenecks.Add(new MapHealthBottleneck(
                "Lighting",
                "cluster-lights-overflow",
                $"局部區域重疊光源峰值高達 {peakClusterLights}，已突破原生管線 64 光源硬限制！超出部分將被強制截斷，引發閃爍瑕疵。",
                $"Peak local light overlap ({peakClusterLights}) exceeds native 64 lights limit! Excess lights will be dropped, causing flickering.",
                worst?.WorldX,
                worst?.WorldZ,
                IsEngineLimitViolation: true));
        }
        else if (peakClusterLights >= MapBudgetLimits.RedlineLightsPerCluster)
        {
            var worst = filteredHotspots.FirstOrDefault();
            bottlenecks.Add(new MapHealthBottleneck(
                "Lighting",
                "cluster-lights-redline",
                $"局部區域光源密集度接近上限 (當前峰值 {peakClusterLights} / 64)，相機平移時可能發生動態光源更替與抖動。",
                $"Local light density approaches limit (peak {peakClusterLights} / 64). Camera movement may cause light popping.",
                worst?.WorldX,
                worst?.WorldZ));
        }

        return new LightDensityBudget(
            totalLights,
            peakClusterLights,
            peakClusterLights / (float)MapBudgetLimits.MaxActiveLightsInView,
            filteredHotspots,
            GridRes,
            densityGrid
        );
    }

    // =========================================================================
    // 3. 繪圖呼叫 (Draw Calls) 與圖集切換估算
    // =========================================================================

    private static DrawCallBudget AnalyzeDrawCalls(
        MapBudgetSnapshot snapshot,
        ObjectSlotBudget objectBudget,
        List<MapHealthBottleneck> bottlenecks)
    {
        int spriteQuads = snapshot.DecodedSpriteCount > 0
            ? snapshot.DecodedSpriteCount
            : (int)(objectBudget.TotalActiveObjects * 0.95f);

        int shadowQuads = snapshot.DecodedShadowCount > 0
            ? snapshot.DecodedShadowCount
            : (int)(objectBudget.TotalActiveObjects * 0.30f);

        int spriteTypes = snapshot.DecodedSpriteTypes > 0
            ? snapshot.DecodedSpriteTypes
            : Math.Clamp(objectBudget.TotalActiveObjects / 30, 10, 300);

        // 1. 地形 Draw Calls：底層紋理 + 水面 + 邊界/道路
        int terrainPasses = 3;

        // 2. Sprite 批次估算：
        // 單批次上限：非索引 10,922 quads (65,532 頂點)，16-bit 索引上限 16,384 quads
        int baseSpriteBatches = Math.Max(1, (int)MathF.Ceiling((float)spriteQuads / MapBudgetLimits.SafeSpriteQuadsPerDrawCall));

        // 4096×4096 圖集約可容納 220 種多方向/動畫 Sprite
        int estimatedAtlasPages = Math.Max(1, (int)MathF.Ceiling((float)spriteTypes / 220f));

        // 跨頁圖集切換：若超過 1 頁，在等角深度排序下頁面交錯引起的切換次數估算
        int atlasSwitches = 0;
        if (estimatedAtlasPages > 1)
        {
            atlasSwitches = (int)MathF.Min(
                (estimatedAtlasPages - 1) * MathF.Min(spriteQuads / 40.0f, 150.0f),
                200.0f);
        }

        int totalSpriteBatches = baseSpriteBatches + atlasSwitches;

        // 3. 陰影批次
        int shadowBatches = shadowQuads > 0
            ? Math.Max(1, (int)MathF.Ceiling((float)shadowQuads / MapBudgetLimits.MaxSpriteQuadsPerBatch))
            : 0;

        int totalDrawCalls = terrainPasses + totalSpriteBatches + shadowBatches;

        float fragScore = spriteQuads > 0
            ? Math.Clamp(atlasSwitches / Math.Max(1.0f, spriteQuads / 50.0f), 0.0f, 1.0f)
            : 0.0f;

        // 規則檢查
        if (estimatedAtlasPages > MapBudgetLimits.RedlineAtlasPages)
        {
            bottlenecks.Add(new MapHealthBottleneck(
                "Rendering",
                "atlas-pages-redline",
                $"Sprite 種類過多，預估圖集頁數達 {estimatedAtlasPages} 頁 (建議 <= {MapBudgetLimits.RecommendedAtlasPages})，將引起頻繁紋理切換並大幅增加 Draw Calls。",
                $"Too many sprite types ({spriteTypes}). Estimated atlas pages ({estimatedAtlasPages}) exceed recommended {MapBudgetLimits.RecommendedAtlasPages}, causing texture thrashing."));
        }

        if (spriteQuads > MapBudgetLimits.MaxSpriteQuadsPerBatch)
        {
            bottlenecks.Add(new MapHealthBottleneck(
                "Rendering",
                "sprite-quads-overflow",
                $"場景 Sprite 數量 ({spriteQuads}) 超出 16-bit 頂點索引單批次容量 (16,384 quads)，必須拆分多個繪製批次。",
                $"Scene sprite count ({spriteQuads}) exceeds 16-bit index capacity (16,384 quads), requiring multiple draw batches."));
        }

        if (totalDrawCalls > 100)
        {
            bottlenecks.Add(new MapHealthBottleneck(
                "Rendering",
                "high-draw-calls",
                $"預估單幀 Draw Calls ({totalDrawCalls}) 偏高，可能增加 CPU 驅動程式調度開銷。",
                $"Estimated draw calls per frame ({totalDrawCalls}) are elevated, increasing CPU driver submission overhead."));
        }

        return new DrawCallBudget(
            totalDrawCalls,
            terrainPasses,
            totalSpriteBatches,
            shadowBatches,
            estimatedAtlasPages,
            atlasSwitches,
            fragScore,
            spriteQuads,
            shadowQuads
        );
    }

    // =========================================================================
    // 4. 尋路負載與隘口瓶頸分析
    // =========================================================================

    private static PathfindingBudget AnalyzePathfinding(
        MapBudgetSnapshot snapshot,
        List<MapHealthBottleneck> bottlenecks)
    {
        int size = snapshot.CollisionSize > 0 ? snapshot.CollisionSize : MapBudgetLimits.MapGridDimension;
        IReadOnlyList<byte>? rawCollision = snapshot.Collision;

        if (rawCollision == null || rawCollision.Count != size * size)
        {
            return new PathfindingBudget(
                0f, 1, 0, Array.Empty<ChokepointLocation>(), 0, 0, "Unknown");
        }

        int totalCells = size * size;
        bool[] blocked = new bool[totalCells];
        int blockedCount = 0;

        bool hasHeight = snapshot.Heights != null && snapshot.HeightSize > 1
            && snapshot.Heights.Count == snapshot.HeightSize * snapshot.HeightSize
            && float.IsFinite(snapshot.HeightStep) && float.IsFinite(snapshot.WaterLevel);

        for (int y = 0; y < size; y++)
        {
            for (int x = 0; x < size; x++)
            {
                int idx = y * size + x;
                bool isBlocked = rawCollision[idx] != 0;

                if (!isBlocked && hasHeight)
                {
                    int hx = Math.Clamp((int)((x + 0.5f) * (snapshot.HeightSize - 1) / size), 0, snapshot.HeightSize - 1);
                    int hy = Math.Clamp((int)((y + 0.5f) * (snapshot.HeightSize - 1) / size), 0, snapshot.HeightSize - 1);
                    float height = snapshot.Heights![hy * snapshot.HeightSize + hx] * snapshot.HeightStep;
                    if (height < snapshot.WaterLevel) isBlocked = true;
                }

                blocked[idx] = isBlocked;
                if (isBlocked) blockedCount++;
            }
        }

        float obstaclePercent = (float)blockedCount / totalCells * 100.0f;

        // 1. 連通區域分析 (Flood Fill)
        var components = new int[totalCells];
        Array.Fill(components, -1);
        int componentCount = 0;
        var queue = new int[totalCells];

        for (int origin = 0; origin < totalCells; origin++)
        {
            if (blocked[origin] || components[origin] != -1) continue;

            int head = 0, tail = 0;
            queue[tail++] = origin;
            components[origin] = componentCount;

            while (head < tail)
            {
                int curr = queue[head++];
                int cx = curr % size;
                int cy = curr / size;

                void TryVisit(int next)
                {
                    if (next >= 0 && next < totalCells && !blocked[next] && components[next] == -1)
                    {
                        components[next] = componentCount;
                        queue[tail++] = next;
                    }
                }

                if (cx > 0) TryVisit(curr - 1);
                if (cx + 1 < size) TryVisit(curr + 1);
                if (cy > 0) TryVisit(curr - size);
                if (cy + 1 < size) TryVisit(curr + size);
            }

            componentCount++;
        }

        // 2. 窄道隘口偵測 (Chokepoints: 寬度 <= 2 格之雙側收縮隘口)
        var chokepointCells = new List<(int X, int Y, int Width)>();

        for (int y = 1; y < size - 1; y++)
        {
            for (int x = 1; x < size - 1; x++)
            {
                int idx = y * size + x;
                if (blocked[idx]) continue;

                // 檢查水平收縮（左右為障礙，上下可通行）
                bool leftWall = blocked[idx - 1];
                bool rightWall = blocked[idx + 1] || (x + 2 < size && !blocked[idx + 1] && blocked[idx + 2]);
                bool vertPassable = !blocked[idx - size] && !blocked[idx + size];

                if (leftWall && rightWall && vertPassable)
                {
                    int width = blocked[idx + 1] ? 1 : 2;
                    chokepointCells.Add((x, y, width));
                    continue;
                }

                // 檢查垂直收縮（上下為障礙，左右可通行）
                bool topWall = blocked[idx - size];
                bool bottomWall = blocked[idx + size] || (y + 2 < size && !blocked[idx + size] && blocked[idx + 2 * size]);
                bool horizPassable = !blocked[idx - 1] && !blocked[idx + 1];

                if (topWall && bottomWall && horizPassable)
                {
                    int width = blocked[idx + size] ? 1 : 2;
                    chokepointCells.Add((x, y, width));
                }
            }
        }

        // 聚類相鄰隘口點 (避免一條長走廊產生幾十個重複熱點)
        var chokepointLocations = new List<ChokepointLocation>();
        var cpVisited = new bool[chokepointCells.Count];

        for (int i = 0; i < chokepointCells.Count; i++)
        {
            if (cpVisited[i]) continue;
            var cp = chokepointCells[i];
            cpVisited[i] = true;

            int sumX = cp.X, sumY = cp.Y, count = 1, minW = cp.Width;

            for (int j = i + 1; j < chokepointCells.Count; j++)
            {
                if (cpVisited[j]) continue;
                var other = chokepointCells[j];
                if (Math.Abs(cp.X - other.X) <= 2 && Math.Abs(cp.Y - other.Y) <= 2)
                {
                    cpVisited[j] = true;
                    sumX += other.X;
                    sumY += other.Y;
                    count++;
                    minW = Math.Min(minW, other.Width);
                }
            }

            int avgX = sumX / count;
            int avgY = sumY / count;
            float wx = (avgX + 0.5f) * MapBudgetLimits.WorldUnitsPerTile;
            float wz = (avgY + 0.5f) * MapBudgetLimits.WorldUnitsPerTile;
            chokepointLocations.Add(new ChokepointLocation(avgX, avgY, wx, wz, minW));
        }

        // 3. 死胡同口袋估算 (Dead ends)
        int deadEndCount = 0;
        for (int y = 1; y < size - 1; y++)
        {
            for (int x = 1; x < size - 1; x++)
            {
                int idx = y * size + x;
                if (blocked[idx]) continue;

                int wallNeighbors = 0;
                if (blocked[idx - 1]) wallNeighbors++;
                if (blocked[idx + 1]) wallNeighbors++;
                if (blocked[idx - size]) wallNeighbors++;
                if (blocked[idx + size]) wallNeighbors++;

                if (wallNeighbors == 3) deadEndCount++;
            }
        }

        // 4. 尋路複雜度評分 (0 ~ 100)
        float scoreObs = MathF.Min(25.0f, obstaclePercent * 0.5f);
        float scoreChoke = MathF.Min(35.0f, chokepointLocations.Count * 1.5f);
        float scoreConn = MathF.Min(20.0f, MathF.Max(0, componentCount - 1) * 4.0f);
        float scoreDead = MathF.Min(20.0f, deadEndCount * 1.0f);

        int complexityScore = Math.Clamp((int)MathF.Round(scoreObs + scoreChoke + scoreConn + scoreDead), 0, 100);
        string rating = complexityScore switch
        {
            <= 30 => "Low",
            <= 60 => "Medium",
            <= 80 => "High",
            _ => "Severe"
        };

        // 規則檢查
        if (chokepointLocations.Count > MapBudgetLimits.RedlineChokepoints)
        {
            var worstCp = chokepointLocations.FirstOrDefault();
            bottlenecks.Add(new MapHealthBottleneck(
                "Pathfinding",
                "excessive-chokepoints",
                $"地圖存在大量窄道隘口 ({chokepointLocations.Count} 處)，部隊集體行軍時容易發生擠塞與高頻率 A* 重新尋路風暴。",
                $"Map contains excessive narrow chokepoints ({chokepointLocations.Count}), causing crowd congestion and A* re-pathing storms.",
                worstCp?.WorldX,
                worstCp?.WorldZ));
        }

        if (componentCount > MapBudgetLimits.RedlineIsolatedRegions)
        {
            bottlenecks.Add(new MapHealthBottleneck(
                "Pathfinding",
                "isolated-regions",
                $"地圖通行區域碎裂為 {componentCount} 個獨立連通區塊，部分區域部隊可能永遠無法抵達。",
                $"Passable ground is fragmented into {componentCount} disconnected regions; units may be unreachable."));
        }

        if (complexityScore > 80)
        {
            bottlenecks.Add(new MapHealthBottleneck(
                "Pathfinding",
                "severe-complexity",
                $"尋路複雜度達嚴重等級 ({complexityScore} / 100)，極易造成遊戲尋路執行緒卡死與幀率暴跌。",
                $"Pathfinding complexity is Severe ({complexityScore} / 100), posing a high risk of pathfinder freezing and FPS drops."));
        }

        return new PathfindingBudget(
            obstaclePercent,
            componentCount,
            chokepointLocations.Count,
            chokepointLocations,
            deadEndCount,
            complexityScore,
            rating
        );
    }

    // =========================================================================
    // 5. 綜合健康分數與評級判定
    // =========================================================================

    private static (MapHealthGrade Grade, int Score) ComputeOverallHealth(
        ObjectSlotBudget objectBudget,
        LightDensityBudget lightBudget,
        DrawCallBudget drawBudget,
        PathfindingBudget pathBudget,
        List<MapHealthBottleneck> bottlenecks)
    {
        // 檢查是否有直接突破原生引擎硬限制的項目
        bool hasHardLimitViolation = bottlenecks.Any(b => b.IsEngineLimitViolation);

        float penalty = 0.0f;

        // 物件插槽扣分
        if (objectBudget.TotalActiveObjects > MapBudgetLimits.MaxObjectsDatSlots) penalty += 60f;
        else if (objectBudget.TotalActiveObjects > MapBudgetLimits.RedlineObjectsDatSlots) penalty += 30f;
        else if (objectBudget.TotalActiveObjects > MapBudgetLimits.RecommendedObjectsDatSlots) penalty += 15f;

        // 光源扣分
        if (lightBudget.PeakClusterLights > MapBudgetLimits.HardLimitLightsPerCluster) penalty += 40f;
        else if (lightBudget.PeakClusterLights >= MapBudgetLimits.RedlineLightsPerCluster) penalty += 20f;
        else if (lightBudget.PeakClusterLights > MapBudgetLimits.RecommendedLightsPerCluster) penalty += 10f;

        // Draw calls 扣分
        if (drawBudget.EstimatedTotalDrawCalls > 120) penalty += 25f;
        else if (drawBudget.EstimatedTotalDrawCalls > 60) penalty += 12f;

        // 尋路扣分
        if (pathBudget.PathfindingComplexityScore > 80) penalty += 30f;
        else if (pathBudget.PathfindingComplexityScore > 60) penalty += 15f;
        else if (pathBudget.PathfindingComplexityScore > 40) penalty += 5f;

        int overallScore = Math.Clamp((int)MathF.Round(100.0f - penalty), 0, 100);

        MapHealthGrade grade;
        if (hasHardLimitViolation)
        {
            grade = MapHealthGrade.EngineExceeded;
        }
        else if (overallScore < 45 || objectBudget.TotalActiveObjects > 12_000 || lightBudget.PeakClusterLights > 58 || pathBudget.PathfindingComplexityScore > 80)
        {
            grade = MapHealthGrade.Critical;
        }
        else if (overallScore < 70 || objectBudget.TotalActiveObjects > MapBudgetLimits.RedlineObjectsDatSlots || lightBudget.PeakClusterLights >= MapBudgetLimits.RedlineLightsPerCluster || pathBudget.PathfindingComplexityScore > 60)
        {
            grade = MapHealthGrade.Warning;
        }
        else if (overallScore < 85 || objectBudget.TotalActiveObjects > MapBudgetLimits.RecommendedObjectsDatSlots || lightBudget.PeakClusterLights > MapBudgetLimits.RecommendedLightsPerCluster || pathBudget.PathfindingComplexityScore > 40)
        {
            grade = MapHealthGrade.Caution;
        }
        else
        {
            grade = MapHealthGrade.Healthy;
        }

        return (grade, overallScore);
    }
}
