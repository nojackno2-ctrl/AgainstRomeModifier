namespace AgainstRomeMapEditor;

/// <summary>
/// 單一被偵測到的懸崖圖塊網格單元資訊。
/// </summary>
public sealed record CliffCell(
    int X,
    int Y,
    CliffFacing Facing,
    float SlopeDegrees,
    float ElevationDelta,
    float GradientX,
    float GradientY);

/// <summary>
/// 連續的等高線/懸崖邊界路徑。
/// </summary>
public sealed record CliffContourPath(
    int Id,
    IReadOnlyList<CliffCell> Cells,
    bool IsClosedLoop);

/// <summary>
/// 懸崖邊界偵測結果。
/// </summary>
public sealed record CliffDetectionResult(
    int Dimension,
    IReadOnlyList<CliffCell> CliffCells,
    IReadOnlyList<CliffContourPath> Contours)
{
    private readonly Dictionary<(int X, int Y), CliffCell> _cellMap = CliffCells.ToDictionary(c => (c.X, c.Y));

    public bool IsCliff(int x, int y) => _cellMap.ContainsKey((x, y));

    public CliffCell? GetCell(int x, int y) => _cellMap.GetValueOrDefault((x, y));
}

/// <summary>
/// 從地形高度圖自動計算梯度、坡度臨界值並生成階梯式等高線邊界的偵測器。
/// </summary>
public static class CliffEdgeDetector
{
    /// <summary>預設坡度臨界值（度數，45 度對應 100% 坡度）。</summary>
    public const float DefaultMinSlopeDegrees = 45.0f;

    /// <summary>預設高度差臨界值（原版 boden.bmp 綠通道 0-255 之刻度，15 階相當於 60 遊戲高度單位）。</summary>
    public const float DefaultMinElevationDelta = 15.0f;

    /// <summary>
    /// 從 64x64 圖塊平均高度資料偵測懸崖與陡坡邊緣。
    /// </summary>
    public static CliffDetectionResult DetectFromTileHeights(
        int dimension,
        IReadOnlyList<float> tileHeights,
        float minSlopeDegrees = DefaultMinSlopeDegrees,
        float minElevationDelta = DefaultMinElevationDelta)
    {
        if (dimension <= 0 || tileHeights.Count != dimension * dimension)
            throw new ArgumentException("圖塊高度維度與數量不符。", nameof(tileHeights));

        var detectedCells = new List<CliffCell>();

        for (int y = 0; y < dimension; y++)
        {
            for (int x = 0; x < dimension; x++)
            {
                float currentH = tileHeights[y * dimension + x];

                float hN = y > 0 ? tileHeights[(y - 1) * dimension + x] : currentH;
                float hS = y < dimension - 1 ? tileHeights[(y + 1) * dimension + x] : currentH;
                float hE = x < dimension - 1 ? tileHeights[y * dimension + (x + 1)] : currentH;
                float hW = x > 0 ? tileHeights[y * dimension + (x - 1)] : currentH;

                float hNE = (y > 0 && x < dimension - 1) ? tileHeights[(y - 1) * dimension + (x + 1)] : currentH;
                float hNW = (y > 0 && x > 0) ? tileHeights[(y - 1) * dimension + (x - 1)] : currentH;
                float hSE = (y < dimension - 1 && x < dimension - 1) ? tileHeights[(y + 1) * dimension + (x + 1)] : currentH;
                float hSW = (y < dimension - 1 && x > 0) ? tileHeights[(y + 1) * dimension + (x - 1)] : currentH;

                float dropN = currentH - hN;
                float dropS = currentH - hS;
                float dropE = currentH - hE;
                float dropW = currentH - hW;

                float dropNE = currentH - hNE;
                float dropNW = currentH - hNW;
                float dropSE = currentH - hSE;
                float dropSW = currentH - hSW;

                float maxOrthogonalDrop = Math.Max(Math.Max(Math.Abs(dropN), Math.Abs(dropS)), Math.Max(Math.Abs(dropE), Math.Abs(dropW)));
                float maxDiagonalDrop = Math.Max(Math.Max(Math.Abs(dropNE), Math.Abs(dropNW)), Math.Max(Math.Abs(dropSE), Math.Abs(dropSW)));
                float maxElevationDelta = Math.Max(maxOrthogonalDrop, maxDiagonalDrop);

                // 計算中心差分梯度
                float gx = (hE - hW) / 2.0f;
                float gy = (hS - hN) / 2.0f;

                // 換算為遊戲空間坡度角（頂點間距基準為 64，高度縮放係數為 4）
                // tan(theta) = (deltaH * 4) / 64 = deltaH / 16
                float slopeDegrees = MathF.Atan(maxElevationDelta / 16.0f) * (180.0f / MathF.PI);

                if (slopeDegrees >= minSlopeDegrees || maxElevationDelta >= minElevationDelta)
                {
                    CliffFacing facing = ClassifyFacing(dropN, dropS, dropE, dropW, minElevationDelta);
                    detectedCells.Add(new CliffCell(x, y, facing, slopeDegrees, maxElevationDelta, gx, gy));
                }
            }
        }

        // 串接等高線連續路徑
        IReadOnlyList<CliffContourPath> contours = ChainContours(dimension, detectedCells);

        return new CliffDetectionResult(dimension, detectedCells, contours);
    }

