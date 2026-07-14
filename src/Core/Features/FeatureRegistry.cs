namespace AgainstRomeModifier.Core.Features;

public sealed record FeatureDefinition(string Id, FeatureCategory Category, FeatureValue DisabledValue) : IFeatureModule
{
    public void Plan(PatchContext context, FeatureValue value) => context.Set(Id, value);
    public FeatureValue Detect(DetectContext context) => context.DetectedProfile.Get(Id);
}

public static class FeatureRegistry
{
    public static IReadOnlyList<FeatureDefinition> All { get; } = new FeatureDefinition[]
    {
        Bool("FastCiviProduction", FeatureCategory.Stats), Bool("InfiniteMorale", FeatureCategory.Stats),
        Bool("Balance", FeatureCategory.Stats), Bool("FreeProduction", FeatureCategory.Stats),
        Bool("FreeUpgrade", FeatureCategory.Stats), Bool("NoSpellCost", FeatureCategory.Stats),
        Bool("HousingCapacity20x", FeatureCategory.Stats), Bool("StorageCapacity10x", FeatureCategory.Stats),
        Bool("HqHp10x", FeatureCategory.Stats), Bool("FastBuildUpgradeRepair", FeatureCategory.Stats),
        Bool("FoodHealing10x", FeatureCategory.Stats), Bool("MaxPopulation", FeatureCategory.Stats),
        Bool("CiviProduce20", FeatureCategory.Stats),
        Bool("RomanEndless", FeatureCategory.Stats),
        Bool("SpellDamage5x", FeatureCategory.Stats), Bool("SpellHealing10x", FeatureCategory.Stats),
        Bool("SpellResurrection", FeatureCategory.Stats), Bool("GeneralSkills", FeatureCategory.Stats),
        Bool("LeaderGlory", FeatureCategory.Stats),
        Bool("RangedRange3x", FeatureCategory.Stats), Bool("UnitMovementSpeed2x", FeatureCategory.Stats),
        Bool("SpellEntireMap", FeatureCategory.Stats), Bool("SpellRange3x", FeatureCategory.Stats),
        Bool("ProjectileArcHeight", FeatureCategory.Stats),
        new("CustomUnitStats", FeatureCategory.Stats, FeatureValue.Of((object?)null)),
        Bool("FocusLoss", FeatureCategory.Compat), Bool("VillageBuildRange", FeatureCategory.Compat),
        Bool("NoSpellAltar", FeatureCategory.Compat), new("GameSpeed", FeatureCategory.Compat, FeatureValue.Of(1)),
        Bool("DgVoodoo", FeatureCategory.Compat),
        Bool("EndlessAi.M1", FeatureCategory.Compat), Bool("EndlessAi.M2", FeatureCategory.Compat),
        Bool("EndlessAi.M3", FeatureCategory.Compat), Bool("EndlessAi.M4", FeatureCategory.Compat),
        Bool("EndlessAi.M5", FeatureCategory.Compat), Bool("EndlessAi.M6", FeatureCategory.Compat),
        Bool("ToEnglish", FeatureCategory.Language),
    };

    public static IEnumerable<FeatureDefinition> ByCategory(FeatureCategory category) => All.Where(x => x.Category == category);

    public static FeatureValue GetDisabledValue(string id) =>
        All.FirstOrDefault(x => x.Id.Equals(id, StringComparison.OrdinalIgnoreCase))?.DisabledValue ?? FeatureValue.Of(false);

    private static FeatureDefinition Bool(string id, FeatureCategory category) => new(id, category, FeatureValue.Of(false));
}
