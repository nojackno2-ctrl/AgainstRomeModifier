using System.Text;
using System.Text.RegularExpressions;

namespace AgainstRomeMapEditor.Modules.Localization;

/// <summary>
/// 表示 .put 指令碼中的單一抽象語法樹節點（註解、空白行或變數定義）。
/// 保留原始程式碼排版、行首縮排與行末符號，以支援無損回寫。
/// </summary>
public abstract record PutScriptNode
{
    public abstract string ToScriptText();
}

/// <summary>註解行節點（例如 // 或 ; 開頭）。</summary>
public sealed record PutCommentNode(string RawText) : PutScriptNode
{
    public override string ToScriptText() => RawText;
}

/// <summary>空白或排版行節點。</summary>
public sealed record PutWhitespaceNode(string RawText) : PutScriptNode
{
    public override string ToScriptText() => RawText;
}

/// <summary>
/// .put 變數指派節點：var:名稱 = 表達式;
/// 支援單行字串、相鄰字串串接（複合字串）、加號連接以及整數常數。
/// </summary>
public sealed record PutVariableNode : PutScriptNode
{
    public string Key { get; init; }
    public string? StringValue { get; init; }
    public int? IntegerValue { get; init; }
    public bool IsComposite { get; init; }
    public string Indentation { get; init; } = "";
    public string TrailingComment { get; init; } = "";
    public bool HasSemicolon { get; init; } = true;

    /// <summary>包含原始所有字面值片段（已跳脫或未跳脫）。</summary>
    public IReadOnlyList<string> LiteralFragments { get; init; }

    public PutVariableNode(
        string key,
        string? stringValue,
        int? integerValue = null,
        bool isComposite = false,
        IReadOnlyList<string>? fragments = null,
        string indentation = "",
        string trailingComment = "",
        bool hasSemicolon = true)
    {
        Key = key ?? throw new ArgumentNullException(nameof(key));
        StringValue = stringValue;
        IntegerValue = integerValue;
        IsComposite = isComposite;
        LiteralFragments = fragments ?? (stringValue is not null ? new[] { stringValue } : Array.Empty<string>());
        Indentation = indentation;
        TrailingComment = trailingComment;
        HasSemicolon = hasSemicolon;
    }

    public override string ToScriptText()
    {
        var builder = new StringBuilder();
        builder.Append(Indentation);
        builder.Append("var:").Append(Key).Append(" =");

        if (IntegerValue.HasValue)
        {
            builder.Append(' ').Append(IntegerValue.Value);
        }
        else if (StringValue is not null)
        {
            if (IsComposite || LiteralFragments.Count > 1)
            {
                builder.Append('\n');
                for (int i = 0; i < LiteralFragments.Count; i++)
                {
                    builder.Append(Indentation).Append("    \"")
                           .Append(PutScriptFormatting.Escape(LiteralFragments[i]))
                           .Append('"');
                    if (i < LiteralFragments.Count - 1)
                    {
                        builder.Append('\n');
                    }
                }
            }
            else
            {
                builder.Append('"').Append(PutScriptFormatting.Escape(StringValue)).Append('"');
            }
        }
        else
        {
            builder.Append("\"\"");
        }

        if (HasSemicolon) builder.Append(';');
        if (!string.IsNullOrEmpty(TrailingComment))
        {
            builder.Append(' ').Append(TrailingComment);
        }
        return builder.ToString();
    }
}

/// <summary>
/// .put 格式字串跳脫、反跳脫與常值切段輔助工具。
/// </summary>
public static class PutScriptFormatting
{
    public const int DefaultMaxLiteralBytes = 100;

    public static string Escape(string value)
    {
        if (string.IsNullOrEmpty(value)) return "";
        return value
            .Replace("\\", "\\\\", StringComparison.Ordinal)
            .Replace("\r", "\\r", StringComparison.Ordinal)
            .Replace("\n", "\\n", StringComparison.Ordinal)
            .Replace("\t", "\\t", StringComparison.Ordinal)
            .Replace("\"", "\\\"", StringComparison.Ordinal);
    }

    public static string Unescape(string escaped)
    {
        if (string.IsNullOrEmpty(escaped)) return "";
        var sb = new StringBuilder(escaped.Length);
        for (int i = 0; i < escaped.Length; i++)
        {
            if (escaped[i] == '\\' && i + 1 < escaped.Length)
            {
                char next = escaped[++i];
                sb.Append(next switch
                {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '"' => '"',
                    '\\' => '\\',
                    _ => "\\" + next
                });
            }
            else
            {
                sb.Append(escaped[i]);
            }
        }
        return sb.ToString();
    }