    /// <summary>
    /// 從原始頂點高度圖 (257x257 boden.bmp) 自動降採樣並偵測懸崖陡坡。
    /// 同時評估單一圖塊內部（4x4 網格）的最大高低落差，防止圖塊內部陡坡被均化抹平。
    /// </summary>
    public static CliffDetectionResult DetectFromVertexHeights(
        int vertexSize,
        IReadOnlyList<byte> vertexHeights,
        int tileDimension = 64,
        int heightmapStep = 4,
        float minSlopeDegrees = DefaultMinSlopeDegrees,
        float minElevationDelta = DefaultMinElevationDelta)
    {
        if (vertexSize <= 1 || vertexHeights.Count != vertexSize * vertexSize)
            throw new ArgumentException("頂點高度資料長度無效。", nameof(vertexHeights));

        var tileAverages = new float[tileDimension * tileDimension];
        var tileIntraDeltas = new float[tileDimension * tileDimension];

        for (int ty = 0; ty < tileDimension; ty++)
        {
            for (int tx = 0; tx < tileDimension; tx++)
            {
                int minVx = tx * heightmapStep;
                int maxVx = Math.Min(vertexSize - 1, (tx + 1) * heightmapStep);
                int minVy = ty * heightmapStep;
                int maxVy = Math.Min(vertexSize - 1, (ty + 1) * heightmapStep);

                float sum = 0f;
                byte minH = 255;
                byte maxH = 0;
                int count = 0;

                for (int vy = minVy; vy <= maxVy; vy++)
                {
                    for (int vx = minVx; vx <= maxVx; vx++)
                    {
                        byte val = vertexHeights[vy * vertexSize + vx];
                        sum += val;
                        if (val < minH) minH = val;
                        if (val > maxH) maxH = val;
                        count++;
                    }
                }

                int tileIdx = ty * tileDimension + tx;
                tileAverages[tileIdx] = count > 0 ? sum / count : 0f;
                tileIntraDeltas[tileIdx] = maxH - minH;
            }
        }

        // 先以平均高度計算圖塊間邊界
        CliffDetectionResult result = DetectFromTileHeights(tileDimension, tileAverages, minSlopeDegrees, minElevationDelta);

        // 合併圖塊內部落差超過臨界值之單元
        var combinedCells = new Dictionary<(int X, int Y), CliffCell>();
        foreach (CliffCell cell in result.CliffCells)
        {
            combinedCells[(cell.X, cell.Y)] = cell;
        }

        for (int ty = 0; ty < tileDimension; ty++)
        {
            for (int tx = 0; tx < tileDimension; tx++)
            {
                if (combinedCells.ContainsKey((tx, ty))) continue;

                float intraDelta = tileIntraDeltas[ty * tileDimension + tx];
                float intraSlope = MathF.Atan(intraDelta / 16.0f) * (180.0f / MathF.PI);

                if (intraDelta >= minElevationDelta || intraSlope >= minSlopeDegrees)
                {
                    // 根據相鄰圖塊落差決定內部懸崖朝向
                    float curH = tileAverages[ty * tileDimension + tx];
                    float dropN = ty > 0 ? curH - tileAverages[(ty - 1) * tileDimension + tx] : 0;
                    float dropS = ty < tileDimension - 1 ? curH - tileAverages[(ty + 1) * tileDimension + tx] : 0;
                    float dropE = tx < tileDimension - 1 ? curH - tileAverages[ty * tileDimension + (tx + 1)] : 0;
                    float dropW = tx > 0 ? curH - tileAverages[ty * tileDimension + (tx - 1)] : 0;

                    CliffFacing facing = ClassifyFacing(dropN, dropS, dropE, dropW, minElevationDelta);
                    combinedCells[(tx, ty)] = new CliffCell(tx, ty, facing, intraSlope, intraDelta, 0, 0);
                }
            }
        }

        var finalCellList = combinedCells.Values.OrderBy(c => c.Y).ThenBy(c => c.X).ToArray();
        var chainedContours = ChainContours(tileDimension, finalCellList);

        return new CliffDetectionResult(tileDimension, finalCellList, chainedContours);
    }

