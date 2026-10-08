namespace AgainstRomeMapEditor.Modules.Scripting.Models;

/// <summary>
/// 指令參數型別列舉。
/// </summary>
public enum CommandParameterType
{
    String,
    Integer,
    Float,
    Boolean,
    Rectangle,
    Circle,
    TextureName,
    TemplateName,
    TeamId,
    Flag
}

/// <summary>
/// 指令參數定義中繼資料，供解析器驗證、自動補全與說明文件使用。
/// </summary>
public sealed record CommandParameter(
    string Name,
    CommandParameterType Type,
    string DescriptionZh,
    string DescriptionEn,
    bool IsRequired = true,
    string? DefaultValue = null,
    IReadOnlyList<string>? PossibleValues = null);
