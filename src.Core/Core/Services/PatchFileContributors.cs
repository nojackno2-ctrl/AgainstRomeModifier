using AgainstRomeModifier.Core.Features;
using AgainstRomeModifier.Core.Features.Exe;
using AgainstRomeModifier.Core.Features.Ini;
using AgainstRomeModifier.Core.Features.Map;
using AgainstRomeModifier.Core.Features.Objdef;
using AgainstRomeModifier.Core.Patches;

namespace AgainstRomeModifier.Core.Services;

internal interface IPatchFileContributor
{
    void Contribute(PatchFileCompositionContext context);
}

internal sealed class PatchFileCompositionContext
{
    private readonly Dictionary<string, byte[]> _files = new(StringComparer.OrdinalIgnoreCase);

    internal PatchFileCompositionContext(string gamePath, BackupManager backup, PatchProfile profile, ILogger logger)
    {
        GamePath = gamePath;
        Backup = backup;
        Profile = profile;
        Logger = logger;
    }

    internal string GamePath { get; }
    internal BackupManager Backup { get; }
    internal PatchProfile Profile { get; }
    internal ILogger Logger { get; }
    internal IReadOnlyDictionary<string, byte[]> Files => _files;

    internal string Resolve(string relativePath) =>
        Path.Combine(GamePath, relativePath.Replace('/', Path.DirectorySeparatorChar));

    internal void Add(string relativePath, byte[] bytes) => _files[Resolve(relativePath)] = bytes;
}

internal static class PatchFileContributors
{
    internal static IReadOnlyList<IPatchFileContributor> CreateDefault() => new IPatchFileContributor[]
    {
        new ExePatchFileContributor(),
        new ClScriptPatchFileContributor(),
        new EparaPatchFileContributor(),
        new PartgeoPatchFileContributor(),
        new ScintPatchFileContributor(),
        new RessPatchFileContributor(),
        new ObjdefPatchFileContributor(),
        new TeamDatPatchFileContributor(),
    };
}

internal sealed class ExePatchFileContributor : IPatchFileContributor
{
    public void Contribute(PatchFileCompositionContext context)
    {
        string path = context.Resolve("Against_Rome.exe");
        byte[] bytes = File.ReadAllBytes(path);
        PatchProfile profile = context.Profile;
        bool modified = ExeFeaturePatcher.Apply(bytes, profile.FocusLoss, profile.VillageBuildRange,
            profile.NoSpellAltar, profile.RomanEndless, profile.GameSpeed, context.Logger);
        modified |= ExeFeaturePatcher.ApplyCiviProduce20(bytes, profile.CiviProduce20, context.Logger);
        modified |= ExeFeaturePatcher.ApplyUnitRecruit20(bytes, profile.UnitRecruit20, context.Logger);
        // The native 1920x1080 EXE experiment is runtime-rejected: both fullscreen and
        // windowed modes retained a legacy internal viewport. Always migrate it back to
        // the verified stock mode; the profile now controls centered dgVoodoo presentation.
        modified |= ExeFeaturePatcher.ApplyNativeWidescreen(bytes, false, context.Logger);
        if (modified) context.Add("Against_Rome.exe", bytes);
    }
}

internal sealed class ClScriptPatchFileContributor : IPatchFileContributor
{
    public void Contribute(PatchFileCompositionContext context) =>
        context.Add("SYSTEM/cl_script.ini", IniFeaturePatcher.BuildClScript(context.Backup, context.Profile));
}

internal sealed class EparaPatchFileContributor : IPatchFileContributor
{
    public void Contribute(PatchFileCompositionContext context) =>
        context.Add("SYSTEM/cl_epara.ini", EparaPatcher.GetPatchedBytes(
            context.Backup.GetBackupBytes("SYSTEM/cl_epara.ini"), context.Profile.RangedRange3x));
}

internal sealed class PartgeoPatchFileContributor : IPatchFileContributor
{
    private const string Key = "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau";

    public void Contribute(PatchFileCompositionContext context)
    {
        if (context.Backup.HasFile(Key))
        {
            context.Add(Key, PartgeoPatcher.GetPatchedBytes(context.Backup.GetBackupBytes(Key),
                new PartgeoOptions(context.Profile.ProjectileArcHeight)));
        }
        else if (context.Profile.ProjectileArcHeight)
        {
            throw new InvalidDataException("缺少 partgeo.dau 原始備份，無法套用拋射彈道增高。 / partgeo.dau backup missing; cannot apply projectile arc height.");
        }
    }
}

internal sealed class ScintPatchFileContributor : IPatchFileContributor
{
    public void Contribute(PatchFileCompositionContext context) =>
        context.Add("SYSTEM/CLAK/cl_scint.ini", context.Backup.GetBackupBytes("SYSTEM/CLAK/cl_scint.ini"));
}

internal sealed class RessPatchFileContributor : IPatchFileContributor
{
    public void Contribute(PatchFileCompositionContext context) =>
        context.Add("SYSTEM/ress.ini", IniFeaturePatcher.BuildRess(context.Backup,
            context.Profile.FreeProduction, context.Profile.FreeUpgrade, context.Profile.NoSpellCost));
}

internal sealed class ObjdefPatchFileContributor : IPatchFileContributor
{
    public void Contribute(PatchFileCompositionContext context) =>
        context.Add("SYSTEM/DATA_MP/DEFAULTS/objdef.dau",
            ObjdefFeaturePatcher.Build(context.Backup, context.Profile));
}

internal sealed class TeamDatPatchFileContributor : IPatchFileContributor
{
    public void Contribute(PatchFileCompositionContext context)
    {
        PatchProfile profile = context.Profile;
        Dictionary<string, byte[]> files = MaxPopulationFeature.Build(
            context.Backup, profile.MaxPopulation, profile.RomanEndless);
        foreach (string key in files.Keys.Where(key => !File.Exists(context.Resolve(key))).ToList())
            files.Remove(key);

        context.Logger.Log(profile.MaxPopulation
            ? string.Format(Loc.Get("SvcLogTeamDatApplied"), TeamDatPatcher.DefaultPopulationLimit, files.Count)
            : Loc.Get("SvcLogTeamDatRestored"));
        int endlessCount = files.Keys.Count(key => key.StartsWith("MAPS/ENDL_", StringComparison.OrdinalIgnoreCase));
        context.Logger.Log(profile.RomanEndless
            ? string.Format(Loc.Get("SvcLogTeamDatRoman"), endlessCount)
            : Loc.Get("SvcLogTeamDatRomanRestored"));

        foreach ((string relativePath, byte[] bytes) in files)
            context.Add(relativePath, bytes);
    }
}
