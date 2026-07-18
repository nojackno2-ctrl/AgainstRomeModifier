namespace AgainstRomeModifier.Core.Features;

public interface IFeatureKey
{
    string Id { get; }
    FeatureValue DisabledValue { get; }
}

public sealed class FeatureKey<T> : IFeatureKey
{
    private readonly Func<T, FeatureValue> _encode;
    private readonly Func<FeatureValue, T> _decode;

    private FeatureKey(string id, T disabledValue, Func<T, FeatureValue> encode, Func<FeatureValue, T> decode)
    {
        Id = id;
        Disabled = disabledValue;
        _encode = encode;
        _decode = decode;
    }

    public string Id { get; }
    public T Disabled { get; }
    public FeatureValue DisabledValue => Encode(Disabled);

    internal FeatureValue Encode(T value) => _encode(value);
    internal T Decode(FeatureValue value) => _decode(value);

    public static FeatureKey<bool> Bool(string id) =>
        new(id, false, FeatureValue.Of, value => value.AsBool);

    public static FeatureKey<int> Int(string id, int disabledValue) =>
        new(id, disabledValue, FeatureValue.Of, value => value.AsInt);

    public static FeatureKey<TReference?> Reference<TReference>(string id) where TReference : class =>
        new(id, null, value => FeatureValue.Of(value), value => value.AsObject as TReference);
}

