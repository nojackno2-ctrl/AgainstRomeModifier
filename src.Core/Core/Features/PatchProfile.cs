namespace AgainstRomeModifier.Core.Features;

public sealed class PatchProfile
{
    private readonly Dictionary<string, FeatureValue> _values = new(StringComparer.OrdinalIgnoreCase);

    public void Set(string id, FeatureValue value) => _values[id] = value;

    public void Set<T>(FeatureKey<T> key, T value) => Set(key.Id, key.Encode(value));

    public FeatureValue Get(string id) =>
        _values.TryGetValue(id, out FeatureValue value) ? value : FeatureRegistry.GetDisabledValue(id);

    public T Get<T>(FeatureKey<T> key) => key.Decode(Get(key.Id));

    public bool GetBool(string id) => Get(id).AsBool;
    public int GetInt(string id) => Get(id).AsInt;
    public T? GetObject<T>(string id) where T : class => Get(id).AsObject as T;

    public bool FocusLoss { get => Get(FeatureKeys.FocusLoss); set => Set(FeatureKeys.FocusLoss, value); }
    public bool FastCiviProduction { get => Get(FeatureKeys.FastCiviProduction); set => Set(FeatureKeys.FastCiviProduction, value); }
    public bool InfiniteMorale { get => Get(FeatureKeys.InfiniteMorale); set => Set(FeatureKeys.InfiniteMorale, value); }
    public bool FreeProduction { get => Get(FeatureKeys.FreeProduction); set => Set(FeatureKeys.FreeProduction, value); }
    public bool FreeUpgrade { get => Get(FeatureKeys.FreeUpgrade); set => Set(FeatureKeys.FreeUpgrade, value); }
    public bool NoSpellCost { get => Get(FeatureKeys.NoSpellCost); set => Set(FeatureKeys.NoSpellCost, value); }
    public bool MaxPopulation { get => Get(FeatureKeys.MaxPopulation); set => Set(FeatureKeys.MaxPopulation, value); }
    public bool RomanEndless { get => Get(FeatureKeys.RomanEndless); set => Set(FeatureKeys.RomanEndless, value); }
    public bool Balance { get => Get(FeatureKeys.Balance); set => Set(FeatureKeys.Balance, value); }
    public bool HousingCapacity20x { get => Get(FeatureKeys.HousingCapacity20x); set => Set(FeatureKeys.HousingCapacity20x, value); }
    public bool StorageCapacity10x { get => Get(FeatureKeys.StorageCapacity10x); set => Set(FeatureKeys.StorageCapacity10x, value); }
    public bool HqHp10x { get => Get(FeatureKeys.HqHp10x); set => Set(FeatureKeys.HqHp10x, value); }
    public bool FastBuildUpgradeRepair { get => Get(FeatureKeys.FastBuildUpgradeRepair); set => Set(FeatureKeys.FastBuildUpgradeRepair, value); }
    public bool FoodHealing10x { get => Get(FeatureKeys.FoodHealing10x); set => Set(FeatureKeys.FoodHealing10x, value); }
    public bool CiviProduce20 { get => Get(FeatureKeys.CiviProduce20); set => Set(FeatureKeys.CiviProduce20, value); }
    public bool UnitRecruit20 { get => Get(FeatureKeys.UnitRecruit20); set => Set(FeatureKeys.UnitRecruit20, value); }
    public bool IdleSelect999 { get => Get(FeatureKeys.IdleSelect999); set => Set(FeatureKeys.IdleSelect999, value); }
    public bool VillageBuildRange { get => Get(FeatureKeys.VillageBuildRange); set => Set(FeatureKeys.VillageBuildRange, value); }
    public bool DgVoodoo { get => Get(FeatureKeys.DgVoodoo); set => Set(FeatureKeys.DgVoodoo, value); }
    public bool NativeWidescreen1920x1080 { get => Get(FeatureKeys.NativeWidescreen1920x1080); set => Set(FeatureKeys.NativeWidescreen1920x1080, value); }
    public bool CameraZoomOut1 { get => Get(FeatureKeys.CameraZoomOut1); set => Set(FeatureKeys.CameraZoomOut1, value); }
    public bool ToEnglish { get => Get(FeatureKeys.ToEnglish); set => Set(FeatureKeys.ToEnglish, value); }
    public bool NoSpellAltar { get => Get(FeatureKeys.NoSpellAltar); set => Set(FeatureKeys.NoSpellAltar, value); }
    public bool SpellDamage5x { get => Get(FeatureKeys.SpellDamage5x); set => Set(FeatureKeys.SpellDamage5x, value); }
    public bool SpellHealing10x { get => Get(FeatureKeys.SpellHealing10x); set => Set(FeatureKeys.SpellHealing10x, value); }
    public bool SpellResurrection { get => Get(FeatureKeys.SpellResurrection); set => Set(FeatureKeys.SpellResurrection, value); }
    public bool GeneralSkills { get => Get(FeatureKeys.GeneralSkills); set => Set(FeatureKeys.GeneralSkills, value); }
    public bool LeaderGlory { get => Get(FeatureKeys.LeaderGlory); set => Set(FeatureKeys.LeaderGlory, value); }
    public bool AllUnitsEntireMapVision { get => Get(FeatureKeys.AllUnitsEntireMapVision); set => Set(FeatureKeys.AllUnitsEntireMapVision, value); }
    public bool RangedRange3x { get => Get(FeatureKeys.RangedRange3x); set => Set(FeatureKeys.RangedRange3x, value); }
    public bool UnitMovementSpeed2x { get => Get(FeatureKeys.UnitMovementSpeed2x); set => Set(FeatureKeys.UnitMovementSpeed2x, value); }
    public bool VillagerMovementSpeed5x { get => Get(FeatureKeys.VillagerMovementSpeed5x); set => Set(FeatureKeys.VillagerMovementSpeed5x, value); }
    public bool SpellEntireMap { get => Get(FeatureKeys.SpellEntireMap); set => Set(FeatureKeys.SpellEntireMap, value); }
    public bool SpellRange3x { get => Get(FeatureKeys.SpellRange3x); set => Set(FeatureKeys.SpellRange3x, value); }
    public bool ProjectileArcHeight { get => Get(FeatureKeys.ProjectileArcHeight); set => Set(FeatureKeys.ProjectileArcHeight, value); }
    public int GameSpeed { get => Get(FeatureKeys.GameSpeed); set => Set(FeatureKeys.GameSpeed, value); }
    public Dictionary<string, bool> EndlessAiModules { get; set; } = new(StringComparer.OrdinalIgnoreCase);
    public bool GetEndlessAiModule(string moduleId)
    {
        if (EndlessAiModules.TryGetValue(moduleId, out bool enabled)) return enabled;
        if (IsLegacyCoreModule(moduleId)) return Get(FeatureKeys.EndlessAiCore);
        return Get(FeatureKeys.EndlessAi(moduleId));
    }
    public Dictionary<string, double[]>? CustomUnitStats
    {
        get => Get(FeatureKeys.CustomUnitStats);
        set => Set(FeatureKeys.CustomUnitStats, value);
    }

