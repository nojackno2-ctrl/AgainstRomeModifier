namespace AgainstRomeMapEditor.Modules.Diff;

/// <summary>
/// 局部還原範圍定義。
/// </summary>
public sealed record RollbackRegionScope(
    float MinWorldX,
    float MinWorldZ,
    float MaxWorldX,
    float MaxWorldZ,
    RollbackLayerFlags Layers = RollbackLayerFlags.All,
    bool EnableEdgeBlend = true,
    int EdgeBlendRadius = 2,
    IReadOnlySet<Guid>? SelectedObjectIds = null
)
{
    /// <summary>
    /// 建立全圖範圍的還原定義。
    /// </summary>
    public static RollbackRegionScope EntireMap(
        RollbackLayerFlags layers = RollbackLayerFlags.All,
        IReadOnlySet<Guid>? selectedObjectIds = null)
        => new(0, 0, 16384, 16384, layers, EnableEdgeBlend: false, EdgeBlendRadius: 0, selectedObjectIds);

    /// <summary>
    /// 檢查給定世界坐標是否落在所選空間範圍內。
    /// </summary>
    public bool ContainsWorld(float x, float z)
    {
        return x >= MinWorldX && x <= MaxWorldX && z >= MinWorldZ && z <= MaxWorldZ;
    }
}

/// <summary>
/// 局部還原交易成果包裹（支援一鍵套用與撤銷）。
/// </summary>
public sealed record RollbackTransaction(
    string Description,
    RollbackRegionScope Scope,
    IReadOnlyList<DiffTerrainSampleChange> HeightChanges,
    IReadOnlyList<TileTextureChange> TextureChanges,
    IReadOnlyList<CollisionTileChange> CollisionChanges,
    IReadOnlyList<DiffObjectItem> ObjectsToRestore,
    IReadOnlyList<Guid> ObjectsToRemove,
    IReadOnlyList<ObjectModification> ObjectsToRevert
)
{
    /// <summary>
    /// 本次還原包含之總變更項目數。
    /// </summary>
    public int TotalChangesCount =>
        HeightChanges.Count +
        TextureChanges.Count +
        CollisionChanges.Count +
        ObjectsToRestore.Count +
        ObjectsToRemove.Count +
        ObjectsToRevert.Count;

    /// <summary>
    /// 轉換為內部 TerrainSampleChange 清單以直接推入 TerrainHeightEditSession。
    /// </summary>
    internal IReadOnlyList<TerrainSampleChange> ToTerrainSampleChanges() =>
        HeightChanges.Select(c => c.ToSampleChange()).ToArray();
}

