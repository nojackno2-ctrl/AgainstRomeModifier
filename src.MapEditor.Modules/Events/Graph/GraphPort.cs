namespace AgainstRomeMapEditor.Modules.Events.Graph;

/// <summary>節點連接埠，定義資料或控制信號的出入口。</summary>
public sealed class GraphPort
{
    public Guid Id { get; init; } = Guid.NewGuid();
    public Guid NodeId { get; set; }
    public string Name { get; init; } = "";
    public string DisplayName { get; init; } = "";
    public PortType Type { get; init; }
    public PortDirection Direction { get; init; }
    public bool AllowMultiple { get; init; }

    public GraphPort() { }

    public GraphPort(Guid nodeId, string name, string displayName, PortType type, PortDirection direction, bool allowMultiple = false)
    {
        NodeId = nodeId;
        Name = name;
        DisplayName = displayName;
        Type = type;
        Direction = direction;
        AllowMultiple = allowMultiple;
    }

    /// <summary>驗證此連接埠是否能與另一連接埠建立連線。</summary>
    public bool CanConnectTo(GraphPort other)
    {
        if (other is null) return false;
        if (NodeId == other.NodeId) return false; // 禁止自己連自己節點
        if (Direction == other.Direction) return false; // 必須一進一出
        if (Type != other.Type) return false; // 類型必須相容
        return true;
    }
}
