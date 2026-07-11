using System.Globalization;
using System.Text.RegularExpressions;

namespace AgainstRomeModifier.Core.Patches;

public sealed record ClScriptOptions(
    bool FastCivilianProduction,
    bool InfiniteMorale,
    bool SpellDamage5x,
    bool SpellHealing10x,
    bool SpellResurrection,
    bool GeneralSkills5x,
    double GermanSpellRadiusMultiplier = 1.0,
    double CeltSpellRadiusMultiplier = 1.0,
    double HunSpellRadiusMultiplier = 1.0,
    byte[]? OriginalBytes = null,
    bool SpellRange3x = false);

public static class ClScriptPatcher {
    /// <summary>SpellDamage5x 影響的傷害法術（faction_spell），偵測邏輯（FeatureDetector）共用同一份清單。</summary>
    internal static readonly string[] DamageSpellKeys = { "GER_Spell2", "HUN_Spell0", "HUN_Spell1", "HUN_Spell2", "KEL_Spell0", "KEL_Spell2" };

    private static readonly Regex Radius = new(@"^Radius\s*=\s*([A-Z]{3})\s*,\s*(Spell\d+)\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
    private static readonly Regex Civi = new(@"^CiviDelay\s*=\s*([A-Z]{3})\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
    private static readonly Regex LpIdle = new(@"^LPIncIdle\s*=\s*([A-Z]{3})\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
    private static readonly Regex SpellValue = new(@"^(Value\d*)\s*=\s*([A-Z]{3})\s*,\s*(Spell\d+)\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
    private static readonly Regex AbilityValue = new(@"^(Value\d*)\s*=\s*([A-Z]{3})\s*,\s*(SAbility\d+)\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
    private static readonly Regex MoraleKey = new(@"^(MoralsDecLostMem|MoralsDecFlee|MoralsDecOverPop|MoralsIncIdle)\s*=\s*([A-Z]{3})\s*,", RegexOptions.Compiled);
    private static readonly Regex MoraleLost = new(@"^(MoralsDecLostMem\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
    private static readonly Regex MoraleFlee = new(@"^(MoralsDecFlee\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
    private static readonly Regex MoraleOverPop = new(@"^(MoralsDecOverPop\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
    private static readonly Regex MoraleIdle = new(@"^(MoralsIncIdle\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);

    public static byte[] GetPatchedBytes(byte[] original, ClScriptOptions options) {
        ArgumentNullException.ThrowIfNull(original);
        ArgumentNullException.ThrowIfNull(options);
        byte[] canonical = options.OriginalBytes ?? original;
        var (_, _, originalLines) = PatchText.Read(canonical);
        var originalRadius = ReadValues(originalLines, Radius, m => $"{m.Groups[1].Value.Trim()}_{m.Groups[2].Value.Trim()}", 3);
        var originalSpells = ReadValues(originalLines, SpellValue, m => $"{m.Groups[1].Value.Trim()}_{m.Groups[2].Value.Trim()}_{m.Groups[3].Value.Trim()}", 4);
        var originalAbilities = ReadValues(originalLines, AbilityValue, m => $"{m.Groups[1].Value.Trim()}_{m.Groups[2].Value.Trim()}_{m.Groups[3].Value.Trim()}", 4);
        var (text, lineEnding, lines) = PatchText.Read(original);

        for (int i = 0; i < lines.Length; i++) {
            string line = lines[i];
            string processed = line;
            Match match = Radius.Match(line);
            if (match.Success) {
                string faction = match.Groups[1].Value.Trim();
                string spell = match.Groups[2].Value.Trim();
                double value = GetOriginal(originalRadius, $"{faction}_{spell}", match.Groups[3].Value);
                double multiplier = faction == "GER" ? options.GermanSpellRadiusMultiplier : faction == "KEL" ? options.CeltSpellRadiusMultiplier : faction == "HUN" ? options.HunSpellRadiusMultiplier : 1.0;
                if (options.SpellRange3x) multiplier *= 3.0;
                processed = string.Format("Radius     ={0}, {1}, {2,-10}{3}", match.Groups[1].Value, match.Groups[2].Value, (int)(value * multiplier), match.Groups[4].Value);
            }
            match = SpellValue.Match(line);
            if (match.Success) {
                string key = match.Groups[1].Value.Trim(), faction = match.Groups[2].Value.Trim(), spell = match.Groups[3].Value.Trim();
                double originalValue = GetOriginal(originalSpells, $"{key}_{faction}_{spell}", match.Groups[4].Value);
                int value = (int)originalValue;
                if (options.SpellDamage5x) {
                    if (key == "Value" && DamageSpellKeys.Contains($"{faction}_{spell}")) value = (int)(originalValue * 5);
                }
                if (options.SpellHealing10x) {
                    if (faction == "KEL" && spell == "Spell1" && key == "Value") value = (int)(originalValue * 10);
                }
                if (options.SpellResurrection) {
                    if (faction == "KEL" && spell == "Spell3" && (key == "Value" || key == "Value2")) value = 100;
                }
                processed = string.Format("{0,-10} ={1}, {2}, {3,-10}{4}", key, faction, spell, value, match.Groups[5].Value);
            }
            match = AbilityValue.Match(line);
            if (match.Success) {
                string key = match.Groups[1].Value.Trim(), faction = match.Groups[2].Value.Trim(), ability = match.Groups[3].Value.Trim();
                double value = GetOriginal(originalAbilities, $"{key}_{faction}_{ability}", match.Groups[4].Value);
                if (options.GeneralSkills5x) value *= 5;
                processed = string.Format("{0,-10} ={1}, {2}, {3,-10}{4}", key, faction, ability, (int)value, match.Groups[5].Value);
            }
            match = Civi.Match(line);
            if (match.Success) {
                double value = options.FastCivilianProduction ? 500 : FindFactionValue(originalLines, Civi, match.Groups[1].Value.Trim(), 5000);
                processed = string.Format("CiviDelay  ={0}, {1,-10}{2}", match.Groups[1].Value, (int)value, match.Groups[3].Value);
            }
            match = LpIdle.Match(line);
            if (match.Success) {
                double value = FindFactionValue(originalLines, LpIdle, match.Groups[1].Value.Trim(), 15000);
                processed = string.Format("LPIncIdle       ={0}, {1,-10}{2}", match.Groups[1].Value, (int)value, match.Groups[3].Value);
            }
            if (options.InfiniteMorale) {
                processed = ReplaceMorale(processed, line, "MoralsDecLostMem", MoraleLost, "0");
                processed = ReplaceMorale(processed, line, "MoralsDecFlee", MoraleFlee, "0");
                processed = ReplaceMorale(processed, line, "MoralsDecOverPop", MoraleOverPop, "99999999");
                processed = ReplaceMorale(processed, line, "MoralsIncIdle", MoraleIdle, "500");
            } else {
                Match key = MoraleKey.Match(line);
                if (key.Success) {
                    string? saved = originalLines.FirstOrDefault(candidate => { Match other = MoraleKey.Match(candidate); return other.Success && other.Groups[1].Value.Equals(key.Groups[1].Value, StringComparison.OrdinalIgnoreCase) && other.Groups[2].Value.Equals(key.Groups[2].Value, StringComparison.OrdinalIgnoreCase); });
                    if (saved != null) processed = saved;
                }
            }
            lines[i] = processed;
        }
        return PatchText.Write(text, lineEnding, lines, canonical);
    }

    private static Dictionary<string, double> ReadValues(IEnumerable<string> lines, Regex regex, Func<Match, string> key, int valueGroup) {
        var values = new Dictionary<string, double>(StringComparer.OrdinalIgnoreCase);
        foreach (string line in lines) { Match match = regex.Match(line); if (match.Success && double.TryParse(match.Groups[valueGroup].Value.Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double value)) values[key(match)] = value; }
        return values;
    }
    private static double GetOriginal(IReadOnlyDictionary<string, double> values, string key, string fallback) => values.TryGetValue(key, out double value) ? value : double.TryParse(fallback.Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out value) ? value : 0;
    private static double FindFactionValue(IEnumerable<string> lines, Regex regex, string faction, double fallback) { foreach (string line in lines) { Match match = regex.Match(line); if (match.Success && match.Groups[1].Value.Trim() == faction && double.TryParse(match.Groups[2].Value.Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double value)) return value; } return fallback; }
    private static string ReplaceMorale(string current, string original, string prefix, Regex regex, string value) { if (!original.StartsWith(prefix, StringComparison.Ordinal)) return current; Match match = regex.Match(original); return match.Success ? match.Groups[1].Value + value + match.Groups[2].Value : current; }
}
