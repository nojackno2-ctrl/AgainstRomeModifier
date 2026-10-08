namespace AgainstRomeMapEditor.Modules.AI;

/// <summary>AI 接戰與遭遇規則</summary>
public enum EngagementRule
{
    /// <summary>視線內主動攻擊任何敵人</summary>
    AttackOnSight,
    /// <summary>僅在領地/警戒範圍內交戰，敵軍逃離則放棄追擊</summary>
    DefendTerritory,
    /// <summary>受到攻擊前保持中立被動</summary>
    PassiveUntilAttacked,
    /// <summary>遭攻擊後持續反擊直到威脅消除</summary>
    RetaliateOnly
}

/// <summary>戰術姿態</summary>
public enum TacticalPosture
{
    /// <summary>主動突擊衝鋒</summary>
    AggressiveRush,
    /// <summary>均衡推進與陣形掩護</summary>
    BalancedFormation,
    /// <summary>固守原地與盾牆防線</summary>
    DefensiveHold,
    /// <summary>游擊拉扯與騎射騷擾</summary>
    HitAndRun
}

/// <summary>目標優先級</summary>
public enum TargetPriority
{
    /// <summary>距離最近的敵對目標</summary>
    NearestEnemy,
    /// <summary>玩家聚落主建築（城鎮中心）</summary>
    PlayerSettlementCenter,
    /// <summary>經濟生產與資源採集點</summary>
    EconomicStructures,
    /// <summary>落單或脆弱部隊</summary>
    WeakestSquad,
    /// <summary>重型防禦塔與軍事哨站</summary>
    MilitaryStructures
}

/// <summary>兵種偏好與權重</summary>
public sealed record UnitPreferenceWeight(
    string Alias,
    int Weight = 50,
    int MinCount = 1,
    int MaxCount = 20)
{
    public bool IsValid => !string.IsNullOrWhiteSpace(Alias)
        && Weight is >= 1 and <= 100
        && MinCount is >= 1 and <= 20
        && MaxCount >= MinCount && MaxCount <= 20;
}

/// <summary>AI 戰略原型設定檔</summary>
public sealed record AiArchetypeProfile(
    string Id,
    string DisplayName,
    string Description,
    string PreferredTribe,
    float Aggressiveness,
    float ExpansionDesire,
    float EconomicFocus,
    float DefensePriority,
    float PatrolRadius,
    float RetreatHealthRatio,
    int MinRaidIntervalSeconds,
    int MaxRaidIntervalSeconds,
    int GarrisonCap,
    EngagementRule EngagementRule,
    TacticalPosture TacticalPosture,
    TargetPriority TargetPriority,
    IReadOnlyList<UnitPreferenceWeight> UnitPreferences)
{
    public void Validate()
    {
        if (string.IsNullOrWhiteSpace(Id)) throw new InvalidDataException("AI 原型 ID 不能為空。");
        if (string.IsNullOrWhiteSpace(DisplayName)) throw new InvalidDataException("AI 原型名稱不能為空。");
        if (Aggressiveness is < 0f or > 1f) throw new ArgumentOutOfRangeException(nameof(Aggressiveness), "侵略度必須介於 0.0 與 1.0。");
        if (ExpansionDesire is < 0f or > 1f) throw new ArgumentOutOfRangeException(nameof(ExpansionDesire), "擴張欲必須介於 0.0 與 1.0。");
        if (EconomicFocus is < 0f or > 1f) throw new ArgumentOutOfRangeException(nameof(EconomicFocus), "經濟重心必須介於 0.0 與 1.0。");
        if (DefensePriority is < 0f or > 1f) throw new ArgumentOutOfRangeException(nameof(DefensePriority), "防守優先級必須介於 0.0 與 1.0。");
        if (PatrolRadius is < 100f or > 16384f) throw new ArgumentOutOfRangeException(nameof(PatrolRadius), "巡邏警戒半徑必須介於 100 與 16384。");
        if (RetreatHealthRatio is < 0f or > 0.8f) throw new ArgumentOutOfRangeException(nameof(RetreatHealthRatio), "撤退血量比率必須介於 0.0 與 0.8。");
        if (MinRaidIntervalSeconds < 5 || MaxRaidIntervalSeconds < MinRaidIntervalSeconds)
            throw new InvalidDataException("突襲間隔設定不合法。");
        if (GarrisonCap is < 0 or > 100) throw new ArgumentOutOfRangeException(nameof(GarrisonCap), "駐軍上限必須介於 0 與 100。");
        if (UnitPreferences is not null)
        {
            foreach (var pref in UnitPreferences)
            {
                if (!pref.IsValid) throw new InvalidDataException($"兵種權重設定不合法：{pref.Alias}");
            }
        }
    }
}

