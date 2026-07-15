using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features.Install;

internal sealed class InstallFeatureDetector
{
    private readonly ILogger _logger;

    internal InstallFeatureDetector(ILogger logger) => _logger = logger;

    internal void Detect(string gamePath, PatchProfile profile)
    {
        try
        {
            new LanguagePackFeature(_logger).TryGetLanguageOverlayState(gamePath, out bool languageOverlayEnabled);
            profile.ToEnglish = languageOverlayEnabled;
        }
        catch (Exception ex)
        {
            _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "ToEng overlay", ex.Message));
        }

        try
        {
            profile.DgVoodoo = new DgVoodooFeature(_logger).IsInstalled(gamePath);
        }
        catch (Exception ex)
        {
            _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "dgVoodoo2", ex.Message));
        }
    }
}
