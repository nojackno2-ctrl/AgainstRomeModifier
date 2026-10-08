namespace AgainstRomeMapEditor;

/// <summary>
/// 《反抗羅馬》河流水系自動規劃引擎。
/// 負責：
/// 1. 筆畫軌跡轉正交平滑河道。
/// 2. 水文流向判定（支援順流分析與等高線海拔高低自動反推）。
/// 3. 高度場落差驗證與河床自動開鑿（避免河水逆流，強制單調遞減與深蝕槽化）。
/// 4. 河道直道、轉彎、T型匯流與十字交叉圖塊智能挑選。
/// 5. 兩側河岸泥地（ErdeFlussR）過渡圖塊生成與既有水系自動連通。
/// </summary>
public static class RiverFlowPlanner
{
    private static readonly (int X, int Y, RiverConnections Out, RiverConnections Back, RiverFlowDirections OutFlow, RiverFlowDirections InFlow, RiverBankSide Bank)[] Neighbors =
    [
        (0, -1, RiverConnections.North, RiverConnections.South, RiverFlowDirections.OutflowNorth, RiverFlowDirections.InflowSouth, RiverBankSide.North),
        (1, 0,  RiverConnections.East,  RiverConnections.West,  RiverFlowDirections.OutflowEast,  RiverFlowDirections.InflowWest,  RiverBankSide.East),
        (0, 1,  RiverConnections.South, RiverConnections.North, RiverFlowDirections.OutflowSouth, RiverFlowDirections.InflowNorth, RiverBankSide.South),
        (-1, 0, RiverConnections.West,  RiverConnections.East,  RiverFlowDirections.OutflowWest,  RiverFlowDirections.InflowEast,  RiverBankSide.West)
    ];

