using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class UnitStatParserTests
{
    [Fact]
    public void Parser_classifies_damage_reload_and_range_from_shared_weapon_slots()
    {
        string[] columns = Enumerable.Repeat("0", 256).ToArray();
        SetWeapon(columns, slot: 1, damageType: "0", damage: "10", reload: "2.5", minimumRange: "1", maximumRange: "50");
        SetWeapon(columns, slot: 2, damageType: "1", damage: "20", reload: "1.5", minimumRange: "5", maximumRange: "100");

        UnitStatParser.GetMeleeAndRangedDamage(columns, "hybrid_inf", out double meleeDamage, out double rangedDamage);
        UnitStatParser.GetMeleeAndRangedReload(columns, "hybrid_inf", out double meleeReload, out double rangedReload);

        Assert.Equal(10, meleeDamage);
        Assert.Equal(20, rangedDamage);
        Assert.Equal(2.5, meleeReload);
        Assert.Equal(1.5, rangedReload);
        Assert.Equal(100, UnitStatParser.GetMaximumRange(columns, "hybrid_inf"));

        columns[(int)ObjdefIndex.Sirad] = "300";
        Assert.Equal(300, UnitStatParser.GetMaximumRange(columns, "priest"));
    }

    private static void SetWeapon(
        string[] columns,
        int slot,
        string damageType,
        string damage,
        string reload,
        string minimumRange,
        string maximumRange)
    {
        int offset = (slot - 1) * 8;
        int activeIndex = (int)ObjdefIndex.Weapon1Akti + offset;
        columns[activeIndex] = "1";
        columns[activeIndex + 1] = damage;
        columns[activeIndex + 6] = reload;
        columns[(int)ObjdefIndex.Weapon1Dtyp + slot - 1] = damageType;
        columns[(int)ObjdefIndex.Weapon1RangeMin + offset] = minimumRange;
        columns[(int)ObjdefIndex.Weapon1RangeMax + offset] = maximumRange;
    }
}
