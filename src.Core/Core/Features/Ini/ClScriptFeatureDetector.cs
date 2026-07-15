using System.Globalization;
using System.Text.RegularExpressions;
using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features.Ini;

internal sealed class ClScriptFeatureDetector
{
    private const string BackupKey = "SYSTEM/cl_script.ini";
    private static readonly Regex RegexCiviLoad = new(@"CiviDelay\s*=\s*([A-Z]{3})\s*,\s*(\d+)", RegexOptions.Compiled);
    private static readonly Regex RegexMoraleLostMemLoad = new(@"MoralsDecLostMem\s*=\s*GER\s*,\s*(\d+)", RegexOptions.Compiled);
    private static readonly Regex RegexMoraleFleeLoad = new(@"MoralsDecFlee\s*=\s*GER\s*,\s*(\d+)", RegexOptions.Compiled);
    private static readonly Regex RegexMoraleOverPopLoad = new(@"MoralsDecOverPop\s*=\s*GER\s*,\s*(\d+)", RegexOptions.Compiled);
    private static readonly Regex RegexMoraleIdleLoad = new(@"MoralsIncIdle\s*=\s*GER\s*,\s*(\d+)", RegexOptions.Compiled);
    private static readonly Regex RegexSpellValueLoad = new(@"^(Value\d*)\s*=\s*([A-Z]{3})\s*,\s*(Spell\d+)\s*,\s*([^;\r\n]+)", RegexOptions.Compiled | RegexOptions.Multiline);
    private static readonly Regex RegexRadiusLoad = new(@"^Radius\s*=\s*([A-Z]{3})\s*,\s*(Spell\d+)\s*,\s*([^;\r\n]+)", RegexOptions.Compiled | RegexOptions.Multiline);
    private static readonly Regex RegexAbilityValueLoad = new(@"^(Value\d*)\s*=\s*([A-Z]{3})\s*,\s*(SAbility\d+)\s*,\s*([^;\r\n]+)", RegexOptions.Compiled | RegexOptions.Multiline);
    private readonly ILogger _logger;

    internal ClScriptFeatureDetector(ILogger logger) => _logger = logger;

    internal void Detect(string gamePath, BackupManager backupManager, PatchProfile profile)
    {
        string path = Path.Combine(gamePath, @"SYSTEM\cl_script.ini");
        if (!File.Exists(path)) return;

        try
        {
            string text = Decode(File.ReadAllBytes(path));

            MatchCollection civiMatches = RegexCiviLoad.Matches(text);
            profile.FastCiviProduction = civiMatches.Count > 0 && civiMatches.Cast<Match>().All(match =>
                double.TryParse(match.Groups[2].Value, NumberStyles.Any, CultureInfo.InvariantCulture, out double delay) &&
                delay > 0 && delay <= 500.0);

            Match matchLost = RegexMoraleLostMemLoad.Match(text);
            Match matchFlee = RegexMoraleFleeLoad.Match(text);
            Match matchOverPop = RegexMoraleOverPopLoad.Match(text);
            Match matchIdle = RegexMoraleIdleLoad.Match(text);
            profile.InfiniteMorale = matchLost.Success && matchFlee.Success && matchOverPop.Success && matchIdle.Success &&
                int.TryParse(matchLost.Groups[1].Value, out int lost) && lost == 0 &&
                int.TryParse(matchFlee.Groups[1].Value, out int flee) && flee == 0 &&
                int.TryParse(matchOverPop.Groups[1].Value, out int overPop) && overPop >= 99999999 &&
                int.TryParse(matchIdle.Groups[1].Value, out int idle) && idle == 500;

            if (!backupManager.HasFile(BackupKey)) return;

            string originalText = Decode(backupManager.GetBackupBytes(BackupKey));
            Dictionary<string, double> currentSpells = ReadKeyedValues(text, RegexSpellValueLoad);
            Dictionary<string, double> originalSpells = ReadKeyedValues(originalText, RegexSpellValueLoad);
            Dictionary<string, double> currentRadius = ReadRadiusValues(text);
            Dictionary<string, double> originalRadius = ReadRadiusValues(originalText);
            profile.SpellDamage5x = AllScaled(currentSpells, originalSpells, ClScriptPatcher.DamageSpellKeys.Select(key => $"Value_{key}"), 5);
            profile.SpellHealing10x = AllScaled(currentSpells, originalSpells, new[] { "Value_KEL_Spell1" }, 10);
            profile.SpellResurrection = DetectResurrection(currentSpells, originalSpells);
            profile.SpellRange3x = RadiusScaledByThree(currentRadius, originalRadius);

            Dictionary<string, double> currentAbilities = ReadKeyedValues(text, RegexAbilityValueLoad);
            Dictionary<string, double> originalAbilities = ReadKeyedValues(originalText, RegexAbilityValueLoad);
            profile.GeneralSkills = AllScaled(currentAbilities, originalAbilities, originalAbilities.Keys, 5);
        }
        catch (Exception ex)
        {
            _logger.Log(string.Format(Loc.Get("SvcLogDetectFailed"), "cl_script.ini", ex.Message));
        }
    }

