namespace AgainstRomeModifier.Core.Features;

public enum FeatureControlKind
{
    Toggle,
    Specialized,
}

public sealed record FeatureDefinition(IFeatureKey Key, FeatureCategory Category, FeatureControlKind ControlKind = FeatureControlKind.Toggle) : IFeatureModule
{
    public string Id => Key.Id;
    public FeatureValue DisabledValue => Key.DisabledValue;
    public void Plan(PatchContext context, FeatureValue value) => context.Set(Id, value);
    public FeatureValue Detect(DetectContext context) => context.DetectedProfile.Get(Id);
}

public static class FeatureRegistry
{
    public static IReadOnlyList<FeatureDefinition> All { get; } = new FeatureDefinition[]
    {
        Bool(FeatureKeys.FastCiviProduction, FeatureCategory.Stats), Bool(FeatureKeys.InfiniteMorale, FeatureCategory.Stats),
        Bool(FeatureKeys.NoRunHpLoss, FeatureCategory.Stats),
        Bool(FeatureKeys.Balance, FeatureCategory.Stats), Bool(FeatureKeys.FreeProduction, FeatureCategory.Stats),
        Bool(FeatureKeys.FreeUpgrade, FeatureCategory.Stats), Bool(FeatureKeys.NoSpellCost, FeatureCategory.Stats),
        Bool(FeatureKeys.HousingCapacity20x, FeatureCategory.Stats), Bool(FeatureKeys.StorageCapacity10x, FeatureCategory.Stats),
        Bool(FeatureKeys.HqHp10x, FeatureCategory.Stats), Bool(FeatureKeys.FastBuildUpgradeRepair, FeatureCategory.Stats),
        Bool(FeatureKeys.FoodHealing10x, FeatureCategory.Stats), Bool(FeatureKeys.MaxPopulation, FeatureCategory.Stats),
        Bool(FeatureKeys.CiviProduce20, FeatureCategory.Stats), Bool(FeatureKeys.UnitRecruit20, FeatureCategory.Stats),
        Bool(FeatureKeys.IdleSelect999, FeatureCategory.Stats),
        Bool(FeatureKeys.RomanEndless, FeatureCategory.Stats),
        Bool(FeatureKeys.RomanReinforcementGarrison, FeatureCategory.Compat),
        new(FeatureKeys.VillageGarrisonQuotaMultiplier, FeatureCategory.Compat, FeatureControlKind.Specialized),
        Bool(FeatureKeys.SpellDamage5x, FeatureCategory.Stats), Bool(FeatureKeys.SpellHealing10x, FeatureCategory.Stats),
        Bool(FeatureKeys.SpellResurrection, FeatureCategory.Stats), Bool(FeatureKeys.GeneralSkills, FeatureCategory.Stats),
        Bool(FeatureKeys.LeaderGlory, FeatureCategory.Stats),
        Bool(FeatureKeys.RangedRange3x, FeatureCategory.Stats), Bool(FeatureKeys.UnitMovementSpeed2x, FeatureCategory.Stats),
        Bool(FeatureKeys.VillagerMovementSpeed5x, FeatureCategory.Stats),
        Bool(FeatureKeys.SpellEntireMap, FeatureCategory.Stats), Bool(FeatureKeys.SpellRange3x, FeatureCategory.Stats),
        Bool(FeatureKeys.ProjectileArcHeight, FeatureCategory.Stats),
        Bool(FeatureKeys.AllUnitsEntireMapVision, FeatureCategory.Stats),
        new(FeatureKeys.CustomUnitStats, FeatureCategory.Stats, FeatureControlKind.Specialized),
        Bool(FeatureKeys.FocusLoss, FeatureCategory.Compat), Bool(FeatureKeys.VillageBuildRange, FeatureCategory.Compat),
        Bool(FeatureKeys.NoSpellAltar, FeatureCategory.Compat), new(FeatureKeys.GameSpeed, FeatureCategory.Compat, FeatureControlKind.Specialized),
        Bool(FeatureKeys.DgVoodoo, FeatureCategory.Compat),
        Bool(FeatureKeys.ArgmTrace, FeatureCategory.Compat),
        Bool(FeatureKeys.NativeWidescreen1920x1080, FeatureCategory.Compat),
        Bool(FeatureKeys.CameraZoomOut1, FeatureCategory.Compat),
        Bool(FeatureKeys.EndlessAiM1, FeatureCategory.Compat), Bool(FeatureKeys.EndlessAiCore, FeatureCategory.Compat),
        new(FeatureKeys.EndlessAiM2, FeatureCategory.Compat, FeatureControlKind.Specialized),
        new(FeatureKeys.EndlessAiM3, FeatureCategory.Compat, FeatureControlKind.Specialized),
        new(FeatureKeys.EndlessAiM4, FeatureCategory.Compat, FeatureControlKind.Specialized),
        Bool(FeatureKeys.EndlessAiM5, FeatureCategory.Compat),
        new(FeatureKeys.EndlessAiM6, FeatureCategory.Compat, FeatureControlKind.Specialized),
        Bool(FeatureKeys.ToEnglish, FeatureCategory.Language),
    };

    public static IEnumerable<FeatureDefinition> ByCategory(FeatureCategory category) => All.Where(x => x.Category == category);
    public static IEnumerable<FeatureDefinition> ToggleFeatures => All.Where(x => x.ControlKind == FeatureControlKind.Toggle);

    public static FeatureValue GetDisabledValue(string id) =>
        All.FirstOrDefault(x => x.Id.Equals(id, StringComparison.OrdinalIgnoreCase))?.DisabledValue ?? FeatureValue.Of(false);

    public static void ValidateToggleIds(IEnumerable<string> ids)
    {
        var actual = ids.ToHashSet(StringComparer.OrdinalIgnoreCase);
        var expected = ToggleFeatures.Select(x => x.Id).ToHashSet(StringComparer.OrdinalIgnoreCase);
        if (actual.SetEquals(expected)) return;

        string missing = string.Join(", ", expected.Except(actual, StringComparer.OrdinalIgnoreCase).OrderBy(x => x));
        string unexpected = string.Join(", ", actual.Except(expected, StringComparer.OrdinalIgnoreCase).OrderBy(x => x));
        throw new InvalidOperationException($"Feature toggle map mismatch. Missing: [{missing}]. Unexpected: [{unexpected}].");
    }

    private static FeatureDefinition Bool(FeatureKey<bool> key, FeatureCategory category) => new(key, category);
}
