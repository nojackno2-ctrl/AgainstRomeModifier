using System;
using System.Collections.Generic;
using System.Linq;
using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features.Map;

internal static class MaxPopulationFeature
{
    internal static Dictionary<string, byte[]> Build(BackupManager backup, bool enabled)
    {
        var results = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
        foreach (var item in backup.BackupFiles.Where(item => item.Key.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) && item.Key.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase)))
            results[item.Key] = TeamDatPatcher.GetPatchedBytes(item.Value, new TeamDatOptions(enabled));
        return results;
    }
}
