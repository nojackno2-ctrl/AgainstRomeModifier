namespace AgainstRomeMapEditor;

/// <summary>
/// 懸崖圖塊放置指令。
/// </summary>
public sealed record CliffTilePlacement(int X, int Y, string Texture, CliffFacing Facing);

/// <summary>
/// 碎石/岩屑坡腳過渡材質放置指令。
/// </summary>
public sealed record ScreePlacement(int X, int Y, string MaterialId);

/// <summary>
/// 懸崖生成規劃結果。
/// </summary>
public sealed record CliffPlanResult(
    bool Succeeded,
    IReadOnlyList<CliffTilePlacement> CliffTiles,
    IReadOnlyList<ScreePlacement> ScreePlacements,
    IReadOnlyList<(int X, int Y)> BlockedCollisionPixels,
    IReadOnlyList<string> Issues);

/// <summary>
/// 懸崖生成規劃參數。
/// </summary>
public sealed record CliffPlannerOptions(
    string? PreferredFamily = "FELS",
    bool GenerateScree = true,
    string ScreeMaterialId = "BK",
    int ScreeRadius = 1,
    bool GenerateCrestTransition = false,
    string CrestMaterialId = "B8",
    bool AutoMarkCollision = true,
    int RandomSeed = 42);

/// <summary>
/// 規劃結果套用至編輯會話後的狀態。
/// </summary>
public sealed record CliffApplyResult(
    bool Succeeded,
    int StampedCliffTiles,
    int PaintedScreeTiles,
    int BlockedCollisionPixels,
    IReadOnlyList<string> Errors);

/// <summary>
/// 懸崖岩壁與過渡材質規劃器。
/// 沿著等高線梯度邊緣匹配合適朝向圖塊，並在坡腳展開碎石過渡材質與生成碰撞阻擋層。
/// </summary>
public static class CliffFacePlanner
{
    public static CliffPlanResult Plan(
        int dimension,
        CliffDetectionResult detection,
        CliffTileCatalog catalog,
        CliffPlannerOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(detection);
        ArgumentNullException.ThrowIfNull(catalog);
        options ??= new CliffPlannerOptions();

        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(dimension);

        var cliffPlacements = new List<CliffTilePlacement>();
        var issues = new List<string>();

        // 1. 為每個懸崖單元挑選合適朝向與變體的岩壁圖塊印章
        foreach (CliffCell cell in detection.CliffCells)
        {
            string? texture = catalog.PickTile(cell.Facing, cell.X, cell.Y, options.RandomSeed, options.PreferredFamily);
            if (string.IsNullOrWhiteSpace(texture))
            {
                issues.Add($"無法為座標 ({cell.X}, {cell.Y}) 朝向 {cell.Facing} 找到合適的岩壁圖塊。");
                continue;
            }

            cliffPlacements.Add(new CliffTilePlacement(cell.X, cell.Y, texture, cell.Facing));
        }

        var placedCoordinates = cliffPlacements.Select(p => (p.X, p.Y)).ToHashSet();
        var effectCells = catalog.RequiresExactFacing
            ? detection.CliffCells.Where(c => placedCoordinates.Contains((c.X, c.Y))).ToArray()
            : detection.CliffCells;

        // 2. 坡腳碎石與岩屑過渡帶 (Talus / Scree Apron)
        var screePlacements = new List<ScreePlacement>();
        if (options.GenerateScree && !string.IsNullOrWhiteSpace(options.ScreeMaterialId))
        {
            var screeTiles = new HashSet<(int X, int Y)>();

            foreach (CliffCell cell in effectCells)
            {
                (int Dx, int Dy)[] downhillOffsets = GetDownhillOffsets(cell.Facing);
                foreach (var (dx, dy) in downhillOffsets)
                {
                    for (int r = 1; r <= options.ScreeRadius; r++)
                    {
                        int sx = cell.X + dx * r;
                        int sy = cell.Y + dy * r;

                        if (sx >= 0 && sy >= 0 && sx < dimension && sy < dimension)
                        {
                            // 碎石帶不覆蓋懸崖圖塊本體
                            if (!detection.IsCliff(sx, sy))
                            {
                                screeTiles.Add((sx, sy));
                            }
                        }
                    }
                }
            }

            foreach (var (sx, sy) in screeTiles.OrderBy(t => t.Y).ThenBy(t => t.X))
            {
                screePlacements.Add(new ScreePlacement(sx, sy, options.ScreeMaterialId));
            }
        }

        // 3. 崖頂邊緣過渡 (Crest Outcrop Transition)
        if (options.GenerateCrestTransition && !string.IsNullOrWhiteSpace(options.CrestMaterialId))
        {
            var crestTiles = new HashSet<(int X, int Y)>();
            foreach (CliffCell cell in effectCells)
            {
                (int Dx, int Dy)[] uphillOffsets = GetUphillOffsets(cell.Facing);
                foreach (var (dx, dy) in uphillOffsets)
                {
                    int cx = cell.X + dx;
                    int cy = cell.Y + dy;
                    if (cx >= 0 && cy >= 0 && cx < dimension && cy < dimension && !detection.IsCliff(cx, cy))
                    {
                        crestTiles.Add((cx, cy));
                    }
                }
            }

            foreach (var (cx, cy) in crestTiles.OrderBy(t => t.Y).ThenBy(t => t.X))
            {
                screePlacements.Add(new ScreePlacement(cx, cy, options.CrestMaterialId));
            }
        }

        // 4. 計算通行碰撞阻擋像素（每圖塊對應 4x4 通行碰撞像素，碰撞值設為 255）
        var blockedPixels = new List<(int X, int Y)>();
        if (options.AutoMarkCollision)
        {
            foreach (CliffCell cell in effectCells)
            {
                for (int py = 0; py < 4; py++)
                {
                    for (int px = 0; px < 4; px++)
                    {
                        blockedPixels.Add((cell.X * 4 + px, cell.Y * 4 + py));
                    }
                }
            }
        }

        bool success = cliffPlacements.Count > 0 && (issues.Count == 0 || catalog.RequiresExactFacing);
        return new CliffPlanResult(success, cliffPlacements, screePlacements, blockedPixels, issues);
    }

