namespace AgainstRomeMapEditor.Modules.Diff;

using AgainstRomeModifier.Scripting;

/// <summary>
/// 比對設定參數。
/// </summary>
public sealed record MapDiffOptions(
    float HeightTolerance = 0.0f,
    float PositionTolerance = 0.5f,
    float RotationTolerance = 1.0f,
    float SpatialMatchRadius = 150.0f
);

/// <summary>
/// 地圖歷史版本差異比對引擎 (逐圖層精確比對)。
/// </summary>
public static class MapDiffEngine
{
    /// <summary>
    /// 對兩個地圖快照進行逐圖層比對並產出差異報告。
    /// </summary>
    public static MapDiffReport Compare(
        MapVersionSnapshot baseline,
        MapVersionSnapshot current,
        MapDiffOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(baseline);
        ArgumentNullException.ThrowIfNull(current);
        options ??= new MapDiffOptions();

        DateTimeOffset now = DateTimeOffset.Now;

        HeightDiffRecord heightDiff = CompareHeightmaps(baseline, current, options);
        TextureDiffRecord textureDiff = CompareTextures(baseline, current);
        CollisionDiffRecord collisionDiff = CompareCollision(baseline, current);
        ObjectDiffRecord objectDiff = CompareObjects(baseline.Objects, current.Objects, options);
        EventDiffRecord eventDiff = CompareEvents(baseline.Events, current.Events);

        return new MapDiffReport(
            now,
            baseline.VersionLabel,
            current.VersionLabel,
            heightDiff,
            textureDiff,
            collisionDiff,
            objectDiff,
            eventDiff
        );
    }

    private static HeightDiffRecord CompareHeightmaps(
        MapVersionSnapshot baseline,
        MapVersionSnapshot current,
        MapDiffOptions options)
    {
        int dim = Math.Max(baseline.HeightDimension, current.HeightDimension);
        if (dim <= 0 || baseline.Heights is null || current.Heights is null)
        {
            return new HeightDiffRecord(0, Array.Empty<float>(), 0, 0, 0, 0, Array.Empty<HeightVertexChange>());
        }

        int bDim = baseline.HeightDimension;
        int cDim = current.HeightDimension;
        float bStep = baseline.HeightStep;
        float cStep = current.HeightStep;

        float[] deltas = new float[dim * dim];
        int changedCount = 0;
        float maxRaise = 0f;
        float maxLower = 0f;
        float totalVolume = 0f;
        var samples = new List<HeightVertexChange>();

        for (int y = 0; y < dim; y++)
        {
            for (int x = 0; x < dim; x++)
            {
                byte bVal = SampleHeight(baseline.Heights, bDim, dim, x, y);
                byte cVal = SampleHeight(current.Heights, cDim, dim, x, y);

                float bWorld = bVal * bStep;
                float cWorld = cVal * cStep;
                float delta = cWorld - bWorld;

                deltas[y * dim + x] = delta;

                if (MathF.Abs(delta) > options.HeightTolerance)
                {
                    changedCount++;
                    if (delta > 0)
                    {
                        if (delta > maxRaise) maxRaise = delta;
                    }
                    else
                    {
                        float lower = -delta;
                        if (lower > maxLower) maxLower = lower;
                    }
                    totalVolume += delta;

                    if (samples.Count < 500)
                    {
                        samples.Add(new HeightVertexChange(x, y, bVal, cVal, delta));
                    }
                }
            }
        }

        return new HeightDiffRecord(
            dim,
            deltas,
            changedCount,
            maxRaise,
            maxLower,
            totalVolume,
            samples
        );
    }

