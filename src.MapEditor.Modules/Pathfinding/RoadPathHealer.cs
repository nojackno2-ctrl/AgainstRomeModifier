namespace AgainstRomeMapEditor.Modules.Pathfinding;

/// <summary>
/// 道路網與通行網格自動修復器：負責將偵測到的間隙轉為具備原子性、可復原之修復處方，並支援與編輯器 Session 整合。
/// </summary>
public static class RoadPathHealer
{
    /// <summary>
    /// 將道路中斷候選清單轉換為具體修復動作。
    /// </summary>
    public static IReadOnlyList<NavMeshRepairAction> CreateRepairActions(IReadOnlyList<RoadGapCandidate> candidates)
    {
        ArgumentNullException.ThrowIfNull(candidates);
        var actions = new List<NavMeshRepairAction>();

        for (int i = 0; i < candidates.Count; i++)
        {
            var candidate = candidates[i];
            var changes = new List<NavMeshTileChange>();

            foreach (var gap in candidate.GapTiles)
            {
                byte? newCollision = candidate.RequiresCollisionClear ? (byte)0 : null;
                changes.Add(new NavMeshTileChange(gap.TileX, gap.TileZ, candidate.SuggestedTexture, newCollision));
            }

            var mid = candidate.GapTiles.Count > 0 ? candidate.GapTiles[0] : candidate.Start;
            string titleZh = $"自動補齊道路間隙（{candidate.GapDistance} 格，{candidate.Style} 風格）";
            string titleEn = $"Heal Road Gap ({candidate.GapDistance} tiles, {candidate.Style} style)";
            string descZh = $"在座標 ({mid.TileX}, {mid.TileZ}) 填補圖塊「{candidate.SuggestedTexture}」" +
                           (candidate.RequiresCollisionClear ? " 並清除碰撞硬閘阻擋。" : "。");
            string descEn = $"Place tile '{candidate.SuggestedTexture}' at ({mid.TileX}, {mid.TileZ})" +
                           (candidate.RequiresCollisionClear ? " and clear collision blockage." : ".");

            actions.Add(new NavMeshRepairAction(
                $"road-heal-{i + 1}-{mid.TileX}-{mid.TileZ}",
                NavMeshIssueKind.RoadGap,
                titleZh,
                titleEn,
                descZh,
                descEn,
                changes,
                mid));
        }

        return actions;
    }

    /// <summary>
    /// 為孤立區域建立貫穿障礙的通行橋樑修復動作。
    /// </summary>
    public static NavMeshRepairAction? CreateBridgeAction(IsolatedRegion region, string roadTexture = "H_WEG1")
    {
        ArgumentNullException.ThrowIfNull(region);
        if (region.RecommendedBridgePoint is not { } bridge || region.BridgeDistance <= 0) return null;

        var changes = new List<NavMeshTileChange>
        {
            new(bridge.TileX, bridge.TileZ, roadTexture, (byte)0)
        };

        string titleZh = $"打通孤立區域過渡通道（跨度 {region.BridgeDistance} 格）";
        string titleEn = $"Open Passage to Isolated Region ({region.BridgeDistance} tiles)";
        string descZh = $"在座標 ({bridge.TileX}, {bridge.TileZ}) 清除阻擋硬閘並鋪設過渡道路，連接包含 {region.TileCount} 格之孤立陸地。";
        string descEn = $"Clear collision and place transition road at ({bridge.TileX}, {bridge.TileZ}) to reconnect {region.TileCount} isolated tiles.";

        return new NavMeshRepairAction(
            $"bridge-{region.ComponentId}-{bridge.TileX}-{bridge.TileZ}",
            NavMeshIssueKind.IsolatedLand,
            titleZh,
            titleEn,
            descZh,
            descEn,
            changes,
            bridge);
    }

    /// <summary>
    /// 套用修復動作至地圖編輯器 Session（支援材質與通行層）。
    /// </summary>
    internal static bool ApplyRepair(
        NavMeshRepairAction action,
        TerrainBlendEditSession blendSession,
        TerrainHeightEditSession? heightSession = null)
    {
        ArgumentNullException.ThrowIfNull(action);
        ArgumentNullException.ThrowIfNull(blendSession);

        bool textureChanged = false;
        bool collisionChanged = false;

        foreach (var change in action.Changes)
        {
            if (!string.IsNullOrWhiteSpace(change.NewTexture))
            {
                if (blendSession.StampTexture(change.TileX, change.TileZ, change.NewTexture) is not null)
                {
                    textureChanged = true;
                }
            }

            if (change.NewCollision.HasValue && heightSession is not null && heightSession.HasCollision)
            {
                var op = change.NewCollision.Value == 0 ? TerrainCollisionOperation.Clear : TerrainCollisionOperation.Block;
                var collisionRes = heightSession.PaintCollision(change.TileX + 0.5f, change.TileZ + 0.5f, 0.5f, op);
                if (collisionRes.Count > 0)
                {
                    collisionChanged = true;
                }
            }
        }

        if (textureChanged) blendSession.CommitStroke();
        if (collisionChanged && heightSession is not null) heightSession.CommitStroke();

        return textureChanged || collisionChanged;
    }
}
