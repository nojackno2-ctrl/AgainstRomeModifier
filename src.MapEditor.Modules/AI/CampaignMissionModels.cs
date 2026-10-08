namespace AgainstRomeMapEditor.Modules.AI;

/// <summary>戰役任務目標類型</summary>
public enum CampaignObjectiveType
{
    /// <summary>防守存活指定時間</summary>
    SurviveTime,
    /// <summary>守護特定建築或 VIP 部隊（目標被毀即失敗）</summary>
    DefendTarget,
    /// <summary>摧毀指定目標建築或擊殺敵將（達成即獲勝）</summary>
    DestroyTarget,
    /// <summary>抵禦並消滅所有進攻波次</summary>
    WaveSurvival,
    /// <summary>護送或引導部隊進入特定矩形區域</summary>
    ReachArea
}

/// <summary>戰役目標定義</summary>
public sealed record CampaignObjective(
    Guid Id,
    CampaignObjectiveType Type,
    string Title,
    string Description,
    Guid? TargetId = null,
    int TargetMinX = 0,
    int TargetMinZ = 0,
    int TargetMaxX = 16383,
    int TargetMaxZ = 16383,
    int RequiredSeconds = 0,
    bool IsPrimary = true,
    bool IsHidden = false)
{
    public void Validate()
    {
        if (Id == Guid.Empty) throw new InvalidDataException("戰役目標必須具備持久 ID。");
        if (string.IsNullOrWhiteSpace(Title)) throw new InvalidDataException("戰役目標標題不能為空。");
        if (Type is CampaignObjectiveType.DefendTarget or CampaignObjectiveType.DestroyTarget)
        {
            if (TargetId is null || TargetId == Guid.Empty)
                throw new InvalidDataException($"目標類型「{Type}」必須指定目標物件 ID。");
        }
        if (Type == CampaignObjectiveType.SurviveTime && RequiredSeconds <= 0)
        {
            throw new InvalidDataException("存活時間目標必須指定大於 0 的秒數。");
        }
        if (Type == CampaignObjectiveType.ReachArea)
        {
            if (TargetId is null || TargetId == Guid.Empty)
                throw new InvalidDataException("區域到達目標必須指定護送目標物件 ID。");
            if (TargetMinX < 0 || TargetMinZ < 0 || TargetMaxX > 16383 || TargetMaxZ > 16383 || TargetMinX > TargetMaxX || TargetMinZ > TargetMaxZ)
                throw new InvalidDataException("目標區域邊界必須合法且介於 0–16383。");
        }
    }
}

/// <summary>波次或增援部隊配置</summary>
public sealed record WaveSquadDefinition(
    string Alias,
    int Count,
    int Team,
    int RelativeDelaySeconds = 0)
{
    public void Validate()
    {
        if (string.IsNullOrWhiteSpace(Alias)) throw new InvalidDataException("部隊別名不能為空。");
        if (Count is < 1 or > 20) throw new ArgumentOutOfRangeException(nameof(Count), "部隊人數必須介於 1 與 20 之間。");
        if (Team is < 0 or > 7) throw new ArgumentOutOfRangeException(nameof(Team), "部隊隊伍編號必須介於 0 與 7 之間。");
        if (RelativeDelaySeconds < 0) throw new ArgumentOutOfRangeException(nameof(RelativeDelaySeconds), "相對延遲秒數不能為負數。");
    }
}

/// <summary>進攻波次定義</summary>
public sealed record WaveAttackDefinition(
    int WaveIndex,
    int TriggerDelaySeconds,
    string Announcement,
    float SpawnX,
    float SpawnZ,
    IReadOnlyList<WaveSquadDefinition> Squads,
    Guid? AssignedPathId = null,
    Guid? TargetObjectId = null)
{
    public void Validate()
    {
        if (WaveIndex < 1) throw new ArgumentOutOfRangeException(nameof(WaveIndex), "波次序號必須 >= 1。");
        if (TriggerDelaySeconds < 0) throw new ArgumentOutOfRangeException(nameof(TriggerDelaySeconds), "波次觸發延遲不能為負數。");
        if (!float.IsFinite(SpawnX) || !float.IsFinite(SpawnZ) || SpawnX is < 0 or > 16383 || SpawnZ is < 0 or > 16383)
            throw new ArgumentOutOfRangeException("SpawnX/SpawnZ", "生成點座標必須介於 0 與 16383 之間。");
        if (Squads is null || Squads.Count == 0)
            throw new InvalidDataException($"第 {WaveIndex} 波至少需要包含一個部隊編制。");
        foreach (var squad in Squads) squad.Validate();
    }
}

/// <summary>定時增援定義</summary>
public sealed record ReinforcementDefinition(
    Guid Id,
    string Name,
    int TriggerDelaySeconds,
    int Team,
    float SpawnX,
    float SpawnZ,
    IReadOnlyList<WaveSquadDefinition> Squads,
    string NotificationText = "",
    Guid? AssignedPathId = null)
{
    public void Validate()
    {
        if (Id == Guid.Empty) throw new InvalidDataException("增援項目必須具備持久 ID。");
        if (string.IsNullOrWhiteSpace(Name)) throw new InvalidDataException("增援名稱不能為空。");
        if (TriggerDelaySeconds < 0) throw new ArgumentOutOfRangeException(nameof(TriggerDelaySeconds), "增援觸發延遲不能為負數。");
        if (Team is < 0 or > 7) throw new ArgumentOutOfRangeException(nameof(Team), "增援隊伍必須介於 0 與 7。");
        if (!float.IsFinite(SpawnX) || !float.IsFinite(SpawnZ) || SpawnX is < 0 or > 16383 || SpawnZ is < 0 or > 16383)
            throw new ArgumentOutOfRangeException("SpawnX/SpawnZ", "生成點座標必須介於 0 與 16383 之間。");
        if (Squads is null || Squads.Count == 0)
            throw new InvalidDataException($"增援「{Name}」至少需要包含一個部隊編制。");
        foreach (var squad in Squads) squad.Validate();
    }
}

/// <summary>高階戰役企劃總綱</summary>
public sealed record CampaignMissionPlan(
    string Title,
    string Briefing,
    IReadOnlyList<CampaignObjective> Objectives,
    IReadOnlyList<WaveAttackDefinition> Waves,
    IReadOnlyList<ReinforcementDefinition> Reinforcements,
    IReadOnlyList<FactionAiProfile> FactionProfiles)
{
    public void Validate()
    {
        if (string.IsNullOrWhiteSpace(Title)) throw new InvalidDataException("戰役任務標題不能為空。");
        var objIds = new HashSet<Guid>();
        foreach (var obj in Objectives)
        {
            obj.Validate();
            if (!objIds.Add(obj.Id)) throw new InvalidDataException($"包含重複的戰役目標 ID：{obj.Id}");
        }

        var waveIndices = new HashSet<int>();
        foreach (var wave in Waves)
        {
            wave.Validate();
            if (!waveIndices.Add(wave.WaveIndex)) throw new InvalidDataException($"包含重複的波次序號：{wave.WaveIndex}");
        }

        var reinfIds = new HashSet<Guid>();
        foreach (var reinf in Reinforcements)
        {
            reinf.Validate();
            if (!reinfIds.Add(reinf.Id)) throw new InvalidDataException($"包含重複的增援 ID：{reinf.Id}");
        }

        foreach (var faction in FactionProfiles)
        {
            faction.Validate();
        }
    }
}