    /// <summary>
    /// 將已跳脫的字串安全切段為每個片段位元組數 ≤ maxBytes 的清單，
    /// 絕不切斷反斜線跳脫序列（例如 \n），優先於空格或換行處折行。
    /// </summary>
    public static IReadOnlyList<string> SplitEscapedIntoSafeChunks(string escaped, Encoding encoding, int maxBytes = DefaultMaxLiteralBytes)
    {
        ArgumentNullException.ThrowIfNull(escaped);
        ArgumentNullException.ThrowIfNull(encoding);

        var chunks = new List<string>();
        int start = 0;
        int length = escaped.Length;

        while (start < length)
        {
            int remaining = length - start;
            int currentBytes = encoding.GetByteCount(escaped.AsSpan(start, remaining));
            if (currentBytes <= maxBytes)
            {
                chunks.Add(Unescape(escaped[start..]));
                break;
            }

            int position = start;
            int lastBreak = -1;

            while (position < length)
            {
                int tokenLen = (escaped[position] == '\\' && position + 1 < length) ? 2 : 1;
                string testSub = escaped[start..(position + tokenLen)];
                if (encoding.GetByteCount(testSub) > maxBytes) break;

                position += tokenLen;
                if (escaped[position - 1] == ' ' || (tokenLen == 2 && escaped[position - 1] == 'n'))
                {
                    lastBreak = position;
                }
            }

            int cut = (lastBreak > start && lastBreak - start >= (position - start) / 2) ? lastBreak : position;
            if (cut == start)
            {
                // 單一 token 本身超標（極端例外），至少前進該 token 避免無窮迴圈
                cut = (escaped[start] == '\\' && start + 1 < length) ? start + 2 : start + 1;
            }

            chunks.Add(Unescape(escaped[start..cut]));
            start = cut;
        }

        return chunks.Count > 0 ? chunks : new[] { "" };
    }
}

/// <summary>
/// .put 指令碼抽象語法樹文件模型，提供無損結構保留、變數存取與序列化。
/// </summary>
public sealed class PutScriptDocument
{
    private readonly List<PutScriptNode> _nodes = new();
    private string _lineEnding = "\r\n";

    public IReadOnlyList<PutScriptNode> Nodes => _nodes;

    public IEnumerable<PutVariableNode> Variables => _nodes.OfType<PutVariableNode>();

    public string LineEnding
    {
        get => _lineEnding;
        set => _lineEnding = value is "\r\n" or "\n" ? value : "\r\n";
    }

    public static PutScriptDocument Parse(string content)
    {
        var doc = new PutScriptDocument();
        if (string.IsNullOrEmpty(content)) return doc;

        doc.LineEnding = content.Contains("\r\n", StringComparison.Ordinal) ? "\r\n" : "\n";
        string[] rawLines = content.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None);

        int i = 0;
        while (i < rawLines.Length)
        {
            string line = rawLines[i];
            string trimmed = line.Trim();

            if (trimmed.Length == 0)
            {
                doc._nodes.Add(new PutWhitespaceNode(line));
                i++;
                continue;
            }

            if (trimmed.StartsWith("//", StringComparison.Ordinal) || trimmed.StartsWith(';'))
            {
                doc._nodes.Add(new PutCommentNode(line));
                i++;
                continue;
            }

            // 檢查是否為變數開頭 var:
            var varMatch = Regex.Match(line, @"^(\s*)var:\s*([a-zA-Z0-9_]+)\s*=\s*(.*)$");
            if (varMatch.Success)
            {
                string indent = varMatch.Groups[1].Value;
                string key = varMatch.Groups[2].Value;
                string rest = varMatch.Groups[3].Value;

                // 檢查是否為單行整數：var:key = 123;
                var intMatch = Regex.Match(rest, @"^(\d+)\s*;?(.*)$");
                if (intMatch.Success && !rest.TrimStart().StartsWith('"'))
                {
                    int intVal = int.Parse(intMatch.Groups[1].Value);
                    string trail = intMatch.Groups[2].Value.Trim();
                    doc._nodes.Add(new PutVariableNode(key, null, intVal, false, null, indent, trail, rest.Contains(';')));
                    i++;
                    continue;
                }

                // 收集多行字串常值表達式直到分號 ;
                var exprBuilder = new StringBuilder();
                exprBuilder.Append(rest);
                int currentLineIdx = i;

                while (!exprBuilder.ToString().Contains(';') && currentLineIdx + 1 < rawLines.Length)
                {
                    currentLineIdx++;
                    string nextLine = rawLines[currentLineIdx];
                    exprBuilder.Append('\n').Append(nextLine);
                    if (nextLine.TrimStart().StartsWith("var:", StringComparison.Ordinal))
                    {
                        // 遇到下一個變數但未見分號，防禦性終止
                        currentLineIdx--;
                        break;
                    }
                }
                i = currentLineIdx + 1;

                string fullExpr = exprBuilder.ToString();
                bool hasSemicolon = fullExpr.Contains(';');
                int semiPos = fullExpr.IndexOf(';');
                string codePart = hasSemicolon ? fullExpr[..semiPos] : fullExpr;
                string trailing = hasSemicolon && semiPos + 1 < fullExpr.Length ? fullExpr[(semiPos + 1)..].Trim() : "";

                // 解析所有 "..." 字面值
                var fragmentMatches = Regex.Matches(codePart, @"""((?:\\.|[^""\\])*)""");
                var fragments = new List<string>();
                var combinedSb = new StringBuilder();

                foreach (Match m in fragmentMatches)
                {
                    string unescaped = PutScriptFormatting.Unescape(m.Groups[1].Value);
                    fragments.Add(unescaped);
                    combinedSb.Append(unescaped);
                }

                bool isComposite = fragments.Count > 1 || fullExpr.Contains('\n');
                string finalString = combinedSb.ToString();

                doc._nodes.Add(new PutVariableNode(
                    key,
                    finalString,
                    null,
                    isComposite,
                    fragments,
                    indent,
                    trailing,
                    hasSemicolon));
                continue;
            }

            // 未知行或孤立註解
            doc._nodes.Add(new PutCommentNode(line));
            i++;
        }

