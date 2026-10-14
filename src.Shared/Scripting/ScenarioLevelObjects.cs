using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Scripting;

public sealed record ScenarioFamilyCopyRequest(int SourceSlot, uint SourceUid)
{
    public bool IsNativeSource { get; init; }

    public ScenarioFamilyCopyRequest(int sourceSlot, uint sourceUid, bool isNativeSource)
        : this(sourceSlot, sourceUid)
    {
        IsNativeSource = isNativeSource;
    }
}

public sealed record ScenarioCandidateResult(
    LevelObjectStore Store,
    ScenarioDocument Scenario,
    IReadOnlyList<ScenarioSpawn> Skipped);

/// <summary>
/// 把場景中的預建物件（建築）寫進地圖 DATA；一般物件以範本重建，既有獨立群組平移時保留槽位與 UID。
/// 這與官方地圖的建築相同，開局即為完工狀態（以 s_createObj 生成則是 0% 工地）。
/// </summary>
public static class ScenarioLevelObjects
{
    public static int NormalizeAngle(int angle) => ((angle % 360) + 360) % 360;

    public static int RadiansToDisplayAngle(float radians)
    {
        double deg = (double)radians * (180d / Math.PI);
        int rounded = (int)Math.Round(((deg % 360d) + 360d) % 360d);
        return rounded % 360;
    }

    /// <summary>
    /// 套用到 <paramref name="store"/>（呼叫端負責寫回）；回傳找不到範本而未寫入的項目。
    /// <paramref name="next"/>.DataSlots 會改為本次寫入的槽位。
    /// </summary>
    public static IReadOnlyList<ScenarioSpawn> Apply(LevelObjectStore store, ScenarioDocument previous, ScenarioDocument next,
        Func<ScenarioSpawn, LevelObjectTemplate?> templateFor)
    {
        ArgumentNullException.ThrowIfNull(store);
        ArgumentNullException.ThrowIfNull(previous);
        ArgumentNullException.ThrowIfNull(next);
        ArgumentNullException.ThrowIfNull(templateFor);

        ScenarioDocument.ValidateSpawns(previous.Spawns);
        ScenarioDocument.ValidateSpawns(next.Spawns);
        ScenarioObjectIdentity.Prepare(next, validateBindings: false);
        return ApplyCore(store, previous, next, templateFor);
    }

    /// <summary>
    /// 候選 Scenario 交易入口：來源 <paramref name="store"/>、<paramref name="previous"/> 與 <paramref name="next"/>
    /// 在成功或失敗時均保持不變。回傳 detached Store、detached ScenarioDocument 與 Skipped 清單。
    /// 若提供 <paramref name="familyCopies"/>，則依交易開始時的來源快照依序建立副本並整合至候選狀態。
    /// </summary>
    public static ScenarioCandidateResult ApplyToCandidate(
        LevelObjectStore store,
        ScenarioDocument previous,
        ScenarioDocument next,
        Func<ScenarioSpawn, LevelObjectTemplate?> templateFor,
        IReadOnlyDictionary<Guid, ScenarioFamilyCopyRequest>? familyCopies = null)
        => ApplyToCandidate(store, previous, next, templateFor, familyCopies, null);

