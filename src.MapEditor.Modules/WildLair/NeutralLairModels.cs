namespace AgainstRomeMapEditor.Modules.WildLair;

/// <summary>
/// 野外中立巢穴大類別。
/// </summary>
public enum LairCategory
{
    /// <summary>野獸洞穴（狼穴、熊洞、野豬灌木叢等）。</summary>
    BeastDen,
    /// <summary>野蠻人劫掠營地（日耳曼/凱爾特/匈奴流寇營帳）。</summary>
    BarbarianCamp,
    /// <summary>武裝強盜山寨或叛軍哨站（逃兵堡壘、土匪要塞）。</summary>
    BanditStronghold,
    /// <summary>遠古邪教祭壇、聖林或遺蹟兇兆。</summary>
    SacredGroveOrRuins
}

/// <summary>
/// 野外巢穴威脅階級（難度等級）。
/// </summary>
public enum LairDifficultyTier
{
    /// <summary>T1 初級遊蕩：小型野獸、斥候營地，適合前期探險。</summary>
    Tier1Scout = 1,
    /// <summary>T2 中級威脅：標準洞窟、游擊掠奪者營地，中期騷擾主力。</summary>
    Tier2Standard = 2,
    /// <summary>T3 重裝據點：精銳衛戍要塞、嗜血猛獸巨穴，需集結大軍剿滅。</summary>
    Tier3Major = 3,
    /// <summary>T4 首領巢穴：氏族酋長王帳、傳奇巨獸，消滅觸發重大獎勵與情節進展。</summary>
    Tier4Boss = 4
}

/// <summary>
/// 常駐巢穴守衛部隊規格。
/// </summary>
public sealed record LairGuardUnit(
    string UnitAlias,
    int Count,
    float PatrolRadiusTiles = 4.0f,
    float AggroRadiusTiles = 8.0f,
    int RespawnIntervalSeconds = 0,
    float AngleOffsetDeg = 0.0f);

/// <summary>
/// 週期性敵怪波次生成規則。
/// </summary>
public sealed record LairWaveSpawnRule(
    string WaveId,
    string UnitAlias,
    int SpawnCount,
    int IntervalSeconds,
    int InitialDelaySeconds = 120,
    int MaxActiveWaves = 3,
    float SpawnRadiusTiles = 2.5f,
    string AggroBehavior = "PatrolHostile");

/// <summary>
/// 巢穴破滅戰利品與廣播獎勵。
/// </summary>
public sealed record LairLootReward(
    int Wood = 100,
    int Food = 100,
    int Gold = 50,
    int HonorPoints = 25,
    string CompletionMessage = "Neutral lair eradicated!");

/// <summary>
/// 野外中立巢穴建築/地景原型定義（Catalog Blueprint）。
/// </summary>
public sealed record NeutralLairDefinition(
    string Id,
    string DisplayNameZh,
    string DisplayNameEn,
    LairCategory Category,
    LairDifficultyTier Tier,
    string NativeBuildingOrLandscapeType,
    float FootprintRadiusTiles,
    IReadOnlyList<LairGuardUnit> DefaultGuards,
    IReadOnlyList<LairWaveSpawnRule> WaveRules,
    LairLootReward Loot,
    string BiomeAffinity = "TemperateForest",
    string Description = "");

/// <summary>
/// 實際放置於地圖上的野外中立巢穴實例。
/// </summary>
public sealed record PlacedNeutralLair(
    Guid InstanceId,
    string DefinitionId,
    float WorldX,
    float WorldY,
    float WorldZ,
    float RotationDeg = 0.0f,
    int Team = 7,
    Guid CoreStructureSpawnId = default,
    IReadOnlyList<Guid>? GuardSpawnIds = null,
    IReadOnlyList<string>? BoundEventNames = null,
    bool IsActive = true);
