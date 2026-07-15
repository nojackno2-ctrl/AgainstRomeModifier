using System.Text.RegularExpressions;
using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features.Ini;

internal sealed class RessFeatureDetector
{
    private readonly ILogger _logger;

    internal RessFeatureDetector(ILogger logger) => _logger = logger;

    internal void Detect(string gamePath, PatchProfile profile)
    {
        string path = Path.Combine(gamePath, @"SYSTEM\ress.ini");
        if (!File.Exists(path)) return;

        try
        {
            (string text, _, _) = PatchText.Read(File.ReadAllBytes(path));

            Match production = Regex.Match(text, @"^FigRomInf00_Lanze_Schild\s*,.*$", RegexOptions.Multiline);
            if (production.Success)
            {
                string[] columns = PatchText.ParseCsvLine(production.Value);
                profile.FreeProduction = AllZero(columns, (int)RessIndex.FigProdCostStart, (int)RessIndex.FigProdCostEnd);
            }

            Match upgrade = Regex.Match(text, @"^.*Ger_Kampf.*$", RegexOptions.Multiline);
            if (upgrade.Success)
            {
                string[] columns = PatchText.ParseCsvLine(upgrade.Value);
                profile.FreeUpgrade = AllZero(columns, (int)VolkresIndex.UnitUpgradeStart, (int)VolkresIndex.UnitUpgradeEnd);
            }

            Match priest = Regex.Match(text, @"^FigGerPri00_Priester\s*,.*", RegexOptions.Multiline);
            if (priest.Success)
            {
                string[] columns = PatchText.ParseCsvLine(priest.Value);
                profile.NoSpellCost = AllZero(columns, (int)RessIndex.FigPriestSpellCostStart, (int)RessIndex.FigPriestSpellCostEnd);
            }
        }
        catch (Exception ex)
        {
            _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "ress.ini", ex.Message));
        }
    }

    private static bool AllZero(string[] columns, int first, int last) =>
        Enumerable.Range(first, last - first + 1)
            .All(index => index < columns.Length && columns[index].Trim() == "0");
}
