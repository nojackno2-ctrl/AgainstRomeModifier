namespace AgainstRomeMapEditor.Modules.Fortification;

/// <summary>
/// 防禦工事筆畫路徑規劃器 (WallStrokePlanner)。
/// 負責將滑鼠連續拖曳軌跡進行正交化網格離散、拓撲鄰接圖分析、
/// 轉角防禦塔自動晉升、道路交叉城門自動嵌合、以及起訖點平滑收尾。
/// </summary>
public static class WallStrokePlanner
{
    private const float WorldUnitsPerTile = 256.0f;

    /// <summary>
    /// 規劃防禦工事路徑放置。
    /// </summary>
    public static WallStrokePlan Plan(
        int mapDimension,
        IReadOnlyList<(int X, int Z)> rawPath,
        WallStrokeOptions options,
        WallTileCatalog catalog,
        Func<int, int, float>? getHeightAtTile = null,
        Func<int, int, bool>? isRoadAtTile = null,
        Func<int, int, bool>? isObstructedAtTile = null)
    {
        ArgumentNullException.ThrowIfNull(rawPath);
        ArgumentNullException.ThrowIfNull(options);
        ArgumentNullException.ThrowIfNull(catalog);

        if (mapDimension <= 0)
        {
            return new WallStrokePlan(false, [], [], [], [], [], "地圖尺寸必須大於 0。");
        }

        if (rawPath.Count == 0)
        {
            return new WallStrokePlan(true, [], [], [], [], []);
        }

        // 1. 邊界檢核
        var outOfBounds = rawPath.Where(p => p.X < 0 || p.Z < 0 || p.X >= mapDimension || p.Z >= mapDimension).ToList();
        if (outOfBounds.Count > 0)
        {
            return new WallStrokePlan(false, [], [], [], [], outOfBounds, "路徑座標超出地圖邊界。");
        }

        // 2. 正交離散插值（消除對角破縫）
        var continuousPath = DiscretizeOrthogonalPath(rawPath);

        // 3. 建立連通性拓撲字典
        var connectionMap = new Dictionary<(int X, int Z), WallConnections>();
        void AddConn((int X, int Z) pt, WallConnections conn) =>
            connectionMap[pt] = connectionMap.GetValueOrDefault(pt) | conn;

        if (continuousPath.Count > 0)
        {
            AddConn(continuousPath[0], WallConnections.None);
        }

        for (int i = 1; i < continuousPath.Count; i++)
        {
            var prev = continuousPath[i - 1];
            var curr = continuousPath[i];

            int dx = curr.X - prev.X;
            int dz = curr.Z - prev.Z;

            if (dx == 0 && dz == 0) continue;

            if (dz < 0) // 北
            {
                AddConn(prev, WallConnections.North);
                AddConn(curr, WallConnections.South);
            }
            else if (dx > 0) // 東
            {
                AddConn(prev, WallConnections.East);
                AddConn(curr, WallConnections.West);
            }
            else if (dz > 0) // 南
            {
                AddConn(prev, WallConnections.South);
                AddConn(curr, WallConnections.North);
            }
            else if (dx < 0) // 西
            {
                AddConn(prev, WallConnections.West);
                AddConn(curr, WallConnections.East);
            }
        }

        // 4. 依拓撲生成擺放物與分析阻擋/通行網格
        var placements = new List<FortificationPlacement>();
        var footprintTiles = new List<(int TileX, int TileZ)>();
        var gatePassageTiles = new List<(int TileX, int TileZ)>();
        var blockedTiles = new List<(int TileX, int TileZ)>();
        var unsupported = new List<(int TileX, int TileZ)>();

        foreach (var (pt, conn) in connectionMap.OrderBy(kv => kv.Key.Z).ThenBy(kv => kv.Key.X))
        {
            int tx = pt.X;
            int tz = pt.Z;

            // 障礙檢核
            if (isObstructedAtTile?.Invoke(tx, tz) == true)
            {
                unsupported.Add(pt);
                continue;
            }

            float worldX = (tx + 0.5f) * WorldUnitsPerTile;
            float worldZ = (tz + 0.5f) * WorldUnitsPerTile;
            float worldY = getHeightAtTile?.Invoke(tx, tz) ?? 0f;

            bool isForcedGate = options.ForcedGateLocations?.Contains(pt) == true;
            bool isRoadCrossing = options.AutoGateOnRoadCrossing && isRoadAtTile?.Invoke(tx, tz) == true;

            WallComponentDefinition chosenDef;
            float angleDeg;
            GatePassabilityState? gateState = null;

            if (isForcedGate || isRoadCrossing)
            {
                // 自動嵌合城門
                (chosenDef, angleDeg) = catalog.ResolveGate(options.Style, conn);
                gateState = options.DefaultGateState;
                gatePassageTiles.Add(pt);

                if (gateState == GatePassabilityState.Closed)
                {
                    blockedTiles.Add(pt);
                }
            }
            else
            {
                // 一般城牆 / 轉角 / 防禦塔 / 端點
                (chosenDef, angleDeg) = catalog.ResolveComponent(options.Style, conn, options.AutoCornerTowers);
                blockedTiles.Add(pt);
            }

            footprintTiles.Add(pt);

            var placement = new FortificationPlacement(
                Id: Guid.NewGuid(),
                NameDef: chosenDef.NameDef,
                Kind: chosenDef.Kind,
                TileX: tx,
                TileZ: tz,
                WorldX: worldX,
                WorldY: worldY,
                WorldZ: worldZ,
                AngleDeg: angleDeg,
                Team: options.Team,
                GateState: gateState,
                FoundationTexture: options.StampFoundations ? chosenDef.FoundationTexture : null);

            placements.Add(placement);
        }

        if (unsupported.Count > 0)
        {
            return new WallStrokePlan(false, [], [], [], [], unsupported, "路徑中有位置與不可移動之大型物件衝突。");
        }

        return new WallStrokePlan(true, placements, footprintTiles, gatePassageTiles, blockedTiles, []);
    }

    /// <summary>
    /// 將連續的採樣點序列展開為正交格序列（曼哈頓走線），避免對角破縫。
    /// </summary>
    public static IReadOnlyList<(int X, int Z)> DiscretizeOrthogonalPath(IReadOnlyList<(int X, int Z)> path)
    {
        if (path.Count <= 1) return path;

        var result = new List<(int X, int Z)> { path[0] };

        for (int i = 1; i < path.Count; i++)
        {
            var start = result[^1];
            var target = path[i];

            foreach (var intermediate in OrthogonalLine(start, target))
            {
                if (result[^1] != intermediate)
                {
                    result.Add(intermediate);
                }
            }
        }

        return result;
    }

    private static IEnumerable<(int X, int Z)> OrthogonalLine((int X, int Z) start, (int X, int Z) end)
    {
        int x = start.X;
        int z = start.Z;
        int dx = Math.Sign(end.X - start.X);
        int dz = Math.Sign(end.Z - start.Z);

        // 先走 X 軸，再走 Z 軸（形成直角相接）
        while (x != end.X)
        {
            x += dx;
            yield return (x, z);
        }

        while (z != end.Z)
        {
            z += dz;
            yield return (x, z);
        }
    }
}
