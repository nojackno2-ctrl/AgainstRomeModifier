namespace AgainstRomeMapEditor.Modules.Events.Graph;

/// <summary>邏輯節點抽象基底類別。</summary>
public abstract class GraphNode
{
    public Guid Id { get; init; } = Guid.NewGuid();
    public virtual string Title { get; set; } = "Node";
    public virtual string Category { get; set; } = "General";
    public float X { get; set; }
    public float Y { get; set; }
    public List<GraphPort> Ports { get; init; } = new();

    public GraphPort? FindPort(Guid portId) => Ports.FirstOrDefault(p => p.Id == portId);
    public GraphPort? FindPort(string name) => Ports.FirstOrDefault(p => p.Name.Equals(name, StringComparison.OrdinalIgnoreCase));

    protected GraphPort AddPort(string name, string displayName, PortType type, PortDirection direction, bool allowMultiple = false)
    {
        var port = new GraphPort(Id, name, displayName, type, direction, allowMultiple);
        Ports.Add(port);
        return port;
    }

    public abstract GraphNode Clone();
}
