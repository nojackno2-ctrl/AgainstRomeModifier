namespace AgainstRomeMapEditor.Modules.Fortification;

/// <summary>
/// 防禦工事通行性與碰撞網格同步器 (FortificationPassabilitySync)。
/// 負責將城牆、防禦塔、城門之足跡精確光柵化同步至 collision.bmp (256x256)，
/// 管理城門受控通行狀態（開放/關閉），並提供要塞防禦閉合性診斷分析。
/// </summary>
internal static class FortificationPassabilitySync
{
    public const byte CollisionPassable = 0;
    public const byte CollisionBlocked = 255;

    /// <summary>
    /// 將防禦工事規劃方案光柵化並套用至 collision 網格陣列，並產生單元變更清單以供 Undo/Redo。
    /// </summary>
    public static IReadOnlyList<CollisionPixelChange> SynchronizePassability(
        byte[] collisionGrid,
        int collisionSize,
        int mapDimension,
        WallStrokePlan plan)
    {
        ArgumentNullException.ThrowIfNull(collisionGrid);
        ArgumentNullException.ThrowIfNull(plan);

        if (collisionSize <= 0 || collisionGrid.Length != collisionSize * collisionSize)
            throw new ArgumentException("collisionGrid 尺寸與長度不符。", nameof(collisionGrid));

        if (mapDimension <= 0)
            throw new ArgumentException("mapDimension 必須大於 0。", nameof(mapDimension));

        int scale = collisionSize / mapDimension;
        if (scale < 1) scale = 1;

        var changes = new List<CollisionPixelChange>();

        foreach (var placement in plan.Placements)
        {
            int basePx = placement.TileX * scale;
            int basePz = placement.TileZ * scale;

            if (placement.Kind == WallComponentKind.Gate)
            {
                // 城門：門柱阻擋，通道依 GateState 決定
                bool isOpen = placement.GateState == GatePassabilityState.Open;

                for (int dz = 0; dz < scale; dz++)
                {
                    for (int dx = 0; dx < scale; dx++)
                    {
                        int px = basePx + dx;
                        int pz = basePz + dz;
                        if (px < 0 || pz < 0 || px >= collisionSize || pz >= collisionSize) continue;

                        int idx = pz * collisionSize + px;
                        byte before = collisionGrid[idx];

                        // 中央通道區域（內側像素）
                        bool isPassage = dx >= scale / 4 && dx < scale - scale / 4 &&
                                         dz >= scale / 4 && dz < scale - scale / 4;

                        byte after = isPassage
                            ? (isOpen ? CollisionPassable : CollisionBlocked)
                            : CollisionBlocked;

                        if (before != after)
                        {
                            collisionGrid[idx] = after;
                            changes.Add(new CollisionPixelChange(idx, before, after));
                        }
                    }
                }
            }
            else
            {
                // 一般城牆 / 拐角 / 防禦塔 / 端點：全覆蓋為 CollisionBlocked
                int footprintSpan = placement.Kind == WallComponentKind.Tower ? scale * 2 : scale;

                for (int dz = 0; dz < footprintSpan; dz++)
                {
                    for (int dx = 0; dx < footprintSpan; dx++)
                    {
                        int px = basePx + dx;
                        int pz = basePz + dz;
                        if (px < 0 || pz < 0 || px >= collisionSize || pz >= collisionSize) continue;

                        int idx = pz * collisionSize + px;
                        byte before = collisionGrid[idx];
                        byte after = CollisionBlocked;

                        if (before != after)
                        {
                            collisionGrid[idx] = after;
                            changes.Add(new CollisionPixelChange(idx, before, after));
                        }
                    }
                }
            }
        }

        return changes;
    }

