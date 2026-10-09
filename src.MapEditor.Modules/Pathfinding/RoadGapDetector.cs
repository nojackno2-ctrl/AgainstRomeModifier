namespace AgainstRomeMapEditor.Modules.Pathfinding;

/// <summary>
/// 道路網拓撲間隙偵測器：自動搜尋道路網絡中 1~2 格的微小中斷、未閉合端點或因障礙物阻絕之道路缺口。
/// </summary>
public static class RoadGapDetector
{
    private static readonly (int X, int Z)[] OrthogonalNeighbors = [(0, -1), (1, 0), (0, 1), (-1, 0)];

    /// <summary>
    /// 判定紋理名稱是否屬於道路圖塊（標準石道、羅馬道路、土路等）。
    /// </summary>
    public static bool IsRoadTexture(string? texture)
    {
        if (string.IsNullOrWhiteSpace(texture)) return false;
        return texture.StartsWith("H_WEG", StringComparison.OrdinalIgnoreCase) ||
               texture.StartsWith("V_WEG", StringComparison.OrdinalIgnoreCase) ||
               texture.StartsWith("weg", StringComparison.OrdinalIgnoreCase) ||
               texture.Contains("WEG_", StringComparison.OrdinalIgnoreCase) ||
               texture.Contains("ROM", StringComparison.OrdinalIgnoreCase) ||
               texture.StartsWith("Pflaster", StringComparison.OrdinalIgnoreCase) ||
               texture.StartsWith("PFAD", StringComparison.OrdinalIgnoreCase);
    }

    /// <summary>
    /// 依據相鄰紋理推導道路風格（Roman, Dirt, Standard）。
    /// </summary>
    public static string DetectStyle(string texture)
    {
        if (string.IsNullOrWhiteSpace(texture)) return "Standard";
        if (texture.Contains("ROM", StringComparison.OrdinalIgnoreCase) || texture.StartsWith("Pflaster", StringComparison.OrdinalIgnoreCase))
            return "Roman";
        if (texture.StartsWith("PFAD", StringComparison.OrdinalIgnoreCase))
            return "Dirt";
        return "Standard";
    }

    /// <summary>
    /// 偵測地圖中的道路中斷候選清單。
    /// </summary>
    /// <param name="dimension">網格尺寸（如 256）。</param>
    /// <param name="textures">全圖紋理清單。</param>
    /// <param name="passability">可選的通行性網格模型（用以檢查間隙是否有碰撞硬閘）。</param>
    /// <param name="maxGapDistance">最大間隙距離（預設為 2 格）。</param>
    public static IReadOnlyList<RoadGapCandidate> DetectGaps(
        int dimension,
        IReadOnlyList<string> textures,
        NavMeshPassabilityGrid? passability = null,
        int maxGapDistance = 2)
    {
        ArgumentNullException.ThrowIfNull(textures);
        if (dimension <= 0 || textures.Count != (long)dimension * dimension)
            throw new ArgumentException("紋理資料尺寸無效。", nameof(textures));

        var roadTiles = new HashSet<(int X, int Z)>();
        for (int z = 0; z < dimension; z++)
        for (int x = 0; x < dimension; x++)
        {
            if (IsRoadTexture(textures[z * dimension + x]))
            {
                roadTiles.Add((x, z));
            }
        }

        if (roadTiles.Count == 0) return [];

        // 1. 找出所有道路端點（degree <= 1）
        var endpoints = new List<(int X, int Z)>();
        foreach (var (rx, rz) in roadTiles)
        {
            int degree = 0;
            foreach (var (dx, dz) in OrthogonalNeighbors)
            {
                if (roadTiles.Contains((rx + dx, rz + dz))) degree++;
            }
            if (degree <= 1) endpoints.Add((rx, rz));
        }

        var results = new List<RoadGapCandidate>();
        var seenGaps = new HashSet<string>();

        // 2. 對每個端點向外搜索 1~maxGapDistance 格範圍內的鄰近端點或道路體
        for (int i = 0; i < endpoints.Count; i++)
        {
            var e1 = endpoints[i];
            string tex1 = textures[e1.Z * dimension + e1.X];
            string style1 = DetectStyle(tex1);

            for (int j = i + 1; j < endpoints.Count; j++)
            {
                var e2 = endpoints[j];
                int dx = Math.Abs(e2.X - e1.X);
                int dz = Math.Abs(e2.Z - e1.Z);
                int manhattan = dx + dz;

                string tex2 = textures[e2.Z * dimension + e2.X];
                if (!SupportsDirection(tex1, dx, dz) || !SupportsDirection(tex2, dx, dz)) continue;

                // 間隙為 1 格（manhattan == 2 或直線 dx==2 / dz==2）或 2 格（manhattan == 3 或 dx==3 / dz==3）
                if (manhattan > maxGapDistance + 1 || (dx > maxGapDistance + 1) || (dz > maxGapDistance + 1)) continue;

                // 取得兩端點之間的中斷格子
                var gapPoints = GetGapTilesBetween(e1, e2, roadTiles, dimension);
                if (gapPoints.Count == 0 || gapPoints.Count > maxGapDistance) continue;

                // 檢查間隙是否沒入水下（深水河流通常需專門橋樑，除非設計師意圖填補）
                bool submerged = false;
                bool needsCollisionClear = false;
                if (passability is not null)
                {
                    foreach (var (gx, gz) in gapPoints)
                    {
                        if (passability.IsTileSubmerged(gx, gz, dimension)) submerged = true;
                        if (passability.IsTileBlockedByCollision(gx, gz, dimension)) needsCollisionClear = true;
                    }
                }
                if (submerged) continue;

                // 產生唯一識別鍵避免重複
                string key = string.Join(";", gapPoints.OrderBy(p => p.Z).ThenBy(p => p.X).Select(p => $"{p.X},{p.Z}"));
                if (!seenGaps.Add(key)) continue;

                string style = style1 == "Roman" || DetectStyle(tex2) == "Roman" ? "Roman"
                             : style1 == "Dirt" || DetectStyle(tex2) == "Dirt" ? "Dirt" : "Standard";

                // 依方向推薦圖塊
                string suggestedTexture = RecommendTexture(e1, e2, gapPoints, style);

                var startCoord = NavMeshCoordinate.FromTile(e1.X, e1.Z, dimension);
                var endCoord = NavMeshCoordinate.FromTile(e2.X, e2.Z, dimension);
                var gapCoords = gapPoints.Select(p => NavMeshCoordinate.FromTile(p.X, p.Z, dimension)).ToArray();

                results.Add(new RoadGapCandidate(
                    startCoord,
                    endCoord,
                    gapCoords,
                    gapPoints.Count,
                    suggestedTexture,
                    needsCollisionClear,
                    style));
            }
        }

        return results;
    }

