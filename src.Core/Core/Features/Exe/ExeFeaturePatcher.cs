using System.Collections.Generic;
using System.IO;
using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features.Exe;

internal static class ExeFeaturePatcher
{
    internal static bool Apply(byte[] exeBytes, bool focusLoss, bool villageBuildRange, bool noSpellAltar,
        bool romanEndless, int gameSpeed, ILogger logger)
    {
        bool modified = false;
        ExePatchState state = ExePatchModel.GetExePatchState(exeBytes);
        if (state == ExePatchState.Unknown)
            throw new InvalidDataException("Against_Rome.exe 版本或位元組特徵不符合預期，已停止相容性補丁以避免覆蓋未知版本。");

        IReadOnlyList<ExeWriteOp> focusOps = ExePatchModel.PlanFocus(focusLoss, state);
        if (focusOps.Count > 0) { ExePatchModel.Apply(exeBytes, focusOps); modified = true; }
        logger.Log(focusLoss ? Loc.Get("SvcLogFocusApplied") : Loc.Get("SvcLogFocusRestored"));

        RestoreLegacyVillageRange(exeBytes, logger, ref modified);
        if (villageBuildRange && ExePatchModel.GetVillageBuildRangePatchState(exeBytes) != ExeVillageRangePatchState.Original)
            throw new InvalidOperationException("遊戲主程式不支援村莊建造半徑擴張補丁（特徵碼不符）。");
        ApplyVillageSetter(exeBytes, villageBuildRange, logger, ref modified);
        ApplySpellAltar(exeBytes, noSpellAltar, logger, ref modified);
        modified |= ApplyRomanEndless(exeBytes, romanEndless, logger);
        ApplyGameSpeed(exeBytes, gameSpeed, logger, ref modified);
        return modified;
    }

    internal static bool ApplyRomanEndless(byte[] bytes, bool enabled, ILogger logger)
    {
        ExeRomanEndlessPatchState state = ExePatchModel.GetRomanEndlessPatchState(bytes);
        if (state == ExeRomanEndlessPatchState.Unknown)
        {
            logger.Log(Loc.Get("SvcLogRomanEndlessExeUnknown"));
            return false;
        }

        IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanRomanEndless(enabled, state);
        if (ops.Count == 0) return false;
        ExePatchModel.Apply(bytes, ops);
        logger.Log(enabled ? Loc.Get("SvcLogRomanEndlessExeApplied") : Loc.Get("SvcLogRomanEndlessExeRestored"));
        return true;
    }

    internal static bool ApplyCiviProduce20(byte[] bytes, bool enabled, ILogger logger)
    {
        ExeCiviProduce20PatchState state = ExePatchModel.GetCiviProduce20PatchState(bytes);
        if (state == ExeCiviProduce20PatchState.Unknown)
        {
            if (enabled)
                throw new InvalidDataException("Against_Rome.exe 版本或住宅生產按鈕特徵碼不符合預期，已停止套用「住宅一次生產 20」補丁。");
            return false;
        }

        IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanCiviProduce20(enabled, state);
        if (ops.Count == 0) return false;
        ExePatchModel.Apply(bytes, ops);
        logger.Log(enabled ? Loc.Get("SvcLogCiviProduce20Applied") : Loc.Get("SvcLogCiviProduce20Restored"));
        return true;
    }

    internal static bool ApplyUnitRecruit20(byte[] bytes, bool enabled, ILogger logger)
    {
        ExeUnitRecruit20PatchState state = ExePatchModel.GetUnitRecruit20PatchState(bytes);
        if (state == ExeUnitRecruit20PatchState.Unknown)
        {
            if (enabled)
                throw new InvalidDataException("Against_Rome.exe 版本或招募面板特徵碼不符合預期，已停止套用「招募一次到上限」補丁。");
            return false;
        }

        IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanUnitRecruit20(enabled, state);
        if (ops.Count == 0) return false;
        ExePatchModel.Apply(bytes, ops);
        logger.Log(enabled ? Loc.Get("SvcLogUnitRecruit20Applied") : Loc.Get("SvcLogUnitRecruit20Restored"));
        return true;
    }

    internal static bool ApplyIdleSelect999(byte[] bytes, bool enabled, ILogger logger)
    {
        ExeIdleSelect999PatchState state = ExePatchModel.GetIdleSelect999PatchState(bytes);
        if (state == ExeIdleSelect999PatchState.Unknown)
        {
            if (enabled)
                throw new InvalidDataException(Loc.Get("SvcLogIdleSelect999Unknown"));
            return false;
        }

        IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanIdleSelect999(enabled, state);
        if (ops.Count == 0) return false;
        ExePatchModel.Apply(bytes, ops);
        logger.Log(Loc.Get(enabled ? "SvcLogIdleSelect999Applied" : "SvcLogIdleSelect999Restored"));
        return true;
    }

