using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.Modules.Fortification;

/// <summary>
/// 單次防禦工事筆畫的複合原子事務記錄（支援跨模組統一 Undo/Redo）。
/// </summary>
internal sealed record FortificationTransaction(
    IReadOnlyList<FortificationPlacement> Placements,
    IReadOnlyList<CollisionPixelChange> CollisionChanges,
    IReadOnlyList<(int TileX, int TileZ, string BeforeTexture, string AfterTexture)> TextureChanges);

/// <summary>
/// 防禦工事編輯會話協同器 (FortificationEditSessionCoordinator)。
/// 將城牆繪製規劃（WallStrokePlanner）、放置物件會話（PlacementEditSession）、
/// 通行網格同步（FortificationPassabilitySync）與地表基座紋理烘焙（TerrainBlendEditSession）
/// 整合為單一事務單元，保證筆畫提交與撤銷時資料一致性。
/// </summary>
internal sealed class FortificationEditSessionCoordinator
{
    private readonly Stack<FortificationTransaction> _undoStack = new();
    private readonly Stack<FortificationTransaction> _redoStack = new();

    public bool CanUndo => _undoStack.Count > 0;
    public bool CanRedo => _redoStack.Count > 0;

    /// <summary>
    /// 執行並提交防禦工事筆畫。
    /// </summary>
    public FortificationTransaction CommitStroke(
        WallStrokePlan plan,
        PlacementEditSession placementSession,
        byte[]? collisionGrid,
        int collisionSize,
        int mapDimension,
        Func<int, int, string>? getTextureAtTile = null,
        Action<int, int, string>? setTextureAtTile = null)
    {
        ArgumentNullException.ThrowIfNull(plan);
        ArgumentNullException.ThrowIfNull(placementSession);

        if (!plan.Succeeded || plan.Placements.Count == 0)
        {
            return new FortificationTransaction([], [], []);
        }

        // 1. 同步放置物件 (PlacementEditSession)
        foreach (var p in plan.Placements)
        {
            var sdlType = new SdlObjectType(
                NameDef: p.NameDef,
                Definition: 1,
                Category: SdlObjectCategory.Building,
                Tribe: SdlObjectCatalog.Tribe(p.NameDef),
                Occurrences: 1,
                TemplateFields: new Dictionary<string, string>
                {
                    ["namedef"] = p.NameDef,
                    ["team"] = p.Team.ToString(),
                    ["angle"] = p.AngleDeg.ToString("0.00"),
                    ["onload"] = "1"
                });

            var placed = new SdlPlacedObject(
                Type: sdlType,
                WorldX: p.WorldX,
                WorldY: p.WorldY,
                WorldZ: p.WorldZ,
                Team: p.Team,
                Angle: p.AngleDeg)
            {
                ScenarioId = p.Id
            };

            placementSession.Add(placed);
        }

        // 2. 同步通行網格 (FortificationPassabilitySync)
        IReadOnlyList<CollisionPixelChange> collisionChanges = Array.Empty<CollisionPixelChange>();
        if (collisionGrid is not null && collisionSize > 0)
        {
            collisionChanges = FortificationPassabilitySync.SynchronizePassability(
                collisionGrid, collisionSize, mapDimension, plan);
        }

        // 3. 同步地表基座材質 (若有指定)
        var textureChanges = new List<(int TileX, int TileZ, string Before, string After)>();
        if (setTextureAtTile is not null && getTextureAtTile is not null)
        {
            foreach (var p in plan.Placements)
            {
                if (!string.IsNullOrEmpty(p.FoundationTexture))
                {
                    string before = getTextureAtTile(p.TileX, p.TileZ);
                    string after = p.FoundationTexture;
                    if (!string.Equals(before, after, StringComparison.OrdinalIgnoreCase))
                    {
                        setTextureAtTile(p.TileX, p.TileZ, after);
                        textureChanges.Add((p.TileX, p.TileZ, before, after));
                    }
                }
            }
        }

        var transaction = new FortificationTransaction(plan.Placements, collisionChanges, textureChanges);
        _undoStack.Push(transaction);
        _redoStack.Clear();

        return transaction;
    }

    /// <summary>
    /// 撤銷上一筆防禦工事筆畫。
    /// </summary>
    public bool Undo(
        PlacementEditSession placementSession,
        byte[]? collisionGrid,
        Action<int, int, string>? setTextureAtTile = null)
    {
        if (!_undoStack.TryPop(out var transaction)) return false;

        // 1. 還原放置物件（依 GUID 移除）
        var idsToRemove = new HashSet<Guid>(transaction.Placements.Select(p => p.Id));
        for (int i = placementSession.Count - 1; i >= 0; i--)
        {
            if (idsToRemove.Contains(placementSession[i].ScenarioId))
            {
                placementSession.RemoveAt(i);
            }
        }

        // 2. 還原通行網格
        if (collisionGrid is not null)
        {
            for (int i = transaction.CollisionChanges.Count - 1; i >= 0; i--)
            {
                var change = transaction.CollisionChanges[i];
                if (change.Index >= 0 && change.Index < collisionGrid.Length)
                {
                    collisionGrid[change.Index] = change.Before;
                }
            }
        }

        // 3. 還原地表材質
        if (setTextureAtTile is not null)
        {
            for (int i = transaction.TextureChanges.Count - 1; i >= 0; i--)
            {
                var tc = transaction.TextureChanges[i];
                setTextureAtTile(tc.TileX, tc.TileZ, tc.BeforeTexture);
            }
        }

        _redoStack.Push(transaction);
        return true;
    }

    /// <summary>
    /// 重做上一筆已撤銷的防禦工事筆畫。
    /// </summary>
    public bool Redo(
        PlacementEditSession placementSession,
        byte[]? collisionGrid,
        Action<int, int, string>? setTextureAtTile = null)
    {
        if (!_redoStack.TryPop(out var transaction)) return false;

        // 1. 重新加入放置物件
        foreach (var p in transaction.Placements)
        {
            var sdlType = new SdlObjectType(
                NameDef: p.NameDef,
                Definition: 1,
                Category: SdlObjectCategory.Building,
                Tribe: SdlObjectCatalog.Tribe(p.NameDef),
                Occurrences: 1,
                TemplateFields: new Dictionary<string, string>
                {
                    ["namedef"] = p.NameDef,
                    ["team"] = p.Team.ToString(),
                    ["angle"] = p.AngleDeg.ToString("0.00"),
                    ["onload"] = "1"
                });

            var placed = new SdlPlacedObject(
                Type: sdlType,
                WorldX: p.WorldX,
                WorldY: p.WorldY,
                WorldZ: p.WorldZ,
                Team: p.Team,
                Angle: p.AngleDeg)
            {
                ScenarioId = p.Id
            };

            placementSession.Add(placed);
        }

        // 2. 重新套用通行網格
        if (collisionGrid is not null)
        {
            foreach (var change in transaction.CollisionChanges)
            {
                if (change.Index >= 0 && change.Index < collisionGrid.Length)
                {
                    collisionGrid[change.Index] = change.After;
                }
            }
        }

        // 3. 重新套用地表材質
        if (setTextureAtTile is not null)
        {
            foreach (var tc in transaction.TextureChanges)
            {
                setTextureAtTile(tc.TileX, tc.TileZ, tc.AfterTexture);
            }
        }

        _undoStack.Push(transaction);
        return true;
    }
}