    /// <summary>
    /// 依四向高程差判斷峭壁幾何朝向（正向、外凸角、內凹角）。
    /// drop > 0 表示鄰格比自己低（下坡落差）；drop &lt; 0 表示鄰格比自己高（上坡壁面）。
    /// </summary>
    internal static CliffFacing ClassifyFacing(float dropN, float dropS, float dropE, float dropW, float threshold)
    {
        float cornerThreshold = threshold * 0.5f;

        // 正下坡落差
        float dN = Math.Max(0, dropN);
        float dS = Math.Max(0, dropS);
        float dE = Math.Max(0, dropE);
        float dW = Math.Max(0, dropW);

        // 反面上坡壁面
        float uN = Math.Max(0, -dropN);
        float uS = Math.Max(0, -dropS);
        float uE = Math.Max(0, -dropE);
        float uW = Math.Max(0, -dropW);

        // 1. 外凸角判定（高台山脊向兩個正交方向同時跌落）
        if (dN >= cornerThreshold && dE >= cornerThreshold) return CliffFacing.NorthEastOuter;
        if (dN >= cornerThreshold && dW >= cornerThreshold) return CliffFacing.NorthWestOuter;
        if (dS >= cornerThreshold && dE >= cornerThreshold) return CliffFacing.SouthEastOuter;
        if (dS >= cornerThreshold && dW >= cornerThreshold) return CliffFacing.SouthWestOuter;

        // 2. 內凹角判定（峽谷凹灣，兩個正交方向皆為峭壁高崖）
        if (uN >= cornerThreshold && uE >= cornerThreshold) return CliffFacing.NorthEastInner;
        if (uN >= cornerThreshold && uW >= cornerThreshold) return CliffFacing.NorthWestInner;
        if (uS >= cornerThreshold && uE >= cornerThreshold) return CliffFacing.SouthEastInner;
        if (uS >= cornerThreshold && uW >= cornerThreshold) return CliffFacing.SouthWestInner;

        // 3. 四向正坡判定
        float maxD = Math.Max(Math.Max(dN, dS), Math.Max(dE, dW));
        if (maxD > 0)
        {
            if (maxD == dN) return CliffFacing.North;
            if (maxD == dS) return CliffFacing.South;
            if (maxD == dE) return CliffFacing.East;
            if (maxD == dW) return CliffFacing.West;
        }

        // 若下坡不明顯但受上坡壁面包圍，取背向
        float maxU = Math.Max(Math.Max(uN, uS), Math.Max(uE, uW));
        if (maxU > 0)
        {
            if (maxU == uN) return CliffFacing.South;
            if (maxU == uS) return CliffFacing.North;
            if (maxU == uE) return CliffFacing.West;
            if (maxU == uW) return CliffFacing.East;
        }

        return CliffFacing.None;
    }

    /// <summary>
    /// 將散落的懸崖圖塊單元按八向連通性串接為連續等高線路徑。
    /// </summary>
    private static IReadOnlyList<CliffContourPath> ChainContours(int dimension, IReadOnlyList<CliffCell> cells)
    {
        var remaining = new Dictionary<(int X, int Y), CliffCell>(cells.Select(c => new KeyValuePair<(int X, int Y), CliffCell>((c.X, c.Y), c)));
        var paths = new List<CliffContourPath>();
        int pathId = 1;

        while (remaining.Count > 0)
        {
            // 選取當前尚未訪問的首個頂點
            CliffCell start = remaining.Values.First();
            remaining.Remove((start.X, start.Y));

            var pathCells = new List<CliffCell> { start };
            var current = start;

            while (true)
            {
                // 尋找八鄰格中最近的懸崖單元
                (int X, int Y)? nextCoord = FindNextNeighbor(current.X, current.Y, remaining);
                if (nextCoord is null) break;

                CliffCell nextCell = remaining[nextCoord.Value];
                remaining.Remove(nextCoord.Value);
                pathCells.Add(nextCell);
                current = nextCell;
            }

            // 檢查是否閉合
            int dx = Math.Abs(pathCells[0].X - pathCells[^1].X);
            int dy = Math.Abs(pathCells[0].Y - pathCells[^1].Y);
            bool isClosed = pathCells.Count > 2 && dx <= 1 && dy <= 1;

            paths.Add(new CliffContourPath(pathId++, pathCells, isClosed));
        }

        return paths;
    }

    private static (int X, int Y)? FindNextNeighbor(int cx, int cy, Dictionary<(int X, int Y), CliffCell> pool)
    {
        // 優先考慮四鄰格，其次考慮對角鄰格
        (int Dx, int Dy)[] offsets =
        [
            (0, -1), (1, 0), (0, 1), (-1, 0),
            (1, -1), (1, 1), (-1, 1), (-1, -1)
        ];

        foreach (var (dx, dy) in offsets)
        {
            var target = (cx + dx, cy + dy);
            if (pool.ContainsKey(target)) return target;
        }

        return null;
    }
}