    internal static bool ApplyNativeWidescreen(byte[] bytes, bool enabled, ILogger logger)
    {
        ExeNativeWidescreenPatchState state = ExePatchModel.GetNativeWidescreenPatchState(bytes);
        if (state == ExeNativeWidescreenPatchState.Unknown)
        {
            if (enabled)
                throw new InvalidDataException(Loc.Get("SvcLogNativeWidescreenUnknown"));
            return false;
        }

        IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanNativeWidescreen(enabled, state);
        if (ops.Count == 0) return false;
        ExePatchModel.Apply(bytes, ops);
        logger.Log(Loc.Get(enabled ? "SvcLogNativeWidescreenApplied" : "SvcLogNativeWidescreenRestored"));
        return true;
    }

    internal static bool ApplyCameraZoomOut(byte[] bytes, bool enabled, ILogger logger)
    {
        ExeCameraZoomOutPatchState state = ExePatchModel.GetCameraZoomOutPatchState(bytes);
        if (state == ExeCameraZoomOutPatchState.Unknown)
        {
            if (enabled)
                throw new InvalidDataException(Loc.Get("SvcLogCameraZoomOutUnknown"));
            return false;
        }

        IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanCameraZoomOut(enabled, state);
        if (ops.Count == 0) return false;
        ExePatchModel.Apply(bytes, ops);
        logger.Log(Loc.Get(enabled ? "SvcLogCameraZoomOutApplied" : "SvcLogCameraZoomOutRestored"));
        return true;
    }

    private static void RestoreLegacyVillageRange(byte[] bytes, ILogger logger, ref bool modified)
    {
        ExeVillageRangePatchState state = ExePatchModel.GetVillageBuildRangePatchState(bytes);
        if (state == ExeVillageRangePatchState.Unknown) { logger.Log(Loc.Get("SvcLogVillageLegacyRestoreUnknown")); return; }
        IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanVillageRangeRestore(state);
        if (ops.Count == 0) return;
        ExePatchModel.Apply(bytes, ops); modified = true; logger.Log(Loc.Get("SvcLogVillageLegacyRemoved"));
    }

    private static void ApplyVillageSetter(byte[] bytes, bool enabled, ILogger logger, ref bool modified)
    {
        ExeVillageSetterPatchState state = ExePatchModel.GetVillageSetterPatchState(bytes);
        if (state == ExeVillageSetterPatchState.Unknown)
        {
            if (enabled) throw new InvalidOperationException("無法套用村莊建造半徑補丁：主程式特徵碼不符合。");
            logger.Log(Loc.Get("SvcLogVillageApplyUnknown")); return;
        }
        IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanVillageSetter(enabled, state);
        if (ops.Count > 0) { ExePatchModel.Apply(bytes, ops); modified = true; }
        if (enabled) logger.Log(Loc.Get("SvcLogVillageApplied"));
        else if (ops.Count > 0) logger.Log(Loc.Get("SvcLogVillageRestored"));
    }

    private static void ApplySpellAltar(byte[] bytes, bool enabled, ILogger logger, ref bool modified)
    {
        ExeSpellAltarPatchState state = ExePatchModel.GetSpellAltarPatchState(bytes);
        if (state == ExeSpellAltarPatchState.Unknown)
            throw new InvalidDataException("Against_Rome.exe 版本或法術祭壇特徵碼不符合預期，已停止套用法術祭壇補丁。");
        IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanSpellAltar(enabled, state);
        if (ops.Count > 0) { ExePatchModel.Apply(bytes, ops); modified = true; logger.Log(enabled ? Loc.Get("SvcLogAltarApplied") : Loc.Get("SvcLogAltarRestored")); }
    }

    private static void ApplyGameSpeed(byte[] bytes, int multiplier, ILogger logger, ref bool modified)
    {
        int current = ExePatchModel.GetGameSpeedMultiplier(bytes);
        if (current == 0) { logger.Log(Loc.Get("SvcLogGameSpeedUnknown")); return; }
        IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanGameSpeed(multiplier, current);
        if (ops.Count > 0) { ExePatchModel.Apply(bytes, ops); modified = true; }
        logger.Log(multiplier > 1 ? string.Format(Loc.Get("SvcLogGameSpeedApplied"), multiplier) : Loc.Get("SvcLogGameSpeedOriginal"));
    }
}