    private static byte SampleHeight(IReadOnlyList<byte> heights, int srcDim, int targetDim, int targetX, int targetY)
    {
        if (srcDim == targetDim)
        {
            int idx = targetY * srcDim + targetX;
            return (idx >= 0 && idx < heights.Count) ? heights[idx] : (byte)0;
        }

        float fx = (float)targetX / (targetDim - 1) * (srcDim - 1);
        float fy = (float)targetY / (targetDim - 1) * (srcDim - 1);
        int x0 = Math.Clamp((int)MathF.Floor(fx), 0, srcDim - 1);
        int y0 = Math.Clamp((int)MathF.Floor(fy), 0, srcDim - 1);
        int x1 = Math.Clamp(x0 + 1, 0, srcDim - 1);
        int y1 = Math.Clamp(y0 + 1, 0, srcDim - 1);

        float dx = fx - x0;
        float dy = fy - y0;

        float h00 = heights[y0 * srcDim + x0];
        float h10 = heights[y0 * srcDim + x1];
        float h01 = heights[y1 * srcDim + x0];
        float h11 = heights[y1 * srcDim + x1];

        float interp = (1 - dx) * (1 - dy) * h00 + dx * (1 - dy) * h10 + (1 - dx) * dy * h01 + dx * dy * h11;
        return (byte)Math.Clamp((int)MathF.Round(interp), 0, 255);
    }

    private static TextureDiffRecord CompareTextures(
        MapVersionSnapshot baseline,
        MapVersionSnapshot current)
    {
        int dim = Math.Max(baseline.TileDimension, current.TileDimension);
        if (dim <= 0 || baseline.Textures is null || current.Textures is null)
        {
            return new TextureDiffRecord(0, 0, Array.Empty<TileTextureChange>(), new Dictionary<(string, string), int>());
        }

        var changed = new List<TileTextureChange>();
        var transitions = new Dictionary<(string From, string To), int>();

        int bDim = baseline.TileDimension;
        int cDim = current.TileDimension;

        for (int y = 0; y < dim; y++)
        {
            for (int x = 0; x < dim; x++)
            {
                string bTex = (x < bDim && y < bDim) ? baseline.Textures[y * bDim + x] : string.Empty;
                string cTex = (x < cDim && y < cDim) ? current.Textures[y * cDim + x] : string.Empty;

                if (!string.Equals(bTex, cTex, StringComparison.OrdinalIgnoreCase))
                {
                    changed.Add(new TileTextureChange(x, y, bTex, cTex));
                    var key = (From: bTex, To: cTex);
                    transitions[key] = transitions.GetValueOrDefault(key) + 1;
                }
            }
        }

        return new TextureDiffRecord(dim, changed.Count, changed, transitions);
    }

    private static CollisionDiffRecord CompareCollision(
        MapVersionSnapshot baseline,
        MapVersionSnapshot current)
    {
        int dim = Math.Max(baseline.CollisionDimension, current.CollisionDimension);
        if (dim <= 0 || baseline.Collision is null || current.Collision is null)
        {
            return new CollisionDiffRecord(0, 0, 0, Array.Empty<CollisionTileChange>());
        }

        var changes = new List<CollisionTileChange>();
        int newlyBlocked = 0;
        int newlyCleared = 0;

        int bDim = baseline.CollisionDimension;
        int cDim = current.CollisionDimension;

        for (int y = 0; y < dim; y++)
        {
            for (int x = 0; x < dim; x++)
            {
                byte bVal = (x < bDim && y < bDim) ? baseline.Collision[y * bDim + x] : (byte)0;
                byte cVal = (x < cDim && y < cDim) ? current.Collision[y * cDim + x] : (byte)0;

                if (bVal != cVal)
                {
                    changes.Add(new CollisionTileChange(x, y, bVal, cVal));
                    if (bVal == 0 && cVal != 0) newlyBlocked++;
                    else if (bVal != 0 && cVal == 0) newlyCleared++;
                }
            }
        }

        return new CollisionDiffRecord(dim, newlyBlocked, newlyCleared, changes);
    }

