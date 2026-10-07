using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor.Modules.Persistence;

/// <summary>在開始檔案交易之前檢查可由編輯快照確定的錯誤；DATA 綁定於重建後再驗證。</summary>
public static class ScenarioSavePreflight
{
    public static void Validate(ScenarioDocument scenario, IReadOnlyCollection<string> aliases)
    {
        ArgumentNullException.ThrowIfNull(scenario);
        ScenarioEventValidator.Validate(scenario.Events, aliases);
        var ids = new HashSet<Guid>();
        foreach (ScenarioSpawn spawn in scenario.Spawns)
        {
            if (spawn.Id == Guid.Empty || !ids.Add(spawn.Id)) throw new InvalidDataException("放置物件缺少持久 ID 或 ID 重複。");
            if (!spawn.Prebuilt && !aliases.Contains(spawn.Alias, StringComparer.OrdinalIgnoreCase)) throw new InvalidDataException($"未知的物件別名：{spawn.Alias}");
            if (!float.IsFinite(spawn.X) || !float.IsFinite(spawn.Y) || !float.IsFinite(spawn.Z)
                || spawn.X is < 0 or > 16383 || spawn.Z is < 0 or > 16383)
                throw new InvalidDataException("放置物件的位置必須是有效地圖座標（X/Z：0–16383）。");
            if (spawn.Count is < 0 or > 20 || (spawn.Count > 0 ? spawn.Team is < 0 or > 7 : spawn.Team is < -1 or > 15))
                throw new InvalidDataException("部隊需要隊伍 0–7、人數 1–20；其他物件需要隊伍 -1–15。");
        }
        foreach (ScenarioEvent item in scenario.Events.Where(item => item.Enabled))
            foreach (ScenarioCondition condition in item.Conditions)
                if (condition.TargetId != Guid.Empty && !ids.Contains(condition.TargetId))
                    throw new InvalidDataException($"事件「{item.Name}」的目標物件已刪除，請重新選擇或停用事件。");
    }
}
