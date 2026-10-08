namespace AgainstRomeMapEditor.Modules.Events.Graph;

/// <summary>劇情事件節點圖邏輯容器。</summary>
public sealed class EventGraph
{
    private readonly List<GraphNode> _nodes = new();
    private readonly List<GraphEdge> _edges = new();

    public IReadOnlyList<GraphNode> Nodes => _nodes;
    public IReadOnlyList<GraphEdge> Edges => _edges;

    public void AddNode(GraphNode node)
    {
        ArgumentNullException.ThrowIfNull(node);
        if (_nodes.Any(n => n.Id == node.Id))
            throw new InvalidOperationException($"節點 ID {node.Id} 已存在於圖中。");
        _nodes.Add(node);
    }

    public bool RemoveNode(Guid nodeId)
    {
        int index = _nodes.FindIndex(n => n.Id == nodeId);
        if (index < 0) return false;

        // 同步移除所有連至或源自此節點的連線
        _edges.RemoveAll(e => e.SourceNodeId == nodeId || e.TargetNodeId == nodeId);
        _nodes.RemoveAt(index);
        return true;
    }

    public GraphNode? FindNode(Guid nodeId) => _nodes.FirstOrDefault(n => n.Id == nodeId);

    public GraphPort? FindPort(Guid portId)
    {
        foreach (var node in _nodes)
        {
            var port = node.FindPort(portId);
            if (port is not null) return port;
        }
        return null;
    }

    public bool Connect(Guid sourcePortId, Guid targetPortId, out GraphEdge? edge, out string? error)
    {
        edge = null;
        var p1 = FindPort(sourcePortId);
        var p2 = FindPort(targetPortId);

        if (p1 is null || p2 is null)
        {
            error = "找不到指定的連接埠。";
            return false;
        }

        return Connect(p1, p2, out edge, out error);
    }

    public bool Connect(GraphPort sourcePort, GraphPort targetPort, out GraphEdge? edge, out string? error)
    {
        edge = null;
        error = null;

        // 確保方向為 Output -> Input
        GraphPort from = sourcePort;
        GraphPort to = targetPort;
        if (from.Direction == PortDirection.Input && to.Direction == PortDirection.Output)
        {
            (from, to) = (to, from);
        }

        if (!from.CanConnectTo(to))
        {
            error = $"無法連接連接埠：{from.Name} ({from.Type}/{from.Direction}) 與 {to.Name} ({to.Type}/{to.Direction}) 不相容。";
            return false;
        }

        // 檢查多重連線限制
        if (!from.AllowMultiple && _edges.Any(e => e.SourcePortId == from.Id))
        {
            error = $"連接埠 {from.Name} 不允許輸出多條連線。";
            return false;
        }

        if (!to.AllowMultiple && _edges.Any(e => e.TargetPortId == to.Id))
        {
            error = $"連接埠 {to.Name} 不允許輸入多條連線。";
            return false;
        }

        // 避免重複連線
        if (_edges.Any(e => e.SourcePortId == from.Id && e.TargetPortId == to.Id))
        {
            error = "連線已存在。";
            return false;
        }

        edge = new GraphEdge(from.NodeId, from.Id, to.NodeId, to.Id);
        _edges.Add(edge);
        return true;
    }

    public bool Disconnect(Guid edgeId)
    {
        int count = _edges.RemoveAll(e => e.Id == edgeId);
        return count > 0;
    }

    public bool Disconnect(Guid portAId, Guid portBId)
    {
        int count = _edges.RemoveAll(e =>
            (e.SourcePortId == portAId && e.TargetPortId == portBId) ||
            (e.SourcePortId == portBId && e.TargetPortId == portAId));
        return count > 0;
    }

    public IReadOnlyList<GraphEdge> GetIncomingEdges(Guid portId) =>
        _edges.Where(e => e.TargetPortId == portId).ToList();

    public IReadOnlyList<GraphEdge> GetOutgoingEdges(Guid portId) =>
        _edges.Where(e => e.SourcePortId == portId).ToList();

    public IReadOnlyList<GraphEdge> GetNodeEdges(Guid nodeId) =>
        _edges.Where(e => e.SourceNodeId == nodeId || e.TargetNodeId == nodeId).ToList();

    public void Clear()
    {
        _edges.Clear();
        _nodes.Clear();
    }
}
