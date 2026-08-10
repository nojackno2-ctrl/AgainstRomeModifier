namespace AgainstRomeModifier.Core.Services;

internal static class BackupValidator
{
    private static readonly string[] RequiredFiles =
    {
        "Against_Rome.exe",
        "SYSTEM/cl_script.ini",
        "SYSTEM/cl_epara.ini",
        "SYSTEM/ress.ini",
        "SYSTEM/DATA_MP/DEFAULTS/objdef.dau",
        "SYSTEM/CLMK/icon.ini",
        "SYSTEM/CLAK/cl_scint.ini",
        "SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci"
    };

    internal static List<string> FindMissing(IReadOnlyDictionary<string, byte[]> files)
    {
        var missing = RequiredFiles.Where(key => !files.ContainsKey(key)).ToList();
        bool hasTeamDat = files.Keys.Any(key =>
            key.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) &&
            key.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase));
        if (!hasTeamDat) missing.Add("MAPS/.../team.dat");
        return missing;
    }

    internal static void Validate(IReadOnlyDictionary<string, byte[]> files, ILogger logger)
    {
        List<string> missing = FindMissing(files);
        if (missing.Count == 0) return;

        string message = Loc.Get("SvcLogBackupIncomplete") + "\r\n" + string.Join("\r\n", missing);
        logger.Log(message);
        throw new InvalidDataException(message);
    }
}
