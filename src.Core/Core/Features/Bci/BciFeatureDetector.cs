using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features.Bci;

internal sealed class BciFeatureDetector
{
    private readonly ILogger _logger;

    internal BciFeatureDetector(ILogger logger) => _logger = logger;

    internal void Detect(string gamePath, PatchProfile profile)
    {
        try
        {
            var orchestrator = new EndlessAiOrchestrator();
            foreach (var module in orchestrator.UserModules)
            {
                PatchState state = orchestrator.DetectModule(gamePath, module);
                profile.EndlessAiModules[module.Id] = module.Id == "Core"
                    ? state is PatchState.Ultimate or PatchState.Legacy
                    : state == PatchState.Ultimate;
            }
        }
        catch (Exception ex)
        {
            _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "Endless AI", ex.Message));
        }

        try
        {
            FoodHealingFeature.TryDetect(gamePath, out bool foodHealing);
            profile.FoodHealing10x = foodHealing;
        }
        catch (Exception ex)
        {
            _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "food healing", ex.Message));
        }
    }
}
