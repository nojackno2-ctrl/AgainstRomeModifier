namespace AgainstRomeModifier.Maps;

/// <summary>
/// DATA filenames opened by Against_Rome.exe 0x48e960 (fingerprint and call sites
/// documented in docs/reverse-engineering/map-load-pipeline.md).
/// Presence alone does not establish payload validity or successful game loading.
/// </summary>
public static class NativeMapLoadContract
{
    public static IReadOnlyList<string> RequiredDataFiles { get; } = Array.AsReadOnly(new[]
    {
        "light.dat", "gametime.dat", "rain.dat", "hagel.dat", "snow.dat", "flash.dat",
        "objects.dat", "position.dat", "anim.dat", "gfxtype.dat", "action.dat", "objdata.dat",
        "hirarchy.dat", "formatio.dat", "lager.dat", "engine.dat", "fow.dat", "fowreq.dat",
        "way.dat", "particle.dat", "explos.dat", "hitex.dat", "stat.dat", "biglager.dat"
    });

    public static IReadOnlyList<string> FindMissingDataFiles(string mapDirectory)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(mapDirectory);
        string dataDirectory = Path.Combine(mapDirectory, "DATA");
        return RequiredDataFiles.Where(file => !File.Exists(Path.Combine(dataDirectory, file))).ToArray();
    }
}