        return doc;
    }

    public PutVariableNode? GetVariable(string key)
    {
        return Variables.FirstOrDefault(v => v.Key.Equals(key, StringComparison.OrdinalIgnoreCase));
    }

    public string? GetStringValue(string key) => GetVariable(key)?.StringValue;

    public int? GetIntegerValue(string key) => GetVariable(key)?.IntegerValue;

    public void SetVariable(string key, string value, bool isComposite = false, Encoding? encoding = null, int maxLiteralBytes = PutScriptFormatting.DefaultMaxLiteralBytes)
    {
        ArgumentNullException.ThrowIfNull(key);
        ArgumentNullException.ThrowIfNull(value);

        encoding ??= Encoding.UTF8;
        string escaped = PutScriptFormatting.Escape(value);
        IReadOnlyList<string> fragments;

        if (isComposite || encoding.GetByteCount(escaped) > maxLiteralBytes)
        {
            fragments = PutScriptFormatting.SplitEscapedIntoSafeChunks(escaped, encoding, maxLiteralBytes);
            isComposite = fragments.Count > 1;
        }
        else
        {
            fragments = new[] { value };
        }

        for (int i = 0; i < _nodes.Count; i++)
        {
            if (_nodes[i] is PutVariableNode varNode && varNode.Key.Equals(key, StringComparison.OrdinalIgnoreCase))
            {
                _nodes[i] = varNode with
                {
                    StringValue = value,
                    IntegerValue = null,
                    IsComposite = isComposite,
                    LiteralFragments = fragments
                };
                return;
            }
        }

        // 新增節點
        _nodes.Add(new PutVariableNode(key, value, null, isComposite, fragments));
    }

    public void SetVariable(string key, int value)
    {
        ArgumentNullException.ThrowIfNull(key);

        for (int i = 0; i < _nodes.Count; i++)
        {
            if (_nodes[i] is PutVariableNode varNode && varNode.Key.Equals(key, StringComparison.OrdinalIgnoreCase))
            {
                _nodes[i] = varNode with
                {
                    IntegerValue = value,
                    StringValue = null,
                    IsComposite = false,
                    LiteralFragments = Array.Empty<string>()
                };
                return;
            }
        }

        _nodes.Add(new PutVariableNode(key, null, value));
    }

    public bool RemoveVariable(string key)
    {
        for (int i = 0; i < _nodes.Count; i++)
        {
            if (_nodes[i] is PutVariableNode varNode && varNode.Key.Equals(key, StringComparison.OrdinalIgnoreCase))
            {
                _nodes.RemoveAt(i);
                return true;
            }
        }
        return false;
    }

    public string ToScriptText()
    {
        var sb = new StringBuilder();
        for (int i = 0; i < _nodes.Count; i++)
        {
            sb.Append(_nodes[i].ToScriptText());
            if (i < _nodes.Count - 1)
            {
                sb.Append(_lineEnding);
            }
        }
        if (_nodes.Count > 0 && !sb.ToString().EndsWith(_lineEnding, StringComparison.Ordinal))
        {
            sb.Append(_lineEnding);
        }
        return sb.ToString();
    }
}
