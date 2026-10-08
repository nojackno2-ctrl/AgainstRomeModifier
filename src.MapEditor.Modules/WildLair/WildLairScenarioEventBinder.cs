using AgainstRomeMapEditor.Modules.Events;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor.Modules.WildLair;

/// <summary>
/// 野外中立巢穴與關卡事件定時觸發器自動繫結合約（WildLairScenarioEventBinder）：
/// 負責將中立巢穴之定期刷怪波次、守衛戒備與破滅清剿獎勵，
/// 自動編譯並雙向繫結至原版關卡事件合約（<see cref="ScenarioEvent"/>、<see cref="ScenarioCondition"/>、<see cref="ScenarioAction"/>）。
/// </summary>
public static class WildLairScenarioEventBinder
{
    public const string LairEventPrefix = "LAIR_";

    /// <summary>
    /// 為指定的野外中立巢穴實例生成完整的原版關卡事件清單。
    /// </summary>
    /// <param name="lair">已放置之巢穴實例。</param>
    /// <param name="definition">巢穴目錄原型規格。</param>
    /// <param name="playerTeam">玩家隊伍編號（預設 0）。</param>
    /// <param name="tileWorldSize">世界格大小（預設 64.0）。</param>
    public static IReadOnlyList<ScenarioEvent> GenerateEventsForLair(
        PlacedNeutralLair lair,
        NeutralLairDefinition definition,
        int playerTeam = 0,
        float tileWorldSize = 64.0f)
    {
        ArgumentNullException.ThrowIfNull(lair);
        ArgumentNullException.ThrowIfNull(definition);

        var events = new List<ScenarioEvent>();
        string idTag = lair.InstanceId.ToString("N")[..8];

        // 1. 週期性刷怪波次定時觸發器（Repeat = true）
        // 核心防呆：必須繫結 ObjectExists(CoreStructureSpawnId)。當巢穴本體被推平拆除時，自動停止刷怪！
        for (int i = 0; i < definition.WaveRules.Count; i++)
        {
            var wave = definition.WaveRules[i];
            string eventName = $"{LairEventPrefix}{idTag}_WAVE_{i}_{wave.WaveId}";
            if (eventName.Length > 100) eventName = eventName[..100];

            // 微幅徑向偏移生成坐標，避免多怪疊在同一物理點
            float angleRad = (i * 1.57f + lair.RotationDeg * MathF.PI / 180f);
            float spawnOffset = wave.SpawnRadiusTiles * tileWorldSize;
            float spawnX = Math.Clamp(lair.WorldX + MathF.Cos(angleRad) * spawnOffset, 0f, 16383f);
            float spawnZ = Math.Clamp(lair.WorldZ + MathF.Sin(angleRad) * spawnOffset, 0f, 16383f);

            var spawnEvent = new ScenarioEvent(
                Name: eventName,
                DelaySeconds: Math.Max(1, wave.IntervalSeconds),
                Repeat: true,
                Enabled: lair.IsActive)
            {
                Actions = new List<ScenarioAction>
                {
                    new(
                        Kind: ScenarioActionKind.SpawnUnit,
                        Alias: wave.UnitAlias,
                        Team: Math.Clamp(lair.Team, 0, 7),
                        X: spawnX,
                        Z: spawnZ,
                        Count: Math.Clamp(wave.SpawnCount, 1, 20))
                }
            };

            // 若已有核心建築 ID，附加建築存在條件
            if (lair.CoreStructureSpawnId != Guid.Empty)
            {
                spawnEvent.Conditions.Add(new ScenarioCondition(
                    ScenarioConditionKind.ObjectExists,
                    lair.CoreStructureSpawnId));
            }

            events.Add(spawnEvent);
        }

        // 2. 巢穴破滅清剿事件（Repeat = false）
        // 核心邏輯：當巢穴本體建築被摧毀或移除（ObjectDeadOrRemoved），觸發勝利/清除訊息
        if (lair.CoreStructureSpawnId != Guid.Empty)
        {
            string destroyEventName = $"{LairEventPrefix}{idTag}_CLEARED";
            if (destroyEventName.Length > 100) destroyEventName = destroyEventName[..100];

            string rawMsg = string.IsNullOrWhiteSpace(definition.Loot.CompletionMessage)
                ? $"Neutral lair {definition.DisplayNameEn} has been eradicated!"
                : definition.Loot.CompletionMessage;

            // 確保訊息編碼相容
            string msg = SanitizeMessage(rawMsg);

            var destroyEvent = new ScenarioEvent(
                Name: destroyEventName,
                DelaySeconds: 1,
                Repeat: false,
                Enabled: lair.IsActive)
            {
                Conditions = new List<ScenarioCondition>
                {
                    new(ScenarioConditionKind.ObjectDeadOrRemoved, lair.CoreStructureSpawnId)
                },
                Actions = new List<ScenarioAction>
                {
                    new(ScenarioActionKind.Message, Text: msg)
                }
            };

            events.Add(destroyEvent);
        }

        // 3. 初始敵對外交鎖定（確保中立敵對 Team 7 與玩家處於交戰狀態）
        if (lair.Team != playerTeam)
        {
            string diploEventName = $"{LairEventPrefix}{idTag}_HOSTILE";
            if (diploEventName.Length > 100) diploEventName = diploEventName[..100];

            var diploEvent = new ScenarioEvent(
                Name: diploEventName,
                DelaySeconds: 0,
                Repeat: false,
                Enabled: lair.IsActive)
            {
                Actions = new List<ScenarioAction>
                {
                    new(
                        Kind: ScenarioActionKind.Diplomacy,
                        Team: Math.Clamp(lair.Team, 0, 7),
                        OtherTeam: Math.Clamp(playerTeam, 0, 7),
                        Hostile: true)
                }
            };

            events.Add(diploEvent);
        }

        return events;
    }

