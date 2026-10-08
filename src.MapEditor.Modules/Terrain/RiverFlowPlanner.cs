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

        if (heights is not null && (vertexSize < 2 || heights.Count != (long)vertexSize * vertexSize))
            throw new ArgumentException("Height field dimensions do not match.");
        if (options.WaterLevel is { } water && options.ElevationMode != RiverElevationMode.ValidateOnly &&
            (!float.IsFinite(water) || !float.IsFinite(options.HeightmapStep) || options.HeightmapStep <= 0 ||
             water / options.HeightmapStep < 6))
            return new RiverStrokePlan(false, [], [], [], [], path?.ToArray() ?? []);

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
            float startElevation = SampleTileAverageHeight(dimension, heights, vertexSize, continuous[0].X, continuous[0].Y);
            float endElevation = SampleTileAverageHeight(dimension, heights, vertexSize, continuous[^1].X, continuous[^1].Y);
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
            tileElevations[i] = SampleTileAverageHeight(dimension, heights, vertexSize, path[i].X, path[i].Y);
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
            int waterBed = options.WaterLevel is { } level
                ? (int)Math.Clamp(MathF.Floor(level / options.HeightmapStep) - Math.Max(6, options.WaterBedDepth), 0, 255)
                : 255;
            targetHeights[0] = Math.Min(waterBed, (int)MathF.Round(tileElevations[0]));

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

            if (options.WaterLevel is not null)
            {
                PlanWaterChannel(dimension, heights, vertexSize, path, targetHeights, options, heightAdjustments);
                return;
            }

            // 針對河道途經的頂點計算高度修正
            var vertexTarget = new Dictionary<int, byte>();
            for (int i = 0; i < count; i++)
            {
                var pt = path[i];
                byte target = (byte)Math.Clamp(targetHeights[i], 0, 255);

                double scale = (vertexSize - 1.0) / dimension;
                for (int vy = (int)Math.Ceiling(pt.Y * scale); vy <= (pt.Y + 1) * scale && vy < vertexSize; vy++)
                for (int vx = (int)Math.Ceiling(pt.X * scale); vx <= (pt.X + 1) * scale && vx < vertexSize; vx++)
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

    private static void PlanWaterChannel(
        int dimension, IReadOnlyList<byte> heights, int vertexSize,
        List<(int X, int Y)> path, int[] beds, RiverPlannerOptions options,
        List<RiverHeightAdjustment> adjustments)
    {
        double scale = (vertexSize - 1.0) / dimension;
        double core = Math.Max(0.75, scale / 4);
        double bank = Math.Max(1, options.BankSlopeVertices);
        double radius = core + bank;
        var targets = new Dictionary<int, byte>();
        // Distance to the continuous centreline gives rounded ends and connected turns.
        // Interpolate to the original height only within the local bank footprint; never raise water.
        for (int i = 0; i < path.Count; i++)
        {
            int next = Math.Min(i + 1, path.Count - 1);
            double ax = (path[i].X + 0.5) * scale, ay = (path[i].Y + 0.5) * scale;
            double bx = (path[next].X + 0.5) * scale, by = (path[next].Y + 0.5) * scale;
            double dx = bx - ax, dy = by - ay, length2 = dx * dx + dy * dy;
            int left = Math.Max(0, (int)Math.Ceiling(Math.Min(ax, bx) - radius));
            int right = Math.Min(vertexSize - 1, (int)Math.Floor(Math.Max(ax, bx) + radius));
            int top = Math.Max(0, (int)Math.Ceiling(Math.Min(ay, by) - radius));
            int bottom = Math.Min(vertexSize - 1, (int)Math.Floor(Math.Max(ay, by) + radius));
            for (int y = top; y <= bottom; y++)
            for (int x = left; x <= right; x++)
            {
                double t = length2 == 0 ? 0 : Math.Clamp(((x - ax) * dx + (y - ay) * dy) / length2, 0, 1);
                double distance = Math.Sqrt(Math.Pow(x - ax - t * dx, 2) + Math.Pow(y - ay - t * dy, 2));
                if (distance >= radius) continue;
                int index = y * vertexSize + x;
                double bed = beds[i] + t * (beds[next] - beds[i]);
                double slope = Math.Clamp((distance - core) / bank, 0, 1);
                slope = slope * slope * (3 - 2 * slope);
                byte target = (byte)Math.Clamp(Math.Floor(bed + (heights[index] - bed) * slope), 0, heights[index]);
                if (target < heights[index])
                    targets[index] = targets.TryGetValue(index, out byte existing) ? Math.Min(existing, target) : target;
            }
        }
        foreach (var (index, target) in targets.OrderBy(pair => pair.Key))
            adjustments.Add(new RiverHeightAdjustment(index, index % vertexSize, index / vertexSize, heights[index], target));
    }

    private static float SampleTileAverageHeight(int dimension, IReadOnlyList<byte> heights, int vertexSize, int tx, int ty)
    {
        double scale = (vertexSize - 1.0) / dimension;
        int left = (int)Math.Round(tx * scale), right = (int)Math.Round((tx + 1) * scale);
        int top = (int)Math.Round(ty * scale), bottom = (int)Math.Round((ty + 1) * scale);
        return (heights[top * vertexSize + left] + heights[top * vertexSize + right]
            + heights[bottom * vertexSize + left] + heights[bottom * vertexSize + right]) / 4f;
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
