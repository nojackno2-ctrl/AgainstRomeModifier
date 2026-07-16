using AgainstRomeModifier.Core.Features;
using AgainstRomeModifier.Core.Patches;

namespace AgainstRomeModifier.Core.Services;

internal sealed class UnitStatsProjectionService
{
    private readonly BackupManager _backupManager;

    internal UnitStatsProjectionService(BackupManager backupManager) => _backupManager = backupManager;

    internal double[] Project(string key, PatchProfile profile)
    {
        double[] result = _backupManager.GetBaseStatsForUnit(key, profile);
        if (!TroopConfig.UnitMeta.TryGetValue(key, out var metadata)) return result;

        if (profile.RangedRange3x && TroopConfig.SupportsRangedRange3x(metadata.UnitType))
            result[7] *= 3.0;
        if (profile.UnitMovementSpeed2x)
            result[4] *= 2.0;
        if (profile.AllUnitsEntireMapVision)
            result[5] = ObjdefPatcher.EntireMapSight;
        if (metadata.UnitType == "priest")
        {
            if (profile.AllUnitsEntireMapVision || profile.SpellEntireMap)
                result[7] = ObjdefPatcher.EntireMapSight;
            if (profile.SpellRange3x && UnitStatParser.SupportsConfigurableSpellRadius(key))
                result[8] *= 3.0;
        }

        return result;
    }
}
