using System.Text;
using System.Text.RegularExpressions;

namespace AgainstRomeMapEditor.Modules.Localization;

public enum PutIssueSeverity
{
    Info,
    Warning,
    Error
}

/// <summary>
/// 語法或編碼檢測問題報告項目。
/// </summary>
public sealed record PutScriptIssue(
    string Key,
    PutIssueSeverity Severity,
    string Message,
    int LineNumber = 0,
    int Column = 0,
    string? OffendingFragment = null,
    string? SuggestedFix = null);

/// <summary>
/// 檢驗結果報告容器。
/// </summary>
public sealed class PutScriptValidationReport
{
    private readonly List<PutScriptIssue> _issues = new();

    public IReadOnlyList<PutScriptIssue> Issues => _issues;

    public bool IsValid => !_issues.Any(i => i.Severity == PutIssueSeverity.Error);

    public int ErrorCount => _issues.Count(i => i.Severity == PutIssueSeverity.Error);
    public int WarningCount => _issues.Count(i => i.Severity == PutIssueSeverity.Warning);

    public void AddIssue(PutScriptIssue issue) => _issues.Add(issue);

    public void AddError(string key, string message, int line = 0, int col = 0, string? fragment = null, string? fix = null, string? suggestedFix = null) =>
        AddIssue(new PutScriptIssue(key, PutIssueSeverity.Error, message, line, col, fragment, fix ?? suggestedFix));

    public void AddWarning(string key, string message, int line = 0, int col = 0, string? fragment = null, string? fix = null, string? suggestedFix = null) =>
        AddIssue(new PutScriptIssue(key, PutIssueSeverity.Warning, message, line, col, fragment, fix ?? suggestedFix));

    public void AddInfo(string key, string message, int line = 0, int col = 0, string? fragment = null, string? fix = null, string? suggestedFix = null) =>
        AddIssue(new PutScriptIssue(key, PutIssueSeverity.Info, message, line, col, fragment, fix ?? suggestedFix));
}

/// <summary>
/// .put 指令碼語法、字元編碼安全與引擎限制靜態分析驗證器。
/// 防止引號逃逸、分號缺失、溢位常值以及目標語系不支援的編碼字元導致遊戲當機。
/// </summary>
public static class PutScriptSyntaxValidator
{
    public const int MaxLiteralBytes = 100;

    /// <summary>
    /// 取得指定語系名稱或編碼名稱的嚴格編碼器（遇未支援字元立即拋出 EncoderFallbackException）。
    /// </summary>
    public static Encoding GetStrictEncoding(string encodingNameOrLanguage)
    {
        Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);