public static class FeatureKeys
{
    public static readonly FeatureKey<bool> FastCiviProduction = FeatureKey<bool>.Bool("FastCiviProduction");
    public static readonly FeatureKey<bool> InfiniteMorale = FeatureKey<bool>.Bool("InfiniteMorale");
    public static readonly FeatureKey<bool> Balance = FeatureKey<bool>.Bool("Balance");
    public static readonly FeatureKey<bool> FreeProduction = FeatureKey<bool>.Bool("FreeProduction");
    public static readonly FeatureKey<bool> FreeUpgrade = FeatureKey<bool>.Bool("FreeUpgrade");
    public static readonly FeatureKey<bool> NoSpellCost = FeatureKey<bool>.Bool("NoSpellCost");
    public static readonly FeatureKey<bool> HousingCapacity20x = FeatureKey<bool>.Bool("HousingCapacity20x");
    public static readonly FeatureKey<bool> StorageCapacity10x = FeatureKey<bool>.Bool("StorageCapacity10x");
    public static readonly FeatureKey<bool> HqHp10x = FeatureKey<bool>.Bool("HqHp10x");
    public static readonly FeatureKey<bool> FastBuildUpgradeRepair = FeatureKey<bool>.Bool("FastBuildUpgradeRepair");
    public static readonly FeatureKey<bool> FoodHealing10x = FeatureKey<bool>.Bool("FoodHealing10x");
    public static readonly FeatureKey<bool> MaxPopulation = FeatureKey<bool>.Bool("MaxPopulation");
    public static readonly FeatureKey<bool> CiviProduce20 = FeatureKey<bool>.Bool("CiviProduce20");
    public static readonly FeatureKey<bool> UnitRecruit20 = FeatureKey<bool>.Bool("UnitRecruit20");
    public static readonly FeatureKey<bool> IdleSelect999 = FeatureKey<bool>.Bool("IdleSelect999");
    public static readonly FeatureKey<bool> RomanEndless = FeatureKey<bool>.Bool("RomanEndless");
    public static readonly FeatureKey<bool> RomanReinforcementGarrison = FeatureKey<bool>.Bool("RomanReinforcementGarrison");
    public static readonly FeatureKey<bool> SpellDamage5x = FeatureKey<bool>.Bool("SpellDamage5x");
    public static readonly FeatureKey<bool> SpellHealing10x = FeatureKey<bool>.Bool("SpellHealing10x");
    public static readonly FeatureKey<bool> SpellResurrection = FeatureKey<bool>.Bool("SpellResurrection");
    public static readonly FeatureKey<bool> GeneralSkills = FeatureKey<bool>.Bool("GeneralSkills");
    public static readonly FeatureKey<bool> LeaderGlory = FeatureKey<bool>.Bool("LeaderGlory");
    public static readonly FeatureKey<bool> AllUnitsEntireMapVision = FeatureKey<bool>.Bool("AllUnitsEntireMapVision");
    public static readonly FeatureKey<bool> RangedRange3x = FeatureKey<bool>.Bool("RangedRange3x");
    public static readonly FeatureKey<bool> UnitMovementSpeed2x = FeatureKey<bool>.Bool("UnitMovementSpeed2x");
    public static readonly FeatureKey<bool> VillagerMovementSpeed5x = FeatureKey<bool>.Bool("VillagerMovementSpeed5x");
    public static readonly FeatureKey<bool> SpellEntireMap = FeatureKey<bool>.Bool("SpellEntireMap");
    public static readonly FeatureKey<bool> SpellRange3x = FeatureKey<bool>.Bool("SpellRange3x");
    public static readonly FeatureKey<bool> ProjectileArcHeight = FeatureKey<bool>.Bool("ProjectileArcHeight");
    public static readonly FeatureKey<Dictionary<string, double[]>?> CustomUnitStats = FeatureKey<Dictionary<string, double[]>?>.Reference<Dictionary<string, double[]>>("CustomUnitStats");
    public static readonly FeatureKey<bool> FocusLoss = FeatureKey<bool>.Bool("FocusLoss");
    public static readonly FeatureKey<bool> VillageBuildRange = FeatureKey<bool>.Bool("VillageBuildRange");
    public static readonly FeatureKey<bool> NoSpellAltar = FeatureKey<bool>.Bool("NoSpellAltar");
    public static readonly FeatureKey<int> GameSpeed = FeatureKey<int>.Int("GameSpeed", 1);
    public static readonly FeatureKey<bool> DgVoodoo = FeatureKey<bool>.Bool("DgVoodoo");
    public static readonly FeatureKey<bool> ArgmTrace = FeatureKey<bool>.Bool("ArgmTrace");
    public static readonly FeatureKey<bool> NativeWidescreen1920x1080 = FeatureKey<bool>.Bool("NativeWidescreen1920x1080");
    public static readonly FeatureKey<bool> CameraZoomOut1 = FeatureKey<bool>.Bool("CameraZoomOut1");
    public static readonly FeatureKey<bool> EndlessAiM1 = FeatureKey<bool>.Bool("EndlessAi.M1");
    public static readonly FeatureKey<bool> EndlessAiCore = FeatureKey<bool>.Bool("EndlessAi.Core");
    public static readonly FeatureKey<bool> EndlessAiM2 = FeatureKey<bool>.Bool("EndlessAi.M2");
    public static readonly FeatureKey<bool> EndlessAiM3 = FeatureKey<bool>.Bool("EndlessAi.M3");
    public static readonly FeatureKey<bool> EndlessAiM4 = FeatureKey<bool>.Bool("EndlessAi.M4");
    public static readonly FeatureKey<bool> EndlessAiM5 = FeatureKey<bool>.Bool("EndlessAi.M5");
    public static readonly FeatureKey<bool> EndlessAiM6 = FeatureKey<bool>.Bool("EndlessAi.M6");
    public static readonly FeatureKey<bool> ToEnglish = FeatureKey<bool>.Bool("ToEnglish");

    public static FeatureKey<bool> EndlessAi(string moduleId) => moduleId.ToUpperInvariant() switch
    {
        "M1" => EndlessAiM1,
        "CORE" => EndlessAiCore,
        "M2" => EndlessAiM2,
        "M3" => EndlessAiM3,
        "M4" => EndlessAiM4,
        "M5" => EndlessAiM5,
        "M6" => EndlessAiM6,
        _ => throw new ArgumentOutOfRangeException(nameof(moduleId), moduleId, "Unknown Endless AI module id."),
    };
}
