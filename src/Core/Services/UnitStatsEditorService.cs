namespace AgainstRomeModifier.Core.Services;

internal sealed class UnitStatsEditorService
{
    private readonly BackupManager _backup;

    internal UnitStatsEditorService(BackupManager backup) => _backup = backup;

    internal double[] GetOriginal(string key) => _backup.GetOriginalStats(key);
    internal double[] GetBalanced(string key) => _backup.GetDefaultBalancedStats(key);

    internal double[] NormalizeIndependentFields(string key, double[] stats)
    {
        double[] normalized = (double[])stats.Clone();
        double[] original = GetOriginal(key);
        foreach (int index in new[] { 4, 7, 8 })
            if (index < normalized.Length && index < original.Length) normalized[index] = original[index];
        if (TroopConfig.UnitMeta.TryGetValue(key, out var metadata) && metadata.UnitType == "priest" &&
            normalized.Length > 5 && original.Length > 5)
            normalized[5] = original[5];
        return normalized;
    }
}
