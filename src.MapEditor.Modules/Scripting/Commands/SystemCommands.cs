using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;

namespace AgainstRomeMapEditor.Modules.Scripting.Commands;

/// <summary>
/// 指令說明與說明文件查詢指令 (/help)。
/// </summary>
public sealed class HelpCommand : IEditorCommand
{
    private readonly CommandRegistry _registry;

    public string Name => "help";
    public IReadOnlyList<string> Aliases => new[] { "?", "man" };
    public string Category => "System";
    public string DescriptionZh => "顯示所有可用指令列表，或查詢特定指令的詳細用法與參數說明。";
    public string DescriptionEn => "Show list of available commands or detailed syntax for a specific command.";
    public string UsageSyntax => "/help [commandName]";

    public IReadOnlyList<CommandParameter> Parameters => new[]
    {
        new CommandParameter("commandName", CommandParameterType.String, "欲查詢的指令名稱 (省略則顯示全部)", "Command name to inspect", IsRequired: false)
    };

    internal HelpCommand(CommandRegistry registry)
    {
        _registry = registry ?? throw new ArgumentNullException(nameof(registry));
    }

    public CommandResult Execute(ParsedCommand command, CommandExecutionContext context)
    {
        string? target = command.GetPositional(0);

        if (string.IsNullOrWhiteSpace(target))
        {
            var list = _registry.GetAllCommands().OrderBy(c => c.Category).ThenBy(c => c.Name).ToList();
            context.Log("=== Against Rome Map Editor Macro Console - 指令列表 ===", LogLevel.Info);
            string currentCategory = "";

            foreach (var cmd in list)
            {
                if (cmd.Category != currentCategory)
                {
                    currentCategory = cmd.Category;
                    context.Log($"\n[{currentCategory}]", LogLevel.Debug);
                }
                context.Log($"  /{cmd.Name,-16} : {cmd.DescriptionZh}", LogLevel.Info);
            }

            context.Log("\n輸入 /help <command> 檢視特定指令之詳細語法與參數說明。", LogLevel.Info);
            return CommandResult.Ok($"已列出 {list.Count} 個可用指令。", list.Count);
        }

        if (_registry.TryGetCommand(target, out IEditorCommand? found) && found is not null)
        {
            context.Log($"\n指令: /{found.Name} (分類: {found.Category})", LogLevel.Info);
            if (found.Aliases.Count > 0)
            {
                context.Log($"別名: {string.Join(", ", found.Aliases.Select(a => "/" + a))}", LogLevel.Debug);
            }
            context.Log($"說明: {found.DescriptionZh} ({found.DescriptionEn})", LogLevel.Info);
            context.Log($"語法: {found.UsageSyntax}", LogLevel.Success);

            if (found.Parameters.Count > 0)
            {
                context.Log("參數規格:", LogLevel.Info);
                foreach (var p in found.Parameters)
                {
                    string req = p.IsRequired ? "[必填]" : "[可選]";
                    string def = p.DefaultValue is not null ? $" (預設: {p.DefaultValue})" : "";
                    context.Log($"  --{p.Name,-12} ({p.Type}) {req}: {p.DescriptionZh}{def}", LogLevel.Info);
                }
            }
            return CommandResult.Ok($"已顯示 /{found.Name} 之詳細說明。");
        }

        return CommandResult.Fail($"未找到指令「{target}」。");
    }
}

/// <summary>
/// 訊息回顯與文字輸出指令 (/echo)。
/// </summary>
public sealed class EchoCommand : IEditorCommand
{
    public string Name => "echo";
    public IReadOnlyList<string> Aliases => new[] { "print" };
    public string Category => "System";
    public string DescriptionZh => "在控制台輸出指定訊息文字（支援 $var 變數解析）。";
    public string DescriptionEn => "Print a message or variable value to console.";
    public string UsageSyntax => "/echo <message>";

    public IReadOnlyList<CommandParameter> Parameters => new[]
    {
        new CommandParameter("message", CommandParameterType.String, "欲輸出的文字內容", "Message to display", IsRequired: true)
    };

    public CommandResult Execute(ParsedCommand command, CommandExecutionContext context)
    {
        string message = string.Join(" ", command.PositionalArgs);
        context.Log(message, LogLevel.Info);
        return CommandResult.Ok(message);
    }
}

/// <summary>
/// 執行外部巨集檔案指令 (/run)。
/// </summary>
public sealed class RunMacroCommand : IEditorCommand
{
    public string Name => "run";
    public IReadOnlyList<string> Aliases => new[] { "exec", "macro" };
    public string Category => "System";
    public string DescriptionZh => "執行外部 .armcmd 巨集腳本檔案，支援自訂引數與單一 Undo 交易封裝。";
    public string DescriptionEn => "Execute an external .armcmd script file with optional arguments and single undo encapsulation.";
    public string UsageSyntax => "/run <scriptPath> [args...]";

