using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features.Exe;

internal readonly record struct ExeFeatureDetection(ExeRomanEndlessPatchState RomanEndlessState);

internal sealed class ExeFeatureDetector
{
    private readonly ILogger _logger;

    internal ExeFeatureDetector(ILogger logger) => _logger = logger;

    internal ExeFeatureDetection Detect(string gamePath, PatchProfile profile)
    {
        ExeRomanEndlessPatchState romanEndlessState = ExeRomanEndlessPatchState.Unknown;
        string exePath = Path.Combine(gamePath, "Against_Rome.exe");
        if (!File.Exists(exePath)) return new(romanEndlessState);

        try
        {
            byte[] exeBytes = File.ReadAllBytes(exePath);
            ExePatchState exeState = ExePatchModel.GetExePatchState(exeBytes);
            if (exeState != ExePatchState.Unknown)
                profile.FocusLoss = exeState == ExePatchState.FocusPatched;

            ExeVillageSetterPatchState setterState = ExePatchModel.GetVillageSetterPatchState(exeBytes);
            if (setterState != ExeVillageSetterPatchState.Unknown)
            {
                profile.VillageBuildRange = setterState is
                    ExeVillageSetterPatchState.Legacy2x or
                    ExeVillageSetterPatchState.Legacy2Point5x or
                    ExeVillageSetterPatchState.Legacy3x or
                    ExeVillageSetterPatchState.Legacy5x or
                    ExeVillageSetterPatchState.EntireMap;
            }

            ExeSpellAltarPatchState altarState = ExePatchModel.GetSpellAltarPatchState(exeBytes);
            if (altarState != ExeSpellAltarPatchState.Unknown)
                profile.NoSpellAltar = altarState == ExeSpellAltarPatchState.Patched;

            romanEndlessState = ExePatchModel.GetRomanEndlessPatchState(exeBytes);
            profile.GameSpeed = ExePatchModel.GetGameSpeedMultiplier(exeBytes);
            profile.CiviProduce20 =
                ExePatchModel.GetCiviProduce20PatchState(exeBytes) == ExeCiviProduce20PatchState.Patched;
            profile.UnitRecruit20 =
                ExePatchModel.GetUnitRecruit20PatchState(exeBytes) == ExeUnitRecruit20PatchState.Patched;
            profile.NativeWidescreen1920x1080 =
                ExePatchModel.GetNativeWidescreenPatchState(exeBytes) is
                    ExeNativeWidescreenPatchState.LegacyUnforced or
                    ExeNativeWidescreenPatchState.LegacyForcedStaleUi or
                    ExeNativeWidescreenPatchState.Patched;
        }
        catch (Exception ex)
        {
            _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "Against_Rome.exe", ex.Message));
        }

        return new(romanEndlessState);
    }
}