/// <summary>勢力指派的 AI 設定</summary>
public sealed record FactionAiProfile(
    int Team,
    string ArchetypeId,
    AiArchetypeProfile? CustomOverrides = null,
    Guid? BaseSettlementId = null,
    bool Enabled = true)
{
    public void Validate()
    {
        if (Team is < 0 or > 7) throw new ArgumentOutOfRangeException(nameof(Team), "隊伍編號必須介於 0 與 7。");
        if (string.IsNullOrWhiteSpace(ArchetypeId)) throw new InvalidDataException("勢力 AI 必須指定原型 ID。");
        CustomOverrides?.Validate();
    }
}

/// <summary>標準 AI 戰略原型目錄</summary>
public static class AiArchetypeCatalog
{
    public const string IdNomadicRaider = "NomadicRaider";
    public const string IdRomanFortress = "RomanFortress";
    public const string IdGermanicSettlement = "GermanicSettlement";
    public const string IdBarbarianOutpost = "BarbarianOutpost";

    private static readonly Dictionary<string, AiArchetypeProfile> Registry = new(StringComparer.OrdinalIgnoreCase);

    static AiArchetypeCatalog()
    {
        Register(CreateNomadicRaider());
        Register(CreateRomanFortress());
        Register(CreateGermanicSettlement());
        Register(CreateBarbarianOutpost());
    }

    public static IReadOnlyList<AiArchetypeProfile> All => Registry.Values.ToList();

    public static AiArchetypeProfile? Find(string id)
    {
        if (string.IsNullOrWhiteSpace(id)) return null;
        return Registry.TryGetValue(id, out var profile) ? profile : null;
    }

    public static AiArchetypeProfile Get(string id)
    {
        return Find(id) ?? throw new KeyNotFoundException($"找不到指定的 AI 戰略原型：{id}");
    }

    public static void Register(AiArchetypeProfile profile)
    {
        ArgumentNullException.ThrowIfNull(profile);
        profile.Validate();
        Registry[profile.Id] = profile;
    }

    private static AiArchetypeProfile CreateNomadicRaider() => new(
        Id: IdNomadicRaider,
        DisplayName: "進攻型遊牧部落",
        Description: "高度侵略性的遊牧掠奪者。偏好高速騎兵與騎射騷擾，定時組織突襲掠奪邊界聚落，不注重固定防禦建築，遭遇劣勢時快速撤退脫離。",
        PreferredTribe: "Hun",
        Aggressiveness: 0.90f,
        ExpansionDesire: 0.30f,
        EconomicFocus: 0.15f,
        DefensePriority: 0.20f,
        PatrolRadius: 3500f,
        RetreatHealthRatio: 0.35f,
        MinRaidIntervalSeconds: 45,
        MaxRaidIntervalSeconds: 90,
        GarrisonCap: 4,
        EngagementRule: EngagementRule.AttackOnSight,
        TacticalPosture: TacticalPosture.HitAndRun,
        TargetPriority: TargetPriority.EconomicStructures,
        UnitPreferences:
        [
            new("HUN_KAVINF00", Weight: 60, MinCount: 5, MaxCount: 15),
            new("HUN_KAVSCH00", Weight: 40, MinCount: 5, MaxCount: 10)
        ]);

