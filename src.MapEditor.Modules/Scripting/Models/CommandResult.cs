namespace AgainstRomeMapEditor.Modules.Scripting.Models;

/// <summary>
/// 指令或巨集執行結果模型。
/// </summary>
public sealed record CommandResult(
    bool Success,
    string Message,
    int AffectedCount = 0,
    IReadOnlyList<string>? Details = null,
    Exception? Error = null)
{
    public static CommandResult Ok(string message, int affectedCount = 0, IReadOnlyList<string>? details = null) =>
        new(true, message, affectedCount, details);

    public static CommandResult Fail(string message, Exception? error = null, IReadOnlyList<string>? details = null) =>
        new(false, message, 0, details, error);
}
