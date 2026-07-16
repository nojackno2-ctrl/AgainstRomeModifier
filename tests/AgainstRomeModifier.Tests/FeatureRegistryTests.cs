using AgainstRomeModifier.Core.Features;

namespace AgainstRomeModifier.Tests;

public sealed class FeatureRegistryTests
{
    [Fact]
    public void Registry_ids_are_unique_and_game_speed_disables_to_one()
    {
        Assert.Equal(FeatureRegistry.All.Count, FeatureRegistry.All.Select(x => x.Id).Distinct(StringComparer.OrdinalIgnoreCase).Count());
        Assert.Equal(1, FeatureRegistry.GetDisabledValue(FeatureKeys.GameSpeed.Id).AsInt);
        Assert.All(FeatureRegistry.ByCategory(FeatureCategory.Stats), feature => Assert.Equal(FeatureCategory.Stats, feature.Category));
        Assert.All(FeatureRegistry.All, feature => Assert.IsAssignableFrom<IFeatureModule>(feature));
    }

    [Fact]
    public void Every_declared_feature_key_is_registered_once()
    {
        string[] declaredIds = typeof(FeatureKeys)
            .GetFields(System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.Static)
            .Select(field => Assert.IsAssignableFrom<IFeatureKey>(field.GetValue(null)))
            .Select(key => key.Id)
            .OrderBy(id => id, StringComparer.OrdinalIgnoreCase)
            .ToArray();
        string[] registeredIds = FeatureRegistry.All
            .Select(feature => feature.Id)
            .OrderBy(id => id, StringComparer.OrdinalIgnoreCase)
            .ToArray();

        Assert.Equal(registeredIds, declaredIds, StringComparer.OrdinalIgnoreCase);
    }

    [Fact]
    public void Toggle_map_validation_rejects_missing_or_specialized_features()
    {
        string[] validIds = FeatureRegistry.ToggleFeatures.Select(feature => feature.Id).ToArray();
        FeatureRegistry.ValidateToggleIds(validIds);

        InvalidOperationException missing = Assert.Throws<InvalidOperationException>(
            () => FeatureRegistry.ValidateToggleIds(validIds.Where(id => !id.Equals(FeatureKeys.UnitRecruit20.Id, StringComparison.OrdinalIgnoreCase))));
        Assert.Contains(FeatureKeys.UnitRecruit20.Id, missing.Message);

        InvalidOperationException specialized = Assert.Throws<InvalidOperationException>(
            () => FeatureRegistry.ValidateToggleIds(validIds.Append(FeatureKeys.GameSpeed.Id)));
        Assert.Contains(FeatureKeys.GameSpeed.Id, specialized.Message);
    }

    [Fact]
    public void PatchProfile_supports_named_and_registry_values()
    {
        var profile = new PatchProfile { FocusLoss = true, GameSpeed = 3, Balance = true, SpellDamage5x = true, SpellHealing10x = true, SpellResurrection = true, GeneralSkills = true, LeaderGlory = true, AllUnitsEntireMapVision = true, RangedRange3x = true, UnitMovementSpeed2x = true, SpellEntireMap = true, SpellRange3x = true, ProjectileArcHeight = true, RomanEndless = true, NativeWidescreen1920x1080 = true };
        profile.EndlessAiModules["M4"] = true;
        profile.NormalizeCompositeValues();

        profile.Set(FeatureKeys.UnitRecruit20, true);

        Assert.True(profile.GetBool("FocusLoss"));
        Assert.True(profile.GetBool("Balance"));
        Assert.True(profile.GetBool("SpellDamage5x"));
        Assert.True(profile.GetBool("SpellHealing10x"));
        Assert.True(profile.GetBool("SpellResurrection"));
        Assert.True(profile.GetBool("GeneralSkills"));
        Assert.True(profile.GetBool("LeaderGlory"));
        Assert.True(profile.GetBool("AllUnitsEntireMapVision"));
        Assert.True(profile.GetBool("RangedRange3x"));
        Assert.True(profile.GetBool("UnitMovementSpeed2x"));
        Assert.True(profile.GetBool("SpellEntireMap"));
        Assert.True(profile.GetBool("SpellRange3x"));
        Assert.True(profile.GetBool("ProjectileArcHeight"));
        Assert.True(profile.GetBool("RomanEndless"));
        Assert.True(profile.GetBool("NativeWidescreen1920x1080"));
        Assert.True(profile.GetBool("EndlessAi.M4"));
        Assert.Equal(3, profile.GetInt("GameSpeed"));
        Assert.True(profile.Get(FeatureKeys.UnitRecruit20));
        Assert.Equal(1, new PatchProfile().Get(FeatureKeys.GameSpeed));
    }

    [Fact]
    public void Legacy_endless_core_keys_are_specialized_and_the_core_is_toggleable()
    {
        string[] toggleIds = FeatureRegistry.ToggleFeatures.Select(feature => feature.Id).ToArray();

        Assert.Contains(FeatureKeys.EndlessAiCore.Id, toggleIds);
        Assert.DoesNotContain(FeatureKeys.EndlessAiM2.Id, toggleIds);
        Assert.DoesNotContain(FeatureKeys.EndlessAiM3.Id, toggleIds);
        Assert.DoesNotContain(FeatureKeys.EndlessAiM4.Id, toggleIds);
    }

    [Fact]
    public void Registry_plan_and_detect_round_trip_profile_values()
    {
        var source = new PatchProfile { FocusLoss = true, GameSpeed = 4, Balance = true, SpellDamage5x = true, SpellHealing10x = true, SpellResurrection = true, GeneralSkills = true, LeaderGlory = true, AllUnitsEntireMapVision = true, RangedRange3x = true, UnitMovementSpeed2x = true, SpellEntireMap = true, SpellRange3x = true, ProjectileArcHeight = true, RomanEndless = true, NativeWidescreen1920x1080 = true };
        var plan = new PatchContext();
        foreach (IFeatureModule module in FeatureRegistry.All) module.Plan(plan, source.Get(module.Id));

        var detect = new DetectContext(plan.Profile);
        var roundTrip = new PatchProfile();
        foreach (IFeatureModule module in FeatureRegistry.All) roundTrip.Set(module.Id, module.Detect(detect));

        Assert.True(roundTrip.FocusLoss);
        Assert.True(roundTrip.Balance);
        Assert.True(roundTrip.SpellDamage5x);
        Assert.True(roundTrip.SpellHealing10x);
        Assert.True(roundTrip.SpellResurrection);
        Assert.True(roundTrip.GeneralSkills);
        Assert.True(roundTrip.LeaderGlory);
        Assert.True(roundTrip.AllUnitsEntireMapVision);
        Assert.True(roundTrip.RangedRange3x);
        Assert.True(roundTrip.UnitMovementSpeed2x);
        Assert.True(roundTrip.SpellEntireMap);
        Assert.True(roundTrip.SpellRange3x);
        Assert.True(roundTrip.ProjectileArcHeight);
        Assert.True(roundTrip.RomanEndless);
        Assert.True(roundTrip.NativeWidescreen1920x1080);
        Assert.Equal(4, roundTrip.GameSpeed);
    }
}
