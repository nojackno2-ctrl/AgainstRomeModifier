using System.Globalization;

namespace AgainstRomeModifier.Core.Patches;

public sealed record ClEparaOptions(IReadOnlyDictionary<string, double> GeneralSkills, byte[]? OriginalBytes = null);

public static class ClEparaPatcher {
    public static byte[] GetPatchedBytes(byte[] original, ClEparaOptions options) {
        ArgumentNullException.ThrowIfNull(original);
        ArgumentNullException.ThrowIfNull(options);
        var (text, lineEnding, lines) = PatchText.Read(original);
        for (int i = 0; i < lines.Length; i++) {
            string trimmed = lines[i].Trim();
            if (!trimmed.StartsWith("[", StringComparison.Ordinal) || !trimmed.EndsWith(']')) continue;
            string key = trimmed[1..^1].Trim();
            if (!options.GeneralSkills.TryGetValue(key, out double value)) continue;
            int valueIndex = i + 1;
            while (valueIndex < lines.Length && (string.IsNullOrWhiteSpace(lines[valueIndex]) || lines[valueIndex].TrimStart().StartsWith(';'))) valueIndex++;
            if (valueIndex < lines.Length) lines[valueIndex] = value.ToString("0.##", CultureInfo.InvariantCulture);
        }
        return PatchText.Write(text, lineEnding, lines, options.OriginalBytes ?? original);
    }
}