    public void NormalizeCompositeValues()
    {
        foreach (var (id, enabled) in EndlessAiModules) Set(FeatureKeys.EndlessAi(id), enabled);

        bool coreEnabled = Get(FeatureKeys.EndlessAiCore);
        bool hasLegacyCoreSelection = false;
        foreach (string id in new[] { "M2", "M3", "M4" })
        {
            if (!EndlessAiModules.TryGetValue(id, out bool enabled)) continue;
            hasLegacyCoreSelection = true;
            coreEnabled |= enabled;
        }

        // Old profiles could independently select M2/M3/M4. Once any part of that
        // lifecycle was requested, migrate it to the complete integrated core.
        if (hasLegacyCoreSelection || EndlessAiModules.ContainsKey("Core"))
            EndlessAiModules["Core"] = coreEnabled;

        foreach (string id in new[] { "M2", "M3", "M4" })
        {
            if (hasLegacyCoreSelection || EndlessAiModules.ContainsKey("Core"))
                EndlessAiModules[id] = coreEnabled;
        }

        Set(FeatureKeys.EndlessAiCore, coreEnabled);
        Set(FeatureKeys.EndlessAiM2, coreEnabled);
        Set(FeatureKeys.EndlessAiM3, coreEnabled);
        Set(FeatureKeys.EndlessAiM4, coreEnabled);
    }

    private static bool IsLegacyCoreModule(string moduleId) =>
        moduleId.Equals("M2", StringComparison.OrdinalIgnoreCase) ||
        moduleId.Equals("M3", StringComparison.OrdinalIgnoreCase) ||
        moduleId.Equals("M4", StringComparison.OrdinalIgnoreCase);
}
