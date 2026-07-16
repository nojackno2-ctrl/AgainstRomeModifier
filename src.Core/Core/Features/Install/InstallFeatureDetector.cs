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
            var dgVoodoo = new DgVoodooFeature(_logger);
            profile.DgVoodoo = dgVoodoo.IsInstalled(gamePath);
            if (profile.DgVoodoo && dgVoodoo.IsCenteredPresentationConfigured(gamePath))
                profile.NativeWidescreen1920x1080 = true;
        }
        catch (Exception ex)
        {
            _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "dgVoodoo2", ex.Message));
        }
    }
}