    private static ObjectDiffRecord CompareObjects(
        IReadOnlyList<DiffObjectItem>? baselineList,
        IReadOnlyList<DiffObjectItem>? currentList,
        MapDiffOptions options)
    {
        baselineList ??= Array.Empty<DiffObjectItem>();
        currentList ??= Array.Empty<DiffObjectItem>();

        var baselineById = new Dictionary<Guid, DiffObjectItem>();
        var baselineUnmatched = new List<DiffObjectItem>();

        foreach (var b in baselineList)
        {
            if (b.Id != Guid.Empty) baselineById[b.Id] = b;
            else baselineUnmatched.Add(b);
        }

        var currentById = new Dictionary<Guid, DiffObjectItem>();
        var currentUnmatched = new List<DiffObjectItem>();

        foreach (var c in currentList)
        {
            if (c.Id != Guid.Empty) currentById[c.Id] = c;
            else currentUnmatched.Add(c);
        }

        var added = new List<DiffObjectItem>();
        var deleted = new List<DiffObjectItem>();
        var modified = new List<ObjectModification>();

        var matchedCurrentIds = new HashSet<Guid>();
        var matchedBaselineIds = new HashSet<Guid>();

        // 階段 1：持久 GUID 配對
        foreach (var kvp in currentById)
        {
            Guid id = kvp.Key;
            DiffObjectItem cur = kvp.Value;

            if (baselineById.TryGetValue(id, out DiffObjectItem? baseItem))
            {
                matchedCurrentIds.Add(id);
                matchedBaselineIds.Add(id);

                CheckObjectModification(baseItem, cur, options, modified);
            }
        }

        // 收集尚未透過 GUID 配對的物件
        foreach (var kvp in currentById)
        {
            if (!matchedCurrentIds.Contains(kvp.Key)) currentUnmatched.Add(kvp.Value);
        }
        foreach (var kvp in baselineById)
        {
            if (!matchedBaselineIds.Contains(kvp.Key)) baselineUnmatched.Add(kvp.Value);
        }

        // 階段 2：Slot / UID 配對（針對 DATA 世界物件）
        var matchedSlotBase = new HashSet<int>();
        var matchedSlotCur = new HashSet<int>();

        for (int i = 0; i < currentUnmatched.Count; i++)
        {
            var cur = currentUnmatched[i];
            if (cur.Slot < 0 || cur.Uid == 0) continue;

            for (int j = 0; j < baselineUnmatched.Count; j++)
            {
                if (matchedSlotBase.Contains(j)) continue;
                var baseItem = baselineUnmatched[j];

                if (baseItem.Slot == cur.Slot && baseItem.Uid == cur.Uid)
                {
                    matchedSlotCur.Add(i);
                    matchedSlotBase.Add(j);
                    CheckObjectModification(baseItem, cur, options, modified);
                    break;
                }
            }
        }

        var curRemaining = currentUnmatched.Where((_, idx) => !matchedSlotCur.Contains(idx)).ToList();
        var baseRemaining = baselineUnmatched.Where((_, idx) => !matchedSlotBase.Contains(idx)).ToList();

        // 階段 3：啟發式空間鄰近度與類型配對 (Heuristic Spatial Proximity Matching)
        var usedBaseRemaining = new HashSet<int>();
        var usedCurRemaining = new HashSet<int>();

        for (int i = 0; i < curRemaining.Count; i++)
        {
            var cur = curRemaining[i];
            int bestBaseIdx = -1;
            float bestDist = options.SpatialMatchRadius;

            for (int j = 0; j < baseRemaining.Count; j++)
            {
                if (usedBaseRemaining.Contains(j)) continue;
                var baseItem = baseRemaining[j];

                // 檢查同類型或同別名
                bool typeMatch = (cur.TypeId > 0 && cur.TypeId == baseItem.TypeId) ||
                                 (!string.IsNullOrEmpty(cur.Alias) && string.Equals(cur.Alias, baseItem.Alias, StringComparison.OrdinalIgnoreCase));

                if (typeMatch)
                {
                    float dist = cur.DistanceTo(baseItem);
                    if (dist < bestDist)
                    {
                        bestDist = dist;
                        bestBaseIdx = j;
                    }
                }
            }

            if (bestBaseIdx >= 0)
            {
                usedCurRemaining.Add(i);
                usedBaseRemaining.Add(bestBaseIdx);
                CheckObjectModification(baseRemaining[bestBaseIdx], cur, options, modified);
            }
        }

        // 階段 4：剩餘項目歸類為 Added 或 Deleted
        for (int i = 0; i < curRemaining.Count; i++)
        {
            if (!usedCurRemaining.Contains(i)) added.Add(curRemaining[i]);
        }

        for (int j = 0; j < baseRemaining.Count; j++)
        {
            if (!usedBaseRemaining.Contains(j)) deleted.Add(baseRemaining[j]);
        }

        return new ObjectDiffRecord(added, deleted, modified);
    }

