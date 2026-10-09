namespace AgainstRomeMapEditor.Modules.WildLair;

// Legacy scenario-authoring categories are retained for compatibility. Their presence is not
// evidence that the native game has camps/strongholds/dens or their associated mechanics.
public enum LairCategory { BeastDen, BarbarianCamp, BanditStronghold, SacredGroveOrRuins, WildAnimal }
public enum LairDifficultyTier { Unrated = 0, Tier1Scout = 1, Tier2Standard = 2, Tier3Major = 3, Tier4Boss = 4 }

public sealed record LairGuardUnit(string UnitAlias, int Count, float PatrolRadiusTiles = 4,
    float AggroRadiusTiles = 8, int RespawnIntervalSeconds = 0, float AngleOffsetDeg = 0);

/// <summary>Authored periodic troop timer. MaxActiveWaves=0 means uncapped; None means no AI order.
/// The binder rejects unsupported timing, caps or aggression instead of silently ignoring them.</summary>
public sealed record LairWaveSpawnRule(string WaveId, string UnitAlias, int SpawnCount, int IntervalSeconds,
    int InitialDelaySeconds = 120, int MaxActiveWaves = 0, float SpawnRadiusTiles = 2.5f,
    string AggroBehavior = "None");

/// <summary>Only a tracked-core completion message is supported; numeric rewards are gated.</summary>
public sealed record LairLootReward(int Wood = 0, int Food = 0, int Gold = 0, int HonorPoints = 0,
    string CompletionMessage = "");

/// <summary>Observed object or explicitly authored scenario blueprint. Footprint/tier/biome
/// are editor metadata, not recovered native mechanics. Defaults have no waves, guards or loot.</summary>
public sealed record NeutralLairDefinition(string Id, string DisplayNameZh, string DisplayNameEn,
    LairCategory Category, LairDifficultyTier Tier, string NativeBuildingOrLandscapeType,
    float FootprintRadiusTiles, IReadOnlyList<LairGuardUnit> DefaultGuards,
    IReadOnlyList<LairWaveSpawnRule> WaveRules, LairLootReward Loot,
    string BiomeAffinity = "", string Description = "")
{
    public string? ScriptAlias { get; init; }
}

/// <summary>Team 8 is observed neutral DATA ownership. Timer troop generation requires an
/// explicit team in 0-7; no automatic hostility or team remapping is performed.</summary>
public sealed record PlacedNeutralLair(Guid InstanceId, string DefinitionId, float WorldX, float WorldY,
    float WorldZ, float RotationDeg = 0, int Team = 8, Guid CoreStructureSpawnId = default,
    IReadOnlyList<Guid>? GuardSpawnIds = null, IReadOnlyList<string>? BoundEventNames = null, bool IsActive = true);
