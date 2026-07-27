using System.Globalization;

namespace AgainstRomeModifier.Core.Services;

internal static class UnitStatParser
{
    internal static bool SupportsConfigurableSpellRadius(string key) =>
        key.Equals("FigKelPri00_Priester", StringComparison.OrdinalIgnoreCase) ||
        key.Equals("FigHunPri00_Priester", StringComparison.OrdinalIgnoreCase);

    internal static double[] MergeLayers(
        double[] fallback,
        double[] custom,
        bool supportsSpellRadius,
        bool ignoreMovementSpeed = false,
        bool ignoreRange = false,
        bool ignoreSpellRadius = false,
        bool removePriestSight = false)
    {
        ArgumentNullException.ThrowIfNull(fallback);
        ArgumentNullException.ThrowIfNull(custom);

        double[] layered = new double[9];
        for (int index = 0; index < layered.Length; index++)
        {
            if (index == 8 && !supportsSpellRadius)
            {
                layered[index] = 0;
                continue;
            }

            if (index is 4 or 7 or 8 || (index == 5 && removePriestSight))
            {
                layered[index] = fallback.Length > index ? fallback[index] : 0;
                continue;
            }

            layered[index] = custom.Length > index
                ? custom[index]
                : (fallback.Length > index ? fallback[index] : 0);
        }

        return layered;
    }

    internal static void GetMeleeAndRangedDamage(
        string[] columns,
        string unitType,
        out double meleeDamage,
        out double rangedDamage)
    {
        meleeDamage = 0;
        rangedDamage = 0;
        for (int weapon = 1; weapon <= 8; weapon++)
        {
            int activeIndex = (int)ObjdefIndex.Weapon1Akti + (weapon - 1) * 8;
            int damageIndex = activeIndex + 1;
            int typeIndex = (int)ObjdefIndex.Weapon1Dtyp + (weapon - 1);
            if (activeIndex >= columns.Length || damageIndex >= columns.Length || typeIndex >= columns.Length ||
                columns[activeIndex].Trim() != "1")
                continue;

            double.TryParse(columns[damageIndex].Trim(), NumberStyles.Any,
                CultureInfo.InvariantCulture, out double value);
            bool ranged = IsRangedWeapon(columns[typeIndex].Trim(), unitType);
            if (ranged)
                rangedDamage = Math.Max(rangedDamage, value);
            else
                meleeDamage = Math.Max(meleeDamage, value);
        }
    }

    internal static void GetMeleeAndRangedReload(
        string[] columns,
        string unitType,
        out double meleeReload,
        out double rangedReload)
    {
        meleeReload = 0;
        rangedReload = 0;
        for (int weapon = 1; weapon <= 8; weapon++)
        {
            int activeIndex = (int)ObjdefIndex.Weapon1Akti + (weapon - 1) * 8;
            int reloadIndex = activeIndex + 6;
            int typeIndex = (int)ObjdefIndex.Weapon1Dtyp + (weapon - 1);
            if (activeIndex >= columns.Length || reloadIndex >= columns.Length || typeIndex >= columns.Length ||
                columns[activeIndex].Trim() != "1")
                continue;

            double.TryParse(columns[reloadIndex].Trim(), NumberStyles.Any,
                CultureInfo.InvariantCulture, out double value);
            if (value <= 0) continue;

            bool ranged = IsRangedWeapon(columns[typeIndex].Trim(), unitType);
            if (ranged)
                rangedReload = rangedReload == 0 ? value : Math.Min(rangedReload, value);
            else
                meleeReload = meleeReload == 0 ? value : Math.Min(meleeReload, value);
        }
    }

    internal static double GetMaximumRange(string[] columns, string unitType)
    {
        if (unitType == "priest")
            return double.TryParse(columns[(int)ObjdefIndex.Sirad].Trim(), NumberStyles.Any,
                CultureInfo.InvariantCulture, out double sight) ? sight : 0;

        double maximum = 0;
        for (int weapon = 1; weapon <= 8; weapon++)
        {
            int activeIndex = (int)ObjdefIndex.Weapon1Akti + (weapon - 1) * 8;
            int minimumIndex = (int)ObjdefIndex.Weapon1RangeMin + (weapon - 1) * 8;
            int maximumIndex = (int)ObjdefIndex.Weapon1RangeMax + (weapon - 1) * 8;
            if (maximumIndex >= columns.Length || columns[activeIndex].Trim() != "1") continue;

            if (double.TryParse(columns[minimumIndex].Trim(), NumberStyles.Any,
                    CultureInfo.InvariantCulture, out double minimumRange))
                maximum = Math.Max(maximum, minimumRange);
            if (double.TryParse(columns[maximumIndex].Trim(), NumberStyles.Any,
                    CultureInfo.InvariantCulture, out double maximumRange))
                maximum = Math.Max(maximum, maximumRange);
        }

        return maximum;
    }

    private static bool IsRangedWeapon(string damageType, string unitType) =>
        damageType is "1" or "2" or "3" or "4" || unitType == "siege";
}