    private static void CheckObjectModification(
        DiffObjectItem baseItem,
        DiffObjectItem cur,
        MapDiffOptions options,
        List<ObjectModification> modifiedList)
    {
        var changes = new List<string>();
        float dist = cur.DistanceTo(baseItem);
        float rotDelta = MathF.Abs(cur.Rotation - baseItem.Rotation);

        if (dist > options.PositionTolerance)
        {
            changes.Add($"坐標移動 ({baseItem.X:F0},{baseItem.Z:F0}) -> ({cur.X:F0},{cur.Z:F0}) 距離 {dist:F1}");
        }

        if (rotDelta > options.RotationTolerance)
        {
            changes.Add($"旋轉角度 {baseItem.Rotation:F0}° -> {cur.Rotation:F0}°");
        }

        if (baseItem.Team != cur.Team)
        {
            changes.Add($"隊伍 {baseItem.Team} -> {cur.Team}");
        }

        if (baseItem.UnitCount != cur.UnitCount)
        {
            changes.Add($"部隊人數 {baseItem.UnitCount} -> {cur.UnitCount}");
        }

        if (baseItem.Prebuilt != cur.Prebuilt)
        {
            changes.Add($"預建狀態 {baseItem.Prebuilt} -> {cur.Prebuilt}");
        }

        if (!string.Equals(baseItem.Alias, cur.Alias, StringComparison.OrdinalIgnoreCase) && !string.IsNullOrEmpty(baseItem.Alias) && !string.IsNullOrEmpty(cur.Alias))
        {
            changes.Add($"別名變更 {baseItem.Alias} -> {cur.Alias}");
        }

        if (changes.Count > 0)
        {
            modifiedList.Add(new ObjectModification(baseItem, cur, dist, rotDelta, changes));
        }
    }

    private static EventDiffRecord CompareEvents(
        IReadOnlyList<ScenarioEvent>? baselineEvents,
        IReadOnlyList<ScenarioEvent>? currentEvents)
    {
        baselineEvents ??= Array.Empty<ScenarioEvent>();
        currentEvents ??= Array.Empty<ScenarioEvent>();

        var baselineDict = baselineEvents.ToDictionary(e => e.Name, e => e, StringComparer.OrdinalIgnoreCase);
        var currentDict = currentEvents.ToDictionary(e => e.Name, e => e, StringComparer.OrdinalIgnoreCase);

        var added = new List<ScenarioEvent>();
        var deleted = new List<ScenarioEvent>();
        var modified = new List<EventModification>();

        foreach (var cur in currentEvents)
        {
            if (!baselineDict.TryGetValue(cur.Name, out ScenarioEvent? baseEvt))
            {
                added.Add(cur);
            }
            else
            {
                var detailChanges = new List<string>();

                if (cur.DelaySeconds != baseEvt.DelaySeconds)
                    detailChanges.Add($"延遲時間 {baseEvt.DelaySeconds}s -> {cur.DelaySeconds}s");
                if (cur.Repeat != baseEvt.Repeat)
                    detailChanges.Add($"循環重複 {baseEvt.Repeat} -> {cur.Repeat}");
                if (cur.Enabled != baseEvt.Enabled)
                    detailChanges.Add($"啟用狀態 {baseEvt.Enabled} -> {cur.Enabled}");

                if (cur.Conditions.Count != baseEvt.Conditions.Count || !cur.Conditions.SequenceEqual(baseEvt.Conditions))
                    detailChanges.Add($"觸發條件異動 ({baseEvt.Conditions.Count} 項 -> {cur.Conditions.Count} 項)");

                if (cur.Actions.Count != baseEvt.Actions.Count || !cur.Actions.SequenceEqual(baseEvt.Actions))
                    detailChanges.Add($"執行動作異動 ({baseEvt.Actions.Count} 項 -> {cur.Actions.Count} 項)");

                if (detailChanges.Count > 0)
                {
                    modified.Add(new EventModification(cur.Name, baseEvt, cur, detailChanges));
                }
            }
        }

        foreach (var b in baselineEvents)
        {
            if (!currentDict.ContainsKey(b.Name))
            {
                deleted.Add(b);
            }
        }

        return new EventDiffRecord(added, deleted, modified);
    }
}
