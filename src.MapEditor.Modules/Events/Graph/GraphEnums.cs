namespace AgainstRomeMapEditor.Modules.Events.Graph;

public enum PortDirection
{
    Input,
    Output
}

public enum PortType
{
    /// <summary>邏輯執行流（白色/藍色箭頭連線）</summary>
    Execution,
    /// <summary>條件信號流（綠色圓點連線）</summary>
    Condition,
    /// <summary>資料傳遞（數值/字串/GUID）</summary>
    Data
}

public enum GraphDiagnosticSeverity
{
    Info,
    Warning,
    Error
}

public sealed record GraphDiagnostic(
    GraphDiagnosticSeverity Severity,
    string Code,
    string Message,
    Guid? NodeId = null,
    Guid? PortId = null,
    Guid? EdgeId = null);