    private static string Decode(byte[] bytes) =>
        PatchText.GameEncoding.GetString(GameLZSS.DecompressPfil(bytes));

    private static Dictionary<string, double> ReadKeyedValues(string text, Regex regex)
    {
        var values = new Dictionary<string, double>(StringComparer.OrdinalIgnoreCase);
        foreach (Match match in regex.Matches(text))
        {
            if (double.TryParse(match.Groups[4].Value.Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double value))
                values[$"{match.Groups[1].Value.Trim()}_{match.Groups[2].Value.Trim()}_{match.Groups[3].Value.Trim()}"] = value;
        }
        return values;
    }

    private static Dictionary<string, double> ReadRadiusValues(string text)
    {
        var values = new Dictionary<string, double>(StringComparer.OrdinalIgnoreCase);
        foreach (Match match in RegexRadiusLoad.Matches(text))
        {
            if (double.TryParse(match.Groups[3].Value.Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double value))
                values[$"Radius_{match.Groups[1].Value.Trim()}_{match.Groups[2].Value.Trim()}"] = value;
        }
        return values;
    }

    private static bool AllScaled(Dictionary<string, double> current, Dictionary<string, double> original, IEnumerable<string> keys, int multiplier)
    {
        bool any = false;
        foreach (string key in keys)
        {
            if (!original.TryGetValue(key, out double originalValue) || originalValue <= 0) continue;
            int expected = (int)(originalValue * multiplier);
            if (expected == (int)originalValue) continue;
            if (!current.TryGetValue(key, out double currentValue) || (int)currentValue != expected) return false;
            any = true;
        }
        return any;
    }

    private static bool RadiusScaledByThree(Dictionary<string, double> current, Dictionary<string, double> original)
    {
        bool any = false;
        foreach (string key in original.Keys)
        {
            double baseline = original[key];
            if (baseline <= 0) continue;
            if (!current.TryGetValue(key, out double value)) return false;
            double ratio = value / baseline;
            if (Math.Abs(ratio - 3.0) > 0.05 && Math.Abs(ratio - 7.5) > 0.1) return false;
            any = true;
        }
        return any;
    }

    private static bool DetectResurrection(Dictionary<string, double> current, Dictionary<string, double> original)
    {
        bool any = false;
        foreach (string key in new[] { "Value_KEL_Spell3", "Value2_KEL_Spell3" })
        {
            if (!original.TryGetValue(key, out double originalValue) || (int)originalValue == 100) continue;
            if (!current.TryGetValue(key, out double currentValue) || (int)currentValue != 100) return false;
            any = true;
        }
        return any;
    }
}
