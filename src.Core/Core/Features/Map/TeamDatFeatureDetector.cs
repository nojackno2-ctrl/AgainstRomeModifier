using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features.Map;

internal readonly record struct TeamDatFeatureDetection(bool MaxPopulation, bool RomanPlayer);

internal sealed class TeamDatFeatureDetector
{
    private readonly ILogger _logger;

    internal TeamDatFeatureDetector(ILogger logger) => _logger = logger;

    internal TeamDatFeatureDetection Detect(string gamePath, BackupManager backupManager)
    {
        bool maxPopulation = false;
        bool romanPlayer = false;
        try
        {
            foreach ((string key, byte[] original) in backupManager.BackupFiles)
            {
                if (!key.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) ||
                    !key.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase)) continue;

                string path = Path.Combine(gamePath, key.Replace('/', '\\'));
                if (!File.Exists(path)) continue;

                byte[] current = File.ReadAllBytes(path);
                bool isEndless = key.StartsWith("MAPS/ENDL_", StringComparison.OrdinalIgnoreCase);
                if (TryDetectOptions(original, current, isEndless, out bool fileMaxPopulation, out bool fileRomanPlayer))
                {
                    maxPopulation |= fileMaxPopulation;
                    romanPlayer |= fileRomanPlayer;
                }
                else
                {
                    // Preserve the legacy conservative behavior for unknown team.dat modifications.
                    maxPopulation = true;
                    _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), key, "unknown team.dat state"));
                }
            }
        }
        catch (Exception ex)
        {
            _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "team.dat", ex.Message));
        }

        return new(maxPopulation, romanPlayer);
    }

    internal static bool TryDetectOptions(byte[] original, byte[] current, bool isEndless,
        out bool maxPopulation, out bool romanPlayer)
    {
        (bool MaxPopulation, bool RomanPlayer)[] combinations =
        {
            (false, false),
            (true, false),
            (false, true),
            (true, true),
        };

        foreach ((bool candidateMaxPopulation, bool candidateRomanPlayer) in combinations)
        {
            bool effectiveRoman = candidateRomanPlayer && isEndless;
            byte[] candidate = TeamDatPatcher.GetPatchedBytes(original,
                new TeamDatOptions(candidateMaxPopulation, RomanPlayer: effectiveRoman));
            if (!current.SequenceEqual(candidate)) continue;

            maxPopulation = candidateMaxPopulation;
            romanPlayer = effectiveRoman;
            return true;
        }

        maxPopulation = false;
        romanPlayer = false;
        return false;
    }
}
