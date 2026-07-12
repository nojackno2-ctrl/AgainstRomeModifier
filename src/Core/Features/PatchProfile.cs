namespace AgainstRomeModifier.Core.Features;

public sealed class PatchProfile
{
    private readonly Dictionary<string, FeatureValue> _values = new(StringComparer.OrdinalIgnoreCase);

    public void Set(string id, FeatureValue value) => _values[id] = value;

    public FeatureValue Get(string id) =>
        _values.TryGetValue(id, out FeatureValue value) ? value : FeatureRegistry.GetDisabledValue(id);

    public bool GetBool(string id) => Get(id).AsBool;
    public int GetInt(string id) => Get(id).AsInt;
    public T? GetObject<T>(string id) where T : class => Get(id).AsObject as T;

    private bool Bool(string id) => GetBool(id);
    private void Bool(string id, bool value) => Set(id, FeatureValue.Of(value));

    public bool FocusLoss { get => Bool("FocusLoss"); set => Bool("FocusLoss", value); }
    public bool FastCiviProduction { get => Bool("FastCiviProduction"); set => Bool("FastCiviProduction", value); }
    public bool InfiniteMorale { get => Bool("InfiniteMorale"); set => Bool("InfiniteMorale", value); }
    public bool FreeProduction { get => Bool("FreeProduction"); set => Bool("FreeProduction", value); }
    public bool FreeUpgrade { get => Bool("FreeUpgrade"); set => Bool("FreeUpgrade", value); }
    public bool NoSpellCost { get => Bool("NoSpellCost"); set => Bool("NoSpellCost", value); }
    public bool MaxPopulation { get => Bool("MaxPopulation"); set => Bool("MaxPopulation", value); }
    public bool Balance { get => Bool("Balance"); set => Bool("Balance", value); }
    public bool HousingCapacity20x { get => Bool("HousingCapacity20x"); set => Bool("HousingCapacity20x", value); }
    public bool StorageCapacity10x { get => Bool("StorageCapacity10x"); set => Bool("StorageCapacity10x", value); }
    public bool HqHp10x { get => Bool("HqHp10x"); set => Bool("HqHp10x", value); }
    public bool FastBuildUpgradeRepair { get => Bool("FastBuildUpgradeRepair"); set => Bool("FastBuildUpgradeRepair", value); }
    public bool FoodHealing10x { get => Bool("FoodHealing10x"); set => Bool("FoodHealing10x", value); }
    public bool VillageBuildRange { get => Bool("VillageBuildRange"); set => Bool("VillageBuildRange", value); }
    public bool DgVoodoo { get => Bool("DgVoodoo"); set => Bool("DgVoodoo", value); }
    public bool ToEnglish { get => Bool("ToEnglish"); set => Bool("ToEnglish", value); }
    public bool NoSpellAltar { get => Bool("NoSpellAltar"); set => Bool("NoSpellAltar", value); }
    public bool SpellDamage5x { get => Bool("SpellDamage5x"); set => Bool("SpellDamage5x", value); }
    public bool SpellHealing10x { get => Bool("SpellHealing10x"); set => Bool("SpellHealing10x", value); }
    public bool SpellResurrection { get => Bool("SpellResurrection"); set => Bool("SpellResurrection", value); }
    public bool GeneralSkills { get => Bool("GeneralSkills"); set => Bool("GeneralSkills", value); }
    public bool LeaderGlory { get => Bool("LeaderGlory"); set => Bool("LeaderGlory", value); }
    public bool RangedRange3x { get => Bool("RangedRange3x"); set => Bool("RangedRange3x", value); }
    public bool UnitMovementSpeed2x { get => Bool("UnitMovementSpeed2x"); set => Bool("UnitMovementSpeed2x", value); }
    public bool SpellEntireMap { get => Bool("SpellEntireMap"); set => Bool("SpellEntireMap", value); }
    public bool SpellRange3x { get => Bool("SpellRange3x"); set => Bool("SpellRange3x", value); }
    public bool ProjectileArcHeight { get => Bool("ProjectileArcHeight"); set => Bool("ProjectileArcHeight", value); }
    public bool RangedAccuracy { get => Bool("RangedAccuracy"); set => Bool("RangedAccuracy", value); }
    public int GameSpeed { get => GetInt("GameSpeed"); set => Set("GameSpeed", FeatureValue.Of(value)); }
    public Dictionary<string, bool> EndlessAiModules { get; set; } = new(StringComparer.OrdinalIgnoreCase);
    public bool GetEndlessAiModule(string moduleId) =>
        EndlessAiModules.TryGetValue(moduleId, out bool enabled) ? enabled : GetBool("EndlessAi." + moduleId);
    public Dictionary<string, double[]>? CustomUnitStats
    {
        get => GetObject<Dictionary<string, double[]>>("CustomUnitStats");
        set => Set("CustomUnitStats", FeatureValue.Of(value));
    }

    public void NormalizeCompositeValues()
    {
        foreach (var (id, enabled) in EndlessAiModules) Set("EndlessAi." + id, FeatureValue.Of(enabled));
    }
}
