using System.Diagnostics;
using AgainstRomeMapEditor.Modules.Scripting.Models;
using AgainstRomeMapEditor.Modules.Scripting.Parsing;

namespace AgainstRomeMapEditor.Modules.Scripting.Execution;

/// <summary>
/// 批次巨集腳本執行器：負責執行 .armcmd 檔案、處理變數替換、多命令交易打包與自動 Rollback。
/// </summary>
internal sealed class MacroScriptRunner
{
    private readonly Stack<CompoundMacroCommand> _undoStack = new();
    private readonly Stack<CompoundMacroCommand> _redoStack = new();

    public CommandRegistry Registry { get; }

    public bool CanUndo => _undoStack.Count > 0;
    public bool CanRedo => _redoStack.Count > 0;

    public MacroScriptRunner(CommandRegistry registry)
    {
        Registry = registry ?? throw new ArgumentNullException(nameof(registry));
    }

    /// <summary>
    /// 撤銷上一筆巨集或複合指令。
    /// </summary>
    public CompoundMacroCommand? Undo()
    {
        if (_undoStack.Count == 0) return null;
        CompoundMacroCommand compound = _undoStack.Pop();
        compound.Undo();
        _redoStack.Push(compound);
        return compound;
    }

    /// <summary>
    /// 重做上一筆巨集或複合指令。
    /// </summary>
    public CompoundMacroCommand? Redo()
    {
        if (_redoStack.Count == 0) return null;
        CompoundMacroCommand compound = _redoStack.Pop();
        compound.Redo();
        _undoStack.Push(compound);
        return compound;
    }

    /// <summary>
    /// 執行單行指令文字。若啟用 recordSingleActionAsCompound，該單行操作會獨立封裝入 Undo 歷史。
    /// </summary>
    public CommandResult ExecuteLine(
        string line,
        CommandExecutionContext context,
        bool recordSingleActionAsCompound = false)
    {
        ArgumentNullException.ThrowIfNull(line);
        ArgumentNullException.ThrowIfNull(context);

        string trimmed = line.Trim();
        if (string.IsNullOrWhiteSpace(trimmed) || trimmed.StartsWith('#') || trimmed.StartsWith("//", StringComparison.Ordinal))
        {
            return CommandResult.Ok("忽略空白或註解行。");
        }

        // 1. 處理變數賦值語法: $varName = value
        if (trimmed.StartsWith('$') && trimmed.Contains('='))
        {
            int eqIdx = trimmed.IndexOf('=');
            string varName = trimmed[1..eqIdx].Trim();
            string varVal = trimmed[(eqIdx + 1)..].Trim();
            if (varVal.StartsWith('"') && varVal.EndsWith('"') && varVal.Length >= 2)
            {
                varVal = varVal[1..^1];
            }
            context.Variables[varName] = varVal;
            context.Log($"[VAR] 定義變數 ${varName} = {varVal}", LogLevel.Debug);
            return CommandResult.Ok($"已設定變數 ${varName} = {varVal}", 1);
        }

        // 2. 解析指令
        ParsedCommand? parsed = EditorCommandParser.Parse(trimmed, context.Variables);
        if (parsed is null)
        {
            return CommandResult.Ok("無可執行指令。");
        }

        if (!Registry.TryGetCommand(parsed.CommandName, out IEditorCommand? command) || command is null)
        {
            string err = $"未知指令: /{parsed.CommandName}。輸入 /help 查詢可用指令列表。";
            context.Log(err, LogLevel.Error);
            return CommandResult.Fail(err);
        }

        // 3. 封裝與執行
        CompoundMacroCommand? singleCompound = recordSingleActionAsCompound ? new CompoundMacroCommand(parsed.CommandName, parsed.RawText) : null;
        var prevRecorder = context.StepRecorder;

        try
        {
            if (singleCompound is not null)
            {
                context.StepRecorder = (u, r) => singleCompound.RegisterStep(u, r);
            }

            CommandResult result = command.Execute(parsed, context);
            if (result.Success)
            {
                if (singleCompound is not null && singleCompound.StepCount > 0)
                {
                    _undoStack.Push(singleCompound);
                    _redoStack.Clear();
                }
                context.Log($"[OK] /{parsed.CommandName}: {result.Message}", LogLevel.Success);
            }
            else
            {
                if (singleCompound is not null && singleCompound.StepCount > 0)
                {
                    singleCompound.Rollback();
                }
                context.Log($"[FAIL] /{parsed.CommandName}: {result.Message}", LogLevel.Error);
            }

            return result;
        }
        catch (Exception ex)
        {
            if (singleCompound is not null && singleCompound.StepCount > 0)
            {
                singleCompound.Rollback();
            }
            string err = $"執行指令 /{parsed.CommandName} 時發生未預期例外: {ex.Message}";
            context.Log(err, LogLevel.Error);
            return CommandResult.Fail(err, ex);
        }
        finally
        {
            context.StepRecorder = prevRecorder;
        }
    }

