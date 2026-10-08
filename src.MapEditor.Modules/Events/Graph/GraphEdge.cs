namespace AgainstRomeMapEditor.Modules.Events.Graph;

/// <summary>節點連線，連接來源埠與目標埠。</summary>
public sealed record GraphEdge(
    Guid SourceNodeId,
    Guid SourcePortId,
    Guid TargetNodeId,
    Guid TargetPortId)
{
    public Guid Id { get; init; } = Guid.NewGuid();
}
