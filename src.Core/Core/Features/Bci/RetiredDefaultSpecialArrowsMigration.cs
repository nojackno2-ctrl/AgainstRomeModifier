using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features.Bci;

/// <summary>
/// Restore-only cleanup for the retired active-weapon-index experiment.
/// This migration must never write the former patched value 1.
/// </summary>
internal static class RetiredDefaultSpecialArrowsMigration
{
    private const int NormalWeapon = 0;
    private const int RetiredSpecialWeapon = 1;
    private const string RelativePath = @"SYSTEM\CLAK\SCRIPT\ak_krieger.bci";

    internal static void RestoreIfPresent(
        string gamePath,
        EndlessAiOrchestrator orchestrator,
        ILogger logger)
    {
        string path = Path.Combine(gamePath, RelativePath);
        if (!File.Exists(path)) return;

        BciScriptFile script = orchestrator.GetScriptFile(path);
        byte[] data = script.DecompressedBytes;
        List<int> originals = BciPattern.FindAllBciWordPatternSites(data, Signature(NormalWeapon));
        List<int> retired = BciPattern.FindAllBciWordPatternSites(data, Signature(RetiredSpecialWeapon));

        // Restore only the exact retired output. Original and unknown files are untouched.
        if (originals.Count != 0 || retired.Count != 1) return;

        int literalOffset = retired[0] + (7 * sizeof(int));
        BciPattern.WriteBciInt32(data, literalOffset, RetiredSpecialWeapon, NormalWeapon,
            "retired default-special-arrows active weapon restore");
        script.UpdateDecompressedBytes(data);
        logger.Log(Loc.Get("SvcLogRetiredSpecialArrowsRestored"));
    }

    internal static bool TryDetectLegacy(string gamePath, out bool patched)
    {
        patched = false;
        string path = Path.Combine(gamePath, RelativePath);
        if (!File.Exists(path)) return false;

        byte[] data = GameLZSS.DecompressPfil(File.ReadAllBytes(path));
        int originals = BciPattern.FindAllBciWordPatternSites(data, Signature(NormalWeapon)).Count;
        int retired = BciPattern.FindAllBciWordPatternSites(data, Signature(RetiredSpecialWeapon)).Count;
        if (originals + retired != 1) return false;
        patched = retired == 1;
        return true;
    }

    private static int?[] Signature(int weapon) => new int?[]
    {
        66, 1000001, 87, 130,
        66, 1,
        66, weapon,
        81, 10, 81, 98,
        128, 106, 73, -4, 86
    };
}
