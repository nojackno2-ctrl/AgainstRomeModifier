using AgainstRomeModifier.Core.Features;

namespace AgainstRomeModifier.Tests;

public sealed class FeatureRegistryTests
{
    [Fact]
    public void Registry_ids_are_unique_and_game_speed_disables_to_one()
    {
        Assert.Equal(FeatureRegistry.All.Count, FeatureRegistry.All.Select(x => x.Id).Distinct(StringComparer.OrdinalIgnoreCase).Count());
        Assert.Equal(1, FeatureRegistry.GetDisabledValue("GameSpeed").AsInt);
        Assert.All(FeatureRegistry.ByCategory(FeatureCategory.Stats), feature => Assert.Equal(FeatureCategory.Stats, feature.Category));
        Assert.All(FeatureRegistry.All, feature => Assert.IsAssignableFrom<IFeatureModule>(feature));
    }

    [Fact]
    public void PatchProfile_supports_named_and_registry_values()
    {
        var profile = new PatchProfile { FocusLoss = true, GameSpeed = 3, Balance = true, SpellDamage5x = true, SpellHealing10x = true, SpellResurrection = true, GeneralSkills = true, LeaderGlory = true, RangedRange3x = true, UnitMovementSpeed2x = true, SpellEntireMap = true, SpellRange3x = true, ProjectileArcHeight = true, RangedAccuracy = true };
        profile.EndlessAiModules["M4"] = true;
        profile.NormalizeCompositeValues();

        Assert.True(profile.GetBool("FocusLoss"));
        Assert.True(profile.GetBool("Balance"));
        Assert.True(profile.GetBool("SpellDamage5x"));
        Assert.True(profile.GetBool("SpellHealing10x"));
        Assert.True(profile.GetBool("SpellResurrection"));
        Assert.True(profile.GetBool("GeneralSkills"));
        Assert.True(profile.GetBool("LeaderGlory"));
        Assert.True(profile.GetBool("RangedRange3x"));
        Assert.True(profile.GetBool("UnitMovementSpeed2x"));
        Assert.True(profile.GetBool("SpellEntireMap"));
        Assert.True(profile.GetBool("SpellRange3x"));
        Assert.True(profile.GetBool("ProjectileArcHeight"));
        Assert.True(profile.GetBool("RangedAccuracy"));
        Assert.True(profile.GetBool("EndlessAi.M4"));
        Assert.Equal(3, profile.GetInt("GameSpeed"));
    }

    [Fact]
    public void Registry_plan_and_detect_round_trip_profile_values()
    {
        var source = new PatchProfile { FocusLoss = true, GameSpeed = 4, Balance = true, SpellDamage5x = true, SpellHealing10x = true, SpellResurrection = true, GeneralSkills = true, LeaderGlory = true, RangedRange3x = true, UnitMovementSpeed2x = true, SpellEntireMap = true, SpellRange3x = true, ProjectileArcHeight = true, RangedAccuracy = true };
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
        Assert.True(roundTrip.RangedRange3x);
        Assert.True(roundTrip.UnitMovementSpeed2x);
        Assert.True(roundTrip.SpellEntireMap);
        Assert.True(roundTrip.SpellRange3x);
        Assert.True(roundTrip.ProjectileArcHeight);
        Assert.True(roundTrip.RangedAccuracy);
        Assert.Equal(4, roundTrip.GameSpeed);
    }
}