    /// <summary>
    /// 候選 Scenario 交易入口（含原生別名解析器）：支援來源為原圖未綁定原生根節點的高保真複製。
    /// </summary>
    public static ScenarioCandidateResult ApplyToCandidate(
        LevelObjectStore store,
        ScenarioDocument previous,
        ScenarioDocument next,
        Func<ScenarioSpawn, LevelObjectTemplate?> templateFor,
        IReadOnlyDictionary<Guid, ScenarioFamilyCopyRequest>? familyCopies,
        Func<int, string?>? nativeAliasResolver)
    {
        ArgumentNullException.ThrowIfNull(store);
        ArgumentNullException.ThrowIfNull(previous);
        ArgumentNullException.ThrowIfNull(next);
        ArgumentNullException.ThrowIfNull(templateFor);

        var detachedPrevious = CloneDocument(previous);
        var detachedNext = CloneDocument(next);

        ScenarioDocument.ValidateSpawns(detachedPrevious.Spawns);
        ScenarioDocument.ValidateSpawns(detachedNext.Spawns);
        ScenarioObjectIdentity.Prepare(detachedPrevious, validateBindings: false);
        ScenarioObjectIdentity.Prepare(detachedNext, validateBindings: false);

        var workingStore = store.CloneForEditing();

        if (familyCopies is not null && familyCopies.Count > 0)
        {
            var nextSpawns = detachedNext.Spawns.ToDictionary(spawn => spawn.Id);
            var previousSpawns = detachedPrevious.Spawns.ToDictionary(spawn => spawn.Id);

            var initialWorldObjects = store.Objects().ToDictionary(obj => obj.Slot);

            // 1. 預檢所有 copy request 契約
            foreach (var (targetGuid, request) in familyCopies)
            {
                if (targetGuid == Guid.Empty)
                    throw new InvalidDataException("副本請求的目標 Guid 不能為空。");
                if (detachedPrevious.Spawns.Any(s => s.Id == targetGuid) || detachedPrevious.DataSlots.Any(b => b.SpawnId == targetGuid))
                    throw new InvalidDataException("副本目標 Guid 不能是既有場景物件。");
                if (!nextSpawns.TryGetValue(targetGuid, out var targetSpawn) || !targetSpawn.Prebuilt)
                    throw new InvalidDataException("副本請求必須對應至 next 中新增且有效的預建物件。");
                if (!float.IsFinite(targetSpawn.X) || !float.IsFinite(targetSpawn.Y) || !float.IsFinite(targetSpawn.Z))
                    throw new InvalidDataException("副本目標座標必須為有限數值。");
                if (targetSpawn.Team is < 0 or > 8)
                    throw new InvalidDataException("預建物件的隊伍必須介於 0 與 8。");

                if (!initialWorldObjects.TryGetValue(request.SourceSlot, out var sourceRoot) || sourceRoot.Uid != request.SourceUid)
                    throw new InvalidDataException($"副本來源槽位 {request.SourceSlot} 的 root UID 無效。");

                var sourceFamily = store.FamilyAt(request.SourceSlot, request.SourceUid);
                if (sourceFamily is null)
                    throw new InvalidDataException($"副本來源槽位 {request.SourceSlot} 的群組無效或非獨立支援群組。");

                if (request.IsNativeSource)
                {
                    // 原生 DATA root 請求契約
                    if (detachedPrevious.DataSlots.Any(b => b.Slot == request.SourceSlot && b.Uid == request.SourceUid && b.SpawnId != Guid.Empty))
                        throw new InvalidDataException($"原生副本來源槽位 {request.SourceSlot} 已存在場景綁定，應使用一般副本請求。");

                    if (nativeAliasResolver is null)
                        throw new InvalidDataException("複製原生群組時必須提供可信原生別名解析器。");

                    string? resolvedAlias = nativeAliasResolver(sourceRoot.TypeId);
                    if (string.IsNullOrWhiteSpace(resolvedAlias) || !resolvedAlias.Equals(targetSpawn.Alias, StringComparison.OrdinalIgnoreCase))
                        throw new InvalidDataException($"原生副本來源 TypeId {sourceRoot.TypeId} 解析的別名與目標別名不一致。");

                    if (sourceRoot.Team != targetSpawn.Team)
                        throw new InvalidDataException("群組副本目前只能平移；變更隊伍或方向尚未支援。");

                    if (targetSpawn.RawRotation is not { } targetRaw || !float.IsFinite(targetRaw))
                        throw new InvalidDataException("原生群組副本目標必須明確提供有限的原生旋轉弧度。");

                    if (!float.IsFinite(sourceRoot.Rotation) || BitConverter.SingleToInt32Bits(targetRaw) != BitConverter.SingleToInt32Bits(sourceRoot.Rotation))
                        throw new InvalidDataException("原生群組副本目標旋轉弧度必須與來源 bit-exact 一致。");

                    int expectedDisplayAngle = RadiansToDisplayAngle(sourceRoot.Rotation);
                    if (NormalizeAngle(targetSpawn.Angle) != expectedDisplayAngle)
                        throw new InvalidDataException("群組副本目前只能平移；變更隊伍或方向尚未支援。");
                }
                else
                {
                    // 現有 ordinary bound 請求契約
                    var matchingPreviousBindings = detachedPrevious.DataSlots
                        .Where(b => b.Slot == request.SourceSlot && b.Uid == request.SourceUid && b.SpawnId != Guid.Empty).ToList();
                    if (matchingPreviousBindings.Count != 1)
                        throw new InvalidDataException($"副本來源槽位 {request.SourceSlot} 在 previous 中沒有唯一有效的綁定。");
                    var sourceBinding = matchingPreviousBindings[0];

                    if (!previousSpawns.TryGetValue(sourceBinding.SpawnId, out var sourceSpawn) || !sourceSpawn.Prebuilt)
                        throw new InvalidDataException("副本來源在 previous 中必須是預建物件。");

                    if (!sourceSpawn.Alias.Equals(targetSpawn.Alias, StringComparison.OrdinalIgnoreCase))
                        throw new InvalidDataException("副本物件的別名必須與來源一致。");

                    if (sourceSpawn.Team != targetSpawn.Team || sourceRoot.Team != targetSpawn.Team)
                        throw new InvalidDataException("群組副本目前只能平移；變更隊伍或方向尚未支援。");

                    if (sourceSpawn.RawRotation is { } sourceRaw || targetSpawn.RawRotation is { } targetRaw)
                    {
                        if (sourceSpawn.RawRotation is not { } sRaw || targetSpawn.RawRotation is not { } tRaw
                            || !float.IsFinite(sRaw) || !float.IsFinite(tRaw)
                            || BitConverter.SingleToInt32Bits(sRaw) != BitConverter.SingleToInt32Bits(tRaw)
                            || BitConverter.SingleToInt32Bits(sourceRoot.Rotation) != BitConverter.SingleToInt32Bits(tRaw)
                            || NormalizeAngle(sourceSpawn.Angle) != NormalizeAngle(targetSpawn.Angle))
                            throw new InvalidDataException("群組副本目前只能平移；變更隊伍或方向尚未支援。");
                    }
                    else
                    {
                        double actualAngle = sourceRoot.Rotation * (180d / Math.PI);
                        double desiredTargetAngle = NormalizeAngle(targetSpawn.Angle);
                        double angleDifference = Math.Abs(((actualAngle - desiredTargetAngle + 540d) % 360d + 360d) % 360d - 180d);

                        if (NormalizeAngle(sourceSpawn.Angle) != desiredTargetAngle
                            || !double.IsFinite(actualAngle) || angleDifference > .001d)
                            throw new InvalidDataException("群組副本目前只能平移；變更隊伍或方向尚未支援。");
                    }
                }
            }

            // 2. 依 sorted TargetGuid 順序建立 snapshot 副本
            var sortedRequests = familyCopies.OrderBy(pair => pair.Key).ToList();
            var plannedUids = new HashSet<uint>();
            var plannedSlots = new HashSet<int>();

            foreach (var (targetGuid, request) in sortedRequests)
            {
                var targetSpawn = nextSpawns[targetGuid];
                var sourceRoot = initialWorldObjects[request.SourceSlot];
                int dx = FixedDelta(sourceRoot.X, targetSpawn.X);
                int dy = FixedDelta(sourceRoot.Y, targetSpawn.Y);
                int dz = FixedDelta(sourceRoot.Z, targetSpawn.Z);

                var copyCandidate = workingStore.CreateFamilyCopyCandidate(request.SourceSlot, request.SourceUid, dx, dy, dz);
                if (copyCandidate is null)
                    throw new InvalidOperationException("建立群組副本候選失敗（池容量不足或配置無法滿足）。");

                // 驗證 sequential allocation 不能共用 UID 或 slots
                foreach (int slot in copyCandidate.Allocation.Objects.Values)
                {
                    if (!plannedSlots.Add(slot))
                        throw new InvalidOperationException("多個群組副本之間發生槽位衝突。");
                }
                foreach (uint uid in copyCandidate.Allocation.Uids.Values)
                {
                    if (!plannedUids.Add(uid))
                        throw new InvalidOperationException("多個群組副本之間發生 UID 衝突。");
                }

                workingStore = copyCandidate.Store;

                // 根 binding 僅新 root，children 與原生來源不要新增 scenario binding
                var newBinding = new ScenarioDataSlot(copyCandidate.RootSlot, copyCandidate.RootUid) { SpawnId = targetGuid };
                detachedPrevious.Spawns.Add(targetSpawn);
                detachedPrevious.DataSlots.Add(newBinding);
            }
        }

        var skipped = ApplyCore(workingStore, detachedPrevious, detachedNext, templateFor,
            familyCopies?.Keys.ToHashSet());
        return new ScenarioCandidateResult(workingStore, detachedNext, skipped);
    }

