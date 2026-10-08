using System.Text;
using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;

namespace AgainstRomeMapEditor.Modules.Scripting.Parsing;

/// <summary>
/// DSL 地圖編輯指令解析器與 Tab 自動補全引擎。
/// </summary>
internal static class EditorCommandParser
{
    /// <summary>
    /// 解析單行指令文字為結構化 ParsedCommand。
    /// 若為註解行或空白行，回傳 null。
    /// </summary>
    public static ParsedCommand? Parse(string line, IReadOnlyDictionary<string, string>? variables = null)
    {
        if (string.IsNullOrWhiteSpace(line)) return null;

        string trimmed = line.Trim();
        if (trimmed.StartsWith('#') || trimmed.StartsWith("//", StringComparison.Ordinal))
        {
            return null; // 註解行
        }

        // 變數替換 ($var)
        if (variables is not null && variables.Count > 0 && trimmed.Contains('$'))
        {
            trimmed = SubstituteVariables(trimmed, variables);
        }

        List<string> tokens = Tokenize(trimmed);
        if (tokens.Count == 0) return null;

        string commandToken = tokens[0];
        string commandName = commandToken.StartsWith('/') ? commandToken[1..] : commandToken;

        var positionalArgs = new List<string>();
        var namedFlags = new Dictionary<string, string?>(StringComparer.OrdinalIgnoreCase);

        for (int i = 1; i < tokens.Count; i++)
        {
            string token = tokens[i];
            if (token.StartsWith("--", StringComparison.Ordinal) && token.Length > 2)
            {
                string flagBody = token[2..];
                int eqIdx = flagBody.IndexOf('=');
                if (eqIdx >= 0)
                {
                    string flagName = flagBody[..eqIdx];
                    string flagVal = flagBody[(eqIdx + 1)..];
                    namedFlags[flagName] = flagVal;
                }
                else
                {
                    // 檢查下一個 token 是否為參數值 (非旗標)
                    if (i + 1 < tokens.Count && !tokens[i + 1].StartsWith('-'))
                    {
                        namedFlags[flagBody] = tokens[i + 1];
                        i++;
                    }
                    else
                    {
                        namedFlags[flagBody] = null; // 布林旗標開關
                    }
                }
            }
            else if (token.StartsWith('-') && token.Length > 1 && !char.IsDigit(token[1]))
            {
                // 短旗標 -f val
                string flagBody = token[1..];
                int eqIdx = flagBody.IndexOf('=');
                if (eqIdx >= 0)
                {
                    string flagName = flagBody[..eqIdx];
                    string flagVal = flagBody[(eqIdx + 1)..];
                    namedFlags[flagName] = flagVal;
                }
                else if (i + 1 < tokens.Count && !tokens[i + 1].StartsWith('-'))
                {
                    namedFlags[flagBody] = tokens[i + 1];
                    i++;
                }
                else
                {
                    namedFlags[flagBody] = null;
                }
            }
            else
            {
                positionalArgs.Add(token);
            }
        }

        return new ParsedCommand(trimmed, commandName, positionalArgs, namedFlags);
    }

    /// <summary>
    /// 將指令行切分為 Token，妥善處理引號字串 ("...") 與逸出字元。
    /// </summary>
    public static List<string> Tokenize(string text)
    {
        var tokens = new List<string>();
        if (string.IsNullOrWhiteSpace(text)) return tokens;

        var current = new StringBuilder();
        bool inQuotes = false;
        bool escapeNext = false;

        for (int i = 0; i < text.Length; i++)
        {
            char c = text[i];

            if (escapeNext)
            {
                current.Append(c);
                escapeNext = false;
                continue;
            }

            if (c == '\\')
            {
                escapeNext = true;
                continue;
            }

            if (c == '"')
            {
                inQuotes = !inQuotes;
                continue;
            }

            if (char.IsWhiteSpace(c) && !inQuotes)
            {
                if (current.Length > 0)
                {
                    tokens.Add(current.ToString());
                    current.Clear();
                }
            }
            else
            {
                current.Append(c);
            }
        }

        if (current.Length > 0)
        {
            tokens.Add(current.ToString());
        }

        return tokens;
    }

