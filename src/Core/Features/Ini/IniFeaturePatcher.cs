using System.Collections.Generic;
using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features.Ini;

internal static class IniFeaturePatcher
{
    internal static byte[] BuildClScript(BackupManager backup, bool fastCivilianProduction, bool infiniteMorale, bool balance)
    {
        byte[] original = backup.GetBackupBytes("SYSTEM/cl_script.ini");
        double ger = 1, kel = 1, hun = 1;
        if (balance)
        {
            ger = backup.GetDefaultBalancedStats("FigGerPri00_Priester")[8] / 500.0;
            kel = backup.GetDefaultBalancedStats("FigKelPri00_Priester")[8] / 500.0;
            hun = backup.GetDefaultBalancedStats("FigHunPri00_Priester")[8] / 500.0;
        }
        return ClScriptPatcher.GetPatchedBytes(original, new ClScriptOptions(
            fastCivilianProduction, infiniteMorale, false, new Dictionary<string, double>(), ger, kel, hun, original));
    }

    internal static byte[] BuildRess(BackupManager backup, bool freeProduction, bool freeUpgrade, bool noSpellCost) =>
        RessPatcher.GetPatchedBytes(backup.GetBackupBytes("SYSTEM/ress.ini"), new RessOptions(freeProduction, freeUpgrade, noSpellCost));
}