/// <summary>
/// 局部選擇性復原控制器 (SelectiveRollbackController)。
/// 支援使用者框選矩形區域或指定圖層，將當前地圖的特定區域恢復至歷史版本，保留其他區域改動。
/// </summary>
internal static class SelectiveRollbackController
{
    /// <summary>
    /// 依據選取範圍與圖層設定，計算精確的還原交易項目。
    /// </summary>
    public static RollbackTransaction ComputeRollback(
        MapVersionSnapshot baseline,
        MapVersionSnapshot current,
        MapDiffReport diff,
        RollbackRegionScope scope)
    {
        ArgumentNullException.ThrowIfNull(baseline);
        ArgumentNullException.ThrowIfNull(current);
        ArgumentNullException.ThrowIfNull(diff);
        ArgumentNullException.ThrowIfNull(scope);

        var heightChanges = new List<DiffTerrainSampleChange>();
        var textureChanges = new List<TileTextureChange>();
        var collisionChanges = new List<CollisionTileChange>();
        var objectsToRestore = new List<DiffObjectItem>();
        var objectsToRemove = new List<Guid>();
        var objectsToRevert = new List<ObjectModification>();

        // 1. 地形高度局部還原 (支援邊界羽化平滑過渡)
        if (scope.Layers.HasFlag(RollbackLayerFlags.Height) &&
            baseline.Heights is not null && current.Heights is not null &&
            baseline.HeightDimension > 0 && current.HeightDimension > 0)
        {
            ComputeHeightRollback(baseline, current, scope, heightChanges);
        }

        // 2. 地表材質局部還原
        if (scope.Layers.HasFlag(RollbackLayerFlags.Textures) &&
            baseline.Textures is not null && current.Textures is not null &&
            diff.Textures.ChangedTiles.Count > 0)
        {
            ComputeTextureRollback(baseline, current, scope, diff.Textures.ChangedTiles, textureChanges);
        }

        // 3. 通行碰撞遮罩局部還原
        if (scope.Layers.HasFlag(RollbackLayerFlags.Collision) &&
            baseline.Collision is not null && current.Collision is not null &&
            diff.Collision.Changes.Count > 0)
        {
            ComputeCollisionRollback(scope, diff.Collision.Changes, collisionChanges);
        }

        // 4. 場景物件局部還原
        if (scope.Layers.HasFlag(RollbackLayerFlags.Objects))
        {
            ComputeObjectRollback(scope, diff.Objects, objectsToRestore, objectsToRemove, objectsToRevert);
        }

        string desc = $"局部還原 [{scope.Layers}] 範圍 ({scope.MinWorldX:F0},{scope.MinWorldZ:F0}) ~ ({scope.MaxWorldX:F0},{scope.MaxWorldZ:F0})";

        return new RollbackTransaction(
            desc,
            scope,
            heightChanges,
            textureChanges,
            collisionChanges,
            objectsToRestore,
            objectsToRemove,
            objectsToRevert
        );
    }

    private static void ComputeHeightRollback(
        MapVersionSnapshot baseline,
        MapVersionSnapshot current,
        RollbackRegionScope scope,
        List<DiffTerrainSampleChange> output)
    {
        int dim = current.HeightDimension;
        if (dim != baseline.HeightDimension || current.Heights is null || baseline.Heights is null) return;

        float worldStep = 16384f / (dim - 1);

        int minVx = Math.Clamp((int)MathF.Floor(scope.MinWorldX / worldStep), 0, dim - 1);
        int maxVx = Math.Clamp((int)MathF.Ceiling(scope.MaxWorldX / worldStep), 0, dim - 1);
        int minVz = Math.Clamp((int)MathF.Floor(scope.MinWorldZ / worldStep), 0, dim - 1);
        int maxVz = Math.Clamp((int)MathF.Ceiling(scope.MaxWorldZ / worldStep), 0, dim - 1);

        int blendR = Math.Max(1, scope.EdgeBlendRadius);

        for (int vz = minVz; vz <= maxVz; vz++)
        {
            for (int vx = minVx; vx <= maxVx; vx++)
            {
                int index = vz * dim + vx;
                byte curH = current.Heights[index];
                byte baseH = baseline.Heights[index];

                if (curH == baseH) continue;

                byte targetH;

                if (scope.EnableEdgeBlend && blendR > 0)
                {
                    // 計算到選區邊緣的最小頂點距離
                    int distBorder = Math.Min(
                        Math.Min(vx - minVx, maxVx - vx),
                        Math.Min(vz - minVz, maxVz - vz)
                    );

                    if (distBorder < blendR)
                    {
                        // 餘弦羽化漸變權重 (0 ~ 1)
                        float t = (float)distBorder / blendR;
                        float w = 0.5f * (1.0f - MathF.Cos(MathF.PI * t));
                        float blended = baseH * w + curH * (1.0f - w);
                        targetH = (byte)Math.Clamp((int)MathF.Round(blended), 0, 255);
                    }
                    else
                    {
                        targetH = baseH;
                    }
                }
                else
                {
                    targetH = baseH;
                }

                if (targetH != curH)
                {
                    output.Add(new DiffTerrainSampleChange(index, curH, targetH));
                }
            }
        }
    }