    /// <summary>
    /// 執行變數替換 ($varName -> varValue)。
    /// </summary>
    public static string SubstituteVariables(string text, IReadOnlyDictionary<string, string> variables)
    {
        var sb = new StringBuilder();
        for (int i = 0; i < text.Length; i++)
        {
            if (text[i] == '$' && i + 1 < text.Length && (char.IsLetterOrDigit(text[i + 1]) || text[i + 1] == '_'))
            {
                int start = i + 1;
                while (start < text.Length && (char.IsLetterOrDigit(text[start]) || text[start] == '_'))
                {
                    start++;
                }
                string varName = text[(i + 1)..start];
                if (variables.TryGetValue(varName, out string? value))
                {
                    sb.Append(value);
                }
                else
                {
                    sb.Append('$').Append(varName); // 保持原樣
                }
                i = start - 1;
            }
            else
            {
                sb.Append(text[i]);
            }
        }
        return sb.ToString();
    }

    /// <summary>
    /// Tab 自動補全建議生成器。
    /// </summary>
    internal static IReadOnlyList<string> GetCompletions(
        string currentInput,
        int cursorPosition,
        CommandExecutionContext? context,
        IEnumerable<IEditorCommand> registeredCommands)
    {
        if (currentInput is null) return Array.Empty<string>();
        string beforeCursor = cursorPosition >= 0 && cursorPosition <= currentInput.Length
            ? currentInput[..cursorPosition]
            : currentInput;

        List<string> tokens = Tokenize(beforeCursor);
        bool endsWithSpace = beforeCursor.EndsWith(' ');

        // 1. 若處於第一個 token 且尚未輸入空格：補全指令名稱
        if (tokens.Count == 0 || (tokens.Count == 1 && !endsWithSpace))
        {
            string prefix = tokens.Count == 1 ? tokens[0] : "";
            string cleanPrefix = prefix.StartsWith('/') ? prefix[1..] : prefix;

            var matches = new List<string>();
            foreach (var cmd in registeredCommands)
            {
                if (cmd.Name.StartsWith(cleanPrefix, StringComparison.OrdinalIgnoreCase))
                {
                    matches.Add("/" + cmd.Name);
                }
                foreach (var alias in cmd.Aliases)
                {
                    if (alias.StartsWith(cleanPrefix, StringComparison.OrdinalIgnoreCase))
                    {
                        matches.Add("/" + alias);
                    }
                }
            }
            return matches.Distinct(StringComparer.OrdinalIgnoreCase).OrderBy(x => x).ToArray();
        }

        // 2. 指令已確定，正在輸入引數或旗標
        string commandToken = tokens[0];
        string cmdName = commandToken.StartsWith('/') ? commandToken[1..] : commandToken;
        IEditorCommand? targetCmd = registeredCommands.FirstOrDefault(c =>
            string.Equals(c.Name, cmdName, StringComparison.OrdinalIgnoreCase) ||
            c.Aliases.Any(a => string.Equals(a, cmdName, StringComparison.OrdinalIgnoreCase)));

        if (targetCmd is null) return Array.Empty<string>();

        string lastToken = endsWithSpace ? "" : tokens[^1];

        // 2.1 正在輸入旗標 (--flag)
        if (lastToken.StartsWith("--", StringComparison.Ordinal) || (endsWithSpace && lastToken.Length == 0))
        {
            string flagPrefix = lastToken.StartsWith("--", StringComparison.Ordinal) ? lastToken[2..] : "";
            var flagCompletions = targetCmd.Parameters
                .Where(p => p.Type == CommandParameterType.Flag || !p.IsRequired)
                .Select(p => "--" + p.Name)
                .Where(f => f[2..].StartsWith(flagPrefix, StringComparison.OrdinalIgnoreCase))
                .ToList();

            if (flagCompletions.Count > 0) return flagCompletions;
        }

        // 2.2 正在輸入物件範本 (Template / Object)
        if (context?.AvailableObjectTypes is not null &&
            targetCmd.Parameters.Any(p => p.Type == CommandParameterType.TemplateName))
        {
            var templateMatches = context.AvailableObjectTypes
                .Select(t => t.NameDef)
                .Where(n => n.StartsWith(lastToken, StringComparison.OrdinalIgnoreCase))
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .Take(20)
                .ToList();

            if (templateMatches.Count > 0) return templateMatches;
        }

        // 2.3 正在輸入材質名稱 (Texture)
        if (context?.KnownTextures is not null &&
            targetCmd.Parameters.Any(p => p.Type == CommandParameterType.TextureName))
        {
            var textureMatches = context.KnownTextures
                .Where(t => t.StartsWith(lastToken, StringComparison.OrdinalIgnoreCase))
                .Take(20)
                .ToList();

            if (textureMatches.Count > 0) return textureMatches;
        }

        return Array.Empty<string>();
    }
}