    /// <summary>
    /// 自動同步巢穴事件至 <see cref="ScenarioEventSession"/>：
    /// 清除該巢穴原有的舊事件，重新注入最新生成的刷怪與清剿事件。
    /// </summary>
    public static void SyncLairEvents(
        ScenarioEventSession session,
        PlacedNeutralLair lair,
        NeutralLairDefinition definition,
        int playerTeam = 0)
    {
        ArgumentNullException.ThrowIfNull(session);
        ArgumentNullException.ThrowIfNull(lair);
        ArgumentNullException.ThrowIfNull(definition);

        RemoveLairEvents(session, lair.InstanceId);

        var newEvents = GenerateEventsForLair(lair, definition, playerTeam);
        foreach (var ev in newEvents)
        {
            session.Add(ev);
        }
    }

    /// <summary>
    /// 從 <see cref="ScenarioEventSession"/> 中安全移除該巢穴實例的所有關聯事件，絕不遺留無效 GUID。
    /// </summary>
    public static int RemoveLairEvents(ScenarioEventSession session, Guid instanceId)
    {
        ArgumentNullException.ThrowIfNull(session);

        string prefix = $"{LairEventPrefix}{instanceId:N}[..8]";
        string fallbackPrefix = $"{LairEventPrefix}{instanceId.ToString("N")[..8]}";

        var existing = session.Capture();
        int removedCount = 0;

        for (int i = existing.Count - 1; i >= 0; i--)
        {
            var item = existing[i];
            if (item.Name.StartsWith(prefix, StringComparison.OrdinalIgnoreCase) ||
                item.Name.StartsWith(fallbackPrefix, StringComparison.OrdinalIgnoreCase))
            {
                session.RemoveAt(i);
                removedCount++;
            }
        }

        return removedCount;
    }

    /// <summary>
    /// 驗證關卡事件中的巢穴綁定完整性（偵測是否有殘留或失效的巢穴參照）。
    /// </summary>
    public static IReadOnlyList<string> ValidateLairBindings(
        IReadOnlyList<ScenarioEvent> events,
        IReadOnlyList<PlacedNeutralLair> activeLairs)
    {
        ArgumentNullException.ThrowIfNull(events);
        ArgumentNullException.ThrowIfNull(activeLairs);

        var errors = new List<string>();
        var activeIds = new HashSet<string>(activeLairs.Select(l => l.InstanceId.ToString("N")[..8]), StringComparer.OrdinalIgnoreCase);

        foreach (var ev in events)
        {
            if (ev.Name.StartsWith(LairEventPrefix, StringComparison.OrdinalIgnoreCase))
            {
                string tag = ev.Name.Substring(LairEventPrefix.Length);
                int underscore = tag.IndexOf('_');
                if (underscore > 0) tag = tag[..underscore];

                if (!activeIds.Contains(tag))
                {
                    errors.Add($"事件「{ev.Name}」指向已不存在之巢穴實例（Tag: {tag}）。");
                }
            }
        }

        return errors;
    }

    private static string SanitizeMessage(string message)
    {
        try
        {
            // 若為合法遊戲文字，直接返回
            MapTextEncoding.Game.GetByteCount(message);
            return message;
        }
        catch
        {
            // 若包含非法字元，安全回退至標準 ASCII 訊息
            return "A neutral wild lair has been eradicated!";
        }
    }
}
