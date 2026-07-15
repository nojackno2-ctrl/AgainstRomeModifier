namespace AgainstRomeModifier.Core.Patches;

public readonly record struct TeamDatOptions(
    bool MaxPopulation,
    int PopulationLimit = TeamDatPatcher.DefaultPopulationLimit,
    bool RomanPlayer = false);

public static class TeamDatPatcher {
    /// <summary>「人口上限最大化」功能寫入的預設值；log 與偵測請引用此常數，勿散落魔術數字。</summary>
    public const int DefaultPopulationLimit = 1600;
    public const string RomanFactionToken = "ROM";

    public static byte[] GetPatchedBytes(byte[] original, TeamDatOptions options) {
        ArgumentNullException.ThrowIfNull(original);
        if (!options.MaxPopulation && !options.RomanPlayer) return original.ToArray();
        var (text, lineEnding, lines) = PatchText.Read(original);
        bool inTeamData = false;
        for (int i = 0; i < lines.Length; i++) {
            string stripped = lines[i].Trim();
            if (stripped.StartsWith("[", StringComparison.Ordinal)) { inTeamData = stripped == "[teamdata]"; continue; }
            if (!inTeamData || !lines[i].Contains(',', StringComparison.Ordinal)) continue;
            string[] cols = PatchText.ParseCsvLine(lines[i]);
            bool changed = false;
            if (options.RomanPlayer && cols.Length >= 2 && cols[0].Trim() == "0") {
                cols[1] = RomanFactionToken;
                changed = true;
            }
            if (options.MaxPopulation && cols.Length >= 5 && int.TryParse(cols[4].Trim(), out int value) && value > 0) {
                cols[4] = options.PopulationLimit.ToString(System.Globalization.CultureInfo.InvariantCulture);
                changed = true;
            }
            if (changed) {
                lines[i] = PatchText.ToCsvString(cols);
            }
        }
        return PatchText.Write(text, lineEnding, lines, original);
    }
}