    /// <summary>
    /// 動態更新單座城門的受控通行狀態（開放或關閉）。
    /// </summary>
    public static IReadOnlyList<CollisionPixelChange> SetGateState(
        byte[] collisionGrid,
        int collisionSize,
        int mapDimension,
        FortificationPlacement gate,
        GatePassabilityState newState)
    {
        ArgumentNullException.ThrowIfNull(collisionGrid);
        ArgumentNullException.ThrowIfNull(gate);

        if (gate.Kind != WallComponentKind.Gate)
            throw new ArgumentException("指定的元件不是城門。", nameof(gate));

        int scale = collisionSize / mapDimension;
        if (scale < 1) scale = 1;

        int basePx = gate.TileX * scale;
        int basePz = gate.TileZ * scale;
        bool isOpen = newState == GatePassabilityState.Open;

        var changes = new List<CollisionPixelChange>();

        for (int dz = 0; dz < scale; dz++)
        {
            for (int dx = 0; dx < scale; dx++)
            {
                bool isPassage = dx >= scale / 4 && dx < scale - scale / 4 &&
                                 dz >= scale / 4 && dz < scale - scale / 4;

                if (!isPassage) continue; // 門柱維持阻擋不變

                int px = basePx + dx;
                int pz = basePz + dz;
                if (px < 0 || pz < 0 || px >= collisionSize || pz >= collisionSize) continue;

                int idx = pz * collisionSize + px;
                byte before = collisionGrid[idx];
                byte after = isOpen ? CollisionPassable : CollisionBlocked;

                if (before != after)
                {
                    collisionGrid[idx] = after;
                    changes.Add(new CollisionPixelChange(idx, before, after));
                }
            }
        }

        return changes;
    }

    /// <summary>
    /// 分析地圖上防禦工事的圍閉完整性與要塞區域。
    /// 透過邊界泛洪填充（Boundary Flood Fill）隔離外側世界，檢定內部是否存在受保護的封閉腹地。
    /// </summary>
    public static FortressEnclosureReport AnalyzeFortressEnclosure(
        int mapDimension,
        IReadOnlyList<FortificationPlacement> fortifications,
        Func<int, int, bool>? isNaturalBarrier = null)
    {
        if (mapDimension <= 0) return new FortressEnclosureReport(false, 0, [], []);

        // 標記網格：0=可穿透, 1=城牆/屏障阻擋
        bool[,] grid = new bool[mapDimension, mapDimension];

        foreach (var f in fortifications)
        {
            // 城牆與關閉的城門視為物理阻擋
            if (f.Kind != WallComponentKind.Gate || f.GateState == GatePassabilityState.Closed)
            {
                if (f.TileX >= 0 && f.TileX < mapDimension && f.TileZ >= 0 && f.TileZ < mapDimension)
                {
                    grid[f.TileX, f.TileZ] = true;
                }
            }
        }

        if (isNaturalBarrier is not null)
        {
            for (int z = 0; z < mapDimension; z++)
            {
                for (int x = 0; x < mapDimension; x++)
                {
                    if (isNaturalBarrier(x, z)) grid[x, z] = true;
                }
            }
        }

        // 泛洪填充：從四個邊緣起算所有可到達的外側區域
        bool[,] reachableFromOutside = new bool[mapDimension, mapDimension];
        var queue = new Queue<(int X, int Z)>();

        for (int x = 0; x < mapDimension; x++)
        {
            TryEnqueue(x, 0);
            TryEnqueue(x, mapDimension - 1);
        }
        for (int z = 0; z < mapDimension; z++)
        {
            TryEnqueue(0, z);
            TryEnqueue(mapDimension - 1, z);
        }

        void TryEnqueue(int x, int z)
        {
            if (!grid[x, z] && !reachableFromOutside[x, z])
            {
                reachableFromOutside[x, z] = true;
                queue.Enqueue((x, z));
            }
        }

        int[] dx = [1, -1, 0, 0];
        int[] dz = [0, 0, 1, -1];

        while (queue.Count > 0)
        {
            var (cx, cz) = queue.Dequeue();

            for (int i = 0; i < 4; i++)
            {
                int nx = cx + dx[i];
                int nz = cz + dz[i];

                if (nx >= 0 && nz >= 0 && nx < mapDimension && nz < mapDimension)
                {
                    if (!grid[nx, nz] && !reachableFromOutside[nx, nz])
                    {
                        reachableFromOutside[nx, nz] = true;
                        queue.Enqueue((nx, nz));
                    }
                }
            }
        }

        // 統計未被外側連通的內部保護格數
        int enclosedCount = 0;
        for (int z = 0; z < mapDimension; z++)
        {
            for (int x = 0; x < mapDimension; x++)
            {
                if (!grid[x, z] && !reachableFromOutside[x, z])
                {
                    enclosedCount++;
                }
            }
        }

        var gates = fortifications.Where(f => f.Kind == WallComponentKind.Gate).ToList();
        bool isEnclosed = enclosedCount > 0;

        return new FortressEnclosureReport(isEnclosed, enclosedCount, [], gates);
    }
}