    /// <summary>
    /// 執行 .armcmd 巨集腳本字串。支援單次交易打包與原子性回滾。
    /// </summary>
    public MacroExecutionReport ExecuteScript(
        string scriptContent,
        CommandExecutionContext context,
        string? scriptPath = null,
        IReadOnlyList<string>? scriptArgs = null,
        bool transactional = true)
    {
        ArgumentNullException.ThrowIfNull(scriptContent);
        ArgumentNullException.ThrowIfNull(context);

        var stopwatch = Stopwatch.StartNew();
        var logs = new List<(string Message, LogLevel Level)>();

        void CaptureLog(string msg, LogLevel lvl)
        {
            logs.Add((msg, lvl));
        }

        var prevHandler = context.OutputHandler;
        context.OutputHandler = (msg, lvl) =>
        {
            CaptureLog(msg, lvl);
            prevHandler?.Invoke(msg, lvl);
        };

        string macroName = string.IsNullOrWhiteSpace(scriptPath) ? "Macro_" + DateTime.Now.ToString("HHmmss") : Path.GetFileNameWithoutExtension(scriptPath);
        string description = $"執行腳本 {macroName}";
        var compound = new CompoundMacroCommand(macroName, description);

        var prevRecorder = context.StepRecorder;
        if (transactional)
        {
            context.StepRecorder = (u, r) => compound.RegisterStep(u, r);
        }

        // 參數注入: $1, $2, ...
        if (scriptArgs is not null)
        {
            for (int i = 0; i < scriptArgs.Count; i++)
            {
                context.Variables[(i + 1).ToString()] = scriptArgs[i];
            }
            context.Variables["0"] = macroName;
        }

        string[] lines = scriptContent.Split(new[] { "\r\n", "\r", "\n" }, StringSplitOptions.None);
        int totalCommands = 0;
        int succeededCommands = 0;
        int failedCommands = 0;
        int affectedElements = 0;
        bool wasRolledBack = false;
        string? failureError = null;

        context.Log($"開始執行巨集「{macroName}」，共 {lines.Length} 行腳本...", LogLevel.Info);

        try
        {
            for (int lineIndex = 0; lineIndex < lines.Length; lineIndex++)
            {
                string rawLine = lines[lineIndex].Trim();
                if (string.IsNullOrWhiteSpace(rawLine)) continue;

                // 註解與中繼指令讀取
                if (rawLine.StartsWith('#') || rawLine.StartsWith("//", StringComparison.Ordinal))
                {
                    if (rawLine.StartsWith("# @name", StringComparison.OrdinalIgnoreCase))
                    {
                        macroName = rawLine["# @name".Length..].Trim();
                    }
                    continue;
                }

                totalCommands++;
                CommandResult res = ExecuteLine(rawLine, context, recordSingleActionAsCompound: false);

                if (res.Success)
                {
                    succeededCommands++;
                    affectedElements += res.AffectedCount;
                }
                else
                {
                    failedCommands++;
                    failureError = $"行 {lineIndex + 1}: {res.Message}";

                    if (transactional)
                    {
                        context.Log($"巨集於第 {lineIndex + 1} 行中斷失敗: {res.Message}。正在執行原子性回滾...", LogLevel.Warning);
                        compound.Rollback();
                        wasRolledBack = true;
                        break;
                    }
                }
            }

            if (!wasRolledBack && compound.StepCount > 0)
            {
                _undoStack.Push(compound);
                _redoStack.Clear();
            }
        }
        catch (Exception ex)
        {
            failedCommands++;
            failureError = $"巨集執行發生例外: {ex.Message}";
            if (transactional)
            {
                compound.Rollback();
                wasRolledBack = true;
            }
        }
        finally
        {
            context.StepRecorder = prevRecorder;
            context.OutputHandler = prevHandler;
            stopwatch.Stop();
        }

        bool overallSuccess = failedCommands == 0 && !wasRolledBack;
        string summary = overallSuccess
            ? $"巨集「{macroName}」執行成功！總步驟: {succeededCommands}/{totalCommands}，影響項目: {affectedElements}，耗時: {stopwatch.ElapsedMilliseconds} ms。"
            : $"巨集「{macroName}」執行失敗！失敗步驟: {failedCommands}，已回滾: {(wasRolledBack ? "是" : "否")}，原因: {failureError}";

        context.Log(summary, overallSuccess ? LogLevel.Success : LogLevel.Error);

        return new MacroExecutionReport(
            scriptPath,
            overallSuccess,
            totalCommands,
            succeededCommands,
            failedCommands,
            stopwatch.ElapsedMilliseconds,
            affectedElements,
            wasRolledBack,
            failureError,
            logs);
    }
}