    private static AiArchetypeProfile CreateRomanFortress() => new(
        Id: IdRomanFortress,
        DisplayName: "防守型羅馬堡壘",
        Description: "以軍事要塞與防衛為重心的羅馬軍團。維持龐大軍團駐軍、箭塔防衛網，陣形堅固。僅在領地受到威脅時出動重裝步兵迎擊，並執行組織化反擊。",
        PreferredTribe: "Roman",
        Aggressiveness: 0.25f,
        ExpansionDesire: 0.20f,
        EconomicFocus: 0.50f,
        DefensePriority: 0.95f,
        PatrolRadius: 2200f,
        RetreatHealthRatio: 0.15f,
        MinRaidIntervalSeconds: 120,
        MaxRaidIntervalSeconds: 240,
        GarrisonCap: 20,
        EngagementRule: EngagementRule.DefendTerritory,
        TacticalPosture: TacticalPosture.DefensiveHold,
        TargetPriority: TargetPriority.MilitaryStructures,
        UnitPreferences:
        [
            new("ROM_INF00", Weight: 50, MinCount: 10, MaxCount: 20),
            new("ROM_INF01", Weight: 30, MinCount: 8, MaxCount: 15),
            new("ROM_SCH00", Weight: 20, MinCount: 6, MaxCount: 12)
        ]);

    private static AiArchetypeProfile CreateGermanicSettlement() => new(
        Id: IdGermanicSettlement,
        DisplayName: "經濟擴張型日耳曼聚落",
        Description: "注重人口增長、農耕伐木與資源儲備的日耳曼/凱爾特部落。優先發展聚落建築，組建民兵護送隊，並具備向外建立分聚落與拓荒的戰略傾向。",
        PreferredTribe: "German",
        Aggressiveness: 0.45f,
        ExpansionDesire: 0.85f,
        EconomicFocus: 0.90f,
        DefensePriority: 0.55f,
        PatrolRadius: 2800f,
        RetreatHealthRatio: 0.25f,
        MinRaidIntervalSeconds: 150,
        MaxRaidIntervalSeconds: 300,
        GarrisonCap: 12,
        EngagementRule: EngagementRule.RetaliateOnly,
        TacticalPosture: TacticalPosture.BalancedFormation,
        TargetPriority: TargetPriority.NearestEnemy,
        UnitPreferences:
        [
            new("GER_INF00", Weight: 45, MinCount: 6, MaxCount: 16),
            new("GER_INF01", Weight: 35, MinCount: 6, MaxCount: 14),
            new("GER_KAVINF00", Weight: 20, MinCount: 4, MaxCount: 10)
        ]);

    private static AiArchetypeProfile CreateBarbarianOutpost() => new(
        Id: IdBarbarianOutpost,
        DisplayName: "中立野蠻人巡邏哨",
        Description: "地圖中立或敵對哨站。部隊沿固定路徑點循環巡邏，守護周邊寶箱或隘口；攻擊進入警戒範圍的任何勢力，超出脫離範圍時自動回防巡邏起點。",
        PreferredTribe: "Neutral",
        Aggressiveness: 0.60f,
        ExpansionDesire: 0.00f,
        EconomicFocus: 0.00f,
        DefensePriority: 0.80f,
        PatrolRadius: 1600f,
        RetreatHealthRatio: 0.10f,
        MinRaidIntervalSeconds: 60,
        MaxRaidIntervalSeconds: 120,
        GarrisonCap: 6,
        EngagementRule: EngagementRule.DefendTerritory,
        TacticalPosture: TacticalPosture.AggressiveRush,
        TargetPriority: TargetPriority.NearestEnemy,
        UnitPreferences:
        [
            new("GER_INF01", Weight: 60, MinCount: 4, MaxCount: 10),
            new("GER_SCH00", Weight: 40, MinCount: 4, MaxCount: 8)
        ]);
}
