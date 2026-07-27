namespace AgainstRomeModifier.Core.Patches;

public readonly record struct RessOptions(bool FreeProduction, bool FreeUpgrades, bool NoSpellCost);

public static class RessPatcher {
    public static byte[] GetPatchedBytes(byte[] original, RessOptions options) {
        ArgumentNullException.ThrowIfNull(original);
        var (text, lineEnding, lines) = PatchText.Read(original);
        var output = new List<string>(lines.Length);
        bool inObjres = false, inVolkres = false;

        foreach (string line in lines) {
            string stripped = line.Trim();
            if (stripped.StartsWith('[')) {
                inObjres = stripped == "[objres]";
                inVolkres = stripped == "[volkres]";
                output.Add(line);
                continue;
            }
            if (inObjres && line.Contains(',', StringComparison.Ordinal)) {
                string[] cols = PatchText.ParseCsvLine(line);
                if (cols.Length == 0) { output.Add(line); continue; }
                string name = cols[0].Trim();
                if (name.StartsWith("Bau", StringComparison.Ordinal)) {
                    if (cols.Length < 10) { output.Add(line); continue; }
                    output.Add(PatchText.ToCsvString(Rewrite(cols, (i, _) =>
                        options.FreeUpgrades && i >= (int)RessIndex.BauUpgradeCostStart && i <= (int)RessIndex.BauUpgradeCostEnd ||
                        options.FreeProduction && i >= (int)RessIndex.BauBuildCostStart && i <= (int)RessIndex.BauBuildCostEnd)));
                } else if (name.StartsWith("Fig", StringComparison.Ordinal)) {
                    if (cols.Length < 29 || name.Equals("FigTiePac00_Packpferd", StringComparison.OrdinalIgnoreCase)) { output.Add(line); continue; }
                    bool siege = name.Contains("Art", StringComparison.Ordinal) || name.Contains("Bar", StringComparison.Ordinal) || name.Contains("Fal", StringComparison.Ordinal);
                    bool priest = name.Contains("Pri", StringComparison.Ordinal) || name.Contains("Dru", StringComparison.Ordinal);
                    output.Add(PatchText.ToCsvString(Rewrite(cols, (i, _) =>
                        options.FreeProduction && i >= (int)RessIndex.FigProdCostStart && i <= (int)RessIndex.FigProdCostEnd ||
                        options.FreeProduction && siege && i >= (int)RessIndex.FigSiegeBuildCostStart && i <= (int)RessIndex.FigSiegeBuildCostEnd ||
                        options.NoSpellCost && priest && i >= (int)RessIndex.FigPriestSpellCostStart && i <= (int)RessIndex.FigPriestSpellCostEnd)));
                } else output.Add(line);
            } else if (inVolkres && line.Contains(',', StringComparison.Ordinal)) {
                string[] cols = PatchText.ParseCsvLine(line);
                if (cols.Length < 3) { output.Add(line); continue; }
                output.Add(PatchText.ToCsvString(Rewrite(cols, (i, _) => options.FreeUpgrades && (
                    i == (int)VolkresIndex.ResearchUpgradeWood1 || i == (int)VolkresIndex.ResearchUpgradeGold1 ||
                    i == (int)VolkresIndex.ResearchUpgradeWood2 || i == (int)VolkresIndex.ResearchUpgradeGold2 ||
                    i >= (int)VolkresIndex.TechCostStart && i <= (int)VolkresIndex.TechCostEnd && i % 2 == 0 ||
                    i >= (int)VolkresIndex.UnitUpgradeStart && i <= (int)VolkresIndex.UnitUpgradeEnd))));
            } else output.Add(line);
        }
        return PatchText.Write(text, lineEnding, output, original);
    }

    private static IEnumerable<string> Rewrite(string[] columns, Func<int, string, bool> zero) {
        for (int i = 0; i < columns.Length; i++) {
            string value = columns[i];
            yield return i > 0 && !string.IsNullOrEmpty(value.Trim()) && zero(i, value) ? "0" : value;
        }
    }
}
