using AgainstRomeModifier.Core.Features.Bci;
using AgainstRomeModifier.Core.Features.Exe;
using AgainstRomeModifier.Core.Features.Ini;
using AgainstRomeModifier.Core.Features.Install;
using AgainstRomeModifier.Core.Features.Map;
using AgainstRomeModifier.Core.Features.Objdef;
using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features;

internal sealed class FeatureDetector
{
    private readonly ILogger _logger;
    internal FeatureDetector(ILogger logger) => _logger = logger;

    internal PatchProfile Detect(string gamePath, BackupManager backupManager)
    {
        var options = new PatchProfile();
        ExeFeatureDetection exeDetection = new ExeFeatureDetector(_logger).Detect(gamePath, options);
        new ClScriptFeatureDetector(_logger).Detect(gamePath, backupManager, options);
        new RessFeatureDetector(_logger).Detect(gamePath, options);
        new ObjdefFeatureDetector(_logger).Detect(gamePath, backupManager, options);

        TeamDatFeatureDetection teamDetection = new TeamDatFeatureDetector(_logger).Detect(gamePath, backupManager);
        options.MaxPopulation = teamDetection.MaxPopulation;
        if (exeDetection.RomanEndlessState == ExeRomanEndlessPatchState.Unknown)
        {
            options.RomanEndless = teamDetection.RomanPlayer;
            _logger.Log(Loc.Get("SvcLogRomanEndlessExeUnknown"));
        }
        else
        {
            bool exeEnabled = exeDetection.RomanEndlessState == ExeRomanEndlessPatchState.Patched;
            options.RomanEndless = exeEnabled;
            if (exeEnabled != teamDetection.RomanPlayer)
                _logger.Log(Loc.Get("SvcLogRomanEndlessMismatch"));
        }

        new BciFeatureDetector(_logger).Detect(gamePath, options);
        new InstallFeatureDetector(_logger).Detect(gamePath, options);
        return options;
    }
}