        string norm = encodingNameOrLanguage.ToUpperInvariant();
        return norm switch
        {
            "UTF-8" or "UTF8" => new UTF8Encoding(false, true),
            "BIG5" or "950" or "ZH" or "TW" or "CHT" or "TRADITIONALCHINESE" =>
                Encoding.GetEncoding(950, EncoderFallback.ExceptionFallback, DecoderFallback.ExceptionFallback),
            "1251" or "CP1251" or "WINDOWS-1251" or "RU" or "RUSSIAN" =>
                Encoding.GetEncoding(1251, EncoderFallback.ExceptionFallback, DecoderFallback.ExceptionFallback),
            _ => // 預設西歐／德語 Windows-1252 (CP1252)
                Encoding.GetEncoding(1252, EncoderFallback.ExceptionFallback, DecoderFallback.ExceptionFallback)
        };
    }

    /// <summary>
    /// 檢驗原始 .put 腳本字串文字的語法完整性。
    /// </summary>
    public static PutScriptValidationReport ValidateScriptText(string scriptText, string targetEncoding = "WINDOWS-1252")
    {
        var report = new PutScriptValidationReport();
        if (string.IsNullOrWhiteSpace(scriptText))
        {
            report.AddWarning("", "腳本內容為空。");
            return report;
        }

        Encoding encoding;
        try
        {
            encoding = GetStrictEncoding(targetEncoding);
        }
        catch (Exception ex)
        {
            report.AddError("", $"無法取得指定之目標編碼 '{targetEncoding}': {ex.Message}");
            return report;
        }

        string[] lines = scriptText.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None);
        bool inMultiLineString = false;
        string currentKey = "";
        int stringStartLine = 0;

        for (int lineIdx = 0; lineIdx < lines.Length; lineIdx++)
        {
            int lineNum = lineIdx + 1;
            string line = lines[lineIdx];
            string trimmed = line.Trim();

            if (trimmed.Length == 0 || trimmed.StartsWith("//", StringComparison.Ordinal) || trimmed.StartsWith(';'))
            {
                continue;
            }

            // 檢查 NUL 字元
            if (line.Contains('\0'))
            {
                report.AddError(currentKey, "字串中包含非法的 NUL (\\0) 字元，會造成 C 字串提前截斷。", lineNum, line.IndexOf('\0') + 1);
            }

            // 檢查變數宣告
            var varDeclMatch = Regex.Match(line, @"^\s*var:\s*([a-zA-Z0-9_]+)\s*=\s*(.*)$");
            if (varDeclMatch.Success)
            {
                currentKey = varDeclMatch.Groups[1].Value;
                string rest = varDeclMatch.Groups[2].Value;

                // 檢查變數名稱合法性
                if (!Regex.IsMatch(currentKey, @"^[a-zA-Z0-9_]+$"))
                {
                    report.AddError(currentKey, $"無效的變數名稱 '{currentKey}'，僅允許英數字與底線。", lineNum, 1);
                }

                // 檢查是否包含未轉義的反斜線
                CheckDanglingBackslashes(currentKey, rest, lineNum, report);

                // 檢查引號配對
                int quoteCount = CountUnescapedQuotes(rest);
                if (quoteCount % 2 != 0)
                {
                    inMultiLineString = true;
                    stringStartLine = lineNum;
                }
                else
                {
                    // 單行完成，檢查分號
                    if (!rest.Contains(';'))
                    {
                        report.AddError(currentKey, $"變數 '{currentKey}' 宣告缺少結尾分號 (;)。", lineNum, line.Length, suggestedFix: "於行尾加入分號 ';'");
                    }
                }
            }
            else if (inMultiLineString)
            {
                CheckDanglingBackslashes(currentKey, line, lineNum, report);
                int quoteCount = CountUnescapedQuotes(line);
                if (quoteCount % 2 != 0)
                {
                    inMultiLineString = false;
                    if (!line.Contains(';'))
                    {
                        report.AddError(currentKey, $"多行字串於第 {lineNum} 行閉合但缺少分號 (;)。", lineNum, line.Length);
                    }
                }
            }

            // 檢查該行中所有字面值區段的編碼與溢位
            var literalMatches = Regex.Matches(line, @"""((?:\\.|[^""\\])*)""");
            foreach (Match m in literalMatches)
            {
                string rawLiteral = m.Groups[1].Value;
                int col = m.Index + 1;

                // 1. 編碼安全性
                CheckStringEncoding(currentKey, rawLiteral, encoding, lineNum, col, report);

                // 2. 緩衝區長度檢查 (MaxLiteralBytes)
                int byteLen;
                try
                {
                    byteLen = encoding.GetByteCount(rawLiteral);
                }
                catch
                {
                    byteLen = Encoding.UTF8.GetByteCount(rawLiteral);
                }

                if (byteLen > MaxLiteralBytes)
                {
                    report.AddError(
                        currentKey,
                        $"字串常值位元組長度 ({byteLen} bytes) 超過遊戲緩衝區上限 ({MaxLiteralBytes} bytes)，進入關卡會導致堆疊/堆積崩潰！",
                        lineNum,
                        col,
                        fragment: rawLiteral.Length > 20 ? rawLiteral[..20] + "..." : rawLiteral,
                        suggestedFix: "使用 SplitEscapedIntoSafeChunks 進行安全切段");
                }
            }
        }

        if (inMultiLineString)
        {
            report.AddError(currentKey, $"字串從第 {stringStartLine} 行開始未正確閉合雙引號 (\") 直至檔案結尾。", stringStartLine, 1);
        }

        return report;
    }

    /// <summary>
    /// 檢驗記憶體中的 BriefingTextTable 結構模型。
    /// </summary>
    public static PutScriptValidationReport ValidateTable(BriefingTextTable table, string targetEncoding = "WINDOWS-1252")
    {
        ArgumentNullException.ThrowIfNull(table);
        var report = new PutScriptValidationReport();
        Encoding encoding = GetStrictEncoding(targetEncoding);

        foreach (var node in table.Document.Variables)
        {
            var desc = BriefingTextTableCatalog.GetDescriptor(node.Key);

            if (desc.IsRequired && string.IsNullOrWhiteSpace(node.StringValue))
            {
                report.AddError(node.Key, $"必要變數 '{node.Key}' ({desc.NameZH}) 不得為空。");
            }

            if (desc.ValueType == BriefingVariableType.SingleLineString && node.StringValue is not null)
            {
                if (node.StringValue.Contains('\r') || node.StringValue.Contains('\n'))
                {
                    report.AddError(node.Key, $"單行文字欄位 '{node.Key}' 不得包含換行字元。");
                }
            }

            if (node.StringValue is not null)
            {
                CheckStringEncoding(node.Key, node.StringValue, encoding, 0, 0, report);

                // 檢查每個片段長度
                foreach (string fragment in node.LiteralFragments)
                {
                    string escapedFrag = PutScriptFormatting.Escape(fragment);
                    int bLen;
                    try
                    {
                        bLen = encoding.GetByteCount(escapedFrag);
                    }
                    catch
                    {
                        bLen = Encoding.UTF8.GetByteCount(escapedFrag);
                    }

                    if (bLen > desc.MaxLiteralBytes)
                    {
                        report.AddError(
                            node.Key,
                            $"變數 '{node.Key}' 片段位元組長度 ({bLen}) 超過上限 ({desc.MaxLiteralBytes})。",
                            fragment: fragment.Length > 15 ? fragment[..15] + "..." : fragment);
                    }
                }
            }
        }

        return report;
    }

    private static void CheckStringEncoding(string key, string text, Encoding encoding, int line, int col, PutScriptValidationReport report)
    {
        for (int i = 0; i < text.Length; i++)
        {
            char c = text[i];
            if (c == '\\' && i + 1 < text.Length)
            {
                // 跳過跳脫字元
                i++;
                continue;
            }

            try
            {
                encoding.GetByteCount(text.AsSpan(i, char.IsSurrogate(c) && i + 1 < text.Length ? 2 : 1));
                if (char.IsSurrogate(c)) i++;
            }
            catch (EncoderFallbackException)
            {
                string charRep = char.IsSurrogate(c) && i + 1 < text.Length ? text.Substring(i, 2) : c.ToString();
                int codePoint = char.ConvertToUtf32(text, i);
                report.AddError(
                    key,
                    $"字元 '{charRep}' (U+{codePoint:X4}) 無法由目標編碼 {encoding.WebName} 編碼，遊戲將顯示亂碼或崩潰！",
                    line,
                    col + i,
                    fragment: charRep,
                    suggestedFix: $"替換為英數、ASCII 或改用支援該語系之編碼。");
                if (char.IsSurrogate(c)) i++;
            }
        }
    }

    private static int CountUnescapedQuotes(string text)
    {
        int count = 0;
        for (int i = 0; i < text.Length; i++)
        {
            if (text[i] == '"' && (i == 0 || text[i - 1] != '\\'))
            {
                count++;
            }
        }
        return count;
    }

    private static void CheckDanglingBackslashes(string key, string line, int lineNum, PutScriptValidationReport report)
    {
        for (int i = 0; i < line.Length; i++)
        {
            if (line[i] == '\\')
            {
                if (i + 1 >= line.Length)
                {
                    report.AddError(key, "行末存在懸空的反斜線 (\\)。", lineNum, i + 1);
                    break;
                }
                char next = line[i + 1];
                if (next is not ('n' or 'r' or 't' or '"' or '\\'))
                {
                    report.AddWarning(key, $"非標準的跳脫序列 '\\{next}'。", lineNum, i + 1);
                }
                i++; // 跳過下一字元
            }
        }
    }
}
