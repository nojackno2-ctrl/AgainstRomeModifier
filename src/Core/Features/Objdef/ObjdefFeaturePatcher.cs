using System;
using System.Collections.Generic;
using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Services;
using AgainstRomeModifier.Core.Features;

namespace AgainstRomeModifier.Core.Features.Objdef;

internal static class ObjdefFeaturePatcher
{
    internal static byte[] Build(BackupManager backup, PatchProfile options)
    {
        byte[] original = backup.GetBackupBytes("SYSTEM/DATA_MP/DEFAULTS/objdef.dau");
        var unitStats = new Dictionary<string, double[]>(StringComparer.OrdinalIgnoreCase);
        foreach (string key in TroopConfig.UnitMeta.Keys) unitStats[key] = backup.GetBaseStatsForUnit(key, options);
        return ObjdefPatcher.GetPatchedBytes(original, new ObjdefOptions(
            options.Balance, options.HousingCapacity20x, options.StorageCapacity10x,
            options.FastBuildUpgradeRepair, options.HqHp10x, new Dictionary<string, double[]>(), unitStats));
    }
}
