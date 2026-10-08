using AgainstRomeMapEditor.Modules.Scripting.Models;

namespace AgainstRomeMapEditor.Modules.Scripting.Execution;

/// <summary>
/// 地圖編輯器命令抽象介面。
/// </summary>
public interface IEditorCommand
{
    /// <summary>主要指令名稱 (不含前綴 '/')。</summary>
    string Name { get; }

    /// <summary>指令別名列表。</summary>
    IReadOnlyList<string> Aliases { get; }

    /// <summary>所屬分類 (Terrain, Placement, Nature, Pathfinding, Diagnostics, System)。</summary>
    string Category { get; }

    /// <summary>繁體中文功能說明。</summary>
    string DescriptionZh { get; }

    /// <summary>英文功能說明。</summary>
    string DescriptionEn { get; }

    /// <summary>呼叫語法示範。</summary>
    string UsageSyntax { get; }

    /// <summary>參數中繼資料清單。</summary>
    IReadOnlyList<CommandParameter> Parameters { get; }

    /// <summary>執行指令邏輯。</summary>
    CommandResult Execute(ParsedCommand command, CommandExecutionContext context);
}
