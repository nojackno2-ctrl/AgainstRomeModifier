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

    /// <summary>
    /// 以遊戲資料一致的解析規則（Trim + InvariantCulture）讀取欄位為 double；
    /// 索引越界或無法解析時回傳 0。收斂原本散落各處的 double.TryParse 樣板。
    /// </summary>
    internal static double ParseDouble(string[] columns, int index) =>
        columns != null && index >= 0 && index < columns.Length
            ? ParseDouble(columns[index])
            : 0;

    internal static double ParseDouble(string value) =>
        double.TryParse(
            value?.Trim(),
            System.Globalization.NumberStyles.Any,
            System.Globalization.CultureInfo.InvariantCulture,
            out double result)
            ? result : 0;

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