    private static IReadOnlyList<ScenarioSpawn> ApplyCore(
        LevelObjectStore store,
        ScenarioDocument previous,
        ScenarioDocument next,
        Func<ScenarioSpawn, LevelObjectTemplate?> templateFor,
        IReadOnlySet<Guid>? copiedIds = null)
    {
        foreach (var spawn in next.Spawns.Where(spawn => spawn.Prebuilt))
            if (spawn.Team is < 0 or > 8) throw new InvalidDataException("預建物件的隊伍必須介於 0 與 8。");

        var ownedTemplates = new Dictionary<Guid, LevelObjectTemplate>();
        var retainedFamilies = new Dictionary<Guid, (ScenarioDataSlot Binding, int X, int Y, int Z)>();
        var retainedSlots = new HashSet<int>();
        var familyMembers = new HashSet<int>();
        var currentWorldObjects = store.Objects().ToDictionary(obj => obj.Slot);

        // Preflight & build candidate retained families before any store mutation
        foreach (ScenarioDataSlot binding in previous.DataSlots.Where(binding => binding.SpawnId != Guid.Empty))
        {
            ScenarioSpawn? old = previous.Spawns.SingleOrDefault(spawn => spawn.Id == binding.SpawnId && spawn.Prebuilt);
            ScenarioSpawn? current = next.Spawns.SingleOrDefault(spawn => spawn.Id == binding.SpawnId && spawn.Prebuilt);
            if (old is null || current is null || !old.Alias.Equals(current.Alias, StringComparison.OrdinalIgnoreCase)) continue;
            if (copiedIds?.Contains(binding.SpawnId) != true && old.RawRotation is null && current.RawRotation is null
                && store.OwnedTemplate(binding.Slot, binding.Uid) is { } owned)
            { ownedTemplates.TryAdd(binding.SpawnId, owned); continue; }
            if (!currentWorldObjects.TryGetValue(binding.Slot, out var root) || root.Uid != binding.Uid) continue;
            var family = store.FamilyAt(binding.Slot, binding.Uid);
            if (family is null) throw new InvalidDataException($"預建物件槽位 {binding.Slot} 的群組包含失效或共用資料，無法移動。");

            if (old.Team != current.Team || root.Team != current.Team)
                throw new InvalidDataException($"預建物件槽位 {binding.Slot} 的群組目前只能平移；變更隊伍或方向尚未支援。");

            double desiredAngle = NormalizeAngle(current.Angle);
            if (NormalizeAngle(old.Angle) != desiredAngle)
                throw new InvalidDataException($"預建物件槽位 {binding.Slot} 的群組目前只能平移；變更隊伍或方向尚未支援。");

            if (current.RawRotation is { } currentRaw || old.RawRotation is { } oldRaw)
            {
                if (current.RawRotation is not { } cRaw || old.RawRotation is not { } oRaw
                    || !float.IsFinite(cRaw) || !float.IsFinite(oRaw)
                    || BitConverter.SingleToInt32Bits(cRaw) != BitConverter.SingleToInt32Bits(oRaw)
                    || BitConverter.SingleToInt32Bits(root.Rotation) != BitConverter.SingleToInt32Bits(cRaw)
                    || desiredAngle != RadiansToDisplayAngle(cRaw))
                    throw new InvalidDataException($"預建物件槽位 {binding.Slot} 的群組目前只能平移；變更隊伍或方向尚未支援。");
            }
            else
            {
                double actualAngle = root.Rotation * (180d / Math.PI);
                double angleDifference = Math.Abs(((actualAngle - desiredAngle + 540d) % 360d + 360d) % 360d - 180d);
                if (!double.IsFinite(actualAngle) || angleDifference > .001d)
                    throw new InvalidDataException($"預建物件槽位 {binding.Slot} 的群組目前只能平移；變更隊伍或方向尚未支援。");
            }

            if (family.ObjectSlots.Any(familyMembers.Contains) || !retainedSlots.Add(binding.Slot)
                || retainedFamilies.ContainsKey(binding.SpawnId))
                throw new InvalidDataException("預建物件群組具有重複或重疊的場景綁定。");
            familyMembers.UnionWith(family.ObjectSlots);
            retainedFamilies.Add(binding.SpawnId, (binding, FixedDelta(root.X, current.X), FixedDelta(root.Y, current.Y), FixedDelta(root.Z, current.Z)));
        }

        // Native metadata preflight BEFORE ANY FIRST MUTATION on store:
        // 1. All next prebuilt spawns carrying RawRotation must be strictly verified as retained families
        foreach (var spawn in next.Spawns.Where(spawn => spawn.Prebuilt && spawn.RawRotation is not null))
        {
            if (!retainedFamilies.ContainsKey(spawn.Id))
                throw new InvalidDataException($"預建物件 {spawn.Id} 具有未驗證的原生旋轉弧度，無法轉換為範本物件。");
        }

        // 2. All previous prebuilt spawns carrying RawRotation where next has a matching prebuilt spawn with the same Id
        // must also be successfully retained (rejects alias changes, dropped RawRotation, stale UIDs, missing roots, etc.)
        var nextPrebuiltSpawns = next.Spawns.Where(s => s.Prebuilt).ToDictionary(s => s.Id);
        foreach (var oldSpawn in previous.Spawns.Where(s => s.Prebuilt && s.RawRotation is not null))
        {
            if (nextPrebuiltSpawns.ContainsKey(oldSpawn.Id) && !retainedFamilies.ContainsKey(oldSpawn.Id))
                throw new InvalidDataException($"原生預建物件 {oldSpawn.Id} 的場景綁定、別名或原生狀態無效，無法安全更新。");
        }

        // 3. Any previous prebuilt spawn carrying RawRotation where next has any spawn with the same Id (e.g. Prebuilt dropped to false)
        // must not silently drop raw source and early-continue
        var nextAllSpawns = next.Spawns.ToDictionary(s => s.Id);
        foreach (var oldSpawn in previous.Spawns.Where(s => s.Prebuilt && s.RawRotation is not null))
        {
            if (nextAllSpawns.TryGetValue(oldSpawn.Id, out var nextSpawn) && !nextSpawn.Prebuilt)
                throw new InvalidDataException($"原生預建物件 {oldSpawn.Id} 無法轉換為非預建物件。");
        }

        foreach (ScenarioDataSlot owned in previous.DataSlots)
            if (!retainedSlots.Contains(owned.Slot) && store.UidAt(owned.Slot) == owned.Uid && !store.RemoveIfUid(owned.Slot, owned.Uid)
                && (next.Spawns.Any(spawn => spawn.Id == owned.SpawnId && spawn.Prebuilt)
                    || !store.RemoveFamilyIfUid(owned.Slot, owned.Uid)))
                throw new InvalidDataException($"預建物件槽位 {owned.Slot} 含有連結或共用資料，無法安全更新；請檢查地圖物件資料。");

        var slots = new List<ScenarioDataSlot>();
        var skipped = new List<ScenarioSpawn>();
        foreach (ScenarioSpawn spawn in next.Spawns.Where(spawn => spawn.Prebuilt))
        {
            if (retainedFamilies.TryGetValue(spawn.Id, out var retained))
            {
                if ((retained.X != 0 || retained.Y != 0 || retained.Z != 0)
                    && !store.TranslateFamilyIfUidFixed(retained.Binding.Slot, retained.Binding.Uid, retained.X, retained.Y, retained.Z))
                    throw new InvalidDataException("預建物件群組在儲存期間已變更，無法移動。");
                slots.Add(retained.Binding);
                continue;
            }
            if (spawn.RawRotation is not null)
                throw new InvalidDataException($"預建物件 {spawn.Id} 具有未驗證的原生旋轉弧度，無法轉換為範本物件。");
            if ((ownedTemplates.GetValueOrDefault(spawn.Id) ?? templateFor(spawn)) is not { } template) { skipped.Add(spawn); continue; }
            float rotation = (float)(((spawn.Angle % 360) + 360) % 360 * Math.PI / 180);
            int slot = store.Add(template, spawn.X, spawn.Y, spawn.Z, rotation, spawn.Team);
            if (slot < 0) throw new InvalidOperationException("地圖的物件、位置或關聯資料池空間不足，無法再新增。");
            slots.Add(new ScenarioDataSlot(slot, store.UidAt(slot)!.Value) { SpawnId = spawn.Id });
        }
        next.DataSlots = slots;
        return skipped;
    }

    private static ScenarioDocument CloneDocument(ScenarioDocument doc)
    {
        return new ScenarioDocument
        {
            Version = doc.Version,
            Spawns = doc.Spawns.Select(s => s with { }).ToList(),
            DataSlots = doc.DataSlots.Select(d => d with { }).ToList(),
            Events = doc.Events.Select(e => (e with { }) with
            {
                Actions = e.Actions.Select(a => a with { }).ToList(),
                Conditions = e.Conditions.Select(c => c with { }).ToList()
            }).ToList()
        };
    }
    private static int FixedDelta(float source, float target)
    {
        double value = ((double)target - source) * 65536d;
        if (!double.IsFinite(value) || value < int.MinValue || value > int.MaxValue || value != Math.Truncate(value))
            throw new InvalidDataException("群組移動的座標精度或距離超出目前支援範圍。");
        return (int)value;
    }
}
