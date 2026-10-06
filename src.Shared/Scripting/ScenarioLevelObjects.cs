using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Scripting;

/// <summary>
/// 把場景中的預建物件（建築）寫進地圖 DATA：先移除上次由編輯器寫入、uid 仍相符的槽位，再以範本重新加入並記錄新槽位。
/// 這與官方地圖的建築相同，開局即為完工狀態（以 s_createObj 生成則是 0% 工地）。
/// </summary>
public static class ScenarioLevelObjects
{
    /// <summary>
    /// 套用到 <paramref name="store"/>（呼叫端負責寫回）；回傳找不到範本而未寫入的項目。
    /// <paramref name="next"/>.DataSlots 會改為本次寫入的槽位。
    /// </summary>
    public static IReadOnlyList<ScenarioSpawn> Apply(LevelObjectStore store, ScenarioDocument previous, ScenarioDocument next,
        Func<ScenarioSpawn, LevelObjectTemplate?> templateFor)
    {
        ArgumentNullException.ThrowIfNull(store); ArgumentNullException.ThrowIfNull(previous); ArgumentNullException.ThrowIfNull(next); ArgumentNullException.ThrowIfNull(templateFor);
        foreach (ScenarioDataSlot owned in previous.DataSlots) store.RemoveIfUid(owned.Slot, owned.Uid);
        var slots = new List<ScenarioDataSlot>();
        var skipped = new List<ScenarioSpawn>();
        foreach (ScenarioSpawn spawn in next.Spawns.Where(spawn => spawn.Prebuilt))
        {
            if (spawn.Team is < 0 or > 8) throw new InvalidDataException("預建物件的隊伍必須介於 0 與 8。");
            if (templateFor(spawn) is not { } template) { skipped.Add(spawn); continue; }
            float rotation = (float)(((spawn.Angle % 360) + 360) % 360 * Math.PI / 180);
            int slot = store.Add(template, spawn.X, spawn.Y, spawn.Z, rotation, spawn.Team);
            if (slot < 0) throw new InvalidOperationException("地圖的世界物件已達上限（14,000 個），無法再新增。");
            slots.Add(new ScenarioDataSlot(slot, store.UidAt(slot)!.Value));
        }
        next.DataSlots = slots;
        return skipped;
    }
}
