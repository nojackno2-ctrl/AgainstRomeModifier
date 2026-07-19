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
                bool migrateLegacyWhenSelected = module.Id is "Core" or "M6";
                bool selected = state == PatchState.Ultimate ||
                    (migrateLegacyWhenSelected && state == PatchState.Legacy);
                if (module.Id == "M6")
                {
                    profile.RomanReinforcementGarrison = selected;
                    profile.Set(FeatureKeys.EndlessAiM6, selected);
                }
                else if (module.Id == "VillageGarrisonQuota3x")
                {
                    // Ultimate = installed at the orchestrator default; Legacy =
                    // installed at another supported multiplier. Read the actual
                    // literal back so the UI shows what is really installed.
                    profile.VillageGarrisonQuotaMultiplier =
                        state == PatchState.Ultimate || state == PatchState.Legacy
                            ? orchestrator.DetectVillageGarrisonQuotaMultiplier(gamePath)
                            : 1;
                }
                else
                {
                    profile.EndlessAiModules[module.Id] = selected;
                }
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