    public static RiverStrokePlan Plan(
        int dimension,
        IReadOnlyList<string> textures,
        IReadOnlyList<byte>? heights,
        int vertexSize,
        IReadOnlyList<(int X, int Y)> path,
        RiverTileCatalog catalog,
        RiverPlannerOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(textures);
        ArgumentNullException.ThrowIfNull(catalog);
        options ??= new RiverPlannerOptions();

        if (dimension <= 0 || textures.Count != (long)dimension * dimension)
            throw new ArgumentException("地圖圖塊尺寸與紋理陣列長度不符。");

        if (path is null || path.Count == 0)
            return new RiverStrokePlan(true, [], [], [], [], []);

        // 檢查路徑是否超出邊界
        var outOfBounds = path.Where(p => p.X < 0 || p.Y < 0 || p.X >= dimension || p.Y >= dimension).ToArray();
        if (outOfBounds.Length > 0)
            return new RiverStrokePlan(false, [], [], [], [], outOfBounds);

        // 1. 筆畫正交化與連續路徑擴展
        var continuous = RasterizeOrthogonalPath(path);
        if (continuous.Count == 0)
            return new RiverStrokePlan(true, [], [], [], [], []);

        // 2. 水流方向判定（起訖點高程分析）
        if (options.AutoDetectDownhillFlow && heights is not null && vertexSize > 0 && continuous.Count > 1)
        {
            float startElevation = SampleTileAverageHeight(heights, vertexSize, continuous[0].X, continuous[0].Y);
            float endElevation = SampleTileAverageHeight(heights, vertexSize, continuous[^1].X, continuous[^1].Y);
            if (startElevation < endElevation)
            {
                // 使用者從低處往高處繪製，自動反轉流向以確保水往低處流
                continuous.Reverse();
            }
        }

        // 3. 高度場落差驗證與河床下挖規劃
        var warnings = new List<RiverElevationWarning>();
        var heightAdjustments = new List<RiverHeightAdjustment>();

        if (heights is not null && vertexSize > 0)
        {
            PlanHeightfieldProfile(dimension, heights, vertexSize, continuous, options, warnings, heightAdjustments);
        }

        // 4. 連通圖與水流方向標記
        var links = new Dictionary<(int X, int Y), RiverConnections>();
        var flows = new Dictionary<(int X, int Y), RiverFlowDirections>();

        void AddConn((int X, int Y) pt, RiverConnections c) => links[pt] = links.GetValueOrDefault(pt) | c;
        void AddFlow((int X, int Y) pt, RiverFlowDirections f) => flows[pt] = flows.GetValueOrDefault(pt) | f;

        AddConn(continuous[0], RiverConnections.None);
        AddFlow(continuous[0], RiverFlowDirections.None);

        for (int i = 1; i < continuous.Count; i++)
        {
            var prev = continuous[i - 1];
            var curr = continuous[i];
            int dx = curr.X - prev.X, dy = curr.Y - prev.Y;
            var dir = Neighbors.FirstOrDefault(n => n.X == dx && n.Y == dy);
            if (dir != default)
            {
                AddConn(prev, dir.Out);
                AddConn(curr, dir.Back);
                AddFlow(prev, dir.OutFlow);
                AddFlow(curr, dir.InFlow);
            }
        }

        // 與現有水系融合（若鄰格已經是河流圖塊，建立雙向匯流）
        var pathCoords = new HashSet<(int X, int Y)>(continuous);
        foreach (var p in pathCoords)
        {
            foreach (var n in Neighbors)
            {
                int nx = p.X + n.X, ny = p.Y + n.Y;
                if (nx < 0 || ny < 0 || nx >= dimension || ny >= dimension) continue;
                if (pathCoords.Contains((nx, ny))) continue;

                string adjTex = textures[ny * dimension + nx];
                if (catalog.IsWaterTexture(adjTex))
                {
                    // 併入匯流連通
                    AddConn(p, n.Out);
                }
            }
        }

        // 5. 河道圖塊挑選
        var waterPlacements = new List<RiverTilePlacement>();
        var unsupported = new List<(int X, int Y)>();
        int seed = 17;

        for (int i = 0; i < continuous.Count; i++)
        {
            var p = continuous[i];
            var conn = links.GetValueOrDefault(p, RiverConnections.None);
            var flow = flows.GetValueOrDefault(p, RiverFlowDirections.None);

            RiverTileKind kind = ClassifyTileKind(conn, i, continuous.Count);
            var tile = catalog.FindWaterTile(conn, kind, options.PreferredRiverTexture, seed++);
            if (tile is null)
            {
                unsupported.Add(p);
            }
            else
            {
                waterPlacements.Add(new RiverTilePlacement(p.X, p.Y, tile.Texture, conn, kind, flow));
            }
        }

        // 6. 兩側河岸過渡圖塊規劃（ErdeFlussR1..4）
        var bankPlacements = new List<RiverBankPlacement>();
        if (options.GenerateBankTransitions)
        {
            var visitedBanks = new HashSet<(int X, int Y)>();
            foreach (var p in continuous)
            {
                foreach (var n in Neighbors)
                {
                    int bx = p.X + n.X, by = p.Y + n.Y;
                    if (bx < 0 || by < 0 || bx >= dimension || by >= dimension) continue;
                    if (pathCoords.Contains((bx, by))) continue;
                    if (visitedBanks.Contains((bx, by))) continue;

                    string currentTex = textures[by * dimension + bx];
                    if (catalog.IsWaterTexture(currentTex)) continue; // 若鄰格已是水體則不需過渡

                    // 依相對於河道之方位挑選對應河岸圖塊
                    string? bankTex = catalog.FindBankTile(n.Bank, options.PreferredBankTexture, seed++);
                    if (!string.IsNullOrEmpty(bankTex))
                    {
                        bankPlacements.Add(new RiverBankPlacement(bx, by, bankTex, n.Bank));
                        visitedBanks.Add((bx, by));
                    }
                }
            }
        }

        if (unsupported.Count > 0)
            return new RiverStrokePlan(false, [], [], [], warnings, unsupported);

        return new RiverStrokePlan(true, waterPlacements, bankPlacements, heightAdjustments, warnings, []);
    }

    private static RiverTileKind ClassifyTileKind(RiverConnections conn, int index, int totalCount)
    {
        int count = BitCount((byte)conn);
        if (count == 0) return RiverTileKind.LakeWater;
        if (count == 1) return index == 0 ? RiverTileKind.Source : RiverTileKind.Estuary;
        if (count == 2)
        {
            bool isStraight = conn == (RiverConnections.North | RiverConnections.South)
                           || conn == (RiverConnections.East | RiverConnections.West);
            return isStraight ? RiverTileKind.Straight : RiverTileKind.Turn;
        }
        if (count == 3) return RiverTileKind.TConfluence;
        return RiverTileKind.Cross;
    }

    private static int BitCount(byte value)
    {
        int count = 0;
        while (value != 0)
        {
            count += value & 1;
            value >>= 1;
        }
        return count;
    }

