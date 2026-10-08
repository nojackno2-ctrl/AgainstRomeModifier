namespace AgainstRomeMapEditor.Modules.Pathfinding;

/// <summary>
/// 部隊方陣通道寬度評估器：依據《反抗羅馬》formdef.dau 規範與 1~20 人方陣跨度，
/// 透過距離場（Clearance Transform）檢查關鍵通道（峽谷、橋樑、林間小徑）之通行淨空。
/// </summary>
public static class FormationWidthValidator
{
    private static readonly (int X, int Z)[] OrthogonalDirs = [(0, -1), (1, 0), (0, 1), (-1, 0)];

    /// <summary>
    /// 計算特定部隊人數所需的最小通行網格淨寬（格數與世界空間長度）。
    /// </summary>
    public static (int RequiredTiles, float RequiredWidthUnits) GetRequiredClearance(int unitCount, float spacing = 320f)
    {
        if (unitCount <= 0) return (0, 0f);
        if (unitCount == 1) return (1, 64f);
        if (unitCount <= 5) return (2, 160f);
        if (unitCount <= 10) return (4, 280f);
        if (unitCount <= 15) return (5, 360f);
        return (6, 440f);
    }

    /// <summary>
    /// 依據現有通行淨空格數推估最大可安全通過之部隊編制人數。
    /// </summary>
    public static int GetMaxSafeUnitCount(int clearanceTiles)
    {
        if (clearanceTiles <= 0) return 0;
        if (clearanceTiles == 1) return 1;
        if (clearanceTiles == 2) return 5;
        if (clearanceTiles == 3) return 8;
        if (clearanceTiles == 4) return 12;
        return 20;
    }

    /// <summary>
    /// 計算全圖通行淨空場（Clearance Grid，以多源 BFS 計算到最近障礙物之曼哈頓/切比雪夫距離）。
    /// </summary>
    public static int[] ComputeClearanceField(NavMeshPassabilityGrid grid)
    {
        ArgumentNullException.ThrowIfNull(grid);
        int size = grid.Size;
        var clearance = new int[size * size];
        var queue = new Queue<int>();

        // 1. 初始化：障礙物與邊界距離為 0，通暢格為 -1
        for (int z = 0; z < size; z++)
        for (int x = 0; x < size; x++)
        {
            if (!grid.IsPassable(x, z))
            {
                clearance[z * size + x] = 0;
                queue.Enqueue(z * size + x);
            }
            else
            {
                clearance[z * size + x] = -1;
            }
        }

        // 2. 多源 BFS 擴展距離場
        while (queue.Count > 0)
        {
            int current = queue.Dequeue();
            int curDist = clearance[current];
            int cx = current % size, cz = current / size;

            foreach (var (dx, dz) in OrthogonalDirs)
            {
                int nx = cx + dx, nz = cz + dz;
                if (!grid.IsInBounds(nx, nz)) continue;
                int neighbor = nz * size + nx;

                if (clearance[neighbor] == -1)
                {
                    clearance[neighbor] = curDist + 1;
                    queue.Enqueue(neighbor);
                }
            }
        }

        return clearance;
    }

