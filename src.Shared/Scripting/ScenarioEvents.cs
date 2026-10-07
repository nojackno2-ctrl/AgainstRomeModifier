using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Scripting;

public enum ScenarioActionKind { Message, Diplomacy, SpawnUnit }

public sealed record ScenarioAction(ScenarioActionKind Kind, string Text = "", int Team = 0, int OtherTeam = 1,
    bool Hostile = true, string Alias = "", float X = 8000, float Z = 8000, int Count = 10);

public sealed record ScenarioEvent(string Name, int DelaySeconds = 10, bool Repeat = false, bool Enabled = true)
{
    public List<ScenarioAction> Actions { get; init; } = new();
}

public static class ScenarioEventValidator
{
    public static void Validate(IReadOnlyList<ScenarioEvent> events, IReadOnlyCollection<string> aliases)
    {
        if (events.Count > 256) throw new InvalidDataException("事件上限為 256 個。");
        foreach (ScenarioEvent item in events)
        {
            if (item is null || string.IsNullOrWhiteSpace(item.Name) || item.Name.Length > 100)
                throw new InvalidDataException("事件名稱必須是 1–100 個字元。");
            if (item.DelaySeconds is < 0 or > 86400 || item.Repeat && item.DelaySeconds == 0)
                throw new InvalidDataException("計時必須介於 0 與 86400 秒；重複事件至少間隔 1 秒。");
            if (item.Actions is null || item.Actions.Count is < 1 or > 32)
                throw new InvalidDataException("每個事件必須有 1–32 個動作。");
            foreach (ScenarioAction action in item.Actions)
            {
                if (action is null || !Enum.IsDefined(action.Kind)) throw new InvalidDataException("不支援的事件動作。");
                switch (action.Kind)
                {
                    case ScenarioActionKind.Message:
                        if (string.IsNullOrWhiteSpace(action.Text) || action.Text.Contains('\0') || MapTextEncoding.Game.GetByteCount(action.Text) > 1000)
                            throw new InvalidDataException("訊息必須是遊戲支援的文字，長度上限為 1000 位元組。");
                        break;
                    case ScenarioActionKind.Diplomacy:
                        if (action.Team is < 0 or > 7 || action.OtherTeam is < 0 or > 7 || action.Team == action.OtherTeam)
                            throw new InvalidDataException("外交需要兩個不同隊伍（0–7）。");
                        break;
                    case ScenarioActionKind.SpawnUnit:
                        if (action.Team is < 0 or > 7 || action.Count is < 1 or > 20 || !float.IsFinite(action.X) || !float.IsFinite(action.Z)
                            || action.X is < 0 or > 16383 || action.Z is < 0 or > 16383)
                            throw new InvalidDataException("部隊需要隊伍 0–7、人數 1–20 與有效的地圖座標。");
                        if (!aliases.Contains(action.Alias, StringComparer.OrdinalIgnoreCase)) throw new InvalidDataException($"未知的部隊別名：{action.Alias}");
                        break;
                }
            }
        }
    }
}