    private static void ComputeTextureRollback(
        MapVersionSnapshot baseline,
        MapVersionSnapshot current,
        RollbackRegionScope scope,
        IReadOnlyList<TileTextureChange> changedTiles,
        List<TileTextureChange> output)
    {
        int tDim = current.TileDimension;
        float tileWorldSize = 16384f / tDim;

        int minTileX = Math.Clamp((int)MathF.Floor(scope.MinWorldX / tileWorldSize), 0, tDim - 1);
        int maxTileX = Math.Clamp((int)MathF.Ceiling(scope.MaxWorldX / tileWorldSize), 0, tDim - 1);
        int minTileZ = Math.Clamp((int)MathF.Floor(scope.MinWorldZ / tileWorldSize), 0, tDim - 1);
        int maxTileZ = Math.Clamp((int)MathF.Ceiling(scope.MaxWorldZ / tileWorldSize), 0, tDim - 1);

        foreach (var change in changedTiles)
        {
            if (change.TileX >= minTileX && change.TileX <= maxTileX &&
                change.TileY >= minTileZ && change.TileY <= maxTileZ)
            {
                // 反向還原：由 Current (NewTexture) 改回 Baseline (OldTexture)
                output.Add(new TileTextureChange(change.TileX, change.TileY, change.NewTexture, change.OldTexture));
            }
        }
    }

    private static void ComputeCollisionRollback(
        RollbackRegionScope scope,
        IReadOnlyList<CollisionTileChange> collisionChanges,
        List<CollisionTileChange> output)
    {
        float colWorldSize = 16384f / 256f;

        int minColX = Math.Clamp((int)MathF.Floor(scope.MinWorldX / colWorldSize), 0, 255);
        int maxColX = Math.Clamp((int)MathF.Ceiling(scope.MaxWorldX / colWorldSize), 0, 255);
        int minColZ = Math.Clamp((int)MathF.Floor(scope.MinWorldZ / colWorldSize), 0, 255);
        int maxColZ = Math.Clamp((int)MathF.Ceiling(scope.MaxWorldZ / colWorldSize), 0, 255);

        foreach (var change in collisionChanges)
        {
            if (change.TileX >= minColX && change.TileX <= maxColX &&
                change.TileY >= minColZ && change.TileY <= maxColZ)
            {
                // 反向還原
                output.Add(new CollisionTileChange(change.TileX, change.TileY, change.NewValue, change.OldValue));
            }
        }
    }

    private static void ComputeObjectRollback(
        RollbackRegionScope scope,
        ObjectDiffRecord objectDiff,
        List<DiffObjectItem> toRestore,
        List<Guid> toRemove,
        List<ObjectModification> toRevert)
    {
        // 1. 還原被刪除物件 (Restore Deleted)
        foreach (var obj in objectDiff.Deleted)
        {
            bool matchFilter = scope.SelectedObjectIds is null || scope.SelectedObjectIds.Contains(obj.Id);
            if (matchFilter && scope.ContainsWorld(obj.X, obj.Z))
            {
                toRestore.Add(obj);
            }
        }

        // 2. 移除新增物件 (Remove Added)
        foreach (var obj in objectDiff.Added)
        {
            bool matchFilter = scope.SelectedObjectIds is null || scope.SelectedObjectIds.Contains(obj.Id);
            if (matchFilter && scope.ContainsWorld(obj.X, obj.Z))
            {
                toRemove.Add(obj.Id);
            }
        }

        // 3. 還原修改/移動物件 (Revert Modified)
        foreach (var mod in objectDiff.Modified)
        {
            bool matchFilter = scope.SelectedObjectIds is null || scope.SelectedObjectIds.Contains(mod.Current.Id);
            bool inCurrent = scope.ContainsWorld(mod.Current.X, mod.Current.Z);
            bool inBaseline = scope.ContainsWorld(mod.Baseline.X, mod.Baseline.Z);

            if (matchFilter && (inCurrent || inBaseline))
            {
                toRevert.Add(mod);
            }
        }
    }
}
