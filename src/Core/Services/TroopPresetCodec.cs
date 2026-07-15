using System.Globalization;
using System.Text;

namespace AgainstRomeModifier.Core.Services;

internal sealed record TroopPresetParseResult(IReadOnlyDictionary<string, double[]> Stats, int SkippedLines);

internal static class TroopPresetCodec
{
    internal static TroopPresetParseResult Parse(IEnumerable<string> lines, Func<string, double[]> defaultStats)
    {
        var result = new Dictionary<string, double[]>(StringComparer.OrdinalIgnoreCase);
        int skipped = 0;
        foreach (string sourceLine in lines)
        {
            string line = sourceLine.Trim();
            if (string.IsNullOrEmpty(line) || line.StartsWith('#') || line.StartsWith(';')) continue;
            int separator = line.IndexOf('=');
            if (separator < 1) continue;
            string key = line[..separator].Trim();
            string[] values = line[(separator + 1)..].Split(',');
            if (values.Length < 4)
            {
                skipped++;
                continue;
            }

            double[] parsed = new double[Math.Min(values.Length, 9)];
            bool valid = true;
            for (int index = 0; index < parsed.Length; index++)
            {
                if (!double.TryParse(values[index].Trim(), NumberStyles.Any,
                        CultureInfo.InvariantCulture, out parsed[index]))
                {
                    valid = false;
                    break;
                }
            }
            if (!valid)
            {
                skipped++;
                continue;
            }

            double[] fallback = defaultStats(key);
            double speed = 0;
            double sight = values.Length == 6 ? parsed[4] : parsed.Length > 5 ? parsed[5] : 0;
            double reload = values.Length == 6 ? parsed[5] : parsed.Length > 6 ? parsed[6] : 0;
            double range = parsed.Length > 7 ? parsed[7] : 0;
            double spellRadius = parsed.Length > 8 ? parsed[8] : 0;
            if (values.Length < 9 && values.Length != 6)
            {
                if (values.Length <= 4) speed = At(fallback, 4);
                if (values.Length <= 5) sight = At(fallback, 5);
                if (values.Length <= 6) reload = At(fallback, 6);
                if (values.Length <= 7) range = At(fallback, 7);
                if (values.Length <= 8) spellRadius = At(fallback, 8);
            }
            result[key] = new[] { parsed[0], parsed[1], parsed[2], parsed[3], speed, sight, reload, range, spellRadius };
        }
        return new TroopPresetParseResult(result, skipped);
    }

    internal static string Write(IReadOnlyDictionary<string, double[]> stats, DateTime generatedAt)
    {
        var builder = new StringBuilder();
        builder.AppendLine("# Against Rome Modifier - Custom Troop Preset File (independent modifiers removed)");
        builder.AppendLine($"# Generated on: {generatedAt:yyyy-MM-dd HH:mm:ss}");
        builder.AppendLine("# Format: UnitKey=HP,Dmg,VW,AW,Sight,Relt");
        builder.AppendLine();
        foreach ((string key, double[] values) in stats)
        {
            builder.AppendLine(string.Format(CultureInfo.InvariantCulture, "{0}={1},{2},{3},{4},{5},{6}",
                key, Format(values, 0), Format(values, 1), Format(values, 2), Format(values, 3),
                Format(values, 5), Format(values, 6)));
        }
        return builder.ToString();
    }

    private static double At(double[] values, int index) => index < values.Length ? values[index] : 0;
    private static string Format(double[] values, int index) => At(values, index).ToString("0.##", CultureInfo.InvariantCulture);
}