    private static bool SupportsDirection(string texture, int dx, int dz)
    {
        // 明確標示 H/V 的直線圖塊只能沿自身方向延伸。
        if (texture.StartsWith("H_WEG", StringComparison.OrdinalIgnoreCase) ||
            texture.StartsWith("WEG_H", StringComparison.OrdinalIgnoreCase))
            return dz == 0;
        if (texture.StartsWith("V_WEG", StringComparison.OrdinalIgnoreCase) ||
            texture.StartsWith("WEG_V", StringComparison.OrdinalIgnoreCase))
            return dx == 0;
        return true;
    }

    private static List<(int X, int Z)> GetGapTilesBetween(
        (int X, int Z) a,
        (int X, int Z) b,
        HashSet<(int X, int Z)> roadTiles,
        int dimension)
    {
        var gaps = new List<(int X, int Z)>();
        if (a.X == b.X)
        {
            // 垂直對齊
            int minZ = Math.Min(a.Z, b.Z), maxZ = Math.Max(a.Z, b.Z);
            for (int z = minZ + 1; z < maxZ; z++)
            {
                if (!roadTiles.Contains((a.X, z))) gaps.Add((a.X, z));
            }
        }
        else if (a.Z == b.Z)
        {
            // 水平對齊
            int minX = Math.Min(a.X, b.X), maxX = Math.Max(a.X, b.X);
            for (int x = minX + 1; x < maxX; x++)
            {
                if (!roadTiles.Contains((x, a.Z))) gaps.Add((x, a.Z));
            }
        }
        else
        {
            // 轉角或斜向間隙（以 L 型路徑檢查）
            // 嘗試路徑 1: (a.X, b.Z)
            // 嘗試路徑 2: (b.X, a.Z)
            (int X, int Z) corner1 = (a.X, b.Z);
            (int X, int Z) corner2 = (b.X, a.Z);

            if (!roadTiles.Contains(corner1)) gaps.Add(corner1);
            else if (!roadTiles.Contains(corner2)) gaps.Add(corner2);
        }

        return gaps;
    }

    private static string RecommendTexture((int X, int Z) a, (int X, int Z) b, List<(int X, int Z)> gaps, string style)
    {
        bool isHorizontal = a.Z == b.Z;
        bool isVertical = a.X == b.X;

        return style switch
        {
            "Roman" => isHorizontal ? "WEG_H1ROM" : (isVertical ? "WEG_V1ROM" : "Pflaster_braun1"),
            "Dirt" => "PFAD1",
            _ => isHorizontal ? "H_WEG1" : (isVertical ? "V_WEG1" : "weg1")
        };
    }
}