    private static void PlanHeightfieldProfile(
        int dimension,
        IReadOnlyList<byte> heights,
        int vertexSize,
        List<(int X, int Y)> path,
        RiverPlannerOptions options,
        List<RiverElevationWarning> warnings,
        List<RiverHeightAdjustment> heightAdjustments)
    {
        int count = path.Count;
        float[] tileElevations = new float[count];
        for (int i = 0; i < count; i++)
        {
            tileElevations[i] = SampleTileAverageHeight(heights, vertexSize, path[i].X, path[i].Y);
        }

        // 檢測逆流
        for (int i = 1; i < count; i++)
        {
            float prev = tileElevations[i - 1];
            float curr = tileElevations[i];
            if (curr > prev)
            {
                warnings.Add(new RiverElevationWarning(
                    path[i].X,
                    path[i].Y,
                    (int)MathF.Round(curr),
                    (int)MathF.Round(prev),
                    "河水逆流警示：下游海拔高於上游。"));
            }
        }

        // 若啟用自動下挖開鑿 (AutoCarveDescending 或 AutoCarveTrench)
        if (options.ElevationMode != RiverElevationMode.ValidateOnly)
        {
            int[] targetHeights = new int[count];
            targetHeights[0] = (int)MathF.Round(tileElevations[0]);

            for (int i = 1; i < count; i++)
            {
                int maxAllowed = targetHeights[i - 1] - Math.Max(0, options.MinSlopeDrop);
                targetHeights[i] = Math.Min(maxAllowed, (int)MathF.Round(tileElevations[i]));
                if (targetHeights[i] < 0) targetHeights[i] = 0;
            }

            if (options.ElevationMode == RiverElevationMode.AutoCarveTrench && options.TrenchDepth > 0)
            {
                for (int i = 0; i < count; i++)
                {
                    targetHeights[i] = Math.Max(0, targetHeights[i] - options.TrenchDepth);
                }
            }

            // 針對河道途經的頂點計算高度修正
            var vertexTarget = new Dictionary<int, byte>();
            for (int i = 0; i < count; i++)
            {
                var pt = path[i];
                byte target = (byte)Math.Clamp(targetHeights[i], 0, 255);

                // 包含圍繞該圖塊的4個網格頂點 (vx, vy), (vx+1, vy), (vx, vy+1), (vx+1, vy+1)
                for (int vy = pt.Y; vy <= pt.Y + 1 && vy < vertexSize; vy++)
                for (int vx = pt.X; vx <= pt.X + 1 && vx < vertexSize; vx++)
                {
                    int vIndex = vy * vertexSize + vx;
                    byte currentV = heights[vIndex];
                    if (target < currentV)
                    {
                        if (vertexTarget.TryGetValue(vIndex, out byte existing))
                        {
                            vertexTarget[vIndex] = Math.Min(existing, target);
                        }
                        else
                        {
                            vertexTarget[vIndex] = target;
                        }
                    }
                }
            }

            foreach (var (vIndex, targetVal) in vertexTarget)
            {
                int vx = vIndex % vertexSize, vy = vIndex / vertexSize;
                byte before = heights[vIndex];
                if (before != targetVal)
                {
                    heightAdjustments.Add(new RiverHeightAdjustment(vIndex, vx, vy, before, targetVal));
                }
            }
        }
    }

    private static float SampleTileAverageHeight(IReadOnlyList<byte> heights, int vertexSize, int tx, int ty)
    {
        int v00 = ty * vertexSize + tx;
        int v10 = ty * vertexSize + Math.Min(tx + 1, vertexSize - 1);
        int v01 = Math.Min(ty + 1, vertexSize - 1) * vertexSize + tx;
        int v11 = Math.Min(ty + 1, vertexSize - 1) * vertexSize + Math.Min(tx + 1, vertexSize - 1);
        return (heights[v00] + heights[v10] + heights[v01] + heights[v11]) / 4f;
    }

    private static List<(int X, int Y)> RasterizeOrthogonalPath(IReadOnlyList<(int X, int Y)> rawPath)
    {
        var result = new List<(int X, int Y)>();
        if (rawPath.Count == 0) return result;

        result.Add(rawPath[0]);
        for (int i = 1; i < rawPath.Count; i++)
        {
            var prev = result[^1];
            var next = rawPath[i];
            if (prev == next) continue;

            foreach (var step in OrthogonalBetween(prev, next))
            {
                if (result[^1] != step)
                {
                    // 消除單步立即折返 (A -> B -> A)
                    if (result.Count >= 2 && result[^2] == step)
                    {
                        result.RemoveAt(result.Count - 1);
                    }
                    else
                    {
                        result.Add(step);
                    }
                }
            }
        }
        return result;
    }

    internal static IEnumerable<(int X, int Y)> OrthogonalBetween((int X, int Y) start, (int X, int Y) end)
    {
        var previous = start;
        foreach (var next in TerrainStrokePath.Between(start.X, start.Y, end.X, end.Y))
        {
            if (next.X != previous.X && next.Y != previous.Y)
            {
                yield return (next.X, previous.Y);
            }
            yield return next;
            previous = next;
        }
    }
}
