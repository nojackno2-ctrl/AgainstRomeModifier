namespace AgainstRomeMapEditor.Modules.Scripting.Models;

/// <summary>
/// 巨集腳本執行後之完整審計報告。
/// </summary>
public sealed record MacroExecutionReport(
    string? ScriptPath,
    bool Success,
    int TotalCommands,
    int SucceededCommands,
    int FailedCommands,
    long ElapsedMilliseconds,
    int AffectedElements,
    bool WasRolledBack,
    string? ErrorMessage,
    IReadOnlyList<(string Message, LogLevel Level)> Logs);
