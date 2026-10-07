namespace AgainstRomeMapEditor;

/// <summary>Pure native tile lookup boundary; bitmap loading and catalog construction belong to the host.</summary>
internal interface INativeTerrainMaterialResolver
{
    bool TryResolveNativeCorners(string texture, out IReadOnlyList<string> corners);
    string? ResolveNativeTile(IReadOnlyList<string> cornerMaterialIds, int x, int y);
}
