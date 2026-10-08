namespace AgainstRomeMapEditor.Modules.Pathfinding;

/// <summary>
/// 通行網格全域連通性分析器：利用 BFS / Flood Fill 演算法從起點評估地圖連通性，
/// 標記不可通達的孤立陸地區域，並尋找最短穿透障礙物之過渡連接點。
/// </summary>
public static class NavMeshConnectivityAnalyzer
{
    private static readonly (int X, int Z)[] Directions = [(0, -1), (1, 0), (0, 1), (-1, 0)];

    /// <summary>
    /// 評估全域通行網格連通性。
    /// </summary>
    /// <param name="grid">通行性網格模型。</param>
    /// <param name="primarySeeds">主要基地或起始點（通常為玩家起始部隊、基地建築）。</param>
    /// <param name="interestPoints">其他關注物件或建築錨點（用以標記孤立區是否含有建築/目標）。</param>
    /// <param name="minIsolatedSize">報告孤立區域的最小格數閥值（預設 1 格）。</param>
    public static IReadOnlyList<IsolatedRegion> Analyze(
        NavMeshPassabilityGrid grid,
        IReadOnlyList<NavMeshCoordinate> primarySeeds,
        IReadOnlyList<NavMeshCoordinate>? interestPoints = null,
        int minIsolatedSize = 1)
    {
        ArgumentNullException.ThrowIfNull(grid);
        ArgumentNullException.ThrowIfNull(primarySeeds);

        int size = grid.Size;
        var components = new int[size * size];
        Array.Fill(components, -1);

        // 1. 標記非通行格子為 -2
        for (int z = 0; z < size; z++)
        for (int x = 0; x < size; x++)
        {
            if (!grid.IsPassable(x, z)) components[z * size + x] = -2;
        }

        // 2. 4-向 BFS 劃分連通分量
        int nextComponentId = 0;
        var queue = new int[size * size];
        var componentTiles = new Dictionary<int, List<int>>();

        for (int start = 0; start < components.Length; start++)
        {
            if (components[start] != -1) continue;

            int componentId = nextComponentId++;
            var tiles = new List<int>();
            int head = 0, tail = 0;
            queue[tail++] = start;
            components[start] = componentId;

            while (head < tail)
            {
                int current = queue[head++];
                tiles.Add(current);
                int cx = current % size, cz = current / size;

                foreach (var (dx, dz) in Directions)
                {
                    int nx = cx + dx, nz = cz + dz;
                    if (!grid.IsInBounds(nx, nz)) continue;
                    int neighbor = nz * size + nx;
                    if (components[neighbor] == -1)
                    {
                        components[neighbor] = componentId;
                        queue[tail++] = neighbor;
                    }
                }
            }
            componentTiles[componentId] = tiles;
        }

        // 3. 找出所有由主要種子點覆蓋的連通分量
        var primaryComponents = new HashSet<int>();
        foreach (var seed in primarySeeds)
        {
            if (!grid.IsInBounds(seed.TileX, seed.TileZ)) continue;
            int comp = components[seed.TileZ * size + seed.TileX];
            if (comp >= 0) primaryComponents.Add(comp);
        }

        var interestSet = new HashSet<(int X, int Z)>();
        if (interestPoints is not null)
        {
            foreach (var pt in interestPoints)
            {
                if (grid.IsInBounds(pt.TileX, pt.TileZ))
                    interestSet.Add((pt.TileX, pt.TileZ));
            }
        }

        // 4. 分析所有未覆蓋的連通分量（孤立分量）
        var isolatedList = new List<IsolatedRegion>();
        foreach (var (compId, tiles) in componentTiles)
        {
            if (primaryComponents.Contains(compId)) continue;
            if (tiles.Count < minIsolatedSize) continue;

            int minX = int.MaxValue, minZ = int.MaxValue;
            int maxX = int.MinValue, maxZ = int.MinValue;
            long sumX = 0, sumZ = 0;
            bool hasInterest = false;

            foreach (int cell in tiles)
            {
                int tx = cell % size, tz = cell / size;
                if (tx < minX) minX = tx;
                if (tx > maxX) maxX = tx;
                if (tz < minZ) minZ = tz;
                if (tz > maxZ) maxZ = tz;
                sumX += tx;
                sumZ += tz;

                if (!hasInterest && interestSet.Contains((tx, tz)))
                {
                    hasInterest = true;
                }
            }

            int avgX = (int)(sumX / tiles.Count);
            int avgZ = (int)(sumZ / tiles.Count);
            var centroid = grid.ToCoordinate(avgX, avgZ);

            // 5. 尋找與主要通達區的最短穿透障礙橋樑
            var (bridgeCoord, bridgeDistance) = FindShortestBridge(grid, components, tiles, primaryComponents);

            isolatedList.Add(new IsolatedRegion(
                compId,
                tiles.Count,
                centroid,
                minX, minZ, maxX, maxZ,
                hasInterest,
                bridgeCoord,
                bridgeDistance));
        }

        return isolatedList.OrderByDescending(r => r.ContainsTroopOrBuilding)
                           .ThenByDescending(r => r.TileCount)
                           .ToArray();
    }

    /// <summary>
    /// 計算孤立區域邊界到主要可達區之間的最短阻擋距離與穿越點。
    /// </summary>
    private static (NavMeshCoordinate? BridgeCoord, int Distance) FindShortestBridge(
        NavMeshPassabilityGrid grid,
        int[] components,
        List<int> isolatedTiles,
        HashSet<int> primaryComponents)
    {
        if (primaryComponents.Count == 0) return (null, 0);

        int size = grid.Size;
        var dist = new Dictionary<int, int>();
        var queue = new Queue<int>();

        // 將孤立區域的邊界格子作為多源 BFS 起點
        foreach (int cell in isolatedTiles)
        {
            int cx = cell % size, cz = cell / size;
            bool isBoundary = false;
            foreach (var (dx, dz) in Directions)
            {
                int nx = cx + dx, nz = cz + dz;
                if (!grid.IsInBounds(nx, nz)) continue;
                if (components[nz * size + nx] != components[cell])
                {
                    isBoundary = true;
                    break;
                }
            }
            if (isBoundary)
            {
                dist[cell] = 0;
                queue.Enqueue(cell);
            }
        }

        int maxSearchDepth = 12; // 最多尋找穿過 12 格障礙的橋樑
        while (queue.Count > 0)
        {
            int current = queue.Dequeue();
            int curDist = dist[current];
            if (curDist >= maxSearchDepth) continue;

            int cx = current % size, cz = current / size;
            foreach (var (dx, dz) in Directions)
            {
                int nx = cx + dx, nz = cz + dz;
                if (!grid.IsInBounds(nx, nz)) continue;
                int neighbor = nz * size + nx;

                int targetComp = components[neighbor];
                if (primaryComponents.Contains(targetComp))
                {
                    // 找到連接主要通達區的穿透點
                    int bridgeTileX = (current % size + nx) / 2;
                    int bridgeTileZ = (current / size + nz) / 2;
                    return (grid.ToCoordinate(bridgeTileX, bridgeTileZ), curDist + 1);
                }

                if (!dist.ContainsKey(neighbor))
                {
                    dist[neighbor] = curDist + 1;
                    queue.Enqueue(neighbor);
                }
            }
        }

        return (null, 0);
    }
}
