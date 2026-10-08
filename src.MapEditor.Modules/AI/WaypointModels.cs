namespace AgainstRomeMapEditor.Modules.AI;

/// <summary>路徑移動拓撲模式</summary>
public enum WaypointMovementMode
{
    /// <summary>循環巡邏（A -> B -> C -> A）</summary>
    Loop,
    /// <summary>往返折返巡邏（A -> B -> C -> B -> A）</summary>
    PingPong,
    /// <summary>埋伏待命（駐守原點，觸發後沿路線突擊）</summary>
    Ambush,
    /// <summary>單向行軍（抵達終點後就地防守）</summary>
    OneWay
}

/// <summary>部隊行軍姿態</summary>
public enum WaypointStance
{
    /// <summary>標準行軍（標準隊形與速限）</summary>
    NormalMarch,
    /// <summary>快速疾行（脫離戰鬥、全速推進）</summary>
    FastMarch,
    /// <summary>戒備警戒（搜尋視野內敵人、邊走邊偵查）</summary>
    AggressivePatrol,
    /// <summary>隱蔽埋伏（降低可見度、原地待命）</summary>
    StealthHold
}

/// <summary>路徑點節點定義</summary>
public sealed record WaypointNode(
    Guid Id,
    int OrderIndex,
    float WorldX,
    float WorldZ,
    float ToleranceRadius = 150f,
    int DwellTimeSeconds = 0,
    WaypointStance Stance = WaypointStance.NormalMarch,
    string? ActionTriggerName = null)
{
    public void Validate()
    {
        if (Id == Guid.Empty) throw new InvalidDataException("路徑點節點必須具備持久 ID。");
        if (!float.IsFinite(WorldX) || !float.IsFinite(WorldZ) || WorldX is < 0 or > 16383 || WorldZ is < 0 or > 16383)
            throw new ArgumentOutOfRangeException("WorldX/WorldZ", "路徑點座標必須介於 0 與 16383 之間。");
        if (ToleranceRadius is < 10f or > 2000f)
            throw new ArgumentOutOfRangeException(nameof(ToleranceRadius), "容差半徑必須介於 10 與 2000 之間。");
        if (DwellTimeSeconds is < 0 or > 3600)
            throw new ArgumentOutOfRangeException(nameof(DwellTimeSeconds), "停留時間必須介於 0 與 3600 秒之間。");
    }
}

/// <summary>巡邏與進攻路線（節點鏈）</summary>
public sealed record WaypointPath(
    Guid Id,
    string Name,
    WaypointMovementMode Mode,
    IReadOnlyList<WaypointNode> Nodes,
    IReadOnlyList<Guid> AssignedSpawnIds,
    bool BreakOnCombat = true,
    bool ReturnToStartOnLostTarget = true,
    float AmbushTriggerRadius = 800f,
    int LoopCount = -1)
{
    public void Validate()
    {
        if (Id == Guid.Empty) throw new InvalidDataException("路徑必須具備持久 ID。");
        if (string.IsNullOrWhiteSpace(Name)) throw new InvalidDataException("路徑名稱不能為空。");
        if (Nodes is null || Nodes.Count < 2)
            throw new InvalidDataException($"路徑「{Name}」至少需要 2 個路徑點節點。");
        if (AmbushTriggerRadius is < 50f or > 8000f)
            throw new ArgumentOutOfRangeException(nameof(AmbushTriggerRadius), "埋伏警戒半徑必須介於 50 與 8000 之間。");

        var ids = new HashSet<Guid>();
        for (int i = 0; i < Nodes.Count; i++)
        {
            var node = Nodes[i];
            node.Validate();
            if (!ids.Add(node.Id)) throw new InvalidDataException($"路徑「{Name}」包含重複的節點 ID。");
        }
    }
}

/// <summary>部隊巡邏與進攻路線規劃輔助器</summary>
public static class WaypointPathPlanner
{
    /// <summary>計算路徑總長度（以地圖世界座標單位為準）</summary>
    public static float CalculateTotalDistance(WaypointPath path)
    {
        ArgumentNullException.ThrowIfNull(path);
        if (path.Nodes.Count < 2) return 0f;

        float total = 0f;
        for (int i = 0; i < path.Nodes.Count - 1; i++)
        {
            float dx = path.Nodes[i + 1].WorldX - path.Nodes[i].WorldX;
            float dz = path.Nodes[i + 1].WorldZ - path.Nodes[i].WorldZ;
            total += MathF.Sqrt(dx * dx + dz * dz);
        }

        if (path.Mode == WaypointMovementMode.Loop && path.Nodes.Count >= 3)
        {
            float dx = path.Nodes[0].WorldX - path.Nodes[^1].WorldX;
            float dz = path.Nodes[0].WorldZ - path.Nodes[^1].WorldZ;
            total += MathF.Sqrt(dx * dx + dz * dz);
        }

        return total;
    }

    /// <summary>推進計算下一個節點索引</summary>
    public static int GetNextNodeIndex(WaypointPath path, int currentIndex, ref bool isReversing)
    {
        ArgumentNullException.ThrowIfNull(path);
        if (path.Nodes.Count == 0) return 0;
        if (path.Nodes.Count == 1) return 0;

        switch (path.Mode)
        {
            case WaypointMovementMode.Loop:
                return (currentIndex + 1) % path.Nodes.Count;

            case WaypointMovementMode.PingPong:
                if (isReversing)
                {
                    if (currentIndex <= 0)
                    {
                        isReversing = false;
                        return 1;
                    }
                    return currentIndex - 1;
                }
                else
                {
                    if (currentIndex >= path.Nodes.Count - 1)
                    {
                        isReversing = true;
                        return path.Nodes.Count - 2;
                    }
                    return currentIndex + 1;
                }

            case WaypointMovementMode.OneWay:
            case WaypointMovementMode.Ambush:
                return Math.Min(currentIndex + 1, path.Nodes.Count - 1);

            default:
                return (currentIndex + 1) % path.Nodes.Count;
        }
    }

    /// <summary>檢查整條路徑上所有節點的通行性</summary>
    public static IReadOnlyList<(int NodeIndex, string Reason)> ValidatePassability(
        WaypointPath path,
        Func<float, float, bool>? isPassablePredicate)
    {
        ArgumentNullException.ThrowIfNull(path);
        var issues = new List<(int, string)>();

        for (int i = 0; i < path.Nodes.Count; i++)
        {
            var node = path.Nodes[i];
            if (node.WorldX is < 0 or > 16383 || node.WorldZ is < 0 or > 16383)
            {
                issues.Add((i, $"節點 {i} 座標 ({node.WorldX:F0}, {node.WorldZ:F0}) 超出地圖範圍。"));
                continue;
            }

            if (isPassablePredicate is not null && !isPassablePredicate(node.WorldX, node.WorldZ))
            {
                issues.Add((i, $"節點 {i} 座標 ({node.WorldX:F0}, {node.WorldZ:F0}) 位於不可通行區域（如深水或陡峭山壁）。"));
            }
        }

        return issues;
    }
}
