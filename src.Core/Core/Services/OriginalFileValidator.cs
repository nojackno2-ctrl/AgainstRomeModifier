using System;
using System.Globalization;
using AgainstRomeModifier.Core.Patches;

namespace AgainstRomeModifier.Core.Services;

internal static class OriginalFileValidator
{
    internal static bool IsExeOriginal(byte[] bytes)
    {
        try
        {
            return ExePatchModel.GetExePatchState(bytes) == ExePatchState.Original &&
                   ExePatchModel.GetSpellAltarPatchState(bytes) == ExeSpellAltarPatchState.Original &&
                   ExePatchModel.GetVillageBuildRangePatchState(bytes) == ExeVillageRangePatchState.Original &&
                   ExePatchModel.GetVillageSetterPatchState(bytes) == ExeVillageSetterPatchState.Original &&
                   ExePatchModel.GetRomanEndlessPatchState(bytes) == ExeRomanEndlessPatchState.Original;
        }
        catch
        {
            return false;
        }
    }

    internal static bool IsObjdefOriginal(byte[] bytes)
    {
        try
        {
            byte[] decompressed = GameLZSS.DecompressPfil(bytes);
            string text = PatchText.GameEncoding.GetString(decompressed);
            foreach (string line in text.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None))
            {
                if (line.Length < 100) continue;
                string[] columns = PatchText.ParseCsvLine(line);
                if (columns.Length < 192 || columns[(int)ObjdefIndex.Name].Trim() != "FigRomAnf00_Anfuehrer")
                    continue;

                return double.TryParse(columns[(int)ObjdefIndex.Hp].Trim(), NumberStyles.Any,
                           CultureInfo.InvariantCulture, out double hp) &&
                       Math.Abs(hp - 400) <= 0.1;
            }
            return false;
        }
        catch
        {
            return false;
        }
    }

    internal static bool IsPartgeoOriginal(byte[] bytes)
    {
        try
        {
            byte[] decompressed = GameLZSS.DecompressPfil(bytes);
            string text = PatchText.GameEncoding.GetString(decompressed);
            foreach (string line in text.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None))
            {
                string[] columns = PatchText.ParseCsvLine(line);
                if (columns.Length <= PartgeoPatcher.YsubColumn || columns[2].Trim() != "Pfeil00")
                    continue;
                return long.TryParse(columns[PartgeoPatcher.YsubColumn].Trim(), NumberStyles.Integer,
                           CultureInfo.InvariantCulture, out long ysub) && ysub == 5832704;
            }
            return false;
        }
        catch
        {
            return false;
        }
    }
}
