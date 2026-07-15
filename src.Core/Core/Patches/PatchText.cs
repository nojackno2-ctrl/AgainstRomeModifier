using System.Text;

namespace AgainstRomeModifier.Core.Patches;

internal static class PatchText {
    static PatchText() => Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);

    internal static Encoding GameEncoding => Encoding.GetEncoding(1251);

    internal static (string Text, string LineEnding, string[] Lines) Read(byte[] bytes) {
        string text = GameEncoding.GetString(GameLZSS.DecompressPfil(bytes));
        string lineEnding = text.Contains("\r\n", StringComparison.Ordinal) ? "\r\n" : "\n";
        return (text, lineEnding, text.Split(new[] { lineEnding }, StringSplitOptions.None));
    }

    internal static byte[] Write(string originalText, string lineEnding, IEnumerable<string> lines, byte[] compressionTemplate) {
        string text = string.Join(lineEnding, lines);
        if (originalText.EndsWith(lineEnding, StringComparison.Ordinal) && !text.EndsWith(lineEnding, StringComparison.Ordinal)) {
            text += lineEnding;
        }
        return GameLZSS.CompressPfil(GameEncoding.GetBytes(text), compressionTemplate);
    }

    internal static string[] ParseCsvLine(string line) => line?.Split(',') ?? Array.Empty<string>();
    internal static string ToCsvString(IEnumerable<string> columns) => string.Join(",", columns);

    internal static bool CheckLength(string value, int targetLength, out string finalValue) {
        value = value.Trim();
        finalValue = value;
        if (value.Length <= targetLength) return true;
        if (double.TryParse(value, System.Globalization.NumberStyles.Any, System.Globalization.CultureInfo.InvariantCulture, out double number)) {
            string oneDecimal = number.ToString("F1", System.Globalization.CultureInfo.InvariantCulture);
            if (oneDecimal.Length <= targetLength) {
                finalValue = oneDecimal;
                return true;
            }
            string integer = Math.Round(number).ToString("F0", System.Globalization.CultureInfo.InvariantCulture);
            if (integer.Length <= targetLength) {
                finalValue = integer;
                return true;
            }
        }
        return false;
    }
}