    /// <summary>
    /// 將規劃結果安全地套用至現有地形筆畫編輯會話中，支援原子化 Undo/Redo 交易。
    /// </summary>
    internal static CliffApplyResult ApplyPlan(
        TerrainBlendEditSession blendSession,
        TerrainHeightEditSession? heightSession,
        CliffPlanResult plan)
    {
        ArgumentNullException.ThrowIfNull(blendSession);
        ArgumentNullException.ThrowIfNull(plan);

        if (!plan.Succeeded)
        {
            return new CliffApplyResult(false, 0, 0, 0, plan.Issues);
        }

        var errors = new List<string>();
        int stampedCount = 0;
        int screeCount = 0;
        int collisionCount = 0;

        // 1. 印章圖塊放置
        foreach (CliffTilePlacement placement in plan.CliffTiles)
        {
            if (blendSession.StampTexture(placement.X, placement.Y, placement.Texture) is not null)
            {
                stampedCount++;
            }
        }

        // 2. 碎石坡腳材質塗抹（利用 PaintTiles 與原生 4U/4T 過渡烘焙）
        if (plan.ScreePlacements.Count > 0)
        {
            var byMaterial = plan.ScreePlacements.GroupBy(p => p.MaterialId);
            foreach (var group in byMaterial)
            {
                var tiles = group.Select(p => (p.X, p.Y)).ToArray();
                TerrainBlendPaintResult paintResult = blendSession.PaintTiles(tiles, group.Key, autoBridge: true);
                if (paintResult.Succeeded)
                {
                    screeCount += tiles.Length;
                }
                else
                {
                    errors.Add($"坡腳材質 {group.Key} 自動過渡烘焙失敗，缺少原版邊界圖塊。");
                }
            }
        }

        // 3. 通行碰撞標記
        if (heightSession is not null && plan.BlockedCollisionPixels.Count > 0)
        {
            foreach (var (px, py) in plan.BlockedCollisionPixels)
            {
                heightSession.PaintCollision(px, py, 1.0f, TerrainCollisionOperation.Block);
                collisionCount++;
            }
            heightSession.CommitStroke();
        }

        blendSession.CommitStroke();

        // 坡腳碎石自動過渡失敗只是警告（岩壁圖塊與碰撞標記已套用）；全部都沒套用才算失敗。
        return new CliffApplyResult(stampedCount > 0 || screeCount > 0 || collisionCount > 0, stampedCount, screeCount, collisionCount, errors);
    }

    private static (int Dx, int Dy)[] GetDownhillOffsets(CliffFacing facing) => facing switch
    {
        CliffFacing.North => [(0, -1)],
        CliffFacing.South => [(0, 1)],
        CliffFacing.East => [(1, 0)],
        CliffFacing.West => [(-1, 0)],
        CliffFacing.NorthEastOuter => [(0, -1), (1, 0), (1, -1)],
        CliffFacing.NorthWestOuter => [(0, -1), (-1, 0), (-1, -1)],
        CliffFacing.SouthEastOuter => [(0, 1), (1, 0), (1, 1)],
        CliffFacing.SouthWestOuter => [(0, 1), (-1, 0), (-1, 1)],
        CliffFacing.NorthEastInner => [(0, 1), (-1, 0)],
        CliffFacing.NorthWestInner => [(0, 1), (1, 0)],
        CliffFacing.SouthEastInner => [(0, -1), (-1, 0)],
        CliffFacing.SouthWestInner => [(0, -1), (1, 0)],
        _ => []
    };

    private static (int Dx, int Dy)[] GetUphillOffsets(CliffFacing facing) => facing switch
    {
        CliffFacing.North => [(0, 1)],
        CliffFacing.South => [(0, -1)],
        CliffFacing.East => [(-1, 0)],
        CliffFacing.West => [(1, 0)],
        CliffFacing.NorthEastOuter => [(-1, 1)],
        CliffFacing.NorthWestOuter => [(1, 1)],
        CliffFacing.SouthEastOuter => [(-1, -1)],
        CliffFacing.SouthWestOuter => [(1, -1)],
        CliffFacing.NorthEastInner => [(0, -1), (1, 0)],
        CliffFacing.NorthWestInner => [(0, -1), (-1, 0)],
        CliffFacing.SouthEastInner => [(0, 1), (1, 0)],
        CliffFacing.SouthWestInner => [(0, 1), (-1, 0)],
        _ => []
    };
}
