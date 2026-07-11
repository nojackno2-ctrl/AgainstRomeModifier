using System.Text.RegularExpressions;

namespace AgainstRomeModifier.Core.Patches;

public readonly record struct ClScintOptions(bool SpellEnhancement, byte[]? OriginalBytes = null);

public static class ClScintPatcher {
    private static readonly Regex SpellODef = new(@"^(SpellODef\d*)\s*=\s*(KEL)\s*,\s*(Spell3)\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);

    public static byte[] GetPatchedBytes(byte[] original, ClScintOptions options) {
        ArgumentNullException.ThrowIfNull(original);
        byte[] canonical = options.OriginalBytes ?? original;
        var originalDefs = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        var (_, _, canonicalLines) = PatchText.Read(canonical);
        foreach (string line in canonicalLines) {
            Match match = SpellODef.Match(line);
            if (match.Success) originalDefs[match.Groups[1].Value.Trim()] = match.Groups[4].Value.Trim();
        }
        var (text, lineEnding, lines) = PatchText.Read(original);
        for (int i = 0; i < lines.Length; i++) {
            Match match = SpellODef.Match(lines[i]);
            if (!match.Success) continue;
            string key = match.Groups[1].Value.Trim();
            string value = originalDefs.TryGetValue(key, out string? saved) ? saved : match.Groups[4].Value.Trim();
            if (options.SpellEnhancement) value = key == "SpellODef" ? "KEL_INF01" : key == "SpellODef2" ? "KEL_INF02" : value;
            lines[i] = string.Format("{0,-10}={1}, {2}, {3,-12}{4}", key, match.Groups[2].Value.Trim(), match.Groups[3].Value.Trim(), value, match.Groups[5].Value);
        }
        return PatchText.Write(text, lineEnding, lines, canonical);
    }
}
