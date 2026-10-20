// Minimal stand-ins so RoadPathHealer.cs compiles without the editor session types;
// the generator never calls ApplyRepair.
public enum TerrainCollisionOperation { Clear, Block }
public sealed class TerrainBlendEditSession
{
    public int TileDimension => 64;
    public object? StampTexture(int x, int z, string texture) => null;
    public void CommitStroke() { }
}
public sealed class TerrainHeightEditSession
{
    public bool HasCollision => false;
    public int CollisionSize => 0;
    public List<int> PaintCollision(float x, float z, float r, TerrainCollisionOperation op) => new();
    public void CommitStroke() { }
}
