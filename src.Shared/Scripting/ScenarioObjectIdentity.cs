using System.Globalization;
using System.Security.Cryptography;
using System.Text;

namespace AgainstRomeModifier.Scripting;

/// <summary>編輯器物件身份與本次儲存的 DATA 配對；物件身份不依賴位置或遊戲 UID。</summary>
public static class ScenarioObjectIdentity
{
    public static string RuntimeIndexKey(Guid id) => RuntimeKey(id, "INDEX");
    public static string RuntimeUidKey(Guid id) => RuntimeKey(id, "UID");

    private static string RuntimeKey(Guid id, string suffix) => id != Guid.Empty
        ? $"ARM_OBJECT_{id:N}_{suffix}" : throw new ArgumentException("物件缺少持久 ID。", nameof(id));

    public static void Prepare(ScenarioDocument document, bool legacy = false, bool validateBindings = true)
    {
        var ids = new HashSet<Guid>();
        var spawns = new List<ScenarioSpawn>(document.Spawns.Count);
        for (int index = 0; index < document.Spawns.Count; index++)
        {
            ScenarioSpawn spawn = document.Spawns[index] ?? throw new InvalidDataException("場景物件不能是 null。");
            Guid id = spawn.Id;
            if (id == Guid.Empty)
            {
                // 舊檔案多次唯讀載入必須得到同一 ID；ID 僅在同一地圖內有意義。
                id = legacy ? LegacyId(index) : Guid.NewGuid();
            }
            if (!ids.Add(id)) throw new InvalidDataException("場景物件 ID 重複，無法安全識別事件目標。");
            spawns.Add(spawn with { Id = id });
        }
        if (document.DataSlots.Any(slot => slot is null)) throw new InvalidDataException("物件槽位不能是 null。");
        ScenarioSpawn[] prebuilt = spawns.Where(spawn => spawn.Prebuilt).ToArray();
        var slots = document.DataSlots.ToList();
        // 舊版 Apply 按預建物件順序輸出槽位。只有完整配對才能遷移；不以座標猜測。
        if (legacy && slots.Count == prebuilt.Length && slots.All(slot => slot.SpawnId == Guid.Empty))
            slots = slots.Select((slot, index) => slot with { SpawnId = prebuilt[index].Id }).ToList();
        var bound = new HashSet<Guid>();
        var boundSlots = new HashSet<int>();
        foreach (ScenarioDataSlot slot in slots.Where(slot => validateBindings && slot.SpawnId != Guid.Empty))
            if (slot.Slot is < 0 or >= 14000 || slot.Uid == 0 || !prebuilt.Any(spawn => spawn.Id == slot.SpawnId)
                || !bound.Add(slot.SpawnId) || !boundSlots.Add(slot.Slot))
                throw new InvalidDataException("DATA 槽位的物件 ID 無效或重複。");
        document.Spawns = spawns; document.DataSlots = slots;
    }

    public static ScenarioDataSlot? DataBinding(ScenarioDocument document, Guid id) => id == Guid.Empty ? null
        : document.DataSlots.SingleOrDefault(slot => slot.SpawnId == id);

    private static Guid LegacyId(int index) => new(SHA256.HashData(Encoding.UTF8.GetBytes(
        "AgainstRomeScenarioLegacy:" + index.ToString(CultureInfo.InvariantCulture))).AsSpan(0, 16));
}