    /// <summary>
    /// 評估特定路徑或起點至目標點之間的關鍵瓶頸寬度。
    /// </summary>
    public static IReadOnlyList<FormationChokePoint> EvaluateCorridors(
        NavMeshPassabilityGrid grid,
        IReadOnlyList<(NavMeshCoordinate Start, NavMeshCoordinate Target, int TroopCount)> troopTrips)
    {
        ArgumentNullException.ThrowIfNull(grid);
        ArgumentNullException.ThrowIfNull(troopTrips);

        int[] clearanceField = ComputeClearanceField(grid);
        int size = grid.Size;
        var chokes = new List<FormationChokePoint>();
        var seenLocations = new HashSet<(int X, int Z)>();

        foreach (var (start, target, count) in troopTrips)
        {
            if (count <= 1) continue; // 單兵不列為通道瓶頸警告
            var (reqTiles, reqWidth) = GetRequiredClearance(count);

            var path = FindPath(grid, start.TileX, start.TileZ, target.TileX, target.TileZ);
            if (path is null || path.Count == 0) continue;

            // 沿著路徑找出局部淨空最小之咽喉點
            int minClearance = int.MaxValue;
            (int X, int Z) chokePos = (-1, -1);

            foreach (var cell in path)
            {
                int c = clearanceField[cell.Z * size + cell.X];
                if (c < minClearance)
                {
                    minClearance = c;
                    chokePos = cell;
                }
            }

            if (minClearance < reqTiles && chokePos.X >= 0)
            {
                if (seenLocations.Add(chokePos))
                {
                    var coord = grid.ToCoordinate(chokePos.X, chokePos.Z);
                    chokes.Add(new FormationChokePoint(
                        coord,
                        minClearance,
                        minClearance * grid.TileScale,
                        reqTiles,
                        reqWidth,
                        GetMaxSafeUnitCount(minClearance),
                        count,
                        "TravelPathChoke"));
                }
            }
        }

        return chokes;
    }

    /// <summary>
    /// 偵測全圖幾何上的拓撲瓶頸（峽谷、吊橋、隘口）。
    /// </summary>
    public static IReadOnlyList<FormationChokePoint> DetectTopologicalChokePoints(
        NavMeshPassabilityGrid grid,
        int minTroopCheck = 10)
    {
        ArgumentNullException.ThrowIfNull(grid);
        int[] clearanceField = ComputeClearanceField(grid);
        int size = grid.Size;
        var (reqTiles, reqWidth) = GetRequiredClearance(minTroopCheck);
        var results = new List<FormationChokePoint>();

        for (int z = 1; z < size - 1; z++)
        for (int x = 1; x < size - 1; x++)
        {
            if (!grid.IsPassable(x, z)) continue;
            int c = clearanceField[z * size + x];

            // 淨空過窄且兩側為障礙牆（夾縫通道）
            if (c > 0 && c < reqTiles)
            {
                bool horizontalPinch = !grid.IsPassable(x - 1, z) && !grid.IsPassable(x + 1, z);
                bool verticalPinch = !grid.IsPassable(x, z - 1) && !grid.IsPassable(x, z + 1);

                if (horizontalPinch || verticalPinch)
                {
                    results.Add(new FormationChokePoint(
                        grid.ToCoordinate(x, z),
                        c,
                        c * grid.TileScale,
                        reqTiles,
                        reqWidth,
                        GetMaxSafeUnitCount(c),
                        minTroopCheck,
                        horizontalPinch ? "HorizontalChasm" : "VerticalChasm"));
                }
            }
        }

        return results;
    }

    private static List<(int X, int Z)>? FindPath(NavMeshPassabilityGrid grid, int startX, int startZ, int endX, int endZ)
    {
        if (!grid.IsPassable(startX, startZ) || !grid.IsPassable(endX, endZ)) return null;

        int size = grid.Size;
        var parent = new Dictionary<int, int>();
        var queue = new Queue<int>();

        int startIdx = startZ * size + startX;
        int endIdx = endZ * size + endX;

        parent[startIdx] = -1;
        queue.Enqueue(startIdx);

        while (queue.Count > 0)
        {
            int current = queue.Dequeue();
            if (current == endIdx) break;

            int cx = current % size, cz = current / size;
            foreach (var (dx, dz) in OrthogonalDirs)
            {
                int nx = cx + dx, nz = cz + dz;
                if (!grid.IsInBounds(nx, nz) || !grid.IsPassable(nx, nz)) continue;
                int neighbor = nz * size + nx;
                if (!parent.ContainsKey(neighbor))
                {
                    parent[neighbor] = current;
                    queue.Enqueue(neighbor);
                }
            }
        }

        if (!parent.ContainsKey(endIdx)) return null;

        var path = new List<(int X, int Z)>();
        int cur = endIdx;
        while (cur != -1)
        {
            path.Add((cur % size, cur / size));
            cur = parent[cur];
        }
        path.Reverse();
        return path;
    }
}