    public IReadOnlyList<CommandParameter> Parameters => new[]
    {
        new CommandParameter("scriptPath", CommandParameterType.String, "欲執行的 .armcmd 檔案路徑", "Path to script file", IsRequired: true),
        new CommandParameter("args", CommandParameterType.String, "傳遞給腳本之引數 ($1, $2...)", "Script arguments", IsRequired: false)
    };

    public CommandResult Execute(ParsedCommand command, CommandExecutionContext context)
    {
        if (context.ScriptRunner is null)
        {
            return CommandResult.Fail("巨集執行器 (MacroScriptRunner) 未注入於上下文。");
        }

        string? scriptPath = command.GetPositional(0);
        if (string.IsNullOrWhiteSpace(scriptPath))
        {
            return CommandResult.Fail("語法錯誤。請指定欲執行的腳本路徑。用法: /run <scriptPath> [args...]");
        }

        if (!File.Exists(scriptPath))
        {
            return CommandResult.Fail($"找不到巨集腳本檔案: {scriptPath}");
        }

        string scriptContent;
        try
        {
            scriptContent = File.ReadAllText(scriptPath);
        }
        catch (Exception ex)
        {
            return CommandResult.Fail($"讀取腳本檔案失敗: {ex.Message}", ex);
        }

        var scriptArgs = command.PositionalArgs.Skip(1).ToList();
        var report = context.ScriptRunner.ExecuteScript(scriptContent, context, scriptPath, scriptArgs);

        return report.Success
            ? CommandResult.Ok($"腳本「{Path.GetFileName(scriptPath)}」執行成功 (影響 {report.AffectedElements} 項目)。", report.AffectedElements)
            : CommandResult.Fail($"腳本「{Path.GetFileName(scriptPath)}」執行失敗: {report.ErrorMessage}");
    }
}

/// <summary>
/// 復原上一筆巨集或指令 (/undo)。
/// </summary>
public sealed class UndoCommand : IEditorCommand
{
    public string Name => "undo";
    public IReadOnlyList<string> Aliases => new[] { "u" };
    public string Category => "System";
    public string DescriptionZh => "撤銷上一筆巨集批次操作或複合指令。";
    public string DescriptionEn => "Undo the last macro operation or compound command.";
    public string UsageSyntax => "/undo";
    public IReadOnlyList<CommandParameter> Parameters => Array.Empty<CommandParameter>();

    public CommandResult Execute(ParsedCommand command, CommandExecutionContext context)
    {
        if (context.ScriptRunner is null) return CommandResult.Fail("巨集執行器未就緒。");
        if (!context.ScriptRunner.CanUndo) return CommandResult.Fail("無可撤銷之巨集歷史。");

        var undone = context.ScriptRunner.Undo();
        return CommandResult.Ok($"已撤銷巨集「{undone?.MacroName}」({undone?.StepCount} 個子步驟)。");
    }
}

/// <summary>
/// 重做上一筆巨集或指令 (/redo)。
/// </summary>
public sealed class RedoCommand : IEditorCommand
{
    public string Name => "redo";
    public IReadOnlyList<string> Aliases => new[] { "r" };
    public string Category => "System";
    public string DescriptionZh => "重做上一筆撤銷的巨集批次操作。";
    public string DescriptionEn => "Redo the last undone macro operation.";
    public string UsageSyntax => "/redo";
    public IReadOnlyList<CommandParameter> Parameters => Array.Empty<CommandParameter>();

    public CommandResult Execute(ParsedCommand command, CommandExecutionContext context)
    {
        if (context.ScriptRunner is null) return CommandResult.Fail("巨集執行器未就緒。");
        if (!context.ScriptRunner.CanRedo) return CommandResult.Fail("無可重做之巨集歷史。");

        var redone = context.ScriptRunner.Redo();
        return CommandResult.Ok($"已重做巨集「{redone?.MacroName}」({redone?.StepCount} 個子步驟)。");
    }
}

/// <summary>
/// 清除控制台螢幕內容指令 (/clear)。
/// </summary>
public sealed class ClearCommand : IEditorCommand
{
    public string Name => "clear";
    public IReadOnlyList<string> Aliases => new[] { "cls" };
    public string Category => "System";
    public string DescriptionZh => "清除控制台畫面日誌文字。";
    public string DescriptionEn => "Clear console output.";
    public string UsageSyntax => "/clear";
    public IReadOnlyList<CommandParameter> Parameters => Array.Empty<CommandParameter>();

    public CommandResult Execute(ParsedCommand command, CommandExecutionContext context)
    {
        context.Log("__CLEAR_CONSOLE__", LogLevel.Debug);
        return CommandResult.Ok("已清除螢幕。");
    }
}
