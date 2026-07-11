namespace AgainstRomeModifier.Core.Patches;

public readonly record struct TeamDatOptions(bool MaxPopulation, int PopulationLimit = 1600);

public static class TeamDatPatcher {
    public static byte[] GetPatchedBytes(byte[] original, TeamDatOptions options) {
        ArgumentNullException.ThrowIfNull(original);
        if (!options.MaxPopulation) return original.ToArray();
        var (text, lineEnding, lines) = PatchText.Read(original);
        bool inTeamData = false;
        for (int i = 0; i < lines.Length; i++) {
            string stripped = lines[i].Trim();
            if (stripped.StartsWith("[", StringComparison.Ordinal)) { inTeamData = stripped == "[teamdata]"; continue; }
            if (!inTeamData || !lines[i].Contains(',', StringComparison.Ordinal)) continue;
            string[] cols = PatchText.ParseCsvLine(lines[i]);
            if (cols.Length >= 5 && int.TryParse(cols[4].Trim(), out int value) && value > 0) {
                cols[4] = options.PopulationLimit.ToString(System.Globalization.CultureInfo.InvariantCulture);
                lines[i] = PatchText.ToCsvString(cols);
            }
        }
        return PatchText.Write(text, lineEnding, lines, original);
    }
}
