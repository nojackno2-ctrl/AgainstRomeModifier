using System.Text;

namespace AgainstRomeModifier.Core.Services;

internal enum EndlessSaveAiRepairResult
{
    Changed,
    AlreadyRepaired
}

/// <summary>
/// Opt-in migration for the ak_level BCI embedded in an endless save's scr.dat.
/// Callers must create a full save backup before invoking this service.
/// </summary>
internal sealed class EndlessSaveAiRepairService
{
    private static readonly byte[] EndlessPathPrefix = Encoding.ASCII.GetBytes("MAPS/ENDL_");
    private static readonly byte[] AkLevelPathSuffix = Encoding.ASCII.GetBytes("/SCRIPT/ak_level");
    private static readonly byte[] BciMagic = Encoding.ASCII.GetBytes("BCI0");

    internal EndlessSaveAiRepairResult Repair(string gamePath, string saveFolder)
    {
        if (!SaveBackupService.IsSimpleName(saveFolder))
            throw new ArgumentException("Invalid save folder name.", nameof(saveFolder));

        string savesRoot = Path.GetFullPath(Path.Combine(gamePath, "SAVE"));
        string saveRoot = Path.GetFullPath(Path.Combine(savesRoot, saveFolder));
        string rootPrefix = savesRoot.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar) + Path.DirectorySeparatorChar;
        if (!saveRoot.StartsWith(rootPrefix, StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException("Save path escapes the game SAVE directory.");

        string scrPath = Path.Combine(saveRoot, "CLAK", "scr.dat");
        if (!File.Exists(scrPath))
            throw new FileNotFoundException("The selected save does not contain CLAK/scr.dat.", scrPath);

        byte[] raw = File.ReadAllBytes(scrPath);
        if (raw.Length < 64 || raw[0] != (byte)'P' || raw[1] != (byte)'F' || raw[2] != (byte)'I' || raw[3] != (byte)'L')
            throw new InvalidDataException("The selected scr.dat is not a PFIL save payload.");

        byte[] decompressed = GameLZSS.DecompressPfil(raw);
        int bciOffset = FindUniqueEndlessAkLevelBci(decompressed);
        if (bciOffset < 0)
            throw new InvalidDataException("Could not uniquely locate the endless ak_level BCI embedded in this save.");

        byte[] bciTail = new byte[decompressed.Length - bciOffset];
        Buffer.BlockCopy(decompressed, bciOffset, bciTail, 0, bciTail.Length);

        IEndlessPatch[] patches =
        {
            new P6_LoopDelayPatch(),
            new P8_ReinforcementUnitThresholdPatch(),
            new P9_RetreatQuotaPatch()
        };
        foreach (IEndlessPatch patch in patches)
        {
            if (patch.Detect(bciTail) == PatchState.Unknown)
                throw new InvalidDataException($"The save's embedded ak_level {patch.Id} state is unknown and cannot be migrated safely.");
        }

        bool changed = false;
        foreach (IEndlessPatch patch in patches)
            changed |= patch.Apply(ref bciTail, enabled: true);
        if (!changed)
            return EndlessSaveAiRepairResult.AlreadyRepaired;

        Buffer.BlockCopy(bciTail, 0, decompressed, bciOffset, bciTail.Length);
        byte[] repairedRaw = GameLZSS.CompressPfil(decompressed, raw);
        byte[] verified = GameLZSS.DecompressPfil(repairedRaw);
        if (!verified.AsSpan().SequenceEqual(decompressed))
            throw new InvalidDataException("Recompressed scr.dat failed byte-exact verification.");

        SafeFileWriter.WriteAllBytes(scrPath, repairedRaw);
        return EndlessSaveAiRepairResult.Changed;
    }

    private static int FindUniqueEndlessAkLevelBci(byte[] data)
    {
        int foundBci = -1;
        for (int search = 0; ; )
        {
            int prefix = IndexOf(data, EndlessPathPrefix, search);
            if (prefix < 0) break;
            search = prefix + 1;

            int digits = prefix + EndlessPathPrefix.Length;
            if (digits + 3 + AkLevelPathSuffix.Length > data.Length ||
                !IsAsciiDigit(data[digits]) || !IsAsciiDigit(data[digits + 1]) || !IsAsciiDigit(data[digits + 2]) ||
                !MatchesAt(data, digits + 3, AkLevelPathSuffix))
                continue;

            int pathEnd = digits + 3 + AkLevelPathSuffix.Length;
            int bci = IndexOf(data, BciMagic, pathEnd);
            if (bci < 0 || bci - pathEnd > 16) continue;
            if (foundBci >= 0) return -1;
            foundBci = bci;
        }
        return foundBci;
    }

    private static bool IsAsciiDigit(byte value) => value >= (byte)'0' && value <= (byte)'9';

    private static int IndexOf(byte[] data, byte[] pattern, int start)
    {
        for (int i = Math.Max(0, start); i <= data.Length - pattern.Length; i++)
            if (MatchesAt(data, i, pattern)) return i;
        return -1;
    }

    private static bool MatchesAt(byte[] data, int offset, byte[] pattern)
    {
        if (offset < 0 || offset + pattern.Length > data.Length) return false;
        for (int i = 0; i < pattern.Length; i++)
            if (data[offset + i] != pattern[i]) return false;
        return true;
    }
}
