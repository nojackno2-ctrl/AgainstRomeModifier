namespace AgainstRomeMapEditor.Modules.Scripting.Execution;

/// <summary>
/// 交易型複合巨集指令：將腳本批次執行的所有個別子操作打包為單一 Undo/Redo 單元。
/// 支援完全原子性的 Rollback 回滾與單鍵撤銷/重做。
/// </summary>
public sealed class CompoundMacroCommand
{
    private readonly List<Action> _undoActions = new();
    private readonly List<Action> _redoActions = new();

    public string MacroName { get; }
    public string Description { get; }
    public DateTime CreatedAt { get; } = DateTime.UtcNow;
    public int StepCount => _undoActions.Count;

    public CompoundMacroCommand(string macroName, string description = "")
    {
        MacroName = macroName;
        Description = string.IsNullOrWhiteSpace(description) ? $"Macro: {macroName}" : description;
    }

    /// <summary>
    /// 註冊單一步驟的撤銷與重做操作。
    /// </summary>
    public void RegisterStep(Action undoAction, Action redoAction)
    {
        ArgumentNullException.ThrowIfNull(undoAction);
        ArgumentNullException.ThrowIfNull(redoAction);

        _undoActions.Add(undoAction);
        _redoActions.Add(redoAction);
    }

    /// <summary>
    /// 單次無損撤銷（倒序執行所有子步驟之 Undo）。
    /// </summary>
    public void Undo()
    {
        for (int i = _undoActions.Count - 1; i >= 0; i--)
        {
            _undoActions[i].Invoke();
        }
    }

    /// <summary>
    /// 單次無損重做（正序執行所有子步驟之 Redo）。
    /// </summary>
    public void Redo()
    {
        for (int i = 0; i < _redoActions.Count; i++)
        {
            _redoActions[i].Invoke();
        }
    }

    /// <summary>
    /// 交易異常時的強制回滾（撤銷已執行的所有步驟）。
    /// </summary>
    public void Rollback()
    {
        Undo();
    }
}
